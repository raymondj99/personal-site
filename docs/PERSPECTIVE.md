# Perspective System

The rain simulation uses continuous depth (z-axis) to create a 3D perspective effect.

## Coordinate System

```
z = 0.0  →  Near (close to camera)
z = 1.0  →  Far (horizon)
```

Each droplet has a random z-value between 0 and 1. This determines:

- **Ground position**: Near drops hit the bottom of the screen, far drops hit the horizon
- **Velocity**: Near drops fall fast, far drops fall slow
- **Trail length**: Near drops have long trails (4-5 chars), far drops have short trails (1-2 chars)
- **Opacity/brightness**: Near drops are bright, far drops are dim

## Ground Plane

The ground is not flat - it recedes into the distance:

```
GROUND_NEAR = 1.0   (100% from top = bottom of screen)
GROUND_FAR  = 0.4   (40% from top = horizon line)

ground_y = height * lerp(GROUND_NEAR, GROUND_FAR, z)
```

## Depth Buckets

For rendering efficiency, continuous z-values are quantized into 8 depth buckets:

```
bucket = floor((1.0 - z) * 8)
```

This inverts z so that near objects (low z) get high bucket numbers (brighter colors).

## Painter's Algorithm

Objects are sorted by z (far to near) before rendering. Far objects draw first, near objects overwrite them. This ensures correct visual ordering without a z-buffer.
