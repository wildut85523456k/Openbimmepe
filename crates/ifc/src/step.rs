//! The STEP (ISO 10303-21) text that an IFC file is made of: numbered entities, reals and strings written
//! the way the standard asks.

/// A real number as STEP wants it: always with a point, exponent as `E`.
pub fn real(x: f64) -> String {
    if x == 0.0 {
        return "0.".into();
    }
    let s = format!("{x:?}");
    match s.split_once('e') {
        Some((m, e)) => {
            let m = if m.contains('.') { m.to_string() } else { format!("{m}.") };
            format!("{m}E{e}")
        }
        None => s,
    }
}

/// A string in single quotes, with `'` and `\` doubled and anything outside printable ASCII written as `\X2\`
/// (characters of the basic plane, four hex digits each) or `\X4\` (the others, eight hex digits each), as ISO 10303-21
/// asks. A run of one kind ends with `\X0\`.
pub fn string(s: &str) -> String {
    #[derive(PartialEq, Clone, Copy)]
    enum Mode {
        Plain,
        Basic,
        Wide,
    }
    let mut out = String::from("'");
    let mut mode = Mode::Plain;
    for c in s.chars() {
        let want = if (' '..='~').contains(&c) {
            Mode::Plain
        } else if (c as u32) <= 0xFFFF {
            Mode::Basic
        } else {
            Mode::Wide
        };
        if want != mode {
            if mode != Mode::Plain {
                out.push_str("\\X0\\");
            }
            match want {
                Mode::Basic => out.push_str("\\X2\\"),
                Mode::Wide => out.push_str("\\X4\\"),
                Mode::Plain => {}
            }
            mode = want;
        }
        match (want, c) {
            (Mode::Plain, '\'') => out.push_str("''"),
            (Mode::Plain, '\\') => out.push_str("\\\\"),
            (Mode::Plain, _) => out.push(c),
            (Mode::Basic, _) => out.push_str(&format!("{:04X}", c as u32)),
            (Mode::Wide, _) => out.push_str(&format!("{:08X}", c as u32)),
        }
    }
    if mode != Mode::Plain {
        out.push_str("\\X0\\");
    }
    out.push('\'');
    out
}

/// The entities of one file. `add` takes the text after `#n=` and returns `n`.
#[derive(Default)]
pub struct Step {
    lines: Vec<String>,
}

/// What `Out::gen_gid` writes where a GlobalId of a relationship, property set or other unnumbered thing goes. `derive_ids`
/// replaces it, once the whole file is known, by an id that comes from what the thing is.
pub const AUX_GID: &str = "'@@AUX@@'";

const NOT_ROOTED_HEADS: [&str; 7] = ["IFCPROPERTYSINGLEVALUE", "IFCQUANTITY", "IFCMATERIAL", "IFCPERSON", "IFCORGANIZATION", "IFCAPPLICATION", "IFCLABEL"];

/// The GlobalId an entity carries as its first attribute, when it has one that is already written.
fn rooted_guid(body: &str) -> Option<&str> {
    let open = body.find("('")?;
    let head = &body[..open];
    if !head.starts_with("IFC") || !head.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) || NOT_ROOTED_HEADS.iter().any(|n| head.starts_with(n)) { return None; }
    let id = body.get(open + 2..open + 24)?;
    (body.as_bytes().get(open + 24) == Some(&b'\'') && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$') && id != "@@AUX@@").then_some(id)
}

fn fnv64(text: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in text.bytes() { h ^= u64::from(b); h = h.wrapping_mul(0x0000_0100_0000_01b3); }
    h
}

