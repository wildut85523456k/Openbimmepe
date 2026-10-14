use openbim_geom::*;
fn main() {
    let mut c = WallChain::new(vec![[0.0, 0.0], [6.0, 0.0]], false, 0.2, 3.0);
    c.openings = vec![vec![Opening { offset: 2.0, width: 0.9, sill: 0.0, height: 2.1 }, Opening { offset: 4.0, width: 1.2, sill: 0.9, height: 1.2 }]];
    let p = c.plans().unwrap();
    let m = c.segment_mesh(0, &p[0]).unwrap();
    println!("{:.9} {:.9}", c.segment_volume_exact(0, &p[0]).unwrap(), m.volume());
}
