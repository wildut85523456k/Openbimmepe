//! Operation log spike (S8). No JSON on the data path: records are a small binary format with
//! a CRC-32 per record, so a torn tail (crash while writing) is detected and dropped, and a
//! damaged record in the middle stops replay with its position reported.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

/// A property value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Int(i64),
    Bool(bool),
    Text(String),
}

/// One element: a kind and its properties.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Element {
    pub kind: u32,
    pub props: BTreeMap<u32, Value>,
}

/// The model state: elements by id.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct State {
    pub elements: BTreeMap<u64, Element>,
}

/// A single change.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Create { id: u64, kind: u32 },
    Delete { id: u64 },
    /// Set (Some) or remove (None) one property.
    Set { id: u64, prop: u32, value: Option<Value> },
    /// Put back a whole element (the inverse of Delete).
    Restore { id: u64, element: Element },
}

#[derive(Clone, Debug, PartialEq)]
pub enum OpError {
    Exists(u64),
    Missing(u64),
    NotFinite(u64, u32),
    TooLong,
    /// more operations in one edit than the log can read back
    TooManyOps,
}

/// A group of ops applied or undone together.
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    pub seq: u64,
    pub author: u32,
    pub label: String,
    pub ops: Vec<Op>,
}

pub const MAX_TEXT: usize = 1 << 20;
pub const MAX_OPS: usize = 1 << 20;

impl State {
    /// Apply one op and return its inverse. On error the state is unchanged.
    pub fn apply(&mut self, op: &Op) -> Result<Op, OpError> {
        match op {
            Op::Create { id, kind } => {
                if self.elements.contains_key(id) {
                    return Err(OpError::Exists(*id));
                }
                self.elements.insert(*id, Element { kind: *kind, props: BTreeMap::new() });
                Ok(Op::Delete { id: *id })
            }
            Op::Delete { id } => {
                let e = self.elements.remove(id).ok_or(OpError::Missing(*id))?;
                Ok(Op::Restore { id: *id, element: e })
            }
            Op::Restore { id, element } => {
                if self.elements.contains_key(id) {
                    return Err(OpError::Exists(*id));
                }
                self.elements.insert(*id, element.clone());
                Ok(Op::Delete { id: *id })
            }
            Op::Set { id, prop, value } => {
                if let Some(Value::Text(t)) = value {
                    if t.len() > MAX_TEXT {
                        return Err(OpError::TooLong);
                    }
                }
                if let Some(Value::Num(x)) = value {
                    if !x.is_finite() {
                        return Err(OpError::NotFinite(*id, *prop));
                    }
                }
                let e = self.elements.get_mut(id).ok_or(OpError::Missing(*id))?;
                let old = match value {
                    Some(v) => e.props.insert(*prop, v.clone()),
                    None => e.props.remove(prop),
                };
                Ok(Op::Set { id: *id, prop: *prop, value: old })
            }
        }
    }

    /// Apply a list of ops all-or-nothing; returns the inverse list (already in undo order).
    pub fn apply_all(&mut self, ops: &[Op]) -> Result<Vec<Op>, OpError> {
        let mut inv = Vec::with_capacity(ops.len());
        for op in ops {
            match self.apply(op) {
                Ok(i) => inv.push(i),
                Err(e) => {
                    for i in inv.iter().rev() {
                        // inverses of ops that just succeeded cannot fail
                        let _ = self.apply(i);
                    }
                    return Err(e);
                }
            }
        }
        inv.reverse();
        Ok(inv)
    }

