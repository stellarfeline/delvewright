//! Shortcut doors and their wrong-side answer (spec-0016 §2).

use super::*;

/// `setup_finish` commands for shortcut doors (spec-0016 §2): summon the
/// far-side unlock affordance (a right-click target, the same
/// `minecraft:interaction` primitive as a trap disarm) **and its visible
/// hardware**. The gate needs no command at all — it is **physically sealed in
/// the prefab** from world-load, which is precisely why the pattern needs no
/// "seal it now" verb and why permanence can be structural. Empty for a
/// campaign with no shortcut.
///
/// The hardware is not decoration. `minecraft:interaction` is an invisible
/// hitbox, so the hitbox alone asks the player to right-click a point nothing
/// marks — the drowned-bell soft-lock, where the unlock cell was bare air and
/// the only visible thing there belonged to an unrelated objective. The
/// compiler owns the affordance's visibility; it is never left to whether the
/// tileset happens to carry a lever. Proven by `DW0420`
/// ([`crate::compiler::affordance`]).
pub(super) fn shortcut_setup(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        let v = ent_xyz(sc.unlock);
        out.push(format!(
            "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"dw_sc_{}\"]}}",
            v[0], v[1], v[2], sc.safe
        ));
        out.push(affordance_hardware(
            v,
            &format!("dw_sc_{}", sc.safe),
            "minecraft:lever",
        ));
        // Arm the door itself, so a press from the sealed side reaches
        // something. Unlike a `close-gate` seal — which is armed by the firing
        // that seals it — a shortcut gate is sealed by the PREFAB at world-load,
        // so world init is the only moment its answer can go up. Guarded on
        // absence for the same reason the close-gate arming is: a second,
        // co-located set of hitboxes is the exact ray-pick tie `DW0422` forbids.
        out.push(format!(
            "execute unless entity @e[tag=dw_ws_{}] run function {ns}:ws_arm_{}",
            sc.safe, sc.safe
        ));
    }
    out
}

// The `seal_hint_<safe>` reward functions and their `seal_<safe>` advancements
// are GONE (DSL v0.11). They were `close-gate`'s private copy of "a pressable
// thing answers the player who pressed it": its own advancement shape, its own
// actionbar command, its own baked English — none of which has anything to do
// with closing a gate, and none of which the second object that needed them (a
// sealed shortcut door) could reach.
//
// A seal's answer is now an ordinary `EnvTrigger{on: use, audience: presser}`
// carrying an ordinary `narrate{style: actionbar}`, synthesized by
// `plan::collect_press_answers` and emitted by `env_trigger_fns` /
// `press_dispatch_fn` / `emit_advancements` like any author's own click. The
// wording is unchanged, the revoke-every-press behaviour is unchanged, and the
// shortcut door gets all of it for free — which is the whole finding.

/// Every shortcut door with its derived sealed side.
///
/// **Every** shortcut gets a body — a door with no answer is still a door a
/// player walks up to and pushes — so the only shortcut missing here is one whose
/// side did not resolve, and such a campaign never reaches emission:
/// [`check_shortcut_sides`] fails the build first. That is why `DW0425` binds to
/// every shortcut rather than to the ones that authored something.
pub(super) fn answering_shortcuts<'a>(
    plan: &'a Plan,
) -> Vec<(
    &'a plan::ShortcutPlan,
    &'a crate::compiler::wrongside::SealedSide,
)> {
    plan.shortcuts
        .iter()
        .filter_map(|sc| sc.sealed_side.as_ref().map(|s| (sc, s)))
        .collect()
}

/// `DW0425`: a shortcut door whose sealed side the geometry does not name.
///
/// Build tier (exit 3), raised before any function is emitted — withhold, never
/// invent. Placing the answer on a guessed side would tell a player standing
/// exactly where the door DOES open that it cannot be opened from there, which is
/// a worse failure than the silence this feature exists to end.
pub(super) fn check_shortcut_sides(plan: &Plan) -> Result<(), BuildFailure> {
    for sc in &plan.shortcuts {
        if sc.sealed_side.is_some() {
            continue;
        }
        let (lo, hi) = sc.gate_region;
        return Err(BuildFailure::Diagnostic {
            code: crate::compiler::wrongside::DW_SHORTCUT_SIDE_UNDECIDABLE,
            message: format!(
                "shortcut `{}` needs a clickable body on the sealed side of its gate `{}`, but \
                 the compiler cannot tell which side that is. The sealed side is derived from \
                 the gate slab's thin axis and the side of it the `unlock` anchor `{}` stands on; \
                 here the gate spans {lo:?}..{hi:?} and the unlock resolves to {:?}, which either \
                 gives the region no unique thinnest axis (a cube is not a doorway) or leaves the \
                 unlock level with the doorway rather than beyond it. An answer placed on a \
                 guessed side would fire where the door DOES open. Prescription: put the `unlock` \
                 clear of the gate's own span on the axis the door is thin on — which is where a \
                 far-side bar belongs anyway — or use a gate anchor whose region is a doorway \
                 slab rather than a volume.",
                sc.id, sc.gate_anchor, sc.unlock_anchor, sc.unlock,
            ),
        });
    }
    Ok(())
}

