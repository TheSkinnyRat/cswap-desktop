//! The updater against a local fake of GitHub: what gets downloaded, what is refused.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::AtomicUsize;

use super::*;

struct FakeGithub {
    base: String,
}

/// Serves the routes `routes(base)` returns (path → body) over plain HTTP until the test
/// process ends.
fn serve(routes: impl FnOnce(&str) -> HashMap<String, Vec<u8>>) -> FakeGithub {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let routes = routes(&base);
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut buf = Vec::new();
            let mut b = [0u8; 1024];
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut b) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&b[..n]),
                }
            }
            let req = String::from_utf8_lossy(&buf);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (code, body) = match routes.get(&path) {
                Some(b) => ("200 OK", b.clone()),
                None => ("404 Not Found", b"{}".to_vec()),
            };
            let _ = write!(s, "HTTP/1.1 {code}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = s.write_all(&body);
        }
    });
    FakeGithub { base }
}

const ASSET: &str = "claude-swap-desktop-win-x64-portable.exe";

/// A fresh dir under the OS temp dir, deleted when the guard drops — also when the test
/// panics (a failed assert unwinds through it). Plain `temp_dir().join(..)` dirs were
/// never removed: every `cargo test` left two `cswap-upd-*` dirs per updater test behind.
fn temp_dir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(&format!("cswap-upd-{tag}-")).tempdir().unwrap()
}

/// A release `tag` whose asset serves `served` but whose signature covers `signed`.
fn github(tag: &str, served: &[u8], signed: &[u8], with_sig: bool) -> FakeGithub {
    let version = release::parse_version(tag).unwrap().to_string();
    let sig = sign::sign(&sign::tests::test_key(), ASSET, &version, &Sha512::digest(signed).into());
    let (tag, served) = (tag.to_string(), served.to_vec());
    serve(move |base| {
        let mut assets = vec![
            serde_json::json!({"name": ASSET, "browser_download_url": format!("{base}/dl/{ASSET}"), "size": served.len()}),
        ];
        if with_sig {
            assets.push(serde_json::json!({"name": format!("{ASSET}.sig"), "browser_download_url": format!("{base}/dl/{ASSET}.sig"), "size": sig.len()}));
        }
        let rel = serde_json::json!({"tag_name": tag, "html_url": format!("{base}/releases/{tag}"), "assets": assets});
        let mut routes = HashMap::new();
        routes.insert("/repos/o/r/releases/latest".to_string(), serde_json::to_vec(&rel).unwrap());
        routes.insert(format!("/dl/{ASSET}"), served);
        routes.insert(format!("/dl/{ASSET}.sig"), sig.into_bytes());
        routes
    })
}

struct Run {
    updater: Arc<Updater>,
    quits: Arc<AtomicUsize>,
    dir: PathBuf,
    /// Last field: dropped after the updater, which writes into `dir`.
    _tmp: tempfile::TempDir,
}

fn updater(fake: &FakeGithub, kind: Option<InstallKind>) -> Run {
    let tmp = temp_dir("run");
    let dir = tmp.path().to_path_buf();
    let (sink, _rx) = EventSink::new();
    let quits = Arc::new(AtomicUsize::new(0));
    let q = quits.clone();
    let kind = kind.unwrap_or(InstallKind::Portable { exe: dir.join("claude-swap.exe") });
    let updater = Updater::with(
        UpdateConfig { repo: "o/r".into(), current: "1.0.0".into(), data_dir: dir.clone(), auto: false },
        sink,
        Arc::new(move || {
            q.fetch_add(1, Ordering::SeqCst);
        }),
        kind,
        Ok(sign::tests::test_key().verifying_key()),
        fake.base.clone(),
    );
    std::mem::forget(_rx);
    Run { updater, quits, dir, _tmp: tmp }
}

