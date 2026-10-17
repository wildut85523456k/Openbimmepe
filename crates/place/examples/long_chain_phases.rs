// Times the pieces of the work on one very long chain: usage: long_chain_phases <segments>
use openbim_geom::WallChain;
use rayon::prelude::*;
use std::time::Instant;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(100_000);
    let mut p = vec![[0.0, 0.0]];
    for i in 0..n {
        let l = p[i];
        p.push(match i % 4 { 0 => [l[0] + 4.0, l[1]], 1 => [l[0], l[1] + 4.0], 2 => [l[0] + 4.0, l[1]], _ => [l[0], l[1] - 4.0] });
    }
    let c = WallChain::new(p, false, 0.2, 3.0);
    let t = |name: &str, f: &dyn Fn()| {
        let t0 = Instant::now();
        f();
        println!("{name:<28} {:>8.1} ms", t0.elapsed().as_secs_f64() * 1e3);
    };
    t("validate", &|| c.validate().unwrap());
    let plans = c.plans().unwrap();
    t("plans (includes validate)", &|| {
        c.plans().unwrap();
    });
    t("volume, one thread", &|| {
        let mut s = 0.0;
        for (i, p) in plans.iter().enumerate() {
            s += c.segment_volume_exact(i, p).unwrap();
        }
        std::hint::black_box(s);
    });
    t("volume, all threads", &|| {
        let v: Vec<f64> = plans.par_iter().enumerate().map(|(i, p)| c.segment_volume_exact(i, p).unwrap()).collect();
        std::hint::black_box(v);
    });
    t("clone of the chain", &|| {
        std::hint::black_box(c.clone());
    });
}
