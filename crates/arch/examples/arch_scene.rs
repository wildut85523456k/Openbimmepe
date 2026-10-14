//! Dumps levels, grids and rooms as plain text for the independent IFC check (not a product data path).
use openbim_arch::*;
use openbim_geom::WallChain;

fn rect(w: f64, d: f64, t: f64, h: f64, dx: f64, dy: f64) -> WallChain { WallChain::new(vec![[dx, dy], [dx + w, dy], [dx + w, dy + d], [dx, dy + d]], true, t, h) }

fn main() {
    let lv = Levels::new(vec![Level { name: "Ground".into(), elevation: 0.0 }, Level { name: "First".into(), elevation: 3.5 }]).unwrap();
    for l in lv.all() { println!("LEVEL {} {}", l.name, l.elevation); }
    let grids = vec![
        Grid { name: "1".into(), a: [0.0, -1.0], b: [0.0, 11.0] },
        Grid { name: "A".into(), a: [-1.0, 3.0], b: [16.0, 3.0] },
        Grid { name: "B".into(), a: [-1.0, 8.0], b: [16.0, 8.0] },
    ];
    check_grids(&grids).unwrap();
    for g in &grids { println!("GRID {} {} {} {} {}", g.name, g.a[0], g.a[1], g.b[0], g.b[1]); }
    for (a, b) in [(0, 1), (0, 2)] { let p = grid_crossing(&grids[a], &grids[b]).unwrap(); println!("CROSS {} {} {} {}", grids[a].name, grids[b].name, p[0], p[1]); }
    let office = rect(8.0, 5.0, 0.2, 3.0, 0.0, 0.0);
    let r1 = Room::from_chain("Office_1", "Office", 6.0, lv.index_of("Ground").unwrap(), &office, &[], None).unwrap();
    let l = WallChain::new(vec![[10.0, 0.0], [16.0, 0.0], [16.0, 2.0], [13.0, 2.0], [13.0, 4.0], [10.0, 4.0]], true, 0.4, 3.0);
    let core = rect(1.0, 1.0, 0.2, 3.0, 13.5, 0.6);
    let r2 = Room::from_chain("Lobby", "Lobby", 10.0, lv.index_of("First").unwrap(), &l, &[&core], Some(2.8)).unwrap();
    for r in [&r1, &r2] {
        println!("ROOM {} {} {} {} {} {} {} {} {}", r.name, r.usage, r.occupancy, r.level, lv.all()[r.level].elevation, r.height, r.area(), r.volume(), r.holes.len());
        print!("OUTLINE {} {}", r.name, r.outline.len()); for q in &r.outline { print!(" {} {}", q[0], q[1]); } println!();
        for h in &r.holes { print!("HOLE {} {}", r.name, h.len()); for q in h { print!(" {} {}", q[0], q[1]); } println!(); }
    }
    println!("END");
}
