//! Synthetic linear luminance patterns; sRGB-encoded for display only.
use supra_glass::blur::gaussian;
use supra_geometry::{Corner, RoundedRect};

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 360;

fn encode(x: f64) -> u8 {
    let value = if x <= 0.0031308 { 12.92*x } else { 1.055*x.powf(1.0/2.4)-0.055 };
    (255.0*value.clamp(0.0,1.0)).round() as u8
}

pub fn blur_fixture() -> Vec<u8> {
    let (w,h)=(WIDTH,HEIGHT/3);
    let pattern: Vec<f64> = (0..w*h).map(|i| {
        let x=i%w;
        if x < w/2 { if x/12%2==0 { 0.1 } else { 0.9 } }
        else if x < 3*w/4 { 0.1 } else { 0.9 }
    }).collect();
    let mut pixels = Vec::with_capacity(WIDTH*HEIGHT*3);
    for sigma in [0.0, 4.0, 12.0] {
        for v in gaussian(&pattern,w,h,sigma).unwrap() {
            pixels.extend_from_slice(&[encode(v);3]);
        }
    }
    ppm(pixels)
}

pub fn corners_fixture() -> Vec<u8> {
    let models=[Corner::Circular,Corner::Continuous,Corner::BezierG2];
    let shapes: Vec<_> = models.into_iter().map(|model| RoundedRect::new(180.0, 180.0, 60.0, model).unwrap()).collect();
    let mut pixels=Vec::with_capacity(WIDTH*HEIGHT*3);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let mut a=0.0;
            for (i,shape) in shapes.iter().enumerate() {
                a += shape.coverage(x as f64 - (15+215*i) as f64,y as f64-90.0,1.0);
            }
            let v=encode(0.85*(1.0-a)+0.08*a);
            pixels.extend_from_slice(&[v;3]);
        }
    }
    ppm(pixels)
}

fn ppm(pixels: Vec<u8>) -> Vec<u8> {
    let mut out=format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    out.extend(pixels);
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_fixtures() {
        assert_eq!(blur_fixture(),blur_fixture());
        assert_eq!(corners_fixture(),corners_fixture());
        assert_eq!(blur_fixture().len(), 15+WIDTH*HEIGHT*3);
    }
}
