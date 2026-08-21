# spec-0040: Map composition — how a whole map gets its appearance

- **Status**: Superseded (ADR-0022)
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
2. **One program document composes another.** The `include` list (fenced at
   document version `1.5.0`) copies rules, params and palette roles under a
   prefix, rewriting every reference; `bind` passes arguments — and a paint in
   either frame — through any call depth, per call site; an anchor rename is
   explicit per stem. The seam promise is pinned by test: an included program
   expanded over the same box is byte-identical to the program alone (item 4
   bounds what that covers inside a larger derivation). Both entry points
   that read a program file go through one loader — `--file` on every
   command, and each program `audit` collects — so a composed document is
   judged inside its composition on both paths (`grammar.md` §5c).
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
5. **The compiler places a composed map.** A whole map exceeds the
   48-per-axis template cap on any axis, so it exports as a tile set; a
   tile-set manifest loads, is indexed under its own prefab id, and is placed
   as one piece at one extent, with anchors, contract and `waterline_y`
   staying zone-relative. `DW0346` keeps only what it always meant about any
   document — that this one is malformed: tiles that do not cover the zone
   exactly, a tile past `part_max`, an empty parts list, or a document
   declaring neither structure block or both.
6. **Areas do not touch.** An area's origin is `[i·256, base_y, 0]`
   (`compiler.md` stage 1), and a grammar prefab enters an area by direct
   placement only (no connectors). A contiguous whole is therefore one area
   binding one prefab; there is no path today by which separately placed zone
   prefabs become one silhouette.
7. **The one declaration class include does not carry is the contract.**
   `compose::include` copies rules, params, palette roles and claims (claim
   region names are prefixed; the destination must classify them or
   `validate` refuses) — and drops the composed document's own `contract`
   block without a refusal (`crates/grammar/src/compose.rs`). Its spaces,
   envelopes, out-of-walk reasons and edges — its `exterior` edges first —
   reach the composition only as hand restatements in the destination's
   contract. §4's obligations assumed the part's contract serves the whole;
   as carried, it serves only the part's standalone review.
8. **Composed at eight-zone scale, the method held and the assumption in
   item 7 was what failed** (`docs/trials/trial-0003-halgrave.md`). Six
   contract gates red, every red cashing to composed parts that declare no
   contract; of ten seams, zero aligned by construction; and the probe that
   tried to opt the contractless volumes out of the walk passed four gates
   because a buried box supplies `sealed` and an open box supplies `facade`
   for free — the defect supplying the opt-out's own evidence. The composed
   render read as one place: the decision of §2 is not what failed.
9. **Extent already flows down; nothing stops it flowing up.** The
   whole-constrains-part half of allocation is built: `split` partitions, an
   absolute split too big for its scope refuses naming both numbers, a write
   outside the model region never ships, and a part handed too little refuses
   for itself, loudly (`grammar.md` §4a, §5c). A part cannot overrun its box,
   and never did. What is unconstrained is the reverse: the map's own region
   is a free manifest value, and the first map composed at scale set it to
   the arithmetic sum of the parts' pre-existing depths — verified on the
   manifest itself: seven Z bands, each equal to one zone row's declared
   depth, summing to exactly the map row's 436 — yielding a 1 : 5.5 site
   against a brief asking a compact stepped mass, the crown subtending 6.7°
   from the arrival point against a derived 27°, computable from `zones.json`
   alone before any composition ran (trial-0003, R1 and the compactness
   attribution). The parts predate the site plan, so by the time anything
   composed them their total was a fact: §3's ordering sentence bound
   nothing — the UNRUN shape, inside this spec.

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
only the surfaces that make the already-proven composition — and what a part
already declares about itself — reachable from the artifact of record.

## 3. The artifact of record: the map program and its manifest row

Two files, both campaign content, both in the campaign's `design/`:

- **`design/programs/map.json`** — the map program. Its top-level splits are
  the site plan; its `params` are the whole-map datums; its palette is the
  material table; its `contract` carries the map-level spaces and the seam
  edges; its own rules are the massif, connective and dressing work.