impl Step {
    /// The text of entity `n` with every reference replaced by what the referenced entity is (its GlobalId, or, for one
    /// without, the same again for it), so that it does not depend on entity numbers.
    fn canonical(&self, n: usize, memo: &mut std::collections::HashMap<usize, String>, open: &mut Vec<usize>, skip: usize) -> String {
        let body = &self.lines[n - 1];
        if n == skip { return "@self".into(); }
        if let Some(g) = rooted_guid(body) { return g.to_string(); }
        if skip == 0 { if let Some(c) = memo.get(&n) { return c.clone(); } }
        if open.contains(&n) { return "~".into(); }
        open.push(n);
        let text = body.replace(AUX_GID, "@");
        let (mut out, mut in_str) = (String::new(), false);
        let b = text.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            if c == b'\'' { in_str = !in_str; }
            if c == b'#' && !in_str {
                let mut j = i + 1;
                while j < b.len() && b[j].is_ascii_digit() { j += 1; }
                if let Ok(m) = text[i + 1..j].parse::<usize>() {
                    if m >= 1 && m <= self.lines.len() { out.push_str(&self.canonical(m, memo, open, skip)); i = j; continue; }
                }
            }
            out.push(c as char);
            i += 1;
        }
        open.pop();
        let c = format!("{:016x}", fnv64(&out));
        if skip == 0 { memo.insert(n, c.clone()); }
        c
    }

    /// Gives every placeholder GlobalId its value: derived from the entity it belongs to and what that refers to, never
    /// from the order the entities were written in. An export of a model with something added keeps the ids of the
    /// relationships and property sets it had. Entities that are the same in everything (two equal property sets) are
    /// told apart by the order they come in.
    pub fn derive_ids(&mut self, project_key: u64) {
        let mut memo = std::collections::HashMap::new();
        let referrers = self.referrers();
        let mut seen: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
        for i in 0..self.lines.len() {
            if !self.lines[i].contains(AUX_GID) { continue; }
            let base = self.identity_of(i + 1, &mut memo, &referrers);
            let mut k = 0u32;
            while let Some(at) = self.lines[i].find(AUX_GID) {
                let mut h = fnv64(&format!("{base}#{k}"));
                let n = seen.entry(h).or_insert(0);
                if *n > 0 { h = fnv64(&format!("{h:x}/{n}")); }
                *seen.entry(h).or_insert(0) += 1;
                let id = format!("'{}'", crate::guid::global_id(project_key, 0xA000, h));
                self.lines[i].replace_range(at..at + AUX_GID.len(), &id);
                k += 1;
            }
        }
    }

    /// What an entity with a placeholder is: its own text and, because two property sets can read alike, what refers to
    /// it (a relationship names the element the set belongs to), each read with this entity left as "@self".
    fn identity_of(&self, n: usize, memo: &mut std::collections::HashMap<usize, String>, referrers: &std::collections::HashMap<usize, Vec<usize>>) -> String {
        let mut open = Vec::new();
        let mut text = self.canonical(n, memo, &mut open, 0);
        for &j in referrers.get(&n).map(Vec::as_slice).unwrap_or(&[]) {
            let mut open = Vec::new();
            text.push_str(&self.canonical(j, &mut std::collections::HashMap::new(), &mut open, n));
        }
        format!("{:016x}", fnv64(&text))
    }

    /// For every entity, the entities that refer to it.
    fn referrers(&self) -> std::collections::HashMap<usize, Vec<usize>> {
        let mut out: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
        for (j, l) in self.lines.iter().enumerate() {
            let (b, mut in_str, mut i) = (l.as_bytes(), false, 0);
            while i < b.len() {
                if b[i] == b'\'' { in_str = !in_str; }
                if b[i] == b'#' && !in_str {
                    let mut k = i + 1;
                    while k < b.len() && b[k].is_ascii_digit() { k += 1; }
                    if let Ok(m) = l[i + 1..k].parse::<usize>() { let v = out.entry(m).or_default(); if v.last() != Some(&(j + 1)) { v.push(j + 1); } }
                    i = k;
                    continue;
                }
                i += 1;
            }
        }
        out
    }

    pub fn add(&mut self, body: String) -> u32 {
        self.lines.push(body);
        self.lines.len() as u32
    }
    pub fn len(&self) -> usize {
        self.lines.len()
    }
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
    /// The whole file. `name` and `timestamp` go in the header; nothing else varies between runs.
    pub fn finish(&self, name: &str, timestamp: &str, x43: bool) -> String {
        let mut o = String::new();
        o.push_str("ISO-10303-21;\nHEADER;\n");
        o.push_str(if x43 { "FILE_DESCRIPTION(('ViewDefinition [ReferenceView]'),'2;1');\n" } else { "FILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');\n" });
        o.push_str(&format!("FILE_NAME({},{},(''),(''),'openbim','openbim','');\n", string(name), string(timestamp)));
        o.push_str(if x43 { "FILE_SCHEMA(('IFC4X3_ADD2'));\nENDSEC;\nDATA;\n" } else { "FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n" });
        for (i, l) in self.lines.iter().enumerate() {
            o.push_str(&format!("#{}={};\n", i + 1, l));
        }
        o.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        o
    }

    /// The whole file as IFC2X3. Every rooted entity (`IFCxxx('<22 character GlobalId>',$,...`) gets the owner history
    /// entity `oh`, which 2x3 requires and IFC4 does not.
    pub fn finish_2x3(&self, name: &str, timestamp: &str, oh: u32) -> String {
        let mut o = String::new();
        o.push_str("ISO-10303-21;\nHEADER;\n");
        o.push_str("FILE_DESCRIPTION(('ViewDefinition [CoordinationView_V2.0]'),'2;1');\n");
        o.push_str(&format!("FILE_NAME({},{},(''),(''),'openbim','openbim','');\n", string(name), string(timestamp)));
        o.push_str("FILE_SCHEMA(('IFC2X3'));\nENDSEC;\nDATA;\n");
        for (i, l) in self.lines.iter().enumerate() {
            o.push_str(&format!("#{}={};\n", i + 1, with_owner(l, oh)));
        }
        o.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        o
    }
}

