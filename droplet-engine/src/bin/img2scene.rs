// img2scene - Convert image to pixel art with depth estimation and geometry extraction
//
// Usage: cargo run --bin img2scene -- <image> [--cols 320] [--rows 180] [--colors 32]
//
// Outputs:
//   - Pixel art with palette
//   - Depth map (0-255, 0=far, 255=near)
//   - Normal map (packed xy, derived from depth)
//   - Flow field (packed xy, gradient-based water flow direction)
//   - Ambient occlusion map (pre-baked lighting)

use image::{DynamicImage, GenericImageView, imageops::FilterType};
use ndarray::Array4;
use ort::session::Session;
use ort::value::Value;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

// ============================================================================
// Color operations
// ============================================================================

#[derive(Clone, Copy, Default)]
struct Color { r: f32, g: f32, b: f32 }

impl Color {
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r: r as f32, g: g as f32, b: b as f32 }
    }

    fn to_rgb(self) -> (u8, u8, u8) {
        (self.r.clamp(0.0, 255.0) as u8,
         self.g.clamp(0.0, 255.0) as u8,
         self.b.clamp(0.0, 255.0) as u8)
    }

    fn dist_sq(self, o: Color) -> f32 {
        let (dr, dg, db) = (self.r - o.r, self.g - o.g, self.b - o.b);
        dr*dr + dg*dg + db*db
    }
}

impl std::ops::Add for Color {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self { r: self.r + o.r, g: self.g + o.g, b: self.b + o.b }
    }
}

impl std::ops::Mul<f32> for Color {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self { r: self.r * s, g: self.g * s, b: self.b * s }
    }
}

// ============================================================================
// Palette extraction (k-means with k-means++ init)
// ============================================================================

fn nearest(c: Color, palette: &[Color]) -> usize {
    palette.iter().enumerate()
        .min_by(|(_, a), (_, b)| c.dist_sq(**a).partial_cmp(&c.dist_sq(**b)).unwrap())
        .map(|(i, _)| i).unwrap_or(0)
}

fn kmeans(pixels: &[Color], k: usize, iters: usize) -> Vec<Color> {
    if pixels.is_empty() || k == 0 { return vec![]; }

    // K-means++ initialization
    let mut centroids = Vec::with_capacity(k);
    centroids.push(pixels[pixels.len() / 2]);

    for _ in 1..k {
        let (mut best_dist, mut best_idx) = (0.0f32, 0);
        for (i, p) in pixels.iter().enumerate() {
            let d = centroids.iter().map(|c| p.dist_sq(*c)).fold(f32::MAX, f32::min);
            if d > best_dist { best_dist = d; best_idx = i; }
        }
        centroids.push(pixels[best_idx]);
    }

    // Iterate: assign + update centroids
    let mut counts = vec![0usize; k];
    let mut sums = vec![Color::default(); k];

    for _ in 0..iters {
        counts.fill(0);
        sums.fill(Color::default());

        for p in pixels {
            let c = nearest(*p, &centroids);
            counts[c] += 1;
            sums[c] = sums[c] + *p;
        }

        for i in 0..k {
            if counts[i] > 0 {
                centroids[i] = sums[i] * (1.0 / counts[i] as f32);
            }
        }
    }

    centroids
}

// ============================================================================
// Dithering
// ============================================================================

fn floyd_steinberg(pixels: &mut [Vec<Color>], palette: &[Color]) -> Vec<Vec<u8>> {
    let (h, w) = (pixels.len(), pixels.get(0).map_or(0, |r| r.len()));
    let mut result = vec![vec![0u8; w]; h];

    for y in 0..h {
        for x in 0..w {
            let old = pixels[y][x];
            let idx = nearest(old, palette);
            let new = palette[idx];
            result[y][x] = idx as u8;

            let err = Color { r: old.r - new.r, g: old.g - new.g, b: old.b - new.b };

            if x + 1 < w { pixels[y][x + 1] = pixels[y][x + 1] + err * (7.0/16.0); }
            if y + 1 < h {
                if x > 0 { pixels[y + 1][x - 1] = pixels[y + 1][x - 1] + err * (3.0/16.0); }
                pixels[y + 1][x] = pixels[y + 1][x] + err * (5.0/16.0);
                if x + 1 < w { pixels[y + 1][x + 1] = pixels[y + 1][x + 1] + err * (1.0/16.0); }
            }
        }
    }
    result
}

