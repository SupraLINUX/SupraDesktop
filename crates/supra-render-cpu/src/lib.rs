//! Reference software composition of retained window snapshots.
//! Output scale 1, signed CPU placements, nearest sample, opaque black
//! background, integer premultiplied source-over. No GPU or color management.
pub mod optical;
pub mod presentation;
mod placement;
pub use placement::SignedPlaced;
use supra_scene::{Layer, Rect, disjoint_regions, plan};
use supra_window::{cpu_content::{Format, Snapshot}, surface::Transform};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { InvalidOutput, InvalidSurface, DuplicateWindow, Allocation }

pub struct Placed {
    pub x: u32,
    pub y: u32,
    pub snapshot: Snapshot,
}

impl Placed {
    fn layer(&self) -> Result<Layer, Error> {
        let s = &self.snapshot;
        let size = s.content.layout().logical_size(s.state).map_err(|_| Error::InvalidSurface)?;
        let right = self.x.checked_add(size.width).ok_or(Error::InvalidSurface)?;
        let bottom = self.y.checked_add(size.height).ok_or(Error::InvalidSurface)?;
        if s.state.buffer.is_none() || s.generation == 0 { return Err(Error::InvalidSurface); }
        Ok(Layer { id: s.window, bounds: Rect::new(self.x, self.y, right, bottom).ok_or(Error::InvalidSurface)?,
            revision: s.generation, backdrop_radius: None })
    }

    fn sample(&self, x: u32, y: u32) -> [u8; 4] {
        sample_local(&self.snapshot,x-self.x,y-self.y)
    }
}

fn sample_local(s:&Snapshot,x:u32,y:u32)->[u8;4] {
        let l = s.content.layout();
        let scale = s.state.scale as u32;
        let (u, v) = (x*scale,y*scale);
        let (w, h) = (l.width, l.height);
        // Surface-to-buffer mapping: flip about vertical axis before CCW
        // rotation. Sampling this mapping applies the inverse to display.
        let (bx, by) = match s.state.transform {
            Transform::Normal => (u,v),
            Transform::Rotate90 => (v,h-1-u),
            Transform::Rotate180 => (w-1-u,h-1-v),
            Transform::Rotate270 => (w-1-v,u),
            Transform::Flipped => (w-1-u,v),
            Transform::Flipped90 => (v,u),
            Transform::Flipped180 => (u,h-1-v),
            Transform::Flipped270 => (w-1-v,h-1-u),
        };
        let i = by as usize * l.stride + bx as usize * 4;
        let p = &s.content.bytes()[i..i+4];
        let a = if l.format == Format::Xrgb8888 { 255 } else { p[3] };
        // Invalid premultiplication is clamped, not allowed to overflow.
        [p[2].min(a),p[1].min(a),p[0].min(a),a]
}

fn over(dst: [u8;4], src: [u8;4]) -> [u8;4] {
    let mut out = [0;4];
    let inv = 255-u16::from(src[3]);
    for i in 0..4 {
        out[i] = (u16::from(src[i]) + (u16::from(dst[i])*inv+127)/255).min(255) as u8;
    }
    out
}

fn pixel(x: u32, y: u32, layers: &[Layer], surfaces: &[SignedPlaced]) -> [u8;4] {
    let mut p = [0,0,0,255];
    for (l,s) in layers.iter().zip(surfaces) {
        if l.bounds.contains(x,y) { p = over(p,s.sample(x,y)); }
    }
    p
}

pub struct Frame {
    width: u32,
    output: Rect,
    rgba: Vec<[u8;4]>,
    layers: Vec<Layer>,
    surfaces: Vec<SignedPlaced>,
}

impl Frame {
    /// Explicit lab output cap; caller controls max pixel budget.
    pub fn new(width: u32, height: u32, max_pixels: usize) -> Result<Self, Error> {
        let output = Rect::new(0,0,width,height).ok_or(Error::InvalidOutput)?;
        let len = (width as usize).checked_mul(height as usize).ok_or(Error::InvalidOutput)?;
        if len > max_pixels { return Err(Error::InvalidOutput); }
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(len).map_err(|_| Error::Allocation)?;
        rgba.resize(len,[0,0,0,255]);
        Ok(Self { width,output,rgba,layers:Vec::new(),surfaces:Vec::new() })
    }

    pub fn pixels(&self) -> &[[u8;4]] { &self.rgba }

    /// Count actual recomposited output pixels, not GPU work or timing.
    /// All validation precedes mutation; each damaged pixel is written once.
    pub fn update(&mut self, surfaces: Vec<Placed>) -> Result<usize, Error> {
        self.update_signed(surfaces.into_iter().map(SignedPlaced::from).collect())
    }

    /// Physical output coordinates with signed window origins. Validate even
    /// fully clipped placements and reject duplicate IDs before filtering.
    pub fn update_signed(&mut self, input:Vec<SignedPlaced>)->Result<usize,Error> {
        let mut ids=std::collections::BTreeSet::new();
        let mut layers=Vec::new();
        let mut surfaces=Vec::new();
        for surface in input {
            if !ids.insert(surface.snapshot.window) { return Err(Error::DuplicateWindow); }
            if let Some(layer)=surface.visible_layer(self.output)? {
                layers.push(layer);
                surfaces.push(surface);
            }
        }
        let damage = plan(self.output,self.output,&self.layers,&layers).map_err(|_| Error::DuplicateWindow)?;
        // Defend against a stale/reused revision from a different producer:
        // force redraw if a nominally unchanged layer has different storage
        // or surface state. No duplicate mutable authority for revisions.
        let ambiguous = self.layers.iter().zip(&layers).zip(self.surfaces.iter().zip(&surfaces))
            .any(|((a,b),(old,new))| a == b &&
                (!old.snapshot.content.same_storage(&new.snapshot.content) || old.snapshot.state != new.snapshot.state
                 || old.x != new.x || old.y != new.y));
        let regions = if ambiguous || damage.full_redraw { vec![self.output] }
                      else { disjoint_regions(&damage.regions,self.output) };
        let mut count = 0;
        for region in regions {
            let (x0,y0,x1,y1) = region.edges();
            for y in y0..y1 {
                for x in x0..x1 {
                    self.rgba[y as usize*self.width as usize+x as usize] = pixel(x,y,&layers,&surfaces);
                    count += 1;
                }
            }
        }
        self.layers = layers;
        self.surfaces = surfaces;
        Ok(count)
    }
}
