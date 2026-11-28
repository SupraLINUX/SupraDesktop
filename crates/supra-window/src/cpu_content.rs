//! Experimental CPU-copy backend boundary. Never maps a client fd; the
//! trusted transport supplies a validated, readable slice for the actual
//! attachment. No pixel format conversion, GPU import or wire events.
use std::{collections::BTreeMap, sync::{Arc, atomic::{AtomicUsize, Ordering}}};
use crate::{Configure, Size, WindowId,
    buffers::{BufferId, Completion, Destroy},
    clients::ClientId,
    commit::{Committed, Coordinator, Disconnected},
    surface::{State, Transform}};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format { Argb8888, Xrgb8888 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub offset: usize,
    pub format: Format,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Logical(crate::commit::Error),
    InvalidDimensions,
    InvalidStride,
    OutOfBounds,
    Budget,
    Allocation,
    MissingContent,
    UnexpectedContent,
}

impl From<crate::commit::Error> for Error {
    fn from(value: crate::commit::Error) -> Self { Self::Logical(value) }
}

impl Layout {
    pub fn logical_size(self, state: State) -> Result<Size, Error> {
        let scale = u32::try_from(state.scale).map_err(|_| Error::InvalidDimensions)?;
        if self.width == 0 || self.height == 0 || scale == 0
            || self.width % scale != 0 || self.height % scale != 0 {
            return Err(Error::InvalidDimensions);
        }
        let (mut width, mut height) = (self.width / scale, self.height / scale);
        if matches!(state.transform, Transform::Rotate90 | Transform::Rotate270
            | Transform::Flipped90 | Transform::Flipped270) {
            std::mem::swap(&mut width, &mut height);
        }
        if width > 16384 || height > 16384 { return Err(Error::InvalidDimensions); }
        Ok(Size { width, height })
    }

    fn checked_span(self, len: usize) -> Result<(usize, usize), Error> {
        if self.width == 0 || self.height == 0 { return Err(Error::InvalidDimensions); }
        let row = (self.width as usize).checked_mul(4).ok_or(Error::OutOfBounds)?;
        if self.stride < row || self.stride % 4 != 0 || self.offset % 4 != 0 {
            return Err(Error::InvalidStride);
        }
        let end = self.offset.checked_add((self.height as usize - 1)
            .checked_mul(self.stride).ok_or(Error::OutOfBounds)?)
            .and_then(|n| n.checked_add(row)).ok_or(Error::OutOfBounds)?;
        if end > len { return Err(Error::OutOfBounds); }
        let packed = row.checked_mul(self.height as usize).ok_or(Error::OutOfBounds)?;
        Ok((row, packed))
    }
}

struct Reservation { used: Arc<AtomicUsize>, bytes: usize }
impl Drop for Reservation {
    fn drop(&mut self) { self.used.fetch_sub(self.bytes, Ordering::AcqRel); }
}

struct Pixels {
    layout: Layout,
    bytes: Vec<u8>,
    // Last field: release accounting after dropping pixel allocation.
    _reservation: Reservation,
}

/// Read-only retained content. Clones retain the same allocation; a replaced
/// or detached surface cannot invalidate an already-issued renderer snapshot.
#[derive(Clone)]
pub struct Content(Arc<Pixels>);

impl Content {
    pub fn bytes(&self) -> &[u8] { &self.0.bytes }
    pub fn same_storage(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }
    pub fn layout(&self) -> Layout { self.0.layout }
}

pub struct CopyBudget {
    used: Arc<AtomicUsize>,
    limit: usize,
}

impl CopyBudget {
    pub fn new(limit: usize) -> Self { Self { used: Arc::new(AtomicUsize::new(0)), limit } }
    pub fn used(&self) -> usize { self.used.load(Ordering::Acquire) }

