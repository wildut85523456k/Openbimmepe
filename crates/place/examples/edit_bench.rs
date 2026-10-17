// What one edit costs: a full rebuild (`try_update`) against `update_built`.
// Usage: edit_bench <segments in total> <chains>   e.g. 100000 1  or  100000 64
use openbim_family::library;
use openbim_geom::WallChain;
use openbim_place::*;
use std::time::Instant;

fn zigzag(n: usize, x0: f64) -> Vec<[f64; 2]> {
    let mut p = vec![[x0, 0.0]];
    for i in 0..n {
        let last = p[i];
        p.push(match i % 4 { 0 => [last[0] + 4.0, last[1]], 1 => [last[0], last[1] + 4.0], 2 => [last[0] + 4.0, last[1]], _ => [last[0], last[1] - 4.0] });
    }
    p
}

fn main() {
    let total: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(100_000);
    let chains_n: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut fams = library::all().unwrap();
    fams.remove(0);
    let per = total / chains_n;
    let mut chains = Vec::new();
    let mut ps = Vec::new();
    for c in 0..chains_n {
        chains.push(WallChain::new(zigzag(per, c as f64 * 1.0e4), false, 0.2, 3.0));
        for s in 0..per {
            ps.push(Placement { family: s % 2, type_name: None, overrides: vec![], texts: vec![], chain: c, segment: s, centre: 2.0 });
        }
    }
    let mut full = Model { chains: chains.clone(), families: fams.clone(), placements: ps.clone(), ..Default::default() };
    let mut inc = Model { chains, families: fams, placements: ps, ..Default::default() };
    let t = Instant::now();
    let mut built = inc.build().unwrap();
    println!("{total} segments in {chains_n} chains, each with a door or a window; full build {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let edits: Vec<(usize, f64)> = (0..20).map(|k| ((k * 7919) % total, 1.5 + (k % 5) as f64 * 0.2)).collect();
    let mk = |i: usize, c: f64, m: &Model| Placement { centre: c, ..m.placements[i].clone() };
    let t = Instant::now();
    for &(i, c) in &edits {
        let p = mk(i, c, &full);
        full.try_update(i, p).unwrap();
    }
    let t_full = t.elapsed().as_secs_f64() / edits.len() as f64;
    let t = Instant::now();
    for &(i, c) in &edits {
        let p = mk(i, c, &inc);
        inc.update_built(&mut built, i, p).unwrap();
    }
    let t_inc = t.elapsed().as_secs_f64() / edits.len() as f64;
    let check = full.build().unwrap();
    assert_eq!(check.volume.to_bits(), built.volume.to_bits());
    println!("one edit: full rebuild {:.1} ms, update_built {:.2} ms ({:.0}x); same volume to the last bit", t_full * 1e3, t_inc * 1e3, t_full / t_inc);
}
