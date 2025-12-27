// droplet.rs - Falling rain droplets
//
// Structure-of-Arrays layout for cache-friendly iteration.

use super::{MAX_DROPS, RainWorld, Splashes, Streams};
use crate::world::{hits_surface, has_flow, get_normal};

// Physics constants
const GROUND_NEAR: f32 = 1.0;
const GROUND_FAR: f32 = 0.4;
const VEL_NEAR: f32 = 1.7;   // Near drops fall fast
const VEL_FAR: f32 = 0.35;   // Far drops fall slow (perspective)
const SPLASH_CHANCE: f32 = 0.7;
const DEPTH_MARGIN: u8 = 48;
const SLIDE_CHANCE: f32 = 0.6;

pub struct Droplets {
    // Position
    pub x: [f32; MAX_DROPS],
    pub y: [f32; MAX_DROPS],
    pub z: [f32; MAX_DROPS],  // depth: 0=near, 1=far

    // Velocity (y only, drops fall straight)
    pub v: [f32; MAX_DROPS],

    // Count
    pub n: usize,
}

impl Droplets {
    pub fn new() -> Self {
        Self {
            x: [0.0; MAX_DROPS],
            y: [0.0; MAX_DROPS],
            z: [0.0; MAX_DROPS],
            v: [0.0; MAX_DROPS],
            n: 0,
        }
    }

    pub fn clear(&mut self) {
        self.n = 0;
    }

    /// Spawn new drops at top of screen
    pub fn spawn(&mut self, count: usize, screen_w: f32, rng: &mut u32) {
        for _ in 0..count {
            if self.n >= MAX_DROPS { return; }

            let z = RainWorld::rand(rng);
            let i = self.n;

            self.x[i] = RainWorld::rand(rng) * screen_w;
            self.y[i] = -RainWorld::rand(rng) * 15.0;
            self.z[i] = z;

            // Velocity: near drops fall faster (perspective)
            self.v[i] = (VEL_NEAR + (VEL_FAR - VEL_NEAR) * z)
                      * (0.8 + RainWorld::rand(rng) * 0.4);

            self.n += 1;
        }
    }

    /// Update drop positions, handle collisions
    pub fn update(
        &mut self,
        screen_w: f32,
        screen_h: f32,
        scale_x: f32,
        scale_y: f32,
        splashes: &mut Splashes,
        streams: &mut Streams,
        rng: &mut u32,
    ) {
        let mut write = 0;

        for read in 0..self.n {
            let x = self.x[read];
            let y = self.y[read] + self.v[read];
            let z = self.z[read];

            // Ground line (perspective)
            let ground = screen_h * (GROUND_NEAR + (GROUND_FAR - GROUND_NEAR) * z);

            // Convert to background coords
            let bx = (x * scale_x) as usize;
            let by = (y * scale_y) as usize;

            // Surface collision (only if on screen)
            if y >= 0.0 && y < screen_h && x >= 0.0 && x < screen_w {
                if hits_surface(bx, by, z, DEPTH_MARGIN) {
                    // Hit a surface - spawn splash biased by surface normal
                    if has_flow(bx, by) && RainWorld::rand(rng) < SLIDE_CHANCE {
                        streams.spawn(x, y, z);
                    }
                    let (nx, ny) = get_normal(bx, by);
                    splashes.spawn_with_normal(x, y, z, nx, ny, rng);
                    continue;
                }
            }

            // Ground collision
            if y > ground {
                if RainWorld::rand(rng) < SPLASH_CHANCE {
                    let typ = (RainWorld::rand(rng) * 4.0) as u8;
                    splashes.spawn(x, ground, z, typ, rng);
                }
                continue;
            }

            // Keep falling
            self.x[write] = x;
            self.y[write] = y;
            self.z[write] = z;
            self.v[write] = self.v[read];
            write += 1;
        }

        self.n = write;
    }
}
