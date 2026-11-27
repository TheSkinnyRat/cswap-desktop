//! Putting a verified download in place of this copy.
//!
//! The app has to exit first: it holds the single-instance lock (a new copy started beside
//! it would only ask it to show its window), and Windows will not overwrite a running exe.
//! So the last step always runs in a helper that waits for this process to go:
//! - Windows: a COPY of this exe (the installer replaces the original) started with
//!   [`HELPER_FLAG`]. It re-verifies the download, then runs the installer silently or
//!   copies the portable exe over, and starts the new version.
//! - macOS / AppImage: the new bundle or file is put next to the old one while the app still
//!   runs (both allow that), and `/bin/sh` waits for the exit, renames it into place and
//!   reopens it. No helper binary is needed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::kind::InstallKind;

pub const HELPER_FLAG: &str = "--cswap-update-helper";
/// The helper gives up after this long instead of racing a copy that will not exit.
const WAIT_FOR_EXIT: Duration = Duration::from_secs(60);
pub const FAILURE_FILE: &str = "last-failure.txt";

/// What is to be installed, as verified.
#[derive(Debug, Clone)]
pub struct Download {
    pub file: PathBuf,
    pub asset: String,
    pub version: String,
}

pub fn updates_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("updates")
}

/// Appends a line to `updater.log`; logging must never break an update.
pub fn log_line(data_dir: &Path, msg: &str) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(data_dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(data_dir.join("updater.log")) {
        let _ = writeln!(f, "{} {msg}", crate::time::iso_now());
    }
    tracing::info!(target: "updater", "{msg}");
}

/// Prepares the swap and starts whatever finishes it after this process exits. The caller
/// quits the app right after `Ok`. `relaunch`: start the new version when done (false when
/// installing on quit, as 0.1.x's install-on-quit did not restart either).
pub fn hand_over(kind: &InstallKind, dl: &Download, data_dir: &Path, relaunch: bool) -> Result<(), String> {
    let pid = std::process::id();
    match kind {
        InstallKind::Nsis { exe } => spawn_windows_helper("nsis", pid, dl, exe, data_dir, relaunch),
        InstallKind::Portable { exe } => spawn_windows_helper("portable", pid, dl, exe, data_dir, relaunch),
        InstallKind::MacApp { bundle } => mac_hand_over(pid, dl, bundle, relaunch),
        InstallKind::AppImage { file } => appimage_hand_over(pid, dl, file, relaunch),
        InstallKind::Link { reason } => Err(reason.clone()),
        InstallKind::Dev => Err("a development build does not update itself".into()),
    }
}

// ---- Windows ----------------------------------------------------------------------