// ============================================================================
// MiDaS depth estimation
// ============================================================================

fn estimate_depth(img: &DynamicImage, tw: u32, th: u32) -> Vec<Vec<f32>> {
    const MIDAS_SIZE: u32 = 256;
    let model_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("models/midas_small.onnx");

    if !model_path.exists() {
        eprintln!("    MiDaS model not found, using fallback");
        return fallback_depth(tw, th);
    }

    let Ok(builder) = Session::builder() else {
        return fallback_depth(tw, th);
    };
    let Ok(mut session) = builder.commit_from_file(&model_path) else {
        return fallback_depth(tw, th);
    };

    println!("    Running MiDaS depth estimation...");
    let resized = img.resize_exact(MIDAS_SIZE, MIDAS_SIZE, FilterType::Lanczos3);

    // ImageNet normalization
    const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
    const STD: [f32; 3] = [0.229, 0.224, 0.225];

    let mut input = Array4::<f32>::zeros((1, 3, MIDAS_SIZE as usize, MIDAS_SIZE as usize));
    for y in 0..MIDAS_SIZE {
        for x in 0..MIDAS_SIZE {
            let p = resized.get_pixel(x, y);
            for c in 0..3 {
                input[[0, c, y as usize, x as usize]] = (p[c] as f32 / 255.0 - MEAN[c]) / STD[c];
            }
        }
    }

    let Ok(input_val) = Value::from_array(input) else { return fallback_depth(tw, th); };
    let input_name = session.inputs.first().map(|i| i.name.clone()).unwrap_or_else(|| "image".into());
    let Ok(outputs) = session.run(ort::inputs![input_name => input_val]) else { return fallback_depth(tw, th); };
    let Ok(arr) = outputs[0].try_extract_array::<f32>() else { return fallback_depth(tw, th); };

    let shape = arr.shape();
    let (oh, ow) = match shape.len() {
        4 => (shape[2], shape[3]),
        3 => (shape[1], shape[2]),
        2 => (shape[0], shape[1]),
        _ => return fallback_depth(tw, th),
    };

    // Extract and normalize
    let flat: Vec<f32> = arr.iter().copied().collect();
    let (min_d, max_d) = flat.iter().fold((f32::MAX, f32::MIN), |(mn, mx), &v| (mn.min(v), mx.max(v)));
    let range = (max_d - min_d).max(1e-6);

    // Bilinear resize to target
    let (sx, sy) = (ow as f32 / tw as f32, oh as f32 / th as f32);
    let mut depth = vec![vec![0.0f32; tw as usize]; th as usize];

    for y in 0..th as usize {
        for x in 0..tw as usize {
            let (fx, fy) = (x as f32 * sx, y as f32 * sy);
            let (x0, y0) = (fx as usize, fy as usize);
            let (x1, y1) = ((x0 + 1).min(ow - 1), (y0 + 1).min(oh - 1));
            let (tx, ty) = (fx.fract(), fy.fract());

            let sample = |sx: usize, sy: usize| {
                let v = flat.get(sy * ow + sx).copied().unwrap_or(0.0);
                (v - min_d) / range
            };

            let v = sample(x0, y0) * (1.0 - tx) * (1.0 - ty)
                  + sample(x1, y0) * tx * (1.0 - ty)
                  + sample(x0, y1) * (1.0 - tx) * ty
                  + sample(x1, y1) * tx * ty;
            depth[y][x] = v;
        }
    }

    depth
}

fn fallback_depth(w: u32, h: u32) -> Vec<Vec<f32>> {
    (0..h as usize).map(|y| vec![y as f32 / h as f32; w as usize]).collect()
}

// ============================================================================
// Collision detection from depth
// ============================================================================

