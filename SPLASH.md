# 3D Rain with Perspective Projection

## Overview

Replace the discrete depth layers with true 3D coordinates (x, y, z) and perspective projection. Droplets exist in a 3D world space and are projected onto the 2D screen using the same math as classic 3D games.

## How Classic 3D Games Work

### Wolfenstein 3D / Raycasting
Wolfenstein uses **raycasting**: for each vertical column of the screen, cast a ray from the player into the world to find what wall it hits. The distance determines the wall height (closer = taller). This works great for grid-based wall rendering but isn't ideal for particles.

### Perspective Projection (What We'll Use)
For individual objects/particles, games use **perspective projection**:

```
screen_x = (world_x * focal_length) / world_z + screen_center_x
screen_y = (world_y * focal_length) / world_z + screen_center_y
scale    = focal_length / world_z
```

Key insight: **divide by Z**. Objects farther away (larger Z) appear:
- Closer to the center of the screen
- Smaller in size
- (For rain) we also make them dimmer/more transparent

## 3D World Space

```
        +Y (up)
         |
         |
         |_______ +X (right)
        /
       /
      +Z (into screen / depth)
```

- **Camera** at origin (0, 0, 0), looking down +Z axis
- **Ground plane** at Y = 0, extending in X and Z
- **Rain spawns** at high Y (above camera view), random X and Z
- **Rain falls** by decreasing Y each frame
- **Splash occurs** when droplet.y <= 0 (hits ground)

## Coordinate System

```rust
// World space bounds
const WORLD_X_RANGE: f32 = 100.0;   // -50 to +50
const WORLD_Z_MIN: f32 = 10.0;      // near plane (closest rain)
const WORLD_Z_MAX: f32 = 200.0;     // far plane (farthest rain)
const WORLD_Y_MAX: f32 = 80.0;      // rain spawn height
const GROUND_Y: f32 = -20.0;        // ground plane (below camera creates looking-down effect)

// Camera
const FOCAL_LENGTH: f32 = 200.0;    // higher = narrower FOV, less distortion
```

## Projection Math

```rust
fn project(world_x: f32, world_y: f32, world_z: f32,
           screen_w: f32, screen_h: f32) -> (f32, f32, f32) {
    // Perspective divide
    let scale = FOCAL_LENGTH / world_z;

    let screen_x = world_x * scale + screen_w * 0.5;
    let screen_y = -world_y * scale + screen_h * 0.5;  // flip Y (screen Y increases downward)

    (screen_x, screen_y, scale)
}
```

The `scale` factor is used for:
- Character size (near = larger glyphs, far = smaller)
- Opacity (near = brighter, far = dimmer)
- Trail length (near = longer trails)

## Data Structure

```rust
const MAX_DROPLETS: usize = 4000;
const MAX_SPLASHES: usize = 1000;

#[wasm_bindgen]
pub struct RainWorld {
    // Screen dimensions
    screen_w: u32,
    screen_h: u32,

    // Droplets in 3D world space (SoA layout)
    drop_x: [f32; MAX_DROPLETS],
    drop_y: [f32; MAX_DROPLETS],
    drop_z: [f32; MAX_DROPLETS],
    drop_vel: [f32; MAX_DROPLETS],    // fall speed (Y velocity)
    drop_len: [u8; MAX_DROPLETS],     // trail length
    drop_count: usize,

    // Splashes on ground plane (Y = GROUND_Y)
    splash_x: [f32; MAX_SPLASHES],
    splash_z: [f32; MAX_SPLASHES],
    splash_age: [u8; MAX_SPLASHES],
    splash_size: [u8; MAX_SPLASHES],
    splash_count: usize,

    // Output buffer
    output: Vec<u8>,

    // PRNG
    seed: u32,
}
```

## Rendering Pipeline

### 1. Sort by Z (Painter's Algorithm)
Render far objects first, near objects last. Near objects overwrite far ones.

```rust
fn tick(&mut self) {
    self.spawn_droplets();
    self.update_droplets();    // move, check ground collision
    self.update_splashes();    // age, remove old

    self.output.fill(0);

    // Sort and render back-to-front
    self.render_droplets_sorted();
    self.render_splashes_sorted();
}
```

