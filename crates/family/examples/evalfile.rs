// Reads lines "expr|a=1,b=2" and prints the value or ERR for each. Used to compare against an independent evaluator.
use openbim_family::expr::Expr;
use std::io::BufRead;
fn main() {
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        let (src, vars) = line.rsplit_once('|').unwrap();
        let mut names: Vec<(String, f64)> = Vec::new();
        for kv in vars.split(',').filter(|s| !s.is_empty()) {
            let (k, v) = kv.split_once('=').unwrap();
            names.push((k.to_string(), v.parse().unwrap()));
        }
        let vals: Vec<f64> = names.iter().map(|(_, v)| *v).collect();
        let res = Expr::parse(src, &|n| names.iter().position(|(k, _)| k == n))
            .and_then(|e| e.eval(&vals));
        match res {
            Ok(v) => println!("{:?}", v),
            Err(_) => println!("ERR"),
        }
    }
}
