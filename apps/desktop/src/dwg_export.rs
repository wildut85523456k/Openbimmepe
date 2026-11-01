//! DWG export: convert DXF bytes (already produced by the sheets exporter) into
//! native DWG bytes using the `acadrust` crate (MPL-2.0, used unmodified),
//! mirroring the reverse path in `dwg_import`.
//!
//! The conversion runs on a large-stack thread with `catch_unwind` so a panic
//! becomes an error instead of crashing the app.

use std::io::Cursor;

/// Encode DXF bytes (ASCII or binary) into DWG bytes.
pub fn encode_dwg(dxf_bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let handle = std::thread::Builder::new()
        .stack_size(128 << 20)
        .spawn(move || -> Result<Vec<u8>, String> {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                use acadrust::{DwgWriter, DxfReader};
                let mut reader = DxfReader::from_reader(Cursor::new(dxf_bytes))
                    .map_err(|e| e.to_string())?;
                let doc = reader.read().map_err(|e| e.to_string())?;
                DwgWriter::write_to_vec(&doc).map_err(|e| e.to_string())
            }))
            .map_err(|_| "DWG encoder panicked".to_string())?
        })
        .map_err(|e| format!("thread spawn failed: {e}"))?;
    handle.join().map_err(|_| "DWG encoder thread panicked".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_dxf_to_dwg_magic() {
        // Minimal ASCII DXF with one line.
        let dxf = "0\nSECTION\n2\nHEADER\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n11\n10.0\n21\n5.0\n31\n0.0\n0\nENDSEC\n0\nEOF\n";
        let dwg = encode_dwg(dxf.as_bytes().to_vec()).expect("encode");
        assert!(dwg.len() > 12, "DWG output too small");
        // DWG magic header: AC10xx
        assert!(dwg.starts_with(b"AC10"), "missing DWG magic: {:?}", &dwg[..6.min(dwg.len())]);
    }
}