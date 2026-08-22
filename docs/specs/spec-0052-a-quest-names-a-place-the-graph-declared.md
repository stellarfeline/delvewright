# spec-0052: A quest names a place the graph declared — the campaign's vocabulary below node granularity

- **Status**: Proposed
- **Ground**: spec-0049 §5.2 (the synthesized vocabulary), spec-0050 §6 (the
  owed anchors and the re-binding map), and a measured capability gap: a
  campaign authored under the prefab placement authority references 41
  distinct anchor names at over two hundred sites — respawn points, wave
  seats, lethal volumes, cutscene subjects, NPC stations, loot containers —
  every one declared by its pieces, and not one expressible in a site-plan
  campaign, whose entire anchor vocabulary is the synthesized set: `spawn`,
  one anchor per graph node, and the seam and unlock anchors of its barred
  edges. Moving such a campaign onto the pipeline would flatten every named
  place to a box centre, which un-writes its spatial script. This spec closes
  the gap for both authoring orders at once.
- **DSL**: one `dsl_version` bump, per-stage fenced in the settled shape. The
  number is allocated at implementation dispatch per the standing allocation
  rule; this spec deliberately names none — a version literal written before
  the ledger moves is the recorded staleness shape.
- **Diagnostics**: none allocated here. §7 describes every refusal by what it
  refuses; the implementation round is handed its codes.
- **Non-goals**: any campaign's migration design; several pieces per place or
  a half-derived place (spec-0050 §17, unchanged); a vocabulary for
  whole-owned space (a name on a `volumes[]` massif or on the party fabric —
  excluded until a campaign brief demands it, and the brief is the
  falsifier); authored positions or layout hints for a declared name (§5, and
  the exclusion is the design); retiring `areas[]`; any grammar-program
  surface (pieces already declare anchors; nothing there moves).

## 1. The gap, and the premise it falsifies

The site-plan pipeline was designed for site-plan-first authoring: quests are
written against graph nodes before any piece exists, so a vocabulary of one
anchor per node is complete — there is nowhere finer to point. On that
premise the vocabulary is correct, and nothing here repudiates it.

The inverse order is real: a campaign whose pieces and quests exist first,
whose map is being derived afterwards. Its quest layer names places *inside*
places — the fire pit in the camp, the aft deck of the galley — and those
names are its design. The question is therefore not "the vocabulary is too
small"; it is **what a quest is entitled to name once a piece is bound, and
whether the two authoring orders can share one answer.**

They can, because the answer does not mention pieces at all.

## 2. The decision

**A layout-graph node declares the named places inside it — its `stations[]`
— and those names join the campaign's vocabulary at the same authority as
every synthesized anchor.** A station is a name and a shape (point, region,
or gate), never a position. While the node's box is massed, the derivation
realizes each station at a derived stand-in inside the box; when a piece is
bound, the station joins the owed set and the `detail-plan` `anchors` map —
unchanged in shape — re-binds it to an anchor of the piece, which is where
the name gets its real place.

**A quest names campaign vocabulary and nothing else.** A bound piece's own
anchors never join that vocabulary; the only path from a campaign name to a
piece anchor is the re-binding map, and the only names the map accepts are
still exactly the owed ones.

Both authoring orders now write the same documents: a site-plan-first author
declares stations as the quests need them, before any geometry; a
pieces-first author declares one station per name the quests already use and
binds each to the piece anchor that has always carried it. The spatial script
survives because the names land on the pieces' actual anchors, not on box
centres.

## 3. The vocabulary surface

`nodes[].stations[]` on the layout graph — the document that already owns
the campaign's places, upstream of every consumer:

- **`anchor`** — an anchor id (`anchor/<kebab>`), the name quests reference.
- **`kind`** — `point | region | gate`, mirroring the three shapes a piece
  anchor can take (a cell to stand a body at; a volume; a volume with a
  block that seals and clears). The kind is the station's *shape*, never its
  purpose: there is no enum of bonfire/camera/shop, for the same reason
  `intent` is free-form — a purpose vocabulary would be this month's genre
  wearing a schema's clothes.
- **`note`** (optional) — recorded judgement for the reviewer and the
  detail brief; no check keys on it.

Rules of the namespace, refused at graph validation (§7):

- The derived namespace is the engine's: a station name may not begin
  `anchor/node-`, `anchor/seam-` or `anchor/unlock-`, and may not equal the
  entry anchor's name. Reserving the prefixes (rather than only the names
  currently synthesized) keeps a station from colliding with a node or edge
  added later.
