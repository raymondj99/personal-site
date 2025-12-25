use wasm_bindgen::prelude::*;

// ============================================================================
// RAIN WORLD - Perspective rain simulation with depth-of-field
// ============================================================================

const MAX_DROPS: usize = 3000;
const MAX_SPLASHES: usize = 200;

const GROUND_NEAR: f32 = 1.0;
const GROUND_FAR: f32 = 0.4;

const VEL_NEAR: f32 = 1.2;
const VEL_FAR: f32 = 0.25;

const SPLASH_CHANCE: f32 = 0.7;

// Encoding
const DEPTH_BUCKETS: u8 = 8;
const TRAIL_POSITIONS: u8 = 4;
const SPLASH_OFFSET: u8 = 33;
const SPLASH_CHARS: u8 = 8;

#[wasm_bindgen]
pub struct RainWorld {
    w: u32,
    h: u32,

    // Droplets (SoA)
    dx: [f32; MAX_DROPS],
    dy: [f32; MAX_DROPS],
    dz: [f32; MAX_DROPS],
    dv: [f32; MAX_DROPS],
    dn: usize,

    // Splashes (SoA)
    sx: [f32; MAX_SPLASHES],
    sz: [f32; MAX_SPLASHES],
    sf: [u8; MAX_SPLASHES],
    sd: [i8; MAX_SPLASHES],   // direction: -2, -1, 0, 1, 2
    st: [u8; MAX_SPLASHES],   // type: 0=crown, 1=left-burst, 2=right-burst, 3=spray
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
            dx: [0.0; MAX_DROPS],
            dy: [0.0; MAX_DROPS],
            dz: [0.0; MAX_DROPS],
            dv: [0.0; MAX_DROPS],
            dn: 0,
            sx: [0.0; MAX_SPLASHES],
            sz: [0.0; MAX_SPLASHES],
            sf: [0; MAX_SPLASHES],
            sd: [0; MAX_SPLASHES],
            st: [0; MAX_SPLASHES],
            sn: 0,
            out: vec![0; (w * h) as usize],
            rng: 0xDEADBEEF,
        }
    }

    pub fn tick(&mut self) {
        self.out.fill(0);
        self.spawn();
        self.update_drops();
        self.update_splashes();
        self.render();
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32
    }

    fn spawn(&mut self) {
        let count = (self.w as f32 * 0.02) as usize;
        for _ in 0..count {
            if self.dn >= MAX_DROPS { break; }
            let z = self.rand();
            self.dx[self.dn] = self.rand() * self.w as f32;
            self.dy[self.dn] = -self.rand() * 15.0;
            self.dz[self.dn] = z;
            self.dv[self.dn] = lerp(VEL_NEAR, VEL_FAR, z) * (0.8 + self.rand() * 0.4);
            self.dn += 1;
        }
    }

    fn update_drops(&mut self) {
        let mut w = 0;
        for r in 0..self.dn {
            let y = self.dy[r] + self.dv[r];
            let z = self.dz[r];
            let ground = self.h as f32 * lerp(GROUND_NEAR, GROUND_FAR, z);

            if y > ground {
                // Random chance to splash
                if self.rand() < SPLASH_CHANCE && self.sn < MAX_SPLASHES {
                    self.sx[self.sn] = self.dx[r];
                    self.sz[self.sn] = z;
                    self.sf[self.sn] = 0;
                    // Direction: -2 to +2 for more asymmetry
                    self.sd[self.sn] = (self.rand() * 5.0) as i8 - 2;
                    // Type: 0=crown, 1=left-burst, 2=right-burst, 3=spray
                    self.st[self.sn] = (self.rand() * 4.0) as u8;
                    self.sn += 1;
                }
                continue;
            }

            self.dx[w] = self.dx[r];
            self.dy[w] = y;
            self.dz[w] = z;
            self.dv[w] = self.dv[r];
            w += 1;
        }
        self.dn = w;
    }

    fn update_splashes(&mut self) {
        let mut w = 0;
        for r in 0..self.sn {
            let f = self.sf[r] + 1;
            if f >= 24 { continue; }
            self.sx[w] = self.sx[r];
            self.sz[w] = self.sz[r];
            self.sf[w] = f;
            self.sd[w] = self.sd[r];
            self.st[w] = self.st[r];
            w += 1;
        }
        self.sn = w;
    }

    fn render(&mut self) {
        let mut idx: Vec<usize> = (0..self.dn).collect();
        idx.sort_unstable_by(|&a, &b| self.dz[b].partial_cmp(&self.dz[a]).unwrap());
        for &i in &idx { self.render_drop(i); }

        let mut sidx: Vec<usize> = (0..self.sn).collect();
        sidx.sort_unstable_by(|&a, &b| self.sz[b].partial_cmp(&self.sz[a]).unwrap());
        for &i in &sidx { self.render_splash(i); }
    }

    fn render_drop(&mut self, i: usize) {
        let x = self.dx[i] as i32;
        let y = self.dy[i] as i32;
        let z = self.dz[i];
        let (w, h) = (self.w as i32, self.h as i32);

        if x < 0 || x >= w { return; }

        let bucket = ((1.0 - z) * DEPTH_BUCKETS as f32) as u8;
        let bucket = bucket.min(DEPTH_BUCKETS - 1);
        let trail = (5.0 - z * 4.0).max(1.0) as i32;

        for dy in 0..trail {
            let py = y - dy;
            if py < 0 || py >= h { continue; }
            let pos = dy.min(3) as u8;
            let enc = bucket * TRAIL_POSITIONS + pos + 1;
            let idx = (py * w + x) as usize;
            if enc > self.out[idx] { self.out[idx] = enc; }
        }
    }

    fn render_splash(&mut self, i: usize) {
        let x = self.sx[i] as i32;
        let z = self.sz[i];
        let f = self.sf[i] / 3;
        let dir = self.sd[i] as i32;
        let typ = self.st[i];
        let (w, h) = (self.w as i32, self.h as i32);

        let gy = (self.h as f32 * lerp(GROUND_NEAR, GROUND_FAR, z)) as i32;
        let bucket = ((1.0 - z) * DEPTH_BUCKETS as f32) as u8;
        let bucket = bucket.min(DEPTH_BUCKETS - 1);
        let scale = ((1.0 - z) * 2.5) as i32;

        match typ {
            0 => self.splash_crown(x, gy, f, bucket, scale, dir, w, h),
            1 => self.splash_left(x, gy, f, bucket, scale, dir, w, h),
            2 => self.splash_right(x, gy, f, bucket, scale, dir, w, h),
            _ => self.splash_spray(x, gy, f, bucket, scale, dir, w, h),
        }
    }

    fn put(&mut self, x: i32, y: i32, c: u8, b: u8, w: i32, h: i32) {
        if x >= 0 && x < w && y >= 0 && y < h {
            let idx = (y * w + x) as usize;
            let enc = SPLASH_OFFSET + b * SPLASH_CHARS + c;
            if enc > self.out[idx] { self.out[idx] = enc; }
        }
    }

    // Crown splash - classic symmetric with direction offset
    fn splash_crown(&mut self, cx: i32, gy: i32, f: u8, b: u8, s: i32, d: i32, w: i32, h: i32) {
        if s == 0 {
            match f {
                0..=2 => self.put(cx, gy, 0, b, w, h),
                3..=5 => { self.put(cx-1, gy, 2, b, w, h); self.put(cx+1, gy, 2, b, w, h); }
                _ => self.put(cx, gy, 6, b, w, h),
            }
            return;
        }

        match f {
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
    }

    // Left burst - asymmetric splash biased left
    fn splash_left(&mut self, cx: i32, gy: i32, f: u8, b: u8, s: i32, d: i32, w: i32, h: i32) {
        if s == 0 {
            match f {
                0..=3 => self.put(cx, gy, 0, b, w, h),
                4..=5 => self.put(cx - 1, gy, 2, b, w, h),
                _ => self.put(cx - 1, gy, 6, b, w, h),
            }
            return;
        }

        match f {
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
            _ => {
                self.put(cx - s*2 + d, gy, 6, b, w, h);
            }
        }
    }

    // Right burst - asymmetric splash biased right
    fn splash_right(&mut self, cx: i32, gy: i32, f: u8, b: u8, s: i32, d: i32, w: i32, h: i32) {
        if s == 0 {
            match f {
                0..=3 => self.put(cx, gy, 0, b, w, h),
                4..=5 => self.put(cx + 1, gy, 2, b, w, h),
                _ => self.put(cx + 1, gy, 6, b, w, h),
            }
            return;
        }

        match f {
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
            _ => {
                self.put(cx + s*2 + d, gy, 6, b, w, h);
            }
        }
    }

    // Spray - scattered droplets
    fn splash_spray(&mut self, cx: i32, gy: i32, f: u8, b: u8, s: i32, d: i32, w: i32, h: i32) {
        if s == 0 {
            if f < 6 { self.put(cx, gy, 0, b, w, h); }
            return;
        }

        match f {
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
            _ => {
                self.put(cx + d, gy, 6, b, w, h);
            }
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.w = w;
        self.h = h;
        self.out = vec![0; (w * h) as usize];
        self.dn = 0;
        self.sn = 0;
    }

    pub fn output_ptr(&self) -> *const u8 { self.out.as_ptr() }
    pub fn output_len(&self) -> usize { self.out.len() }
    pub fn width(&self) -> u32 { self.w }
    pub fn height(&self) -> u32 { self.h }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }
