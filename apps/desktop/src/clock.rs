//! The app's clock. The program reads the system's; a test can move its own clock on, so the timers the window runs
//! (the autosave) are driven through the window as they are in the program, without waiting for real time.
use std::time::Instant;

#[cfg(not(test))]
pub(crate) fn now() -> Instant { Instant::now() }

#[cfg(test)]
thread_local! {
    static AHEAD: std::cell::Cell<std::time::Duration> = const { std::cell::Cell::new(std::time::Duration::ZERO) };
}

/// The system's time, plus what this test thread has moved its clock on by.
#[cfg(test)]
pub(crate) fn now() -> Instant { Instant::now() + AHEAD.with(|a| a.get()) }

/// Moves this test thread's clock on by `by`.
#[cfg(test)]
pub(crate) fn advance(by: std::time::Duration) { AHEAD.with(|a| a.set(a.get() + by)); }