- **The scope of uniqueness is the area** — the scope every anchor
  reference already resolves in, unchanged from the standing rule. A
  site-plan campaign has one area, so the campaign's whole vocabulary
  (synthesized ∪ declared) is unique within it: no two stations anywhere in
  the graph share a name, and no station shadows a synthesized name.
  Piece anchor names stay piece-scoped and never enter this scope, so two
  pieces both declaring `anchor/door` collide with nothing — which is the
  ambiguity the re-binding map exists to make impossible, and the reason
  importing piece vocabularies wholesale is not the design.
- A station belongs to one node. A connection *between* places is the
  edge's to declare, as ever: an interior gate station seals a volume inside
  its own place and can never gate node-to-node traversal — topology stays
  the graph's, structurally.

A station reference is area-scoped like every anchor reference, not
node-scoped: a cutscene in one node may take a station of another as its
subject (a vista onto the bell), exactly as prefab campaigns reference
across an area today. Where a body must reach a station, the existing
per-branch nav proofs and the bot prove it on bytes, as for every anchor.

## 4. What a quest may name, in both orders

At every point in a campaign's life, the answer is the same one sentence:
**the campaign vocabulary — synthesized names plus declared stations — and
never a piece anchor directly.** Binding a piece changes where a name
resolves *to*; it never changes what may be named. Consequences, stated so
an implementer cannot guess:

- The vocabulary is **exact at every stage**. A site-plan campaign has no
  prefab pool and no deferral: `AnchorProviders` keeps contributing one
  known set for the site area, now including stations, so an unknown name
  is refused at validation exactly as today — leniency never enters, and
  the mid-build state is fully checked, not waved through.
- Kind is checked at the reference site from the *declaration*, before any
  piece exists: a region consumer (a lethal volume's region, a fill verb)
  naming a point station is refused at validation, not discovered at bind
  time. When a piece binds, the same kind is demanded of the piece anchor
  (§6), so the two readings cannot drift.
- A station no quest references is legal mid-authoring (the graph-before-
  mission case, unchanged) and is visible: station counts join the stated
  binding lines, and a zero is stated, per the standing rule.

## 5. The massed realization — what a reference means before the piece exists

Detail is partial by construction, so a quest referencing a station of a
still-massed place is the ordinary state, not an edge case. Refusing it
would forbid mid-build; resolving it silently against nothing would be an
unbound proof. The design admits neither, because **a station reference is
never unresolved**: the derivation realizes every station of an unbound box
as a stand-in, exactly as it realizes `anchor/node-…` today, from the same
one authority validation resolves against — so a name that validates cannot
fail to exist in the built world, massed or detailed.

The stand-in: a deterministic pure function of the layout graph, the site
plan and the metrics table — no seed, no parameter. Each station of a box
lands at its own standable cell inside the box (distinct cells, a
documented deterministic order); a region station is realized as a minimal
region at its cell; a gate station as a minimal sealed region of the
derivation's bar block, opened and closed by the existing verbs exactly as
a synthesized seam gate is. The battery and the bot prove the massed world
with the stand-ins in it, honestly: what is proven is the massing, which is
what exists.

**The author cannot state where a stand-in goes.** A station has no
coordinate, no offset, no "near the seam" hint — absent fields, not
optional ones, the same tooth as the detail plan's. The stand-in's geometry
is massing, not design; the design lives in the piece, where the name will
land. **Marked judgement, with its falsifier**: if walk evidence shows a
massed round's verdict turning on stand-in placement — a hazard whose
position, not existence, decides the walk — the stand-in derivation gains
*parameters* per spec-0049 §5.1's rule, never authored coordinates.

Declaring a station edits the layout graph, so it moves the graph's canonical
hash and re-opens the walk gate. That is correct, not merely accepted: the
massed world's contents changed, so the whole that was walked is not the
whole that exists.

## 6. The bound realization — the owed set grows, the map does not change shape

`siteplan::owed_anchors` stays the one authority for what a place owes, and
a node's stations join its owed set beside its `anchor/node-…`, its `spawn`
when it is the entry, and its `anchor/unlock-…` sides. Everything downstream
follows from that one widening, through the chain that already exists:

- The `detail-plan` `anchors` map must bind every owed station to an anchor
  of the piece — the existing owed-name refusals, over a wider set. The map
  still refuses any key that is not owed; nothing about its shape or its
  gate changes.
- The engine, not the author, establishes that the piece really has the
  anchor: the binding's value is checked against the piece's metadata (a
  typo is the existing refusal), the piece itself against the library (the
  existing bare-prefab refusal), and the anchor's shape against the
  station's declared kind — a point station demands an anchor with a cell
  in play space; a region station an anchor with a region inside the
  piece; a gate station a gate anchor with its block. "The piece really
  does have that anchor" is never accepted as an assertion, because a
  campaign with a typo says the same words.
- Resolution places the campaign name at the piece anchor's position inside
  the computed frame, exactly as for the owed names today. Two owed names
  bound to one piece anchor is legal — one spot may carry two roles.
- A swapped or re-exported piece that no longer declares a bound anchor is
  refused at validation naming the vanished anchor — the proof never
  silently drifts to a place that stopped existing. The gate-region
  partition is preserved: seam gate regions remain the only synthesized
  names no place owes; every station is owed by exactly one place.

## 7. The refusals owed

Described by what they refuse; each is falsifiable, and the campaign shape
that trips it is named. No codes are allocated here.

1. **A station in the engine's namespace.** A station named with a reserved
   prefix or the entry anchor's name. Trip: a node declares
   `anchor/seam-vestry-door` — refused even though no such edge exists,
   because the prefix, not the collision, is the rule.
2. **Two claims on one name.** Two stations of one name anywhere in the
   graph (one node or two). Trip: two nodes each declare `anchor/fire-pit`.
   The refusal names both nodes and states the scope — the area.
3. **A reference of the wrong shape.** A quest-side consumer whose site
   demands one kind naming a station of another. Trip: a lethal volume's
   region names a `point` station. Judged from the declaration at
   validation, with zero pieces bound.
4. **An owed station left unbound.** A `details[]` row for a node that omits
   one of its stations from `anchors`. Trip: bind a piece to a node with one
   station and an empty map. (The existing owed-name refusal, over the
   widened set.)
5. **A binding without standing.** A station bound to a name the piece does
   not declare; or to a piece anchor whose shape is not the station's kind;
   or, for a point, to an anchor the piece's contract resolves outside play
   space. Trip: a `point` station bound to the piece's gate region. (The
   existing binding and standing refusals, extended by the kind demand.)
