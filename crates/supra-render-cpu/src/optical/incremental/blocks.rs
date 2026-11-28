//! Persistent linear pixel chunks. A private clone shares immutable chunks
//! until the first actual pixel change. No external dependency or unsafe code.
use std::sync::Arc;

pub(super) type Pixel = [u8;4];
const CHUNK:usize=64;

#[derive(Clone)]
pub(super) struct Prefix {
    blocks:Vec<Arc<Vec<Pixel>>>,
    len:usize,
}

impl Prefix {
    pub(super) fn black(len:usize)->Self {
        let full=Arc::new(vec![[0,0,0,255];CHUNK]);
        let mut blocks=vec![full;len/CHUNK];
        if len%CHUNK!=0 {
            blocks.push(Arc::new(vec![[0,0,0,255];len%CHUNK]));
        }
        Self{blocks,len}
    }

    pub(super) fn get(&self,index:usize)->Pixel {
        assert!(index<self.len);
        self.blocks[index/CHUNK][index%CHUNK]
    }

    /// Number of payload pixels copied by this COW write, excluding metadata.
    pub(super) fn set(&mut self,index:usize,value:Pixel)->usize {
        assert!(index<self.len);
        let block=&mut self.blocks[index/CHUNK];
        if block[index%CHUNK]==value { return 0; }
        // No Weak references are created or exposed by this module.
        let copied=if Arc::strong_count(block)>1 { block.len() } else { 0 };
        Arc::make_mut(block)[index%CHUNK]=value;
        copied
    }

    /// Publish into a contiguous consumer buffer only where identity differs.
    /// Comparison of equal values in distinct blocks is intentionally avoided.
    pub(super) fn publish(&self,old:&Self,output:&mut [Pixel])->usize {
        assert_eq!(self.len,old.len);
        assert_eq!(self.len,output.len());
        let mut count=0;
        for (index,(new,prior)) in self.blocks.iter().zip(&old.blocks).enumerate() {
            if !Arc::ptr_eq(new,prior) {
                let start=index*CHUNK;
                output[start..start+new.len()].copy_from_slice(new);
                count+=new.len();
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_writes_and_short_last_block() {
        let a=Prefix::black(131);
        let mut b=a.clone();
        assert_eq!(b.set(63,[1,2,3,255]),64);
        assert_eq!(b.set(62,[4,5,6,255]),0);
        assert_eq!(b.set(64,[0,0,0,255]),0);
        assert_eq!(b.set(130,[7,8,9,255]),3);
        assert_eq!(a.get(63),[0,0,0,255]);
        let mut out=vec![[0,0,0,255];131];
        assert_eq!(b.publish(&a,&mut out),67);
        assert_eq!(out[130],[7,8,9,255]);
        assert_eq!(out[64],[0,0,0,255]);
        assert_eq!(b.publish(&b.clone(),&mut out),0);
        assert_eq!(a.publish(&b,&mut out),67);
        assert!(out.iter().all(|p| *p==[0,0,0,255]));
    }
}