/// The `ws_arm_<safe>` functions: the **clickable body of a sealed
/// shortcut door**.
///
/// A shortcut gate is a solid slab the prefab places, and nothing gave it a body.
/// A `use`/`strike` trigger anchored on it summoned the ordinary point body — one
/// `1.0f x 2.0f` box at the region's first cell — which for the `souls-shortcut`
/// fixture lands at AABB `[4,65,6]..[5,67,7]` inside a slab occupying
/// `[4,65,6]..[6,68,7]`: flush with the block on every face it touches and
/// interior on the rest, so vanilla never finds it strictly nearer than the block
/// and no press from any angle reaches it (see [`SEAL_MARGIN`]).
///
/// The body therefore stands in the **open air in front of the bars**, one cell
/// per doorway cell, on the sealed side only
/// ([`crate::compiler::wrongside::SealedSide::approach_cells`]). That placement is also the
/// entire side mechanism: a near-side ray hits the body before the door, a
/// far-side ray hits the door and stops, because vanilla bounds its entity
/// raycast by the block hit distance. No player test, no DSL surface.
///
/// A click trigger the author anchors on the gate rides these — the same merge
/// `seal_fns` performs for a `close-gate` seal — so the author's own prose and
/// sound, gated by their own flags, are what a wrong-side press produces. The
/// compiler supplies the body; the campaign supplies the answer.
///
/// Empty for a campaign with no shortcut → byte-identical output.
pub(super) fn ws_arm_fns(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (sc, side) in answering_shortcuts(plan) {
        // A click trigger the author anchored on this gate rides these hitboxes
        // rather than summoning its own co-located one — the same merge
        // `seal_fns` performs for a `close-gate` seal, and the reason a trigger
        // at a gate anchor stops being a ray-pick tie.
        let mut tags = vec![format!("dw_ws_{}", sc.safe)];
        tags.extend(seal_rider_tags(plan, chrome, &sc.gate_anchor));
        let tag_list = tags
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(",");
        let body: Vec<String> = side
            .approach_cells()
            .into_iter()
            .map(|c| {
                // Integer hundredths, never f64 arithmetic: the datapack text is
                // part of the byte-identity contract (ADR-0006).
                let x = fmt_centi(c[0] as i64 * 100 + 50);
                let y = fmt_centi(c[1] as i64 * 100 - 1);
                let z = fmt_centi(c[2] as i64 * 100 + 50);
                format!(
                    "summon minecraft:interaction {x} {y} {z} \
                     {{width:{SEAL_BOX_SIZE},height:{SEAL_BOX_SIZE},response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}{tag_list}]}}"
                )
            })
            .collect();
        out.push((format!("ws_arm_{}", sc.safe), lines(&body)));
    }
    out
}

/// Per-tick shortcut unlock detection (spec-0016 §2). Fires **once** — the
/// `#sc_<id>` sentinel is the structural expression of permanence: after the open
/// there is nothing left to fire, and no verb anywhere can put the gate back
/// (`DW0372` forbids `close-gate` on a shortcut gate). Empty without a shortcut.
pub(super) fn shortcut_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        let id = &sc.safe;
        out.push(format!(
            "execute unless score #sc_{id} dw.sys matches 1 if entity @e[tag=dw_sc_{id},nbt={{interaction:{{}}}}] run function {ns}:shortcut_open_{id}"
        ));
        out.push(format!(
            "execute as @e[tag=dw_sc_{id}] run data remove entity @s interaction"
        ));
    }
    out
}

/// The `shortcut_open_<id>` functions (spec-0016 §2): latch the sentinel, clear
/// the gate region to air (the same `fill … replace <block>` an `open-gate`
/// emits), then run the `on_unlock` beat. Server-source-safe — the poll lives on
/// the tick, which has no `@s`.
pub(super) fn emit_shortcut_functions(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        let id = &sc.safe;
        let (from, to) = sc.gate_region;
        let mut body = vec![
            format!("scoreboard players set #sc_{id} dw.sys 1"),
            format!(
                "fill {} {} {} {} {} {} minecraft:air replace {}",
                from[0], from[1], from[2], to[0], to[1], to[2], sc.gate_block
            ),
            // The affordance is spent: the bar is thrown and the door is open,
            // so its hardware retires with it. This is the ONE function allowed
            // to remove it — `DW0421` fails the build if anything else does.
            format!(
                "kill @e[tag={}]",
                crate::compiler::affordance::hardware_tag(&format!("dw_sc_{id}"))
            ),
        ];
        // …and the door's own voice goes with the bars. An opened
        // threshold that still says "this will not open" is a lie, and an
        // invisible box left standing in a now-walkable doorway swallows
        // right-clicks aimed through it — the same retirement `open-gate`
        // performs for a `close-gate` seal.
        body.push(format!("kill @e[tag=dw_ws_{id}]"));
        body.extend(emit_effect_bundle(
            plan,
            &sc.on_unlock,
            root_audience(delvewright_dsl::EffectRootKind::ShortcutUnlock),
        ));
        out.push((format!("shortcut_open_{id}"), lines(&body)));
    }
    out
}
