//! Original synthetic grid; left unshifted, right refracted. CPU only.
use supra_geometry::{Corner, RoundedRect};
use supra_glass::refraction::Refraction;
use crate::{WIDTH, HEIGHT};

fn grid(x: f64, y: f64) -> u8 {
    let line = |v: f64| {
        let p = v.rem_euclid(16.0);
        p.min(16.0 - p).clamp(0.0, 1.0)
    };
    (40.0 + 180.0 * line(x).min(line(y))).round() as u8
}

pub fn ppm() -> Vec<u8> {
    let shape = RoundedRect::new(224.0, 144.0, 44.0, Corner::BezierG2).unwrap();
    let optic = Refraction::new(24.0, 18.0, 1.5, 1.5).unwrap();
    let mut out = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let panel = x / (WIDTH / 2);
            let px = (x % (WIDTH / 2)) as f64 + 0.5;
            let py = y as f64 + 0.5;
            let lx = px - 48.0;
            let ly = py - 108.0;
            let offset = if panel == 1 && shape.contains(lx, ly) {
                let sample = shape.boundary(lx, ly).unwrap();
                optic.displacement(sample.signed_distance, sample.normal).unwrap()
            } else {
                [0.0, 0.0]
            };
            let v = grid(px + offset[0], py + offset[1]);
            out.extend_from_slice(&[v; 3]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_fixture_has_local_distortion() {
        let data = ppm();
        let header = format!("P6\n{WIDTH} {HEIGHT}\n255\n").len();
        assert_eq!(data.len(), header + WIDTH * HEIGHT * 3);
        let pixel = |x: usize, y: usize| data[header + (y * WIDTH + x) * 3];
        assert_eq!(pixel(0, 0), pixel(WIDTH / 2, 0));
        assert_eq!(pixel(160, 180), pixel(160 + WIDTH / 2, 180));
        assert!((108..252).any(|y| (48..272).any(|x| pixel(x, y) != pixel(x + WIDTH / 2, y))));
    }
}