### 2. Depth-Based Appearance

| Z Distance | Opacity | Character | Trail Length |
|------------|---------|-----------|--------------|
| 10-40      | 80-100% | `\|` `:` `.` | 3-5 cells |
| 40-100     | 40-70%  | `\|` `:` | 2-3 cells |
| 100-200    | 15-35%  | `:` `.` | 1-2 cells |

### 3. Splash Projection
Splashes exist at Y = GROUND_Y with varying X and Z. Project to screen:

```rust
let (sx, sy, scale) = project(splash.x, GROUND_Y, splash.z, screen_w, screen_h);
// sy will be near bottom of screen (ground)
// scale determines splash size
```

## Splash Animation

Splashes expand outward on the ground plane:

```
Age 0-2:   .        (impact point)
Age 3-5:   o        (small circle)
Age 6-10:  ( )      (expanding ring)
Age 11-15: .   .    (dissipating)
```

In 3D, the splash ring expands in X while staying at fixed Z:
```rust
// Ring points at angle theta
let ring_x = splash.x + cos(theta) * radius;
let ring_z = splash.z + sin(theta) * radius * 0.3;  // foreshortened in Z
```

## Output Encoding (Extended)

```
0      = empty
1-32   = droplet (encoded depth + trail position)
33-48  = splash (encoded depth + animation frame)
```

Or use separate buffers and composite in JS.

## Implementation Steps

1. **Replace layer arrays with 3D coordinates**
   - Remove `x[layer]`, `y[layer]` arrays
   - Add `drop_x`, `drop_y`, `drop_z` flat arrays

2. **Implement projection function**
   - `project(x, y, z) -> (screen_x, screen_y, scale)`

3. **Update spawn logic**
   - Random X in world range
   - Random Z between near and far planes
   - Y starts above visible area

4. **Update movement**
   - Decrease Y by velocity each frame
   - Velocity can vary by droplet (bigger drops fall faster)

5. **Ground collision detection**
   - When `drop_y <= GROUND_Y`, spawn splash at (drop_x, drop_z)

6. **Implement depth sorting**
   - Simple approach: iterate Z far-to-near
   - Better: maintain sorted indices

7. **Add splash system**
   - Spawn, age, animate, remove
   - Project splash position to screen

8. **Update renderer (Svelte)**
   - Decode depth from output
   - Map depth to opacity/color
   - Render splash characters

9. **Rebuild and tune**
   - Adjust FOCAL_LENGTH for desired perspective
   - Tune spawn rates, velocities, fade curves

## Visual Tuning Parameters

```rust
// Perspective
FOCAL_LENGTH: f32       // 100-400, affects FOV
WORLD_Z_MIN: f32        // near plane
WORLD_Z_MAX: f32        // far plane

// Rain behavior
SPAWN_RATE: f32         // droplets per frame
VELOCITY_MIN: f32       // slowest fall speed
VELOCITY_MAX: f32       // fastest fall speed

// Appearance
OPACITY_NEAR: f32       // opacity at Z_MIN
OPACITY_FAR: f32        // opacity at Z_MAX
TRAIL_LENGTH_NEAR: u8   // trail at Z_MIN
TRAIL_LENGTH_FAR: u8    // trail at Z_MAX
```

## Performance Notes

- Sorting 4000 droplets each frame is expensive
  - Alternative: bucket by Z range, render buckets back-to-front
  - Or: don't sort, accept minor artifacts (rain is chaotic anyway)
- Projection is just multiply + divide per droplet (fast)
- Cache `1/z` if doing multiple operations per droplet

## Comparison: Layers vs True 3D

| Aspect | Discrete Layers | True 3D |
|--------|-----------------|---------|
| Depth values | 3 (far/mid/near) | Continuous |
| Parallax | Stepped | Smooth |
| Sorting | Trivial (3 passes) | Required |
| Perspective | Approximated | Mathematically correct |
| Ground plane | Per-layer Y | Single plane in world space |
| Complexity | Lower | Higher |
| Visual quality | Good | Better |
