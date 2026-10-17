//! Photometric files as manufacturers publish them: IES (IESNA LM-63, 1991 to 2019) and Eulumdat (`.ldt`).
//!
//! `parse` takes the bytes of a file and the file's name, tells the format from the content (the name only breaks a
//! tie), and returns a `Photometry`: the identity the file gives (manufacturer, catalogue number, description), the
//! electrical and lamp data it holds, the size of the luminaire, and the table of luminous intensity over the
//! sphere. From the table it computes the luminous flux the luminaire sends out, exactly for the piecewise linear
//! table, so a program can check the stated flux against the distribution.
//!
//! Angles are degrees, intensity candela, flux lumens, sizes metres. Files are untrusted: sizes are limited, any
//! malformed number is an error that names the line or item, and nothing panics.
//! Only the common type C photometry (C-planes and gamma angles) is evaluated; A and B photometry is read and kept
//! but has no flux (a note says so).

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_ANGLES: usize = 2000;
const MAX_VALUES: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Ies,
    Eulumdat,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PhotoError {
    TooLarge,
    /// neither an IES nor an Eulumdat file
    UnknownFormat,
    /// what is wrong, and where when known
    Bad(String),
}

impl std::fmt::Display for PhotoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhotoError::TooLarge => write!(f, "the file is larger than 8 MB"),
            PhotoError::UnknownFormat => write!(f, "not an IES or Eulumdat file"),
            PhotoError::Bad(s) => write!(f, "{s}"),
        }
    }
}
impl std::error::Error for PhotoError {}

fn bad<T>(s: impl Into<String>) -> Result<T, PhotoError> {
    Err(PhotoError::Bad(s.into()))
}

/// How the table repeats over the full circle of C-planes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symmetry {
    /// one plane, the same all round
    Axial,
    /// planes from 0 to 90: mirrored about C0-C180 and C90-C270
    Quadrant,
    /// planes from 0 to 180: mirrored about C0-C180
    About0_180,
    /// planes from 90 to 270: mirrored about C90-C270
    About90_270,
    /// planes all the way round
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Photometry {
    pub format: Format,
    pub manufacturer: Option<String>,
    /// catalogue or article number
    pub catalogue: Option<String>,
    pub description: Option<String>,
    /// total flux of the lamps in lumens, when the file states it
    pub lamp_flux_lm: Option<f64>,
    /// electrical input in watts, ballast included
    pub watts: Option<f64>,
    /// light output ratio 0..1 as stated by the file (Eulumdat)
    pub lor_stated: Option<f64>,
    pub cct_k: Option<f64>,
    pub cri: Option<f64>,
    /// luminaire length, width and height in metres (0 where the file gives none)
    pub size_m: [f64; 3],
    pub symmetry: Symmetry,
    /// C angles of the stored planes, ascending
    pub c: Vec<f64>,
    /// gamma angles, ascending
    pub g: Vec<f64>,
    /// absolute intensity in candela: `cd[plane][gamma]`
    pub cd: Vec<Vec<f64>>,
    /// things a reader should know (a stated flux that does not match the table, an unsupported photometry type)
    pub notes: Vec<String>,
    type_c: bool,
}

// ---------- numbers ----------

fn tokens(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty())
}

struct Nums<'a, I: Iterator<Item = &'a str>> {
    it: I,
}
impl<'a, I: Iterator<Item = &'a str>> Nums<'a, I> {
    fn next(&mut self, what: &str) -> Result<f64, PhotoError> {
        let t = self.it.next().ok_or_else(|| PhotoError::Bad(format!("the file ends before {what}")))?;
        let v: f64 = t.parse().map_err(|_| PhotoError::Bad(format!("{what}: '{t}' is not a number")))?;
        if v.is_finite() { Ok(v) } else { bad(format!("{what}: not a finite number")) }
    }
    fn count(&mut self, what: &str, max: usize) -> Result<usize, PhotoError> {
        let v = self.next(what)?;
        if v < 0.0 || v.fract() != 0.0 || v > max as f64 { return bad(format!("{what} must be a whole number from 0 to {max}")); }
        Ok(v as usize)
    }
}

fn clean(s: &str) -> Option<String> {
    let t: String = s.chars().filter(|c| !c.is_control()).collect();
    let t = t.trim();
    if t.is_empty() { None } else { Some(t.chars().take(128).collect()) }
}

