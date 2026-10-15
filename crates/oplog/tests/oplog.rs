use openbim_oplog::*;
use proptest::prelude::*;

fn num(x: f64) -> Option<Value> {
    Some(Value::Num(x))
}

fn sample() -> Doc {
    let mut d = Doc::new(7);
    d.edit("wall", vec![
        Op::Create { id: 1, kind: 10 },
        Op::Set { id: 1, prop: 1, value: num(3.5) },
        Op::Set { id: 1, prop: 2, value: Some(Value::Text("Brick".into())) },
    ]).unwrap();
    d.edit("door", vec![Op::Create { id: 2, kind: 20 }, Op::Set { id: 2, prop: 1, value: Some(Value::Bool(true)) }]).unwrap();
    d
}

#[test]
fn undo_redo_roundtrip_and_log_grows() {
    let mut d = sample();
    let full = d.state.clone();
    let len0 = d.log.len();
    assert!(d.undo());
    assert!(!d.state.elements.contains_key(&2));
    assert!(d.log.len() > len0, "undo is logged, log never shrinks");
    assert!(d.can_redo());
    assert!(d.redo());
    assert_eq!(d.state, full);
    assert!(d.undo() && d.undo());
    assert!(d.state.elements.is_empty());
    assert!(!d.undo());
    assert!(!d.can_undo());
    // new edit clears redo
    d.edit("x", vec![Op::Create { id: 9, kind: 1 }]).unwrap();
    assert!(!d.can_redo());
    assert!(!d.redo());
}

#[test]
fn delete_restores_everything_on_undo() {
    let mut d = sample();
    let before = d.state.clone();
    d.edit("del", vec![Op::Delete { id: 1 }]).unwrap();
    assert!(!d.state.elements.contains_key(&1));
    d.undo();
    assert_eq!(d.state, before);
}

#[test]
fn failed_edit_changes_nothing_and_logs_nothing() {
    let mut d = sample();
    let before = d.state.clone();
    let len = d.log.len();
    let e = d.edit("bad", vec![Op::Set { id: 1, prop: 5, value: num(1.0) }, Op::Delete { id: 99 }]);
    assert_eq!(e, Err(OpError::Missing(99)));
    assert_eq!(d.state, before);
    assert_eq!(d.log.len(), len);
    assert_eq!(d.edit("dup", vec![Op::Create { id: 1, kind: 1 }]), Err(OpError::Exists(1)));
    assert_eq!(d.edit("nan", vec![Op::Set { id: 1, prop: 1, value: num(f64::NAN) }]), Err(OpError::NotFinite(1, 1)));
    assert_eq!(d.edit("inf", vec![Op::Set { id: 1, prop: 1, value: num(f64::INFINITY) }]), Err(OpError::NotFinite(1, 1)));
    let big = "x".repeat(MAX_TEXT + 1);
    assert_eq!(d.edit("big", vec![Op::Set { id: 1, prop: 1, value: Some(Value::Text(big)) }]), Err(OpError::TooLong));
    let ok = "x".repeat(MAX_TEXT);
    assert!(d.edit("max", vec![Op::Set { id: 1, prop: 1, value: Some(Value::Text(ok)) }]).is_ok());
    assert_eq!(d.state.apply(&Op::Restore { id: 1, element: Element::default() }), Err(OpError::Exists(1)));
}

