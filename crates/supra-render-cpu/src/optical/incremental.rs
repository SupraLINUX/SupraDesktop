//! Incremental prefix compositor. Whole-prefix buffers are retained for
//! reference correctness; blur is full-plane only when a touched glass
//! region needs it. No GPU or measured latency claim.
use super::{Glass, OpticalLayer, decode, encode, sample};
use crate::{Error, over};
use supra_geometry::RoundedRect;
use supra_glass::blur::gaussian;
use supra_scene::{Layer, Rect, disjoint_regions, plan};
use std::{collections::BTreeSet, sync::Arc};

type Pixel = [u8;4];
type Prefix = Arc<Vec<Pixel>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// Actual prefix pixel evaluations, including copies from lower prefix.
    pub prefix_pixels: usize,
    pub optical_pixels: usize,
    /// Number of complete RGB blur passes (not separable tap counts).
    pub blur_passes: usize,
}

pub struct Incremental {
    output: Rect,
    w: usize,
    h: usize,
    black: Prefix,
    prefixes: Vec<Prefix>,
    previous: Vec<OpticalLayer>,
    descriptions: Vec<Layer>,
}

fn description(layer: &OpticalLayer) -> Result<Layer,Error> {
    let mut d = layer.placed.layer()?;
    if let Some(g) = layer.glass {
        if !g.sigma.is_finite() || !(0.0..=64.0).contains(&g.sigma)
            || !g.radius.is_finite() || g.radius < 0.0
            || g.reflection.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
            return Err(Error::InvalidSurface);
        }
        // Gaussian truncation plus maximum refraction and one bilinear texel.
        d.backdrop_radius = Some((3.0*g.sigma).ceil() as u32
            + g.material.displacement_bound().ceil() as u32 + 1);
    }
    Ok(d)
}

struct GlassPass {
    glass: Glass,
    shape: RoundedRect,
    original: [Vec<f64>;3],
    blurred: [Vec<f64>;3],
}

impl GlassPass {
    fn new(g: Glass, bounds: Rect, lower: &[Pixel], w: usize,h: usize) -> Result<Self,Error> {
        let (l,t,r,b) = bounds.edges();
        let shape = RoundedRect::new(f64::from(r-l),f64::from(b-t),g.radius,g.corner)
            .ok_or(Error::InvalidSurface)?;
        let original: [Vec<f64>;3] = std::array::from_fn(|c| lower.iter().map(|p| decode(p[c])).collect());
        let blurred = [
            gaussian(&original[0],w,h,g.sigma).map_err(|_| Error::InvalidSurface)?,
            gaussian(&original[1],w,h,g.sigma).map_err(|_| Error::InvalidSurface)?,
            gaussian(&original[2],w,h,g.sigma).map_err(|_| Error::InvalidSurface)?,
        ];
        Ok(Self { glass:g,shape,original,blurred })
    }

    fn pixel(&self, x:u32,y:u32,bounds:Rect,w:usize,h:usize) -> Result<Option<Pixel>,Error> {
        let (l,t,_,_) = bounds.edges();
        let (lx,ly) = (f64::from(x-l),f64::from(y-t));
        let coverage = self.shape.coverage(lx,ly,1.0);
        if coverage == 0.0 { return Ok(None); }
        let b = self.shape.boundary(lx+0.5,ly+0.5).ok_or(Error::InvalidSurface)?;
        let i = y as usize*w+x as usize;
        let rgb = self.glass.material.compose(b.signed_distance,b.normal,coverage,
            std::array::from_fn(|c| self.original[c][i]),self.glass.reflection,
            |offset| std::array::from_fn(|c| sample(&self.blurred[c],w,h,
                f64::from(x)+offset[0],f64::from(y)+offset[1])))
            .ok_or(Error::InvalidSurface)?;
        let [r,g,b] = rgb.map(encode);
        Ok(Some([r,g,b,255]))
    }
}

impl Incremental {
    pub fn new(width:u32,height:u32) -> Result<Self,Error> {
        let output = Rect::new(0,0,width,height).ok_or(Error::InvalidOutput)?;
        let (w,h) = (width as usize,height as usize);
        let len = w.checked_mul(h).filter(|&n| n <= 262144).ok_or(Error::InvalidOutput)?;
        if width > 4096 || height > 4096 { return Err(Error::InvalidOutput); }
        Ok(Self { output,w,h,black:Arc::new(vec![[0,0,0,255];len]),
            prefixes:Vec::new(),previous:Vec::new(),descriptions:Vec::new() })
    }

    pub fn pixels(&self) -> &[Pixel] {
        self.prefixes.last().unwrap_or(&self.black)
    }

    /// Transactional publication: new prefixes built privately; any failure
    /// leaves prior frame unchanged. Limit retained prefix count to 16.
    pub fn update(&mut self, layers:Vec<OpticalLayer>) -> Result<Work,Error> {
        if layers.len()>16 { return Err(Error::InvalidOutput); }
        let desc = layers.iter().map(description).collect::<Result<Vec<_>,_>>()?;
        let mut ids = BTreeSet::new();
        if !desc.iter().all(|l| ids.insert(l.id)) { return Err(Error::DuplicateWindow); }
        let structural = !self.descriptions.iter().map(|l| l.id).eq(desc.iter().map(|l| l.id));
        let mut next:Vec<Prefix> = Vec::with_capacity(layers.len());
        let mut work = Work::default();
        let mut force = structural;
        for (index,layer) in layers.iter().enumerate() {
            let d = desc[index];
            // If caller reuses a nominal revision for changed storage/state or
            // material, conservatively invalidate this and subsequent prefixes.
            if !structural {
                let old = &self.previous[index];
                if old.glass != layer.glass || (self.descriptions[index] == d &&
                    (!old.placed.snapshot.content.same_storage(&layer.placed.snapshot.content)
                     || old.placed.snapshot.state != layer.placed.snapshot.state)) { force = true; }
            }
            let regions = if force { vec![self.output] } else {
                let damage = plan(self.output,self.output,&self.descriptions[..=index],&desc[..=index])
                    .map_err(|_| Error::DuplicateWindow)?;
                disjoint_regions(&damage.regions,self.output)
            };
            if regions.is_empty() {
                next.push(Arc::clone(&self.prefixes[index]));
                continue;
            }
            let lower = next.last().unwrap_or(&self.black);
            let mut pixels = if structural { (**lower).clone() } else { (*self.prefixes[index]).clone() };
            let touches = regions.iter().any(|r| r.intersect(d.bounds).is_some());
            let pass = match (layer.glass,touches) {
                (Some(g),true) => { work.blur_passes += 1; Some(GlassPass::new(g,d.bounds,lower,self.w,self.h)?) },
                _ => None,
            };
            for r in regions {
                let (x0,y0,x1,y1) = r.edges();
                for y in y0..y1 { for x in x0..x1 {
                    let i = y as usize*self.w+x as usize;
                    let mut p = lower[i];
                    if d.bounds.contains(x,y) {
                        if let Some(pass) = &pass {
                            if let Some(value) = pass.pixel(x,y,d.bounds,self.w,self.h)? {
                                p = value;
                                work.optical_pixels += 1;
                            }
                        }
                        p = over(p,layer.placed.sample(x,y));
                    }
                    pixels[i]=p;
                    work.prefix_pixels += 1;
                }}
            }
            next.push(Arc::new(pixels));
        }
        self.previous=layers;
        self.descriptions=desc;
        self.prefixes=next;
        Ok(work)
    }
}
