#!/usr/bin/env bash
# Builds the macOS release files into packaging/macos/out, on a Mac:
#
#   claude-swap-desktop-mac-arm64.dmg   Apple silicon
#   claude-swap-desktop-mac-x64.dmg     Intel (cross-compiled from either kind of Mac)
#   <each>.sig                          ed25519 signatures (CSWAP_UPDATE_SIGNING_KEY)
#
# The bundle is signed AD-HOC (`codesign -s -`), not with a Developer ID (decided 7 Sep
# 2026). That is not for trust: an unsigned bundle is not sealed, and macOS calls an
# unsealed quarantined app "damaged" with no way past it (0.1.21 with `identity: null`,
# PR #4); a sealed ad-hoc one gets "Open Anyway" in System Settings. No hardened runtime:
# it only matters for notarization.
#
#   packaging/macos/build.sh [--skip-build] [--require-signature] [--arch arm64|x64]...
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
SKIP_BUILD=0
REQUIRE_SIG=0
ARCHES=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1 ;;
    --require-signature) REQUIRE_SIG=1 ;;
    --arch) ARCHES+=("$2"); shift ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done
[[ ${#ARCHES[@]} -gt 0 ]] || ARCHES=(arm64 x64)

VERSION="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\([^"]*\)".*/\1/p' "$ROOT/Cargo.toml")"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "bad version '$VERSION'" >&2; exit 1; }
OUT="$HERE/out"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT"
cd "$ROOT"

# The release tool runs on this Mac, whatever the target.
[[ $SKIP_BUILD == 1 ]] || cargo build --release --locked -p cswap-update --bin cswap-release
TOOL="$ROOT/target/release/cswap-release"

# icon.icns from the 512 px PNG.
ICONSET="$WORK/icon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256; do
  sips -z $s $s "$ROOT/resources/icon.png" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) "$ROOT/resources/icon.png" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
cp "$ROOT/resources/icon.png" "$ICONSET/icon_512x512.png"
iconutil -c icns "$ICONSET" -o "$WORK/icon.icns"

for arch in "${ARCHES[@]}"; do
  case "$arch" in
    arm64) target=aarch64-apple-darwin ;;
    x64) target=x86_64-apple-darwin ;;
    *) echo "unknown arch $arch" >&2; exit 2 ;;
  esac
  if [[ $SKIP_BUILD == 0 ]]; then
    rustup target add "$target" >/dev/null
    cargo build --release --locked -p cswap-desktop --target "$target"
  fi
  APP="$WORK/$arch/Claude Swap.app"
  mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
  cp "$ROOT/target/$target/release/claude-swap" "$APP/Contents/MacOS/Claude Swap"
  cp "$WORK/icon.icns" "$APP/Contents/Resources/icon.icns"
  sed "s/@VERSION@/$VERSION/g" "$HERE/Info.plist.in" >"$APP/Contents/Info.plist"
  printf 'APPL????' >"$APP/Contents/PkgInfo"
  codesign --force --deep --sign - "$APP"
  codesign --verify --deep --strict --verbose=2 "$APP"

  # The DMG: the app and a link to /Applications to drag it onto.
  SRC="$WORK/$arch/dmg"
  mkdir -p "$SRC"
  ditto "$APP" "$SRC/Claude Swap.app"
  ln -s /Applications "$SRC/Applications"
  DMG="$OUT/claude-swap-desktop-mac-$arch.dmg"
  rm -f "$DMG"
  hdiutil create -volname "Claude Swap" -srcfolder "$SRC" -ov -format UDZO "$DMG" >/dev/null
  echo "wrote $DMG"

  if [[ -n "${CSWAP_UPDATE_SIGNING_KEY:-}" ]]; then
    "$TOOL" sign --version "$VERSION" "$DMG"
    "$TOOL" verify --version "$VERSION" "$DMG"
  elif [[ $REQUIRE_SIG == 1 ]]; then
    echo "CSWAP_UPDATE_SIGNING_KEY is not set" >&2
    exit 1
  else
    echo "warning: CSWAP_UPDATE_SIGNING_KEY not set: $DMG is NOT signed (dev build)" >&2
  fi
done
ls -l "$OUT"
