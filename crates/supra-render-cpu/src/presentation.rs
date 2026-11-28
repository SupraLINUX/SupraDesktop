//! Single-output CPU presentation adapter. The trusted output backend owns
//! submission completion and visibility policy; no wire or GPU timing claims.
use std::sync::Arc;
use supra_window::{WindowId, cpu_content::{CoordinatorIdentity,CpuCoordinator,ingress::XdgIngress}};
use crate::{Frame,SignedPlaced};

const MAX_PLACEMENTS:usize=256;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Placement {
    pub window:WindowId,
    pub x:i64,
    pub y:i64,
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

mod optical;
pub use optical::{OpticalPlacement,OpticalPresentation,OpticalSubmission};

struct Pending {
    token:FrameToken,
    visible:Vec<(WindowId,u64)>,
}

struct Captured {
    surfaces:Vec<SignedPlaced>,
    visible:Vec<(WindowId,u64)>,
}

/// Common submission lifecycle. Rendering backends own their pixels; this
/// is the sole adapter authority over pending tokens and callback boundaries.
struct Boundary {
    owner:CoordinatorIdentity,
    pending:Option<Pending>,
}

impl Boundary {
    fn new(coordinator:&CpuCoordinator)->Self {
        Self{owner:coordinator.identity(),pending:None}
    }

    fn capture(&self,coordinator:&CpuCoordinator,output:supra_scene::Rect,
        placements:impl ExactSizeIterator<Item=Placement>,limit:usize)->Result<Captured,Error> {
        if !coordinator.matches_identity(&self.owner) { return Err(Error::WrongCoordinator); }
        if self.pending.is_some() { return Err(Error::Busy); }
        if placements.len()>limit { return Err(Error::Capacity); }
        let mut surfaces=Vec::with_capacity(placements.len());
        let mut visible=Vec::new();
        for p in placements {
            let snapshot=coordinator.snapshot(p.window).ok_or(Error::UnknownWindow)?;
            let surface=SignedPlaced{x:p.x,y:p.y,snapshot};
            if let Some(layer)=surface.visible_layer(output).map_err(Error::Render)? {
                if p.callback_eligible { visible.push((p.window,layer.revision)); }
            }
            surfaces.push(surface);
        }
        Ok(Captured{surfaces,visible})
    }

    // Publish only after a successful transactional render.
    fn submitted(&mut self,visible:Vec<(WindowId,u64)>)->FrameToken {
        let token=FrameToken(Arc::new(()));
        self.pending=Some(Pending{token:token.clone(),visible});
        token
    }

    fn pending(&self,token:&FrameToken)->Result<&Pending,Error> {
        self.pending.as_ref().filter(|p|Arc::ptr_eq(&p.token.0,&token.0)).ok_or(Error::UnknownFrame)
    }

    fn complete(&mut self,coordinator:&mut CpuCoordinator,token:&FrameToken)->Result<Vec<(WindowId,Vec<u64>)>,Error> {
        let matches=coordinator.matches_identity(&self.owner);
        self.complete_with(matches,token,|visible|coordinator.frame_ready_batch(visible))
    }

    fn complete_ingress(&mut self,ingress:&mut XdgIngress,token:&FrameToken)->Result<Vec<(WindowId,Vec<u64>)>,Error> {
        let matches=ingress.coordinator().matches_identity(&self.owner);
        self.complete_with(matches,token,|visible|ingress.frame_ready_batch(visible))
    }

    fn complete_with(&mut self,matches:bool,token:&FrameToken,finish:impl FnOnce(&[(WindowId,u64)])
        ->Result<Vec<(WindowId,Vec<u64>)>,supra_window::cpu_content::Error>)->Result<Vec<(WindowId,Vec<u64>)>,Error> {
        if !matches { return Err(Error::WrongCoordinator); }
        let pending=self.pending(token)?;
        let ready=finish(&pending.visible).map_err(Error::Logical)?;
        self.pending=None;
        Ok(ready)
    }

    fn discard(&mut self,token:&FrameToken)->Result<(),Error> {
        self.pending(token)?;
        self.pending=None;
        Ok(())
    }
}

/// One in-flight image per output: pixels remain stable until the backend
/// acknowledges or discards that submission. CPU cache survives a discard;
/// only a subsequent accepted boundary may consume frame callbacks.
pub struct CpuPresentation {
    boundary:Boundary,
    frame:Frame,
}

impl CpuPresentation {
    pub fn new(coordinator:&CpuCoordinator,width:u32,height:u32,max_pixels:usize)->Result<Self,Error> {
        Ok(Self{boundary:Boundary::new(coordinator),frame:Frame::new(width,height,max_pixels).map_err(Error::Render)?})
    }

    /// Placements are trusted scene policy, bottom to top. Acquire actual
    /// retained snapshots from the canonical coordinator.
    pub fn submit(&mut self,coordinator:&CpuCoordinator,placements:&[Placement])->Result<Submission,Error> {
        let captured=self.boundary.capture(coordinator,self.frame.output,placements.iter().copied(),MAX_PLACEMENTS)?;
        let recomposited_pixels=self.frame.update_signed(captured.surfaces).map_err(Error::Render)?;
        let token=self.boundary.submitted(captured.visible);
        Ok(Submission{token,recomposited_pixels})
    }

    pub fn pixels(&self,token:&FrameToken)->Result<&[[u8;4]],Error> {
        self.boundary.pending(token)?;
        Ok(self.frame.pixels())
    }

    /// Accepted backend boundary, not presentation-time or storage release.
    pub fn complete(&mut self,coordinator:&mut CpuCoordinator,token:&FrameToken)->Result<Vec<(WindowId,Vec<u64>)>,Error> {
        self.boundary.complete(coordinator,token)
    }

    pub fn complete_ingress(&mut self,ingress:&mut XdgIngress,token:&FrameToken)->Result<Vec<(WindowId,Vec<u64>)>,Error> {
        self.boundary.complete_ingress(ingress,token)
    }

    pub fn discard(&mut self,token:&FrameToken)->Result<(),Error> {
        self.boundary.discard(token)
    }
}
