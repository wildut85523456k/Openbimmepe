//! A reader for STEP physical files (ISO 10303-21), the text format of IFC: every entity of the DATA section as a
//! class name and a tree of values. It knows nothing about IFC; `reader.rs` reads the model out of it.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `$`
    Null,
    /// `*`
    Derived,
    Int(i64),
    Real(f64),
    Str(String),
    /// `.NAME.`, without the dots, upper case as written
    Enum(String),
    Ref(u32),
    List(Vec<Value>),
    /// a value with its type, e.g. `IFCLABEL('x')`
    Typed(String, Vec<Value>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    /// upper case, e.g. `IFCWALL`
    pub class: String,
    pub args: Vec<Value>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepFile {
    /// the first name of FILE_SCHEMA, upper case, e.g. `IFC4X3_ADD2`
    pub schema: String,
    pub entities: BTreeMap<u32, Entity>,
    /// entities written as `#n=(A()B())` (complex entities), which this reader leaves out
    pub complex: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// the text does not start with `ISO-10303-21;`
    NotStep,
    /// no DATA section
    NoData,
    /// a statement that does not parse: the byte offset where it went wrong
    Syntax(usize),
    /// A required section or file terminator is missing.
    Truncated(usize),
    /// Valid STEP syntax with invalid or unsupported IFC content.
    Invalid(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::NotStep => write!(f, "this is not a STEP (IFC) file"),
            ReadError::NoData => write!(f, "the file has no DATA section"),
            ReadError::Syntax(at) => write!(f, "the file cannot be read at byte {at}"),
            ReadError::Truncated(at) => write!(f, "truncated STEP file at byte {at}"),
            ReadError::Invalid(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for ReadError {}

/// Nesting deeper than this is not an IFC file.
const MAX_DEPTH: usize = 64;

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        loop {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"/*") {
                match find(&self.s[self.i + 2..], b"*/") {
                    Some(k) => self.i += k + 4,
                    // Leave an unterminated comment visible to the parser as invalid input.
                    None => break,
                }
            } else {
                break;
            }
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn err<T>(&self) -> Result<T, ReadError> {
        Err(ReadError::Syntax(self.i))
    }

    fn value(&mut self, depth: usize) -> Result<Value, ReadError> {
        if depth > MAX_DEPTH {
            return self.err();
        }
        let Some(c) = self.peek() else {
            return self.err();
        };
        match c {
            b'$' => {
                self.i += 1;
                Ok(Value::Null)
            }
            b'*' => {
                self.i += 1;
                Ok(Value::Derived)
            }
            b'#' => {
                self.i += 1;
                Ok(Value::Ref(self.id()?))
            }
            b'\'' => self.string().map(Value::Str),
            b'.' => {
                self.i += 1;
                let start = self.i;
                while self.i < self.s.len() && self.s[self.i] != b'.' {
                    self.i += 1;
                }
                if self.i >= self.s.len() {
                    return self.err();
                }
                let name = String::from_utf8_lossy(&self.s[start..self.i]).to_ascii_uppercase();
                self.i += 1;
                Ok(Value::Enum(name))
            }
            b'(' => Ok(Value::List(self.list(depth)?)),
            b'-' | b'+' | b'0'..=b'9' => self.number(),
            c if c.is_ascii_alphabetic() => {
                let start = self.i;
                while self.i < self.s.len()
                    && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_')
                {
                    self.i += 1;
                }
                let name = String::from_utf8_lossy(&self.s[start..self.i]).to_ascii_uppercase();
                if self.peek() != Some(b'(') {
                    return self.err();
                }
                Ok(Value::Typed(name, self.list(depth)?))
            }
            _ => self.err(),
        }
    }

    fn list(&mut self, depth: usize) -> Result<Vec<Value>, ReadError> {
        self.i += 1; // (
        let mut out = Vec::new();
        if self.peek() == Some(b')') {
            self.i += 1;
            return Ok(out);
        }
        loop {
            out.push(self.value(depth + 1)?);
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b')') => {
                    self.i += 1;
                    return Ok(out);
                }
                _ => return self.err(),
            }
        }
    }

    fn digits(&mut self) -> Result<u64, ReadError> {
        let start = self.i;
        while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i])
            .ok()
            .and_then(|t| t.parse().ok())
            .map_or_else(|| self.err(), Ok)
    }

    fn id(&mut self) -> Result<u32, ReadError> {
        let n = self.digits()?;
        u32::try_from(n)
            .ok()
            .filter(|n| *n != 0)
            .map_or_else(|| self.err(), Ok)
    }

    fn number(&mut self) -> Result<Value, ReadError> {
        let start = self.i;
        self.i += 1;
        let mut real = false;
        while self.i < self.s.len() {
            match self.s[self.i] {
                b'0'..=b'9' => {}
                b'.' | b'e' | b'E' => real = true,
                b'+' | b'-' if matches!(self.s[self.i - 1], b'e' | b'E') => {}
                _ => break,
            }
            self.i += 1;
        }
        let t = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
        if real {
            t.parse().map(Value::Real).or_else(|_| self.err())
        } else {
            t.parse().map(Value::Int).or_else(|_| self.err())
        }
    }

    /// A string in single quotes, `''` for a quote, with the `\X2\....\X0\`, `\X\hh` and `\S\c` escapes of STEP decoded.
    fn string(&mut self) -> Result<String, ReadError> {
        self.i += 1;
        let mut raw = Vec::new();
        loop {
            match self.s.get(self.i) {
                None => return self.err(),
                Some(b'\'') => {
                    if self.s.get(self.i + 1) == Some(&b'\'') {
                        raw.push(b'\'');
                        self.i += 2;
                    } else {
                        self.i += 1;
                        break;
                    }
                }
                Some(&c) => {
                    raw.push(c);
                    self.i += 1;
                }
            }
        }
        decode_escapes(&raw).map_err(|()| ReadError::Syntax(self.i))
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn decode_escapes(raw: &[u8]) -> Result<String, ()> {
    let text = std::str::from_utf8(raw).map_err(|_| ())?;
    if !text.contains('\\') {
        return Ok(text.to_owned());
    }
    let b = text.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && b[i..].starts_with(b"\\X2\\") {
            let end = i + 4 + find(&b[i + 4..], b"\\X0\\").ok_or(())?;
            let hex = &b[i + 4..end];
            if hex.len() % 4 != 0 {
                return Err(());
            }
            let units = hex
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| hex_unit(bytes))
                .collect::<Result<Vec<_>, _>>()?;
            out.push_str(&String::from_utf16(&units).map_err(|_| ())?);
            i = end + 4;
        } else if b[i..].starts_with(b"\\X\\") {
            let hex = b.get(i + 3..i + 5).ok_or(())?;
            out.push(char::from(u8::try_from(hex_unit(hex)?).map_err(|_| ())?));
            i += 5;
        } else if b[i..].starts_with(b"\\S\\") {
            let c = *b.get(i + 3).filter(|c| c.is_ascii()).ok_or(())?;
            out.push(char::from(c + 128));
            i += 4;
        } else {
            let ch = text[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    Ok(out)
}

fn hex_unit(bytes: &[u8]) -> Result<u16, ()> {
    bytes.iter().try_fold(0u16, |n, b| {
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => return Err(()),
        };
        Ok(n * 16 + u16::from(digit))
    })
}

/// Reads a whole STEP file.
pub fn parse(text: &str) -> Result<StepFile, ReadError> {
    let bytes = text.as_bytes();
    let mut head = P { s: bytes, i: 0 };
    head.ws();
    if !bytes[head.i..].starts_with(b"ISO-10303-21;") {
        return Err(ReadError::NotStep);
    }
    head.i += b"ISO-10303-21;".len();
    head.ws();
    let mut file = StepFile::default();
    // Parse statements instead of searching raw bytes: DATA/ENDSEC inside strings
    // or comments must not masquerade as section delimiters. Headerless DATA is
    // retained for the low-level STEP fragment reader.
    if bytes[head.i..].starts_with(b"HEADER;") {
        head.i += b"HEADER;".len();
        loop {
            head.ws();
            if bytes[head.i..].starts_with(b"ENDSEC;") {
                head.i += b"ENDSEC;".len();
                break;
            }
            if head.i == bytes.len() {
                return Err(ReadError::Truncated(head.i));
            }
            let Value::Typed(name, args) = head.value(0)? else {
                return head.err();
            };
            if name == "FILE_SCHEMA" {
                if let Some(Value::List(names)) = args.first() {
                    if let Some(Value::Str(s)) = names.first() {
                        file.schema = s.to_ascii_uppercase();
                    }
                }
            }
            if head.peek() != Some(b';') {
                return head.err();
            }
            head.i += 1;
        }
    }
    head.ws();
    if !bytes[head.i..].starts_with(b"DATA;") {
        return Err(ReadError::NoData);
    }
    let mut p = P {
        s: bytes,
        i: head.i + 5,
    };
    let mut ids = BTreeSet::new();
    loop {
        match p.peek() {
            None => return Err(ReadError::Truncated(p.i)),
            Some(b'#') => {
                p.i += 1;
                let id = p.id()?;
                if !ids.insert(id) {
                    return p.err();
                }
                if p.peek() != Some(b'=') {
                    return p.err();
                }
                p.i += 1;
                match p.peek() {
                    Some(b'(') => {
                        // Complex entities remain unsupported, but their syntax and
                        // references must pass the same safety checks as simple ones.
                        p.i += 1;
                        let mut count = 0;
                        while p.peek() != Some(b')') {
                            if !matches!(p.value(0)?, Value::Typed(_, _)) {
                                return p.err();
                            }
                            count += 1;
                        }
                        if count == 0 {
                            return p.err();
                        }
                        p.i += 1;
                        file.complex += 1;
                    }
                    Some(c) if c.is_ascii_alphabetic() => {
                        let start = p.i;
                        while p.i < bytes.len()
                            && (bytes[p.i].is_ascii_alphanumeric() || bytes[p.i] == b'_')
                        {
                            p.i += 1;
                        }
                        let class =
                            String::from_utf8_lossy(&bytes[start..p.i]).to_ascii_uppercase();
                        if p.peek() != Some(b'(') {
                            return p.err();
                        }
                        let args = p.list(0)?;
                        file.entities.insert(id, Entity { class, args });
                    }
                    _ => return p.err(),
                }
                if p.peek() != Some(b';') {
                    return p.err();
                }
                p.i += 1;
            }
            Some(b'E') => {
                for token in [b"ENDSEC;".as_slice(), b"END-ISO-10303-21;".as_slice()] {
                    p.ws();
                    if !bytes[p.i..].starts_with(token) {
                        return Err(ReadError::Truncated(p.i));
                    }
                    p.i += token.len();
                }
                if p.peek().is_some() {
                    return p.err();
                }
                break;
            }
            _ => return p.err(),
        }
    }
    Ok(file)
}

impl StepFile {
    pub fn get(&self, id: u32) -> Option<&Entity> {
        self.entities.get(&id)
    }
    /// The entity a reference value points to.
    pub fn deref(&self, v: &Value) -> Option<&Entity> {
        if let Value::Ref(n) = v {
            self.get(*n)
        } else {
            None
        }
    }
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Real(x) => Some(*x),
            Value::Int(i) => Some(*i as f64),
            Value::Typed(_, v) => v.first().and_then(Value::as_f64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Value::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_ref(&self) -> Option<u32> {
        if let Value::Ref(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    pub fn as_list(&self) -> &[Value] {
        if let Value::List(v) = self {
            v
        } else {
            &[]
        }
    }
    pub fn as_enum(&self) -> Option<&str> {
        if let Value::Enum(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_file_reads_values_of_every_kind() {
        let f = parse("ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('ifc4x3_add2'));\nENDSEC;\nDATA;\n/* c */ #1=IFCWALL('a''b;',$,*,.STANDARD.,(#2,#3),-1.5E-3,7,IFCLABEL('x'));\n#2=IFCDIRECTION((0.,0.,1.0));\nENDSEC;\nEND-ISO-10303-21;\n").unwrap();
        assert_eq!(f.schema, "IFC4X3_ADD2");
        let w = f.get(1).unwrap();
        assert_eq!(w.class, "IFCWALL");
        assert_eq!(w.args[0], Value::Str("a'b;".into()));
        assert_eq!(
            w.args[1..4],
            [Value::Null, Value::Derived, Value::Enum("STANDARD".into())]
        );
        assert_eq!(w.args[4], Value::List(vec![Value::Ref(2), Value::Ref(3)]));
        assert_eq!(w.args[5], Value::Real(-0.0015));
        assert_eq!(w.args[6], Value::Int(7));
        assert_eq!(
            w.args[7],
            Value::Typed("IFCLABEL".into(), vec![Value::Str("x".into())])
        );
        assert_eq!(f.get(2).unwrap().args[0].as_list().len(), 3);
    }

    #[test]
    fn unicode_escapes_and_complex_entities_and_bad_files() {
        let f = parse("ISO-10303-21;\nDATA;\n#1=IFCLABEL('Caf\\X2\\00E9\\X0\\ \\X\\E9');\n#2=(IFCA()IFCB());\nENDSEC;\nEND-ISO-10303-21;").unwrap();
        assert_eq!(f.get(1).unwrap().args[0].as_str(), Some("Café é"));
        assert_eq!((f.entities.len(), f.complex), (1, 1));
        assert_eq!(parse("hello"), Err(ReadError::NotStep));
        assert_eq!(
            parse("ISO-10303-21;\nHEADER;\nENDSEC;"),
            Err(ReadError::NoData)
        );
        assert!(matches!(
            parse("ISO-10303-21;\nDATA;\n#1=IFCX('open;\n"),
            Err(ReadError::Syntax(_))
        ));
        // nesting that deep is refused, not overflowed
        let deep = format!(
            "ISO-10303-21;\nDATA;\n#1=IFCX({}1{});",
            "(".repeat(5000),
            ")".repeat(5000)
        );
        assert!(matches!(parse(&deep), Err(ReadError::Syntax(_))));
    }
}
