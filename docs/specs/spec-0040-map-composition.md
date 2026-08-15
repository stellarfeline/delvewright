# spec-0040: Map composition — how a whole map gets its appearance

- **Status**: Accepted
- **Question**: a campaign's zones are designed separately, each judged against
  its own reference and produced as its own prefab. The assembly of those parts
  is the whitebox step. What comes after — how the assembled whole acquires an
  appearance that reads as one place rather than as N buildings touching — is
  answered here.
- **ADRs**: 0004 (extended one scale up, not superseded), 0006 (determinism),
  0015 (composition first), 0018 §7 (the `Program` version fence every new
  document surface rides), 0020 (the spatial contract is the map-level checker)
- **Specs**: 0027 (the back end), 0036 (the obligations this spec binds at map
  scale), 0037 (complementary within-program alignment), 0026 (surrounds — the
  boundary this spec stops at), 0010 (relight, unchanged), 0028 (reference
  images, rank-only)
- **Non-goals**: jigsaw connector emission; multi-prefab layout solving for
  grammar pieces; overlay, positional index, parameterised cross-program
  `call` beyond document-level include (§6.2); runtime water-level mechanics
  (campaign DSL, spec-0031); craft/palette-budget gates (spec-0027 §4, still
  unbuilt); terrain noise and macro-terrain blending (spec-0026 non-goals);
  changing the draw stream's addressing (§8, open).

## 1. The measured ground

Every claim below about what the toolchain does today was established by
running it or reading the shipped behaviour record, not recalled:

1. **Composition is already the working method one scale down.** A zone is a
   grammar program that `include`s vocabulary programs, hands each a box by
   `split`, arguments by `bind`, a frame by `reorient`, and writes itself the
   one thing no included piece can know: the mass around them — margin strips,
   plinths, the gulf (`grammar.md` §5c). Eight zones are built this way, with
   composition-level gates that catch what no piece-level gate can.
2. **One program can call another with a region and parameters — inside one
   document.** `compose::include` copies rules, params and palette roles under
   a prefix, rewriting every reference; `bind` passes arguments through any
   call depth; an anchor rename is explicit per stem. The seam promise is
   pinned by test: an included program expanded over the same box is
   byte-identical to the program alone (item 4 bounds what that covers inside
   a larger derivation). What does **not** exist is a document
   or CLI surface for include: it is a Rust API, so composition today happens
   only in engine source. A creator authoring JSON cannot compose two program
   files (§6.2).
3. **Cost is not a constraint.** Measured on this tree: a 256×48×256 region —
   3.1 M cells, 2.8 M filled, per-cell weighted draws (the worst case), full
   gates, always-on reachability, 36-tile export — expands in 3.6 s.
   Cross-check by an independent run: trial-0002's 32×28×90 zone (81 k cells)
   expands in 60 ms. Expansion is linear and a whole map is seconds.
4. **A composed part does not keep its standalone texture bytes.** The seeded
   stream is one sequential splitmix64 consumed in traversal order
   (`crates/grammar/src/rng.rs`); verified by probe: two programs identical
   except that an *earlier sibling* draws from the stream produce different
   bytes inside the same called piece. Geometry from mutually exclusive guards
   is unaffected (a single candidate never draws); weighted mixes re-texture.
   Consequence in §5 and §8.
5. **The compiler cannot yet place a composed map.** A whole map exceeds the
   48-per-axis template cap on any axis, so it exports as a tile set — and
   `delvec` refuses tile-set metadata by design (`DW0346` names the queued
   work). Confirmed in `compiler::registry` as well as in the diagnostic text.
   Every model in §2 is behind this one capability (§6.1).
6. **Areas do not touch.** An area's origin is `[i·256, base_y, 0]`
   (`compiler.md` stage 1), and a grammar prefab enters an area by direct
   placement only (no connectors). A contiguous whole is therefore one area
   binding one prefab; there is no path today by which separately placed zone
   prefabs become one silhouette.

## 2. The decision: the map is a program

