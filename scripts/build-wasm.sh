#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
ENGINE_DIR="$ROOT_DIR/droplet-engine"

cd "$ENGINE_DIR"

# If image path provided, generate background first
if [ -n "$1" ]; then
    echo "Generating pixel art from $1..."
    cargo run --release --bin img2scene -- "$1" "${@:2}"
    echo ""
fi

# Check if background.rs exists
if [ ! -f "src/background.rs" ]; then
    echo "Error: src/background.rs not found"
    echo ""
    echo "Usage: $0 <image_path> [--cols N] [--rows N] [--colors N]"
    echo ""
    echo "Run with an image to generate the background:"
    echo "  $0 path/to/image.jpg"
    exit 1
fi

echo "Building WASM..."
cargo build --lib --target wasm32-unknown-unknown --release

echo "Generating JS bindings..."
wasm-bindgen \
    --out-dir pkg \
    --target web \
    target/wasm32-unknown-unknown/release/droplet_engine.wasm

echo "Done! WASM package ready at $ENGINE_DIR/pkg"
