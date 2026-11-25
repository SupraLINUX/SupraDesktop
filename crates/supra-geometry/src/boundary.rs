//! Numerical boundary reference for the existing analytic membership models.
//! Not an exact SDF or calibrated macOS geometry. CPU laboratory only.
use super::{Corner, RoundedRect};

/// Negative inside, positive outside; magnitude is distance to the
/// tessellated boundary. Normal is outward, with deterministic tie-breaking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundarySample {
    pub signed_distance: f64,
    pub normal: [f64; 2],
}

const STEPS: usize = 128;

fn corner_point(radius: f64, corner: Corner, t: f64) -> [f64; 2] {
    match corner {
        Corner::Circular | Corner::Continuous => {
            let angle = t * std::f64::consts::FRAC_PI_2;
            let (s, c) = angle.sin_cos();
            let scale = if corner == Corner::Continuous {
                (s.powi(4) + c.powi(4)).sqrt().sqrt()
            } else {
                1.0
            };
            [radius * (1.0 - c / scale), radius * (1.0 - s / scale)]
        }
        Corner::BezierG2 => [radius * t.powi(3), radius * (1.0 - t).powi(3)],
    }
}

struct Nearest {
    distance: f64,
    normal: [f64; 2],
    delta: [f64; 2],
}

impl Nearest {
    fn segment(&mut self, p: [f64; 2], a: [f64; 2], b: [f64; 2]) {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let length = dx.hypot(dy);
        if length == 0.0 || !length.is_finite() {
            return;
        }
        let ux = dx / length;
        let uy = dy / length;
        let along = ((p[0] - a[0]) * ux + (p[1] - a[1]) * uy).clamp(0.0, length);
        let delta = [p[0] - (a[0] + along * ux), p[1] - (a[1] + along * uy)];
        let distance = delta[0].hypot(delta[1]);
        if distance < self.distance {
            *self = Self { distance, normal: [uy, -ux], delta };
        }
    }
}

impl RoundedRect {
    /// CPU reference, O(STEPS) per query. Coordinates are logical units.
    /// Sign uses analytic membership; magnitude uses a 128-segment corner.
    /// Near the approximation error band, use the segment outward normal.
    /// At medial-axis ties or sharp corners the normal is not unique.
    pub fn boundary(self, x: f64, y: f64) -> Option<BoundarySample> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let cx = self.width * 0.5;
        let cy = self.height * 0.5;
        let p = [cx - (x - cx).abs(), cy - (y - cy).abs()];
        let r = self.radius;
        let mut nearest = Nearest {
            distance: f64::INFINITY,
            normal: [0.0, -1.0],
            delta: [0.0, 0.0],
        };
        nearest.segment(p, [0.0, cy], [0.0, r]);
        if r > 0.0 {
            let mut a = [0.0, r];
            for i in 1..=STEPS {
                let b = if i == STEPS {
                    [r, 0.0]
                } else {
                    corner_point(r, self.corner, i as f64 / STEPS as f64)
                };
                nearest.segment(p, a, b);
                a = b;
            }
        }
        nearest.segment(p, [r, 0.0], [cx, 0.0]);
        if !nearest.distance.is_finite() {
            return None;
        }
        let inside = self.contains(x, y);
        let sign = if inside { -1.0 } else { 1.0 };
        // Only use a displacement normal if it agrees with the outward
        // orientation. Otherwise analytic and tessellated signs may differ.
        let dot = nearest.delta[0] * nearest.normal[0] + nearest.delta[1] * nearest.normal[1];
        let mut n = nearest.normal;
        if nearest.distance > 0.0 && dot * sign > 0.0 {
            n = [
                sign * nearest.delta[0] / nearest.distance,
                sign * nearest.delta[1] / nearest.distance,
            ];
        }
        if x > cx {
            n[0] = -n[0];
        }
        if y > cy {
            n[1] = -n[1];
        }
        Some(BoundarySample { signed_distance: sign * nearest.distance, normal: n })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_distances_and_normals() {
        let rect = RoundedRect::new(100.0, 60.0, 0.0, Corner::Circular).unwrap();
        let top = rect.boundary(50.0, -5.0).unwrap();
        assert_eq!(top.signed_distance, 5.0);
        assert_eq!(top.normal, [0.0, -1.0]);
        assert_eq!(rect.boundary(50.0, 10.0).unwrap().signed_distance, -10.0);
        let corner = rect.boundary(-3.0, -4.0).unwrap();
        assert!((corner.signed_distance - 5.0).abs() < 1e-12);
        assert!((corner.normal[0] + 0.6).abs() < 1e-12);
        assert!((corner.normal[1] + 0.8).abs() < 1e-12);
    }

    #[test]
    fn models_have_consistent_sign_and_unit_normals() {
        for model in [Corner::Circular, Corner::Continuous, Corner::BezierG2] {
            let rect = RoundedRect::new(100.0, 60.0, 24.0, model).unwrap();
            for y in -3..64 {
                for x in -3..104 {
                    let (x, y) = (f64::from(x) + 0.3, f64::from(y) + 0.2);
                    let a = rect.boundary(x, y).unwrap();
                    assert_eq!(a.signed_distance.is_sign_negative(), rect.contains(x, y));
                    assert!((a.normal[0].hypot(a.normal[1]) - 1.0).abs() < 1e-10);
                    let b = rect.boundary(100.0 - x, y).unwrap();
                    assert!((a.signed_distance - b.signed_distance).abs() < 1e-10);
                    assert!((a.normal[0] + b.normal[0]).abs() < 1e-8);
                    assert!((a.normal[1] - b.normal[1]).abs() < 1e-8);
                }
            }
        }
    }

    #[test]
    fn circular_reference_and_invalid_input() {
        let r = RoundedRect::new(100.0, 60.0, 20.0, Corner::Circular).unwrap();
        let s = r.boundary(0.0, 0.0).unwrap();
        assert!((s.signed_distance - (20.0_f64.hypot(20.0) - 20.0)).abs() < 0.001);
        assert!(r.boundary(f64::NAN, 0.0).is_none());
    }
}
