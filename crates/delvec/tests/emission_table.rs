//! `light::emission()` against the pinned game's own answer.
//!
//! The fixture is a MEASUREMENT, not a table somebody typed: every value in it
//! is what `BlockState.getLightEmission()` returns inside the pinned 1.21.11
//! server jar, dumped by `tools/dump-block-light.py` (which refuses any jar
//! whose sha256 is not the `versions.toml` pin). That is the property that makes
//! this a proof rather than coverage — a wrong emission value cannot be made
//! green by editing the expectation without editing a file whose header names
//! the jar it came from, and `tools/dump-block-light.py --check` re-derives it.
//!
//! Three assertions, and the first is the one the gate exists for.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_compiler::light::emission;

/// Blocks whose entry is deliberately the MINIMUM over the states the world can
/// drive them to, rather than the shipped blockstate's own value — so they are
/// the only blocks allowed to measure below the game. Each reason is the one
/// recorded on the entry in `crates/compiler/src/light.rs`.
///
/// This set is the round-trip half of the gate: a future Minecraft version that
/// adds an emitter the table does not know puts that block in here, and
/// `the_only_underestimates_are_the_declared_ones` reds until somebody decides
/// what its value is.
const DELIBERATE_MINIMUM: &[(&str, &str)] = &[
    (
        "redstone_lamp",
        "no onPlace; the first neighbour update unlights a shipped lit=true lamp",
    ),
    (
        "trial_spawner",
        "its block entity owns trial_spawner_state; the minimum over that is 0",
    ),
    (
        "vault",
        "its block entity owns vault_state; every state is 6 or 12, so 6 holds",
    ),
];

/// A row of the fixture: a full blockstate string, the game's light, and how
/// many of the game's blockstates that row stands for.
struct Row {
    state: String,
    game: u8,
    states: usize,
}

/// `minecraft:candle[candles=2,lit=true]` -> `candle`.
fn base_id(name: &str) -> &str {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    &n[..n.find('[').unwrap_or(n.len())]
}

/// Read the fixture, and the state total its own header claims.
fn fixture() -> (Vec<Row>, usize) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../compiler/tests/fixtures/light/emission-1.21.11.tsv"
    );
    let text = std::fs::read_to_string(path).expect("the block-light fixture is missing");
    let mut claimed = 0usize;
    let mut rows = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix('#') {
            // "# 1419 rows collapse the game's 29671 blockstates onto the"
            if let Some(tail) = rest.split_once("blockstates onto the") {
                claimed = tail
                    .0
                    .split_whitespace()
                    .last()
                    .and_then(|n| n.parse().ok())
                    .expect("the fixture header states a blockstate total");
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let mut f = line.split('\t');
        let state = f.next().expect("a row has a blockstate").to_string();
        let game = f
            .next()
            .expect("a row has a light")
            .parse()
            .expect("light is a number");
        let states = f
            .next()
            .expect("a row has a state count")
            .parse()
            .expect("count is a number");
        rows.push(Row {
            state,
            game,
            states,
        });
    }
    assert!(
        claimed > 0,
        "the fixture header must state its blockstate total"
    );
    (rows, claimed)
}

/// **The contract.** The model's light must be a LOWER BOUND on the game's at
/// every blockstate the game can produce. An entry brighter than vanilla lets a
/// genuinely dark area pass `DW0210`/`DW0211` and ship — the one failure this
/// whole module exists to prevent, and the one direction a player finds.
#[test]
fn emission_never_overestimates_the_pinned_game() {
    let (rows, claimed) = fixture();

    // The binding count, computed from the objects rather than written beside
    // them: a truncated or empty fixture must not read as a clean pass.
    let covered: usize = rows.iter().map(|r| r.states).sum();
    assert_eq!(
        covered, claimed,
        "the fixture's rows cover {covered} blockstates but its header claims {claimed} — \
         it was truncated or hand-edited"
    );

    let over: Vec<String> = rows
        .iter()
        .filter(|r| emission(&r.state) > r.game)
        .map(|r| format!("{} model={} game={}", r.state, emission(&r.state), r.game))
        .collect();
    assert!(
        over.is_empty(),
        "{} of {} rows model a block BRIGHTER than Minecraft 1.21.11 does:\n{}",
        over.len(),
        rows.len(),
        over.join("\n")
    );
}

