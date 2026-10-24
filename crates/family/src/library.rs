//! Authored families used to test the solver. Dimensions are illustrative defaults, not values from any standard.
//! Use `all()` to compile them together: the door nests the lever handle.

use crate::{
    compile_library, ArrayDef, Axis::*, Category, CheckDef, Compiled, ConnectorDef, Dim, Dir,
    Domain, Family, FamilyError, FlowDir, Host, NestedDef, OpeningDef, PlanDef, Scope::*, ShapeDef,
    light::{LightingDef, Mounting},
    SolidDef, System, TextSrc, TypeDef, ValueDef as V, NUMBER, U_VALUE,
};

fn chk(name: &str, expr: &str) -> CheckDef {
    CheckDef {
        name: name.into(),
        expr: expr.into(),
    }
}
fn ty(name: &str, values: &[(&str, f64)]) -> TypeDef {
    TypeDef {
        name: name.into(),
        values: values.iter().map(|(n, v)| (n.to_string(), *v)).collect(),
        texts: vec![],
        lighting: None,
    }
}

/// Lever handle on both faces of a leaf. Origin: centre of the back plate on the leaf's mid plane.
pub fn handle() -> Family {
    Family {
        name: "Lever handle".into(),
        code: "FAM-0001".into(),
        classification: vec![("Uniclass".into(), "Pr_30_36_59".into())],
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
            SolidDef::new(
                "plate_front",
                ("x_p0", "x_p1"),
                ("y_fp0", "y_fp1"),
                ("z_p0", "z_p1"),
            )
            .material("finish"),
            SolidDef::new(
                "plate_back",
                ("x_p0", "x_p1"),
                ("y_bp0", "y_bp1"),
                ("z_p0", "z_p1"),
            )
            .material("finish"),
            SolidDef::new(
                "grip_front",
                ("x_g0", "x_g1"),
                ("y_fg0", "y_fp0"),
                ("z_g0", "z_g1"),
            )
            .material("finish"),
            SolidDef::new(
                "grip_back",
                ("x_g0", "x_g1"),
                ("y_bp1", "y_bg1"),
                ("z_g0", "z_g1"),
            )
            .material("finish"),
        ],
        checks: vec![chk(
            "grip longer than the plate is wide",
            "grip_len > plate_w",
        )],
        ..Default::default()
    }
}

/// Single flush door: frame (two jambs and a head), one leaf, and a nested lever handle on the side
/// opposite the hinges. Origin: middle of the opening at floor level, x across the opening, y through
/// the wall, z up.
pub fn door() -> Family {
    Family {
        name: "Door, single flush".into(),
        code: "FAM-0002".into(),
        classification: vec![("Uniclass".into(), "Pr_30_59_24".into())],
        values: vec![
            V::input("width", 0.9, 0.5, 3.0, Type),
            V::input("height", 2.1, 1.5, 3.5, Type),
            V::input("frame", 0.05, 0.02, 0.2, Type),
            V::input("leaf_t", 0.04, 0.02, 0.1, Type),
            V::input("wall_t", 0.2, 0.05, 1.0, Instance),
            V::choice("swing", &["left", "right"], "right", Instance),
            V::flag("has_handle", true, Instance),
            V::angle("open_angle", 0.0, 0.0, 170.0, Instance),
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
            NestedDef::new("handle", "Lever handle", ("if(swing.left, x_in_right - 70 mm, x_in_left + 70 mm)", "0", "handle_height"))
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
            chk("handle fits on the leaf", "!has_handle || (width - 2 * frame > 350 mm && handle_height < height - frame - 150 mm)"),
        ],
        opening: Some(OpeningDef::new("width", "height", "0")),
        types: vec![ty("Type A", &[("width", 0.8)]), ty("Type B", &[("width", 1.2), ("height", 2.4)])],
        // the swing as drawn in plan: the leaf open at 90 degrees towards +y and the arc its free end sweeps
        plan: vec![
            PlanDef::line(("x_in_left", "0"), ("x_in_left", "width - 2 * frame")).visible_if("swing.left"),
            PlanDef::arc(("x_in_left", "0"), "width - 2 * frame", "0 deg", "90 deg").visible_if("swing.left"),
            PlanDef::line(("x_in_right", "0"), ("x_in_right", "width - 2 * frame")).visible_if("swing.right"),
            PlanDef::arc(("x_in_right", "0"), "width - 2 * frame", "90 deg", "180 deg").visible_if("swing.right"),
        ],
        category: Category::Door,
        ..Default::default()
    }
}

/// Arched door: two jambs and a leaf up to the springing line of a segmental arch; the fanlight above the
/// springing line is left open (no glazing is modelled). `height` reaches the crown of the arch.
/// Origin: middle of the opening at floor level.
pub fn arched_door() -> Family {
    Family {
        name: "Door, arched".into(),
        code: "FAM-0004".into(),
        classification: vec![("Uniclass".into(), "Pr_30_59_24".into())],
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
            SolidDef::new(
                "jamb_left",
                ("x_left", "x_in_left"),
                ("y_front", "y_back"),
                ("z_floor", "z_spring"),
            )
            .material("frame_material"),
            SolidDef::new(
                "jamb_right",
                ("x_in_right", "x_right"),
                ("y_front", "y_back"),
                ("z_floor", "z_spring"),
            )
            .material("frame_material"),
            SolidDef::new(
                "leaf",
                ("x_in_left", "x_in_right"),
                ("y_leaf_front", "y_leaf_back"),
                ("z_floor", "z_spring"),
            )
            .material("leaf_material"),
        ],
        checks: vec![
            chk("rise at most half the width", "rise <= width / 2"),
            chk("rise less than the height", "rise < height - 500 mm"),
            chk("leaf thinner than the wall", "leaf_t < wall_t"),
        ],
        opening: Some(OpeningDef::new("width", "height", "0").arched("rise")),
        types: vec![ty("Type A", &[("width", 0.9), ("rise", 0.2)])],
        category: Category::Door,
        ..Default::default()
    }
}

