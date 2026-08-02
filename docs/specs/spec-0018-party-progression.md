# spec-0018 — Party-shared progression (co-op division of labor)

- **Status**: Draft (owner ruling 2026-08-02: "多人任务一定是要能分工完成的";
  triggered by the 2026-08-02 audit finding that per-player progression
  state + globally-consumed affordances soft-lock any party that splits
  objectives)
- **Vision**: a delve is played by ONE party of 1–4. Progress is a fact
  about the party, not about a player. Two players completing two
  different objectives in two different rooms must advance everyone —
  It-Takes-Two-style parallel staging is a first-class design.

## The model

**Party state (one holder, shared):** objective completion, quest
activation/completion, story flags, checkpoints (the respawn anchor and
its on_respawn scene state), campaign completion. Any player's completing
action completes the objective FOR THE PARTY; announcements/titles/hints
address all players.

**Per-player state (unchanged):** class + kit, inventory, position,
death/respawn execution (at the party checkpoint), cinematic
attach/restore (already per-player), per-player effect clocks
(night-vision area mitigation).

**Effect multiplicity under party state:** party-fact effects (set-flag,
complete-objective, set-checkpoint, open/close-gate, spawn-*) execute
once; player-facing effects (narrate, title, sound-at-players, give-item,
damage-players) address every party member. give-item on a quest beat
gives to EVERY player unless the item is marked `carrier: one` (quest
props like the wine-skin/stake are one-per-party, delivered to the
completing player — the party can hand them off physically).

## AND-joins (already in the DSL, now actually usable)

`after: [obj/a, obj/b]` on an objective — and quest triggers on
quest-complete — are the AND primitive. With party state they compose
into division of labor: obj/a in room 1 (player A), obj/b in room 2
(player B), successor gated on both. **No new stage-5 syntax.** The
analyzer's producibility model and path replay (spec of the AUDIT-P0
fixes) treat AND-joins as joins over party state.

Simultaneity mechanics (two switches held at the same instant) are OUT of
this spec — a later souls/co-op addition once a vanilla-first primitive
is settled (candidate: pressure-plate trap hardware + a conjunction
window). Nothing here blocks it.

## Validation (two-layer, extended)

1. **Static**: unchanged single-agent completability remains the base
   proof — every campaign must be completable by ONE player (a party of
   one is legal). AND-join branches add a proof that each parallel arm is
   independently reachable from the join's frontier.
2. **Runtime**: a generated **two-dummy PackTest** for every AND-join:
   dummy A completes arm A only, dummy B completes arm B only, assert the
   join objective activates and both dummies see the successor state
   (division-of-labor proven on a real server, batch-model compliant).
   The critical-path bot stays single-bot (valid by the party-of-one
   rule).

## Migration

All emitted `@s`-scored progression scoreboards move to a party holder
(fakeplayer). No DSL surface change; existing campaigns rebuild with
party semantics. The island campaign is the regression canary: its
existing behavior must be preserved for a single player, and the
round-4-class findings ("wine to the whole party") become structural.

## Acceptance

1. Two-dummy division-of-labor PackTests green on a fixture with an
   AND-join (room A + room B → joint successor) and on any island
   AND-joins.
2. A two-player live session (owner + one more account, or two harness
   bots) splits the island's cheese/argument beats and completes.
3. Single-player ladder unchanged and green (honest-ladder version).
4. No per-player progression scoreboard remains in any emitted pack
   (mechanically asserted, like the stealth-sneak removal).

## Non-goals

Simultaneity conditions; per-player divergent story lines; PvP;
drop-in/drop-out mid-delve semantics beyond vanilla defaults.
