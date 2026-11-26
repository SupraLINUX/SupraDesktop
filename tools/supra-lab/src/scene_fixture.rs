//! Small, bounded CPU lab for genuine incremental composition.
//! Uses original synthetic grayscale layers, not a production renderer.
use supra_scene::{Layer, Rect, plan};

const W: usize = 96;
const H: usize = 64;
const N: usize = W * H;
const BACKGROUND: u8 = 16;

fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect::new(x, y, x+w, y+h).unwrap()
}

fn output() -> Rect {
    rect(0, 0, W as u32, H as u32)
}

fn shade(layer: &Layer, lower: &[u8], x: usize, y: usize) -> u8 {
    if !layer.bounds.contains(x as u32, y as u32) {
        return lower[y * W + x];
    }
    if let Some(radius) = layer.backdrop_radius {
        let r = radius as usize;
        let mut sum = 0_u64;
        let mut count = 0_u64;
        for yy in y.saturating_sub(r)..=y.saturating_add(r).min(H-1) {
            for xx in x.saturating_sub(r)..=x.saturating_add(r).min(W-1) {
                sum += u64::from(lower[yy*W+xx]);
                count += 1;
            }
        }
        ((sum/count + 240)/2) as u8
    } else {
        (30 + (layer.id%180*17 + layer.revision%180*31)%180) as u8
    }
}

// Full oracle performs no damage planning or cached composition.
fn full(layers: &[Layer]) -> Vec<u8> {
    let mut lower = vec![BACKGROUND; N];
    for layer in layers {
        lower = (0..N).map(|i| shade(layer, &lower, i%W, i/W)).collect();
    }
    lower
}

#[derive(Default)]
struct Cached {
    previous: Vec<Layer>,
    stages: Vec<Vec<u8>>,
}

impl Cached {
    fn update(&mut self, next: &[Layer]) -> usize {
        let reset = self.stages.is_empty()
            || !self.previous.iter().map(|l| l.id).eq(next.iter().map(|l| l.id));
        if reset {
            self.stages = vec![vec![BACKGROUND; N]; next.len()+1];
        }
        let mut recomputed = 0;
        for (index, layer) in next.iter().enumerate() {
            let regions = if reset {
                vec![output()]
            } else {
                plan(output(), output(), &self.previous[..=index], &next[..=index])
                    .unwrap().regions
            };
            // Prefix i samples only the freshly updated prefix i-1, including
            // unchanged cached pixels in its filter footprint.
            let (lower, current) = self.stages.split_at_mut(index+1);
            let lower = &lower[index];
            let current = &mut current[0];
            for (i, pixel) in current.iter_mut().enumerate() {
                let (x,y) = (i%W,i/W);
                if regions.iter().any(|r| r.contains(x as u32,y as u32)) {
                    *pixel = shade(layer, lower, x,y);
                    recomputed += 1;
                }
            }
        }
        self.previous = next.to_vec();
        recomputed
    }

    fn image(&self) -> &[u8] {
        self.stages.last().unwrap()
    }
}

fn layer(id: u64, bounds: Rect, radius: Option<u32>) -> Layer {
    Layer { id, bounds, revision: 0, backdrop_radius: radius }
}

fn initial() -> Vec<Layer> {
    vec![layer(1,rect(1,1,18,15),None),
         layer(2,rect(20,6,35,30),Some(4)),
         layer(3,rect(53,13,30,35),Some(3)),
         layer(4,rect(6,50,12,10),None)]
}

/// Deterministic counters of evaluated stage pixels, not timing or GPU savings.
pub fn experiment() -> String {
    let mut layers = initial();
    let mut cache = Cached::default();
    let mut incremental_work = 0;
    let mut full_work = 0;
    for frame in 0..64_u32 {
        layers[0].bounds = rect(frame%42,frame%25,18,15);
        layers[0].revision += 1;
        if frame%5 == 0 { layers[1].backdrop_radius = Some(1+frame%7); }
        if frame%3 == 0 { layers[3].revision += 1; }
        if frame == 24 || frame == 40 { layers.swap(1,2); }
        incremental_work += cache.update(&layers);
        full_work += N*layers.len();
        assert_eq!(cache.image(), full(&layers), "frame {frame}");
    }
    assert_eq!(cache.update(&layers), 0);
    assert_eq!(cache.image(), full(&layers));
    assert!(incremental_work < full_work);
    format!("{{\"schema_version\":1,\"kind\":\"cpu-incremental-reference\",\"frames\":64,\"width\":{W},\"height\":{H},\"full_stage_pixels\":{full_work},\"incremental_stage_pixels\":{incremental_work},\"unchanged_frame_stage_pixels\":0,\"all_frames_equal\":true,\"gpu_verified\":false}}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_reference_matches_full_sequence() {
        assert_eq!(experiment(), experiment());
    }

    #[test]
    fn structural_edits_clear_cached_stages() {
        let mut cache = Cached::default();
        let mut layers = initial();
        assert_eq!(cache.update(&layers), N*layers.len());
        layers.remove(1);
        assert_eq!(cache.update(&layers), N*layers.len());
        assert_eq!(cache.image(), full(&layers));
        layers.insert(0, layer(9,rect(0,0,10,10),None));
        assert_eq!(cache.update(&layers), N*layers.len());
        assert_eq!(cache.image(), full(&layers));
        layers.clear();
        assert_eq!(cache.update(&layers), 0);
        assert_eq!(cache.image(), full(&layers));
    }

    #[test]
    fn upper_layer_change_does_not_recompute_lower_stages() {
        let mut cache = Cached::default();
        let mut layers = initial();
        cache.update(&layers);
        layers[3].revision += 1;
        assert_eq!(cache.update(&layers), 120);
        assert_eq!(cache.image(), full(&layers));
    }
}
