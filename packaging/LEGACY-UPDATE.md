# 0.1.x → v1 through 0.1.x's own updater

Every v1 release also carries `latest.yml` (Windows) and `latest-linux.yml` (AppImage),
written by `cswap-release legacy-feed` from `packaging/{windows,linux}/build.*`. Only a
0.1.x copy reads them; v1 never does (it reads `releases/latest` and verifies `.sig`
files). macOS 0.1.x had no auto-update (`updater.ts` disables it on darwin), so Mac users
move to v1 by downloading the DMG once.

Everything below was read from the **electron-updater 6.8.9** sources (the version in
0.1.23's `package-lock.json`; npm tarball `out/AppUpdater.js`, `BaseUpdater.js`,
`NsisUpdater.js`, `AppImageUpdater.js`, `providers/GitHubProvider.js`, `providers/Provider.js`),
plus what 0.1.23 installed on the PC (`resources\app-update.yml`, the registry). Not observed
end to end: see "Not proven".

## What 0.1.23 does, step by step

1. **Check** (15 s after start, then every 6 h; `src/main/updater.ts`). `app-update.yml` is
   `owner: TheSkinnyRat, repo: cswap-desktop, provider: github,
   updaterCacheDirName: cswap-desktop-updater`, no `publisherName`. GitHubProvider reads
   `releases.atom` / `releases/latest` for the newest tag, then downloads
   `releases/download/<tag>/latest.yml` (Linux: `latest-linux.yml`; x64 has no arch suffix,
   `Provider.getChannelFilePrefix`). `isUpdateAvailable`: semver `1.0.0` > `0.1.23` → yes.
   `allowPrerelease = false`, so the v1 release must not be a pre-release.
2. **Download** (`autoDownload = true`).
   - Windows: `findFile(files, "exe")` → `claude-swap-desktop-win-x64-setup.exe`.
     `differentialDownloadInstaller` tries `<file>.blockmap`; v1 publishes none, the
     download throws, it logs "Cannot download differentially, fallback to full download"
     and returns `true` → full download.
   - AppImage: `findFile(files, "AppImage", [rpm, deb, pacman])`; the differential attempt
     fails the same way and returns `process.platform === "linux"` → full download.
3. **Verify.** The download is checked against `sha512` (base64) from the feed;
   a mismatch is `ERR_CHECKSUM_MISMATCH` and nothing runs. No code-signature check:
   `NsisUpdater.verifySignature` returns at once without `publisherName` (and 0.1.23
   replaced `verifyUpdateCodeSignature` with a no-op anyway).
4. **Install**, one of two ways:
   - The user presses Install: `quitAndInstall(true, true)` → `install(isSilent=true,
     isForceRunAfter=true)`.
   - The user quits with the update downloaded (`autoInstallOnAppQuit = true`): the quit
     handler calls `install(true, false)`.
   - Windows (`NsisUpdater.doInstall`): arguments `--updated /S` plus `--force-run` in the
     first case. No `/D=` (no install directory set), no `--package-file` (not a web
     installer). The feed has no `isAdminRightsRequired`, so the installer is spawned
     directly (detached), not through `elevate.exe`; it is per-user, like 0.1.x's.
     0.1.x then quits.
   - AppImage (`AppImageUpdater.doInstall`): deletes `$APPIMAGE`, moves the download to
     the same name (the name has no version in it, so it is overwritten in place), then
     - with `isForceRunAfter`: spawns it detached with `APPIMAGE_SILENT_INSTALL=true`;
     - without (install on quit): runs it **synchronously** with
       `APPIMAGE_EXIT_AFTER_INSTALL=true` and waits for it to exit.

## What v1 does with that

- **Windows installer** (`packaging/windows/installer.nsi`): `/S` silent, `--updated`
  ignored, `--force-run` starts the app afterwards. `stop-app.ps1` waits up to 10 s for the
  quitting 0.1.x (exact exe path, every Electron process), then forces what is left. The
  Electron files are deleted by name, the old `Uninstall Claude Swap.exe` is deleted
  **unrun**, v1's exe goes in as `Claude Swap.exe` in the same folder, the HKCU uninstall
  entry `70c76982-575b-5e2d-80a5-0b90f967d60f` is overwritten, the Start-menu shortcut is
  rewritten, and `%LOCALAPPDATA%\cswap-desktop-updater` (old installers) is removed.
  `%APPDATA%\cswap-desktop` is untouched; the HKCU Run value `electron.app.Claude Swap`
  keeps working because the exe path did not change.
- **AppImage**: v1's `main` calls `cswap_update::finish` first; with
  `APPIMAGE_EXIT_AFTER_INSTALL=true` it exits 0 at once, so 0.1.x's quit is not held up by
  a whole app running inside it. The next start is v1. With `APPIMAGE_SILENT_INSTALL` it
  just starts normally.

## Not proven

- No release has exercised this yet; steps 1–4 are read from source, not observed.
- The installer's takeover was tested on the PC against a FAKE old install in a temp
  folder with a test GUID (build with `/DAPP_GUID=… /DSHORTCUT=… /DRUN_VALUE=…
  /DDEFAULT_DIR=…`), never against the real 0.1.23 install.

## Removing it

Once no 0.1.x copy is left: drop the `legacy-feed` calls in `packaging/windows/build.ps1`
and `packaging/linux/build.sh`, `latest*.yml` in `.github/workflows/release.yml`,
`crates/update/src/legacy.rs`, and the `APPIMAGE_EXIT_AFTER_INSTALL` branch of `finish`.
Keep the installer's takeover for as long as an old install might still be found.
