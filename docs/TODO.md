# TODO MVP: Geometric Mountain Rain Interaction

**Goal:** Ship a beautiful, performant rain-on-mountain experience in 14 days.  
**Philosophy:** Technology + Aesthetics. Minimal geometry, maximum impact.

---

## 🎯 Success Criteria

- [ ] Rain falls and collides with geometric mountain
- [ ] Water slides down surfaces into pools
- [ ] Proper depth layering (rain behind/in-front of mountain)
- [ ] Simple lighting makes mountain feel 3D
- [ ] Runs at 60 FPS with 3000+ particles
- [ ] Code is clean enough to show in interviews

---

## Sprint 1A: Static Mountain (Days 1-3)

**Goal:** See a mountain on screen. No interaction yet.

### Core Geometry
- [ ] Create `mesh.rs` module
- [ ] Define `Mesh` struct using SoA (Structure of Arrays):
  ```rust
  pub struct Mesh {
      // Vertices (normalized 0-1 screen space)
      vx: [f32; 16],    // x positions
      vy: [f32; 16],    // y positions  
      vz: [f32; 16],    // depth (0=near, 1=far)
      vn: usize,        // vertex count (actual used)
      
      // Faces (triangle indices)
      faces: [[usize; 3]; 24],  // vertex index triplets
      fn_count: usize,          // face count (actual used)
  }
  ```

- [ ] Hardcode mountain geometry (7 vertices, 5-7 faces):
  ```rust
  impl Mesh {
      pub fn mountain() -> Self {
          // Copy vertex positions from implementation guide
          // Lower-left anchor, peak offset right
          // Asymmetric for visual interest
      }
  }
  ```

### Triangle Rasterization
- [ ] Create `raster.rs` module
- [ ] Implement `point_in_triangle(px, py, x0, y0, x1, y1, x2, y2) -> bool`
  - Use barycentric coordinates
  - Copy implementation from guide (it works!)
  
- [ ] Implement `fill_triangle(output, width, x0, y0, x1, y1, x2, y2, char_value)`
  - Scanline rasterization
  - Find bounding box
  - Test each pixel with `point_in_triangle`
  - Write character value to output buffer

### First Render
- [ ] Add `render_mesh(mesh, output, width, height)` function
- [ ] For each face in mesh:
  - Get 3 vertex positions in screen coordinates
  - Calculate brightness from average depth
  - Call `fill_triangle` with brightness value
  
- [ ] Update main render loop:
  ```rust
  // Old: just render rain
  // New: render rain, then render mountain
  render_drops_all(&rain, output);
  render_mesh(&mountain, output, width, height);
  ```