fn ascending(v: &[f64], what: &str) -> Result<(), PhotoError> {
    if v.windows(2).any(|w| w[1] <= w[0]) { return bad(format!("{what} must increase")); }
    Ok(())
}

// ---------- IES ----------

fn parse_ies(text: &str) -> Result<Photometry, PhotoError> {
    let mut keys: Vec<(String, String)> = Vec::new();
    let mut lines = text.lines().peekable();
    // header: keyword lines up to TILT=
    let mut rest = String::new();
    let mut tilt = None;
    for l in lines.by_ref() {
        let t = l.trim();
        if let Some(v) = t.strip_prefix("TILT=") {
            tilt = Some(v.trim().to_string());
            break;
        }
        if let Some(r) = t.strip_prefix('[') {
            if let Some((k, v)) = r.split_once(']') {
                let k = k.trim().to_ascii_uppercase();
                if k == "MORE" {
                    if let Some(last) = keys.last_mut() { last.1.push(' '); last.1.push_str(v.trim()); }
                } else {
                    keys.push((k, v.trim().to_string()));
                }
            }
        }
    }
    let Some(tilt) = tilt else { return bad("no TILT= line") };
    for l in lines {
        rest.push_str(l);
        rest.push('\n');
    }
    let mut n = Nums { it: tokens(&rest) };
    if tilt.eq_ignore_ascii_case("INCLUDE") {
        n.next("the lamp-to-luminaire geometry")?;
        let k = n.count("the number of tilt angles", MAX_ANGLES)?;
        for _ in 0..2 * k { n.next("the tilt table")?; }
    }
    let lamps = n.next("the number of lamps")?;
    let lumens = n.next("the lumens per lamp")?;
    let mult = n.next("the candela multiplier")?;
    let nv = n.count("the number of vertical angles", MAX_ANGLES)?;
    let nh = n.count("the number of horizontal angles", MAX_ANGLES)?;
    let ptype = n.next("the photometric type")?;
    let units = n.next("the units type")?;
    let (w, l, h) = (n.next("the width")?, n.next("the length")?, n.next("the height")?);
    n.next("the ballast factor")?;
    n.next("the future use field")?;
    let watts = n.next("the input watts")?;
    if nv < 2 || nh < 1 || nv.saturating_mul(nh) > MAX_VALUES { return bad("the angle counts are not usable"); }
    if mult <= 0.0 { return bad("the candela multiplier must be above 0"); }
    let mut g = Vec::with_capacity(nv);
    for _ in 0..nv { g.push(n.next("a vertical angle")?); }
    let mut c = Vec::with_capacity(nh);
    for _ in 0..nh { c.push(n.next("a horizontal angle")?); }
    ascending(&g, "the vertical angles")?;
    ascending(&c, "the horizontal angles")?;
    let mut cd = Vec::with_capacity(nh);
    for _ in 0..nh {
        let mut row = Vec::with_capacity(nv);
        for _ in 0..nv {
            let v = n.next("a candela value")?;
            if v < 0.0 { return bad("a candela value is negative"); }
            row.push(v * mult);
        }
        cd.push(row);
    }
    let get = |k: &str| keys.iter().find(|(a, _)| a == k).and_then(|(_, v)| clean(v));
    let unit = if units == 1.0 { 0.3048 } else { 1.0 };
    let mut notes = Vec::new();
    let type_c = ptype == 1.0;
    if !type_c { notes.push("A or B photometry: read, but the flux is not computed".to_string()); }
    let symmetry = if !type_c {
        Symmetry::None
    } else {
        match (c.first().copied().unwrap_or(0.0), c.last().copied().unwrap_or(0.0)) {
            (_, last) if nh == 1 && last == 0.0 => Symmetry::Axial,
            (0.0, 90.0) => Symmetry::Quadrant,
            (0.0, 180.0) => Symmetry::About0_180,
            (90.0, 270.0) => Symmetry::About90_270,
            (0.0, 360.0) => Symmetry::None,
            _ => { notes.push("unusual horizontal angles: taken as given, outside them the intensity is zero".to_string()); Symmetry::None }
        }
    };
    let lamp_flux = (lumens > 0.0 && lamps > 0.0).then_some(lumens * lamps);
    let mut p = Photometry {
        format: Format::Ies,
        manufacturer: get("MANUFAC"),
        catalogue: get("LUMCAT").or_else(|| get("LAMPCAT")),
        description: get("LUMINAIRE").or_else(|| get("TEST")),
        lamp_flux_lm: lamp_flux,
        watts: (watts > 0.0).then_some(watts),
        lor_stated: None,
        cct_k: None,
        cri: None,
        size_m: [l.abs() * unit, w.abs() * unit, h.abs() * unit],
        symmetry,
        c,
        g,
        cd,
        notes,
        type_c,
    };
    if let (Some(f), Some(lamp)) = (p.flux_lm(), p.lamp_flux_lm) {
        if f > lamp * 1.05 { p.notes.push(format!("the distribution gives {f:.0} lm, more than the {lamp:.0} lm of the lamps: the candela values may be relative to a reference lamp")); }
    }
    Ok(p)
}