    /// Order-independent-of-history fingerprint (FNV-1a over a canonical encoding).
    pub fn fingerprint(&self) -> u64 {
        let mut b = Vec::new();
        encode_state(self, &mut b);
        let mut h: u64 = 0xcbf29ce484222325;
        for x in b {
            h ^= x as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
}

// ---------- binary encoding ----------

fn put_u32(b: &mut Vec<u8>, x: u32) {
    b.extend_from_slice(&x.to_le_bytes());
}
fn put_u64(b: &mut Vec<u8>, x: u64) {
    b.extend_from_slice(&x.to_le_bytes());
}
fn put_str(b: &mut Vec<u8>, s: &str) {
    put_u32(b, s.len() as u32);
    b.extend_from_slice(s.as_bytes());
}
fn put_value(b: &mut Vec<u8>, v: &Value) {
    match v {
        Value::Num(x) => {
            b.push(0);
            put_u64(b, x.to_bits());
        }
        Value::Int(x) => {
            b.push(1);
            put_u64(b, *x as u64);
        }
        Value::Bool(x) => {
            b.push(2);
            b.push(*x as u8);
        }
        Value::Text(s) => {
            b.push(3);
            put_str(b, s);
        }
    }
}
fn put_element(b: &mut Vec<u8>, e: &Element) {
    put_u32(b, e.kind);
    put_u32(b, e.props.len() as u32);
    for (p, v) in &e.props {
        put_u32(b, *p);
        put_value(b, v);
    }
}
fn put_op(b: &mut Vec<u8>, op: &Op) {
    match op {
        Op::Create { id, kind } => {
            b.push(0);
            put_u64(b, *id);
            put_u32(b, *kind);
        }
        Op::Delete { id } => {
            b.push(1);
            put_u64(b, *id);
        }
        Op::Set { id, prop, value } => {
            b.push(2);
            put_u64(b, *id);
            put_u32(b, *prop);
            match value {
                Some(v) => {
                    b.push(1);
                    put_value(b, v);
                }
                None => b.push(0),
            }
        }
        Op::Restore { id, element } => {
            b.push(3);
            put_u64(b, *id);
            put_element(b, element);
        }
    }
}

/// Why a decode stopped.
#[derive(Clone, Debug, PartialEq)]
pub enum DecodeError {
    Short,
    BadTag(u8),
    BadUtf8,
    TooBig,
}

struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.b.len() - self.i < n {
            return Err(DecodeError::Short);
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn str(&mut self) -> Result<String, DecodeError> {
        let n = self.u32()? as usize;
        if n > MAX_TEXT {
            return Err(DecodeError::TooBig);
        }
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| DecodeError::BadUtf8)
    }
    fn value(&mut self) -> Result<Value, DecodeError> {
        Ok(match self.u8()? {
            0 => Value::Num(f64::from_bits(self.u64()?)),
            1 => Value::Int(self.u64()? as i64),
            2 => Value::Bool(self.u8()? != 0),
            3 => Value::Text(self.str()?),
            t => return Err(DecodeError::BadTag(t)),
        })
    }
    fn element(&mut self) -> Result<Element, DecodeError> {
        let kind = self.u32()?;
        let n = self.u32()? as usize;
        if n > MAX_OPS {
            return Err(DecodeError::TooBig);
        }
        let mut props = BTreeMap::new();
        for _ in 0..n {
            let p = self.u32()?;
            props.insert(p, self.value()?);
        }
        Ok(Element { kind, props })
    }
    fn op(&mut self) -> Result<Op, DecodeError> {
        Ok(match self.u8()? {
            0 => Op::Create { id: self.u64()?, kind: self.u32()? },
            1 => Op::Delete { id: self.u64()? },
            2 => {
                let id = self.u64()?;
                let prop = self.u32()?;
                let value = match self.u8()? {
                    0 => None,
                    1 => Some(self.value()?),
                    t => return Err(DecodeError::BadTag(t)),
                };
                Op::Set { id, prop, value }
            }
            3 => Op::Restore { id: self.u64()?, element: self.element()? },
            t => return Err(DecodeError::BadTag(t)),
        })
    }
}

fn encode_state(s: &State, b: &mut Vec<u8>) {
    put_u64(b, s.elements.len() as u64);
    for (id, e) in &s.elements {
        put_u64(b, *id);
        put_element(b, e);
    }
}

/// CRC-32 (IEEE), bitwise; small and dependency-free.
pub fn crc32(data: &[u8]) -> u32 {
    let mut c: u32 = !0;
    for &x in data {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB88320 } else { c >> 1 };
        }
    }
    !c
}

fn encode_tx(t: &Transaction) -> Vec<u8> {
    let mut p = Vec::new();
    put_u64(&mut p, t.seq);
    put_u32(&mut p, t.author);
    put_str(&mut p, &t.label);
    put_u32(&mut p, t.ops.len() as u32);
    for op in &t.ops {
        put_op(&mut p, op);
    }
    p
}

