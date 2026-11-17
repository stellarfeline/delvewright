//! **A check only reads** (ADR-0040): the bench a build's checks run on, and
//! the types that keep what a check yields away from what emission writes.
//!
//! A build does three kinds of work. A *derivation* computes a fact the bytes
//! are written from. *Emission* writes the player tree from the campaign and
//! the derivations. A *check* judges a declared intent against both, and
//! yields a refusal, advisories and files of the validation tree. Nothing a
//! check yields is read by a derivation or by emission, and the types here
//! are what holds that:
//!
//! - [`Checks::run`] takes an `Fn` closure — one that cannot assign to
//!   anything it captured — and returns pass or refusal, nothing else.
//! - [`Sink`] is where a check writes. It hands the player tree out read-only
//!   and the validation tree to checks alone; emission is handed the player
//!   tree ([`Checks::player`]), a map no validation file is ever in.
//! - [`Held`] is what one check keeps for a later one. It opens only through
//!   a [`Sink`], and a [`Sink`] exists only inside a running check.
//! - [`Derived`] is a derivation's result: the value emission writes from, and
//!   what the derivation could not derive, which only a check can take out.
//!
//! A look build ([`Build::Look`]) calls no closure: [`Checks::run`] returns
//! pass without running it and [`Held`] stays empty, so the build is parsing,
//! the derivations and emission.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use serde_json::Value;

use crate::compiler::emit::{BuildFailure, BuildOutput};
use crate::compiler::failure::Failure;

/// Which build this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Build {
    /// Every check runs; the tree is the one a delve ships from.
    Proven,
    /// No check runs: the player tree alone, marked `look_only` in its
    /// manifest, for a person to walk through (ADR-0040 §3).
    Look,
}

impl Build {
    /// Whether this build runs its checks.
    pub fn checks(self) -> bool {
        self == Build::Proven
    }
}

/// Whether `path` is a file of the player tree: the datapack, the server's
/// files, the creator overlay, the resource pack and its note. Everything
/// else a build writes is the validation tree's.
pub fn is_player_path(path: &str) -> bool {
    path.starts_with("datapack/")
        || path.starts_with("server/")
        || path.starts_with("creator-datapack/")
        || path == "resourcepack.zip"
        || path == "SKINS.md"
}

/// Where a check writes, and what it may read of the build so far.
pub struct Sink {
    player: BuildOutput,
    validation: BuildOutput,
    warnings: Vec<delvewright_dsl::Diagnostic>,
    late: Vec<delvewright_dsl::Diagnostic>,
}

impl Sink {
    /// Write one validation file as canonical JSON.
    pub fn put_json(&mut self, path: &str, value: &Value) {
        let mut bytes = serde_json::to_vec_pretty(value).expect("json serializes");
        bytes.push(b'\n');
        self.insert(path.to_string(), bytes);
    }

    /// Write one validation file.
    ///
    /// # Panics
    /// When `path` is a player path: a check writes no byte a player is served.
    pub fn insert(&mut self, path: String, bytes: Vec<u8>) {
        assert!(
            !is_player_path(&path),
            "a check wrote the player file `{path}` (ADR-0040 §1)"
        );
        self.validation.insert(path, bytes);
    }

    /// The validation tree so far, for a check that writes several files or
    /// reads one it wrote.
    pub fn validation(&mut self) -> &mut BuildOutput {
        &mut self.validation
    }

    /// The player tree so far.
    pub fn player(&self) -> &BuildOutput {
        &self.player
    }

    /// Raise an advisory.
    pub fn warn(&mut self, d: delvewright_dsl::Diagnostic) {
        self.warnings.push(d);
    }

    /// Raise advisories.
    pub fn warn_all(&mut self, ds: impl IntoIterator<Item = delvewright_dsl::Diagnostic>) {
        self.warnings.extend(ds);
    }

    /// Raise advisories that are reported after every other one.
    pub fn warn_late(&mut self, ds: impl IntoIterator<Item = delvewright_dsl::Diagnostic>) {
        self.late.extend(ds);
    }

