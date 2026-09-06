//! Round-8 island QA: the trigger/actor machinery defects the owner's playtest
//! surfaced, each pinned at the emission level.
//!
//! 1. **Shared-hitbox starvation.** Two `strike-npc` triggers legitimately ride one
//!    NPC's interaction hitbox. The old `tick` cleared the `attack` record inline,
//!    per trigger, right after that trigger's own fire clause — so the
//!    earlier-declared trigger consumed the click even with its gate shut and the
//!    later one could never fire. Declaration order silently decided which of two
//!    legal triggers worked. The tick is now two phases: every fire clause, then
//!    every clear clause.
//! 2. **The unleashed warden burrowed away.** `/summon <e> <pos> <nbt>` — any NBT
//!    compound — makes vanilla skip `finalizeSpawn`, which for a warden is the only
//!    place the `minecraft:dig_cooldown` brain memory is seeded. Proven on a live
//!    pinned 1.21.11 server: a bare summon gets `dig_cooldown ttl:1200`, a summon
//!    with `{}` gets an empty brain and digs itself out of the world in ~5s.
//! 3. **Aggro lock**: a hostile unleashed from a click trigger
//!    comes for the player who struck it, through the warden's own vanilla `anger`
//!    NBT — the one species-level aggro primitive that survives a tick on 1.21.11.
//! 4. **`despawn-actor` `vanish`** relocated to the command source's column (world
//!    spawn), not the actor's own.

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

const NS: &str = "hello-world";

fn scratch_dir(kind: &str) -> std::path::PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "dw-r8-{kind}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ))
}

fn build_dir(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
}

/// hello-world reshaped into the island's giant: ONE NPC (`npc/keeper`) carrying
/// TWO `strike-npc` triggers with mutually exclusive flag gates, plus the actor the
/// first of them unleashes. `trigger/wake` requires `flag/asleep`; `trigger/house`
/// requires `flag/sealed` and forbids `flag/asleep` — the island's exact shape.
///
/// `entity` is a parameter so the same fixture proves the warden path and a
/// species with no vanilla aggro primitive.
fn build_two_trigger(entity: &str) -> BuildOutput {
    let src = common::hello_world_dir();
    let dst = scratch_dir("two-trigger");
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    common::patch_file(&dst.join("quests.json"), |d| {
        d["dsl_version"] = serde_json::json!("0.6.0");
        d["content"]["triggers"] = serde_json::json!([
            { "id": "trigger/wake", "on": { "on": "strike-npc", "npc": "npc/keeper" },
              "once": false, "requires_flags": ["flag/asleep"],
              "effects": [
                  { "type": "narrate", "style": "chat", "text": "He wakes." },
                  { "type": "spawn-actor", "actor": "actor/giant" },
                  { "type": "unleash-actor", "actor": "actor/giant" }
              ] },
            { "id": "trigger/house", "on": { "on": "strike-npc", "npc": "npc/keeper" },
              "once": false, "requires_flags": ["flag/sealed"],
              "forbids_flags": ["flag/asleep"],
              "effects": [
                  { "type": "narrate", "style": "chat", "text": "He swats." }
              ] }
        ]);
        d["content"]["actors"] = serde_json::json!([
            { "id": "actor/giant", "entity": entity, "name": "The Sleeper",
              "anchor": "anchor/keeper-stand", "facing": "east" }
        ]);
    });
    let out = build_dir(&dst);
    let _ = std::fs::remove_dir_all(&dst);
    out
}

/// hello-world with FOUR actors each given their own `move-actor` in one
/// `sequence`, so the concurrency claim below has something to measure.
fn build_four_moves() -> BuildOutput {
    let src = common::hello_world_dir();
    let dst = scratch_dir("four-moves");
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    let steps: Vec<serde_json::Value> = (1..=4)
        .map(|i| {
            serde_json::json!({
                "at_ticks": (i - 1) * 20,
                "effects": [
                    { "type": "move-actor", "actor": format!("actor/a{i}"),
                      "to_anchor": "anchor/exit" }
                ]
            })
        })
        .collect();
    common::patch_file(&dst.join("quests.json"), |d| {
        d["dsl_version"] = serde_json::json!("0.6.0");
        let effects = common::objective_effects(d, 0, "obj/talk");
        for i in 1..=4 {
            effects
                .push(serde_json::json!({ "type": "spawn-actor", "actor": format!("actor/a{i}") }));
        }
        effects.push(serde_json::json!({ "type": "sequence", "steps": steps }));
        d["content"]["actors"] = serde_json::Value::Array(
            (1..=4)
                .map(|i| {
                    serde_json::json!({
                        "id": format!("actor/a{i}"), "entity": "minecraft:sheep",
                        "anchor": "anchor/keeper-stand"
                    })
                })
                .collect(),
        );
    });
    let out = build_dir(&dst);
    let _ = std::fs::remove_dir_all(&dst);
    out
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| {
                panic!(
                    "{path} emitted; have:\n{:#?}",
                    out.keys().collect::<Vec<_>>()
                )
            })
            .clone(),
    )
    .unwrap()
}

