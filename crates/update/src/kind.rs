//! How this running copy was installed, which decides how it is replaced.

use std::path::{Component, Path, PathBuf};

/// The NSIS installer's uninstaller, beside the exe. Its presence is what "installed"
/// means on Windows; 0.1.x's electron-builder uninstaller had the same name.
pub const UNINSTALLER: &str = "Uninstall Claude Swap.exe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallKind {
    /// Per-user NSIS install: run the new installer silently after exit.
    Nsis { exe: PathBuf },
    /// A bare exe anywhere writable: copy the new one over it after exit.
    Portable { exe: PathBuf },
    /// `Claude Swap.app`: the new bundle from the DMG replaces it, then reopen.
    MacApp { bundle: PathBuf },
    /// `$APPIMAGE`: rename the new file over it, restart.
    AppImage { file: PathBuf },
    /// Updates are found but installed by hand from the release page (deb, a translocated
    /// or read-only macOS bundle, a read-only folder). `reason` is shown to the user.
    Link { reason: String },
    /// `cargo run` / tests: no updater at all.
    Dev,
}

impl InstallKind {
    pub fn can_swap(&self) -> bool {
        !matches!(self, InstallKind::Link { .. } | InstallKind::Dev)
    }
}

/// What [`detect`] looks at, gathered by [`Probe::current`] or written by a test.
pub struct Probe<'a> {
    pub os: &'a str,
    pub exe: PathBuf,
    /// `$APPIMAGE`, set by the AppImage runtime.
    pub appimage: Option<PathBuf>,
    pub exists: &'a dyn Fn(&Path) -> bool,
    pub writable: &'a dyn Fn(&Path) -> bool,
}

/// A cargo build runs from `target/<profile>/`; it must never replace itself with a
/// release download (and its "install folder" is the build tree).
fn is_dev_build(exe: &Path) -> bool {
    let parts: Vec<String> = exe.components().filter_map(|c| match c {
        Component::Normal(s) => Some(s.to_string_lossy().to_ascii_lowercase()),
        _ => None,
    }).collect();
    parts.windows(2).any(|w| w[0] == "target" && (w[1] == "debug" || w[1] == "release" || w[1].contains('-')))
}

/// `…/Foo.app/Contents/MacOS/exe` → `…/Foo.app`.
fn mac_bundle(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && bundle.extension()? == "app").then(|| bundle.to_path_buf())
}

pub fn detect(p: &Probe<'_>) -> InstallKind {
    if p.os == "linux" {
        if let Some(file) = p.appimage.as_ref().filter(|f| f.is_absolute()) {
            return match file.parent() {
                Some(dir) if (p.writable)(dir) => InstallKind::AppImage { file: file.clone() },
                _ => InstallKind::Link { reason: format!("{} is in a folder this user cannot write to", file.display()) },
            };
        }
    }
    if is_dev_build(&p.exe) {
        return InstallKind::Dev;
    }
    match p.os {
        "windows" => {
            let dir = p.exe.parent().unwrap_or(Path::new("."));
            if (p.exists)(&dir.join(UNINSTALLER)) {
                InstallKind::Nsis { exe: p.exe.clone() }
            } else if (p.writable)(dir) {
                InstallKind::Portable { exe: p.exe.clone() }
            } else {
                InstallKind::Link { reason: format!("{} cannot be written to", dir.display()) }
            }
        }
        "macos" => match mac_bundle(&p.exe) {
            // Gatekeeper runs a quarantined app opened from Downloads (or the DMG) from a
            // random read-only mount; replacing that copy would change nothing.
            Some(b) if b.to_string_lossy().contains("/AppTranslocation/") => InstallKind::Link {
                reason: "macOS is running this copy from a temporary location. Move Claude Swap to Applications, open it from there, and it can update itself".into(),
            },
            Some(b) => match b.parent() {
                Some(parent) if (p.writable)(parent) && (p.writable)(&b) => InstallKind::MacApp { bundle: b },
                _ => InstallKind::Link { reason: format!("{} cannot be replaced by this user", b.display()) },
            },
            None => InstallKind::Link { reason: "not running from an app bundle".into() },
        },
        _ => {
            let s = p.exe.to_string_lossy();
            if s.starts_with("/usr/") || s.starts_with("/opt/") {
                InstallKind::Link { reason: "installed from the .deb package: install the new .deb from the release page".into() }
            } else {
                InstallKind::Link { reason: "only the AppImage updates itself on Linux".into() }
            }
        }
    }
}

