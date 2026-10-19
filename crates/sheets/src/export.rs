//! Shared, side-effect-free PDF/DXF command for desktop and batch callers.
//! Destination selection, overwrite policy and filesystem writes belong to the caller.
use std::collections::BTreeSet;

use crate::dxf::write_dxf;
use crate::model::{PrintContext, SheetSet};
use crate::pdf::write_pdf;
use crate::profile::PrintProfile;
use crate::render::render_sheet_with_mode;
use crate::visibility::ViewLibrary;
use crate::{SheetError, render::ViewContents};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Pdf,
    Dxf,
}

/// Explicit, ordered sheet selection. An empty selection is an error.
#[derive(Debug, Clone)]
pub struct ExportCommand {
    pub format: ExportFormat,
    pub sheets: Vec<String>,
    pub title: String,
}

pub struct ExportSource<'a> {
    pub set: &'a SheetSet,
    pub views: &'a ViewLibrary,
    pub contents: &'a ViewContents,
    pub profile: &'a PrintProfile,
    pub context: &'a PrintContext,
}

#[derive(Debug)]
pub struct ExportFile {
    /// Portable suggested basename, never a user-provided path.
    pub name: String,
    /// Ordered mapping from file pages to source sheet numbers.
    pub sheets: Vec<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct ExportBundle {
    pub files: Vec<ExportFile>,
    pub warnings: Vec<String>,
}

impl ExportCommand {
    pub fn execute(&self, source: ExportSource<'_>) -> Result<ExportBundle, SheetError> {
        if self.sheets.is_empty() {
            return Err(SheetError::BadValue {
                what: "export selection", value: "select at least one sheet".into(),
            });
        }
        if self.sheets.len() > crate::model::MAX_SHEETS {
            return Err(SheetError::TooMany("selected sheets"));
        }
        let mut seen = BTreeSet::new();
        // Preflight the whole request before encoding anything. In particular, never
        // silently export a blank viewport because its content adapter was not connected.
        for number in &self.sheets {
            if !seen.insert(number) {
                return Err(SheetError::Duplicate { what: "selected sheet", value: number.clone() });
            }
            let sheet = source.set.sheet(number).ok_or_else(|| SheetError::Unknown {
                what: "sheet", value: number.clone(),
            })?;
            for viewport in &sheet.viewports {
                if !source.contents.contains_key(&viewport.view) {
                    return Err(SheetError::Unknown {
                        what: "view contents", value: viewport.view.clone(),
                    });
                }
            }
        }
        source.profile.validate()?;
        let weights = source.profile.line_weights()?;
        let rendered = self.sheets.iter().map(|number| {
            render_sheet_with_mode(
                source.set, source.views, source.contents, &weights,
                number, source.context, source.profile.mode,
            )
        }).collect::<Result<Vec<_>, _>>()?;
        match self.format {
            ExportFormat::Pdf => {
                let pdf = write_pdf(&rendered, &self.title);
                Ok(ExportBundle {
                    files: vec![ExportFile {
                        name: "export.pdf".into(), sheets: self.sheets.clone(), bytes: pdf.bytes,
                    }],
                    warnings: pdf.warnings,
                })
            }
            ExportFormat::Dxf => {
                let mut warnings: Vec<_> = rendered.iter()
                    .flat_map(|sheet| sheet.warnings.iter().cloned()).collect();
                warnings.push("DXF R12 uses approximate indexed colours and pen-width layer names; receiving CAD settings control appearance.".into());
                let files = rendered.iter().enumerate().map(|(index, sheet)| ExportFile {
                    name: format!("sheet-{:04}.dxf", index + 1),
                    sheets: vec![sheet.number.clone()],
                    bytes: write_dxf(sheet).into_bytes(),
                }).collect();
                Ok(ExportBundle { files, warnings })
            }
        }
    }
}
