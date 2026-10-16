//! The luminaire schedule: every placed luminaire with the room it is in, where it points and the lighting data
//! of its family, one row each, in a file a lighting program (or a person) can read. It is the same table DIALux
//! calls the luminaire layout plan, so the two can be compared, and it is the input of the exchange with DIALux.
//! Positions are in metres in the project frame, the turn in degrees counter-clockwise from east, the mounting
//! height above the floor of the level the room is on. A luminaire outside every room gets an empty room cell.
use crate::Scene;
use openbim_place::penetrations::csv_safe;

fn inside(p: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut c = false;
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
    }
    c
}

pub const HEADER: &str = "id,name,level,room,room_usage,x_m,y_m,z_m,rotation_deg,mount_height_m,mounting,photometry,manufacturer,article,flux_lm,power_w,cct_k,cri,dimming,ip,missing_for_calculation\n";

/// The schedule as comma separated text, plus a warning for each luminaire whose family lacks data a calculation needs.
pub fn luminaire_schedule(scene: &Scene) -> (String, Vec<String>) {
    let mut out = String::from(HEADER);
    let mut warnings = Vec::new();
    for it in &scene.items {
        let Some(fr) = scene.families.get(it.family) else { continue };
        let Some(l) = fr.family.lighting() else { continue };
        let p = [it.frame.origin[0], it.frame.origin[1]];
        let z = it.frame.origin[2];
        let room = scene.rooms.iter().find(|r| {
            let rm = &r.room;
            let base = scene.levels.all().get(rm.level).map_or(0.0, |lv| lv.elevation);
            z >= base - 1e-6 && z <= base + rm.height + 1e-6 && inside(p, &rm.outline) && !rm.holes.iter().any(|h| inside(p, h))
        });
        let (level, rname, usage, base) = match room {
            Some(r) => {
                let lv = scene.levels.all().get(r.room.level);
                (lv.map_or(String::new(), |l| csv_safe(&l.name)), csv_safe(&r.room.name), csv_safe(&r.room.usage), lv.map_or(0.0, |l| l.elevation))
            }
            None => (String::new(), String::new(), String::new(), 0.0),
        };
        let missing = l.missing_for_calculation();
        if !missing.is_empty() {
            warnings.push(format!("luminaire {}: missing {}", it.name, missing.join(", ")));
        }
        let num = |v: Option<f64>| v.map_or(String::new(), |x| format!("{x}"));
        let text = |v: &Option<String>| v.as_deref().map(csv_safe).unwrap_or_default();
        let rot = it.frame.x[1].atan2(it.frame.x[0]).to_degrees();
        out.push_str(&format!(
            "{},{},{},{},{},{:.4},{:.4},{:.4},{:.2},{:.4},{},{},{},{},{},{},{},{},{},{},{}\n",
            it.id, csv_safe(&it.name), level, rname, usage, p[0], p[1], z, rot, z - base, l.mounting.name(),
            text(&l.photometry), text(&l.manufacturer), text(&l.article), num(l.flux_lm), num(l.power_w), num(l.cct_k), num(l.cri), num(l.dimming), text(&l.ip),
            missing.join(" | ")
        ));
    }
    (out, warnings)
}
