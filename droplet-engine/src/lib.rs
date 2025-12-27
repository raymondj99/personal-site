// droplet-engine - Rain simulation with depth-aware physics
//
// Architecture:
//   scene/  - Auto-generated scene data from AI analysis
//   world/  - Terrain queries (depth, flow, normals)
//   sim/    - Simulation entities (drops, splashes, streams)
//   render  - Output encoding

use wasm_bindgen::prelude::*;

mod scene;
mod world;
mod sim;
mod render;

use sim::RainWorld as RainWorldInner;

// WASM wrapper - keeps the public API stable
#[wasm_bindgen]
pub struct RainWorld(RainWorldInner);

#[wasm_bindgen]
impl RainWorld {
    #[wasm_bindgen(constructor)]
    pub fn new(w: u32, h: u32) -> Self {
        Self(RainWorldInner::new(w, h))
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.0.resize(w, h);
    }

    pub fn tick(&mut self) {
        self.0.tick();
    }

    pub fn output_ptr(&self) -> *const u8 {
        self.0.output_ptr()
    }

    pub fn output_len(&self) -> usize {
        self.0.output_len()
    }

    pub fn width(&self) -> u32 {
        self.0.width()
    }

    pub fn height(&self) -> u32 {
        self.0.height()
    }
}
