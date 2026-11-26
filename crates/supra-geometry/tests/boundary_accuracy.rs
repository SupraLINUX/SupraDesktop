//! Independent analytic checks for the experimental CPU boundary reference.
use supra_geometry::{Corner, RoundedRect};

fn circular_sdf(w: f64, h: f64, r: f64, x: f64, y: f64) -> f64 {
    let qx = (x - w * 0.5).abs() - (w * 0.5 - r);
    let qy = (y - h * 0.5).abs() - (h * 0.5 - r);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

#[test]
fn circular_distance_matches_analytic_oracle() {
    for (w, h, r) in [(100.0, 60.0, 24.0), (12.0, 12.0, 6.0), (240.0, 16.0, 8.0), (40.0, 30.0, 0.0)] {
        let rect = RoundedRect::new(w, h, r, Corner::Circular).unwrap();
        // Maximum chord sagitta for a quarter circle divided into 128 pieces.
        let tolerance = r * (1.0 - (std::f64::consts::PI / 512.0).cos()) + 1e-9;
        for j in -4..=44 {
            for i in -4..=44 {
                let x = w * (f64::from(i) + 0.17) / 40.0;
                let y = h * (f64::from(j) + 0.31) / 40.0;
                let observed = rect.boundary(x, y).unwrap().signed_distance;
                let expected = circular_sdf(w, h, r, x, y);
                // Analytic sign is separate from distance magnitude near a chord.
                assert!((observed - expected).abs() <= tolerance,
                    "w={w} h={h} r={r} x={x} y={y}: {observed} vs {expected}");
            }
        }
    }
}

#[test]
fn sampled_implicit_contours_remain_close_to_reference() {
    for r in [1.0, 12.0, 80.0] {
        for model in [Corner::Continuous, Corner::BezierG2] {
            let rect = RoundedRect::new(4.0 * r, 3.0 * r, r, model).unwrap();
            // Independently solve the implicit curve for y at fixed x,
            // rather than reuse the implementation's tessellation parameters.
            for i in 1..1000 {
                let u = f64::from(i) / 1000.0;
                let v = match model {
                    Corner::Continuous => 1.0 - (1.0 - (1.0 - u).powi(4)).powf(0.25),
                    Corner::BezierG2 => (1.0 - u.cbrt()).powi(3),
                    Corner::Circular => unreachable!(),
                };
                let error = rect.boundary(r * u, r * v).unwrap().signed_distance.abs();
                assert!(error <= r * 0.0004 + 1e-9,
                    "{model:?} radius={r} u={u}: {error}");
            }
        }
    }
}

#[test]
fn uniform_scaling_preserves_distance_and_normal() {
    for model in [Corner::Circular, Corner::Continuous, Corner::BezierG2] {
        let rect = RoundedRect::new(100.0, 60.0, 24.0, model).unwrap();
        // Avoid medial-axis ties, where outward normals need not be unique.
        for (x, y) in [(-3.0, -4.0), (4.2, 7.3), (35.0, -2.0), (91.3, 48.2)] {
            let a = rect.boundary(x, y).unwrap();
            for scale in [0.5, 1.25, 1.5, 2.0, 4.0] {
                let b = RoundedRect::new(100.0 * scale, 60.0 * scale, 24.0 * scale, model)
                    .unwrap().boundary(x * scale, y * scale).unwrap();
                assert!((a.signed_distance * scale - b.signed_distance).abs() < 1e-8);
                assert!((a.normal[0] - b.normal[0]).abs() < 1e-8);
                assert!((a.normal[1] - b.normal[1]).abs() < 1e-8);
            }
        }
    }
}