/// Fixed window: frame, one glass pane and a variable number of evenly spaced mullions (on both sides of the glass).
pub fn window() -> Family {
    let mullion = |name: &str, y: (&str, &str)| {
        let mut s = SolidDef::new(
            name,
            ("x_in_left", "x_in_right"),
            y,
            ("z_in_sill", "z_in_top"),
        )
        .material("frame_material");
        s.array = Some(ArrayDef {
            axis: X,
            count: "mullions".into(),
            bar: "mullion_w".into(),
        });
        s
    };
    Family {
        name: "Window, fixed with mullions".into(),
        code: "FAM-0003".into(),
        classification: vec![("Uniclass".into(), "Pr_30_59_98".into())],
        values: vec![
            V::input("width", 1.2, 0.3, 4.0, Type),
            V::input("height", 1.2, 0.3, 3.0, Type),
            V::input("frame", 0.05, 0.02, 0.2, Type),
            V::input("glass_t", 0.024, 0.004, 0.05, Type),
            V::input("mullion_w", 0.03, 0.01, 0.1, Type),
            V::input("ug", 1.1, 0.1, 8.0, Type).with_dim(U_VALUE),
            V::input("uf", 2.0, 0.1, 10.0, Type).with_dim(U_VALUE),
            V::input("psi", 0.06, 0.0, 1.0, Type).with_dim(Dim { len: 1, time: -3, mass: 1, temp: -1, ..NUMBER }),
            V::number("g_value", 0.6, 0.0, 1.0, Type),
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
            chk("each pane at least 1 cm wide", "narrowest_pane >= 10 mm"),
        ],
        opening: Some(OpeningDef::new("width", "height", "sill")),
        types: vec![ty("Type A", &[("width", 1.8), ("height", 1.5)])],
        // as drawn in plan: a line on each face of the wall and one through the glass
        plan: vec![
            PlanDef::line(("x_left", "y_front"), ("x_right", "y_front")),
            PlanDef::line(("x_left", "0"), ("x_right", "0")),
            PlanDef::line(("x_left", "y_back"), ("x_right", "y_back")),
        ],
        category: Category::Window,
        ..Default::default()
    }
}

/// All sample families, uncompiled, in the order handle, door, window, arched door, fan coil unit, air handling
/// unit, ceiling diffuser, VAV terminal unit, pump, valve, WC, washbasin, sprinkler head, luminaire, panel board,
/// pipe/duct hanger. This is the one list: `all()` compiles it, and a loader that has to check a catalogue
/// against the library reads it. Every family carries a Uniclass code (see docs/revit-style-ui/09).
pub fn families() -> Vec<Family> {
    vec![
        handle(),
        door(),
        window(),
        arched_door(),
        fan_coil(),
        ahu(),
        ceiling_diffuser(),
        vav(),
        pump(),
        valve(),
        wc(),
        washbasin(),
        sprinkler_head(),
        luminaire(),
        panel_board(),
        hanger(),
    ]
}

/// All sample families compiled together, in the same order as `families()`.
pub fn all() -> Result<Vec<Compiled>, FamilyError> {
    compile_library(&families())
}