fn generate_collision(depth: &[Vec<f32>], w: usize, h: usize) -> Vec<Vec<u8>> {
    // Find ground line via depth gradient
    let row_avg: Vec<f32> = depth.iter()
        .map(|row| row.iter().sum::<f32>() / w as f32)
        .collect();

    let mut ground_y = (h as f32 * 0.85) as usize;
    let mut max_grad = 0.0f32;

    for y in (h / 3)..(h - 5) {
        let before: f32 = row_avg[y.saturating_sub(3)..y].iter().sum::<f32>() / 3.0;
        let after: f32 = row_avg[y..(y + 3).min(h)].iter().sum::<f32>() / 3.0;
        let grad = after - before;
        if grad > max_grad && grad > 0.03 {
            max_grad = grad;
            ground_y = y;
        }
    }

    let threshold = (row_avg[ground_y.min(h - 1)] - 0.1).max(0.3);
    println!("    Ground at y={}, threshold={:.2}", ground_y, threshold);

    let mut collision = vec![vec![0u8; w]; h];

    // Mark solid based on depth
    for y in 0..h {
        for x in 0..w {
            let d = depth[y][x];
            if (y >= ground_y && d > threshold * 0.8) || d > 0.85 {
                collision[y][x] = 1;
            }
        }
    }

    // Extend upward from ground for objects
    for x in 0..w {
        let mut in_obj = false;
        for y in (0..ground_y).rev() {
            let d = depth[y][x];
            let below = depth.get(y + 1).and_then(|r| r.get(x)).copied().unwrap_or(0.0);
            if d > 0.6 && d > below - 0.1 {
                collision[y][x] = 1;
                in_obj = true;
            } else if in_obj && d < 0.3 {
                in_obj = false;
            }
        }
    }

    // Horizontal smoothing
    let mut smooth = collision.clone();
    for y in 0..h {
        for x in 2..w.saturating_sub(2) {
            let sum: u8 = (0..5).map(|dx| collision[y][x - 2 + dx]).sum();
            smooth[y][x] = if sum >= 3 { 1 } else { 0 };
        }
    }

    // Fill vertical gaps
    for y in 1..h.saturating_sub(1) {
        for x in 0..w {
            if smooth[y - 1][x] == 1 && smooth[y + 1][x] == 1 {
                smooth[y][x] = 1;
            }
        }
    }

    smooth
}

// ============================================================================
// Ground mask from semantic segmentation
// ============================================================================

/// Compute ground mask from semantic segmentation.
/// Ground surfaces are where water can flow: everything except sky, trees, and similar.
/// Uses exclusion list approach - any surface not excluded is considered ground.
fn compute_ground_mask(segments: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let h = segments.len();
    let w = segments.get(0).map_or(0, |r| r.len());
    let mut ground = vec![vec![0u8; w]; h];

    // ADE20K classes that are NOT ground (water cannot flow on these)
    const NON_GROUND_CLASSES: &[u8] = &[
        2,   // sky
        4,   // tree (vertical trunks, canopy)
        17,  // plant (vertical vegetation)
        72,  // palm tree
    ];

    let mut ground_count = 0;
    for y in 0..h {
        for x in 0..w {
            let class = segments[y][x];
            if !NON_GROUND_CLASSES.contains(&class) {
                ground[y][x] = 1;
                ground_count += 1;
            }
        }
    }

    let pct = ground_count as f32 / (w * h) as f32 * 100.0;
    println!("    Ground coverage: {:.1}% ({} pixels)", pct, ground_count);

    ground
}

// ============================================================================
// Semantic segmentation using SegFormer (ADE20K - 150 classes)
// ============================================================================

/// ADE20K class labels for reference (used in visualization)
/// Key natural scene classes:
///   2=sky, 4=tree, 6=road, 9=grass, 13=earth, 16=mountain,
///   21=water, 26=sea, 29=field, 34=rock, 46=sand, 52=path,
///   60=river, 68=hill, 72=palm, 94=land, 113=waterfall, 128=lake

