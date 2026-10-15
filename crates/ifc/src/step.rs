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

/// A string in single quotes, with `'` and `\` doubled and anything outside printable ASCII written as `\X2\`.
pub fn string(s: &str) -> String {
    let mut out = String::from("'");
    let mut wide = false;
    for c in s.chars() {
        let plain = (' '..='~').contains(&c);
        if plain && wide {
            out.push_str("\\X0\\");
            wide = false;
        }
        if plain {
            match c {
                '\'' => out.push_str("''"),
                '\\' => out.push_str("\\\\"),
                _ => out.push(c),
            }
        } else {
            if !wide {
                out.push_str("\\X2\\");
                wide = true;
            }
            let mut buf = [0u16; 2];
            for u in c.encode_utf16(&mut buf) {
                out.push_str(&format!("{u:04X}"));
            }
        }
    }
    if wide {
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

impl Step {
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
    pub fn finish(&self, name: &str, timestamp: &str) -> String {
        let mut o = String::new();
        o.push_str("ISO-10303-21;\nHEADER;\n");
        o.push_str("FILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');\n");
        o.push_str(&format!("FILE_NAME({},{},(''),(''),'openbim','openbim','');\n", string(name), string(timestamp)));
        o.push_str("FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n");
        for (i, l) in self.lines.iter().enumerate() {
            o.push_str(&format!("#{}={};\n", i + 1, l));
        }
        o.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        o
    }
}
