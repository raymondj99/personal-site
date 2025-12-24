use wasm_bindgen::prelude::*;
use std::f32;

const DROPLET_CHARS: [char; 4] = ['.', ':', '|', '!'];
const SPLASH_CHARS: [char; 2] = ['`', '\''];

#[wasm_bindgen]
pub struct DropletWorld {
    width: u32,
    height: u32,
    droplets: Vec<Droplet>,
    grid: Vec<char>, 
    spawn_rate: f32,
    gravity: f32,
    frame_count: u32,
}

struct Droplet {
    x: f32,
    y: f32,
    velocity_y: f32,
    length :u32,
    alive: bool,
}

impl Droplet {
    fn new(x: f32, velocity_y: f32, length: u32) -> Self {
        Self {
            x,
            y: 0.0,
            velocity_y,
            length,
            alive: true,
        }
    }
}

#[wasm_bindgen]
impl DropletWorld {
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> Self {
        let grid_size = (width * height) as usize;
        Self {
            width,
            height,
            droplets: Vec::with_capacity(100),
            grid: vec![' '; grid_size],
            spawn_rate: 0.3,
            gravity: 0.8,
            frame_count: 0,
        }
    }

    pub fn step(&mut self) {
        self.frame_count += 1;

        // Clear grid
        self.grid.fill(' ');

        // Spawn new droplets
        if js_sys::Math::random() < self.spawn_rate as f64 {
            let x = (js_sys::Math::random() * self.width as f64) as f32;
            let velocity_y = 0.5 + (js_sys::Math::random() * 0.8) as f32;
            let length = 2 + (js_sys::Math::random() * 4.0) as u32;
            self.droplets.push(Droplet::new(x, velocity_y, length));
        }

        // Update droplets
        for droplet in &mut self.droplets {
            if !droplet.alive {
                continue;
            }

            // Apply gravity
            droplet.velocity_y += self.gravity * 0.05;
            droplet.y += droplet.velocity_y;

            // Check if hit bottom
            if droplet.y >= self.height as f32 {
                droplet.alive = false;
            }
        }

        // Remove dead droplets
        self.droplets.retain(|d| d.alive);

        // Render droplets to grid
        for droplet in &self.droplets {
            Self::render_droplet(droplet, &mut self.grid, self.width, self.height);
        }
    }

    fn render_droplet(droplet: &Droplet, grid: &mut [char], width: u32, height: u32) {
        let x = droplet.x as i32;

        for i in 0..droplet.length {
            let y = droplet.y as i32 - i as i32;

            if y >= 0 && y < height as i32 && x >= 0 && x < width as i32 {
                let idx = (y as u32 * width + x as u32) as usize;

                if idx < grid.len() {
                    // Head is brightest, tail fades
                    let char_idx = if i == 0 {
                        3
                    } else if i == 1 { 
                        2
                    } else if i == 2 { 
                        1
                    } else {
                        0
                    };

                    grid[idx] = DROPLET_CHARS[char_idx];
                }
            }
        }
    }

    /// Get the current frame as a string with newlines
    pub fn frame_string(&self) -> String {
        let mut result = String::with_capacity((self.width * self.height + self.height) as usize);

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x) as usize;
                result.push(self.grid[idx]);
            }
            result.push('\n');
        }

        result
    }

    /// Configure spawn rate (0.0 - 1.0)
    pub fn set_spawn_rate(&mut self, rate: f32) {
        self.spawn_rate = rate.clamp(0.0, 1.0);
    }

    /// Configure gravity strength
    pub fn set_gravity(&mut self, gravity: f32) {
        self.gravity = gravity.clamp(0.0, 2.0);
    }

    /// Get current droplet count
    pub fn droplet_count(&self) -> usize {
        self.droplets.len()
    }

    /// Clear all droplets
    pub fn clear(&mut self) {
        self.droplets.clear();
        self.grid.fill(' ');
    }
}