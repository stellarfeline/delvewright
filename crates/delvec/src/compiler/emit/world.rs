//! World sealing and the campaign spawn.

use super::*;

pub(super) fn sealing_commands(
    time: delvewright_dsl::WorldTime,
    weather: delvewright_dsl::WorldWeather,
    difficulty: Option<delvewright_dsl::WorldDifficulty>,
) -> Vec<String> {
    let mut cmds = vec![
        "gamerule spawn_mobs false".to_string(),
        "gamerule advance_time false".to_string(),
        "gamerule advance_weather false".to_string(),
        "gamerule fire_spread_radius_around_player 0".to_string(),
        "gamerule mob_griefing false".to_string(),
        // Spawn scatter OFF. Vanilla scatters a first join / spawnpoint-less
        // respawn uniformly in a square of this radius around world spawn; in a box
        // garden every scattered cell is solid prefab (or void), so the only
        // correct radius is 0 — the exact anchor the compiler chose. 1.21.11
        // renamed the legacy `spawnRadius` to `respawn_radius` (the legacy spelling
        // is rejected outright); verified against the vendored 1.21.11 command tree
        // (`data/commands-1.21.11.json`, which is what the compiler's own command
        // validator checks every emitted line against).
        "gamerule respawn_radius 0".to_string(),
        // Box-garden death policy: dying must never cost quest items (a dropped
        // trial key despawns in 5 minutes = softlock for a human player).
        "gamerule keep_inventory true".to_string(),
        // The delve's own machinery must not narrate itself. Dialogue options are
        // `trigger`-type objectives (`dw.dlg_<npc>`, `dw.class`), so every option a
        // player picks runs `/trigger` and vanilla answers it in chat — "Triggered
        // [dw.dlg_antiphos]" beside the line the character just said. Command
        // feedback is engine implementation reaching
        // the player, which is what every other rule in this list exists to stop.
        // NOT version-gated: a campaign at any
        // `dsl_version` wants its dialogue to stop announcing its scoreboard.
        // rcon replies to the caller regardless of this rule. The bot tier does NOT
        // drive over rcon — it drives over chat as an opped player — so on that
        // channel this rule silences every success reply it might have read, and
        // only a refusal (always delivered) survives. A harness question the
        // server has to ANSWER therefore goes out as `execute <condition> run
        // tellraw @s`, which reaches its target whatever this rule says; reading
        // `/scoreboard players get` there comes back empty on the success path,
        // measured on the gallery. The creator overlay's log stamp is
        // `log_admin_commands`, a different rule. (The legacy camelCase spelling is
        // rejected outright by 1.21.11 — the compiler's own command validator caught
        // `sendCommandFeedback` here before it could reach a world.)
        "gamerule send_command_feedback false".to_string(),
        format!("time set {}", time.token(time.world_clock())),
    ];
    // Traps (DSL v0.6, spec-0011) exclude TNT as a payload — no gamerule separates
    // explosion *block* damage from *entity* damage, so a TNT trap would deform the
    // sealed jigsaw world and poison every downstream proof. `tnt_explodes false` is
    // the defense-in-depth seal against a stray primed-TNT source (e.g. a dispenser
    // loaded with TNT the schema forbids anyway).
    cmds.push("gamerule tnt_explodes false".to_string());
    // Weather is emitted for the declared state, whatever it is (spec-0010, and
    // spec-0061 which made the declaration mandatory). The rule is unchanged —
    // *emit what the world declares* — and so are the bytes of every campaign
    // that already declared one: what moved is that there is no longer a
    // campaign that declares none.
    cmds.push(format!("weather {}", weather.token()));
    // Declared combat difficulty (v0.6). The shipped
    // `server/server.properties` already carries it, so this line is not what
    // makes the delve image correct — it is what makes the DATAPACK correct
    // wherever else it is loaded (the owner's own test save, a PackTest world
    // whose properties someone edited). `/difficulty` is idempotent — re-running
    // it with the current value is a no-op that merely reports "did not change" —
    // and it is emitted only when the field is declared, so a campaign that
    // declares none is byte-identical.
    if let Some(diff) = difficulty {
        cmds.push(format!("difficulty {}", diff.token()));
    }
    cmds
}

/// The campaign's **entry point**: the cell the party begins the delve on,
/// through [`Plan::campaign_start`] — the one resolver, which is also where the
/// party's first leg begins. This cell is `setworldspawn`, the class-apply
/// teleport, the first-join placement, and the `dw:cp` seed. `None` is a hard
/// build error (`DW0345`).
pub(super) fn campaign_spawn(plan: &Plan) -> Option<[i32; 3]> {
    plan.campaign_start().map(|(_, pos)| pos)
}
