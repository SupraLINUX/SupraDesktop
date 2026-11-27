use supra_window::{clients::{ClientId, Connections, Error}, Error as WindowError, Size, WindowId};

const SIZE: Size = Size { width: 800, height: 600 };

fn map(c: &mut Connections, client: ClientId, object: u32) -> WindowId {
    let id = c.create(client, object).unwrap();
    let configure = c.configure(id, SIZE).unwrap();
    c.acknowledge(client, object, configure.token).unwrap();
    c.commit(client, object, SIZE).unwrap();
    id
}

#[test]
fn local_object_ids_are_scoped_to_connection() {
    let mut c = Connections::default();
    let a = c.connect().unwrap();
    let b = c.connect().unwrap();
    let first = map(&mut c, a, 7);
    let second = map(&mut c, b, 7);
    assert_ne!(first, second);
    assert_eq!(c.create(a, 7), Err(Error::DuplicateObject));
    assert_eq!(c.create(a, 0), Err(Error::InvalidObject));
    let token = c.configure(first, SIZE).unwrap().token;
    assert_eq!(c.acknowledge(b, 7, token), Err(Error::Window(WindowError::StaleConfigure)));
    assert_eq!(c.destroy(b, 8), Err(Error::UnknownObject));
    c.destroy(b, 7).unwrap();
    assert!(c.get(first).unwrap().mapped());
    assert!(c.get(second).is_none());
}

#[test]
fn disconnect_removes_only_owned_windows_and_pending_state() {
    let mut c = Connections::default();
    let a = c.connect().unwrap();
    let b = c.connect().unwrap();
    let first = map(&mut c, a, 7);
    let second = map(&mut c, b, 7);
    let unmapped = c.create(a, 8).unwrap();
    c.configure(unmapped, SIZE).unwrap();
    c.activate(first).unwrap();
    assert_eq!(c.disconnect(a).unwrap(), vec![first, unmapped]);
    assert_eq!(c.focus(), None);
    assert_eq!(c.stacking(), &[second]);
    assert!(c.get(unmapped).is_none());
    assert_eq!(c.commit(a, 7, SIZE), Err(Error::UnknownClient));
    c.commit(b, 7, SIZE).unwrap();
    c.activate(second).unwrap();
    assert_eq!(c.focus(), Some(second));
    assert_eq!(c.disconnect(a), Err(Error::UnknownClient));
}

#[test]
fn reconnect_and_object_reuse_do_not_reuse_internal_identity() {
    let mut c = Connections::default();
    let a = c.connect().unwrap();
    let old = c.create(a, 7).unwrap();
    let token = c.configure(old, SIZE).unwrap().token;
    c.destroy(a, 7).unwrap();
    let new = c.create(a, 7).unwrap();
    assert_ne!(old, new);
    assert_eq!(c.acknowledge(a, 7, token), Err(Error::Window(WindowError::StaleConfigure)));
    c.disconnect(a).unwrap();
    let b = c.connect().unwrap();
    assert_ne!(a, b);
    let next = map(&mut c, b, 7);
    assert_ne!(new, next);
    assert_eq!(c.resolve(a, 7), Err(Error::UnknownClient));
}
