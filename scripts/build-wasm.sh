#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
ENGINE_DIR="$ROOT_DIR/droplet-engine"

echo "Building droplet-engine..."
cd "$ENGINE_DIR"

cargo build --target wasm32-unknown-unknown --release

echo "Generating JS bindings..."
wasm-bindgen \
    --out-dir pkg \
    --target web \
    target/wasm32-unknown-unknown/release/droplet_engine.wasm

echo "Done! WASM package ready at $ENGINE_DIR/pkg"
