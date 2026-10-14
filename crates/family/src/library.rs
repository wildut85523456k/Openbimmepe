//! Authored families used to test the solver. Dimensions are illustrative defaults, not values from any standard.
//! Use `all()` to compile them together: the door nests the lever handle.

use crate::{compile_library, ArrayDef, Axis::*, CheckDef, Compiled, Family, FamilyError, NestedDef, OpeningDef, Scope::*, SolidDef, TextSrc, TypeDef, ValueDef as V};

fn chk(name: &str, expr: &str) -> CheckDef { CheckDef { name: name.into(), expr: expr.into() } }
fn ty(name: &str, values: &[(&str, f64)]) -> TypeDef {
    TypeDef { name: name.into(), values: values.iter().map(|(n, v)| (n.to_string(), *v)).collect(), texts: vec![] }
}

/// Lever handle on both faces of a leaf. Origin: centre of the back plate on the leaf's mid plane.
pub fn handle() -> Family {
    Family {
        name: "Lever handle".into(),
        values: vec![
            V::input("leaf_t", 0.04, 0.02, 0.1, Type),
            V::input("plate_w", 0.04, 0.02, 0.1, Type),
            V::input("plate_h", 0.16, 0.05, 0.4, Type),
            V::input("plate_t", 0.008, 0.003, 0.03, Type),
            V::input("grip_len", 0.12, 0.05, 0.3, Type),
            V::input("grip_proj", 0.05, 0.02, 0.12, Type),
            V::input("grip_t", 0.02, 0.01, 0.05, Type),
            V::flag("lever_right", true, Instance),
            V::material("finish", "Handle finish", Type),
            V::plane("x_p0", X, "-plate_w / 2"),
            V::plane("x_p1", X, "plate_w / 2"),
            V::plane("x_g0", X, "if(lever_right, x_p0, x_p1 - grip_len)"),
            V::plane("x_g1", X, "if(lever_right, x_p0 + grip_len, x_p1)"),
            V::plane("y_fp0", Y, "-leaf_t / 2 - plate_t"),
            V::plane("y_fp1", Y, "-leaf_t / 2"),
            V::plane("y_bp0", Y, "leaf_t / 2"),
            V::plane("y_bp1", Y, "leaf_t / 2 + plate_t"),
            V::plane("y_fg0", Y, "y_fp0 - grip_proj"),
            V::plane("y_bg1", Y, "y_bp1 + grip_proj"),
            V::plane("z_p0", Z, "-plate_h / 2"),
            V::plane("z_p1", Z, "plate_h / 2"),
            V::plane("z_g0", Z, "-grip_t / 2"),
            V::plane("z_g1", Z, "grip_t / 2"),
        ],
        solids: vec![
            SolidDef::new("plate_front", ("x_p0", "x_p1"), ("y_fp0", "y_fp1"), ("z_p0", "z_p1")).material("finish"),
            SolidDef::new("plate_back", ("x_p0", "x_p1"), ("y_bp0", "y_bp1"), ("z_p0", "z_p1")).material("finish"),
            SolidDef::new("grip_front", ("x_g0", "x_g1"), ("y_fg0", "y_fp0"), ("z_g0", "z_g1")).material("finish"),
            SolidDef::new("grip_back", ("x_g0", "x_g1"), ("y_bp1", "y_bg1"), ("z_g0", "z_g1")).material("finish"),
        ],
        checks: vec![chk("grip longer than the plate is wide", "grip_len > plate_w")],
        ..Default::default()
    }
}

