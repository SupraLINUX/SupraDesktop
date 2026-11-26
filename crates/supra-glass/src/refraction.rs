//! Experimental single-interface refraction reference, not a macOS material.
//! Distances are in caller-selected consistent units.
#[derive(Clone, Copy, Debug)]
pub struct Refraction {
    band: f64,
    depth: f64,
    slope: f64,
    index: f64,
}

impl Refraction {
    /// Bounded experimental parameters. Index is medium / incident index.
    pub fn new(band: f64, depth: f64, slope: f64, index: f64) -> Option<Self> {
        if !band.is_finite() || !(0.0..=4096.0).contains(&band) || band == 0.0
            || !depth.is_finite() || !(0.0..=4096.0).contains(&depth)
            || !slope.is_finite() || !(0.0..=4.0).contains(&slope)
            || !index.is_finite() || !(1.0..=3.0).contains(&index)
        {
            return None;
        }
        Some(Self { band, depth, slope, index })
    }

    /// Cosine of the incident angle for the same synthetic surface profile.
    pub fn incidence_cosine(self, signed_distance: f64) -> Option<f64> {
        if !signed_distance.is_finite() {
            return None;
        }
        let slope = if signed_distance < 0.0 && signed_distance > -self.band {
            let u = -signed_distance / self.band;
            self.slope * 4.0 * u * (1.0 - u)
        } else {
            0.0
        };
        Some(1.0 / slope.hypot(1.0))
    }

    /// Backdrop sampling offset for a ray along -Z through a local surface.
    /// Distance is negative inside; normal points outward in the image plane.
    /// The synthetic slope profile vanishes at the rim and inner band edge.
    /// Snell's law is applied once; no second interface, dispersion or Fresnel.
    pub fn displacement(self, signed_distance: f64, normal: [f64; 2]) -> Option<[f64; 2]> {
        let length = normal[0].hypot(normal[1]);
        if !signed_distance.is_finite() || !length.is_finite()
            || (length - 1.0).abs() > 1e-8
        {
            return None;
        }
        if signed_distance >= 0.0 || signed_distance <= -self.band
            || self.depth == 0.0 || self.slope == 0.0 || self.index == 1.0
        {
            return Some([0.0, 0.0]);
        }
        let u = -signed_distance / self.band;
        let slope = self.slope * 4.0 * u * (1.0 - u);
        let nz = 1.0 / slope.hypot(1.0);
        let eta = 1.0 / self.index;
        let cos_t = (1.0 - eta * eta * (1.0 - nz * nz)).sqrt();
        let coefficient = eta * nz - cos_t;
        let tz = -eta + coefficient * nz;
        let shift = self.depth * coefficient * slope * nz / -tz;
        Some([shift * normal[0] / length, shift * normal[1] / length])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_effect_outside_or_without_interface() {
        for (depth, slope, index) in [(0.0, 1.0, 1.5), (10.0, 0.0, 1.5), (10.0, 1.0, 1.0)] {
            let r = Refraction::new(12.0, depth, slope, index).unwrap();
            assert_eq!(r.displacement(-6.0, [1.0, 0.0]), Some([0.0, 0.0]));
        }
        let r = Refraction::new(12.0, 10.0, 1.0, 1.5).unwrap();
        for d in [1.0, 0.0, -12.0, -20.0] {
            assert_eq!(r.displacement(d, [1.0, 0.0]), Some([0.0, 0.0]));
        }
    }

    #[test]
    fn agrees_with_scalar_snell_angle() {
        let r = Refraction::new(12.0, 10.0, 1.0, 1.5).unwrap();
        let angle = 1.0_f64.atan();
        let transmitted = (angle.sin() / 1.5).asin();
        let expected = -10.0 * (angle - transmitted).tan();
        let offset = r.displacement(-6.0, [1.0, 0.0]).unwrap();
        assert!((offset[0] - expected).abs() < 1e-12);
        assert_eq!(offset[1], 0.0);
    }

    #[test]
    fn symmetric_continuous_and_scales_with_depth() {
        let r = Refraction::new(12.0, 10.0, 1.0, 1.5).unwrap();
        let twice = Refraction::new(24.0, 20.0, 1.0, 1.5).unwrap();
        for i in 0..=100 {
            let d = -12.0 * f64::from(i) / 100.0;
            let a = r.displacement(d, [1.0, 0.0]).unwrap()[0];
            let b = r.displacement(d, [-1.0, 0.0]).unwrap()[0];
            assert!((a + b).abs() < 1e-12);
            assert!(a.is_finite() && a <= 0.0);
            assert!((2.0 * a - twice.displacement(2.0 * d, [1.0, 0.0]).unwrap()[0]).abs() < 1e-12);
        }
        for d in [-1e-8, -12.0 + 1e-8] {
            assert!(r.displacement(d, [1.0, 0.0]).unwrap()[0].abs() < 1e-6);
        }
    }

    #[test]
    fn invalid_values_rejected() {
        assert!(Refraction::new(0.0, 1.0, 1.0, 1.5).is_none());
        assert!(Refraction::new(1.0, f64::NAN, 1.0, 1.5).is_none());
        assert!(Refraction::new(1.0, 1.0, 1.0, 0.9).is_none());
        let r = Refraction::new(12.0, 10.0, 1.0, 1.5).unwrap();
        assert!(r.displacement(-1.0, [0.0, 0.0]).is_none());
        assert!(r.displacement(f64::NAN, [1.0, 0.0]).is_none());
    }
}
