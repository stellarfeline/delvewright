# spec-0041: Instanced claims, and the climb that leaves the piece

- **Status**: Proposed
- **Question**: three of the eight bell-r2 zone programs could not declare a
  spatial contract, and the round that tried recorded one language fact as the
  cause: *a region name is a literal, so a rule that recurses to build N steps
  gives all N steps one name*. This spec re-measured every refusal against the
  current engine before designing anything. The premise is **false for climbs**
  and **true at the wrong granularity for repeated pieces**; what is genuinely
  missing is smaller than the record says, and this spec designs exactly that
  remainder.
- **ADRs**: 0018 §7 (the `Program` version fence every new document surface
  rides), 0020 (the contract is the checker), 0006 (determinism)
- **Specs**: 0036 (extended — the obligations are untouched), 0040 §6.2
  (document-level `include`, whose qualification this widens), 0033 (corpus
  discipline: each new construct owes an example)
- **Non-goals**: any change to the one-floor rule or to any spec-0036
  obligation; qualification of anchors (never — an anchor is the campaign's
  id, `compose` doctrine); a positional index; assembly-side rise measurement
  across a mated seam (connector emission, spec-0036 §2.8 / DW0780); the
  partly-roofed-space envelope gap (§6, a separate finding).

## 1. The measured ground

Every claim was demonstrated on the current binary; the twin fixtures below
become this spec's red/green pairs. "The record" is the bell-r2 production
log's refusal entries for Z0, Z3 and Z7.

1. **A recursion-built interior climb is already declarable, and the record's
   premise is falsified for it.** The recursion's one name is not a defect: a
   climb was never a space. Spec-0036 already houses a run's treads in the
   `stair` edge's transit volume (`via`), which has **no one-floor rule** and
   for which one name unioning every tread is exactly right. Demonstrated
   twice: a minimal two-room fixture (name-as-space reds one-floor verbatim;
   the same name as `via` is green, prefab written), and **the real Z7
   program** — claims added to `ramp/foot`, `ramp/terrace`, `ramp/head_way`,
   nothing else changed: as a space, the recorded refusal byte for byte
   (`standable floor at y 1..7, which is 7 levels`); as a `stair` edge's
   `via`, `contract-well-formed` passes and `contract-edge-proof` proves the
   edge — the real cobbled way walks, foot to head, rise 6 as declared.
2. **A piece used twice is already separable — at document granularity only.**
   One rule chain called twice writes one name from both calls; the honest
   contract then reds reachability on exactly the second call's cells, and
   with both calls reachable it would silently describe a room that is not
   there (the hazard `compose.rs` names). The cure exists: `include` the
   chain as its own document twice under two prefixes — demonstrated green,
   instances fully separated, one classified in play and one out of walk. Its
   costs are the finding: a file boundary per distinction, and a **token
   contract** — the source must classify its own claims to validate, so the
   author writes a standalone contract that is fiction (an entry and exterior
   edge the piece never has alone), which `include` then discards. A
   validation obligation whose satisfying document is known-false is not a
   proof; it is paperwork the mechanism forces.
   Held against the real Z3 program rather than the fixture, "separable" does
   not reproduce as a cure at all: the zone declares `1.4.0` and `include` is
   fenced at `1.5.0`, and composition composes program **files** (spec-0040) —
   the file set is what the map's `include` list names and what its audit
   accounts, so splitting a chain out of a zone changes the composed artifact
   itself. For a zone in production the cure is a restructure plus a version
   move, never a declaration added to the zone; §2.1 is what makes it one.
3. **A climb with an exterior endpoint has no true spelling, and the one
   spelling the surface accepts is a lie that proves nothing.** With a rise, a
   `stair` edge touching `exterior` is refused (no resolved box to measure
   against); without one the document does not parse (`missing field 'rise'`);
   and with `rise: 0` — the only spelling serde and the checker both accept —
   the expansion is **green**: edge-proof skips exterior edges (bound 0), the
   false rise is checked by nothing, and the face contract exports the climb
   as an `up` face because the transit columns touch the sky plane. A green
   with three unread answers. Z0's cut ledge and the Z0→Z1 seam sit behind
   this, not behind the naming fact.
4. **One exterior edge can export many faces, and nothing aims it.**
   `exterior_faces` already honours a declared `via` — the aiming mechanism
   exists — but an opening's cells may not lie inside any space, and an open
   space over ground (Z0's shore: the claimable scope is the void from sand to
   sky) contains every candidate cell. So the edge falls back to the space's
   whole cell set, exports a face on every outer plane the space touches, sky
   included, and `--traversable` then demands a walk between shore and sky.

## 2. Surface (all fenced at document version `1.6.0`)

