#!/bin/bash
# Klik kanan → Open (pertama kali), atau klik dua kali di Finder.
# Membuat dist/Nusa-<versi>.dmg siap install.
cd "$(dirname "$0")" || exit 1

pause() { echo; read -n1 -r -p "Tekan tombol apa saja untuk menutup jendela ini…"; echo; }

echo "=============================="
echo "   Membuat Nusa.dmg"
echo "=============================="
echo

if [[ "$(uname)" != "Darwin" ]]; then
  echo "File ini hanya untuk macOS."; pause; exit 1
fi

# 1. Xcode Command Line Tools (linker, codesign, hdiutil)
if ! xcode-select -p >/dev/null 2>&1; then
  echo "→ Xcode Command Line Tools belum ada. Dialog instalasi akan muncul."
  echo "  Setelah selesai, jalankan file ini lagi."
  xcode-select --install >/dev/null 2>&1 || true
  pause; exit 1
fi

# 2. Rust
if ! command -v cargo >/dev/null 2>&1; then
  if [[ -f "$HOME/.cargo/env" ]]; then
    source "$HOME/.cargo/env"
  else
    echo "→ Menginstal Rust (sekali saja)…"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal || { echo "Gagal menginstal Rust."; pause; exit 1; }
    source "$HOME/.cargo/env"
  fi
fi

# 3. Build. Default: hanya arsitektur Mac ini (lebih cepat).
#    Untuk DMG yang jalan di Apple Silicon + Intel: ARCH=universal ./Buat-DMG.command
if [[ -z "${ARCH:-}" ]]; then
  if [[ "$(uname -m)" == "arm64" ]]; then export ARCH=arm64; else export ARCH=x86_64; fi
fi
echo "→ Build Nusa ($ARCH). Build pertama ±3–6 menit…"
echo
if ./scripts/bundle-macos.sh; then
  echo
  echo "✓ Selesai! Buka DMG di folder 'dist', lalu seret Nusa ke Applications."
  open dist/
else
  echo
  echo "✗ Build gagal. Salin pesan error di atas untuk diperbaiki."
fi
pause
