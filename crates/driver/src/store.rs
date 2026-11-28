//! The app's `settings.json` (port of `src/main/settings.ts`): tolerant read, atomic write.

use std::path::{Path, PathBuf};

use cswap_core::AppSettings;
use serde_json::{Map, Value};

/// Write to `<file>.tmp`, then rename over: a crash mid-write leaves the old file, never
/// half a file (rename replaces the target atomically on Windows too).
pub fn write_atomic(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = file.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, file)
}

pub struct SettingsStore {
    file: PathBuf,
    data: AppSettings,
    /// Keys this version does not know, written back unchanged so a downgrade (or a
    /// newer copy sharing the folder) keeps its settings.
    extra: Map<String, Value>,
}

impl SettingsStore {
    pub fn open(data_dir: &Path) -> Self {
        let file = data_dir.join("settings.json");
        let (data, extra) = std::fs::read(&file).ok().map(|b| parse(&b)).unwrap_or_default();
        Self { file, data, extra }
    }

    pub fn get(&self) -> AppSettings {
        self.data.clone()
    }

    /// Applies the edit and persists. A failed write keeps the new value in memory (the
    /// UI must not snap back) and is logged.
    pub fn edit(&mut self, f: impl FnOnce(&mut AppSettings)) -> AppSettings {
        f(&mut self.data);
        let mut obj = match serde_json::to_value(&self.data) {
            Ok(Value::Object(o)) => o,
            _ => Map::new(),
        };
        for (k, v) in &self.extra {
            obj.entry(k.clone()).or_insert_with(|| v.clone());
        }
        match serde_json::to_string_pretty(&Value::Object(obj)) {
            Ok(json) => {
                if let Err(e) = write_atomic(&self.file, json.as_bytes()) {
                    tracing::warn!("settings: cannot write {}: {e}", self.file.display());
                }
            }
            Err(e) => tracing::warn!("settings: {e}"),
        }
        self.get()
    }
}

/// Defaults overlaid key by key, as 0.1.x's `{ ...DEFAULT_SETTINGS, ...raw }`, except
/// that a value of the wrong type falls back to its default instead of poisoning the
/// setting: a file written by any older or newer version must still open.
fn parse(bytes: &[u8]) -> (AppSettings, Map<String, Value>) {
    let Ok(Value::Object(raw)) = serde_json::from_slice::<Value>(bytes) else {
        return Default::default();
    };
    let Ok(Value::Object(mut merged)) = serde_json::to_value(AppSettings::default()) else {
        return Default::default();
    };
    let mut extra = Map::new();
    for (k, v) in raw {
        if !merged.contains_key(&k) {
            extra.insert(k, v);
            continue;
        }
        let before = merged.insert(k.clone(), v);
        if serde_json::from_value::<AppSettings>(Value::Object(merged.clone())).is_err()
            && let Some(b) = before
        {
            merged.insert(k, b);
        }
    }
    (serde_json::from_value(Value::Object(merged)).unwrap_or_default(), extra)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cswap_core::ThemeSetting;

    #[test]
    fn reads_0_1_x_settings_tolerantly_and_keeps_unknown_keys() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"theme":"dark","refreshSeconds":"soon","cswapPath":"C:\\x\\cswap.exe","futureKey":[1]}"#,
        )
        .unwrap();
        let mut s = SettingsStore::open(d.path());
        let v = s.get();
        assert_eq!(v.theme, ThemeSetting::Dark);
        assert_eq!(v.refresh_seconds, 60, "a wrong type falls back to the default");
        assert_eq!(v.cswap_path.as_deref(), Some("C:\\x\\cswap.exe"));
        s.edit(|x| x.mask_emails = true);
        let raw: Value = serde_json::from_slice(&std::fs::read(d.path().join("settings.json")).unwrap()).unwrap();
        assert_eq!(raw["futureKey"], serde_json::json!([1]));
        assert_eq!(raw["maskEmails"], Value::Bool(true));
        assert!(!d.path().join("settings.json.tmp").exists());
        assert!(SettingsStore::open(d.path()).get().mask_emails);
    }

    #[test]
    fn a_corrupt_or_missing_file_is_defaults() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(SettingsStore::open(d.path()).get(), AppSettings::default());
        std::fs::write(d.path().join("settings.json"), "{nope").unwrap();
        assert_eq!(SettingsStore::open(d.path()).get(), AppSettings::default());
    }
}
