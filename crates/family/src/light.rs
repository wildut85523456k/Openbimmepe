//! Lighting data of a luminaire family: what a lighting program such as DIALux needs to know about the fitting,
//! beyond its place in the model. The position, the turn and the mounting height come from where the luminaire is
//! placed; the data below comes from the family author (or the manufacturer's data sheet). No standard values are
//! shipped: every number is typed in, and every field is optional until a calculation needs it.
use crate::{clean_text, Family, FamilyError, TypeDef};

/// How the luminaire is fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mounting {
    #[default]
    Recessed,
    Surface,
    Suspended,
    Wall,
    Free,
}

impl Mounting {
    pub fn name(self) -> &'static str {
        match self {
            Mounting::Recessed => "recessed",
            Mounting::Surface => "surface",
            Mounting::Suspended => "suspended",
            Mounting::Wall => "wall",
            Mounting::Free => "free",
        }
    }
    pub fn from_name(s: &str) -> Option<Mounting> {
        [Mounting::Recessed, Mounting::Surface, Mounting::Suspended, Mounting::Wall, Mounting::Free].into_iter().find(|m| m.name() == s)
    }
    /// the value of `Pset_LightFixtureTypeCommon.LightFixtureMountingType`
    pub fn ifc(self) -> &'static str {
        match self {
            Mounting::Recessed => "RECESSED",
            Mounting::Surface => "SURFACE",
            Mounting::Suspended => "SUSPENDED",
            Mounting::Wall => "OTHER",
            Mounting::Free => "FREESTANDING",
        }
    }
}

/// Lighting data of a family whose category is `LightFixture`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LightingDef {
    /// the photometric file the program reads, by name: an IES (`.ies`) or Eulumdat (`.ldt`) file kept with the project
    pub photometry: Option<String>,
    /// manufacturer and article number, so that a lighting program can find the luminaire in a catalogue
    pub manufacturer: Option<String>,
    pub article: Option<String>,
    /// luminous flux of the luminaire in lumens
    pub flux_lm: Option<f64>,
    /// electrical power in watts, driver included
    pub power_w: Option<f64>,
    /// correlated colour temperature in kelvin
    pub cct_k: Option<f64>,
    /// colour rendering index, 0 to 100
    pub cri: Option<f64>,
    /// ingress protection code as printed, for example "IP20"
    pub ip: Option<String>,
    /// share of the flux used, 0 to 1 (1 = full output)
    pub dimming: Option<f64>,
    pub mounting: Mounting,
    /// a picture of the luminaire (`.png`, `.jpg` or `.jpeg`) kept with the project under the role "picture";
    /// shown where the luminaire is chosen and printed in reports
    pub picture: Option<String>,
    /// the manufacturer's data sheet (`.pdf`) kept with the project under the role "datasheet"
    pub datasheet: Option<String>,
}

/// The file kinds a luminaire picture or data sheet may have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Png,
    Jpeg,
    Pdf,
}

/// What a file really is, judged by its first bytes and not by its name; None when it is none of the kinds above.
pub fn attachment_kind(data: &[u8]) -> Option<AttachmentKind> {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(AttachmentKind::Png)
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(AttachmentKind::Jpeg)
    } else if data.starts_with(b"%PDF-") {
        Some(AttachmentKind::Pdf)
    } else {
        None
    }
}

/// Check a file about to be kept as a luminaire picture (role "picture", PNG or JPEG) or data sheet (role "datasheet",
/// PDF): the content must be of that kind, whatever the name says, and the name must fit the kind.
pub fn check_attachment(role: &str, name: &str, data: &[u8]) -> Result<(), &'static str> {
    let l = name.to_ascii_lowercase();
    if name.contains(['/', '\\']) || name.trim().is_empty() {
        return Err("the file name must not be empty or contain a folder");
    }
    match (role, attachment_kind(data)) {
        ("picture", Some(AttachmentKind::Png)) if l.ends_with(".png") => Ok(()),
        ("picture", Some(AttachmentKind::Jpeg)) if l.ends_with(".jpg") || l.ends_with(".jpeg") => Ok(()),
        ("picture", Some(AttachmentKind::Png | AttachmentKind::Jpeg)) => Err("the picture's name does not match its content (.png or .jpg)"),
        ("picture", _) => Err("a picture must be a PNG or JPEG file"),
        ("datasheet", Some(AttachmentKind::Pdf)) if l.ends_with(".pdf") => Ok(()),
        ("datasheet", Some(AttachmentKind::Pdf)) => Err("the data sheet's name must end in .pdf"),
        ("datasheet", _) => Err("a data sheet must be a PDF file"),
        _ => Err("the role is \"picture\" or \"datasheet\""),
    }
}

pub const MAX_LIGHT_FLUX: f64 = 1.0e7;
pub const MAX_LIGHT_POWER: f64 = 1.0e6;

