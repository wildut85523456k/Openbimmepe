//! The storage model of the operation log (decision of 23 Oct 2026, `docs/decisions/0001-storage-model.md`).
//!
//! The model is still written as the text records of the `.obim` file, and that text stays the one description of the
//! format. The log keeps those records as its state: one log element per record, whose property 0 is the record line
//! and property 1 its place in the file. A record with an `id=` field is identified by its tag and id, so an edited
//! element is one `Set`; any other record (a link, an end point, a setting) is identified by its own text, so a change
//! is a `Delete` and a `Create`. A transaction is the difference between two states, so what is appended is what
//! changed, in a form that survives a torn write (CRC per record) and can be merged by record.
use openbim_oplog::{Element, Op, State, Transaction, Value};

const LINE: u32 = 0;
const PLACE: u32 = 1;

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bytes { h ^= u64::from(*b); h = h.wrapping_mul(0x0000_0100_0000_01b3); }
    h
}

/// The identity of a record: tag and id when it has one, else its text.
fn key_of(line: &str) -> u64 {
    let tag = line.split('|').next().unwrap_or("");
    match line.split('|').find_map(|f| f.strip_prefix("id=")) {
        Some(id) => fnv(format!("{tag}#{id}").as_bytes()),
        None => fnv(line.as_bytes()),
    }
}

/// The records of a model as a log state.
pub(crate) fn state_of(lines: &[String]) -> State {
    let mut s = State::default();
    for (i, line) in lines.iter().enumerate() {
        let mut key = key_of(line);
        while s.elements.contains_key(&key) { key = key.wrapping_add(1); } // a collision or a repeated line keeps both
        let mut e = Element { kind: u32::from(line.as_bytes().first().copied().unwrap_or(0)), ..Default::default() };
        e.props.insert(LINE, Value::Text(line.clone()));
        e.props.insert(PLACE, Value::Int(i as i64));
        s.elements.insert(key, e);
    }
    s
}

/// The records back, in file order.
pub(crate) fn lines_of(s: &State) -> Vec<String> {
    let mut v: Vec<(i64, &str)> = s.elements.values().filter_map(|e| match (e.props.get(&LINE), e.props.get(&PLACE)) {
        (Some(Value::Text(t)), Some(Value::Int(p))) => Some((*p, t.as_str())),
        _ => None,
    }).collect();
    v.sort_by_key(|(p, _)| *p);
    v.into_iter().map(|(_, t)| t.to_string()).collect()
}

/// What turns `old` into `new`: nothing when they are the same.
pub(crate) fn diff(old: &State, new: &State) -> Vec<Op> {
    let mut ops = Vec::new();
    for id in old.elements.keys() { if !new.elements.contains_key(id) { ops.push(Op::Delete { id: *id }); } }
    for (id, e) in &new.elements {
        match old.elements.get(id) {
            None => {
                ops.push(Op::Create { id: *id, kind: e.kind });
                for (p, v) in &e.props { ops.push(Op::Set { id: *id, prop: *p, value: Some(v.clone()) }); }
            }
            Some(o) if o != e => for (p, v) in &e.props { if o.props.get(p) != Some(v) { ops.push(Op::Set { id: *id, prop: *p, value: Some(v.clone()) }); } },
            Some(_) => {}
        }
    }
    ops
}

const HEAD: usize = 10;

/// A journal file: one snapshot, then the transactions after it.
pub(crate) fn base_bytes(seq: u64, s: &State) -> Vec<u8> { openbim_oplog::snapshot(seq, s) }

pub(crate) fn tx_bytes(seq: u64, label: &str, ops: Vec<Op>) -> Vec<u8> {
    openbim_oplog::frame(&Transaction { seq, author: 0, label: label.to_string(), ops })
}

/// How a journal was read.
pub(crate) struct Read { pub state: State, pub seq: u64, pub transactions: usize, pub complete: bool }

