//! The settings of the application itself (not of a model): kept in one small text file in the user's configuration
//! folder, `key=value` per line. A missing or unreadable file means the defaults, and an unknown line is ignored, so
//! an older or newer version can share it.

/// Longest interval offered, minutes.
pub(crate) const MAX_AUTOSAVE_MINUTES: u32 = 120;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AppSettings {
    /// minutes between automatic saves of unsaved work; 0 switches it off
    pub autosave_minutes: u32,
    /// also write the model's own file at that interval (only a model that already has a file), not just the journal
    pub autosave_to_file: bool,
}

impl Default for AppSettings {
    fn default() -> Self { Self { autosave_minutes: 1, autosave_to_file: false } }
}

impl AppSettings {
    pub(crate) fn to_text(&self) -> String {
        format!("autosave_minutes={}\nautosave_to_file={}\n", self.autosave_minutes, u8::from(self.autosave_to_file))
    }

    pub(crate) fn from_text(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "autosave_minutes" => if let Ok(m) = v.trim().parse::<u32>() { s.autosave_minutes = m.min(MAX_AUTOSAVE_MINUTES); },
                "autosave_to_file" => s.autosave_to_file = v.trim() == "1",
                _ => {}
            }
        }
        s
    }

    /// The settings file of this user, when the system says where the configuration folder is.
    pub(crate) fn path() -> Option<std::path::PathBuf> {
        let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
        let base = if cfg!(windows) { var("APPDATA") }
            else if cfg!(target_os = "macos") { var("HOME").map(|h| h.join("Library/Application Support")) }
            else { var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config"))) };
        base.map(|b| b.join("openbim").join("settings.txt"))
    }

    pub(crate) fn load() -> Self {
        Self::path().and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(Self::default, |t| Self::from_text(&t))
    }

    /// Writes the settings; returns whether it could (a read-only home leaves the choice for this session only).
    pub(crate) fn save(&self) -> bool {
        let Some(p) = Self::path() else { return false };
        p.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(p, self.to_text()).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_default_when_unreadable_and_are_bounded() {
        let s = AppSettings { autosave_minutes: 5, autosave_to_file: true };
        assert_eq!(AppSettings::from_text(&s.to_text()), s);
        assert_eq!(AppSettings::from_text(""), AppSettings::default());
        assert_eq!(AppSettings::from_text("garbage\nautosave_minutes=abc\nfuture_key=1\n"), AppSettings::default());
        assert_eq!(AppSettings::from_text("autosave_minutes=99999").autosave_minutes, MAX_AUTOSAVE_MINUTES);
        assert_eq!(AppSettings::from_text("autosave_minutes=0").autosave_minutes, 0);
    }
}
