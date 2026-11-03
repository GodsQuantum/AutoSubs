//! Cross-platform native system tray for a single AutoSubs desktop process.
//! The app owns its backend child; the tray does not start any external daemon.
#[cfg(target_os = "windows")]
use super::APP_NAME;
use super::open_browser;
use std::{
    env,
    process::Child,
    time::{Duration, Instant},
};
#[cfg(not(target_os = "windows"))]
use std::{fs, path::PathBuf};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
};

enum Signal {
    Menu(MenuEvent),
}

#[cfg(not(target_os = "windows"))]
fn autostart_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME").ok_or("HOME unavailable")?;
    #[cfg(target_os = "macos")]
    return Ok(
        PathBuf::from(home).join("Library/LaunchAgents/io.github.GodsQuantum.AutoSubs.plist")
    );
    #[cfg(not(target_os = "macos"))]
    return Ok(PathBuf::from(home).join(".config/autostart/autosubs.desktop"));
}

fn autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
            .ok()
            .and_then(|key| key.get_value::<String, _>(APP_NAME).ok())
            .is_some()
    }
    #[cfg(not(target_os = "windows"))]
    {
        autostart_path().is_ok_and(|path| path.is_file())
    }
}

fn set_autostart(enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};
        let path = r"Software\Microsoft\Windows\CurrentVersion\Run";
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(path)
            .map_err(|e| e.to_string())?;
        if enable {
            let exe = env::current_exe().map_err(|e| e.to_string())?;
            key.set_value(APP_NAME, &format!("\"{}\" --no-browser", exe.display()))
                .map_err(|e| e.to_string())?;
        } else {
            let _ = key.delete_value(APP_NAME);
        }
        return Ok(());
    }
    #[cfg(not(target_os = "windows"))]
    {
        let target = autostart_path()?;
        if !enable {
            if target.exists() {
                fs::remove_file(target).map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        let exe = env::current_exe().map_err(|e| e.to_string())?;
        fs::create_dir_all(target.parent().ok_or("Invalid autostart parent")?)
            .map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        {
            fn xml_escape(value: &str) -> String {
                value
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
            }
            let body = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                 <plist version=\"1.0\"><dict>\
                 <key>Label</key><string>io.github.GodsQuantum.AutoSubs</string>\
                 <key>ProgramArguments</key><array><string>{}</string><string>--no-browser</string></array>\
                 <key>RunAtLoad</key><true/></dict></plist>\n",
                xml_escape(&exe.to_string_lossy())
            );
            fs::write(target, body).map_err(|e| e.to_string())?;
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Desktop Entry double-quote escaping; no shell is involved.
            let escaped = exe
                .to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            let body = format!(
                "[Desktop Entry]\nType=Application\nName=AutoSubs\n\
                 Exec=\"{escaped}\" --no-browser\nTerminal=false\n\
                 X-GNOME-Autostart-enabled=true\n"
            );
            fs::write(target, body).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

fn application_icon() -> Result<Icon, String> {
    // Embed the already shipped AutoSubs icon: no extra file or copy at runtime.
    let bitmap =
        image::load_from_memory(include_bytes!("../../../../frontend/static/icon-192.png"))
            .map_err(|e| e.to_string())?
            .into_rgba8();
    let (width, height) = bitmap.dimensions();
    Icon::from_rgba(bitmap.into_raw(), width, height).map_err(|e| e.to_string())
}

// Closing the tray or an error during GUI initialization must never leave a
// headless backend listening on the desktop port.
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Ok(None) = self.0.try_wait() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

struct TrayApp {
    port: u16,
    child: OwnedChild,
    menu: Menu,
    open: MenuItem,
    startup: MenuItem,
    quit: MenuItem,
    icon: Icon,
    tray: Option<TrayIcon>,
    last_health: Instant,
}

impl ApplicationHandler<Signal> for TrayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.tray.is_some() {
            return;
        }
        match TrayIconBuilder::new()
            .with_icon(self.icon.clone())
            .with_menu(Box::new(self.menu.clone()))
            .with_tooltip("AutoSubs — rendu de sous-titres")
            .build()
        {
            Ok(icon) => self.tray = Some(icon),
            Err(error) => {
                eprintln!("AutoSubs tray creation failed: {error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _id: winit::window::WindowId,
        _event: WindowEvent,
    ) {
        // This application has no native top-level window; UI runs in browser.
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Signal) {
        let Signal::Menu(event) = event;
        if event.id == self.open.id() {
            if let Err(error) = open_browser(self.port) {
                eprintln!("AutoSubs open browser failed: {error}");
            }
        } else if event.id == self.startup.id() {
            if let Err(error) = set_autostart(!autostart_enabled()) {
                eprintln!("AutoSubs startup preference: {error}");
            }
            self.startup.set_text(if autostart_enabled() {
                "Désactiver le lancement automatique"
            } else {
                "Activer le lancement automatique"
            });
        } else if event.id == self.quit.id() {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_secs(2),
        ));
        if self.last_health.elapsed() >= Duration::from_secs(2) {
            self.last_health = Instant::now();
            match self.child.0.try_wait() {
                Ok(Some(_)) => event_loop.exit(),
                Err(error) => {
                    eprintln!("AutoSubs backend status failed: {error}");
                    event_loop.exit();
                }
                _ => {}
            }
        }
    }
}

pub fn run(port: u16, child: Child) -> Result<(), String> {
    let child = OwnedChild(child); // guards every early failure path
    let event_loop = EventLoop::<Signal>::with_user_event()
        .build()
        .map_err(|error| error.to_string())?;
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(Signal::Menu(event));
    }));
    let menu = Menu::new();
    let open = MenuItem::new("Ouvrir AutoSubs", true, None);
    let startup = MenuItem::new("Lancement automatique", true, None);
    let quit = MenuItem::new("Quitter AutoSubs", true, None);
    menu.append_items(&[&open, &startup, &quit])
        .map_err(|error| error.to_string())?;
    startup.set_text(if autostart_enabled() {
        "Désactiver le lancement automatique"
    } else {
        "Activer le lancement automatique"
    });
    let mut app = TrayApp {
        port,
        child,
        menu,
        open,
        startup,
        quit,
        icon: application_icon()?,
        tray: None,
        last_health: Instant::now(),
    };
    event_loop
        .run_app(&mut app)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn owned_child_is_reaped_on_drop_if_the_tray_cannot_start() {
        use std::path::Path;
        use std::process::Command;
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("test process");
        let pid = child.id();
        {
            let _child_guard = OwnedChild(child);
        }
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "orphan child process"
        );
    }

    #[test]
    fn desktop_tray_uses_autosubs_identity_not_other_apps() {
        let icon = application_icon().expect("AutoSubs image embedded");
        // Icon constructed successfully from the canonical AutoSubs PNG.
        let _ = icon;
        assert_eq!(crate::APP_NAME, "AutoSubs");
    }
}
