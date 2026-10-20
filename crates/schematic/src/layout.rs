//! The grid layout engine of doc 06 (SCH-002): a pure function from the extracted schematic
//! data to symbol slots and wire routes. No I/O, no rendering, no model access — the same input
//! always gives the same layout.
//!
//! Coordinates are metres of sheet space (1 mm on the sheet is 0.001); rows grow downward from
//! y = 0. Ordering keys are canonical quantities (m3/s, A): display units never reach this crate.
//! See 06 "Grid layout" for the reviewed rules this implements.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Default distance between two level rows, in metres of sheet space (50 mm).
pub const DEFAULT_ROW_PITCH: f64 = 0.05;
/// Default distance between two columns (uniform mode) or the gutter between slots
/// (footprint-proportional mode), in metres of sheet space (40 mm).
pub const DEFAULT_COLUMN_PITCH: f64 = 0.04;
/// Smallest slot in footprint-proportional mode, in metres of sheet space (10 mm).
pub const DEFAULT_MIN_SLOT: f64 = 0.01;
/// A layout pass repeats until the order stops changing, capped at this many passes (06).
pub const MAX_PASSES: usize = 8;

/// How the columns of a level row are spaced (06, decision 3: the designer selects per generation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpacingMode {
    /// Every column has the same pitch: `column_pitch`.
    #[default]
    Uniform,
    /// A column is as wide as its element's footprint (at least `min_slot`), plus `column_pitch` of gutter.
    FootprintProportional,
}

/// One level: a row of the grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutLevel {
    pub id: u64,
    pub name: String,
    pub elevation_m: f64,
}

/// One element to place: a node or a group of the extracted draft. `key` is the canonical ordering
/// quantity (largest first); `footprint` is the element's width in metres of sheet space, used by
/// footprint-proportional spacing only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutElement {
    pub id: u64,
    pub level: u64,
    pub key: f64,
    pub footprint: f64,
}

/// An undirected connection between two elements. Deduplicated per pair; only connections between
/// elements of the same level row count for the barycenter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub a: u64,
    pub b: u64,
}

/// The input of the layout: levels, elements, connections, and the spacing parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutInput {
    pub levels: Vec<LayoutLevel>,
    pub elements: Vec<LayoutElement>,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub mode: SpacingMode,
    #[serde(default = "default_row_pitch")]
    pub row_pitch: f64,
    #[serde(default = "default_column_pitch")]
    pub column_pitch: f64,
    #[serde(default = "default_min_slot")]
    pub min_slot: f64,
}

fn default_row_pitch() -> f64 {
    DEFAULT_ROW_PITCH
}
fn default_column_pitch() -> f64 {
    DEFAULT_COLUMN_PITCH
}
fn default_min_slot() -> f64 {
    DEFAULT_MIN_SLOT
}

impl LayoutInput {
    pub fn new(
        levels: Vec<LayoutLevel>,
        elements: Vec<LayoutElement>,
        connections: Vec<Connection>,
        mode: SpacingMode,
    ) -> Self {
        Self {
            levels,
            elements,
            connections,
            mode,
            row_pitch: DEFAULT_ROW_PITCH,
            column_pitch: DEFAULT_COLUMN_PITCH,
            min_slot: DEFAULT_MIN_SLOT,
        }
    }
}

/// Where an element is drawn: its slot's lower-left corner (`x`, `y`), its width, and its row and
/// column. The symbol is drawn centered on the slot.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    pub element: u64,
    pub level: u64,
    pub row: usize,
    pub column: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
}

/// A wire between two elements: the intermediate via-points; the two endpoints are the symbols.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireRoute {
    pub a: u64,
    pub b: u64,
    pub via: Vec<[f64; 2]>,
}

/// The layout: the slots, the wires, and the connections and elements that could not be placed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub slots: Vec<Slot>,
    pub wires: Vec<WireRoute>,
    /// connections naming an element that is not in the layout
    pub unresolved_connections: Vec<Connection>,
    /// elements whose level is not in the input, or whose key is not a finite number
    pub skipped_elements: usize,
}