6. **The fence.** A layout graph declaring `stations[]` under a
   `dsl_version` below this surface's is refused by the per-stage fence, as
   for every fenced surface; no document below it moves by a byte.

Not refused, deliberately: a station no quest references (mid-authoring
state; stated as a count), and a beat-node/station-node disagreement (§3 —
cross-node reference is legal and always was).

## 8. Why the answer is not a wider `anchors` map, recorded

The third review shape was applied first: the general mechanism is the pair
of `synthesized_anchors` (the vocabulary authority) and the `detail-plan`
`anchors` map (the re-binding), and the too-narrow binding is in the
**authority** — the graph could name places only at node granularity — not
in the map. Widening the map's accepted keys instead (letting a `details[]`
row mint vocabulary) was tested and rejected on three grounds:

1. It authors the vocabulary at stage 6, downstream of the quests that
   consume it, so a quest could not validate until its places were
   detailed — refusing the ordinary mid-build state, and inverting the
   ordering the pipeline exists to make structural.
2. It splits the one authority: validation would resolve names out of the
   detail document while the derivation synthesizes out of the graph — the
   two-functions-agreeing-about-spelling drift the authority note exists to
   remove.
3. It disarms the typo refusal: a map that accepts new keys cannot tell a
   new name from a misspelled owed one.

What the map's narrowness protects — every key is a name the campaign
already owns, so a binding cannot invent vocabulary and a typo cannot pass
as intent — is preserved exactly: the owed set grows upstream, and the map
still refuses everything outside it. No fourth mechanism is added; the two
existing ones are reached by the object that needed them.

## 9. What the engine deliberately does not know

- **What a station is for.** Kind is shape; purpose is content. A bonfire,
  a camera subject and a shop counter are the same `point` to every check.
- **Where a station is, before a piece is bound.** The author declares
  existence and shape; position is derived — the box's while massed, the
  piece's when bound. There is no surface on which to assert it.
- **The piece's own vocabulary.** The engine never reads names out of a
  bound piece into the campaign; the campaign's names are authored in the
  campaign, and the binding is explicit, per name, refused when wrong.

## 10. The hatch question

Opt-outs this spec creates: **none.** A station is declared or it is not; a
place is bound or it is not; every combination gets the full derivation,
the full owed-set demand and the full battery. There is no acknowledgement,
no exemption list, no author-selected severity. The one soft edge is the
stand-in's geometry (§5), and it is secured by a property the defect cannot
supply: an author wanting a particular massed position has no field to put
it in — the escape from "the stand-in is not where I want it" is binding a
piece, which is the fully-gated path.

