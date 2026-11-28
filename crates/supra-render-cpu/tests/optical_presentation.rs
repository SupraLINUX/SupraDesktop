use supra_geometry::Corner;
use supra_glass::material::Material;
use supra_render_cpu::{SignedPlaced,optical::{Glass,SignedOpticalLayer,compose_signed},
    presentation::{CpuPresentation,OpticalPresentation,OpticalPlacement,Placement,Error}};
use supra_window::{Size,WindowId,clients::ClientId,buffers::BufferId,cpu_content::{CpuCoordinator,Format,Layout,View}};

const SIZE:Size=Size{width:12,height:12};
const LAYOUT:Layout=Layout{width:12,height:12,stride:48,offset:0,format:Format::Argb8888};

fn mapped(c:&mut CpuCoordinator,rgba:[u8;4],callback:u64)->(ClientId,WindowId,BufferId) {
    let client=c.connect().unwrap();
    let id=c.create(client,1).unwrap();
    let buffer=c.register_buffer(client,2).unwrap();
    c.configure(id,1,SIZE).unwrap();
    c.acknowledge(client,1,1).unwrap();
    c.frame(client,1,callback).unwrap();
    c.attach(client,1,Some(buffer)).unwrap();
    let bytes=rgba.repeat(144);
    c.commit(client,1,Some(View{buffer,layout:LAYOUT,bytes:&bytes})).unwrap();
    (client,id,buffer)
}
fn glass()->Glass {
    Glass{sigma:2.0,radius:4.0,corner:Corner::BezierG2,
        material:Material::new(6.0,8.0,1.0,[1.44,1.5,1.56],[0.8;3],0.08).unwrap(),reflection:[0.9;3]}
}
fn at(window:WindowId,x:i64,glass:Option<Glass>)->OpticalPlacement {
    OpticalPlacement{placement:Placement{window,x,y:0,callback_eligible:true},glass}
}
fn reference(c:&CpuCoordinator,scene:&[OpticalPlacement])->Vec<[u8;4]> {
    let layers:Vec<_>=scene.iter().map(|p|SignedOpticalLayer{placed:SignedPlaced{
        x:p.placement.x,y:p.placement.y,snapshot:c.snapshot(p.placement.window).unwrap()},glass:p.glass}).collect();
    compose_signed(12,12,&layers).unwrap()
}

#[test]
fn optical_submission_is_exact_stable_and_generation_bound() {
    let mut c=CpuCoordinator::new(4096);
    let (_,bg,_)=mapped(&mut c,[60,110,180,255],10);
    let (client,pane,buffer)=mapped(&mut c,[0;4],20);
    let scene=[at(bg,0,None),at(pane,-3,Some(glass()))];
    let expected=reference(&c,&scene);
    let mut p=OpticalPresentation::new(&c,12,12).unwrap();
    let first=p.submit(&c,&scene).unwrap();
    assert_eq!(p.pixels(&first.token).unwrap(),expected.as_slice());
    assert!(first.work.optical_pixels>0);
    c.frame(client,1,21).unwrap();
    c.attach(client,1,Some(buffer)).unwrap();
    c.commit(client,1,Some(View{buffer,layout:LAYOUT,bytes:&[0,128,0,128].repeat(144)})).unwrap();
    assert!(matches!(p.submit(&c,&scene),Err(Error::Busy)));
    assert_eq!(p.pixels(&first.token).unwrap(),expected.as_slice());
    assert_eq!(p.complete(&mut c,&first.token).unwrap(),vec![(bg,vec![10]),(pane,vec![20])]);
    let second=p.submit(&c,&scene).unwrap();
    assert_eq!(p.pixels(&second.token).unwrap(),reference(&c,&scene).as_slice());
    assert_eq!(p.complete(&mut c,&second.token).unwrap(),vec![(pane,vec![21])]);
}

#[test]
fn optical_invalid_discard_offscreen_and_cross_backend_tokens_preserve_callbacks() {
    let mut c=CpuCoordinator::new(4096);
    let (_,id,_)=mapped(&mut c,[0;4],10);
    let mut p=OpticalPresentation::new(&c,12,12).unwrap();
    let mut ordinary=CpuPresentation::new(&c,12,12,144).unwrap();
    let ordinary_frame=ordinary.submit(&c,&[at(id,0,None).placement]).unwrap();
    let mut invalid=glass();
    invalid.sigma=f64::NAN;
    assert!(matches!(p.submit(&c,&[at(id,0,Some(invalid))]),Err(Error::Render(_))));
    let off=p.submit(&c,&[at(id,-12,Some(glass()))]).unwrap();
    assert!(p.complete(&mut c,&off.token).unwrap().is_empty());
    let a=p.submit(&c,&[at(id,0,Some(glass()))]).unwrap();
    assert_eq!(p.complete(&mut c,&ordinary_frame.token),Err(Error::UnknownFrame));
    ordinary.discard(&ordinary_frame.token).unwrap();
    p.discard(&a.token).unwrap();
    let b=p.submit(&c,&[at(id,0,Some(glass()))]).unwrap();
    assert_eq!(b.work.prefix_pixels,0);
    assert_eq!(p.complete(&mut c,&a.token),Err(Error::UnknownFrame));
    assert_eq!(p.complete(&mut c,&b.token).unwrap(),vec![(id,vec![10])]);
}
