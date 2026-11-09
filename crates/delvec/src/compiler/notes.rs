//! **A build's binding notes have one outlet.**
//!
//! Every proof the build runs states what it bound — its counts, zeroes
//! included — as one line on stderr (`CLAUDE.md`, vacuity). Those lines are the
//! whole's census: `delvec build` prints them as they are computed. A caller
//! that runs the build for a narrower question — `detail --check`, whose author
//! is asking about one place — may [`hold`] them instead, and gets every line
//! back, in the order it was said, to write where its reader can find it.
//!
//! A refusal is never a note: it is printed where it is raised, held or not.
use std::sync::Mutex;
use std::sync::OnceLock;

/// The lines said while a hold is open; `None` when nothing holds them.
static HELD: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// **Phase timing** (`DELVEC_PHASE_TIMING`): when the variable is set, every
/// note printed on stderr is prefixed with the wall seconds since the process
/// started, the process's CPU seconds so far and its thread count (the last
/// two read from `/proc/self/stat`, so on Linux only), and [`mark`] prints a
/// line of its own at a phase boundary that owns no note. It is the log's
/// alone: a held note is kept without its stamp and a mark is never held, so
/// what a hold hands its caller, and every byte of a build's output, are the
/// same with the variable set or unset. Unset, stderr is unchanged too.
static START: OnceLock<std::time::Instant> = OnceLock::new();

/// Whether phase timing is on (`DELVEC_PHASE_TIMING` set).
pub fn timing() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("DELVEC_PHASE_TIMING").is_some())
}

/// Start the wall clock; called once at the top of `main` so validation and
/// analysis are on it. A later first stamp starts it itself.
pub fn start_clock() {
    START.get_or_init(std::time::Instant::now);
}

/// The process's CPU seconds (user + system) and thread count, read from
/// `/proc/self/stat`; `None` where there is no such file.
fn cpu_and_threads() -> Option<(f64, u64)> {
    let s = std::fs::read_to_string("/proc/self/stat").ok()?;
    // Fields after the parenthesised command name: utime is field 14, stime
    // 15, num_threads 20 (1-based over the whole line).
    let rest = &s[s.rfind(')')? + 2..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    let utime: f64 = f.get(11)?.parse().ok()?;
    let stime: f64 = f.get(12)?.parse().ok()?;
    let threads: u64 = f.get(17)?.parse().ok()?;
    Some(((utime + stime) / 100.0, threads))
}

/// The timing prefix: `[t=+W s cpu=C s thr=N] `.
fn stamp() -> String {
    let wall = START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64();
    match cpu_and_threads() {
        Some((cpu, thr)) => format!("[t=+{wall:.1}s cpu={cpu:.1}s thr={thr}] "),
        None => format!("[t=+{wall:.1}s] "),
    }
}

/// Print a phase mark on stderr when phase timing is on; nothing otherwise.
/// A mark is never held.
pub fn mark(what: &str) {
    if timing() {
        eprintln!("{}phase: {what}", stamp());
    }
}

/// Say one binding note: printed on stderr (stamped when phase timing is
/// on), or kept, unstamped, by the open [`hold`].
pub fn say(line: String) {
    let mut held = HELD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match held.as_mut() {
        Some(lines) => lines.push(line),
        None if timing() => eprintln!("{}{line}", stamp()),
        None => eprintln!("{line}"),
    }
}

/// Hold every note said until [`Hold::take`] (or the hold's drop): the notes
/// are kept, in the order they were said, and not printed.
#[must_use = "a hold lets go of its notes when dropped"]
pub fn hold() -> Hold {
    let mut held = HELD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(held.is_none(), "one hold of the build's notes at a time");
    *held = Some(Vec::new());
    Hold { open: true }
}

/// An open hold of the build's notes.
pub struct Hold {
    open: bool,
}

impl Hold {
    /// Close the hold: every note said since it opened, in order.
    pub fn take(mut self) -> Vec<String> {
        self.open = false;
        HELD.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .unwrap_or_default()
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        if self.open {
            HELD.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
        }
    }
}

/// Say one binding note, formatted as `eprintln!` formats ([`say`]).
#[macro_export]
macro_rules! note {
    ($($arg:tt)*) => {
        $crate::compiler::notes::say(format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_keeps_every_note_in_order_and_lets_go_when_taken() {
        // Other tests' builds may say notes while this one holds: read only
        // this test's own.
        let mine = |lines: Vec<String>| -> Vec<String> {
            lines
                .into_iter()
                .filter(|l| l.starts_with("notes-test "))
                .collect()
        };
        let h = hold();
        say("notes-test first".to_string());
        crate::note!("notes-test second {}", 2);
        assert_eq!(
            mine(h.take()),
            vec![
                "notes-test first".to_string(),
                "notes-test second 2".to_string()
            ]
        );
        // Closed: a note said now is printed, and the next hold starts empty.
        say("notes-test printed".to_string());
        let h = hold();
        assert!(mine(h.take()).is_empty());
    }
}
