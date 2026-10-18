//! Revit-style ribbon: tabs, groups of large icon buttons with the group title underneath.
//! Commands are identified by stable IDs (also the icon IDs); labels are display text only.

use crate::icons;
use eframe::egui::{self, Align, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

pub struct Cmd {
    pub id: &'static str,
    pub label: &'static str,
}

pub struct Group {
    pub title: &'static str,
    pub commands: &'static [Cmd],
}

const fn c(id: &'static str, label: &'static str) -> Cmd {
    Cmd { id, label }
}

/// Ribbon tabs with the icon shown beside each name.
pub const TABS: &[(&str, &str)] = &[
    ("Architecture", "wall"),
    ("Structure", "beam"),
    ("Steel", "structural_section"),
    ("Precast", "slab"),
    ("Systems", "duct"),
    ("Insert", "link_model"),
    ("Annotate", "dimension"),
    ("Analyze", "interference_check"),
    ("Massing & Site", "component"),
    ("Collaborate", "workset"),
    ("View", "view"),
    ("Manage", "settings"),
    ("Add-Ins", "new"),
    ("Modify", "select"),
];

pub const SYSTEMS_SUBTABS: &[(&str, &str)] = &[
    ("HVAC", "duct"),
    ("Plumbing & Piping", "pipe"),
    ("Electrical", "cable_tray"),
    ("All", "component"),
];

pub const QUICK_ACCESS: &[(&str, &str)] = &[
    ("new", "New"),
    ("open", "Open"),
    ("save", "Save"),
    ("undo", "Undo"),
    ("redo", "Redo"),
    ("measure", "Measure"),
    ("settings", "Settings"),
];

const HVAC: &[Group] = &[
    Group { title: "HVAC", commands: &[
        c("duct", "Duct"), c("duct_fitting", "Duct Fitting"), c("duct_elbow", "Duct Elbow"),
        c("duct_transition", "Transition"), c("flex_duct", "Flex Duct"), c("duct_insulation", "Insulation"),
        c("damper", "Damper"),
    ]},
    Group { title: "Air Terminals", commands: &[
        c("air_terminal", "Air Terminal"), c("diffuser", "Diffuser"), c("grille", "Grille"),
    ]},
    Group { title: "Analysis", commands: &[c("end_point", "End Point"), c("compute", "Compute")] },
    Group { title: "Mechanical Equipment", commands: &[
        c("ahu", "AHU"), c("fan", "Fan"), c("vav_box", "VAV Box"),
        c("mechanical_equipment", "Equipment"), c("chiller", "Chiller"), c("cooling_tower", "Cooling Tower"),
    ]},
];

const PLUMBING: &[Group] = &[
    Group { title: "Plumbing & Piping", commands: &[
        c("pipe", "Pipe"), c("pipe_fitting", "Pipe Fitting"), c("pipe_elbow", "Elbow"), c("pipe_tee", "Tee"),
        c("pipe_transition", "Transition"), c("pipe_insulation", "Insulation"),
    ]},
    Group { title: "Valves & Accessories", commands: &[
        c("valve_gate", "Gate Valve"), c("valve_ball", "Ball Valve"), c("valve_butterfly", "Butterfly Valve"),
        c("backflow_preventer", "Backflow Preventer"), c("hanger", "Hanger"), c("pipe_support", "Support"),
    ]},
    Group { title: "Fixtures & Equipment", commands: &[
        c("plumbing_fixture", "Plumbing Fixture"), c("sprinkler", "Sprinkler"), c("drain", "Drain"),
        c("cleanout", "Cleanout"), c("pump", "Pump"),
    ]},
    Group { title: "Analysis", commands: &[c("end_point", "End Point"), c("compute", "Compute")] },
];

const ELECTRICAL: &[Group] = &[
    Group { title: "Containment", commands: &[
        c("cable_tray", "Cable Tray"), c("cable_tray_fitting", "Tray Fitting"), c("conduit", "Conduit"),
        c("conduit_fitting", "Conduit Fitting"), c("raceway", "Raceway"),
        c("junction_box", "Junction Box"), c("pull_box", "Pull Box"),
    ]},
    Group { title: "Lighting & Devices", commands: &[
        c("lighting_fixture", "Lighting Fixture"), c("light_pendant", "Pendant"), c("light_surface", "Surface Light"),
        c("switch", "Switch"), c("socket", "Socket"), c("device", "Device"),
    ]},
    Group { title: "Electrical Equipment", commands: &[
        c("panel", "Panel Board"), c("transformer", "Transformer"), c("generator", "Generator"),
        c("disconnect", "Disconnect"),
    ]},
    Group { title: "Power Distribution", commands: &[c("elec_node", "Electrical Node"), c("elec_cable", "Cable"), c("elec_compute", "Compute")] },
];

const ARCHITECTURE: &[Group] = &[
    Group { title: "Build", commands: &[
        c("wall", "Wall"), c("door", "Door"), c("window", "Window"), c("component", "Component"),
        c("column", "Column"), c("roof", "Roof"), c("ceiling", "Ceiling"), c("floor", "Floor"),
        c("curtain_wall", "Curtain Wall"),
    ]},
    Group { title: "Circulation", commands: &[c("stair", "Stair"), c("railing", "Railing"), c("ramp", "Ramp")] },
    Group { title: "Model", commands: &[c("model_line", "Model Line")] },
    Group { title: "Room & Area", commands: &[c("room", "Room"), c("area", "Area")] },
    Group { title: "Opening", commands: &[c("opening_face", "By Face"), c("shaft", "Shaft"), c("opening_wall", "Wall"), c("opening_vertical", "Vertical"), c("opening_dormer", "Dormer")] },
    Group { title: "Datum", commands: &[c("level", "Level"), c("grid", "Grid")] },
];

const STRUCTURE: &[Group] = &[
    Group { title: "Structure", commands: &[
        c("beam", "Beam"), c("beam_system", "Beam System"), c("structural_wall", "Wall"), c("structural_column", "Column"),
        c("structural_floor", "Floor"), c("truss", "Truss"), c("brace", "Brace"), c("brace_frame", "Brace Frame"),
        c("framing", "Framing"),
    ]},
    Group { title: "Foundation", commands: &[c("foundation", "Isolated"), c("footing", "Footing"), c("slab", "Slab")] },
    Group { title: "Reinforcement", commands: &[c("rebar", "Rebar"), c("shear_wall", "Shear Wall")] },
    Group { title: "Connection", commands: &[
        c("connection", "Connection"), c("anchor", "Anchor"), c("anchor_cast_in", "Cast-in Anchor"),
    ]},
];

const STEEL: &[Group] = &[Group { title: "Steel", commands: &[
    c("structural_section", "Section"), c("connection", "Connection"), c("beam", "Beam"), c("brace", "Brace"),
]}];
const PRECAST: &[Group] = &[Group { title: "Precast", commands: &[
    c("slab", "Slab"), c("structural_wall", "Wall"), c("foundation", "Foundation"),
]}];
const INSERT: &[Group] = &[Group { title: "Link", commands: &[
    c("link_model", "Link Model"), c("ifc_import", "Import IFC"), c("ifc_export", "Export IFC"),
]}];
const ANNOTATE: &[Group] = &[
    Group { title: "Dimension", commands: &[c("dimension", "Aligned")] },
    Group { title: "Detail", commands: &[c("detail", "Detail")] },
    Group { title: "Tag", commands: &[c("tag", "Tag")] },
    Group { title: "Revision", commands: &[c("revision_cloud", "Revision Cloud")] },
];
const ANALYZE: &[Group] = &[
    Group { title: "Systems", commands: &[c("end_point", "End Point"), c("compute", "Compute")] },
    Group { title: "Coordination", commands: &[c("clash", "Clash Detection"), c("interference_check", "Interference Check")] },
];
const MASSING: &[Group] = &[Group { title: "Model Site", commands: &[c("component", "Site Component")] }];
const COLLABORATE: &[Group] = &[
    Group { title: "Manage Collaboration", commands: &[c("workset", "Worksets"), c("phase", "Phases")] },
    Group { title: "Coordinate", commands: &[c("link_model", "Link Model"), c("clash", "Clash Detection")] },
];
const VIEW: &[Group] = &[
    Group { title: "Create", commands: &[
        c("view", "View"), c("section", "Section"), c("elevation", "Elevation"), c("level", "Level"),
    ]},
    Group { title: "Navigate", commands: &[
        c("zoom_in", "Zoom In"), c("zoom_out", "Zoom Out"), c("pan", "Pan"), c("orbit", "Orbit"), c("search", "Find"),
    ]},
];
const MANAGE: &[Group] = &[
    Group { title: "Settings", commands: &[c("settings", "Settings"), c("project_parameters", "Types")] },
    Group { title: "Automation", commands: &[c("automation_editor", "Graph Editor"), c("automation_player", "Player")] },
    Group { title: "AI Integration", commands: &[c("automation_mcp", "MCP Setup"), c("automation_snapshot", "Export Snapshot")] },
];
const ADDINS: &[Group] = &[Group { title: "Automation", commands: &[c("automation_editor", "Graph Editor"), c("automation_player", "Player")] }];
const MODIFY: &[Group] = &[
    Group { title: "Select", commands: &[c("select", "Modify")] },
    Group { title: "Clipboard", commands: &[c("copy", "Copy")] },
    Group { title: "Modify", commands: &[
        c("move", "Move"), c("rotate", "Rotate"), c("mirror", "Mirror"), c("align", "Align"),
        c("offset", "Offset"), c("trim", "Trim/Extend"), c("delete", "Delete"),
    ]},
    Group { title: "Geometry", commands: &[c("wall_joins", "Wall Joins")] },
    Group { title: "Measure", commands: &[c("measure", "Measure")] },
];

pub fn groups(tab: &str, systems_sub: &str) -> Vec<&'static Group> {
    let list: &[&'static [Group]] = match tab {
        "Architecture" => &[ARCHITECTURE],
        "Structure" => &[STRUCTURE],
        "Steel" => &[STEEL],
        "Precast" => &[PRECAST],
        "Systems" => match systems_sub {
            "HVAC" => &[HVAC],
            "Plumbing & Piping" => &[PLUMBING],
            "Electrical" => &[ELECTRICAL],
            _ => &[HVAC, PLUMBING, ELECTRICAL],
        },
        "Insert" => &[INSERT],
        "Annotate" => &[ANNOTATE],
        "Analyze" => &[ANALYZE],
        "Massing & Site" => &[MASSING],
        "Collaborate" => &[COLLABORATE],
        "View" => &[VIEW],
        "Manage" => &[MANAGE],
        "Add-Ins" => &[ADDINS],
        _ => &[MODIFY],
    };
    list.iter().flat_map(|g| g.iter()).collect()
}