## 11. The general-engine test

Could a creator making an entirely different game want this, configured to
their own fiction? Any quest-bearing game names places inside places — the
throne in the hall, the winch on the pier, the reliquary in the crypt — and
wants those names stable while the geometry under them is still being
built. Nothing in the surface is delve-shaped: kinds mirror what an anchor
can be, not what this genre does with one; names are free; counts are
unbounded. Falsifier, carried from the pipeline specs: the first campaign
brief this vocabulary cannot state without a workaround — a name on
whole-owned space is the known candidate — is the evidence, and the answer
is a first-class surface or a refused feature, decided on that brief.

## 12. Version discipline

- One `dsl_version` bump (number allocated at implementation dispatch),
  per-stage fenced: a graph below it cannot carry `stations[]`, and no
  document below it moves by a byte. Released campaigns carry no site plan
  and are untouched; the emission baseline is the instrument.
- Every in-development site-plan campaign adopts per the standing rule.
- No grammar-ledger movement; no prefab-metadata change.

## 13. Gallery, demos, docs

- Every new schema property and enum variant (`stations[]`, its fields, the
  three kinds) becomes a coverage unit on landing (`schema --stage all`
  authority). The site-plan overlay binds them: at least one station of
  each kind, one re-bound to the overlay's generated piece. Committed
  probes bind the refusals in the probe form — each of §7's shapes produces
  exactly the machine refusal the hatch demands.
- The mechanic's demo row: a two-place plan with a declared station walked
  massed, then bound and walked again — the name standing still while the
  place under it changes — queued in `docs/demo-levels.md` by the
  implementation PR.
- `docs/reference/compiler.md` carries the station table and every new DW
  row; the skill workflows carry the declaration step where quests are
  authored — same PRs as the surfaces, per the tooling-sync rule.

## 14. Acceptance criteria

Machine-checkable; each names its verdict's instrument. All are assertions
about the implemented surface, evaluable in-repo when it lands; the one
claim not evaluable in-repo is stated as such where it appears.

1. `delvec schema --stage all` includes `stations[]` with the three kinds;
   `tools/check-gallery-coverage.py` is green with every new unit bound in
   the gallery domain or refusal-proven.
2. A site-plan fixture declaring a point station and a region station, with
   quest references to both and **zero pieces bound**, validates exit 0 and
   builds; two builds are byte-identical; changing the seed changes no
   byte of the stand-ins.
3. Each refusal in §7 has a fixture the compiler refuses with its allocated
   code, a test asserting that code, and — where the gallery is the bearer —
   a committed probe; `tools/check-dw-codes.py` is green in both directions
   with zero new allowlist entries.
4. On the fixture with a piece bound to the station-bearing node: the owed
   line's count includes the stations; the campaign name resolves at the
   piece anchor's position inside the computed frame (asserted on the
   compiled output, not on the map); removing the piece's anchor from its
   metadata turns validation red naming the vanished anchor.
5. The partition proof (`the_owed_anchors_partition_the_synthesized_set`)
   extends over declared stations and stays green: every name is owed by
   exactly one place or is a seam gate region owed by none.
6. Declaring a station moves `layout_graph_sha256` and re-opens the walk
   gate (the existing walk-record refusal fires naming the graph hash);
   editing a station's `note` alone also moves it — the cost §5 states,
   demonstrated rather than implied.
7. A campaign at a `dsl_version` below the allocated one carrying
   `stations[]` is refused by the per-stage fence; the emission baseline
   over every released campaign is byte-identical before and after the
   implementation lands.
8. Every new or widened check states its binding count with its
   denominator; `tools/check-stated-counts.py` and the docs job are green;
   station counts of zero are stated, not omitted.
9. Whether stand-in placement suffices for walk judgement **cannot be
   evaluated in-repo** — it is walk evidence, and §5 names it as the
   falsifier that would add derivation parameters.

## 15. Not settled here

- **Names on whole-owned space** — a station belongs to a node; a name on
  a massif, a party plane or the open sea between clusters waits for the
  campaign brief that demands it (§11).
- **Stand-in placement quality** — deterministic and unauthored now;
  parameters only on the walk evidence §5 reserves them for.
- **Whether `intent`-adjacent tooling reads `note`** — reviewer surface,
  decided by the detail-brief tooling when it exists, never by a check.
