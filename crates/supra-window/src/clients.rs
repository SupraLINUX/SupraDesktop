//! Connection-scoped window dispatch; the backend must provide authenticated
//! connection identity from its own transport, never from a client payload.
//! No wire serials, buffer ownership or seat authorization here.
use crate::{Configure, Error as WindowError, Size, Window, WindowId, Windows};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClientId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    UnknownClient,
    UnknownObject,
    DuplicateObject,
    InvalidObject,
    Exhausted,
    Window(WindowError),
}

impl From<WindowError> for Error {
    fn from(error: WindowError) -> Self { Self::Window(error) }
}

/// Object IDs are local to a connection; internal IDs are never reused.
/// Windows remains the sole owner of mapping, stacking and focus state.
#[derive(Default)]
pub struct Connections {
    clients: BTreeMap<ClientId, BTreeMap<u32, WindowId>>,
    windows: Windows,
    next_client: u64,
    next_window: u64,
}

impl Connections {
    /// Trusted transport calls this for a new connection.
    pub fn connect(&mut self) -> Result<ClientId, Error> {
        let id = ClientId(self.next_client.checked_add(1).ok_or(Error::Exhausted)?);
        self.next_client = id.0;
        self.clients.insert(id, BTreeMap::new());
        Ok(id)
    }

    pub fn create(&mut self, client: ClientId, object: u32) -> Result<WindowId, Error> {
        if object == 0 { return Err(Error::InvalidObject); }
        let objects = self.clients.get_mut(&client).ok_or(Error::UnknownClient)?;
        if objects.contains_key(&object) { return Err(Error::DuplicateObject); }
        let id = self.next_window.checked_add(1).ok_or(Error::Exhausted)?;
        self.windows.create(id)?;
        self.next_window = id;
        objects.insert(object, id);
        Ok(id)
    }

    pub(crate) fn is_live(&self, client: ClientId) -> bool { self.clients.contains_key(&client) }

    pub fn resolve(&self, client: ClientId, object: u32) -> Result<WindowId, Error> {
        self.clients.get(&client).ok_or(Error::UnknownClient)?
            .get(&object).copied().ok_or(Error::UnknownObject)
    }

    pub fn acknowledge(&mut self, client: ClientId, object: u32, token: u64) -> Result<(), Error> {
        let id = self.resolve(client, object)?;
        Ok(self.windows.acknowledge(id, token)?)
    }

    pub(crate) fn validate_commit(&self, client: ClientId, object: u32, actual: Size) -> Result<(), Error> {
        Ok(self.windows.validate_commit(self.resolve(client, object)?, actual)?)
    }

    pub fn commit(&mut self, client: ClientId, object: u32, actual: Size) -> Result<(), Error> {
        let id = self.resolve(client, object)?;
        Ok(self.windows.commit(id, actual)?)
    }

    pub fn unmap(&mut self, client: ClientId, object: u32) -> Result<(), Error> {
        let id = self.resolve(client, object)?;
        Ok(self.windows.unmap(id)?)
    }

    pub fn destroy(&mut self, client: ClientId, object: u32) -> Result<(), Error> {
        let id = self.resolve(client, object)?;
        self.windows.destroy(id)?;
        self.clients.get_mut(&client).ok_or(Error::UnknownClient)?.remove(&object);
        Ok(())
    }

    /// Remove all windows for one connection, preserving surviving clients.
    /// Returns removed internal IDs for renderer/backend cleanup.
    pub fn disconnect(&mut self, client: ClientId) -> Result<Vec<WindowId>, Error> {
        let objects = self.clients.remove(&client).ok_or(Error::UnknownClient)?;
        let mut removed = Vec::with_capacity(objects.len());
        for id in objects.into_values() {
            self.windows.destroy(id)?;
            removed.push(id);
        }
        Ok(removed)
    }

    pub fn get(&self, id: WindowId) -> Option<&Window> { self.windows.get(id) }
    pub fn focus(&self) -> Option<WindowId> { self.windows.focus() }
    pub fn stacking(&self) -> &[WindowId] { self.windows.stacking() }

    // Trusted compositor policy methods. Never dispatch arbitrary client
    // requests to these methods without the separate authorization policy.
    pub fn configure(&mut self, id: WindowId, suggested: Size) -> Result<Configure, Error> {
        Ok(self.windows.configure(id, suggested)?)
    }

    pub fn activate(&mut self, id: WindowId) -> Result<(), Error> {
        Ok(self.windows.activate(id)?)
    }

    pub fn request_close(&mut self, id: WindowId) -> Result<(), Error> {
        Ok(self.windows.request_close(id)?)
    }
}
