//! Keyboard shortcuts.
//!
//! A `Keymap` has a *preset* (the defaults, loaded from a reference such as Revit's `KeyboardShortcuts.xml`)
//! and the user's *overrides* (what the settings menu changes). The effective map is the preset with the
//! overrides on top. Two commands may share a shortcut, as in Revit: typing it offers the commands and the user
//! picks one (`Match::Choice`); the settings menu can list the sharing with [`Keymap::shared`]. A shortcut may not
//! be the beginning of a different one (typing `W` `A` must not also be a complete shortcut `W`), so a typed
//! sequence is either a whole shortcut, the start of a longer one, or nothing.
//!
//! The preset values are data, not code: this crate contains no shortcut of any product.

pub mod mouse;

use std::collections::{BTreeMap, BTreeSet};

pub const MAX_COMMANDS: usize = 10_000;
pub const MAX_SEQ_LEN: usize = 4;
pub const MAX_PER_COMMAND: usize = 8;
pub const MAX_TEXT: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mods(u8);
impl Mods {
    pub const NONE: Mods = Mods(0);
    pub const CTRL: Mods = Mods(1);
    pub const SHIFT: Mods = Mods(2);
    pub const ALT: Mods = Mods(4);
}

/// One key press with modifiers. `key` is upper-case for letters ("A"), "0".."9", "F1".."F24", or a name
/// from `NAMED` ("Escape", "Delete", "Space", ...).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chord { pub mods: Mods, pub key: String }

pub type Sequence = Vec<Chord>;

const NAMED: [&str; 14] = ["Escape", "Delete", "Space", "Enter", "Tab", "Backspace", "Home", "End", "PageUp", "PageDown", "Up", "Down", "Left", "Right"];

#[derive(Debug, Clone, PartialEq)]
pub enum KeymapError {
    BadKey(String),
    EmptySequence,
    TooLong,
    TooMany,
    UnknownCommand(String),
    /// the shortcut is the beginning of another command's shortcut, or the other way round (an equal shortcut is allowed)
    Conflict { shortcut: String, with_command: String },
    /// two shortcuts of the same command clash
    SelfConflict(String),
    TooBig,
    BadLine(usize),
}
impl std::fmt::Display for KeymapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeymapError::BadKey(k) => write!(f, "'{k}' is not a key"),
            KeymapError::EmptySequence => write!(f, "empty shortcut"),
            KeymapError::TooLong => write!(f, "shortcut is longer than {MAX_SEQ_LEN} keys"),
            KeymapError::TooMany => write!(f, "too many shortcuts"),
            KeymapError::UnknownCommand(c) => write!(f, "unknown command '{c}'"),
            KeymapError::Conflict { shortcut, with_command } => write!(f, "'{shortcut}' conflicts with a shortcut of '{with_command}'"),
            KeymapError::SelfConflict(s) => write!(f, "'{s}' clashes with another shortcut of the same command"),
            KeymapError::TooBig => write!(f, "text too large"),
            KeymapError::BadLine(n) => write!(f, "line {n} is not valid"),
        }
    }
}
impl std::error::Error for KeymapError {}

fn valid_key(k: &str) -> bool {
    if k.len() == 1 { return k.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()); }
    if let Some(n) = k.strip_prefix('F') {
        if let Ok(v) = n.parse::<u8>() { return (1..=24).contains(&v) && k == format!("F{v}"); }
    }
    NAMED.contains(&k)
}