    pub fn copy(&self, layout: Layout, bytes: &[u8]) -> Result<Content, Error> {
        let (row, packed) = layout.checked_span(bytes.len())?;
        self.used.fetch_update(Ordering::AcqRel, Ordering::Acquire, |used|
            used.checked_add(packed).filter(|&next| next <= self.limit))
            .map_err(|_| Error::Budget)?;
        let reservation = Reservation { used: Arc::clone(&self.used), bytes: packed };
        let mut owned = Vec::new();
        owned.try_reserve_exact(packed).map_err(|_| Error::Allocation)?;
        for y in 0..layout.height as usize {
            let start = layout.offset + y * layout.stride;
            owned.extend_from_slice(&bytes[start..start + row]);
        }
        let layout = Layout { stride: row, offset: 0, ..layout };
        Ok(Content(Arc::new(Pixels { layout, bytes: owned, _reservation: reservation })))
    }
}

/// This is a trusted import result, not a client-provided handle. The actual
/// transport must resolve the ID, validate the fd/pool and contain SIGBUS.
pub struct View<'a> { pub buffer: BufferId, pub layout: Layout, pub bytes: &'a [u8] }

pub struct Snapshot {
    pub window: WindowId,
    pub generation: u64,
    pub state: State,
    pub content: Content,
}

pub struct CpuCommit {
    pub logical: Committed,
    pub release: Option<Completion>,
}

/// Opaque instance identity for adapters with asynchronous work. Holding
/// a clone prevents identity reuse after the originating coordinator dies.
#[derive(Clone)]
pub struct CoordinatorIdentity(Arc<()>);

/// Owns logical coordinator; never exposes a mutable bypass. The logical
/// surface remains the sole authority on mapping/identity; this map owns
/// only the independent copied content for each mapped surface.
pub struct CpuCoordinator {
    logical: Coordinator,
    content: BTreeMap<WindowId, Content>,
    budget: CopyBudget,
    identity: CoordinatorIdentity,
}

impl CpuCoordinator {
    pub fn new(max_bytes: usize) -> Self {
        Self { logical: Coordinator::default(), content: BTreeMap::new(), budget: CopyBudget::new(max_bytes), identity: CoordinatorIdentity(Arc::new(())) }
    }

    pub fn identity(&self) -> CoordinatorIdentity { self.identity.clone() }
    pub fn matches_identity(&self, identity: &CoordinatorIdentity) -> bool {
        Arc::ptr_eq(&self.identity.0, &identity.0)
    }
    pub fn used_bytes(&self) -> usize { self.budget.used() }
    pub fn connect(&mut self) -> Result<ClientId, Error> { Ok(self.logical.connect()?) }
    pub fn create(&mut self, c: ClientId, object: u32) -> Result<WindowId, Error> {
        Ok(self.logical.create(c, object)?)
    }
    pub fn register_buffer(&mut self, c: ClientId, object: u32) -> Result<BufferId, Error> {
        Ok(self.logical.register_buffer(c, object)?)
    }
    pub fn destroy_buffer(&mut self, c: ClientId, object: u32) -> Result<Destroy, Error> {
        Ok(self.logical.destroy_buffer(c, object)?)
    }
    pub fn configure(&mut self, id: WindowId, serial: u32, size: Size) -> Result<Configure, Error> {
        Ok(self.logical.configure(id, serial, size)?)
    }
    pub fn acknowledge(&mut self, c: ClientId, object: u32, serial: u32) -> Result<(), Error> {
        Ok(self.logical.acknowledge(c, object, serial)?)
    }
    pub fn attach(&mut self, c: ClientId, object: u32, buffer: Option<BufferId>) -> Result<(), Error> {
        Ok(self.logical.attach(c, object, buffer)?)
    }
    pub fn set_scale(&mut self, c: ClientId, object: u32, scale: i32) -> Result<(), Error> {
        Ok(self.logical.set_scale(c, object, scale)?)
    }
    pub fn set_transform(&mut self, c: ClientId, object: u32, transform: Transform) -> Result<(), Error> {
        Ok(self.logical.set_transform(c, object, transform)?)
    }
    pub fn frame(&mut self, c: ClientId, object: u32, callback: u64) -> Result<(), Error> {
        Ok(self.logical.frame(c, object, callback)?)
    }

