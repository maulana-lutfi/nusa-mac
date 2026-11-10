#!/usr/bin/env bash
# Build Nusa.app (universal: Apple Silicon + Intel) dan Nusa.dmg.
# Jalankan di Mac:  ./scripts/bundle-macos.sh
# Opsi: ARCH=arm64 (hanya Apple Silicon, lebih cepat) | ARCH=x86_64 | ARCH=universal (default)
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$PWD"
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
ARCH="${ARCH:-universal}"
OUT="$ROOT/dist"
APP="$OUT/Nusa.app"
export MACOSX_DEPLOYMENT_TARGET=12.0

if [[ "$(uname)" != "Darwin" ]]; then
  echo "Script ini harus dijalankan di macOS." >&2
  exit 1
fi
command -v cargo >/dev/null || { echo "Install Rust dulu: https://rustup.rs" >&2; exit 1; }

build() {
  local target="$1"
  rustup target add "$target" >/dev/null 2>&1 || true
  cargo build --release --target "$target"
}

echo "==> Build ($ARCH) v$VERSION"
rm -rf "$OUT" && mkdir -p "$OUT"
case "$ARCH" in
  arm64)   build aarch64-apple-darwin; BIN="target/aarch64-apple-darwin/release/nusa" ;;
  x86_64)  build x86_64-apple-darwin;  BIN="target/x86_64-apple-darwin/release/nusa" ;;
  *)
    build aarch64-apple-darwin
    build x86_64-apple-darwin
    lipo -create -output "$OUT/nusa" \
      target/aarch64-apple-darwin/release/nusa \
      target/x86_64-apple-darwin/release/nusa
    BIN="$OUT/nusa"
    ;;
esac

echo "==> Bundle"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/nusa"
sed "s/__VERSION__/$VERSION/g" packaging/Info.plist > "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"

# Ikon .icns dari PNG 1024px
ICONSET="$OUT/AppIcon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s packaging/icon.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d packaging/icon.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

# Tanda tangan ad-hoc (cukup untuk jalan lokal). Untuk distribusi publik,
# ganti "-" dengan "Developer ID Application: …" lalu notarize.
SIGN_ID="${SIGN_ID:--}"
codesign --force --deep --options runtime --sign "$SIGN_ID" "$APP"

echo "==> DMG"
STAGE="$OUT/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Nusa" -srcfolder "$STAGE" -ov -format UDZO "$OUT/Nusa-$VERSION.dmg" >/dev/null
rm -rf "$STAGE" "$OUT/nusa"

echo
echo "Selesai:"
du -sh "$APP" "$OUT/Nusa-$VERSION.dmg"
echo
echo "Install:  buka dist/Nusa-$VERSION.dmg lalu seret Nusa ke Applications."
echo "Atau langsung jalankan:  open \"$APP\""