#[test]
fn replay_equals_live_state() {
    let mut d = sample();
    d.undo();
    d.edit("c", vec![Op::Set { id: 1, prop: 1, value: None }]).unwrap();
    let (txs, tail) = read_log(&d.log);
    assert_eq!(tail, Tail::Clean);
    assert_eq!(txs.len(), 4);
    assert_eq!(txs[0].label, "wall");
    assert_eq!(txs[0].author, 7);
    assert_eq!(txs.iter().map(|t| t.seq).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    let s = replay(State::default(), &txs).unwrap();
    assert_eq!(s, d.state);
    assert_eq!(s.fingerprint(), d.state.fingerprint());
}

#[test]
fn torn_tail_is_dropped_at_every_cut_point() {
    let d = sample();
    let (txs, _) = read_log(&d.log);
    let first = frame(&txs[0]).len();
    for cut in 0..d.log.len() {
        let (got, tail) = read_log(&d.log[..cut]);
        let expect = if cut < first { 0 } else if cut < d.log.len() { 1 } else { 2 };
        assert_eq!(got.len(), expect, "cut {cut}");
        if cut == 0 || cut == first {
            assert_eq!(tail, Tail::Clean);
        } else {
            assert!(matches!(tail, Tail::Torn { .. }), "cut {cut}: {tail:?}");
        }
    }
}

#[test]
fn damage_is_detected_not_ignored() {
    let d = sample();
    let (txs, _) = read_log(&d.log);
    let first = frame(&txs[0]).len();
    // flip every single byte of the second record: always detected, first record survives
    for i in first..d.log.len() {
        let mut b = d.log.clone();
        b[i] ^= 0x01;
        let (got, tail) = read_log(&b);
        assert_eq!(got.len(), 1, "byte {i}");
        assert!(matches!(tail, Tail::Damaged { at } if at == first) || matches!(tail, Tail::Torn { at } if at == first), "byte {i}: {tail:?}");
    }
    // bad magic at start
    let mut b = d.log.clone();
    b[0] = 0;
    assert_eq!(read_log(&b), (vec![], Tail::Damaged { at: 0 }));
    // a huge claimed length is damage, not a torn write
    let mut b = d.log.clone();
    b[5] = 0xFF;
    assert_eq!(read_log(&b).1, Tail::Damaged { at: 0 });
    // short garbage
    assert_eq!(read_log(&[1, 2, 3]).1, Tail::Damaged { at: 0 });
    assert_eq!(read_log(&[0x0B, 0x1A, 1]).1, Tail::Torn { at: 0 });
}

#[test]
fn crc_known_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF43926);
    assert_eq!(crc32(b""), 0);
}

#[test]
fn snapshot_plus_tail_equals_full_replay() {
    let mut d = sample();
    let snap = snapshot(2, &d.state);
    d.edit("later", vec![Op::Set { id: 2, prop: 3, value: Some(Value::Int(-5)) }]).unwrap();
    let (seq, s0) = load_snapshot(&snap).unwrap();
    assert_eq!(seq, 2);
    let (txs, _) = read_log(&d.log);
    let tail: Vec<_> = txs.into_iter().filter(|t| t.seq > seq).collect();
    assert_eq!(tail.len(), 1);
    assert_eq!(replay(s0, &tail).unwrap(), d.state);
    // corruption of any byte, or truncation, is rejected
    for i in 0..snap.len() {
        let mut b = snap.clone();
        b[i] ^= 0x40;
        assert!(load_snapshot(&b).is_none(), "byte {i}");
    }
    assert!(load_snapshot(&snap[..snap.len() - 1]).is_none());
    assert!(load_snapshot(&[]).is_none());
    let mut extra = snap.clone();
    extra.push(0);
    assert!(load_snapshot(&extra).is_none());
}

#[test]
fn all_value_types_roundtrip_exactly() {
    let mut d = Doc::new(1);
    d.edit("v", vec![
        Op::Create { id: u64::MAX, kind: u32::MAX },
        Op::Set { id: u64::MAX, prop: 0, value: num(-0.0) },
        Op::Set { id: u64::MAX, prop: 1, value: num(f64::MIN_POSITIVE) },
        Op::Set { id: u64::MAX, prop: 2, value: Some(Value::Int(i64::MIN)) },
        Op::Set { id: u64::MAX, prop: 3, value: Some(Value::Bool(false)) },
        Op::Set { id: u64::MAX, prop: 4, value: Some(Value::Text("É🏗".into())) },
    ]).unwrap();
    let (txs, t) = read_log(&d.log);
    assert_eq!(t, Tail::Clean);
    let s = replay(State::default(), &txs).unwrap();
    assert_eq!(s, d.state);
    let e = &s.elements[&u64::MAX];
    assert!(matches!(e.props[&0], Value::Num(x) if x.is_sign_negative() && x == 0.0));
    assert_eq!(e.props[&2], Value::Int(i64::MIN));
}

#[test]
fn replay_error_names_the_transaction() {
    let t = Transaction { seq: 42, author: 1, label: "x".into(), ops: vec![Op::Delete { id: 3 }] };
    assert_eq!(replay(State::default(), &[t]), Err((42, OpError::Missing(3))));
}