- **An ordinary `zones.json` entry naming the map program** — the same
  manifest row every zone has: id, program file, region, seed, claimed
  gates. There is no separate composition manifest, deliberately: which
  zones the map composes is its own `include` list, and a manifest field
  restating that list would be a copy nothing checks, written to be
  believed. What the audit accounts against is instead the **loader's own
  record** of the files each entry's document composed — a record a file
  nothing composes cannot produce. A program file no entry names and no
  document composes is an audit red by name; a genuinely detached zone is
  simply its own manifest row, judged standalone, so detachment needs no
  marking and an unaccounted program cannot look like it.

The map program is authored **before the zones are final, as the site plan**:
its first version allocates boxes, datums and seam edges with the zone entry
rules as stubs, and zones are then authored into the boxes it hands them. (A
campaign whose zones predate it — the adoption case — is the transitional
class of §3c: the site plan is still derived from the whole's brief, never
from the parts' extents, and every pre-existing part confronts its allocation
at the map's first expansion.)
The whitebox step **is** the map program's first green expansion: every zone
placed, every seam edge proven, contract obligations green over the whole,
route and datum identities holding. Appearance work follows, under §5.

**What it is judged against.** By machine: the §4 obligations, at every
expansion. By eye: the composed render set — which must include named
square-on elevations of the map's identity faces (`--view`), since silhouette
is judged from elevations and no planned camera provides one — reviewed
beside the campaign's reference imagery under the campaign's own design-gate
rules. Silhouette complexity stays a measurement, never a gate.

**What makes it impossible to skip.** Bound to events, not to a checklist,
and the guarded event's entry points are enumerated, not the one someone
pointed at. *The map is judged*: `delve-grammar audit --campaign-root` (run
by both repos' CI on every PR and push) expands every manifest entry, and
both paths that read a program file — `--file` on every command, and the
audit's manifest sweep — go through the one loader, so a composed document
cannot be read unjudged on either. *A zone escapes both judgement and
composition*: the orphan red above. *A dead manifest surface is written*:
`zones.json` refuses a key the tool does not read, naming it — so a
manifest field that binds nothing cannot be written in the belief that it
binds, which is the exact shape a `composition` block would have had.
*A composed zone is staged alone*: the export-metadata refusal below.
*A part is authored before the whole has allocated its box*: the §3c
allocation identity, whose entry points are enumerated there — a
pre-existing part is a red with a named debtor, never a fact the site
plan inherits.

- Every command that reads a document prints what it composed, prefix by
  prefix, with its `include` binding count; `audit --campaign-root` totals
  the count over the campaign corpus and states a zero by name rather than
  omitting the line.
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

## 3c. The allocation cascade: extent flows down, never up

Between a whole and its parts, "how big" has exactly one authority, and it is
the whole's design of record. §1.9 is the measured failure of the other
direction: a site plan whose totals were inherited from the parts. The rule:
**extent flows from brief to region to boxes to parts. A part that does not
fit is cut down or redesigned; the container is never grown to what the parts
happen to sum to.** Each link, with its check and its status:

1. **Brief → region and plan.** The whole's geometric facts — extent,
   proportions, standoffs, whatever the campaign's written design fixes — are
   declared as map params and guarded as identities over the map's own region
   and top-level splits. `cmp` guards over integers state all of them today;
   this link adds no surface. A site plan violating its own brief facts
   refuses at expansion naming both numbers. What the machine cannot check,
   stated plainly: that the transcribed numbers are faithful to the written
   brief, and the brief to the confirmed reference — that is the design
   gate's reading, fact by fact against the text, the trial-0003 R2 shape.
   The reference image stays rank-only (spec-0028): guards bind to the
   written design of record, never to a picture.
2. **Region → boxes.** Built: `split` partitions, so allocations cannot sum
   past the region; an oversized split refuses naming both numbers.
3. **Box → part.** Built (§1.9): a part cannot write outside its box, and a
   part handed too little refuses for itself. Overrunning an allocation is
   already a refusal by name; no new machinery is owed here.
4. **The part's own row names the same box.** New, and the ordering lives in
   it: for every composed prefix that also has its own manifest row, `audit`
   compares the row's region **extents** (not origins — standalone
   development sits wherever it likes) to the box the composition allocates
   that prefix, up to the include site's declared reorientation. A mismatch
   is a refusal naming the prefix and both extent triples. The box a part is
   reviewed in is the box the composition places it in, or the review
   certifies a different object.

**Why link 4 is the ordering, bound to events.** The entry points by which a
part can exist, enumerated:

- *A part with its own manifest row*: the link-4 identity, red at `audit`,
  which both repos' CI runs on every push and pull request.
- *A composed-only part* (no row): the allocated box is the only box it is
  ever judged at; the ordering is structural.
- *A draft* (`--file`, ad-hoc region): drafting, upstream of record. Every
  artifact-of-record event — audit judgement, the review set, export, the
  release freeze — reads the manifest or the composition, so a draft becomes
  a part only through one of the two cases above.
- *A part that predates the site plan*: the transitional class, below.

Authoring a part first therefore remains typable — nothing can forbid a
file — and stays outside the record: the moment it enters, it confronts an
allocation the brief governs, and a disagreement is a red naming its debtor,
never a fact the plan inherits.

**Revising an allocation, distinct from overrunning one.** The site plan is a
first draft, not an oracle; a part negotiates by refusing its box, and the
plan is then deliberately revised. A revision is a geometry-class change (§5)
to the map program: the splits move, the recorded signature hash moves with
them as one visible diff line, the design review reopens — and the link-1
identities re-run, so a revision can redistribute extent within the brief's
facts but cannot grow past them unless the brief params move in the same
diff. A change to the brief params is a change to the whole's design of
record: it goes through the campaign's design gate as a reference-set
decision, never inside a part-fit round. Sixth-vacuity test, applied: the
permission demands that the whole's own identities still hold over the
revised plan and that the whole's review reopens — a part wanting more room
can supply neither; all it can force is a visible trade against the other
allocations. There is deliberately no per-part surface: no exemption flag, no
oversize acknowledgement, no site-side waiver. The only two authorable moves
are revising the part and revising the plan, and each is proved where it
lands.

**The transitional class: parts that predate their site plan.** A campaign
adopted into composition holds parts whose extents were chosen when no
allocation existed — the state the first composed campaign is in. Handling,
named rather than exempted:

- The site plan is derived from the campaign's written brief exactly as the
  fresh case is — never from the parts' extents. A map region set to the sum
  of part depths is the refused direction whatever the campaign's age.
- Every pre-existing part confronts its allocation at the map's first
  expansion. A part in debt — guards refusing the box, or the link-4
  mismatch — stays red, attributed per prefix, the same debtor-naming
  consequence a contractless part carries (§4); the remedy is that part's own
  revision under its own zone review.
- The whole's brief and reference are not re-derived from the parts, and a
  transitional path that amounts to cutting the whole down to what the parts
  sum to is refused. A brief fact the campaign genuinely wants changed is a
  design-gate decision about the whole, taken before the plan is re-derived,
  never a consequence of part arithmetic.

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
   when it does not. The answer is the part revised, or the plan deliberately
   revised under §3c's proofs — never a box grown in place to what the part
   demands: the plan's totals answer to the brief, not to the parts.
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
   override, visible at the include site. This reaches framed roles too: a
   `bind` value is a paint in either frame (`1.4.0`), so a role a zone
   states in its scopes' own axes is rebound with a `local` paint and
   resolves through each scope's frame — while a world literal pushed over
   it into a turned scope is the `oriented-fills` red, never a silent strip.

### The contract under composition

§1.7 is the gap: as carried today, every obligation above that routes
through the part's contract is met by hand restatement at the include
site, which is the refused shell's defect one level down. Two surfaces
were asked for by the first composition at scale; one is specified, one is
refused by name.

**Specified: the contract rides the include.** The contract is a
declaration class of the document, and include carries every other
declaration class it has; carrying this one is widening the existing
mechanism's reach, not a new mechanism. Fenced at the next `Program`
version (ADR-0018 §7). Under the include's prefix: the part's spaces and
envelopes, its out-of-walk regions (the author's `reason` rides; kinds
stay computed off the composed blocks), and its interior edges with their
class, rise, bars and transit volumes. The declaration is carried, never
the proof: the same checker re-proves every carried edge and envelope
against the composed expansion, in the composed frame. An included `entry`
designates nothing at the composed level — the composition names its own.
Below the fence a composed contract still does not ride, and the
per-document composition report says so by name; a drop is stated, never
silent.

**The part's `exterior` edges become the seam surface.** `exterior` is the
one endpoint a prefix cannot qualify — it names the world, and inside a
composition the world is the composing program. Each included `exterior`
edge therefore arrives as an open seam obligation, and the composing
document adopts each one, by edge and prefix, as exactly one of:

1. **an interior seam** — the edge re-ends on a declared space of the
   composition (a map space, or another part's space; two included
   exterior edges whose openings coincide adopt as one seam, proven
   once). The adopting site states the class and rise it now proves —
   the rise between two composed floors is a fact only the site knows —
   and the part's opening cells and any declared bar ride with the edge,
   so a barred way cannot be adopted as an open one: the standing bar
   fails the walk proof.
2. **re-export** — the edge remains `exterior` on the composition's own
   contract, and `contract-exterior-faces` binds it there: the face
   survives to assembly.

An included exterior edge left unadopted is a refusal at `validate`,
naming the edge and its prefix. There is deliberately no third kind. The
adoption is a choice among proofs the seam must then pass, never among
exemptions — in particular a site cannot declare a part's way buried,
because burial supplies unreachability for free, and an opt-out secured by
what the defect supplies is no gate (§1.8 measured exactly this). A part
whose way may legitimately be shut offers that itself — a `barred` edge, a
parameterised alternative — because the capability belongs to the part,
whose door it is.

**Refused: a composing-site claim over a contractless part's volume.** No
surface will let the map declare spaces, exemptions or coverage over cells
an included document fills but does not itself classify. Three reasons,
each sufficient:

1. The spatial contract belongs to the part — the object whose cells they
   are; spec-0036 is the primitive and it exists. A site-level claim keys
   the capability to the composing verb, and leaves the part contractless
   everywhere else it is reviewed or reused.
2. Any evidence such a claim could offer is supplied by the defect it
   papers over: a buried volume earns `sealed` and an open one `facade`
   from the burial and the openness themselves (§1.8). No demand statable
   at the site separates a designed void from a stranding.
3. A part's interior restated from outside is the shell of §2 one level
   down: hand-computed constants nothing checks.

The consequence is accepted and made legible instead: a composed
contractless part stays red at map expansion, and the red names its
debtor — contract obligations attribute their red cells and counts **per
included prefix**, so "which part owes a contract" is a printed line, not
arithmetic over standalone counts. The remedy is the part's own contract,
landed under the part's own review.

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

The class is enforced as data, not discipline: the map's `zones.json` entry
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

1. **Tile-set placement in `delvec`** — built (§1.5): the compiler places a
   manifest's tiles at their zone-relative offsets as one piece, anchors,
   contract and `waterline_y` stay zone-relative, and the assembled world
   double-builds byte-identically. Every composition model needs it,
   including doing nothing: three zones exceed the cap alone.
2. **Document-level include** — built (§1.2): the `include` list, fenced at
   `1.5.0`, with the `compose::include_renaming` semantics, refusals and
   seam byte-identity promise; extended to `splits` when spec-0037 lands
   (its AC4 already anticipates this).
3. **Contract adoption under include** (§4), fenced at the next `Program`
   version. Until it lands, a composed map is judged only by the contract
   its own document states by hand, and every carried-contract obligation
   of §4 binds zero — which is a stated fact in the composition report,
   never a silent one.
4. **The allocation identity in `audit`** (§3c link 4): per composed prefix
   with its own manifest row, extents compared up to the declared
   reorientation, refusal naming the prefix and both triples; the
   composition report states the compared-row count, a zero by name. Until
   it lands, the ordering rests on links 1–3 alone and the report says so.
   The rest of §3c's cascade is built (splits partition, oversize refuses,
   guards exist) and adds no authoring surface.

