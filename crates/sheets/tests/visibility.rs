mod common;
use common::*;
use openbim_sheets::model::Params;
use openbim_sheets::visibility::*;
use openbim_sheets::SheetError;

fn shown(l: &ViewLibrary, v: &ViewSettings, cat: &str, p: &Params) -> Display {
    l.display(v, &ElementRef { category: cat, params: p, link: None }).unwrap()
}
fn is_hidden(d: &Display) -> bool {
    *d == Display::Hidden
}
fn is_half(d: &Display) -> bool {
    matches!(d, Display::Shown { halftone: true, .. })
}
fn is_full(d: &Display) -> bool {
    matches!(d, Display::Shown { halftone: false, .. })
}

#[test]
fn discipline_defaults_by_view() {
    let l = library();
    let none = Params::new();
    let arch = ViewSettings::new("a", Discipline::Architectural);
    assert!(is_full(&shown(&l, &arch, "Walls", &none)));
    assert!(is_half(&shown(&l, &arch, "Columns", &none)));
    for c in ["Ducts", "Pipes", "Cable Trays", "Sprinklers"] {
        assert!(is_hidden(&shown(&l, &arch, c, &none)), "{c}");
    }
    let mech = ViewSettings::new("m", Discipline::Mechanical);
    assert!(is_full(&shown(&l, &mech, "Ducts", &none)));
    assert!(is_half(&shown(&l, &mech, "Walls", &none)));
    assert!(is_half(&shown(&l, &mech, "Columns", &none)));
    for c in ["Pipes", "Cable Trays", "Sprinklers"] {
        assert!(is_hidden(&shown(&l, &mech, c, &none)), "{c}");
    }
    let st = ViewSettings::new("s", Discipline::Structural);
    assert!(is_full(&shown(&l, &st, "Columns", &none)));
    assert!(is_half(&shown(&l, &st, "Walls", &none)));
    assert!(is_hidden(&shown(&l, &st, "Ducts", &none)));
    let co = ViewSettings::new("c", Discipline::Coordination);
    for c in ["Walls", "Doors", "Columns", "Ducts", "Pipes", "Cable Trays", "Sprinklers", "Grids"] {
        assert!(is_full(&shown(&l, &co, c, &none)), "{c}");
    }
    // shared categories show in every discipline
    for d in Discipline::ALL {
        assert!(is_full(&shown(&l, &ViewSettings::new("v", d), "Grids", &none)), "{d:?}");
    }
}

#[test]
fn explicit_overrides_beat_the_discipline_mode() {
    let l = library();
    let none = Params::new();
    let mut arch = ViewSettings::new("a", Discipline::Architectural);
    arch.categories.insert("Ducts".into(), Override { visible: Some(true), weight: Some(10), colour: Some([255, 0, 0]), ..Default::default() });
    arch.categories.insert("Walls".into(), Override { visible: Some(false), ..Default::default() });
    arch.categories.insert("Columns".into(), Override { halftone: Some(false), ..Default::default() });
    assert_eq!(shown(&l, &arch, "Ducts", &none), Display::Shown { halftone: false, weight: Some(10), colour: Some([255, 0, 0]) });
    assert!(is_hidden(&shown(&l, &arch, "Walls", &none)));
    assert!(is_full(&shown(&l, &arch, "Columns", &none)), "an explicit halftone=false wins over the discipline halftone");
}

#[test]
fn changing_the_modes_changes_the_view() {
    let l = library();
    let none = Params::new();
    let mut m = ViewSettings::new("m", Discipline::Mechanical);
    m.others.insert(Discipline::Architectural, Mode::Hide);
    m.others.insert(Discipline::Plumbing, Mode::Halftone);
    assert!(is_hidden(&shown(&l, &m, "Walls", &none)));
    assert!(is_half(&shown(&l, &m, "Pipes", &none)));
    // a discipline missing from the list is shown
    m.others.remove(&Discipline::Electrical);
    assert!(is_full(&shown(&l, &m, "Cable Trays", &none)));
    // but the view's own discipline and shared categories cannot be hidden by the table
    m.others.insert(Discipline::Mechanical, Mode::Hide);
    m.others.insert(Discipline::Coordination, Mode::Hide);
    assert!(is_full(&shown(&l, &m, "Ducts", &none)));
    assert!(is_full(&shown(&l, &m, "Grids", &none)));
}

