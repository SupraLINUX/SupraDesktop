use supra_window::{Size, buffers::Completion, cpu_content::{CpuCoordinator, CopyBudget, Error, Format, Layout, View}, surface::{State, Transform}};
const SIZE: Size = Size { width: 2, height: 1 };
const LAYOUT: Layout = Layout { width: 2, height: 1, stride: 8, offset: 0, format: Format::Argb8888 };

#[test]
fn packed_copy_ignores_padding_and_rejects_bad_ranges() {
    let budget = CopyBudget::new(16);
    let layout = Layout { height: 2, stride: 12, offset: 4, ..LAYOUT };
    let bytes: Vec<u8> = (0..24).collect();
    let copy = budget.copy(layout, &bytes).unwrap();
    assert_eq!(copy.bytes(), &[4,5,6,7,8,9,10,11,16,17,18,19,20,21,22,23]);
    assert_eq!(copy.layout().stride, 8);
    assert_eq!(budget.used(), 16);
    assert!(matches!(budget.copy(LAYOUT, &[0; 8]), Err(Error::Budget)));
    assert!(matches!(budget.copy(layout, &bytes[..23]), Err(Error::OutOfBounds)));
    assert!(matches!(budget.copy(Layout { stride: 4, ..LAYOUT }, &bytes), Err(Error::InvalidStride)));
    let clone = copy.clone();
    drop(copy);
    assert_eq!(budget.used(), 16);
    drop(clone);
    assert_eq!(budget.used(), 0);
}

#[test]
fn geometry_validates_scale_and_all_axis_swapping_transforms() {
    let layout = Layout { width: 8, height: 4, stride: 32, ..LAYOUT };
    for transform in [Transform::Normal, Transform::Rotate90, Transform::Rotate180, Transform::Rotate270,
                      Transform::Flipped, Transform::Flipped90, Transform::Flipped180, Transform::Flipped270] {
        let size = layout.logical_size(State { scale: 2, transform, ..State::default() }).unwrap();
        let swaps = matches!(transform, Transform::Rotate90 | Transform::Rotate270 | Transform::Flipped90 | Transform::Flipped270);
        assert_eq!(size, if swaps { Size { width: 2, height: 4 } } else { Size { width: 4, height: 2 } });
    }
    assert_eq!(layout.logical_size(State { scale: 3, ..State::default() }), Err(Error::InvalidDimensions));
    assert_eq!(layout.logical_size(State { scale: 0, ..State::default() }), Err(Error::InvalidDimensions));
}

#[test]
fn copy_survives_release_client_mutation_detach_and_disconnect() {
    let mut c = CpuCoordinator::new(16);
    let client = c.connect().unwrap();
    let id = c.create(client, 1).unwrap();
    let buffer = c.register_buffer(client, 2).unwrap();
    c.configure(id, 1, SIZE).unwrap();
    c.acknowledge(client, 1, 1).unwrap();
    c.attach(client, 1, Some(buffer)).unwrap();
    let mut pixels = [7; 8];
    let committed = c.commit(client, 1, Some(View { buffer, layout: LAYOUT, bytes: &pixels })).unwrap();
    assert!(matches!(committed.release, Some(Completion::Release { buffer: b, .. }) if b == buffer));
    pixels.fill(99);
    let snapshot = c.snapshot(id).unwrap();
    assert_eq!(snapshot.content.bytes(), &[7; 8]);
    assert!(c.commit(client, 1, None).unwrap().release.is_none());
    c.attach(client, 1, None).unwrap();
    c.commit(client, 1, None).unwrap();
    assert!(c.snapshot(id).is_none());
    c.disconnect(client).unwrap();
    assert_eq!(snapshot.content.bytes(), &[7; 8]);
    assert_eq!(c.used_bytes(), 8);
    drop(snapshot);
    assert_eq!(c.used_bytes(), 0);
}

#[test]
fn failed_import_or_role_commit_does_not_publish_or_leak_budget() {
    let mut c = CpuCoordinator::new(8);
    let client = c.connect().unwrap();
    let id = c.create(client, 1).unwrap();
    let buffer = c.register_buffer(client, 2).unwrap();
    let pixels = [1; 8];
    c.attach(client, 1, Some(buffer)).unwrap();
    assert!(c.commit(client, 1, Some(View { buffer, layout: LAYOUT, bytes: &pixels })).is_err());
    assert_eq!(c.used_bytes(), 0);
    assert!(c.snapshot(id).is_none());
    c.configure(id, 1, SIZE).unwrap();
    c.acknowledge(client, 1, 1).unwrap();
    c.frame(client, 1, 99).unwrap();
    assert!(matches!(c.commit(client, 1, Some(View { buffer, layout: LAYOUT, bytes: &pixels[..7] })), Err(Error::OutOfBounds)));
    let good = c.commit(client, 1, Some(View { buffer, layout: LAYOUT, bytes: &pixels })).unwrap();
    assert_eq!(good.logical.surface.generation, 1);
    assert_eq!(good.logical.surface.frame_requests, vec![99]);
    c.attach(client, 1, Some(buffer)).unwrap();
    assert!(matches!(c.commit(client, 1, Some(View { buffer, layout: LAYOUT, bytes: &pixels })), Err(Error::Budget)));
    assert_eq!(c.snapshot(id).unwrap().generation, 1);
    assert_eq!(c.used_bytes(), 8);
}
