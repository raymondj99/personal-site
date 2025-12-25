use wasm_bindgen::prelude::*;

const MAX_DROPLETS: usize = 3000;
const MAX_SPLASHES: usize = 200;

// Depth range (0.0 = near, 1.0 = far)
const GROUND_Y_NEAR: f32 = 0.95;  // Near ground at 95% from top
const GROUND_Y_FAR: f32 = 0.40;   // Far ground at 40% from top (horizon)

// Velocity range
const VEL_NEAR: f32 = 1.2;
const VEL_FAR: f32 = 0.25;

// Encoding: use 8 depth buckets for color gradation
const DEPTH_BUCKETS: u8 = 8;
const TRAIL_POSITIONS: u8 = 4;
// Droplets: 1-32 = bucket * 4 + trail_pos + 1
// Splashes: 33+ = 33 + bucket * 8 + char_type
const SPLASH_OFFSET: u8 = 33;
const SPLASH_CHAR_TYPES: u8 = 8;

#[wasm_bindgen]
pub struct RainWorld {
    width: u32,
    height: u32,

    // Droplets with continuous z-depth (SoA)
    drop_x: [f32; MAX_DROPLETS],
    drop_y: [f32; MAX_DROPLETS],
    drop_z: [f32; MAX_DROPLETS],  // 0.0 = near, 1.0 = far
    drop_vel: [f32; MAX_DROPLETS],
    drop_count: usize,

    // Splashes (SoA)
    splash_x: [f32; MAX_SPLASHES],
    splash_z: [f32; MAX_SPLASHES],
    splash_frame: [u8; MAX_SPLASHES],
    splash_dir: [i8; MAX_SPLASHES],
    splash_count: usize,

    // Output buffer
    output: Vec<u8>,

    seed: u32,
    frame: u32,
}

