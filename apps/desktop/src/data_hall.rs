//! MEP-004: the data hall the MEP chain test builds (decided 14 Oct 2026: the data hall for the MEP chain, the office
//! for architecture). One storey, 30 x 18 m, an open white space under a ceiling, two air handling units on the north
//! wall and a grid of ceiling diffusers. The test generates it; nothing here is hand-modelled.
use openbim_document::{Document, ElementId, FamilySpec, Material, SlabKind, SlabSpec};

/// As Revit has it for a supply system (Create Systems > Duct): each air handler is the base of its own system and the
/// terminals it feeds are the other members; return air would be a system of its own. Here: the west half of the
/// hall (columns 0..3) is fed by the first air handler, the east half by the second.
pub(crate) struct DataHall {
    pub doc: Document,
    pub level: ElementId,
    pub ahus: Vec<ElementId>,
    pub diffusers: Vec<ElementId>,
    /// (system name, base air handler, terminals)
    pub systems: Vec<(String, ElementId, Vec<ElementId>)>,
}

pub(crate) const W: f64 = 30.0;
pub(crate) const D: f64 = 18.0;
pub(crate) const CEILING_M: f64 = 3.2;

pub(crate) fn data_hall() -> DataHall {
    let mut d = Document::new();
    let level = d.place_level("Level 1", 0.0).unwrap();
    let corners = [([0.0, 0.0], [W, 0.0]), ([W, 0.0], [W, D]), ([W, D], [0.0, D]), ([0.0, D], [0.0, 0.0])];
    d.place_walls(level, &corners, 0.3, 4.2, 0.0, None).unwrap();
    let rect = vec![[0.0, 0.0], [W, 0.0], [W, D], [0.0, D]];
    d.place_slab(level, &SlabSpec { outline: rect.clone(), thickness_m: 0.3, top_offset_m: 0.0, kind: SlabKind::Floor, material: Material::Concrete }).unwrap();
    d.place_ceiling(level, rect, CEILING_M, 0.05, Material::Unspecified).unwrap();
    let room = d.place_room(level, [W / 2.0, D / 2.0]).unwrap();
    d.set_room_data(room, "D01", "Data hall", "IT").unwrap();
    let spec = |family: &str, elevation_m: f64| FamilySpec { family: family.into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m };
    let ahus = [10.2, 24.6].iter().map(|x| d.place_family(level, [*x, 1.5], &spec("Air handling unit", 0.0)).unwrap()).collect();
    let ahus: Vec<ElementId> = ahus;
    let mut diffusers = Vec::new();
    let mut halves: Vec<Vec<ElementId>> = vec![Vec::new(), Vec::new()];
    for row in 0..3 {
        for col in 0..6 {
            let id = d.place_family(level, [3.0 + col as f64 * 4.8, 5.0 + row as f64 * 4.0], &spec("Ceiling diffuser, square", CEILING_M)).unwrap();
            diffusers.push(id);
            halves[usize::from(col >= 3)].push(id);
        }
    }
    // Create Systems > Duct: one supply system per air handler, the air handler as its base
    let mut systems = Vec::new();
    for (k, terminals) in halves.into_iter().enumerate() {
        let name = format!("SA-{}", k + 1);
        let mut members = vec![ahus[k]];
        members.extend(terminals.iter().copied());
        let ops = d.create_system_commands(&name, openbim_family::mep::System::SupplyAir, &members).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        let ops = d.set_system_base_commands(&name, Some(ahus[k])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        systems.push((name, ahus[k], terminals));
    }
    DataHall { doc: d, level, ahus, diffusers, systems }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbim_document::Category;
    use openbim_family::mep::System;

    fn count(d: &Document, c: Category) -> usize { d.elements().filter(|e| e.category == c).count() }

    #[test]
    fn the_data_hall_is_generated_with_its_equipment_where_the_test_put_it() {
        let h = data_hall();
        assert_eq!((count(&h.doc, Category::Wall), count(&h.doc, Category::Room)), (4, 1));
        assert_eq!((h.ahus.len(), h.diffusers.len()), (2, 18));
        assert_eq!(count(&h.doc, Category::Family), 20);
        assert_eq!(h.doc.get(h.level).map(|l| l.category), Some(Category::Level));
        assert!(h.doc.room_shape(h.doc.elements().find(|e| e.category == Category::Room).unwrap().id).is_ok());
        // generated, not hand-modelled: the same call gives the same model
        assert_eq!(openbim_document::native::records(&h.doc), openbim_document::native::records(&data_hall().doc));
    }

    fn gid(doc: &Document, id: ElementId) -> String {
        let key = doc.project_key();
        openbim_ifc::guid::global_id(key, 0, id.0)
    }

    /// MEP-004, the chain of Revit's tutorial 3 (mep-part2 appendix A.6) on the data hall: terminals and an air handler
    /// make a supply system (done by the fixture), Generate Layout with a trunk of the size typed in the window, Finish
    /// Layout, End Points (the air handler is the source, the diffusers take their flow from their connectors),
    /// Compute, Apply sizes, and the IFC file: the system by its name with its members, and a port for every joined
    /// connector.
    #[test]
    fn the_supply_chain_runs_from_the_air_handler_to_the_diffusers_and_into_the_ifc_file() {
        use crate::compute_ui::{EndDraft, Mark};
        use openbim_document::{compute, Geometry, MepNode};
        let h = data_hall();
        let mut app = crate::Desktop::sample();
        app.document = h.doc.clone();
        let (name, base, terminals) = h.systems[0].clone();
        // Generate Layout: the diffusers take air from above at the ceiling, the air handler leaves sideways low down,
        // so the main runs above the ceiling and each is reached by a section of its own
        app.generate_layout_for(&name, Some([400.0, 300.0]), 0);
        let layout = app.systems.layout.as_ref().unwrap_or_else(|| panic!("{}", app.message));
        assert!(layout.solutions[0].label.contains("network at 3.90 m"), "{}", layout.solutions[0].label);
        assert!(layout.solutions[0].unconnected.is_empty(), "{:?}", layout.solutions[0].unconnected);
        // Finish Layout: ducts joined to every connector, in one undo step
        let steps = app.document.history_mark();
        app.finish_layout();
        assert!(app.message.starts_with("Layout placed") && app.message.contains("joined to 10 connector"), "{}", app.message);
        assert_eq!(app.document.history_mark(), steps + 1);
        let ducts: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Duct).map(|e| e.id).collect();
        assert!(ducts.len() > 10);
        let links = app.document.mep().equip_links.clone();
        assert_eq!(links.len(), 10, "the air handler and the nine diffusers");
        assert!(links.iter().any(|q| q.element == base) && terminals.iter().all(|t| links.iter().any(|q| q.element == *t)));
        // End Points
        app.choose_tool(crate::Tool::EndPoint);
        for q in &links {
            let MepNode::Run(rid) = q.port.node else { panic!("{q:?}") };
            let Some(Geometry::Linear { start, end, .. }) = app.document.get(rid).unwrap().geometry else { panic!() };
            let at = if q.port.port == 0 { start } else { end };
            app.end_draft = if q.element == base { EndDraft { mark: Mark::Source, kind: 0, a: 0.0, b: 0.0, pressure_kpa: 0.0 } } else { EndDraft { mark: Mark::Terminal, kind: 4, a: 0.0, b: 0.0, pressure_kpa: 0.0 } };
            app.end_point_click(h.level, at);
            assert!(app.message.contains("placed") || app.message.contains("Source marked"), "{}", app.message);
        }
        assert_eq!(app.document.mep().endpoints.len(), 10);
        // Compute the named system: nine diffusers of 0.05 m3/s
        app.compute.system = System::SupplyAir;
        app.compute.scope = name.clone();
        (app.compute.density, app.compute.viscosity_mpas, app.compute.specific_heat, app.compute.roughness_mm) = (1.2, 0.0181, 1006.0, 0.09);
        app.compute.catalogue = "300x200,300,200\n400x300,400,300\n500x300,500,300\n600x400,600,400\n800x500,800,500\n1000x600,1000,600\n".into();
        app.run_compute();
        assert!(app.compute.error.is_empty(), "{}", app.compute.error);
        let report = app.compute.report.clone().expect("a report").1;
        assert!((report.total_flow - 0.45).abs() < 1e-9, "{}", report.total_flow);
        assert_eq!((report.ends.len(), report.unreached.len(), report.runs.len()), (9, 0, ducts.len()));
        assert!(report.runs.iter().any(|r| r.critical));
        // Apply sizes: the branches carry less than the trunk, and one undo step takes it back
        let before: Vec<(f64, f64)> = ducts.iter().map(|d| { let e = app.document.get(*d).unwrap(); (e.width_m, e.height_m) }).collect();
        let n = compute::apply_sizes(&mut app.document, &report).unwrap();
        assert!(n > 0);
        let after: Vec<(f64, f64)> = ducts.iter().map(|d| { let e = app.document.get(*d).unwrap(); (e.width_m, e.height_m) }).collect();
        assert_ne!(before, after);
        // the IFC file: the system by its name and type, with the air handler in it, and the ports
        let key = app.document.ensure_project_key();
        let out = crate::export::export_ifc(&app.document, "Data hall", key, "2026-10-24T00:00:00").unwrap();
        let sys = out.text.lines().find(|l| l.contains("=IFCDISTRIBUTIONSYSTEM(") && l.contains("'SA-1'")).expect("the system SA-1 is in the file").to_string();
        assert!(sys.contains(".VENTILATION."), "{sys}");
        let sys_id = sys.split('=').next().unwrap().to_string();
        let assign = out.text.lines().find(|l| l.contains("=IFCRELASSIGNSTOGROUP(") && l.contains(&format!(",{sys_id})"))).expect("the members are assigned to the system");
        let ahu_line = out.text.lines().find(|l| l.contains("=IFCUNITARYEQUIPMENT(") && l.contains(&gid(&app.document, base))).expect("the air handler is in the file");
        let ahu_id = ahu_line.split('=').next().unwrap();
        assert!(assign.contains(&format!("{ahu_id},")) || assign.contains(&format!("{ahu_id})")), "{assign}");
        let count = |c: &str| out.text.lines().filter(|l| l.contains(&format!("={c}("))).count();
        assert_eq!(count("IFCDUCTSEGMENT"), ducts.len());
        assert_eq!(count("IFCAIRTERMINAL"), 18);
        assert!(count("IFCRELCONNECTSPORTS") >= 10, "a port joined for every connector: {}", count("IFCRELCONNECTSPORTS"));
        // the second system lays out as well
        let mut app2 = crate::Desktop::sample();
        app2.document = h.doc;
        app2.generate_layout_for("SA-2", Some([400.0, 300.0]), 0);
        let l2 = app2.systems.layout.as_ref().unwrap_or_else(|| panic!("{}", app2.message));
        assert!(l2.solutions[0].unconnected.is_empty(), "{:?}", l2.solutions[0].unconnected);
    }
}
