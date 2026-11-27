use supra_window::{Error as WindowError, MAX_PENDING_CONFIGURES, Size,
                   clients::Error as ClientError, wire::{Error, WireWindows}};

const SIZE: Size = Size { width: 800, height: 600 };

#[test]
fn configure_order_across_uint32_wrap() {
    let mut w = WireWindows::default();
    let c = w.connect().unwrap();
    let id = w.create(c, 9).unwrap();
    let first = w.configure(id, u32::MAX, SIZE).unwrap();
    let second = w.configure(id, 0, SIZE).unwrap();
    let third = w.configure(id, 1, SIZE).unwrap();
    assert!(first.token < second.token && second.token < third.token);
    w.acknowledge(c, 9, 0).unwrap();
    assert_eq!(w.get(id).unwrap().pending_count(), 1);
    assert_eq!(w.acknowledge(c, 9, u32::MAX), Err(Error::InvalidSerial));
    assert_eq!(w.acknowledge(c, 9, 0), Err(Error::InvalidSerial));
    w.commit(c, 9, SIZE).unwrap();
    w.acknowledge(c, 9, 1).unwrap();
    w.commit(c, 9, SIZE).unwrap();
    assert_eq!(w.get(id).unwrap().pending_count(), 0);
}

#[test]
fn wrong_connection_and_unmap_invalidate_mappings() {
    let mut w = WireWindows::default();
    let a = w.connect().unwrap();
    let b = w.connect().unwrap();
    let first = w.create(a, 7).unwrap();
    let second = w.create(b, 7).unwrap();
    w.configure(first, 11, SIZE).unwrap();
    w.configure(second, 12, SIZE).unwrap();
    assert_eq!(w.acknowledge(b, 7, 11), Err(Error::InvalidSerial));
    w.unmap(a, 7).unwrap();
    assert_eq!(w.acknowledge(a, 7, 11), Err(Error::InvalidSerial));
    w.configure(first, 13, SIZE).unwrap();
    w.acknowledge(a, 7, 13).unwrap();
    w.commit(a, 7, SIZE).unwrap();
    w.disconnect(a).unwrap();
    assert_eq!(w.acknowledge(a, 7, 13), Err(Error::Client(ClientError::UnknownClient)));
    w.acknowledge(b, 7, 12).unwrap();
    w.commit(b, 7, SIZE).unwrap();
    assert_eq!(w.stacking(), &[second]);
}

#[test]
fn failed_configure_does_not_consume_or_insert_wire_serial() {
    let mut w = WireWindows::default();
    let c = w.connect().unwrap();
    let id = w.create(c, 7).unwrap();
    w.configure(id, 1, SIZE).unwrap();
    assert_eq!(w.configure(id, 1, SIZE), Err(Error::SerialInUse));
    for serial in 2..=MAX_PENDING_CONFIGURES as u32 {
        w.configure(id, serial, SIZE).unwrap();
    }
    let rejected = MAX_PENDING_CONFIGURES as u32 + 1;
    assert_eq!(w.configure(id, rejected, SIZE),
               Err(Error::Client(ClientError::Window(WindowError::ConfigureQueueFull))));
    assert_eq!(w.acknowledge(c, 7, rejected), Err(Error::InvalidSerial));
    w.acknowledge(c, 7, MAX_PENDING_CONFIGURES as u32).unwrap();
    w.configure(id, rejected, SIZE).unwrap();
    w.acknowledge(c, 7, rejected).unwrap();
    w.commit(c, 7, SIZE).unwrap();
}

#[test]
fn object_reuse_clears_old_serials() {
    let mut w = WireWindows::default();
    let c = w.connect().unwrap();
    let old = w.create(c, 7).unwrap();
    w.configure(old, 21, SIZE).unwrap();
    w.destroy(c, 7).unwrap();
    let new = w.create(c, 7).unwrap();
    assert_ne!(old, new);
    assert_eq!(w.acknowledge(c, 7, 21), Err(Error::InvalidSerial));
    w.configure(new, 22, SIZE).unwrap();
    w.acknowledge(c, 7, 22).unwrap();
    w.commit(c, 7, SIZE).unwrap();
}