impl LightingDef {
    /// Lighting data from a manufacturer's photometric file read by `openbim_photo`. `file_name` is the name the
    /// file is kept under in the project. What the file states fills the fields (manufacturer, catalogue number as
    /// the article, the luminous flux the table adds up to, input power, and colour temperature and rendering when the
    /// file has them); the mounting, IP code and dimming level are not in such files and stay as they were in `base`.
    pub fn from_photometry(p: &openbim_photo::Photometry, file_name: &str, base: LightingDef) -> LightingDef {
        LightingDef {
            photometry: Some(file_name.to_string()),
            manufacturer: p.manufacturer.clone().or(base.manufacturer),
            article: p.catalogue.clone().or(base.article),
            flux_lm: p.flux_lm().filter(|f| *f > 0.0).or(base.flux_lm),
            power_w: p.watts.or(base.power_w),
            cct_k: p.cct_k.or(base.cct_k),
            cri: p.cri.or(base.cri),
            ip: base.ip,
            dimming: base.dimming,
            mounting: base.mounting,
            picture: base.picture,
            datasheet: base.datasheet,
        }
    }
    pub(crate) fn check(&self) -> Result<(), FamilyError> {
        let bad = |why: &'static str| Err(FamilyError::BadMeta(why));
        for (what, v) in [("photometry file", &self.photometry), ("manufacturer", &self.manufacturer), ("article number", &self.article), ("IP code", &self.ip), ("picture file", &self.picture), ("data sheet file", &self.datasheet)] {
            if let Some(s) = v {
                if s.trim().is_empty() {
                    return bad("a lighting text is empty");
                }
                clean_text(what, s, 128)?;
            }
        }
        if let Some(p) = &self.photometry {
            let l = p.to_ascii_lowercase();
            if !(l.ends_with(".ies") || l.ends_with(".ldt")) || p.contains(['/', '\\']) {
                return bad("the photometry must be the name of an .ies or .ldt file, without a folder");
            }
        }
        if let Some(p) = &self.picture {
            let l = p.to_ascii_lowercase();
            if !(l.ends_with(".png") || l.ends_with(".jpg") || l.ends_with(".jpeg")) || p.contains(['/', '\\']) {
                return bad("the picture must be the name of a .png or .jpg file, without a folder");
            }
        }
        if let Some(p) = &self.datasheet {
            if !p.to_ascii_lowercase().ends_with(".pdf") || p.contains(['/', '\\']) {
                return bad("the data sheet must be the name of a .pdf file, without a folder");
            }
        }
        let pos = |v: Option<f64>, max: f64| v.is_none_or(|x| x.is_finite() && x > 0.0 && x <= max);
        if !pos(self.flux_lm, MAX_LIGHT_FLUX) {
            return bad("the luminous flux must be above 0 and at most 10 000 000 lm");
        }
        if !pos(self.power_w, MAX_LIGHT_POWER) {
            return bad("the power must be above 0 and at most 1 000 000 W");
        }
        if !pos(self.cct_k, 100_000.0) {
            return bad("the colour temperature must be above 0 and at most 100 000 K");
        }
        if !self.cri.is_none_or(|x| x.is_finite() && (0.0..=100.0).contains(&x)) {
            return bad("the colour rendering index is between 0 and 100");
        }
        if !self.dimming.is_none_or(|x| x.is_finite() && x > 0.0 && x <= 1.0) {
            return bad("the dimming level is above 0 and at most 1");
        }
        Ok(())
    }

    /// What a lighting calculation still lacks: the names of the fields that are empty and needed.
    /// A photometric file (or a manufacturer and article) says how the light is spread; the flux and power scale it.
    pub fn missing_for_calculation(&self) -> Vec<&'static str> {
        let mut m = Vec::new();
        if self.photometry.is_none() && !(self.manufacturer.is_some() && self.article.is_some()) {
            m.push("photometry file or manufacturer and article");
        }
        if self.flux_lm.is_none() {
            m.push("luminous flux");
        }
        if self.power_w.is_none() {
            m.push("power");
        }
        m
    }
}

/// One type made from one photometric file by `types_from_imports`.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantType {
    /// the name of the type in the family
    pub type_name: String,
    /// the name the file is to be kept under in the project (no folder); put `imports[import].data` there
    pub file_name: String,
    /// index into the list that was imported
    pub import: usize,
}

/// What `types_from_imports` did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VariantReport {
    pub added: Vec<VariantType>,
    /// files that could not be read, and types that were given another name because theirs was taken
    pub warnings: Vec<String>,
}

fn flat(name: &str) -> String {
    let n = name.rsplit(['/', '\\']).next().unwrap_or(name);
    n.chars().filter(|c| !c.is_control()).take(120).collect::<String>().trim().to_string()
}

fn stem(file: &str) -> &str {
    let l = file.to_ascii_lowercase();
    if l.ends_with(".ies") || l.ends_with(".ldt") { &file[..file.len() - 4] } else { file }
}

