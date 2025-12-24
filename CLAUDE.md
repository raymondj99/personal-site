# Claude Code Instructions

Personal website with Rust WASM rain animation (droplet-engine) and SvelteKit frontend (web).

## Quick Start

```bash
# Build WASM
cd droplet-engine
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen --out-dir pkg --target web target/wasm32-unknown-unknown/release/droplet_engine.wasm

# Run dev server
cd web
pnpm install
pnpm dev
```

See [CONTRIBUTING.md](./CONTRIBUTING.md) for full setup, hot reload, and troubleshooting.
