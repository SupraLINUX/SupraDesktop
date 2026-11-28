//! Optical renderer adapter using the same submission lifecycle as ordinary CPU.
use super::{Boundary,Error,FrameToken,Placement};
use supra_scene::Rect;
use supra_window::{WindowId,cpu_content::CpuCoordinator};
use crate::optical::{Glass,SignedOpticalLayer,incremental::{Incremental,Work}};

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct OpticalPlacement {
    pub placement:Placement,
    /// Trusted shell/compositor policy, never raw unvalidated client material.
    pub glass:Option<Glass>,
}

#[derive(Debug)]
pub struct OpticalSubmission {
    pub token:FrameToken,
    pub work:Work,
}

/// Independent optical backend with shared token/generation semantics.
/// Its existing reference limits remain: 16 layers, 262144 output pixels.
pub struct OpticalPresentation {
    boundary:Boundary,
    output:Rect,
    frame:Incremental,
}

impl OpticalPresentation {
    pub fn new(coordinator:&CpuCoordinator,width:u32,height:u32)->Result<Self,Error> {
        let frame=Incremental::new(width,height).map_err(Error::Render)?;
        let output=Rect::new(0,0,width,height).ok_or(Error::Render(crate::Error::InvalidOutput))?;
        Ok(Self{boundary:Boundary::new(coordinator),output,frame})
    }

    pub fn submit(&mut self,coordinator:&CpuCoordinator,placements:&[OpticalPlacement])->Result<OpticalSubmission,Error> {
        let captured=self.boundary.capture(coordinator,self.output,placements.iter().map(|p|p.placement),16)?;
        let layers=captured.surfaces.into_iter().zip(placements).map(|(placed,p)|
            SignedOpticalLayer{placed,glass:p.glass}).collect();
        let work=self.frame.update_signed(layers).map_err(Error::Render)?;
        let token=self.boundary.submitted(captured.visible);
        Ok(OpticalSubmission{token,work})
    }

    pub fn pixels(&self,token:&FrameToken)->Result<&[[u8;4]],Error> {
        self.boundary.pending(token)?;
        Ok(self.frame.pixels())
    }

    pub fn complete(&mut self,coordinator:&mut CpuCoordinator,token:&FrameToken)->Result<Vec<(WindowId,Vec<u64>)>,Error> {
        self.boundary.complete(coordinator,token)
    }

    pub fn discard(&mut self,token:&FrameToken)->Result<(),Error> {
        self.boundary.discard(token)
    }
}
