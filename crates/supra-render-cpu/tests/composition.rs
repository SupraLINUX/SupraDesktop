use supra_render_cpu::{Error, Frame, Placed};
use supra_window::{Size, cpu_content::{CpuCoordinator, Format, Layout, View}, surface::Transform};

fn mapped(c: &mut CpuCoordinator, width: u32, height: u32, format: Format, data: &[u8], transform: Transform) -> supra_window::cpu_content::Snapshot {
    let client = c.connect().unwrap();
    let id = c.create(client,1).unwrap();
    let buffer = c.register_buffer(client,2).unwrap();
    c.configure(id,1,Size { width:0,height:0 }).unwrap();
    c.acknowledge(client,1,1).unwrap();
    c.set_transform(client,1,transform).unwrap();
    c.attach(client,1,Some(buffer)).unwrap();
    c.commit(client,1,Some(View { buffer,layout:Layout { width,height,stride:width as usize*4,offset:0,format },bytes:data })).unwrap();
    c.snapshot(id).unwrap()
}

#[test]
fn real_retained_pixels_are_composited_with_premultiplied_alpha_and_clipping() {
    let mut c = CpuCoordinator::new(128);
    let a = mapped(&mut c,2,1,Format::Xrgb8888,&[0,0,255,0, 0,255,0,0],Transform::Normal);
    let b = mapped(&mut c,2,1,Format::Argb8888,&[128,0,0,128, 0,0,0,0],Transform::Normal);
    let mut frame = Frame::new(2,1,2).unwrap();
    assert_eq!(frame.update(vec![Placed { x:0,y:0,snapshot:a },Placed { x:1,y:0,snapshot:b }]).unwrap(),2);
    assert_eq!(frame.pixels(),&[[255,0,0,255],[0,127,128,255]]);
}

#[test]
fn all_eight_transforms_use_known_asymmetric_pixel_positions() {
    let cases = [
        (Transform::Normal,3,2,vec![1,2,3,4,5,6]),
        (Transform::Rotate90,2,3,vec![4,1,5,2,6,3]),
        (Transform::Rotate180,3,2,vec![6,5,4,3,2,1]),
        (Transform::Rotate270,2,3,vec![3,6,2,5,1,4]),
        (Transform::Flipped,3,2,vec![3,2,1,6,5,4]),
        (Transform::Flipped90,2,3,vec![1,4,2,5,3,6]),
        (Transform::Flipped180,3,2,vec![4,5,6,1,2,3]),
        (Transform::Flipped270,2,3,vec![6,3,5,2,4,1]),
    ];
    let data: Vec<u8> = (1..=6).flat_map(|v| [0,0,v,255]).collect();
    for (transform,w,h,expected) in cases {
        let mut c = CpuCoordinator::new(128);
        let snapshot = mapped(&mut c,3,2,Format::Xrgb8888,&data,transform);
        let mut frame = Frame::new(w,h,6).unwrap();
        frame.update(vec![Placed { x:0,y:0,snapshot }]).unwrap();
        assert_eq!(frame.pixels().iter().map(|p| p[0]).collect::<Vec<_>>(),expected);
    }
}

#[test]
fn damage_move_and_remove_match_fresh_render_and_unchanged_costs_zero() {
    let mut c = CpuCoordinator::new(128);
    let snapshot = mapped(&mut c,2,1,Format::Xrgb8888,&[0,0,255,0, 0,255,0,0],Transform::Normal);
    let id = snapshot.window;
    let mut incremental = Frame::new(8,4,32).unwrap();
    incremental.update(vec![Placed { x:0,y:0,snapshot }]).unwrap();
    assert_eq!(incremental.update(vec![Placed { x:0,y:0,snapshot:c.snapshot(id).unwrap() }]).unwrap(),0);
    assert_eq!(incremental.update(vec![Placed { x:2,y:1,snapshot:c.snapshot(id).unwrap() }]).unwrap(),4);
    let mut fresh = Frame::new(8,4,32).unwrap();
    fresh.update(vec![Placed { x:2,y:1,snapshot:c.snapshot(id).unwrap() }]).unwrap();
    assert_eq!(incremental.pixels(),fresh.pixels());
    let old = incremental.pixels().to_vec();
    assert_eq!(incremental.update(vec![Placed { x:0,y:0,snapshot:c.snapshot(id).unwrap() },Placed { x:2,y:1,snapshot:c.snapshot(id).unwrap() }]),Err(Error::DuplicateWindow));
    assert_eq!(incremental.pixels(),old);
    incremental.update(vec![]).unwrap();
    assert!(incremental.pixels().iter().all(|p| *p == [0,0,0,255]));
}