/// The total column order of one level's elements (06): the ordering key descending, then
/// connected elements before disconnected ones, then the barycenter ascending, then the stable id
/// ascending. The barycenter passes start from the key order and repeat until the order stops
/// changing, capped at [`MAX_PASSES`].
fn total_order<'a>(els: &[&'a LayoutElement], pairs: &[(u64, u64)]) -> Vec<&'a LayoutElement> {
    let neighbors = |id: u64| -> Vec<u64> {
        let mut out: Vec<u64> = pairs
            .iter()
            .filter(|&&(a, b)| a == id || b == id)
            .map(|&(a, b)| if a == id { b } else { a })
            .filter(|&n| els.iter().any(|e| e.id == n))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    };
    // the initial order: the ordering key descending, then the stable id ascending
    let mut order: Vec<&LayoutElement> = els.to_vec();
    order.sort_by(|a, b| b.key.total_cmp(&a.key).then(a.id.cmp(&b.id)));
    for _ in 0..MAX_PASSES {
        let rank: BTreeMap<u64, usize> = order.iter().enumerate().map(|(i, e)| (e.id, i)).collect();
        let mut bary: BTreeMap<u64, (bool, f64)> = BTreeMap::new();
        for e in &order {
            let ns = neighbors(e.id);
            let v = if ns.is_empty() {
                (false, f64::INFINITY)
            } else {
                (true, ns.iter().map(|&n| rank[&n] as f64).sum::<f64>() / ns.len() as f64)
            };
            bary.insert(e.id, v);
        }
        let mut next = order.clone();
        next.sort_by(|a, b| {
            b.key
                .total_cmp(&a.key)
                .then(bary[&b.id].0.cmp(&bary[&a.id].0)) // connected (true) before disconnected (false)
                .then(bary[&a.id].1.total_cmp(&bary[&b.id].1))
                .then(a.id.cmp(&b.id))
        });
        if next == order {
            break;
        }
        order = next;
    }
    order
}