Nothing else is required. Named partitions (spec-0037) reduce the map
program's restated-plan cost and are wanted, not prerequisite.

## 7. Acceptance criteria

1. A campaign whose `zones.json` names a map program in an ordinary entry
   has it expanded and judged by `delve-grammar audit` at the entry's region
   and seed, every composed document judged inside it; a program file no
   entry names and no document composes is an audit red naming the file;
   an unknown key in `zones.json` is a refusal naming the key. Asserted by
   fixture, and the fixture asserts a non-zero `include` count — a map that
   composes nothing makes the first two assertions vacuous.
2. `delvec` refuses (validation tier, by code) a campaign binding as an area
   a prefab that another registry prefab's metadata declares it composes;
   the map export is what writes that declaration.
3. Cross-zone red demo: a fixture of two included zones whose connective rule
   drops one seam wall course is red on closure at map expansion while both
   zones alone stay green on every gate; restoring the course is the green.
4. Datum red demo: perturbing one composed zone's floor param against the
   map-bound water plane refuses at expansion naming both numbers; the
   unperturbed fixture builds. The perturbed param is one the **part**
   declares (§4 part obligation 3), and the fixture asserts the guard reads
   one number from each side — an identity over two map-owned numbers
   demonstrates nothing about the part obligation.
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
8. The include surface's binding count is printed where the surface can
   exist: every command that reads a document states what it composed,
   prefix by prefix, with the count, and `audit --campaign-root` totals it
   over the campaign corpus, stating a zero by name rather than omitting
   the line; a fixture campaign with a composing program asserts a non-zero
   total. Not `delve-grammar coverage`: its corpus is the Rust-built
   library, where a document include structurally cannot occur, and
   counting campaign programs there would turn a gap into a green.
