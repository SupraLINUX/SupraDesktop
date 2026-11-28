//! Test-only bridge from our CPU optical presenter to an external Wayland
//! client helper. No host-display access from the library or runtime dependency.
use std::{io::{self,BufRead,BufReader,Write},path::Path,process::{Command,Stdio}};
use supra_geometry::Corner;
use supra_glass::material::Material;
use supra_render_cpu::presentation::{OpticalPlacement,OpticalPresentation,Placement};
use supra_render_cpu::optical::Glass;
use supra_window::{Size,WindowId,clients::ClientId,cpu_content::{CpuCoordinator,Format,Layout,View}};

const W:u32=160;
const H:u32=96;

fn mapped(c:&mut CpuCoordinator,w:u32,h:u32,bytes:&[u8],callback:u64)->(ClientId,WindowId) {
    let client=c.connect().unwrap();
    let id=c.create(client,1).unwrap();
    let buffer=c.register_buffer(client,2).unwrap();
    c.configure(id,1,Size{width:w,height:h}).unwrap();
    c.acknowledge(client,1,1).unwrap();
    c.frame(client,1,callback).unwrap();
    c.attach(client,1,Some(buffer)).unwrap();
    c.commit(client,1,Some(View{buffer,layout:Layout{width:w,height:h,stride:w as usize*4,
        offset:0,format:Format::Argb8888},bytes})).unwrap();
    (client,id)
}
fn placement(window:WindowId,x:i64,y:i64,glass:Option<Glass>)->OpticalPlacement {
    OpticalPlacement{placement:Placement{window,x,y,callback_eligible:true},glass}
}
fn expect_line(reader:&mut impl BufRead,expected:&str)->io::Result<()> {
    let mut line=String::new();
    reader.read_line(&mut line)?;
    if line.trim_end()!=expected { return Err(io::Error::other(format!("Expected {expected}, received {line:?}"))); }
    Ok(())
}

fn run_frames(input:&mut impl Write,output:&mut impl BufRead,dir:&Path)->io::Result<()> {
    let mut c=CpuCoordinator::new(262144);
    let background:Vec<u8>=(0..W*H).flat_map(|i| {
        if (i%W/10+i/W/10)%2==0 {[65,90,180,255]} else {[170,120,35,255]}
    }).collect();
    let (_,bg)=mapped(&mut c,W,H,&background,10);
    let (client,pane)=mapped(&mut c,84,54,&vec![0;84*54*4],20);
    let glass=Glass{sigma:2.0,radius:14.0,corner:Corner::BezierG2,
        material:Material::new(10.0,8.0,1.0,[1.44,1.5,1.56],[0.8;3],0.08).unwrap(),reflection:[0.9;3]};
    let mut p=OpticalPresentation::new(&c,W,H).unwrap();
    let mut previous=Vec::new();
    for n in 1..=2 {
        if n==2 {
            c.frame(client,1,21).unwrap();
            c.commit(client,1,None).unwrap();
        }
        let x=if n==1 {25} else {-8};
        let scene=[placement(bg,0,0,None),placement(pane,x,18,Some(glass))];
        let frame=p.submit(&c,&scene).unwrap();
        let pixels=p.pixels(&frame.token).unwrap();
        assert!(frame.work.optical_pixels>0);
        let rgba:Vec<u8>=pixels.iter().flatten().copied().collect();
        assert!(rgba.chunks_exact(4).all(|p|p[3]==255));
        if n==2 { assert_ne!(rgba,previous); }
        let mut ppm=format!("P6\n{W} {H}\n255\n").into_bytes();
        for p in pixels { ppm.extend_from_slice(&p[..3]); }
        std::fs::write(dir.join(format!("nested-frame-{n}.ppm")),ppm)?;
        input.write_all(&rgba)?;
        input.flush()?;
        // A real parent wl_surface.frame callback is a drawing boundary,
        // not proof of physical scanout. No synthetic completion on error.
        expect_line(output,&format!("FRAME {n}"))?;
        let callbacks=p.complete(&mut c,&frame.token).unwrap();
        let expected=if n==1 {vec![(bg,vec![10]),(pane,vec![20])]} else {vec![(pane,vec![21])]};
        assert_eq!(callbacks,expected);
        println!("SUPRA_NESTED_FRAME_{n}_COMPLETE");
        // Optional isolated readback experiment: keep the same surface
        // committed until the external capture process has exited.
        if let Some(capture)=std::env::var_os("SUPRA_LAB_CAPTURE_SCRIPT") {
            let status=Command::new("bash").arg(capture).arg(dir).arg(n.to_string())
                .env_remove("WAYLAND_DEBUG").stdin(Stdio::null())
                .stdout(Stdio::null()).stderr(Stdio::inherit()).status()?;
            if !status.success() { return Err(io::Error::other(format!("Host capture failed: {status}"))); }
        }
        input.write_all(b"A")?;
        input.flush()?;
        previous=rgba;
    }
    expect_line(output,"RELEASE 2")?;
    println!("SUPRA_NESTED_BUFFERS_RELEASED");
    Ok(())
}

pub fn run(helper:&Path,dir:&Path)->io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut child=Command::new(helper).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
    let mut input=child.stdin.take().expect("piped stdin");
    let mut output=BufReader::new(child.stdout.take().expect("piped stdout"));
    let result=run_frames(&mut input,&mut output,dir);
    drop(input);
    if result.is_err() { let _=child.kill(); }
    let status=child.wait()?;
    result?;
    if !status.success() { return Err(io::Error::other(format!("Wayland helper exited {status}"))); }
    Ok(())
}
