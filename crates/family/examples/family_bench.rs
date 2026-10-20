// Solve many window placements and time 1, 2 and 4 threads. Usage: bench <count>
use openbim_family::{library::window, Request};
use std::time::Instant;
fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);
    let c = window().compile().unwrap();
    let reqs: Vec<Request> = (0..n)
        .map(|k| Request {
            type_name: if k % 3 == 0 { Some("Type A") } else { None },
            overrides: vec![("mullions", (k % 6) as f64), ("sill", (k % 7) as f64 * 0.2)],
            texts: vec![],
        })
        .collect();
    let mut base = 0.0;
    for t in [1usize, 2, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(t)
            .build()
            .unwrap();
        let t0 = Instant::now();
        let out = pool.install(|| c.solve_many(&reqs));
        let dt = t0.elapsed().as_secs_f64();
        if t == 1 {
            base = dt;
        }
        let ok = out.iter().filter(|r| r.is_ok()).count();
        println!(
            "threads {t}: {dt:.3} s  speed-up {:.2}x  ok {ok} refused {}  ({:.0} placements/s)",
            base / dt,
            n - ok,
            n as f64 / dt
        );
    }
}
