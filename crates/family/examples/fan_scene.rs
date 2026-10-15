//! Prints category/system tables and the fan coil's connectors (local and placed) for the independent IFC check.
use openbim_family::library::fan_coil;
use openbim_family::mep::*;
use openbim_family::*;

fn size(s: &Size) -> String {
    match s {
        Size::Round { diameter } => format!("round {diameter:.9}"),
        Size::Rect { width, height } => format!("rect {width:.9} {height:.9}"),
        Size::Electrical { voltage, poles } => format!("elec {voltage:.9} {poles}"),
        Size::Signal => "signal".to_string(),
    }
}
fn line(tag: &str, c: &Connector) {
    println!(
        "{tag} {} {:?} {:?} {:?} {:.9} {:.9} {:.9} {:.9} {:.9} {:.9} {} {:.9} {:.9}",
        c.name,
        c.domain,
        c.system,
        c.flow_dir,
        c.pos[0],
        c.pos[1],
        c.pos[2],
        c.dir[0],
        c.dir[1],
        c.dir[2],
        size(&c.size),
        c.flow,
        c.pressure_drop
    );
}

fn main() {
    for c in Category::ALL {
        println!("CLASS {:?} {}", c, c.ifc_class());
    }
    use System::*;
    for s in [
        SupplyAir,
        ReturnAir,
        ExhaustAir,
        OutsideAir,
        ChilledWaterSupply,
        ChilledWaterReturn,
        HeatingWaterSupply,
        HeatingWaterReturn,
        CondenserWaterSupply,
        CondenserWaterReturn,
        DomesticCold,
        DomesticHot,
        Sanitary,
        Vent,
        StormWater,
        Condensate,
        FireProtection,
        Gas,
        Power,
        Data,
        Controls,
        Other,
    ] {
        println!("SYS {:?} {}", s, s.ifc_system());
    }
    for f in [FlowDir::In, FlowDir::Out, FlowDir::Both] {
        println!("FLOW {:?} {}", f, f.ifc());
    }
    for d in [
        Domain::Duct,
        Domain::Pipe,
        Domain::CableTray,
        Domain::Conduit,
        Domain::Electrical,
    ] {
        println!("PORT {:?} {}", d, d.ifc_port_type());
    }
    let c = fan_coil().compile().unwrap();
    let inst = c.solve(&Request::default()).unwrap();
    let poses = [
        Pose {
            x: 10.0,
            y: 5.0,
            z: 3.0,
            angle: 90.0,
        },
        Pose {
            x: -3.2,
            y: 7.7,
            z: 2.6,
            angle: 37.0,
        },
    ];
    for (k, p) in poses.iter().enumerate() {
        println!("POSE {k} {} {} {} {}", p.x, p.y, p.z, p.angle);
        for cn in &inst.connectors {
            line(&format!("LOCAL {k}"), cn);
            line(&format!("WORLD {k}"), &cn.world(p));
        }
    }
    // pairs: each world connector of pose 0 against the facing copy of pose 0 and against pose 1 connectors
    let w0: Vec<Connector> = inst.connectors.iter().map(|c| c.world(&poses[0])).collect();
    let w1: Vec<Connector> = inst.connectors.iter().map(|c| c.world(&poses[1])).collect();
    for a in &w0 {
        for b in &w1 {
            let r = mate(a, b, 0.001, Resize::Strict);
            println!("MATE {} {} {:?}", a.name, b.name, r);
        }
        let mut f = a.clone();
        f.dir = [-a.dir[0], -a.dir[1], -a.dir[2]];
        f.flow_dir = match a.flow_dir {
            FlowDir::In => FlowDir::Out,
            FlowDir::Out => FlowDir::In,
            FlowDir::Both => FlowDir::Both,
        };
        println!("SELF {} {:?}", a.name, mate(a, &f, 0.001, Resize::Strict));
    }
}
