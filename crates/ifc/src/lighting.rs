//! The luminaire schedule: every placed luminaire with the room it is in, where it points and the lighting data
//! of its family, one row each, in a file a lighting program (or a person) can read. It is the same table DIALux
//! calls the luminaire layout plan, so the two can be compared, and it is the input of the exchange with DIALux.
//! Positions are in metres in the project frame, the turn in degrees counter-clockwise from east, the mounting
//! height above the floor of the level the room is on. A luminaire outside every room gets an empty room cell.
use crate::Scene;
use crate::quantities::ring_area;
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

/// What a lighting calculation needs to know about a room, typed in by the author or taken from the brief: nothing is
/// shipped, no standard values are assumed, and every field is optional until a calculation needs it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoomLighting {
    /// height of the calculation plane above the floor, metres
    pub work_plane_height_m: Option<f64>,
    /// width of the strip along the walls left out of the calculation plane, metres
    pub boundary_zone_m: Option<f64>,
    /// maintenance factor, above 0 and at most 1
    pub maintenance_factor: Option<f64>,
    /// maintained illuminance to reach on the plane, lux
    pub required_lux: Option<f64>,
    /// lowest ratio of minimum to average illuminance, 0 to 1
    pub min_uniformity: Option<f64>,
    /// highest unified glare rating allowed
    pub max_ugr: Option<f64>,
    /// lowest colour rendering index allowed, 0 to 100
    pub min_cri: Option<f64>,
    /// reflectance 0 to 1 of the ceiling, the walls and the floor
    pub reflectance_ceiling: Option<f64>,
    pub reflectance_walls: Option<f64>,
    pub reflectance_floor: Option<f64>,
}

impl RoomLighting {
    pub fn check(&self) -> Result<(), String> {
        let in_range = |v: Option<f64>, lo: f64, hi: f64, left_open: bool| v.is_none_or(|x| x.is_finite() && x <= hi && if left_open { x > lo } else { x >= lo });
        let rules: [(&str, bool); 10] = [
            ("work plane height must be 0 to 10 m", in_range(self.work_plane_height_m, 0.0, 10.0, false)),
            ("boundary zone must be 0 to 10 m", in_range(self.boundary_zone_m, 0.0, 10.0, false)),
            ("maintenance factor must be above 0 and at most 1", in_range(self.maintenance_factor, 0.0, 1.0, true)),
            ("required illuminance must be above 0 and at most 100 000 lux", in_range(self.required_lux, 0.0, 100_000.0, true)),
            ("uniformity must be 0 to 1", in_range(self.min_uniformity, 0.0, 1.0, false)),
            ("UGR limit must be 0 to 50", in_range(self.max_ugr, 0.0, 50.0, false)),
            ("colour rendering index must be 0 to 100", in_range(self.min_cri, 0.0, 100.0, false)),
            ("ceiling reflectance must be 0 to 1", in_range(self.reflectance_ceiling, 0.0, 1.0, false)),
            ("wall reflectance must be 0 to 1", in_range(self.reflectance_walls, 0.0, 1.0, false)),
            ("floor reflectance must be 0 to 1", in_range(self.reflectance_floor, 0.0, 1.0, false)),
        ];
        match rules.iter().find(|(_, ok)| !ok) {
            Some((why, _)) => Err((*why).to_string()),
            None => Ok(()),
        }
    }

    /// The inputs a calculation cannot do without.
    pub fn missing_for_calculation(&self) -> Vec<&'static str> {
        let mut m = Vec::new();
        if self.work_plane_height_m.is_none() { m.push("work plane height"); }
        if self.maintenance_factor.is_none() { m.push("maintenance factor"); }
        if self.required_lux.is_none() { m.push("required illuminance"); }
        if self.reflectance_ceiling.is_none() || self.reflectance_walls.is_none() || self.reflectance_floor.is_none() { m.push("surface reflectances"); }
        m
    }
}

/// Index into `scene.rooms` of the room a point (x, y, z) is in.
pub fn room_at(scene: &Scene, p: [f64; 2], z: f64) -> Option<usize> {
    scene.rooms.iter().position(|r| {
        let rm = &r.room;
        let base = scene.levels.all().get(rm.level).map_or(0.0, |lv| lv.elevation);
        z >= base - 1e-6 && z <= base + rm.height + 1e-6 && inside(p, &rm.outline) && !rm.holes.iter().any(|h| inside(p, h))
    })
}

pub const ROOM_HEADER: &str = "id,room,level,usage,area_m2,height_m,luminaires,flux_lm,power_w,lm_per_m2,w_per_m2,work_plane_m,boundary_zone_m,maintenance_factor,required_lux,min_uniformity,max_ugr,min_cri,reflectance_ceiling,reflectance_walls,reflectance_floor,missing_for_calculation\n";