### 2.1 `qualify` — one subtree's claims under a prefix

```json
{ "op": "qualify", "prefix": "east", "body": { "op": "call", "symbol": "colonnade" } }
```

A wrapper node, like `bind` and for `bind`'s reason: the capability belongs to
the scope, not to the call verb. The prefix frame is **inherited through
calls** exactly as a `bind` frame is: every `claim` expanded under it records
`east/<name>`. Nested qualifiers compose outermost-first. A self-call under no
new `qualify` adds nothing, so a recursion's iterations still share one name —
which §1.1 shows is the wanted behaviour.

This is `include`'s own qualification with its binding widened from the
document boundary to any scope — the mechanism `compose.rs` already states
("left unqualified, a piece included twice would union its two boxes into one
region"), now reaching two calls inside one program, with no file boundary and
no token contract. Under an `include`, a claim literal is already
include-prefixed; the effective name is the expansion's qualifier chain,
outermost first, prepended to that literal.

Rules:

- `prefix` is one kebab segment; empty, `/`-bearing or non-kebab is a
  `Program::validate` refusal.
- **Anchors are untouched**, verbatim `compose` doctrine. Two calls of a
  marking rule still collide on the anchor name and are refused as today; the
  remedy remains `index`/renames, deliberately not this node.
- A `qualify` on a call cycle is refused at `validate` (an unbounded name
  family that no finite contract could classify).
- Reference integrity stays **static and bidirectional**: the effective claim
  names are computed over the call graph (finite, given the cycle refusal);
  a contract region nothing can claim, and a claim whose every effective name
  is unclassified, are refused before expansion as today.

### 2.2 The exterior climb — `stair` and `drop` edges may touch `exterior`

- `rise` on `stair`/`drop` becomes optional **in the document form** (a fenced
  field change). The obligations move to the checker, symmetric with `walk`:
  missing `rise` on an *interior* `stair`/`drop` is refused (as today); a
  *declared* `rise` on **any** exterior edge is refused — tightened from
  "non-zero", which is what closes the `rise: 0` spelling of §1.3. A refusal
  of a spelling shown to produce a vacuous green is a check gained, not
  weakened.
- `via` remains required on `stair` and keeps its transit-volume constraints
  (disjoint from every space; touching every non-exterior endpoint).
- **Edge-proof stops skipping exterior transit edges.** An exterior `stair`
  proves: the transit volume's standable cells and the interior endpoint
  connect (both ways), and the volume has an **exit** — at least one standable
  via cell on the piece's outer layer, on the declared face (§2.3), reached by
  the air outside the piece. An exterior `drop` into a space (the fall entry)
  proves reachability under the fall model from those outer cells to the
  space and refuses the walk back; a `drop` out of the piece proves the fall
  forward from its space into the volume. The gate's binding count includes
  exterior transit edges, so §1.3's `bound 0` green is structurally dead.
- What no piece can prove alone — the level relation across the seam — is
  assembly's measurement over the two mated faces (non-goal here), and the
  face contract now carries what that needs: the exit cells at their true y.

### 2.3 `face` — an exterior edge names its side

Optional field on any exterior edge, one of the six face keywords; refused on
interior edges. The exported face is derived only on that plane. Required on
exterior `stair`/`drop` (new classes — no existing document is constrained).

With it, two well-formed demands, each per §0 of spec-0036 (the demand is a
fact the defect cannot supply):

- **A face must hold what its class claims.** Face cells are filtered by
  class: `walk`/`stair` faces keep only standable cells, `drop`/`vision`
  faces passable ones. The existing empty-face red then fires on a walk aimed
  at the sky — the phantom `up walk`/`up stair` of §1.3–§1.4 cannot be
  exported, while a genuine roof-hatch stair (standable treads on the top
  plane) and a genuine fall entry both remain expressible.
- **An edge whose space touches several outer planes must aim** — `face` or
  `via`; a via-less, face-less exterior edge on such a space is refused as
  ambiguous. A single-plane space needs nothing. This is what turns Z0's
  seam edges from one many-faced export into one face per declared way.

The class filter changes exported face metadata for existing contract-bearing
prefabs (door faces shrink to their standable rows), so the bell adoption
round re-exports Z5/Z6 metadata and re-mates their faces in the same
milestone, named in that round's summary. Geometry emission is untouched at
every version (AC7).

## 3. What is deliberately not changed

- **The one-floor rule**, verbatim. Every design here routes multi-level cells
  into transit volumes, which is where spec-0036 put them.
- **The inside-a-space refusal on non-transit `via`.** The considered
  alternative — letting an exterior opening claim cells inside its own
  endpoint space — was rejected on §0 grounds: a breach to the outside
  supplies "reached by outside air" for free, so the relaxed demand could not
  discriminate a claimed door from a hole. `face` aims without weakening it.
- **Anchor naming**, in every mechanism above.

## 4. Version fence

`1.6.0` — claimed here per the one-number-one-surface ledger rule: the
`qualify` node, optional `rise` on `stair`/`drop`, and `face` on exterior
edges. A `1.5.0` document writing any of them is refused naming the field and
the version; a `1.6.0` document using none expands byte-identically to its
`1.5.0` self. `tools/check-grammar-ir-compat.py` gains all three rows.

## 5. Acceptance criteria

Each states what would make it vacuous.

1. **The climb pair, distilled from Z7**: one program, recursion-built run
   between two rooms; run-as-space red at `contract-well-formed` naming the
   level span; the same name as `stair` `via` green with `contract-edge-proof`
   bound ≥ 1. Vacuous if the red comes from any other gate, or the green's
   edge-proof binds 0.
2. **The instance pair, distilled from Z3**: one rule chain called twice;
   unqualified, reachability red listing exactly the second call's cells;
   under `qualify` with the two families classified apart (one in play, one
   out of walk), green — with the space's cell count and the out-of-walk
   region's computed kind both asserted non-zero. Vacuous if either family
   binds zero cells.
3. **Anchors cross the qualifier unrenamed**: a marking rule called twice
   under two qualifiers still refuses with `AnchorCollision`; one call's mark
   keeps its unqualified exported name. Vacuous if the fixture's rule declares
   no mark.
4. **Qualifier composition and refusals**: nested qualifiers yield
   `outer/inner/name` (asserted on the resolved contract); a bad prefix and a
   `qualify` on a self-call cycle are `validate` refusals named as such;
   reference integrity refusals still fire before expansion in both
   directions over effective names.
5. **The exterior climb, three spellings dead and one alive**: declared
   `rise` on an exterior edge refused **including `rise: 0`** (the §1.3
   spelling, kept as a red fixture); missing `rise` on an interior stair
   still refused; a green exterior `stair` (fenced fixture) with edge-proof
   binding it and proving interior connectivity plus the exit. Teeth twins: a
   broken tread reds the connection; the exit walled off (no standable via
   cell on the declared face reached by outside air) reds the exit clause.
   Vacuous if edge-proof's binding count excludes exterior transit edges.
6. **Aiming**: two exterior walk edges on one open multi-plane space, each
   with a `face`, export exactly one face each on their declared planes; the
   same contract with either `face` removed is refused as ambiguous; a walk
   edge aimed at a plane holding no standable cell reds via the empty-face
   rule. Vacuous if the fixture's space touches only one outer plane.
7. **Fence, both directions**: each new surface in a `1.5.0` document is
   refused naming field and version; a corpus program upgraded to `1.6.0`
   without them is byte-identical across the fence, and double-expand
   determinism extends over qualified region names.
8. **One checker, two doors**: `delvec prefab audit` agrees with `expand` on a
   piece whose metadata carries a `face`-aimed exterior edge and an exterior
   stair — same bytes, same resolved contract, same verdict. Vacuous if the
   admit-side fixture omits the new fields.

## 6. What this spec knowingly does not fix, so nobody rediscovers it

- **The partly-roofed open space** (Z3: 266 flooded-floor cells under decks
  and tower can be neither `open` — own blocks overhead — nor `enclosed` —
  boundaries run into open basins — nor any computed out-of-walk kind).
  A claim-granularity or envelope question, independent of naming; it blocks
  Z3's full contract even with `qualify` landed, and owes its own design
  round.
- **Z7's tower climb** — `contract-edge-proof` refuses it because each flight
  tops out three courses under the next landing: a real unwalkable build,
  found by the checker doing its job. The fix is a zone rule change, content
  work, not surface.
- **Z0's mire lethality, lighting-probe and beat-audit items** in the same
  record: unrelated to the contract surface, already triaged there.

## 7. Order of work

1. IR + `validate` (node, fields, cycle refusal, effective-name integrity);
   fence rows; `expand`'s prefix frame.
2. Checker: rise arms, exterior transit proof, face filter + ambiguity
   refusal; `delvec prefab` metadata halves of the same fields.
3. Docs and corpus in the same PR: `grammar.md` §2d, an `idiom`-adjacent
   corpus program per construct (spec-0033), `prefab-procedure.md` §1.
4. Bell adoption round, same milestone: Z0/Z3/Z7 contracts on the new
   surface; Z5/Z6 face-metadata re-export; the round summary names the Z3
   residual (§6) as open and not to be tested.
