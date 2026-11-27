//! Full-redraw reference for optical layers on retained surfaces.
//! The backdrop is the completed lower prefix, never the output being
//! written. Color assumes sRGB for opaque prefix conversion; ordinary
//! window blending retains the CPU renderer's encoded-space contract.
use super::{Error, Placed, over};
use supra_geometry::{Corner, RoundedRect};
use supra_glass::{blur::gaussian, material::Material};
use supra_scene::Rect;
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub struct Glass {
    pub sigma: f64,
    pub radius: f64,
    pub corner: Corner,
    pub material: Material,
    pub reflection: [f64;3],
}

pub struct OpticalLayer {
    pub placed: Placed,
    pub glass: Option<Glass>,
}

fn decode(v: u8) -> f64 {
    let v = f64::from(v)/255.0;
    if v <= 0.04045 { v/12.92 } else { ((v+0.055)/1.055).powf(2.4) }
}

fn encode(v: f64) -> u8 {
    let v = if v <= 0.0031308 { 12.92*v } else { 1.055*v.powf(1.0/2.4)-0.055 };
    (v.clamp(0.0,1.0)*255.0).round() as u8
}

fn sample(p: &[f64], w: usize, h: usize, x: f64, y: f64) -> f64 {
    let x = x.clamp(0.0,(w-1) as f64);
    let y = y.clamp(0.0,(h-1) as f64);
    let (ix,iy) = (x.floor() as usize,y.floor() as usize);
    let (jx,jy) = ((ix+1).min(w-1),(iy+1).min(h-1));
    let (fx,fy) = (x.fract(),y.fract());
    let a = p[iy*w+ix]*(1.0-fx)+p[iy*w+jx]*fx;
    let b = p[jy*w+ix]*(1.0-fx)+p[jy*w+jx]*fx;
    (a*(1.0-fy)+b*fy).clamp(0.0,1.0)
}

/// Small reference output, hard capped to bound full-frame floating point
/// scratch memory. Returns a new frame or error, never partial publication.
pub fn compose(width: u32, height: u32, layers: &[OpticalLayer]) -> Result<Vec<[u8;4]>,Error> {
    let output = Rect::new(0,0,width,height).ok_or(Error::InvalidOutput)?;
    let (w,h) = (width as usize,height as usize);
    let len = w.checked_mul(h).filter(|&n| n <= 262144).ok_or(Error::InvalidOutput)?;
    if width > 4096 || height > 4096 { return Err(Error::InvalidOutput); }
    let mut ids = BTreeSet::new();
    let mut frame = vec![[0,0,0,255];len];
    for layer in layers {
        let placed = &layer.placed;
        let geometry = placed.layer()?;
        if !ids.insert(geometry.id) { return Err(Error::DuplicateWindow); }
        let Some(clip) = geometry.bounds.intersect(output) else { continue; };
        let (x0,y0,x1,y1) = clip.edges();
        if let Some(glass) = layer.glass {
            let (left,top,right,bottom) = geometry.bounds.edges();
            let shape = RoundedRect::new(f64::from(right-left),f64::from(bottom-top),glass.radius,glass.corner)
                .ok_or(Error::InvalidSurface)?;
            if glass.reflection.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
                return Err(Error::InvalidSurface);
            }
            // Freeze lower prefix in independent linear planes BEFORE modifying
            // frame. Every sample, including nested glass, uses this prefix.
            let original: [Vec<f64>;3] = std::array::from_fn(|c| frame.iter().map(|p| decode(p[c])).collect());
            let blurred = [
                gaussian(&original[0],w,h,glass.sigma).map_err(|_| Error::InvalidSurface)?,
                gaussian(&original[1],w,h,glass.sigma).map_err(|_| Error::InvalidSurface)?,
                gaussian(&original[2],w,h,glass.sigma).map_err(|_| Error::InvalidSurface)?,
            ];
            for y in y0..y1 {
                for x in x0..x1 {
                    let (lx,ly) = (f64::from(x-left),f64::from(y-top));
                    let coverage = shape.coverage(lx,ly,1.0);
                    if coverage == 0.0 { continue; }
                    let b = shape.boundary(lx+0.5,ly+0.5).ok_or(Error::InvalidSurface)?;
                    let i = y as usize*w+x as usize;
                    let rgb = glass.material.compose(b.signed_distance,b.normal,coverage,
                        std::array::from_fn(|c| original[c][i]),glass.reflection,
                        |offset| std::array::from_fn(|c| sample(&blurred[c],w,h,
                            f64::from(x)+offset[0],f64::from(y)+offset[1])))
                        .ok_or(Error::InvalidSurface)?;
                    let [r,g,b] = rgb.map(encode);
                    frame[i] = [r,g,b,255];
                }
            }
        }
        // Content of this surface is added after its own backdrop effect.
        for y in y0..y1 {
            for x in x0..x1 {
                let i = y as usize*w+x as usize;
                frame[i] = over(frame[i],placed.sample(x,y));
            }
        }
    }
    Ok(frame)
}