/// `IFCxxx('<GlobalId>',$,rest` becomes `IFCxxx('<GlobalId>',#oh,rest`; other entities are as they were.
fn with_owner(line: &str, oh: u32) -> String {
    let Some(open) = line.find("('") else { return line.to_string() };
    let head = &line[..open];
    // entities whose first attribute is a name, not a GlobalId (a name of 22 characters must stay as it is)
    const NOT_ROOTED: [&str; 7] = ["IFCPROPERTYSINGLEVALUE", "IFCQUANTITY", "IFCMATERIAL", "IFCPERSON", "IFCORGANIZATION", "IFCAPPLICATION", "IFCLABEL"];
    if oh == 0 || !head.starts_with("IFC") || !head.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) || NOT_ROOTED.iter().any(|n| head.starts_with(n)) {
        return line.to_string();
    }
    let rest = &line[open + 2..];
    let (id, after) = match rest.get(..22) {
        Some(id) if !id.contains('\'') => (id, &rest[22..]),
        _ => return line.to_string(),
    };
    match after.strip_prefix("',$,") {
        Some(tail) => format!("{head}('{id}',#{oh},{tail}"),
        None => line.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::with_owner;

    #[test]
    fn only_rooted_entities_get_the_owner_history() {
        let gid = "018qLdYQi0000000400003";
        assert_eq!(gid.len(), 22);
        assert_eq!(with_owner(&format!("IFCWALL('{gid}',$,'W',$)"), 7), format!("IFCWALL('{gid}',#7,'W',$)"));
        assert_eq!(with_owner(&format!("IFCPROPERTYSET('{gid}',$,'P',$,(#1))"), 7), format!("IFCPROPERTYSET('{gid}',#7,'P',$,(#1))"));
        // a name that is 22 characters long is not a GlobalId
        let name = "ABCDEFGHIJKLMNOPQRSTUV";
        for line in [format!("IFCPROPERTYSINGLEVALUE('{name}',$,IFCLABEL('x'),$)"), format!("IFCQUANTITYLENGTH('{name}',$,$,1.)"), format!("IFCMATERIAL('{name}')")] {
            assert_eq!(with_owner(&line, 7), line);
        }
        // not an entity, no owner history asked for, a short name, a quote inside the first 22 characters, an owner already there
        for (line, oh) in [("#1".to_string(), 7), (format!("IFCWALL('{gid}',$,'W',$)"), 0), ("IFCWALL('short',$,'W',$)".to_string(), 7), ("IFCWALL('ab''cdefghijklmnopqrstuvwx',$)".to_string(), 7), (format!("IFCWALL('{gid}',#3,'W',$)"), 7), ("ifcwall('x')".to_string(), 7), ("NOTIFC('x')".to_string(), 7)] {
            assert_eq!(with_owner(&line, oh), line, "{line}");
        }
    }
}
