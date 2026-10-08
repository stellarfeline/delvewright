//! The playable-region boundary and night vision (spec-0013).

use super::*;

/// The effective "unbounded up" ceiling for a playable region. Well above the
/// 1.21.11 build limit (y=319); no reachable adventure-mode position in a box
/// garden exceeds it, so the vertical selector is unbounded in practice.
pub(super) const REGION_CEIL_Y: i32 = 1024;

/// The compiler's default boundary return message (English-first, CLAUDE.md
/// language policy). Overridable via `boundary.message`, which is then l10n
/// inventoried under `world.boundary.message`.
const _: &str = delvewright_dsl::chrome::BOUNDARY_MESSAGE.en;

/// A soft, non-alarming cue played on a boundary return.
pub(super) const BOUNDARY_SOUND: &str = "minecraft:block.amethyst_block.chime";

/// The derived playable region (spec-0013): the union of every placed-piece AABB,
/// inflated horizontally by `boundary.margin`, floored at the lowest placed block
/// − 8, unbounded upward (capped at [`REGION_CEIL_Y`] for the selector). Every
/// bound is derived from the final layout, so "every anchor is inside" is
/// structural.
pub(super) struct PlayableRegion {
    /// Inclusive min corner `[x, y_floor, z]`.
    pub(super) min: [i32; 3],
    /// Max corner `[x, REGION_CEIL_Y, z]`.
    pub(super) max: [i32; 3],
}

impl PlayableRegion {
    /// The `@s[…]` volume-selector fragment matching a player INSIDE the region.
    /// Biased inclusive (dx/dz span the far block fully) so an edge-standing player
    /// is never falsely ejected — the safe direction, further buffered by `margin`.
    fn inside_selector(&self) -> String {
        format!(
            "[x={},dx={},y={},dy={},z={},dz={}]",
            self.min[0],
            self.max[0] - self.min[0] + 1,
            self.min[1],
            self.max[1] - self.min[1],
            self.min[2],
            self.max[2] - self.min[2] + 1,
        )
    }

    /// The SNBT compound written to `dw:region bounds` — the readable region
    /// contract (mirrors `dw:cp`'s readable last-checkpoint contract).
    pub(super) fn bounds_snbt(&self) -> String {
        format!(
            "{{min:[{},{},{}],max:[{},{},{}]}}",
            self.min[0], self.min[1], self.min[2], self.max[0], self.max[1], self.max[2]
        )
    }
}

/// Whether the declared boundary returns a player who leaves it (spec-0092 §10):
/// `boundary.returns`, default `true`; `false` when no boundary is declared.
pub(super) fn boundary_returns(plan: &Plan) -> bool {
    plan.campaign
        .world
        .content
        .boundary
        .as_ref()
        .is_some_and(|b| b.returns)
}

/// The playable region's inclusive corners, for a proof outside this module —
/// `None` when no `boundary` is declared.
pub fn playable_region_box(plan: &Plan) -> Option<([i32; 3], [i32; 3])> {
    playable_region(plan).map(|r| (r.min, r.max))
}

/// Derive the playable region, or `None` when no `boundary` is declared (the whole
/// feature is then off and output stays byte-identical).
pub(super) fn playable_region(plan: &Plan) -> Option<PlayableRegion> {
    let b = plan.campaign.world.content.boundary.as_ref()?;
    let margin = i32::from(b.margin);
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];
    for area in &plan.areas {
        let (amin, amax) = area.bounds();
        for a in 0..3 {
            min[a] = min[a].min(amin[a]);
            max[a] = max[a].max(amax[a]);
        }
    }
    // A validated campaign always has >=1 placed area; guard defensively.
    if min[0] == i32::MAX {
        return None;
    }
    Some(PlayableRegion {
        min: [min[0] - margin, min[1] - 8, min[2] - margin],
        max: [max[0] + margin, REGION_CEIL_Y, max[2] + margin],
    })
}

