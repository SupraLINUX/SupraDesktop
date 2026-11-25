//! Experimental canonical corner coverage in logical coordinates.
//! These are mathematical candidates, NOT calibrated macOS geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    Circular,
    Continuous,
}

#[derive(Clone, Copy, Debug)]
pub struct RoundedRect {
    width: f64,
    height: f64,
    radius: f64,
    corner: Corner,
}

impl RoundedRect {
    pub fn new(width: f64, height: f64, radius: f64, corner: Corner) -> Option<Self> {
        if !width.is_finite()
            || !height.is_finite()
            || !radius.is_finite()
            || width <= 0.0
            || height <= 0.0
            || radius < 0.0
        {
            return None;
        }
        Some(Self {
            width,
            height,
            radius: radius.min(width.min(height) * 0.5),
            corner,
        })
    }

    pub fn radius(self) -> f64 {
        self.radius
    }

    /// Membership, not a signed distance. Local corner superellipse with n=4.
    pub fn contains(self, x: f64, y: f64) -> bool {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x > self.width
            || y > self.height
        {
            return false;
        }
        if self.radius == 0.0 {
            return true;
        }
        let dx = ((x - self.width * 0.5).abs() - (self.width * 0.5 - self.radius))
            .max(0.0)
            / self.radius;
        let dy = ((y - self.height * 0.5).abs() - (self.height * 0.5 - self.radius))
            .max(0.0)
            / self.radius;
        match self.corner {
            Corner::Circular => dx * dx + dy * dy <= 1.0,
            Corner::Continuous => dx.powi(4) + dy.powi(4) <= 1.0,
        }
    }

    /// Reference box-filter coverage, in logical units. Fixed 4x4 sampling.
    pub fn coverage(self, x: f64, y: f64, pixel_size: f64) -> f64 {
        if !pixel_size.is_finite() || pixel_size <= 0.0 {
            return 0.0;
        }
        let mut hits = 0;
        for sy in 0..4 {
            for sx in 0..4 {
                let px = x + (f64::from(sx) + 0.5) * pixel_size / 4.0;
                let py = y + (f64::from(sy) + 0.5) * pixel_size / 4.0;
                hits += u32::from(self.contains(px, py));
            }
        }
        f64::from(hits) / 16.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_input_and_clamps_radius() {
        assert!(RoundedRect::new(f64::NAN, 20.0, 5.0, Corner::Circular).is_none());
        assert!(RoundedRect::new(0.0, 20.0, 5.0, Corner::Circular).is_none());
        assert!(RoundedRect::new(20.0, 20.0, -1.0, Corner::Circular).is_none());
        assert_eq!(
            RoundedRect::new(10.0, 20.0, 100.0, Corner::Continuous)
                .unwrap()
                .radius(),
            5.0
        );
    }

    #[test]
    fn symmetry_and_bounds() {
        for corner in [Corner::Circular, Corner::Continuous] {
            let r = RoundedRect::new(120.0, 60.0, 18.0, corner).unwrap();
            for y in 0..60 {
                for x in 0..120 {
                    let x = f64::from(x) + 0.5;
                    let y = f64::from(y) + 0.5;
                    assert_eq!(r.contains(x, y), r.contains(120.0 - x, y));
                    assert_eq!(r.contains(x, y), r.contains(x, 60.0 - y));
                }
            }
            assert!(!r.contains(-1.0, 30.0));
            assert!(!r.contains(0.0, 0.0));
            assert!(r.contains(60.0, 0.0));
            assert!(r.contains(60.0, 30.0));
        }
    }

    #[test]
    fn continuous_candidate_differs_from_circle() {
        let c = RoundedRect::new(100.0, 100.0, 20.0, Corner::Circular).unwrap();
        let s = RoundedRect::new(100.0, 100.0, 20.0, Corner::Continuous).unwrap();
        assert!(!c.contains(4.0, 4.0));
        assert!(s.contains(4.0, 4.0));
        assert_eq!(s.coverage(50.0, 50.0, 1.0), 1.0);
        assert_eq!(s.coverage(-10.0, -10.0, 1.0), 0.0);
        assert_eq!(s.coverage(50.0, 50.0, f64::NAN), 0.0);
    }

    #[test]
    fn coverage_is_bounded_and_symmetric() {
        let s = RoundedRect::new(80.0, 40.0, 12.0, Corner::Continuous).unwrap();
        for y in 0..40 {
            for x in 0..80 {
                let a = s.coverage(f64::from(x), f64::from(y), 1.0);
                let b = s.coverage(f64::from(79 - x), f64::from(y), 1.0);
                assert!((0.0..=1.0).contains(&a));
                assert_eq!(a, b);
            }
        }
    }
}
