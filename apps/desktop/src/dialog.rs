//! The platform file dialogs, behind one boundary.
//!
//! The app asks for a file with a [`Question`]: its title, its file types and the name it suggests. In the program
//! the question is shown as the native dialog (`rfd`). In a test there is no native dialog (it would block the run):
//! every question is recorded, so a test checks what the user would be asked, and the test queues the answer the
//! user would give with [`answer`]; an unqueued question is answered with Cancel. The native dialog itself is
//! reviewed in a real window (P0 gate 4), not here.
use std::path::{Path, PathBuf};

/// What the app asks the platform: where to save, or which file to open.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Question {
    pub title: String,
    /// (name, extensions) of each file type offered
    pub filters: Vec<(String, Vec<String>)>,
    /// the name offered for a new file
    pub file_name: Option<String>,
    pub directory: Option<PathBuf>,
    /// true for Save, false for Open (set when it is asked)
    pub save: bool,
}

impl Question {
    pub(crate) fn new() -> Self { Self::default() }
    pub(crate) fn set_title(mut self, title: impl Into<String>) -> Self { self.title = title.into(); self }
    pub(crate) fn add_filter(mut self, name: &str, extensions: &[&str]) -> Self {
        self.filters.push((name.to_string(), extensions.iter().map(|e| e.to_string()).collect()));
        self
    }
    pub(crate) fn set_file_name(mut self, name: impl Into<String>) -> Self { self.file_name = Some(name.into()); self }
    pub(crate) fn set_directory(mut self, dir: impl AsRef<Path>) -> Self { self.directory = Some(dir.as_ref().to_path_buf()); self }

    /// The file the user names to save to, or `None` when they cancel.
    pub(crate) fn ask_save(mut self) -> Option<PathBuf> { self.save = true; ask(self) }
    /// The file the user picks to open, or `None` when they cancel.
    pub(crate) fn ask_open(mut self) -> Option<PathBuf> { self.save = false; ask(self) }
}

#[cfg(not(test))]
fn ask(q: Question) -> Option<PathBuf> {
    let mut fd = rfd::FileDialog::new().set_title(&q.title);
    for (name, ext) in &q.filters {
        let ext: Vec<&str> = ext.iter().map(String::as_str).collect();
        fd = fd.add_filter(name, &ext);
    }
    if let Some(n) = &q.file_name { fd = fd.set_file_name(n); }
    if let Some(d) = &q.directory { fd = fd.set_directory(d); }
    if q.save { fd.save_file() } else { fd.pick_file() }
}

#[cfg(test)]
fn ask(q: Question) -> Option<PathBuf> {
    ASKED.with(|a| a.borrow_mut().push(q));
    ANSWERS.with(|a| a.borrow_mut().pop_front()).flatten()
}

#[cfg(test)]
thread_local! {
    static ANSWERS: std::cell::RefCell<std::collections::VecDeque<Option<PathBuf>>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
    static ASKED: std::cell::RefCell<Vec<Question>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Queue the answer to the next file dialog this test thread opens: a path, or `None` for Cancel.
#[cfg(test)]
pub(crate) fn answer(path: Option<PathBuf>) { ANSWERS.with(|a| a.borrow_mut().push_back(path)); }

/// How many file dialogs this test thread has opened.
#[cfg(test)]
pub(crate) fn asked() -> usize { ASKED.with(|a| a.borrow().len()) }

/// The last question this test thread was asked.
#[cfg(test)]
pub(crate) fn last_question() -> Option<Question> { ASKED.with(|a| a.borrow().last().cloned()) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_queued_answer_is_given_once_an_unqueued_question_is_cancelled_and_every_question_is_kept() {
        let before = asked();
        answer(Some("a.obim".into()));
        assert_eq!(Question::new().set_title("Save model").add_filter("OpenBIM model", &["obim"]).set_file_name("model.obim").ask_save(), Some("a.obim".into()));
        let q = last_question().unwrap();
        assert_eq!((q.title.as_str(), q.filters.clone(), q.file_name.as_deref(), q.save), ("Save model", vec![("OpenBIM model".to_string(), vec!["obim".to_string()])], Some("model.obim"), true));
        assert_eq!(Question::new().ask_open(), None, "nothing queued: Cancel");
        assert_eq!(asked(), before + 2);
    }
}
