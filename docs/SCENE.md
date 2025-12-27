# Scene Data

Scene geometry extracted from images using AI models.

## Pipeline

```
Input Image
    ↓
┌───────────────┐
│ Resize        │  Target resolution (320x180 default)
└───────────────┘
    ↓
┌───────────────┐
│ K-means       │  Extract color palette (32 colors)
│ + Dither      │  Floyd-Steinberg to indexed pixels
└───────────────┘
    ↓
┌───────────────┐
│ MiDaS         │  Monocular depth estimation
│ (256x256)     │  Output: 0=far, 1=near
└───────────────┘
    ↓
┌───────────────┐
│ SegFormer     │  Semantic segmentation (ADE20K)
│ (512x512)     │  Output: class ID per pixel (0-149)
└───────────────┘
    ↓
┌───────────────┐
│ Derived Maps  │  Computed from depth + segments
└───────────────┘
    ↓
Output: Rust + TypeScript source files
```

## Generated Data

### Visual
- `BG_PALETTE` - Color palette as RGB tuples
- `BG_PIXELS` - Indexed pixel values (0 to palette size)

### Depth
- `BG_DEPTH` - Depth map (0=far, 255=near)

### Surface
- `BG_NORMAL_X/Y` - Surface normals as packed i8 (-127 to 127)
- `BG_AO` - Ambient occlusion (0=dark, 255=bright)

### Flow
- `BG_FLOW_X/Y` - Water flow direction as packed i8
- `BG_GROUND` - Walkable surface mask (0=no, 1=yes)

### Semantic
- `BG_SEGMENTS` - ADE20K class ID per pixel

## Depth Map

MiDaS produces relative depth (not metric). Values are normalized to [0, 255].

```
0   = Far (sky, horizon)
255 = Near (close objects)
```

Height in world space is inverse: `height = 1 - depth/255`

## Ground Mask

Computed from semantic segmentation using an exclusion list:

```rust
NON_GROUND_CLASSES = [
    2,   // sky
    4,   // tree
    17,  // plant
    72,  // palm tree
]
```

Any pixel not in this list is considered ground where water can flow.

## Flow Field

Water flow direction computed from depth gradient:

```
flow_direction = gradient(depth)  // steepest descent
flow_speed = sqrt(slope)          // Manning's equation
```

Only computed on ground surfaces. Non-ground areas have (0, 0) flow.

Multi-scale gradient sampling captures both fine detail and overall slope:
- Fine scale (2px) - Local variations
- Medium scale (5px) - Surface contours
- Coarse scale (10px) - Overall terrain slope

## Surface Normals

Derived from depth using central differences:

```
dz/dx = (depth[x+1] - depth[x-1]) / 2
dz/dy = (depth[y+1] - depth[y-1]) / 2
normal = normalize(-dz/dx, -dz/dy, 1)
```

Packed as i8: `nx = normal.x * 127`

## Usage

Run the pipeline:
```bash
cd droplet-engine
cargo run --bin img2scene -- image.jpg --cols 320 --rows 180 --colors 32
```

Output files:
- `src/scene/data.rs` - Rust source
- `../web/src/lib/scene/data.ts` - TypeScript source