/// The icon for a command ID; unknown IDs fall back to the generic component icon.
pub fn icon_image(ui: &egui::Ui, id: &str, dark: bool, size: Vec2) -> egui::Image<'static> {
    if id.starts_with("automation_") { return crate::automation_icons::image(ui, id, dark, size); }
    let icon = icons::find(id).or_else(|| icons::find("component")).expect("component icon is embedded");
    let (bytes, theme) = match (dark, icon.dark) {
        (true, Some(d)) => (d, "dark"),
        _ => (icon.light, "light"),
    };
    // Prepare at physical display resolution instead of asking the GPU to
    // shrink a detailed 128 px illustration to a tiny toolbar in one step.
    let pixels = (size * ui.ctx().pixels_per_point()).round();
    let width = (pixels.x as u32).clamp(1, 512);
    let height = (pixels.y as u32).clamp(1, 512);
    let key = egui::Id::new(("filtered-icon-v1", icon.id, theme, width, height));
    let cached = ui.ctx().data_mut(|data| data.get_temp::<egui::TextureHandle>(key));
    let texture = cached.unwrap_or_else(|| {
        let rgba = prepare_icon(bytes, width, height);
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize], rgba.as_raw());
        let texture = ui.ctx().load_texture(
            format!("icon/{theme}/{}/{width}x{height}", icon.id), color,
            egui::TextureOptions::LINEAR);
        ui.ctx().data_mut(|data| data.insert_temp(key, texture.clone()));
        texture
    });
    egui::Image::new((texture.id(), size)).fit_to_exact_size(size)
}