impl Chord {
    pub fn new(mods: Mods, key: &str) -> Result<Chord, KeymapError> {
        let key = if key.len() == 1 { key.to_ascii_uppercase() } else { key.to_string() };
        if !valid_key(&key) { return Err(KeymapError::BadKey(key)); }
        Ok(Chord { mods, key })
    }
    /// "Ctrl+Shift+Z", "F8", "Escape"
    pub fn parse(s: &str) -> Result<Chord, KeymapError> {
        let mut mods = 0u8;
        let parts: Vec<&str> = s.split('+').collect();
        let (key, mod_parts) = parts.split_last().ok_or(KeymapError::EmptySequence)?;
        for m in mod_parts {
            let bit = match m.to_ascii_lowercase().as_str() { "ctrl" => 1, "shift" => 2, "alt" => 4, _ => return Err(KeymapError::BadKey(s.into())) };
            if mods & bit != 0 { return Err(KeymapError::BadKey(s.into())); }
            mods |= bit;
        }
        Chord::new(Mods(mods), key)
    }
}
impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.mods.0 & 1 != 0 { write!(f, "Ctrl+")?; }
        if self.mods.0 & 2 != 0 { write!(f, "Shift+")?; }
        if self.mods.0 & 4 != 0 { write!(f, "Alt+")?; }
        write!(f, "{}", self.key)
    }
}

