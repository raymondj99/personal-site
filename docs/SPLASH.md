# Splash System

When a droplet hits the ground, there's a 40% chance it spawns a splash animation.

## Data Layout

```rust
sx: [f32; MAX_SPLASHES]  // x position
sz: [f32; MAX_SPLASHES]  // depth (inherited from droplet)
sf: [u8; MAX_SPLASHES]   // frame counter
sd: [i8; MAX_SPLASHES]   // direction (-2 to +2)
st: [u8; MAX_SPLASHES]   // type (0-3)
sn: usize                // active count
```

## Splash Types

Four distinct splash animations add visual variety:

### Type 0: Crown
Classic symmetric crown splash with optional tilt.
```
  ' ' '     Droplets rise
 \ | /      Crown forms
  \./       Wings emerge
   .        Impact
```

### Type 1: Left Burst
Asymmetric splash biased to the left.
```
'  '        Droplets fly left
\ |         Left-heavy crown
 \.         Left wing
  .         Impact
```

### Type 2: Right Burst
Asymmetric splash biased to the right.
```
  '  '      Droplets fly right
   | /      Right-heavy crown
    ./      Right wing
    .       Impact
```

### Type 3: Spray
Scattered droplets without clear structure.
```
'   '  '    Random scatter
  '   '     Chaos
 '    '     More chaos
   .        Impact
```

## Direction

Random offset (-2 to +2) applied to x-positions, creating tilt and asymmetry even within symmetric splash types.

## Depth Scaling

```
scale = floor((1.0 - z) * 2.5)

scale = 0: Minimal animation (far splashes)
scale = 1: Small splash
scale = 2: Large splash (near)
```

## Character Set

```
Index 0: .   (center dot)
Index 1: |   (vertical spike)
Index 2: '   (flying droplet)
Index 3: *   (unused)
Index 4: \   (left wing)
Index 5: /   (right wing)
Index 6: .   (dissipating)
Index 7: .   (dissipating)
```

## Output Encoding

```
encoded = 33 + bucket * 8 + char_index
```
