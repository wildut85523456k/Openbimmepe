//! The platform file dialogs, behind one boundary.
//!
//! The app asks for a file through [`Ask::ask_save`] and [`Ask::ask_open`] instead of calling `rfd` directly.
//! In the program they show the native dialog. In a test there is no native dialog (it would block the test
//! run): the test queues the answer the user would give with [`answer`] before clicking the control that opens
//! the dialog, and an unqueued question is answered with Cancel. This is the only place where a window test
//! substitutes for the user, and the substitution is the chosen path, nothing else (P0 office acceptance).
use std::path::PathBuf;

/// A question to the platform: where to save, or which file to open.
pub(crate) trait Ask {
    /// The file the user names to save to, or `None` when they cancel.
    fn ask_save(self) -> Option<PathBuf>;
    /// The file the user picks to open, or `None` when they cancel.
    fn ask_open(self) -> Option<PathBuf>;
}

#[cfg(not(test))]
impl Ask for rfd::FileDialog {
    fn ask_save(self) -> Option<PathBuf> { self.save_file() }
    fn ask_open(self) -> Option<PathBuf> { self.pick_file() }
}

#[cfg(test)]
impl Ask for rfd::FileDialog {
    fn ask_save(self) -> Option<PathBuf> { next_answer() }
    fn ask_open(self) -> Option<PathBuf> { next_answer() }
}

#[cfg(test)]
thread_local! {
    static ANSWERS: std::cell::RefCell<std::collections::VecDeque<Option<PathBuf>>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
    static ASKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Queue the answer to the next file dialog this test thread opens: a path, or `None` for Cancel.
#[cfg(test)]
pub(crate) fn answer(path: Option<PathBuf>) { ANSWERS.with(|a| a.borrow_mut().push_back(path)); }

/// How many file dialogs this test thread has opened.
#[cfg(test)]
pub(crate) fn asked() -> usize { ASKED.with(|n| n.get()) }

#[cfg(test)]
fn next_answer() -> Option<PathBuf> {
    ASKED.with(|n| n.set(n.get() + 1));
    ANSWERS.with(|a| a.borrow_mut().pop_front()).flatten()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_queued_answer_is_given_once_and_an_unqueued_question_is_cancelled() {
        use super::Ask;
        let before = super::asked();
        super::answer(Some("a.obim".into()));
        assert_eq!(rfd::FileDialog::new().ask_save(), Some("a.obim".into()));
        assert_eq!(rfd::FileDialog::new().ask_open(), None, "nothing queued: Cancel");
        assert_eq!(super::asked(), before + 2);
    }
}