/// Ceiling-mounted fan coil unit, ducted. Origin: centre of the casing in plan, underside of the casing.
/// Face +Y carries the supply air outlet, face -Y the return air inlet, face +X the water, condensate and power
/// connections. Sizes and flows are illustrative defaults to be replaced by the manufacturer's data.
pub fn fan_coil() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "Fan coil unit, ducted".into(),
        code: "FAM-0005".into(),
        classification: vec![("Uniclass".into(), "Pr_70_65_04_88".into())],
        values: vec![
            V::input("length", 1.0, 0.4, 3.0, Instance),
            V::input("depth", 0.6, 0.3, 1.5, Instance),
            V::input("height", 0.25, 0.15, 0.8, Instance),
            V::number("air_flow", 0.1, 0.01, 5.0, Type),
            V::number("water_flow", 0.0003, 0.00001, 0.05, Type),
            V::input("water_pipe", 0.022, 0.01, 0.1, Type),
            V::input("condensate_pipe", 0.02, 0.01, 0.05, Type),
            V::input("duct_w", 0.4, 0.1, 1.5, Type),
            V::input("duct_h", 0.2, 0.1, 1.0, Type),
            V::number("voltage", 230.0, 12.0, 1000.0, Type),
            V::number("power", 80.0, 1.0, 5000.0, Type),
            V::flag("with_return_duct", true, Instance),
            V::material("casing", "Casing", Type),
            V::plane("x0", X, "-length / 2"),
            V::plane("x1", X, "length / 2"),
            V::plane("y0", Y, "-depth / 2"),
            V::plane("y1", Y, "depth / 2"),
            V::plane("z0", Z, "0"),
            V::plane("z1", Z, "height"),
        ],
        solids: vec![
            SolidDef::new("body", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("casing"),
        ],
        checks: vec![
            chk(
                "outlet fits on the casing",
                "duct_w < length && duct_h < height",
            ),
            chk(
                "pipes fit the casing height",
                "water_pipe < height && condensate_pipe < height",
            ),
        ],
        connectors: vec![
            ConnectorDef::new(
                "supply",
                Duct,
                SupplyAir,
                Out,
                ("0", "depth / 2", "height / 2"),
                PlusY,
                ShapeDef::Rect("duct_w".into(), "duct_h".into()),
            )
            .flow("air_flow")
            .pressure_drop("0"),
            ConnectorDef::new(
                "return",
                Duct,
                ReturnAir,
                In,
                ("0", "-depth / 2", "height / 2"),
                MinusY,
                ShapeDef::Rect("duct_w".into(), "duct_h".into()),
            )
            .flow("air_flow")
            .visible_if("with_return_duct"),
            ConnectorDef::new(
                "chw_in",
                Pipe,
                ChilledWaterSupply,
                In,
                ("length / 2", "depth / 4", "height / 3"),
                PlusX,
                ShapeDef::Round("water_pipe".into()),
            )
            .flow("water_flow"),
            ConnectorDef::new(
                "chw_out",
                Pipe,
                ChilledWaterReturn,
                Out,
                ("length / 2", "-depth / 4", "height / 3"),
                PlusX,
                ShapeDef::Round("water_pipe".into()),
            )
            .flow("water_flow"),
            ConnectorDef::new(
                "condensate",
                Pipe,
                Condensate,
                Out,
                ("length / 2", "0", "height / 6"),
                PlusX,
                ShapeDef::Round("condensate_pipe".into()),
            ),
            ConnectorDef::new(
                "power",
                Electrical,
                Power,
                In,
                ("length / 2", "depth / 2 - 50 mm", "2 * height / 3"),
                PlusX,
                ShapeDef::Electrical {
                    voltage: "voltage".into(),
                    poles: "1".into(),
                },
            )
            .flow("power"),
            // BMS link to the unit controller (signal both ways)
            ConnectorDef::new(
                "bms",
                Electrical,
                Controls,
                Both,
                ("length / 2", "depth / 2 - 120 mm", "2 * height / 3"),
                PlusX,
                ShapeDef::Signal,
            ),
        ],
        plan: vec![
            PlanDef::line(("x0", "y0"), ("x1", "y0")),
            PlanDef::line(("x1", "y0"), ("x1", "y1")),
            PlanDef::line(("x1", "y1"), ("x0", "y1")),
            PlanDef::line(("x0", "y1"), ("x0", "y0")),
            PlanDef::line(("x0", "y0"), ("x1", "y1")),
            PlanDef::line(("x0", "y1"), ("x1", "y0")),
        ],
        category: Category::UnitaryEquipment,
        ..Default::default()
    }
}

/// Air handling unit, floor mounted: a casing with a supply and a return duct connection, a chilled and a heating
/// water coil, condensate, power and a BMS link. Origin: centre of the casing in plan, underside of the casing.
/// Face +Y carries the supply air outlet, face -Y the return air inlet, face +X the water, condensate and power
/// connections. Sizes and flows are illustrative defaults to be replaced by the manufacturer data.
pub fn ahu() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "Air handling unit".into(),
        code: "FAM-0006".into(),
        classification: vec![("Uniclass".into(), "Pr_60_65_03".into())],
        values: vec![
            V::input("length", 2.5, 1.0, 6.0, Instance),
            V::input("depth", 1.2, 0.6, 3.0, Instance),
            V::input("height", 1.4, 0.6, 2.5, Instance),
            V::number("air_flow", 1.2, 0.05, 10.0, Type),
            V::input("supply_duct_w", 0.8, 0.1, 2.0, Type),
            V::input("supply_duct_h", 0.5, 0.1, 1.5, Type),
            V::input("return_duct_w", 0.8, 0.1, 2.0, Type),
            V::input("return_duct_h", 0.5, 0.1, 1.5, Type),
            V::number("chw_flow", 0.0006, 0.00001, 0.05, Type),
            V::input("chw_pipe", 0.05, 0.01, 0.2, Type),
            V::number("hw_flow", 0.0004, 0.00001, 0.05, Type),
            V::input("hw_pipe", 0.04, 0.01, 0.2, Type),
            V::input("condensate_pipe", 0.032, 0.01, 0.05, Type),
            V::number("voltage", 400.0, 12.0, 1000.0, Type),
            V::number("power", 4000.0, 1.0, 100000.0, Type),
            V::flag("with_return_duct", true, Instance),
            V::material("casing", "Casing", Type),
            V::plane("x0", X, "-length / 2"),
            V::plane("x1", X, "length / 2"),
            V::plane("y0", Y, "-depth / 2"),
            V::plane("y1", Y, "depth / 2"),
            V::plane("z0", Z, "0"),
            V::plane("z1", Z, "height"),
        ],
        solids: vec![
            SolidDef::new("body", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("casing"),
        ],
        checks: vec![
            chk("the supply duct fits the casing face", "supply_duct_w < length && supply_duct_h < height"),
            chk("the return duct fits the casing face", "return_duct_w < length && return_duct_h < height"),
            chk("the pipes fit the casing height", "chw_pipe < height && hw_pipe < height && condensate_pipe < height"),
        ],
        connectors: vec![
            ConnectorDef::new(
                "supply",
                Duct,
                SupplyAir,
                Out,
                ("0", "depth / 2", "height / 2"),
                PlusY,
                ShapeDef::Rect("supply_duct_w".into(), "supply_duct_h".into()),
            )
            .flow("air_flow")
            .pressure_drop("0"),
            ConnectorDef::new(
                "return",
                Duct,
                ReturnAir,
                In,
                ("0", "-depth / 2", "height / 2"),
                MinusY,
                ShapeDef::Rect("return_duct_w".into(), "return_duct_h".into()),
            )
            .flow("air_flow")
            .visible_if("with_return_duct"),
            ConnectorDef::new(
                "chw_in",
                Pipe,
                ChilledWaterSupply,
                In,
                ("length / 2", "depth / 4", "height / 4"),
                PlusX,
                ShapeDef::Round("chw_pipe".into()),
            )
            .flow("chw_flow"),
            ConnectorDef::new(
                "chw_out",
                Pipe,
                ChilledWaterReturn,
                Out,
                ("length / 2", "0", "height / 4"),
                PlusX,
                ShapeDef::Round("chw_pipe".into()),
            )
            .flow("chw_flow"),
            ConnectorDef::new(
                "hw_in",
                Pipe,
                HeatingWaterSupply,
                In,
                ("length / 2", "-depth / 4", "height / 4"),
                PlusX,
                ShapeDef::Round("hw_pipe".into()),
            )
            .flow("hw_flow"),
            ConnectorDef::new(
                "hw_out",
                Pipe,
                HeatingWaterReturn,
                Out,
                ("length / 2", "-depth / 2 + 150 mm", "height / 4"),
                PlusX,
                ShapeDef::Round("hw_pipe".into()),
            )
            .flow("hw_flow"),
            ConnectorDef::new(
                "condensate",
                Pipe,
                Condensate,
                Out,
                ("length / 2", "depth / 4", "height / 8"),
                PlusX,
                ShapeDef::Round("condensate_pipe".into()),
            ),
            ConnectorDef::new(
                "power",
                Electrical,
                Power,
                In,
                ("length / 2", "depth / 2 - 150 mm", "3 * height / 4"),
                PlusX,
                ShapeDef::Electrical {
                    voltage: "voltage".into(),
                    poles: "3".into(),
                },
            )
            .flow("power"),
            // BMS link to the unit controller (signal both ways)
            ConnectorDef::new(
                "bms",
                Electrical,
                Controls,
                Both,
                ("length / 2", "depth / 2 - 250 mm", "3 * height / 4"),
                PlusX,
                ShapeDef::Signal,
            ),
        ],
        plan: vec![
            PlanDef::line(("x0", "y0"), ("x1", "y0")),
            PlanDef::line(("x1", "y0"), ("x1", "y1")),
            PlanDef::line(("x1", "y1"), ("x0", "y1")),
            PlanDef::line(("x0", "y1"), ("x0", "y0")),
            PlanDef::line(("x0", "y0"), ("x1", "y1")),
            PlanDef::line(("x0", "y1"), ("x1", "y0")),
        ],
        category: Category::UnitaryEquipment,
        host: Some(Host::Floor),
        ..Default::default()
    }
}

