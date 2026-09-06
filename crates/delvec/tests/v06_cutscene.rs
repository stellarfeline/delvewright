//! DSL v0.6 cutscene camera aim + multi-shot emission.
//!
//! Asserts the *rotation* baked into every dolly `tp` (the fix for a cutscene
//! that framed the open sea instead of its subject), the default aim along the
//! direction of travel, the back-to-back multi-shot timeline inside one
//! save/restore bracket, and that the two single-shot spellings are byte-
//! identical.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::{Plan, ResolvedAnchor};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

const NS: &str = "hello-world";

/// A v0.6 quests document whose exit beat plays the given `cutscene` effect.
fn quests_doc(cutscene: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {cutscene}, {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn build_plan(
    campaign: &'static Campaign,
    prefabs: &'static PrefabRegistry,
) -> (Plan<'static>, BuildOutput) {
    let plan = Plan::build(campaign, prefabs).expect("plan builds");
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
    let out = emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &tree,
        prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("every emitted command validates");
    (plan, out)
}

/// Build the hello-world campaign with `cutscene` on its exit beat. The campaign
/// and prefab registry are intentionally leaked so the returned `Plan` (which
/// borrows both) can outlive this call — a test-only convenience.
fn build(cutscene: &str) -> (Plan<'static>, BuildOutput) {
    let prefabs: &'static PrefabRegistry = Box::leak(Box::new(
        PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap(),
    ));
    let campaign: &'static Campaign = Box::leak(Box::new(parse_hw(&quests_doc(cutscene))));
    build_plan(campaign, prefabs)
}

/// The block-centre world point of `anchor + offset` — the same convention the
/// compiler resolves camera waypoints and `look_at` subjects with.
fn anchor_point(plan: &Plan, anchor: &str, offset: [i32; 3]) -> [f64; 3] {
    let base = plan
        .anchors
        .iter()
        .find(|((_, name), _)| name == anchor)
        .map(|(_, r)| match r {
            ResolvedAnchor::Point { pos, .. } => *pos,
            ResolvedAnchor::Gate { from, .. } => *from,
        })
        .expect("anchor resolves");
    [
        (base[0] + offset[0]) as f64 + 0.5,
        (base[1] + offset[1]) as f64 + 0.5,
        (base[2] + offset[2]) as f64 + 0.5,
    ]
}

/// The sole generated cutscene tick function's body.
fn tick_body(out: &BuildOutput) -> String {
    let key = out
        .keys()
        .find(|k| k.contains(&format!("data/{NS}/function/cs_tick_")))
        .expect("a cutscene tick function is emitted")
        .clone();
    String::from_utf8(out[&key].clone()).unwrap()
}

/// The cutscene's `start` function body (`cs_<bare>`, not `cs_tick_`/`cs_end_`).
fn start_body(out: &BuildOutput) -> String {
    let prefix = format!("data/{NS}/function/cs_");
    let key = out
        .keys()
        .find(|k| {
            k.contains(&prefix)
                && !k.contains("function/cs_tick_")
                && !k.contains("function/cs_end_")
        })
        .expect("a cutscene start function is emitted")
        .clone();
    String::from_utf8(out[&key].clone()).unwrap()
}

/// One dolly frame: `(tick, position, yaw, pitch)` parsed from a `tp` line.
fn frames(body: &str) -> Vec<(i64, [f64; 3], f64, f64)> {
    let mut out = Vec::new();
    for line in body.lines() {
        let Some((head, tail)) = line.split_once(" run tp @e[tag=dw_cam_") else {
            continue;
        };
        let t: i64 = head
            .rsplit(' ')
            .next()
            .and_then(|s| s.parse().ok())
            .expect("tick index");
        let args: Vec<f64> = tail
            .split_once("] ")
            .expect("selector")
            .1
            .split_whitespace()
            .map(|s| s.parse().expect("float arg"))
            .collect();
        assert_eq!(args.len(), 5, "tp must carry x y z yaw pitch: {line}");
        out.push((t, [args[0], args[1], args[2]], args[3], args[4]));
    }
    assert!(!out.is_empty(), "no dolly frames parsed");
    out
}

/// Minecraft entity rotation aiming `pos` at `target`, re-derived independently
/// of the compiler: yaw 0 = +Z (south), 90 = −X (west); pitch positive = down.
fn expect_aim(pos: [f64; 3], target: [f64; 3]) -> (f64, f64) {
    let d = [target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]];
    let horiz = (d[0] * d[0] + d[2] * d[2]).sqrt();
    let yaw = (-d[0]).atan2(d[2]).to_degrees();
    let pitch = (-d[1]).atan2(horiz).to_degrees();
    (yaw, pitch)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

