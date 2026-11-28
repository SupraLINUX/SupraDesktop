//! Controlled CPU lab comparison. No deterministic timing or GPU claim.
use crate::window_optics_fixture::{clone_layer,fixture_layers};
use std::{fmt::Write,time::Instant};
use supra_render_cpu::optical::{OpticalLayer,compose,incremental::Incremental};

const SIZES:[usize;6]=[16,32,64,128,256,512];
const REPEATS:usize=7;

fn frames(clipped:bool)->Vec<Vec<OpticalLayer>> {
    let layers=fixture_layers();
    (0..24).map(|step| {
        let mut next:Vec<_>=layers.iter().map(clone_layer).collect();
        next[1].placed.x+=(step/3)%8;
        next[2].placed.y+=(step/5)%5;
        if clipped && step>=12 { next[2].placed.x=145+(step%5); }
        if step>=20 { next.swap(1,2); }
        if clipped && step==23 { next.clear(); }
        next
    }).collect()
}

#[derive(Default,Debug,PartialEq,Eq)]
struct Counts { copied:usize, published:usize, prefix:usize }

fn trial(w:u32,h:u32,chunk:usize,sequence:&[Vec<OpticalLayer>],expected:&[Vec<[u8;4]>])->(u128,Counts) {
    let mut renderer=Incremental::with_chunk_size(w,h,chunk).unwrap();
    let mut nanos=0;
    let mut counts=Counts::default();
    for (layers,oracle) in sequence.iter().zip(expected) {
        let input=layers.iter().map(clone_layer).collect();
        let start=Instant::now();
        let result=renderer.update(input);
        nanos+=start.elapsed().as_nanos();
        let work=result.unwrap();
        assert_eq!(renderer.pixels(),oracle.as_slice());
        counts.copied+=work.cow_copied_pixels;
        counts.published+=work.published_pixels;
        counts.prefix+=work.prefix_pixels;
    }
    (nanos,counts)
}

pub fn csv()->String {
    let mut out=String::from("scenario,chunk_pixels,repeat,frames,update_ns,cow_copied_pixels,published_pixels,prefix_pixels,pixel_exact\n");
    for (scenario,w,h,clipped) in [("movement",160,96,false),("clipped",159,95,true)] {
        let sequence=frames(clipped);
        let expected:Vec<_>=sequence.iter().map(|layers|compose(w,h,layers).unwrap()).collect();
        // Full oracle and input creation are outside the timed region.
        // Warm each size once; rotate measurement order on the same runner.
        let warm:Vec<_>=SIZES.iter().map(|&chunk|trial(w,h,chunk,&sequence,&expected).1).collect();
        for repeat in 0..REPEATS {
            for offset in 0..SIZES.len() {
                let index=(offset+repeat)%SIZES.len();
                let chunk=SIZES[index];
                let (ns,counts)=trial(w,h,chunk,&sequence,&expected);
                assert_eq!(counts,warm[index]);
                writeln!(out,"{scenario},{chunk},{repeat},{},{ns},{},{},{},true",
                    sequence.len(),counts.copied,counts.published,counts.prefix).unwrap();
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_chunk_sizes_match_oracle_at_non_multiple_dimensions() {
        let sequence=frames(true);
        let expected:Vec<_>=sequence.iter().map(|layers|compose(159,95,layers).unwrap()).collect();
        for size in SIZES {
            trial(159,95,size,&sequence,&expected);
        }
        for size in [0,15,17,1024] {
            assert!(Incremental::with_chunk_size(159,95,size).is_err());
        }
    }
}