/// Ceiling diffuser, square faced with a round neck. Origin: centre of the face on the ceiling plane (the face hangs
/// a little below the ceiling and the neck rises from it into the duct above). The one connector is the duct drop
/// above the unit, so a run joins it from +Z. Sizes and flows are illustrative defaults to be replaced by the
/// manufacturer data.
pub fn ceiling_diffuser() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "Ceiling diffuser, square".into(),
        code: "FAM-0007".into(),
        classification: vec![("Uniclass".into(), "Pr_70_65_04_02".into())],
        values: vec![
            V::input("face", 0.6, 0.15, 1.2, Instance),
            V::input("neck_d", 0.2, 0.05, 0.6, Type),
            V::input("neck_h", 0.15, 0.02, 0.5, Type),
            V::number("air_flow", 0.05, 0.005, 1.0, Type),
            V::number("pressure_drop", 30.0, 0.0, 500.0, Type),
            V::material("face_material", "Diffuser", Type),
            V::plane("nx0", X, "-neck_d / 2"),
            V::plane("nx1", X, "neck_d / 2"),
            V::plane("ny0", Y, "-neck_d / 2"),
            V::plane("ny1", Y, "neck_d / 2"),
            V::plane("x0", X, "-face / 2"),
            V::plane("x1", X, "face / 2"),
            V::plane("y0", Y, "-face / 2"),
            V::plane("y1", Y, "face / 2"),
            V::plane("z0", Z, "-0.025"),
            V::plane("z1", Z, "0"),
            V::plane("z2", Z, "neck_h"),
        ],
        solids: vec![
            SolidDef::new("face", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("face_material"),
            SolidDef::new("neck", ("nx0", "nx1"), ("ny0", "ny1"), ("z1", "z2")).material("face_material"),
        ],
        checks: vec![
            chk("the neck fits inside the face", "neck_d < face"),
            chk("the neck rises above the face", "neck_h > 0"),
        ],
        connectors: vec![
            ConnectorDef::new(
                "supply",
                Duct,
                SupplyAir,
                In,
                ("0", "0", "z2"),
                PlusZ,
                ShapeDef::Round("neck_d".into()),
            )
            .flow("air_flow")
            .pressure_drop("pressure_drop"),
        ],
        plan: vec![
            PlanDef::circle(("0", "0"), "face / 2"),
            PlanDef::line(("x0", "0"), ("x1", "0")),
            PlanDef::line(("0", "y0"), ("0", "y1")),
        ],
        category: Category::AirTerminal,
        ..Default::default()
    }
}

