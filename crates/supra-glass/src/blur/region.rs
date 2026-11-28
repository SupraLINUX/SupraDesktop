//! Cropped separable Gaussian, equivalent to full-plane convolution.
//! Coordinates are global, clamped only to the actual image edges.
use super::{BlurError,gaussian_kernel};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x0: usize, pub y0: usize, pub x1: usize, pub y1: usize,
}

pub struct Cropped {
    region: Region,
    values: Vec<f64>,
    pub horizontal_samples: usize,
    pub vertical_samples: usize,
}

impl Cropped {
    pub fn get(&self,x:usize,y:usize)->Option<f64> {
        let r=self.region;
        (x>=r.x0 && x<r.x1 && y>=r.y0 && y<r.y1)
            .then(|| self.values[(y-r.y0)*(r.x1-r.x0)+x-r.x0])
    }
}

/// Evaluate only requested output texels. Horizontal pass additionally
/// evaluates the vertical kernel halo. Same weights and operation order
/// as full gaussian; never clamp at an artificial crop boundary.
pub fn gaussian_region(input:&[f64],width:usize,height:usize,sigma:f64,r:Region)->Result<Cropped,BlurError> {
    if width==0 || height==0 || width>4096 || height>4096
        || width.checked_mul(height)!=Some(input.len())
        || r.x0>=r.x1 || r.y0>=r.y1 || r.x1>width || r.y1>height {
        return Err(BlurError::InvalidDimensions);
    }
    if input.iter().any(|x| !x.is_finite() || !(0.0..=1.0).contains(x)) {
        return Err(BlurError::InvalidSample);
    }
    let weights=gaussian_kernel(sigma)?;
    let radius=weights.len()/2;
    let rw=r.x1-r.x0;
    let rh=r.y1-r.y0;
    if radius==0 {
        let values=(r.y0..r.y1).flat_map(|y| input[y*width+r.x0..y*width+r.x1].iter().copied()).collect();
        return Ok(Cropped {region:r,values,horizontal_samples:0,vertical_samples:0});
    }
    let hy0=r.y0.saturating_sub(radius);
    let hy1=(r.y1+radius).min(height);
    let mut horizontal=vec![0.0;rw*(hy1-hy0)];
    for y in hy0..hy1 { for x in r.x0..r.x1 {
        horizontal[(y-hy0)*rw+x-r.x0]=weights.iter().enumerate().map(|(i,w)| {
            let sx=(x as isize+i as isize-radius as isize).clamp(0,width as isize-1) as usize;
            input[y*width+sx]*w
        }).sum();
    }}
    let mut values=vec![0.0;rw*rh];
    for y in r.y0..r.y1 { for x in r.x0..r.x1 {
        values[(y-r.y0)*rw+x-r.x0]=weights.iter().enumerate().map(|(i,w)| {
            let sy=(y as isize+i as isize-radius as isize).clamp(0,height as isize-1) as usize;
            horizontal[(sy-hy0)*rw+x-r.x0]*w
        }).sum::<f64>().clamp(0.0,1.0);
    }}
    Ok(Cropped {region:r,values,horizontal_samples:horizontal.len(),vertical_samples:rw*rh})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blur::gaussian;
    #[test]
    fn exact_global_edges_and_halos() {
        for (w,h) in [(17,11),(1,19),(19,1)] {
            let input:Vec<_>=(0..w*h).map(|i| (i*17%251) as f64/250.0).collect();
            for sigma in [0.0,0.1,1.3,4.0,64.0] {
                let full=gaussian(&input,w,h,sigma).unwrap();
                for r in [Region{x0:0,y0:0,x1:1,y1:1}, Region{x0:w/2,y0:h/2,x1:w,y1:h},
                    Region{x0:0,y0:0,x1:w,y1:h}] {
                    let crop=gaussian_region(&input,w,h,sigma,r).unwrap();
                    for y in 0..h { for x in 0..w {
                        let expected=(x>=r.x0 && y>=r.y0 && x<r.x1 && y<r.y1).then(|| full[y*w+x]);
                        assert_eq!(crop.get(x,y),expected);
                    }}
                    assert!(crop.horizontal_samples<=w*h && crop.vertical_samples<=w*h);
                }
            }
        }
    }
}