fn spawn_windows_helper(mode: &str, pid: u32, dl: &Download, target: &Path, data_dir: &Path, relaunch: bool) -> Result<(), String> {
    let me = std::env::current_exe().map_err(|e| format!("cannot find this exe: {e}"))?;
    let dir = updates_dir(data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let helper = dir.join(format!("update-helper-{pid}.exe"));
    std::fs::copy(&me, &helper).map_err(|e| format!("cannot copy the update helper: {e}"))?;
    let mut cmd = Command::new(&helper);
    cmd.arg(HELPER_FLAG)
        .arg(mode)
        .arg(pid.to_string())
        .arg(&dl.file)
        .arg(target)
        .arg(&dl.asset)
        .arg(&dl.version)
        .arg(if relaunch { "1" } else { "0" })
        .arg(data_dir);
    spawn_detached(cmd).map_err(|e| format!("cannot start the update helper: {e}"))
}

/// The helper's side, from [`crate::finish`]. `args` follow [`HELPER_FLAG`].
pub fn run_helper(args: &[std::ffi::OsString]) -> Result<(), String> {
    let s = |i: usize| args.get(i).map(PathBuf::from).ok_or_else(|| "update helper: missing argument".to_string());
    let mode = s(0)?.to_string_lossy().into_owned();
    let pid: u32 = s(1)?.to_string_lossy().parse().map_err(|_| "update helper: bad pid")?;
    let dl = Download { file: s(2)?, asset: s(4)?.to_string_lossy().into_owned(), version: s(5)?.to_string_lossy().into_owned() };
    let target = s(3)?;
    let relaunch = s(6)?.to_string_lossy() == "1";
    let data_dir = s(7)?;
    let r = crate::sign::release_key().and_then(|key| helper_steps(&mode, pid, &dl, &target, relaunch, &key));
    match &r {
        Ok(()) => log_line(&data_dir, &format!("[helper] {mode}: installed {}", dl.version)),
        Err(e) => {
            log_line(&data_dir, &format!("[helper] {mode} failed: {e}"));
            let _ = std::fs::write(updates_dir(&data_dir).join(FAILURE_FILE), e);
        }
    }
    r
}

fn helper_steps(mode: &str, pid: u32, dl: &Download, target: &Path, relaunch: bool, key: &ed25519_dalek::VerifyingKey) -> Result<(), String> {
    if !wait_for_exit(pid, WAIT_FOR_EXIT) {
        return Err(format!("Claude Swap (pid {pid}) did not exit within {}s", WAIT_FOR_EXIT.as_secs()));
    }
    // The file sat in the user's folder between download and now: check it again right
    // before running it.
    crate::verify_download(dl, key)?;
    match mode {
        "nsis" => {
            // `--updated` is what electron-updater passes too; the installer ignores it.
            let mut cmd = Command::new(&dl.file);
            cmd.arg("/S").arg("--updated");
            if relaunch {
                cmd.arg("--force-run");
            }
            let status = cmd.status().map_err(|e| format!("cannot run the installer: {e}"))?;
            let _ = std::fs::remove_file(&dl.file);
            if !status.success() {
                return Err(format!("the installer exited with {status}"));
            }
        }
        "portable" => {
            // The exe may still be held for a moment after its process is gone (antivirus).
            let mut last = String::new();
            let mut done = false;
            for _ in 0..40 {
                match std::fs::copy(&dl.file, target) {
                    Ok(_) => {
                        done = true;
                        break;
                    }
                    Err(e) => last = e.to_string(),
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            if !done {
                return Err(format!("cannot replace {}: {last}", target.display()));
            }
            let _ = std::fs::remove_file(&dl.file);
            if relaunch {
                spawn_detached(Command::new(target)).map_err(|e| format!("installed, but cannot start it: {e}"))?;
            }
        }
        m => return Err(format!("update helper: unknown mode {m}")),
    }
    Ok(())
}

#[cfg(windows)]
fn wait_for_exit(pid: u32, timeout: Duration) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
    // SAFETY: plain handle use; closed on every path.
    unsafe {
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if h.is_null() {
            return true; // gone already (or never ours to wait for)
        }
        let r = WaitForSingleObject(h, timeout.as_millis() as u32);
        CloseHandle(h);
        r == WAIT_OBJECT_0
    }
}

#[cfg(not(windows))]
fn wait_for_exit(pid: u32, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        let alive = Command::new("/bin/kill").arg("-0").arg(pid.to_string()).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success());
        if !alive {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Started so it outlives this process and never flashes a console.
pub fn spawn_detached(mut cmd: Command) -> std::io::Result<()> {
    use std::process::Stdio;
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map(|_| ())
}

// ---- macOS / AppImage -----------------------------------------------------------------

/// `/bin/sh` waiting for `pid` (at most a minute), then running `script` with `$@` = `args`.
#[cfg(unix)]
fn after_exit(pid: u32, script: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let full = format!(
        r#"pid=$1; shift; i=0; while kill -0 "$pid" 2>/dev/null; do [ $i -ge 600 ] && exit 1; sleep 0.1; i=$((i+1)); done; {script}"#
    );
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg(full).arg("sh").arg(pid.to_string()).args(args);
    spawn_detached(cmd).map_err(|e| format!("cannot schedule the restart: {e}"))
}

#[cfg(not(unix))]
fn after_exit(_pid: u32, _script: &str, _args: &[&std::ffi::OsStr]) -> Result<(), String> {
    Err("not supported on this platform".into())
}

fn run(cmd: &str, args: &[&std::ffi::OsStr]) -> Result<String, String> {
    let out = Command::new(cmd).args(args).stdin(std::process::Stdio::null()).output().map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("{cmd} failed ({}): {}", out.status, String::from_utf8_lossy(&out.stderr).trim()))
    }
}

fn mac_hand_over(pid: u32, dl: &Download, bundle: &Path, relaunch: bool) -> Result<(), String> {
    let parent = bundle.parent().ok_or("the app bundle has no parent folder")?;
    let name = bundle.file_name().ok_or("bad bundle path")?.to_string_lossy().into_owned();
    let staging = parent.join(format!(".{name}.update-{pid}"));
    let old = parent.join(format!(".{name}.old-{pid}"));
    let mount = std::env::temp_dir().join(format!("cswap-update-{pid}"));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&mount).map_err(|e| format!("cannot create {}: {e}", mount.display()))?;
    // The DMG's own checksum pass is redundant after the signature check.
    run("/usr/bin/hdiutil", &["attach".as_ref(), "-nobrowse".as_ref(), "-readonly".as_ref(), "-noautoopen".as_ref(), "-noverify".as_ref(), "-mountpoint".as_ref(), mount.as_os_str(), dl.file.as_os_str()])?;
    let src = mount.join("Claude Swap.app");
    // --noextattr: the volume's Finder metadata on the files makes `codesign --verify
    // --strict` reject the copy as carrying "detritus"; the signature lives in the files.
    let copied = if src.is_dir() {
        run("/usr/bin/ditto", &["--norsrc".as_ref(), "--noextattr".as_ref(), "--noacl".as_ref(), src.as_os_str(), staging.as_os_str()]).map(|_| ())
    } else {
        Err("the disk image does not contain Claude Swap.app".into())
    };
    let _ = run("/usr/bin/hdiutil", &["detach".as_ref(), "-quiet".as_ref(), mount.as_os_str()]);
    let _ = std::fs::remove_dir(&mount);
    if let Err(e) = copied {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    // Swap only once the old copy has exited; put it back if the new one cannot go in.
    let reopen = if relaunch { r#"/usr/bin/open "$1""# } else { ":" };
    let script = format!(
        r#"mv "$1" "$3" || exit 1; if mv "$2" "$1"; then rm -rf "$3"; {reopen}; else mv "$3" "$1"; rm -rf "$2"; exit 1; fi"#
    );
    after_exit(pid, &script, &[bundle.as_os_str(), staging.as_os_str(), old.as_os_str()])?;
    let _ = std::fs::remove_file(&dl.file);
    Ok(())
}

fn appimage_hand_over(pid: u32, dl: &Download, file: &Path, relaunch: bool) -> Result<(), String> {
    let dir = file.parent().ok_or("the AppImage has no folder")?;
    let name = file.file_name().ok_or("bad AppImage path")?.to_string_lossy().into_owned();
    // Same folder, so the final rename stays on one filesystem (atomic, and allowed while
    // the old file is running).
    let staged = dir.join(format!(".{name}.update-{pid}"));
    std::fs::copy(&dl.file, &staged).map_err(|e| format!("cannot write {}: {e}", staged.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).map_err(|e| format!("chmod: {e}"))?;
    }
    if let Err(e) = std::fs::rename(&staged, file) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("cannot replace {}: {e}", file.display()));
    }
    let _ = std::fs::remove_file(&dl.file);
    if relaunch {
        after_exit(pid, r#"exec "$1""#, &[file.as_os_str()])?;
    }
    Ok(())
}

/// Removes helper copies and staging files earlier updates left (they cannot delete
/// themselves while running).
pub fn clean_leftovers(data_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(updates_dir(data_dir)) else { return };
    let me = format!("update-helper-{}.exe", std::process::id());
    for e in entries.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.starts_with("update-helper-") && n != me {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The failure an earlier helper recorded, once.
pub fn take_failure(data_dir: &Path) -> Option<String> {
    let f = updates_dir(data_dir).join(FAILURE_FILE);
    let s = std::fs::read_to_string(&f).ok()?;
    let _ = std::fs::remove_file(&f);
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed(dir: &Path, name: &str, bytes: &[u8], version: &str) -> Download {
        let file = dir.join(name);
        std::fs::write(&file, bytes).unwrap();
        let k = crate::sign::tests::test_key();
        let sig = crate::sign::sign(&k, name, version, &crate::sign::sha512_file(&file).unwrap());
        std::fs::write(dir.join(format!("{name}.sig")), sig).unwrap();
        Download { file, asset: name.into(), version: version.into() }
    }

    #[test]
    fn portable_helper_replaces_only_a_verified_exe() {
        let dir = std::env::temp_dir().join(format!("cswap-portable-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("claude-swap-portable.exe");
        std::fs::write(&target, b"old").unwrap();
        let key = crate::sign::tests::test_key().verifying_key();
        // pid 0 / a pid that is not running: the wait returns at once.
        let dead = u32::MAX - 1;
        let dl = signed(&dir, "claude-swap-desktop-win-x64-portable.exe", b"new", "1.0.1");
        std::fs::write(&dl.file, b"tampered").unwrap();
        assert!(helper_steps("portable", dead, &dl, &target, false, &key).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"old", "a file failing verification is never put in place");
        let dl = signed(&dir, "claude-swap-desktop-win-x64-portable.exe", b"new", "1.0.1");
        helper_steps("portable", dead, &dl, &target, false, &key).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn appimage_is_renamed_over_and_executable() {
        let dir = std::env::temp_dir().join(format!("cswap-appimage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let current = dir.join("claude-swap-desktop-linux-x86_64.AppImage");
        std::fs::write(&current, b"old").unwrap();
        let new = dir.join("download");
        std::fs::write(&new, b"new").unwrap();
        let dl = Download { file: new.clone(), asset: "a".into(), version: "1.0.1".into() };
        appimage_hand_over(std::process::id(), &dl, &current, false).unwrap();
        assert_eq!(std::fs::read(&current).unwrap(), b"new");
        assert!(!new.exists());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&current).unwrap().permissions().mode() & 0o777, 0o755);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
