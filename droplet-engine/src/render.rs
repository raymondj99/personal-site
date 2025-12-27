// render.rs - Encode simulation state to output buffer
//
// Output encoding (for canvas rendering):
//   0        = empty
//   1-32     = drops (8 depths x 4 trail lengths)
//   33-96    = splashes (8 depths x 8 chars)
//   97-128   = streams (8 depths x 4 sizes)

use crate::sim::{Droplets, Splashes, Streams};

const SPLASH_OFFSET: u8 = 33;
const STREAM_OFFSET: u8 = 97;

pub struct Encoder {
    out: Vec<u8>,
    w: u32,
    h: u32,
}

impl Encoder {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            out: vec![0; (w * h) as usize],
            w,
            h,
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.w = w;
        self.h = h;
        self.out.resize((w * h) as usize, 0);
    }

    pub fn clear(&mut self) {
        self.out.fill(0);
    }

    pub fn ptr(&self) -> *const u8 {
        self.out.as_ptr()
    }

    pub fn len(&self) -> usize {
        self.out.len()
    }

    /// Encode drops to output buffer
    pub fn encode_drops(&mut self, drops: &Droplets, w: i32, h: i32) {
        for i in 0..drops.n {
            let x = drops.x[i] as i32;
            let y = drops.y[i] as i32;
            if x < 0 || x >= w { continue; }

            let z = drops.z[i];
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
    }

    /// Encode splashes to output buffer
    pub fn encode_splashes(&mut self, splashes: &Splashes, w: i32, h: i32, rng: &mut u32) {
        for i in 0..splashes.n {
            let x = splashes.x[i] as i32;
            let y = splashes.y[i] as i32;
            let z = splashes.z[i];

            let bucket = (((1.0 - z) * 8.0) as u8).min(7);
            let scale = ((1.0 - z) * 2.5) as i32;
            let frame = splashes.frame[i] / 3;
            let dir = splashes.dir[i] as i32;
            let typ = splashes.typ[i];

            self.render_splash(x, y, bucket, scale, frame, dir, typ, w, h, rng);
        }
    }

    /// Encode streams to output buffer
    pub fn encode_streams(&mut self, streams: &Streams, w: i32, h: i32) {
        for i in 0..streams.n {
            let x = streams.x[i] as i32;
            let y = streams.y[i] as i32;
            let z = streams.z[i];

            if x < 0 || x >= w || y < 0 || y >= h { continue; }

            let bucket = (((1.0 - z) * 8.0) as u8).min(7);
            let life = streams.life[i];

            // Size varies with lifetime
            let size = if life > 80 { 3 } else if life > 40 { 2 } else if life > 10 { 1 } else { 0 };

            let idx = (y * w + x) as usize;
            let enc = STREAM_OFFSET + bucket * 4 + size;
            if enc > self.out[idx] { self.out[idx] = enc; }
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

    fn render_splash(&mut self, cx: i32, gy: i32, b: u8, s: i32, f: u8, d: i32, typ: u8, w: i32, h: i32, _rng: &mut u32) {
        // Tiny splashes
        if s == 0 {
            let c = if f < 3 { 0 } else if f < 6 { 2 } else { 6 };
            self.put(cx, gy, c, b, w, h);
            return;
        }

        // Pattern by type and frame
        match typ {
            0 => self.splash_crown(cx, gy, b, s, f, d, w, h),
            1 => self.splash_left(cx, gy, b, s, f, d, w, h),
            2 => self.splash_right(cx, gy, b, s, f, d, w, h),
            _ => self.splash_spray(cx, gy, b, s, f, d, w, h),
        }
    }

    fn splash_crown(&mut self, cx: i32, gy: i32, b: u8, s: i32, f: u8, d: i32, w: i32, h: i32) {
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

    fn splash_left(&mut self, cx: i32, gy: i32, b: u8, s: i32, f: u8, d: i32, w: i32, h: i32) {
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
            _ => self.put(cx - s*2 + d, gy, 6, b, w, h),
        }
    }

    fn splash_right(&mut self, cx: i32, gy: i32, b: u8, s: i32, f: u8, d: i32, w: i32, h: i32) {
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
            _ => self.put(cx + s*2 + d, gy, 6, b, w, h),
        }
    }

    fn splash_spray(&mut self, cx: i32, gy: i32, b: u8, s: i32, f: u8, d: i32, w: i32, h: i32) {
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
            _ => self.put(cx + d, gy, 6, b, w, h),
        }
    }
}
