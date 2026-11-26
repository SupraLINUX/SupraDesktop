//! Backend-independent experimental toplevel state model.
//! Not a Wayland protocol implementation: backend must translate configure
//! serials, buffer commits, input authorization and disconnect semantics.
use std::collections::BTreeMap;

pub type WindowId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    fn valid(self) -> bool {
        (1..=16384).contains(&self.width) && (1..=16384).contains(&self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configure {
    pub token: u64,
    pub suggested: Size,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Duplicate,
    Unknown,
    InvalidSize,
    InvalidState,
    StaleConfigure,
    Exhausted,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Window {
    committed: Option<Size>,
    pending: Option<Configure>,
    acknowledged: Option<Configure>,
    close_requested: bool,
}

impl Window {
    pub fn committed(&self) -> Option<Size> { self.committed }
    pub fn pending(&self) -> Option<Configure> { self.pending }
    pub fn mapped(&self) -> bool { self.committed.is_some() }
    pub fn close_requested(&self) -> bool { self.close_requested }
}

#[derive(Default)]
pub struct Windows {
    entries: BTreeMap<WindowId, Window>,
    stacking: Vec<WindowId>,
    focus: Option<WindowId>,
    next_token: u64,
}

impl Windows {
    pub fn create(&mut self, id: WindowId) -> Result<(), Error> {
        if self.entries.contains_key(&id) {
            return Err(Error::Duplicate);
        }
        self.entries.insert(id, Window::default());
        Ok(())
    }

    pub fn get(&self, id: WindowId) -> Option<&Window> { self.entries.get(&id) }
    pub fn focus(&self) -> Option<WindowId> { self.focus }
    /// Mapped windows ordered bottom to top.
    pub fn stacking(&self) -> &[WindowId] { &self.stacking }

    /// Internal tokens never wrap. The Wayland adapter will own wire serials.
    pub fn configure(&mut self, id: WindowId, suggested: Size) -> Result<Configure, Error> {
        if !suggested.valid() { return Err(Error::InvalidSize); }
        let entry = self.entries.get_mut(&id).ok_or(Error::Unknown)?;
        if entry.close_requested { return Err(Error::InvalidState); }
        let token = self.next_token.checked_add(1).ok_or(Error::Exhausted)?;
        self.next_token = token;
        let request = Configure { token, suggested };
        entry.pending = Some(request);
        Ok(request)
    }

    /// This lab coalesces unacknowledged requests to the latest token.
    /// An already acknowledged request remains eligible for the next commit.
    pub fn acknowledge(&mut self, id: WindowId, token: u64) -> Result<(), Error> {
        let entry = self.entries.get_mut(&id).ok_or(Error::Unknown)?;
        let request = entry.pending.filter(|p| p.token == token).ok_or(Error::StaleConfigure)?;
        if entry.close_requested { return Err(Error::InvalidState); }
        entry.pending = None;
        entry.acknowledged = Some(request);
        Ok(())
    }

    /// Initial map requires a configure acknowledgement. For mapped windows,
    /// content-only commits are legal. A suggested size is not a hard limit.
    pub fn commit(&mut self, id: WindowId, actual: Size) -> Result<(), Error> {
        if !actual.valid() { return Err(Error::InvalidSize); }
        let entry = self.entries.get_mut(&id).ok_or(Error::Unknown)?;
        if entry.close_requested || (!entry.mapped() && entry.acknowledged.is_none()) {
            return Err(Error::InvalidState);
        }
        if !entry.mapped() {
            self.stacking.push(id);
        }
        entry.committed = Some(actual);
        entry.acknowledged = None;
        Ok(())
    }

    /// Only a trusted compositor policy should call this. No client auth here.
    pub fn activate(&mut self, id: WindowId) -> Result<(), Error> {
        let entry = self.entries.get(&id).ok_or(Error::Unknown)?;
        if !entry.mapped() || entry.close_requested { return Err(Error::InvalidState); }
        self.stacking.retain(|&other| other != id);
        self.stacking.push(id);
        self.focus = Some(id);
        Ok(())
    }

    /// Close is advisory, not destruction. Keep the mapped surface until unmap.
    pub fn request_close(&mut self, id: WindowId) -> Result<(), Error> {
        let entry = self.entries.get_mut(&id).ok_or(Error::Unknown)?;
        if !entry.mapped() { return Err(Error::InvalidState); }
        entry.close_requested = true;
        entry.pending = None;
        entry.acknowledged = None;
        if self.focus == Some(id) { self.focus = None; }
        Ok(())
    }

    /// Unmap resets configure state; remap requires a fresh acknowledgement.
    pub fn unmap(&mut self, id: WindowId) -> Result<(), Error> {
        let entry = self.entries.get_mut(&id).ok_or(Error::Unknown)?;
        *entry = Window::default();
        self.stacking.retain(|&other| other != id);
        if self.focus == Some(id) { self.focus = None; }
        Ok(())
    }

    pub fn destroy(&mut self, id: WindowId) -> Result<(), Error> {
        self.unmap(id)?;
        self.entries.remove(&id);
        Ok(())
    }
}