/// A dolly that travels east-west while its subject sits to the north: with
/// `look_at`, every frame's rotation aims at the subject from that frame's own
/// position, and the camera visibly turns across the move.
#[test]
fn look_at_aims_every_frame_at_the_subject() {
    let (plan, out) = build(
        r#"{ "type": "cutscene", "seconds": 2,
             "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                       { "anchor": "anchor/exit", "offset": [2, 2, 0] } ],
             "look_at": { "anchor": "anchor/keeper-stand", "offset": [0, 2, 0] } }"#,
    );
    let subject = anchor_point(&plan, "anchor/keeper-stand", [0, 2, 0]);
    let fr = frames(&tick_body(&out));
    for (t, pos, yaw, pitch) in &fr {
        let (ey, ep) = expect_aim(*pos, subject);
        assert!(
            close(*yaw, ey) && close(*pitch, ep),
            "frame {t}: emitted ({yaw}, {pitch}) != aim-at-subject ({ey}, {ep})"
        );
    }
    assert!(
        !close(fr[0].2, fr[fr.len() - 1].2),
        "a look_at camera crossing its subject must turn: {} vs {}",
        fr[0].2,
        fr[fr.len() - 1].2
    );
}

/// Without `look_at`, the camera faces along the direction of travel — NOT the
/// vanilla summon default (yaw 0 = south), which is what framed the open sea.
#[test]
fn default_aim_follows_the_direction_of_travel() {
    let (_, out) = build(
        r#"{ "type": "cutscene", "seconds": 2,
             "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                       { "anchor": "anchor/exit", "offset": [2, 2, 0] } ] }"#,
    );
    let fr = frames(&tick_body(&out));
    // Travel is +X (east) → yaw −90, level pitch, constant for the whole shot.
    for (t, _, yaw, pitch) in &fr {
        assert!(
            close(*yaw, -90.0) && close(*pitch, 0.0),
            "frame {t}: expected the travel-direction aim (−90, 0), got ({yaw}, {pitch})"
        );
    }
}

/// Two shots play back-to-back inside ONE save/restore bracket: a single
/// spectator/marker/camera-pair setup, one tick counter carrying both shots'
/// frames, and one restore at the end of the second shot.
#[test]
fn multi_shot_chains_two_dollies_in_one_bracket() {
    let (plan, out) = build(
        r#"{ "type": "cutscene", "shots": [
             { "seconds": 2,
               "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                         { "anchor": "anchor/exit", "offset": [2, 2, 0] } ] },
             { "seconds": 1,
               "path": [ { "anchor": "anchor/keeper-stand", "offset": [0, 2, 1] } ],
               "look_at": { "anchor": "anchor/keeper-stand", "offset": [0, 1, 0] } } ] }"#,
    );
    let tick = tick_body(&out);
    let fr = frames(&tick);
    // Keyframe cadence: shot 1 (2 s = 40 ticks, a straight dolly →
    // widest cadence 10) emits the tick-0 snap + keyframes at 1, 11, 21, 31,
    // with the client tweening between them via `teleport_duration:10`. Shot 2
    // is a single-waypoint static shot: just its snap at 41 (the hard cut).
    let ticks: Vec<i64> = fr.iter().map(|f| f.0).collect();
    assert_eq!(ticks, vec![0, 1, 11, 21, 31, 41], "keyframe timeline");
    // Shot 1 arms its cadence on its first tick (the snap still lands
    // instantly: position syncs flush before metadata within a tick) and
    // re-arms the hard cut by resetting the tween on its last owned tick.
    assert!(
        tick.contains("matches 0 as @e[tag=dw_cam_") && tick.contains("{teleport_duration:10}"),
        "shot 1 arms teleport_duration:10 at its first tick:\n{tick}"
    );
    assert!(
        tick.contains("matches 40 as @e[tag=dw_cam_") && tick.contains("{teleport_duration:0}"),
        "the tween resets one tick before the next shot's snap:\n{tick}"
    );
    // A static shot never arms a tween of its own.
    assert!(
        !tick.contains("matches 41 as @e[tag=dw_cam_"),
        "static shot 2 needs no teleport_duration:\n{tick}"
    );
    // The hard cut: frame 41 is the second shot's (static) first waypoint.
    let close_up = anchor_point(&plan, "anchor/keeper-stand", [0, 2, 1]);
    let cut = fr.iter().find(|f| f.0 == 41).unwrap();
    assert_eq!(cut.1, close_up, "shot 2 opens on its own first waypoint");
    // …aimed at its own subject, which shot 1 did not use.
    let (ey, ep) = expect_aim(
        close_up,
        anchor_point(&plan, "anchor/keeper-stand", [0, 1, 0]),
    );
    assert!(
        close(cut.2, ey) && close(cut.3, ep),
        "shot 2 must use its own look_at: ({}, {}) != ({ey}, {ep})",
        cut.2,
        cut.3
    );
    // One bracket: the driver ends once, after the last frame.
    assert!(
        tick.contains("matches 62.. run function hello-world:cs_end_"),
        "the restore fires one tick after the last frame:\n{tick}"
    );
    let start = start_body(&out);
    assert_eq!(
        start.matches("gamemode spectator @a").count(),
        1,
        "one gamemode save for the whole cutscene"
    );
    assert_eq!(
        start.matches("summon minecraft:item_display").count(),
        2,
        "one camera pair for the whole cutscene"
    );
}