fn tx(seq: u64, ops: Vec<Op>) -> Transaction {
    Transaction { seq, author: 0, label: String::new(), ops }
}

#[test]
fn merge_disjoint_edits_commute() {
    let base = sample().state;
    let a = vec![tx(1, vec![Op::Set { id: 1, prop: 1, value: num(9.0) }])];
    let b = vec![tx(1, vec![Op::Set { id: 2, prop: 1, value: Some(Value::Bool(false)) }, Op::Create { id: 3, kind: 5 }])];
    let ab = merge(&base, &a, &b).unwrap();
    let ba = merge(&base, &b, &a).unwrap();
    assert!(ab.conflicts.is_empty() && ba.conflicts.is_empty());
    assert_eq!(ab.state, ba.state);
    assert_eq!(ab.state.elements[&1].props[&1], Value::Num(9.0));
    assert!(ab.state.elements.contains_key(&3));
}

#[test]
fn merge_reports_clashes_and_a_wins() {
    let base = sample().state;
    let a = vec![tx(1, vec![Op::Set { id: 1, prop: 1, value: num(1.0) }])];
    // same property
    let b = vec![tx(5, vec![Op::Set { id: 1, prop: 1, value: num(2.0) }])];
    let m = merge(&base, &a, &b).unwrap();
    assert_eq!(m.conflicts, vec![(5, Key::Prop(1, 1))]);
    assert_eq!(m.state.elements[&1].props[&1], Value::Num(1.0));
    // other property of same element: no clash
    let b = vec![tx(5, vec![Op::Set { id: 1, prop: 2, value: num(2.0) }])];
    assert!(merge(&base, &a, &b).unwrap().conflicts.is_empty());
    // B deletes an element A edited: clash
    let b = vec![tx(6, vec![Op::Delete { id: 1 }])];
    let m = merge(&base, &a, &b).unwrap();
    assert_eq!(m.conflicts, vec![(6, Key::Element(1))]);
    assert!(m.state.elements.contains_key(&1));
    // A deletes, B edits: clash, element stays deleted
    let a2 = vec![tx(1, vec![Op::Delete { id: 2 }])];
    let b2 = vec![tx(7, vec![Op::Set { id: 2, prop: 1, value: num(1.0) }])];
    let m = merge(&base, &a2, &b2).unwrap();
    assert_eq!(m.conflicts, vec![(7, Key::Prop(2, 1))]);
    assert!(!m.state.elements.contains_key(&2));
    // both create the same id
    let a3 = vec![tx(1, vec![Op::Create { id: 50, kind: 1 }])];
    let b3 = vec![tx(8, vec![Op::Create { id: 50, kind: 2 }, Op::Set { id: 50, prop: 1, value: num(1.0) }])];
    let m = merge(&base, &a3, &b3).unwrap();
    assert_eq!(m.conflicts.len(), 2);
    assert_eq!(m.state.elements[&50].kind, 1);
    // an error from base replay is reported with its seq
    assert_eq!(merge(&base, &[tx(3, vec![Op::Delete { id: 77 }])], &[]).unwrap_err(), (3, OpError::Missing(77)));
    assert_eq!(merge(&base, &[], &[tx(4, vec![Op::Delete { id: 77 }])]).unwrap_err(), (4, OpError::Missing(77)));
}

// ---- property tests ----

fn arb_value() -> impl Strategy<Value = Option<Value>> {
    prop_oneof![
        Just(None),
        (-1e6f64..1e6).prop_map(|x| Some(Value::Num(x))),
        any::<i64>().prop_map(|x| Some(Value::Int(x))),
        any::<bool>().prop_map(|x| Some(Value::Bool(x))),
        "[a-z]{0,8}".prop_map(|s| Some(Value::Text(s))),
    ]
}