**A whole map is authored the way a zone already is: as a grammar program one
scale up.** The map program `include`s the campaign's zone programs, allocates
each its box by `split`, binds the whole-map datums and materials into them by
`bind`, and writes itself what no zone can know — the massif the zones are
carved from, the connective pieces between them, the curtain, roofscape and
ground that carry the silhouette. It expands as one derivation, is judged by
the same gates, exports as one (tiled) prefab, and is placed as the campaign's
area. Composition is authored in the same language as the parts, checked by
the same checker, and reviewed through the same render loop.

This is model (B) of the design discussion, and it is chosen for a structural
reason, not preference: **mutual consistency needs one medium.** A part and
the whole can only owe each other things that are statable where both can
read them — a box, a param, a palette role, a contract edge. The grammar is
the one medium both already occupy.

**Why not a whole-map shell (model A).** A shell is a second design object in
a second derivation, laid over parts that cannot answer it and that it cannot
reach — every agreement between shell and interior becomes a hand-computed
constant nothing checks, which is precisely the defect class trial-0002
measured (every silent failure sat in the layer *around* the aligned holes)
and ADR-0020 §Context records at building scale. The only form in which a
shell survives is as the map program's own outermost rules — the massif and
curtain the map writes around the boxes it allocated — at which point it is
not a second object and not model (A). A shell wrapped over the whitebox as a
separate artifact is refused.

**Why not placement composition as the primary (model C: zones stay frozen
prefabs, the whole is assembled by placing them plus connective prefabs).**
It preserves reviewed zone bytes exactly, and it is the ADR-0004 path — but
today it is three capabilities short where (B) is one: grammar prefabs emit
no connectors, zone metadata carries no spatial contracts (the face-mating
check `DW0780` binds to zero faces, `DW0781`), and there is no mechanism for
material or silhouette continuity across placed pieces at all — the
connective tissue still needs a program, so (C) contains (B) plus placement
machinery it does not need. (C) is not deleted: `DW0780`/`DW0781` and the
solver remain the check for multi-prefab areas built from socketed tilesets,
and ADR-0004's "layout validation reduces to graph properties" is what the
map program's contract restores at whole-map scale in-language. No ADR is
relitigated: the map program sits **upstream** of assembly and produces one
prefab; ADR-0004 continues to govern what assembly does with prefabs.

Per ADR-0015 this stays composition-first: §6 adds no new geometry construct,
only the two surfaces that make the already-proven composition reachable from
the artifact of record.

## 3. The artifact of record: the map program and the composition manifest

Two files, both campaign content, both in the campaign's `design/`:

- **`design/programs/map.json`** — the map program. Its top-level splits are
  the site plan; its `params` are the whole-map datums; its palette is the
  material table; its `contract` carries the map-level spaces and the seam
  edges; its own rules are the massif, connective and dressing work.
- **A `composition` block in `zones.json`** — the manifest entry that makes
  the map expandable and auditable without a person reading a design page:
  the map program file, its region and seed, the gates it claims, and the
  list of zone ids it composes. A zone it does not compose is marked
  `detached` in the manifest with a reason — a genuinely detached zone is
  legal; an unaccounted one is an audit red.

The map program is authored **before the zones are final, as the site plan**:
its first version allocates boxes, datums and seam edges with the zone entry
rules as stubs, and zones are then authored into the boxes it hands them. (A
campaign whose zones predate it — the adoption case — authors the map program
around the existing zone programs instead; the obligations are identical.)
The whitebox step **is** the map program's first green expansion: every zone
placed, every seam edge proven, contract obligations green over the whole,
route and datum identities holding. Appearance work follows, under §5.

**What it is judged against.** By machine: the §4 obligations, at every
expansion. By eye: the composed render set — which must include named
square-on elevations of the map's identity faces (`--view`), since silhouette
is judged from elevations and no planned camera provides one — reviewed
beside the campaign's reference imagery under the campaign's own design-gate
rules. Silhouette complexity stays a measurement, never a gate.

**What makes it impossible to skip.** Bound to events, not to a checklist:

