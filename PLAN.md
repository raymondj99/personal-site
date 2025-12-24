# Rain Animation Overhaul

Personal website with realistic ASCII rain backdrop.

## Goals

1. Full-screen rain animation as page background
2. Realistic rain physics and appearance
3. Subtle rain colors (blue-gray-white, not matrix green)
4. Performance: 60fps with thousands of droplets
5. Prepare for anime backdrop overlay

---

## Architecture

### Data-Oriented Design (Rust/WASM)

Current: Array of Structs (AoS) - poor cache utilization
```rust
struct Droplet { x, y, velocity_y, length, alive }
Vec<Droplet>
```

New: Struct of Arrays (SoA) - cache-friendly, SIMD-ready
```rust
struct RainLayer {
    xs: Vec<f32>,
    ys: Vec<f32>,
    velocities: Vec<f32>,
    lengths: Vec<u8>,
    count: usize,
    capacity: usize,
}
```

### Depth Layers

Three rain layers create parallax depth:

| Layer | Speed | Size | Opacity | Count |
|-------|-------|------|---------|-------|
| Far   | 0.3x  | 1-2  | 30%     | 40%   |
| Mid   | 0.6x  | 2-3  | 60%     | 35%   |
| Near  | 1.0x  | 3-5  | 100%    | 25%   |

Single simulation tick updates all layers. Far drops move less per frame (slower apparent velocity = further away).

---

## Rust Implementation

### Core Simulation (`lib.rs`)

