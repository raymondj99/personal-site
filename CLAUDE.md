# Claude Code Instructions

Personal website with Rust WASM rain animation.

## Structure

```
droplet-engine/     Rust WASM simulation
web/                SvelteKit frontend
docs/               System documentation
```

## Build

```bash
cd droplet-engine
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen --out-dir pkg --target web target/wasm32-unknown-unknown/release/droplet_engine.wasm

cd ../web
pnpm install
pnpm dev
```

## Docs

- `docs/PERSPECTIVE.md` - Depth and ground plane
- `docs/DROPS.md` - Rain droplet system
- `docs/SPLASH.md` - Splash animation
