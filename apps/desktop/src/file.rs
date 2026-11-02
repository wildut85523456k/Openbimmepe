//! The app's own file: the document as the lines of `openbim_document::native`, one per element, in a `.obim` text
//! file. Saving writes a temporary file beside the target and renames it, so a failed write never damages the file
//! that was there.
use crate::Desktop;
use eframe::egui;
use openbim_document::{native, Document};

/// The text of a saved file.
pub(crate) fn file_text(doc: &Document) -> String {
    let mut t = native::records(doc).join("\n");
    t.push('\n');
    t
}

/// The document a saved file holds, with no undo history.
pub(crate) fn document_from(text: &str) -> Result<Document, String> {
    let lines: Vec<String> = text.lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect();
    let (elements, site) = native::parse_records(&lines)?;
    let mep = native::parse_mep(&lines)?;
    let mut doc = Document::new();
    let mut ops = native::restore_commands(elements, site);
    if !mep.is_empty() { ops.push(openbim_document::Command::SetMep(Box::new(mep))); }
    doc.transact(0, &ops).map_err(|e| format!("the file does not make a model: {e:?}"))?;
    doc.clear_undo_history();
    doc.set_project_key(native::parse_project_key(&lines));
    doc.set_app_records(native::parse_app_records(&lines));
    doc.set_family_defs(native::parse_family_defs(&lines));
    doc.set_shared_params(native::parse_shared_params(&lines));
    Ok(doc)
}

/// What the file name, the open question and the unsaved edits are.
#[derive(Default)]
pub(crate) struct FileState {
    pub path: Option<String>,
    /// the revision that is on disk
    pub saved_revision: u64,
    /// a file chosen to open while there were unsaved changes: waits for the answer
    pub confirm_open: Option<String>,
    /// "New" asked while there were unsaved changes
    pub confirm_new: bool,
    /// the revision the recovery copy holds, and when it was written (B20)
    pub autosaved_revision: Option<u64>,
    pub last_autosave: Option<std::time::Instant>,
    /// what the journal on disk holds: its state, the number of its last transaction, how many it has and its size
    pub journal: Option<JournalState>,
    /// a recovery copy newer than the file it belongs to, waiting for the answer to recover or discard it
    pub recovery: Option<String>,
}

/// The journal as this session wrote it.
#[derive(Clone)]
pub(crate) struct JournalState { pub state: openbim_oplog::State, pub seq: u64, pub transactions: usize, pub bytes: usize }

/// A journal is folded into a new base when it holds this many transactions or this many bytes.
const COMPACT_TRANSACTIONS: usize = 500;
const COMPACT_BYTES: usize = 8 << 20;


impl Desktop {
    pub(crate) fn is_dirty(&self) -> bool { self.document.revision() != self.file.saved_revision }

    /// Writes the model to its file, asking for a name when it has none (or when `ask` is set).
    pub(crate) fn save_file(&mut self, ask: bool) {
        let path = match (&self.file.path, ask) {
            (Some(p), false) => p.clone(),
            _ => {
                let mut fd = crate::dialog::Question::new().set_title("Save model").add_filter("OpenBIM model", &["obim"]);
                if let Some(name) = self.file.path.as_deref().and_then(|p| std::path::Path::new(p).file_name()).and_then(|n| n.to_str()) { fd = fd.set_file_name(name); } else { fd = fd.set_file_name("model.obim"); }
                let Some(p) = fd.ask_save() else { return };
                let mut p = p.display().to_string();
                if !p.to_lowercase().ends_with(".obim") { p.push_str(".obim"); }
                p
            }
        };
        self.write_model(&path);
    }

