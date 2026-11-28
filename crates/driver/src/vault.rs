//! cswap's vault ("backup root": sequence.json, settings.json, mappings.json,
//! autoswitch_state.json). Read-only from here, every write goes through the CLI, but
//! watching it keeps the UI in sync with changes made from a terminal or cswap's TUI.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use serde_json::Value;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::resolve::home_dir;

pub fn default_vault_path() -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        let xdg = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.to_string_lossy().trim().is_empty());
        let base = xdg.map(PathBuf::from).or_else(|| home_dir().map(|h| h.join(".local").join("share")))?;
        return Some(base.join("claude-swap"));
    }
    home_dir().map(|h| h.join(".claude-swap-backup"))
}

/// `cswap config path` names the settings file; the vault is its folder.
pub fn vault_from_settings_path(settings_json: Option<&str>) -> Option<PathBuf> {
    Path::new(settings_json?).parent().map(Path::to_path_buf)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingFileEntry {
    pub email: Option<String>,
    pub organization_uuid: Option<String>,
}

/// `mappings.json`, either `{ "mappings": {...} }` or flat, depending on the cswap
/// version. Unreadable = none (the caller falls back to `cswap map`).
pub fn read_mappings_file(vault: &Path) -> Option<BTreeMap<String, MappingFileEntry>> {
    let raw: Value = serde_json::from_slice(&std::fs::read(vault.join("mappings.json")).ok()?).ok()?;
    let obj = raw.as_object()?;
    let inner = match obj.get("mappings") {
        Some(Value::Object(m)) => m,
        _ => obj,
    };
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    Some(
        inner
            .iter()
            .filter(|(_, v)| v.is_object())
            .map(|(k, v)| {
                (k.clone(), MappingFileEntry { email: s(v, "email"), organization_uuid: s(v, "organizationUuid") })
            })
            .collect(),
    )
}

const INTERESTING: [&str; 4] = ["sequence.json", "settings.json", "mappings.json", "autoswitch_state.json"];
const DEBOUNCE: Duration = Duration::from_millis(600);

/// A live watch on the vault folder; dropping it stops watching.
pub struct VaultWatch {
    _watcher: notify::RecommendedWatcher,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for VaultWatch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Calls `on_change` once things settle (600 ms after the last change to one of the
/// files cswap writes). `None` when the folder is missing or cannot be watched.
pub fn watch(rt: &Handle, path: &Path, on_change: impl Fn() + Send + 'static) -> Option<VaultWatch> {
    if !path.is_dir() {
        return None;
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(ev) = res else { return };
        // An event without a file name passes, as 0.1.23's fs.watch did.
        let relevant = ev.paths.is_empty()
            || ev
                .paths
                .iter()
                .any(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| INTERESTING.contains(&n)));
        if relevant {
            let _ = tx.send(());
        }
    })
    .ok()?;
    watcher.watch(path, RecursiveMode::NonRecursive).ok()?;
    let task = rt.spawn(async move {
        while rx.recv().await.is_some() {
            loop {
                match tokio::time::timeout(DEBOUNCE, rx.recv()).await {
                    Ok(Some(())) => continue,
                    Ok(None) => return,
                    Err(_) => break,
                }
            }
            on_change();
        }
    });
    Some(VaultWatch { _watcher: watcher, task })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_shapes_of_mappings_json() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("mappings.json"),
            r#"{"mappings":{"/a":{"email":"a@example.com","organizationUuid":"o"},"bad":1}}"#,
        )
        .unwrap();
        let m = read_mappings_file(d.path()).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m["/a"].organization_uuid.as_deref(), Some("o"));
        std::fs::write(d.path().join("mappings.json"), r#"{"/b":{"email":"b@example.com"}}"#).unwrap();
        assert_eq!(read_mappings_file(d.path()).unwrap()["/b"].email.as_deref(), Some("b@example.com"));
        std::fs::write(d.path().join("mappings.json"), "not json").unwrap();
        assert!(read_mappings_file(d.path()).is_none());
    }

    #[test]
    fn a_change_to_a_vault_file_fires_once_after_the_debounce() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let d = tempfile::tempdir().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let _w = watch(rt.handle(), d.path(), move || {
            let _ = tx.send(());
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(d.path().join("unrelated.txt"), "x").unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(1200)).is_err(), "an unrelated file must not refresh");
        for i in 0..3 {
            std::fs::write(d.path().join("sequence.json"), format!("{i}")).unwrap();
        }
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
        assert!(rx.recv_timeout(Duration::from_millis(1200)).is_err(), "a burst is one refresh");
    }
}
