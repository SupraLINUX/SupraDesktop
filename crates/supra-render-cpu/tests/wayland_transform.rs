//! Independent client-side buffer transform oracle.
//! Wayland core wl_surface.set_buffer_transform requires the compositor
//! to undo the client transform; wl_output.transform flips before CCW rotation.
//! This fixture constructs buffers by rotating rows, not by copying the
//! renderer's surface-to-buffer coordinate expressions.
use supra_render_cpu::{Frame,Placed};
use supra_window::{Size,cpu_content::{CpuCoordinator,Format,Layout,View},surface::Transform};

fn ccw(rows:Vec<Vec<u8>>)->Vec<Vec<u8>> {
    let width=rows[0].len();
    (0..width).rev().map(|column|rows.iter().map(|row|row[column]).collect()).collect()
}

fn client_buffer(scale:usize,flip:bool,turns:usize)->Vec<Vec<u8>> {
    let mut rows=Vec::new();
    for row in [[11,22,33],[44,55,66]] {
        let expanded:Vec<_>=row.into_iter().flat_map(|p|std::iter::repeat_n(p,scale)).collect();
        for _ in 0..scale { rows.push(expanded.clone()); }
    }
    if flip { for row in &mut rows { row.reverse(); } }
    for _ in 0..turns { rows=ccw(rows); }
    rows
}

#[test]
fn inverse_client_transform_and_scale_round_trip() {
    let cases=[
        (Transform::Normal,false,0),
        (Transform::Rotate90,false,1),
        (Transform::Rotate180,false,2),
        (Transform::Rotate270,false,3),
        (Transform::Flipped,true,0),
        (Transform::Flipped90,true,1),
        (Transform::Flipped180,true,2),
        (Transform::Flipped270,true,3),
    ];
    for scale in [1,2,3] {
        for (transform,flip,turns) in cases {
            let rows=client_buffer(scale,flip,turns);
            let (width,height)=(rows[0].len() as u32,rows.len() as u32);
            // Include a distinct offset and row padding to exercise import.
            let stride=width as usize*4+8;
            let offset=4;
            let mut data=vec![0xA5;offset+stride*height as usize];
            for (y,row) in rows.iter().enumerate() {
                for (x,&red) in row.iter().enumerate() {
                    let i=offset+y*stride+x*4;
                    data[i..i+4].copy_from_slice(&[0,0,red,0]);
                }
            }
            let mut c=CpuCoordinator::new(4096);
            let client=c.connect().unwrap();
            let id=c.create(client,1).unwrap();
            let buffer=c.register_buffer(client,2).unwrap();
            c.configure(id,1,Size{width:3,height:2}).unwrap();
            c.acknowledge(client,1,1).unwrap();
            c.set_transform(client,1,transform).unwrap();
            c.set_scale(client,1,scale as i32).unwrap();
            c.attach(client,1,Some(buffer)).unwrap();
            c.commit(client,1,Some(View{buffer,layout:Layout{width,height,stride,offset,format:Format::Xrgb8888},bytes:&data})).unwrap();
            let snapshot=c.snapshot(id).unwrap();
            assert_eq!(snapshot.content.layout().logical_size(snapshot.state).unwrap(),Size{width:3,height:2});
            let mut frame=Frame::new(3,2,6).unwrap();
            frame.update(vec![Placed{x:0,y:0,snapshot}]).unwrap();
            assert_eq!(frame.pixels(),&[[11,0,0,255],[22,0,0,255],[33,0,0,255],
                [44,0,0,255],[55,0,0,255],[66,0,0,255]],"{transform:?}, scale {scale}");
        }
    }
}

#[test]
fn client_ccw_direction_is_explicit() {
    assert_eq!(client_buffer(1,false,1),vec![vec![33,66],vec![22,55],vec![11,44]]);
    assert_eq!(client_buffer(1,true,1),vec![vec![11,44],vec![22,55],vec![33,66]]);
}
