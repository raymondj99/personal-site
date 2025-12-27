// img2scene - Extract scene geometry from images using AI
//
// Pipeline:
//   1. Load image, resize to target resolution
//   2. Extract color palette, dither to indexed
//   3. Run MiDaS for depth estimation
//   4. Run SegFormer for semantic segmentation
//   5. Compute derived maps (normals, flow, AO, ground)
//   6. Export to Rust + TypeScript
//
// Usage: cargo run --bin img2scene -- <image> [--cols N] [--rows N] [--colors N]

mod color;
mod ai;
mod geometry;
mod export;

use image::imageops::FilterType;
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <image> [--cols N] [--rows N] [--colors N]", args[0]);
        std::process::exit(1);
    }

    // Parse arguments
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

    println!("Processing {} ({}x{}, {} colors)...", image_path, cols, rows, num_colors);

    // Load and resize image
    let img = image::open(image_path).expect("Failed to open image");
    let resized = img.resize_exact(cols, rows, FilterType::Lanczos3);

    // Extract pixels as Color
    let mut pixels = color::extract_pixels(&resized, cols, rows);
    let all_pixels: Vec<color::Color> = pixels.iter().flatten().copied().collect();

    // Palette extraction + dithering
    println!("  Extracting palette...");
    let palette = color::kmeans(&all_pixels, num_colors, 20);
    let rgb_palette: Vec<(u8, u8, u8)> = palette.iter().map(|c| c.to_rgb()).collect();

    println!("  Dithering...");
    let indexed = color::floyd_steinberg(&mut pixels, &palette);

    // AI: Depth estimation
    println!("  Running depth estimation...");
    let depth_f = ai::estimate_depth(&resized, cols, rows);
    let depth_u8: Vec<Vec<u8>> = depth_f.iter()
        .map(|row| row.iter().map(|&d| (d * 255.0) as u8).collect())
        .collect();

    // AI: Semantic segmentation
    println!("  Running semantic segmentation...");
    let segments = ai::estimate_segmentation(&resized, cols, rows);

    // Derived: Ground mask
    println!("  Computing ground mask...");
    let ground = geometry::compute_ground_mask(&segments);

    // Derived: Surface normals
    println!("  Computing surface normals...");
    let (normal_x, normal_y) = geometry::compute_normals(&depth_f, 50.0);

    // Derived: Flow field
    println!("  Computing flow field...");
    let (flow_x, flow_y) = geometry::compute_flow_field(&depth_f, &ground);

    // Derived: Ambient occlusion
    println!("  Computing ambient occlusion...");
    let ao = geometry::compute_ao(&depth_f, 3);

    // Bundle geometry
    let geom = export::SceneGeometry {
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
    let mem_kb = (cols * rows * 7) / 1024;
    println!("  Geometry size: ~{} KB", mem_kb);

    // Export
    export::write_rust(Path::new("src/scene/data.rs"), &rgb_palette, &indexed, &geom);
    export::write_ts(Path::new("../web/src/lib/scene/data.ts"), &rgb_palette, &indexed, &geom);

    println!("Done!");
}