/// "W A" or "Ctrl+Z" (chords separated by spaces).
pub fn parse_sequence(s: &str) -> Result<Sequence, KeymapError> {
    let seq: Result<Sequence, _> = s.split_whitespace().map(Chord::parse).collect();
    let seq = seq?;
    if seq.is_empty() { return Err(KeymapError::EmptySequence); }
    if seq.len() > MAX_SEQ_LEN { return Err(KeymapError::TooLong); }
    Ok(seq)
}
pub fn format_sequence(s: &[Chord]) -> String { s.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ") }

/// Revit writes letter sequences without separators ("WA") and chords with "+" ("Ctrl+Z", "Shift+F10").
pub fn parse_revit_token(tok: &str) -> Result<Sequence, KeymapError> {
    let tok = tok.trim();
    if tok.is_empty() { return Err(KeymapError::EmptySequence); }
    if tok.contains('+') || tok.contains(' ') { return parse_sequence(tok); }
    // a key's name ("Escape", "Tab", "space") is one key, not letters typed one after the other
    if let Some(name) = NAMED.iter().find(|n| n.eq_ignore_ascii_case(tok)) { return parse_sequence(name); }
    let up = tok.to_ascii_uppercase();
    if valid_key(&up) { return parse_sequence(&up); }                       // one key, or an F-key
    if tok.len() > MAX_SEQ_LEN || !tok.chars().all(|c| c.is_ascii_alphanumeric()) { return Err(KeymapError::BadKey(tok.into())); }
    tok.chars().map(|c| Chord::new(Mods::NONE, &c.to_string())).collect()   // typed one key at a time
}

/// one is the beginning of the other, or they are equal
fn clash(a: &[Chord], b: &[Chord]) -> bool { let n = a.len().min(b.len()); a[..n] == b[..n] }
/// one is the beginning of the other but they are not equal: refused between commands (equal ones share the shortcut)
fn blocks(a: &[Chord], b: &[Chord]) -> bool { a != b && clash(a, b) }

/// a command id and its shortcuts as text
pub type Preset = (String, Vec<String>);

#[derive(Debug, Clone, PartialEq)]
pub struct ImportReport { pub bound: usize, pub skipped: Vec<String> }

#[derive(Debug, Clone, Default)]
pub struct Keymap {
    commands: BTreeSet<String>,
    preset: BTreeMap<String, Vec<Sequence>>,
    overrides: BTreeMap<String, Vec<Sequence>>,
}

impl Keymap {
    pub fn new(commands: &[&str]) -> Result<Keymap, KeymapError> {
        if commands.len() > MAX_COMMANDS { return Err(KeymapError::TooMany); }
        Ok(Keymap { commands: commands.iter().map(|c| c.to_string()).collect(), ..Default::default() })
    }
    pub fn commands(&self) -> impl Iterator<Item = &str> { self.commands.iter().map(|s| s.as_str()) }

    fn effective(&self, command: &str) -> &[Sequence] {
        self.overrides.get(command).or_else(|| self.preset.get(command)).map(|v| v.as_slice()).unwrap_or(&[])
    }
    /// shortcuts in force for a command
    pub fn shortcuts(&self, command: &str) -> Vec<String> { self.effective(command).iter().map(|s| format_sequence(s)).collect() }

    fn check_set(&self, command: &str, seqs: &[Sequence]) -> Result<(), KeymapError> {
        if !self.commands.contains(command) { return Err(KeymapError::UnknownCommand(command.into())); }
        if seqs.len() > MAX_PER_COMMAND { return Err(KeymapError::TooMany); }
        for (i, a) in seqs.iter().enumerate() {
            if a.is_empty() { return Err(KeymapError::EmptySequence); }
            if a.len() > MAX_SEQ_LEN { return Err(KeymapError::TooLong); }
            for b in &seqs[i + 1..] { if clash(a, b) { return Err(KeymapError::SelfConflict(format_sequence(a))); } }
            for (other, os) in self.all_effective() {
                if other == command { continue; }
                if os.iter().any(|o| blocks(a, o)) { return Err(KeymapError::Conflict { shortcut: format_sequence(a), with_command: other.to_string() }); }
            }
        }
        Ok(())
    }
    fn all_effective(&self) -> impl Iterator<Item = (&str, &[Sequence])> {
        self.commands.iter().map(|c| (c.as_str(), self.effective(c)))
    }

    /// Load the default shortcuts. Replaces the preset; overrides are kept (and must still fit, else the call fails and nothing changes).
    pub fn set_preset(&mut self, entries: &[(&str, &[&str])]) -> Result<(), KeymapError> {
        let mut preset: BTreeMap<String, Vec<Sequence>> = BTreeMap::new();
        for (c, ss) in entries {
            if !self.commands.contains(*c) { return Err(KeymapError::UnknownCommand((*c).into())); }
            let seqs = ss.iter().map(|s| parse_sequence(s)).collect::<Result<Vec<_>, _>>()?;
            if preset.insert((*c).to_string(), seqs).is_some() { return Err(KeymapError::SelfConflict((*c).into())); }
        }
        let trial = Keymap { commands: self.commands.clone(), preset, overrides: self.overrides.clone() };
        trial.validate()?;
        *self = trial;
        Ok(())
    }

    /// Check the whole map. Used after loading.
    pub fn validate(&self) -> Result<(), KeymapError> {
        let all: Vec<(&str, &[Sequence])> = self.all_effective().collect();
        for (i, (c, seqs)) in all.iter().enumerate() {
            for (k, a) in seqs.iter().enumerate() {
                for b in &seqs[k + 1..] { if clash(a, b) { return Err(KeymapError::SelfConflict(format_sequence(a))); } }
                for (o, os) in &all[i + 1..] {
                    if os.iter().any(|x| blocks(a, x)) { return Err(KeymapError::Conflict { shortcut: format_sequence(a), with_command: o.to_string() }); }
                }
            }
            let _ = c;
        }
        Ok(())
    }

    /// Settings menu: give a command new shortcuts (an empty list removes them). Refused when one is taken.
    pub fn set(&mut self, command: &str, shortcuts: &[&str]) -> Result<(), KeymapError> {
        let seqs = shortcuts.iter().map(|s| parse_sequence(s)).collect::<Result<Vec<_>, _>>()?;
        self.check_set(command, &seqs)?;
        self.overrides.insert(command.to_string(), seqs);
        Ok(())
    }
    /// Settings menu: "reassign" — take the shortcut from whichever command has it (that command ends up without it).
    pub fn reassign(&mut self, command: &str, shortcut: &str) -> Result<Vec<String>, KeymapError> {
        let seq = parse_sequence(shortcut)?;
        if !self.commands.contains(command) { return Err(KeymapError::UnknownCommand(command.into())); }
        let mut stripped = Vec::new();
        let mut next = self.overrides.clone();
        for c in self.commands.iter().filter(|c| c.as_str() != command) {
            let cur = self.effective(c);
            if cur.iter().any(|o| clash(&seq, o)) {
                next.insert(c.clone(), cur.iter().filter(|o| !clash(&seq, o)).cloned().collect());
                stripped.push(c.clone());
            }
        }
        let mut mine: Vec<Sequence> = self.effective(command).to_vec();
        if mine.iter().any(|m| clash(m, &seq)) { mine.retain(|m| !clash(m, &seq)); }
        mine.push(seq);
        if mine.len() > MAX_PER_COMMAND { return Err(KeymapError::TooMany); }
        next.insert(command.to_string(), mine);
        let trial = Keymap { commands: self.commands.clone(), preset: self.preset.clone(), overrides: next };
        trial.validate()?;
        *self = trial;
        Ok(stripped)
    }
    /// back to the preset for one command; refused if the preset shortcut is now taken by another command
    pub fn reset(&mut self, command: &str) -> Result<(), KeymapError> {
        if !self.commands.contains(command) { return Err(KeymapError::UnknownCommand(command.into())); }
        let mut trial = self.clone();
        trial.overrides.remove(command);
        trial.validate()?;
        *self = trial;
        Ok(())
    }
    pub fn reset_all(&mut self) { self.overrides.clear(); }
    pub fn is_customised(&self, command: &str) -> bool { self.overrides.contains_key(command) }

    /// Which command does this exact sequence run? `None` when none does, and also when more than one does
    /// (use [`Keymap::commands_for`] for those).
    pub fn command_for(&self, shortcut: &str) -> Option<&str> {
        let all = self.commands_for(shortcut);
        if all.len() == 1 { Some(all[0]) } else { None }
    }
    /// Every command that has this exact sequence, in the order of their ids.
    pub fn commands_for(&self, shortcut: &str) -> Vec<&str> {
        let Ok(seq) = parse_sequence(shortcut) else { return Vec::new() };
        self.all_effective().filter(|(_, ss)| ss.contains(&seq)).map(|(c, _)| c).collect()
    }
    /// For the settings menu: each shortcut of the command that another command has too, with those commands.
    pub fn shared(&self, command: &str) -> Vec<(String, Vec<String>)> {
        let mut out = Vec::new();
        for s in self.effective(command) {
            let others: Vec<String> = self.all_effective().filter(|(c, ss)| *c != command && ss.contains(s)).map(|(c, _)| c.to_string()).collect();
            if !others.is_empty() { out.push((format_sequence(s), others)); }
        }
        out
    }

    /// Typed keys, one at a time. Create with `matcher()`.
    pub fn matcher(&self) -> Matcher<'_> { Matcher { map: self, typed: Vec::new() } }

    // ---- settings file: one line per customised command: `command<TAB>shortcut|shortcut`  (empty = removed)
    pub fn save_overrides(&self) -> String {
        let mut out = String::new();
        for (c, ss) in &self.overrides { out.push_str(c); out.push('\t'); out.push_str(&ss.iter().map(|s| format_sequence(s)).collect::<Vec<_>>().join("|")); out.push('\n'); }
        out
    }
    /// All or nothing: a bad or conflicting file changes nothing.
    pub fn load_overrides(&mut self, text: &str) -> Result<(), KeymapError> {
        if text.len() > MAX_TEXT { return Err(KeymapError::TooBig); }
        let mut next = BTreeMap::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() { continue; }
            let (c, rest) = line.split_once('\t').ok_or(KeymapError::BadLine(n + 1))?;
            if !self.commands.contains(c) { return Err(KeymapError::UnknownCommand(c.into())); }
            let seqs = if rest.is_empty() { vec![] } else { rest.split('|').map(parse_sequence).collect::<Result<Vec<_>, _>>()? };
            if next.insert(c.to_string(), seqs).is_some() { return Err(KeymapError::BadLine(n + 1)); }
        }
        let trial = Keymap { commands: self.commands.clone(), preset: self.preset.clone(), overrides: next };
        trial.validate()?;
        *self = trial;
        Ok(())
    }

    /// Read a Revit `KeyboardShortcuts.xml` into (our command id, shortcuts) pairs. `map` gives, for a Revit CommandId,
    /// our command id; entries without a mapping, without a shortcut, or with an invalid one are listed in `skipped`.
    pub fn parse_revit_xml(text: &str, map: &dyn Fn(&str) -> Option<String>) -> Result<(Vec<Preset>, ImportReport), KeymapError> {
        if text.len() > MAX_TEXT { return Err(KeymapError::TooBig); }
        let mut out: Vec<Preset> = Vec::new();
        let mut skipped = Vec::new();
        let mut rest = text;
        while let Some(p) = rest.find("<ShortcutItem") {
            rest = &rest[p + "<ShortcutItem".len()..];
            let end = rest.find('>').unwrap_or(rest.len());
            let tag = &rest[..end];
            let id = attr(tag, "CommandId").unwrap_or_default();
            let name = attr(tag, "CommandName").unwrap_or_default();
            let label = if name.is_empty() { id.clone() } else { name };
            let Some(sc) = attr(tag, "Shortcuts").filter(|s| !s.trim().is_empty()) else { continue };
            let Some(ours) = map(&id) else { skipped.push(format!("{label}: no matching command")); continue };
            let mut good = Vec::new();
            for tok in sc.split('#') {
                match parse_revit_token(tok) { Ok(s) => good.push(format_sequence(&s)), Err(e) => skipped.push(format!("{label} '{tok}': {e}")) }
            }
            if !good.is_empty() { out.push((ours, good)); }
        }
        let bound = out.len();
        Ok((out, ImportReport { bound, skipped }))
    }
}

