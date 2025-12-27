// flow.rs - Water flow field queries
//
// Flow direction is derived from depth gradient.
// Water flows toward lower elevation (higher depth values).

use crate::scene::{BG_WIDTH, BG_HEIGHT, BG_FLOW_X, BG_FLOW_Y};

/// Flow direction at pixel, returns (fx, fy) in range [-1.0, 1.0]
/// Returns (0, 0) if no flow (flat or non-ground)
#[inline]
pub fn get_flow(x: usize, y: usize) -> (f32, f32) {
    if x >= BG_WIDTH || y >= BG_HEIGHT { return (0.0, 0.0); }

    let fx = BG_FLOW_X[y][x] as f32 / 127.0;
    let fy = BG_FLOW_Y[y][x] as f32 / 127.0;
    (fx, fy)
}

/// Check if there's significant flow at this position
/// Returns false for flat areas where water would pool
#[inline]
pub fn has_flow(x: usize, y: usize) -> bool {
    if x >= BG_WIDTH || y >= BG_HEIGHT { return false; }

    let fx = BG_FLOW_X[y][x].abs();
    let fy = BG_FLOW_Y[y][x].abs();
    fx > 10 || fy > 10
}

/// Flow strength (0.0 = no flow, 1.0 = max flow)
#[inline]
pub fn flow_strength(x: usize, y: usize) -> f32 {
    if x >= BG_WIDTH || y >= BG_HEIGHT { return 0.0; }

    let fx = BG_FLOW_X[y][x] as f32;
    let fy = BG_FLOW_Y[y][x] as f32;
    ((fx * fx + fy * fy).sqrt() / 127.0).min(1.0)
}
