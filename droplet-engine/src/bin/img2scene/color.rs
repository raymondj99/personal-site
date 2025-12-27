// color.rs - Color operations, palette extraction, dithering
//
// Uses k-means clustering for palette extraction and
// Floyd-Steinberg dithering for indexed color conversion.

use image::{DynamicImage, GenericImageView};

#[derive(Clone, Copy, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r: r as f32, g: g as f32, b: b as f32 }
    }

    pub fn to_rgb(self) -> (u8, u8, u8) {
        (
            self.r.clamp(0.0, 255.0) as u8,
            self.g.clamp(0.0, 255.0) as u8,
            self.b.clamp(0.0, 255.0) as u8,
        )
    }

    pub fn dist_sq(self, other: Color) -> f32 {
        let dr = self.r - other.r;
        let dg = self.g - other.g;
        let db = self.b - other.b;
        dr * dr + dg * dg + db * db
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

/// Extract pixels from image as Color grid
pub fn extract_pixels(img: &DynamicImage, cols: u32, rows: u32) -> Vec<Vec<Color>> {
    (0..rows)
        .map(|y| {
            (0..cols)
                .map(|x| {
                    let p = img.get_pixel(x, y);
                    Color::from_rgb(p[0], p[1], p[2])
                })
                .collect()
        })
        .collect()
}

/// Find nearest palette color
fn nearest(c: Color, palette: &[Color]) -> usize {
    palette
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| c.dist_sq(**a).partial_cmp(&c.dist_sq(**b)).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// K-means clustering with k-means++ initialization
pub fn kmeans(pixels: &[Color], k: usize, iters: usize) -> Vec<Color> {
    if pixels.is_empty() || k == 0 {
        return vec![];
    }

    // K-means++ initialization
    let mut centroids = Vec::with_capacity(k);
    centroids.push(pixels[pixels.len() / 2]);

    for _ in 1..k {
        let (mut best_dist, mut best_idx) = (0.0f32, 0);
        for (i, p) in pixels.iter().enumerate() {
            let d = centroids.iter().map(|c| p.dist_sq(*c)).fold(f32::MAX, f32::min);
            if d > best_dist {
                best_dist = d;
                best_idx = i;
            }
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

/// Floyd-Steinberg dithering
pub fn floyd_steinberg(pixels: &mut [Vec<Color>], palette: &[Color]) -> Vec<Vec<u8>> {
    let h = pixels.len();
    let w = pixels.get(0).map_or(0, |r| r.len());
    let mut result = vec![vec![0u8; w]; h];

    for y in 0..h {
        for x in 0..w {
            let old = pixels[y][x];
            let idx = nearest(old, palette);
            let new = palette[idx];
            result[y][x] = idx as u8;

            let err = Color {
                r: old.r - new.r,
                g: old.g - new.g,
                b: old.b - new.b,
            };

            if x + 1 < w {
                pixels[y][x + 1] = pixels[y][x + 1] + err * (7.0 / 16.0);
            }
            if y + 1 < h {
                if x > 0 {
                    pixels[y + 1][x - 1] = pixels[y + 1][x - 1] + err * (3.0 / 16.0);
                }
                pixels[y + 1][x] = pixels[y + 1][x] + err * (5.0 / 16.0);
                if x + 1 < w {
                    pixels[y + 1][x + 1] = pixels[y + 1][x + 1] + err * (1.0 / 16.0);
                }
            }
        }
    }

    result
}
