use wasm_bindgen::prelude::*;

mod background;
use background::{BG_WIDTH, BG_HEIGHT, BG_DEPTH};

// Capacity
const MAX_DROPS: usize = 3000;
const MAX_SPLASHES: usize = 200;

// Physics
const GROUND_NEAR: f32 = 1.0;
const GROUND_FAR: f32 = 0.4;
const VEL_NEAR: f32 = 1.2;
const VEL_FAR: f32 = 0.25;
const SPLASH_CHANCE: f32 = 0.7;
const DEPTH_MARGIN: u8 = 48;

// Encoding: 1-32 = drops (8 depths × 4 trail), 33-96 = splashes (8 depths × 8 chars)
const SPLASH_OFFSET: u8 = 33;

#[wasm_bindgen]
pub struct RainWorld {
    w: u32,
    h: u32,
    // Precomputed scale factors for screen->bg mapping (avoid division in hot path)
    scale_x: f32,
    scale_y: f32,

    // Drops (SoA for cache efficiency)
    dx: [f32; MAX_DROPS],
    dy: [f32; MAX_DROPS],
    dz: [f32; MAX_DROPS],
    dv: [f32; MAX_DROPS],
    dn: usize,

    // Splashes (SoA)
    sx: [f32; MAX_SPLASHES],
    sy: [f32; MAX_SPLASHES],
    sz: [f32; MAX_SPLASHES],
    sf: [u8; MAX_SPLASHES],   // frame
    sd: [i8; MAX_SPLASHES],   // direction offset
    st: [u8; MAX_SPLASHES],   // type (0-3)
    sn: usize,

    out: Vec<u8>,
    rng: u32,
}