#[wasm_bindgen]
impl RainWorld {
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            drop_x: [0.0; MAX_DROPLETS],
            drop_y: [0.0; MAX_DROPLETS],
            drop_z: [0.0; MAX_DROPLETS],
            drop_vel: [0.0; MAX_DROPLETS],
            drop_count: 0,
            splash_x: [0.0; MAX_SPLASHES],
            splash_z: [0.0; MAX_SPLASHES],
            splash_frame: [0; MAX_SPLASHES],
            splash_dir: [0; MAX_SPLASHES],
            splash_count: 0,
            output: vec![0u8; (width * height) as usize],
            seed: 0xDEADBEEF,
            frame: 0,
        }
    }

    #[inline]
    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed as f32) / (u32::MAX as f32)
    }

    // Convert continuous z (0-1) to depth bucket (0-7)
    #[inline]
    fn z_to_bucket(&self, z: f32) -> u8 {
        let bucket = (z * DEPTH_BUCKETS as f32) as u8;
        bucket.min(DEPTH_BUCKETS - 1)
    }

    // Get ground Y position for a given z-depth
    #[inline]
    fn ground_y_for_z(&self, z: f32) -> f32 {
        let h = self.height as f32;
        // Lerp between near ground and far ground based on z
        h * (GROUND_Y_NEAR + (GROUND_Y_FAR - GROUND_Y_NEAR) * z)
    }

    // Get velocity for a given z-depth
    #[inline]
    fn vel_for_z(&self, z: f32) -> f32 {
        VEL_NEAR + (VEL_FAR - VEL_NEAR) * z
    }

    // Get trail length for a given z-depth
    #[inline]
    fn trail_len_for_z(&self, z: f32) -> u8 {
        // Near (z=0): 4-5 chars, Far (z=1): 1-2 chars
        let base = 5.0 - z * 4.0;
        base.max(1.0) as u8
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.output.fill(0);

        self.spawn_drops();
        self.update_drops();
        self.update_splashes();
        self.render();
    }

    fn spawn_drops(&mut self) {
        // Spawn rate
        let target = (self.width as f32 * 0.02) as usize;

        for _ in 0..target {
            if self.drop_count >= MAX_DROPLETS {
                break;
            }

            let i = self.drop_count;

            // Random z-depth (continuous)
            let z = self.rand();

            self.drop_x[i] = self.rand() * self.width as f32;
            self.drop_y[i] = -(self.rand() * 15.0);
            self.drop_z[i] = z;
            self.drop_vel[i] = self.vel_for_z(z) * (0.8 + self.rand() * 0.4);

            self.drop_count += 1;
        }
    }

    fn update_drops(&mut self) {
        let mut write = 0;

        for read in 0..self.drop_count {
            let x = self.drop_x[read];
            let z = self.drop_z[read];
            let y = self.drop_y[read] + self.drop_vel[read];

            let ground_y = self.ground_y_for_z(z);

            // Hit ground - spawn splash
            if y > ground_y {
                if self.splash_count < MAX_SPLASHES {
                    let s = self.splash_count;
                    self.splash_x[s] = x;
                    self.splash_z[s] = z;
                    self.splash_frame[s] = 0;
                    self.splash_dir[s] = ((self.rand() * 3.0) as i8) - 1;
                    self.splash_count += 1;
                }
                continue;
            }

            // Keep droplet
            self.drop_x[write] = x;
            self.drop_y[write] = y;
            self.drop_z[write] = z;
            self.drop_vel[write] = self.drop_vel[read];
            write += 1;
        }

        self.drop_count = write;
    }

    fn update_splashes(&mut self) {
        let mut write = 0;

        for read in 0..self.splash_count {
            let new_frame = if self.frame % 3 == 0 {
                self.splash_frame[read] + 1
            } else {
                self.splash_frame[read]
            };

            if new_frame >= 8 {
                continue;
            }

            self.splash_x[write] = self.splash_x[read];
            self.splash_z[write] = self.splash_z[read];
            self.splash_frame[write] = new_frame;
            self.splash_dir[write] = self.splash_dir[read];
            write += 1;
        }

        self.splash_count = write;
    }

    fn render(&mut self) {
        // Sort droplets by z (far first) for correct overdraw
        let mut indices: Vec<usize> = (0..self.drop_count).collect();
        indices.sort_unstable_by(|&a, &b| {
            self.drop_z[b].partial_cmp(&self.drop_z[a]).unwrap()
        });

        for &i in &indices {
            self.render_drop(i);
        }

        // Sort splashes by z (far first)
        let mut splash_indices: Vec<usize> = (0..self.splash_count).collect();
        splash_indices.sort_unstable_by(|&a, &b| {
            self.splash_z[b].partial_cmp(&self.splash_z[a]).unwrap()
        });

        for &i in &splash_indices {
            self.render_splash(i);
        }
    }

    fn render_drop(&mut self, i: usize) {
        let x = self.drop_x[i] as i32;
        let base_y = self.drop_y[i] as i32;
        let z = self.drop_z[i];

        let w = self.width as i32;
        let h = self.height as i32;

        if x < 0 || x >= w {
            return;
        }

        let bucket = self.z_to_bucket(z);
        let trail_len = self.trail_len_for_z(z) as i32;

        for dy in 0..trail_len {
            let y = base_y - dy;
            if y < 0 || y >= h {
                continue;
            }

            let idx = (y * w + x) as usize;
            let trail_pos = dy.min(3) as u8;
            // Encode: bucket (0-7) determines color, invert so near=bright
            let encoded = (DEPTH_BUCKETS - 1 - bucket) * TRAIL_POSITIONS + trail_pos + 1;

            if encoded > self.output[idx] {
                self.output[idx] = encoded;
            }
        }
    }

    fn render_splash(&mut self, i: usize) {
        let x = self.splash_x[i] as i32;
        let z = self.splash_z[i];
        let frame = self.splash_frame[i];
        let dir = self.splash_dir[i] as i32;

        let w = self.width as i32;
        let h = self.height as i32;

        let gy = self.ground_y_for_z(z) as i32;
        let bucket = self.z_to_bucket(z);

        // Scale based on z (near = large, far = tiny)
        // z=0 -> scale=2, z=1 -> scale=0
        let scale = ((1.0 - z) * 2.5) as i32;

        self.render_splash_frame(x, gy, frame, bucket, scale, dir, w, h);
    }

    fn render_splash_frame(&mut self, cx: i32, gy: i32, frame: u8, bucket: u8, scale: i32, dir: i32, w: i32, h: i32) {
        // For very far (scale=0), minimal splash
        if scale == 0 {
            match frame {
                0..=2 => self.put_splash(cx, gy, 0, bucket, w, h),
                3..=5 => {
                    self.put_splash(cx - 1, gy, 2, bucket, w, h);
                    self.put_splash(cx + 1, gy, 2, bucket, w, h);
                }
                _ => self.put_splash(cx, gy, 6, bucket, w, h),
            }
            return;
        }

        let s = |offset: i32| -> i32 { offset * scale };

        match frame {
            0 => {
                self.put_splash(cx, gy, 0, bucket, w, h);
            }
            1 => {
                self.put_splash(cx - s(1) + dir, gy, 4, bucket, w, h);
                self.put_splash(cx, gy, 0, bucket, w, h);
                self.put_splash(cx + s(1) + dir, gy, 5, bucket, w, h);
            }
            2 => {
                self.put_splash(cx + dir, gy - s(1), 1, bucket, w, h);
                self.put_splash(cx - s(1) + dir, gy, 4, bucket, w, h);
                self.put_splash(cx, gy, 0, bucket, w, h);
                self.put_splash(cx + s(1) + dir, gy, 5, bucket, w, h);
            }
            3 => {
                self.put_splash(cx - s(2) + dir, gy - s(1), 4, bucket, w, h);
                self.put_splash(cx + dir, gy - s(1), 1, bucket, w, h);
                self.put_splash(cx + s(2) + dir, gy - s(1), 5, bucket, w, h);
                self.put_splash(cx - s(1), gy, 4, bucket, w, h);
                self.put_splash(cx + s(1), gy, 5, bucket, w, h);
            }
            4 => {
                self.put_splash(cx - s(2) + dir * 2, gy - s(2), 2, bucket, w, h);
                self.put_splash(cx + dir, gy - s(2), 2, bucket, w, h);
                self.put_splash(cx + s(2) + dir * 2, gy - s(2), 2, bucket, w, h);
                self.put_splash(cx - s(2) + dir, gy - s(1), 4, bucket, w, h);
                self.put_splash(cx + dir, gy - s(1), 1, bucket, w, h);
                self.put_splash(cx + s(2) + dir, gy - s(1), 5, bucket, w, h);
            }
            5 => {
                self.put_splash(cx - s(3) + dir * 2, gy - s(2), 2, bucket, w, h);
                self.put_splash(cx + dir, gy - s(2), 2, bucket, w, h);
                self.put_splash(cx + s(3) + dir * 2, gy - s(2), 2, bucket, w, h);
                self.put_splash(cx - s(2) + dir, gy - s(1), 4, bucket, w, h);
                self.put_splash(cx + s(2) + dir, gy - s(1), 5, bucket, w, h);
            }
            6 => {
                self.put_splash(cx - s(3) + dir * 2, gy - s(1), 2, bucket, w, h);
                self.put_splash(cx + dir, gy - s(1), 2, bucket, w, h);
                self.put_splash(cx + s(3) + dir * 2, gy - s(1), 2, bucket, w, h);
            }
            _ => {
                self.put_splash(cx - s(2) + dir, gy, 6, bucket, w, h);
                self.put_splash(cx + s(2) + dir, gy, 6, bucket, w, h);
            }
        }
    }

    fn put_splash(&mut self, x: i32, y: i32, char_type: u8, bucket: u8, w: i32, h: i32) {
        if x >= 0 && x < w && y >= 0 && y < h {
            let idx = (y * w + x) as usize;
            // Invert bucket so near=bright (bucket 0 = far = dim, bucket 7 = near = bright)
            let encoded = SPLASH_OFFSET + (DEPTH_BUCKETS - 1 - bucket) * SPLASH_CHAR_TYPES + char_type;
            if encoded > self.output[idx] {
                self.output[idx] = encoded;
            }
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.output = vec![0u8; (width * height) as usize];
        self.drop_count = 0;
        self.splash_count = 0;
    }

    pub fn output_ptr(&self) -> *const u8 {
        self.output.as_ptr()
    }

    pub fn output_len(&self) -> usize {
        self.output.len()
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
