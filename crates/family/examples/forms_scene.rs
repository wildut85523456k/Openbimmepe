//! Prints volumes and exact bounds of a fixed set of forms for the independent IFC / OpenCASCADE check.
use openbim_family::forms::*;
use openbim_family::*;

fn show(name: &str, f: FormDef) {
    let fam = Family {
        name: "F".into(),
        forms: vec![f],
        ..Default::default()
    };
    let i = fam.compile().unwrap().solve(&Request::default()).unwrap();
    let form = &i.forms[0];
    let (lo, hi, exact) = form.bounds();
    println!(
        "FORM {name} {:.9} {:.9} {:.9} {:.9} {:.9} {:.9} {:.9} {}",
        form.volume(),
        lo[0],
        lo[1],
        lo[2],
        hi[0],
        hi[1],
        hi[2],
        exact
    );
}

fn main() {
    let r = ProfileDef::rect;
    let c = ProfileDef::circle;
    show(
        "E1",
        FormDef::extrusion("e", Axis::Z, "0", "3", r("0", "0", "0.4", "0.2")),
    );
    show(
        "E2",
        FormDef::extrusion(
            "e",
            Axis::Z,
            "1",
            "1.5",
            ProfileDef::polygon(&[
                ("0", "0"),
                ("0", "2"),
                ("1", "2"),
                ("1", "1"),
                ("2", "1"),
                ("2", "0"),
            ]),
        ),
    );
    show(
        "E3",
        FormDef::extrusion("e", Axis::X, "5", "8", c("1", "2", "0.05")).hole(c("1", "2", "0.04")),
    );
    show(
        "E4",
        FormDef::extrusion("e", Axis::Y, "7", "8", r("0", "3", "2", "5")).hole(c("1", "4", "0.5")),
    );
    show(
        "E5",
        FormDef::extrusion("e", Axis::Z, "0", "1", r("0", "0", "4", "2"))
            .hole(r("0.5", "0.5", "1.5", "1.5"))
            .hole(c("3", "1", "0.5")),
    );
    let rv = |p: Axis, off: &str, v: bool, at: &str, ang: &str, pr: ProfileDef| {
        FormDef::revolve("r", p, off, v, at, ang, pr)
    };
    for a in ["360", "180", "90", "270", "45", "200", "30"] {
        show(
            &format!("R{a}"),
            rv(Axis::Z, "0", true, "0", a, r("0", "0", "0.5", "2")),
        );
    }
    show(
        "Rtorus",
        rv(Axis::Z, "0", true, "0", "360", c("1", "0", "0.1")),
    );
    show(
        "Rtorus120",
        rv(Axis::Z, "0", true, "0", "120", c("1", "0", "0.1")),
    );
    show(
        "Ru",
        rv(Axis::Z, "0", false, "0", "360", r("0", "1", "3", "2")),
    );
    show(
        "Ru100",
        rv(
            Axis::Z,
            "0.5",
            false,
            "-1",
            "100",
            r("0", "0", "3", "2").clone(),
        ),
    );
    show(
        "Rx",
        rv(Axis::X, "4", true, "0", "360", r("0", "0", "1", "2")),
    );
    show(
        "Rx60",
        rv(Axis::X, "4", true, "0.5", "60", r("1", "0", "2", "2")),
    );
    show(
        "Ry",
        rv(Axis::Y, "9", true, "0", "300", r("0", "0", "1", "2")),
    );
    show(
        "Rcone",
        rv(
            Axis::Z,
            "0",
            true,
            "0",
            "360",
            ProfileDef::polygon(&[("0", "0"), ("0.5", "0"), ("0", "1")]),
        ),
    );
    show(
        "Rhole",
        rv(Axis::Z, "0", true, "0", "250", r("0.2", "0", "1", "2"))
            .hole(r("0.4", "0.5", "0.8", "1.5")),
    );
    let sw = |path: &[(&str, &str)], p: ProfileDef| FormDef::sweep("s", "1", path, p);
    let l = [("0", "0"), ("4", "0"), ("4", "3")];
    show("S1", sw(&l, r("-0.1", "-0.05", "0.1", "0.05")));
    show("S2", sw(&l, r("0", "-0.05", "0.2", "0.05")));
    show("S3", sw(&l, r("-0.2", "-0.05", "0", "0.05")));
    show(
        "S4",
        sw(
            &[("0", "0"), ("4", "0"), ("4", "-3")],
            r("0", "-0.05", "0.2", "0.05"),
        ),
    );
    show(
        "S5",
        sw(
            &[("0", "0"), ("4", "0"), ("4", "3"), ("1", "3")],
            r("0", "-0.05", "0.2", "0.05"),
        ),
    );
    show(
        "S6",
        sw(
            &[("0", "0"), ("4", "0"), ("4", "3"), ("8", "3")],
            r("0", "-0.05", "0.2", "0.05"),
        ),
    );
    show(
        "S7",
        sw(
            &[("0", "0"), ("4", "0"), ("6", "2")],
            r("0", "-0.05", "0.2", "0.05"),
        ),
    );
    show(
        "S8",
        sw(
            &[("0", "0"), ("5", "0"), ("7", "1"), ("8", "4"), ("3", "6")],
            r("-0.1", "0", "0.3", "0.2"),
        ),
    );
    show(
        "S9",
        sw(
            &[("0", "0"), ("3", "0"), ("3", "2")],
            r("0.02", "-0.02", "0.3", "0.04"),
        ),
    );
    show(
        "S10",
        sw(
            &[("0", "0"), ("3", "0"), ("2", "1.5"), ("-2", "1.2")],
            r("-0.15", "-0.05", "0.1", "0.2"),
        ),
    );
    show(
        "S11",
        sw(
            &[("0", "0"), ("4", "0"), ("4", "3")],
            r("-0.1", "-0.1", "0.1", "0.1"),
        )
        .hole(r("0.02", "-0.02", "0.08", "0.02")),
    );
}
