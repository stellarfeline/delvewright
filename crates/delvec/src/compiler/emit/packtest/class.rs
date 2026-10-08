use super::*;

/// v0.6 boundary PackTests (spec-0013): a player outside the region is returned to
/// the last checkpoint; a player inside is never moved. Drives the real
/// `boundary_tick` on a dummy — its direct call IS the 1s clock's body, so no
/// schedule wait is needed (well under the 2s acceptance bound). Uses only
/// `assert score` (PackTest-known-good on the validation server): the player's
/// block-x, captured via `data get … Pos[0]`, discriminates the checkpoint from
/// the interior cell, and is robust to teleport centering (both sides floor the
/// same way). Emits nothing when the campaign declares no `boundary`.
/// **The class trigger is one-shot per player** — the seal, proved on a
/// live server.
///
/// `class_apply_<c>` ends in `teleport @s <campaign entry point>`, so a second
/// `/trigger dw.class` mid-run used to re-class whoever ran it AND warp them
/// back to the start of the delve. The compiler now arms the trigger only for a
/// player who has not classed (`class_arm`), so the seal is a property of the
/// emitted pack rather than a rule every caller has to know.
///
/// The template drives the REAL arming path as its own dummy — it never restates
/// the guard, which would prove only its own copy — and takes the one
/// unambiguous read-back vanilla offers for "was this trigger usable": the
/// success of the `trigger` command itself.
///
/// Three claims, in the order that makes them mean something:
///
/// 1. an UNCLASSED player's trigger is armed and works (the seal must not have
///    weakened the first, legitimate class — a template that only proved the
///    "no" would pass just as well against a pack where classing is broken);
/// 2. the apply consumes the trigger and records the class;
/// 3. after it, the arming path runs again and the trigger stays DEAD: the
///    `trigger` command fails, `dw.class` gets no score, so the dispatch cannot
///    fire — same class, same place, measured on the dummy's own `Pos`.
///
/// The dummy is parked away from the entry point before claim 3 precisely so a
/// warp back to it would be visible.
pub(super) fn emit_class_seal_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(first) = plan.classes.first() else {
        return;
    };
    let Some(entry) = campaign_spawn(plan) else {
        return;
    };
    // A genuinely DIFFERENT class for the second attempt when the campaign has
    // one, so "the class did not change" is a claim about identity and not only
    // about position.
    let second = plan.classes.get(1).unwrap_or(first);
    // Distinct from the entry cell by construction: this is where a warp would
    // be visible. Reading block-x back the way `emit_boundary_packtest` does
    // makes the assertion robust to teleport centering.
    let probe_x = entry[0] + 32;

    let mut b = packtest_header(&format!(
        "{title}: the class trigger is one-shot — a second `/trigger dw.class` cannot re-class or \
         warp"
    ));
    b.push(format!("function {ns}:setup"));
    // Own init: the batch is one shared server, so "never set" is not 0.
    b.push("scoreboard players reset @s dw.class".to_string());
    b.push("scoreboard players reset @s dw.classed".to_string());

    // --- 1. unclassed: the trigger is armed and the class can be taken --------
    b.push(format!("execute as @s run function {ns}:class_arm"));
    b.push(format!(
        "execute store success score #cls_arm1 dw.sys run trigger dw.class set {}",
        first.n
    ));
    b.push("assert score #cls_arm1 dw.sys matches 1".to_string());

    // --- 2. the apply consumes the trigger and records the class -------------
    b.push(format!("function {ns}:class_apply_{}", first.safe));
    b.push(
        "execute store success score #cls_taken dw.sys if score @s dw.classed matches 1"
            .to_string(),
    );
    b.push("assert score #cls_taken dw.sys matches 1".to_string());
    b.push(
        "execute store success score #cls_left dw.sys if score @s dw.class matches -2147483648.."
            .to_string(),
    );
    b.push("assert score #cls_left dw.sys matches 0".to_string());

    // --- 3. the seal: arm again, and the trigger stays dead ------------------
    // Park the dummy away from the entry the apply teleported it to, so the warp
    // this task exists to kill would move it.
    b.push(format!("tp @s {probe_x} {} {}", entry[1], entry[2]));
    b.push("execute store result score #cls_x dw.sys run data get entity @s Pos[0] 1".to_string());
    // Precondition: the park really landed where it was asked to, so a later
    // equality is a fact about the seal and not about a teleport that no-op'd.
    b.push(format!("assert score #cls_x dw.sys matches {probe_x}"));
    b.push(format!("execute as @s run function {ns}:class_arm"));
    b.push(format!(
        "execute store success score #cls_arm2 dw.sys run trigger dw.class set {}",
        second.n
    ));
    b.push("assert score #cls_arm2 dw.sys matches 0".to_string());
    b.push(
        "execute store success score #cls_left2 dw.sys if score @s dw.class matches -2147483648.."
            .to_string(),
    );
    b.push("assert score #cls_left2 dw.sys matches 0".to_string());
    // …so the dispatch cannot fire: same class score, same place.
    b.push(
        "execute store success score #cls_still dw.sys if score @s dw.classed matches 1"
            .to_string(),
    );
    b.push("assert score #cls_still dw.sys matches 1".to_string());
    b.push("execute store result score #cls_x2 dw.sys run data get entity @s Pos[0] 1".to_string());
    b.push(format!("assert score #cls_x2 dw.sys matches {probe_x}"));

    // The class the player actually wears, when the campaign tags it (the flask
    // path): still the first class, never the second.
    if !plan.flasks().is_empty() {
        let worn = class_tag(&first.safe);
        b.push(format!(
            "execute store success score #cls_worn dw.sys if entity @s[tag={worn}]"
        ));
        b.push("assert score #cls_worn dw.sys matches 1".to_string());
        if second.safe != first.safe {
            let other = class_tag(&second.safe);
            b.push(format!(
                "execute store success score #cls_other dw.sys if entity @s[tag={other}]"
            ));
            b.push("assert score #cls_other dw.sys matches 0".to_string());
        }
    }

    // Leave no residue for the shared batch (pin_dummy rule 3/4): the party-unique
    // kit latches this template's apply may have taken are batch-global.
    for (k, item) in plan.campaign.classes.content.classes[0]
        .kit
        .iter()
        .enumerate()
    {
        if matches!(item.carrier, Some(delvewright_dsl::Carrier::One)) {
            b.push(format!(
                "scoreboard players reset #kit_{}_{k} dw.sys",
                first.safe
            ));
        }
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/class_trigger_once.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// Every declared class's own apply body really dresses the player in THAT
/// class.
///
/// `class_apply_<id>` is per-class code: its own kit, its own party-unique
/// latches, its own worn tag. The gallery drove one of two through
/// `class_trigger_once`, whose subject is the one-shot SEAL — a property of the
/// trigger, not of any class — so the second class's kit, tag and entry warp
/// shipped with a compile-time proof and nothing else.
pub(super) fn emit_class_apply_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(entry) = campaign_spawn(plan) else {
        return;
    };
    // `plan.classes` and the authored class list are index-parallel — the same
    // pairing `class_apply_<id>` itself is emitted from.
    for (cl, class) in plan
        .classes
        .iter()
        .zip(plan.campaign.classes.content.classes.iter())
    {
        let safe = &cl.safe;
        let mut b = packtest_header(&format!(
            "{title}: class `{}` applies its own kit, tag and entry warp",
            cl.class_id
        ));
        b.push(format!("function {ns}:setup"));
        // Own init: the batch is one shared server, so "never set" is not 0.
        b.push("scoreboard players reset @s dw.class".to_string());
        b.push("scoreboard players reset @s dw.classed".to_string());
        // Park away from the entry cell, so the warp this class performs is
        // visible rather than assumed. Distinct from the entry by construction.
        b.push(format!("tp @s {} {} {}", entry[0] + 32, entry[1], entry[2]));
        b.push(format!(
            "execute as @s run function {ns}:class_apply_{safe}"
        ));
        b.push(format!(
            "execute store success score #cap_{safe} dw.sys if score @s dw.classed matches 1"
        ));
        b.push(format!("assert score #cap_{safe} dw.sys matches 1"));
        // The class the player WEARS, when the campaign tags it (the flask path).
        if !plan.flasks().is_empty() {
            b.push(format!(
                "execute store success score #capt_{safe} dw.sys if entity @s[tag={}]",
                class_tag(safe)
            ));
            b.push(format!("assert score #capt_{safe} dw.sys matches 1"));
        }
        // …and it landed the player at the campaign entry. Read block-x back the
        // way `emit_boundary_packtest` does, so the assertion is robust to
        // teleport centering.
        b.push(format!(
            "execute store result score #capx_{safe} dw.sys run data get entity @s Pos[0] 1"
        ));
        b.push(format!(
            "assert score #capx_{safe} dw.sys matches {}",
            entry[0]
        ));
        // Leave no residue for the shared batch (pin_dummy rule 3/4): the kit, the
        // worn tag, and the party-unique latches this apply may have taken are all
        // batch-global or player-visible.
        b.push("clear @s".to_string());
        b.push(format!("tag @s remove {}", class_tag(safe)));
        b.push("scoreboard players reset @s dw.classed".to_string());
        for (k, item) in class.kit.iter().enumerate() {
            if matches!(item.carrier, Some(delvewright_dsl::Carrier::One)) {
                b.push(format!("scoreboard players reset #kit_{safe}_{k} dw.sys"));
            }
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/class_apply_{safe}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

pub(super) fn class_apply_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "class-apply",
        families: vec!["class_apply_".to_string()],
        declared: plan.classes.iter().map(|c| c.safe.clone()).collect(),
    }
}
