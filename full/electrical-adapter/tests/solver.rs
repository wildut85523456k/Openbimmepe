//! The edesign-alike solver behind the document's schematic, end to end. These are the solver-specific
//! tests that lived in `openbim-document` before the solver moved out of the main workspace; the
//! document keeps its plumbing tests with a marker engine.

use openbim_document::elec::NODE_BASE;
use openbim_document::electrical_calc::{self, CalcOutcome, ElectricalCalc};
use openbim_document::{Command, Discipline, Document, ElecCable, ElecKind, ElecNode, ElementId, MepData};
use openbim_electrical_adapter::{scenario, solver_network, EdesignSolver, NOMINAL_VOLTAGE_V};
use openbim_electrical_solver::{solve, SolverSettings};

fn install() {
    let _ = electrical_calc::install(Box::new(EdesignSolver));
}

fn with_mep(data: MepData) -> Document {
    let mut d = Document::spatial_sample();
    d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
    d
}

/// TX1 -> MSB (with `board_w` of its own) -> a load `name` of `load_w`, both 0.85 pf.
fn elec_doc(board_w: f64, load_w: f64, feeder_three: bool, branch_three: bool, name: &str) -> Document {
    let (a, b, c) = (NODE_BASE, NODE_BASE + 1, NODE_BASE + 2);
    let mut board = ElecNode::new(b, "MSB", ElecKind::Board, [10.0, 0.0], ElementId(10));
    board.watts = board_w;
    board.power_factor = 0.85;
    let mut load = ElecNode::new(c, name, ElecKind::Load, [20.0, 10.0], ElementId(10));
    load.watts = load_w;
    load.power_factor = 0.85;
    with_mep(MepData {
        elec_nodes: vec![ElecNode::new(a, "TX1", ElecKind::Source, [0.0, 0.0], ElementId(10)), board, load],
        elec_cables: vec![ElecCable::new(a, b, feeder_three), ElecCable::new(b, c, branch_three)],
        ..Default::default()
    })
}

fn current(annotation: &str) -> f64 {
    annotation.strip_prefix("A ").unwrap_or_else(|| panic!("{annotation:?} is not a current")).parse().unwrap()
}

/// Parse a stamped "V <n>"/"% <n>" annotation; fails on the placeholder and on NaN/inf.
fn finite_number(annotation: &str, prefix: &str) -> f64 {
    let value: f64 = annotation
        .strip_prefix(prefix)
        .unwrap_or_else(|| panic!("{annotation:?} is not stamped with {prefix:?}"))
        .parse()
        .unwrap_or_else(|_| panic!("{annotation:?} does not parse as a number"));
    assert!(value.is_finite(), "{annotation:?} is not finite");
    value
}

#[test]
fn the_installed_solver_stamps_the_load_current_and_marks_the_view_preliminary() {
    install();
    assert_eq!(electrical_calc::installed().name(), "edesign-alike solver");
    let mut d = elec_doc(0.0, 45_000.0, true, true, "Chiller");
    let view = d.generate_schematic(Discipline::Electrical).unwrap();
    d.transact(d.revision(), &[Command::GenerateSchematic { diagram: Box::new(view) }]).unwrap();
    let view = &d.schematic_views()[0];
    let msb = &view.symbols[0];
    // the feeder current is the chiller load current: Ib = P / (sqrt(3) * V * pf)
    let expected = 45_000.0 / (3f64.sqrt() * 400.0 * 0.85);
    assert!((current(&msb.annotations[0]) - expected).abs() < 1e-2, "{:?}", msb.annotations);
    // with the prototype impedance the drop is ~0, but the slots are computed, not placeholders
    assert!(finite_number(&msb.annotations[1], "V ").abs() < 1e-3);
    assert!(finite_number(&msb.annotations[2], "% ").abs() < 1e-3);
    assert!(view.not_approved);
    assert!(view.warnings.iter().any(|w| w.contains("MEP-010")));
    assert!(view.warnings.iter().any(|w| w.contains("MEP-002")));
    // undo then redo regenerates the stamp from the model
    assert!(d.undo().unwrap());
    assert!(d.redo().unwrap());
    assert!((current(&d.schematic_views()[0].symbols[0].annotations[0]) - expected).abs() < 1e-2);
}

