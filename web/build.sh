#!/bin/sh
# Builds dist/centrality-explorer.html: the app compiled to WebAssembly and
# wrapped as one self-contained page (an Eigendeck demo, or open it in any
# browser). Needs the wasm32-unknown-unknown target and wasm-bindgen-cli at
# the version in Cargo.lock. Set WASM_OPT=1 to shrink the module with
# wasm-opt; binaryen 108 (Debian 12) produces a module browsers reject, so use
# a recent release.
set -eu
cd "$(dirname "$0")/.."
TARGET_DIR="${CARGO_TARGET_DIR:-target}"
CARGO_PROFILE_RELEASE_LTO=true CARGO_PROFILE_RELEASE_OPT_LEVEL=s \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_PANIC=abort \
  cargo rustc --release --lib --crate-type cdylib --target wasm32-unknown-unknown
mkdir -p dist/bindgen
wasm-bindgen --target web --no-typescript --out-dir dist/bindgen \
  "$TARGET_DIR/wasm32-unknown-unknown/release/centrality_explorer.wasm"
WASM=dist/bindgen/centrality_explorer_bg.wasm
if [ "${WASM_OPT:-0}" = 1 ]; then
  wasm-opt -Oz --strip-debug --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue \
    -o dist/bindgen/opt.wasm "$WASM"
  WASM=dist/bindgen/opt.wasm
fi
python3 web/wrap_demo.py dist/bindgen "$WASM" dist/centrality-explorer.html
