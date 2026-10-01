//! Interned block states.
//!
//! The assembled world names a block in every placed cell — millions of cells
//! on a large campaign — but only a few hundred distinct block states. A
//! [`BlockState`] is a reference to the one interned copy of its text, so a
//! cell costs a pointer instead of an owned `String`.
//!
//! It reads as the text it names: equality, ordering, hashing, `Display` and
//! `Debug` are the text's own, so replacing a `String` with a `BlockState`
//! changes no comparison, no sort and no message.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};

/// One block state, e.g. `minecraft:oak_stairs[facing=east,half=top]`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockState(&'static str);

/// A cell→block map whose values are interned.
pub type BlockMap = BTreeMap<[i32; 3], BlockState>;

impl BlockState {
    /// The interned state for `text`, interning it on first use.
    ///
    /// Every distinct text is stored once for the life of the process; the
    /// table only grows, by one entry per distinct block state ever named.
    pub fn new(text: &str) -> BlockState {
        static TABLE: OnceLock<Mutex<BTreeSet<&'static str>>> = OnceLock::new();
        let mut table = TABLE
            .get_or_init(|| Mutex::new(BTreeSet::new()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(s) = table.get(text) {
            return BlockState(s);
        }
        let s: &'static str = Box::leak(text.to_owned().into_boxed_str());
        table.insert(s);
        BlockState(s)
    }

    /// The text this state names.
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::ops::Deref for BlockState {
    type Target = str;
    fn deref(&self) -> &str {
        self.0
    }
}

impl AsRef<str> for BlockState {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl std::borrow::Borrow<str> for BlockState {
    fn borrow(&self) -> &str {
        self.0
    }
}

impl std::fmt::Display for BlockState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.0, f)
    }
}

impl std::fmt::Debug for BlockState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.0, f)
    }
}

impl From<&str> for BlockState {
    fn from(text: &str) -> Self {
        BlockState::new(text)
    }
}

impl From<String> for BlockState {
    fn from(text: String) -> Self {
        BlockState::new(&text)
    }
}

impl From<&String> for BlockState {
    fn from(text: &String) -> Self {
        BlockState::new(text)
    }
}

impl From<BlockState> for String {
    fn from(state: BlockState) -> Self {
        state.0.to_string()
    }
}

impl PartialEq<str> for BlockState {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for BlockState {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for BlockState {
    fn eq(&self, other: &String) -> bool {
        self.0 == other.as_str()
    }
}

/// `blocks` with every value interned — the entry point for a map built from
/// owned strings (a test fixture, a synthetic world).
pub fn interned(blocks: BTreeMap<[i32; 3], String>) -> BlockMap {
    let mut seen: BTreeMap<String, BlockState> = BTreeMap::new();
    blocks
        .into_iter()
        .map(|(c, name)| {
            let state = match seen.get(&name) {
                Some(s) => *s,
                None => {
                    let s = BlockState::new(&name);
                    seen.insert(name, s);
                    s
                }
            };
            (c, state)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A state reads as its text: one copy per text, and equality, order and
    /// both formats are the text's.
    #[test]
    fn a_state_reads_as_its_text() {
        let a = BlockState::new("minecraft:stone");
        let b = BlockState::from("minecraft:stone".to_string());
        let c = BlockState::new("minecraft:andesite");
        assert!(std::ptr::eq(a.as_str(), b.as_str()));
        assert_eq!(a, b);
        assert_eq!(a, "minecraft:stone");
        assert!(c < a);
        assert_eq!(
            format!("{a} {a:?}"),
            format!("{} {:?}", "minecraft:stone", "minecraft:stone")
        );
        let mut names = vec![a, c, b];
        names.sort();
        assert_eq!(names, vec![c, a, a]);
    }
}
