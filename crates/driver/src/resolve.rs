//! Finding cswap (port of `src/main/cswap/resolve.ts`). cswap is NOT bundled: two cswap
//! versions writing one vault lose accounts silently, so the app drives whichever one the
//! user installed. Precedence: Settings path > `CSWAP_DESKTOP_BIN` > uv/pipx dirs > PATH >
//! (macOS) the login shell's PATH.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cswap_core::{BinarySource, CswapBinaryInfo};

use crate::proc::{self, Spec};

#[cfg(windows)]
const EXE: &str = "cswap.exe";
#[cfg(not(windows))]
const EXE: &str = "cswap";

pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let h = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let h = std::env::var_os("HOME");
    h.filter(|h| !h.is_empty()).map(PathBuf::from)
}

pub fn is_batch(p: &Path) -> bool {
    cfg!(windows)
        && p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

/// Where uv / pipx put the shim, per platform. Checked before PATH because an app
/// launched from the Start menu or Finder gets the login PATH, not the shell's.
pub fn candidate_paths() -> Vec<PathBuf> {
    let mut c = Vec::new();
    let home = home_dir();
    if let Some(h) = &home {
        c.push(h.join(".local").join("bin").join(EXE));
    }
    #[cfg(windows)]
    {
        if let Some(a) = std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
            c.push(Path::new(&a).join("Python").join("Scripts").join(EXE));
        }
        if let Some(a) = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()) {
            c.push(Path::new(&a).join("Programs").join("Python").join("Scripts").join(EXE));
        }
    }
    #[cfg(not(windows))]
    {
        c.push("/opt/homebrew/bin/cswap".into());
        c.push("/usr/local/bin/cswap".into());
        if let Some(h) = &home {
            c.push(h.join(".pipx").join("bin").join("cswap"));
        }
    }
    c
}

pub fn find_on_path(path_var: Option<&OsString>, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path_var?).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join(name)).find(|p| p.is_file())
}

/// macOS Finder-launched apps miss the shell PATH entirely; ask the login shell once.
async fn login_shell_path() -> Option<OsString> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());
    let args = ["-ilc".to_owned(), "echo -n \"$PATH\"".to_owned()];
    let out = proc::run(Spec {
        bin: Path::new(&shell),
        args: &args,
        env: &[],
        stdin: None,
        timeout: Duration::from_secs(5),
        cwd: None,
    })
    .await;
    let p = out.stdout.trim();
    (out.spawn_error.is_none() && out.exit_code == Some(0) && !p.is_empty()).then(|| p.into())
}

/// The first `\d+\.\d+\.\d+\S*` in `s`.
pub fn version_in(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    let digits = |mut i: usize| {
        let st = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        (i > st).then_some(i)
    };
    for start in 0..b.len() {
        let Some(i) = digits(start) else { continue };
        let Some(i) = (i < b.len() && b[i] == b'.').then_some(i + 1).and_then(digits) else { continue };
        let Some(mut i) = (i < b.len() && b[i] == b'.').then_some(i + 1).and_then(digits) else { continue };
        while i < b.len() && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        // `start` is ASCII and `i` stops at ASCII whitespace or the end: both boundaries.
        return s.get(start..i);
    }
    None
}

/// `cswap --version`: the version, or why there is none.
pub async fn run_version(bin: &Path, env: &[(String, String)]) -> (Option<String>, Option<String>) {
    let args = ["--version".to_owned()];
    let out = proc::run(Spec { bin, args: &args, env, stdin: None, timeout: Duration::from_secs(15), cwd: None }).await;
    if let Some(e) = out.spawn_error {
        return (None, Some(e));
    }
    if out.exit_code != Some(0) {
        let e = out.stderr.trim();
        let e = if e.is_empty() {
            format!("exit {}", out.exit_code.map_or("null".into(), |c| c.to_string()))
        } else {
            e.to_owned()
        };
        return (None, Some(e));
    }
    let v = version_in(&out.stdout).map(str::to_owned).or_else(|| {
        let t = out.stdout.trim();
        (!t.is_empty()).then(|| t.to_owned())
    });
    (v, None)
}

pub async fn resolve_cswap(
    settings_path: Option<&str>,
    env_override: Option<&str>,
    env: &[(String, String)],
) -> CswapBinaryInfo {
    let candidates = candidate_paths();
    let names: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
    let try_bin = async |path: &Path, source: BinarySource| {
        let (version, error) = run_version(path, env).await;
        CswapBinaryInfo { path: Some(path.display().to_string()), source, version, error, candidates: names.clone() }
    };

    if let Some(sp) = settings_path.filter(|s| !s.trim().is_empty()) {
        let p = Path::new(sp);
        if p.exists() || is_batch(p) {
            return try_bin(p, BinarySource::Settings).await;
        }
        return CswapBinaryInfo {
            path: Some(sp.to_owned()),
            source: BinarySource::Settings,
            version: None,
            error: Some("File not found".into()),
            candidates: names.clone(),
        };
    }
    if let Some(e) = env_override.filter(|s| !s.is_empty()) {
        return try_bin(Path::new(e), BinarySource::Env).await;
    }
    for p in &candidates {
        if !p.exists() {
            continue;
        }
        let source = if p.to_string_lossy().contains("pipx") { BinarySource::Pipx } else { BinarySource::Uv };
        let r = try_bin(p, source).await;
        if r.version.is_some() {
            return r;
        }
    }
    let mut on_path = find_on_path(std::env::var_os("PATH").as_ref(), EXE);
    if on_path.is_none() {
        on_path = find_on_path(login_shell_path().await.as_ref(), EXE);
    }
    if let Some(p) = on_path {
        let r = try_bin(&p, BinarySource::Path).await;
        if r.version.is_some() {
            return r;
        }
    }
    CswapBinaryInfo { path: None, source: BinarySource::None, version: None, error: None, candidates: names.clone() }
}

/// uv, for "Install with uv".
pub fn find_uv() -> Option<PathBuf> {
    let name = if cfg!(windows) { "uv.exe" } else { "uv" };
    let mut dirs = Vec::new();
    if let Some(h) = home_dir() {
        dirs.push(h.join(".local").join("bin"));
        dirs.push(h.join(".cargo").join("bin"));
    }
    dirs.push("/opt/homebrew/bin".into());
    dirs.push("/usr/local/bin".into());
    dirs.into_iter()
        .map(|d| d.join(name))
        .find(|p| p.is_file())
        .or_else(|| find_on_path(std::env::var_os("PATH").as_ref(), name))
}

#[cfg(test)]
mod tests {
    use super::version_in;

    #[test]
    fn reads_the_version_out_of_the_version_line() {
        assert_eq!(version_in("cswap 0.25.0\n"), Some("0.25.0"));
        assert_eq!(version_in("claude-swap v1.2.3rc1 (python 3.12)"), Some("1.2.3rc1"));
        assert_eq!(version_in("build 12.4 then 1.2.3"), Some("1.2.3"));
        assert_eq!(version_in("no version — here"), None);
    }
}
