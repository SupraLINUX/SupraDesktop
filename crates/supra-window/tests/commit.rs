use supra_window::{Size, buffers::{Completion, Destroy}, commit::{Coordinator, Error}};

const SIZE: Size = Size { width: 640, height: 480 };

#[test]
fn rejected_role_and_foreign_buffer_leave_all_pending_state_intact() {
    let mut c = Coordinator::default();
    let a = c.connect().unwrap();
    let b = c.connect().unwrap();
    let id = c.create(a, 2).unwrap();
    let own = c.register_buffer(a, 3).unwrap();
    let foreign = c.register_buffer(b, 3).unwrap();
    c.attach(a, 2, Some(own)).unwrap();
    c.frame(a, 2, 42).unwrap();
    assert!(c.commit(a, 2, Some(SIZE)).is_err());
    assert_eq!(c.surface(id).unwrap().generation(), 0);
    assert!(!c.window(id).unwrap().mapped());
    c.configure(id, 9, SIZE).unwrap();
    c.acknowledge(a, 2, 9).unwrap();
    c.attach(a, 2, Some(foreign)).unwrap();
    assert!(matches!(c.commit(a, 2, Some(SIZE)), Err(Error::Buffer(_))));
    assert_eq!(c.surface(id).unwrap().generation(), 0);
    assert_eq!(c.surface(id).unwrap().current().buffer, None);
    assert!(!c.window(id).unwrap().mapped());
    c.attach(a, 2, Some(own)).unwrap();
    assert!(c.commit(a, 2, Some(Size { width: 0, height: 480 })).is_err());
    let first = c.commit(a, 2, Some(SIZE)).unwrap();
    assert_eq!(first.surface.frame_requests, vec![42]);
    assert!(c.window(id).unwrap().mapped());
    assert!(matches!(c.complete(first.new_use.unwrap()), Ok(Completion::Release { buffer, .. }) if buffer == own));
    assert_eq!(c.destroy_buffer(b, 3).unwrap(), Destroy::Retired(foreign));
}

#[test]
fn null_unmaps_and_remap_requires_new_ack_without_releasing_pending_attach() {
    let mut c = Coordinator::default();
    let a = c.connect().unwrap();
    let id = c.create(a, 2).unwrap();
    let replaced = c.register_buffer(a, 3).unwrap();
    let used = c.register_buffer(a, 4).unwrap();
    assert!(c.commit(a, 2, None).unwrap().new_use.is_none());
    c.configure(id, 1, SIZE).unwrap();
    c.acknowledge(a, 2, 1).unwrap();
    c.attach(a, 2, Some(replaced)).unwrap();
    c.attach(a, 2, Some(used)).unwrap();
    let first = c.commit(a, 2, Some(SIZE)).unwrap();
    assert_eq!(c.destroy_buffer(a, 3).unwrap(), Destroy::Retired(replaced));
    assert!(c.commit(a, 2, Some(SIZE)).unwrap().new_use.is_none());
    c.attach(a, 2, None).unwrap();
    assert!(c.commit(a, 2, None).unwrap().new_use.is_none());
    assert!(!c.window(id).unwrap().mapped());
    c.attach(a, 2, Some(used)).unwrap();
    assert!(c.commit(a, 2, Some(SIZE)).is_err());
    c.configure(id, 2, SIZE).unwrap();
    c.acknowledge(a, 2, 2).unwrap();
    let second = c.commit(a, 2, Some(SIZE)).unwrap();
    assert_eq!(c.complete(first.new_use.unwrap()).unwrap(), Completion::StillInUse);
    assert!(matches!(c.complete(second.new_use.unwrap()), Ok(Completion::Release { buffer, .. }) if buffer == used));
}

#[test]
fn retired_identity_cannot_be_rebound_and_disconnect_preserves_other_clients() {
    let mut c = Coordinator::default();
    let a = c.connect().unwrap();
    let b = c.connect().unwrap();
    let id = c.create(a, 2).unwrap();
    let peer = c.create(b, 2).unwrap();
    let old = c.register_buffer(a, 3).unwrap();
    c.configure(id, 1, SIZE).unwrap();
    c.acknowledge(a, 2, 1).unwrap();
    c.attach(a, 2, Some(old)).unwrap();
    let use_id = c.commit(a, 2, Some(SIZE)).unwrap().new_use.unwrap();
    assert_eq!(c.destroy_buffer(a, 3).unwrap(), Destroy::Deferred(old));
    let new = c.register_buffer(a, 3).unwrap();
    c.attach(a, 2, Some(old)).unwrap();
    assert!(matches!(c.commit(a, 2, Some(SIZE)), Err(Error::Buffer(_))));
    assert_eq!(c.surface(id).unwrap().generation(), 1);
    c.frame(a, 2, 77).unwrap();
    let removed = c.disconnect(a).unwrap();
    assert_eq!(removed.windows, vec![id]);
    assert_eq!(removed.pending_callbacks, vec![77]);
    assert_eq!(removed.buffers, vec![Destroy::Retired(new)]);
    assert_eq!(c.complete(use_id).unwrap(), Completion::Retired { buffer: old });
    assert_eq!(c.register_buffer(a, 5), Err(Error::UnknownClient));
    assert!(c.surface(id).is_none());
    assert!(c.surface(peer).is_some());
    assert!(c.commit(b, 2, None).is_ok());
}
