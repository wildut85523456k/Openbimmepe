//! Dumps a test scene as plain text for the independent Python checks (not a product data path).
use openbim_geom::*;
fn circle(cx: f64, cz: f64, r: f64, n: usize) -> Vec<[f64; 2]> {
    (0..n).map(|k| { let a = std::f64::consts::TAU * k as f64 / n as f64; [cx + r * a.cos(), cz + r * a.sin()] }).collect()
}
fn arch(x0: f64, w: f64, spring: f64, n: usize) -> Vec<[f64; 2]> {
    let r = w / 2.0;
    let mut v = vec![[x0, 0.0], [x0 + w, 0.0], [x0 + w, spring]];
    for k in 1..n { let a = std::f64::consts::PI * k as f64 / n as f64; v.push([x0 + r + r * a.cos(), spring + r * a.sin()]); }
    v.push([x0, spring]); v
}
fn main() {
    // 1. floor plan: a ring of rooms with a concave corner, three T-junction partitions, one angled
    let mut plan: Vec<WallChain> = Vec::new();
    let mut outer = WallChain::new(vec![[0.,0.],[12.,0.],[12.,6.],[7.,6.],[7.,9.],[0.,9.]], true, 0.3, 3.0);
    outer.openings = vec![vec![Opening{offset:2.0,width:1.0,sill:0.0,height:2.1}], vec![], vec![Opening{offset:1.0,width:1.5,sill:0.9,height:1.2}]];
    plan.push(outer.clone());
    // partition P3 runs left wall -> over to the angled stub; P1 hangs off P3 down to the south wall;
    // P2 is a free-standing stub that P3's angled end butts onto
    let mut p3 = WallChain::new(vec![[0.0, 4.5], [7.0, 4.5], [9.0, 2.5]], false, 0.15, 3.0);
    p3.start_stop = Some(outer.face_stop_for(5, [3.0, 4.5]).unwrap());
    let mut p2 = WallChain::new(vec![[9.0, 4.0], [9.0, 0.0]], false, 0.15, 3.0);
    p2.end_stop = Some(outer.face_stop_for(0, [9.0, 1.0]).unwrap());
    p3.end_stop = Some(p2.face_stop_for(0, [7.0, 4.5]).unwrap());
    let mut p1 = WallChain::new(vec![[4.0, 4.5], [4.0, 0.0]], false, 0.15, 3.0);
    p1.start_stop = Some(p3.face_stop_for(0, [4.0, 1.0]).unwrap());
    p1.end_stop = Some(outer.face_stop_for(0, [4.0, 1.0]).unwrap());
    p1.openings = vec![vec![Opening{offset:1.0,width:0.9,sill:0.0,height:2.1}]];
    plan.extend([p1, p2, p3]);
    for c in &plan {
        let pl = c.plans().unwrap();
        for (i, p) in pl.iter().enumerate() {
            println!("POLY {} {} {} {} {} {} {} {} {} {}", p.l0[0], p.l0[1], p.l1[0], p.l1[1], p.r1[0], p.r1[1], p.r0[0], p.r0[1], c.thickness, c.height);
            let _ = i;
        }
        let m = c.chain_mesh().unwrap();
        println!("CHAIN {} {} {}", total_volume_serial(std::slice::from_ref(c)).unwrap(), m.volume(), m.is_closed_manifold());
    }
    // 2. one feature wall: gable top, arched door, round window
    let mut w = WallChain::new(vec![[0.0, 0.0], [8.0, 0.0]], false, 0.25, 4.0);
    w.tops = vec![Some(vec![[0.0, 3.0], [4.0, 4.5], [8.0, 3.0]])];
    w.poly_openings = vec![vec![arch(1.5, 1.2, 1.9, 32), circle(5.5, 2.0, 0.6, 40)]];
    w.openings = vec![vec![Opening{offset:3.0, width:1.0, sill:1.0, height:1.0}]];
    let m = w.chain_mesh().unwrap();
    println!("FEATURE {} {}", total_volume_serial(std::slice::from_ref(&w)).unwrap(), m.volume());
}
