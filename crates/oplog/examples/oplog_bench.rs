use openbim_oplog::*;
use std::time::Instant;
fn main() {
    let n = 200_000u64;
    let mut d = Doc::new(1);
    let t = Instant::now();
    for i in 0..n {
        d.edit("e", vec![
            Op::Create { id: i, kind: 1 },
            Op::Set { id: i, prop: 1, value: Some(Value::Num(i as f64)) },
            Op::Set { id: i, prop: 2, value: Some(Value::Int(i as i64)) },
        ]).unwrap();
    }
    let w = t.elapsed();
    println!("edits {} in {:?}  log {} MB", n, w, d.log.len() / 1_000_000);
    let t = Instant::now();
    let (txs, tail) = read_log(&d.log);
    let r = t.elapsed();
    let t = Instant::now();
    let s = replay(State::default(), &txs).unwrap();
    println!("read {} tx {:?} ({:?}), replay {:?}, equal {}", txs.len(), r, tail, t.elapsed(), s == d.state);
    let t = Instant::now();
    let snap = snapshot(n, &s);
    let a = t.elapsed();
    let t = Instant::now();
    let ok = load_snapshot(&snap).map(|(_, x)| x == s);
    println!("snapshot {} MB write {:?} load {:?} equal {:?}", snap.len() / 1_000_000, a, t.elapsed(), ok);
    let t = Instant::now();
    let mut k = 0;
    while d.undo() && k < 50_000 { k += 1; }
    println!("50k undos {:?}", t.elapsed());
}