    /// What an earlier check kept.
    ///
    /// # Panics
    /// When `held` is empty, which a running check never meets: the check
    /// that fills it ran before this one or refused the build.
    pub fn open<'a, T>(&self, held: &'a Held<T>) -> &'a T {
        held.0
            .as_ref()
            .expect("a check opened what an earlier check of the same build did not keep")
    }

    /// The sink's parts, borrowed at once: a check written against a tree and
    /// a list of advisories names them as it always did.
    pub fn parts(&mut self) -> Parts<'_> {
        Parts {
            player: &self.player,
            out: &mut self.validation,
            warnings: &mut self.warnings,
            late: &mut self.late,
        }
    }

    /// Run `pass` over the player and validation trees as one map — the form
    /// the passes over the finished tree read — and split them again.
    ///
    /// # Panics
    /// When `pass` added, removed or changed a player file.
    pub fn whole<R>(&mut self, pass: impl FnOnce(&mut BuildOutput) -> R) -> R {
        self.whole_restating(|_| false, pass)
    }

    /// [`Sink::whole`], with the player files `restated` names left to the
    /// pass to rewrite. One caller: the passes over the finished tree, for the
    /// delve's `load`, whose quota line is stated again once the PackTest pack
    /// exists (`chain::declare`; ADR-0040, *not separated*).
    ///
    /// # Panics
    /// When `pass` added or removed a player file, or changed one `restated`
    /// does not name.
    pub fn whole_restating<R>(
        &mut self,
        restated: impl Fn(&str) -> bool,
        pass: impl FnOnce(&mut BuildOutput) -> R,
    ) -> R {
        let before = digest(&self.player, &restated);
        let mut tree = std::mem::take(&mut self.player);
        tree.append(&mut self.validation);
        let r = pass(&mut tree);
        let (player, validation): (BuildOutput, BuildOutput) =
            tree.into_iter().partition(|(k, _)| is_player_path(k));
        self.player = player;
        self.validation = validation;
        assert!(
            digest(&self.player, &restated) == before,
            "a pass over the finished tree changed a player file (ADR-0040 §1)"
        );
        r
    }
}

/// A [`Sink`]'s parts ([`Sink::parts`]).
pub struct Parts<'a> {
    /// The player tree so far, to read.
    pub player: &'a BuildOutput,
    /// The validation tree, to write.
    pub out: &'a mut BuildOutput,
    /// The advisories raised so far, to add to.
    pub warnings: &'a mut Vec<delvewright_dsl::Diagnostic>,
    /// The advisories reported after every other one.
    pub late: &'a mut Vec<delvewright_dsl::Diagnostic>,
}

/// One number for a tree's paths and the bytes of every file `restated` does
/// not name, to compare a tree with itself across a pass. Never written
/// anywhere: it is not an identity of the build.
fn digest(tree: &BuildOutput, restated: &impl Fn(&str) -> bool) -> (usize, u64) {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (k, v) in tree {
        k.hash(&mut h);
        if !restated(k) {
            v.hash(&mut h);
        }
    }
    (tree.len(), h.finish())
}

/// What one check keeps for a later check of the same build. Empty in a look
/// build, where no check runs to fill or to open it.
pub struct Held<T>(Option<T>);

impl<T> Held<T> {
    /// Seal the judgement half of a function that derives and judges in one
    /// pass: what it found wrong, its binding, its ledger. The caller takes
    /// the derived half for emission and seals the rest here, in the same
    /// statement that calls the function; from then on only a check opens it.
    pub fn sealed(judged: T) -> Self {
        Held(Some(judged))
    }
}

/// A derivation's result (ADR-0040 §1): the value emission writes from, and
/// what the derivation could not derive.
///
/// `value` is whole wherever `short` is empty. Where it is not, `value` holds
/// what could be derived — fewer seats than a wave has mobs, no route for a
/// walk — and emission writes that. The shortfalls are refusals in waiting:
/// only a check can take them out ([`Derived::short`] asks for a [`Sink`]).
pub struct Derived<T> {
    /// What emission writes from.
    pub value: T,
    short: Vec<Failure>,
}

impl<T> Derived<T> {
    /// A derivation that derived everything.
    pub fn whole(value: T) -> Self {
        Derived {
            value,
            short: Vec::new(),
        }
    }

    /// A derivation and what it could not derive, in the order it met them.
    pub fn with_short(value: T, short: Vec<Failure>) -> Self {
        Derived { value, short }
    }

    /// A derivation that derived all of its value or none of it: `fallback`
    /// is what emission writes from when it derived none.
    pub fn or(derived: Result<T, Failure>, fallback: impl FnOnce() -> T) -> Self {
        match derived {
            Ok(value) => Derived::whole(value),
            Err(f) => Derived::with_short(fallback(), vec![f]),
        }
    }

