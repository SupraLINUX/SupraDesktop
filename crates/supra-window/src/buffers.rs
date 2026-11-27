//! Backend-independent ownership ledger for committed buffer use.
//! Renderer completion, not frame callbacks, permits release. No mapped
//! memory, GPU synchronization, Wayland event emission or import here.
use crate::clients::ClientId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BufferId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UseId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    UnknownBuffer,
    UnknownUse,
    DuplicateObject,
    InvalidObject,
    Exhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    StillInUse,
    Release { client: ClientId, object: u32, buffer: BufferId },
    Retired { buffer: BufferId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destroy {
    Retired(BufferId),
    Deferred(BufferId),
}

struct Buffer {
    owner: ClientId,
    object: Option<u32>,
    uses: BTreeSet<UseId>,
}

/// Backend owns connection registration and serializes these operations.
/// Internal IDs are never recycled; late renderer completions cannot target
/// a newly registered protocol object with the same numeric ID.
#[derive(Default)]
pub struct Buffers {
    objects: BTreeMap<(ClientId, u32), BufferId>,
    entries: BTreeMap<BufferId, Buffer>,
    uses: BTreeMap<UseId, BufferId>,
    next_buffer: u64,
    next_use: u64,
}

impl Buffers {
    pub fn register(&mut self, owner: ClientId, object: u32) -> Result<BufferId, Error> {
        if object == 0 { return Err(Error::InvalidObject); }
        if self.objects.contains_key(&(owner, object)) { return Err(Error::DuplicateObject); }
        let id = BufferId(self.next_buffer.checked_add(1).ok_or(Error::Exhausted)?);
        self.next_buffer = id.0;
        self.objects.insert((owner, object), id);
        self.entries.insert(id, Buffer { owner, object: Some(object), uses: BTreeSet::new() });
        Ok(id)
    }

    /// Record only actual committed use, not a pending attach. Multiple
    /// internal consumers may hold the same buffer until independent completion.
    pub fn begin_use(&mut self, owner: ClientId, object: u32) -> Result<UseId, Error> {
        let id = *self.objects.get(&(owner, object)).ok_or(Error::UnknownBuffer)?;
        self.begin_use_id(owner, id)
    }

    /// Resolve the stable attachment identity, not a potentially recycled
    /// protocol object number. Retired and foreign buffers are rejected.
    pub(crate) fn validate_use_id(&self, owner: ClientId, id: BufferId) -> Result<(), Error> {
        let entry = self.entries.get(&id).ok_or(Error::UnknownBuffer)?;
        if entry.owner != owner || entry.object.is_none() { return Err(Error::UnknownBuffer); }
        self.next_use.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(())
    }

    pub(crate) fn begin_use_id(&mut self, owner: ClientId, id: BufferId) -> Result<UseId, Error> {
        self.validate_use_id(owner, id)?;
        let entry = self.entries.get_mut(&id).ok_or(Error::UnknownBuffer)?;
        let token = UseId(self.next_use.checked_add(1).ok_or(Error::Exhausted)?);
        self.next_use = token.0;
        entry.uses.insert(token);
        self.uses.insert(token, id);
        Ok(token)
    }

    /// Trusted renderer reports that this use no longer needs the backing
    /// storage. A release disposition is only for a still-live object.
    pub fn complete(&mut self, token: UseId) -> Result<Completion, Error> {
        let id = self.uses.remove(&token).ok_or(Error::UnknownUse)?;
        let entry = self.entries.get_mut(&id).ok_or(Error::UnknownBuffer)?;
        entry.uses.remove(&token);
        if !entry.uses.is_empty() { return Ok(Completion::StillInUse); }
        if let Some(object) = entry.object {
            return Ok(Completion::Release { client: entry.owner, object, buffer: id });
        }
        self.entries.remove(&id);
        Ok(Completion::Retired { buffer: id })
    }

    /// Destroy removes the protocol resource immediately, but in-flight use
    /// retains backing ownership until the last renderer completion.
    pub fn destroy(&mut self, owner: ClientId, object: u32) -> Result<Destroy, Error> {
        let id = self.objects.remove(&(owner, object)).ok_or(Error::UnknownBuffer)?;
        let entry = self.entries.get_mut(&id).ok_or(Error::UnknownBuffer)?;
        entry.object = None;
        if entry.uses.is_empty() {
            self.entries.remove(&id);
            Ok(Destroy::Retired(id))
        } else {
            Ok(Destroy::Deferred(id))
        }
    }

    pub fn disconnect(&mut self, owner: ClientId) -> Result<Vec<Destroy>, Error> {
        let objects: Vec<u32> = self.objects.keys()
            .filter_map(|(client, object)| (*client == owner).then_some(*object)).collect();
        objects.into_iter().map(|object| self.destroy(owner, object)).collect()
    }
}