// ---------- Eulumdat ----------

fn parse_ldt(text: &str) -> Result<Photometry, PhotoError> {
    let lines: Vec<&str> = text.lines().collect();
    let line = |i: usize, what: &str| -> Result<&str, PhotoError> { lines.get(i).copied().ok_or_else(|| PhotoError::Bad(format!("the file ends before {what}"))) };
    let num = |i: usize, what: &str| -> Result<f64, PhotoError> {
        let t = line(i, what)?.trim().replace(',', ".");
        t.parse::<f64>().ok().filter(|v| v.is_finite()).ok_or_else(|| PhotoError::Bad(format!("line {}: {what} is not a number", i + 1)))
    };
    let isym = num(2, "the symmetry indicator")? as i64;
    let mc = num(3, "the number of C-planes")?;
    let dc = num(4, "the C-plane step")?;
    let ng = num(5, "the number of intensities per plane")?;
    let dg = num(6, "the gamma step")?;
    for (v, what) in [(mc, "the number of C-planes"), (ng, "the number of intensities")] {
        if v < 0.0 || v.fract() != 0.0 || v > MAX_ANGLES as f64 { return bad(format!("{what} is not usable")); }
    }
    let (mc, ng) = (mc as usize, ng as usize);
    if !(0..=4).contains(&isym) { return bad("the symmetry indicator is not 0 to 4"); }
    if ng < 2 { return bad("a plane needs at least 2 intensities"); }
    let mm = |i: usize, w: &str| num(i, w).map(|v| (v / 1000.0).abs());
    let (len, wid, hei) = (mm(12, "the luminaire length")?, mm(13, "the luminaire width")?, mm(14, "the luminaire height")?);
    let lorl = num(22, "the light output ratio")?;
    let conv = num(23, "the conversion factor")?;
    let nsets = num(25, "the number of lamp sets")?;
    if !(1.0..=20.0).contains(&nsets) || nsets.fract() != 0.0 { return bad("the number of lamp sets is not 1 to 20"); }
    let nsets = nsets as usize;
    let mut flux = 0.0;
    let mut watts = 0.0;
    let (mut cct, mut cri) = (None, None);
    for s in 0..nsets {
        let b = 26 + 6 * s;
        flux += num(b + 2, "the lamp flux")?;
        watts += num(b + 5, "the lamp wattage")?;
        if s == 0 {
            let first = |i: usize| -> String { lines.get(i).copied().unwrap_or("").trim().to_string() };
            let digits: String = first(b + 3).chars().take_while(|c| c.is_ascii_digit()).collect();
            cct = digits.parse::<f64>().ok().filter(|v| (1000.0..=20000.0).contains(v));
            cri = first(b + 4).parse::<f64>().ok().filter(|v| (0.0..=100.0).contains(v));
        }
    }
    if flux < 0.0 || watts < 0.0 { return bad("a lamp flux or wattage is negative"); }
    // after the lamp sets: 10 direct ratios, then the angles and intensities, all plain numbers
    let tail_start = 26 + 6 * nsets;
    let tail: String = lines.iter().skip(tail_start).copied().collect::<Vec<_>>().join("\n");
    let mut n = Nums { it: tokens(&tail) };
    for _ in 0..10 { n.next("the direct ratios")?; }
    let remaining: Vec<&str> = n.it.collect();
    // the number of planes actually stored depends on the symmetry
    let (stored, sym) = match isym {
        0 => (mc, Symmetry::None),
        1 => (1, Symmetry::Axial),
        2 => (mc / 2 + 1, Symmetry::About0_180),
        3 => (mc / 2 + 1, Symmetry::About90_270),
        _ => (mc / 4 + 1, Symmetry::Quadrant),
    };
    let stored = stored.max(1);
    // files differ on whether the list of C angles has all planes or only the stored ones: whatever is left after
    // the gamma angles and the table is the list of C angles
    let nc = remaining.len().checked_sub(stored * ng + ng).filter(|&k| k <= MAX_ANGLES);
    let Some(nc) = nc else { return bad(format!("the intensity table has {} numbers; {} planes of {} intensities were expected", remaining.len(), stored, ng)) };
    if stored.saturating_mul(ng) > MAX_VALUES { return bad("the table is too large"); }
    let mut it = Nums { it: remaining.into_iter() };
    let mut cs = Vec::new();
    for _ in 0..nc { cs.push(it.next("a C angle")?); }
    let mut gs = Vec::new();
    for _ in 0..ng { gs.push(it.next("a gamma angle")?); }
    // regular grids when the file gives fewer angles than it should
    let c_angles: Vec<f64> = match (isym, dc) {
        (1, _) => vec![0.0],
        (_, d) if d > 0.0 => {
            let first = if isym == 3 { 90.0 } else { 0.0 };
            (0..stored).map(|k| first + k as f64 * d).collect()
        }
        _ => cs.iter().copied().take(stored).collect(),
    };
    let g_angles = if gs.windows(2).all(|w| w[1] > w[0]) { gs } else { (0..ng).map(|k| k as f64 * dg).collect() };
    ascending(&g_angles, "the gamma angles")?;
    let scale = if conv > 0.0 { conv } else { 1.0 } * flux / 1000.0;
    let mut cd = Vec::with_capacity(stored);
    for _ in 0..stored {
        let mut row = Vec::with_capacity(ng);
        for _ in 0..ng {
            let v = it.next("an intensity")?;
            if v < 0.0 { return bad("an intensity is negative"); }
            row.push(v * scale);
        }
        cd.push(row);
    }
    // symmetry about C90-C270 stores planes 270..360 then 0..90 in some files and 90..270 in others: ascending 90.. is used
    let mut notes = Vec::new();
    if flux <= 0.0 { notes.push("the file states no lamp flux, so the intensities cannot be turned into candela".to_string()); }
    let name = |i: usize| lines.get(i).and_then(|l| clean(l));
    let mut p = Photometry {
        format: Format::Eulumdat,
        manufacturer: name(0),
        catalogue: name(9),
        description: name(8),
        lamp_flux_lm: (flux > 0.0).then_some(flux),
        watts: (watts > 0.0).then_some(watts),
        lor_stated: (lorl > 0.0 && lorl <= 100.0).then_some(lorl / 100.0),
        cct_k: cct,
        cri,
        size_m: [len, wid, hei],
        symmetry: sym,
        c: c_angles,
        g: g_angles,
        cd,
        notes,
        type_c: true,
    };
    ascending(&p.c, "the C angles")?;
    if let (Some(f), Some(lor)) = (p.flux_lm(), p.lor_stated.zip(p.lamp_flux_lm).map(|(l, lamp)| l * lamp)) {
        if lor > 0.0 && (f - lor).abs() > 0.05 * lor { p.notes.push(format!("the table gives {f:.0} lm, the stated light output ratio {lor:.0} lm")); }
    }
    Ok(p)
}