fn func(out: &BuildOutput, name: &str) -> String {
    text(
        out,
        &format!("datapack/data/{NS}/function/{name}.mcfunction"),
    )
}

// ---------------------------------------------------------------------------
// 1. shared-hitbox starvation
// ---------------------------------------------------------------------------

/// Both triggers really do ride ONE hitbox — the precondition that makes the
/// consumption order load-bearing at all.
#[test]
fn both_triggers_ride_the_one_npc_hitbox() {
    let out = build_two_trigger("minecraft:warden");
    let setup = func(&out, "setup_finish");
    let summon = setup
        .lines()
        .find(|l| l.contains("summon minecraft:interaction"))
        .expect("the keeper's hitbox is summoned");
    assert!(
        summon.contains("dw_trig_wake") && summon.contains("dw_trig_house"),
        "one interaction entity carries BOTH trigger tags:\n{summon}"
    );
    assert_eq!(
        setup.matches("summon minecraft:interaction").count(),
        1,
        "one cell, one hitbox:\n{setup}"
    );
}

/// **The regression.** Every fire clause must precede every clear clause: a
/// suppressed trigger may not consume a record its sibling has not been offered.
#[test]
fn every_fire_clause_precedes_every_clear_clause() {
    let out = build_two_trigger("minecraft:warden");
    let tick = func(&out, "tick");
    let last_fire = tick
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("run function hello-world:trig_"))
        .map(|(i, _)| i)
        .max()
        .expect("fire clauses emitted");
    let first_clear = tick
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("data remove entity @s attack"))
        .map(|(i, _)| i)
        .min()
        .expect("clear clauses emitted");
    assert!(
        last_fire < first_clear,
        "the LAST trigger dispatch (line {last_fire}) must come before the FIRST record \
         clear (line {first_clear}) — otherwise a gated-off trigger eats its sibling's \
         click:\n{tick}"
    );
    // Both triggers still get a clear (consumption is unchanged).
    assert_eq!(
        tick.matches("data remove entity @s attack").count(),
        2,
        "each trigger still clears its own record:\n{tick}"
    );
}

/// Every trigger — not only `once` ones — writes its fire sentinel, which is what
/// makes "which of the two actually ran" observable to PackTest at all.
#[test]
fn every_trigger_writes_its_fire_sentinel() {
    let out = build_two_trigger("minecraft:warden");
    for id in ["wake", "house"] {
        let body = func(&out, &format!("trig_{id}"));
        assert!(
            body.contains(&format!("scoreboard players set #trig_{id} dw.sys 1")),
            "trig_{id} records that it fired:\n{body}"
        );
    }
}