/// Filter premultiplied pixels, so transparent borders cannot bleed into the
/// illustration. Source artwork and its proportions remain unchanged.
fn prepare_icon(bytes: &[u8], width: u32, height: u32) -> image::RgbaImage {
    let source = image::load_from_memory(bytes).expect("embedded PNG icon").to_rgba8();
    let mut premultiplied = image::Rgba32FImage::new(source.width(), source.height());
    for (dst, src) in premultiplied.pixels_mut().zip(source.pixels()) {
        let alpha = f32::from(src[3]) / 255.0;
        *dst = image::Rgba([
            f32::from(src[0]) / 255.0 * alpha,
            f32::from(src[1]) / 255.0 * alpha,
            f32::from(src[2]) / 255.0 * alpha, alpha,
        ]);
    }
    let scale = (width as f32 / source.width() as f32)
        .min(height as f32 / source.height() as f32);
    let w = ((source.width() as f32 * scale).round() as u32).clamp(1, width);
    let h = ((source.height() as f32 * scale).round() as u32).clamp(1, height);
    let filtered = image::imageops::resize(&premultiplied, w, h,
        image::imageops::FilterType::Lanczos3);
    let mut output = image::RgbaImage::new(width, height);
    let left = (width - w) / 2;
    let top = (height - h) / 2;
    for (x, y, pixel) in filtered.enumerate_pixels() {
        let alpha = pixel[3].clamp(0.0, 1.0);
        let mut rgba = [0; 4];
        if alpha > 1.0 / 255.0 {
            for channel in 0..3 {
                rgba[channel] = ((pixel[channel] / alpha).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            rgba[3] = (alpha * 255.0).round() as u8;
        }
        output.put_pixel(left + x, top + y, image::Rgba(rgba));
    }
    output
}

/// A ribbon panel: the commands in a row with the title centred underneath, then a divider.
pub fn panel(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
        let inner = ui.horizontal(|ui| add(ui));
        let w = inner.response.rect.width();
        ui.allocate_ui(Vec2::new(w, 14.0), |ui| {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new(title).size(11.0).color(ui.visuals().weak_text_color()));
            });
        });
    });
    ui.separator();
}

