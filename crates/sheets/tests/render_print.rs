mod common;
use common::*;
use openbim_sheets::model::Params;
use openbim_sheets::paper::Rect;
use openbim_sheets::render::*;
use openbim_sheets::titleblock::Align;
use proptest::prelude::*;

fn render(number: &str) -> RenderedSheet {
    render_sheet(&office_set(), &views(), &contents(), &lw(), number, &ctx()).unwrap()
}

type Seg = ([f64; 2], [f64; 2], f64, [u8; 3]);

fn lines(s: &RenderedSheet, layer: &str) -> Vec<Seg> {
    s.prims
        .iter()
        .filter_map(|p| match p {
            Prim::Line { a, b, weight_mm, rgb, layer: l } if l == layer => Some((*a, *b, *weight_mm, *rgb)),
            _ => None,
        })
        .collect()
}

fn texts(s: &RenderedSheet) -> Vec<([f64; 2], Align, String)> {
    s.prims
        .iter()
        .filter_map(|p| match p {
            Prim::Text { at, align, text, .. } => Some((*at, *align, text.clone())),
            _ => None,
        })
        .collect()
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

#[test]
fn viewport_maps_model_metres_to_sheet_millimetres() {
    // A1, free area x 22..649 and y 16..582, so the viewport centre is (335.5, 299); model centre (10, 6), 1:100 = 10 mm per m
    let s = render("A-101");
    let walls = lines(&s, "A-WALLS");
    assert_eq!(walls.len(), 4);
    let (a, b, w, rgb) = walls[0];
    assert!(near(a, [235.5, 239.0]) && near(b, [435.5, 239.0]), "{a:?} {b:?}");
    assert_eq!(w, 0.7, "weight number 8");
    assert_eq!(rgb, [0, 0, 0]);
    let grid = lines(&s, "C-GRIDS");
    assert_eq!(grid.len(), 1);
    assert!(near(grid[0].0, [205.5, 299.0]) && near(grid[0].1, [465.5, 299.0]));
}

#[test]
fn architectural_view_hides_services_and_halftones_structure() {
    let s = render("A-101");
    let layers: Vec<String> = s.prims.iter().filter_map(|p| if let Prim::Line { layer, .. } = p { Some(layer.clone()) } else { None }).collect();
    for hidden in ["M-DUCTS", "P-PIPES", "E-CABLE_TRAYS", "F-SPRINKLERS"] {
        assert!(!layers.iter().any(|l| l == hidden), "{hidden} must be hidden");
    }
    assert_eq!(lines(&s, "A-DOORS").len(), 1);
    let col = lines(&s, "S-COLUMNS");
    assert_eq!(col.len(), 1);
    assert_eq!(col[0].3, [127, 127, 127], "halftone is halfway to white");
    assert_eq!(col[0].2, 1.0, "weight number 9");
}

#[test]
fn mechanical_view_shows_ducts_over_a_halftone_background() {
    let s = render("M-101");
    let duct = lines(&s, "M-DUCTS");
    assert_eq!(duct.len(), 1);
    assert_eq!(duct[0].3, [0, 0, 0]);
    assert_eq!(duct[0].2, 0.35, "weight number 6");
    assert!(near(duct[0].0, [335.5 + (2.0 - 10.0) * 10.0, 299.0 + (3.0 - 6.0) * 10.0]));
    for l in ["A-WALLS", "A-DOORS", "S-COLUMNS"] {
        assert!(lines(&s, l).iter().all(|x| x.3 == [127, 127, 127]), "{l} is halftone");
    }
    for l in ["P-PIPES", "E-CABLE_TRAYS", "F-SPRINKLERS"] {
        assert!(lines(&s, l).is_empty(), "{l} hidden");
    }
}

#[test]
fn coordination_view_shows_everything_at_its_own_scale() {
    let s = render("C-001");
    for l in ["A-WALLS", "A-DOORS", "S-COLUMNS", "M-DUCTS", "P-PIPES", "E-CABLE_TRAYS", "F-SPRINKLERS", "C-GRIDS"] {
        assert!(!lines(&s, l).is_empty(), "{l}");
        assert!(lines(&s, l).iter().all(|x| x.3 == [0, 0, 0]));
    }
    // A3 at 1:200: free area centre (133.1, 150.5), 5 mm per metre
    let w = lines(&s, "A-WALLS")[0];
    assert!(near(w.0, [133.1 - 50.0, 150.5 - 30.0]), "{:?}", w.0);
}

#[test]
fn lines_are_clipped_to_the_viewport_frame() {
    let mut c = contents();
    c.get_mut("GF Coordination").unwrap().push(ModelPrim { a: [-1000.0, 6.0], b: [1000.0, 6.0], category: "Grids".into(), params: Params::new(), link: None, weight: 2 });
    c.get_mut("GF Coordination").unwrap().push(ModelPrim { a: [-1000.0, 500.0], b: [1000.0, 500.0], category: "Grids".into(), params: Params::new(), link: None, weight: 2 });
    let set = office_set();
    let s = render_sheet(&set, &views(), &c, &lw(), "C-001", &ctx()).unwrap();
    let fr = set.sheet("C-001").unwrap().viewports[0].frame();
    let grid = lines(&s, "C-GRIDS");
    assert_eq!(grid.len(), 2, "the line far above the frame is dropped");
    let long = grid.iter().find(|g| (g.0[0] - g.1[0]).abs() > 200.0).unwrap();
    assert!((long.0[0] - fr.x0).abs() < 1e-9 && (long.1[0] - fr.x1).abs() < 1e-9, "{:?}", long);
    assert!((long.0[1] - 150.5).abs() < 1e-9);
}

#[test]
fn clip_agrees_with_dense_sampling() {
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    proptest!(ProptestConfig::with_cases(400), |(ax in -300.0f64..300.0, ay in -300.0f64..300.0, bx in -300.0f64..300.0, by in -300.0f64..300.0)| {
        let (a, b) = ([ax, ay], [bx, by]);
        let n = 4000;
        let inside: Vec<f64> = (0..=n).map(|i| i as f64 / n as f64).filter(|t| r.contains([ax + t * (bx - ax), ay + t * (by - ay)])).collect();
        match clip_segment(a, b, &r) {
            None => prop_assert!(inside.is_empty(), "dropped a segment that crosses the rectangle"),
            Some((p, q)) => {
                prop_assert!(p[0] >= r.x0 - 1e-9 && p[0] <= r.x1 + 1e-9 && p[1] >= r.y0 - 1e-9 && p[1] <= r.y1 + 1e-9);
                prop_assert!(q[0] >= r.x0 - 1e-9 && q[0] <= r.x1 + 1e-9 && q[1] >= r.y0 - 1e-9 && q[1] <= r.y1 + 1e-9);
                // collinear with the original
                let cross = (bx - ax) * (p[1] - ay) - (by - ay) * (p[0] - ax);
                prop_assert!(cross.abs() < 1e-6 * (1.0 + (bx - ax).abs() + (by - ay).abs()) * 600.0);
                // every sampled inside point lies on the clipped piece
                let len2 = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2);
                for t in &inside {
                    let s = [ax + t * (bx - ax), ay + t * (by - ay)];
                    let u = if len2 == 0.0 { 0.0 } else { ((s[0] - p[0]) * (q[0] - p[0]) + (s[1] - p[1]) * (q[1] - p[1])) / len2 };
                    prop_assert!(u > -1e-3 && u < 1.0 + 1e-3, "inside point outside the clipped piece");
                }
            }
        }
    });
}

