# spec-0034 — Declared body traversal

- Status: Accepted
- DSL version: 0.11.0
- Diagnostics: `DW0454` (compiler), `DW0455` (dsl); `DW0141` fence
- Supersedes nothing. Extends the traversal proof of island round 21
  (`DW0452`/`DW0453`, `compiler::traversal`).

## Problem

`Traversal::of_entity` derives a `Locomotion` from the entity id alone: `Climber`
for spider/cave_spider, `Flier` for a closed cited list, `Aquatic` from vanilla's
own tag, and `Ground` — the CHECKED class — for everything else, unrecognised ids
included. That default is correct and is unchanged by this spec.

What was missing is the **author's** side. Spiders really do climb, so the rules
cannot be absolute; but an author who wants a body that moves unlike its species —
a sheep that climbs — had no way to say so. The exception could only happen by
accident and merely render.

## Decision

The DSL exposes a **proactive declaration** of a body's locomotion, and the engine
holds the author to it. It does not forbid, and it does not merely warn.

### Where the capability lives

`traversal { locomotion }` — one shared type, `BodyTraversal` — on **every object
class in the DSL that has a body, a position and a compiler-emitted route**:

| Object class | Carries it | Why |
|---|---|---|
| stage-2 `npcs[]` | yes | walked by `move-npc` |
| stage-5 `actors[]` | yes | walked by `move-actor` |
| stage-5 `waves[].mobs[]` | **no** | driven by native vanilla AI, never by a compiler-emitted route. The compiler makes no claim about the moves a wave mob makes (a lane's proof is about lane *geometry*), so a locomotion declaration on it could change no verdict and would be inert by construction. It becomes a consumer the day the lane proof reasons about how its bodies move — and it joins through this same type, never through a field of its own. |

Traversal is a property of the body, not of the verb that first needed it
(CLAUDE.md). The consumers are enumerated once, as a closed sum type
(`dsl::BodyRef` / `dsl::body_traversal_sites`), so a third body class is a compile
error at every consumer until each one says what it does with it.

### What "proven" means, per value

A declaration changes **which rules examine the body**. It never changes how a
route is computed: every body is routed on ground rules, which was already true
of a derived class (a ghast actor walks), so the declaration inherits a
documented property rather than introducing new folklore.

| Value | What it changes | How it is proven |
|---|---|---|
| `climber` | exempt from `DW0453` (going over a barrier line is what a climber does) | the body's route must actually contain such a crossing, else `DW0454` |
| `flier` | same exemption, same reason (a flier makes no ground step-up) | same |
| `ground` | **binds** a derived climber/flier back to `DW0453` — the tightening direction | the route must contain a crossing the derived class would have been excused, else `DW0454` |
| `aquatic` | nothing — it carries no exemption and governs no rule | **refused at declaration time (`DW0455`)**, with the gap named: routing has one reachability model (standable ground) and flooded cells are impassable for every body, so nothing could hold a body to the claim. When routing grows a water model, this refusal is what is deleted to enable the value. |

`opens_gates` is deliberately **not** authorable. Passing a closed fence gate is a
right-click; a scripted walk is a `tp` polyline whose puppet performs no
interaction, and no runtime verb changes a fence gate's state. Declaring it would
not make it true, so the error tier has no authorable exemption at all.

### Why a declaration must be exercised

A declaration that only silences a diagnostic converts a check into an opt-out —
the opposite of the ruling. So the compiler computes the findings each declared
body's legs earn under the **declared** class and under the **derived** one, and
the declaration is *exercised* only where the two differ. An inert declaration is
`DW0454`, error tier. The test is written as a difference of verdicts rather than
as "is it a climber", so a second locomotion-governed rule joins it by existing.

Three inert shapes, each named in the message because the fix differs: the
declared class is the one the species already had; the body walks no leg at all;
or every leg earns identical verdicts either way.

The obligation binds **only where a declaration was authored**. A real spider that
walks a flat corridor is a fact about the species, not a claim, and owes nothing.

The proof cannot be dodged by a campaign that assembles no world: `DW0454` runs
inside `emit`'s `assembles_world(plan)` arm, and `assembles_world` is true
whenever the campaign has any NPC or actor (`clearance::has_bodies`) — which is
exactly the condition under which a declaration can exist at all.

## Version and fences

`dsl_version` 0.11.0, additive. The fence is **per stage**: the NPC declaration is
gated on the `npcs` document's own `dsl_version`, the actor declaration on the
`quests` document's, so a campaign may adopt the surface one stage at a time.
Declaring below 0.11.0 is `DW0141`. There is no requirement half — nothing obliges
a body to declare anything, and a campaign that declares none emits byte-for-byte
what it emitted before.

**Adoption**: no active campaign declares traversal, so no adoption round is owed
by this version. `nobodys-cave-island` (the one active campaign) stays on its
current per-stage versions and its shipped datapack is byte-identical.

## Acceptance criteria

1. `Npc` and `Actor` each carry `traversal: Option<BodyTraversal>`, of one shared
   type, serialized as `{"locomotion": "<token>"}` with `deny_unknown_fields` and a
   closed enum.
2. `dsl::body_traversal_sites` is the only enumeration of declaration sites, and
   `BodyRef` is a closed sum type matched exhaustively by every consumer.
3. Declaring `traversal` on a body whose declaring stage is below `dsl_version`
   0.11.0 raises `DW0141` **in that stage only**; the sibling stage at 0.11.0
   raises nothing.
4. Declaring `locomotion: aquatic` on either body class raises `DW0455`, and the
   message names the gap (`standable ground`).
5. A body declaring `climber` whose route crosses a barrier line over a full-cube
   course raises no `DW0453`; the same body and the same wall with the declaration
   removed does raise it.
6. A body declaring `climber` whose route crosses no barrier line fails the build
   with `DW0454`; so does one declaring the class its entity id already implies,
   and one that walks no leg at all. The three cases are distinguishable from the
   message.
7. A body declaring `ground` whose entity id derives `Climber`, on a route that
   crosses a barrier line, raises `DW0453` — and the same body without the
   declaration does not.
8. A body declaring any locomotion whose route enters a closed fence-gate cell
   still fails with `DW0452`.
9. `validation/traversal-gate.json` carries a `declared` block stating `bodies`,
   `by_class` (one row per `Locomotion`, always), `exercised` and
   `advisories_waived`; a campaign that declares none reports zeros rather than
   omitting the block.
10. Every criterion above is asserted by a test naming its DW code, and both
    `tools/check-dw-codes.py` and `tools/check-reference-versions.py` are green.
11. A campaign that declares no `traversal` builds byte-identically to the same
    campaign built by the pre-0.11 engine, in every language it declares — apart
    from the engine's own `dsl_version` stamp (`creator-datapack/layout.json`),
    the new `declared` block in the traversal ledger, and the hash manifest over
    both. The shipped `datapack/` tree is unchanged.
