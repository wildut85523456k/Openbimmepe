//! Mouse controls: which gesture (a button dragged or clicked, or the wheel, with modifier keys held) does which
//! navigation action. Like the shortcuts they are data read from a preset file, not code, and a gesture does one
//! thing only (unlike shortcuts there is no list to pick from while the mouse is moving).
use crate::{KeymapError, Mods, MAX_COMMANDS, MAX_TEXT};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Button {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Input {
    Drag(Button),
    Click(Button),
    DoubleClick(Button),
    /// the wheel turned, either way (the sign of the turn is the action's own business)
    Wheel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Gesture {
    pub mods: Mods,
    pub input: Input,
}

impl Gesture {
    /// "MiddleDrag", "Wheel", "Shift+MiddleDrag", "Ctrl+LeftClick", "MiddleDoubleClick"
    pub fn parse(s: &str) -> Result<Gesture, KeymapError> {
        let bad = || KeymapError::BadKey(s.into());
        let parts: Vec<&str> = s.split('+').collect();
        let (last, mod_parts) = parts.split_last().ok_or_else(bad)?;
        let mut mods = 0u8;
        for m in mod_parts {
            let bit = match m.to_ascii_lowercase().as_str() {
                "ctrl" => 1,
                "shift" => 2,
                "alt" => 4,
                _ => return Err(bad()),
            };
            if mods & bit != 0 {
                return Err(bad());
            }
            mods |= bit;
        }
        let button = |name: &str| match name {
            "Left" => Some(Button::Left),
            "Middle" => Some(Button::Middle),
            "Right" => Some(Button::Right),
            _ => None,
        };
        let input = if *last == "Wheel" {
            Input::Wheel
        } else if let Some(b) = last.strip_suffix("DoubleClick").and_then(button) {
            Input::DoubleClick(b)
        } else if let Some(b) = last.strip_suffix("Click").and_then(button) {
            Input::Click(b)
        } else if let Some(b) = last.strip_suffix("Drag").and_then(button) {
            Input::Drag(b)
        } else {
            return Err(bad());
        };
        Ok(Gesture { mods: Mods(mods), input })
    }
}

impl std::fmt::Display for Gesture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.mods.0 & 1 != 0 {
            write!(f, "Ctrl+")?;
        }
        if self.mods.0 & 2 != 0 {
            write!(f, "Shift+")?;
        }
        if self.mods.0 & 4 != 0 {
            write!(f, "Alt+")?;
        }
        let b = |b: &Button| match b {
            Button::Left => "Left",
            Button::Middle => "Middle",
            Button::Right => "Right",
        };
        match &self.input {
            Input::Wheel => write!(f, "Wheel"),
            Input::Drag(x) => write!(f, "{}Drag", b(x)),
            Input::Click(x) => write!(f, "{}Click", b(x)),
            Input::DoubleClick(x) => write!(f, "{}DoubleClick", b(x)),
        }
    }
}

/// Mouse gestures and what they do, the preset with the user's changes on top.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MouseMap {
    preset: BTreeMap<Gesture, String>,
    overrides: BTreeMap<Gesture, Option<String>>,
}

impl MouseMap {
    /// Read a preset file: `gesture<TAB>action<TAB>name`, `#` at the start of a line is a comment. A gesture twice is refused.
    pub fn from_preset_text(text: &str) -> Result<MouseMap, KeymapError> {
        if text.len() > MAX_TEXT {
            return Err(KeymapError::TooBig);
        }
        let mut preset = BTreeMap::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let mut f = line.split('\t');
            let (Some(g), Some(action)) = (f.next(), f.next()) else { return Err(KeymapError::BadLine(n + 1)) };
            if action.trim().is_empty() {
                return Err(KeymapError::BadLine(n + 1));
            }
            let g = Gesture::parse(g)?;
            if preset.insert(g, action.trim().to_string()).is_some() {
                return Err(KeymapError::SelfConflict(g.to_string()));
            }
            if preset.len() > MAX_COMMANDS {
                return Err(KeymapError::TooMany);
            }
        }
        Ok(MouseMap { preset, overrides: BTreeMap::new() })
    }

    /// What the gesture does, if anything.
    pub fn action(&self, g: Gesture) -> Option<&str> {
        match self.overrides.get(&g) {
            Some(o) => o.as_deref(),
            None => self.preset.get(&g).map(|s| s.as_str()),
        }
    }
    /// Settings menu: make a gesture do an action, replacing what it did.
    pub fn set(&mut self, g: Gesture, action: &str) -> Result<(), KeymapError> {
        if action.trim().is_empty() {
            return Err(KeymapError::EmptySequence);
        }
        self.overrides.insert(g, Some(action.trim().to_string()));
        Ok(())
    }
    /// Settings menu: make a gesture do nothing.
    pub fn clear(&mut self, g: Gesture) {
        self.overrides.insert(g, None);
    }
    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }
    /// The gestures of an action, in order.
    pub fn gestures_for(&self, action: &str) -> Vec<Gesture> {
        let mut all: Vec<Gesture> = self.preset.keys().chain(self.overrides.keys()).copied().collect();
        all.sort();
        all.dedup();
        all.into_iter().filter(|g| self.action(*g) == Some(action)).collect()
    }
}
