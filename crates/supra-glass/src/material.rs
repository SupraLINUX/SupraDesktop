//! Experimental linear RGB composition. Independent of geometry and renderer.
use crate::refraction::Refraction;

/// Unpolarized Fresnel reflectance at a single dielectric interface.
/// Index is transmitted / incident medium, with index >= 1.
pub fn fresnel(cosine: f64, index: f64) -> Option<f64> {
    if !cosine.is_finite() || !(0.0..=1.0).contains(&cosine)
        || !index.is_finite() || !(1.0..=3.0).contains(&index)
    {
        return None;
    }
    if index == 1.0 {
        return Some(0.0);
    }
    let transmitted = (1.0 - (1.0 - cosine * cosine) / (index * index)).sqrt();
    let rs = (cosine - index * transmitted) / (cosine + index * transmitted);
    let rp = (index * cosine - transmitted) / (index * cosine + transmitted);
    Some((0.5 * (rs * rs + rp * rp)).clamp(0.0, 1.0))
}

fn valid_rgb(rgb: [f64; 3]) -> bool {
    rgb.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    optics: [Refraction; 3],
    indices: [f64; 3],
    tint: [f64; 3],
    tint_weight: f64,
}

impl Material {
    /// Indices are independent R/G/B laboratory parameters, not spectral data.
    pub fn new(band: f64, depth: f64, slope: f64, indices: [f64; 3],
               tint: [f64; 3], tint_weight: f64) -> Option<Self> {
        if !valid_rgb(tint) || !tint_weight.is_finite()
            || !(0.0..=1.0).contains(&tint_weight)
        {
            return None;
        }
        let optics = [Refraction::new(band, depth, slope, indices[0])?,
                      Refraction::new(band, depth, slope, indices[1])?,
                      Refraction::new(band, depth, slope, indices[2])?];
        Some(Self { optics, indices, tint, tint_weight })
    }

    /// Maximum conservative per-axis sampling displacement in geometry units.
    pub fn displacement_bound(self) -> f64 {
        self.optics.iter().map(|r| r.displacement_bound()).fold(0.0,f64::max)
    }

    /// Input and output are linear RGB in [0,1]. Backdrop supplies prefiltered
    /// samples at offsets in geometry units; reflection is supplied externally.
    /// Coverage mixes the optical layer with the original, unfiltered pixel.
    pub fn compose(self, distance: f64, normal: [f64; 2], coverage: f64,
                   original: [f64; 3], reflection: [f64; 3],
                   mut backdrop: impl FnMut([f64; 2]) -> [f64; 3]) -> Option<[f64; 3]> {
        if !valid_rgb(original) || !valid_rgb(reflection) || !coverage.is_finite()
            || !(0.0..=1.0).contains(&coverage)
        {
            return None;
        }
        if coverage == 0.0 {
            return Some(original);
        }
        let mut out = [0.0; 3];
        for channel in 0..3 {
            let optic = self.optics[channel];
            let offset = optic.displacement(distance, normal)?;
            let sample = backdrop(offset);
            if !valid_rgb(sample) {
                return None;
            }
            let f = fresnel(optic.incidence_cosine(distance)?, self.indices[channel])?;
            let transmission = sample[channel] * (1.0 - self.tint_weight)
                + self.tint[channel] * self.tint_weight;
            let layer = transmission * (1.0 - f) + reflection[channel] * f;
            out[channel] = original[channel] * (1.0 - coverage) + layer * coverage;
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresnel_normal_and_grazing() {
        assert!((fresnel(1.0, 1.5).unwrap() - 0.04).abs() < 1e-12);
        assert_eq!(fresnel(0.0, 1.5), Some(1.0));
        assert_eq!(fresnel(0.0, 1.0), Some(0.0));
        assert!(fresnel(0.2, 1.5).unwrap() > fresnel(1.0, 1.5).unwrap());
        assert!(fresnel(f64::NAN, 1.5).is_none());
    }

    #[test]
    fn identity_and_coverage() {
        let m = Material::new(12.0, 8.0, 1.0, [1.0; 3], [0.0; 3], 0.0).unwrap();
        let original = [0.2, 0.4, 0.6];
        let input = [0.8, 0.2, 0.1];
        assert_eq!(m.compose(-6.0, [1.0, 0.0], 0.0, original, [1.0; 3], |_| panic!("no sampling")), Some(original));
        let out = m.compose(-6.0, [1.0, 0.0], 0.5, original, [1.0; 3], |offset| {
            assert_eq!(offset, [0.0; 2]);
            input
        }).unwrap();
        for i in 0..3 {
            assert!((out[i] - (original[i] + input[i]) * 0.5).abs() < 1e-12);
        }
    }

    #[test]
    fn dispersion_changes_channel_offsets_and_invalid_data_rejected() {
        let m = Material::new(12.0, 8.0, 1.0, [1.4, 1.5, 1.6], [0.5; 3], 0.1).unwrap();
        let mut offsets = Vec::new();
        let out = m.compose(-6.0, [1.0, 0.0], 1.0, [0.0; 3], [1.0; 3], |offset| {
            offsets.push(offset[0]);
            [0.5; 3]
        }).unwrap();
        assert!(offsets[0] > offsets[1] && offsets[1] > offsets[2]);
        assert!(valid_rgb(out));
        assert!(m.compose(-6.0, [1.0, 0.0], 1.0, [0.0; 3], [1.0; 3], |_| [f64::NAN; 3]).is_none());
    }
}
