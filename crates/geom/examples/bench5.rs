use openbim_geom::*;
use std::time::Instant;
fn room(i: usize) -> WallChain {
    let w = 3.0 + (i % 7) as f64; let d = 2.0 + (i % 5) as f64;
    let mut c = WallChain::new(vec![[0.,0.],[w,0.],[w,d],[0.,d]], true, 0.2, 2.8);
    c.openings = vec![vec![Opening{offset:0.4,width:0.9,sill:0.0,height:2.1}], vec![], vec![Opening{offset:0.5,width:1.2,sill:0.9,height:1.2}], vec![]];
    c
}
/// (user ticks, system ticks, minor page faults) of this process, from /proc/self/stat
fn cpu() -> (u64, u64, u64) {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap();
    let f: Vec<&str> = s[s.rfind(')').unwrap() + 2..].split_whitespace().collect();
    (f[11].parse().unwrap(), f[12].parse().unwrap(), f[7].parse().unwrap())
}
fn main() {
    let n = 1_000_000;
    let chains: Vec<WallChain> = (0..n).map(room).collect();
    let (l, r) = chains.split_at(n / 2);
    let c0 = cpu(); let t = Instant::now();
    let _ = total_volume_serial(&chains).unwrap();
    let c1 = cpu(); let one = t.elapsed().as_secs_f64();
    let t = Instant::now();
    std::thread::scope(|s| { let a = s.spawn(|| total_volume_serial(l).unwrap()); let b = s.spawn(|| total_volume_serial(r).unwrap()); a.join().unwrap(); b.join().unwrap(); });
    let c2 = cpu(); let two = t.elapsed().as_secs_f64();
    println!("1 thread : {one:.2} s wall, user {:>4} ticks, system {:>4} ticks, page faults {}", c1.0 - c0.0, c1.1 - c0.1, c1.2 - c0.2);
    println!("2 threads: {two:.2} s wall, user {:>4} ticks, system {:>4} ticks, page faults {}", c2.0 - c1.0, c2.1 - c1.1, c2.2 - c1.2);
}