fn decode_tx(p: &[u8]) -> Result<Transaction, DecodeError> {
    let mut r = Rd { b: p, i: 0 };
    let seq = r.u64()?;
    let author = r.u32()?;
    let label = r.str()?;
    let n = r.u32()? as usize;
    if n > MAX_OPS {
        return Err(DecodeError::TooBig);
    }
    let mut ops = Vec::new();
    for _ in 0..n {
        ops.push(r.op()?);
    }
    if r.i != p.len() {
        return Err(DecodeError::Short);
    }
    Ok(Transaction { seq, author, label, ops })
}

/// Frame: magic(2) | len u32 | crc u32 | payload.
const MAGIC: [u8; 2] = [0x0B, 0x1A];
const HEAD: usize = 10;

pub fn frame(t: &Transaction) -> Vec<u8> {
    let p = encode_tx(t);
    let mut f = Vec::with_capacity(HEAD + p.len());
    f.extend_from_slice(&MAGIC);
    put_u32(&mut f, p.len() as u32);
    put_u32(&mut f, crc32(&p));
    f.extend_from_slice(&p);
    f
}

/// How a replay ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Tail {
    /// Every byte was a complete, valid record.
    Clean,
    /// The last record was cut short (crash during write): dropped.
    Torn { at: usize },
    /// A record failed its check or did not decode: replay stopped there.
    Damaged { at: usize },
}

/// Read as many valid records as possible.
pub fn read_log(bytes: &[u8]) -> (Vec<Transaction>, Tail) {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        if rest.len() < HEAD {
            return (out, if rest.iter().zip(MAGIC.iter()).all(|(a, b)| a == b) { Tail::Torn { at: i } } else { Tail::Damaged { at: i } });
        }
        if rest[..2] != MAGIC {
            return (out, Tail::Damaged { at: i });
        }
        let len = u32::from_le_bytes(rest[2..6].try_into().unwrap()) as usize;
        let crc = u32::from_le_bytes(rest[6..10].try_into().unwrap());
        if rest.len() - HEAD < len {
            // length field itself could be damaged; a torn write is only plausible when
            // the claimed length is sane
            return (out, if len <= (1 << 28) { Tail::Torn { at: i } } else { Tail::Damaged { at: i } });
        }
        let payload = &rest[HEAD..HEAD + len];
        if crc32(payload) != crc {
            return (out, Tail::Damaged { at: i });
        }
        match decode_tx(payload) {
            Ok(t) => out.push(t),
            Err(_) => return (out, Tail::Damaged { at: i }),
        }
        i += HEAD + len;
    }
    (out, Tail::Clean)
}

/// Replay transactions on a state.
pub fn replay(start: State, txs: &[Transaction]) -> Result<State, (u64, OpError)> {
    let mut s = start;
    for t in txs {
        s.apply_all(&t.ops).map_err(|e| (t.seq, e))?;
    }
    Ok(s)
}

/// Snapshot: seq of the last applied transaction + the full state, with a checksum.
pub fn snapshot(seq: u64, s: &State) -> Vec<u8> {
    let mut p = Vec::new();
    put_u64(&mut p, seq);
    encode_state(s, &mut p);
    let mut f = Vec::new();
    f.extend_from_slice(&[0x5A, 0x1B]);
    put_u32(&mut f, p.len() as u32);
    put_u32(&mut f, crc32(&p));
    f.extend_from_slice(&p);
    f
}

