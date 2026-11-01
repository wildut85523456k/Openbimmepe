//! The settings of the application itself (not of a model): kept in one small text file in the user's configuration
//! folder, `key=value` per line. A missing or unreadable file means the defaults, and an unknown line is ignored, so
//! an older or newer version can share it.

/// Longest interval offered, minutes.
pub(crate) const MAX_AUTOSAVE_MINUTES: u32 = 120;

/// Light/Dark application theme, persisted as `theme=light|dark`. A missing or unknown value means Light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Theme { Light, Dark }

impl Default for Theme { fn default() -> Self { Theme::Light } }

impl Theme {
    pub(crate) fn is_dark(self) -> bool { matches!(self, Theme::Dark) }
    pub(crate) fn name(self) -> &'static str { match self { Theme::Light => "light", Theme::Dark => "dark" } }
}

/// Which screen edge a dockable side panel is pinned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DockEdge { Left, Right, Top, Bottom }

impl Default for DockEdge { fn default() -> Self { DockEdge::Left } }

impl DockEdge {
    pub(crate) fn name(self) -> &'static str { match self { DockEdge::Left => "left", DockEdge::Right => "right", DockEdge::Top => "top", DockEdge::Bottom => "bottom" } }
    pub(crate) fn from_name(s: &str) -> Self { match s { "right" => DockEdge::Right, "top" => DockEdge::Top, "bottom" => DockEdge::Bottom, _ => DockEdge::Left } }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AppSettings {
    /// minutes between automatic saves of unsaved work; 0 switches it off
    pub autosave_minutes: u32,
    /// also write the model's own file at that interval (only a model that already has a file), not just the journal
    pub autosave_to_file: bool,
    /// light/dark application theme (persisted; defaults to Light)
    pub theme: Theme,
    /// keyboard-shortcut overrides, one line per customised command (escaped single line); empty = Revit defaults
    pub keymap_overrides: String,
    /// width of the left docking sidebar (browser + properties), persisted
    pub sidebar_width: f32,
    /// fraction of the sidebar height given to the top Properties strip vs the Browser, persisted
    pub properties_split: f32,
    /// the edge each dockable side panel (Properties, Browser, Warnings) is pinned to, persisted
    pub panel_edges: [DockEdge; 3],
    /// whether each dockable side panel (Properties, Browser, Warnings) is shown, persisted
    pub panel_visible: [bool; 3],
    /// cross-size (width for Left/Right, height for Top/Bottom) of each screen edge's dock group, persisted
    pub edge_sizes: [f32; 4],
}

impl Default for AppSettings {
    fn default() -> Self { Self { autosave_minutes: 1, autosave_to_file: false, theme: Theme::Light, keymap_overrides: String::new(), sidebar_width: 310.0, properties_split: 0.55, panel_edges: [DockEdge::Left, DockEdge::Left, DockEdge::Right], panel_visible: [true, true, false], edge_sizes: [310.0, 360.0, 240.0, 200.0] } }
}

impl AppSettings {
    pub(crate) fn to_text(&self) -> String {
        let escaped = self.keymap_overrides.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t");
        let edges = self.panel_edges.iter().map(|e| e.name()).collect::<Vec<_>>().join(",");
        let vis = self.panel_visible.iter().map(|v| if *v { "1" } else { "0" }).collect::<Vec<_>>().join(",");
        let sizes = self.edge_sizes.iter().map(|s| format!("{s:.1}")).collect::<Vec<_>>().join(",");
        format!("autosave_minutes={}\nautosave_to_file={}\ntheme={}\nkeymap_overrides={}\nsidebar_width={:.1}\nproperties_split={:.3}\npanel_edges={}\npanel_visible={}\nedge_sizes={}\n", self.autosave_minutes, u8::from(self.autosave_to_file), self.theme.name(), escaped, self.sidebar_width, self.properties_split, edges, vis, sizes)
    }