/// Single flush door: frame (two jambs and a head), one leaf, and a nested lever handle on the side
/// opposite the hinges. Origin: middle of the opening at floor level, x across the opening, y through
/// the wall, z up.
pub fn door() -> Family {
    Family {
        name: "Door, single flush".into(),
        values: vec![
            V::input("width", 0.9, 0.5, 3.0, Type),
            V::input("height", 2.1, 1.5, 3.5, Type),
            V::input("frame", 0.05, 0.02, 0.2, Type),
            V::input("leaf_t", 0.04, 0.02, 0.1, Type),
            V::input("wall_t", 0.2, 0.05, 1.0, Instance),
            V::choice("swing", &["left", "right"], "right", Instance),
            V::flag("has_handle", true, Instance),
            V::input("open_angle", 0.0, 0.0, 170.0, Instance),
            V::input("handle_height", 1.0, 0.6, 1.5, Type),
            V::text("mark", "", Instance),
            V::text("remarks", "", Type),
            V::material("frame_material", "Frame material", Type),
            V::material("leaf_material", "Leaf material", Type),
            V::material("handle_finish", "Handle finish", Type),
            V::plane("x_left", X, "-width / 2"),
            V::plane("x_right", X, "width / 2"),
            V::plane("x_in_left", X, "x_left + frame"),
            V::plane("x_in_right", X, "x_right - frame"),
            V::plane("y_front", Y, "-wall_t / 2"),
            V::plane("y_back", Y, "wall_t / 2"),
            V::plane("y_leaf_front", Y, "-leaf_t / 2"),
            V::plane("y_leaf_back", Y, "leaf_t / 2"),
            V::plane("z_floor", Z, "0"),
            V::plane("z_head", Z, "height"),
            V::plane("z_in_head", Z, "z_head - frame"),
        ],
        solids: vec![
            SolidDef::new("jamb_left", ("x_left", "x_in_left"), ("y_front", "y_back"), ("z_floor", "z_head")).material("frame_material"),
            SolidDef::new("jamb_right", ("x_in_right", "x_right"), ("y_front", "y_back"), ("z_floor", "z_head")).material("frame_material"),
            SolidDef::new("head", ("x_in_left", "x_in_right"), ("y_front", "y_back"), ("z_in_head", "z_head")).material("frame_material"),
            SolidDef::new("leaf", ("x_in_left", "x_in_right"), ("y_leaf_front", "y_leaf_back"), ("z_floor", "z_in_head")).material("leaf_material")
                .rotate_about("if(swing.left, open_angle, -open_angle)", "if(swing.left, x_in_left, x_in_right)", "0"),
        ],
        nested: vec![
            NestedDef::new("handle", "Lever handle", ("if(swing.left, x_in_right - 0.07, x_in_left + 0.07)", "0", "handle_height"))
                .set("leaf_t", "leaf_t")
                .set("lever_right", "swing.right")
                .set_text("finish", TextSrc::Param("handle_finish".into()))
                .visible_if("has_handle")
                .rotate_about("if(swing.left, open_angle, -open_angle)", "if(swing.left, x_in_left, x_in_right)", "0"),
        ],
        checks: vec![
            chk("opening wider than four frames", "width > 4 * frame"),
            chk("opening higher than four frames", "height > 4 * frame"),
            chk("leaf thinner than the wall", "leaf_t < wall_t"),
            chk("handle fits on the leaf", "!has_handle || (width - 2 * frame > 0.35 && handle_height < height - frame - 0.15)"),
        ],
        opening: Some(OpeningDef::new("width", "height", "0")),
        types: vec![ty("Type A", &[("width", 0.8)]), ty("Type B", &[("width", 1.2), ("height", 2.4)])],
    }
}

/// Arched door: two jambs and a leaf up to the springing line of a segmental arch; the fanlight above the
/// springing line is left open (no glazing is modelled). `height` reaches the crown of the arch.
/// Origin: middle of the opening at floor level.
pub fn arched_door() -> Family {
    Family {
        name: "Door, arched".into(),
        values: vec![
            V::input("width", 1.0, 0.5, 3.0, Type),
            V::input("height", 2.4, 1.5, 4.0, Type),
            V::input("rise", 0.3, 0.05, 1.5, Type),
            V::input("frame", 0.05, 0.02, 0.2, Type),
            V::input("leaf_t", 0.04, 0.02, 0.1, Type),
            V::input("wall_t", 0.2, 0.05, 1.0, Instance),
            V::text("mark", "", Instance),
            V::material("frame_material", "Frame material", Type),
            V::material("leaf_material", "Leaf material", Type),
            V::plane("x_left", X, "-width / 2"),
            V::plane("x_right", X, "width / 2"),
            V::plane("x_in_left", X, "x_left + frame"),
            V::plane("x_in_right", X, "x_right - frame"),
            V::plane("y_front", Y, "-wall_t / 2"),
            V::plane("y_back", Y, "wall_t / 2"),
            V::plane("y_leaf_front", Y, "-leaf_t / 2"),
            V::plane("y_leaf_back", Y, "leaf_t / 2"),
            V::plane("z_floor", Z, "0"),
            V::plane("z_spring", Z, "height - rise"),
        ],
        solids: vec![
            SolidDef::new("jamb_left", ("x_left", "x_in_left"), ("y_front", "y_back"), ("z_floor", "z_spring")).material("frame_material"),
            SolidDef::new("jamb_right", ("x_in_right", "x_right"), ("y_front", "y_back"), ("z_floor", "z_spring")).material("frame_material"),
            SolidDef::new("leaf", ("x_in_left", "x_in_right"), ("y_leaf_front", "y_leaf_back"), ("z_floor", "z_spring")).material("leaf_material"),
        ],
        checks: vec![
            chk("rise at most half the width", "rise <= width / 2"),
            chk("rise less than the height", "rise < height - 0.5"),
            chk("leaf thinner than the wall", "leaf_t < wall_t"),
        ],
        opening: Some(OpeningDef::new("width", "height", "0").arched("rise")),
        types: vec![ty("Type A", &[("width", 0.9), ("rise", 0.2)])],
        ..Default::default()
    }
}