pub fn load_snapshot(b: &[u8]) -> Option<(u64, State)> {
    if b.len() < HEAD || b[..2] != [0x5A, 0x1B] {
        return None;
    }
    let len = u32::from_le_bytes(b[2..6].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(b[6..10].try_into().unwrap());
    if b.len() - HEAD != len || crc32(&b[HEAD..]) != crc {
        return None;
    }
    let mut r = Rd { b: &b[HEAD..], i: 0 };
    let seq = r.u64().ok()?;
    let n = r.u64().ok()? as usize;
    let mut s = State::default();
    for _ in 0..n {
        let id = r.u64().ok()?;
        let e = r.element().ok()?;
        s.elements.insert(id, e);
    }
    if r.i != r.b.len() {
        return None;
    }
    Some((seq, s))
}

// ---------- document with undo/redo ----------

/// A live document: state, the append-only log bytes, and undo/redo stacks.
/// Undo and redo are themselves logged transactions, so the log only ever grows.
#[derive(Default)]
pub struct Doc {
    pub state: State,
    pub log: Vec<u8>,
    next_seq: u64,
    next_id: u32,
    author: u32,
    undo: Vec<Vec<Op>>, // inverse op lists, most recent last
    redo: Vec<Vec<Op>>,
}

impl Doc {
    pub fn new(author: u32) -> Self {
        Doc { author, next_seq: 1, ..Default::default() }
    }

    fn commit(&mut self, label: &str, ops: Vec<Op>) {
        let t = Transaction { seq: self.next_seq, author: self.author, label: label.to_string(), ops };
        self.next_seq += 1;
        self.log.extend_from_slice(&frame(&t));
    }

    /// A fresh element number that cannot collide with another author's: the author number is the high half.
    /// Numbers already present (for example typed by hand) are skipped.
    pub fn new_id(&mut self) -> u64 {
        loop {
            self.next_id = self.next_id.wrapping_add(1);
            let id = (u64::from(self.author) << 32) | u64::from(self.next_id);
            if !self.state.elements.contains_key(&id) { return id; }
        }
    }

    /// Run one edit. Nothing is logged if it fails. The same size limits as the reader apply, so what is written
    /// can always be read back.
    pub fn edit(&mut self, label: &str, ops: Vec<Op>) -> Result<(), OpError> {
        if label.len() > MAX_TEXT { return Err(OpError::TooLong); }
        if ops.len() > MAX_OPS { return Err(OpError::TooManyOps); }
        let inv = self.state.apply_all(&ops)?;
        self.commit(label, ops);
        self.undo.push(inv);
        self.redo.clear();
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(inv) = self.undo.pop() else { return false };
        let redo = self.state.apply_all(&inv).expect("inverse of an applied edit always applies");
        self.commit("undo", inv);
        self.redo.push(redo);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(ops) = self.redo.pop() else { return false };
        let inv = self.state.apply_all(&ops).expect("redo of an undone edit always applies");
        self.commit("redo", ops);
        self.undo.push(inv);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

// ---------- merge ----------

/// What a change touches: a whole element, or one property of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Element(u64),
    Prop(u64, u32),
}

fn keys_of(op: &Op) -> Vec<Key> {
    match op {
        Op::Create { id, .. } | Op::Delete { id } | Op::Restore { id, .. } => vec![Key::Element(*id)],
        Op::Set { id, prop, .. } => vec![Key::Prop(*id, *prop)],
    }
}

fn clashes(a: &BTreeSet<Key>, k: Key) -> bool {
    match k {
        Key::Element(id) => a.iter().any(|x| matches!(x, Key::Element(i) | Key::Prop(i, _) if *i == id)),
        Key::Prop(id, p) => a.contains(&Key::Prop(id, p)) || a.contains(&Key::Element(id)),
    }
}

/// Result of merging two branches that started from the same base.
#[derive(Debug, PartialEq)]
pub struct Merged {
    pub state: State,
    /// Transactions of B (by seq) that were left out whole, with the first thing that clashed.
    pub conflicts: Vec<(u64, Key)>,
}

/// Apply A fully, then each transaction of B as a whole: if any of its ops touches something A touched, or
/// something an earlier skipped transaction of B touched, the whole transaction is left out and reported (A wins).
/// A transaction is never applied in part. The result does not depend on the order of two non-clashing ops.
pub fn merge(base: &State, a: &[Transaction], b: &[Transaction]) -> Result<Merged, (u64, OpError)> {
    let mut s = replay(base.clone(), a)?;
    let mut touched = BTreeSet::new();
    for t in a {
        for op in &t.ops {
            touched.extend(keys_of(op));
        }
    }
    let mut dropped = BTreeSet::new();
    let mut conflicts = Vec::new();
    for t in b {
        let keys: Vec<Key> = t.ops.iter().flat_map(keys_of).collect();
        let bad = keys.iter().find(|k| clashes(&touched, **k)).or_else(|| keys.iter().find(|k| clashes(&dropped, **k)));
        if let Some(k) = bad {
            conflicts.push((t.seq, *k));
            dropped.extend(keys);
            continue;
        }
        s.apply_all(&t.ops).map_err(|e| (t.seq, e))?;
    }
    Ok(Merged { state: s, conflicts })
}