#[wasm_bindgen]
impl RainWorld {
    #[wasm_bindgen(constructor)]
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            w, h,
            scale_x: BG_WIDTH as f32 / w as f32,
            scale_y: BG_HEIGHT as f32 / h as f32,
            dx: [0.0; MAX_DROPS],
            dy: [0.0; MAX_DROPS],
            dz: [0.0; MAX_DROPS],
            dv: [0.0; MAX_DROPS],
            dn: 0,
            sx: [0.0; MAX_SPLASHES],
            sy: [0.0; MAX_SPLASHES],
            sz: [0.0; MAX_SPLASHES],
            sf: [0; MAX_SPLASHES],
            sd: [0; MAX_SPLASHES],
            st: [0; MAX_SPLASHES],
            sn: 0,
            out: vec![0; (w * h) as usize],
            rng: 0xDEADBEEF,
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.w = w;
        self.h = h;
        self.scale_x = BG_WIDTH as f32 / w as f32;
        self.scale_y = BG_HEIGHT as f32 / h as f32;
        self.out.resize((w * h) as usize, 0);
        self.dn = 0;
        self.sn = 0;
    }

    pub fn tick(&mut self) {
        self.out.fill(0);
        self.spawn();
        self.update_drops();
        self.update_splashes();
        self.render();
    }

    #[inline(always)]
    fn rand(&mut self) -> f32 {
        // xorshift32
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 * (1.0 / 16777216.0) // avoid u32::MAX division
    }

    #[inline(always)]
    fn hits_surface(&self, x: f32, y: f32, drop_z: f32) -> bool {
        // Convert screen coords to background coords (multiplication, not division)
        let bx = ((x * self.scale_x) as usize).min(BG_WIDTH - 1);
        let by = ((y * self.scale_y) as usize).min(BG_HEIGHT - 1);
        let bg_depth = BG_DEPTH[by][bx];

        // Skip sky (depth near 0)
        if bg_depth <= 30 { return false; }

        // Check depth match: drop_z 0=near, 1=far; bg_depth 0=far, 255=near
        let drop_depth = ((1.0 - drop_z) * 255.0) as u8;
        let diff = (drop_depth as i16 - bg_depth as i16).unsigned_abs() as u8;
        diff < DEPTH_MARGIN
    }

    fn spawn(&mut self) {
        let count = ((self.w >> 6) + 1) as usize; // ~2% of width
        let wf = self.w as f32;

        for _ in 0..count {
            if self.dn >= MAX_DROPS { return; }
            let z = self.rand();
            let n = self.dn;
            self.dx[n] = self.rand() * wf;
            self.dy[n] = -self.rand() * 15.0;
            self.dz[n] = z;
            // vel = lerp(VEL_NEAR, VEL_FAR, z) * random(0.8..1.2)
            self.dv[n] = (VEL_NEAR + (VEL_FAR - VEL_NEAR) * z) * (0.8 + self.rand() * 0.4);
            self.dn += 1;
        }
    }

    fn update_drops(&mut self) {
        let hf = self.h as f32;
        let wf = self.w as f32;
        let mut w = 0;

        for r in 0..self.dn {
            let x = self.dx[r];
            let y = self.dy[r] + self.dv[r];
            let z = self.dz[r];

            // Ground line (perspective: near=bottom, far=40% up)
            let ground = hf * (GROUND_NEAR + (GROUND_FAR - GROUND_NEAR) * z);

            // Check surface collision (only if on screen)
            if y >= 0.0 && y < hf && x >= 0.0 && x < wf && self.hits_surface(x, y, z) {
                self.spawn_splash(x, y, z, 3); // spray type
                continue;
            }

            // Check ground collision
            if y > ground {
                let r1 = self.rand();
                let r2 = self.rand();
                if r1 < SPLASH_CHANCE {
                    self.spawn_splash(x, ground, z, (r2 * 4.0) as u8);
                }
                continue;
            }

            // Keep falling
            self.dx[w] = x;
            self.dy[w] = y;
            self.dz[w] = z;
            self.dv[w] = self.dv[r];
            w += 1;
        }
        self.dn = w;
    }

    #[inline]
    fn spawn_splash(&mut self, x: f32, y: f32, z: f32, typ: u8) {
        if self.sn >= MAX_SPLASHES { return; }
        let n = self.sn;
        self.sx[n] = x + (self.rand() - 0.5) * 4.0;
        self.sy[n] = y;
        self.sz[n] = z;
        self.sf[n] = 0;
        self.sd[n] = (self.rand() * 5.0) as i8 - 2;
        self.st[n] = typ;
        self.sn += 1;
    }

    fn update_splashes(&mut self) {
        let mut w = 0;
        for r in 0..self.sn {
            let f = self.sf[r] + 1;
            if f >= 24 { continue; }
            self.sx[w] = self.sx[r];
            self.sy[w] = self.sy[r];
            self.sz[w] = self.sz[r];
            self.sf[w] = f;
            self.sd[w] = self.sd[r];
            self.st[w] = self.st[r];
            w += 1;
        }
        self.sn = w;
    }

    fn render(&mut self) {
        let (w, h) = (self.w as i32, self.h as i32);

        // Render drops - no sorting needed, encoding handles depth priority
        for i in 0..self.dn {
            let x = self.dx[i] as i32;
            let y = self.dy[i] as i32;
            if x < 0 || x >= w { continue; }

            let z = self.dz[i];
            let bucket = (((1.0 - z) * 8.0) as u8).min(7);
            let trail = (5.0 - z * 4.0).max(1.0) as i32;

            for dy in 0..trail {
                let py = y - dy;
                if py >= 0 && py < h {
                    let idx = (py * w + x) as usize;
                    let enc = bucket * 4 + (dy.min(3) as u8) + 1;
                    if enc > self.out[idx] { self.out[idx] = enc; }
                }
            }
        }

        // Render splashes
        for i in 0..self.sn {
            let x = self.sx[i] as i32;
            let y = self.sy[i] as i32;
            let z = self.sz[i];
            let bucket = (((1.0 - z) * 8.0) as u8).min(7);
            let scale = ((1.0 - z) * 2.5) as i32;
            let frame = self.sf[i] / 3;
            let dir = self.sd[i] as i32;

            self.render_splash_frame(x, y, bucket, scale, frame, dir, self.st[i], w, h);
        }
    }

    #[inline]
    fn put(&mut self, x: i32, y: i32, char_idx: u8, bucket: u8, w: i32, h: i32) {
        if (x as u32) < (w as u32) && (y as u32) < (h as u32) {
            let idx = (y * w + x) as usize;
            let enc = SPLASH_OFFSET + bucket * 8 + char_idx;
            if enc > self.out[idx] { self.out[idx] = enc; }
        }
    }

    fn render_splash_frame(&mut self, cx: i32, gy: i32, b: u8, s: i32, f: u8, d: i32, typ: u8, w: i32, h: i32) {
        // Tiny splashes (s==0) are simple
        if s == 0 {
            let c = if f < 3 { 0 } else if f < 6 { 2 } else { 6 };
            self.put(cx, gy, c, b, w, h);
            return;
        }

        // Splash patterns encoded as (dx, dy, char) tuples per frame
        // Pattern selection by type: 0=crown, 1=left, 2=right, 3=spray
        match typ {
            0 => match f { // Crown - symmetric
                0 => self.put(cx, gy, 0, b, w, h),
                1 => {
                    self.put(cx - s + d, gy, 4, b, w, h);
                    self.put(cx, gy, 0, b, w, h);
                    self.put(cx + s + d, gy, 5, b, w, h);
                }
                2 => {
                    self.put(cx + d, gy - s, 1, b, w, h);
                    self.put(cx - s + d, gy, 4, b, w, h);
                    self.put(cx + s + d, gy, 5, b, w, h);
                }
                3 => {
                    self.put(cx - s*2 + d, gy - s, 4, b, w, h);
                    self.put(cx + d, gy - s, 1, b, w, h);
                    self.put(cx + s*2 + d, gy - s, 5, b, w, h);
                    self.put(cx - s, gy, 4, b, w, h);
                    self.put(cx + s, gy, 5, b, w, h);
                }
                4 => {
                    self.put(cx - s*2 + d*2, gy - s*2, 2, b, w, h);
                    self.put(cx + d, gy - s*2, 2, b, w, h);
                    self.put(cx + s*2 + d*2, gy - s*2, 2, b, w, h);
                    self.put(cx - s*2 + d, gy - s, 4, b, w, h);
                    self.put(cx + s*2 + d, gy - s, 5, b, w, h);
                }
                5 => {
                    self.put(cx - s*3 + d*2, gy - s*2, 2, b, w, h);
                    self.put(cx + d, gy - s*2, 2, b, w, h);
                    self.put(cx + s*3 + d*2, gy - s*2, 2, b, w, h);
                }
                6 => {
                    self.put(cx - s*2 + d, gy - s, 2, b, w, h);
                    self.put(cx + s*2 + d, gy - s, 2, b, w, h);
                }
                _ => {
                    self.put(cx - s + d, gy, 6, b, w, h);
                    self.put(cx + s + d, gy, 6, b, w, h);
                }
            }
            1 => match f { // Left burst
                0 => self.put(cx, gy, 0, b, w, h),
                1 => {
                    self.put(cx - s + d, gy, 4, b, w, h);
                    self.put(cx, gy, 0, b, w, h);
                }
                2 => {
                    self.put(cx - s + d, gy - s, 4, b, w, h);
                    self.put(cx + d, gy - s, 1, b, w, h);
                    self.put(cx - s*2 + d, gy, 4, b, w, h);
                }
                3 => {
                    self.put(cx - s*2 + d, gy - s*2, 2, b, w, h);
                    self.put(cx - s + d, gy - s, 4, b, w, h);
                    self.put(cx + d, gy - s, 1, b, w, h);
                    self.put(cx - s*3 + d, gy, 4, b, w, h);
                }
                4 => {
                    self.put(cx - s*3 + d, gy - s*2, 2, b, w, h);
                    self.put(cx - s + d, gy - s*2, 2, b, w, h);
                    self.put(cx - s*2 + d, gy - s, 4, b, w, h);
                    self.put(cx + d, gy - s, 1, b, w, h);
                }
                5 => {
                    self.put(cx - s*4 + d, gy - s*2, 2, b, w, h);
                    self.put(cx - s*2 + d, gy - s*2, 2, b, w, h);
                    self.put(cx - s*3 + d, gy - s, 4, b, w, h);
                }
                6 => {
                    self.put(cx - s*3 + d, gy - s, 2, b, w, h);
                    self.put(cx - s + d, gy - s, 2, b, w, h);
                }
                _ => self.put(cx - s*2 + d, gy, 6, b, w, h),
            }
            2 => match f { // Right burst
                0 => self.put(cx, gy, 0, b, w, h),
                1 => {
                    self.put(cx, gy, 0, b, w, h);
                    self.put(cx + s + d, gy, 5, b, w, h);
                }
                2 => {
                    self.put(cx + d, gy - s, 1, b, w, h);
                    self.put(cx + s + d, gy - s, 5, b, w, h);
                    self.put(cx + s*2 + d, gy, 5, b, w, h);
                }
                3 => {
                    self.put(cx + d, gy - s, 1, b, w, h);
                    self.put(cx + s + d, gy - s, 5, b, w, h);
                    self.put(cx + s*2 + d, gy - s*2, 2, b, w, h);
                    self.put(cx + s*3 + d, gy, 5, b, w, h);
                }
                4 => {
                    self.put(cx + d, gy - s, 1, b, w, h);
                    self.put(cx + s*2 + d, gy - s, 5, b, w, h);
                    self.put(cx + s + d, gy - s*2, 2, b, w, h);
                    self.put(cx + s*3 + d, gy - s*2, 2, b, w, h);
                }
                5 => {
                    self.put(cx + s*3 + d, gy - s, 5, b, w, h);
                    self.put(cx + s*2 + d, gy - s*2, 2, b, w, h);
                    self.put(cx + s*4 + d, gy - s*2, 2, b, w, h);
                }
                6 => {
                    self.put(cx + s + d, gy - s, 2, b, w, h);
                    self.put(cx + s*3 + d, gy - s, 2, b, w, h);
                }
                _ => self.put(cx + s*2 + d, gy, 6, b, w, h),
            }
            _ => match f { // Spray
                0 => self.put(cx, gy, 0, b, w, h),
                1 => {
                    self.put(cx + d, gy, 0, b, w, h);
                    self.put(cx - s + d, gy, 2, b, w, h);
                    self.put(cx + s + d, gy, 2, b, w, h);
                }
                2 => {
                    self.put(cx + d*2, gy - s, 2, b, w, h);
                    self.put(cx - s + d, gy - s, 2, b, w, h);
                    self.put(cx + s*2 + d, gy, 2, b, w, h);
                }
                3 => {
                    self.put(cx + d*2, gy - s*2, 2, b, w, h);
                    self.put(cx - s*2 + d, gy - s, 2, b, w, h);
                    self.put(cx + s + d, gy - s, 2, b, w, h);
                    self.put(cx + s*3 + d, gy, 2, b, w, h);
                }
                4 => {
                    self.put(cx - s + d, gy - s*2, 2, b, w, h);
                    self.put(cx + s*2 + d, gy - s*2, 2, b, w, h);
                    self.put(cx - s*2 + d, gy - s, 2, b, w, h);
                    self.put(cx + s*3 + d, gy - s, 2, b, w, h);
                }
                5 => {
                    self.put(cx - s*2 + d, gy - s*2, 2, b, w, h);
                    self.put(cx + s + d, gy - s*2, 2, b, w, h);
                    self.put(cx + s*3 + d, gy - s, 2, b, w, h);
                }
                6 => {
                    self.put(cx - s + d, gy - s, 2, b, w, h);
                    self.put(cx + s*2 + d, gy - s, 2, b, w, h);
                }
                _ => self.put(cx + d, gy, 6, b, w, h),
            }
        }
    }

    pub fn output_ptr(&self) -> *const u8 { self.out.as_ptr() }
    pub fn output_len(&self) -> usize { self.out.len() }
    pub fn width(&self) -> u32 { self.w }
    pub fn height(&self) -> u32 { self.h }
}