    pub(crate) fn from_text(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "autosave_minutes" => if let Ok(m) = v.trim().parse::<u32>() { s.autosave_minutes = m.min(MAX_AUTOSAVE_MINUTES); },
                "autosave_to_file" => s.autosave_to_file = v.trim() == "1",
                "theme" => s.theme = match v.trim() { "dark" => Theme::Dark, _ => Theme::Light },
                "keymap_overrides" => {
                    let unesc = v.replace("\\\\", "\x00").replace("\\n", "\n").replace("\\t", "\t").replace("\x00", "\\");
                    s.keymap_overrides = unesc;
                }
                "sidebar_width" => if let Ok(w) = v.trim().parse::<f32>() { s.sidebar_width = w.clamp(260.0, 520.0); },
                "properties_split" => if let Ok(f) = v.trim().parse::<f32>() { s.properties_split = f.clamp(0.1, 0.9); },
                "panel_edges" => {
                    for (i, part) in v.split(',').enumerate() {
                        if i < 3 { s.panel_edges[i] = DockEdge::from_name(part.trim()); }
                    }
                }
                "panel_visible" => {
                    for (i, part) in v.split(',').enumerate() {
                        if i < 3 { s.panel_visible[i] = part.trim() == "1"; }
                    }
                }
                "edge_sizes" => {
                    for (i, part) in v.split(',').enumerate() {
                        if i < 4 { if let Ok(z) = part.trim().parse::<f32>() { s.edge_sizes[i] = z; } }
                    }
                }
                _ => {}
            }
        }
        s
    }

    /// The settings file of this user, when the system says where the configuration folder is.
    pub(crate) fn path() -> Option<std::path::PathBuf> {
        // a test never reads or writes the settings of the user who runs it
        if cfg!(test) { return None; }
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
        let s = AppSettings { autosave_minutes: 5, autosave_to_file: true, ..Default::default() };
        assert_eq!(AppSettings::from_text(&s.to_text()), s);
        assert_eq!(AppSettings::from_text(""), AppSettings::default());
        assert_eq!(AppSettings::from_text("garbage\nautosave_minutes=abc\nfuture_key=1\n"), AppSettings::default());
        assert_eq!(AppSettings::from_text("autosave_minutes=99999").autosave_minutes, MAX_AUTOSAVE_MINUTES);
        assert_eq!(AppSettings::from_text("autosave_minutes=0").autosave_minutes, 0);
    }

    #[test]
    fn theme_round_trip_and_defaults() {
        assert_eq!(AppSettings::from_text("theme=dark").theme, Theme::Dark);
        assert_eq!(AppSettings::from_text("theme=light").theme, Theme::Light);
        assert_eq!(AppSettings::from_text("theme=neon").theme, Theme::Light, "unknown value keeps the default");
        assert_eq!(AppSettings::from_text("").theme, Theme::Light, "missing line keeps the default");
        assert_eq!(AppSettings::from_text(&AppSettings::default().to_text()).theme, Theme::Light);
        assert_eq!(AppSettings::from_text(&AppSettings { theme: Theme::Dark, ..Default::default() }.to_text()).theme, Theme::Dark);
        assert!(Theme::Dark.is_dark() && !Theme::Light.is_dark());
        assert_eq!(Theme::Light.name(), "light");
        assert_eq!(Theme::Dark.name(), "dark");
    }

    #[test]
    fn dock_widths_round_trip_defaults_and_clamping() {
        assert_eq!(AppSettings::default().sidebar_width, 310.0);
        assert_eq!(AppSettings::default().properties_split, 0.55);
        let s = AppSettings { sidebar_width: 420.0, properties_split: 0.7, ..Default::default() };
        assert_eq!(AppSettings::from_text(&s.to_text()), s, "dock widths round-trip");
        assert_eq!(AppSettings::from_text("sidebar_width=600").sidebar_width, 520.0, "clamped to max");
        assert_eq!(AppSettings::from_text("sidebar_width=100").sidebar_width, 260.0, "clamped to min");
        assert_eq!(AppSettings::from_text("properties_split=0.95").properties_split, 0.9, "clamped to max");
        assert_eq!(AppSettings::from_text("properties_split=0.02").properties_split, 0.1, "clamped to min");
    }
}