    pub(crate) fn write_model(&mut self, path: &str) {
        self.document.ensure_project_key();
        self.sync_app_records();
        let tmp = format!("{path}.saving");
        let result = std::fs::write(&tmp, file_text(&self.document)).and_then(|()| std::fs::rename(&tmp, path));
        self.message = match result {
            Ok(()) => {
                // what was saved supersedes the recovery copies of this file and of the model before it had a name
                let _ = std::fs::remove_file(Self::recovery_path(Some(path)));
                let _ = std::fs::remove_file(Self::recovery_path(None));
                self.file.path = Some(path.into()); self.file.saved_revision = self.document.revision(); self.file.autosaved_revision = None; self.file.journal = None;
                format!("Saved {path}")
            }
            Err(e) => { let _ = std::fs::remove_file(&tmp); format!("Could not save {path}: {e}") }
        };
    }

    /// Where the journal of a model is kept: beside its file, or in the temporary folder while it has no name (B20).
    pub(crate) fn recovery_path(file: Option<&str>) -> std::path::PathBuf {
        match file { Some(p) => std::path::PathBuf::from(format!("{p}.journal")), None => std::env::temp_dir().join("openbim-untitled.journal") }
    }

    /// The automatic save the user chose in Options (every N minutes, 0 = never): the journal of the unsaved work, or,
    /// when asked and the model has a file, the file itself.
    pub(crate) fn autosave(&mut self) {
        let minutes = self.settings.autosave_minutes;
        if minutes == 0 { return; }
        let interval = std::time::Duration::from_secs(u64::from(minutes) * 60);
        if let (true, Some(path)) = (self.settings.autosave_to_file, self.file.path.clone()) {
            if self.is_dirty() && self.file.last_autosave.is_none_or(|t| crate::clock::now() - t >= interval) {
                self.write_model(&path);
                self.file.last_autosave = Some(crate::clock::now());
                if self.message.starts_with("Saved") { self.message = format!("Autosaved {path}"); }
            }
            return;
        }
        self.autosave_tick(interval);
    }