/// The harness hint is the WHOLE cinematic's duration, so a bot waits out every
/// shot (2 s + 1 s = 3 s), not just the first.
#[test]
fn critical_path_cutscene_seconds_is_the_total() {
    let (_, out) = build(
        r#"{ "type": "cutscene", "shots": [
             { "seconds": 2,
               "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                         { "anchor": "anchor/exit", "offset": [2, 2, 0] } ] },
             { "seconds": 1,
               "path": [ { "anchor": "anchor/keeper-stand", "offset": [0, 2, 1] } ] } ] }"#,
    );
    let cp = String::from_utf8(out["critical-path.json"].clone()).unwrap();
    assert!(
        cp.contains("\"cutscene_seconds\": 3"),
        "total cutscene duration must reach the harness:\n{cp}"
    );
}

/// The v0.4 single-shot spelling and a one-entry `shots` list are the same
/// cutscene: identical function names, identical bytes, everywhere.
#[test]
fn single_shot_spellings_are_byte_identical() {
    let path = r#""path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                            { "anchor": "anchor/exit", "offset": [2, 2, 0] } ]"#;
    let (_, legacy) = build(&format!(
        r#"{{ "type": "cutscene", "seconds": 2, {path} }}"#
    ));
    let (_, shots) = build(&format!(
        r#"{{ "type": "cutscene", "shots": [ {{ "seconds": 2, {path} }} ] }}"#
    ));
    assert_eq!(
        legacy.keys().collect::<Vec<_>>(),
        shots.keys().collect::<Vec<_>>(),
        "both spellings emit the same file set"
    );
    assert!(legacy == shots, "both spellings emit identical bytes");
}

/// Same DSL in, same bytes out (ADR-0006) — the aim math is pure `atan2` with a
/// fixed rounding, so a rebuild reproduces every rotation exactly.
#[test]
fn cutscene_aim_emission_is_deterministic() {
    let body = r#"{ "type": "cutscene", "seconds": 3,
        "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                  { "anchor": "anchor/exit", "offset": [2, 2, 1] } ],
        "look_at": { "anchor": "anchor/keeper-stand", "offset": [0, 2, 0] } }"#;
    let (_, a) = build(body);
    let (_, b) = build(body);
    assert!(
        a == b,
        "cutscene emission must be byte-identical on rebuild"
    );
}

// ---------------------------------------------------------------------------
// Shot styles (spec-0015): deterministic expansion at emission
// ---------------------------------------------------------------------------

/// `push-in` expands to a dolly toward its subject: the camera starts at
/// `dist` and ends at a third of it (min 2), every frame aimed at the subject;
/// the style's default duration (4 s) shapes the driver timeline.
#[test]
fn push_in_expands_toward_subject() {
    let (plan, out) = build(
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "push-in", "dist": 3.5, "bearing": 90,
               "subject": { "anchor": "anchor/keeper-stand", "offset": [0, 1, 0] } } ] }"#,
    );
    // An anchor subject aims at the block centre exactly (entity subjects lift
    // one block to torso height; anchors do not).
    let subject = anchor_point(&plan, "anchor/keeper-stand", [0, 1, 0]);
    let tick = tick_body(&out);
    let fr = frames(&tick);
    let d = |p: &[f64; 3]| {
        ((p[0] - subject[0]).powi(2) + (p[1] - subject[1]).powi(2) + (p[2] - subject[2]).powi(2))
            .sqrt()
    };
    let (first, last) = (&fr[0], &fr[fr.len() - 1]);
    assert!(
        d(&first.1) > d(&last.1) + 1.0,
        "push-in closes distance: {} -> {}",
        d(&first.1),
        d(&last.1)
    );
    for (t, pos, yaw, pitch) in &fr {
        let (ey, ep) = expect_aim(*pos, subject);
        assert!(
            close(*yaw, ey) && close(*pitch, ep),
            "frame {t}: styled aim must track the subject: ({yaw}, {pitch}) != ({ey}, {ep})"
        );
    }
    // Default duration: push-in = 4 s = 80 ticks; restore fires at 81.
    assert!(
        tick.contains("matches 81.. run function hello-world:cs_end_"),
        "style default seconds shape the timeline:\n{tick}"
    );
}