/// Fixed window: frame, one glass pane and a variable number of evenly spaced mullions (on both sides of the glass).
pub fn window() -> Family {
    let mullion = |name: &str, y: (&str, &str)| {
        let mut s = SolidDef::new(name, ("x_in_left", "x_in_right"), y, ("z_in_sill", "z_in_top")).material("frame_material");
        s.array = Some(ArrayDef { axis: X, count: "mullions".into(), bar: "mullion_w".into() });
        s
    };
    Family {
        name: "Window, fixed with mullions".into(),
        values: vec![
            V::input("width", 1.2, 0.3, 4.0, Type),
            V::input("height", 1.2, 0.3, 3.0, Type),
            V::input("frame", 0.05, 0.02, 0.2, Type),
            V::input("glass_t", 0.024, 0.004, 0.05, Type),
            V::input("mullion_w", 0.03, 0.01, 0.1, Type),
            V::input("ug", 1.1, 0.1, 8.0, Type),
            V::input("uf", 2.0, 0.1, 10.0, Type),
            V::input("psi", 0.06, 0.0, 1.0, Type),
            V::input("g_value", 0.6, 0.0, 1.0, Type),
            V::input("sill", 0.9, 0.0, 2.0, Instance),
            V::count("mullions", 1.0, 0.0, 20.0, Instance),
            V::input("wall_t", 0.2, 0.05, 1.0, Instance),
            V::text("mark", "", Instance),
            V::material("frame_material", "Frame material", Type),
            V::material("glass_material", "Glass", Type),
            V::plane("x_left", X, "-width / 2"),
            V::plane("x_right", X, "width / 2"),
            V::plane("x_in_left", X, "x_left + frame"),
            V::plane("x_in_right", X, "x_right - frame"),
            V::plane("y_front", Y, "-wall_t / 2"),
            V::plane("y_back", Y, "wall_t / 2"),
            V::plane("y_glass_front", Y, "-glass_t / 2"),
            V::plane("y_glass_back", Y, "glass_t / 2"),
            V::plane("z_sill", Z, "sill"),
            V::plane("z_top", Z, "sill + height"),
            V::plane("z_in_sill", Z, "z_sill + frame"),
            V::plane("z_in_top", Z, "z_top - frame"),
            // whole-window U-value: Uw = (Ag Ug + Af Uf + lg psi) / (Ag + Af); bars cover the glass, so each of the
            // mullions + 1 panes is seen between the frame and the bars
            V::formula("pane_w", "(width - 2 * frame - mullions * mullion_w) / (mullions + 1)"), // mean pane width
            V::formula("narrowest_pane", "(width - 2 * frame) / (mullions + 1) - if(mullions < 1, 0, if(mullions < 2, mullion_w / 2, mullion_w))"),
            V::formula("pane_h", "height - 2 * frame"),
            V::formula("window_area", "width * height"),
            V::formula("glass_area", "(mullions + 1) * pane_w * pane_h"),
            V::formula("frame_area", "window_area - glass_area"),
            V::formula("glass_edge_length", "(mullions + 1) * 2 * (pane_w + pane_h)"),
            V::formula("u_window", "(glass_area * ug + frame_area * uf + glass_edge_length * psi) / window_area"),
        ],
        solids: vec![
            SolidDef::new("jamb_left", ("x_left", "x_in_left"), ("y_front", "y_back"), ("z_sill", "z_top")).material("frame_material"),
            SolidDef::new("jamb_right", ("x_in_right", "x_right"), ("y_front", "y_back"), ("z_sill", "z_top")).material("frame_material"),
            SolidDef::new("sill_member", ("x_in_left", "x_in_right"), ("y_front", "y_back"), ("z_sill", "z_in_sill")).material("frame_material"),
            SolidDef::new("head_member", ("x_in_left", "x_in_right"), ("y_front", "y_back"), ("z_in_top", "z_top")).material("frame_material"),
            SolidDef::new("glass", ("x_in_left", "x_in_right"), ("y_glass_front", "y_glass_back"), ("z_in_sill", "z_in_top")).material("glass_material"),
            mullion("mullion_front", ("y_front", "y_glass_front")),
            mullion("mullion_back", ("y_glass_back", "y_back")),
        ],
        checks: vec![
            chk("opening wider than four frames", "width > 4 * frame"),
            chk("opening higher than four frames", "height > 4 * frame"),
            chk("glass thinner than the wall", "glass_t < wall_t"),
            chk("each pane at least 1 cm wide", "narrowest_pane >= 0.01"),
        ],
        opening: Some(OpeningDef::new("width", "height", "sill")),
        types: vec![ty("Type A", &[("width", 1.8), ("height", 1.5)])],
        ..Default::default()
    }
}

/// All sample families compiled together, in the order handle, door, window, arched door.
pub fn all() -> Result<Vec<Compiled>, FamilyError> { compile_library(&[handle(), door(), window(), arched_door()]) }
