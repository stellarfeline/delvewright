//! `DW0879` — a numeric gate judged against the writes the path performs before
//! it (`compiler::statepath` over `compiler::flow`'s replay).
//!
//! The fixture `state-path-order` is a reduction of the shape the engine's own
//! gallery shipped: a datum a first beat produces, a `clear-state` that returns
//! it to its `initial`, and a later objective gated on it being at least one. As
//! committed the clear stands **after** the reading beat and the campaign is
//! finishable; every red below moves the clear in front of the gate, which is
//! exactly the arrangement the gallery had.
//!
//! Both directions are pinned, and so are the two things the rule deliberately
//! does NOT claim: a datum no ordered walk can date is never refused, and a
//! failure some play order avoids is withheld rather than reported.

mod common;

use std::path::{Path, PathBuf};

use delvewright_compiler::flow::{DW_STATE_GATE_CLEARED, Flow};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::statepath;
use delvewright_dsl::{Campaign, Diagnostic, parse_campaign};

fn fixture_dir() -> PathBuf {
    common::compiler_fixtures_dir().join("state-path-order")
}

fn parse(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).expect("fixture loads");
    parse_campaign(&loaded.raw).expect("fixture parses")
}

/// The fixture's stage-5 document, as a mutable JSON value.
fn quests_json() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(fixture_dir().join("quests.json")).unwrap())
        .unwrap()
}

/// Materialize the fixture with `quests.json` replaced, into a named directory.
fn materialize(name: &str, quests: serde_json::Value) -> PathBuf {
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dst);
    common::materialize_from(
        &fixture_dir(),
        &serde_json::json!({ "documents": { "quests": quests } }),
        &dst,
    );
    dst
}

/// The fixture with `quests.json` replaced, parsed.
fn variant(name: &str, quests: serde_json::Value) -> Campaign {
    parse(&materialize(name, quests))
}

fn codes(d: &[Diagnostic]) -> Vec<&str> {
    d.iter().map(|x| x.code.as_str()).collect()
}

/// The stage-5 quest with this id, mutably.
fn quest<'a>(q: &'a mut serde_json::Value, id: &str) -> &'a mut serde_json::Value {
    q["content"]["quests"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|x| x["id"] == id)
        .expect("quest is in the fixture")
}

/// The fixture's own `clear-state` effect, lifted out of `obj/exit`'s completion
/// bundle. Every red below is *the same effect in a different place*, which is
/// what makes the pair a red→green rather than two campaigns.
fn take_the_clear(q: &mut serde_json::Value) -> serde_json::Value {
    let leave = quest(q, "quest/leave");
    let bundle = leave["on_objective_complete"]["obj/exit"]
        .as_array_mut()
        .unwrap();
    let n = bundle.len();
    let taken = bundle.remove(0);
    assert_eq!(n, 1, "the fixture's `obj/exit` bundle is the clear alone");
    assert_eq!(taken["type"], "clear-state", "{taken}");
    leave["on_objective_complete"]
        .as_object_mut()
        .unwrap()
        .remove("obj/exit");
    taken
}

// ---------------------------------------------------------------------------
// green: the fixture as committed
// ---------------------------------------------------------------------------

/// As committed the clear stands after the beat that reads the datum, so the
/// gate holds where it is read and the walk finds nothing. The binding is stated
/// with it: a green that examined nothing would be the same silence.
#[test]
fn the_fixture_as_committed_is_finishable() {
    let c = parse(&fixture_dir());
    let (d, b) = statepath::check(&c);
    assert!(d.is_empty(), "{:?}", codes(&d));
    assert_eq!(b.walk.data, 1, "one declared datum");
    assert_eq!(b.walk.undatable, 0, "and an ordered walk can date it");
    assert!(b.walk.gates >= 1, "the gate was read: {b:?}");
    assert!(b.walk.writes >= 2, "the writes were replayed: {b:?}");
    assert_eq!(b.walk.withheld, 0, "{b:?}");
    assert!(b.paths >= 1, "{b:?}");
    assert!(b.not_judged.is_none(), "{b:?}");
}

// ---------------------------------------------------------------------------
// red: the gallery's own former arrangement
// ---------------------------------------------------------------------------

