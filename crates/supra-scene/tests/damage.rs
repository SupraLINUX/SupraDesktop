use supra_scene::{Layer, Rect, SceneError, plan};

fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect::new(x, y, x+w, y+h).unwrap()
}

fn layer(id: u64, bounds: Rect, radius: Option<u32>) -> Layer {
    Layer { id, bounds, revision: 0, backdrop_radius: radius }
}

#[test]
fn lower_damage_propagates_but_upper_does_not() {
    let output = rect(0, 0, 40, 40);
    let old = [layer(1, rect(2, 2, 4, 4), None),
               layer(2, rect(8, 2, 8, 8), Some(3)),
               layer(3, rect(18, 2, 8, 8), Some(3))];
    let mut new = old;
    new[0].revision += 1;
    let damage = plan(output, output, &old, &new).unwrap();
    assert_eq!(damage.invalidated_backdrops, [2, 3]);
    assert!(!damage.full_redraw);
    new = old;
    new[2].revision += 1;
    assert_eq!(plan(output, output, &old, &new).unwrap().invalidated_backdrops, [3]);
    assert!(plan(output, output, &old, &old).unwrap().regions.is_empty());
}

#[test]
fn structure_changes_and_duplicate_ids() {
    let output = rect(0, 0, 40, 40);
    let old = [layer(1, rect(2, 2, 4, 4), None), layer(2, rect(0, 0, 10, 10), Some(2))];
    let swapped = [old[1], old[0]];
    assert!(plan(output, output, &old, &swapped).unwrap().full_redraw);
    assert!(plan(output, output, &old, &old[..1]).unwrap().full_redraw);
    assert!(plan(output, rect(0,0,50,40), &old, &old).unwrap().full_redraw);
    assert_eq!(plan(output, output, &old, &[old[0],old[0]]), Err(SceneError::DuplicateLayer));
}

// A tiny independent full renderer. Materials sample a box-filtered snapshot
// of lower layers only; the planner knows nothing about pixel values.
fn render(layers: &[Layer]) -> Vec<u8> {
    const N: usize = 32;
    let mut image = vec![16_u8; N*N];
    for l in layers {
        let lower = image.clone();
        for y in 0..N {
            for x in 0..N {
                if !l.bounds.contains(x as u32,y as u32) { continue; }
                image[y*N+x] = if let Some(r) = l.backdrop_radius {
                    let r = r as usize;
                    let mut total = 0_u32;
                    let mut n = 0_u32;
                    for yy in y.saturating_sub(r)..=(y+r).min(N-1) {
                        for xx in x.saturating_sub(r)..=(x+r).min(N-1) {
                            total += u32::from(lower[yy*N+xx]);
                            n += 1;
                        }
                    }
                    ((total/n + 240)/2) as u8
                } else {
                    (30 + (l.id*17 + l.revision*31)%180) as u8
                };
            }
        }
    }
    image
}

#[test]
fn conservative_damage_matches_full_redraw_on_synthetic_sequence() {
    let output = rect(0,0,32,32);
    let initial = [layer(1,rect(1,1,9,9),None),
                   layer(2,rect(9,3,12,12),Some(3)),
                   layer(3,rect(20,7,10,15),Some(2)),
                   layer(4,rect(4,25,4,5),None)];
    let mut old = initial;
    let mut incremental = render(&old);
    for frame in 0..48_u32 {
        let mut new = old;
        new[0].bounds = rect(frame%19, frame%13, 9, 9);
        new[0].revision += 1;
        if frame%5 == 0 { new[1].backdrop_radius = Some(1+frame%4); }
        if frame%3 == 0 { new[3].revision += 1; }
        let damage = plan(output,output,&old,&new).unwrap();
        let full = render(&new);
        for y in 0..32_u32 {
            for x in 0..32_u32 {
                if damage.regions.iter().any(|r| r.contains(x,y)) {
                    incremental[(y*32+x) as usize] = full[(y*32+x) as usize];
                }
            }
        }
        assert_eq!(incremental,full,"frame {frame}");
        old = new;
    }
}
