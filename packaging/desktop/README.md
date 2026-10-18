# AutoSubs desktop installer candidates

Sources: native Rust 1.98 + existing SvelteKit. This packaging wraps the real server,
without Docker, Python or a separate browser extension. The launcher opens the app
in the system's default browser and runs a loopback-only service.

- Windows 11 AMD64: NSIS per-user setup, includes backend, frontend,
  FFmpeg/FFprobe with verified libass/subtitles support, license, shortcut, clean uninstaller. Keeps user data.
- macOS 15 Apple Silicon and Intel: .app in a mounted .dmg with all resources.
  The candidate uses **ad-hoc** signing only. Production Apple distribution
  requires a Developer ID certificate, hardened signing and notarization.
- Persistent data: Windows LOCALAPPDATA/AutoSubs; macOS
  ~/Library/Application Support/AutoSubs. SQLite remains on local storage.
- The PATH contains only the bundled FFmpeg first, no system PATH mutation.
- Installer checks enforce FFmpeg codecs; AutoSubs additionally requires libass.
- Tests build/assemble the installer on the actual target OS, unpack/install,
  verify launcher's --check-install, launch a real Rust backend with --smoke,
  and exercise clean uninstallation on Windows.
- FFmpeg binaries are pinned by exact SHA256 to source-defined upstream assets, not automatically upgraded;
  bundling its original -L license terms and source URL.
- Unsigned preview artifacts are **NOT** certified as signed/notarized public
  binaries and should not be promoted into a stable release without signing
  credentials and Gatekeeper/SmartScreen testing.
- GPU end-to-end, real audio codec/transcript and complete UX/system tray
  require separate integration tests. CI runners may lack compatible GPUs.
- Cloud9-Speaches pairing is available in the existing app UI; it does
  not imply that Cloud9-Speaches has itself been packaged on Windows/macOS.
