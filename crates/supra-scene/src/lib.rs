//! Experimental conservative damage planning in physical output coordinates.
//! No Wayland buffers, GPU, occlusion, or runtime compositor dependencies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Rect {
    pub fn new(x0: u32, y0: u32, x1: u32, y1: u32) -> Option<Self> {
        (x0 < x1 && y0 < y1).then_some(Self { x0, y0, x1, y1 })
    }

    pub fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    pub fn intersect(self, other: Self) -> Option<Self> {
        Self::new(self.x0.max(other.x0), self.y0.max(other.y0),
                  self.x1.min(other.x1), self.y1.min(other.y1))
    }

    fn expanded(self, radius: u32) -> Self {
        Self { x0: self.x0.saturating_sub(radius), y0: self.y0.saturating_sub(radius),
               x1: self.x1.saturating_add(radius), y1: self.y1.saturating_add(radius) }
    }
}

/// Bottom-to-top order. Revision must change whenever content or other
/// appearance-affecting state changes. Footprint is a conservative radius
/// in physical pixels around the visible bounds, including blur/refraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layer {
    pub id: u64,
    pub bounds: Rect,
    pub revision: u64,
    pub backdrop_radius: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneError {
    InvalidOutput,
    DuplicateLayer,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Damage {
    pub regions: Vec<Rect>,
    pub invalidated_backdrops: Vec<u64>,
    pub full_redraw: bool,
}

fn unique(layers: &[Layer]) -> bool {
    let mut ids = std::collections::BTreeSet::new();
    layers.iter().all(|l| ids.insert(l.id))
}

fn push(regions: &mut Vec<Rect>, rect: Rect, output: Rect) {
    if let Some(clipped) = rect.intersect(output) {
        if !regions.contains(&clipped) {
            regions.push(clipped);
        }
    }
}

/// Conservative reference planner. Structure or output changes use full
/// redraw; otherwise propagate lower-layer changes upward only. The caller
/// must use the same output/clip semantics for rendering and revisions.
pub fn plan(old_output: Rect, output: Rect, old: &[Layer], new: &[Layer]) -> Result<Damage, SceneError> {
    if output.x0 != 0 || output.y0 != 0 || old_output.x0 != 0 || old_output.y0 != 0 {
        return Err(SceneError::InvalidOutput);
    }
    if !unique(old) || !unique(new) {
        return Err(SceneError::DuplicateLayer);
    }
    if old_output != output || !old.iter().map(|l| l.id).eq(new.iter().map(|l| l.id)) {
        return Ok(Damage { regions: vec![output], full_redraw: true,
            invalidated_backdrops: new.iter().filter(|l| l.backdrop_radius.is_some()).map(|l| l.id).collect() });
    }
    let mut result = Damage { regions: Vec::new(), invalidated_backdrops: Vec::new(), full_redraw: false };
    for (before, after) in old.iter().zip(new) {
        let own_change = before != after;
        let dependency_change = after.backdrop_radius.is_some_and(|r| {
            after.bounds.intersect(output).is_some() &&
            result.regions.iter().any(|d| d.intersect(after.bounds.expanded(r)).is_some())
        });
        if own_change {
            push(&mut result.regions, before.bounds, output);
            push(&mut result.regions, after.bounds, output);
        }
        if dependency_change {
            push(&mut result.regions, after.bounds, output);
        }
        if (own_change || dependency_change) && after.backdrop_radius.is_some() {
            result.invalidated_backdrops.push(after.id);
        }
    }
    Ok(result)
}
