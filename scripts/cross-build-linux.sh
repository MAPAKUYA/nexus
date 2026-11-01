#!/usr/bin/env bash
# Кросс-сборка NEXUS.exe для Windows (GNU/MinGW) из Linux. Нужна только в среде без Windows.
# На Windows используйте обычную сборку: `npm install && npm run tauri build`.
#
# Требования (Ubuntu 24.04): rustc-1.91, cargo-1.91, rust-1.91-src (с полным Windows-кодом в
# library/), gcc-mingw-w64-x86-64, binutils-mingw-w64-x86-64, nodejs.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH=/usr/lib/rust-1.91/bin:$PATH
export RUSTC_BOOTSTRAP=1                      # нужен для -Zbuild-std на стабильном компиляторе
SC=/usr/lib/rust-1.91/lib/rustlib/x86_64-pc-windows-gnu/lib/self-contained
cp "$SC/rsbegin.o" "$SC/rsend.o" .            # линкер ищет их в cwd
trap 'rm -f rsbegin.o rsend.o' EXIT
npm run build
cargo build --locked --release --target x86_64-pc-windows-gnu -p nexus \
  --features custom-protocol -Zbuild-std=std,panic_abort "$@"
