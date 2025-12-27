// geometry.rs - Compute derived geometry from AI outputs
//
// Normals, flow field, ambient occlusion, ground mask.

/// Compute ground mask from semantic segmentation
/// Ground = surfaces where water can flow (excludes sky, trees, etc.)
pub fn compute_ground_mask(segments: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let h = segments.len();
    let w = segments.get(0).map_or(0, |r| r.len());
    let mut ground = vec![vec![0u8; w]; h];

    // ADE20K classes that are NOT ground
    const NON_GROUND: &[u8] = &[
        2,   // sky
        4,   // tree
        17,  // plant
        72,  // palm tree
    ];

    let mut count = 0;
    for y in 0..h {
        for x in 0..w {
            if !NON_GROUND.contains(&segments[y][x]) {
                ground[y][x] = 1;
                count += 1;
            }
        }
    }

    let pct = count as f32 / (w * h) as f32 * 100.0;
    println!("    Ground coverage: {:.1}% ({} pixels)", pct, count);

    ground
}

/// Compute surface normals from depth using central differences
/// Returns (nx, ny) packed as i8 values
pub fn compute_normals(depth: &[Vec<f32>], scale: f32) -> (Vec<Vec<i8>>, Vec<Vec<i8>>) {
    let h = depth.len();
    let w = depth.get(0).map_or(0, |r| r.len());

    let mut nx = vec![vec![0i8; w]; h];
    let mut ny = vec![vec![0i8; w]; h];

    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let dzdx = (depth[y][x + 1] - depth[y][x - 1]) * scale;
            let dzdy = (depth[y + 1][x] - depth[y - 1][x]) * scale;

            // Normal = normalize(-dzdx, -dzdy, 1.0)
            let len = (dzdx * dzdx + dzdy * dzdy + 1.0).sqrt();
            let norm_x = -dzdx / len;
            let norm_y = -dzdy / len;

            nx[y][x] = (norm_x * 127.0).clamp(-127.0, 127.0) as i8;
            ny[y][x] = (norm_y * 127.0).clamp(-127.0, 127.0) as i8;
        }
    }

    // Fill edges
    fill_edges(&mut nx, w, h);
    fill_edges(&mut ny, w, h);

    (nx, ny)
}

/// Compute flow field from depth gradient
/// Water flows toward higher depth (lower elevation)
pub fn compute_flow_field(depth: &[Vec<f32>], ground: &[Vec<u8>]) -> (Vec<Vec<i8>>, Vec<Vec<i8>>) {
    let h = depth.len();
    let w = depth.get(0).map_or(0, |r| r.len());

    let mut fx = vec![vec![0i8; w]; h];
    let mut fy = vec![vec![0i8; w]; h];

    // Multi-scale gradient with horizontal boost
    let scales: [(i32, f32); 3] = [
        (2, 0.25),   // Fine
        (5, 0.40),   // Medium
        (10, 0.35),  // Coarse
    ];
    let horizontal_boost = 2.5f32;
    let margin = 10;

    for y in margin..h.saturating_sub(margin) {
        for x in margin..w.saturating_sub(margin) {
            // Only compute on ground
            if ground[y][x] == 0 {
                continue;
            }

            let mut grad_x = 0.0f32;
            let mut grad_y = 0.0f32;

            for &(offset, weight) in &scales {
                let o = offset as usize;
                let dx = depth[y][x + o] - depth[y][x - o];
                let dy = depth[y + o][x] - depth[y - o][x];

                grad_x += dx * weight * horizontal_boost;
                grad_y += dy * weight;
            }

            // Gravity bias
            grad_y += 0.02;

            // Normalize
            let len = (grad_x * grad_x + grad_y * grad_y).sqrt();
            if len > 0.001 {
                let norm_x = grad_x / len;
                let norm_y = grad_y / len;
                let strength = (len * 8.0 + 0.4).min(1.0);

                fx[y][x] = (norm_x * strength * 127.0).clamp(-127.0, 127.0) as i8;
                fy[y][x] = (norm_y * strength * 127.0).clamp(-127.0, 127.0) as i8;
            } else {
                // Flat: pure gravity
                fy[y][x] = 51;
            }
        }
    }

    (fx, fy)
}

/// Compute screen-space ambient occlusion from depth
pub fn compute_ao(depth: &[Vec<f32>], radius: usize) -> Vec<Vec<u8>> {
    let h = depth.len();
    let w = depth.get(0).map_or(0, |r| r.len());
    let mut ao = vec![vec![255u8; w]; h];

    let r = radius as i32;

    for y in 0..h {
        for x in 0..w {
            let center = depth[y][x];
            let mut occlusion = 0.0f32;
            let mut samples = 0;

            for dy in -r..=r {
                for dx in -r..=r {
                    if dx == 0 && dy == 0 { continue; }

                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                    let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                    let sample = depth[sy][sx];

                    if sample > center {
                        let diff = (sample - center).min(0.15);
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        occlusion += diff / (1.0 + dist * 0.5);
                    }
                    samples += 1;
                }
            }

            let ao_factor = 1.0 - (occlusion / samples as f32 * 8.0).min(0.7);
            ao[y][x] = (ao_factor * 255.0) as u8;
        }
    }

    ao
}

fn fill_edges<T: Copy>(arr: &mut [Vec<T>], w: usize, h: usize) {
    if h < 2 || w < 2 { return; }

    for y in 0..h {
        arr[y][0] = arr[y][1];
        arr[y][w - 1] = arr[y][w - 2];
    }
    for x in 0..w {
        arr[0][x] = arr[1][x];
        arr[h - 1][x] = arr[h - 2][x];
    }
}