/// Variable air volume (VAV) terminal unit, single duct, pressure independent. Origin: centre of the casing in
/// plan, underside of the casing. The inlet is the duct drop above the unit (+Z), the outlet faces down (-Z)
/// towards the diffuser, and the control connector carries the damper actuator signal. Sizes and flows are
/// illustrative defaults to be replaced by the manufacturer data.
pub fn vav() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "VAV terminal unit, single duct".into(),
        code: "FAM-0008".into(),
        classification: vec![("Uniclass".into(), "Pr_70_65_04_94".into())],
        values: vec![
            V::input("length", 0.9, 0.4, 2.5, Instance),
            V::input("width", 0.35, 0.15, 1.0, Type),
            V::input("height", 0.3, 0.15, 0.8, Type),
            V::input("inlet_d", 0.2, 0.05, 0.6, Type),
            V::input("outlet_d", 0.2, 0.05, 0.6, Type),
            V::number("air_flow", 0.1, 0.005, 2.0, Type),
            V::number("min_flow", 0.03, 0.0, 0.5, Type),
            V::number("pressure_drop", 50.0, 0.0, 500.0, Type),
            V::material("casing", "VAV casing", Type),
            V::plane("x0", X, "-length / 2"),
            V::plane("x1", X, "length / 2"),
            V::plane("y0", Y, "-width / 2"),
            V::plane("y1", Y, "width / 2"),
            V::plane("z0", Z, "0"),
            V::plane("z1", Z, "height"),
            V::plane("z2", Z, "height + 150 mm"),
            V::plane("ix0", X, "-inlet_d / 2"),
            V::plane("ix1", X, "inlet_d / 2"),
            V::plane("iy0", Y, "-inlet_d / 2"),
            V::plane("iy1", Y, "inlet_d / 2"),
        ],
        solids: vec![
            SolidDef::new("casing", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("casing"),
            SolidDef::new("inlet", ("ix0", "ix1"), ("iy0", "iy1"), ("z1", "z2")).material("casing"),
        ],
        checks: vec![
            chk("the inlet fits inside the casing", "inlet_d < width && inlet_d < length"),
            chk("the outlet fits inside the casing", "outlet_d < width && outlet_d < length"),
            chk("the minimum flow is below the maximum", "min_flow < air_flow"),
        ],
        connectors: vec![
            ConnectorDef::new(
                "supply",
                Duct,
                SupplyAir,
                In,
                ("0", "0", "z2"),
                PlusZ,
                ShapeDef::Round("inlet_d".into()),
            )
            .flow("air_flow")
            .pressure_drop("pressure_drop"),
            ConnectorDef::new("outlet", Duct, SupplyAir, Out, ("0", "0", "z0"), MinusZ, ShapeDef::Round("outlet_d".into()))
                .flow("air_flow"),
            ConnectorDef::new("control", Electrical, Controls, In, ("x1", "0", "height / 2"), PlusX, ShapeDef::Signal),
        ],
        plan: vec![
            PlanDef::line(("x0", "y0"), ("x1", "y0")),
            PlanDef::line(("x1", "y0"), ("x1", "y1")),
            PlanDef::line(("x1", "y1"), ("x0", "y1")),
            PlanDef::line(("x0", "y1"), ("x0", "y0")),
        ],
        category: Category::UnitaryEquipment,
        host: Some(Host::Ceiling),
        ..Default::default()
    }
}

/// End-suction centrifugal pump, close coupled: a base, a volute casing and a motor. Origin: centre of the base
/// in plan, underside of the base. The suction enters along +X at the centreline, the discharge leaves along -X
/// at the top of the volute. Sizes and flows are illustrative defaults to be replaced by the manufacturer data.
pub fn pump() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "Pump, end suction".into(),
        code: "FAM-0009".into(),
        classification: vec![("Uniclass".into(), "Pr_65_53".into())],
        values: vec![
            V::input("length", 0.9, 0.4, 3.0, Instance),
            V::input("width", 0.5, 0.3, 1.5, Instance),
            V::input("height", 0.5, 0.3, 1.5, Instance),
            V::input("base_h", 0.05, 0.02, 0.2, Type),
            V::input("suction_d", 0.1, 0.025, 0.4, Type),
            V::input("discharge_d", 0.075, 0.02, 0.3, Type),
            V::number("water_flow", 0.001, 0.00001, 0.1, Type),
            V::number("head_m", 20.0, 0.0, 200.0, Type),
            V::number("voltage", 400.0, 12.0, 1000.0, Type),
            V::number("power", 2200.0, 1.0, 50000.0, Type),
            V::material("casing", "Pump casing", Type),
            V::plane("x0", X, "-length / 2"),
            V::plane("x1", X, "length / 2"),
            V::plane("y0", Y, "-width / 2"),
            V::plane("y1", Y, "width / 2"),
            V::plane("z0", Z, "0"),
            V::plane("z1", Z, "base_h"),
            V::plane("z2", Z, "height"),
            V::plane("sx0", X, "-suction_d / 2"),
            V::plane("sx1", X, "suction_d / 2"),
            V::plane("sy0", Y, "-suction_d / 2"),
            V::plane("sy1", Y, "suction_d / 2"),
        ],
        solids: vec![
            SolidDef::new("base", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("casing"),
            SolidDef::new("volute", ("sx0", "sx1"), ("sy0", "sy1"), ("z1", "z2")).material("casing"),
            SolidDef::new("motor", ("x0", "sx1"), ("sy0", "sy1"), ("z1", "z2")).material("casing"),
        ],
        checks: vec![
            chk("the suction fits the casing", "suction_d < width"),
            chk("the discharge is smaller than the suction", "discharge_d < suction_d"),
            chk("the base is thinner than the unit", "base_h < height"),
        ],
        connectors: vec![
            ConnectorDef::new(
                "suction",
                Pipe,
                ChilledWaterSupply,
                In,
                ("x0", "0", "height / 2"),
                MinusX,
                ShapeDef::Round("suction_d".into()),
            )
            .flow("water_flow"),
            ConnectorDef::new(
                "discharge",
                Pipe,
                ChilledWaterSupply,
                Out,
                ("x1", "0", "height / 2"),
                PlusX,
                ShapeDef::Round("discharge_d".into()),
            )
            .flow("water_flow"),
            ConnectorDef::new(
                "power",
                Electrical,
                Power,
                In,
                ("0", "y1", "height / 2"),
                PlusY,
                ShapeDef::Electrical {
                    voltage: "voltage".into(),
                    poles: "3".into(),
                },
            )
            .flow("power"),
        ],
        plan: vec![
            PlanDef::line(("x0", "y0"), ("x1", "y0")),
            PlanDef::line(("x1", "y0"), ("x1", "y1")),
            PlanDef::line(("x1", "y1"), ("x0", "y1")),
            PlanDef::line(("x0", "y1"), ("x0", "y0")),
        ],
        category: Category::Pump,
        host: Some(Host::Floor),
        ..Default::default()
    }
}

