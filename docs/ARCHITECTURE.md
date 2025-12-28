# Architecture

Carmack-style design: clear layers, minimal abstraction, data-oriented.

## Layers

```
┌─────────────────────────────────────────────────────────────┐
│  img2scene (CLI)                                            │
│  Image → AI inference → Scene data generation               │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│  scene/ - Auto-generated data                               │
│  BG_DEPTH, BG_FLOW_X/Y, BG_GROUND, BG_NORMAL_X/Y, etc.     │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│  world/ - Terrain queries (pure functions)                  │
│  hits_surface(), get_flow(), has_flow(), get_normal()       │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│  sim/ - Simulation entities                                 │
│  Droplets, Splashes, Streams (SoA layout)                   │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│  render - Output encoding                                   │
│  Encode entity state → byte buffer for canvas               │
└─────────────────────────────────────────────────────────────┘
```

## Rust Modules

### `scene/`
Auto-generated data from AI analysis. Do not edit manually.
- `data.rs` - All `BG_*` constants and arrays

### `world/`
Pure functions to query scene geometry. No state, no allocation.
All hot-path functions use `#[inline(always)]` for performance.
- `terrain.rs` - Surface collision, normals
- `flow.rs` - Flow direction and magnitude

### `sim/`
Entity management using Structure-of-Arrays (SoA) for cache efficiency.
- `mod.rs` - `RainWorld` struct, main simulation loop
- `droplet.rs` - Falling rain drops
- `splash.rs` - Impact animations
- `stream.rs` - Sliding water particles

### `render.rs`
Encode simulation state to output buffer. Output is a flat byte array
where each byte encodes entity type, depth bucket, and variant.

## TypeScript Modules

### `scene/`
Mirror of Rust scene data for client-side use.
- `data.ts` - Auto-generated (same content as Rust)

### `world/`
Terrain query functions for 3D topology viewer.
- `terrain.ts` - Height, flow, normals (pure functions)

### `components/`
Svelte components for rendering.
- `RainCanvas.svelte` - Main rain simulation canvas with debug overlays
- `AsciiCanvas.svelte` - Alternative ASCII renderer

### Debug Overlays
Press keys to toggle visualization overlays:
- `D` - Depth map (MiDaS output)
- `F` - Flow field arrows
- `S` - Semantic segmentation (ADE20K classes)
- `G` - Ground mask
- `N` - Surface normals (color + arrows)
- `↑/↓` - Adjust overlay opacity

FPS counter always visible in top-right corner.

## Design Principles

1. **One purpose per file** - Each module does exactly one thing
2. **Data flows down** - scene → world → sim → render
3. **Pure functions** - world/ has no side effects
4. **SoA layout** - Arrays of components, not array of structs
5. **Minimal abstraction** - No unnecessary indirection
