# TODO: Image-Driven Physics Simulation Pipeline

**Goal:** Extract physical geometry from images using AI models, then use that geometry to drive realistic water, lighting, and wind simulations.

**Philosophy:** Let AI see the world, then simulate physics on what it sees.

---

## Research Summary

### Model Landscape (2024-2025)

| Model | Output | Strengths | Browser-Ready? |
|-------|--------|-----------|----------------|
| [Depth Anything V2/V3](https://github.com/DepthAnything/Depth-Anything-V2) | Depth only | Fast, accurate, small models available (25M-1.3B params) | Yes (Transformers.js) |
| [GeoWizard](https://github.com/fuxiao0719/GeoWizard) | Depth + Normals | Joint prediction, scene-aware | No (diffusion-based) |
| [Marigold](https://github.com/prs-eth/Marigold) | Depth + Normals | CVPR 2024 Best Paper, high detail | No (diffusion-based) |
| [Metric3D V2](https://github.com/YvanYin/Metric3D) | Metric Depth + Normals | Real-world scale, TPAMI 2024 | Partial (large models) |
| [MiDaS](https://github.com/isl-org/MiDaS) (current) | Depth + Normals | Proven, multiple backends | Yes (ONNX) |
| [SAM 2](https://github.com/facebookresearch/sam2) | Segmentation masks | Object isolation | Partial |

### Recommended Stack

**Offline Pipeline (Build Time):**
- **Depth + Normals:** GeoWizard or Marigold (highest quality)
- **Segmentation:** SAM 2 (object isolation)
- **Output:** Preprocessed geometry maps embedded in WASM

**Runtime Pipeline (Browser):**
- **Option A:** Pre-baked geometry (current approach, extended)
- **Option B:** [Transformers.js](https://huggingface.co/docs/transformers.js) + Depth Anything V2 Small (24M params)
- **Option C:** [WONNX](https://github.com/webonnx/wonnx) (Rust/WebGPU ONNX runtime)

---

## Current System Analysis

### What You Have (`droplet-engine/`)

```
lib.rs          - Rain simulation with depth-based collision
background.rs   - Pre-baked 320x180 pixel art + depth map
img2scene.rs    - MiDaS depth estimation + palette extraction
```

**Collision System (lib.rs:102-115):**
```rust
fn hits_surface(&self, x: f32, y: f32, drop_z: f32) -> bool {
    let bg_depth = BG_DEPTH[by][bx];
    let drop_depth = ((1.0 - drop_z) * 255.0) as u8;
    let diff = (drop_depth - bg_depth).abs();
    diff < DEPTH_MARGIN  // ~48 units tolerance
}
```

**Strengths:**
- Fast O(1) depth lookup
- Depth-aware collision working
- SoA layout for cache efficiency

**Gaps:**
- No surface normals (can't determine flow direction)
- No object segmentation (everything is "ground")
- No material properties (water behaves same everywhere)

---

## Phase 1: Enhanced Geometry Extraction

### 1.1 Upgrade Depth Model

Replace MiDaS with Depth Anything V2 for better quality:

```rust
// img2scene.rs - Updated model loading
fn estimate_depth_v2(img: &DynamicImage, tw: u32, th: u32) -> Vec<Vec<f32>> {
    // Depth Anything V2 Small: 24M params, ~50ms inference
    let model_path = "models/depth_anything_v2_small.onnx";
    // ... ONNX inference
}
```

**Tasks:**
- [ ] Download Depth Anything V2 Small ONNX model
- [ ] Update `estimate_depth()` to use new model
- [ ] Benchmark quality improvement vs MiDaS

**Resources:**
- [Depth Anything V2 ONNX](https://huggingface.co/onnx-community/depth-anything-v2-small)
- [Transformers.js depth demo](https://huggingface.co/spaces/Xenova/depth-anything-web)

### 1.2 Add Surface Normal Estimation

Surface normals are critical for water flow direction:

```rust
// New: normal.rs
pub struct NormalMap {
    nx: Vec<Vec<f32>>,  // X component (-1 to 1)
    ny: Vec<Vec<f32>>,  // Y component (-1 to 1)
    nz: Vec<Vec<f32>>,  // Z component (0 to 1, facing camera)
}

impl NormalMap {
    /// Derive normals from depth map using cross-product method
    pub fn from_depth(depth: &[Vec<f32>], scale: f32) -> Self {
        let (h, w) = (depth.len(), depth[0].len());
        let mut nx = vec![vec![0.0; w]; h];
        let mut ny = vec![vec![0.0; w]; h];
        let mut nz = vec![vec![1.0; w]; h];

        for y in 1..h-1 {
            for x in 1..w-1 {
                // Central differences
                let dzdx = (depth[y][x+1] - depth[y][x-1]) * scale;
                let dzdy = (depth[y+1][x] - depth[y-1][x]) * scale;

                // Normal = normalize(-dzdx, -dzdy, 1.0)
                let len = (dzdx*dzdx + dzdy*dzdy + 1.0).sqrt();
                nx[y][x] = -dzdx / len;
                ny[y][x] = -dzdy / len;
                nz[y][x] = 1.0 / len;
            }
        }
        Self { nx, ny, nz }
    }
}
```

**Alternative: Use GeoWizard for Joint Estimation**

For higher quality, run GeoWizard offline:

```bash
# Python preprocessing script
python geowizard_inference.py --input scene.jpg \
    --output-depth depth.npy \
    --output-normal normal.npy
```

**Tasks:**
- [ ] Implement `from_depth()` derivative method (quick, good enough)
- [ ] Create Python script for GeoWizard inference (optional, higher quality)
- [ ] Add normal map to `background.rs` output
- [ ] Compress normal map (quantize to u8, only store xy)

### 1.3 Object Segmentation with SAM 2

Segment distinct objects for different physics behaviors:

```rust
// New: segments.rs
pub struct SegmentMap {
    labels: Vec<Vec<u8>>,     // 0=sky, 1=ground, 2=water, 3+=objects
    properties: Vec<Material>,
}

pub struct Material {
    friction: f32,      // 0.0 = ice, 1.0 = rough stone
    absorption: f32,    // 0.0 = repels water, 1.0 = absorbs
    reflectivity: f32,  // For lighting
}
```

**Workflow:**
1. Run SAM 2 on input image (offline)
2. Manually or auto-label segments (sky, ground, roof, tree, etc.)
3. Assign material properties per segment
4. Bake into `background.rs`

**Tasks:**
- [ ] Create SAM 2 inference script
- [ ] Define material property schema
- [ ] Add `BG_SEGMENTS` and `BG_MATERIALS` to background.rs
- [ ] Update collision to use segment-aware behavior

---

## Phase 2: Water Flow Simulation

### 2.1 Gradient-Based Flow Direction

Water flows downhill. Use depth + normals to determine flow:

```rust
// New: flow.rs
pub struct FlowField {
    // Flow direction at each pixel (unit vectors)
    fx: Vec<Vec<f32>>,
    fy: Vec<Vec<f32>>,
    // Flow magnitude (steeper = faster)
    magnitude: Vec<Vec<f32>>,
}

impl FlowField {
    pub fn from_depth(depth: &[Vec<f32>]) -> Self {
        let (h, w) = (depth.len(), depth[0].len());
        let mut fx = vec![vec![0.0; w]; h];
        let mut fy = vec![vec![0.0; w]; h];
        let mut magnitude = vec![vec![0.0; w]; h];

        for y in 1..h-1 {
            for x in 1..w-1 {
                // Gradient points uphill; negate for downhill flow
                let gx = depth[y][x+1] - depth[y][x-1];
                let gy = depth[y+1][x] - depth[y-1][x];
                let mag = (gx*gx + gy*gy).sqrt();

                if mag > 0.001 {
                    fx[y][x] = -gx / mag;
                    fy[y][x] = -gy / mag;
                    magnitude[y][x] = mag;
                }
            }
        }
        Self { fx, fy, magnitude }
    }
}
```

### 2.2 Stream Particle System

Add a new particle type for sliding water:

```rust
// Extended rain.rs or new stream.rs
pub enum WaterState {
    Falling { velocity: f32 },
    Sliding {
        face_x: f32,
        face_y: f32,
        accumulated_water: f32,
    },
    Pooling { depth: f32 },
    Evaporating { timer: u8 },
}

pub struct StreamParticle {
    x: f32, y: f32, z: f32,
    vx: f32, vy: f32,
    volume: f32,
    state: WaterState,
}
```

### 2.3 Shallow Water Equations (Advanced)

For more realistic water accumulation and waves:

```rust
// New: shallow_water.rs
pub struct WaterGrid {
    // Height field
    height: Vec<Vec<f32>>,
    // Velocity field (staggered grid)
    vx: Vec<Vec<f32>>,  // Horizontal edges
    vy: Vec<Vec<f32>>,  // Vertical edges
    // Terrain height (from depth map)
    terrain: Vec<Vec<f32>>,
}

impl WaterGrid {
    pub fn step(&mut self, dt: f32, gravity: f32) {
        // 1. Update velocities from height differences
        for y in 0..self.h {
            for x in 0..self.w-1 {
                let dh = self.height[y][x] - self.height[y][x+1];
                self.vx[y][x] += gravity * dh * dt;
            }
        }
        // 2. Update heights from velocity divergence
        // 3. Enforce terrain constraints
        // 4. Apply boundary conditions
    }
}
```

**Resources:**
- [Shallow Water Simulation Tutorial](https://matthias-research.github.io/pages/publications/hfFluid.pdf)
- [Pipe Model Implementation](https://lisyarus.github.io/blog/posts/simulating-water-over-terrain.html)
- [Hydraulic Erosion Simulation](https://jobtalle.com/simulating_hydraulic_erosion.html)

**Tasks:**
- [ ] Implement `FlowField` from depth gradients
- [ ] Add `StreamParticle` type with sliding physics
- [ ] Convert rain splashes to stream particles on slopes
- [ ] Implement water accumulation at local minima
- [ ] (Optional) Add shallow water solver for pooling

---

## Phase 3: Lighting System

### 3.1 Screen-Space Ambient Occlusion

Use depth to approximate shadows in crevices:

```rust
// New: lighting.rs
pub fn calculate_ao(depth: &[Vec<f32>], x: usize, y: usize, radius: usize) -> f32 {
    let center_depth = depth[y][x];
    let mut occlusion = 0.0;
    let mut samples = 0;

    for dy in -(radius as i32)..=(radius as i32) {
        for dx in -(radius as i32)..=(radius as i32) {
            let sx = (x as i32 + dx).clamp(0, w-1) as usize;
            let sy = (y as i32 + dy).clamp(0, h-1) as usize;
            let sample_depth = depth[sy][sx];

            // Closer samples occlude more
            if sample_depth > center_depth {
                occlusion += (sample_depth - center_depth).min(0.1);
            }
            samples += 1;
        }
    }
    1.0 - (occlusion / samples as f32).min(1.0)
}
```

### 3.2 Normal-Based Directional Lighting

Apply simple diffuse lighting from normals:

```rust
pub fn calculate_diffuse(normal: &NormalMap, x: usize, y: usize, light_dir: (f32, f32, f32)) -> f32 {
    let nx = normal.nx[y][x];
    let ny = normal.ny[y][x];
    let nz = normal.nz[y][x];

    // Dot product with light direction
    let dot = nx * light_dir.0 + ny * light_dir.1 + nz * light_dir.2;
    dot.max(0.0)  // Clamp to [0, 1]
}
```

### 3.3 Dynamic Light Sources

Support moving light (e.g., time of day, cursor):

```rust
pub struct LightSource {
    position: (f32, f32, f32),  // Can be directional (far) or point (near)
    color: (f32, f32, f32),
    intensity: f32,
}

pub struct LightingSystem {
    ambient: f32,
    sources: Vec<LightSource>,
    ao_map: Vec<Vec<f32>>,      // Pre-baked AO
    normal_map: NormalMap,
}
```

**Tasks:**
- [ ] Pre-compute AO map in img2scene
- [ ] Add lighting uniforms to render shader
- [ ] Implement diffuse lighting from normals
- [ ] Add time-of-day light direction animation
- [ ] (Optional) Mouse-following light source

---

## Phase 4: Wind Simulation

### 4.1 Wind Field

Affect rain direction and particles:

```rust
// New: wind.rs
pub struct WindField {
    base_velocity: (f32, f32),   // Prevailing wind
    turbulence: f32,             // Random variation
    gusts: Vec<Gust>,            // Temporary strong winds
}

pub struct Gust {
    center: (f32, f32),
    radius: f32,
    strength: f32,
    direction: (f32, f32),
    lifetime: f32,
}

impl WindField {
    pub fn sample(&self, x: f32, y: f32, time: f32) -> (f32, f32) {
        let mut vx = self.base_velocity.0;
        let mut vy = self.base_velocity.1;

        // Add turbulence (Perlin noise)
        vx += noise(x * 0.01, y * 0.01, time) * self.turbulence;
        vy += noise(x * 0.01 + 100.0, y * 0.01, time) * self.turbulence;

        // Add gusts
        for gust in &self.gusts {
            let dist = ((x - gust.center.0).powi(2) + (y - gust.center.1).powi(2)).sqrt();
            if dist < gust.radius {
                let falloff = 1.0 - dist / gust.radius;
                vx += gust.direction.0 * gust.strength * falloff;
                vy += gust.direction.1 * gust.strength * falloff;
            }
        }

        (vx, vy)
    }
}
```

### 4.2 Wind-Affected Rain

Modify rain physics to respond to wind:

```rust
fn update_drops(&mut self, wind: &WindField, time: f32) {
    for i in 0..self.dn {
        let (wx, wy) = wind.sample(self.dx[i], self.dy[i], time);

        // Wind affects horizontal velocity
        self.dx[i] += wx * 0.1;

        // Wind can slow/speed vertical fall slightly
        self.dy[i] += self.dv[i] + wy * 0.05;
    }
}
```

### 4.3 Geometry-Aware Wind

Use normals to create wind shadows behind objects:

```rust
fn wind_occlusion(&self, x: f32, y: f32, wind_dir: (f32, f32), normal: &NormalMap) -> f32 {
    // Objects facing into wind block it
    let nx = normal.nx[y as usize][x as usize];
    let ny = normal.ny[y as usize][x as usize];

    let dot = nx * wind_dir.0 + ny * wind_dir.1;
    if dot > 0.5 {
        // This surface faces into wind, creates shadow behind
        0.3  // Reduce wind in shadow
    } else {
        1.0
    }
}
```

**Tasks:**
- [ ] Implement basic `WindField` with Perlin noise
- [ ] Add wind velocity to rain update loop
- [ ] Create gust system with random spawning
- [ ] Use normals for wind shadows
- [ ] Add visual wind indicators (leaves, dust)

---

## Phase 5: Runtime Inference (Optional)

### 5.1 Browser-Side Depth Estimation

For dynamic/user-uploaded images:

**Option A: Transformers.js**

```typescript
// web/src/lib/depth.ts
import { pipeline } from '@xenova/transformers';

const depth = await pipeline('depth-estimation',
    'onnx-community/depth-anything-v2-small',
    { device: 'webgpu' }  // Falls back to wasm
);

const result = await depth(imageUrl);
const depthMap = result.depth;  // Float32Array
```

**Option B: WONNX (Rust/WASM)**

```rust
// Requires wonnx-wasm crate
use wonnx::Session;

let session = Session::from_bytes(&model_bytes).await?;
let input = prepare_input(&image);
let output = session.run(&input).await?;
```

**Trade-offs:**
- Transformers.js: Easier setup, good community support
- WONNX: Rust-native, integrates with existing code, WebGPU acceleration

**Tasks:**
- [ ] Evaluate Transformers.js performance in-browser
- [ ] Prototype WONNX integration for depth model
- [ ] Measure latency: startup time, inference time
- [ ] Decide: pre-baked vs runtime based on use case

---

## Data Structure Summary

### Enhanced Background Format

```rust
// background.rs (generated)
pub const BG_WIDTH: usize = 320;
pub const BG_HEIGHT: usize = 180;

// Visual
pub static BG_PALETTE: [(u8,u8,u8); 32] = [...];
pub static BG_PIXELS: [[u8; BG_WIDTH]; BG_HEIGHT] = [...];

// Geometry (NEW)
pub static BG_DEPTH: [[u8; BG_WIDTH]; BG_HEIGHT] = [...];
pub static BG_NORMAL_XY: [[i8; BG_WIDTH * 2]; BG_HEIGHT] = [...];  // Packed xy

// Physics (NEW)
pub static BG_SEGMENTS: [[u8; BG_WIDTH]; BG_HEIGHT] = [...];  // Object labels
pub static BG_FLOW_XY: [[i8; BG_WIDTH * 2]; BG_HEIGHT] = [...];  // Pre-computed flow

// Lighting (NEW)
pub static BG_AO: [[u8; BG_WIDTH]; BG_HEIGHT] = [...];  // Ambient occlusion
```

### Memory Budget

| Data | Size (320x180) | Notes |
|------|----------------|-------|
| Pixels | 57.6 KB | 1 byte per pixel |
| Depth | 57.6 KB | 1 byte per pixel |
| Normals | 115.2 KB | 2 bytes (xy packed) |
| Segments | 57.6 KB | 1 byte per pixel |
| Flow | 115.2 KB | 2 bytes (xy packed) |
| AO | 57.6 KB | 1 byte per pixel |
| **Total** | **~460 KB** | Acceptable for WASM |

---

## Implementation Order

### Sprint 1: Foundation (Geometry Extraction)
1. [ ] Upgrade to Depth Anything V2
2. [ ] Implement normal map derivation from depth
3. [ ] Add flow field computation
4. [ ] Update img2scene to output all maps

### Sprint 2: Water Flow
1. [ ] Add `WaterState::Sliding` to particles
2. [ ] Implement flow-following movement
3. [ ] Create water accumulation pools
4. [ ] Connect rain splashes to stream system

### Sprint 3: Lighting
1. [ ] Pre-compute AO map
2. [ ] Add basic diffuse shading
3. [ ] Implement time-of-day cycle
4. [ ] Add specular highlights on water

### Sprint 4: Wind
1. [ ] Create wind field with Perlin noise
2. [ ] Apply wind to rain particles
3. [ ] Add random gusts
4. [ ] Implement wind shadows from geometry

### Sprint 5: Polish & Runtime
1. [ ] Optimize all maps for size
2. [ ] Evaluate runtime inference options
3. [ ] Add debug visualizations
4. [ ] Performance profiling

---

## Key Resources

### Models & Tools
- [Depth Anything V2](https://github.com/DepthAnything/Depth-Anything-V2) - Fast depth estimation
- [Depth Anything V3](https://github.com/ByteDance-Seed/Depth-Anything-3) - Latest with 3D Gaussians
- [GeoWizard](https://github.com/fuxiao0719/GeoWizard) - Joint depth + normals
- [Marigold](https://github.com/prs-eth/Marigold) - Diffusion-based depth + normals
- [Metric3D V2](https://github.com/YvanYin/Metric3D) - Metric depth + normals
- [SAM 2](https://github.com/facebookresearch/sam2) - Segmentation
- [WONNX](https://github.com/webonnx/wonnx) - Rust WebGPU ONNX runtime
- [Transformers.js](https://huggingface.co/docs/transformers.js) - Browser ML

### Water Simulation
- [Shallow Water Paper](https://matthias-research.github.io/pages/publications/hfFluid.pdf)
- [Water Over Terrain Blog](https://lisyarus.github.io/blog/posts/simulating-water-over-terrain.html)
- [Hydraulic Erosion](https://jobtalle.com/simulating_hydraulic_erosion.html)
- [Procedural Hydrology](https://nickmcd.me/2020/04/15/procedural-hydrology/)

### Lighting
- [LearnOpenGL Shadow Mapping](https://learnopengl.com/Advanced-Lighting/Shadows/Shadow-Mapping)
- [Screen Space Shadows](https://panoskarabelas.com/posts/screen_space_shadows/)

### Demos
- [Depth Anything Web Demo](https://huggingface.co/spaces/Xenova/depth-anything-web)
- [Realtime Depth WebGPU](https://huggingface.co/spaces/Xenova/webgpu-realtime-depth-estimation)

---

## Notes on "Onyx Model"

You mentioned "onyx model" - this likely refers to **ONNX** (Open Neural Network Exchange), which is the model format used for cross-platform inference. All the models above can be exported to ONNX format for use with:

- **ort** (Rust) - Used in your current `img2scene.rs`
- **ONNX Runtime Web** - Browser JavaScript
- **WONNX** - Rust WebGPU runtime
- **Transformers.js** - Uses ONNX under the hood

The key insight is that **any** of the depth/normal models (Depth Anything, GeoWizard, Marigold, Metric3D) can be converted to ONNX and integrated into your pipeline.

---

## Success Criteria

- [ ] Rain responds to surface geometry (slides down slopes)
- [ ] Water accumulates in valleys and depressions
- [ ] Lighting creates depth perception
- [ ] Wind affects rain realistically
- [ ] 60 FPS with all systems active
- [ ] Works on any input image (not just pre-made scenes)