/// An explicit `seconds` always overrides the style default.
#[test]
fn explicit_seconds_overrides_style_default() {
    let (_, out) = build(
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "push-in", "dist": 3.5, "bearing": 90, "seconds": 2,
               "subject": { "anchor": "anchor/keeper-stand", "offset": [0, 1, 0] } } ] }"#,
    );
    let tick = tick_body(&out);
    assert!(
        tick.contains("matches 41.. run function hello-world:cs_end_"),
        "explicit 2 s (40 ticks) overrides the 4 s default:\n{tick}"
    );
}

/// `orbit-arc` holds a constant radius around its subject for the whole sweep.
#[test]
fn orbit_arc_holds_radius() {
    let (plan, out) = build(
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "orbit-arc", "dist": 2, "degrees": 90, "bearing": 90,
               "subject": { "anchor": "anchor/keeper-stand", "offset": [0, 0, -1] } } ] }"#,
    );
    let subject = anchor_point(&plan, "anchor/keeper-stand", [0, 0, -1]);
    let fr = frames(&tick_body(&out));
    assert!(fr.len() > 2, "an orbit is a moving shot");
    for (t, pos, _, _) in &fr {
        let horiz = ((pos[0] - subject[0]).powi(2) + (pos[2] - subject[2]).powi(2)).sqrt();
        assert!(
            (horiz - 2.0).abs() < 0.05,
            "frame {t}: orbit radius drifts: {horiz}"
        );
    }
}

/// `two-shot` places a static camera equidistant from both subjects (the
/// Toric-inspired perpendicular-bisector construction), aimed at their
/// midpoint.
#[test]
fn two_shot_is_equidistant_and_aims_at_midpoint() {
    let (plan, out) = build(
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "two-shot", "dist": 2, "bearing": 180,
               "subject": { "anchor": "anchor/keeper-stand", "offset": [0, 1, 0] },
               "subject_b": { "anchor": "anchor/keeper-stand", "offset": [-3, 1, 0] } } ] }"#,
    );
    let a = anchor_point(&plan, "anchor/keeper-stand", [0, 1, 0]);
    let b = anchor_point(&plan, "anchor/keeper-stand", [-3, 1, 0]);
    let fr = frames(&tick_body(&out));
    assert_eq!(fr.len(), 1, "a two-shot is static");
    let cam = fr[0].1;
    let da = ((cam[0] - a[0]).powi(2) + (cam[2] - a[2]).powi(2)).sqrt();
    let db = ((cam[0] - b[0]).powi(2) + (cam[2] - b[2]).powi(2)).sqrt();
    assert!((da - db).abs() < 0.01, "equidistant: {da} vs {db}");
    let mid = [
        (a[0] + b[0]) / 2.0,
        (a[1] + b[1]) / 2.0,
        (a[2] + b[2]) / 2.0,
    ];
    let (ey, ep) = expect_aim(cam, mid);
    assert!(
        close(fr[0].2, ey) && close(fr[0].3, ep),
        "aims at the midpoint: ({}, {}) != ({ey}, {ep})",
        fr[0].2,
        fr[0].3
    );
}

/// `side-track` rides the subject's compiler-planned `move-npc` path at a
/// constant world offset: the camera keyframes actually travel with the move.
#[test]
fn side_track_rides_the_move_path() {
    let (_, out) = build(
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "side-track", "dist": 1, "seconds": 2,
               "subject": { "npc": "npc/keeper" } } ] },
           { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }"#,
    );
    let fr = frames(&tick_body(&out));
    assert!(
        fr.len() > 2,
        "a side-track of a moving subject is a moving shot: {fr:?}"
    );
    let total: f64 = fr
        .windows(2)
        .map(|w| {
            ((w[1].1[0] - w[0].1[0]).powi(2)
                + (w[1].1[1] - w[0].1[1]).powi(2)
                + (w[1].1[2] - w[0].1[2]).powi(2))
            .sqrt()
        })
        .sum();
    assert!(total > 1.0, "the camera actually travels: {total}");
}

