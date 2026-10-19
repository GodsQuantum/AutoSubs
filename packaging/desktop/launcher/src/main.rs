use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

const APP_NAME: &str = "AutoSubs";
const APP_ENV: &str = "AUTOSUBS";
const SERVER_NAME: &str = "autosubs-server";
const HEALTH: &str = "health";
const DEFAULT_PORT: u16 = 3051;

fn resources() -> Result<PathBuf, String> {
    let executable = env::current_exe().map_err(|e| e.to_string())?;
    let parent = executable.parent().ok_or("Missing executable parent")?;
    if cfg!(target_os = "macos") {
        Ok(parent
            .parent()
            .ok_or("Missing .app Contents directory")?
            .join("Resources"))
    } else {
        Ok(parent.to_path_buf())
    }
}

fn data_root() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let path = env::var_os("LOCALAPPDATA")
            .or_else(|| env::var_os("APPDATA"))
            .ok_or("Local application data folder is not configured")?;
        return Ok(PathBuf::from(path).join(APP_NAME));
    }
    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME").ok_or("HOME is not configured")?;
        return Ok(PathBuf::from(home)
            .join("Library/Application Support")
            .join(APP_NAME));
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = env::var_os("HOME").ok_or("HOME is not configured")?;
        Ok(PathBuf::from(home).join(".local/share").join(APP_NAME))
    }
}

fn executable(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn check_binary(file: &Path, arg: &str) -> Result<String, String> {
    let result = Command::new(file)
        .arg(arg)
        .output()
        .map_err(|e| format!("{}: {e}", file.display()))?;
    if !result.status.success() {
        return Err(format!("{} exited with {}", file.display(), result.status));
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}

fn http_health(port: u16) -> Result<String, String> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(800))
        .map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    write!(
        stream,
        "GET /api/v1/{HEALTH} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return Err("The port is occupied by a nonhealthy server".to_owned());
    }
    if !response.contains(&format!("\"version\":\"{}\"", env!("CARGO_PKG_VERSION"))) {
        return Err("The port is occupied by another application/version".to_owned());
    }
    Ok(response)
}

fn open_browser(port: u16) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("explorer.exe");
        c.arg(&url);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(&url);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(&url);
        c
    };
    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn launch(smoke: bool, no_browser: bool) -> Result<(), String> {
    let root = resources()?;
    let server = root.join("bin").join(executable(SERVER_NAME));
    let ffmpeg = root.join("bin").join(executable("ffmpeg"));
    let ffprobe = root.join("bin").join(executable("ffprobe"));
    let frontend = root.join("frontend");
    if !frontend.join("index.html").is_file() {
        return Err(format!("Bundled frontend missing: {}", frontend.display()));
    }
    let version = check_binary(&server, "--version")?;
    if !version.contains(env!("CARGO_PKG_VERSION")) {
        return Err("Backend version does not match installer".into());
    }
    check_binary(&ffmpeg, "-version")?;
    check_binary(&ffprobe, "-version")?;
    if APP_ENV == "AUTOSUBS" {
        let filters = check_binary(&ffmpeg, "-filters")?;
        if !filters.contains("subtitles") || !filters.contains("ass") {
            return Err("Bundled FFmpeg does not provide libass/subtitles filters".into());
        }
    }
    if env::args().any(|arg| arg == "--check-install") {
        println!(
            "DESKTOP_BUNDLE_VALID {} {}",
            APP_NAME,
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }

    let base = data_root()?;
    let config = base.join("config");
    let data = base.join("data");
    let fonts = data.join("fonts");
    for folder in [&base, &config, &data, &fonts] {
        fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    let port = env::var(format!("{APP_ENV}_PORT"))
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    if http_health(port).is_ok() {
        if smoke {
            return Err("A server already occupies the smoke test port".into());
        }
        if !no_browser {
            open_browser(port)?;
        }
        return Ok(());
    }
    if TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], port))).is_ok() {
        return Err(format!("Port {port} is used by another application"));
    }

    let path = env::var_os("PATH").unwrap_or_default();
    let path = env::join_paths(std::iter::once(root.join("bin")).chain(env::split_paths(&path)))
        .map_err(|e| format!("PATH: {e}"))?;
    let userhome = env::var_os(if cfg!(target_os = "windows") {
        "USERPROFILE"
    } else {
        "HOME"
    })
    .ok_or("Unable to determine user home")?;
    let roots = env::join_paths([PathBuf::from(userhome), data.clone()])
        .map_err(|e| format!("Allowed roots: {e}"))?;
    let logpath = base.join("desktop.log");
    let log: File = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&logpath)
        .map_err(|e| e.to_string())?;
    let mut child = Command::new(&server)
        .env(format!("{APP_ENV}_HOST"), "127.0.0.1")
        .env(format!("{APP_ENV}_PORT"), port.to_string())
        .env(format!("{APP_ENV}_CONFIG_DIR"), &config)
        .env(format!("{APP_ENV}_DATA_DIR"), &data)
        .env("AUTOSUBS_FONTS_DIR", &fonts)
        .env(format!("{APP_ENV}_DIST_DIR"), &frontend)
        .env(format!("{APP_ENV}_ALLOWED_ROOTS"), roots)
        .env("PATH", path)
        .current_dir(&root)
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|e| format!("Start native server: {e}"))?;
    let ready = (0..100).any(|_| {
        if http_health(port).is_ok() {
            return true;
        }
        if let Ok(Some(_)) = child.try_wait() {
            return false;
        }
        thread::sleep(Duration::from_millis(250));
        false
    });
    if !ready {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "Server did not become healthy; see {}",
            logpath.display()
        ));
    }
    if smoke {
        child.kill().map_err(|e| e.to_string())?;
        child.wait().map_err(|e| e.to_string())?;
        println!(
            "DESKTOP_SMOKE_OK {} {}",
            APP_NAME,
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if !no_browser {
        open_browser(port)?;
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("Server exited: {status}"));
    }
    Ok(())
}

fn main() {
    let smoke = env::args().any(|arg| arg == "--smoke");
    let no_browser = env::args().any(|arg| arg == "--no-browser");
    if let Err(error) = launch(smoke, no_browser) {
        eprintln!("{APP_NAME}: {error}");
        std::process::exit(1);
    }
}
