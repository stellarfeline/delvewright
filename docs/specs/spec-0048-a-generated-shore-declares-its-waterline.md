# spec-0048: A generated shore declares its waterline

- **Status**: Accepted
- **Question**: a placed piece that declares `waterline_y` — the local y of its
  top authored water block — is held by `DW0344` to land that plane at the
  ocean horizon's sea level, so a shore's water meets the sea it depicts. Two
  of the three producers of prefab metadata can write the declaration: the
  hand-written tileset generators do (five shipped island prefabs carry it),
  and external admission preserves it. The third, the grammar back end,
  structurally cannot: a `Program` document has no construct that reaches the
  field, and both export sites write `waterline_y: None` unconditionally. So
  on every grammar-built zone the proof examines zero pieces, and a
  grammar-built `horizon: ocean` world carries the `DW0344` zero-binding
  report **permanently** — no authoring act a grammar author can take answers
  it. This spec gives the program document a first-class waterline datum,
  verified at export against the fluid the piece actually authors, written
  through to the existing metadata field.
- **ADRs**: 0006 (determinism — the datum is a function of the document, and a
  hand edit to an exported file breaks the provenance row's regeneration
  claim), 0011 (the compiler owns the placement check; nothing here moves it),
  0018 §7 (a new optional document field rides a version fence)
- **Specs**: 0027 (grammar back end — the document surface that grows), 0040
  §4.3 (a part's datums are params the whole can bind — this is that
  obligation given a surface) and §1.5 (`waterline_y` stays zone-relative on a
  tile set, already built), 0013 (the horizon owns the world-side fact — sea
  level — and this field never restates it), 0026 (Proposed: the per-area
  datum, and the severity of the zero-binding seal — both stay there), 0038
  (Proposed: the runtime flood plane — the horizon's side of the same
  boundary), 0046 (the precedent: a metadata property two producers could
  write and the grammar could not), 0047 §5 (this finding, named there as
  fixed elsewhere)
- **Non-goals**: raising the `DW0344` zero-binding warning to a refusal
  (spec-0026 owns that, with the per-area datum that makes a dry ocean piece
  authorable). Any change to `DW0344`'s placement rule, to `SEA_LEVEL`, or to
  the campaign DSL (`dsl_version` untouched). Deriving a waterline from
  geometry with no declaration (§2.1). A per-body or named-fluid surface
  (§3, last paragraph). Fluids other than water — a horizon whose ambient is
  another fluid brings its own generalization. Campaign content: the bell
  zones adopt on their own branch, in their own round.

## 1. The measured ground

Instruments: engine worktree at revision `4f396bae` (source, and the
`delvec` binary built from it), the content library at the checkout the
`campaigns/` symlink resolves to (revision `72913b06`), and the compiler's own
committed fixtures run on the same tree. Three readings per claim where the
claim is load-bearing, with unrelated failure modes.

- **The grammar cannot spell it, by construction.** `Program` is
  `#[serde(deny_unknown_fields)]` over exactly eight fields (`version`,
  `name`, `start`, `params`, `palette`, `include`, `rules`, `contract`);
  none is a datum. Live cross-check: a corpus document given a `"waterline"`
  key is refused by `delvec grammar check` naming the unknown field and the
  whole closed list. And both export sites (`crates/grammar/src/export.rs:507`
  and `:627`, the single-template and tile-set writers) construct their
  metadata with `waterline_y: None` literally.
- **The exported document does not even carry the key.** The field serializes
  as `skip_serializing_if = "Option::is_none"`, so a grammar export's `.json`
  has no `waterline_y` at all — byte-identical to the shape left behind when
  an admission step *deleted* the field, which is the exact
  indistinguishability `DW0344`'s zero-binding doctrine names. Demonstrated:
  `delvec grammar expand --program ambush-door --region 11x5x13` writes
  metadata whose keys are `anchors`, `connectors`, `license`, `lighting`,
  `prefab_id`, `structure`.
- **The proof works wherever a declaration exists, and seals where none does.**
  On this tree, `ocean_waterline_off_sea_level_exits_3_with_dw0344` and
  `an_ocean_world_where_nothing_declares_a_waterline_reports_dw0344_unbound`
  (`crates/delvec/tests/cli.rs`) both pass: an off-datum declaration is a
  build error, and an ocean world with zero declarations gets the `DW0344`
  binding report rather than a silent pass.
