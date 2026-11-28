use supra_render_cpu::presentation::{CpuPresentation,Error,Placement};
use supra_window::{WindowId,Size,buffers::{BufferId,Completion},clients::ClientId,cpu_content::{CpuCoordinator,Format,Layout,View}};

const SIZE:Size=Size{width:2,height:1};
const LAYOUT:Layout=Layout{width:2,height:1,stride:8,offset:0,format:Format::Xrgb8888};
const RED:[u8;8]=[0,0,255,0,0,0,255,0];

fn mapped(c:&mut CpuCoordinator,callback:u64)->(ClientId,WindowId,BufferId) {
    let client=c.connect().unwrap();
    let id=c.create(client,1).unwrap();
    let buffer=c.register_buffer(client,2).unwrap();
    c.configure(id,1,SIZE).unwrap();
    c.acknowledge(client,1,1).unwrap();
    c.frame(client,1,callback).unwrap();
    c.attach(client,1,Some(buffer)).unwrap();
    let commit=c.commit(client,1,Some(View{buffer,layout:LAYOUT,bytes:&RED})).unwrap();
    assert!(matches!(commit.release,Some(Completion::Release{..})));
    (client,id,buffer)
}

fn at(window:WindowId,x:i64,eligible:bool)->Placement {
    Placement{window,x,y:0,callback_eligible:eligible}
}

#[test]
fn stable_inflight_pixels_and_generation_specific_callback_completion() {
    let mut c=CpuCoordinator::new(128);
    let (client,id,buffer)=mapped(&mut c,10);
    let mut p=CpuPresentation::new(&c,2,1,2).unwrap();
    let first=p.submit(&c,&[at(id,0,true)]).unwrap();
    assert_eq!(p.pixels(&first.token).unwrap(),&[[255,0,0,255];2]);
    c.frame(client,1,11).unwrap();
    c.attach(client,1,Some(buffer)).unwrap();
    c.commit(client,1,Some(View{buffer,layout:LAYOUT,bytes:&[0,255,0,0,0,255,0,0]})).unwrap();
    assert!(matches!(p.submit(&c,&[at(id,0,true)]),Err(Error::Busy)));
    assert_eq!(p.pixels(&first.token).unwrap(),&[[255,0,0,255];2]);
    assert_eq!(p.complete(&mut c,&first.token).unwrap(),vec![(id,vec![10])]);
    assert_eq!(p.complete(&mut c,&first.token),Err(Error::UnknownFrame));
    let second=p.submit(&c,&[at(id,0,true)]).unwrap();
    assert_eq!(p.pixels(&second.token).unwrap(),&[[0,255,0,255];2]);
    assert_eq!(p.complete(&mut c,&second.token).unwrap(),vec![(id,vec![11])]);
}

#[test]
fn discarded_offscreen_occluded_and_wrong_session_never_signal() {
    let mut c=CpuCoordinator::new(128);
    let (_,id,_)=mapped(&mut c,10);
    let mut other=CpuCoordinator::new(128);
    let (_,other_id,_)=mapped(&mut other,20);
    assert_eq!(id,other_id); // numeric IDs alone are not session identities
    let mut p=CpuPresentation::new(&c,2,1,2).unwrap();
    assert!(matches!(p.submit(&other,&[at(id,0,true)]),Err(Error::WrongCoordinator)));
    let off=p.submit(&c,&[at(id,2,true)]).unwrap();
    assert!(p.complete(&mut c,&off.token).unwrap().is_empty());
    let hidden=p.submit(&c,&[at(id,0,false)]).unwrap();
    assert!(p.complete(&mut c,&hidden.token).unwrap().is_empty());
    let discarded=p.submit(&c,&[at(id,0,true)]).unwrap();
    p.discard(&discarded.token).unwrap();
    assert!(p.pixels(&discarded.token).is_err());
    let accepted=p.submit(&c,&[at(id,0,true)]).unwrap();
    assert_eq!(accepted.recomposited_pixels,0);
    assert_eq!(p.complete(&mut other,&accepted.token),Err(Error::WrongCoordinator));
    assert_eq!(p.complete(&mut c,&discarded.token),Err(Error::UnknownFrame));
    assert_eq!(p.complete(&mut c,&accepted.token).unwrap(),vec![(id,vec![10])]);
    assert_eq!(other.frame_ready(other_id,1,true).unwrap(),vec![20]);
}

#[test]
fn remap_and_destroy_while_inflight_do_not_signal_stale_work() {
    let mut c=CpuCoordinator::new(128);
    let (client,id,buffer)=mapped(&mut c,10);
    let (_,peer,_)=mapped(&mut c,20);
    let mut p=CpuPresentation::new(&c,4,1,4).unwrap();
    let first=p.submit(&c,&[at(id,0,true),at(peer,2,true)]).unwrap();
    c.attach(client,1,None).unwrap();
    c.commit(client,1,None).unwrap();
    c.configure(id,2,SIZE).unwrap();
    c.acknowledge(client,1,2).unwrap();
    c.frame(client,1,11).unwrap();
    c.attach(client,1,Some(buffer)).unwrap();
    c.commit(client,1,Some(View{buffer,layout:LAYOUT,bytes:&RED})).unwrap();
    assert_eq!(p.complete(&mut c,&first.token).unwrap(),vec![(peer,vec![20])]);
    let next=p.submit(&c,&[at(id,0,true)]).unwrap();
    assert_eq!(c.destroy(client,1).unwrap(),vec![10,11]);
    assert!(p.complete(&mut c,&next.token).unwrap().is_empty());
}

#[test]
fn invalid_scene_and_foreign_output_token_leave_callback_pending() {
    let mut c=CpuCoordinator::new(128);
    let (_,id,_)=mapped(&mut c,10);
    let mut p=CpuPresentation::new(&c,2,1,2).unwrap();
    let mut other=CpuPresentation::new(&c,2,1,2).unwrap();
    assert!(matches!(p.submit(&c,&[at(id,0,true),at(id,0,true)]),Err(Error::Render(_))));
    let a=p.submit(&c,&[at(id,0,true)]).unwrap();
    let b=other.submit(&c,&[at(id,0,true)]).unwrap();
    assert_eq!(p.complete(&mut c,&b.token),Err(Error::UnknownFrame));
    other.discard(&b.token).unwrap();
    assert_eq!(p.complete(&mut c,&a.token).unwrap(),vec![(id,vec![10])]);
}

#[test]
fn negative_partial_visibility_and_fully_clipped_callback_policy() {
    let mut c=CpuCoordinator::new(128);
    let (client,id,_)=mapped(&mut c,10);
    let mut p=CpuPresentation::new(&c,2,1,2).unwrap();
    let off=p.submit(&c,&[at(id,-2,true)]).unwrap();
    assert_eq!(p.pixels(&off.token).unwrap(),&[[0,0,0,255];2]);
    assert!(p.complete(&mut c,&off.token).unwrap().is_empty());
    let partial=p.submit(&c,&[at(id,-1,true)]).unwrap();
    assert_eq!(p.pixels(&partial.token).unwrap(),&[[255,0,0,255],[0,0,0,255]]);
    assert_eq!(p.complete(&mut c,&partial.token).unwrap(),vec![(id,vec![10])]);
    c.frame(client,1,11).unwrap();
    c.commit(client,1,None).unwrap();
    let next=p.submit(&c,&[at(id,-1,true)]).unwrap();
    assert_eq!(p.complete(&mut c,&next.token).unwrap(),vec![(id,vec![11])]);
}
