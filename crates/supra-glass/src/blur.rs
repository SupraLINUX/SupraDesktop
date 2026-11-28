#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlurError {
    InvalidSigma,
    InvalidDimensions,
    InvalidSample,
}

/// Normalized, truncated Gaussian; sigma is in physical sample units.
pub fn gaussian_kernel(sigma: f64) -> Result<Vec<f64>, BlurError> {
    if !sigma.is_finite() || !(0.0..=64.0).contains(&sigma) {
        return Err(BlurError::InvalidSigma);
    }
    if sigma == 0.0 {
        return Ok(vec![1.0]);
    }
    let radius = (3.0 * sigma).ceil() as i32;
    let mut weights: Vec<f64> = (-radius..=radius)
        .map(|i| {
            let x = f64::from(i) / sigma;
            (-0.5 * x * x).exp()
        })
        .collect();
    let sum: f64 = weights.iter().sum();
    for weight in &mut weights {
        *weight /= sum;
    }
    Ok(weights)
}

/// Separable convolution with clamp-to-edge sampling. Linear samples in [0,1].
pub fn gaussian(
    input: &[f64],
    width: usize,
    height: usize,
    sigma: f64,
) -> Result<Vec<f64>, BlurError> {
    if width == 0 || height == 0 || width > 4096 || height > 4096
        || width.checked_mul(height) != Some(input.len())
    {
        return Err(BlurError::InvalidDimensions);
    }
    if input.iter().any(|x| !x.is_finite() || !(0.0..=1.0).contains(x)) {
        return Err(BlurError::InvalidSample);
    }
    let weights = gaussian_kernel(sigma)?;
    if weights.len() == 1 {
        return Ok(input.to_vec());
    }
    let radius = weights.len() as isize / 2;
    let mut horizontal = vec![0.0; input.len()];
    let mut output = vec![0.0; input.len()];
    for y in 0..height {
        for x in 0..width {
            horizontal[y * width + x] = weights.iter().enumerate().map(|(i, w)| {
                let sx = (x as isize + i as isize - radius).clamp(0, width as isize - 1) as usize;
                input[y * width + sx] * w
            }).sum();
        }
    }
    for y in 0..height {
        for x in 0..width {
            output[y * width + x] = weights.iter().enumerate().map(|(i, w)| {
                let sy = (y as isize + i as isize - radius).clamp(0, height as isize - 1) as usize;
                horizontal[sy * width + x] * w
            }).sum::<f64>().clamp(0.0, 1.0);
        }
    }
    Ok(output)
}

pub mod region;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_symmetric_kernel() {
        for sigma in [0.0, 0.1, 1.0, 4.0, 64.0] {
            let k = gaussian_kernel(sigma).unwrap();
            assert!((k.iter().sum::<f64>() - 1.0).abs() < 1e-12);
            assert!(k.iter().zip(k.iter().rev()).all(|(a,b)| a == b));
        }
    }

    #[test]
    fn constant_and_identity() {
        let input = vec![0.37; 9 * 5];
        assert_eq!(gaussian(&input, 9, 5, 0.0).unwrap(), input);
        assert!(gaussian(&input, 9, 5, 2.0).unwrap().iter().all(|v| (v-0.37).abs()<1e-12));
    }

    #[test]
    fn impulse_mass_and_symmetry() {
        let mut input = vec![0.0; 21 * 21];
        input[10 * 21 + 10] = 1.0;
        let output = gaussian(&input, 21, 21, 2.0).unwrap();
        assert!((output.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert_eq!(output[10*21+8], output[10*21+12]);
        assert!(output[10*21+10] > output[10*21+11]);
        assert_eq!(output[0], 0.0);
    }

    #[test]
    fn edge_is_monotonic() {
        let input: Vec<f64> = (0..41).map(|x| if x < 20 { 0.0 } else { 1.0 }).collect();
        let output = gaussian(&input, 41, 1, 3.0).unwrap();
        assert!(output.windows(2).all(|w| w[0] <= w[1] + 1e-12));
        assert!(output[19] < 0.5 && output[20] > 0.5);
        assert!((output[19]+output[20]-1.0).abs()<1e-12);
    }

    #[test]
    fn invalid_inputs() {
        assert_eq!(gaussian_kernel(f64::NAN), Err(BlurError::InvalidSigma));
        assert_eq!(gaussian_kernel(65.0), Err(BlurError::InvalidSigma));
        assert_eq!(gaussian(&[0.0], 2, 1, 1.0), Err(BlurError::InvalidDimensions));
        assert_eq!(gaussian(&[f64::NAN], 1, 1, 1.0), Err(BlurError::InvalidSample));
    }
}