/// Add one type to `fam` for each photometric file in `imports` that was read (a manufacturer's archive holds one
/// file per variant: a power setting, a colour temperature, an optic). Each type carries the lighting data its file
/// gives, on top of the family's own `lighting` (mounting, IP code and dimming, which files do not state). Types are
/// named after the files; a name or file name that is already taken gets " (2)", " (3)" ... The family is changed only
/// when every type can be added; the files that were read are listed so the caller can keep them in the project.
pub fn types_from_imports(fam: &mut Family, imports: &[openbim_photo::Imported]) -> Result<VariantReport, FamilyError> {
    let all: Vec<usize> = (0..imports.len()).collect();
    add_variant_types(fam, imports, &all)
}

/// What a person sees when choosing variants from an import: one row per file, in the order of the import.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantRow {
    /// index into the list that was imported (what `types_from_selection` takes)
    pub import: usize,
    /// the file's name without any folder
    pub file: String,
    /// why the file cannot be used, when it cannot
    pub unusable: Option<String>,
    pub description: Option<String>,
    /// flux computed from the intensity table, lm
    pub flux_lm: Option<f64>,
    pub power_w: Option<f64>,
    pub cct_k: Option<f64>,
    pub cri: Option<f64>,
}

/// The rows of a picker for the files of an import. Nothing is chosen by this: the person ticks rows, and the numbers of the
/// ticked rows go to `types_from_selection`.
pub fn variant_rows(imports: &[openbim_photo::Imported]) -> Vec<VariantRow> {
    imports
        .iter()
        .enumerate()
        .map(|(k, im)| {
            let file = flat(&im.name);
            match &im.result {
                Ok(p) => VariantRow { import: k, file, unusable: None, description: p.description.clone(), flux_lm: p.flux_lm().filter(|f| *f > 0.0), power_w: p.watts, cct_k: p.cct_k, cri: p.cri },
                Err(e) => VariantRow { import: k, file, unusable: Some(e.to_string()), description: None, flux_lm: None, power_w: None, cct_k: None, cri: None },
            }
        })
        .collect()
}

/// Like `types_from_imports`, for the files the person ticked: `chosen` holds indexes into `imports` (the `import` of the
/// rows). The types are added in the order of the import, whatever the order or repeats in `chosen`. Nothing chosen, or an
/// index outside the import, is refused and the family stays as it was; a chosen file that could not be read is reported
/// in the warnings like any other.
pub fn types_from_selection(fam: &mut Family, imports: &[openbim_photo::Imported], chosen: &[usize]) -> Result<VariantReport, FamilyError> {
    if chosen.is_empty() {
        return Err(FamilyError::BadMeta("no file is chosen"));
    }
    if chosen.iter().any(|&k| k >= imports.len()) {
        return Err(FamilyError::BadMeta("a chosen file is not in the import"));
    }
    let mut ks = chosen.to_vec();
    ks.sort_unstable();
    ks.dedup();
    add_variant_types(fam, imports, &ks)
}

fn add_variant_types(fam: &mut Family, imports: &[openbim_photo::Imported], which: &[usize]) -> Result<VariantReport, FamilyError> {
    let base = fam.lighting.clone().unwrap_or_default();
    let mut report = VariantReport::default();
    let mut types: Vec<TypeDef> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    for &k in which {
        let im = &imports[k];
        let p = match &im.result {
            Ok(p) => p,
            Err(e) => {
                report.warnings.push(format!("{}: not used, {e}", flat(&im.name)));
                continue;
            }
        };
        let file = flat(&im.name);
        if file.is_empty() {
            report.warnings.push(format!("file {}: not used, it has no name", k + 1));
            continue;
        }
        let unique = |want: &str, taken: &dyn Fn(&str) -> bool| -> String {
            let mut n = want.to_string();
            let mut i = 2;
            while taken(&n) {
                n = format!("{want} ({i})");
                i += 1;
            }
            n
        };
        // a taken file name is numbered before its extension, so that it stays an .ies or .ldt file
        let file_name = {
            let base = stem(&file);
            let ext = &file[base.len()..];
            unique(base, &|n| files.iter().any(|f| *f == format!("{n}{ext}"))) + ext
        };
        let wanted: String = stem(&file).chars().take(64).collect();
        let type_name = unique(&wanted, &|n| fam.types.iter().chain(types.iter()).any(|t| t.name == n));
        if type_name != wanted {
            report.warnings.push(format!("{file}: the type is called '{type_name}' because '{wanted}' was taken"));
        }
        let l = LightingDef::from_photometry(p, &file_name, base.clone());
        l.check()?;
        types.push(TypeDef { name: type_name.clone(), lighting: Some(l), ..Default::default() });
        files.push(file_name.clone());
        report.added.push(VariantType { type_name, file_name, import: k });
    }
    fam.types.extend(types);
    Ok(report)
}
