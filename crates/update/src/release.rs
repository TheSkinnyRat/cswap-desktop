//! GitHub's `releases/latest` answer, version comparison and which asset this copy wants.
//! Asset names carry no version (the README links `releases/latest/download/<name>`), and
//! are 0.1.x's names exactly, so those links keep working across the rewrite.

use semver::Version;
use serde::Deserialize;

use crate::kind::InstallKind;

pub const RELEASES_PAGE: &str = "https://github.com/TheSkinnyRat/cswap-desktop/releases/latest";

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

/// `v1.2.3` or `1.2.3`.
pub fn parse_version(tag: &str) -> Option<Version> {
    Version::parse(tag.trim().trim_start_matches(['v', 'V'])).ok()
}

/// Names of every asset a release carries, for the release workflow's completeness check.
pub const ALL_ASSETS: &[&str] = &[
    "claude-swap-desktop-win-x64-setup.exe",
    "claude-swap-desktop-win-x64-portable.exe",
    "claude-swap-desktop-mac-x64.dmg",
    "claude-swap-desktop-mac-arm64.dmg",
    "claude-swap-desktop-linux-x86_64.AppImage",
    "claude-swap-desktop-linux-amd64.deb",
];

/// The asset that replaces this copy, for kinds that can swap themselves. `arch` is
/// `std::env::consts::ARCH`.
pub fn asset_name(kind: &InstallKind, arch: &str) -> Option<String> {
    let mac_arch = if arch == "aarch64" { "arm64" } else { "x64" };
    Some(match kind {
        InstallKind::Nsis { .. } => "claude-swap-desktop-win-x64-setup.exe".into(),
        InstallKind::Portable { .. } => "claude-swap-desktop-win-x64-portable.exe".into(),
        InstallKind::MacApp { .. } => format!("claude-swap-desktop-mac-{mac_arch}.dmg"),
        InstallKind::AppImage { .. } => "claude-swap-desktop-linux-x86_64.AppImage".into(),
        InstallKind::Link { .. } | InstallKind::Dev => return None,
    })
}

pub fn sig_name(asset: &str) -> String {
    format!("{asset}.sig")
}

/// The asset and its signature, or why the release cannot be installed.
pub fn pick<'a>(release: &'a Release, name: &str) -> Result<(&'a Asset, &'a Asset), String> {
    let find = |n: &str| release.assets.iter().find(|a| a.name == n);
    let sig = sig_name(name);
    match (find(name), find(&sig)) {
        (Some(a), Some(s)) => Ok((a, s)),
        (None, _) => Err(format!("release {} has no {name}", release.tag_name)),
        (_, None) => Err(format!("release {} has no {sig}; an unsigned file is never installed", release.tag_name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn versions_compare_as_semver() {
        let cur = parse_version("1.0.0").unwrap();
        assert!(parse_version("v1.0.1").unwrap() > cur);
        assert!(parse_version("1.10.0").unwrap() > parse_version("1.9.0").unwrap());
        assert!(parse_version("v0.1.23").unwrap() < cur);
        assert!(parse_version("latest").is_none());
    }

    #[test]
    fn each_kind_wants_its_own_asset_and_its_signature() {
        let p = PathBuf::from("x");
        assert_eq!(asset_name(&InstallKind::Nsis { exe: p.clone() }, "x86_64").unwrap(), "claude-swap-desktop-win-x64-setup.exe");
        assert_eq!(asset_name(&InstallKind::Portable { exe: p.clone() }, "x86_64").unwrap(), "claude-swap-desktop-win-x64-portable.exe");
        assert_eq!(asset_name(&InstallKind::MacApp { bundle: p.clone() }, "aarch64").unwrap(), "claude-swap-desktop-mac-arm64.dmg");
        assert_eq!(asset_name(&InstallKind::MacApp { bundle: p.clone() }, "x86_64").unwrap(), "claude-swap-desktop-mac-x64.dmg");
        assert_eq!(asset_name(&InstallKind::AppImage { file: p }, "x86_64").unwrap(), "claude-swap-desktop-linux-x86_64.AppImage");
        assert!(asset_name(&InstallKind::Link { reason: String::new() }, "x86_64").is_none());
        for kind_asset in ALL_ASSETS {
            assert!(!kind_asset.contains("1.0"), "asset names stay version-free: {kind_asset}");
        }

        let asset = |n: &str| Asset { name: n.into(), browser_download_url: format!("https://x/{n}"), size: 1 };
        let mut r = Release {
            tag_name: "v1.0.1".into(),
            html_url: String::new(),
            draft: false,
            prerelease: false,
            assets: vec![asset("claude-swap-desktop-win-x64-setup.exe"), asset("latest.yml")],
        };
        assert!(pick(&r, "claude-swap-desktop-win-x64-setup.exe").unwrap_err().contains("unsigned"));
        r.assets.push(asset("claude-swap-desktop-win-x64-setup.exe.sig"));
        let (a, s) = pick(&r, "claude-swap-desktop-win-x64-setup.exe").unwrap();
        assert_eq!((a.name.as_str(), s.name.as_str()), ("claude-swap-desktop-win-x64-setup.exe", "claude-swap-desktop-win-x64-setup.exe.sig"));
        assert!(pick(&r, "claude-swap-desktop-mac-x64.dmg").is_err());
    }
}