#[test]
fn halftone_and_weights_follow_the_tables() {
    let t = LineWeightTable::new(WEIGHTS.to_vec(), vec![(50, vec![0.1; 16]), (200, vec![0.3; 16])]).unwrap();
    assert_eq!(t.model_mm(5, 20), 0.1);
    assert_eq!(t.model_mm(5, 50), 0.1);
    assert_eq!(t.model_mm(5, 51), 0.3);
    assert_eq!(t.model_mm(5, 200), 0.3);
    assert_eq!(t.model_mm(5, 5000), 0.3, "beyond the last listed scale the last list is used");
    assert_eq!(t.paper_mm(1), 0.05);
    assert_eq!(t.paper_mm(16), 5.0);
    assert_eq!(t.paper_mm(0), 0.05);
    assert_eq!(t.paper_mm(99), 5.0);
    assert!(LineWeightTable::new(vec![0.1; 15], vec![(1, vec![0.1; 16])]).is_err());
    assert!(LineWeightTable::new(vec![0.1; 16], vec![]).is_err());
    assert!(LineWeightTable::new(vec![0.0; 16], vec![(1, vec![0.1; 16])]).is_err());
    assert!(LineWeightTable::new(vec![f64::NAN; 16], vec![(1, vec![0.1; 16])]).is_err());
    assert!(LineWeightTable::new(vec![0.1; 16], vec![(0, vec![0.1; 16])]).is_err());
    let s = render_sheet(&office_set(), &views(), &contents(), &t, "A-101", &ctx()).unwrap();
    assert_eq!(lines(&s, "A-WALLS")[0].2, 0.3, "at 1:100 the first listed scale of at least 100 is 200");
}

#[test]
fn title_block_text_is_filled_from_the_sheet_set() {
    let s = render("M-101");
    let t = texts(&s);
    let find = |text: &str| t.iter().find(|x| x.2 == text).unwrap_or_else(|| panic!("no text '{text}'")).clone();
    assert!(near(find("Sheet 2 of 3").0, [830.0, 58.0]));
    assert_eq!(find("Sheet 2 of 3").1, Align::Right);
    assert_eq!(find("Étage 0 – ventilation").2, "Étage 0 – ventilation");
    assert!(near(find("Étage 0 – ventilation").0, [652.0, 43.0]));
    find("Office Tower");
    find("Block B"); // {Facility} {Area}: the missing Area prints as nothing
    find("A1");
    find("1:100");
    assert!(!t.iter().any(|x| x.2 == "C-77"), "the contract number is not on the generic block");
}
