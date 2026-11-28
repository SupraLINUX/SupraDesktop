//! Single-output CPU presentation adapter. The trusted output backend owns
//! submission completion and visibility policy; no wire or GPU timing claims.
use std::sync::Arc;
use supra_window::{WindowId, cpu_content::{CoordinatorIdentity,CpuCoordinator}};
use crate::{Frame,Placed};

const MAX_PLACEMENTS:usize=256;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Placement {
    pub window:WindowId,
    pub x:u32,
    pub y:u32,
    /// Trusted compositor policy: false for a fully obscured surface.
    /// Geometry is additionally checked against the actual output.
    pub callback_eligible:bool,
}

#[derive(Clone,Debug)]
pub struct FrameToken(Arc<()>);

#[derive(Debug)]
pub struct Submission {
    pub token:FrameToken,
    pub recomposited_pixels:usize,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Error {
    WrongCoordinator,
    Busy,
    UnknownFrame,
    UnknownWindow,
    Capacity,
    Render(crate::Error),
    Logical(supra_window::cpu_content::Error),
}

struct Pending {
    token:FrameToken,
    visible:Vec<(WindowId,u64)>,
}

/// One in-flight image per output: pixels remain stable until the backend
/// acknowledges or discards that submission. CPU cache survives a discard;
/// only a subsequent accepted boundary may consume frame callbacks.
pub struct CpuPresentation {
    owner:CoordinatorIdentity,
    frame:Frame,
    pending:Option<Pending>,
}

impl CpuPresentation {
    pub fn new(coordinator:&CpuCoordinator,width:u32,height:u32,max_pixels:usize)->Result<Self,Error> {
        Ok(Self{owner:coordinator.identity(),frame:Frame::new(width,height,max_pixels).map_err(Error::Render)?,pending:None})
    }

    /// Placements are trusted scene policy, bottom to top. Acquire actual
    /// retained snapshots from the canonical coordinator, never caller-supplied
    /// generations or fabricated pixel snapshots.
    pub fn submit(&mut self,coordinator:&CpuCoordinator,placements:&[Placement])->Result<Submission,Error> {
        if !coordinator.matches_identity(&self.owner) { return Err(Error::WrongCoordinator); }
        if self.pending.is_some() { return Err(Error::Busy); }
        if placements.len()>MAX_PLACEMENTS { return Err(Error::Capacity); }
        let mut surfaces=Vec::with_capacity(placements.len());
        let mut visible=Vec::new();
        for p in placements {
            let snapshot=coordinator.snapshot(p.window).ok_or(Error::UnknownWindow)?;
            let surface=Placed{x:p.x,y:p.y,snapshot};
            let layer=surface.layer().map_err(Error::Render)?;
            if p.callback_eligible && layer.bounds.intersect(self.frame.output).is_some() {
                visible.push((p.window,layer.revision));
            }
            surfaces.push(surface);
        }
        let recomposited_pixels=self.frame.update(surfaces).map_err(Error::Render)?;
        let token=FrameToken(Arc::new(()));
        self.pending=Some(Pending{token:token.clone(),visible});
        Ok(Submission{token,recomposited_pixels})
    }

    fn pending(&self,token:&FrameToken)->Result<&Pending,Error> {
        self.pending.as_ref().filter(|p|Arc::ptr_eq(&p.token.0,&token.0)).ok_or(Error::UnknownFrame)
    }

    /// Valid only for the current submission, not a discarded/stale image.
    pub fn pixels(&self,token:&FrameToken)->Result<&[[u8;4]],Error> {
        self.pending(token)?;
        Ok(self.frame.pixels())
    }

    /// Trusted backend calls this at an accepted frame boundary. This returns
    /// throttling callback IDs, not wp_presentation feedback or buffer release.
    pub fn complete(&mut self,coordinator:&mut CpuCoordinator,token:&FrameToken)->Result<Vec<(WindowId,Vec<u64>)>,Error> {
        if !coordinator.matches_identity(&self.owner) { return Err(Error::WrongCoordinator); }
        let pending=self.pending(token)?;
        let ready=coordinator.frame_ready_batch(&pending.visible).map_err(Error::Logical)?;
        self.pending=None;
        Ok(ready)
    }

    pub fn discard(&mut self,token:&FrameToken)->Result<(),Error> {
        self.pending(token)?;
        self.pending=None;
        Ok(())
    }
}