```rust
use wasm_bindgen::prelude::*;

const MAX_DROPLETS_PER_LAYER: usize = 2000;
const NUM_LAYERS: usize = 3;

// Characters by intensity: subtle to prominent
const RAIN_CHARS: [char; 5] = ['.', '·', ':', '|', '│'];

#[wasm_bindgen]
pub struct RainWorld {
    width: u32,
    height: u32,

    // SoA layout per layer - far [0], mid [1], near [2]
    x: [[f32; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
    y: [[f32; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
    vel: [[f32; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
    len: [[u8; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
    count: [usize; NUM_LAYERS],

    // Output buffer - single allocation, reused
    output: Vec<u8>,

    // Wind: horizontal drift (-1.0 to 1.0)
    wind: f32,

    // Spawn rates per layer
    spawn_rates: [f32; NUM_LAYERS],

    // Base velocities per layer (depth multiplier)
    base_vel: [f32; NUM_LAYERS],

    frame: u32,
}

#[wasm_bindgen]
impl RainWorld {
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> Self {
        // Layer configs: [far, mid, near]
        let base_vel = [0.3, 0.6, 1.0];
        let spawn_rates = [0.4, 0.35, 0.25];

        Self {
            width,
            height,
            x: [[0.0; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
            y: [[0.0; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
            vel: [[0.0; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
            len: [[0; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
            count: [0; NUM_LAYERS],
            output: vec![b' '; (width * height) as usize],
            wind: 0.0,
            spawn_rates,
            base_vel,
            frame: 0,
        }
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);

        // Gentle wind variation using sine
        self.wind = (self.frame as f32 * 0.01).sin() * 0.15;

        // Clear output buffer
        self.output.fill(b' ');

        // Update each layer
        for layer in 0..NUM_LAYERS {
            self.spawn_layer(layer);
            self.update_layer(layer);
            self.render_layer(layer);
        }
    }

    fn spawn_layer(&mut self, layer: usize) {
        let count = &mut self.count[layer];
        if *count >= MAX_DROPLETS_PER_LAYER {
            return;
        }

        // Spawn probability based on layer
        let spawn_count = (self.width as f32 * self.spawn_rates[layer] * 0.1) as usize;

        for _ in 0..spawn_count {
            if *count >= MAX_DROPLETS_PER_LAYER {
                break;
            }

            let i = *count;
            self.x[layer][i] = fastrand() * self.width as f32;
            self.y[layer][i] = -(fastrand() * 10.0); // Start above screen

            // Velocity varies by layer + small random variation
            let base = self.base_vel[layer];
            self.vel[layer][i] = base * (0.8 + fastrand() * 0.4);

            // Length: far drops shorter, near drops longer
            self.len[layer][i] = match layer {
                0 => 1 + (fastrand() * 2.0) as u8,
                1 => 2 + (fastrand() * 2.0) as u8,
                _ => 3 + (fastrand() * 3.0) as u8,
            };

            *count += 1;
        }
    }

    fn update_layer(&mut self, layer: usize) {
        let mut write_idx = 0;
        let count = self.count[layer];
        let height = self.height as f32;
        let wind = self.wind * self.base_vel[layer]; // Wind affected by depth

        for read_idx in 0..count {
            let y = self.y[layer][read_idx] + self.vel[layer][read_idx];

            // Remove if below screen
            if y > height + 5.0 {
                continue;
            }

            // Compact arrays (remove dead droplets)
            self.x[layer][write_idx] = self.x[layer][read_idx] + wind;
            self.y[layer][write_idx] = y;
            self.vel[layer][write_idx] = self.vel[layer][read_idx];
            self.len[layer][write_idx] = self.len[layer][read_idx];
            write_idx += 1;
        }

        self.count[layer] = write_idx;
    }

    fn render_layer(&mut self, layer: usize) {
        let w = self.width;
        let h = self.height;

        // Character set varies by layer (depth)
        let chars: &[u8] = match layer {
            0 => b"..",   // Far: dots only
            1 => b".::",  // Mid: dots and colons
            _ => b".:|",  // Near: full variety
        };

        for i in 0..self.count[layer] {
            let x = self.x[layer][i] as i32;
            let base_y = self.y[layer][i] as i32;
            let len = self.len[layer][i] as i32;

            if x < 0 || x >= w as i32 {
                continue;
            }

            // Render droplet trail
            for dy in 0..len {
                let y = base_y - dy;
                if y < 0 || y >= h as i32 {
                    continue;
                }

                let idx = (y as u32 * w + x as u32) as usize;

                // Head is brightest char, tail fades
                let char_idx = if dy == 0 {
                    chars.len() - 1
                } else {
                    (chars.len() - 1).saturating_sub(dy as usize)
                };

                // Near layers overwrite far layers
                self.output[idx] = chars[char_idx.min(chars.len() - 1)];
            }
        }
    }

    /// Returns pointer to output buffer for zero-copy JS access
    pub fn output_ptr(&self) -> *const u8 {
        self.output.as_ptr()
    }

    pub fn output_len(&self) -> usize {
        self.output.len()
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.output.resize((width * height) as usize, b' ');
        self.clear();
    }

    pub fn clear(&mut self) {
        self.count = [0; NUM_LAYERS];
        self.output.fill(b' ');
    }

    pub fn droplet_count(&self) -> usize {
        self.count.iter().sum()
    }
}

// Fast PRNG (xorshift) - avoid js_sys::Math::random overhead
static mut SEED: u32 = 0xDEADBEEF;

fn fastrand() -> f32 {
    unsafe {
        SEED ^= SEED << 13;
        SEED ^= SEED >> 17;
        SEED ^= SEED << 5;
        (SEED as f32) / (u32::MAX as f32)
    }
}
```

---

## Frontend Implementation

### Svelte Component (`RainCanvas.svelte`)