/// Reads a journal: the snapshot, then every valid transaction after it. A torn last record is dropped (the usual
/// end of a crash); a damaged one stops the replay there and `complete` says the end was not clean.
pub(crate) fn read(bytes: &[u8]) -> Result<Read, String> {
    if bytes.len() < HEAD { return Err("the journal is empty".into()); }
    let len = u32::from_le_bytes(bytes[2..6].try_into().map_err(|_| "short header")?) as usize;
    let end = HEAD.checked_add(len).filter(|e| *e <= bytes.len()).ok_or("the base of the journal is cut short")?;
    let (mut seq, mut state) = openbim_oplog::load_snapshot(&bytes[..end]).ok_or("the base of the journal does not read")?;
    let (txs, tail) = openbim_oplog::read_log(&bytes[end..]);
    for t in &txs {
        state.apply_all(&t.ops).map_err(|e| format!("transaction {} does not apply: {e:?}", t.seq))?;
        seq = t.seq;
    }
    Ok(Read { state, seq, transactions: txs.len(), complete: matches!(tail, openbim_oplog::Tail::Clean | openbim_oplog::Tail::Torn { .. }) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn records_go_into_the_log_state_and_come_back_in_order() {
        let l = lines(&["F|1", "E|id=3|name=a", "E|id=2|name=b", "L|x=1", "L|x=1"]);
        assert_eq!(lines_of(&state_of(&l)), l, "order, ids that are not sorted and a repeated line all survive");
    }

    #[test]
    fn an_edited_element_is_one_set_and_an_unchanged_model_is_no_transaction() {
        let a = state_of(&lines(&["F|1", "E|id=3|name=a", "E|id=2|name=b", "L|x=1"]));
        assert!(diff(&a, &a).is_empty());
        let b = state_of(&lines(&["F|1", "E|id=3|name=A", "E|id=2|name=b", "L|x=1"]));
        let ops = diff(&a, &b);
        assert_eq!(ops.len(), 1, "{ops:?}");
        assert!(matches!(&ops[0], Op::Set { prop: 0, value: Some(Value::Text(t)), .. } if t == "E|id=3|name=A"));
        // a record without an id is replaced: deleted and created
        let c = state_of(&lines(&["F|1", "E|id=3|name=A", "E|id=2|name=b", "L|x=2"]));
        let ops = diff(&b, &c);
        assert!(ops.iter().any(|o| matches!(o, Op::Delete { .. })) && ops.iter().any(|o| matches!(o, Op::Create { .. })));
        // applying the difference gives the new state
        let mut s = b.clone();
        s.apply_all(&ops).unwrap();
        assert_eq!(lines_of(&s), lines_of(&c));
    }

    #[test]
    fn a_journal_reads_back_after_a_torn_write_and_stops_at_a_damaged_one() {
        let s0 = state_of(&lines(&["F|1", "E|id=1|name=a"]));
        let s1 = state_of(&lines(&["F|1", "E|id=1|name=a", "E|id=2|name=b"]));
        let s2 = state_of(&lines(&["F|1", "E|id=1|name=z", "E|id=2|name=b"]));
        let mut bytes = base_bytes(0, &s0);
        bytes.extend(tx_bytes(1, "autosave", diff(&s0, &s1)));
        let good = bytes.len();
        bytes.extend(tx_bytes(2, "autosave", diff(&s1, &s2)));
        let full = read(&bytes).unwrap();
        assert_eq!((full.seq, full.transactions, full.complete), (2, 2, true));
        assert_eq!(lines_of(&full.state), lines_of(&s2));
        // the process stopped in the middle of the last record
        let torn = read(&bytes[..bytes.len() - 5]).unwrap();
        assert_eq!((torn.seq, torn.transactions), (1, 1));
        assert_eq!(lines_of(&torn.state), lines_of(&s1));
        // a flipped byte inside the last record: the earlier work is kept and the end is reported as not clean
        let mut bad = bytes.clone();
        let n = bad.len();
        bad[n - 3] ^= 0xFF;
        let r = read(&bad).unwrap();
        assert_eq!((r.seq, r.complete), (1, false));
        assert_eq!(good, base_bytes(0, &s0).len() + tx_bytes(1, "autosave", diff(&s0, &s1)).len());
        // a base that does not read is an error, not a guess
        assert!(read(&bytes[..6]).is_err());
    }
}
