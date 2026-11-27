//! electron-updater feeds for copies still on 0.1.x: `latest.yml` (Windows NSIS) and
//! `latest-linux.yml` (AppImage). v1 never reads them; they exist so 0.1.x's own updater
//! installs v1 once. What electron-updater 6.8.9 (the version 0.1.23 bundles) does with
//! each field is in packaging/LEGACY-UPDATE.md. Delete this module and the two assets
//! once no 0.1.x copy is left.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;

/// The YAML for one installable file. `sha512` is the raw digest; electron-updater checks
/// the download against its base64 and fails with ERR_CHECKSUM_MISMATCH otherwise.
///
/// No `isAdminRightsRequired`: the v1 installer is per-user like 0.1.x's, and `true` would
/// make electron-updater start it through elevate.exe (a UAC prompt for nothing). No
/// blockmap is published, so electron-updater's differential download fails, logs
/// "Cannot download differentially, fallback to full download" and downloads it whole.
pub fn feed(version: &str, file: &str, sha512: &[u8; 64], size: u64, release_date: &str) -> String {
    let sum = B64.encode(sha512);
    format!(
        "version: {version}\nfiles:\n  - url: {file}\n    sha512: {sum}\n    size: {size}\npath: {file}\nsha512: {sum}\nreleaseDate: '{release_date}'\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_shape_electron_builder_wrote() {
        let y = feed("1.0.0", "claude-swap-desktop-win-x64-setup.exe", &[0u8; 64], 42, "2026-11-28T00:00:00.000Z");
        let want = "version: 1.0.0\nfiles:\n  - url: claude-swap-desktop-win-x64-setup.exe\n    sha512: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==\n    size: 42\npath: claude-swap-desktop-win-x64-setup.exe\nsha512: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==\nreleaseDate: '2026-11-28T00:00:00.000Z'\n";
        assert_eq!(y, want);
        assert!(!y.contains("isAdminRightsRequired"), "per-user installer: no elevate.exe");
    }
}
