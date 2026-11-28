//! Signed placement geometry for ordinary CPU composition. Keep scene damage
//! rectangles in nonnegative physical output space; never rebase the sample
//! origin when clipping a surface.
use supra_scene::{Layer,Rect};
use supra_window::cpu_content::Snapshot;
use crate::{Error,Placed,sample_local};

pub struct SignedPlaced {
    pub x:i64,
    pub y:i64,
    pub snapshot:Snapshot,
}

impl From<Placed> for SignedPlaced {
    fn from(value:Placed)->Self {
        Self{x:i64::from(value.x),y:i64::from(value.y),snapshot:value.snapshot}
    }
}

impl SignedPlaced {
    pub(crate) fn visible_layer(&self,output:Rect)->Result<Option<Layer>,Error> {
        let s=&self.snapshot;
        let size=s.content.layout().logical_size(s.state).map_err(|_|Error::InvalidSurface)?;
        if s.state.buffer.is_none() || s.generation==0 { return Err(Error::InvalidSurface); }
        let right=self.x.checked_add(i64::from(size.width)).ok_or(Error::InvalidSurface)?;
        let bottom=self.y.checked_add(i64::from(size.height)).ok_or(Error::InvalidSurface)?;
        let (left,top,out_right,out_bottom)=output.edges();
        let x0=self.x.max(i64::from(left));
        let y0=self.y.max(i64::from(top));
        let x1=right.min(i64::from(out_right));
        let y1=bottom.min(i64::from(out_bottom));
        if x0>=x1 || y0>=y1 { return Ok(None); }
        let bounds=Rect::new(
            u32::try_from(x0).map_err(|_|Error::InvalidSurface)?,
            u32::try_from(y0).map_err(|_|Error::InvalidSurface)?,
            u32::try_from(x1).map_err(|_|Error::InvalidSurface)?,
            u32::try_from(y1).map_err(|_|Error::InvalidSurface)?,
        ).ok_or(Error::InvalidSurface)?;
        Ok(Some(Layer{id:s.window,bounds,revision:s.generation,backdrop_radius:None}))
    }

    pub(crate) fn sample(&self,x:u32,y:u32)->[u8;4] {
        // Caller only samples inside the validated visible rectangle.
        let local_x=(i64::from(x)-self.x) as u32;
        let local_y=(i64::from(y)-self.y) as u32;
        sample_local(&self.snapshot,local_x,local_y)
    }
}
