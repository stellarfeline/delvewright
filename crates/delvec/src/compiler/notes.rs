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

/// The lines said while a hold is open; `None` when nothing holds them.
static HELD: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Say one binding note: printed on stderr, or kept by the open [`hold`].
pub fn say(line: String) {
    let mut held = HELD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match held.as_mut() {
        Some(lines) => lines.push(line),
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
