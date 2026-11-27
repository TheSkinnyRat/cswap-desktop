//! `claude-swap.log` in the data dir: the few facts a user report needs that a release
//! build (no console) would otherwise lose: which renderer the window got and why, and a
//! window that could not start. One line per event, capped so it never grows unbounded.

use std::io::Write;
use std::path::Path;

const FILE: &str = "claude-swap.log";
/// Past this the log is moved to `claude-swap.log.old` (one generation kept).
const CAP: u64 = 256 * 1024;

pub fn line(data_dir: &Path, msg: &str) {
    tracing::info!("{msg}");
    let _ = std::fs::create_dir_all(data_dir);
    let path = data_dir.join(FILE);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > CAP) {
        let _ = std::fs::rename(&path, data_dir.join(format!("{FILE}.old")));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {msg}", cswap_update::time::iso_now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_rotates_past_the_cap() {
        let dir = std::env::temp_dir().join(format!("cswap-applog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        line(&dir, "first");
        line(&dir, "second");
        let s = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(s.lines().count() == 2 && s.contains(" first\n") && s.ends_with(" second\n"), "{s}");
        std::fs::write(dir.join(FILE), vec![b'x'; CAP as usize + 1]).unwrap();
        line(&dir, "after the cap");
        assert!(dir.join(format!("{FILE}.old")).exists());
        assert!(std::fs::read_to_string(dir.join(FILE)).unwrap().ends_with(" after the cap\n"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