fn estimate_segmentation(img: &DynamicImage, tw: u32, th: u32) -> Vec<Vec<u8>> {
    const SEGFORMER_SIZE: u32 = 512;
    let model_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("models/segformer_b0_ade20k.onnx");

    if !model_path.exists() {
        eprintln!("    SegFormer model not found, using fallback segmentation");
        return fallback_segmentation(tw, th);
    }

    let Ok(builder) = Session::builder() else {
        return fallback_segmentation(tw, th);
    };
    let Ok(mut session) = builder.commit_from_file(&model_path) else {
        return fallback_segmentation(tw, th);
    };

    println!("    Running SegFormer semantic segmentation...");
    let resized = img.resize_exact(SEGFORMER_SIZE, SEGFORMER_SIZE, FilterType::Lanczos3);

    // ImageNet normalization (same as MiDaS)
    const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
    const STD: [f32; 3] = [0.229, 0.224, 0.225];

    let mut input = Array4::<f32>::zeros((1, 3, SEGFORMER_SIZE as usize, SEGFORMER_SIZE as usize));
    for y in 0..SEGFORMER_SIZE {
        for x in 0..SEGFORMER_SIZE {
            let p = resized.get_pixel(x, y);
            for c in 0..3 {
                input[[0, c, y as usize, x as usize]] = (p[c] as f32 / 255.0 - MEAN[c]) / STD[c];
            }
        }
    }

    let Ok(input_val) = Value::from_array(input) else { return fallback_segmentation(tw, th); };
    let input_name = session.inputs.first().map(|i| i.name.clone()).unwrap_or_else(|| "pixel_values".into());
    let Ok(outputs) = session.run(ort::inputs![input_name => input_val]) else { return fallback_segmentation(tw, th); };
    let Ok(arr) = outputs[0].try_extract_array::<f32>() else { return fallback_segmentation(tw, th); };

    let shape = arr.shape();
    // SegFormer output: (1, num_classes, H/4, W/4) = (1, 150, 128, 128)
    let (num_classes, oh, ow) = match shape.len() {
        4 => (shape[1], shape[2], shape[3]),
        3 => (shape[0], shape[1], shape[2]),
        _ => return fallback_segmentation(tw, th),
    };

    // Argmax over classes for each pixel
    let flat: Vec<f32> = arr.iter().copied().collect();
    let mut seg_map = vec![vec![0u8; ow]; oh];

    for y in 0..oh {
        for x in 0..ow {
            let mut max_val = f32::MIN;
            let mut max_class = 0u8;
            for c in 0..num_classes {
                let idx = c * oh * ow + y * ow + x;
                let val = flat.get(idx).copied().unwrap_or(f32::MIN);
                if val > max_val {
                    max_val = val;
                    max_class = c as u8;
                }
            }
            seg_map[y][x] = max_class;
        }
    }

    // Bilinear resize to target dimensions
    let (sx, sy) = (ow as f32 / tw as f32, oh as f32 / th as f32);
    let mut segments = vec![vec![0u8; tw as usize]; th as usize];

    for y in 0..th as usize {
        for x in 0..tw as usize {
            // Nearest neighbor for class labels (bilinear doesn't make sense for discrete classes)
            let src_x = ((x as f32 + 0.5) * sx) as usize;
            let src_y = ((y as f32 + 0.5) * sy) as usize;
            let src_x = src_x.min(ow - 1);
            let src_y = src_y.min(oh - 1);
            segments[y][x] = seg_map[src_y][src_x];
        }
    }

    // Count unique classes for logging
    let mut class_counts = [0u32; 150];
    for row in &segments {
        for &c in row {
            if (c as usize) < 150 {
                class_counts[c as usize] += 1;
            }
        }
    }
    let unique_classes: Vec<u8> = class_counts.iter().enumerate()
        .filter(|&(_, count)| *count > 0)
        .map(|(i, _)| i as u8)
        .collect();
    println!("    Found {} unique semantic classes", unique_classes.len());

    segments
}

fn fallback_segmentation(w: u32, h: u32) -> Vec<Vec<u8>> {
    // Simple fallback: sky at top (class 2), ground at bottom (class 13=earth)
    (0..h as usize).map(|y| {
        let class = if y < (h as usize / 3) { 2u8 } else { 13u8 }; // sky vs earth
        vec![class; w as usize]
    }).collect()
}

// ============================================================================
// Normal map extraction (derived from depth)
// ============================================================================

