use openbim_geom::*;
use std::time::Instant;
fn circle(n: usize) -> Vec<[f64; 2]> {
    (0..n).map(|k| { let a = std::f64::consts::TAU * k as f64 / n as f64; [3.0 + 1.0 * a.cos(), 1.5 + 1.0 * a.sin()] }).collect()
}
fn main() {
    for n in [100usize, 1_000, 4_000, 4_096, 4_097, 20_000] {
        let mut c = WallChain::new(vec![[0., 0.], [6., 0.]], false, 0.2, 3.0);
        c.poly_openings = vec![vec![circle(n)]];
        let t = Instant::now();
        let r = c.chain_mesh();
        println!("opening with {n:>6} points: {:>8.3} s  {}", t.elapsed().as_secs_f64(), match r { Ok(m) => format!("ok, {} triangles", m.tris.len()), Err(e) => format!("{e:?}") });
    }
    // many openings, each small
    let mut c = WallChain::new(vec![[0., 0.], [1000., 0.]], false, 0.2, 3.0);
    c.openings = vec![(0..1000).map(|k| Opening { offset: 0.5 + k as f64 * 0.99, width: 0.5, sill: 0.5, height: 1.0 }).collect()];
    let t = Instant::now(); let r = c.chain_mesh();
    println!("1000 openings:               {:>8.3} s  {}", t.elapsed().as_secs_f64(), r.map(|m| format!("ok, {} triangles", m.tris.len())).unwrap_or_else(|e| format!("{e:?}")));
    // many segments in one chain
    let pts: Vec<[f64; 2]> = (0..200_000).map(|k| [k as f64 * 0.5, if k % 2 == 0 { 0.0 } else { 0.3 }]).collect();
    let c = WallChain::new(pts, false, 0.1, 3.0);
    let t = Instant::now(); let r = c.chain_mesh();
    println!("200000-segment zigzag chain: {:>8.3} s  {}", t.elapsed().as_secs_f64(), r.map(|m| format!("ok, {} triangles", m.tris.len())).unwrap_or_else(|e| format!("{e:?}")));
}