#[test]
fn board_local_load_single_phase_and_rejections() {
    install();
    let three = 45_000.0 / (3f64.sqrt() * 400.0 * 0.85);
    let one = 45_000.0 / ((400.0 / 3f64.sqrt()) * 0.85);
    let stamp = |d: Document| d.generate_schematic(Discipline::Electrical).unwrap();
    // downstream 45 kW load on a three-phase feeder (golden)
    let v = stamp(elec_doc(0.0, 45_000.0, true, true, "Chiller"));
    assert!((current(&v.symbols[0].annotations[0]) - three).abs() < 1e-2);
    assert!(v.not_approved);
    // the board's own 45 kW is stamped from the board's load
    let v = stamp(elec_doc(45_000.0, 0.0, true, true, "Chiller"));
    assert!((current(&v.symbols[0].annotations[0]) - three).abs() < 1e-2);
    // all single-phase: 400/sqrt(3) V line-to-neutral, not 400 V line-to-line
    let v = stamp(elec_doc(0.0, 45_000.0, false, false, "Chiller"));
    assert!((current(&v.symbols[0].annotations[0]) - one).abs() < 1e-2);
    // a three-phase feeder into a single-phase load is rejected, not stamped 0 A
    let v = stamp(elec_doc(0.0, 45_000.0, true, false, "Chiller"));
    assert_eq!(v.symbols[0].annotations[0], "A —");
    assert!(v.warnings.iter().any(|w| w.contains("mixed-phase")));
    // an unnamed load is rejected by the solver graph
    let v = stamp(elec_doc(0.0, 45_000.0, true, true, ""));
    assert_eq!(v.symbols[0].annotations[0], "A —");
    assert!(v.warnings.iter().any(|w| w.contains("rejected")));
}

#[test]
fn an_unknown_cable_end_is_rejected_not_stamped() {
    let mut d = elec_doc(0.0, 45_000.0, true, true, "Chiller");
    let mut mep = d.mep().clone();
    mep.elec_cables.push(ElecCable::new(NODE_BASE, NODE_BASE + 99, true));
    d.transact(d.revision(), &[Command::SetMep(Box::new(mep))]).unwrap();
    match EdesignSolver.calculate(&d.electrical_network()) {
        CalcOutcome::Rejected { reasons } => assert!(reasons.iter().any(|r| r.contains("unknown to-node")), "{reasons:?}"),
        other => panic!("expected a rejection, got {other:?}"),
    }
}

#[test]
fn the_board_shows_the_cumulative_drop_not_its_feeders_own() {
    // TX1 -> MSB -> DB2 over two identical 100 m cables with a realistic 0.5 + j0.1 ohm/km, 45 kW at
    // DB2. The board's own cable has one drop, the source-to-DB2 path two: the annotation must carry
    // the path value, so reverting to `branch_voltage_drop_pct` fails this test.
    let (a, b, c) = (NODE_BASE, NODE_BASE + 1, NODE_BASE + 2);
    let cable = |from, to| {
        let mut k = ElecCable::new(from, to, true);
        k.extra_m = 100.0;
        k.r_ohm_per_km = Some(0.5);
        k.x_ohm_per_km = Some(0.1);
        k
    };
    let mut db2 = ElecNode::new(c, "DB2", ElecKind::Board, [20.0, 0.0], ElementId(10));
    db2.watts = 45_000.0;
    db2.power_factor = 0.85;
    let d = with_mep(MepData {
        elec_nodes: vec![
            ElecNode::new(a, "TX1", ElecKind::Source, [0.0, 0.0], ElementId(10)),
            ElecNode::new(b, "MSB", ElecKind::Board, [10.0, 0.0], ElementId(10)),
            db2,
        ],
        elec_cables: vec![cable(a, b), cable(b, c)],
        ..Default::default()
    });
    // the solver's own numbers for the DB2 cable
    let (network, branches) = solver_network(&d.electrical_network()).expect("a valid network");
    let report = solve(&network, &scenario(), &SolverSettings::default()).unwrap();
    let db2_cable = report.branches.iter().find(|r| branches.get(&r.branch) == Some(&1)).expect("DB2 cable result");
    let (own, path) = (db2_cable.branch_voltage_drop_pct, db2_cable.path_voltage_drop_pct);
    assert!(path > 1.5 * own && (path - own).abs() > 1.0, "the fixture separates branch {own} from path {path}");
    // what the engine hands the document for DB2
    let CalcOutcome::Solved { feeders, .. } = EdesignSolver.calculate(&d.electrical_network()) else { panic!("solved") };
    let f = feeders.iter().find(|f| f.to == c).expect("DB2 is fed");
    assert!((f.drop_pct - path).abs() < 1e-12, "the % is the path drop");
    assert!((f.drop_v - NOMINAL_VOLTAGE_V * path / 100.0).abs() < 1e-9, "V is that % of 400 V");
}
