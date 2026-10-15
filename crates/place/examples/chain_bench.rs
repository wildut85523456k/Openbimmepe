// How the build scales when the same wall length is cut into 1, 8, 64 or 4096 chains.
// Usage: chain_bench <segments in total> [max threads]. Prints times for 1..=max threads (powers of two).
use openbim_family::library::{self};
use openbim_geom::WallChain;
use openbim_place::*;
use std::time::Instant;

fn zigzag(n: usize, x0: f64) -> Vec<[f64; 2]> {
    // n segments of 4 m turning 90 degrees left and right in turn, so the chain never closes or crosses itself
    let mut p = vec![[x0, 0.0]];
    for i in 0..n {
        let last = p[i];
        p.push(match i % 4 { 0 => [last[0] + 4.0, last[1]], 1 => [last[0], last[1] + 4.0], 2 => [last[0] + 4.0, last[1]], _ => [last[0], last[1] - 4.0] });
    }
    p
}

fn main() {
    let total: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let max_t: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(4);
    let mut fams = library::all().unwrap();
    fams.remove(0);
    println!("{total} wall segments in all, each with a door or a window; {} hardware threads", std::thread::available_parallelism().map_or(1, |n| n.get()));
    println!("{:>8} {:>10} {:>8} {:>10} {:>9}", "chains", "segs/chain", "threads", "build s", "speed-up");
    for chains_n in [1usize, 8, 64, 4096] {
        let per = total / chains_n;
        let mut chains = Vec::new();
        let mut ps = Vec::new();
        for c in 0..chains_n {
            chains.push(WallChain::new(zigzag(per, c as f64 * 1.0e4), false, 0.2, 3.0));
            for s in 0..per {
                ps.push(Placement { family: s % 2, type_name: None, overrides: vec![], texts: vec![], chain: c, segment: s, centre: 2.0 });
            }
        }
        let m = Model { chains, families: fams.clone(), placements: ps, ..Default::default() };
        let mut base = 0.0;
        let mut vol = None;
        let mut t = 1;
        while t <= max_t {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(t).build().unwrap();
            let t0 = Instant::now();
            let b = pool.install(|| m.build()).unwrap();
            let dt = t0.elapsed().as_secs_f64();
            if t == 1 {
                base = dt;
            }
            // the answer must not depend on the thread count
            match vol {
                None => vol = Some(b.volume),
                Some(v) => assert_eq!(v.to_bits(), b.volume.to_bits(), "volume changed with the thread count"),
            }
            println!("{chains_n:>8} {per:>10} {t:>8} {dt:>10.3} {:>8.2}x", base / dt);
            t *= 2;
        }
    }
}