/// **The instance the gallery shipped.** Move the clear onto the beat before the
/// reader and the finale's gate can never open: the value is 0 where the gate
/// wants 1, and the campaign's own `after` chain forces the clear to happen
/// first.
#[test]
fn a_clear_between_a_producer_and_its_reader_is_refused() {
    let mut q = quests_json();
    let clear = take_the_clear(&mut q);
    quest(&mut q, "quest/leave")["on_objective_complete"]["obj/shelve"] =
        serde_json::json!([clear]);
    let c = variant("dw0879-clear-before-the-gate", q);

    let (d, b) = statepath::check(&c);
    assert_eq!(codes(&d), vec!["DW0879"], "{d:#?}");
    assert_eq!(b.walk.withheld, 0, "the order is forced: {b:?}");

    let m = &d[0].message;
    for want in [
        "obj/exit",
        "state/labels-read",
        "at-least 1",
        "holds 0",
        "`obj/shelve`'s completion bundle",
        "clear-state",
    ] {
        assert!(m.contains(want), "message lacks `{want}`:\n{m}");
    }
    assert_eq!(
        d[0].path, "/content/quests/1/objectives/1",
        "the refusal points at the beat, not at the document"
    );
}

/// The same defect written into a quest's `on_complete` rather than an
/// objective's. A quest's completion bundle runs after every objective of that
/// quest and before every beat of every quest it triggers, so it is forced ahead
/// of the gate exactly as an `after` prerequisite is — and the message names it
/// as what it is.
#[test]
fn a_quest_completion_bundle_is_blamed_by_name() {
    let mut q = quests_json();
    let clear = take_the_clear(&mut q);
    quest(&mut q, "quest/ask")["on_complete"]
        .as_array_mut()
        .unwrap()
        .push(clear);
    let c = variant("dw0879-clear-in-on-complete", q);

    let (d, _) = statepath::check(&c);
    assert_eq!(codes(&d), vec!["DW0879"], "{d:#?}");
    assert!(
        d[0].message.contains("`quest/ask`'s `on_complete` bundle"),
        "{}",
        d[0].message
    );
}

/// **The gate is invoked, not merely correct.** A check nothing calls protects
/// nothing, so the refusal is taken from the process every author actually runs:
/// `delvec validate` exits non-zero and prints the code.
#[test]
fn the_refusal_reaches_the_command_line() {
    let mut q = quests_json();
    let clear = take_the_clear(&mut q);
    quest(&mut q, "quest/leave")["on_objective_complete"]["obj/shelve"] =
        serde_json::json!([clear]);
    let dir = materialize("dw0879-cli", q);

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["validate", dir.to_str().unwrap(), "--prefabs"])
        .arg(common::prefabs_dir())
        .output()
        .expect("run delvec");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("DW0879"), "{text}");
    assert!(
        text.contains("state path binding:"),
        "the run states what it walked:\n{text}"
    );
}

// ---------------------------------------------------------------------------
// what the rule deliberately does not claim
// ---------------------------------------------------------------------------

/// **A failure some play order avoids is not a defect.** With `obj/shelve`'s
/// `after` link gone, a player may reach the exit before shelving the ledger, so
/// the clear is no longer forced ahead of the gate. The term is withheld and
/// counted rather than refused — and the count is what says the walk looked.
#[test]
fn an_unforced_order_withholds_the_refusal() {
    let mut q = quests_json();
    let clear = take_the_clear(&mut q);
    {
        let leave = quest(&mut q, "quest/leave");
        leave["on_objective_complete"]["obj/shelve"] = serde_json::json!([clear]);
        let objs = leave["objectives"].as_array_mut().unwrap();
        let exit = objs.iter_mut().find(|o| o["id"] == "obj/exit").unwrap();
        exit["after"] = serde_json::json!([]);
    }
    let c = variant("dw0879-unforced", q);

    let (d, b) = statepath::check(&c);
    assert!(d.is_empty(), "{:?}", codes(&d));
    assert!(b.walk.withheld >= 1, "the term was read and withheld: {b:?}");
}