- **The binding count today is five, all of it hand-generated.** One ocean
  world exists in the released corpus, `nobodys-cave-island`: four solver
  pieces in `area/island` (its accepted build's `render-plan.json` names
  `island-beach-camp`, `island-greenfield` ×2, `island-mountain`) plus
  `island-galley` bound directly to `area/open-sea` — five placed pieces, and
  all five prefabs declare `waterline_y: 2`, written by the Rust island
  generators. Over grammar-built zones the count is zero, over all 35 corpus
  programs, the 8 bell zone programs, and every composed map — and
  `crates/compiler/tests/registry_load.rs` says in as many words that no
  shipped grammar-exported prefab declares the field.
- **The declaration channel that exists is not reachable from generation.**
  `docs/reference/prefab-procedure.md` lists `waterline_y` in the metadata
  schema and admission preserves it (`crates/admit/tests/
  metadata_preservation.rs`), but no admission step writes it, so the only way
  a grammar-built shore could carry one today is a hand edit of the exported
  `.json` — see §2.2 for why that is refused rather than documented.

## 2. Why the obvious repairs are refused

1. **Derive it: write the top of the piece's edge-reaching fluid as the
   waterline, no surface at all.** The exporter already measures fluid bodies
   and which of them reach the piece's outer face (`settle::fluid_bodies`, the
   `fluid-contained` gate's own machinery). But the top of edge-reaching fluid
   is not a claim about the sea: a stream legitimately running off a face into
   an ocean world tops above sea level, and derivation would either red that
   piece or, declared silently, green a wrong datum. "This water is the
   world's water" is intent, and a derived intent is unfalsifiable — the same
   ground on which spec-0046 §2.3 refused deriving the entry point.
2. **Hand-edit the exported `.json`** — the field exists there and admission
   preserves it. Refused twice over: re-export overwrites the document, so the
   edit dies on every regeneration; and the export's provenance row claims
   that program + region + seed + options reproduce both files byte for byte
   (ADR-0006), a claim a hand-edited document falsifies. It is also a layer
   boundary left to folklore, which the constitution forbids: if content
   needs the primitive, the document exposes it first-class.
3. **A free-form metadata passthrough on the program** (`"metadata": {…}`
   copied to the export). This writes a declaration nothing ties to the
   bytes: a document could declare a waterline for water it does not author,
   and `DW0344` would then bind to a fiction and pass it — an opt-out whose
   proof obligation the defect can supply. A datum the exporter does not
   verify is worse than the zero binding, because the zero at least reports.
4. **Let the program state the sea level** (`"sea_level": 62`). The
   world-side fact belongs to the horizon (spec-0013, spec-0026, spec-0038
   all place it there); a program naming a world y binds a piece to one
   campaign's world, which is a design decision wearing a primitive's
   clothes. The piece owns only its local datum; the compiler joins the two.

## 3. The decision

**The program document declares its waterline as a document-level datum, and
the exporter verifies it against the fluid the document authors.**

- `Program` grows an optional `waterline`: an integer expression over the
  document's own params — at minimum a literal and a param reference; a form
  that names a scope dimension is refused where written, because a
  document-level datum has no box. Making it param-expressible is spec-0040
  §4.3's obligation ("a floor height, a waterline … is a declared `param` the
  map can bind") given the surface it was missing: the map binds the param,
  and the geometry the param drives and the datum it states move together.
- **The exporter verifies before it writes**: at the declared local y there is
  at least one authored **water** body whose top surface is that plane and
  which reaches the piece's outer boundary — the same "fluid at the outer
  face, counted and never judged" object `DW0800` measures, here finally
  judged by the party that knows: the document's own claim. A declaration out
  of the region's y range, or naming a plane no such body tops, is an export
  refusal with its own DW code — so a declaration that was deleted and one
  that was never true stay distinguishable, and `DW0344` can never bind to a
  fiction. A fluid body strictly interior to the piece (a flooded ward, a
  fountain) neither satisfies nor requires a declaration.
- Written through to the existing `waterline_y` on **both** export shapes,
  zone-relative — the tile-set half is already built and promised
  (spec-0040 §1.5); this fills the value in.
