//! The document as an IFC4 file: levels, walls and beams. The document's other elements are named in the warnings.
use openbim_arch::{Level, Levels};
use openbim_document::{Category, Document, Geometry};
use openbim_geom::WallChain;
use openbim_ifc::{BeamItem, Scene, WallItem};

pub struct Exported {
    pub text: String,
    pub walls: usize,
    pub beams: usize,
    pub warnings: Vec<String>,
}

/// The beam of a document element: its line at the height of its centre on its level, and its section.
pub fn beam_item(doc: &Document, el: &openbim_document::Element) -> Option<BeamItem> {
    let Some(Geometry::Linear { start, end, level }) = el.geometry else { return None };
    let z = doc.get(level)?.elevation_m + el.elevation_m;
    Some(BeamItem { id: el.id.0, name: el.name.clone(), a: [start[0], start[1], z], b: [end[0], end[1], z], profile: el.profile() })
}

/// `timestamp` is written into the header, `project_key` makes the GlobalIds of this project its own.
pub fn export_ifc(doc: &Document, project_name: &str, project_key: u64, timestamp: &str) -> Result<Exported, String> {
    let level_of = |id| doc.get(id).filter(|l| matches!(l.geometry, Some(Geometry::Level))).map(|l| l.elevation_m);
    let levels: Vec<Level> = doc.elements().filter(|e| matches!(e.geometry, Some(Geometry::Level)))
        .map(|e| Level { name: e.name.clone(), elevation: e.elevation_m }).collect();
    let (mut walls, mut beams) = (Vec::new(), Vec::new());
    let mut skipped = std::collections::BTreeMap::<&str, usize>::new();
    for el in doc.elements() {
        match (el.category, el.geometry) {
            (Category::Wall, Some(Geometry::Linear { start, end, level })) => {
                let mut chain = WallChain::new(vec![start, end], false, el.width_m, el.height_m);
                chain.base_z = level_of(level).ok_or("a wall stands on a level that is gone")? + el.elevation_m;
                walls.push(WallItem { id: el.id.0, name: el.name.clone(), chain, wall_type: None, room_bounding: true });
            }
            (Category::Beam, Some(Geometry::Linear { .. })) => {
                beams.push(beam_item(doc, el).ok_or("a beam lies on a level that is gone")?);
            }
            (Category::Level | Category::Grid, _) => {}
            (Category::Duct, _) => *skipped.entry("ducts").or_default() += 1,
            (Category::Pipe, _) => *skipped.entry("pipes").or_default() += 1,
            (Category::Equipment, _) => *skipped.entry("equipment").or_default() += 1,
            _ => *skipped.entry("elements without geometry").or_default() += 1,
        }
    }
    let scene = Scene {
        project_name: project_name.into(), project_key, beams: beams.clone(), timestamp: timestamp.into(),
        levels: Levels::new(levels).map_err(|e| format!("levels: {e:?}"))?,
        walls, wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], columns: vec![], shafts: vec![],
        floor_penetrations: vec![], networks: vec![], families: vec![], items: vec![], rooms: vec![], wall_types: vec![],
        wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![],
        room_lighting: Default::default(), pset_map: Default::default(),
    };
    let out = openbim_ifc::export(&scene).map_err(|e| format!("{e:?}"))?;
    let mut warnings = out.report.warnings;
    warnings.extend(skipped.into_iter().map(|(what, n)| format!("{n} {what} not exported yet")));
    if doc.site().is_some() { warnings.push("the project site is not exported: it needs a coordinate reference system".into()); }
    Ok(Exported { text: out.text, walls: scene.walls.len(), beams: beams.len(), warnings })
}

/// `2026-10-17T19:34:56` for a number of seconds since 1970 (UTC).
pub fn iso_time(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbim_document::{ElementId, Section};

    #[test]
    fn the_document_exports_its_walls_and_beams_and_names_what_it_leaves_out() {
        let mut d = Document::spatial_sample();
        d.place_beam(ElementId(10), [1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.8, Section::Rectangle).unwrap();
        d.place_beam(ElementId(11), [1.0, 4.0], [5.0, 4.0], 0.3, 0.3, 0.2, Section::Circle).unwrap();
        d.place_beam(ElementId(11), [1.0, 5.0], [5.0, 5.0], 0.2, 0.4, 0.2, Section::IShape { web_m: 0.01, flange_m: 0.02 }).unwrap();
        let out = export_ifc(&d, "Sample", 1, "2026-10-17T00:00:00").unwrap();
        assert_eq!((out.walls, out.beams), (6, 3));
        assert_eq!(out.text.matches("IFCWALL(").count(), 6);
        assert_eq!(out.text.matches("IFCBEAM(").count(), 3);
        assert!(out.text.contains("IFCCIRCLEPROFILEDEF") && out.text.contains("IFCISHAPEPROFILEDEF"));
        assert_eq!(out.text.matches("IFCBUILDINGSTOREY(").count(), 2);
        assert!(out.warnings.iter().any(|w| w.contains("1 ducts")) && out.warnings.iter().any(|w| w.contains("1 pipes")), "{:?}", out.warnings);
        // the upper wall stands on Level 2 (3 m): the export is stable and the same twice
        assert_eq!(out.text, export_ifc(&d, "Sample", 1, "2026-10-17T00:00:00").unwrap().text);
    }

    #[test]
    fn the_clock_is_written_as_iso() {
        assert_eq!(iso_time(0), "1970-01-01T00:00:00");
        assert_eq!(iso_time(1_781_998_496), "2026-06-20T23:34:56");
        assert_eq!(iso_time(951_782_400), "2000-02-29T00:00:00");
    }
}