/// One row per room: its size, what the luminaires inside add up to (installed flux and power and the ratios to the
/// floor area, which are not a simulation), and the inputs typed in for a calculation, with what is still missing.
pub fn room_schedule(scene: &Scene) -> (String, Vec<String>) {
    let mut out = String::from(ROOM_HEADER);
    let mut warnings = Vec::new();
    let mut count = vec![0usize; scene.rooms.len()];
    let mut flux = vec![0.0; scene.rooms.len()];
    let mut power = vec![0.0; scene.rooms.len()];
    for it in &scene.items {
        let Some(l) = scene.families.get(it.family).and_then(|f| f.family.lighting_for(it.type_name.as_deref())) else { continue };
        if let Some(r) = room_at(scene, [it.frame.origin[0], it.frame.origin[1]], it.frame.origin[2]) {
            count[r] += 1;
            flux[r] += l.flux_lm.unwrap_or(0.0) * l.dimming.unwrap_or(1.0);
            power[r] += l.power_w.unwrap_or(0.0) * l.dimming.unwrap_or(1.0);
        }
    }
    for (k, r) in scene.rooms.iter().enumerate() {
        let rm = &r.room;
        let area = ring_area(&rm.outline).abs() - rm.holes.iter().map(|h| ring_area(h).abs()).sum::<f64>();
        let rl = scene.room_lighting.get(&r.id).cloned().unwrap_or_default();
        if let Err(why) = rl.check() {
            warnings.push(format!("room {}: {why}", rm.name));
        }
        let missing = rl.missing_for_calculation();
        if count[k] > 0 && !missing.is_empty() {
            warnings.push(format!("room {}: missing {}", rm.name, missing.join(", ")));
        }
        let num = |v: Option<f64>| v.map_or(String::new(), |x| format!("{x}"));
        let per = |t: f64| if area > 0.0 && count[k] > 0 { format!("{:.2}", t / area) } else { String::new() };
        out.push_str(&format!(
            "{},{},{},{},{:.3},{},{},{:.1},{:.1},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            r.id, csv_safe(&rm.name), csv_safe(&scene.levels.all().get(rm.level).map_or(String::new(), |l| l.name.clone())), csv_safe(&rm.usage), area, rm.height, count[k], flux[k], power[k],
            per(flux[k]), per(power[k]), num(rl.work_plane_height_m), num(rl.boundary_zone_m), num(rl.maintenance_factor), num(rl.required_lux), num(rl.min_uniformity), num(rl.max_ugr), num(rl.min_cri),
            num(rl.reflectance_ceiling), num(rl.reflectance_walls), num(rl.reflectance_floor), missing.join(" | ")
        ));
    }
    (out, warnings)
}

pub const HEADER: &str = "id,name,level,room,room_usage,x_m,y_m,z_m,rotation_deg,mount_height_m,mounting,photometry,manufacturer,article,flux_lm,power_w,cct_k,cri,dimming,ip,picture,datasheet,missing_for_calculation\n";

/// The schedule as comma separated text, plus a warning for each luminaire whose family lacks data a calculation needs.
pub fn luminaire_schedule(scene: &Scene) -> (String, Vec<String>) {
    let mut out = String::from(HEADER);
    let mut warnings = Vec::new();
    for it in &scene.items {
        let Some(fr) = scene.families.get(it.family) else { continue };
        let Some(l) = fr.family.lighting_for(it.type_name.as_deref()) else { continue };
        let p = [it.frame.origin[0], it.frame.origin[1]];
        let z = it.frame.origin[2];
        let room = room_at(scene, p, z).map(|k| &scene.rooms[k]);
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
            "{},{},{},{},{},{:.4},{:.4},{:.4},{:.2},{:.4},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            it.id, csv_safe(&it.name), level, rname, usage, p[0], p[1], z, rot, z - base, l.mounting.name(),
            text(&l.photometry), text(&l.manufacturer), text(&l.article), num(l.flux_lm), num(l.power_w), num(l.cct_k), num(l.cri), num(l.dimming), text(&l.ip), text(&l.picture), text(&l.datasheet),
            missing.join(" | ")
        ));
    }
    (out, warnings)
}

/// A data sheet larger than this is named on the page but not put inside it.
const MAX_EMBEDDED_SHEET: usize = 5 << 20;

fn esc(s: &str) -> String {
    s.chars().map(|c| match c { '&' => "&amp;".into(), '<' => "&lt;".into(), '>' => "&gt;".into(), '"' => "&quot;".into(), '\'' => "&#39;".into(), c => c.to_string() }).collect()
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= c.len() { s.push(T[((n >> (18 - 6 * k)) & 63) as usize] as char) } else { s.push('=') }
        }
    }
    s
}