- **Fence**: the field is refused below its program document version, per the
  `version` module's own doctrine for optional fields (`mirror` precedent).
  The version number is allocated at dispatch of the implementing round and
  reserved in `RESERVED_VERSIONS` until the constant lands; this spec
  deliberately names none.
- **Include**: an included part's `waterline` does not carry into the
  composition — a datum is a fact about the exported artifact, and only the
  outermost document exports. Named here so the drop is a stated rule rather
  than the silent contract-drop shape spec-0040 §1.7 recorded; what keeps it
  honest is geometric: the whole's own declaration is verified against the
  composed bytes, and a sea-authoring whole that declares nothing is exactly
  the compiler's zero-binding report, unchanged.
- Compiler untouched: `DW0344`'s rule, `SEA_LEVEL`, the zero-binding seal and
  its severity all stay where they are. The change is that generated zones
  join the population the proof examines.

Why one scalar and not a per-body surface: a piece meets one ambient, so it
has one datum; fluid bodies are emergent from fills and have no names to hang
a per-body claim on. The verification is what keys the capability to the
object class it acts on — a body of standing water at the piece's boundary —
without inventing an addressing scheme no other consumer needs.

## 4. Acceptance criteria — each stating what would make it vacuous

1. A program declaring `waterline` exports metadata carrying `waterline_y`
   with the evaluated value, on **both** export shapes — asserted on a
   single-template export and on a tile-set export (a region past the
   48-per-axis cap on one axis), zone-relative in both. *Vacuous if* only the
   single shape is exercised, or if the asserted value could arise as a
   default: the fixtures assert key presence and the exact declared value,
   and the tile-set fixture's declared plane sits inside a tile that is not
   the origin tile.
2. A declaration the bytes do not realise is an export refusal naming its DW
   code: one fixture declaring a plane no edge-reaching water body tops, one
   declaring a y outside the region. *Vacuous if* the fixture fails for any
   other reason — the test asserts the code, not a nonzero exit — and
   *vacuous if* only the red half exists: the same document with the water
   authored exports green, as the paired half of the same test.
3. A param-bound waterline moves with its param: the same document expanded
   under two bindings of the named param exports two values, each verified
   against its own geometry. *Vacuous if* the two bindings produce identical
   geometry — then the verification never re-decided anything; the fixture
   asserts the two exported values differ.
4. The document is refused below the fence version, naming the construct and
   both versions. *Vacuous if* the pre-fence document differs from the
   post-fence one in anything but its `version` line.
5. An ocean campaign placing a grammar-exported declaring zone builds with
   `DW0344` examining a nonzero count, and the same campaign with the zone's
   declaration absent gets the zero-binding report — the red→green pair at
   the compiler seam, on a grammar-built piece. *Vacuous if* the fixture's
   declaring piece is a hand-tileset prefab, which could already declare; the
   fixture names its piece's generator as the grammar export. A third arm
   places the declaring zone off-datum and asserts the `DW0344` build error.
6. Every existing document — none of which can carry the field — exports
   byte-identically: `.nbt` and `.json` both, compared against artifacts
   produced by the pinned pre-spec engine, named by revision. *Vacuous if*
   measured by running one engine twice.

## 5. Order of work

1. Fence: version reservation (number handed by the planner at dispatch) and
   the field's row in the `tools/check-grammar-ir-compat.py` ledger.
2. `Program::waterline` + validation (no-dims rule, fence refusal).
3. Export: evaluate, verify against `settle::fluid_bodies`, write at both
   sites; the refusal's DW code (handed at dispatch), its catalog row, and
   the test asserting it (DW-coverage rule).
4. Corpus demonstration: a library or idiom program that authors a shore and
   declares its waterline, plus the committed refusal probe — until it
   exists, `delvec grammar coverage` names the construct undemonstrated, which
   is the grammar's own analogue of the gallery obligation and reds in the
   same PR.
5. Docs, same PR: `grammar.md` §2 table and §7 export; `prefab-procedure.md`
   metadata row (the field now has a generation-time producer);
   `compiler.md` `DW0344` row, whose account of what cannot declare changes.
6. Demo level row queued in `docs/demo-levels.md`: a shore zone whose water
   meets the sea, and the off-datum red beside it.
7. Adoption (separate round, campaign branch): the bell's sea-meeting zones
   declare; the zero-binding report on that campaign's ocean world going
   silent is that round's acceptance criterion.