/// Large ribbon button: 32 px icon above a label wrapped on up to two lines.
pub fn big_button(ui: &mut egui::Ui, id: &str, label: &str, dark: bool, active: bool) -> egui::Response {
    let size = Vec2::new(64.0, 66.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        if active || response.hovered() {
            let (fill, stroke) = if active {
                (visuals.selection.bg_fill.gamma_multiply(0.45), visuals.selection.stroke)
            } else {
                (visuals.widgets.hovered.weak_bg_fill, visuals.widgets.hovered.bg_stroke)
            };
            ui.painter().rect(rect, 4.0, fill, Stroke::new(1.0_f32, stroke.color), StrokeKind::Inside);
        }
        let icon_rect = Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 20.0), Vec2::splat(32.0));
        icon_image(ui, id, dark, icon_rect.size()).paint_at(ui, icon_rect);
        let color: Color32 = visuals.text_color();
        let mut job = egui::text::LayoutJob::simple(label.to_owned(), FontId::proportional(11.0), color, size.x - 4.0);
        job.halign = Align::Center;
        job.wrap.max_rows = 2;
        let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
        ui.painter().galley(egui::pos2(rect.center().x, rect.top() + 39.0), galley, color);
    }
    response.on_hover_text(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_pixels_do_not_tint_filtered_edges() {
        // Invisible magenta must not bleed into the white foreground.
        let mut source = image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 255, 0]));
        for y in 4..12 {
            for x in 4..12 {
                source.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(source)
            .write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let filtered = prepare_icon(bytes.get_ref(), 7, 7);
        assert!(filtered.pixels().any(|p| p[3] > 0 && p[3] < 255));
        for pixel in filtered.pixels().filter(|p| p[3] > 0) {
            assert_eq!(pixel[0], pixel[1]);
            assert_eq!(pixel[1], pixel[2]);
        }
    }

    fn png_size(b: &[u8]) -> Option<(u32, u32)> {
        if b.len() < 24 || b[..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] || &b[12..16] != b"IHDR" {
            return None;
        }
        Some((u32::from_be_bytes(b[16..20].try_into().ok()?), u32::from_be_bytes(b[20..24].try_into().ok()?)))
    }

    /// Every ID the ribbon, tab strip, sub-tabs and quick-access bar use must have a real icon, not the fallback.
    #[test]
    fn every_ribbon_id_has_an_icon() {
        let mut ids: Vec<&str> = TABS.iter().map(|t| t.1).collect();
        ids.extend(SYSTEMS_SUBTABS.iter().map(|t| t.1));
        ids.extend(QUICK_ACCESS.iter().map(|t| t.0));
        for (tab, _) in TABS {
            for sub in ["HVAC", "Plumbing & Piping", "Electrical", "All"] {
                for g in groups(tab, sub) {
                    ids.extend(g.commands.iter().map(|c| c.id));
                }
            }
        }
        let missing: Vec<_> = ids.iter().filter(|id| icons::find(id).is_none() && !["automation_editor", "automation_player", "automation_mcp", "automation_snapshot"].contains(id)).collect();
        assert!(missing.is_empty(), "ribbon IDs without an icon: {missing:?}");
    }

    #[test]
    fn embedded_icons_are_valid_png_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for i in icons::ICONS {
            assert!(seen.insert(i.id), "duplicate icon id {}", i.id);
            let (w, h) = png_size(i.light).unwrap_or_else(|| panic!("{} is not a PNG", i.id));
            assert!(w >= 32 && h >= 32, "{} is only {w}x{h}", i.id);
            if let Some(d) = i.dark {
                assert!(png_size(d).is_some(), "{} dark is not a PNG", i.id);
            }
        }
    }
}
