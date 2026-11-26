//! Synthetic material comparison: identity left, optical composite right.
use crate::{WIDTH, HEIGHT};
use supra_geometry::{Corner, RoundedRect};
use supra_glass::{blur::gaussian, material::Material};

fn encode(x: f64) -> u8 {
    let v = if x <= 0.0031308 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn sample(plane: &[f64], w: usize, h: usize, x: f64, y: f64) -> f64 {
    let x = x.clamp(0.0, (w - 1) as f64);
    let y = y.clamp(0.0, (h - 1) as f64);
    let (ix, iy) = (x.floor() as usize, y.floor() as usize);
    let (jx, jy) = ((ix + 1).min(w - 1), (iy + 1).min(h - 1));
    let (fx, fy) = (x.fract(), y.fract());
    let top = plane[iy * w + ix] * (1.0 - fx) + plane[iy * w + jx] * fx;
    let bottom = plane[jy * w + ix] * (1.0 - fx) + plane[jy * w + jx] * fx;
    (top * (1.0 - fy) + bottom * fy).clamp(0.0, 1.0)
}

pub fn ppm() -> Vec<u8> {
    let (w, h) = (WIDTH / 2, HEIGHT);
    let planes: [Vec<f64>; 3] = std::array::from_fn(|c| (0..w*h).map(|i| {
        let (x, y) = (i % w, i / w);
        if (x + c * 7) % 24 < 3 || y % 24 < 3 { 0.08 }
        else { [0.72, 0.50, 0.25][(x / 80 + y / 90 + c) % 3] }
    }).collect());
    let blurred: [Vec<f64>; 3] = std::array::from_fn(|c| gaussian(&planes[c], w, h, 2.0).unwrap());
    let shape = RoundedRect::new(224.0, 144.0, 44.0, Corner::BezierG2).unwrap();
    let m = Material::new(24.0, 18.0, 1.5, [1.44, 1.50, 1.56], [0.8; 3], 0.08).unwrap();
    let mut out = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    for y in 0..h {
        for x in 0..WIDTH {
            let px = x % w;
            let original = std::array::from_fn(|c| planes[c][y*w+px]);
            let (lx, ly) = (px as f64 - 48.0, y as f64 - 108.0);
            let coverage = if x >= w { shape.coverage(lx, ly, 1.0) } else { 0.0 };
            let rgb = if coverage > 0.0 {
                let b = shape.boundary(lx + 0.5, ly + 0.5).unwrap();
                m.compose(b.signed_distance, b.normal, coverage, original, [0.9; 3],
                    |offset| std::array::from_fn(|c| sample(&blurred[c], w, h,
                        px as f64 + offset[0], y as f64 + offset[1]))).unwrap()
            } else { original };
            out.extend(rgb.map(encode));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outside_identical_inside_changed() {
        let bytes = ppm();
        let header = format!("P6\n{WIDTH} {HEIGHT}\n255\n").len();
        let pixel = |x: usize, y: usize| &bytes[header + (y*WIDTH+x)*3..header + (y*WIDTH+x)*3+3];
        assert_eq!(pixel(0, 0), pixel(WIDTH/2, 0));
        assert!((100..200).any(|x| pixel(x, 180) != pixel(x+WIDTH/2, 180)));
    }
}
