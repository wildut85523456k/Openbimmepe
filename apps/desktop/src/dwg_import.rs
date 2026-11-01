//! DWG/DXF import: decode a DWG (via acadrust) into DXF bytes, parse with the `dxf`
//! crate, and keep the result as a renderable overlay on the plan view.

use std::io::Cursor;

#[derive(Clone, Debug, Default)]
pub struct Pt { pub x: f64, pub y: f64 }

#[derive(Clone, Debug, Default)]
pub struct CadLine { pub a: Pt, pub b: Pt }

#[derive(Clone, Debug, Default)]
pub struct CadCircle { pub c: Pt, pub r: f64 }

#[derive(Clone, Debug, Default)]
pub struct CadArc { pub c: Pt, pub r: f64, pub a0: f64, pub a1: f64 }

#[derive(Clone, Debug, Default)]
pub struct CadPoly { pub pts: Vec<Pt> }

#[derive(Clone, Debug, Default)]
pub struct CadText { pub p: Pt, pub text: String, pub h: f64 }

#[derive(Clone, Debug, Default)]
pub struct CadDrawing {
    pub lines: Vec<CadLine>,
    pub circles: Vec<CadCircle>,
    pub arcs: Vec<CadArc>,
    pub polys: Vec<CadPoly>,
    pub texts: Vec<CadText>,
}

impl CadDrawing {
    pub fn primitive_count(&self) -> usize {
        self.lines.len() + self.circles.len() + self.arcs.len() + self.polys.len() + self.texts.len()
    }
}

/// Decode a DWG file's bytes into binary DXF bytes (acadrust), running on a
/// large-stack thread with catch_unwind so a panic becomes an error.
pub fn decode_dwg(data: Vec<u8>) -> Result<Vec<u8>, String> {
    let handle = std::thread::Builder::new()
        .stack_size(128 << 20)
        .spawn(move || -> Result<Vec<u8>, String> {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                use acadrust::{DwgReadOptions, DwgReader, DxfWriter};
                let options = DwgReadOptions::failsafe()
                    .with_max_decompressed_bytes(1 << 30)
                    .with_mesh_limits(100_000, 400_000);
                let mut reader = DwgReader::from_stream_with_options(Cursor::new(data), options);
                let mut doc = reader.read().map_err(|e| e.to_string())?;
                match DxfWriter::new_binary(&doc).write_to_vec() {
                    Ok(b) => Ok(b),
                    Err(_) => {
                        doc.objects.retain(|_, o| !matches!(o, acadrust::objects::ObjectType::Unknown { .. }));
                        DxfWriter::new_binary(&doc).write_to_vec().map_err(|e| e.to_string())
                    }
                }
            }))
            .map_err(|_| "DWG decoder panicked".to_string())?
        })
        .map_err(|e| format!("thread spawn failed: {e}"))?;
    handle.join().map_err(|_| "DWG decoder thread panicked".to_string())?
}

/// Parse DXF bytes (binary or ASCII) into a CadDrawing.
pub fn parse_dxf(bytes: &[u8]) -> Result<CadDrawing, String> {
    let mut cur = Cursor::new(bytes);
    let drawing = dxf::Drawing::load(&mut cur).map_err(|e| e.to_string())?;
    let mut out = CadDrawing::default();
    for e in drawing.entities() {
        use dxf::entities::EntityType;
        match e.specific {
            EntityType::Line(ref l) => out.lines.push(CadLine {
                a: Pt { x: l.p1.x, y: l.p1.y },
                b: Pt { x: l.p2.x, y: l.p2.y },
            }),
            EntityType::Circle(ref c) => out.circles.push(CadCircle {
                c: Pt { x: c.center.x, y: c.center.y },
                r: c.radius,
            }),
            EntityType::Arc(ref a) => out.arcs.push(CadArc {
                c: Pt { x: a.center.x, y: a.center.y },
                r: a.radius,
                a0: a.start_angle,
                a1: a.end_angle,
            }),
            EntityType::LwPolyline(ref p) => {
                let mut pts = Vec::new();
                for v in &p.vertices { pts.push(Pt { x: v.x, y: v.y }); }
                if pts.len() >= 2 { out.polys.push(CadPoly { pts }); }
            }
            EntityType::Polyline(ref p) => {
                // legacy polyline: vertices are separate entities; skip geometry here
                let _ = p;
            }
            EntityType::Text(ref t) => out.texts.push(CadText {
                p: Pt { x: t.location.x, y: t.location.y },
                text: t.value.clone(),
                h: t.text_height,
            }),
            _ => {}
        }
    }
    Ok(out)
}

/// Import a DWG file: decode + parse. Returns the drawing and a status message.
pub fn import_dwg_file(path: &str) -> Result<(CadDrawing, String), String> {
    let data = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
    // If it's already a DXF, parse directly; otherwise treat as DWG.
    let ext = std::path::Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let (bytes, note) = if ext == "dxf" {
        (data, "DXF".to_string())
    } else {
        let b = decode_dwg(data)?;
        (b, "DWG".to_string())
    };
    let drawing = parse_dxf(&bytes)?;
    let count = drawing.primitive_count();
    Ok((drawing, format!("{note}: {count} primitives")))
}