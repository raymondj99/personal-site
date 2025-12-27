// ai.rs - AI model inference (MiDaS, SegFormer)
//
// Runs ONNX models for depth estimation and semantic segmentation.

use image::{DynamicImage, GenericImageView, imageops::FilterType};
use ndarray::Array4;
use ort::session::Session;
use ort::value::Value;
use std::env;
use std::path::Path;

// ImageNet normalization constants
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

/// Estimate depth using MiDaS model
/// Returns depth map normalized to [0, 1] where 0=far, 1=near
pub fn estimate_depth(img: &DynamicImage, tw: u32, th: u32) -> Vec<Vec<f32>> {
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

    println!("    Running MiDaS...");
    let resized = img.resize_exact(MIDAS_SIZE, MIDAS_SIZE, FilterType::Lanczos3);

    // Prepare input tensor
    let mut input = Array4::<f32>::zeros((1, 3, MIDAS_SIZE as usize, MIDAS_SIZE as usize));
    for y in 0..MIDAS_SIZE {
        for x in 0..MIDAS_SIZE {
            let p = resized.get_pixel(x, y);
            for c in 0..3 {
                input[[0, c, y as usize, x as usize]] = (p[c] as f32 / 255.0 - MEAN[c]) / STD[c];
            }
        }
    }

    // Run inference
    let Ok(input_val) = Value::from_array(input) else { return fallback_depth(tw, th); };
    let input_name = session.inputs.first().map(|i| i.name.clone()).unwrap_or_else(|| "image".into());
    let Ok(outputs) = session.run(ort::inputs![input_name => input_val]) else { return fallback_depth(tw, th); };
    let Ok(arr) = outputs[0].try_extract_array::<f32>() else { return fallback_depth(tw, th); };

    // Extract output dimensions
    let shape = arr.shape();
    let (oh, ow) = match shape.len() {
        4 => (shape[2], shape[3]),
        3 => (shape[1], shape[2]),
        2 => (shape[0], shape[1]),
        _ => return fallback_depth(tw, th),
    };

    // Normalize depth values
    let flat: Vec<f32> = arr.iter().copied().collect();
    let (min_d, max_d) = flat.iter().fold((f32::MAX, f32::MIN), |(mn, mx), &v| (mn.min(v), mx.max(v)));
    let range = (max_d - min_d).max(1e-6);

    // Bilinear resize to target
    bilinear_resize(&flat, ow, oh, tw as usize, th as usize, min_d, range)
}

/// Estimate semantic segmentation using SegFormer
/// Returns class indices (ADE20K: 150 classes)
pub fn estimate_segmentation(img: &DynamicImage, tw: u32, th: u32) -> Vec<Vec<u8>> {
    const SEGFORMER_SIZE: u32 = 512;
    let model_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("models/segformer_b0_ade20k.onnx");

    if !model_path.exists() {
        eprintln!("    SegFormer model not found, using fallback");
        return fallback_segmentation(tw, th);
    }

    let Ok(builder) = Session::builder() else {
        return fallback_segmentation(tw, th);
    };
    let Ok(mut session) = builder.commit_from_file(&model_path) else {
        return fallback_segmentation(tw, th);
    };

    println!("    Running SegFormer...");
    let resized = img.resize_exact(SEGFORMER_SIZE, SEGFORMER_SIZE, FilterType::Lanczos3);

    // Prepare input tensor
    let mut input = Array4::<f32>::zeros((1, 3, SEGFORMER_SIZE as usize, SEGFORMER_SIZE as usize));
    for y in 0..SEGFORMER_SIZE {
        for x in 0..SEGFORMER_SIZE {
            let p = resized.get_pixel(x, y);
            for c in 0..3 {
                input[[0, c, y as usize, x as usize]] = (p[c] as f32 / 255.0 - MEAN[c]) / STD[c];
            }
        }
    }

    // Run inference
    let Ok(input_val) = Value::from_array(input) else { return fallback_segmentation(tw, th); };
    let input_name = session.inputs.first().map(|i| i.name.clone()).unwrap_or_else(|| "pixel_values".into());
    let Ok(outputs) = session.run(ort::inputs![input_name => input_val]) else { return fallback_segmentation(tw, th); };
    let Ok(arr) = outputs[0].try_extract_array::<f32>() else { return fallback_segmentation(tw, th); };

    // Extract output (1, num_classes, H/4, W/4)
    let shape = arr.shape();
    let (num_classes, oh, ow) = match shape.len() {
        4 => (shape[1], shape[2], shape[3]),
        3 => (shape[0], shape[1], shape[2]),
        _ => return fallback_segmentation(tw, th),
    };

    // Argmax over classes
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

    // Nearest-neighbor resize to target
    let (sx, sy) = (ow as f32 / tw as f32, oh as f32 / th as f32);
    let mut segments = vec![vec![0u8; tw as usize]; th as usize];

    for y in 0..th as usize {
        for x in 0..tw as usize {
            let src_x = ((x as f32 + 0.5) * sx) as usize;
            let src_y = ((y as f32 + 0.5) * sy) as usize;
            segments[y][x] = seg_map[src_y.min(oh - 1)][src_x.min(ow - 1)];
        }
    }

    // Log unique classes
    let mut class_counts = [0u32; 150];
    for row in &segments {
        for &c in row {
            if (c as usize) < 150 {
                class_counts[c as usize] += 1;
            }
        }
    }
    let unique: usize = class_counts.iter().filter(|&&c| c > 0).count();
    println!("    Found {} unique semantic classes", unique);

    segments
}

// Fallback functions when models aren't available

fn fallback_depth(w: u32, h: u32) -> Vec<Vec<f32>> {
    (0..h as usize)
        .map(|y| vec![y as f32 / h as f32; w as usize])
        .collect()
}

fn fallback_segmentation(w: u32, h: u32) -> Vec<Vec<u8>> {
    (0..h as usize)
        .map(|y| {
            let class = if y < (h as usize / 3) { 2u8 } else { 13u8 }; // sky vs earth
            vec![class; w as usize]
        })
        .collect()
}

fn bilinear_resize(
    src: &[f32],
    sw: usize,
    sh: usize,
    tw: usize,
    th: usize,
    min_d: f32,
    range: f32,
) -> Vec<Vec<f32>> {
    let (sx, sy) = (sw as f32 / tw as f32, sh as f32 / th as f32);
    let mut depth = vec![vec![0.0f32; tw]; th];

    for y in 0..th {
        for x in 0..tw {
            let (fx, fy) = (x as f32 * sx, y as f32 * sy);
            let (x0, y0) = (fx as usize, fy as usize);
            let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
            let (tx, ty) = (fx.fract(), fy.fract());

            let sample = |sx: usize, sy: usize| {
                let v = src.get(sy * sw + sx).copied().unwrap_or(0.0);
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