/// The effective boundary return message (authored or the English default).
pub(super) fn boundary_message(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> String {
    match plan
        .campaign
        .world
        .content
        .boundary
        .as_ref()
        .and_then(|b| b.message.as_deref())
    {
        // Authored: an ordinary inventoried campaign string.
        Some(m) => m.to_string(),
        // Unauthored: the compiler's own line, which is chrome and ships
        // translated with the compiler (spec-0029 addendum).
        None => chrome.get(delvewright_dsl::chrome::BOUNDARY_MESSAGE),
    }
}

/// Re-application period of the night-vision clock, in ticks (1 s).
pub(super) const NIGHT_VISION_PERIOD_TICKS: u32 = 20;

/// Duration handed to each `effect give`, in **seconds**. Must stay comfortably
/// above vanilla's 10 s night-vision wind-down: `GameRenderer` ramps the night-
/// vision brightness down (the flicker) once the remaining duration drops to
/// 200 ticks, so with a 1 s clock the remaining duration never falls below
/// `12 s − 1 s = 11 s` (220 ticks) and the effect never blinks. A player who walks
/// out of a mitigated area keeps it for at most this long — deliberate: shortening
/// it below ~11 s would re-introduce the flicker, and no vanilla primitive removes
/// an effect on a region exit without also stripping effects the campaign granted
/// for other reasons.
pub(super) const NIGHT_VISION_SECONDS: u32 = 12;

/// Vanilla's night-vision wind-down, in **seconds**. `GameRenderer` ramps the
/// brightness down once the remaining duration drops below 200 ticks, so an
/// effect that has less than this left is *already* visibly flickering even
/// though it has not expired. Read from the one sight table
/// ([`delvewright_dsl::perception::SIGHT`]), which `DW0944` reads too.
pub(super) fn night_vision_flicker_seconds() -> u32 {
    delvewright_dsl::perception::sight_wind_down_ticks("minecraft:night_vision")
        .expect("night vision is a sight effect")
        .div_ceil(20)
}

/// The lease every `effect give` hands out, in seconds.
///
/// **The camera-coverage guarantee**: a vision
/// effect the compiler grants must outlast any authored camera it can overlap,
/// with vanilla's flicker window to spare.
///
/// The mitigation is declared per area and re-applied by a 1 s clock to the
/// players *inside that area's box*. A player who leaves the box keeps whatever
/// is left of their lease — and the island's ending does exactly that: boarding
/// transports the party from the mitigated island to `area/open-sea` at x=256
/// and immediately plays a 15-second cutscene. They arrived holding at most 12 s,
/// so the ramp began ~1.5 s in and the effect died mid-shot. Owner playtest:
/// "the night-vision effect expires mid-ending-cutscene and flickers."
///
/// **Why the lease, and not a re-grant at the cutscene.** Re-applying the effect
/// from the cutscene driver would light up *every* player in *every* cutscene,
/// including ones who were never granted sight and cameras the author framed as
/// bright — a spectator on a night ocean would be handed cave vision. Vanilla has
/// no "extend only if present" primitive to do it selectively. Lengthening the
/// lease changes **who** has the effect not at all; it only makes the lease a
/// leaving player already holds long enough that no camera can outlive it.
///
/// **Why the campaign's longest camera.** The compiler cannot know which cutscene
/// a player who steps out of a mitigated area will land in, so the only sound
/// bound is the longest one the campaign authors. Sized to that plus the flicker
/// window plus one clock period, so the remaining duration is still above the
/// ramp threshold when the last shot ends.
///
/// The cost is stated rather than hidden: sight trails a player out of a
/// mitigated area for this long. That is the deliberate trade the pre-existing
/// 12 s already made for the same reason (no vanilla primitive strips an effect
/// on region exit without also stripping effects the story granted); this only
/// moves the number, and only for a campaign that authors a longer camera than
/// the floor.
pub(super) fn night_vision_seconds(plan: &Plan) -> u32 {
    // Measured from the ticks the camera driver really runs for
    // (`camera::shot_ticks` resolves `shot_style` defaults and applies vanilla's
    // per-shot clamp), so the bound is the emitted reality, not the authored
    // intent. Rounded up to whole seconds, which is the unit `effect give` takes.
    let longest_camera_ticks: i32 = all_campaign_effects(plan.campaign)
        .into_iter()
        .filter_map(|e| e.cutscene_shots())
        .map(|shots| {
            shots
                .iter()
                .map(|s| crate::compiler::camera::shot_ticks(s.resolved_seconds()))
                .sum::<i32>()
        })
        .max()
        .unwrap_or(0);
    let longest_camera = (longest_camera_ticks.max(0) as u32).div_ceil(20);
    NIGHT_VISION_SECONDS.max(
        longest_camera + night_vision_flicker_seconds() + NIGHT_VISION_PERIOD_TICKS.div_ceil(20),
    )
}

/// The v0.6 night-vision mitigation clock: for every area declaring
/// `mitigation: "night-vision"`, a self-rescheduling 1 s function that gives
/// `minecraft:night_vision` to the players inside **that area's placed bounds**.
///
/// This is the mechanism the `DW0210` gate now keys on (`light::area_night_vision`).
/// Before v0.6 the gate keyed on a class-kit item's display *name*, which a renamed
/// water bottle satisfied — the check passed while nothing granted night vision
/// (owner, island QA). Declaration and emission are now the same fact.
///
/// The selector box is the area's final placed bounds — compile-time literals, no
/// runtime search — so emission is deterministic. Empty for a campaign that declares
/// no mitigation, keeping pre-0.6 output byte-identical.
pub(super) fn night_vision_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let seconds = night_vision_seconds(plan);
    let mut gives: Vec<String> = Vec::new();
    for area in &plan.areas {
        let declared = plan
            .campaign
            .world
            .content
            .areas
            .iter()
            .find(|a| a.id.as_str() == area.area_id)
            .is_some_and(crate::compiler::light::area_night_vision);
        if !declared {
            continue;
        }
        let (min, max) = area.bounds();
        // An area's `bounds()` are inclusive corners, so the selector span is
        // `max - min + 1` — a placed area is a count of cells, where a declared
        // volume (`box_selector_args`) is a span between two corners. The two are
        // one block apart on purpose and this is the only place the difference
        // lives.
        let span = [
            max[0] - min[0] + 1,
            max[1] - min[1] + 1,
            max[2] - min[2] + 1,
        ];
        let sel = format!(
            "@a[{}]",
            box_selector_args(min, [min[0] + span[0], min[1] + span[1], min[2] + span[2]])
        );
        gives.push(effect_give_command(
            &sel,
            "minecraft:night_vision",
            seconds,
            0,
            true,
        ));
    }
    if gives.is_empty() {
        return Vec::new();
    }
    // `schedule … <n>t` uses vanilla replace-mode, so the clock can never double up.
    gives.push(format!(
        "schedule function {ns}:night_vision_tick {NIGHT_VISION_PERIOD_TICKS}t"
    ));
    vec![("night_vision_tick".to_string(), lines(&gives))]
}