/// Globe valve, screwed, in a pipe run. Origin: centre of the body in plan, on the pipe centreline. Flow is
/// along X. Sizes and flows are illustrative defaults to be replaced by the manufacturer data.
pub fn valve() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "Valve, globe".into(),
        code: "FAM-0010".into(),
        classification: vec![("Uniclass".into(), "Pr_65_54".into())],
        values: vec![
            V::input("body_l", 0.2, 0.08, 0.6, Instance),
            V::input("body_d", 0.1, 0.05, 0.3, Type),
            V::input("pipe_d", 0.05, 0.015, 0.2, Type),
            V::number("water_flow", 0.0005, 0.000001, 0.05, Type),
            V::number("kv", 6.0, 0.1, 100.0, Type),
            V::material("body", "Valve body", Type),
            V::plane("x0", X, "-body_l / 2"),
            V::plane("x1", X, "body_l / 2"),
            V::plane("y0", Y, "-body_d / 2"),
            V::plane("y1", Y, "body_d / 2"),
            V::plane("z0", Z, "-body_d / 2"),
            V::plane("z1", Z, "body_d / 2"),
        ],
        solids: vec![SolidDef::new("body", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("body")],
        checks: vec![
            chk("the pipe fits the body", "pipe_d < body_d"),
            chk("the body is longer than it is wide", "body_l > body_d"),
        ],
        connectors: vec![
            ConnectorDef::new("in", Pipe, ChilledWaterSupply, In, ("x0", "0", "0"), MinusX, ShapeDef::Round("pipe_d".into()))
                .flow("water_flow"),
            ConnectorDef::new("out", Pipe, ChilledWaterSupply, Out, ("x1", "0", "0"), PlusX, ShapeDef::Round("pipe_d".into()))
                .flow("water_flow"),
        ],
        plan: vec![
            PlanDef::line(("x0", "y0"), ("x1", "y0")),
            PlanDef::line(("x1", "y0"), ("x1", "y1")),
            PlanDef::line(("x1", "y1"), ("x0", "y1")),
            PlanDef::line(("x0", "y1"), ("x0", "y0")),
        ],
        category: Category::Valve,
        host: Some(Host::Free),
        ..Default::default()
    }
}

/// Wash-down WC with a close-coupled cistern. Origin: centre of the bowl in plan, on the floor. The water
/// supply enters at the back (+Y) and the sanitary discharge leaves at the base (-Y). Sizes and flows are
/// illustrative defaults to be replaced by the manufacturer data.
pub fn wc() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "WC, close coupled".into(),
        code: "FAM-0011".into(),
        classification: vec![("Uniclass".into(), "Pr_40_20_93".into())],
        values: vec![
            V::input("length", 0.7, 0.5, 1.0, Instance),
            V::input("width", 0.4, 0.3, 0.6, Instance),
            V::input("height", 0.8, 0.5, 1.2, Instance),
            V::input("supply_d", 0.015, 0.008, 0.05, Type),
            V::input("discharge_d", 0.1, 0.05, 0.2, Type),
            V::number("wfu", 4.0, 0.0, 10.0, Type),
            V::number("cwfu", 4.0, 0.0, 10.0, Type),
            V::number("discharge_flow", 0.002, 0.0001, 0.02, Type),
            V::material("finish", "WC finish", Type),
            V::plane("x0", X, "-width / 2"),
            V::plane("x1", X, "width / 2"),
            V::plane("y0", Y, "-length / 2"),
            V::plane("y1", Y, "length / 2"),
            V::plane("z0", Z, "0"),
            V::plane("z1", Z, "height"),
        ],
        solids: vec![SolidDef::new("bowl", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("finish")],
        checks: vec![
            chk("the discharge fits the bowl", "discharge_d < width"),
            chk("the WC stands on the floor", "height > 0"),
        ],
        connectors: vec![
            ConnectorDef::new("supply", Pipe, DomesticCold, In, ("0", "y1", "height / 2"), PlusY, ShapeDef::Round("supply_d".into())),
            ConnectorDef::new("discharge", Pipe, Sanitary, Out, ("0", "y0", "0"), MinusY, ShapeDef::Round("discharge_d".into()))
                .flow("discharge_flow"),
        ],
        plan: vec![
            PlanDef::line(("x0", "y0"), ("x1", "y0")),
            PlanDef::line(("x1", "y0"), ("x1", "y1")),
            PlanDef::line(("x1", "y1"), ("x0", "y1")),
            PlanDef::line(("x0", "y1"), ("x0", "y0")),
        ],
        category: Category::SanitaryTerminal,
        host: Some(Host::Floor),
        ..Default::default()
    }
}

