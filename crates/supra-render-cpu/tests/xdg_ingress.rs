use supra_window::{Size,cpu_content::{Format,Layout,View,ingress::{Error,XdgIngress}}};
use supra_render_cpu::presentation::{CpuPresentation,Placement,Error as PresentError};

const SIZE:Size=Size{width:2,height:2};
const LAYOUT:Layout=Layout{width:2,height:2,stride:8,offset:0,format:Format::Argb8888};
const PIXELS:[u8;16]=[255;16];

#[test]
fn initial_commit_ack_and_remap_are_required() {
    let mut c=XdgIngress::new(4096);
    let client=c.connect().unwrap();
    let id=c.create_toplevel(client,1).unwrap();
    let b=c.register_buffer(client,2).unwrap();
    assert_eq!(c.configure(id,1,SIZE),Err(Error::InitialCommitRequired));
    assert_eq!(c.attach(client,1,Some(b)),Err(Error::ConfigureRequired));
    assert_eq!(c.acknowledge(client,1,1),Err(Error::ConfigureRequired));
    c.commit(client,1,None).unwrap();
    assert_eq!(c.attach(client,1,Some(b)),Err(Error::ConfigureRequired));
    c.configure(id,1,SIZE).unwrap();
    c.attach(client,1,Some(b)).unwrap();
    assert!(matches!(c.commit(client,1,Some(View{buffer:b,layout:LAYOUT,bytes:&PIXELS})),Err(Error::Core(_))));
    assert_eq!(c.coordinator().used_bytes(),0);
    c.acknowledge(client,1,1).unwrap();
    c.commit(client,1,Some(View{buffer:b,layout:LAYOUT,bytes:&PIXELS})).unwrap();
    assert!(c.coordinator().snapshot(id).is_some());
    c.attach(client,1,None).unwrap();
    c.commit(client,1,None).unwrap();
    assert!(c.coordinator().snapshot(id).is_none());
    assert_eq!(c.configure(id,2,SIZE),Err(Error::InitialCommitRequired));
    assert_eq!(c.attach(client,1,Some(b)),Err(Error::ConfigureRequired));
    c.commit(client,1,None).unwrap();
    c.configure(id,2,SIZE).unwrap();
    assert!(c.acknowledge(client,1,1).is_err());
    c.acknowledge(client,1,2).unwrap();
    c.attach(client,1,Some(b)).unwrap();
    c.commit(client,1,Some(View{buffer:b,layout:LAYOUT,bytes:&PIXELS})).unwrap();
    assert!(c.coordinator().snapshot(id).is_some());
}

#[test]
fn separate_clients_and_frame_boundary_survive_destroy_and_reuse() {
    let mut c=XdgIngress::new(4096);
    let a=c.connect().unwrap();
    let b=c.connect().unwrap();
    let wa=c.create_toplevel(a,1).unwrap();
    let wb=c.create_toplevel(b,1).unwrap();
    c.commit(a,1,None).unwrap();
    c.configure(wa,1,SIZE).unwrap();
    assert_eq!(c.acknowledge(b,1,1),Err(Error::ConfigureRequired));
    c.acknowledge(a,1,1).unwrap();
    let buffer=c.register_buffer(a,2).unwrap();
    c.attach(a,1,Some(buffer)).unwrap();
    c.frame(a,1,101).unwrap();
    c.commit(a,1,Some(View{buffer,layout:LAYOUT,bytes:&PIXELS})).unwrap();
    let mut p=CpuPresentation::new(c.coordinator(),2,2,4).unwrap();
    let f=p.submit(c.coordinator(),&[Placement{window:wa,x:0,y:0,callback_eligible:true}]).unwrap();
    let mut other=XdgIngress::new(4096);
    assert_eq!(p.complete_ingress(&mut other,&f.token),Err(PresentError::WrongCoordinator));
    assert_eq!(p.complete_ingress(&mut c,&f.token).unwrap(),vec![(wa,vec![101])]);
    c.frame(a,1,102).unwrap();
    c.commit(a,1,None).unwrap();
    let pending=p.submit(c.coordinator(),&[Placement{window:wa,x:0,y:0,callback_eligible:true}]).unwrap();
    assert_eq!(c.disconnect(a).unwrap().windows,vec![wa]);
    assert!(p.complete_ingress(&mut c,&pending.token).unwrap().is_empty());
    assert_eq!(c.configure(wb,1,SIZE),Err(Error::InitialCommitRequired));
    c.destroy(b,1).unwrap();
    let replacement=c.create_toplevel(b,1).unwrap();
    assert_ne!(replacement,wb);
    assert_eq!(c.configure(replacement,1,SIZE),Err(Error::InitialCommitRequired));
}
