//! Serialized logical toplevel commits. The transport must resolve all wire
//! resources and validate imports, dimensions and scale before invoking commit.
//! No GPU backing, presentation, wire events or authorization is implemented.
use std::collections::BTreeMap;
use crate::{Configure, Size, Window, WindowId,
    buffers::{BufferId, Buffers, Completion, Destroy, UseId},
    clients::ClientId, frames::Frames, surface::{Attachment, Commit, Surface}, wire::WireWindows};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Wire(crate::wire::Error),
    Buffer(crate::buffers::Error),
    Surface(crate::surface::Error),
    UnknownClient,
    InconsistentContent,
    DuplicateCallback,
    CallbackCapacity,
    InvalidGeneration,
}

impl From<crate::wire::Error> for Error {
    fn from(value: crate::wire::Error) -> Self { Self::Wire(value) }
}
impl From<crate::buffers::Error> for Error {
    fn from(value: crate::buffers::Error) -> Self { Self::Buffer(value) }
}
impl From<crate::surface::Error> for Error {
    fn from(value: crate::surface::Error) -> Self { Self::Surface(value) }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Committed {
    pub window: WindowId,
    pub surface: Commit,
    /// One tracked use for a newly committed attachment only. Completion
    /// must wait until the trusted consumer no longer needs that storage.
    pub new_use: Option<UseId>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Disconnected {
    pub windows: Vec<WindowId>,
    pub pending_callbacks: Vec<u64>,
    pub committed_callbacks: Vec<u64>,
    pub buffers: Vec<Destroy>,
}

/// Owns the dispatch, per-window surface state and ledger. Components retain
/// canonical ownership of their respective state. No mutable role/ledger
/// references escape. Connection liveness remains canonical in dispatch.
#[derive(Default)]
pub struct Coordinator {
    wire: WireWindows,
    buffers: Buffers,
    surfaces: BTreeMap<WindowId, Surface>,
    frames: Frames,
}

impl Coordinator {
    pub fn connect(&mut self) -> Result<ClientId, Error> {
        Ok(self.wire.connect()?)
    }

    pub fn create(&mut self, client: ClientId, object: u32) -> Result<WindowId, Error> {
        let id = self.wire.create(client, object)?;
        self.surfaces.insert(id, Surface::default());
        Ok(id)
    }

    pub fn register_buffer(&mut self, client: ClientId, object: u32) -> Result<BufferId, Error> {
        if !self.wire.is_live(client) { return Err(Error::UnknownClient); }
        Ok(self.buffers.register(client, object)?)
    }

    pub fn destroy_buffer(&mut self, client: ClientId, object: u32) -> Result<Destroy, Error> {
        Ok(self.buffers.destroy(client, object)?)
    }

    pub fn configure(&mut self, id: WindowId, serial: u32, suggested: Size) -> Result<Configure, Error> {
        Ok(self.wire.configure(id, serial, suggested)?)
    }

    pub fn acknowledge(&mut self, client: ClientId, object: u32, serial: u32) -> Result<(), Error> {
        Ok(self.wire.acknowledge(client, object, serial)?)
    }

    pub fn window(&self, id: WindowId) -> Option<&Window> { self.wire.get(id) }
    pub fn surface(&self, id: WindowId) -> Option<&Surface> { self.surfaces.get(&id) }

    pub fn attach(&mut self, client: ClientId, object: u32, buffer: Option<BufferId>) -> Result<(), Error> {
        self.pending(client, object)?.attach(buffer);
        Ok(())
    }

    pub fn set_scale(&mut self, client: ClientId, object: u32, scale: i32) -> Result<(), Error> {
        Ok(self.pending(client, object)?.set_scale(scale)?)
    }

    pub fn set_transform(&mut self, client: ClientId, object: u32, transform: crate::surface::Transform) -> Result<(), Error> {
        self.pending(client, object)?.set_transform(transform);
        Ok(())
    }

    pub fn frame(&mut self, client: ClientId, object: u32, callback: u64) -> Result<(), Error> {
        let id = self.wire.resolve(client, object)?;
        let surface = self.surfaces.get_mut(&id).ok_or(Error::InconsistentContent)?;
        self.frames.validate_reservation(id, callback, surface.pending_frame_count())?;
        surface.frame(callback)?;
        self.frames.reserve(id, callback);
        Ok(())
    }

    fn pending(&mut self, client: ClientId, object: u32) -> Result<&mut Surface, Error> {
        let id = self.wire.resolve(client, object)?;
        self.surfaces.get_mut(&id).ok_or(Error::InconsistentContent)
    }

    /// Validate all fallible logical checks before publishing any mutation.
    /// Size is the trusted backend's actual logical content size, not a
    /// client-supplied unverified claim. None denotes no content.
    pub fn commit(&mut self, client: ClientId, object: u32, actual: Option<Size>) -> Result<Committed, Error> {
        let id = self.wire.resolve(client, object)?;
        let surface = self.surfaces.get_mut(&id).ok_or(Error::InconsistentContent)?;
        let mapped = self.wire.get(id).ok_or(Error::InconsistentContent)?.mapped();
        if mapped != surface.current().buffer.is_some() {
            return Err(Error::InconsistentContent);
        }
        surface.validate_commit()?;
        if surface.preview().buffer.is_some() != actual.is_some() {
            return Err(Error::InconsistentContent);
        }
        if let Some(size) = actual { self.wire.validate_commit(client, object, size)?; }
        let attachment = surface.pending_attachment();
        if let Attachment::Attached(buffer) = attachment {
            self.buffers.validate_use_id(client, buffer)?;
        }

        // All checks above use the same exclusively borrowed state. No
        // user callback or fallible backend import can occur between phases.
        let new_use = match attachment {
            Attachment::Attached(buffer) => Some(self.buffers.begin_use_id(client, buffer)
                .expect("validated buffer use")),
            _ => None,
        };
        if let Some(size) = actual {
            self.wire.commit(client, object, size).expect("validated role commit");
        } else if mapped {
            self.wire.unmap(client, object).expect("validated role unmap");
        }
        let committed = surface.commit().expect("validated surface generation");
        if !mapped && actual.is_some() {
            self.frames.mapped(id, committed.generation);
        } else if mapped && actual.is_none() {
            self.frames.unmapped(id);
        }
        self.frames.committed(id, committed.generation, &committed.frame_requests);
        Ok(Committed { window: id, surface: committed, new_use })
    }

    /// Trusted render scheduler reports a visible frame boundary for the
    /// generation it actually processed. This is not a presentation timestamp
    /// or storage-release signal. The backend must emit each returned callback
    /// once and retire its resource, or disconnect on transport failure.
    pub fn frame_ready(&mut self, id: WindowId, generation: u64, visible: bool) -> Result<Vec<u64>, Error> {
        let surface = self.surfaces.get(&id).ok_or(Error::InconsistentContent)?;
        if generation == 0 || generation > surface.generation() { return Err(Error::InvalidGeneration); }
        Ok(self.frames.ready(id, generation, visible))
    }

    pub fn complete(&mut self, token: UseId) -> Result<Completion, Error> {
        Ok(self.buffers.complete(token)?)
    }

    pub fn destroy(&mut self, client: ClientId, object: u32) -> Result<Vec<u64>, Error> {
        let id = self.wire.resolve(client, object)?;
        self.wire.destroy(client, object)?;
        let mut pending = self.surfaces.remove(&id).expect("registered surface").discard_pending();
        let committed = self.frames.cancel(id, &pending);
        pending.extend(committed);
        Ok(pending)
    }

    pub fn disconnect(&mut self, client: ClientId) -> Result<Disconnected, Error> {
        let windows = self.wire.disconnect(client)?;
        let mut pending_callbacks = Vec::new();
        let mut committed_callbacks = Vec::new();
        for id in &windows {
            let pending = self.surfaces.remove(id).expect("registered surface").discard_pending();
            committed_callbacks.extend(self.frames.cancel(*id, &pending));
            pending_callbacks.extend(pending);
        }
        let buffers = self.buffers.disconnect(client)?;
        Ok(Disconnected { windows, pending_callbacks, committed_callbacks, buffers })
    }
}
