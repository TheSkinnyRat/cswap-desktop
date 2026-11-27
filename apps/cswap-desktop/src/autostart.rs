//! "Launch at login" (the `launchAtLogin` setting), started with `--hidden`.
//!
//! - Windows: the HKCU Run value Electron's `setLoginItemSettings` wrote for 0.1.x, under the
//!   SAME name (`electron.app.Claude Swap`, read off the PC: 0.1.x's app name is its
//!   productName) and the same exe path. A 0.1.x install that had it on keeps launching v1
//!   unchanged, and v1 turning it off removes that same value instead of leaving a second.
//! - macOS: a LaunchAgent plist. 0.1.x's Electron login item (SMAppService) cannot be
//!   removed from here without more Objective-C; see the report.
//! - Linux: an XDG autostart entry (0.1.x had none: Electron does not support it there).

use std::path::{Path, PathBuf};

pub const HIDDEN_FLAG: &str = "--hidden";
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const RUN_VALUE: &str = "electron.app.Claude Swap";
pub const MAC_LABEL: &str = "id.my.reinahatsuki.cswap-desktop";

/// What starts this copy at login: the AppImage file rather than its mount point.
pub fn launch_target() -> PathBuf {
    if let Some(a) = std::env::var_os("APPIMAGE").filter(|a| !a.is_empty()) {
        return PathBuf::from(a);
    }
    let exe = std::env::current_exe().unwrap_or_default();
    PathBuf::from(exe.to_string_lossy().trim_start_matches(r"\\?\").to_string())
}

/// Whether a Run value starts `exe`: a value left pointing at another copy does not.
pub fn value_launches(value: &str, exe: &Path) -> bool {
    let exe = exe.to_string_lossy().to_lowercase();
    let v = value.trim().to_lowercase();
    let path = match v.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or(""),
        None => v.as_str(),
    };
    path.trim() == exe
}

pub fn run_value(exe: &Path) -> String {
    format!("\"{}\" {HIDDEN_FLAG}", exe.display())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn mac_plist(exe: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{MAC_LABEL}</string>
  <key>ProgramArguments</key><array><string>{}</string><string>{HIDDEN_FLAG}</string></array>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
"#,
        xml_escape(&exe.to_string_lossy())
    )
}

/// `Exec=` quoting per the Desktop Entry spec: quote the path, escape `"`, `` ` ``, `$`, `\`.
pub fn desktop_entry(exe: &Path) -> String {
    let mut q = String::new();
    for c in exe.to_string_lossy().chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            q.push('\\');
        }
        q.push(c);
    }
    format!(
        "[Desktop Entry]\nType=Application\nName=Claude Swap\nComment=Manage and switch Claude Code accounts\nExec=\"{q}\" {HIDDEN_FLAG}\nIcon=claude-swap\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from)
}

#[allow(dead_code)] // per platform
fn mac_plist_path() -> Option<PathBuf> {
    Some(home()?.join("Library/LaunchAgents").join(format!("{MAC_LABEL}.plist")))
}

#[allow(dead_code)] // per platform
fn linux_entry_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| Some(home()?.join(".config")))?;
    Some(base.join("autostart").join("claude-swap.desktop"))
}

#[cfg(windows)]
pub fn is_enabled() -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>(RUN_VALUE))
        .is_ok_and(|v| value_launches(&v, &launch_target()))
}

#[cfg(windows)]
pub fn set_enabled(on: bool) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if on {
        let (key, _) = hkcu.create_subkey(RUN_KEY).map_err(|e| e.to_string())?;
        key.set_value(RUN_VALUE, &run_value(&launch_target())).map_err(|e| e.to_string())
    } else {
        match hkcu.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE) {
            Ok(key) => match key.delete_value(RUN_VALUE) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            },
            Err(_) => Ok(()),
        }
    }
}

#[cfg(not(windows))]
fn entry_file() -> Option<(PathBuf, String)> {
    let exe = launch_target();
    if cfg!(target_os = "macos") { Some((mac_plist_path()?, mac_plist(&exe))) } else { Some((linux_entry_path()?, desktop_entry(&exe))) }
}

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    entry_file().is_some_and(|(p, _)| p.exists())
}

#[cfg(not(windows))]
pub fn set_enabled(on: bool) -> Result<(), String> {
    let (path, text) = entry_file().ok_or("no home folder")?;
    if on {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    } else {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", path.display())),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_value_matches_0_1_x_and_only_this_exe() {
        let exe = Path::new(r"C:\Users\U\AppData\Local\Programs\cswap\Claude Swap.exe");
        // The value 0.1.23 wrote, as read from the PC (user folder replaced).
        assert!(value_launches(r#""C:\Users\U\AppData\Local\Programs\cswap\Claude Swap.exe" --hidden"#, exe));
        assert_eq!(run_value(exe), r#""C:\Users\U\AppData\Local\Programs\cswap\Claude Swap.exe" --hidden"#);
        assert!(value_launches(r"c:\users\u\appdata\local\programs\cswap\claude swap.exe", exe));
        assert!(!value_launches(r#""D:\other\Claude Swap.exe" --hidden"#, exe));
    }

    #[test]
    fn login_entries_quote_the_path() {
        let d = desktop_entry(Path::new("/home/u/Apps/a $b.AppImage"));
        assert!(d.contains("Exec=\"/home/u/Apps/a \\$b.AppImage\" --hidden\n"), "{d}");
        let p = mac_plist(Path::new("/Applications/Claude Swap.app/Contents/MacOS/claude-swap"));
        assert!(p.contains("<string>/Applications/Claude Swap.app/Contents/MacOS/claude-swap</string><string>--hidden</string>"));
    }
}
