# Rain Drops

Droplets fall from the top of the screen and hit the ground plane.

## Data Layout

Structure of Arrays (SoA) for cache efficiency:

```rust
dx: [f32; MAX_DROPS]  // x position
dy: [f32; MAX_DROPS]  // y position
dz: [f32; MAX_DROPS]  // depth (0=near, 1=far)
dv: [f32; MAX_DROPS]  // velocity
dn: usize             // active count
```

## Lifecycle

1. **Spawn**: Random x, z. Start above screen (negative y). Velocity from z.
2. **Update**: Add velocity to y. Check ground collision.
3. **Ground hit**: Spawn splash, remove droplet.
4. **Removal**: Swap-remove to keep array compact.

## Trail Rendering

Each droplet renders as a vertical trail:

```
trail_length = max(1, 5 - z * 4)

Position 0: |  (head, brightest)
Position 1: :
Position 2: .
Position 3: .  (tail, dimmest)
```

## Output Encoding

Droplets write to output buffer:

```
encoded = bucket * 4 + trail_position + 1
```

Where bucket is 0-7 (depth) and trail_position is 0-3.
