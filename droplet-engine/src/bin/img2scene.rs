// img2scene - Convert image to pixel art with MiDaS depth estimation
//
// Usage: cargo run --bin img2scene -- <image> [--cols 320] [--rows 180] [--colors 32]

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
// File generation
// ============================================================================

fn write_rust(path: &Path, palette: &[(u8,u8,u8)], pixels: &[Vec<u8>], depth: &[Vec<u8>]) {
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

    write_array(&mut f, "BG_PIXELS", pixels);
    write_array(&mut f, "BG_DEPTH", depth);

    println!("  Generated {}", path.display());
}

fn write_array<W: Write>(f: &mut W, name: &str, data: &[Vec<u8>]) {
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

fn write_ts(path: &Path, palette: &[(u8,u8,u8)], pixels: &[Vec<u8>], depth: &[Vec<u8>]) {
    let (h, w) = (pixels.len(), pixels.get(0).map_or(0, |r| r.len()));
    let mut f = BufWriter::new(File::create(path).expect("create ts file"));

    writeln!(f, "// Auto-generated - do not edit\n").unwrap();
    writeln!(f, "export const BG_WIDTH = {};", w).unwrap();
    writeln!(f, "export const BG_HEIGHT = {};\n", h).unwrap();

    writeln!(f, "export const BG_PALETTE: string[] = [").unwrap();
    for (r, g, b) in palette { writeln!(f, "  '#{:02x}{:02x}{:02x}',", r, g, b).unwrap(); }
    writeln!(f, "];\n").unwrap();

    write_ts_array(&mut f, "BG_PIXELS", pixels);
    write_ts_array(&mut f, "BG_DEPTH", depth);

    println!("  Generated {}", path.display());
}

fn write_ts_array<W: Write>(f: &mut W, name: &str, data: &[Vec<u8>]) {
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

    // Depth + collision
    println!("  Depth estimation...");
    let depth_f = estimate_depth(&resized, cols, rows);
    let depth_u8: Vec<Vec<u8>> = depth_f.iter()
        .map(|row| row.iter().map(|&d| (d * 255.0) as u8).collect())
        .collect();

    let _collision = generate_collision(&depth_f, cols as usize, rows as usize);

    // Write output
    write_rust(Path::new("src/background.rs"), &rgb_palette, &indexed, &depth_u8);
    write_ts(Path::new("../web/src/lib/background.ts"), &rgb_palette, &indexed, &depth_u8);

    println!("Done!");
}
