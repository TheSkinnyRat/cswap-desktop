#!/usr/bin/env bash
# Builds the Linux release files into packaging/linux/out:
#
#   claude-swap-desktop-linux-x86_64.AppImage   updates itself (rename over + restart)
#   claude-swap-desktop-linux-amd64.deb         package "cswap-desktop", as 0.1.x's, so
#                                               installing it upgrades the Electron deb
#   <each>.sig                                  ed25519 signatures (CSWAP_UPDATE_SIGNING_KEY)
#   latest-linux.yml                            electron-updater feed for 0.1.x AppImages
#
# Same script in release.yml and by hand on an x86_64 Linux. Build on the OLDEST glibc
# you support (release.yml: ubuntu-22.04, glibc 2.35): the binary needs that or newer.
#
#   packaging/linux/build.sh [--skip-build] [--require-signature]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
SKIP_BUILD=0
REQUIRE_SIG=0
for a in "$@"; do
  case "$a" in
    --skip-build) SKIP_BUILD=1 ;;
    --require-signature) REQUIRE_SIG=1 ;;
    *) echo "unknown option $a" >&2; exit 2 ;;
  esac
done

VERSION="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\([^"]*\)".*/\1/p' "$ROOT/Cargo.toml")"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "bad version '$VERSION'" >&2; exit 1; }

cd "$ROOT"
if [[ $SKIP_BUILD == 0 ]]; then
  cargo build --release --locked -p cswap-desktop -p cswap-update
fi
BIN="$ROOT/target/release/claude-swap"
TOOL="$ROOT/target/release/cswap-release"
OUT="$HERE/out"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
rm -rf "$OUT" && mkdir -p "$OUT"

check_sha256() { echo "$1  $2" | sha256sum -c --quiet - || { echo "checksum mismatch: $2" >&2; exit 1; }; }

# ---- shared tree -------------------------------------------------------------------------
stage() {
  local dst="$1"
  install -Dm755 "$BIN" "$dst/usr/bin/claude-swap"
  install -Dm644 "$HERE/claude-swap.desktop" "$dst/usr/share/applications/claude-swap.desktop"
  install -Dm644 "$ROOT/resources/icon.png" "$dst/usr/share/icons/hicolor/512x512/apps/claude-swap.png"
  install -Dm644 "$ROOT/resources/icon-256.png" "$dst/usr/share/icons/hicolor/256x256/apps/claude-swap.png"
}

# ---- .deb ---------------------------------------------------------------------------------
DEB_ROOT="$WORK/deb"
stage "$DEB_ROOT"
mkdir -p "$DEB_ROOT/DEBIAN"
SIZE_KB="$(du -sk "$DEB_ROOT/usr" | cut -f1)"
# Package name and maintainer are 0.1.x's (electron-builder used package.json `name`), so
# dpkg treats this as an upgrade and removes the Electron files under /opt/Claude Swap.
# libgl1/libegl1: the window draws with OpenGL (glow); X11/Wayland/xkbcommon are opened
# at run time by winit.
cat >"$DEB_ROOT/DEBIAN/control" <<CONTROL
Package: cswap-desktop
Version: $VERSION
Section: utils
Priority: optional
Architecture: amd64
Maintainer: TheSkinnyRat <cswap-desktop@users.noreply.github.com>
Installed-Size: $SIZE_KB
Depends: libc6 (>= 2.35), libgl1, libegl1, libxkbcommon0
Recommends: libxkbcommon-x11-0, libwayland-client0
Homepage: https://github.com/TheSkinnyRat/cswap-desktop
Description: Claude Swap: desktop app for claude-swap (cswap)
 Manage and switch Claude Code accounts without the terminal. Needs the
 cswap CLI (claude-swap), which the app can install with uv.
CONTROL
DEB="$OUT/claude-swap-desktop-linux-amd64.deb"
dpkg-deb --root-owner-group --build "$DEB_ROOT" "$DEB"

# ---- AppImage -----------------------------------------------------------------------------
# Pinned tool and runtime (craft-launcher's pins): appimagetool would otherwise embed
# the unpinned `continuous` runtime.
APPIMAGETOOL_VERSION=1.9.1
APPIMAGETOOL_SHA256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
RUNTIME_VERSION=20251108
RUNTIME_SHA256=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d
CACHE="$ROOT/target/appimage-tools"
mkdir -p "$CACHE"
APPIMAGETOOL="$CACHE/appimagetool-$APPIMAGETOOL_VERSION-x86_64.AppImage"
RUNTIME="$CACHE/runtime-$RUNTIME_VERSION-x86_64"
[[ -f "$APPIMAGETOOL" ]] || curl --proto '=https' -fsSL -o "$APPIMAGETOOL" "https://github.com/AppImage/appimagetool/releases/download/$APPIMAGETOOL_VERSION/appimagetool-x86_64.AppImage"
[[ -f "$RUNTIME" ]] || curl --proto '=https' -fsSL -o "$RUNTIME" "https://github.com/AppImage/type2-runtime/releases/download/$RUNTIME_VERSION/runtime-x86_64"
check_sha256 "$APPIMAGETOOL_SHA256" "$APPIMAGETOOL"
check_sha256 "$RUNTIME_SHA256" "$RUNTIME"
chmod 755 "$APPIMAGETOOL"

APPDIR="$WORK/ClaudeSwap.AppDir"
stage "$APPDIR"
cat >"$APPDIR/AppRun" <<'APPRUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/claude-swap" "$@"
APPRUN
chmod 755 "$APPDIR/AppRun"
cp "$HERE/claude-swap.desktop" "$APPDIR/claude-swap.desktop"
cp "$ROOT/resources/icon-256.png" "$APPDIR/claude-swap.png"
ln -s claude-swap.png "$APPDIR/.DirIcon"
APPIMAGE="$OUT/claude-swap-desktop-linux-x86_64.AppImage"
# Extract-and-run: no FUSE needed on the build machine; the output embeds the static
# runtime, so users need no libfuse2 either.
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGETOOL" --no-appstream --runtime-file "$RUNTIME" "$APPDIR" "$APPIMAGE"

# ---- signatures and the legacy feed -----------------------------------------------------
if [[ -n "${CSWAP_UPDATE_SIGNING_KEY:-}" ]]; then
  "$TOOL" sign --version "$VERSION" "$APPIMAGE" "$DEB"
  # The key compiled into the app must accept what was just signed.
  "$TOOL" verify --version "$VERSION" "$APPIMAGE" "$DEB"
elif [[ $REQUIRE_SIG == 1 ]]; then
  echo "CSWAP_UPDATE_SIGNING_KEY is not set" >&2
  exit 1
else
  echo "warning: CSWAP_UPDATE_SIGNING_KEY not set: files are NOT signed (dev build)" >&2
fi
# 0.1.x -> v1 only (packaging/LEGACY-UPDATE.md).
"$TOOL" legacy-feed --version "$VERSION" --out "$OUT/latest-linux.yml" "$APPIMAGE"
"$BIN" --version
ls -l "$OUT"