### Validation
- [ ] Mountain appears on screen
- [ ] No crashes, no NaN positions
- [ ] Can see individual triangular faces
- [ ] Rain renders over mountain (wrong, but that's OK for now)

**Deliverable:** Static mountain renders. Rain passes through it.  
**Time Check:** If this takes >3 days, simplify the mountain to 4 vertices, 2 triangles.

---

## Sprint 1B: Collision Detection (Days 4-7)

**Goal:** Rain hits the mountain and reacts.

### Collision System
- [ ] Create `collision.rs` module
- [ ] Add AABB bounds check:
  ```rust
  pub fn drop_in_bounds(drop_x: f32, drop_y: f32, bounds: AABB) -> bool {
      // Simple rectangle test
      // Reject 90% of drops without expensive tests
  }
  ```

- [ ] Implement ray-triangle intersection:
  ```rust
  pub fn ray_hits_triangle(
      ray_x: f32, ray_y: f32,  // drop position
      x0: f32, y0: f32,         // triangle vertices
      x1: f32, y1: f32,
      x2: f32, y2: f32,
  ) -> Option<f32> {
      // Returns y-position of hit, or None
      // Use point_in_triangle test
      // Check if ray is above triangle
  }
  ```

- [ ] Add collision check to drop update:
  ```rust
  // In rain.rs update loop:
  if drop_in_bounds(dx[i], dy[i], mountain_bounds) {
      for face_idx in 0..mountain.fn_count {
          if let Some(hit_y) = ray_hits_face(drop, face) {
              // Collision!
              handle_collision(i, face_idx, hit_y);
              break;
          }
      }
  }
  ```

### Drop State Extension
- [ ] Add state enum to drops:
  ```rust
  #[derive(Copy, Clone, PartialEq)]
  pub enum DropState {
      Falling,
      Hit { face: usize, timer: u8 },
  }
  ```

- [ ] Add state array to rain SoA:
  ```rust
  // In rain.rs
  ds: [DropState; MAX_DROPS],
  ```

### Collision Response
- [ ] On collision, set state to `Hit { face, timer: 0 }`
- [ ] Spawn 3-5 splatter particles:
  ```rust
  fn spawn_splatter(&mut self, idx: usize) {
      for _ in 0..4 {
          let offset_x = (rand() - 0.5) * 4.0;
          let offset_y = -rand() * 2.0;
          self.spawn_particle(
              self.dx[idx] + offset_x,
              self.dy[idx] + offset_y,
              self.dz[idx],
              DropState::Hit { face: 0, timer: 10 }
          );
      }
  }
  ```

- [ ] Update `Hit` state in main loop:
  ```rust
  DropState::Hit { timer } => {
      timer += 1;
      if timer > 10 {
          remove_drop(i);  // Fade out after 10 frames
      }
  }
  ```

### Validation
- [ ] Rain hits mountain and stops
- [ ] Small splatter appears on impact
- [ ] No performance drop (still 60 FPS)
- [ ] Collision works at different screen sizes

**Deliverable:** Rain collides with mountain and splatters.  
**Time Check:** If collision is too slow, reduce face count or add simpler bounds.

---

## Sprint 2: Depth & Sliding (Days 8-12)

**Goal:** Proper 3D layering and water flow.

### Three-Pass Rendering
- [ ] Refactor render to three passes:
  ```rust
  pub fn render_scene(rain, mountain, output, width, height) {
      // Pass 1: Background drops (z > 0.55)
      for i in 0..rain.dn {
          if rain.dz[i] > 0.55 {
              render_drop(i, output);
          }
      }
      
      // Pass 2: Mountain
      render_mesh(mountain, output, width, height);
      
      // Pass 3: Foreground drops (z <= 0.55)
      for i in 0..rain.dn {
          if rain.dz[i] <= 0.55 {
              render_drop(i, output);
          }
      }
  }
  ```

- [ ] Test that rain behind mountain is occluded
- [ ] Test that rain in front renders over mountain

### Depth-Aware Collision
- [ ] Only check collision if `abs(drop.z - face.avg_z) < 0.15`
- [ ] Skip faces that are too far in front/behind
- [ ] Improves performance and correctness

### Sliding State
- [ ] Extend `DropState`:
  ```rust
  pub enum DropState {
      Falling,
      Sliding { face: usize, progress: f32 },
      Splatter { timer: u8 },
  }
  ```

- [ ] On collision, transition to `Sliding { progress: 0.0 }`
- [ ] Update sliding drops:
  ```rust
  DropState::Sliding { face, progress } => {
      // Find lowest vertex of face
      let target = lowest_vertex(mountain, face);
      
      // Interpolate from current position to target
      let t = progress.min(1.0);
      self.dx[i] = lerp(self.dx[i], target.x, t);
      self.dy[i] = lerp(self.dy[i], target.y, t);
      
      progress += dt * 2.0;  // Takes ~0.5s to slide
      
      if progress >= 1.0 {
          // Reached bottom, start pooling
          self.ds[i] = DropState::Splatter { timer: 0 };
          add_to_pool(self.dx[i], self.dz[i]);
      }
  }
  ```

### Pool Integration
- [ ] Extend existing splash pool system:
  ```rust
  // In your existing pool code
  pub fn add_mountain_water(x: f32, z: f32) {
      // Find or create pool at (x, z)
      // Increment height
  }
  ```

- [ ] Call from sliding update when drop reaches bottom
- [ ] Render pools in final pass (always on top)

### Validation
- [ ] Drops behind mountain are hidden
- [ ] Drops in front are visible
- [ ] Water slides down mountain faces
- [ ] Pools accumulate at mountain base

**Deliverable:** Complete water flow from sky → mountain → pool.  
**Time Check:** Sliding animation should feel natural, not robotic.

---

## Sprint 3: Lighting & Polish (Days 13-14)

**Goal:** Make it beautiful.

### Face Normal Lighting
- [ ] Calculate face normals in mesh initialization:
  ```rust
  impl Mesh {
      fn calculate_normals(&mut self) {
          for i in 0..self.fn_count {
              let face = self.faces[i];
              // Cross product of two edges
              let normal = cross_2d(edge1, edge2);
              self.nx[i] = normal.x;
              self.ny[i] = normal.y;
          }
      }
  }
  ```

- [ ] Add lighting to face rendering:
  ```rust
  fn get_face_brightness(face_idx: usize, mesh: &Mesh) -> u8 {
      // Simple directional light from top-left
      let light_dir = (-0.5, -1.0);  // Normalized
      let normal = (mesh.nx[face_idx], mesh.ny[face_idx]);
      
      // Dot product
      let diffuse = (normal.0 * light_dir.0 + normal.1 * light_dir.1)
          .max(0.0);
      
      // Map to brightness range
      let ambient = 0.3;
      let final_light = ambient + diffuse * 0.7;
      
      (final_light * 32.0) as u8  // Your brightness range
  }
  ```

- [ ] Update `fill_triangle` to use calculated brightness

### Depth Fog
- [ ] Dim distant faces:
  ```rust
  fn apply_depth_fog(brightness: u8, z: f32) -> u8 {
      let fog_factor = 1.0 - (z * 0.5);  // z=0 (near) full bright, z=1 (far) 50% bright
      (brightness as f32 * fog_factor) as u8
  }
  ```

### Waterfall Effect
- [ ] Add waterfall spawner:
  ```rust
  pub struct Waterfall {
      edge: (usize, usize),  // Vertex pair
      spawn_timer: u32,
  }
  
  impl Waterfall {
      pub fn update(&mut self, rain: &mut Rain, mesh: &Mesh) {
          self.spawn_timer += 1;
          
          if self.spawn_timer % 2 == 0 {  // Every other frame
              // Spawn drop at top of edge
              let (v0, v1) = self.edge;
              let x = mesh.vx[v0];
              let y = mesh.vy[v0];
              let z = mesh.vz[v0];
              
              rain.spawn_at(x, y, z);
          }
      }
  }
  ```

- [ ] Add to mountain initialization:
  ```rust
  let waterfall = Waterfall {
      edge: (6, 1),  // Peak to right base
      spawn_timer: 0,
  };
  ```

- [ ] Call `waterfall.update()` in main loop

### Final Polish
- [ ] Tune particle counts (3000 drops feels good?)
- [ ] Tune splatter intensity (4 particles per hit?)
- [ ] Tune sliding speed (0.5s per face?)
- [ ] Tune pool evaporation rate
- [ ] Verify 60 FPS on target device

### Debug Tooling
- [ ] Add FPS counter to Svelte UI
- [ ] Add particle count display
- [ ] Add toggle for wireframe view (render face edges)
- [ ] Add toggle to disable collision (test rendering alone)

**Deliverable:** Polished, shippable experience.

---

## Post-MVP: Future Enhancements

**Don't build these now. Ship first, iterate later.**

### Phase 2: Advanced Geometry
- [ ] MeshBuilder API for runtime mesh creation
- [ ] Primitive generators (procedural mountains, rocks)
- [ ] Multiple meshes in scene
- [ ] Per-pixel depth buffer (vs painter's algorithm)
- [ ] Edge rendering (silhouette detection)

### Phase 3: Water Dynamics
- [ ] Edge-to-edge water flow
- [ ] Multiple waterfall sources
- [ ] Valley accumulation
- [ ] Stream formation

### Phase 4: Scene System
- [ ] Scene graph with transforms
- [ ] Level of detail (LOD)
- [ ] Mesh serialization (save/load)
- [ ] Material system with per-face properties

### Phase 5: Interactions
- [ ] Mouse-reactive lighting (cursor is light source)
- [ ] Seasonal variations (snow mode?)
- [ ] Sound effects on collision
- [ ] Mobile touch interactions

### Technical Debt:
- [ ] Profile and optimize hot paths
- [ ] Add comprehensive unit tests
- [ ] Document coordinate systems
- [ ] Clean up magic numbers
- [ ] Add error handling for edge cases

---

## Performance Budget

### Target Performance:
- **60 FPS** (16.67ms per frame)
- **3000 particles** active simultaneously
- **< 500 triangles** total (realistically 5-10 for MVP)

### Frame Time Breakdown:
- Drop update: < 5ms
- Collision detection: < 2ms
- Rendering: < 8ms
- Other (Svelte, WASM overhead): < 2ms

### Memory Budget:
- Mesh data: ~2 KB (16 vertices * 4 bytes * 4 attributes)
- Drop data: ~100 KB (3000 drops * 6 attributes * 4 bytes)
- Output buffer: ~100 KB (typical terminal size)
- **Total: ~500 KB** (well under WASM limits)

---

## Debug Checklist

### Before Committing Each Sprint:
- [ ] No console errors
- [ ] No NaN or Infinity values
- [ ] Works at 800x600 and 1920x1080
- [ ] FPS stays above 50
- [ ] Memory usage is stable (no leaks)
- [ ] Code passes `cargo clippy`
- [ ] Visual appearance matches your aesthetic vision

---

## Validation Questions

**After Sprint 1A:**
- Can you see the mountain?
- Does it look geometric and intentional?
- Is the shape interesting?

**After Sprint 1B:**
- Does rain convincingly hit the mountain?
- Do splatter particles feel satisfying?
- Is performance still smooth?

**After Sprint 2:**
- Does depth layering look correct?
- Does sliding motion feel natural?
- Do pools accumulate as expected?

**After Sprint 3:**
- Does lighting make the mountain feel 3D?
- Does the waterfall draw your eye?
- Would you put this on your portfolio?

---

## File Structure

```
src/
├── lib.rs           # WASM entry point, main loop
├── rain.rs          # Existing rain system (extend with DropState)
├── mesh.rs          # NEW: Mesh struct, hardcoded mountain
├── raster.rs        # NEW: Triangle rasterization
├── collision.rs     # NEW: AABB, ray-triangle tests
└── waterfall.rs     # NEW: Waterfall spawner
```

**Keep it simple.** Don't create files until you need them.

---

## Implementation Tips

### When You Get Stuck:
1. **Simplify the mesh** - Can you reproduce the bug with 3 vertices?
2. **Add debug rendering** - Draw vertex positions, face normals
3. **Test in isolation** - Does `point_in_triangle` work with known inputs?
4. **Check your math** - Print intermediate values, verify formulas
5. **Take a break** - Walk away, come back with fresh eyes

### Performance Optimization Order:
1. **Profile first** - Don't optimize what you haven't measured
2. **Algorithmic wins** - AABB before triangle tests
3. **Cache-friendly data** - You're already using SoA ✓
4. **Batch by depth bucket** - Group drops before rendering to reduce state changes
5. **SIMD if needed** - But probably not for MVP

### Key Algorithms (Reference):
- **Barycentric point-in-triangle**: O(1), 6 muls, 3 adds
- **Möller–Trumbore ray-triangle**: O(1), fastest intersection test
- **Scanline rasterization**: O(pixels in triangle), simple and cache-friendly
- **Painter's algorithm**: O(n log n) sort, good enough for <100 faces

### Code Quality:
- **Comments for "why"** not "what"
- **Descriptive variable names** - `face_idx` not `fi`
- **Small functions** - Each does one thing well
- **No premature abstraction** - Wait until you use it 3x

---

## Success Metrics

**You're done when:**
- Rain falls, hits mountain, slides, pools
- Mountain looks 3D with lighting
- Runs at 60 FPS
- You're proud to show it to people
- Code is clean enough to discuss in an interview

**Ship it and iterate.** Perfect is the enemy of shipped.

---

## Timeline Overview

| Days   | Sprint | Focus                  | Deliverable                    |
|--------|--------|------------------------|--------------------------------|
| 1-3    | 1A     | Static mountain        | Mountain renders               |
| 4-7    | 1B     | Collision              | Rain splatters on mountain     |
| 8-12   | 2      | Depth & sliding        | Water flows down to pools      |
| 13-14  | 3      | Lighting & polish      | Shippable experience           |

**Total: 14 days from zero to portfolio-ready.**

Good luck! Remember: **shipping beats perfection.**