/// Compute surface normals from depth map using central differences.
/// Returns (nx, ny) packed as i8 values (-127 to 127).
/// nz is implicit: nz = sqrt(1 - nx^2 - ny^2)
fn compute_normals(depth: &[Vec<f32>], scale: f32) -> (Vec<Vec<i8>>, Vec<Vec<i8>>) {
    let h = depth.len();
    let w = depth.get(0).map_or(0, |r| r.len());

    let mut nx = vec![vec![0i8; w]; h];
    let mut ny = vec![vec![0i8; w]; h];

    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            // Central differences for gradient
            let dzdx = (depth[y][x + 1] - depth[y][x - 1]) * scale;
            let dzdy = (depth[y + 1][x] - depth[y - 1][x]) * scale;

            // Normal = normalize(-dzdx, -dzdy, 1.0)
            let len = (dzdx * dzdx + dzdy * dzdy + 1.0).sqrt();
            let norm_x = -dzdx / len;
            let norm_y = -dzdy / len;

            // Pack to i8 range (-127 to 127)
            nx[y][x] = (norm_x * 127.0).clamp(-127.0, 127.0) as i8;
            ny[y][x] = (norm_y * 127.0).clamp(-127.0, 127.0) as i8;
        }
    }

    // Fill edges by copying neighbors
    for y in 0..h {
        nx[y][0] = nx[y][1.min(w - 1)];
        ny[y][0] = ny[y][1.min(w - 1)];
        nx[y][w - 1] = nx[y][w.saturating_sub(2)];
        ny[y][w - 1] = ny[y][w.saturating_sub(2)];
    }
    for x in 0..w {
        nx[0][x] = nx[1.min(h - 1)][x];
        ny[0][x] = ny[1.min(h - 1)][x];
        nx[h - 1][x] = nx[h.saturating_sub(2)][x];
        ny[h - 1][x] = ny[h.saturating_sub(2)][x];
    }

    (nx, ny)
}

// ============================================================================
// Flow field computation (depth gradient + gravity)
// ============================================================================

/// Compute flow direction using depth gradient with horizontal boost.
/// Only computes flow on ground surfaces (where ground mask is 1).
///
/// On tilted surfaces (like stairs), water flows towards the camera AND downward.
/// The depth gradient captures the "towards camera" direction.
/// We boost the horizontal component because typical scenes have diagonal slopes.
///
/// Key insight: depth increases towards the camera along the surface slope,
/// so the depth gradient direction approximates the downhill flow direction.
///
/// Returns (fx, fy) as i8 values (-127 to 127), representing flow direction.
/// Non-ground areas have (0, 0) to indicate no flow.
fn compute_flow_field(depth: &[Vec<f32>], ground: &[Vec<u8>]) -> (Vec<Vec<i8>>, Vec<Vec<i8>>) {
    let h = depth.len();
    let w = depth.get(0).map_or(0, |r| r.len());

    let mut fx = vec![vec![0i8; w]; h];
    let mut fy = vec![vec![0i8; w]; h];

    // Multi-scale depth gradient with horizontal boost
    // Larger scales capture the overall slope direction
    let scales: [(i32, f32); 3] = [
        (2, 0.25),   // Fine scale
        (5, 0.40),   // Medium scale
        (10, 0.35),  // Coarse scale (captures diagonal stairs better)
    ];

    // Horizontal boost factor: amplify horizontal depth changes
    // because typical scenes have diagonal slopes that MiDaS underestimates
    let horizontal_boost = 2.5_f32;

    let margin = 10;
    for y in margin..h.saturating_sub(margin) {
        for x in margin..w.saturating_sub(margin) {
            // Only compute flow on ground surfaces
            if ground[y][x] == 0 {
                fx[y][x] = 0;
                fy[y][x] = 0;
                continue;
            }

            let mut grad_x = 0.0f32;
            let mut grad_y = 0.0f32;

            // Accumulate depth gradients at multiple scales
            for &(offset, weight) in &scales {
                let o = offset as usize;

                // Central difference of depth
                // Positive gradient = depth increases in that direction = closer to camera
                let dx = depth[y][x + o] - depth[y][x - o];
                let dy = depth[y + o][x] - depth[y - o][x];

                // Water flows TOWARDS higher depth (closer to camera)
                // Boost horizontal component to capture diagonal slopes
                grad_x += dx * weight * horizontal_boost;
                grad_y += dy * weight;
            }

            // Add gravity component (water always tends to flow down)
            let gravity = 0.02_f32;
            grad_y += gravity;

            // Normalize and convert to flow direction
            let len = (grad_x * grad_x + grad_y * grad_y).sqrt();

            if len > 0.001 {
                let norm_x = grad_x / len;
                let norm_y = grad_y / len;

                // Flow strength based on gradient magnitude
                let strength = (len * 8.0 + 0.4).min(1.0);

                fx[y][x] = (norm_x * strength * 127.0).clamp(-127.0, 127.0) as i8;
                fy[y][x] = (norm_y * strength * 127.0).clamp(-127.0, 127.0) as i8;
            } else {
                // Flat ground area: pure gravity (straight down)
                fx[y][x] = 0;
                fy[y][x] = 51;
            }
        }
    }

    // Edges are non-ground
    for y in 0..h {
        for x in 0..w {
            if y < margin || y >= h - margin || x < margin || x >= w - margin {
                fx[y][x] = 0;
                fy[y][x] = 0;
            }
        }
    }

    (fx, fy)
}