    /// Whether everything was derived.
    pub fn is_whole(&self) -> bool {
        self.short.is_empty()
    }

    /// What could not be derived, for the check that refuses it.
    pub fn short(&self, _sink: &Sink) -> &[Failure] {
        &self.short
    }

    /// The first thing that could not be derived, as the refusal a check
    /// raises; pass when everything was.
    pub fn refuse_short(&self, sink: &Sink) -> Result<(), BuildFailure> {
        match self.short(sink).first() {
            Some(f) => Err(BuildFailure::Diagnostic {
                code: f.code,
                message: f.message.clone(),
            }),
            None => Ok(()),
        }
    }

    /// How many things could not be derived — a count, for the line a look
    /// build prints about itself.
    pub fn short_count(&self) -> usize {
        self.short.len()
    }
}

/// The bench a build's checks run on.
pub struct Checks {
    build: Build,
    sink: Sink,
    ran: usize,
    skipped: usize,
}

impl Checks {
    /// A bench for one build.
    pub fn new(build: Build) -> Self {
        Checks {
            build,
            sink: Sink {
                player: BTreeMap::new(),
                validation: BTreeMap::new(),
                warnings: Vec::new(),
                late: Vec::new(),
            },
            ran: 0,
            skipped: 0,
        }
    }

    /// Which build this bench serves.
    pub fn build(&self) -> Build {
        self.build
    }

    /// The player tree, for emission to write.
    pub fn player(&mut self) -> &mut BuildOutput {
        &mut self.sink.player
    }

    /// Run one check. In a look build it is not called.
    pub fn run(
        &mut self,
        check: impl Fn(&mut Sink) -> Result<(), BuildFailure>,
    ) -> Result<(), BuildFailure> {
        if !self.build.checks() {
            self.skipped += 1;
            return Ok(());
        }
        self.ran += 1;
        check(&mut self.sink)
    }

    /// Run one check that keeps something for a later one.
    pub fn hold<T>(
        &mut self,
        check: impl Fn(&mut Sink) -> Result<T, BuildFailure>,
    ) -> Result<Held<T>, BuildFailure> {
        if !self.build.checks() {
            self.skipped += 1;
            return Ok(Held(None));
        }
        self.ran += 1;
        check(&mut self.sink).map(|t| Held(Some(t)))
    }

    /// How many checks ran, and how many a look build passed over.
    pub fn counts(&self) -> (usize, usize) {
        (self.ran, self.skipped)
    }

