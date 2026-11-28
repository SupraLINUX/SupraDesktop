//! Deterministic CPU-only geometry fixture. Not a desktop screenshot.
pub mod optics;
pub mod refraction_fixture;
pub mod material_fixture;

use supra_geometry::{Corner, RoundedRect};

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 360;

pub fn render() -> Vec<u8> {
    let shapes = [
        (32.0, 24.0, 264.0, 140.0, 48.0, Corner::Circular),
        (344.0, 24.0, 264.0, 140.0, 48.0, Corner::Continuous),
        (32.0, 204.0, 264.0, 112.0, 24.0, Corner::Circular),
        (344.0, 204.0, 264.0, 112.0, 24.0, Corner::Continuous),
    ];
    let shapes: Vec<_> = shapes
        .into_iter()
        .map(|(x, y, w, h, r, c)| (x, y, RoundedRect::new(w, h, r, c).unwrap()))
        .collect();
    let mut pixels = Vec::with_capacity(WIDTH * HEIGHT * 3);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let base = if (x / 16 + y / 16) % 2 == 0 {
                220_u8
            } else {
                200_u8
            };
            let mut rgb = [base; 3];
            for (ox, oy, shape) in &shapes {
                let a = shape.coverage(x as f64 - ox, y as f64 - oy, 1.0);
                for (v, foreground) in rgb.iter_mut().zip([52.0, 96.0, 160.0]) {
                    *v = (f64::from(*v) * (1.0 - a) + foreground * a).round() as u8;
                }
            }
            pixels.extend_from_slice(&rgb);
        }
    }
    pixels
}

pub fn ppm() -> Vec<u8> {
    let mut output = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    output.extend(render());
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_correct_size() {
        let a = render();
        assert_eq!(a.len(), WIDTH * HEIGHT * 3);
        assert_eq!(a, render());
    }

    #[test]
    fn fixture_has_expected_regions() {
        let pixels = render();
        let pixel = |x: usize, y: usize| &pixels[(y * WIDTH + x) * 3..(y * WIDTH + x) * 3 + 3];
        assert_eq!(pixel(0, 0), &[220, 220, 220]);
        assert_eq!(pixel(160, 80), &[52, 96, 160]);
        assert_eq!(pixel(470, 80), &[52, 96, 160]);
        assert_ne!(pixel(42, 34), pixel(354, 34));
    }
}

pub mod scene_fixture;

pub mod window_optics_fixture;

pub mod optics_benchmark;