/// Whether the campaign declares the night-vision mitigation on any area.
pub(super) fn has_night_vision_areas(plan: &Plan) -> bool {
    plan.campaign
        .world
        .content
        .areas
        .iter()
        .any(crate::compiler::light::area_night_vision)
}

/// The v0.6 boundary clock (spec-0013): a self-rescheduling 1s (20t) region check
/// plus a per-player macro return. Empty for a campaign with no `boundary`. The
/// return teleports via `dw:cp` (the last checkpoint), so wanderers always land on
/// the current respawn anchor rather than a fixed point.
pub(super) fn boundary_fns(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, String)> {
    let Some(region) = playable_region(plan).filter(|_| boundary_returns(plan)) else {
        return Vec::new();
    };
    let ns = &plan.namespace;
    let sel = region.inside_selector();
    let msg = tr(&boundary_message(plan, chrome));

    // boundary_tick: snapshot the live checkpoint into a scratch compound, eject
    // every player outside the region to it, re-arm the clock. `schedule … 20t`
    // uses vanilla replace-mode, so the clock can never double up.
    let tick = vec![
        "data modify storage dw:region cp.x set from storage dw:cp pos[0]".to_string(),
        "data modify storage dw:region cp.y set from storage dw:cp pos[1]".to_string(),
        "data modify storage dw:region cp.z set from storage dw:cp pos[2]".to_string(),
        format!(
            "execute as @a[tag=!dw_cutscene,tag=!{free}] unless entity @s{sel} run function {ns}:boundary_return with storage dw:region cp",
            free = crate::compiler::creator::FREE_TAG,
        ),
        format!("schedule function {ns}:boundary_tick 20t"),
    ];

    // boundary_return: a macro run per offending player (`@s`). Teleport to the
    // checkpoint, show the message on the actionbar, play a soft cue. No damage.
    let ret = vec![
        "$tp @s $(x) $(y) $(z)".to_string(),
        format!("title @s actionbar {msg}"),
        format!("playsound {BOUNDARY_SOUND} player @s ~ ~ ~ 0.6 1"),
    ];

    vec![
        ("boundary_tick".to_string(), lines(&tick)),
        ("boundary_return".to_string(), lines(&ret)),
    ]
}
