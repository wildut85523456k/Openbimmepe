use openbim_oplog::*;

fn ies(flux_cd: f64) -> Vec<u8> {
    let v: Vec<String> = (0..=6).map(|k| (k * 30).to_string()).collect();
    format!("IESNA:LM-63-2002\n[MANUFAC] M\nTILT=NONE\n1 -1 1 7 1 1 2 0.6 0.6 0\n1 1 40\n{}\n0\n{}\n", v.join(" "), vec![flux_cd.to_string(); 7].join(" ")).into_bytes()
}

#[test]
fn a_file_is_kept_in_the_project_and_comes_back_after_replay() {
    let mut d = Doc::new(1);
    let f = ies(100.0);
    let id = d.put_asset("photometry", "a.ies", &f).unwrap();
    assert_eq!(d.state.find_asset("photometry", "a.ies"), Some(id));
    assert_eq!(d.state.asset_data(id), Some(f.as_slice()));
    // same content again: nothing logged
    let len = d.log.len();
    assert_eq!(d.put_asset("photometry", "a.ies", &f).unwrap(), id);
    assert_eq!(d.log.len(), len);
    // other content under the same name replaces it, and undo brings the first one back
    let g = ies(50.0);
    assert_eq!(d.put_asset("photometry", "a.ies", &g).unwrap(), id);
    assert_eq!(d.state.asset_data(id), Some(g.as_slice()));
    assert!(d.undo());
    assert_eq!(d.state.asset_data(id), Some(f.as_slice()));
    // another role with the same name is another asset
    let other = d.put_asset("drawing", "a.ies", b"x").unwrap();
    assert_ne!(other, id);
    // the log and a snapshot both give the same state back
    let (txs, tail) = read_log(&d.log);
    assert_eq!(tail, Tail::Clean);
    let replayed = replay(State::default(), &txs).unwrap();
    assert_eq!(replayed, d.state);
    let (_, loaded) = load_snapshot(&snapshot(txs.last().unwrap().seq, &d.state)).unwrap();
    assert_eq!(loaded, d.state);
    assert_eq!(loaded.asset_data(id), Some(f.as_slice()));
}

#[test]
fn a_damaged_file_is_noticed_and_the_limits_hold() {
    let mut d = Doc::new(1);
    let id = d.put_asset("photometry", "a.ies", &ies(1.0)).unwrap();
    // content that no longer matches its checksum is not served
    let mut s = d.state.clone();
    if let Some(Value::Bytes(b)) = s.elements.get_mut(&id).unwrap().props.get_mut(&asset_prop::DATA) { b[3] ^= 1; }
    assert!(s.asset_data(id).is_none());
    // a flipped byte in the log stops the replay at that record
    let mut log = d.log.clone();
    let n = log.len();
    log[n - 40] ^= 0xFF;
    assert!(matches!(read_log(&log).1, Tail::Damaged { .. }));
    // too large, or a name that is too long
    assert_eq!(d.put_asset("photometry", "big.ies", &vec![0u8; MAX_BLOB + 1]), Err(OpError::TooLong));
    assert_eq!(d.put_asset("photometry", &"n".repeat(300), b"x"), Err(OpError::TooLong));
    assert!(d.state.find_asset("photometry", "big.ies").is_none());
    // the largest allowed file goes in and reads back
    let big = vec![7u8; MAX_BLOB];
    let bid = d.put_asset("blob", "big.bin", &big).unwrap();
    let (txs, tail) = read_log(&d.log);
    assert_eq!(tail, Tail::Clean);
    assert_eq!(replay(State::default(), &txs).unwrap().asset_data(bid), Some(big.as_slice()));
}

#[test]
fn photometric_files_from_an_import_are_kept_and_read_again_from_the_project() {
    let mut d = Doc::new(2);
    for it in openbim_photo::import(&ies(100.0), "x.ies") {
        let p = it.result.unwrap();
        let name = it.name.rsplit('/').next().unwrap();
        d.put_asset("photometry", name, &it.data).unwrap();
        assert!((p.flux_lm().unwrap() - 4.0 * std::f64::consts::PI * 100.0).abs() < 1e-6);
    }
    let id = d.state.find_asset("photometry", "x.ies").unwrap();
    let again = openbim_photo::parse(d.state.asset_data(id).unwrap(), "x.ies").unwrap();
    assert_eq!(again.manufacturer.as_deref(), Some("M"));
}

#[test]
fn name_and_role_lengths_are_limited_exactly() {
    let mut d = Doc::new(1);
    assert!(d.put_asset("r", &"n".repeat(255), b"x").is_ok());
    assert_eq!(d.put_asset("r", &"n".repeat(256), b"x"), Err(OpError::TooLong));
    assert!(d.put_asset(&"r".repeat(64), "a", b"x").is_ok());
    assert_eq!(d.put_asset(&"r".repeat(65), "a", b"x"), Err(OpError::TooLong));
    // exactly the largest blob is kept
    assert!(d.put_asset("r", "max", &vec![1u8; MAX_BLOB]).is_ok());
}
