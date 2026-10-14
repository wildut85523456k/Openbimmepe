use openbim_geom::*;
use std::time::Instant;
fn room(i: usize) -> WallChain {
    let w = 3.0 + (i % 7) as f64; let d = 2.0 + (i % 5) as f64;
    let mut c = WallChain::new(vec![[0.,0.],[w,0.],[w,d],[0.,d]], true, 0.2, 2.8);
    c.openings = vec![vec![Opening{offset:0.4,width:0.9,sill:0.0,height:2.1}], vec![], vec![Opening{offset:0.5,width:1.2,sill:0.9,height:1.2}], vec![]];
    c
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let mesh = args.get(2).map(|s| s == "mesh").unwrap_or(true);
    let chains: Vec<WallChain> = (0..n).map(room).collect();
    println!("{} rooms = {} wall segments, mode {}", n, n * 4, if mesh { "mesh" } else { "exact" });
    let mut base = 0.0;
    for threads in [1usize, 2, 4] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        let t = Instant::now();
        let v = pool.install(|| if mesh { total_mesh_volume_parallel(&chains).unwrap() } else { total_volume_parallel(&chains).unwrap() });
        let s = t.elapsed().as_secs_f64();
        if threads == 1 { base = s; }
        println!("threads {threads:>2}: {s:.3} s  speed-up {:.2}x  volume {v:.3}", base / s);
    }
}
