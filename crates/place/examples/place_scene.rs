//! Dumps a placed-family scene as plain text for the independent IFC check (not a product data path).
use openbim_family::library::{self};
use openbim_geom::*;
use openbim_place::*;

fn pl(family: usize, chain: usize, segment: usize, centre: f64, ty: Option<&str>, ov: &[(&str, f64)]) -> Placement {
    Placement { family, type_name: ty.map(|s| s.to_string()), overrides: ov.iter().map(|(n, v)| (n.to_string(), *v)).collect(), texts: vec![], chain, segment, centre }
}

fn main() {
    let mut chains = Vec::new();
    // chain 0: a closed mitred room, wall thickness 0.2
    chains.push(WallChain::new(vec![[0.0, 0.0], [8.0, 0.0], [8.0, 5.0], [0.0, 5.0]], true, 0.2, 3.0));
    // chain 1: a diagonal free wall, thicker, on an upper level
    let mut up = WallChain::new(vec![[10.0, 1.0], [16.0, 9.0]], false, 0.3, 3.2);
    up.base_z = 3.0;
    chains.push(up);
    let placements = vec![
        Placement { texts: vec![("swing".into(), "left".into()), ("mark".into(), "D-01".into())], ..pl(0, 0, 0, 2.0, None, &[]) },
        Placement { texts: vec![("mark".into(), "W-01".into())], ..pl(1, 0, 1, 2.5, None, &[("sill", 0.9), ("mullions", 2.0)]) },
        pl(1, 0, 2, 2.0, Some("Type A"), &[("sill", 0.8), ("mullions", 3.0)]),
        pl(1, 0, 2, 5.5, None, &[("sill", 1.0), ("mullions", 0.0)]),
        Placement { texts: vec![("mark".into(), "D-02".into())], ..pl(0, 0, 3, 2.5, Some("Type B"), &[("has_handle", 0.0), ("open_angle", 60.0)]) },
        pl(0, 1, 0, 3.0, Some("Type A"), &[]),
        pl(1, 1, 0, 7.0, None, &[("sill", 0.7), ("mullions", 1.0)]),
        Placement { texts: vec![("mark".into(), "D-03".into())], ..pl(2, 0, 0, 5.5, None, &[]) },
        Placement { texts: vec![("mark".into(), "D-04".into())], ..pl(2, 1, 0, 4.5, Some("Type A"), &[]) },
    ];
    let m = Model { chains, families: library_families(), placements, ..Default::default() };
    let b = m.build().expect("scene builds");
    for (ci, c) in b.chains.iter().enumerate() {
        let plans = c.plans().unwrap();
        for (si, p) in plans.iter().enumerate() {
            let v = c.segment_volume_exact(si, p).unwrap();
            println!("SEG {ci} {si} {} {} {} {} {} {} {} {} {} {} {v:.12}", p.l0[0], p.l0[1], p.l1[0], p.l1[1], p.r1[0], p.r1[1], p.r0[0], p.r0[1], c.height, c.base_z);
        }
        for si in 0..c.segment_count() {
            let (q0, q1) = (c.points[si], c.points[(si + 1) % c.points.len()]);
            println!("LINE {ci} {si} {} {} {} {} {}", q0[0], q0[1], q1[0], q1[1], c.thickness);
            for poly in c.poly_openings.get(si).into_iter().flatten() {
                print!("CUTP {ci} {si} {}", poly.len());
                for q in poly { print!(" {} {}", q[0], q[1]); }
                println!();
            }
            for o in c.openings.get(si).into_iter().flatten() { println!("CUT {ci} {si} {} {} {} {}", o.offset, o.width, o.sill, o.height); }
        }
        println!("CHAIN {ci} {:.12}", openbim_geom::total_volume_serial(std::slice::from_ref(c)).unwrap());
    }
    println!("TOTAL {:.12}", b.volume);
    for (k, p) in m.placements.iter().enumerate() {
        let fam = &m.families[p.family];
        let inst = &b.instances[k];
        let tname = p.type_name.clone().unwrap_or_else(|| "(default)".into()).replace(' ', "_");
        println!("PLACED {k} {} {} {} {} {}", p.chain, p.segment, fam.name().replace(' ', "_"), tname, p.centre);
        for (n, v) in fam.input_values(inst) { println!("VAL {k} {n} {v}"); }
        for (n, v) in fam.derived_values(inst) { println!("DERIVED {k} {n} {v}"); }
        for (n, v) in fam.input_texts(inst) { println!("TEXT {k} {n} {}", if v.is_empty() { "-".to_string() } else { v.replace(' ', "_") }); }
        let c = &b.chains[p.chain];
        let (a0, a1) = (c.points[p.segment], c.points[(p.segment + 1) % c.points.len()]);
        let d = [a1[0] - a0[0], a1[1] - a0[1]]; let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let (u, nl) = ([d[0] / l, d[1] / l], [-d[1] / l, d[0] / l]);
        if let Some([w, h, sill]) = inst.opening {
            // cut box: through the full wall thickness plus a little
            let t = c.thickness + 0.1;
            let o = p.centre - w / 2.0;
            let org = [a0[0] + u[0] * o - nl[0] * t / 2.0, a0[1] + u[1] * o - nl[1] * t / 2.0, c.base_z + sill];
            let area = match inst.opening_outline(o) {
                Some(poly) if inst.opening_rise != 0.0 => {
                    // arched head: the cut is the outline (s, z above the wall base) pushed through the wall
                    let n = poly.len();
                    let a: f64 = (0..n).map(|i| poly[i][0] * poly[(i + 1) % n][1] - poly[(i + 1) % n][0] * poly[i][1]).sum::<f64>() / 2.0;
                    let ob = [a0[0] + nl[0] * t / 2.0, a0[1] + nl[1] * t / 2.0, c.base_z];
                    print!("OPENPOLY {k} {} {} {} {} {} {} {} {} {}", ob[0], ob[1], ob[2], u[0], u[1], nl[0], nl[1], t, n);
                    for q in &poly { print!(" {} {}", q[0], q[1]); }
                    println!();
                    a
                }
                _ => w * h,
            };
            println!("OPENING {k} {} {} {} {} {} {} {} {} {} {} {}", org[0], org[1], org[2], u[0], u[1], nl[0], nl[1], w, t, h, area * c.thickness);
        }
        let mut vol = 0.0;
        for (bi, bx) in m.world_boxes(&b, k).into_iter().enumerate() {
            println!("MAT {k} {bi} {}", inst.material_of(bi).map(|s| s.replace(' ', "_")).unwrap_or_else(|| "-".into()));
            println!("BOX {k} {} {} {} {} {} {} {} {} {} {} {}", fam.solid_name(bx.solid), bx.origin[0], bx.origin[1], bx.origin[2], bx.ax[0], bx.ax[1], bx.ay[0], bx.ay[1], bx.size[0], bx.size[1], bx.size[2]);
            vol += bx.size[0] * bx.size[1] * bx.size[2];
        }
        println!("FAMVOL {k} {vol:.12}");
    }
}

/// door, window and arched door, compiled together with the families they contain (the handle)
fn library_families() -> Vec<openbim_family::Compiled> {
    let mut v = library::all().unwrap();
    v.remove(0);
    v
}