- `delve-grammar audit --campaign-root` (already run by both repos' CI on
  every PR and push) expands the map program at the manifest's region and
  seed and judges it like any zone; a `composition` block naming a missing
  zone, a zone program neither composed nor marked `detached`, or a red
  obligation is an audit red.
- Exporting the map writes the composed zone prefab ids into the map
  prefab's own metadata, so the fact of composition reaches the layer
  `delvec` reads. `delvec` refuses to build a campaign that binds, as its
  own area, a prefab that another prefab in the same registry declares it
  composes (new diagnostic, validation tier): a composed zone ships only
  inside its composition, so a playtest cannot be staged around the map.
- The export's provenance row makes the map prefab regenerable from the
  program, hash, region and seed like any prefab; the release freeze names it
  like any campaign file.

## 3b. The ground, when the ground is designed

A map stands on something, and where that something is *designed* it is part of
the composition rather than a setting the composition is dropped into.

**The boundary, and it is the same test every primitive is held to.** The horizon
line exists to place a finished scene into a **backdrop**: terrain produced by a
parameter-controlled analytic generator, chosen from a small set of bases, never
authored the way a building is authored. That is the right shape for ground
nobody is designing. It is the wrong shape for ground that carries the
silhouette, that the parts are cut into, and that a reviewer judges by eye —
because a base built to satisfy one map's landform would be one campaign's
design wearing a primitive's clothes: buildable by nobody else, configurable
into nothing.

So: **designed ground is content and is written in the DSL; undesigned ground is
backdrop and belongs to the horizon.** A map whose ground is designed declares
the flat, surround-free backdrop under it and builds its landform as ordinary
rules in the map program.

This is not a loss of fidelity. A landform assembled from stepped axis-aligned
masses reads as rock at playable scale, which is the scale the constitution says
judgement happens at; the curve in a piece of concept art is a drawing
convention, and the silhouette is what carries the recognition. There is
precedent in the corpus: a sea cliff with a road cut across it has already been
built this way, striation and all.

### The seam class this creates

A part cut into ground is **not** the zone-to-zone seam section 4 is written
for. Two zones meeting is two authored volumes agreeing on a shared face. A zone
in ground is one volume **displacing** another: the ground owes a void exactly
where the part sits, the part owes closure exactly where the ground stops, and
neither obligation is the other's mirror.

Stated as obligations, so they are checked rather than remembered:

1. **The ground declares no cell inside a part's box.** A ground rule that fills
   into a placed part's volume is a refusal at expansion, never a silent
   overwrite — an overwrite reads as a solved seam and ships a part whose
   interior has been replaced by rock.
2. **A part's exterior edges meeting ground are exterior obligations, not
   interior ones.** A face opening onto sky above a cliff is not an unwritten
   wall; a face opening into the rock is.
3. **The ground's walkable surface is part of the map's reachability**, so a
   part reachable only across ground the ground does not actually provide is a
   stranded part, caught by the same proof that catches a stranded zone.
4. **The count is stated.** How many part-to-ground seams the map has, and how
   many were examined. A map reporting zero has either no designed ground or a
   binding failure, and those two must not look alike.

## 4. Mutual consistency: what a part owes, what the whole owes, what is checked

**A part owes the whole:**

1. **Refusal, not accommodation.** Its guards refuse a box it cannot build in
   (`grammar.md` §5c: "a piece handed too little refuses for itself, loudly").
   A part that silently degrades in a wrong box is the defect.
2. **A spatial contract** — spaces, envelopes, edges, and above all its
   `exterior` edges, which are the faces it opens to the world (spec-0036).
   A part without one leaves every map-level obligation over its cells unbound.
3. **Its datums as params, never as constants.** A floor height, a waterline,
   a rise the whole must meet is a declared `param` the map can bind, with
   the part's own guard tying it to the geometry that realises it (the
   `climb == treads()` pattern).
4. **Its materials as palette roles** — restylable without knowledge of how
   the part was laid (`local` frames), so the whole can be the palette
   authority (below).
5. **Renameable anchor stems**, so two parts declaring one stem are the
   include site's explicit decision, never a silent union.

**The whole owes a part:**

