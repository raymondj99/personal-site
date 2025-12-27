/**
 * terrain.ts - Terrain geometry computations
 *
 * Pure functions for computing terrain properties from depth data.
 * No rendering code - just math.
 */

import { BG_WIDTH, BG_HEIGHT, BG_DEPTH, BG_GROUND } from '../scene';

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface Vec2 {
    x: number;
    y: number;
}

export interface Vec3 {
    x: number;
    y: number;
    z: number;
}

// -----------------------------------------------------------------------------
// Depth & Height
// -----------------------------------------------------------------------------

/** Get normalized depth at pixel (0 = far, 1 = near) */
export function getDepth(x: number, y: number): number {
    if (x < 0 || x >= BG_WIDTH || y < 0 || y >= BG_HEIGHT) return 0;
    return BG_DEPTH[y][x] / 255;
}

/** Get ground height at pixel (0 = low, 1 = high) */
export function getHeight(x: number, y: number): number {
    return 1 - getDepth(x, y);
}

/** Check if pixel is walkable ground */
export function isGround(x: number, y: number): boolean {
    if (x < 0 || x >= BG_WIDTH || y < 0 || y >= BG_HEIGHT) return false;
    return BG_GROUND[y][x] === 1;
}

// -----------------------------------------------------------------------------
// Surface Normal
// -----------------------------------------------------------------------------

/** Compute surface normal at pixel using central differences */
export function getNormal(x: number, y: number): Vec3 {
    const s = 2; // sample distance

    // Height gradients
    const dhdx = getHeight(x + s, y) - getHeight(x - s, y);
    const dhdy = getHeight(x, y + s) - getHeight(x, y - s);

    // Normal = (-dh/dx, 1, -dh/dy) normalized
    // Scale height gradient to match world space
    const scale = 4.0;
    const nx = -dhdx * scale;
    const ny = 1.0;
    const nz = -dhdy * scale;

    const len = Math.sqrt(nx * nx + ny * ny + nz * nz);
    return { x: nx / len, y: ny / len, z: nz / len };
}

// -----------------------------------------------------------------------------
// Flow Field
// -----------------------------------------------------------------------------

/** Compute water flow direction at pixel (steepest descent) */
export function getFlowDirection(x: number, y: number): Vec2 | null {
    const s = 3; // sample distance

    // Height gradients (positive = uphill)
    const dhdx = getHeight(x + s, y) - getHeight(x - s, y);
    const dhdy = getHeight(x, y + s) - getHeight(x, y - s);

    // Flow goes downhill (negative gradient)
    const fx = -dhdx;
    const fy = -dhdy;

    const len = Math.sqrt(fx * fx + fy * fy);
    if (len < 0.008) return null; // flat area

    return { x: fx / len, y: fy / len };
}

/** Compute flow speed from slope (Manning's equation: v ~ sqrt(slope)) */
export function getFlowSpeed(x: number, y: number): number {
    const s = 3;

    const dhdx = getHeight(x + s, y) - getHeight(x - s, y);
    const dhdy = getHeight(x, y + s) - getHeight(x, y - s);

    const slope = Math.sqrt(dhdx * dhdx + dhdy * dhdy);
    return Math.sqrt(slope);
}

// -----------------------------------------------------------------------------
// Batch Computation (for visualization)
// -----------------------------------------------------------------------------

export interface FlowSample {
    x: number;
    y: number;
    dir: Vec2;
    speed: number;
    height: number;
}

/** Sample flow field on a grid */
export function sampleFlowField(spacing: number): FlowSample[] {
    const samples: FlowSample[] = [];
    const margin = spacing;

    for (let y = margin; y < BG_HEIGHT - margin; y += spacing) {
        for (let x = margin; x < BG_WIDTH - margin; x += spacing) {
            if (!isGround(x, y)) continue;

            const dir = getFlowDirection(x, y);
            if (!dir) continue;

            samples.push({
                x,
                y,
                dir,
                speed: getFlowSpeed(x, y),
                height: getHeight(x, y)
            });
        }
    }

    return samples;
}

export interface NormalSample {
    x: number;
    y: number;
    normal: Vec3;
    height: number;
}

/** Sample surface normals on a grid */
export function sampleNormals(spacing: number): NormalSample[] {
    const samples: NormalSample[] = [];
    const margin = spacing;

    for (let y = margin; y < BG_HEIGHT - margin; y += spacing) {
        for (let x = margin; x < BG_WIDTH - margin; x += spacing) {
            samples.push({
                x,
                y,
                normal: getNormal(x, y),
                height: getHeight(x, y)
            });
        }
    }

    return samples;
}

// -----------------------------------------------------------------------------
// Constants (re-export for convenience)
// -----------------------------------------------------------------------------

export { BG_WIDTH, BG_HEIGHT };