/// The generated PackTest makes the runtime claim both ways round.
#[test]
fn shared_hitbox_packtest_asserts_both_directions() {
    let out = build_two_trigger("minecraft:warden");
    let t = text(
        &out,
        &format!("packtest-datapack/data/{NS}/test/v06_shared_hitbox.mcfunction"),
    );
    // Precondition: one entity, both tags.
    assert!(
        t.contains("if entity @e[type=minecraft:interaction,tag=dw_trig_wake,tag=dw_trig_house]"),
        "template proves the hitbox is shared:\n{t}"
    );
    // The starved trigger fires while the earlier one is gated shut…
    assert!(
        t.contains("assert score #trig_house dw.sys matches 1")
            && t.contains("assert score #trig_wake dw.sys matches 0"),
        "template pins the starvation regression:\n{t}"
    );
    // …and the earlier one is reachable on its own flags.
    assert!(
        t.contains("assert score #trig_wake dw.sys matches 1"),
        "template pins the mirror direction:\n{t}"
    );
    // Flags are party state on a shared batch server: handed back untouched.
    assert!(
        t.trim_end()
            .ends_with(&format!("function {NS}:setup_finish")),
        "template restores the world on exit:\n{t}"
    );
    for f in ["dw.f_asleep", "dw.f_sealed"] {
        assert!(
            t.contains(&format!("scoreboard players set #party {f} 0")),
            "template clears {f} on exit:\n{t}"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. the burrowing warden
// ---------------------------------------------------------------------------

/// A warden twin carries the brain memory vanilla's skipped `finalizeSpawn` would
/// have given it. Without it the mob digs down and despawns within ~5s (live-proven
/// on the pinned server).
#[test]
fn unleashed_warden_carries_the_dig_cooldown_finalize_spawn_skips() {
    let out = build_two_trigger("minecraft:warden");
    let unleash = func(&out, "unleash_giant");
    assert!(
        unleash.contains("Brain:{memories:{\"minecraft:dig_cooldown\":{value:{},ttl:1200L}}}"),
        "the twin summon seeds vanilla's own dig cooldown:\n{unleash}"
    );
}

/// A species `finalizeSpawn` does nothing load-bearing for is untouched — the
/// byte-identity guarantee for every campaign that unleashes something else.
#[test]
fn non_warden_twin_summon_is_unchanged() {
    let out = build_two_trigger("minecraft:zombie");
    let unleash = func(&out, "unleash_giant");
    assert!(
        !unleash.contains("Brain:"),
        "no finalize NBT for a species that needs none:\n{unleash}"
    );
}

// ---------------------------------------------------------------------------
// 3. aggro lock
// ---------------------------------------------------------------------------

/// The click trigger parks the striking player's UUID for the length of its own
/// bundle, and drops it again so it can never bleed into a later beat.
#[test]
fn click_trigger_captures_and_releases_the_striker() {
    let out = build_two_trigger("minecraft:warden");
    let wake = func(&out, "trig_wake");
    assert!(
        wake.contains(
            "data modify storage dw:strike player set from entity @e[tag=dw_trig_wake,limit=1] attack.player"
        ),
        "the striker is captured from the click record:\n{wake}"
    );
    assert!(
        wake.trim_end()
            .ends_with("data remove storage dw:strike player"),
        "and released at the end of the bundle:\n{wake}"
    );
    // The sibling trigger unleashes nothing, so it captures nothing.
    let house = func(&out, "trig_house");
    assert!(
        !house.contains("dw:strike"),
        "a trigger that never unleashes does not touch the storage:\n{house}"
    );
}

/// The warden's vanilla `anger` primitive, seeded from the captured UUID.
#[test]
fn unleashed_warden_locks_onto_the_striker() {
    let out = build_two_trigger("minecraft:warden");
    let unleash = func(&out, "unleash_giant");
    assert!(
        unleash.contains(
            "execute if data storage dw:strike player run data modify entity @e[tag=dw_actor_giant,limit=1] anger.suspects set value [{anger:150,uuid:[I;0,0,0,0]}]"
        ),
        "a max-anger suspect slot is written:\n{unleash}"
    );
    assert!(
        unleash.contains("anger.suspects[0].uuid set from storage dw:strike player"),
        "and its UUID comes from the striker storage:\n{unleash}"
    );
    // Order matters: the puppet must be gone before the twin is addressed by the
    // body tag, or `limit=1` could pick the puppet.
    let kill = unleash.find("kill @e[tag=dw_pup_giant]").unwrap();
    let lock = unleash.find("anger.suspects").unwrap();
    assert!(
        kill < lock,
        "the lock addresses the twin, not the puppet:\n{unleash}"
    );
}

/// A species with no vanilla way to be handed a target gets none invented for it —
/// it falls back to its own nearest-player acquisition (documented limit). This
/// covers the `NeutralMob` families too: `AngerTime`/`AngryAt` looked like the
/// primitive but does not survive a tick on 1.21.11 (tested live against endermen,
/// piglins, wolves and iron golems, with a real player's UUID), so nothing is
/// emitted for them either.
#[test]
fn species_without_an_aggro_primitive_gets_no_invented_one() {
    for entity in [
        "minecraft:zombie",
        "minecraft:zombified_piglin",
        "minecraft:enderman",
    ] {
        let out = build_two_trigger(entity);
        let unleash = func(&out, "unleash_giant");
        assert!(
            !unleash.contains("dw:strike") && !unleash.contains("AngryAt"),
            "no aggro-lock hardware for {entity}, which vanilla gives no working \
             primitive:\n{unleash}"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. despawn-actor `vanish`
// ---------------------------------------------------------------------------

/// `vanish` drops the body straight down ITS OWN column. `tp <targets> ~ -128 ~`
/// resolved `~ ~` against the command source (world spawn) — live-observed on the
/// island: a puppet standing at `6.5,-55.5` died at `10.0,-128.0,9.0`.
#[test]
fn vanish_relocates_each_actor_relative_to_itself() {
    let out = build_four_moves_with_vanish();
    let arrive = func(&out, "ma_arrive_a1_exit");
    assert!(
        arrive.contains("execute as @e[tag=dw_actor_a1] at @s run tp @s ~ -128 ~"),
        "the drop is per-actor, not per-command-source:\n{arrive}"
    );
    assert!(
        !arrive.contains("tp @e[tag=dw_actor_a1] ~ -128 ~"),
        "the source-relative form is gone:\n{arrive}"
    );
}

/// hello-world with four moving actors, the first of which vanishes on arrival.
fn build_four_moves_with_vanish() -> BuildOutput {
    let src = common::hello_world_dir();
    let dst = scratch_dir("vanish");
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    common::patch_file(&dst.join("quests.json"), |d| {
        d["dsl_version"] = serde_json::json!("0.6.0");
        common::objective_effects(d, 0, "obj/talk").extend([
            serde_json::json!({ "type": "spawn-actor", "actor": "actor/a1" }),
            serde_json::json!({
                "type": "move-actor", "actor": "actor/a1", "to_anchor": "anchor/exit",
                "on_arrive": [
                    { "type": "despawn-actor", "actor": "actor/a1", "style": "vanish" }
                ]
            }),
        ]);
        d["content"]["actors"] = serde_json::json!([
            { "id": "actor/a1", "entity": "minecraft:sheep", "anchor": "anchor/keeper-stand" }
        ]);
    });
    let out = build_dir(&dst);
    let _ = std::fs::remove_dir_all(&dst);
    out
}

// ---------------------------------------------------------------------------
// 5. concurrent move-actor drivers
// ---------------------------------------------------------------------------

/// N `move-actor`s in flight at once all advance: each has its own driver pair, its
/// own run latch and its own step counter, and each teleports only its own puppet.
/// Nothing is shared between them, so no ordering of their starts can starve one —
/// the property the island's four-sheep-plus-giant cinematic relies on.
#[test]
fn concurrent_move_actors_share_no_state() {
    let out = build_four_moves();
    let mut totals = Vec::new();
    for i in 1..=4 {
        let bare = format!("a{i}_exit");
        let start = func(&out, &format!("ma_{bare}"));
        assert!(
            start.contains(&format!("scoreboard players set #arun_{bare} dw.sys 1"))
                && start.contains(&format!("scoreboard players set #at_{bare} dw.sys 0"))
                && start.contains(&format!("schedule function {NS}:ma_tick_{bare} 1t")),
            "actor a{i} has its own latch, counter and driver:\n{start}"
        );
        let tick = func(&out, &format!("ma_tick_{bare}"));
        // Every teleport in this driver addresses this actor's puppet and no other.
        for line in tick.lines().filter(|l| l.contains(" run tp ")) {
            assert!(
                line.contains(&format!("tp @e[tag=dw_pup_a{i}]")),
                "driver {bare} moves only its own puppet:\n{line}"
            );
        }
        // …and advances its own counter, not a shared one.
        assert!(
            tick.contains(&format!("scoreboard players add #at_{bare} dw.sys 1")),
            "driver {bare} advances its own counter:\n{tick}"
        );
        totals.push(tick.matches(" run tp ").count());
    }
    assert!(
        totals.iter().all(|n| *n > 1),
        "each driver really walks a path (waypoint counts {totals:?})"
    );
    // No two drivers share a scoreboard holder or a function name.
    let holders: Vec<String> = (1..=4).map(|i| format!("#at_a{i}_exit")).collect();
    for h in &holders {
        let uses: usize = (1..=4)
            .map(|i| {
                func(&out, &format!("ma_tick_a{i}_exit"))
                    .matches(h.as_str())
                    .count()
            })
            .filter(|n| *n > 0)
            .count();
        assert_eq!(uses, 1, "{h} belongs to exactly one driver");
    }
}
