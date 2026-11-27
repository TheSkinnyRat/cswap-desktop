//! "Run in terminal" (port of `src/main/session.ts`): opens a terminal window running
//! `cswap run <target>` (session mode, that terminal only). The terminal starts where the
//! caller asks (a mapped project, or home), never wherever the app was started from.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cswap_core::{CsResult, CswapError, LaunchedSession};

use crate::resolve::home_dir;

/// The target is pasted into a shell line on every platform. cswap's targets are slot
/// numbers, emails and aliases (`[A-Za-z0-9._-]`), so anything else is refused rather than
/// quoted for three different shells.
fn safe_target(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || "@._+-".contains(c))
}

/// `no_spawn` is the test seam `CSWAP_DESKTOP_TEST_NO_SPAWN`: it proves the arguments,
/// never the spawn.
pub fn launch_session(
    bin: Option<&Path>,
    target: &str,
    cwd: Option<&str>,
    no_spawn: bool,
) -> CsResult<LaunchedSession> {
    let Some(bin) = bin else { return Err(CswapError::new("NotFound", "cswap binary not configured")) };
    let dir: PathBuf = match cwd.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => PathBuf::from(c),
        None => home_dir().ok_or_else(|| CswapError::new("NotFound", "No home directory"))?,
    };
    if !dir.is_dir() {
        return Err(CswapError::new("NotFound", format!("No such directory: {}", dir.display())));
    }
    if !safe_target(target) {
        return Err(CswapError::new("LaunchError", format!("Unsupported account identifier: {target}")));
    }
    let command = format!("cswap run {target}");
    let ok = LaunchedSession { command, cwd: dir.display().to_string() };
    if no_spawn {
        return Ok(ok);
    }
    spawn_terminal(bin, target, &dir).map(|()| ok)
}

#[cfg(windows)]
fn spawn_terminal(bin: &Path, target: &str, dir: &Path) -> CsResult<()> {
    use std::os::windows::process::CommandExt;
    // A console window of its own (CREATE_NEW_CONSOLE), not tied to the app's group.
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    // Verbatim: cmd does not understand the backslash-escaped quotes std would produce;
    // /s strips the outer pair and keeps the inner quoting of the path.
    let line = format!("/d /s /k \"title cswap run {target}& \"{}\" run {target}\"", bin.display());
    let comspec = std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into());
    Command::new(comspec)
        .raw_arg(line)
        .current_dir(dir)
        .creation_flags(CREATE_NEW_CONSOLE | CREATE_NEW_PROCESS_GROUP)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| CswapError::new("LaunchError", e.to_string()))
}

#[cfg(target_os = "macos")]
fn spawn_terminal(bin: &Path, target: &str, dir: &Path) -> CsResult<()> {
    // Terminal.app opens in its own default directory, so the cd belongs in the command
    // itself. Two levels of quoting: the shell line inside an AppleScript string.
    let sh = |v: &str| format!("'{}'", v.replace('\'', "'\\''"));
    let line = format!("cd {} && {} run {target}", sh(&dir.display().to_string()), sh(&bin.display().to_string()));
    let script =
        format!("tell application \"Terminal\" to do script \"{}\"", line.replace('\\', "\\\\").replace('"', "\\\""));
    Command::new("osascript")
        .args(["-e", &script, "-e", "tell application \"Terminal\" to activate"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| CswapError::new("LaunchError", e.to_string()))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_terminal(bin: &Path, target: &str, dir: &Path) -> CsResult<()> {
    use std::os::unix::process::CommandExt;
    let sh = |v: &str| format!("'{}'", v.replace('\'', "'\\''"));
    let cmd = format!(
        "cd {} && {} run {target}; exec $SHELL",
        sh(&dir.display().to_string()),
        sh(&bin.display().to_string())
    );
    let bash_line = format!("bash -lc {}", sh(&cmd));
    let terminals: [(&str, Vec<&str>); 4] = [
        ("x-terminal-emulator", vec!["-e", &bash_line]),
        ("gnome-terminal", vec!["--", "bash", "-lc", &cmd]),
        ("konsole", vec!["-e", "bash", "-lc", &cmd]),
        ("xterm", vec!["-e", "bash", "-lc", &cmd]),
    ];
    // A missing terminal fails at spawn here (0.1.23 learned that only asynchronously and
    // reported success for the first name on the list).
    for (t, args) in terminals {
        let spawned = Command::new(t)
            .args(&args)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn();
        if spawned.is_ok() {
            return Ok(());
        }
    }
    Err(CswapError::new("NotFound", "No terminal emulator found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIN: &str = "/usr/bin/cswap";

    #[test]
    fn starts_in_the_directory_it_is_given() {
        let d = tempfile::tempdir().unwrap();
        let r = launch_session(Some(Path::new(BIN)), "2", d.path().to_str(), true).unwrap();
        assert_eq!(r, LaunchedSession { command: "cswap run 2".into(), cwd: d.path().display().to_string() });
    }

    #[test]
    fn falls_back_to_home_when_no_directory_is_asked_for() {
        let home = home_dir().unwrap().display().to_string();
        for cwd in [None, Some("   ")] {
            let r = launch_session(Some(Path::new(BIN)), "2", cwd, true).unwrap();
            assert_eq!(r, LaunchedSession { command: "cswap run 2".into(), cwd: home.clone() });
        }
    }

    #[test]
    fn refuses_a_directory_that_is_not_there() {
        let d = tempfile::tempdir().unwrap();
        let missing = d.path().join("cswap-run-definitely-missing");
        let e = launch_session(Some(Path::new(BIN)), "2", missing.to_str(), true).unwrap_err();
        assert!(e.message.contains("No such directory"), "{e}");
    }

    #[test]
    fn needs_a_binary() {
        assert_eq!(launch_session(None, "2", None, true).unwrap_err().kind, "NotFound");
    }

    #[test]
    fn refuses_a_target_that_would_be_shell_syntax() {
        assert!(launch_session(Some(Path::new(BIN)), "you@example.com", None, true).is_ok());
        assert!(launch_session(Some(Path::new(BIN)), "a&calc", None, true).is_err());
        assert!(launch_session(Some(Path::new(BIN)), "", None, true).is_err());
    }
}
