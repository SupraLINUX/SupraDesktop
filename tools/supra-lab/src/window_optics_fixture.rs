//! Original deterministic optical composition of real retained window bytes.
use supra_geometry::Corner;
use supra_glass::material::Material;
use supra_render_cpu::{Placed,optical::{Glass,OpticalLayer,compose}};
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

fn fixture_layers()->Vec<OpticalLayer> {
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