```svelte
<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import init, { RainWorld } from "droplet-engine";

    let canvas: HTMLCanvasElement;
    let ctx: CanvasRenderingContext2D;
    let world: RainWorld;
    let memory: WebAssembly.Memory;
    let animationId: number;
    let charWidth: number;
    let charHeight: number;

    // Rain color palette (subtle blue-gray-white)
    const COLORS = {
        far: 'rgba(120, 140, 160, 0.3)',
        mid: 'rgba(160, 180, 200, 0.6)',
        near: 'rgba(200, 210, 220, 0.9)',
    };

    onMount(async () => {
        const wasm = await init();
        memory = wasm.memory;

        ctx = canvas.getContext('2d')!;

        // Measure character dimensions
        ctx.font = '14px "JetBrains Mono", monospace';
        const metrics = ctx.measureText('|');
        charWidth = metrics.width;
        charHeight = 14;

        resize();
        window.addEventListener('resize', resize);

        loop();
    });

    onDestroy(() => {
        if (animationId) cancelAnimationFrame(animationId);
        window.removeEventListener('resize', resize);
    });

    function resize() {
        const cols = Math.floor(window.innerWidth / charWidth);
        const rows = Math.floor(window.innerHeight / charHeight);

        canvas.width = window.innerWidth;
        canvas.height = window.innerHeight;

        if (world) {
            world.resize(cols, rows);
        } else {
            world = new RainWorld(cols, rows);
        }

        // Reset font after canvas resize
        ctx.font = '14px "JetBrains Mono", monospace';
        ctx.textBaseline = 'top';
    }

    function loop() {
        world.tick();
        render();
        animationId = requestAnimationFrame(loop);
    }

    function render() {
        const w = world.width();
        const h = world.height();
        const ptr = world.output_ptr();
        const len = world.output_len();

        // Zero-copy access to WASM memory
        const output = new Uint8Array(memory.buffer, ptr, len);

        // Clear
        ctx.fillStyle = '#0a0a0f';
        ctx.fillRect(0, 0, canvas.width, canvas.height);

        // Render characters
        ctx.fillStyle = COLORS.near;

        for (let y = 0; y < h; y++) {
            for (let x = 0; x < w; x++) {
                const char = output[y * w + x];
                if (char === 32) continue; // Skip spaces

                // Color based on character (depth indicator)
                if (char === 46) { // '.'
                    ctx.fillStyle = COLORS.far;
                } else if (char === 58) { // ':'
                    ctx.fillStyle = COLORS.mid;
                } else {
                    ctx.fillStyle = COLORS.near;
                }

                ctx.fillText(
                    String.fromCharCode(char),
                    x * charWidth,
                    y * charHeight
                );
            }
        }
    }
</script>

<canvas bind:this={canvas}></canvas>

<style>
    canvas {
        position: fixed;
        top: 0;
        left: 0;
        width: 100vw;
        height: 100vh;
        z-index: -1;
    }
</style>
```

### Page Layout (`+page.svelte`)

```svelte
<script lang="ts">
    import RainCanvas from "$lib/components/RainCanvas.svelte";
</script>

<svelte:head>
    <title>Your Name</title>
</svelte:head>

<RainCanvas />

<main>
    <!-- Content overlays rain background -->
    <slot />
</main>

<style>
    main {
        position: relative;
        z-index: 1;
        min-height: 100vh;
    }
</style>
```

---

## Implementation Order

1. **Rust rewrite** - Replace `lib.rs` with SoA design, depth layers, fast PRNG
2. **Rebuild WASM** - `cargo build --target wasm32-unknown-unknown --release && wasm-bindgen ...`
3. **New Svelte component** - Canvas-based renderer with proper colors
4. **Full-screen layout** - Fixed position background, content overlay
5. **Performance tuning** - Adjust spawn rates, layer counts for target density

---

## Performance Notes

- **Zero-copy rendering**: JS reads directly from WASM linear memory via `output_ptr()`
- **No allocations in hot path**: Fixed-size arrays, output buffer reused
- **Minimal branching**: Layer configs in arrays indexed by layer number
- **Cache-friendly iteration**: SoA layout keeps related data contiguous
- **Fast PRNG**: Inline xorshift avoids JS interop overhead for random numbers

Target: 60fps with 3000+ droplets on mid-range hardware.

---

## Future: Anime Backdrop

Once rain works:
1. Add `<img>` or `<div>` behind rain canvas with anime art
2. Rain canvas uses `mix-blend-mode` or partial transparency
3. Rain appears to fall "in front of" the scene
4. Consider: rain splashes on "ground" level of artwork