/// Whether this user may create files in `dir` (tries one).
pub fn dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".cswap-write-test-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

impl Probe<'static> {
    pub fn current() -> InstallKind {
        let exe = std::env::current_exe().unwrap_or_default();
        let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
        // canonicalize on Windows gives `\\?\C:\...`; keep the plain form for display and spawn.
        let exe = PathBuf::from(exe.to_string_lossy().trim_start_matches(r"\\?\").to_string());
        detect(&Probe {
            os: std::env::consts::OS,
            exe,
            appimage: std::env::var_os("APPIMAGE").map(PathBuf::from),
            exists: &|p| p.exists(),
            writable: &dir_writable,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe<'a>(os: &'a str, exe: &str, exists: &'a dyn Fn(&Path) -> bool, writable: &'a dyn Fn(&Path) -> bool) -> Probe<'a> {
        Probe { os, exe: PathBuf::from(exe), appimage: None, exists, writable }
    }

    #[test]
    fn windows_install_vs_portable_vs_dev() {
        let yes = |_: &Path| true;
        let no = |_: &Path| false;
        let uninst = |p: &Path| p.ends_with(UNINSTALLER);
        assert!(matches!(detect(&probe("windows", r"C:\U\AppData\Local\Programs\cswap\Claude Swap.exe", &uninst, &yes)), InstallKind::Nsis { .. }));
        assert!(matches!(detect(&probe("windows", r"D:\tools\claude-swap-desktop-win-x64-portable.exe", &no, &yes)), InstallKind::Portable { .. }));
        assert!(matches!(detect(&probe("windows", r"D:\ro\x.exe", &no, &no)), InstallKind::Link { .. }));
        assert_eq!(detect(&probe("windows", r"D:\src\cswap\target\release\claude-swap.exe", &uninst, &yes)), InstallKind::Dev);
        assert_eq!(detect(&probe("linux", "/src/target/x86_64-unknown-linux-gnu/release/claude-swap", &no, &yes)), InstallKind::Dev);
    }

    #[test]
    fn macos_translocated_or_readonly_bundle_gets_the_link() {
        let yes = |_: &Path| true;
        let no = |_: &Path| false;
        let exe = "/Applications/Claude Swap.app/Contents/MacOS/claude-swap";
        assert_eq!(detect(&probe("macos", exe, &no, &yes)), InstallKind::MacApp { bundle: PathBuf::from("/Applications/Claude Swap.app") });
        assert!(matches!(detect(&probe("macos", exe, &no, &no)), InstallKind::Link { .. }));
        let tl = "/private/var/folders/x/T/AppTranslocation/ABC/d/Claude Swap.app/Contents/MacOS/claude-swap";
        match detect(&probe("macos", tl, &no, &yes)) {
            InstallKind::Link { reason } => assert!(reason.contains("Applications")),
            k => panic!("{k:?}"),
        }
        assert!(matches!(detect(&probe("macos", "/usr/local/bin/claude-swap", &no, &yes)), InstallKind::Link { .. }));
    }

    #[test]
    fn linux_appimage_else_link() {
        let yes = |_: &Path| true;
        let no = |_: &Path| false;
        let mut p = probe("linux", "/tmp/.mount_x/usr/bin/claude-swap", &no, &yes);
        p.appimage = Some(PathBuf::from("/home/u/Apps/claude-swap-desktop-linux-x86_64.AppImage"));
        assert!(matches!(detect(&p), InstallKind::AppImage { .. }));
        p.writable = &no;
        assert!(matches!(detect(&p), InstallKind::Link { .. }));
        match detect(&probe("linux", "/usr/bin/claude-swap", &no, &yes)) {
            InstallKind::Link { reason } => assert!(reason.contains(".deb")),
            k => panic!("{k:?}"),
        }
    }
}
