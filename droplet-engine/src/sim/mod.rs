// sim/ - Rain simulation
//
// Entity management using Structure-of-Arrays for cache efficiency.
// Each entity type in its own module.

mod droplet;
mod splash;
mod stream;

pub use droplet::Droplets;
pub use splash::Splashes;
pub use stream::Streams;

use crate::scene::{BG_WIDTH, BG_HEIGHT};
use crate::render::Encoder;

// Capacity limits
pub const MAX_DROPS: usize = 3000;
pub const MAX_SPLASHES: usize = 200;
pub const MAX_STREAMS: usize = 500;

/// Rain simulation world
pub struct RainWorld {
    // Screen dimensions
    w: u32,
    h: u32,

    // Precomputed scale factors (screen -> background)
    scale_x: f32,
    scale_y: f32,

    // Entities
    drops: Droplets,
    splashes: Splashes,
    streams: Streams,

    // Output
    encoder: Encoder,

    // RNG state
    rng: u32,
}

impl RainWorld {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            scale_x: BG_WIDTH as f32 / w as f32,
            scale_y: BG_HEIGHT as f32 / h as f32,
            drops: Droplets::new(),
            splashes: Splashes::new(),
            streams: Streams::new(),
            encoder: Encoder::new(w, h),
            rng: 0xDEADBEEF,
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.w = w;
        self.h = h;
        self.scale_x = BG_WIDTH as f32 / w as f32;
        self.scale_y = BG_HEIGHT as f32 / h as f32;
        self.encoder.resize(w, h);
        self.drops.clear();
        self.splashes.clear();
        self.streams.clear();
    }

    pub fn tick(&mut self) {
        self.encoder.clear();

        // Spawn new drops
        let spawn_count = ((self.w >> 6) + 1) as usize;
        self.drops.spawn(spawn_count, self.w as f32, &mut self.rng);

        // Update entities
        self.drops.update(
            self.w as f32,
            self.h as f32,
            self.scale_x,
            self.scale_y,
            &mut self.splashes,
            &mut self.streams,
            &mut self.rng,
        );

        self.splashes.update();
        self.streams.update(
            self.w as f32,
            self.h as f32,
            self.scale_x,
            self.scale_y,
            &mut self.splashes,
        );

        // Render to output buffer
        self.encoder.encode_drops(&self.drops, self.w as i32, self.h as i32);
        self.encoder.encode_splashes(&self.splashes, self.w as i32, self.h as i32, &mut self.rng);
        self.encoder.encode_streams(&self.streams, self.w as i32, self.h as i32);
    }

    // Random number generator (xorshift32)
    #[inline(always)]
    pub fn rand(rng: &mut u32) -> f32 {
        *rng ^= *rng << 13;
        *rng ^= *rng >> 17;
        *rng ^= *rng << 5;
        (*rng >> 8) as f32 * (1.0 / 16777216.0)
    }

    // Accessors for WASM
    pub fn output_ptr(&self) -> *const u8 { self.encoder.ptr() }
    pub fn output_len(&self) -> usize { self.encoder.len() }
    pub fn width(&self) -> u32 { self.w }
    pub fn height(&self) -> u32 { self.h }
}
