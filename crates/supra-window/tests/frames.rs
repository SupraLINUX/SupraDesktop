use supra_window::{Size, commit::{Coordinator, Error}};
const SIZE: Size = Size { width: 640, height: 480 };

#[test]
fn committed_callbacks_follow_fifo_visibility_and_generation() {
    let mut c = Coordinator::default();
    let client = c.connect().unwrap();
    let id = c.create(client, 1).unwrap();
    let buffer = c.register_buffer(client, 2).unwrap();
    c.configure(id, 1, SIZE).unwrap();
    c.acknowledge(client, 1, 1).unwrap();
    c.attach(client, 1, Some(buffer)).unwrap();
    c.frame(client, 1, 10).unwrap();
    c.frame(client, 1, 11).unwrap();
    let first = c.commit(client, 1, Some(SIZE)).unwrap();
    c.frame(client, 1, 12).unwrap();
    assert!(c.frame_ready(id, 1, false).unwrap().is_empty());
    assert_eq!(c.frame_ready(id, 2, true), Err(Error::InvalidGeneration));
    assert_eq!(c.frame_ready(id, 1, true).unwrap(), vec![10, 11]);
    assert!(c.frame_ready(id, 1, true).unwrap().is_empty());
    c.commit(client, 1, Some(SIZE)).unwrap();
    assert!(c.frame_ready(id, 1, true).unwrap().is_empty());
    assert_eq!(c.frame_ready(id, 2, true).unwrap(), vec![12]);
    // Delivering callbacks did not complete the buffer use.
    assert!(c.complete(first.new_use.unwrap()).is_ok());
}

#[test]
fn old_mapping_generation_cannot_signal_after_remap() {
    let mut c = Coordinator::default();
    let client = c.connect().unwrap();
    let id = c.create(client, 1).unwrap();
    let buffer = c.register_buffer(client, 2).unwrap();
    c.configure(id, 1, SIZE).unwrap();
    c.acknowledge(client, 1, 1).unwrap();
    c.attach(client, 1, Some(buffer)).unwrap();
    c.frame(client, 1, 10).unwrap();
    c.commit(client, 1, Some(SIZE)).unwrap();
    c.attach(client, 1, None).unwrap();
    c.commit(client, 1, None).unwrap();
    assert!(c.frame_ready(id, 1, true).unwrap().is_empty());
    c.configure(id, 2, SIZE).unwrap();
    c.acknowledge(client, 1, 2).unwrap();
    c.attach(client, 1, Some(buffer)).unwrap();
    c.frame(client, 1, 11).unwrap();
    c.commit(client, 1, Some(SIZE)).unwrap();
    assert!(c.frame_ready(id, 1, true).unwrap().is_empty());
    assert!(c.frame_ready(id, 2, true).unwrap().is_empty());
    assert_eq!(c.frame_ready(id, 3, true).unwrap(), vec![10, 11]);
}

#[test]
fn live_identity_is_unique_and_disconnect_cancels_only_its_own_callbacks() {
    let mut c = Coordinator::default();
    let a = c.connect().unwrap();
    let b = c.connect().unwrap();
    let old = c.create(a, 1).unwrap();
    let peer = c.create(b, 1).unwrap();
    c.frame(a, 1, 10).unwrap();
    assert_eq!(c.frame(b, 1, 10), Err(Error::DuplicateCallback));
    c.commit(a, 1, None).unwrap();
    c.frame(a, 1, 11).unwrap();
    c.frame(b, 1, 12).unwrap();
    let gone = c.disconnect(a).unwrap();
    assert_eq!(gone.pending_callbacks, vec![11]);
    assert_eq!(gone.committed_callbacks, vec![10]);
    assert!(c.frame_ready(old, 1, true).is_err());
    assert_eq!(c.destroy(b, 1).unwrap(), vec![12]);
    let new = c.create(b, 1).unwrap();
    assert_ne!(peer, new);
}

#[test]
fn committed_backlog_has_a_bound_without_dropping_callbacks() {
    let mut c = Coordinator::default();
    let client = c.connect().unwrap();
    let id = c.create(client, 1).unwrap();
    for group in 0..2 {
        for index in 1..=256 { c.frame(client, 1, group * 256 + index).unwrap(); }
        c.commit(client, 1, None).unwrap();
    }
    assert_eq!(c.frame(client, 1, 513), Err(Error::CallbackCapacity));
    assert_eq!(c.surface(id).unwrap().generation(), 2);
    assert_eq!(c.destroy(client, 1).unwrap(), (1..=512).collect::<Vec<_>>());
}
