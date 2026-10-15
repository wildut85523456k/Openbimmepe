//! Tiny formula language: numbers (with units such as `1000 mm`), names (letters in any language, digits, underscores
//! and single spaces, as in `Rough Width`), + - * /, unary -, comparisons (`=`, `==`, `<>`, `!=`, `<`, `>`, `<=`, `>=`),
//! && || !, and(...) or(...) not(...), and a few functions.
//! Every formula is parsed once (names resolved to slots) and then evaluated many times without allocation.
//! Hard limits keep hostile input cheap: length, nesting depth and node count.

use std::fmt;

pub const MAX_LEN: usize = 1024;
pub const MAX_DEPTH: usize = 48;
pub const MAX_NODES: usize = 512;
/// most conditions in one and(...) / or(...)
pub const MAX_LOGIC_ARGS: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub enum ExprError {
    TooLong,
    TooDeep,
    TooManyNodes,
    Syntax(String),
    UnknownName(String),
    UnknownFunction(String),
    Arity { name: String, expected: usize, got: usize },
    DivByZero,
    NotFinite,
    Domain(&'static str),
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprError::TooLong => write!(f, "formula is longer than {MAX_LEN} characters"),
            ExprError::TooDeep => write!(f, "formula is nested more than {MAX_DEPTH} levels"),
            ExprError::TooManyNodes => write!(f, "formula has more than {MAX_NODES} parts"),
            ExprError::Syntax(s) => write!(f, "syntax error: {s}"),
            ExprError::UnknownName(n) => write!(f, "unknown name '{n}'"),
            ExprError::UnknownFunction(n) => write!(f, "unknown function '{n}'"),
            ExprError::Arity { name, expected, got } => write!(f, "{name} needs {expected} arguments, got {got}"),
            ExprError::DivByZero => write!(f, "division by zero"),
            ExprError::NotFinite => write!(f, "result is not a finite number"),
            ExprError::Domain(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Bin { Add, Sub, Mul, Div, Lt, Le, Gt, Ge, Eq, Ne, And, Or }

#[derive(Debug, Clone, Copy, PartialEq)]
enum Fun { Min, Max, Abs, Sqrt, Floor, Ceil, Round, If, Clamp, Sin, Cos, Tan, Asin, Acos, Atan, Atan2, Pi }

impl Fun {
    fn from_name(n: &str) -> Option<(Fun, usize)> {
        Some(match n {
            "min" => (Fun::Min, 2), "max" => (Fun::Max, 2), "abs" => (Fun::Abs, 1), "sqrt" => (Fun::Sqrt, 1),
            "floor" => (Fun::Floor, 1), "ceil" => (Fun::Ceil, 1), "round" => (Fun::Round, 1),
            "if" => (Fun::If, 3), "clamp" => (Fun::Clamp, 3),
            "sin" => (Fun::Sin, 1), "cos" => (Fun::Cos, 1), "tan" => (Fun::Tan, 1),
            "asin" => (Fun::Asin, 1), "acos" => (Fun::Acos, 1), "atan" => (Fun::Atan, 1), "atan2" => (Fun::Atan2, 2), "pi" => (Fun::Pi, 0),
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Num(f64),
    Var(usize),
    Neg(Box<Node>),
    Not(Box<Node>),
    Bin(Bin, Box<Node>, Box<Node>),
    Call(Fun, Vec<Node>),
}

/// A parsed formula whose names are slots in a value table.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr { root: Node, deps: Vec<usize> }

impl Expr {
    /// Slots this formula reads (sorted, unique).
    pub fn deps(&self) -> &[usize] { &self.deps }

    pub fn parse(src: &str, resolve: &dyn Fn(&str) -> Option<usize>) -> Result<Expr, ExprError> {
        if src.len() > MAX_LEN { return Err(ExprError::TooLong); }
        let toks = lex(src, resolve)?;
        let mut p = Parser { toks: &toks, i: 0, resolve, nodes: 0, deps: Vec::new() };
        let root = p.or(0)?;
        if p.i != toks.len() { return Err(ExprError::Syntax(format!("unexpected '{}'", toks[p.i].text()))); }
        let mut deps = p.deps; deps.sort_unstable(); deps.dedup();
        Ok(Expr { root, deps })
    }

    pub fn eval(&self, values: &[f64]) -> Result<f64, ExprError> {
        let v = ev(&self.root, values)?;
        if v.is_finite() { Ok(v) } else { Err(ExprError::NotFinite) }
    }
}

fn b(x: bool) -> f64 { if x { 1.0 } else { 0.0 } }

fn ev(n: &Node, vals: &[f64]) -> Result<f64, ExprError> {
    let fin = |v: f64| if v.is_finite() { Ok(v) } else { Err(ExprError::NotFinite) };
    match n {
        Node::Num(x) => Ok(*x),
        Node::Var(i) => Ok(vals[*i]),
        Node::Neg(a) => Ok(-ev(a, vals)?),
        Node::Not(a) => Ok(b(ev(a, vals)? == 0.0)),
        Node::Bin(op, l, r) => match op {
            // short circuit so that if(w > 0 && 1/w < 5, ...) is safe
            Bin::And => { let x = ev(l, vals)?; if x == 0.0 { Ok(0.0) } else { Ok(b(ev(r, vals)? != 0.0)) } }
            Bin::Or => { let x = ev(l, vals)?; if x != 0.0 { Ok(1.0) } else { Ok(b(ev(r, vals)? != 0.0)) } }
            _ => {
                let (x, y) = (ev(l, vals)?, ev(r, vals)?);
                match op {
                    Bin::Add => fin(x + y), Bin::Sub => fin(x - y), Bin::Mul => fin(x * y),
                    Bin::Div => { if y == 0.0 { Err(ExprError::DivByZero) } else { fin(x / y) } }
                    Bin::Lt => Ok(b(x < y)), Bin::Le => Ok(b(x <= y)), Bin::Gt => Ok(b(x > y)), Bin::Ge => Ok(b(x >= y)),
                    Bin::Eq => Ok(b(x == y)), Bin::Ne => Ok(b(x != y)),
                    Bin::And | Bin::Or => unreachable!(),
                }
            }
        },
        Node::Call(f, a) => match f {
            Fun::Min => Ok(ev(&a[0], vals)?.min(ev(&a[1], vals)?)),
            Fun::Max => Ok(ev(&a[0], vals)?.max(ev(&a[1], vals)?)),
            Fun::Abs => Ok(ev(&a[0], vals)?.abs()),
            Fun::Sqrt => { let x = ev(&a[0], vals)?; if x < 0.0 { Err(ExprError::Domain("sqrt of a negative number")) } else { Ok(x.sqrt()) } }
            Fun::Floor => Ok(ev(&a[0], vals)?.floor()),
            Fun::Ceil => Ok(ev(&a[0], vals)?.ceil()),
            Fun::Round => Ok(ev(&a[0], vals)?.round()), // half away from zero
            Fun::If => { if ev(&a[0], vals)? != 0.0 { ev(&a[1], vals) } else { ev(&a[2], vals) } }
            Fun::Sin => Ok(sin_deg(ev(&a[0], vals)?)),
            Fun::Cos => Ok(cos_deg(ev(&a[0], vals)?)),
            Fun::Tan => {
                let x = ev(&a[0], vals)?;
                let c = cos_deg(x);
                if c.abs() < 1e-12 { Err(ExprError::Domain("tan of 90 degrees (or 270)")) } else { Ok(sin_deg(x) / c) }
            }
            Fun::Asin => { let x = ev(&a[0], vals)?; if !(-1.0..=1.0).contains(&x) { Err(ExprError::Domain("asin outside -1 to 1")) } else { Ok(x.asin().to_degrees()) } }
            Fun::Acos => { let x = ev(&a[0], vals)?; if !(-1.0..=1.0).contains(&x) { Err(ExprError::Domain("acos outside -1 to 1")) } else { Ok(x.acos().to_degrees()) } }
            Fun::Atan => Ok(ev(&a[0], vals)?.atan().to_degrees()),
            Fun::Atan2 => {
                let (y, x) = (ev(&a[0], vals)?, ev(&a[1], vals)?);
                if x == 0.0 && y == 0.0 { Err(ExprError::Domain("atan2 of (0, 0)")) } else { Ok(y.atan2(x).to_degrees()) }
            }
            Fun::Pi => Ok(std::f64::consts::PI),
            Fun::Clamp => {
                let (x, lo, hi) = (ev(&a[0], vals)?, ev(&a[1], vals)?, ev(&a[2], vals)?);
                if lo > hi { Err(ExprError::Domain("clamp with lo above hi")) } else { Ok(x.max(lo).min(hi)) }
            }
        },
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok { Num(f64), Id(String), Op(&'static str), LP, RP, Comma }

impl Tok {
    fn text(&self) -> String {
        match self { Tok::Num(x) => x.to_string(), Tok::Id(s) => s.clone(), Tok::Op(s) => (*s).into(), Tok::LP => "(".into(), Tok::RP => ")".into(), Tok::Comma => ",".into() }
    }
}

/// Length units a number may carry, as metres; angles are kept in degrees.
fn unit_factor(w: &str) -> Option<f64> {
    Some(match w {
        "mm" => 0.001, "cm" => 0.01, "m" => 1.0, "km" => 1000.0, "in" => 0.0254, "ft" => 0.3048,
        "deg" => 1.0, "rad" => 180.0 / std::f64::consts::PI,
        _ => return None,
    })
}

fn is_word(ch: char) -> bool { ch.is_alphanumeric() || ch == '_' }
fn starts_word(ch: char) -> bool { ch.is_alphabetic() || ch == '_' }

/// End of the name part starting at `i`. A '.' joins two parts (choice.option) only when a letter or underscore follows.
fn word_end(c: &[char], mut i: usize) -> usize {
    while i < c.len() && (is_word(c[i]) || (c[i] == '.' && i + 1 < c.len() && starts_word(c[i + 1]))) { i += 1; }
    i
}

fn lex(src: &str, resolve: &dyn Fn(&str) -> Option<usize>) -> Result<Vec<Tok>, ExprError> {
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() { i += 1; continue; }
        if ch.is_ascii_digit() || ch == '.' {
            let s = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') { i += 1; }
            if i < c.len() && (c[i] == 'e' || c[i] == 'E') {
                let mut j = i + 1;
                if j < c.len() && (c[j] == '+' || c[j] == '-') { j += 1; }
                if j < c.len() && c[j].is_ascii_digit() { while j < c.len() && c[j].is_ascii_digit() { j += 1; } i = j; }
            }
            let t: String = c[s..i].iter().collect();
            let mut v: f64 = t.parse().map_err(|_| ExprError::Syntax(format!("bad number '{t}'")))?;
            // a unit straight after the number, with or without a space: 1000 mm, 2.5cm
            let mut j = i;
            while j < c.len() && c[j] == ' ' { j += 1; }
            if j < c.len() && starts_word(c[j]) {
                let e = { let mut e = j; while e < c.len() && is_word(c[e]) { e += 1; } e };
                let w: String = c[j..e].iter().collect();
                if let Some(f) = unit_factor(&w) { v *= f; i = e; }
            }
            if !v.is_finite() { return Err(ExprError::NotFinite); }
            out.push(Tok::Num(v));
        } else if starts_word(ch) {
            // the longest run of words separated by single spaces that is a known name ("Rough Width"), else one word
            let first = word_end(&c, i);
            let mut ends = vec![first];
            while ends.len() < 32 {
                let j = *ends.last().unwrap();
                if j + 1 < c.len() && c[j] == ' ' && starts_word(c[j + 1]) { ends.push(word_end(&c, j + 1)); } else { break; }
            }
            let end = ends.iter().rev().copied().find(|&e| { let cand: String = c[i..e].iter().collect(); resolve(&cand).is_some() }).unwrap_or(first);
            out.push(Tok::Id(c[i..end].iter().collect()));
            i = end;
        } else {
            let two: String = c[i..(i + 2).min(c.len())].iter().collect();
            let op2: Option<&'static str> = match two.as_str() { "<=" => Some("<="), ">=" => Some(">="), "==" => Some("=="), "!=" => Some("!="), "<>" => Some("!="), "&&" => Some("&&"), "||" => Some("||"), _ => None };
            if let Some(o) = op2 { out.push(Tok::Op(o)); i += 2; continue; }
            let t = match ch {
                '+' => Tok::Op("+"), '-' => Tok::Op("-"), '*' => Tok::Op("*"), '/' => Tok::Op("/"),
                '<' => Tok::Op("<"), '>' => Tok::Op(">"), '!' => Tok::Op("!"), '=' => Tok::Op("=="),
                '(' => Tok::LP, ')' => Tok::RP, ',' => Tok::Comma,
                _ => return Err(ExprError::Syntax(format!("unexpected character '{ch}'"))),
            };
            out.push(t); i += 1;
        }
        if out.len() > MAX_NODES * 4 { return Err(ExprError::TooManyNodes); }
    }
    Ok(out)
}

struct Parser<'a> { toks: &'a [Tok], i: usize, resolve: &'a dyn Fn(&str) -> Option<usize>, nodes: usize, deps: Vec<usize> }

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> { self.toks.get(self.i) }
    /// Counts one tree node (also checks nesting).
    fn bump(&mut self, depth: usize) -> Result<(), ExprError> {
        self.nodes += 1;
        if self.nodes > MAX_NODES { return Err(ExprError::TooManyNodes); }
        self.deep(depth)
    }
    /// Checks nesting only. Parentheses, call arguments and chains of unary operators are the nesting levels.
    fn deep(&self, depth: usize) -> Result<(), ExprError> {
        if depth > MAX_DEPTH { return Err(ExprError::TooDeep); }
        Ok(())
    }
    fn is_op(&self, s: &str) -> bool { matches!(self.peek(), Some(Tok::Op(o)) if *o == s) }

    fn or(&mut self, d: usize) -> Result<Node, ExprError> {
        self.deep(d)?;
        let mut l = self.and(d)?;
        while self.is_op("||") { self.i += 1; let r = self.and(d)?; self.bump(d)?; l = Node::Bin(Bin::Or, Box::new(l), Box::new(r)); }
        Ok(l)
    }
    fn and(&mut self, d: usize) -> Result<Node, ExprError> {
        self.deep(d)?;
        let mut l = self.cmp(d)?;
        while self.is_op("&&") { self.i += 1; let r = self.cmp(d)?; self.bump(d)?; l = Node::Bin(Bin::And, Box::new(l), Box::new(r)); }
        Ok(l)
    }
    fn cmp(&mut self, d: usize) -> Result<Node, ExprError> {
        self.deep(d)?;
        let l = self.add(d)?;
        let op = match self.peek() { Some(Tok::Op("<")) => Some(Bin::Lt), Some(Tok::Op("<=")) => Some(Bin::Le), Some(Tok::Op(">")) => Some(Bin::Gt), Some(Tok::Op(">=")) => Some(Bin::Ge), Some(Tok::Op("==")) => Some(Bin::Eq), Some(Tok::Op("!=")) => Some(Bin::Ne), _ => None };
        if let Some(op) = op { self.i += 1; let r = self.add(d)?; self.bump(d)?; return Ok(Node::Bin(op, Box::new(l), Box::new(r))); }
        Ok(l)
    }
    fn add(&mut self, d: usize) -> Result<Node, ExprError> {
        self.deep(d)?;
        let mut l = self.mul(d)?;
        loop {
            let op = if self.is_op("+") { Bin::Add } else if self.is_op("-") { Bin::Sub } else { break };
            self.i += 1; let r = self.mul(d)?; self.bump(d)?;
            l = Node::Bin(op, Box::new(l), Box::new(r));
        }
        Ok(l)
    }
    fn mul(&mut self, d: usize) -> Result<Node, ExprError> {
        self.deep(d)?;
        let mut l = self.unary(d)?;
        loop {
            let op = if self.is_op("*") { Bin::Mul } else if self.is_op("/") { Bin::Div } else { break };
            self.i += 1; let r = self.unary(d)?; self.bump(d)?;
            l = Node::Bin(op, Box::new(l), Box::new(r));
        }
        Ok(l)
    }
    fn unary(&mut self, d: usize) -> Result<Node, ExprError> {
        self.deep(d)?;
        if self.is_op("-") { self.i += 1; return Ok({ self.bump(d)?; Node::Neg(Box::new(self.unary(d + 1)?)) }); }
        if self.is_op("!") { self.i += 1; return Ok({ self.bump(d)?; Node::Not(Box::new(self.unary(d + 1)?)) }); }
        self.atom(d)
    }
    fn atom(&mut self, d: usize) -> Result<Node, ExprError> {
        self.bump(d)?;
        let t = self.peek().cloned().ok_or_else(|| ExprError::Syntax("formula ends too early".into()))?;
        self.i += 1;
        match t {
            Tok::Num(x) => Ok(Node::Num(x)),
            Tok::LP => {
                let e = self.or(d + 1)?;
                if !matches!(self.peek(), Some(Tok::RP)) { return Err(ExprError::Syntax("missing ')'".into())); }
                self.i += 1; Ok(e)
            }
            Tok::Id(name) => {
                if matches!(self.peek(), Some(Tok::LP)) {
                    self.i += 1;
                    let logic = match name.as_str() { "and" => Some(Bin::And), "or" => Some(Bin::Or), "not" => Some(Bin::Eq /* only a marker: not() has no binary operator */), _ => None };
                    let (f, arity) = if logic.is_some() { (Fun::Pi, 0) } else { Fun::from_name(&name).ok_or_else(|| ExprError::UnknownFunction(name.clone()))? };
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Tok::RP)) {
                        loop {
                            args.push(self.or(d + 1)?);
                            if matches!(self.peek(), Some(Tok::Comma)) { self.i += 1; } else { break; }
                        }
                    }
                    if !matches!(self.peek(), Some(Tok::RP)) { return Err(ExprError::Syntax("missing ')'".into())); }
                    self.i += 1;
                    if let Some(op) = logic {
                        // and(a, b, ...) / or(a, b, ...) take two to sixteen conditions; not(a) takes one
                        if name == "not" {
                            if args.len() != 1 { return Err(ExprError::Arity { name, expected: 1, got: args.len() }); }
                            return Ok(Node::Not(Box::new(args.remove(0))));
                        }
                        if args.len() < 2 { return Err(ExprError::Arity { name, expected: 2, got: args.len() }); }
                        if args.len() > MAX_LOGIC_ARGS { return Err(ExprError::TooManyNodes); }
                        let mut it = args.into_iter();
                        let first = it.next().expect("two arguments");
                        return Ok(it.fold(first, |l, r| Node::Bin(op, Box::new(l), Box::new(r))));
                    }
                    if args.len() != arity { return Err(ExprError::Arity { name, expected: arity, got: args.len() }); }
                    Ok(Node::Call(f, args))
                } else {
                    let idx = (self.resolve)(&name).ok_or(ExprError::UnknownName(name))?;
                    self.deps.push(idx);
                    Ok(Node::Var(idx))
                }
            }
            other => Err(ExprError::Syntax(format!("unexpected '{}'", other.text()))),
        }
    }
}

/// sine of an angle in degrees, exact at multiples of 90 degrees
pub(crate) fn sin_deg(x: f64) -> f64 {
    let r = x.rem_euclid(360.0);
    if r % 90.0 == 0.0 {
        // r can come out as exactly 360 for a tiny negative angle (rem_euclid rounds up), so reduce the quadrant too
        return match (r / 90.0) as u8 % 4 { 0 | 2 => 0.0, 1 => 1.0, _ => -1.0 };
    }
    r.to_radians().sin()
}

/// cosine of an angle in degrees; the angle is reduced first so very large angles do not lose their low bits
pub(crate) fn cos_deg(x: f64) -> f64 { sin_deg(x.rem_euclid(360.0) + 90.0) }