1. **A box that satisfies its guards** — and takes the refusal as the answer
   when it does not.
2. **The datums, bound once.** Every whole-map fact a part obeys locally is
   one map-level `param` pushed down by `bind`, stated in exactly one place.
   The worked example is the single water plane: one `water_y`, bound into
   every wet zone, with each zone's floor offsets declared against it and the
   identities guarded at the one scope where both numbers are visible — so a
   zone floor that drifts against the plane refuses at expansion with both
   numbers named, instead of flooding at boot. The same shape carries the
   elevation datum between adjacent zones (the plinth arithmetic, one scale
   up) and any campaign-specific invariant of the same kind.
3. **The mass and the seams.** Everything between and around the allocated
   boxes is the map program's own writing — the licensed "mass no piece can
   know about". Each zone-to-zone junction is a declared contract edge with
   its class and `rise`, whose `via` cells the map's connective rules build.
4. **The silhouette.** The outer form — massif, curtain, roofscape — is map
   rules over map-owned boxes, composed *around* the zones' own exteriors,
   never overwriting a cell a zone owns (split children partition; there is
   no overlay, by design).
5. **Material continuity.** The map palette is the authority: it rebinds each
   included zone's structural roles from one material table, so "the same
   stone" is one binding read eight times, not eight measurements that happen
   to agree. A zone's deliberate divergence is a binding the map declines to
   override, visible at the include site.

**What the machine checks, and where:**

- **The spec-0036 obligations over the whole expansion.** This is the load
  the decision carries: once the map is one derivation, every zone-to-zone
  seam is *interior* — closure catches an unwritten seam wall (the
  transept-class defect, now across zones), graph-confined reachability
  catches a stranded zone, every seam edge owes its declared `rise`. No
  per-zone gate can see any of these; the checker already runs inside
  `expand` and a red writes no prefab.
- **Guard refusals** for every datum identity and undersized box, at
  expansion, upstream of any artifact.
- **`audit`** binds all of it to CI (§3); binding counts stay map-level, per
  the tile-set rule that a tile is packaging, never a unit of judgement.
- **Determinism**: the map prefab double-expands byte-identically (ADR-0006);
  the manifest row regenerates it.
- What is **not** machine-checked, stated so nobody waits for it: whether the
  silhouette reads as the place, and whether the materials read as one
  fiction. Those are the elevation review and the palette look (spec-0035's
  visual leaf), by eye, at the design gate.

## 5. Appearance after whitebox: the line that protects a playtest

After the whitebox freeze, changes are classed by what they can move, and the
class is **verified, not declared**:

- **Class T (texture)**: palette rebinds and mix-weight changes — including
  air shares. These re-run every gate (seconds) and must leave the
  **geometry signature** unchanged: the standable-cell set, the anchors, and
  the resolved contract of the expansion. A mix visits the same cells
  whatever its weights, but an air member can still open a floor — which is
  exactly why the signature is compared rather than trusted. Class T changes
  invalidate nothing downstream: no route proof, no anchor binding, no
  human judgement of the space.
- **Class G (geometry)**: any change to rules, splits, params, claims or the
  contract. These reopen route proofs and the design review, and are never
  folded into an appearance round.

The class is enforced as data, not discipline: the composition manifest
records the map expansion's **geometry signature hash** (canonical bytes over
standable set + anchors + resolved contract). `audit` recomputes it on every
run; a mismatch is a red naming the moved element. A geometry change
therefore updates the recorded hash in the same change — visible in the diff
as one line — so geometry can never move silently under an appearance edit,
and an appearance edit is provably inert by the hash not moving. Relight
(spec-0010) and the campaign's own proofs run downstream of the placed
prefab, unchanged.

Because of §1.4, texture is composition-relative: an included zone re-draws
its mixes inside the map derivation, so per-zone accepted renders certify
geometry, palette and distribution but not the exact texture bytes. The
review that certifies appearance is the **composed** one — which is the point
of this spec — and §8 records the alternative.

## 6. Capabilities required, in order