/// The layout: rows are the levels sorted by elevation, then name, then id; columns are the
/// elements of the row in their total order; wires follow the connections (straight within a row,
/// an elbow across rows).
pub fn layout(input: &LayoutInput) -> Layout {
    let mut rows: Vec<&LayoutLevel> = input.levels.iter().collect();
    rows.sort_by(|a, b| {
        a.elevation_m
            .total_cmp(&b.elevation_m)
            .then(a.name.cmp(&b.name))
            .then(a.id.cmp(&b.id))
    });

    // connections: undirected, deduplicated per pair, self-connections dropped
    let mut pairs: Vec<(u64, u64)> = input
        .connections
        .iter()
        .filter(|c| c.a != c.b)
        .map(|c| (c.a.min(c.b), c.a.max(c.b)))
        .collect();
    pairs.sort_unstable();
    pairs.dedup();

    let skipped_elements = input
        .elements
        .iter()
        .filter(|e| rows.iter().all(|l| l.id != e.level) || !e.key.is_finite())
        .count();
    let mut layout = Layout { skipped_elements, ..Layout::default() };

    let mut slot_of: BTreeMap<u64, Slot> = BTreeMap::new();
    for (row, level) in rows.iter().enumerate() {
        let els: Vec<&LayoutElement> = input
            .elements
            .iter()
            .filter(|e| e.level == level.id && e.key.is_finite())
            .collect();
        if els.is_empty() {
            continue;
        }
        let order = total_order(&els, &pairs);
        let mut x = 0.0;
        let y = row as f64 * input.row_pitch;
        for (column, el) in order.iter().enumerate() {
            let width = match input.mode {
                SpacingMode::Uniform => input.column_pitch,
                SpacingMode::FootprintProportional => el.footprint.max(input.min_slot),
            };
            let width = if width.is_finite() && width > 0.0 { width } else { input.min_slot };
            let slot = Slot { element: el.id, level: level.id, row, column, x, y, width };
            slot_of.insert(el.id, slot);
            layout.slots.push(slot);
            x += width
                + if input.mode == SpacingMode::FootprintProportional {
                    input.column_pitch
                } else {
                    0.0
                };
        }
    }

    for &(a, b) in &pairs {
        let (Some(sa), Some(sb)) = (slot_of.get(&a), slot_of.get(&b)) else {
            layout.unresolved_connections.push(Connection { a, b });
            continue;
        };
        let anchor = |s: &Slot| [s.x + s.width / 2.0, s.y + input.row_pitch / 2.0];
        let (pa, pb) = (anchor(sa), anchor(sb));
        let via = if (pa[1] - pb[1]).abs() < 1e-12 {
            Vec::new()
        } else {
            let mid_x = (pa[0] + pb[0]) / 2.0;
            vec![[mid_x, pa[1]], [mid_x, pb[1]]]
        };
        layout.wires.push(WireRoute { a, b, via });
    }
    layout
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (LayoutInput, serde_json::Value) {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/revit-style-ui/fixtures/schematic-layout-v1.json"
        );
        let text = std::fs::read_to_string(path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let input: LayoutInput = serde_json::from_value(v["input"].clone()).unwrap();
        (input, v)
    }

    /// Positions in metres, absolute tolerance 1e-9 (06); orders and ids are exact.
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() <= 1e-9, "{a} != {b}");
    }

    fn level_order(out: &Layout, level: u64) -> Vec<u64> {
        out.slots.iter().filter(|s| s.level == level).map(|s| s.element).collect()
    }

    #[test]
    fn golden_uniform_from_fixture() {
        let (input, v) = fixture();
        let out = layout(&input);
        // rows: level 1 -> row 0; the elevation tie at 3.0 is broken by name (Atrium < Level 2)
        for r in v["expected"]["rows"].as_array().unwrap() {
            let level = r["level"].as_u64().unwrap();
            let row = r["row"].as_u64().unwrap() as usize;
            assert_eq!(out.slots.iter().find(|s| s.level == level).unwrap().row, row);
        }
        // orders, exact
        for o in v["expected"]["order"].as_array().unwrap() {
            let level = o["level"].as_u64().unwrap();
            let ids: Vec<u64> = o["elements"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap()).collect();
            assert_eq!(level_order(&out, level), ids);
        }
        // inline literals: level 1's key tie is resolved by the barycenter passes capped at 8
        // (the order oscillates between two states and lands on this one), and the disconnected
        // element 13 follows the connected ones at the same key
        assert_eq!(level_order(&out, 1), vec![10, 11, 13, 12]);
        assert_eq!(level_order(&out, 2), vec![20, 21]);
        // slots at 1e-9
        for s in v["expected"]["slots_uniform"].as_array().unwrap() {
            let slot = out.slots.iter().find(|x| x.element == s["element"].as_u64().unwrap()).unwrap();
            close(slot.x, s["x"].as_f64().unwrap());
            close(slot.y, s["y"].as_f64().unwrap());
        }
        // wires: two straight (same row), one elbow (cross-level), one unresolved
        assert_eq!(out.wires.len(), 3);
        assert_eq!(out.unresolved_connections, vec![Connection { a: 10, b: 99 }]);
        for w in v["expected"]["wires_uniform"].as_array().unwrap() {
            let got = out
                .wires
                .iter()
                .find(|x| x.a == w["a"].as_u64().unwrap() && x.b == w["b"].as_u64().unwrap())
                .unwrap();
            let via = w["via"].as_array().unwrap();
            assert_eq!(got.via.len(), via.len());
            for (i, p) in via.iter().enumerate() {
                close(got.via[i][0], p[0].as_f64().unwrap());
                close(got.via[i][1], p[1].as_f64().unwrap());
            }
        }
        // inline literals: the cross-level elbow via-points
        let cross = out.wires.iter().find(|w| w.a == 10 && w.b == 20).unwrap();
        assert_eq!(cross.via.len(), 2);
        close(cross.via[0][0], 0.02);
        close(cross.via[0][1], 0.025);
        close(cross.via[1][0], 0.02);
        close(cross.via[1][1], 0.125);
    }

    #[test]
    fn golden_footprint_from_fixture() {
        let (mut input, v) = fixture();
        input.mode = SpacingMode::FootprintProportional;
        let out = layout(&input);
        for s in v["expected"]["slots_footprint"].as_array().unwrap() {
            let slot = out.slots.iter().find(|x| x.element == s["element"].as_u64().unwrap()).unwrap();
            close(slot.x, s["x"].as_f64().unwrap());
            close(slot.y, s["y"].as_f64().unwrap());
        }
        // the spacing mode never changes the order
        assert_eq!(level_order(&out, 1), vec![10, 11, 13, 12]);
        // inline literal: 11 sits one slot (0.02) and one gutter (0.04) after 10
        let s10 = out.slots.iter().find(|s| s.element == 10).unwrap();
        let s11 = out.slots.iter().find(|s| s.element == 11).unwrap();
        close(s11.x - s10.x, 0.06);
        // 12's slot is as wide as its footprint (0.03, above the 0.01 minimum)
        let s12 = out.slots.iter().find(|s| s.element == 12).unwrap();
        close(s12.width, 0.03);
    }

    #[test]
    fn unknown_level_and_non_finite_key_are_skipped() {
        let input = LayoutInput::new(
            vec![LayoutLevel { id: 1, name: "L1".into(), elevation_m: 0.0 }],
            vec![
                LayoutElement { id: 1, level: 1, key: 1.0, footprint: 0.02 },
                LayoutElement { id: 2, level: 99, key: 1.0, footprint: 0.02 },
                LayoutElement { id: 3, level: 1, key: f64::NAN, footprint: 0.02 },
            ],
            vec![],
            SpacingMode::Uniform,
        );
        let out = layout(&input);
        assert_eq!(out.slots.len(), 1);
        assert_eq!(out.skipped_elements, 2);
    }

    #[test]
    fn duplicate_and_self_connections_count_once() {
        let input = LayoutInput::new(
            vec![LayoutLevel { id: 1, name: "L1".into(), elevation_m: 0.0 }],
            vec![
                LayoutElement { id: 1, level: 1, key: 1.0, footprint: 0.02 },
                LayoutElement { id: 2, level: 1, key: 1.0, footprint: 0.02 },
            ],
            vec![
                Connection { a: 1, b: 2 },
                Connection { a: 2, b: 1 },
                Connection { a: 1, b: 1 },
            ],
            SpacingMode::Uniform,
        );
        let out = layout(&input);
        assert_eq!(out.wires.len(), 1);
        assert_eq!(out.wires[0].via.len(), 0); // same row: straight
        assert!(out.unresolved_connections.is_empty());
    }
}