8b. Contract adoption red demos, each half required: (a) an included
   contract's `exterior` edge left unadopted refuses at `validate`, naming
   the edge and prefix; (b) adopted as an interior seam, the checker proves
   the site-stated rise against the composed blocks — red with the rise off
   by one, green at the true value; (c) a document below the fenced version
   refuses the adoption surface by name. The fixture asserts the carried
   contract's binding count (spaces plus edges under the prefix) is
   non-zero — a part with an empty contract makes all three vacuous.
8c. Per-prefix attribution: a fixture composing one contractless part reds
   coverage with the uncovered count attributed to that part's prefix in
   the printed detail. The fixture asserts the part carries no `contract`
   key — attribution demonstrated on a covered part proves nothing.
9. The composed render set for a map fixture includes at least one named
   square-on elevation per declared identity face, and the run's shot
   manifest records it.
10. Reference imagery stays rank-only at map scale: the existing structural
    enforcement (`DW0725`/`DW0726`) covers a whole-map contact sheet, by
    fixture.
11. `docs/reference/` (grammar, tools, prefab-procedure, compiler DW rows)
    updated in the same PRs that land each piece, per the tooling-sync rule.
12. Allocation identity red demo: a fixture map composing a part that has its
    own manifest row is green with extents agreeing; the same fixture with
    the row grown one block on one axis is an audit red naming the prefix and
    both extent triples; a reoriented include agrees under its declared
    frame. The fixture asserts the compared-row count is non-zero — a
    composed-only corpus makes the identity vacuous.
13. Brief identity red demo: a map fixture guarding one proportion fact over
    its own region and top splits is green as written and red when one
    allocation grows past the plan, naming both numbers. The fixture asserts
    the guard reads the region extent — an identity over two program
    constants the plan cannot move demonstrates nothing (the AC4 shape).

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
- **Two included ways that cannot mate by their declared faces** (one part's
  way out and its neighbour's way in on unmatable axes). Two candidate
  resolutions exist — a face-adaptation construct, or a part obligation
  that declared ways in and out be co-axial with the route — and choosing
  is a design ruling, not an evidence question, so neither is specified
  here. Until one lands, a map-built corridor joining such ways is a
  transit volume (`via`) on the seam edge, never a mating.
