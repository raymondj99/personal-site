// terrain.rs - Height and depth queries
//
// All coordinates are in background space (BG_WIDTH x BG_HEIGHT).
// Caller must convert from screen space if needed.

use crate::scene::{BG_WIDTH, BG_HEIGHT, BG_DEPTH, BG_GROUND, BG_NORMAL_X, BG_NORMAL_Y};

/// Depth at pixel (0.0 = far, 1.0 = near)
#[inline]
pub fn get_depth(x: usize, y: usize) -> f32 {
    if x >= BG_WIDTH || y >= BG_HEIGHT { return 0.0; }
    BG_DEPTH[y][x] as f32 / 255.0
}

/// Height at pixel (0.0 = low, 1.0 = high)
/// Inverse of depth - near objects are "lower" in world space
#[inline]
pub fn get_height(x: usize, y: usize) -> f32 {
    1.0 - get_depth(x, y)
}

/// Check if pixel is walkable ground
#[inline]
pub fn is_ground(x: usize, y: usize) -> bool {
    if x >= BG_WIDTH || y >= BG_HEIGHT { return false; }
    BG_GROUND[y][x] == 1
}

/// Raw depth value (0-255)
#[inline]
pub fn get_depth_raw(x: usize, y: usize) -> u8 {
    if x >= BG_WIDTH || y >= BG_HEIGHT { return 0; }
    BG_DEPTH[y][x]
}

/// Check if drop at depth z hits surface at (x, y)
/// z: 0.0 = near camera, 1.0 = far
#[inline(always)]
pub fn hits_surface(x: usize, y: usize, drop_z: f32, margin: u8) -> bool {
    if x >= BG_WIDTH || y >= BG_HEIGHT { return false; }

    let bg_depth = BG_DEPTH[y][x];

    // Skip sky (depth near 0)
    if bg_depth <= 30 { return false; }

    // Check depth match
    let drop_depth = ((1.0 - drop_z) * 255.0) as u8;
    let diff = (drop_depth as i16 - bg_depth as i16).unsigned_abs() as u8;
    diff < margin
}

/// Surface normal at pixel (returns x,y components, z assumed positive/up)
/// Normal points outward from surface. Values normalized to [-1, 1].
#[inline(always)]
pub fn get_normal(x: usize, y: usize) -> (f32, f32) {
    if x >= BG_WIDTH || y >= BG_HEIGHT { return (0.0, 0.0); }
    let nx = BG_NORMAL_X[y][x] as f32 / 127.0;
    let ny = BG_NORMAL_Y[y][x] as f32 / 127.0;
    (nx, ny)
}
