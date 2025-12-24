# Contributing

## Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (v18+)
- [pnpm](https://pnpm.io/) (`npm install -g pnpm`)

Install the WASM target and wasm-bindgen CLI:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
```

**Important:** The `wasm-bindgen-cli` version must match the `wasm-bindgen` crate version in `droplet-engine/Cargo.toml`. Check with:

```bash
wasm-bindgen --version
grep wasm-bindgen droplet-engine/Cargo.toml
```

If mismatched, update CLI: `cargo install wasm-bindgen-cli --version X.X.X`

## Project Structure

```
personal-site/
├── droplet-engine/     # Rust WASM rain simulation
│   ├── src/lib.rs
│   ├── Cargo.toml
│   └── pkg/            # Generated WASM package (git-ignored)
└── web/                # SvelteKit frontend
    └── src/
```

## Building the WASM Package

From the repository root:

```bash
# Build Rust to WASM
cd droplet-engine
cargo build --target wasm32-unknown-unknown --release

# Generate JS bindings
wasm-bindgen \
  --out-dir pkg \
  --target web \
  target/wasm32-unknown-unknown/release/droplet_engine.wasm
```

Or use the shorthand script:

```bash
./scripts/build-wasm.sh
```

## Development

### First Time Setup

```bash
# Build WASM package
cd droplet-engine
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen --out-dir pkg --target web target/wasm32-unknown-unknown/release/droplet_engine.wasm

# Install web dependencies
cd ../web
pnpm install
```

### Running the Dev Server

```bash
cd web
pnpm dev
```

Open http://localhost:5173

### Hot Reload for Rust Changes

For automatic rebuilding when Rust files change, install `cargo-watch`:

```bash
cargo install cargo-watch
```

Then run in separate terminals:

**Terminal 1 - Watch Rust:**
```bash
cd droplet-engine
cargo watch -s 'cargo build --target wasm32-unknown-unknown --release && wasm-bindgen --out-dir pkg --target web target/wasm32-unknown-unknown/release/droplet_engine.wasm'
```

**Terminal 2 - Vite Dev Server:**
```bash
cd web
pnpm dev
```

After Rust rebuilds, hard-refresh the browser (Cmd+Shift+R) to load the new WASM.

### Using Concurrently (Optional)

For single-command development, install concurrently in the web project:

```bash
cd web
pnpm add -D concurrently
```

Add to `web/package.json`:

```json
{
  "scripts": {
    "dev": "vite dev",
    "dev:all": "concurrently \"pnpm run watch:wasm\" \"vite dev\"",
    "watch:wasm": "cd ../droplet-engine && cargo watch -s 'cargo build --target wasm32-unknown-unknown --release && wasm-bindgen --out-dir pkg --target web target/wasm32-unknown-unknown/release/droplet_engine.wasm'"
  }
}
```

Then run:

```bash
pnpm dev:all
```

## Troubleshooting

### "Cannot find module 'droplet-engine'"

The WASM package needs rebuilding or reinstalling:

```bash
# Rebuild WASM
cd droplet-engine
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen --out-dir pkg --target web target/wasm32-unknown-unknown/release/droplet_engine.wasm

# Clear web cache and reinstall
cd ../web
rm -rf node_modules/.pnpm/droplet-engine*
pnpm install
```

### WASM changes not reflecting

1. Ensure `pkg/package.json` exists in droplet-engine
2. Hard refresh browser (Cmd+Shift+R or Ctrl+Shift+R)
3. Clear pnpm cache: `rm -rf node_modules/.pnpm/droplet-engine*`

### Version mismatch errors

Ensure wasm-bindgen CLI matches the crate version:

```bash
cargo install wasm-bindgen-cli --version $(grep -oP 'wasm-bindgen = "\K[^"]+' droplet-engine/Cargo.toml)
```
