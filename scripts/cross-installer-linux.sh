#!/usr/bin/env bash
# Builds the NSIS installer for Windows (GNU/MinGW target) from Linux. Dev-only helper, like cross-build-linux.sh.
# On Windows or in CI use the normal command:  npm run tauri build
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH=/usr/lib/rust-1.91/bin:$PATH
export RUSTC_BOOTSTRAP=1
export CARGO_UNSTABLE_BUILD_STD="std,panic_abort"
SC=/usr/lib/rust-1.91/lib/rustlib/x86_64-pc-windows-gnu/lib/self-contained
cp "$SC/rsbegin.o" "$SC/rsend.o" .
trap 'rm -f rsbegin.o rsend.o' EXIT
# --config adds WebView2Loader.dll next to the exe (needed by the GNU build) and skips the WebView2
# bootstrapper download (this sandbox has no access to Microsoft servers).
npx tauri build --ci --target x86_64-pc-windows-gnu --bundles nsis --config src-tauri/tauri.cross.conf.json -- --locked