// ============================================================================
// Ambient occlusion computation
// ============================================================================

/// Compute screen-space ambient occlusion from depth.
/// Areas surrounded by closer geometry are darker.
fn compute_ao(depth: &[Vec<f32>], radius: usize) -> Vec<Vec<u8>> {
    let h = depth.len();
    let w = depth.get(0).map_or(0, |r| r.len());
    let mut ao = vec![vec![255u8; w]; h];

    let r = radius as i32;

    for y in 0..h {
        for x in 0..w {
            let center_depth = depth[y][x];
            let mut occlusion = 0.0f32;
            let mut samples = 0;

            for dy in -r..=r {
                for dx in -r..=r {
                    if dx == 0 && dy == 0 {
                        continue;
                    }

                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                    let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                    let sample_depth = depth[sy][sx];

                    // If sample is closer (higher depth value), it occludes
                    if sample_depth > center_depth {
                        let diff = (sample_depth - center_depth).min(0.15);
                        // Weight by distance
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        occlusion += diff / (1.0 + dist * 0.5);
                    }
                    samples += 1;
                }
            }

            // Convert occlusion to AO value (255 = fully lit, 0 = fully occluded)
            let ao_factor = 1.0 - (occlusion / samples as f32 * 8.0).min(0.7);
            ao[y][x] = (ao_factor * 255.0) as u8;
        }
    }

    ao
}

// ============================================================================
// File generation
// ============================================================================

/// Scene geometry data for output
struct SceneGeometry {
    depth: Vec<Vec<u8>>,
    normal_x: Vec<Vec<i8>>,
    normal_y: Vec<Vec<i8>>,
    flow_x: Vec<Vec<i8>>,
    flow_y: Vec<Vec<i8>>,
    ao: Vec<Vec<u8>>,
    segments: Vec<Vec<u8>>,
    ground: Vec<Vec<u8>>,  // Ground mask: 1 = ground surface, 0 = non-ground
}

fn write_rust(path: &Path, palette: &[(u8,u8,u8)], pixels: &[Vec<u8>], geom: &SceneGeometry) {
    let (h, w) = (pixels.len(), pixels.get(0).map_or(0, |r| r.len()));
    let mut f = BufWriter::new(File::create(path).expect("create rust file"));

    writeln!(f, "// Auto-generated - do not edit").unwrap();
    writeln!(f, "#![allow(dead_code)]\n").unwrap();
    writeln!(f, "pub const BG_WIDTH: usize = {};", w).unwrap();
    writeln!(f, "pub const BG_HEIGHT: usize = {};", h).unwrap();
    writeln!(f, "pub const BG_PALETTE_SIZE: usize = {};\n", palette.len()).unwrap();

    writeln!(f, "pub static BG_PALETTE: [(u8,u8,u8); BG_PALETTE_SIZE] = [").unwrap();
    for (r, g, b) in palette { writeln!(f, "    ({},{},{}),", r, g, b).unwrap(); }
    writeln!(f, "];\n").unwrap();

    // Visual data
    write_array_u8(&mut f, "BG_PIXELS", pixels);

    // Geometry data
    write_array_u8(&mut f, "BG_DEPTH", &geom.depth);
    write_array_i8(&mut f, "BG_NORMAL_X", &geom.normal_x);
    write_array_i8(&mut f, "BG_NORMAL_Y", &geom.normal_y);
    write_array_i8(&mut f, "BG_FLOW_X", &geom.flow_x);
    write_array_i8(&mut f, "BG_FLOW_Y", &geom.flow_y);
    write_array_u8(&mut f, "BG_AO", &geom.ao);
    write_array_u8(&mut f, "BG_SEGMENTS", &geom.segments);
    write_array_u8(&mut f, "BG_GROUND", &geom.ground);

    println!("  Generated {}", path.display());
}