    pub fn commit(&mut self, c: ClientId, object: u32, view: Option<View<'_>>) -> Result<CpuCommit, Error> {
        let id = self.logical.resolve(c, object)?;
        let surface = self.logical.surface(id).ok_or(Error::MissingContent)?;
        let state = surface.preview();
        use crate::surface::Attachment;
        let (new, size) = match surface.pending_attachment() {
            Attachment::Attached(id) => {
                let view = view.ok_or(Error::MissingContent)?;
                if view.buffer != id { return Err(Error::UnexpectedContent); }
                let size = view.layout.logical_size(state)?;
                let copy = self.budget.copy(view.layout, view.bytes)?;
                (Some(copy), Some(size))
            }
            Attachment::Detached => {
                if view.is_some() { return Err(Error::UnexpectedContent); }
                (None, None)
            }
            Attachment::Unchanged => {
                if view.is_some() { return Err(Error::UnexpectedContent); }
                let size = match state.buffer {
                    Some(_) => Some(self.content.get(&id).ok_or(Error::MissingContent)?.layout().logical_size(state)?),
                    None => None,
                };
                (None, size)
            }
        };
        // Copy/geometry errors above and logical errors here publish nothing.
        let logical = self.logical.commit(c, object, size)?;
        if let Some(content) = new { self.content.insert(id, content); }
        if state.buffer.is_none() { self.content.remove(&id); }
        // Copy is owned by the compositor, so this consumer needs the
        // client storage no longer. Renderer snapshots retain the copy.
        let release = logical.new_use.map(|token|
            self.logical.complete(token).expect("just-created use"));
        Ok(CpuCommit { logical, release })
    }

    pub fn snapshot(&self, id: WindowId) -> Option<Snapshot> {
        let surface = self.logical.surface(id)?;
        Some(Snapshot { window: id, generation: surface.generation(),
            state: surface.current(), content: self.content.get(&id)?.clone() })
    }

    pub fn frame_ready(&mut self, id: WindowId, generation: u64, visible: bool) -> Result<Vec<u64>, Error> {
        Ok(self.logical.frame_ready(id, generation, visible)?)
    }

    /// Process an adapter's recorded visible frame boundary as one serialized
    /// batch. Destroyed/unmapped windows do not receive notifications.
    /// The existing scheduler owns generation epochs and callback queues.
    pub fn frame_ready_batch(&mut self, boundaries: &[(WindowId, u64)]) -> Result<Vec<(WindowId, Vec<u64>)>, Error> {
        let mut seen = std::collections::BTreeSet::new();
        for &(id, generation) in boundaries {
            if generation == 0 || !seen.insert(id) {
                return Err(crate::commit::Error::InvalidGeneration.into());
            }
            if self.logical.surface(id).is_some_and(|s| generation > s.generation()) {
                return Err(crate::commit::Error::InvalidGeneration.into());
            }
        }
        let mut ready = Vec::new();
        for &(id, generation) in boundaries {
            if self.content.contains_key(&id) {
                // All live generations were checked above under the same
                // exclusive borrow; no state changes can invalidate them.
                let callbacks = self.logical.frame_ready(id, generation, true)?;
                if !callbacks.is_empty() { ready.push((id, callbacks)); }
            }
        }
        Ok(ready)
    }

    pub fn destroy(&mut self, c: ClientId, object: u32) -> Result<Vec<u64>, Error> {
        let id = self.logical.resolve(c, object)?;
        let callbacks = self.logical.destroy(c, object)?;
        self.content.remove(&id);
        Ok(callbacks)
    }

    pub fn disconnect(&mut self, c: ClientId) -> Result<Disconnected, Error> {
        let result = self.logical.disconnect(c)?;
        for id in &result.windows { self.content.remove(id); }
        Ok(result)
    }
}