// ---------- evaluation ----------

impl Photometry {
    fn plane_of(&self, c: f64) -> (usize, usize, f64) {
        // the stored planes that bracket C (after folding it by the symmetry), and the weight of the upper one
        let c = c.rem_euclid(360.0);
        let f = match self.symmetry {
            Symmetry::Axial => 0.0,
            Symmetry::Quadrant => { let a = if c > 180.0 { 360.0 - c } else { c }; if a > 90.0 { 180.0 - a } else { a } }
            Symmetry::About0_180 => if c > 180.0 { 360.0 - c } else { c },
            Symmetry::About90_270 => if c < 90.0 { 180.0 - c } else if c > 270.0 { 540.0 - c } else { c },
            Symmetry::None => c,
        };
        let n = self.c.len();
        if n == 1 { return (0, 0, 0.0); }
        let k = self.c.partition_point(|&a| a <= f).clamp(1, n - 1);
        let (a, b) = (self.c[k - 1], self.c[k]);
        (k - 1, k, ((f - a) / (b - a)).clamp(0.0, 1.0))
    }

    /// Luminous intensity in candela towards (C, gamma) in degrees, linear between table entries; 0 outside the
    /// gamma range of the table.
    pub fn intensity(&self, c: f64, gamma: f64) -> f64 {
        if self.g.is_empty() || gamma < self.g[0] - 1e-9 || gamma > self.g[self.g.len() - 1] + 1e-9 { return 0.0; }
        let (i, j, w) = self.plane_of(c);
        let at = |row: &Vec<f64>| -> f64 {
            let k = self.g.partition_point(|&a| a <= gamma).clamp(1, self.g.len() - 1);
            let (a, b) = (self.g[k - 1], self.g[k]);
            let t = ((gamma - a) / (b - a)).clamp(0.0, 1.0);
            row[k - 1] * (1.0 - t) + row[k] * t
        };
        at(&self.cd[i]) * (1.0 - w) + at(&self.cd[j]) * w
    }

