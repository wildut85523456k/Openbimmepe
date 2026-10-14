//! Dumps levels, floor slabs, a ceiling and risers as plain text for the independent IFC check (not a product data path).
use openbim_arch::spaces::*;
use openbim_arch::*;
use openbim_geom::WallChain;

fn main() {
    let lv = Levels::new(vec![Level { name: "G".into(), elevation: 0.0 }, Level { name: "L1".into(), elevation: 3.6 }, Level { name: "L2".into(), elevation: 7.2 }, Level { name: "Roof".into(), elevation: 10.8 }]).unwrap();
    for l in lv.all() { println!("LEVEL {} {}", l.name, l.elevation); }
    // office 12 x 8 with 0.3 walls; slabs 0.2 thick under every level above the ground (top face at the level elevation)
    let office = WallChain::new(vec![[0.0, 0.0], [12.0, 0.0], [12.0, 8.0], [0.0, 8.0]], true, 0.3, 3.6);
    let room = Room::from_chain("Office", "Office", 12.0, 1, &office, &[], Some(3.6)).unwrap();
    print!("SLAB 0.2 {}", room.outline.len()); for q in &room.outline { print!(" {} {}", q[0], q[1]); } println!();
    let ceil = Ceiling { name: "Suspended".into(), height: 2.7, thickness: 0.05 };
    let void = ceil.void_depth(3.6, 0.2).unwrap();
    println!("CEILING {} {} {} {} {}", ceil.name, ceil.height, ceil.thickness, void, Ceiling::area_of(&room));
    let shafts = vec![
        Shaft { name: "Riser_1".into(), usage: "Plumbing".into(), outline: vec![[10.0, 2.0], [10.8, 2.0], [10.8, 2.8], [10.0, 2.8]], from: 0, to: 3 },
        Shaft { name: "Riser_2".into(), usage: "Electrical".into(), outline: vec![[10.0, 5.0], [10.6, 5.0], [10.6, 5.5], [10.0, 5.5]], from: 0, to: 2 },
    ];
    check_shafts(&shafts, &lv).unwrap();
    for s in &shafts {
        print!("SHAFT {} {} {} {} {}", s.name, s.from, s.to, s.area(), s.outline.len()); for q in &s.outline { print!(" {} {}", q[0], q[1]); } println!();
        println!("CUTS {} {}", s.name, s.floors_cut().iter().map(|i| i.to_string()).collect::<Vec<_>>().join(" "));
    }
    println!("END");
}
