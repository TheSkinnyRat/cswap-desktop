//! Spawning cswap (and uv, the login shell, taskkill): one place that knows the platform
//! rules. Every child gets its own stdin pipe or null, never ours: a child inheriting a
//! stdin another thread reads hangs on Windows (LESSONS, 25 Oct 2026). On Windows a GUI
//! parent must pass CREATE_NO_WINDOW or every call flashes a console.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// What a finished (or failed-to-start) child left behind.
#[derive(Debug, Clone, Default)]
pub struct RawOutcome {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub spawn_error: Option<String>,
}

pub struct Spec<'a> {
    pub bin: &'a Path,
    pub args: &'a [String],
    pub env: &'a [(String, String)],
    pub stdin: Option<&'a str>,
    pub timeout: Duration,
    pub cwd: Option<&'a Path>,
}

/// A command for `bin` with the platform flags set. `.cmd`/`.bat` need no special case:
/// Rust's std runs them through `cmd.exe /e:ON /v:OFF /d /c` and quotes each argument for
/// cmd (empty ones as `""`, `%` escaped, CVE-2024-24576), which is what 0.1.23's `quoteWin`
/// fix (PR #6) did by hand. `driver_tests::empty_and_special_arguments_survive` holds it.
pub fn command(bin: &Path, args: &[String]) -> tokio::process::Command {
    let mut c = tokio::process::Command::new(bin);
    c.args(args);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    // Own process group, so a timeout or stop reaches the whole tree (a shim's grandchild
    // would otherwise keep stdout open and the wait would never end).
    #[cfg(unix)]
    c.process_group(0);
    c
}

/// Kills `pid` and everything under it. Windows has no process groups to signal: taskkill
/// /t walks the tree (a `.cmd` shim's node/python grandchild survives `child.kill()`).
pub async fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let taskkill = std::env::var_os("SystemRoot")
            .map(|r| Path::new(&r).join("System32").join("taskkill.exe"))
            .filter(|p| p.exists())
            .unwrap_or_else(|| "taskkill".into());
        let mut c = tokio::process::Command::new(taskkill);
        c.args(["/pid", &pid.to_string(), "/t", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _ = c.status().await;
    }
    #[cfg(unix)]
    signal_group(pid, libc::SIGKILL);
}

/// Signals the process group `pid` leads (every child here is spawned as a group leader).
#[cfg(unix)]
pub fn signal_group(pid: u32, sig: i32) {
    if let Ok(pid) = i32::try_from(pid)
        && pid > 0
    {
        // SAFETY: kill(2) takes plain integers; a stale group id at worst returns ESRCH.
        unsafe {
            libc::kill(-pid, sig);
        }
    }
}

/// Runs a child to completion with a deadline. Never fails: a spawn error is reported in
/// the outcome, a hung child is killed and says so in stderr.
pub async fn run(spec: Spec<'_>) -> RawOutcome {
    let started = Instant::now();
    let mut cmd = command(spec.bin, spec.args);
    for (k, v) in spec.env {
        cmd.env(k, v);
    }
    if let Some(cwd) = spec.cwd {
        cmd.current_dir(cwd);
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return RawOutcome {
                spawn_error: Some(e.to_string()),
                duration_ms: started.elapsed().as_millis() as u64,
                ..Default::default()
            };
        }
    };
    let pid = child.id();
    // Always close stdin, even with nothing to say: a prompt then reads EOF instead of
    // waiting forever.
    if let Some(mut w) = child.stdin.take() {
        let input = spec.stdin.map(str::to_owned);
        tokio::spawn(async move {
            if let Some(input) = input {
                let _ = w.write_all(input.as_bytes()).await;
            }
            let _ = w.shutdown().await;
        });
    }
    let out_task = child.stdout.take().map(|mut r| {
        tokio::spawn(async move {
            let mut b = Vec::new();
            let _ = r.read_to_end(&mut b).await;
            b
        })
    });
    let err_task = child.stderr.take().map(|mut r| {
        tokio::spawn(async move {
            let mut b = Vec::new();
            let _ = r.read_to_end(&mut b).await;
            b
        })
    });

    let mut timed_out = false;
    let status = match tokio::time::timeout(spec.timeout, child.wait()).await {
        Ok(s) => s.ok(),
        Err(_) => {
            timed_out = true;
            if let Some(pid) = pid {
                kill_tree(pid).await;
            }
            let _ = child.start_kill();
            child.wait().await.ok()
        }
    };
    // Bounded: a grandchild that escaped the kill may hold the pipe open.
    let collect = |t: Option<tokio::task::JoinHandle<Vec<u8>>>| async move {
        match t {
            Some(t) => match tokio::time::timeout(Duration::from_secs(3), t).await {
                Ok(Ok(b)) => String::from_utf8_lossy(&b).into_owned(),
                _ => String::new(),
            },
            None => String::new(),
        }
    };
    let stdout = collect(out_task).await;
    let mut stderr = collect(err_task).await;
    if timed_out {
        stderr.push_str(&format!("\n[cswap-desktop] timed out after {} ms", spec.timeout.as_millis()));
    }
    RawOutcome {
        stdout,
        stderr,
        // A killed child must never read as success, even if it raced to exit 0.
        exit_code: status.and_then(|s| s.code()).filter(|c| !timed_out || *c != 0),
        duration_ms: started.elapsed().as_millis() as u64,
        spawn_error: None,
    }
}