fn write_array_u8<W: Write>(f: &mut W, name: &str, data: &[Vec<u8>]) {
    let (h, w) = (data.len(), data.get(0).map_or(0, |r| r.len()));
    writeln!(f, "pub static {}: [[u8; {}]; {}] = [", name, w, h).unwrap();
    for row in data {
        write!(f, "    [").unwrap();
        for (i, v) in row.iter().enumerate() {
            if i > 0 { write!(f, ",").unwrap(); }
            write!(f, "{}", v).unwrap();
        }
        writeln!(f, "],").unwrap();
    }
    writeln!(f, "];\n").unwrap();
}

fn write_array_i8<W: Write>(f: &mut W, name: &str, data: &[Vec<i8>]) {
    let (h, w) = (data.len(), data.get(0).map_or(0, |r| r.len()));
    writeln!(f, "pub static {}: [[i8; {}]; {}] = [", name, w, h).unwrap();
    for row in data {
        write!(f, "    [").unwrap();
        for (i, v) in row.iter().enumerate() {
            if i > 0 { write!(f, ",").unwrap(); }
            write!(f, "{}", v).unwrap();
        }
        writeln!(f, "],").unwrap();
    }
    writeln!(f, "];\n").unwrap();
}

fn write_ts(path: &Path, palette: &[(u8,u8,u8)], pixels: &[Vec<u8>], geom: &SceneGeometry) {
    let (h, w) = (pixels.len(), pixels.get(0).map_or(0, |r| r.len()));
    let mut f = BufWriter::new(File::create(path).expect("create ts file"));

    writeln!(f, "// Auto-generated - do not edit\n").unwrap();
    writeln!(f, "export const BG_WIDTH = {};", w).unwrap();
    writeln!(f, "export const BG_HEIGHT = {};\n", h).unwrap();

    writeln!(f, "export const BG_PALETTE: string[] = [").unwrap();
    for (r, g, b) in palette { writeln!(f, "  '#{:02x}{:02x}{:02x}',", r, g, b).unwrap(); }
    writeln!(f, "];\n").unwrap();

    // Visual data
    write_ts_array_u8(&mut f, "BG_PIXELS", pixels);

    // Geometry data
    write_ts_array_u8(&mut f, "BG_DEPTH", &geom.depth);
    write_ts_array_i8(&mut f, "BG_NORMAL_X", &geom.normal_x);
    write_ts_array_i8(&mut f, "BG_NORMAL_Y", &geom.normal_y);
    write_ts_array_i8(&mut f, "BG_FLOW_X", &geom.flow_x);
    write_ts_array_i8(&mut f, "BG_FLOW_Y", &geom.flow_y);
    write_ts_array_u8(&mut f, "BG_AO", &geom.ao);
    write_ts_array_u8(&mut f, "BG_SEGMENTS", &geom.segments);
    write_ts_array_u8(&mut f, "BG_GROUND", &geom.ground);

    println!("  Generated {}", path.display());
}

fn write_ts_array_u8<W: Write>(f: &mut W, name: &str, data: &[Vec<u8>]) {
    writeln!(f, "export const {}: number[][] = [", name).unwrap();
    for row in data {
        write!(f, "  [").unwrap();
        for (i, v) in row.iter().enumerate() {
            if i > 0 { write!(f, ",").unwrap(); }
            write!(f, "{}", v).unwrap();
        }
        writeln!(f, "],").unwrap();
    }
    writeln!(f, "];\n").unwrap();
}