/// Wall-hung washbasin with a pedestal. Origin: centre of the basin in plan, on the wall face (+Y is the room).
/// The water supply enters at the back (-Y) and the waste leaves at the base. Sizes and flows are illustrative
/// defaults to be replaced by the manufacturer data.
pub fn washbasin() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "Washbasin, wall hung".into(),
        code: "FAM-0012".into(),
        classification: vec![("Uniclass".into(), "Pr_40_20_96".into())],
        values: vec![
            V::input("length", 0.6, 0.4, 1.0, Instance),
            V::input("width", 0.5, 0.35, 0.8, Instance),
            V::input("height", 0.8, 0.6, 1.1, Instance),
            V::input("supply_d", 0.015, 0.008, 0.05, Type),
            V::input("waste_d", 0.04, 0.025, 0.08, Type),
            V::number("wfu", 2.0, 0.0, 10.0, Type),
            V::number("hwfu", 2.0, 0.0, 10.0, Type),
            V::number("waste_flow", 0.0005, 0.00001, 0.01, Type),
            V::material("finish", "Basin finish", Type),
            V::plane("x0", X, "-width / 2"),
            V::plane("x1", X, "width / 2"),
            V::plane("y0", Y, "-length / 2"),
            V::plane("y1", Y, "length / 2"),
            V::plane("z0", Z, "0"),
            V::plane("z1", Z, "height"),
        ],
        solids: vec![SolidDef::new("basin", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("finish")],
        checks: vec![
            chk("the waste fits the basin", "waste_d < width"),
            chk("the basin stands off the floor", "height > 0"),
        ],
        connectors: vec![
            ConnectorDef::new("supply", Pipe, DomesticHot, In, ("0", "y0", "height / 2"), MinusY, ShapeDef::Round("supply_d".into())),
            ConnectorDef::new("waste", Pipe, Sanitary, Out, ("0", "y1", "0"), PlusY, ShapeDef::Round("waste_d".into()))
                .flow("waste_flow"),
        ],
        plan: vec![
            PlanDef::line(("x0", "y0"), ("x1", "y0")),
            PlanDef::line(("x1", "y0"), ("x1", "y1")),
            PlanDef::line(("x1", "y1"), ("x0", "y1")),
            PlanDef::line(("x0", "y1"), ("x0", "y0")),
        ],
        category: Category::SanitaryTerminal,
        host: Some(Host::Wall),
        ..Default::default()
    }
}

/// Pendent sprinkler head, standard response. Origin: the deflector on the ceiling plane; the frame rises above
/// it into the branch pipe. The one connector is the branch pipe above. Sizes and flows are illustrative
/// defaults to be replaced by the manufacturer data.
pub fn sprinkler_head() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "Sprinkler head, pendent".into(),
        code: "FAM-0013".into(),
        classification: vec![("Uniclass".into(), "Pr_70_55_84".into())],
        values: vec![
            V::input("frame_h", 0.06, 0.02, 0.15, Instance),
            V::input("deflector_d", 0.04, 0.02, 0.1, Type),
            V::input("pipe_d", 0.02, 0.01, 0.08, Type),
            V::number("k_factor", 80.0, 50.0, 360.0, Type),
            V::number("design_area", 12.0, 6.0, 40.0, Type),
            V::number("flow", 0.00013, 0.00001, 0.001, Type),
            V::material("frame", "Sprinkler frame", Type),
            V::plane("x0", X, "-deflector_d / 2"),
            V::plane("x1", X, "deflector_d / 2"),
            V::plane("y0", Y, "-deflector_d / 2"),
            V::plane("y1", Y, "deflector_d / 2"),
            V::plane("z0", Z, "0"),
            V::plane("z1", Z, "frame_h"),
            V::plane("zd", Z, "10 mm"),
            V::plane("px0", X, "-pipe_d / 2"),
            V::plane("px1", X, "pipe_d / 2"),
            V::plane("py0", Y, "-pipe_d / 2"),
            V::plane("py1", Y, "pipe_d / 2"),
        ],
        solids: vec![
            SolidDef::new("frame", ("px0", "px1"), ("py0", "py1"), ("z0", "z1")).material("frame"),
            SolidDef::new("deflector", ("x0", "x1"), ("y0", "y1"), ("z0", "zd")).material("frame"),
        ],
        checks: vec![
            chk("the frame is narrower than the deflector", "pipe_d < deflector_d"),
            chk("the frame hangs below the pipe", "frame_h > 0"),
        ],
        connectors: vec![
            ConnectorDef::new("pipe", Pipe, FireProtection, In, ("0", "0", "z1"), PlusZ, ShapeDef::Round("pipe_d".into())),
        ],
        plan: vec![PlanDef::circle(("0", "0"), "deflector_d / 2")],
        category: Category::Sprinkler,
        host: Some(Host::Ceiling),
        ..Default::default()
    }
}

