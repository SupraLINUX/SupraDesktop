//! Original deterministic optical composition of real retained window bytes.
use supra_geometry::Corner;
use supra_glass::material::Material;
use supra_render_cpu::{Placed,optical::{Glass,OpticalLayer,compose,incremental::Incremental}};
use supra_window::{Size,cpu_content::{CpuCoordinator,Format,Layout,View,Snapshot}};

const W:u32=160;
const H:u32=96;

fn mapped(c:&mut CpuCoordinator,w:u32,h:u32,bytes:&[u8])->Snapshot {
    let client=c.connect().unwrap();
    let id=c.create(client,1).unwrap();
    let buffer=c.register_buffer(client,2).unwrap();
    c.configure(id,1,Size{width:w,height:h}).unwrap();
    c.acknowledge(client,1,1).unwrap();
    c.attach(client,1,Some(buffer)).unwrap();
    c.commit(client,1,Some(View{buffer,layout:Layout{width:w,height:h,stride:w as usize*4,offset:0,format:Format::Argb8888},bytes})).unwrap();
    c.snapshot(id).unwrap()
}

pub(crate) fn fixture_layers()->Vec<OpticalLayer> {
    let mut c=CpuCoordinator::new(262144);
    let background:Vec<u8>=(0..W*H).flat_map(|i| {
        let (x,y)=(i%W,i/W);
        if (x/10+y/10)%2==0 { [65,90,180,255] } else { [170,120,35,255] }
    }).collect();
    let glass=Glass{sigma:2.0,radius:14.0,corner:Corner::BezierG2,
        material:Material::new(10.0,8.0,1.0,[1.44,1.5,1.56],[0.8;3],0.08).unwrap(),reflection:[0.9;3]};
    let mut window=vec![0;84*54*4];
    for y in 18..21 {
        for x in 15..64 { window[(y*84+x)*4..(y*84+x)*4+4].copy_from_slice(&[220,220,220,255]); }
    }
    let front=vec![0;48*34*4];
    vec![
        OpticalLayer{placed:Placed{x:0,y:0,snapshot:mapped(&mut c,W,H,&background)},glass:None},
        OpticalLayer{placed:Placed{x:25,y:18,snapshot:mapped(&mut c,84,54,&window)},glass:Some(glass)},
        OpticalLayer{placed:Placed{x:86,y:45,snapshot:mapped(&mut c,48,34,&front)},glass:Some(glass)},
    ]
}