fn check(u: &Updater) -> UpdaterState {
    let mut p = u.check();
    for _ in 0..200 {
        if let Some(s) = p.take() {
            return s;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("check did not finish");
}

#[test]
fn a_signed_newer_release_downloads_and_verifies() {
    let fake = github("v1.0.1", b"new exe", b"new exe", true);
    let r = updater(&fake, None);
    let s = check(&r.updater);
    assert_eq!(s.status, UpdaterStatus::Downloaded, "{s:?}");
    assert_eq!(s.version.as_deref(), Some("1.0.1"));
    let file = swap::updates_dir(&r.dir).join(ASSET);
    assert_eq!(std::fs::read(&file).unwrap(), b"new exe");
    assert!(!swap::updates_dir(&r.dir).join(format!("{ASSET}.part")).exists());
}

#[test]
fn a_tampered_download_is_deleted_and_never_run() {
    let fake = github("v1.0.1", b"evil exe", b"new exe", true);
    let r = updater(&fake, None);
    let s = check(&r.updater);
    assert_eq!(s.status, UpdaterStatus::Error, "{s:?}");
    assert!(s.error.as_deref().unwrap().contains("nothing was run"), "{s:?}");
    let dir = swap::updates_dir(&r.dir);
    assert!(!dir.join(ASSET).exists() && !dir.join(format!("{ASSET}.part")).exists());
    r.updater.install();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(r.quits.load(Ordering::SeqCst), 0, "install after a failed verification must not quit");
}

#[test]
fn an_unsigned_release_is_refused() {
    let fake = github("v1.0.1", b"new exe", b"new exe", false);
    let r = updater(&fake, None);
    let s = check(&r.updater);
    assert_eq!(s.status, UpdaterStatus::Error);
    assert!(s.error.unwrap().contains("unsigned"));
}

#[test]
fn a_file_changed_after_download_is_refused_at_install() {
    let fake = github("v1.0.1", b"new exe", b"new exe", true);
    let r = updater(&fake, None);
    assert_eq!(check(&r.updater).status, UpdaterStatus::Downloaded);
    std::fs::write(swap::updates_dir(&r.dir).join(ASSET), b"swapped later").unwrap();
    r.updater.install();
    for _ in 0..100 {
        if r.updater.state().status == UpdaterStatus::Error {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(r.updater.state().status, UpdaterStatus::Error);
    assert_eq!(r.quits.load(Ordering::SeqCst), 0);
    assert!(!swap::updates_dir(&r.dir).join(ASSET).exists());
}

#[test]
fn same_or_older_is_not_available() {
    for tag in ["v1.0.0", "v0.1.23"] {
        let fake = github(tag, b"x", b"x", true);
        let r = updater(&fake, None);
        let s = check(&r.updater);
        assert_eq!(s.status, UpdaterStatus::NotAvailable, "{tag}");
        assert!(s.checked_at.is_some());
    }
}

#[test]
fn link_only_copies_see_the_update_but_download_nothing() {
    let fake = github("v1.0.1", b"new exe", b"new exe", true);
    let r = updater(&fake, Some(InstallKind::Link { reason: "deb".into() }));
    let s = check(&r.updater);
    assert_eq!(s.status, UpdaterStatus::Available);
    assert_eq!(s.disabled_reason.as_deref(), Some("deb"));
    assert!(s.release_url.unwrap().ends_with("/releases/v1.0.1"));
    assert!(!swap::updates_dir(&r.dir).join(ASSET).exists());
}

#[test]
fn no_key_or_dev_build_means_disabled_and_no_request() {
    let (sink, _rx) = EventSink::new();
    let tmp = temp_dir("off");
    let cfg = UpdateConfig { repo: "o/r".into(), current: "1.0.0".into(), data_dir: tmp.path().into(), auto: false };
    let quit: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    // An address nothing listens on: a request would fail as Error, not stay Disabled.
    let u = Updater::with(
        cfg.clone(),
        sink.clone(),
        quit.clone(),
        InstallKind::Portable { exe: "x".into() },
        Err("no key".into()),
        "http://127.0.0.1:9".into(),
    );
    assert_eq!(check(&u).status, UpdaterStatus::Disabled);
    let u = Updater::with(
        cfg,
        sink,
        quit,
        InstallKind::Dev,
        Ok(sign::tests::test_key().verifying_key()),
        "http://127.0.0.1:9".into(),
    );
    let s = check(&u);
    assert_eq!(s.status, UpdaterStatus::Disabled);
    assert!(s.disabled_reason.unwrap().contains("development"));
}

#[test]
fn finish_is_none_for_a_normal_launch() {
    assert!(finish(&["claude-swap".into(), "--hidden".into()]).is_none());
    assert!(finish(&["claude-swap".into()]).is_none());
}