    /// The Options window: how often unsaved work is saved automatically. A change is kept for the next session.
    pub(crate) fn options_window(&mut self, ctx: &egui::Context) {
        if !self.show_options { return; }
        let before = self.settings.clone();
        let mut open = true;
        egui::Window::new("Options").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            let mut on = self.settings.autosave_minutes > 0;
            let r = ui.checkbox(&mut on, "Save automatically");
            test_hit!(self, r, "options:autosave");
            if on && self.settings.autosave_minutes == 0 { self.settings.autosave_minutes = 1; }
            if !on { self.settings.autosave_minutes = 0; }
            ui.add_enabled_ui(on, |ui| {
                ui.horizontal(|ui| {
                    ui.label("every");
                    ui.add(egui::DragValue::new(&mut self.settings.autosave_minutes).range(1..=crate::app_settings::MAX_AUTOSAVE_MINUTES).suffix(" min"));
                });
                ui.checkbox(&mut self.settings.autosave_to_file, "Write the model file itself (a model that has a file)")
                    .on_hover_text("Off: the unsaved work is kept in a recovery journal beside the file and the file changes only when you save. On: the file is overwritten each time.");
            });
            if on && !self.settings.autosave_to_file { ui.small("Unsaved work is kept in a recovery journal; the file changes only when you save."); }

            ui.separator();
            ui.label(egui::RichText::new("Customisation").heading());
            let theme = self.settings.theme;
            ui.horizontal(|ui| {
                ui.label("Theme");
                ui.selectable_value(&mut self.settings.theme, crate::app_settings::Theme::Light, "Light");
                ui.selectable_value(&mut self.settings.theme, crate::app_settings::Theme::Dark, "Dark");
            });
            if self.settings.theme != theme {
                self.dark = self.settings.theme.is_dark();
                self.apply_theme(ui.ctx());
            }

            ui.separator();
            ui.label(egui::RichText::new("Keyboard (Revit defaults)").heading());
            ui.small("Shortcuts match Revit's defaults. Middle-drag pans, Shift+Middle-drag orbits, wheel zooms.");
            egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                for cmd in self.keymap.commands() {
                    let sc = self.keymap.shortcuts(cmd).join(", ");
                    ui.horizontal(|ui| { ui.label(format!("{cmd}:")); ui.label(if sc.is_empty() { "(none)".into() } else { sc }); });
                }
            });
            if ui.button("Reset all shortcuts to Revit defaults").clicked() {
                self.keymap.reset_all();
                self.settings.keymap_overrides = self.keymap.save_overrides();
                self.settings.save();
                self.message = "Keyboard shortcuts reset to Revit defaults".into();
            }
        });
        if !open { self.show_options = false; }
        if self.settings != before && !self.settings.save() { self.message = "The options could not be written; they hold for this session".into(); }
    }

    /// Keeps the unsaved work in the journal when the model has changes that neither the file nor the journal hold and
    /// `interval` has passed. The first time it writes the whole model as the base of the journal (written whole and
    /// renamed into place); after that it appends only the records that changed, which is cheap and which a crash in
    /// the middle cannot spoil beyond the last record. The file itself is never touched. Returns whether it wrote.
    pub(crate) fn autosave_tick(&mut self, interval: std::time::Duration) -> bool {
        use std::io::Write;
        let revision = self.document.revision();
        if !self.is_dirty() || self.file.autosaved_revision == Some(revision) { return false; }
        if self.file.last_autosave.is_some_and(|t| crate::clock::now() - t < interval) { return false; }
        self.document.ensure_project_key();
        self.sync_app_records();
        let now = crate::journal::state_of(&native::records(&self.document));
        let path = Self::recovery_path(self.file.path.as_deref());
        self.file.last_autosave = Some(crate::clock::now());
        let ops = self.file.journal.as_ref().map(|j| crate::journal::diff(&j.state, &now));
        let wrote = match (&self.file.journal, ops) {
            (Some(j), Some(ops)) if j.transactions < COMPACT_TRANSACTIONS && j.bytes < COMPACT_BYTES => {
                if ops.is_empty() { true } else {
                    let seq = j.seq + 1;
                    let frame = crate::journal::tx_bytes(seq, "autosave", ops);
                    let ok = std::fs::OpenOptions::new().append(true).open(&path).and_then(|mut f| { f.write_all(&frame)?; f.sync_data() }).is_ok();
                    if ok { if let Some(j) = self.file.journal.as_mut() { j.state = now; j.seq = seq; j.transactions += 1; j.bytes += frame.len(); } }
                    ok
                }
            }
            _ => {
                // the base: the whole model, the earlier transactions folded in
                let base = crate::journal::base_bytes(0, &now);
                let tmp = path.with_extension("journal.saving");
                let ok = std::fs::write(&tmp, &base).and_then(|()| std::fs::rename(&tmp, &path)).is_ok();
                if ok { self.file.journal = Some(JournalState { state: now, seq: 0, transactions: 0, bytes: base.len() }); } else { let _ = std::fs::remove_file(&tmp); }
                ok
            }
        };
        if wrote { self.file.autosaved_revision = Some(revision); }
        wrote
    }

    /// Looks for a journal newer than `file` (or, with no file, any journal of an unnamed model) and asks about it.
    pub(crate) fn check_recovery(&mut self, file: Option<&str>) {
        let copy = Self::recovery_path(file);
        let modified = |p: &std::path::Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let Some(newest) = modified(&copy) else { return };
        if file.and_then(|f| modified(std::path::Path::new(f))).is_some_and(|t| t > newest) { return; }
        self.file.recovery = Some(copy.display().to_string());
        self.message = "Unsaved work from an earlier session was found: recover it or discard it".into();
    }

    /// Takes the journal as the model: it stays unsaved, so the file is not replaced until the user saves. If the end
    /// of the journal is damaged, the work up to there is recovered and the message says so.
    pub(crate) fn recover(&mut self) {
        let Some(copy) = self.file.recovery.take() else { return };
        let named = self.file.path.clone();
        let loaded = std::fs::read(&copy).map_err(|e| format!("Could not read the journal: {e}")).and_then(|b| crate::journal::read(&b))
            .and_then(|r| { let doc = document_from(&crate::journal::lines_of(&r.state).join("\n"))?; Ok((doc, r.complete, r.transactions, r.seq)) });
        match loaded {
            Ok((doc, complete, transactions, seq)) => {
                self.document = doc;
                self.file = FileState { path: named, saved_revision: u64::MAX, ..Default::default() };
                self.compute.restore(self.document.app_records());
                self.georef = crate::georef_ui::GeorefInput::from_records(self.document.app_records());
                self.selected = None;
                self.sync_duct_controls();
                self.message = format!("Recovered unsaved work ({} elements, {transactions} autosaves, the last numbered {seq}){}; save to keep it", self.document.elements().count(), if complete { "" } else { ", up to a damaged part of the journal" });
            }
            Err(e) => self.message = format!("The journal cannot be used: {e}"),
        }
    }

    /// Throws the recovery copy away.
    pub(crate) fn discard_recovery(&mut self) {
        if let Some(copy) = self.file.recovery.take() { let _ = std::fs::remove_file(copy); }
    }

    /// Asks which file to open, then opens it (after the question about unsaved changes, if there are any).
    pub(crate) fn open_file(&mut self) {
        let Some(p) = crate::dialog::Question::new().set_title("Open model").add_filter("OpenBIM model", &["obim"]).ask_open() else { return };
        self.open_path(&p.display().to_string());
    }

    pub(crate) fn open_path(&mut self, path: &str) {
        if self.is_dirty() { self.file.confirm_open = Some(path.into()); } else { self.load_model(path); }
    }

    /// Replaces the model with the file's. A file that cannot be read leaves the model as it is.
    pub(crate) fn load_model(&mut self, path: &str) {
        let loaded = std::fs::read_to_string(path).map_err(|e| format!("Could not read {path}: {e}")).and_then(|t| document_from(&t).map_err(|e| format!("Could not open {path}: {e}")));
        match loaded {
            Ok(doc) => {
                self.document = doc;
                self.file = FileState { path: Some(path.into()), saved_revision: self.document.revision(), ..Default::default() };
                self.compute.restore(self.document.app_records());
                self.georef = crate::georef_ui::GeorefInput::from_records(self.document.app_records());
                self.selected = None;
                self.sync_duct_controls();
                self.message = format!("Opened {path} ({} elements)", self.document.elements().count());
                self.check_recovery(Some(path));
            }
            Err(e) => self.message = e,
        }
    }

    /// An empty model (after the question about unsaved changes, if there are any).
    pub(crate) fn new_model(&mut self) {
        if self.is_dirty() { self.file.confirm_new = true; return; }
        self.reset_model();
    }

    pub(crate) fn reset_model(&mut self) {
        self.document = Document::new();
        // Revit's default template: Level 1 at 0 and Level 2 at 3 m. Without a level, every drawing tool would point at
        // a level that is not there and the IFC export of the first wall would fail (A2).
        let _ = self.document.place_level("Level 1", 0.0);
        let _ = self.document.place_level("Level 2", 3.0);
        // the template levels are not the user's work: a new, untouched model has nothing to save, so opening a
        // file over it asks nothing (as in Revit)
        self.file = FileState { saved_revision: self.document.revision(), ..Default::default() };
        self.compute = Default::default();
        self.georef = Default::default();
        self.selected = None;
        self.reset_view(crate::View::Plan);
        self.tool = crate::Tool::Select;
        self.message = "New model: Level 1 and Level 2 are in place - draw a wall, or add your own levels".into();
    }

    /// The question asked when a file or a new model would replace unsaved changes.
    pub(crate) fn file_window(&mut self, ctx: &egui::Context) {
        if self.file.recovery.is_some() {
            let (mut recover, mut discard) = (false, false);
            egui::Window::new("Recover unsaved work").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label("The program stopped before this work was saved, and a recovery copy was kept.");
                ui.horizontal(|ui| {
                    let r = ui.button("Recover");
                    test_hit!(self, r, "recovery:recover");
                    recover = r.clicked();
                    discard = ui.button("Discard").clicked();
                });
            });
            if recover { self.recover(); } else if discard { self.discard_recovery(); }
            return;
        }
        if self.file.confirm_open.is_none() && !self.file.confirm_new { return; }
        let (mut save, mut discard, mut cancel) = (false, false, false);
        egui::Window::new("Unsaved changes").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("The model has changes that are not saved.");
            ui.horizontal(|ui| {
                save = ui.button("Save first").clicked();
                discard = ui.button("Discard changes").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if cancel { self.file.confirm_open = None; self.file.confirm_new = false; return; }
        if save {
            self.save_file(false);
            if self.is_dirty() { return; } // not saved (cancelled or failed): keep asking
        }
        if save || discard {
            if let Some(p) = self.file.confirm_open.take() { self.load_model(&p); }
            else if self.file.confirm_new { self.file.confirm_new = false; self.reset_model(); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("obim-file-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_saved_model_opens_equal_with_no_undo_and_a_bad_file_changes_nothing() {
        let mut app = Desktop::sample();
        let d = dir("roundtrip");
        let path = d.join("m.obim");
        let path = path.to_str().unwrap();
        assert!(!app.is_dirty());
        let beam = app.document.place_beam(openbim_document::ElementId(10), &openbim_document::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.3, 0.5, 2.8)).unwrap();
        app.document.create_type(beam, "B 300 x 500").unwrap();
        let size = openbim_place::penetrations::RunSize::Rect { width: 0.4, height: 0.3, roll: 0.0 };
        let route = openbim_route::route_path(&[[1.0, 6.5, 2.7], [5.0, 6.5, 2.7], [5.0, 7.5, 2.7]], &[size, size], &crate::export::tests_support::duct_prefs(), &openbim_route::snaps::Snaps::new(openbim_route::snaps::SnapSettings::off())).unwrap();
        app.document.place_mep_route(openbim_document::ElementId(10), &route).unwrap();
        assert_eq!(app.document.mep().links.len(), 2);
        assert!(app.is_dirty());
        app.write_model(path);
        assert!(!app.is_dirty() && app.message.starts_with("Saved"), "{}", app.message);
        assert!(!std::path::Path::new(&format!("{path}.saving")).exists());
        let want: Vec<_> = app.document.elements().cloned().map(|mut e| { e.revision = 0; e }).collect();

        let mut other = Desktop::sample();
        other.load_model(path);
        let got: Vec<_> = other.document.elements().cloned().map(|mut e| { e.revision = 0; e }).collect();
        assert_eq!(want, got, "{}", other.message);
        assert_eq!(other.document.site(), app.document.site());
        assert_eq!(other.document.mep(), app.document.mep(), "the fittings and links come back");
        assert!(!other.document.can_undo() && !other.is_dirty());
        assert_eq!(other.file.path.as_deref(), Some(path));

        std::fs::write(d.join("bad.obim"), "not a model").unwrap();
        let before = other.document.elements().count();
        other.load_model(d.join("bad.obim").to_str().unwrap());
        assert!(other.message.starts_with("Could not open"), "{}", other.message);
        other.load_model(d.join("missing.obim").to_str().unwrap());
        assert!(other.message.starts_with("Could not read"), "{}", other.message);
        assert_eq!(other.document.elements().count(), before);
        assert_eq!(other.file.path.as_deref(), Some(path), "the open file is still the same one");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn opening_or_starting_over_with_unsaved_changes_asks_first() {
        let mut app = Desktop::sample();
        let d = dir("ask");
        let path = d.join("m.obim");
        let path = path.to_str().unwrap();
        app.write_model(path);
        app.document.place_beam(openbim_document::ElementId(10), &openbim_document::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.3, 0.5, 2.8)).unwrap();
        let n = app.document.elements().count();
        app.open_path(path);
        assert_eq!(app.file.confirm_open.as_deref(), Some(path));
        assert_eq!(app.document.elements().count(), n, "nothing replaced yet");
        app.file.confirm_open = None;
        app.new_model();
        assert!(app.file.confirm_new && app.document.elements().count() == n);
        app.file.confirm_new = false;
        app.write_model(path);
        app.new_model();
        // New gives the project its two levels and nothing else (decision of 21 Oct 2026, A2/A6)
        assert_eq!(app.document.elements().count(), 2, "{}", app.message);
        assert!(app.document.elements().all(|e| e.geometry == Some(openbim_document::Geometry::Level)), "a new model holds its levels and no other element");
        assert!(app.file.path.is_none());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_compute_inputs_come_back_from_the_file_and_a_new_model_starts_clean() {
        let mut app = Desktop::sample();
        assert!(app.compute.record().is_none(), "nothing typed, nothing written");
        app.compute.use_fluid(true);
        app.compute.catalogue = "a|b 300x200,300,200\n500x300,500,300\n100%".into();
        app.compute.loading = "1,0.1\n2,0.2\n".into();
        app.compute.k = [0.4, 0.2, 0.6, 0.7];
        app.compute.count_static = true;
        app.compute.system = openbim_family::mep::System::SupplyAir;
        let path = std::env::temp_dir().join(format!("openbim-compute-{}.obim", std::process::id())).to_str().unwrap().to_string();
        app.write_model(&path);
        let mut back = Desktop::sample();
        assert!(back.compute.record().is_none());
        back.open_path(&path);
        assert_eq!(back.compute.record(), app.compute.record(), "{}", back.message);
        assert_eq!((back.compute.max_velocity, back.compute.k, back.compute.count_static), (8.0, [0.4, 0.2, 0.6, 0.7], true));
        assert_eq!(back.compute.catalogue, app.compute.catalogue, "a bar, a percent sign and line breaks survive");
        assert_eq!(back.compute.loading, "1,0.1\n2,0.2\n");
        // the document stays equal to the one that was saved, and Compute inputs of another file do not stay behind
        assert_eq!(back.document, app.document);
        back.reset_model();
        assert!(back.compute.record().is_none());
        std::fs::remove_file(&path).ok();
    }
    #[test]
    fn unsaved_work_is_kept_in_a_recovery_copy_and_comes_back_after_a_crash() {
        let mut app = Desktop::sample();
        let d = dir("recovery");
        let path = d.join("m.obim");
        let path = path.to_str().unwrap();
        app.write_model(path);
        let copy = Desktop::recovery_path(Some(path));
        // nothing to keep while the model is as saved
        assert!(!app.autosave_tick(std::time::Duration::ZERO) && !copy.exists());
        // a change is written to the copy, not to the file, and only once
        let saved = std::fs::read_to_string(path).unwrap();
        app.document.place_beam(openbim_document::ElementId(10), &openbim_document::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.3, 0.5, 2.8)).unwrap();
        let n = app.document.elements().count();
        assert!(app.autosave_tick(std::time::Duration::ZERO) && copy.exists());
        assert!(!app.autosave_tick(std::time::Duration::ZERO), "the same revision is not written again");
        assert_eq!(std::fs::read_to_string(path).unwrap(), saved, "the file is not touched");
        assert!(!std::path::Path::new(&format!("{}.saving", copy.display())).exists());
        // the interval holds back the next copy
        app.document.place_beam(openbim_document::ElementId(10), &openbim_document::BeamSpec::level([1.0, 2.0], [5.0, 2.0], 0.3, 0.5, 2.8)).unwrap();
        assert!(!app.autosave_tick(std::time::Duration::from_secs(3600)));
        // the program stops; the next one opens the file and is told about the copy
        let mut next = Desktop::sample();
        next.load_model(path);
        assert!(next.file.recovery.is_some(), "{}", next.message);
        let before = next.document.elements().count();
        next.recover();
        assert_eq!(next.document.elements().count(), n);
        assert!(n > before && next.is_dirty(), "the recovered work is unsaved, so the file is not replaced until the user saves");
        assert_eq!(next.file.path.as_deref(), Some(path));
        // saving makes the copy unnecessary
        next.write_model(path);
        assert!(!copy.exists() && !next.is_dirty());
        // a copy can be thrown away
        app.autosave_tick(std::time::Duration::ZERO);
        let mut again = Desktop::sample();
        again.load_model(path);
        if again.file.recovery.is_some() { again.discard_recovery(); }
        assert!(again.file.recovery.is_none() && !copy.exists());
        std::fs::remove_dir_all(&d).unwrap();
    }
    #[test]
    fn later_autosaves_append_only_what_changed_and_a_torn_last_write_loses_only_that_write() {
        let mut app = Desktop::sample();
        let d = dir("journal");
        let path = d.join("m.obim");
        let path = path.to_str().unwrap();
        app.write_model(path);
        let journal = Desktop::recovery_path(Some(path));
        let beam = |app: &mut Desktop, y: f64| { app.document.place_beam(openbim_document::ElementId(10), &openbim_document::BeamSpec::level([1.0, y], [5.0, y], 0.3, 0.5, 2.8)).unwrap(); };
        beam(&mut app, 1.0);
        assert!(app.autosave_tick(std::time::Duration::ZERO));
        let base = std::fs::metadata(&journal).unwrap().len();
        beam(&mut app, 2.0);
        assert!(app.autosave_tick(std::time::Duration::ZERO));
        let after_one = std::fs::metadata(&journal).unwrap().len();
        beam(&mut app, 3.0);
        assert!(app.autosave_tick(std::time::Duration::ZERO));
        let after_two = std::fs::metadata(&journal).unwrap().len();
        let whole = file_text(&app.document).len() as u64;
        assert!(after_one > base && after_two > after_one, "appended, not rewritten");
        assert!(after_two - after_one < whole / 4, "a transaction holds the changed records ({} bytes), not the model ({whole})", after_two - after_one);
        assert_eq!(app.file.journal.as_ref().map(|j| j.transactions), Some(2));
        let n = app.document.elements().count();
        // the program stops: everything is recovered
        let mut next = Desktop::sample();
        next.load_model(path);
        next.recover();
        assert_eq!(next.document.elements().count(), n);
        assert_eq!(next.document.mep(), app.document.mep());
        // the last write is cut short: the work before it is recovered
        let bytes = std::fs::read(&journal).unwrap();
        std::fs::write(&journal, &bytes[..bytes.len() - 4]).unwrap();
        let mut torn = Desktop::sample();
        torn.load_model(path);
        torn.recover();
        assert_eq!(torn.document.elements().count(), n - 1, "{}", torn.message);
        std::fs::remove_dir_all(&d).unwrap();
    }
    #[test]
    fn the_automatic_save_follows_the_options_off_journal_or_the_file_itself() {
        let mut app = Desktop::sample();
        let d = dir("options");
        let path = d.join("m.obim");
        let path = path.to_str().unwrap();
        app.write_model(path);
        let journal = Desktop::recovery_path(Some(path));
        let beam = |app: &mut Desktop| { app.document.place_beam(openbim_document::ElementId(10), &openbim_document::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.3, 0.5, 2.8)).unwrap(); };
        // off: nothing is written
        app.settings.autosave_minutes = 0;
        beam(&mut app);
        app.autosave();
        assert!(!journal.exists());
        // on, to the journal: the file stays as saved
        app.settings.autosave_minutes = 1;
        let saved = std::fs::read_to_string(path).unwrap();
        app.autosave();
        assert!(journal.exists() && std::fs::read_to_string(path).unwrap() == saved && app.is_dirty());
        // on, to the file: the file takes the work, the journal goes, and the model is no longer dirty
        app.settings.autosave_to_file = true;
        app.file.last_autosave = None;
        app.autosave();
        assert!(!app.is_dirty() && !journal.exists() && std::fs::read_to_string(path).unwrap() != saved, "{}", app.message);
        assert!(app.message.starts_with("Autosaved"));
        // a model with no file is still kept in the journal
        let mut fresh = Desktop::sample();
        fresh.settings = crate::app_settings::AppSettings { autosave_minutes: 1, autosave_to_file: true, ..Default::default() };
        beam(&mut fresh);
        fresh.file.path = None;
        fresh.autosave();
        let untitled = Desktop::recovery_path(None);
        assert!(untitled.exists());
        let _ = std::fs::remove_file(untitled);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