/// **A datum no ordered walk can date is never refused.** The campaign's
/// `on_death` bundle writes the same datum, and a reaction bundle fires at a
/// moment nothing in the document names — so the value at the gate is not a
/// function of the path, and the walk withholds instead of guessing. The binding
/// says so: one declared datum, one undatable.
#[test]
fn an_undatable_datum_is_never_refused() {
    let mut q = quests_json();
    let clear = take_the_clear(&mut q);
    quest(&mut q, "quest/leave")["on_objective_complete"]["obj/shelve"] =
        serde_json::json!([clear]);
    q["content"]["on_death"] = serde_json::json!([
        {"amount": 1, "state": "state/labels-read", "type": "add-state"}
    ]);
    let c = variant("dw0879-undatable", q);

    let (d, b) = statepath::check(&c);
    assert!(d.is_empty(), "{:?}", codes(&d));
    assert_eq!(b.walk.data, 1, "{b:?}");
    assert_eq!(b.walk.undatable, 1, "{b:?}");
}

/// **A consequence is not named where the cause already is.** A campaign whose
/// exported path is not a playthrough has a fault upstream of this rule; the
/// walk withholds itself and says which code owns the cause, rather than adding
/// a second refusal about the same break.
#[test]
fn a_campaign_with_no_walkable_path_is_not_judged() {
    let mut q = quests_json();
    {
        let leave = quest(&mut q, "quest/leave");
        let objs = leave["objectives"].as_array_mut().unwrap();
        let exit = objs.iter_mut().find(|o| o["id"] == "obj/exit").unwrap();
        exit["requires_flags"] = serde_json::json!(["flag/nobody-sets-this"]);
    }
    let c = variant("dw0879-no-path", q);

    let (d, b) = statepath::check(&c);
    assert!(d.is_empty(), "{:?}", codes(&d));
    assert_eq!(b.paths, 0, "{b:?}");
    let why = b.not_judged.expect("the run says why it walked nothing");
    assert!(why.contains("DW0201") || why.contains("DW0204"), "{why}");
    assert!(b.line().contains("0 path(s) walked"), "{}", b.line());
}

// ---------------------------------------------------------------------------
// the constant, and the binding's own honesty
// ---------------------------------------------------------------------------

/// The code, its tier and its scope, asserted where a reader can find them. It
/// judges what the document SAYS — a comparison and the writes around it — so it
/// binds every version; nothing a pre-0.10 campaign could not have had is
/// required of it, because a campaign with no `state[]` declares no gate this
/// rule can read.
#[test]
fn the_constant_declares_its_tier_and_its_scope() {
    assert_eq!(DW_STATE_GATE_CLEARED, "DW0879");
    assert_eq!(
        DW_STATE_GATE_CLEARED.exit_tier(),
        delvewright_dsl::ExitTier::Analysis
    );
    let c = parse(&common::hello_world_dir());
    let (d, b) = statepath::check(&c);
    assert!(d.is_empty(), "{:?}", codes(&d));
    assert_eq!(
        b.walk.data, 0,
        "a campaign that declares no datum has none to walk"
    );
}

/// **The binding count is computed from the objects, not written beside them.**
/// Perturb the fixture so it reads a second gate term and the count moves by
/// exactly one — a constant would not.
#[test]
fn the_gate_count_moves_with_the_gates() {
    let before = {
        let c = parse(&fixture_dir());
        statepath::check(&c).1.walk.gates
    };
    let mut q = quests_json();
    {
        let leave = quest(&mut q, "quest/leave");
        let objs = leave["objectives"].as_array_mut().unwrap();
        let shelve = objs.iter_mut().find(|o| o["id"] == "obj/shelve").unwrap();
        shelve["requires_state"] = serde_json::json!([
            {"op": "at-least", "state": "state/labels-read", "value": 0}
        ]);
    }
    let c = variant("dw0879-second-gate", q);
    let after = statepath::check(&c).1.walk.gates;
    assert_eq!(
        after,
        before + 2,
        "one term on one objective, read once per path walked (2 paths)"
    );
}

/// The replay the rule rides is the one the rest of the model rides: teaching
/// `fire` about numeric gates must not have moved what the flag half proves. The
/// fixture's own critical path still replays.
#[test]
fn the_replay_is_still_the_same_replay() {
    let c = parse(&fixture_dir());
    let flow = Flow::new(&c);
    let p = flow.playthrough();
    assert!(!p.degenerate);
    assert!(flow.replay(&p).is_ok(), "{:?}", flow.replay(&p).err());
    assert_eq!(
        p.steps.iter().map(|s| s.objective.as_str()).collect::<Vec<_>>(),
        vec!["obj/talk", "obj/shelve", "obj/exit"]
    );
}
