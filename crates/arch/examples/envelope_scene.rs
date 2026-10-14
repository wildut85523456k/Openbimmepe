//! Dumps the office wall type and its envelope as plain text for the independent IFC check (not a product data path).
use openbim_arch::envelope::*;
use openbim_arch::layers::*;
use openbim_geom::{Opening, WallChain};

fn main() {
    let m = |n: &str, l: f64| Material::new(n, l);
    let lib = Library::new(vec![m("Plaster", 0.5), m("Block", 1.0), m("Insulation", 0.04)]).unwrap();
    let s = |mat: &str, t: f64| Layer::Solid { material: mat.into(), thickness: t };
    let a = Assembly {
        name: "External_wall".into(), kind: AssemblyKind::Wall,
        layers: vec![s("Plaster", 0.02), s("Block", 0.2), s("Insulation", 0.05), s("Plaster", 0.015), Layer::Resistance { name: "Cavity".into(), thickness: 0.03, r: 0.18 }],
        r_se: 0.04, r_si: 0.13,
    };
    for mat in ["Plaster", "Block", "Insulation"] { println!("MATERIAL {mat} {}", lib.get(mat).unwrap().conductivity); }
    println!("ASSEMBLY {} {} {} {}", a.name, a.r_se, a.r_si, a.layers.len());
    for l in &a.layers {
        match l { Layer::Solid { material, thickness } => println!("LAYER solid {material} {thickness}"), Layer::Resistance { name, thickness, r } => println!("LAYER resistance {name} {thickness} {r}") }
    }
    println!("U {:.15}", a.u_value(&lib).unwrap());
    println!("THICKNESS {:.15}", a.thickness());
    let mut c = WallChain::new(vec![[0.0, 0.0], [12.0, 0.0], [12.0, 8.0], [0.0, 8.0]], true, a.thickness(), 3.2);
    c.openings = vec![
        (0..3).map(|k| Opening { offset: 1.5 + 3.5 * k as f64, width: 1.8, sill: 0.9, height: 1.5 }).collect(),
        vec![Opening { offset: 3.0, width: 1.0, sill: 0.0, height: 2.1 }],
        vec![], vec![],
    ];
    for e in wall_envelope(&c, Side::Outside, 30.0).unwrap() {
        println!("ENV {} {:.12} {} {:.12} {:.12} {:.12}", e.segment, e.azimuth, SECTOR_NAMES[e.sector], e.gross_area, e.opening_area, e.net_area);
    }
    println!("END");
}