#[test]
fn filters_apply_to_matching_elements_in_list_order() {
    let mut l = filters_lib();
    let mut v = ViewSettings::new("a", Discipline::Architectural);
    v.filters.push(("Fire rated".into(), Override { colour: Some([255, 0, 0]), weight: Some(12), ..Default::default() }));
    v.categories.insert("Walls".into(), Override { colour: Some([0, 0, 255]), weight: Some(7), ..Default::default() });
    l.add_view(v.clone()).unwrap();
    let rated = params(&[("Fire Rating", "60")]);
    let low = params(&[("Fire Rating", "30")]);
    let text = params(&[("Fire Rating", "n/a")]);
    let none = Params::new();
    assert_eq!(shown(&l, &v, "Walls", &rated), Display::Shown { halftone: false, weight: Some(12), colour: Some([255, 0, 0]) }, "filter beats the category override");
    assert_eq!(shown(&l, &v, "Walls", &low), Display::Shown { halftone: false, weight: Some(7), colour: Some([0, 0, 255]) }, "30 is not above 30");
    assert_eq!(shown(&l, &v, "Walls", &text), Display::Shown { halftone: false, weight: Some(7), colour: Some([0, 0, 255]) }, "text is not a number");
    assert_eq!(shown(&l, &v, "Walls", &none), Display::Shown { halftone: false, weight: Some(7), colour: Some([0, 0, 255]) }, "a missing parameter never matches");
    // the filter lists Walls and Doors only
    assert_eq!(shown(&l, &v, "Columns", &rated), Display::Shown { halftone: true, weight: None, colour: None });
    // the first filter in the list wins
    let mut two = ViewSettings::new("two", Discipline::Coordination);
    two.filters.push(("Fire rated".into(), Override { colour: Some([1, 2, 3]), ..Default::default() }));
    two.filters.push(("Fire rated".into(), Override { colour: Some([9, 9, 9]), weight: Some(5), ..Default::default() }));
    assert_eq!(shown(&l, &two, "Doors", &rated), Display::Shown { halftone: false, weight: Some(5), colour: Some([1, 2, 3]) });
    // a filter can hide
    let mut h = ViewSettings::new("h", Discipline::Mechanical);
    h.filters.push(("Large ducts".into(), Override { visible: Some(false), ..Default::default() }));
    assert!(is_hidden(&shown(&l, &h, "Ducts", &params(&[("Width", "500")]))));
    assert!(is_full(&shown(&l, &h, "Ducts", &params(&[("Width", "300")]))));
}

#[test]
fn rules() {
    let p = params(&[("Name", "Fire Door 1"), ("Level", "2"), ("Width", "0.9")]);
    let r = |param: &str, op, v: &str| Rule { param: param.into(), op, value: v.into() }.matches(&p);
    assert!(r("name", Op::Equals, "fire door 1"));
    assert!(!r("Name", Op::NotEquals, "FIRE DOOR 1"));
    assert!(r("Name", Op::Contains, "door"));
    assert!(r("Name", Op::BeginsWith, "fire"));
    assert!(!r("Name", Op::BeginsWith, "door"));
    assert!(r("Level", Op::Greater, "1"));
    assert!(r("Level", Op::Less, "10"), "numbers compare as numbers, not text");
    assert!(r("Width", Op::Less, "1"));
    assert!(!r("Name", Op::Greater, "1"));
    assert!(!r("Missing", Op::NotEquals, "x"));
    assert!(!r("Level", Op::Greater, "NaN"));
}

#[test]
fn linked_models_follow_host_a_linked_view_or_a_template() {
    let mut l = library();
    let none = Params::new();
    // template: only walls, everything else hidden
    let mut t = ViewSettings::new("Walls only", Discipline::Coordination);
    for c in ["Doors", "Columns", "Ducts", "Pipes", "Cable Trays", "Sprinklers", "Grids"] {
        t.categories.insert(c.into(), Override { visible: Some(false), ..Default::default() });
    }
    l.add_template(t).unwrap();
    l.add_view(ViewSettings::new("Linked arch", Discipline::Architectural)).unwrap();
    let mut host = ViewSettings::new("Host", Discipline::Mechanical);
    let el = |cat| ElementRef { category: cat, params: &none, link: Some("ARCH.obim") };
    // not listed: by host
    assert!(is_half(&l.display(&host, &el("Walls")).unwrap()));
    host.links.insert("ARCH.obim".into(), LinkDisplay::ByLinked("Linked arch".into()));
    assert!(is_full(&l.display(&host, &el("Walls")).unwrap()), "the linked view is architectural, walls show fully");
    assert!(is_half(&l.display(&host, &el("Columns")).unwrap()));
    host.links.insert("ARCH.obim".into(), LinkDisplay::Custom("Walls only".into()));
    assert!(is_full(&l.display(&host, &el("Walls")).unwrap()));
    assert!(is_hidden(&l.display(&host, &el("Doors")).unwrap()));
    // elements of the host itself are not affected by the link table
    assert!(is_full(&shown(&l, &host, "Ducts", &none)));
    host.links.insert("ARCH.obim".into(), LinkDisplay::ByLinked("Nope".into()));
    assert!(matches!(l.display(&host, &el("Walls")), Err(SheetError::Unknown { .. })));
}