/// Wherever the table is not deliberately taking a minimum, it must agree with
/// the game EXACTLY. This is what would answer differently if a value were
/// merely plausible: a remembered 7 for a block that is really 6, or a
/// state-dependent block collapsed onto one state, moves this assertion.
#[test]
fn emission_matches_the_pinned_game() {
    let (rows, _) = fixture();
    let deliberate: BTreeSet<&str> = DELIBERATE_MINIMUM.iter().map(|(id, _)| *id).collect();

    let mut checked = 0usize;
    let mut wrong = Vec::new();
    for r in &rows {
        if deliberate.contains(base_id(&r.state)) {
            continue;
        }
        checked += r.states;
        let ours = emission(&r.state);
        if ours != r.game {
            wrong.push(format!("{} model={} game={}", r.state, ours, r.game));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} rows disagree with Minecraft 1.21.11 (checked {checked} blockstates):\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    // A guard against the whole population being skipped by a widened
    // exclusion: the deliberate set is three blocks, so almost everything is
    // still measured.
    assert!(
        checked > 29_000,
        "only {checked} blockstates were actually compared — the exclusion set has eaten the gate"
    );
}

/// The round trip: the set of blocks measuring BELOW the game is exactly the
/// declared one. A new emitter in a future Minecraft, or a block quietly
/// dropped from the table, lands here rather than passing as an underestimate
/// — which is safe for the proof and silently costs a designer their fixture.
#[test]
fn the_only_underestimates_are_the_declared_ones() {
    let (rows, _) = fixture();
    let declared: BTreeMap<&str, &str> = DELIBERATE_MINIMUM.iter().copied().collect();

    let mut found: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &rows {
        if emission(&r.state) < r.game {
            *found.entry(base_id(&r.state)).or_default() += r.states;
        }
    }
    let found_ids: BTreeSet<&str> = found.keys().copied().collect();
    let declared_ids: BTreeSet<&str> = declared.keys().copied().collect();

    let undeclared: Vec<&&str> = found_ids.difference(&declared_ids).collect();
    assert!(
        undeclared.is_empty(),
        "these blocks emit light in Minecraft 1.21.11 and measure 0 (or low) here, with no \
         recorded reason — a room lit only by them refuses to build: {undeclared:?}"
    );
    let stale: Vec<&&str> = declared_ids.difference(&found_ids).collect();
    assert!(
        stale.is_empty(),
        "these blocks are declared as deliberate minima but now match the game exactly; \
         drop them from DELIBERATE_MINIMUM: {stale:?}"
    );
    // Every declared block must actually BIND — a reason recorded against a
    // block the fixture never exercises is a doc line.
    for (id, why) in DELIBERATE_MINIMUM {
        assert!(
            found.get(id).copied().unwrap_or(0) > 0,
            "{id} is declared a deliberate minimum ({why}) but binds to no blockstate"
        );
    }
}

/// The state axis, both directions, on the block that motivated the repair.
/// A repair that made every candle bright would break the contract in the
/// dangerous direction and this is where it shows.
#[test]
fn a_candle_is_dark_until_it_is_lit() {
    // Vanilla places candles UNLIT, and `candles` defaults to 1.
    assert_eq!(
        emission("minecraft:candle"),
        0,
        "a placed candle is not alight"
    );
    assert_eq!(emission("minecraft:candle[candles=4,lit=false]"), 0);
    assert_eq!(emission("minecraft:black_candle[candles=4,lit=false]"), 0);
    assert_eq!(emission("minecraft:candle_cake"), 0);
    assert_eq!(emission("minecraft:candle_cake[lit=false]"), 0);

    // Lit, it is 3 per candle — the value that makes a candlelit room buildable.
    assert_eq!(emission("minecraft:candle[candles=1,lit=true]"), 3);
    assert_eq!(emission("minecraft:candle[candles=2,lit=true]"), 6);
    assert_eq!(emission("minecraft:candle[candles=3,lit=true]"), 9);
    assert_eq!(emission("minecraft:candle[candles=4,lit=true]"), 12);
    // Every dyed candle is the same block with a different colour.
    assert_eq!(
        emission("minecraft:light_gray_candle[candles=4,lit=true]"),
        12
    );
    assert_eq!(emission("minecraft:candle_cake[lit=true]"), 3);
    assert_eq!(emission("minecraft:black_candle_cake[lit=true]"), 3);

    // The copper bulb's other axis: oxidation, and only while lit.
    assert_eq!(emission("minecraft:copper_bulb"), 0, "a bulb ships unlit");
    assert_eq!(emission("minecraft:copper_bulb[lit=true]"), 15);
    assert_eq!(emission("minecraft:exposed_copper_bulb[lit=true]"), 12);
    assert_eq!(emission("minecraft:weathered_copper_bulb[lit=true]"), 8);
    assert_eq!(emission("minecraft:oxidized_copper_bulb[lit=true]"), 4);
    assert_eq!(
        emission("minecraft:waxed_oxidized_copper_bulb[lit=true]"),
        4
    );

    // The copper lantern does NOT step down with oxidation — 15 throughout.
    assert_eq!(emission("minecraft:copper_lantern"), 15);
    assert_eq!(emission("minecraft:oxidized_copper_lantern"), 15);

    // The overestimate this round removed: a bare `glow_lichen` is the faceless
    // default state, which vanilla lights at 0, not 7.
    assert_eq!(emission("minecraft:glow_lichen"), 0);
    assert_eq!(
        emission(
            "minecraft:glow_lichen[down=false,east=false,north=false,south=false,up=true,waterlogged=false,west=false]"
        ),
        7
    );
}