1. **Tile-set placement in `delvec`** (the already-queued chunked-export
   phase 2 that `DW0346` names). Required by every composition model,
   including doing nothing (three zones already exceed the cap alone). The
   compiler places a manifest's tiles at their zone-relative offsets as one
   piece; anchors, contract and `waterline_y` remain zone-relative; the
   assembled world double-builds byte-identically. `DW0346`'s tile-set case
   retires when this lands.
2. **Document-level include.** The `compose::include` / `include_renaming`
   semantics, reachable from the artifact of record: a manifest-driven
   compose step (CLI) or an include block in the `Program` document, fenced
   at the next `Program` version (ADR-0018 §7). Same refusals, same
   anchor-rename rule, same seam byte-identity promise, extended to `splits`
   when spec-0037 lands (its AC4 already anticipates this). Without it the
   map program is only writable in engine Rust, which breaks both "the IR is
   the artifact of record" and the rule that everything authoring needs runs
   on the creator's own machine.

Nothing else is required. Named partitions (spec-0037) reduce the map
program's restated-plan cost and are wanted, not prerequisite.

## 7. Acceptance criteria

1. A campaign whose `zones.json` carries a `composition` block has its map
   program expanded and judged by `delve-grammar audit` at the declared
   region and seed, every spec-0036 obligation reporting a non-zero binding;
   a block naming a missing zone, an unaccounted zone program, or a red
   obligation is an audit red (asserted by fixture).
2. `delvec` refuses (validation tier, by code) a campaign binding as an area
   a prefab that another registry prefab's metadata declares it composes;
   the map export is what writes that declaration.
3. Cross-zone red demo: a fixture of two included zones whose connective rule
   drops one seam wall course is red on closure at map expansion while both
   zones alone stay green on every gate; restoring the course is the green.
4. Datum red demo: perturbing one composed zone's floor param against the
   map-bound water plane refuses at expansion naming both numbers; the
   unperturbed fixture builds.
5. Geometry signature: (a) a palette-only rebind of the map fixture leaves
   the recorded signature hash unchanged and the emission differing only in
   palette member blocks (script-asserted, the spec-0026 AC6 shape); (b) an
   air-share change that removes a standable cell is an audit red on the
   signature; (c) an edit that moves the signature is red until the recorded
   hash is updated in the same change, and updating it is one visible diff
   line.
6. Determinism: the map fixture double-expands to byte-identical tiles,
   manifest and metadata; with tile placement (§6.1) the assembled world
   double-builds byte-identically.
7. Document-level include reproduces the Rust-composed bytes for the three
   programs the existing seam test pins, from JSON alone; a loader below the
   fenced `Program` version refuses the surface by name.
8. `delve-grammar coverage` counts the include surface with a binding count
   and a corpus program demonstrates it.
9. The composed render set for a map fixture includes at least one named
   square-on elevation per declared identity face, and the run's shot
   manifest records it.
10. Reference imagery stays rank-only at map scale: the existing structural
    enforcement (`DW0725`/`DW0726`) covers a whole-map contact sheet, by
    fixture.
11. `docs/reference/` (grammar, tools, prefab-procedure, compiler DW rows)
    updated in the same PRs that land each piece, per the tooling-sync rule.

## 8. Open, stated as open

- **Texture preservation under composition.** The draw stream is sequential
  (§1.4), so composing a zone re-textures its mixes. If a re-textured
  composition is ever rejected at review *because* an accepted standalone
  look was lost, the remedy is a scope-addressed stream — a real IR change
  with its own determinism story — and it is deliberately not designed here.
- **Whether a whole-map reference image joins a campaign's approved set** is
  a campaign decision under its own design gate; this spec only fixes the
  mechanism (reference-never-target, rank-only, drawn before the map program
  is authored, style-anchored on the part images where the tool supports it)
  and the triage rule: a concept element the language cannot state is probed
  and then resolved as concept-vs-capability (the spec-0037 §1 method), never
  silently dropped and never voxelized.
- **Per-zone review artifacts after map review exists**: whether zone-level
  render acceptance remains a gate for composed zones or becomes advisory
  once the composed review is the appearance authority.
