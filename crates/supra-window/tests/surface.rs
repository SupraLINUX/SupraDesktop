use supra_window::{buffers::Buffers, clients::Connections,
                   surface::{Attachment, Error, MAX_PENDING_FRAMES, Surface, Transform}};

#[test]
fn pending_state_is_invisible_until_commit_and_persists_afterward() {
    let mut clients = Connections::default();
    let c = clients.connect().unwrap();
    let mut buffers = Buffers::default();
    let id = buffers.register(c, 7).unwrap();
    let mut s = Surface::default();
    s.attach(Some(id));
    s.set_scale(2).unwrap();
    s.set_transform(Transform::Rotate90);
    s.frame(41).unwrap();
    assert_eq!(s.current().buffer, None);
    assert_eq!(s.current().scale, 1);
    assert_eq!(s.preview().buffer, Some(id));
    assert_eq!(s.generation(), 0);
    let first = s.commit().unwrap();
    assert_eq!(first.attachment, Attachment::Attached(id));
    assert_eq!(first.current.scale, 2);
    assert_eq!(first.current.transform, Transform::Rotate90);
    assert_eq!(first.frame_requests, vec![41]);
    let next = s.commit().unwrap();
    assert_eq!(next.generation, 2);
    assert_eq!(next.attachment, Attachment::Unchanged);
    assert_eq!(next.current, first.current);
    assert!(next.frame_requests.is_empty());
}

#[test]
fn last_pending_attach_wins_and_null_commit_differs_from_no_attach() {
    let mut clients = Connections::default();
    let c = clients.connect().unwrap();
    let mut buffers = Buffers::default();
    let a = buffers.register(c, 7).unwrap();
    let b = buffers.register(c, 8).unwrap();
    let mut s = Surface::default();
    s.attach(Some(a));
    s.attach(Some(b));
    assert_eq!(s.commit().unwrap().attachment, Attachment::Attached(b));
    s.attach(None);
    assert_eq!(s.current().buffer, Some(b));
    let detach = s.commit().unwrap();
    assert_eq!(detach.attachment, Attachment::Detached);
    assert_eq!(detach.current.buffer, None);
    assert_eq!(s.commit().unwrap().attachment, Attachment::Unchanged);
}

#[test]
fn rejected_requests_do_not_replace_valid_pending_state() {
    let mut s = Surface::default();
    s.set_scale(2).unwrap();
    assert_eq!(s.set_scale(0), Err(Error::InvalidScale));
    assert_eq!(s.set_scale(-1), Err(Error::InvalidScale));
    assert_eq!(s.preview().scale, 2);
    assert_eq!(s.frame(0), Err(Error::InvalidFrame));
    s.frame(1).unwrap();
    assert_eq!(s.frame(1), Err(Error::DuplicateFrame));
    for id in 2..=MAX_PENDING_FRAMES as u64 {
        s.frame(id).unwrap();
    }
    assert_eq!(s.frame(MAX_PENDING_FRAMES as u64 + 1), Err(Error::FrameQueueFull));
    assert_eq!(s.commit().unwrap().frame_requests.len(), MAX_PENDING_FRAMES);
    assert!(s.commit().unwrap().frame_requests.is_empty());
}

#[test]
fn discard_pending_returns_callbacks_without_changing_committed_state() {
    let mut s = Surface::default();
    s.set_scale(3).unwrap();
    s.commit().unwrap();
    s.set_scale(2).unwrap();
    s.set_transform(Transform::Flipped90);
    s.frame(19).unwrap();
    s.frame(20).unwrap();
    assert_eq!(s.discard_pending(), vec![19, 20]);
    assert_eq!(s.preview(), s.current());
    assert_eq!(s.generation(), 1);
    assert_eq!(s.current().scale, 3);
    assert!(s.commit().unwrap().frame_requests.is_empty());
}
