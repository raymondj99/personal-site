# Simulation

Rain physics with depth-aware collision and surface flow.

## Coordinate Systems

### Screen Space
- Origin: top-left
- X: 0 to screen_width (pixels)
- Y: 0 to screen_height (pixels)

### Depth Space
- Z: 0.0 (near camera) to 1.0 (far/horizon)
- Affects velocity, size, opacity

### Background Space
- Maps screen to scene data arrays
- `bx = x * BG_WIDTH / screen_width`
- `by = y * BG_HEIGHT / screen_height`

## Entities

All entities use Structure-of-Arrays (SoA) for cache efficiency.

### Droplets

Falling rain particles.

```rust
struct Droplets {
    x: [f32; MAX_DROPS],   // screen x
    y: [f32; MAX_DROPS],   // screen y
    z: [f32; MAX_DROPS],   // depth (0=near, 1=far)
    v: [f32; MAX_DROPS],   // velocity
    n: usize,              // count
}
```

**Lifecycle:**
1. Spawn at top with random x, z
2. Fall with velocity based on z (near=fast, far=slow)
3. Check surface collision each frame
4. On collision: spawn splash/stream, remove drop

**Velocity:**
```
v = lerp(VEL_NEAR, VEL_FAR, z) * random(0.8, 1.2)

VEL_NEAR = 1.7   (near drops fall fast)
VEL_FAR  = 0.35  (far drops fall slow)
```

### Splashes

Impact animations at collision points.

```rust
struct Splashes {
    x: [f32; MAX],
    y: [f32; MAX],
    z: [f32; MAX],
    frame: [u8; MAX],      // animation frame
    dir: [i8; MAX],        // horizontal drift
    typ: [u8; MAX],        // splash type (0-3)
    n: usize,
}
```

**Types:**
- 0: Crown (symmetric)
- 1: Left burst
- 2: Right burst
- 3: Spray (scattered)

**Surface Normal Influence:**

When a drop hits a surface, the splash direction is biased by the surface normal:

```rust
// Horizontal drift follows normal x-component
dir = nx * 6.0 + random(-1, 1)

// Splash type probability scales with tilt
dir_prob = 0.3 + |nx| * 0.6

if random() < dir_prob {
    typ = if nx < 0 { LEFT_BURST } else { RIGHT_BURST }
} else {
    typ = CROWN or SPRAY
}
```

Even slight surface tilts bias splash direction. A surface tilting right
(positive nx) produces right-bursting splashes with rightward drift.

**Animation:** 24 frames, 8 keyframes (frame/3)

### Streams

Sliding water particles that flow along surfaces.

```rust
struct Streams {
    x: [f32; MAX],
    y: [f32; MAX],
    z: [f32; MAX],
    life: [u8; MAX],       // frames remaining
    n: usize,
}
```

**Behavior:**
1. Spawn when drop hits sloped surface
2. Move along flow field each frame
3. Speed scaled by depth (perspective)
4. Remove when: off-screen, left surface, reached pool, lifetime expired

**Flow sampling:**
```
(fx, fy) = get_flow(bx, by)  // from scene data
x += fx * FLOW_SPEED * (1 - z * 0.5)
y += fy * FLOW_SPEED * (1 - z * 0.5)
```

## Collision Detection

### Surface Collision

Drop hits a surface when its depth matches the scene depth:

```rust
fn hits_surface(x, y, drop_z) -> bool {
    let scene_depth = BG_DEPTH[by][bx];

    // Skip sky
    if scene_depth <= 30 { return false; }

    // Convert drop z to depth (0=near becomes 255)
    let drop_depth = (1 - drop_z) * 255;

    // Check if close enough
    abs(drop_depth - scene_depth) < DEPTH_MARGIN
}
```

`DEPTH_MARGIN = 48` allows some tolerance.

### Ground Collision

Fallback when no surface hit. Ground recedes with perspective:

```
ground_y = height * lerp(GROUND_NEAR, GROUND_FAR, z)

GROUND_NEAR = 1.0  (100% = bottom of screen)
GROUND_FAR  = 0.4  (40% = horizon)
```

## Output Encoding

Single byte per screen cell:

```
0        = empty
1-32     = drops   (8 depths × 4 trail lengths)
33-96    = splashes (8 depths × 8 characters)
97-128   = streams (8 depths × 4 sizes)
```

### Depth Buckets

Continuous z quantized to 8 levels:
```
bucket = floor((1 - z) * 8)
```

Near objects (low z) get high bucket numbers (brighter).

### Drop Encoding
```
encoded = bucket * 4 + trail_position + 1

trail_position 0: | (head)
trail_position 1: :
trail_position 2: .
trail_position 3: . (tail)
```

### Splash Encoding
```
encoded = 33 + bucket * 8 + char_index

char 0: .  (center)
char 1: |  (spike)
char 2: '  (droplet)
char 4: \  (left wing)
char 5: /  (right wing)
char 6: .  (fade)
```

### Stream Encoding
```
encoded = 97 + bucket * 4 + size

size 0-3 based on remaining lifetime
```

## Rendering

Client reads output buffer and draws characters:

```typescript
for (y = 0; y < h; y++) {
    for (x = 0; x < w; x++) {
        const code = buffer[y * w + x];
        if (code === 0) continue;

        // Decode and draw appropriate character
        // with color based on depth bucket
    }
}
```

Characters drawn in bucket order (far to near) for correct occlusion.

## Performance

### Entity Limits
```
MAX_DROPS    = 3000
MAX_SPLASHES = 200
MAX_STREAMS  = 500
```

### Optimizations

**Structure-of-Arrays (SoA):**
Entity data stored as separate arrays per component, not array of structs.
This maximizes cache hits when iterating over a single component (e.g., all positions).

**Inline Hot Paths:**
World query functions (`hits_surface`, `get_flow`, `has_flow`, `get_normal`)
use `#[inline(always)]` to eliminate function call overhead.

**XORshift RNG:**
Fast pseudo-random number generator (~3 cycles, no branching):
```rust
fn rand(rng: &mut u32) -> f32 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    (*rng >> 8) as f32 * (1.0 / 16777216.0)
}
```

**Swap Removal:**
Dead entities removed by overwriting with live entities from end of array.
O(1) removal, maintains contiguous iteration.

**Pre-computed Scale Factors:**
Screen-to-background coordinate conversion factors computed once on resize,
not per-entity per-frame.

### FPS Monitoring
Client-side FPS counter updates every second via `requestAnimationFrame` timestamp.
Color-coded: green (≥55), yellow (≥30), red (<30).