#[test]
fn a_template_replaces_the_views_own_settings() {
    let mut l = library();
    let none = Params::new();
    let mut t = ViewSettings::new("MEP plan", Discipline::Mechanical);
    t.categories.insert("Walls".into(), Override { colour: Some([10, 10, 10]), ..Default::default() });
    l.add_template(t).unwrap();
    let mut v = ViewSettings::new("Level 1 HVAC", Discipline::Architectural);
    v.categories.insert("Ducts".into(), Override { visible: Some(true), ..Default::default() });
    v.template = Some("MEP plan".into());
    l.add_view(v.clone()).unwrap();
    let e = l.effective(&v);
    assert_eq!((e.name.as_str(), e.discipline), ("Level 1 HVAC", Discipline::Mechanical));
    assert!(is_full(&shown(&l, &v, "Ducts", &none)));
    assert!(matches!(shown(&l, &v, "Walls", &none), Display::Shown { halftone: true, colour: Some([10, 10, 10]), .. }));
    // the view's own Ducts override is not used, Pipes are hidden by the template's mode table
    assert!(is_hidden(&shown(&l, &v, "Pipes", &none)));
}

#[test]
fn listed_categories_follow_the_discipline() {
    let l = library();
    let names = |d| {
        let mut v: Vec<String> = l.listed_categories(&ViewSettings::new("v", d)).iter().map(|c| c.name.clone()).collect();
        v.sort();
        v
    };
    assert_eq!(names(Discipline::Mechanical), vec!["Columns", "Doors", "Ducts", "Grids", "Walls"]);
    assert_eq!(names(Discipline::Coordination).len(), 8);
    assert_eq!(names(Discipline::Electrical), vec!["Cable Trays", "Columns", "Doors", "Grids", "Walls"]);
}

#[test]
fn library_checks() {
    let mut l = library();
    assert!(matches!(l.add_category(Category { name: "Walls".into(), discipline: Discipline::Architectural, kind: CategoryKind::Model }), Err(SheetError::Duplicate { .. })));
    assert!(l.add_category(Category { name: "".into(), discipline: Discipline::Architectural, kind: CategoryKind::Model }).is_err());
    let mut bad = ViewSettings::new("v", Discipline::Mechanical);
    bad.categories.insert("Unknown".into(), Override::default());
    assert!(l.add_view(bad).is_err());
    let mut bad = ViewSettings::new("v", Discipline::Mechanical);
    bad.filters.push(("Nope".into(), Override::default()));
    assert!(l.add_view(bad).is_err());
    let mut bad = ViewSettings::new("v", Discipline::Mechanical);
    bad.categories.insert("Walls".into(), Override { weight: Some(17), ..Default::default() });
    assert!(l.add_view(bad).is_err());
    let mut bad = ViewSettings::new("v", Discipline::Mechanical);
    bad.template = Some("Nope".into());
    assert!(l.add_view(bad).is_err());
    assert!(l.add_filter(Filter { name: "f".into(), categories: vec![], rules: vec![] }).is_err());
    assert!(l.add_filter(Filter { name: "f".into(), categories: vec!["Nope".into()], rules: vec![Rule { param: "p".into(), op: Op::Equals, value: "v".into() }] }).is_err());
    let none = Params::new();
    let v = ViewSettings::new("v", Discipline::Mechanical);
    assert!(matches!(l.display(&v, &ElementRef { category: "Ghost", params: &none, link: None }), Err(SheetError::Unknown { .. })));
}

#[test]
fn discipline_names_parse() {
    for (t, d) in [("mechanical", Discipline::Mechanical), ("M", Discipline::Mechanical), ("Fire Protection", Discipline::FireProtection), ("fire", Discipline::FireProtection), (" e ", Discipline::Electrical), ("COORDINATION", Discipline::Coordination)] {
        assert_eq!(Discipline::parse(t), Some(d), "{t}");
    }
    assert_eq!(Discipline::parse("x"), None);
    assert_eq!(Discipline::parse(""), None);
}
