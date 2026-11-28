//! Incremental prefix compositor. Whole-prefix buffers are retained for
//! reference correctness; Gaussian passes evaluate a conservative crop.
//! No GPU or measured latency claim.
use super::{Glass, OpticalLayer, decode, encode};
use crate::{Error, over};
use supra_geometry::RoundedRect;
use supra_glass::blur::region::{Cropped,Region,gaussian_region_cropped,source_region};
use supra_scene::{Layer, Rect, disjoint_regions, plan};
use std::{collections::BTreeSet, sync::Arc};

type Pixel = [u8;4];
type Prefix = Arc<Vec<Pixel>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// Actual prefix pixel evaluations, including copies from lower prefix.
    pub prefix_pixels: usize,
    pub optical_pixels: usize,
    /// Number of RGB blur requests, not full-plane passes.
    pub blur_passes: usize,
    pub blur_samples: usize,
    pub full_plane_blur_samples: usize,
    pub decoded_channels: usize,
    pub full_plane_decoded_channels: usize,
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
    source: Region,
    blurred: [Cropped;3],
}

impl GlassPass {
    fn new(g: Glass, bounds: Rect, crop:Region, lower: &[Pixel], w: usize,h: usize) -> Result<Self,Error> {
        let (l,t,r,b) = bounds.edges();
        let shape = RoundedRect::new(f64::from(r-l),f64::from(b-t),g.radius,g.corner)
            .ok_or(Error::InvalidSurface)?;
        let source=source_region(w,h,g.sigma,crop).map_err(|_| Error::InvalidSurface)?;
        let original: [Vec<f64>;3] = std::array::from_fn(|c| (source.y0..source.y1).flat_map(|y|
            lower[y*w+source.x0..y*w+source.x1].iter().map(move |p| decode(p[c]))).collect());
        let blurred = [
            gaussian_region_cropped(&original[0],w,h,g.sigma,crop).map_err(|_| Error::InvalidSurface)?,
            gaussian_region_cropped(&original[1],w,h,g.sigma,crop).map_err(|_| Error::InvalidSurface)?,
            gaussian_region_cropped(&original[2],w,h,g.sigma,crop).map_err(|_| Error::InvalidSurface)?,
        ];
        Ok(Self { glass:g,shape,original,source,blurred })
    }

    fn pixel(&self, x:u32,y:u32,bounds:Rect,w:usize,h:usize) -> Result<Option<Pixel>,Error> {
        let (l,t,_,_) = bounds.edges();
        let (lx,ly) = (f64::from(x-l),f64::from(y-t));
        let coverage = self.shape.coverage(lx,ly,1.0);
        if coverage == 0.0 { return Ok(None); }
        let b = self.shape.boundary(lx+0.5,ly+0.5).ok_or(Error::InvalidSurface)?;
        let (x0,y0)=(x as usize,y as usize);
        let s=self.source;
        if x0<s.x0 || x0>=s.x1 || y0<s.y0 || y0>=s.y1 { return Err(Error::InvalidSurface); }
        let i=(y0-s.y0)*(s.x1-s.x0)+x0-s.x0;
        let mut missing=false;
        let rgb = self.glass.material.compose(b.signed_distance,b.normal,coverage,
            std::array::from_fn(|c| self.original[c][i]),self.glass.reflection,
            |offset| std::array::from_fn(|c| {
                crop_sample(&self.blurred[c],w,h,f64::from(x)+offset[0],f64::from(y)+offset[1])
                    .unwrap_or_else(|| { missing=true; 0.0 })
            }))
            .ok_or(Error::InvalidSurface)?;
        if missing { return Err(Error::InvalidSurface); }
        let [r,g,b] = rgb.map(encode);
        Ok(Some([r,g,b,255]))
    }
}

fn crop_sample(p:&Cropped,w:usize,h:usize,x:f64,y:f64)->Option<f64> {
    let x=x.clamp(0.0,(w-1) as f64);
    let y=y.clamp(0.0,(h-1) as f64);
    let (ix,iy)=(x.floor() as usize,y.floor() as usize);
    let (jx,jy)=((ix+1).min(w-1),(iy+1).min(h-1));
    let (fx,fy)=(x.fract(),y.fract());
    let a=p.get(ix,iy)?*(1.0-fx)+p.get(jx,iy)?*fx;
    let b=p.get(ix,jy)?*(1.0-fx)+p.get(jx,jy)?*fx;
    Some((a*(1.0-fy)+b*fy).clamp(0.0,1.0))
}

fn sampling_crop(regions:&[Rect],bounds:Rect,g:Glass,w:usize,h:usize)->Option<Region> {
    let mut clips=regions.iter().filter_map(|r| r.intersect(bounds));
    let (mut l,mut t,mut r,mut b)=clips.next()?.edges();
    for clip in clips {
        let (x0,y0,x1,y1)=clip.edges();
        l=l.min(x0); t=t.min(y0); r=r.max(x1); b=b.max(y1);
    }
    let halo=g.material.displacement_bound().ceil() as usize+1;
    Some(Region{x0:(l as usize).saturating_sub(halo),y0:(t as usize).saturating_sub(halo),
        x1:(r as usize+halo).min(w),y1:(b as usize+halo).min(h)})
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
            let pass = if let Some(g)=layer.glass {
                if let Some(crop)=sampling_crop(&regions,d.bounds,g,self.w,self.h) {
                    let pass=GlassPass::new(g,d.bounds,crop,lower,self.w,self.h)?;
                    work.blur_passes+=1;
                    work.decoded_channels+=pass.original.iter().map(Vec::len).sum::<usize>();
                    work.full_plane_decoded_channels+=3*self.w*self.h;
                    work.blur_samples+=pass.blurred.iter().map(|p| p.horizontal_samples+p.vertical_samples).sum::<usize>();
                    if g.sigma!=0.0 { work.full_plane_blur_samples+=6*self.w*self.h; }
                    Some(pass)
                } else { None }
            } else { None };
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