fn write_ts_array_i8<W: Write>(f: &mut W, name: &str, data: &[Vec<i8>]) {
    writeln!(f, "export const {}: number[][] = [", name).unwrap();
    for row in data {
        write!(f, "  [").unwrap();
        for (i, v) in row.iter().enumerate() {
            if i > 0 { write!(f, ",").unwrap(); }
            write!(f, "{}", v).unwrap();
        }
        writeln!(f, "],").unwrap();
    }
    writeln!(f, "];\n").unwrap();
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <image> [--cols N] [--rows N] [--colors N]", args[0]);
        std::process::exit(1);
    }

    let image_path = &args[1];
    let mut cols = 320u32;
    let mut rows = 180u32;
    let mut num_colors = 32usize;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--cols" => { cols = args.get(i+1).and_then(|s| s.parse().ok()).unwrap_or(320); i += 2; }
            "--rows" => { rows = args.get(i+1).and_then(|s| s.parse().ok()).unwrap_or(180); i += 2; }
            "--colors" => { num_colors = args.get(i+1).and_then(|s| s.parse().ok()).unwrap_or(32); i += 2; }
            _ => i += 1,
        }
    }

    println!("Converting {} ({}x{}, {} colors)...", image_path, cols, rows, num_colors);

    let img = image::open(image_path).expect("open image");
    let resized = img.resize_exact(cols, rows, FilterType::Lanczos3);

    // Extract pixels
    let mut pixels: Vec<Vec<Color>> = (0..rows).map(|y| {
        (0..cols).map(|x| {
            let p = resized.get_pixel(x, y);
            Color::from_rgb(p[0], p[1], p[2])
        }).collect()
    }).collect();

    let all_pixels: Vec<Color> = pixels.iter().flatten().copied().collect();

    // Palette + dither
    println!("  Extracting palette...");
    let palette = kmeans(&all_pixels, num_colors, 20);
    let rgb_palette: Vec<(u8,u8,u8)> = palette.iter().map(|c| c.to_rgb()).collect();

    println!("  Dithering...");
    let indexed = floyd_steinberg(&mut pixels, &palette);

    // Depth estimation
    println!("  Depth estimation...");
    let depth_f = estimate_depth(&resized, cols, rows);
    let depth_u8: Vec<Vec<u8>> = depth_f.iter()
        .map(|row| row.iter().map(|&d| (d * 255.0) as u8).collect())
        .collect();

    // Normal map (derived from depth) - used for splash direction
    println!("  Computing surface normals...");
    let normal_scale = 50.0; // Tunable: higher = more pronounced normals
    let (normal_x, normal_y) = compute_normals(&depth_f, normal_scale);

    // Ambient occlusion
    println!("  Computing ambient occlusion...");
    let ao = compute_ao(&depth_f, 3); // radius=3 pixels

    // Semantic segmentation (ML-based using SegFormer) - needed for ground mask
    println!("  Computing semantic segmentation...");
    let segments = estimate_segmentation(&resized, cols, rows);

    // Ground mask (from semantic segmentation) - surfaces where water flows
    println!("  Computing ground mask...");
    let ground = compute_ground_mask(&segments);

    // Flow field (elevation gradient) - only computed on ground surfaces
    println!("  Computing elevation gradient...");
    let (flow_x, flow_y) = compute_flow_field(&depth_f, &ground);

    // Bundle geometry
    let geom = SceneGeometry {
        depth: depth_u8,
        normal_x,
        normal_y,
        flow_x,
        flow_y,
        ao,
        segments,
        ground,
    };

    // Memory estimate
    let mem_kb = (cols * rows * 7) / 1024; // 7 bytes per pixel (depth + nx + ny + fx + fy + ao + pixels)
    println!("  Geometry size: ~{} KB", mem_kb);

    // Write output
    write_rust(Path::new("src/background.rs"), &rgb_palette, &indexed, &geom);
    write_ts(Path::new("../web/src/lib/background.ts"), &rgb_palette, &indexed, &geom);

    println!("Done!");
}
