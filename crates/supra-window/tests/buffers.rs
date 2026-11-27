use supra_window::{buffers::{Buffers, Completion, Destroy, Error}, clients::Connections};

#[test]
fn releases_only_after_last_actual_use_and_can_be_reused() {
    let mut connections = Connections::default();
    let c = connections.connect().unwrap();
    let mut b = Buffers::default();
    let id = b.register(c, 7).unwrap();
    let first = b.begin_use(c, 7).unwrap();
    let second = b.begin_use(c, 7).unwrap();
    assert_eq!(b.complete(second), Ok(Completion::StillInUse));
    assert_eq!(b.complete(second), Err(Error::UnknownUse));
    assert_eq!(b.complete(first), Ok(Completion::Release { client: c, object: 7, buffer: id }));
    let next = b.begin_use(c, 7).unwrap();
    assert_eq!(b.complete(next), Ok(Completion::Release { client: c, object: 7, buffer: id }));
}

#[test]
fn destroyed_inflight_object_is_retired_without_release_to_reused_id() {
    let mut connections = Connections::default();
    let c = connections.connect().unwrap();
    let mut b = Buffers::default();
    let old = b.register(c, 7).unwrap();
    let token = b.begin_use(c, 7).unwrap();
    assert_eq!(b.destroy(c, 7), Ok(Destroy::Deferred(old)));
    assert_eq!(b.begin_use(c, 7), Err(Error::UnknownBuffer));
    let new = b.register(c, 7).unwrap();
    assert_ne!(old, new);
    assert_eq!(b.complete(token), Ok(Completion::Retired { buffer: old }));
    assert_eq!(b.destroy(c, 7), Ok(Destroy::Retired(new)));
}

#[test]
fn disconnect_leaves_other_connections_and_defers_inflight_cleanup() {
    let mut connections = Connections::default();
    let a = connections.connect().unwrap();
    let c = connections.connect().unwrap();
    let mut b = Buffers::default();
    let first = b.register(a, 7).unwrap();
    let idle = b.register(a, 8).unwrap();
    let other = b.register(c, 7).unwrap();
    assert_eq!(b.register(a, 7), Err(Error::DuplicateObject));
    assert_eq!(b.register(a, 0), Err(Error::InvalidObject));
    let x = b.begin_use(a, 7).unwrap();
    let y = b.begin_use(c, 7).unwrap();
    assert_eq!(b.disconnect(a), Ok(vec![Destroy::Deferred(first), Destroy::Retired(idle)]));
    assert_eq!(b.complete(x), Ok(Completion::Retired { buffer: first }));
    assert_eq!(b.complete(y), Ok(Completion::Release { client: c, object: 7, buffer: other }));
    assert_eq!(b.begin_use(a, 8), Err(Error::UnknownBuffer));
}