    /// Close the bench: the whole output tree — the player tree, and beside it
    /// every validation file the checks wrote — and the advisories, in the
    /// order they were raised, the late ones last.
    pub fn close(self) -> (BuildOutput, Vec<delvewright_dsl::Diagnostic>) {
        let Sink {
            mut player,
            mut validation,
            mut warnings,
            late,
        } = self.sink;
        assert!(
            player.keys().all(|k| is_player_path(k)),
            "emission wrote a file outside the player tree: {:?} (ADR-0040 §1)",
            player.keys().find(|k| !is_player_path(k))
        );
        player.append(&mut validation);
        warnings.extend(late);
        (player, warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal() -> BuildFailure {
        BuildFailure::Diagnostic {
            code: crate::compiler::plan::DW_BUILD,
            message: "refused".to_string(),
        }
    }

    #[test]
    fn a_look_build_calls_no_check_and_keeps_nothing() {
        let mut bench = Checks::new(Build::Look);
        let called = std::cell::Cell::new(0);
        bench
            .run(|_| {
                called.set(called.get() + 1);
                Err(refusal())
            })
            .expect("a look build runs no check, so nothing refuses");
        let held = bench
            .hold(|_| {
                called.set(called.get() + 1);
                Ok(7)
            })
            .expect("nor one that keeps something");
        assert_eq!(called.get(), 0);
        assert!(held.0.is_none());
        assert_eq!(bench.counts(), (0, 2));
        let (tree, warnings) = bench.close();
        assert!(tree.is_empty() && warnings.is_empty());
    }

    #[test]
    fn a_proven_build_runs_each_check_and_hands_on_what_one_keeps() {
        let mut bench = Checks::new(Build::Proven);
        bench
            .player()
            .insert("datapack/pack.mcmeta".to_string(), b"{}".to_vec());
        let held = bench
            .hold(|sink| {
                sink.put_json("validation/a.json", &serde_json::json!({"n": 1}));
                Ok(41)
            })
            .unwrap();
        bench
            .run(|sink| {
                let n = *sink.open(&held) + 1;
                sink.put_json("validation/b.json", &serde_json::json!({ "n": n }));
                assert!(sink.player().contains_key("datapack/pack.mcmeta"));
                Ok(())
            })
            .unwrap();
        assert!(bench.run(|_| Err(refusal())).is_err());
        assert_eq!(bench.counts(), (3, 0));
        let (tree, _) = bench.close();
        assert_eq!(
            tree.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "datapack/pack.mcmeta",
                "validation/a.json",
                "validation/b.json"
            ]
        );
    }

    #[test]
    #[should_panic(expected = "a check wrote the player file")]
    fn a_check_that_writes_a_player_file_is_stopped() {
        let mut bench = Checks::new(Build::Proven);
        let _ = bench.run(|sink| {
            sink.insert(
                "datapack/data/n/function/f.mcfunction".to_string(),
                Vec::new(),
            );
            Ok(())
        });
    }

    #[test]
    #[should_panic(expected = "changed a player file")]
    fn a_pass_over_the_finished_tree_that_changes_a_player_file_is_stopped() {
        let mut bench = Checks::new(Build::Proven);
        bench.player().insert(
            "datapack/data/n/function/f.mcfunction".to_string(),
            b"say a\n".to_vec(),
        );
        let _ = bench.run(|sink| {
            sink.whole(|tree| {
                tree.insert(
                    "datapack/data/n/function/f.mcfunction".to_string(),
                    b"say b\n".to_vec(),
                );
            });
            Ok(())
        });
    }

    #[test]
    fn a_pass_may_restate_the_one_player_file_it_is_handed_and_no_other() {
        let load = "datapack/data/n/function/load.mcfunction";
        let other = "datapack/data/n/function/f.mcfunction";
        let bench = || {
            let mut bench = Checks::new(Build::Proven);
            bench.player().insert(load.to_string(), b"say a\n".to_vec());
            bench
                .player()
                .insert(other.to_string(), b"say a\n".to_vec());
            bench
        };
        let mut restating = bench();
        restating
            .run(|sink| {
                sink.whole_restating(
                    |p| p == load,
                    |tree| tree.insert(load.to_string(), b"say b\n".to_vec()),
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(restating.close().0[load], b"say b\n");
        let mut straying = bench();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = straying.run(|sink| {
                sink.whole_restating(
                    |p| p == load,
                    |tree| tree.insert(other.to_string(), b"say b\n".to_vec()),
                );
                Ok(())
            });
        }));
        assert!(
            caught.is_err(),
            "a file the pass was not handed changed, and nothing stopped it"
        );
    }

    #[test]
    fn a_pass_over_the_finished_tree_reads_both_and_writes_the_validation_tree() {
        let mut bench = Checks::new(Build::Proven);
        bench.player().insert(
            "datapack/data/n/function/f.mcfunction".to_string(),
            b"say a\n".to_vec(),
        );
        bench
            .run(|sink| {
                sink.insert(
                    "packtest-datapack/data/n/test/t.mcfunction".to_string(),
                    Vec::new(),
                );
                let seen = sink.whole(|tree| {
                    tree.insert("validation/seen.json".to_string(), b"2".to_vec());
                    tree.len()
                });
                assert_eq!(seen, 3);
                Ok(())
            })
            .unwrap();
        let (tree, _) = bench.close();
        assert_eq!(tree.len(), 3);
    }

    #[test]
    fn a_shortfall_is_refused_by_a_check_and_by_nothing_in_a_look() {
        let short = Derived::with_short(
            vec![1, 2],
            vec![Failure {
                code: crate::compiler::plan::DW_BUILD,
                message: "one seat short".to_string(),
            }],
        );
        let mut proven = Checks::new(Build::Proven);
        assert!(proven.run(|sink| short.refuse_short(sink)).is_err());
        let mut look = Checks::new(Build::Look);
        assert!(look.run(|sink| short.refuse_short(sink)).is_ok());
        assert_eq!(short.value, [1, 2]);
        assert_eq!(short.short_count(), 1);
    }
}
