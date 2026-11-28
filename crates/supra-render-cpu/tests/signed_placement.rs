use supra_render_cpu::{Error,Frame,SignedPlaced};
use supra_window::{Size,WindowId,cpu_content::{CpuCoordinator,Format,Layout,View},surface::Transform};

fn mapped(c:&mut CpuCoordinator,transform:Transform)->WindowId {
    let client=c.connect().unwrap();
    let id=c.create(client,1).unwrap();
    let buffer=c.register_buffer(client,2).unwrap();
    c.configure(id,1,Size{width:0,height:0}).unwrap();
    c.acknowledge(client,1,1).unwrap();
    c.set_transform(client,1,transform).unwrap();
    c.attach(client,1,Some(buffer)).unwrap();
    let bytes:Vec<u8>=(1..=12).flat_map(|v|[0,0,v,0]).collect();
    c.commit(client,1,Some(View{buffer,layout:Layout{width:4,height:3,stride:16,offset:0,format:Format::Xrgb8888},bytes:&bytes})).unwrap();
    id
}
fn at(c:&CpuCoordinator,id:WindowId,x:i64,y:i64)->SignedPlaced {
    SignedPlaced{x,y,snapshot:c.snapshot(id).unwrap()}
}
fn red(frame:&Frame)->Vec<u8> { frame.pixels().iter().map(|p|p[0]).collect() }

#[test]
fn clipping_keeps_origin_and_movement_with_identical_visible_bounds_invalidates() {
    let mut c=CpuCoordinator::new(128);
    let id=mapped(&mut c,Transform::Normal);
    let mut frame=Frame::new(2,2,4).unwrap();
    frame.update_signed(vec![at(&c,id,-1,-1)]).unwrap();
    assert_eq!(red(&frame),vec![6,7,10,11]);
    assert_eq!(frame.update_signed(vec![at(&c,id,-1,-1)]).unwrap(),0);
    frame.update_signed(vec![at(&c,id,-2,-1)]).unwrap();
    assert_eq!(red(&frame),vec![7,8,11,12]);
    frame.update_signed(vec![at(&c,id,1,1)]).unwrap();
    assert_eq!(red(&frame),vec![0,0,0,1]);
    frame.update_signed(vec![at(&c,id,-4,0)]).unwrap();
    assert_eq!(red(&frame),vec![0;4]);
    assert_eq!(frame.update_signed(vec![at(&c,id,i64::MIN,0)]).unwrap(),0);
}

#[test]
fn rotated_buffer_is_sampled_from_the_unclipped_logical_origin() {
    let mut c=CpuCoordinator::new(128);
    let id=mapped(&mut c,Transform::Rotate90);
    let mut frame=Frame::new(2,2,4).unwrap();
    frame.update_signed(vec![at(&c,id,-1,-1)]).unwrap();
    assert_eq!(red(&frame),vec![6,2,7,3]);
}

#[test]
fn invalid_and_duplicate_even_offscreen_inputs_preserve_published_frame() {
    let mut c=CpuCoordinator::new(128);
    let id=mapped(&mut c,Transform::Normal);
    let mut frame=Frame::new(2,2,4).unwrap();
    frame.update_signed(vec![at(&c,id,0,0)]).unwrap();
    let before=red(&frame);
    assert_eq!(frame.update_signed(vec![at(&c,id,i64::MAX,0)]),Err(Error::InvalidSurface));
    assert_eq!(frame.update_signed(vec![at(&c,id,-20,0),at(&c,id,20,0)]),Err(Error::DuplicateWindow));
    assert_eq!(red(&frame),before);
    let mut invalid=at(&c,id,-20,0);
    invalid.snapshot.generation=0;
    assert_eq!(frame.update_signed(vec![invalid]),Err(Error::InvalidSurface));
    assert_eq!(red(&frame),before);
}