fn arb_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u64..6, 0u32..4).prop_map(|(id, kind)| Op::Create { id, kind }),
        (0u64..6).prop_map(|id| Op::Delete { id }),
        (0u64..6, 0u32..4, arb_value()).prop_map(|(id, prop, value)| Op::Set { id, prop, value }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn prop_undo_everything_returns_to_empty_and_redo_returns_back(edits in prop::collection::vec(prop::collection::vec(arb_op(), 1..4), 1..20)) {
        let mut d = Doc::new(1);
        let mut done = 0;
        for e in edits {
            if d.edit("e", e).is_ok() { done += 1; }
        }
        let end = d.state.clone();
        for _ in 0..done { prop_assert!(d.undo()); }
        prop_assert!(d.state.elements.is_empty());
        prop_assert!(!d.undo());
        for _ in 0..done { prop_assert!(d.redo()); }
        prop_assert_eq!(&d.state, &end);
        // log replays to the live state, snapshot at any point + tail agrees
        let (txs, tail) = read_log(&d.log);
        prop_assert_eq!(tail, Tail::Clean);
        prop_assert_eq!(replay(State::default(), &txs).unwrap(), end.clone());
        let k = txs.len() / 2;
        let mid = replay(State::default(), &txs[..k]).unwrap();
        let snap = snapshot(k as u64, &mid);
        let (seq, s0) = load_snapshot(&snap).unwrap();
        prop_assert_eq!(seq, k as u64);
        prop_assert_eq!(replay(s0, &txs[k..]).unwrap(), end);
    }

    #[test]
    fn prop_truncation_yields_a_state_that_existed(edits in prop::collection::vec(prop::collection::vec(arb_op(), 1..3), 1..10), cut in 0usize..4000) {
        let mut d = Doc::new(1);
        let mut states = vec![State::default()];
        for e in edits {
            if d.edit("e", e).is_ok() { states.push(d.state.clone()); }
        }
        let cut = cut.min(d.log.len());
        let (txs, _) = read_log(&d.log[..cut]);
        let s = replay(State::default(), &txs).unwrap();
        prop_assert_eq!(&s, &states[txs.len()]);
    }

    #[test]
    fn prop_merge_is_symmetric_when_nothing_clashes(a in prop::collection::vec(arb_op(), 0..6), b in prop::collection::vec(arb_op(), 0..6)) {
        let mut base = State::default();
        for id in 0..6u64 { base.apply(&Op::Create { id, kind: 0 }).unwrap(); }
        let ta = vec![Transaction { seq: 1, author: 1, label: String::new(), ops: a }];
        let tb = vec![Transaction { seq: 1, author: 2, label: String::new(), ops: b }];
        if let (Ok(x), Ok(y)) = (merge(&base, &ta, &tb), merge(&base, &tb, &ta)) {
            if x.conflicts.is_empty() && y.conflicts.is_empty() {
                prop_assert_eq!(x.state, y.state);
            }
        }
    }
}

fn reseal(mut f: Vec<u8>, head_magic: [u8; 2]) -> Vec<u8> {
    // append one byte to the payload and fix length and checksum so only the structure is wrong
    f.push(0);
    let len = (f.len() - 10) as u32;
    f[2..6].copy_from_slice(&len.to_le_bytes());
    let crc = crc32(&f[10..]);
    f[6..10].copy_from_slice(&crc.to_le_bytes());
    f[..2].copy_from_slice(&head_magic);
    f
}

#[test]
fn valid_checksum_but_extra_bytes_is_damage() {
    let d = sample();
    let (txs, _) = read_log(&d.log);
    let bad = reseal(frame(&txs[0]), [0x0B, 0x1A]);
    assert_eq!(read_log(&bad), (vec![], Tail::Damaged { at: 0 }));
    let snap = snapshot(1, &d.state);
    assert!(load_snapshot(&reseal(snap, [0x5A, 0x1B])).is_none());
}

#[test]
fn partial_header_classification() {
    assert_eq!(read_log(&[0x0B]).1, Tail::Torn { at: 0 });
    assert_eq!(read_log(&[0x0B, 0xFF, 1]).1, Tail::Damaged { at: 0 });
    assert_eq!(read_log(&[0xFF, 0x1A]).1, Tail::Damaged { at: 0 });
}

#[test]
fn fingerprint_is_stable_across_versions() {
    // pinned so a stored fingerprint stays comparable
    assert_eq!(State::default().fingerprint(), 0xa8c7f832281a39c5);
    let mut s = State::default();
    s.apply(&Op::Create { id: 1, kind: 2 }).unwrap();
    assert_ne!(s.fingerprint(), State::default().fingerprint());
}