    /// Greatest intensity in the table, candela.
    pub fn max_intensity(&self) -> f64 {
        self.cd.iter().flatten().copied().fold(0.0, f64::max)
    }

    /// Luminous flux sent out in all directions, lumens: the integral of the intensity over the sphere. It is
    /// exact for the table read as piecewise linear in gamma, and sums the C-planes in 1 degree steps. None for A and B photometry.
    pub fn flux_lm(&self) -> Option<f64> {
        if !self.type_c { return None; }
        let steps = 360;
        let mut total = 0.0;
        for s in 0..steps {
            let c = s as f64 + 0.5;
            // integral over gamma of I sin(gamma), exact for linear I between the stored angles
            let mut ring = 0.0;
            let (i, j, w) = self.plane_of(c);
            for k in 1..self.g.len() {
                let (g0, g1) = (self.g[k - 1].to_radians(), self.g[k].to_radians());
                let blend = |idx: usize| self.cd[i][idx] * (1.0 - w) + self.cd[j][idx] * w;
                let (i0, i1) = (blend(k - 1), blend(k));
                // I(g) = i0 + sl (g - g0); the integral of g sin g is -g cos g + sin g
                let sl = (i1 - i0) / (g1 - g0);
                let int_sin = g0.cos() - g1.cos();
                ring += (i0 - sl * g0) * int_sin + sl * ((-g1 * g1.cos() + g1.sin()) - (-g0 * g0.cos() + g0.sin()));
            }
            total += ring * (std::f64::consts::TAU / steps as f64);
        }
        Some(total)
    }

    /// Light output ratio: the flux of the luminaire over the flux of its lamps, when both are known.
    pub fn lor(&self) -> Option<f64> {
        Some(self.flux_lm()? / self.lamp_flux_lm?)
    }

    /// Full beam angle in degrees at half the greatest intensity in the C0-C180 plane (the angle between the two
    /// directions where the intensity falls to half), None when the table does not fall that far.
    pub fn beam_angle_deg(&self) -> Option<f64> {
        let peak = self.max_intensity();
        if peak <= 0.0 { return None; }
        let half = peak / 2.0;
        // walk away from the peak direction of plane C0 on both sides
        let (mut best, mut at) = (0.0, 0.0);
        for k in 0..=3600 {
            let g = k as f64 * 0.05 + self.g[0];
            if g > self.g[self.g.len() - 1] { break; }
            let v = self.intensity(0.0, g);
            if v > best { best = v; at = g; }
        }
        let find = |dir: f64| -> Option<f64> {
            let mut g = at;
            while g >= self.g[0] && g <= self.g[self.g.len() - 1] {
                if self.intensity(0.0, g) < half { return Some((g - at).abs()); }
                g += dir * 0.05;
            }
            None
        };
        // a downlight peaks at gamma 0, and the other side is the mirror image through the axis
        let up = find(1.0)?;
        let down = if at - self.g[0] < 1e-6 { up } else { find(-1.0)? };
        Some(up + down)
    }
}

