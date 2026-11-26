use supra_window::{Error, Size, Windows};

fn size(width: u32, height: u32) -> Size { Size { width, height } }

fn map(w: &mut Windows, id: u64) {
    w.create(id).unwrap();
    let c = w.configure(id, size(800, 600)).unwrap();
    w.acknowledge(id, c.token).unwrap();
    w.commit(id, size(800, 600)).unwrap();
}

#[test]
fn initial_map_requires_ack_and_focus_is_explicit() {
    let mut w = Windows::default();
    w.create(1).unwrap();
    assert_eq!(w.create(1), Err(Error::Duplicate));
    assert_eq!(w.commit(1, size(100,100)), Err(Error::InvalidState));
    assert_eq!(w.activate(1), Err(Error::InvalidState));
    let c = w.configure(1, size(800,600)).unwrap();
    w.acknowledge(1,c.token).unwrap();
    assert!(!w.get(1).unwrap().mapped());
    w.commit(1,size(790,590)).unwrap();
    assert_eq!(w.get(1).unwrap().committed(), Some(size(790,590)));
    assert_eq!(w.focus(), None);
    w.activate(1).unwrap();
    assert_eq!(w.focus(), Some(1));
}

#[test]
fn configure_coalescing_and_stale_tokens() {
    let mut w = Windows::default();
    map(&mut w, 1);
    let first = w.configure(1,size(900,600)).unwrap();
    let latest = w.configure(1,size(1000,600)).unwrap();
    assert_eq!(w.acknowledge(1,first.token), Err(Error::StaleConfigure));
    assert_eq!(w.get(1).unwrap().pending(), Some(latest));
    w.acknowledge(1,latest.token).unwrap();
    let next = w.configure(1,size(1100,600)).unwrap();
    w.commit(1,size(990,590)).unwrap();
    assert_eq!(w.get(1).unwrap().pending(), Some(next));
    w.acknowledge(1,next.token).unwrap();
    w.commit(1,size(1100,600)).unwrap();
    assert_eq!(w.acknowledge(1,next.token), Err(Error::StaleConfigure));
}

#[test]
fn activation_close_unmap_remap_and_destroy() {
    let mut w = Windows::default();
    map(&mut w,1);
    map(&mut w,2);
    w.activate(1).unwrap();
    assert_eq!(w.stacking(), &[2,1]);
    w.request_close(1).unwrap();
    assert_eq!(w.focus(), None);
    assert!(w.get(1).unwrap().mapped());
    assert!(w.get(1).unwrap().close_requested());
    assert_eq!(w.activate(1), Err(Error::InvalidState));
    w.unmap(1).unwrap();
    assert_eq!(w.stacking(), &[2]);
    assert_eq!(w.commit(1,size(800,600)), Err(Error::InvalidState));
    let c = w.configure(1,size(800,600)).unwrap();
    w.acknowledge(1,c.token).unwrap();
    w.commit(1,size(800,600)).unwrap();
    w.activate(1).unwrap();
    w.destroy(1).unwrap();
    assert_eq!(w.focus(), None);
    assert_eq!(w.stacking(), &[2]);
    assert!(w.get(1).is_none());
    assert_eq!(w.destroy(1), Err(Error::Unknown));
}

#[test]
fn invalid_sizes_and_foreign_tokens_do_not_change_state() {
    let mut w = Windows::default();
    map(&mut w,1);
    map(&mut w,2);
    let token = w.configure(1,size(900,600)).unwrap();
    assert_eq!(w.configure(1,size(0,600)), Err(Error::InvalidSize));
    assert_eq!(w.commit(1,size(17000,600)), Err(Error::InvalidSize));
    assert_eq!(w.acknowledge(2,token.token), Err(Error::StaleConfigure));
    assert_eq!(w.get(1).unwrap().pending(), Some(token));
    assert_eq!(w.get(1).unwrap().committed(), Some(size(800,600)));
}