/// Two cutscenes that share a first waypoint, duration and waypoint count but
/// frame different subjects must not collapse onto one generated function.
#[test]
fn shots_with_different_subjects_get_distinct_functions() {
    let (_, out) = build(
        r#"{ "type": "cutscene", "shots": [
             { "seconds": 2,
               "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                         { "anchor": "anchor/exit", "offset": [2, 2, 0] } ],
               "look_at": { "anchor": "anchor/keeper-stand", "offset": [0, 2, 0] } } ] }"#,
    );
    let (_, other) = build(
        r#"{ "type": "cutscene", "shots": [
             { "seconds": 2,
               "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                         { "anchor": "anchor/exit", "offset": [2, 2, 0] } ],
               "look_at": { "anchor": "anchor/keeper-stand", "offset": [0, 3, 0] } } ] }"#,
    );
    let name = |o: &BuildOutput| {
        o.keys()
            .find(|k| k.contains(&format!("data/{NS}/function/cs_tick_")))
            .cloned()
            .unwrap()
    };
    assert_ne!(
        name(&out),
        name(&other),
        "a different look_at is a different cutscene"
    );
}

// ---------------------------------------------------------------------------
// Sneak-gated spectate bounce (round-6 camera-flicker fix)
// ---------------------------------------------------------------------------

/// The per-tick `spectate` bounce must never target a player actively holding
/// sneak: in spectator mode the sneak key dismounts the spectated entity, so an
/// unconditional re-attach strobes (attach → client dismount → attach …) for as
/// long as the key is held. Every bounce line is gated on the negated
/// `<ns>:sneak_held` input predicate — a held sneak yields a stable detached
/// spectator, and release resumes the shot on the next bounce tick.
#[test]
fn spectate_bounce_is_sneak_gated() {
    let (_, out) = build(
        r#"{ "type": "cutscene", "seconds": 2,
             "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                       { "anchor": "anchor/exit", "offset": [2, 2, 0] } ] }"#,
    );
    let tick = tick_body(&out);
    let gated = format!("as @a[predicate=!{NS}:sneak_held] run spectate ");
    assert_eq!(
        tick.matches(&gated).count(),
        2,
        "both bounce parities re-attach only non-sneaking players:\n{tick}"
    );
    assert!(
        !tick.contains("as @a run spectate"),
        "no unguarded re-attach may survive — it is the flicker loop:\n{tick}"
    );
}

/// A campaign with a cutscene ships the `sneak_held` predicate: the vanilla
/// `minecraft:player` `input` sub-predicate reading the raw sneak key (which is
/// reported in every gamemode, spectator included).
#[test]
fn cutscene_campaign_emits_the_sneak_held_predicate() {
    let (_, out) = build(
        r#"{ "type": "cutscene", "seconds": 2,
             "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                       { "anchor": "anchor/exit", "offset": [2, 2, 0] } ] }"#,
    );
    let path = format!("datapack/data/{NS}/predicate/sneak_held.json");
    let body = std::str::from_utf8(out.get(&path).expect("predicate emitted")).unwrap();
    let json: serde_json::Value = serde_json::from_str(body).expect("predicate is JSON");
    assert_eq!(
        json["condition"], "minecraft:entity_properties",
        "entity_properties condition"
    );
    assert_eq!(
        json["predicate"]["type_specific"]["type"], "minecraft:player",
        "player sub-predicate"
    );
    assert_eq!(
        json["predicate"]["type_specific"]["input"]["sneak"], true,
        "matches the held sneak key"
    );
}

/// A campaign without any cutscene emits no predicate at all — the gate's only
/// consumer is the bounce, so everything else stays byte-identical.
#[test]
fn cutscene_less_campaign_emits_no_predicate() {
    let prefabs: &'static PrefabRegistry = Box::leak(Box::new(
        PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap(),
    ));
    let campaign: &'static Campaign = Box::leak(Box::new(parse_hw(&read_hw("quests.json"))));
    let (_, out) = build_plan(campaign, prefabs);
    assert!(
        !out.keys().any(|k| k.contains("/predicate/")),
        "no predicate directory for a cutscene-less campaign"
    );
}