/// Read a preset file: `id<TAB>shortcut#shortcut<TAB>name`, `#` at the start of a line is a comment.
pub fn parse_preset_text(text: &str) -> Result<Vec<Preset>, KeymapError> {
    if text.len() > MAX_TEXT { return Err(KeymapError::TooBig); }
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') { continue; }
        let mut f = line.split('\t');
        let (Some(id), Some(sc)) = (f.next(), f.next()) else { return Err(KeymapError::BadLine(n + 1)) };
        let mut list = Vec::new();
        for tok in sc.split('#') { list.push(format_sequence(&parse_revit_token(tok)?)); }
        out.push((id.to_string(), list));
    }
    Ok(out)
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let mut from = 0;
    while let Some(p) = tag[from..].find(&key) {
        let s = from + p;
        if s == 0 || tag.as_bytes()[s - 1].is_ascii_whitespace() {
            let v = &tag[s + key.len()..];
            return v.find('"').map(|e| v[..e].replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\""));
        }
        from = s + key.len();
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub enum Match {
    /// not the start of any shortcut: typing starts over
    None,
    /// the start of a longer shortcut: keep typing
    Pending,
    /// a whole shortcut of one command
    Command(String),
    /// a whole shortcut of several commands, in the order of their ids: the user picks one (arrow keys, Enter)
    Choice(Vec<String>),
}

pub struct Matcher<'a> { map: &'a Keymap, typed: Sequence }
impl Matcher<'_> {
    pub fn feed(&mut self, chord: Chord) -> Match {
        self.typed.push(chord);
        let mut pending = false;
        let mut whole: Vec<String> = Vec::new();
        for (c, ss) in self.map.all_effective() {
            for s in ss {
                if *s == self.typed { whole.push(c.to_string()); }
                if s.len() > self.typed.len() && s[..self.typed.len()] == self.typed[..] { pending = true; }
            }
        }
        // a whole shortcut cannot also begin a longer one (the map refuses that), so nothing is pending here
        match whole.len() {
            0 if pending => Match::Pending,
            0 => { self.typed.clear(); Match::None }
            1 => { self.typed.clear(); Match::Command(whole.remove(0)) }
            _ => { self.typed.clear(); Match::Choice(whole) }
        }
    }
    pub fn cancel(&mut self) { self.typed.clear(); }
}
