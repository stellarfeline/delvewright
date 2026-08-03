# spec-0020: The NPC scene ledger — declared presence, checked against staging

- **Status**: Draft (owner directive 2026-08-03: every story node must declare
  where each NPC is, what they are doing, and what their right-click offers —
  unless explicitly dead/removed — and both the skill prompt and the compiler
  must enforce it. Motivating defects, island round 8: two crew NPCs stood
  forgotten in the stealth alcoves while the player escaped the cave; the
  sleeping giant still offered his awake dialogue tree)
- **ADRs**: 0001 (schema-enforced DSL), 0005 (static proofs)
- **Builds on**: `compiler::continuity` (NPC lifecycle history), `flow.rs`
  (DW0195 presence corner — task #81, absorbed here), per-option dialogue
  flag gates (spec-0008)

An unaccounted NPC is not a style problem: it is the compiler holding a
provable effect history (spawn/move/despawn per NPC, beat by beat) that nobody
compares against the story's intent, because the intent is never written down.
This spec makes the intent a declared artifact and the comparison a build
proof.

## 1. DSL surface — the `cast` block (stage 5, per quest)

Every quest declares a `cast` map covering **every** stage-2 NPC that has ever
been spawned and not explicitly removed:

```json
"cast": {
  "npc/eurylochus": { "at": "anchor/pen", "doing": "crouched among the rams,
                       eyes on the gap", "dialogue": "eurylochus_pen" },
  "npc/polyphemus": { "at": "anchor/fire-side", "doing": "wine-drowned sleep",
                      "dialogue": "none" },
  "npc/antiphos":   "dead"
}
```

- `at`: an anchor, or `"offstage"` (explicitly not in the world — must match a
  despawn), or `"dead"` (shorthand object form allowed).
- `doing`: free prose. Not machine-checked; it is the LLM's forcing function —
  you cannot fill it without deciding the character's business, and it feeds
  the dialogue stage as context.
- `dialogue`: what right-click offers during this quest — one of:
  - a dialogue root id (the full branching tree);
  - `{"barks": ["…", "…"]}` — a **bark pool** (owner addition, 2026-08-03):
    right-click yields one inconsequential in-character line, no tree, no
    consequences. The sleeping giant murmurs in his sleep; a town's
    background NPCs make small talk instead of standing mute. Lines cycle
    deterministically (per-NPC counter — no unseeded RNG anywhere near the
    compiler; runtime cycling is vanilla scoreboard arithmetic). Bark strings
    are player-visible: they enter the l10n inventory like narrate text.
  - `"none"` — genuinely no reaction. Legal but the skill treats it as a
    last resort: if a body is clickable, the world should answer; prefer a
    bark.

## 2. Compiler proofs (build tier)

1. **Completeness**: every live NPC appears in every quest's `cast`; a missing
   entry is an error naming the NPC and the quest ("unaccounted — say where
   they are or remove them").
2. **Placement consistency**: the declared `at` must equal the position the
   effect history actually produces by the time the quest is active —
   `continuity`'s per-NPC ledger already computes this. Declared `anchor/pen`
   while the history leaves the NPC at `anchor/alcove-4` is an error citing
   both cells and the missing `move-npc`. This is the check that catches
   "the crew stood forgotten in the alcoves" at compile time.
3. **Dialogue gating**: the declaration IS the gate. A declared root must be
   the one the emitted gating exposes for that quest's duration; `barks`
   emits the bark cycler in place of the tree; `none` emits suppression —
   all flag-gated with the same machinery per-option gates use today. The
   sleeping giant's awake tree becomes unreachable *because the cast says
   his right-click is a sleep-murmur bark*, not because an author remembered
   a flag.
4. **Branch honesty**: where the history is branch-dependent (different
   dialogue outcomes stage different worlds), the declaration must hold on
   every reachable branch, or the quest declares per-branch casts. No
   optimistic merging (`continuity`'s stance).

## 3. Skill enforcement

`/new-delve` stage 5 brief: the dev subagent must produce the `cast` block for
every quest *before* writing objectives — position first, story second. Stage 6
receives each NPC's `doing` prose as dialogue-writing context. The skill's
schema-first loop picks the requirement up automatically once the schema
carries `cast` as required.

## 4. Migration

Existing campaigns fail proof 1 loudly on rebuild (missing `cast`). The two
live campaigns (nobodys-cave-island, the-drowned-bell) are updated by hand as
the fixture proof; hello-world grows the minimal block. `dsl_version` bumps to
0.7.0; pre-0.7 documents without `cast` keep building with a **warning** for
one version window (the deprecation lever), then the requirement hardens.

## Amendment (owner, 2026-08-03)

Two additions, landed with the implementation.

**1. `dialogue: "unchanged"`.** A fourth `dialogue` form: the keyword
`"unchanged"` resolves to whatever this NPC's dialogue was at its previous
appearance in the node ordering. Carrying dialogue forward must be a *conscious,
declared act* — never an implicit default (an omitted `dialogue` stays an error)
and never a silently repeated root id that then drifts out of sync with the tree.
It resolves transitively (`unchanged` → `unchanged` → a root) and the resolved
value must exist; used at an NPC's **first** appearance it is an error, because
there is nothing to carry. Emission is a no-op — no root swap is emitted for that
NPC at that node — which is what keeps the sugar cheap and byte-stable.

**2. Whole-campaign staleness lint (warning tier).** If an NPC appears in the
cast of 2+ nodes and its dialogue never changes across *all* its appearances —
the same root throughout, however spelled — the compiler warns: plot-relevant
NPCs should evolve with the story; a genuinely background character should carry
a bark pool instead. Warning, not error: a static minor NPC is legal, the author
just has to see the flag. Bark pools are exempt, since a bark pool never claims
to advance anything.

## Acceptance criteria

- [ ] Schema: `cast` required at 0.7.0, each entry `{at, doing, dialogue}` or
      `"dead"`; schema export (`delvec schema --stage 5`) carries it.
- [ ] Proof 1 fixture: a quest omitting one live NPC fails, naming NPC + quest.
- [ ] Proof 2 fixture: declared `at` contradicting the effect-history position
      fails, citing both cells; the fixed declaration passes.
- [ ] Proof 3 fixtures: `dialogue: none` provably suppresses right-click
      advancement (PackTest: interaction record written, no dialog opens,
      record consumed safely); a declared root stays reachable; a `barks`
      pool cycles deterministically through its lines on repeated
      right-clicks (PackTest), and bark strings appear in the l10n
      inventory.
- [ ] Proof 4 fixture: a branch-divergent position with a single flat
      declaration fails; per-branch casts pass.
- [ ] Island + drowned-bell rebuilt green with real cast blocks; the island's
      round-8 "crew forgotten in alcoves" state, replayed against the new
      compiler, is RED (regression-proof of the motivating defect).
- [ ] Pre-0.7 campaigns: warning, not error; documented in compiler.md.
- [ ] Amendment: `"unchanged"` at a first appearance fails; a second-appearance
      `"unchanged"` resolves to the carried scene and emits no new artifact; an
      NPC whose dialogue never changes across 2+ appearances warns (including
      when the repetition is spelled `"unchanged"`), and a repeated bark pool
      does not.
