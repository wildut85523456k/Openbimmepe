// Build a model of many rooms with doors and windows; time 1, 2 and 4 threads. Usage: bench <rooms>
use openbim_family::library::{self};
use openbim_geom::WallChain;
use openbim_place::*;
use std::time::Instant;
fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(50_000);
    let mut chains = Vec::with_capacity(n); let mut ps = Vec::new();
    for k in 0..n {
        chains.push(WallChain::new(vec![[0.0, 0.0], [5.0 + (k % 7) as f64 * 0.3, 0.0], [5.0 + (k % 7) as f64 * 0.3, 4.0], [0.0, 4.0]], true, 0.2, 3.0));
        ps.push(Placement { family: 0, type_name: None, overrides: vec![], texts: vec![], chain: k, segment: 0, centre: 1.2 + (k % 5) as f64 * 0.1 });
        ps.push(Placement { family: 1, type_name: None, overrides: vec![("mullions".into(), (k % 4) as f64)], texts: vec![], chain: k, segment: 2, centre: 2.0 });
        ps.push(Placement { family: 1, type_name: Some("Type A".into()), overrides: vec![], texts: vec![], chain: k, segment: 1, centre: 2.0 });
    }
    let m = Model { chains, families: library_families(), placements: ps, ..Default::default() };
    let mut base = 0.0;
    for t in [1usize, 2, 4] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(t).build().unwrap();
        let t0 = Instant::now();
        let b = pool.install(|| m.build()).unwrap();
        let dt = t0.elapsed().as_secs_f64();
        if t == 1 { base = dt; }
        println!("threads {t}: {dt:.3} s  speed-up {:.2}x  {} walls, {} placements, volume {:.3}", base / dt, 4 * n, m.placements.len(), b.volume);
    }
}

/// door and window, compiled together with the families they contain (the handle)
fn library_families() -> Vec<openbim_family::Compiled> {
    let mut v = library::all().unwrap();
    v.remove(0);
    v
}
