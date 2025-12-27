// splash.rs - Water splash effects
//
// Animated splash particles that appear on impact.

use super::{MAX_SPLASHES, RainWorld};

const SPLASH_FRAMES: u8 = 24;

pub struct Splashes {
    // Position
    pub x: [f32; MAX_SPLASHES],
    pub y: [f32; MAX_SPLASHES],
    pub z: [f32; MAX_SPLASHES],

    // Animation
    pub frame: [u8; MAX_SPLASHES],
    pub dir: [i8; MAX_SPLASHES],   // horizontal drift
    pub typ: [u8; MAX_SPLASHES],   // splash type (0-3)

    // Count
    pub n: usize,
}

impl Splashes {
    pub fn new() -> Self {
        Self {
            x: [0.0; MAX_SPLASHES],
            y: [0.0; MAX_SPLASHES],
            z: [0.0; MAX_SPLASHES],
            frame: [0; MAX_SPLASHES],
            dir: [0; MAX_SPLASHES],
            typ: [0; MAX_SPLASHES],
            n: 0,
        }
    }

    pub fn clear(&mut self) {
        self.n = 0;
    }

    /// Spawn a new splash
    pub fn spawn(&mut self, x: f32, y: f32, z: f32, typ: u8, rng: &mut u32) {
        if self.n >= MAX_SPLASHES { return; }

        let i = self.n;
        self.x[i] = x + (RainWorld::rand(rng) - 0.5) * 4.0;
        self.y[i] = y;
        self.z[i] = z;
        self.frame[i] = 0;
        self.dir[i] = (RainWorld::rand(rng) * 5.0) as i8 - 2;
        self.typ[i] = typ;
        self.n += 1;
    }

    /// Advance animation, remove finished splashes
    pub fn update(&mut self) {
        let mut write = 0;

        for read in 0..self.n {
            let frame = self.frame[read] + 1;
            if frame >= SPLASH_FRAMES { continue; }

            self.x[write] = self.x[read];
            self.y[write] = self.y[read];
            self.z[write] = self.z[read];
            self.frame[write] = frame;
            self.dir[write] = self.dir[read];
            self.typ[write] = self.typ[read];
            write += 1;
        }

        self.n = write;
    }
}
