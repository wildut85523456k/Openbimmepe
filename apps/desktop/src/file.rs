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
    let mut doc = Document::new();
    doc.transact(0, &native::restore_commands(elements, site)).map_err(|e| format!("the file does not make a model: {e:?}"))?;
    doc.clear_undo_history();
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
}

impl Desktop {
    pub(crate) fn is_dirty(&self) -> bool { self.document.revision() != self.file.saved_revision }

    /// Writes the model to its file, asking for a name when it has none (or when `ask` is set).
    pub(crate) fn save_file(&mut self, ask: bool) {
        let path = match (&self.file.path, ask) {
            (Some(p), false) => p.clone(),
            _ => {
                let mut fd = rfd::FileDialog::new().set_title("Save model").add_filter("OpenBIM model", &["obim"]);
                if let Some(name) = self.file.path.as_deref().and_then(|p| std::path::Path::new(p).file_name()).and_then(|n| n.to_str()) { fd = fd.set_file_name(name); } else { fd = fd.set_file_name("model.obim"); }
                let Some(p) = fd.save_file() else { return };
                let mut p = p.display().to_string();
                if !p.to_lowercase().ends_with(".obim") { p.push_str(".obim"); }
                p
            }
        };
        self.write_model(&path);
    }

    pub(crate) fn write_model(&mut self, path: &str) {
        let tmp = format!("{path}.saving");
        let result = std::fs::write(&tmp, file_text(&self.document)).and_then(|()| std::fs::rename(&tmp, path));
        self.message = match result {
            Ok(()) => { self.file.path = Some(path.into()); self.file.saved_revision = self.document.revision(); format!("Saved {path}") }
            Err(e) => { let _ = std::fs::remove_file(&tmp); format!("Could not save {path}: {e}") }
        };
    }

    /// Asks which file to open, then opens it (after the question about unsaved changes, if there are any).
    pub(crate) fn open_file(&mut self) {
        let Some(p) = rfd::FileDialog::new().set_title("Open model").add_filter("OpenBIM model", &["obim"]).pick_file() else { return };
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
                self.selected = None;
                self.sync_duct_controls();
                self.message = format!("Opened {path} ({} elements)", self.document.elements().count());
            }
            Err(e) => self.message = e,
        }
    }

    /// An empty model (after the question about unsaved changes, if there are any).
    pub(crate) fn new_model(&mut self) {
        if self.is_dirty() { self.file.confirm_new = true; return; }
        self.reset_model();
    }

    fn reset_model(&mut self) {
        self.document = Document::new();
        self.file = FileState::default();
        self.selected = None;
        self.message = "New empty model".into();
    }

    /// The question asked when a file or a new model would replace unsaved changes.
    pub(crate) fn file_window(&mut self, ctx: &egui::Context) {
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
        assert_eq!(app.document.elements().count(), 0, "{}", app.message);
        assert!(app.file.path.is_none());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
