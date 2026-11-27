use supra_scene::{Rect,disjoint_regions};

#[test]
fn union_is_exact_nonoverlapping_and_clipped() {
    let out = Rect::new(0,0,19,13).unwrap();
    let r = [Rect::new(1,1,9,6).unwrap(),Rect::new(4,2,20,9).unwrap(),
             Rect::new(1,7,5,12).unwrap(),Rect::new(5,7,6,12).unwrap()];
    let union = disjoint_regions(&r,out);
    for y in 0..13 {
        for x in 0..19 {
            assert_eq!(union.iter().filter(|a| a.contains(x,y)).count(),
                       usize::from(r.iter().any(|a| a.contains(x,y))));
        }
    }
    assert!(disjoint_regions(&[],out).is_empty());
    assert_eq!(disjoint_regions(&[out,out],out),vec![out]);
}
