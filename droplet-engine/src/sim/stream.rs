// stream.rs - Sliding water streams
//
// Water particles that slide along surfaces following the flow field.

use super::{MAX_STREAMS, Splashes};
use crate::world::{get_flow, has_flow, hits_surface};

const FLOW_SPEED: f32 = 0.4;
const FLOW_LIFETIME: u8 = 120;
const DEPTH_MARGIN: u8 = 48;

pub struct Streams {
    // Position
    pub x: [f32; MAX_STREAMS],
    pub y: [f32; MAX_STREAMS],
    pub z: [f32; MAX_STREAMS],

    // Lifetime
    pub life: [u8; MAX_STREAMS],

    // Count
    pub n: usize,
}

impl Streams {
    pub fn new() -> Self {
        Self {
            x: [0.0; MAX_STREAMS],
            y: [0.0; MAX_STREAMS],
            z: [0.0; MAX_STREAMS],
            life: [0; MAX_STREAMS],
            n: 0,
        }
    }

    pub fn clear(&mut self) {
        self.n = 0;
    }

    /// Spawn a new stream particle
    pub fn spawn(&mut self, x: f32, y: f32, z: f32) {
        if self.n >= MAX_STREAMS { return; }

        let i = self.n;
        self.x[i] = x;
        self.y[i] = y;
        self.z[i] = z;
        self.life[i] = FLOW_LIFETIME;
        self.n += 1;
    }

    /// Move streams along flow field
    pub fn update(
        &mut self,
        screen_w: f32,
        screen_h: f32,
        scale_x: f32,
        scale_y: f32,
        splashes: &mut Splashes,
    ) {
        let mut rng = 0x12345678u32; // Local RNG for splashes
        let mut write = 0;

        for read in 0..self.n {
            let life = self.life[read];
            if life == 0 { continue; }

            let mut x = self.x[read];
            let mut y = self.y[read];
            let z = self.z[read];

            // Get flow at current position
            let bx = (x * scale_x) as usize;
            let by = (y * scale_y) as usize;
            let (fx, fy) = get_flow(bx, by);

            // Move along flow (slower when far for perspective)
            let speed = FLOW_SPEED * (1.0 - z * 0.5);
            x += fx * speed;
            y += fy * speed;

            // Check bounds
            if x < 0.0 || x >= screen_w || y < 0.0 || y >= screen_h {
                continue;
            }

            // Check if still on surface
            let bx = (x * scale_x) as usize;
            let by = (y * scale_y) as usize;

            if !hits_surface(bx, by, z, DEPTH_MARGIN) {
                // Fell off - splash
                if life > 60 {
                    splashes.spawn(x, y, z, 2, &mut rng);
                }
                continue;
            }

            // Check if flow stopped (reached pool)
            if !has_flow(bx, by) {
                splashes.spawn(x, y, z, 0, &mut rng);
                continue;
            }

            // Keep sliding
            self.x[write] = x;
            self.y[write] = y;
            self.z[write] = z;
            self.life[write] = life - 1;
            write += 1;
        }

        self.n = write;
    }
}
