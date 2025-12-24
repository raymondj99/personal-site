use wasm_bindgen::prelude::*;

const MAX_DROPLETS_PER_LAYER: usize = 2000;
const NUM_LAYERS: usize = 3;

#[wasm_bindgen]
pub struct RainWorld {
    width: u32,
    height: u32,

    // SoA layout per layer - [far, mid, near]
    x: [[f32; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
    y: [[f32; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
    vel: [[f32; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
    len: [[u8; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
    count: [usize; NUM_LAYERS],

    // Output buffer - single allocation, reused
    output: Vec<u8>,

    // Wind: horizontal drift
    wind: f32,

    // Spawn rates per layer [far, mid, near]
    spawn_rates: [f32; NUM_LAYERS],

    // Base velocities per layer (depth multiplier)
    base_vel: [f32; NUM_LAYERS],

    // PRNG state
    seed: u32,

    frame: u32,
}

#[wasm_bindgen]
impl RainWorld {
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> Self {
        let base_vel = [0.4, 0.7, 1.2];
        let spawn_rates = [0.5, 0.35, 0.25];

        Self {
            width,
            height,
            x: [[0.0; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
            y: [[0.0; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
            vel: [[0.0; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
            len: [[0; MAX_DROPLETS_PER_LAYER]; NUM_LAYERS],
            count: [0; NUM_LAYERS],
            output: vec![0u8; (width * height) as usize],
            wind: 0.0,
            spawn_rates,
            base_vel,
            seed: 0xDEADBEEF,
            frame: 0,
        }
    }

    #[inline]
    fn fastrand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed as f32) / (u32::MAX as f32)
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);

        // Gentle wind variation
        self.wind = ((self.frame as f32) * 0.008).sin() * 0.2;

        // Clear output buffer (0 = empty, 1-3 = layer index + char)
        self.output.fill(0);

        // Update each layer: far first, near last (near overwrites)
        for layer in 0..NUM_LAYERS {
            self.spawn_layer(layer);
            self.update_layer(layer);
            self.render_layer(layer);
        }
    }

    fn spawn_layer(&mut self, layer: usize) {
        let count = self.count[layer];
        if count >= MAX_DROPLETS_PER_LAYER {
            return;
        }

        // Spawn based on width and layer rate
        let spawn_target = (self.width as f32 * self.spawn_rates[layer] * 0.08) as usize;
        let spawn_count = spawn_target.min(MAX_DROPLETS_PER_LAYER - count);

        for _ in 0..spawn_count {
            let i = self.count[layer];
            if i >= MAX_DROPLETS_PER_LAYER {
                break;
            }

            self.x[layer][i] = self.fastrand() * self.width as f32;
            self.y[layer][i] = -(self.fastrand() * 15.0);

            // Velocity varies by layer + random variation
            let base = self.base_vel[layer];
            self.vel[layer][i] = base * (0.7 + self.fastrand() * 0.6);

            // Length: far drops shorter, near drops longer
            self.len[layer][i] = match layer {
                0 => 1 + (self.fastrand() * 2.0) as u8,
                1 => 2 + (self.fastrand() * 2.0) as u8,
                _ => 2 + (self.fastrand() * 4.0) as u8,
            };

            self.count[layer] += 1;
        }
    }

    fn update_layer(&mut self, layer: usize) {
        let mut write_idx = 0;
        let count = self.count[layer];
        let height = self.height as f32;
        let width = self.width as f32;
        let wind = self.wind * self.base_vel[layer];

        for read_idx in 0..count {
            let mut x = self.x[layer][read_idx] + wind;
            let y = self.y[layer][read_idx] + self.vel[layer][read_idx];

            // Remove if below screen
            if y > height + 10.0 {
                continue;
            }

            // Wrap horizontally
            if x < 0.0 {
                x += width;
            } else if x >= width {
                x -= width;
            }

            // Compact arrays (remove dead droplets)
            self.x[layer][write_idx] = x;
            self.y[layer][write_idx] = y;
            self.vel[layer][write_idx] = self.vel[layer][read_idx];
            self.len[layer][write_idx] = self.len[layer][read_idx];
            write_idx += 1;
        }

        self.count[layer] = write_idx;
    }

    fn render_layer(&mut self, layer: usize) {
        let w = self.width as i32;
        let h = self.height as i32;

        // Encode: layer (0-2) in high bits, position in trail (0-3) in low bits
        // 0 = empty
        // 1-12 = (layer * 4) + trail_pos + 1
        let layer_base = (layer as u8) * 4 + 1;

        for i in 0..self.count[layer] {
            let x = self.x[layer][i] as i32;
            let base_y = self.y[layer][i] as i32;
            let len = self.len[layer][i] as i32;

            if x < 0 || x >= w {
                continue;
            }

            // Render droplet trail
            for dy in 0..len {
                let y = base_y - dy;
                if y < 0 || y >= h {
                    continue;
                }

                let idx = (y * w + x) as usize;

                // Trail position: 0 = head, 1,2,3 = tail
                let trail_pos = dy.min(3) as u8;
                let encoded = layer_base + trail_pos;

                // Near layers overwrite far layers
                if encoded > self.output[idx] {
                    self.output[idx] = encoded;
                }
            }
        }
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

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.output = vec![0u8; (width * height) as usize];
        self.clear();
    }

    pub fn clear(&mut self) {
        self.count = [0; NUM_LAYERS];
        self.output.fill(0);
    }

    pub fn droplet_count(&self) -> usize {
        self.count.iter().sum()
    }
}
