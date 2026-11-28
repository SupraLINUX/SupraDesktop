use supra_geometry::Corner;
use supra_glass::material::Material;
use supra_render_cpu::{SignedPlaced,Error,optical::{Glass,SignedOpticalLayer,compose_signed,incremental::Incremental}};
use supra_window::{Size,WindowId,cpu_content::{CpuCoordinator,Format,Layout,View}};

fn mapped(c:&mut CpuCoordinator,w:u32,h:u32,rgba:[u8;4])->WindowId {
    let client=c.connect().unwrap();
    let id=c.create(client,1).unwrap();
    let buffer=c.register_buffer(client,2).unwrap();
    c.configure(id,1,Size{width:w,height:h}).unwrap();
    c.acknowledge(client,1,1).unwrap();
    c.attach(client,1,Some(buffer)).unwrap();
    let bytes:Vec<_>=(0..w*h).flat_map(|_|rgba).collect();
    c.commit(client,1,Some(View{buffer,layout:Layout{width:w,height:h,stride:w as usize*4,offset:0,format:Format::Argb8888},bytes:&bytes})).unwrap();
    id
}
fn glass()->Glass {
    Glass{sigma:2.0,radius:6.0,corner:Corner::BezierG2,
        material:Material::new(6.0,8.0,1.0,[1.44,1.5,1.56],[0.8;3],0.08).unwrap(),reflection:[0.9;3]}
}
fn layer(c:&CpuCoordinator,id:WindowId,x:i64,y:i64,glass:Option<Glass>)->SignedOpticalLayer {
    SignedOpticalLayer{placed:SignedPlaced{x,y,snapshot:c.snapshot(id).unwrap()},glass}
}

#[test]
fn clipped_material_keeps_full_shape_against_translated_uniform_background() {
    let mut c=CpuCoordinator::new(32768);
    let bg=mapped(&mut c,40,40,[60,110,180,255]);
    let pane=mapped(&mut c,20,18,[0;4]);
    // Uniform lower prefix makes output-boundary clamping irrelevant.
    let full=compose_signed(40,40,&[layer(&c,bg,0,0,None),layer(&c,pane,4,5,Some(glass()))]).unwrap();
    let cropped=compose_signed(24,24,&[layer(&c,bg,0,0,None),layer(&c,pane,-4,-3,Some(glass()))]).unwrap();
    for y in 0..24 { for x in 0..24 {
        assert_eq!(cropped[y*24+x],full[(y+8)*40+x+8],"pixel {x},{y}");
    }}
    assert!(cropped.iter().any(|p|*p!=[180,110,60,255]));
}

#[test]
fn incremental_signed_glass_matches_full_reference_and_rejects_invalid_offscreen() {
    let mut c=CpuCoordinator::new(32768);
    let bg=mapped(&mut c,24,24,[60,110,180,255]);
    let pane=mapped(&mut c,32,28,[0;4]);
    let front=mapped(&mut c,10,10,[0;4]);
    let mut renderer=Incremental::new(24,24).unwrap();
    for (x,y) in [(-1,-1),(-2,-2),(-3,-3),(6,6),(-32,0),(-2,-2)] {
        let layers=||vec![layer(&c,bg,0,0,None),layer(&c,pane,x,y,Some(glass())),layer(&c,front,-3,4,Some(glass()))];
        let expected=compose_signed(24,24,&layers()).unwrap();
        renderer.update_signed(layers()).unwrap();
        assert_eq!(renderer.pixels(),expected.as_slice());
        assert_eq!(renderer.update_signed(layers()).unwrap().prefix_pixels,0);
    }
    let before=renderer.pixels().to_vec();
    let mut invalid=glass();
    invalid.sigma=f64::NAN;
    assert_eq!(renderer.update_signed(vec![layer(&c,pane,-100,0,Some(invalid))]),Err(Error::InvalidSurface));
    assert_eq!(renderer.update_signed(vec![layer(&c,pane,-100,0,None),layer(&c,pane,0,0,None)]),Err(Error::DuplicateWindow));
    assert_eq!(renderer.pixels(),before.as_slice());
}