pub fn ppm()->Vec<u8> {
    let rgba=compose(W,H,&fixture_layers()).unwrap();
    let mut result=format!("P6\n{W} {H}\n255\n").into_bytes();
    for p in rgba { result.extend_from_slice(&p[..3]); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_lower_prefix_changes_only_covered_area_and_upper_layer_cannot_feed_back() {
        let mut layers=fixture_layers();
        let all=compose(W,H,&layers).unwrap();
        layers.pop();
        let lower=compose(W,H,&layers).unwrap();
        layers.pop();
        let bg=compose(W,H,&layers).unwrap();
        assert_eq!(all[0],bg[0]);
        assert_ne!(lower[40*W as usize+50],bg[40*W as usize+50]);
        for y in 0..H { for x in 0..W {
            if !(86..134).contains(&x) || !(45..79).contains(&y) {
                let i=(y*W+x) as usize;
                assert_eq!(all[i],lower[i]);
            }
        }}
        assert!((45..79).any(|y| (86..134).any(|x| {
            let i=(y*W+x) as usize; all[i]!=lower[i]
        })));
        assert_eq!(ppm(),ppm());
    }
}

// Clones retain the same immutable pixel allocation and revision. Positions
// change independently of the client commit, as a window manager would.
pub(crate) fn clone_layer(layer:&OpticalLayer)->OpticalLayer {
    let s=&layer.placed.snapshot;
    OpticalLayer { glass:layer.glass,placed:Placed { x:layer.placed.x,y:layer.placed.y,
        snapshot:Snapshot { window:s.window,generation:s.generation,state:s.state,content:s.content.clone() } } }
}

pub fn incremental_experiment()->String {
    let layers=fixture_layers();
    let mut cached=Incremental::new(W,H).unwrap();
    let mut prefix_pixels=0;
    let mut optical_pixels=0;
    let mut blur_passes=0;
    let mut full_pixels=0;
    let mut blur_samples=0;
    let mut full_plane_blur_samples=0;
    let mut decoded_channels=0;
    let mut full_plane_decoded_channels=0;
    let mut cow_copied_pixels=0;
    let mut full_prefix_clone_pixels=0;
    let mut published_pixels=0;
    for step in 0..24 {
        let mut next:Vec<_>=layers.iter().map(clone_layer).collect();
        next[1].placed.x += (step/3)%8;
        next[2].placed.y += (step/5)%5;
        if step>=20 { next.swap(1,2); }
        let expected=compose(W,H,&next).unwrap();
        let work=cached.update(next).unwrap();
        assert_eq!(cached.pixels(),expected, "frame {step}");
        prefix_pixels += work.prefix_pixels;
        optical_pixels += work.optical_pixels;
        blur_passes += work.blur_passes;
        blur_samples += work.blur_samples;
        full_plane_blur_samples += work.full_plane_blur_samples;
        decoded_channels += work.decoded_channels;
        full_plane_decoded_channels += work.full_plane_decoded_channels;
        cow_copied_pixels += work.cow_copied_pixels;
        full_prefix_clone_pixels += work.full_prefix_clone_pixels;
        published_pixels += work.published_pixels;
        full_pixels += (W*H*3) as usize;
    }
    assert!(blur_samples < full_plane_blur_samples);
    assert!(decoded_channels < full_plane_decoded_channels);
    assert!(cow_copied_pixels < full_prefix_clone_pixels);
    format!("{{\"schema_version\":4,\"frames\":24,\"pixel_exact\":true,\"incremental_prefix_pixels\":{prefix_pixels},\"full_prefix_pixels\":{full_pixels},\"optical_pixels\":{optical_pixels},\"blur_requests\":{blur_passes},\"cropped_blur_samples\":{blur_samples},\"full_plane_blur_samples\":{full_plane_blur_samples},\"decoded_channels\":{decoded_channels},\"full_plane_decoded_channels\":{full_plane_decoded_channels},\"cow_copied_pixels\":{cow_copied_pixels},\"full_prefix_clone_pixels\":{full_prefix_clone_pixels},\"published_pixels\":{published_pixels}}}\n")
}

#[cfg(test)]
mod incremental_tests {
    use super::*;
    #[test]
    fn movement_and_reorder_match_full_render() { assert!(incremental_experiment().contains("\"pixel_exact\":true")); }

    #[test]
    fn unchanged_and_upper_only_skip_lower_prefixes_and_invalid_update_preserves_frame() {
        let layers=fixture_layers();
        let mut c=Incremental::new(W,H).unwrap();
        c.update(layers.iter().map(clone_layer).collect()).unwrap();
        let zero=c.update(layers.iter().map(clone_layer).collect()).unwrap();
        assert_eq!(zero.prefix_pixels,0);
        assert_eq!(zero.blur_passes,0);
        assert_eq!(zero.cow_copied_pixels,0);
        assert_eq!(zero.published_pixels,0);
        let mut moved:Vec<_>=layers.iter().map(clone_layer).collect();
        moved[2].placed.x+=1;
        let expected=compose(W,H,&moved).unwrap();
        let work=c.update(moved).unwrap();
        assert_eq!(c.pixels(),expected);
        assert_eq!(work.blur_passes,1);
        assert!(work.prefix_pixels < (W*H) as usize);
        let mut invalid:Vec<_>=layers.iter().map(clone_layer).collect();
        invalid[1].glass.as_mut().unwrap().sigma=f64::NAN;
        assert!(c.update(invalid).is_err());
        assert_eq!(c.pixels(),expected);
        let mut changed:Vec<_>=layers.iter().map(clone_layer).collect();
        changed[1].glass.as_mut().unwrap().sigma=3.0;
        let expected=compose(W,H,&changed).unwrap();
        c.update(changed).unwrap();
        assert_eq!(c.pixels(),expected);
        c.update(vec![]).unwrap();
        assert!(c.pixels().iter().all(|p| *p==[0,0,0,255]));
    }
}
