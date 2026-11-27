//! Original double-buffered surface state contract.
//! A backend must resolve protocol resources and validate the complete commit
//! with its role, renderer and buffer ledger before publishing the result.
use crate::buffers::BufferId;

/// Experimental per-surface pending frame request limit, not a protocol limit.
pub const MAX_PENDING_FRAMES: usize = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Attachment {
    #[default]
    Unchanged,
    Detached,
    Attached(BufferId),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transform {
    #[default]
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// Logical content identity, not proof that backing storage is retained.
    pub buffer: Option<BufferId>,
    pub scale: i32,
    pub transform: Transform,
}

impl Default for State {
    fn default() -> Self {
        Self { buffer: None, scale: 1, transform: Transform::Normal }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidScale,
    InvalidFrame,
    DuplicateFrame,
    FrameQueueFull,
    Exhausted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub generation: u64,
    pub attachment: Attachment,
    pub previous: State,
    pub current: State,
    /// Hand off to presentation scheduling; these are not release events.
    pub frame_requests: Vec<u64>,
}

#[derive(Default)]
pub struct Surface {
    current: State,
    attachment: Attachment,
    scale: Option<i32>,
    transform: Option<Transform>,
    frames: Vec<u64>,
    generation: u64,
}

impl Surface {
    pub fn current(&self) -> State { self.current }
    pub fn generation(&self) -> u64 { self.generation }

    /// A replacement before commit has no committed use in the buffer ledger.
    pub fn attach(&mut self, buffer: Option<BufferId>) {
        self.attachment = match buffer {
            Some(id) => Attachment::Attached(id),
            None => Attachment::Detached,
        };
    }

    pub fn set_scale(&mut self, scale: i32) -> Result<(), Error> {
        if scale <= 0 { return Err(Error::InvalidScale); }
        self.scale = Some(scale);
        Ok(())
    }

    pub fn set_transform(&mut self, transform: Transform) {
        self.transform = Some(transform);
    }

    /// Trusted backend supplies a unique callback identity, not a raw wire ID.
    /// Backend owns the identity until delivery or cancellation.
    pub fn frame(&mut self, id: u64) -> Result<(), Error> {
        if id == 0 { return Err(Error::InvalidFrame); }
        if self.frames.contains(&id) { return Err(Error::DuplicateFrame); }
        if self.frames.len() >= MAX_PENDING_FRAMES { return Err(Error::FrameQueueFull); }
        self.frames.push(id);
        Ok(())
    }

    /// Preview without mutating any state. Role/import validation must precede
    /// commit; actual protocol/GPU integration is outside this pure module.
    pub fn preview(&self) -> State {
        State {
            buffer: match self.attachment {
                Attachment::Unchanged => self.current.buffer,
                Attachment::Detached => None,
                Attachment::Attached(id) => Some(id),
            },
            scale: self.scale.unwrap_or(self.current.scale),
            transform: self.transform.unwrap_or(self.current.transform),
        }
    }

    /// Call only after validating this pending state in the serialized backend.
    /// No actual buffer use, presentation or event emission occurs here.
    pub fn commit(&mut self) -> Result<Commit, Error> {
        let generation = self.generation.checked_add(1).ok_or(Error::Exhausted)?;
        let next = self.preview();
        let result = Commit {
            generation,
            attachment: std::mem::take(&mut self.attachment),
            previous: self.current,
            current: next,
            frame_requests: std::mem::take(&mut self.frames),
        };
        self.current = next;
        self.generation = generation;
        self.scale = None;
        self.transform = None;
        Ok(result)
    }

    /// Destroying the surface discards uncommitted changes, returning pending
    /// callbacks so the backend can destroy resources without signaling done.
    /// Already committed callbacks belong to the presentation scheduler.
    pub fn discard_pending(&mut self) -> Vec<u64> {
        self.attachment = Attachment::Unchanged;
        self.scale = None;
        self.transform = None;
        std::mem::take(&mut self.frames)
    }
}