/// Recessed luminaire, 2x36 W, with a prismatic diffuser. Origin: centre of the diffuser on the ceiling plane.
/// The one connector is the power supply. Sizes and flows are illustrative defaults to be replaced by the
/// manufacturer data.
pub fn luminaire() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "Luminaire, recessed".into(),
        code: "FAM-0014".into(),
        classification: vec![("Uniclass".into(), "Pr_70_70_48".into())],
        values: vec![
            V::input("length", 0.6, 0.3, 2.4, Instance),
            V::input("width", 0.15, 0.1, 0.6, Instance),
            V::input("height", 0.1, 0.05, 0.4, Type),
            V::number("voltage", 230.0, 12.0, 1000.0, Type),
            V::number("power", 72.0, 1.0, 1000.0, Type),
            V::material("housing", "Luminaire housing", Type),
            V::plane("x0", X, "-length / 2"),
            V::plane("x1", X, "length / 2"),
            V::plane("y0", Y, "-width / 2"),
            V::plane("y1", Y, "width / 2"),
            V::plane("z0", Z, "0"),
            V::plane("z1", Z, "height"),
        ],
        solids: vec![SolidDef::new("housing", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("housing")],
        checks: vec![chk("the diffuser is wider than it is deep", "length > width")],
        connectors: vec![
            ConnectorDef::new(
                "power",
                Electrical,
                Power,
                In,
                ("0", "0", "z1"),
                PlusZ,
                ShapeDef::Electrical {
                    voltage: "voltage".into(),
                    poles: "2".into(),
                },
            )
            .flow("power"),
        ],
        plan: vec![
            PlanDef::line(("x0", "y0"), ("x1", "y0")),
            PlanDef::line(("x1", "y0"), ("x1", "y1")),
            PlanDef::line(("x1", "y1"), ("x0", "y1")),
            PlanDef::line(("x0", "y1"), ("x0", "y0")),
        ],
        lighting: Some(LightingDef {
            flux_lm: Some(6400.0),
            power_w: Some(72.0),
            cct_k: Some(4000.0),
            cri: Some(80.0),
            mounting: Mounting::Recessed,
            ..Default::default()
        }),
        category: Category::LightFixture,
        host: Some(Host::Ceiling),
        ..Default::default()
    }
}

/// Distribution board, surface mounted, 12-way. Origin: centre of the board on the wall face. The supply enters
/// at the top (+Z) and the circuits leave at the bottom (-Z). Sizes and flows are illustrative defaults to be
/// replaced by the manufacturer data.
pub fn panel_board() -> Family {
    use Dir::*;
    use Domain::*;
    use FlowDir::*;
    use System::*;
    Family {
        name: "Distribution board, 12-way".into(),
        code: "FAM-0015".into(),
        classification: vec![("Uniclass".into(), "Pr_60_70_22".into())],
        values: vec![
            V::input("height", 0.8, 0.4, 2.0, Instance),
            V::input("width", 0.5, 0.3, 1.5, Instance),
            V::input("depth", 0.15, 0.08, 0.4, Type),
            V::number("voltage", 400.0, 12.0, 1000.0, Type),
            V::number("current", 100.0, 1.0, 630.0, Type),
            V::material("enclosure", "Enclosure", Type),
            V::plane("x0", X, "-width / 2"),
            V::plane("x1", X, "width / 2"),
            V::plane("y0", Y, "-depth / 2"),
            V::plane("y1", Y, "depth / 2"),
            V::plane("z0", Z, "0"),
            V::plane("z1", Z, "height"),
        ],
        solids: vec![SolidDef::new("enclosure", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("enclosure")],
        checks: vec![chk("the board is taller than it is wide", "height > width")],
        connectors: vec![
            ConnectorDef::new(
                "supply",
                Electrical,
                Power,
                In,
                ("0", "0", "z1"),
                PlusZ,
                ShapeDef::Electrical {
                    voltage: "voltage".into(),
                    poles: "4".into(),
                },
            )
            .flow("current"),
            ConnectorDef::new(
                "circuits",
                Electrical,
                Power,
                Out,
                ("0", "0", "z0"),
                MinusZ,
                ShapeDef::Electrical {
                    voltage: "voltage".into(),
                    poles: "4".into(),
                },
            )
            .flow("current"),
        ],
        plan: vec![
            PlanDef::line(("x0", "y0"), ("x1", "y0")),
            PlanDef::line(("x1", "y0"), ("x1", "y1")),
            PlanDef::line(("x1", "y1"), ("x0", "y1")),
            PlanDef::line(("x0", "y1"), ("x0", "y0")),
        ],
        category: Category::DistributionBoard,
        host: Some(Host::Wall),
        ..Default::default()
    }
}

/// Clevis hanger with a pipe clamp, for a pipe or duct run. Origin: the point on the host face where the rod
/// meets it; the rod hangs below and the clamp sits at the bottom. No connector: the run is placed against it.
/// Sizes are illustrative defaults to be replaced by the manufacturer data.
pub fn hanger() -> Family {
    Family {
        name: "Pipe hanger, clevis".into(),
        code: "FAM-0016".into(),
        classification: vec![("Uniclass".into(), "Pr_20_85_47".into())],
        values: vec![
            V::input("rod_h", 0.5, 0.1, 3.0, Instance),
            V::input("rod_d", 0.012, 0.006, 0.05, Type),
            V::input("pipe_d", 0.15, 0.025, 0.6, Type),
            V::material("steel", "Hanger steel", Type),
            V::plane("x0", X, "-pipe_d / 2"),
            V::plane("x1", X, "pipe_d / 2"),
            V::plane("y0", Y, "-pipe_d / 2"),
            V::plane("y1", Y, "pipe_d / 2"),
            V::plane("z0", Z, "-rod_h"),
            V::plane("z1", Z, "0"),
            V::plane("zc", Z, "z0 + 20 mm"),
            V::plane("rx0", X, "-rod_d / 2"),
            V::plane("rx1", X, "rod_d / 2"),
            V::plane("ry0", Y, "-rod_d / 2"),
            V::plane("ry1", Y, "rod_d / 2"),
        ],
        solids: vec![
            SolidDef::new("rod", ("rx0", "rx1"), ("ry0", "ry1"), ("z0", "z1")).material("steel"),
            SolidDef::new("clamp", ("x0", "x1"), ("y0", "y1"), ("z0", "zc")).material("steel"),
        ],
        checks: vec![
            chk("the clamp is narrower than the pipe", "rod_d < pipe_d"),
            chk("the rod hangs below the host", "rod_h > 0"),
        ],
        plan: vec![PlanDef::circle(("0", "0"), "pipe_d / 2")],
        category: Category::Generic,
        host: Some(Host::Face),
        ..Default::default()
    }
}
