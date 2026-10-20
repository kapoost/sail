#!/usr/bin/env bash
# Build the game for a browser.
#
#   dist/web.sh            # build into dist/out/web
#   dist/web.sh --serve    # and then serve it on http://localhost:8777
#
# What this does that `cargo build` does not: it picks the `web` profile (stripped —
# the debug information is most of a wasm module, 144 MB against 62), it asks for
# WebGPU, because the sea reads its spectrum out of a storage buffer and WebGL2 has no
# such thing, and it puts the assets beside the page, because on the web the asset
# server fetches them over HTTP rather than unpacking them out of the binary.
#
# `wasm-bindgen-cli` has to match the `wasm-bindgen` in `Cargo.lock` exactly, which is
# checked below rather than discovered as a stack trace in a browser console.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"
out="dist/out/web"
serve=""
for arg in "$@"; do
  case "$arg" in
    --serve) serve=1 ;;
    *) out="$arg" ;;
  esac
done

want="$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/[^0-9.]/, "", $0); print; exit}' Cargo.lock)"
if ! command -v wasm-bindgen >/dev/null; then
  echo "sail: wasm-bindgen-cli is not installed. cargo install -f wasm-bindgen-cli --version $want" >&2
  exit 1
fi
has="$(wasm-bindgen --version | awk '{print $2}')"
if [ "$has" != "$want" ]; then
  echo "sail: wasm-bindgen is $has and the lockfile says $want." >&2
  echo "      cargo install -f wasm-bindgen-cli --version $want" >&2
  exit 1
fi
if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
  echo "sail: rustup target add wasm32-unknown-unknown" >&2
  exit 1
fi

echo "sail: building for the browser (web profile, webgpu)"
cargo build --profile web --target wasm32-unknown-unknown -p sail-app --features webgpu

rm -rf "$out"
mkdir -p "$out"
wasm-bindgen --out-dir "$out" --target web --no-typescript \
  --remove-name-section --remove-producers-section \
  target/wasm32-unknown-unknown/web/sail-app.wasm

# Binaryen if it happens to be here, and no fuss if it is not: `-Oz` takes a few more
# megabytes off, and the stripped module is already inside what a page should ask for.
if command -v wasm-opt >/dev/null; then
  echo "sail: wasm-opt -Oz"
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    "$out/sail-app_bg.wasm" -o "$out/sail-app_bg.wasm"
fi

cp dist/web/index.html "$out/index.html"
cp -R assets "$out/assets"
# The hull is ten megabytes and the page does not need the one under evaluation.
rm -f "$out/assets/trellis-hull.glb"

size="$(du -m "$out/sail-app_bg.wasm" | cut -f1)"
wire="$(gzip -9 -c "$out/sail-app_bg.wasm" | wc -c | awk '{printf "%.1f", $1/1e6}')"
echo "sail: $out — ${size} MB of wasm, ${wire} MB gzipped, assets beside it"

if [ -n "$serve" ]; then
  echo "sail: http://localhost:8777/  (ctrl-c to stop)"
  cd "$out" && exec python3 -m http.server 8777
fi
