//! Versioned print-profile data. Company values must come from supplied standards.
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::model::{PrintContext, SheetSet};
use crate::paper::{Margins, PaperSize};
use crate::render::{render_all_with_mode, LineWeightTable, PrintMode, RenderedSheet, ViewContents};
use crate::titleblock::TitleBlockDef;
use crate::visibility::ViewLibrary;
use crate::{clean_name, finite, SheetError};

fn invalid(message: impl Into<String>) -> SheetError {
    SheetError::BadValue { what: "print profile", value: message.into() }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileMargins {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl ProfileMargins {
    pub fn margins(&self) -> Margins {
        Margins { left: self.left, top: self.top, right: self.right, bottom: self.bottom }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePaper {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub margins_mm: ProfileMargins,
}

impl ProfilePaper {
    pub fn paper_size(&self) -> Result<PaperSize, SheetError> {
        PaperSize::custom(&self.name, self.width_mm, self.height_mm)
    }

    fn validate(&self) -> Result<(), SheetError> {
        self.paper_size()?;
        let m = &self.margins_mm;
        for value in [m.left, m.top, m.right, m.bottom] {
            finite(value, "profile margin")?;
            if value < 0.0 {
                return Err(invalid(format!("paper '{}': negative margin", self.name)));
            }
        }
        if self.width_mm - m.left - m.right < 20.0 || self.height_mm - m.top - m.bottom < 20.0 {
            return Err(invalid(format!("paper '{}': printable area must be at least 20 x 20 mm", self.name)));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalePens {
    pub scale: u32,
    pub widths_mm: Vec<f64>,
}

/// All values are required. No company defaults are inferred on load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintProfile {
    pub schema_version: u32,
    pub name: String,
    pub papers: Vec<ProfilePaper>,
    /// Sixteen widths, indexed by pen number minus one.
    pub paper_pens_mm: Vec<f64>,
    pub model_pen_tables: Vec<ScalePens>,
    pub mode: PrintMode,
}

impl PrintProfile {
    pub fn from_json(input: &str) -> Result<Self, SheetError> {
        if input.len() > 1_048_576 {
            return Err(invalid("JSON exceeds 1 MiB"));
        }
        let profile: Self = serde_json::from_str(input).map_err(|e| invalid(e.to_string()))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn to_json(&self) -> Result<String, SheetError> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| invalid(e.to_string()))
    }

    pub fn validate(&self) -> Result<(), SheetError> {
        if self.schema_version != 1 {
            return Err(invalid(format!("unsupported schema_version {}", self.schema_version)));
        }
        clean_name(&self.name, "profile name")?;
        if self.papers.is_empty() || self.papers.len() > 256 {
            return Err(invalid("needs 1 to 256 named papers"));
        }
        let mut names = BTreeSet::new();
        for paper in &self.papers {
            paper.validate()?;
            if !names.insert(paper.name.to_lowercase()) {
                return Err(invalid(format!("duplicate paper '{}'", paper.name)));
            }
        }
        self.line_weights()?;
        Ok(())
    }

    pub fn line_weights(&self) -> Result<LineWeightTable, SheetError> {
        if self.model_pen_tables.is_empty() || self.model_pen_tables.len() > 128 {
            return Err(invalid("needs 1 to 128 scale pen tables"));
        }
        let mut scales = BTreeSet::new();
        for row in &self.model_pen_tables {
            if !scales.insert(row.scale) {
                return Err(invalid(format!("duplicate scale 1:{}", row.scale)));
            }
        }
        LineWeightTable::new(self.paper_pens_mm.clone(), self.model_pen_tables.iter().map(|r| (r.scale, r.widths_mm.clone())).collect())
    }

    /// Create a placeholder title block from an explicitly chosen named profile paper.
    /// Does not replace existing project title blocks or claim to load a company template.
    pub fn generic_titleblock(&self, paper_name: &str, block_name: &str) -> Result<TitleBlockDef, SheetError> {
        self.validate()?;
        let paper = self.papers.iter().find(|p| p.name == paper_name).ok_or_else(|| SheetError::Unknown { what: "profile paper", value: paper_name.into() })?;
        TitleBlockDef::generic(block_name, paper.paper_size()?, paper.margins_mm.margins())
    }

    /// Use the profile pens and mode without changing any existing title-block geometry.
    pub fn render_all(&self, set: &SheetSet, lib: &ViewLibrary, contents: &ViewContents, ctx: &PrintContext) -> Result<Vec<RenderedSheet>, SheetError> {
        self.validate()?;
        render_all_with_mode(set, lib, contents, &self.line_weights()?, ctx, self.mode)
    }

    /// Compare named source media, preserving orientation. Differences are profile minus source.
    /// A report never silently changes the profile or substitutes a standard size.
    pub fn compare_media(&self, source: &[SourceMedia], tolerance_mm: f64) -> Result<Vec<MediaComparison>, SheetError> {
        self.validate()?;
        finite(tolerance_mm, "media tolerance")?;
        if tolerance_mm < 0.0 {
            return Err(invalid("negative media tolerance"));
        }
        let mut names = BTreeSet::new();
        let mut report = Vec::new();
        for media in source {
            PaperSize::custom(&media.name, media.width_mm, media.height_mm)?;
            if !names.insert(media.name.to_lowercase()) {
                return Err(invalid(format!("duplicate source media '{}'", media.name)));
            }
            let profile_size_mm = self.papers.iter().find(|p| p.name == media.name).map(|p| [p.width_mm, p.height_mm]);
            let difference_mm = profile_size_mm.map(|p| [p[0] - media.width_mm, p[1] - media.height_mm]);
            let status = match difference_mm {
                None => MediaStatus::Missing,
                Some(delta) if delta.iter().all(|v| v.abs() <= tolerance_mm) => MediaStatus::Match,
                Some(_) => MediaStatus::Different,
            };
            report.push(MediaComparison { name: media.name.clone(), source_size_mm: [media.width_mm, media.height_mm], profile_size_mm, difference_mm, status });
        }
        Ok(report)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceMedia {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaStatus {
    Match,
    Different,
    Missing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaComparison {
    pub name: String,
    pub source_size_mm: [f64; 2],
    pub profile_size_mm: Option<[f64; 2]>,
    pub difference_mm: Option<[f64; 2]>,
    pub status: MediaStatus,
}
