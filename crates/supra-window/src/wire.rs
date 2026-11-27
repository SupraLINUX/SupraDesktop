//! Original adapter contract for xdg_surface configure serials.
//! The eventual transport supplies wire serials and authenticated ClientId
//! dispatch. This module does not send protocol events or authorize input.
use crate::{Configure, Size, Window, WindowId, clients::{ClientId, Connections, Error as ClientError}};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Client(ClientError),
    SerialInUse,
    InvalidSerial,
}

impl From<ClientError> for Error {
    fn from(error: ClientError) -> Self { Self::Client(error) }
}

/// Own the connection model so internal and wire ack APIs cannot be mixed.
/// Serial equality is scoped to one live surface; ordering is FIFO, not
/// numerical, because uint32 wire serials can wrap.
#[derive(Default)]
pub struct WireWindows {
    connections: Connections,
    pending: BTreeMap<WindowId, VecDeque<(u32, u64)>>,
}

impl WireWindows {
    pub fn connect(&mut self) -> Result<ClientId, Error> {
        Ok(self.connections.connect()?)
    }

    pub fn create(&mut self, client: ClientId, object: u32) -> Result<WindowId, Error> {
        Ok(self.connections.create(client, object)?)
    }

    /// Called by the trusted backend before emitting a configure. A collision
    /// or core backpressure fails without mutating the pending queue.
    /// If transport emission fails afterwards, backend must destroy/disconnect
    /// the affected surface rather than pretending the event was delivered.
    pub fn configure(&mut self, id: WindowId, serial: u32, suggested: Size) -> Result<Configure, Error> {
        if self.pending.get(&id).is_some_and(|q| q.iter().any(|(s, _)| *s == serial)) {
            return Err(Error::SerialInUse);
        }
        let configure = self.connections.configure(id, suggested)?;
        self.pending.entry(id).or_default().push_back((serial, configure.token));
        Ok(configure)
    }

    pub fn acknowledge(&mut self, client: ClientId, object: u32, serial: u32) -> Result<(), Error> {
        let id = self.connections.resolve(client, object)?;
        let queue = self.pending.get_mut(&id).ok_or(Error::InvalidSerial)?;
        let index = queue.iter().position(|(s, _)| *s == serial).ok_or(Error::InvalidSerial)?;
        // Only consume wire mappings after the canonical state accepts the ack.
        self.connections.acknowledge(client, object, queue[index].1)?;
        queue.drain(..=index);
        if queue.is_empty() { self.pending.remove(&id); }
        Ok(())
    }

    pub fn commit(&mut self, client: ClientId, object: u32, actual: Size) -> Result<(), Error> {
        Ok(self.connections.commit(client, object, actual)?)
    }

    pub fn unmap(&mut self, client: ClientId, object: u32) -> Result<(), Error> {
        let id = self.connections.resolve(client, object)?;
        self.connections.unmap(client, object)?;
        self.pending.remove(&id);
        Ok(())
    }

    pub fn destroy(&mut self, client: ClientId, object: u32) -> Result<(), Error> {
        let id = self.connections.resolve(client, object)?;
        self.connections.destroy(client, object)?;
        self.pending.remove(&id);
        Ok(())
    }

    pub fn disconnect(&mut self, client: ClientId) -> Result<Vec<WindowId>, Error> {
        let removed = self.connections.disconnect(client)?;
        for id in &removed { self.pending.remove(id); }
        Ok(removed)
    }

    pub fn get(&self, id: WindowId) -> Option<&Window> { self.connections.get(id) }
    pub fn focus(&self) -> Option<WindowId> { self.connections.focus() }
    pub fn stacking(&self) -> &[WindowId] { self.connections.stacking() }

    pub fn activate(&mut self, id: WindowId) -> Result<(), Error> {
        Ok(self.connections.activate(id)?)
    }

    pub fn request_close(&mut self, id: WindowId) -> Result<(), Error> {
        Ok(self.connections.request_close(id)?)
    }
}
