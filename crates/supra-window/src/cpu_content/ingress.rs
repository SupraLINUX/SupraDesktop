//! Backend-facing xdg-toplevel handshake, independent of actual transport.
//! Only trusted transport calls these methods with its authenticated client
//! identity and copied/imported buffer view. Existing core owns all content,
//! configure serials, generations, buffer use and callback queues.
use super::{CpuCoordinator,CpuCommit,Error as CoreError,View};
use crate::{Configure,Size,WindowId,buffers::{BufferId,Destroy},clients::ClientId,
    commit::Disconnected,surface::Transform};
use std::collections::BTreeMap;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Error {
    Core(CoreError),
    InitialCommitRequired,
    ConfigureRequired,
    UnknownToplevel,
}
impl From<CoreError> for Error {
    fn from(value:CoreError)->Self { Self::Core(value) }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Handshake { Initial, AwaitConfigure, Configured }

/// Owns core; no mutable coordinator escape. A registered role is assumed
/// already constructed by the transport. It must commit no content before
/// the first configure; a mapped null-buffer commit resets the handshake.
pub struct XdgIngress {
    core:CpuCoordinator,
    handshake:BTreeMap<WindowId,Handshake>,
}
impl XdgIngress {
    pub fn new(max_bytes:usize)->Self {
        Self{core:CpuCoordinator::new(max_bytes),handshake:BTreeMap::new()}
    }
    pub fn coordinator(&self)->&CpuCoordinator { &self.core }
    pub fn connect(&mut self)->Result<ClientId,Error> { Ok(self.core.connect()?) }
    pub fn create_toplevel(&mut self,c:ClientId,object:u32)->Result<WindowId,Error> {
        let id=self.core.create(c,object)?;
        self.handshake.insert(id,Handshake::Initial);
        Ok(id)
    }
    fn resolve(&self,c:ClientId,object:u32)->Result<WindowId,Error> {
        self.core.logical.resolve(c,object).map_err(CoreError::from).map_err(Error::from)
    }
    fn state(&self,id:WindowId)->Result<Handshake,Error> {
        self.handshake.get(&id).copied().ok_or(Error::UnknownToplevel)
    }
    pub fn register_buffer(&mut self,c:ClientId,object:u32)->Result<BufferId,Error> {
        Ok(self.core.register_buffer(c,object)?)
    }
    pub fn destroy_buffer(&mut self,c:ClientId,object:u32)->Result<Destroy,Error> {
        Ok(self.core.destroy_buffer(c,object)?)
    }
    pub fn configure(&mut self,id:WindowId,serial:u32,size:Size)->Result<Configure,Error> {
        if self.state(id)?==Handshake::Initial { return Err(Error::InitialCommitRequired); }
        let event=self.core.configure(id,serial,size)?;
        self.handshake.insert(id,Handshake::Configured);
        Ok(event)
    }
    pub fn acknowledge(&mut self,c:ClientId,object:u32,serial:u32)->Result<(),Error> {
        if self.state(self.resolve(c,object)?)?!=Handshake::Configured { return Err(Error::ConfigureRequired); }
        Ok(self.core.acknowledge(c,object,serial)?)
    }
    pub fn attach(&mut self,c:ClientId,object:u32,buffer:Option<BufferId>)->Result<(),Error> {
        if buffer.is_some() && self.state(self.resolve(c,object)?)?!=Handshake::Configured {
            return Err(Error::ConfigureRequired);
        }
        Ok(self.core.attach(c,object,buffer)?)
    }
    pub fn set_scale(&mut self,c:ClientId,object:u32,scale:i32)->Result<(),Error> {
        Ok(self.core.set_scale(c,object,scale)?)
    }
    pub fn set_transform(&mut self,c:ClientId,object:u32,transform:Transform)->Result<(),Error> {
        Ok(self.core.set_transform(c,object,transform)?)
    }
    pub fn frame(&mut self,c:ClientId,object:u32,callback:u64)->Result<(),Error> {
        Ok(self.core.frame(c,object,callback)?)
    }
    pub fn commit(&mut self,c:ClientId,object:u32,view:Option<View<'_>>)->Result<CpuCommit,Error> {
        let id=self.resolve(c,object)?;
        let state=self.state(id)?;
        let surface=self.core.logical.surface(id).ok_or(Error::UnknownToplevel)?;
        let mapped=surface.current().buffer.is_some();
        let next=surface.preview().buffer.is_some();
        if next && state!=Handshake::Configured { return Err(Error::ConfigureRequired); }
        let commit=self.core.commit(c,object,view)?;
        if state==Handshake::Initial {
            self.handshake.insert(id,Handshake::AwaitConfigure);
        } else if mapped && !next {
            self.handshake.insert(id,Handshake::Initial);
        }
        Ok(commit)
    }
    pub fn destroy(&mut self,c:ClientId,object:u32)->Result<Vec<u64>,Error> {
        let id=self.resolve(c,object)?;
        let callbacks=self.core.destroy(c,object)?;
        self.handshake.remove(&id);
        Ok(callbacks)
    }
    pub fn disconnect(&mut self,c:ClientId)->Result<Disconnected,Error> {
        let disconnected=self.core.disconnect(c)?;
        for id in &disconnected.windows { self.handshake.remove(id); }
        Ok(disconnected)
    }
    /// Only a trusted output completion boundary calls this, never client
    /// dispatch. Identical canonical scheduler used by existing CPU path.
    pub fn frame_ready_batch(&mut self,boundaries:&[(WindowId,u64)])->Result<Vec<(WindowId,Vec<u64>)>,CoreError> {
        self.core.frame_ready_batch(boundaries)
    }
}
