# Claude Code Instructions

Rain simulation with AI-based scene understanding.

## Architecture

```
droplet-engine/
├── src/
│   ├── lib.rs              # WASM entry point
│   ├── scene/              # Auto-generated scene data
│   ├── world/              # Terrain queries (depth, flow)
│   ├── sim/                # Simulation (drops, splashes, streams)
│   └── render.rs           # Output encoding
│
├── src/bin/img2scene/      # CLI: image → scene data
│   ├── main.rs             # Pipeline entry
│   ├── color.rs            # Palette extraction
│   ├── ai.rs               # MiDaS + SegFormer
│   ├── geometry.rs         # Normals, flow, AO
│   └── export.rs           # Write Rust/TS

web/
├── src/lib/
│   ├── scene/              # Scene data (auto-generated)
│   ├── world/              # Terrain queries
│   └── components/         # Svelte components
```

## Build

```bash
# Process image with AI
cd droplet-engine
cargo run --bin img2scene -- path/to/image.jpg

# Build WASM
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen --out-dir pkg --target web \
  target/wasm32-unknown-unknown/release/droplet_engine.wasm

# Run web
cd ../web
pnpm install
pnpm dev
```

## Data Flow

```
Image → AI Models → Scene Data → Physics World → Simulation → Render
         (MiDaS)     (depth,      (queries)      (drops,      (output
         (SegFormer)  flow,                       splashes,    buffer)
                      ground)                     streams)
```

## Docs

- `docs/ARCHITECTURE.md` - System design
- `docs/SCENE.md` - AI pipeline and scene data
- `docs/SIMULATION.md` - Physics and entities