/// Read a photometric file. `name` is the file's name (used only to break a tie between formats).
pub fn parse(bytes: &[u8], name: &str) -> Result<Photometry, PhotoError> {
    if bytes.len() > MAX_BYTES { return Err(PhotoError::TooLarge); }
    // manufacturers write Latin-1 as often as UTF-8
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    };
    let text = text.trim_start_matches('\u{feff}');
    let looks_ies = text.lines().take(80).any(|l| l.trim_start().to_ascii_uppercase().starts_with("TILT="));
    let lower = name.to_ascii_lowercase();
    if looks_ies || text.trim_start().starts_with("IESNA") {
        parse_ies(text)
    } else if lower.ends_with(".ldt") || lower.ends_with(".eul") || text.lines().count() > 30 && text.lines().nth(2).is_some_and(|l| l.trim().len() == 1 && l.trim().chars().all(|c| c.is_ascii_digit())) {
        parse_ldt(text)
    } else {
        Err(PhotoError::UnknownFormat)
    }
}

/// One file found by `import`: its name (inside the archive when there is one) and what reading it gave.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub name: String,
    /// the file's own bytes, to keep in the project exactly as the manufacturer wrote them
    pub data: Vec<u8>,
    pub result: Result<Photometry, PhotoError>,
}

const MAX_ENTRIES: usize = 256;
const MAX_TOTAL: u64 = 64 * 1024 * 1024;

/// Read what a manufacturer hands out: one IES or Eulumdat file, or a zip archive holding several (many
/// catalogues ship one archive per luminaire family, one file per colour temperature or power setting, and
/// some name the archive `.ies`). Archives are read without being unpacked to disk; entry count and size are
/// limited, folders and other file types inside are skipped, and a file that cannot be read is listed with its error.
pub fn import(bytes: &[u8], name: &str) -> Vec<Imported> {
    use std::io::Read;
    if !bytes.starts_with(b"PK\x03\x04") {
        return vec![Imported { name: name.to_string(), data: bytes.to_vec(), result: parse(bytes, name) }];
    }
    let mut out = Vec::new();
    let mut zip = match zip::ZipArchive::new(std::io::Cursor::new(bytes)) {
        Ok(z) => z,
        Err(e) => return vec![Imported { name: name.to_string(), data: Vec::new(), result: Err(PhotoError::Bad(format!("the archive cannot be read: {e}"))) }],
    };
    if zip.len() > MAX_ENTRIES {
        return vec![Imported { name: name.to_string(), data: Vec::new(), result: Err(PhotoError::Bad(format!("the archive holds more than {MAX_ENTRIES} files"))) }];
    }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let Ok(mut f) = zip.by_index(i) else { continue };
        // an entry whose name is not valid (an unsafe path, bad encoding) is listed with an error, not read
        let inner = match f.name() {
            Ok(n) => n.to_string(),
            Err(e) => {
                out.push(Imported { name: format!("entry {}", i + 1), data: Vec::new(), result: Err(PhotoError::Bad(format!("the entry name is not usable: {e}"))) });
                continue;
            }
        };
        let lower = inner.to_ascii_lowercase();
        if f.is_dir() || !(lower.ends_with(".ies") || lower.ends_with(".ldt")) { continue; }
        total = total.saturating_add(f.size());
        if f.size() > MAX_BYTES as u64 || total > MAX_TOTAL {
            out.push(Imported { name: inner, data: Vec::new(), result: Err(PhotoError::TooLarge) });
            continue;
        }
        let mut buf = Vec::new();
        // the declared size can lie: read no more than the limit
        match (&mut f).take(MAX_BYTES as u64 + 1).read_to_end(&mut buf) {
            Ok(_) => out.push(Imported { result: parse(&buf, &inner), data: buf, name: inner }),
            Err(e) => out.push(Imported { name: inner, data: Vec::new(), result: Err(PhotoError::Bad(format!("cannot be unpacked: {e}"))) }),
        }
    }
    if out.is_empty() {
        out.push(Imported { name: name.to_string(), data: Vec::new(), result: Err(PhotoError::Bad("the archive holds no .ies or .ldt file".into())) });
    }
    out
}