/// The luminaire types used in the model as a printable page: one card per type with its picture, the data a lighting
/// calculation uses, the data sheet's name and how many are placed. `asset(role, name)` gives the content of a file kept
/// with the project. A picture that is missing or is not what its name says is left out with a warning, never embedded.
/// The page is self-contained (pictures are inside it) and every text is escaped.
pub fn luminaire_catalogue_html(scene: &Scene, asset: &dyn Fn(&str, &str) -> Option<Vec<u8>>) -> (String, Vec<String>) {
    use openbim_family::light::{attachment_kind, check_attachment, AttachmentKind};
    let mut warnings = Vec::new();
    let mut groups: Vec<(usize, Option<String>, usize)> = Vec::new();
    for it in &scene.items {
        let Some(fr) = scene.families.get(it.family) else { continue };
        if fr.family.lighting_for(it.type_name.as_deref()).is_none() { continue }
        match groups.iter_mut().find(|g| g.0 == it.family && g.1 == it.type_name) {
            Some(g) => g.2 += 1,
            None => groups.push((it.family, it.type_name.clone(), 1)),
        }
    }
    let mut h = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Luminaires \u{2013} {}</title><style>body{{font:14px sans-serif;margin:16px}}.card{{display:flex;gap:16px;border:1px solid #888;padding:12px;margin:0 0 12px;break-inside:avoid}}.pic{{width:180px;height:180px;object-fit:contain;border:1px solid #ccc}}.nopic{{width:180px;height:180px;border:1px dashed #aaa;display:flex;align-items:center;justify-content:center;color:#666}}table{{border-collapse:collapse}}td{{padding:2px 12px 2px 0}}td:first-child{{color:#555}}@media print{{body{{margin:0}}}}</style></head><body><h1>Luminaires \u{2013} {}</h1>\n",
        esc(&scene.project_name), esc(&scene.project_name)
    );
    for (fi, tn, count) in &groups {
        let fr = &scene.families[*fi];
        let Some(l) = fr.family.lighting_for(tn.as_deref()) else { continue };
        let title = match tn { Some(t) => format!("{} \u{2013} {}", fr.family.name(), t), None => fr.family.name().to_string() };
        let mut pic = String::from("<div class=\"nopic\">no picture</div>");
        if let Some(name) = &l.picture {
            match asset("picture", name) {
                None => warnings.push(format!("{title}: the picture {name} is not kept in the project")),
                Some(d) => match check_attachment("picture", name, &d) {
                    Err(why) => warnings.push(format!("{title}: the picture {name} is not used: {why}")),
                    Ok(()) => {
                        let mime = if attachment_kind(&d) == Some(AttachmentKind::Png) { "image/png" } else { "image/jpeg" };
                        pic = format!("<img class=\"pic\" alt=\"{}\" src=\"data:{mime};base64,{}\">", esc(&title), base64(&d));
                    }
                },
            }
        }
        let mut sheet = String::new();
        if let Some(name) = &l.datasheet {
            match asset("datasheet", name) {
                None => warnings.push(format!("{title}: the data sheet {name} is not kept in the project")),
                Some(d) => match check_attachment("datasheet", name, &d) {
                    Err(why) => warnings.push(format!("{title}: the data sheet {name} is not used: {why}")),
                    Ok(()) if d.len() > MAX_EMBEDDED_SHEET => {
                        warnings.push(format!("{title}: the data sheet {name} is over {} MB and is named but not embedded", MAX_EMBEDDED_SHEET >> 20));
                        sheet = esc(name);
                    }
                    Ok(()) => sheet = format!("<a download=\"{0}\" href=\"data:application/pdf;base64,{1}\">{0}</a>", esc(name), base64(&d)),
                },
            }
        }
        let num = |v: Option<f64>, unit: &str| v.map(|x| format!("{x} {unit}")).unwrap_or_default();
        let txt = |v: &Option<String>| v.as_deref().map(esc).unwrap_or_default();
        let rows = [
            ("Manufacturer", txt(&l.manufacturer)), ("Article", txt(&l.article)), ("Luminous flux", num(l.flux_lm, "lm")), ("Power", num(l.power_w, "W")),
            ("Colour temperature", num(l.cct_k, "K")), ("Colour rendering index", num(l.cri, "")), ("Protection", txt(&l.ip)), ("Mounting", l.mounting.name().to_string()),
            ("Photometry", txt(&l.photometry)), ("Data sheet", sheet), ("Placed", count.to_string()),
        ];
        h.push_str(&format!("<div class=\"card\">{pic}<div><h2>{}</h2><table>", esc(&title)));
        for (k, v) in rows.iter().filter(|r| !r.1.trim().is_empty()) {
            h.push_str(&format!("<tr><td>{k}</td><td>{v}</td></tr>"));
        }
        h.push_str("</table></div></div>\n");
    }
    h.push_str("</body></html>\n");
    (h, warnings)
}
