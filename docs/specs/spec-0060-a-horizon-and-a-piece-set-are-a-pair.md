# spec-0060: A horizon and a piece set are a pair

- **Status**: Proposed
- **Ground**: written against engine `c2a90ac2`. Every number below was
  measured on that revision with a `--release` `delvec` built inside the
  worktree, against a prefab directory holding the seven pieces
  `prefabs/gallery-generator` writes merged with the thirty-six pieces of the
  content library (`campaigns/prefabs`, reached through the `campaigns/`
  symlink), pools merged into one `pools.json`. The carrier for every
  campaign-level number is the gallery's primary campaign with **one added
  `areas[]` element** seating the pool under test — the same device the
  committed probes use. The library-level numbers were taken twice, by two
  readers that share no configuration: a standalone NBT reader and
  `delvec palette`, the engine's own.
- **Diagnostics**: **DW0886** (a pool that cannot be seated on the declared
  horizon) and **DW0887** (a declared waterline that is not in the piece's
  bytes) are allocated to this spec. Both were verified free across all 83
  remote refs at writing, as was the number 0060.
- **Adopts**: spec-0026 §2 (the per-area datum) by citation, and re-allocates
  the codes that section left provisional — see §9.
- **Non-goals**: the three horizon bases spec-0026 proposes and nobody has
  built (`sky`, `flatland`, `summit`); any content work in the content
  repository (§8 names it and stops); the surround generator; the boundary
  clock; retiring `areas[]`.

## 1. The defect: three gates, each naming a remedy the next refuses

Every rule in this section is **cited** — from the engine at `c2a90ac2`, by
running it.

A campaign that seats the shipped cave library, or the shipped island library,
cannot be built on **any** base the engine declares; a campaign that seats
either keep library builds on `void` alone, and on nothing else. The refusals
form a cycle, and each one names as its remedy a base the next refuses:

- `horizon: ocean` → **DW0344** (exit 3): *"its box reaches down to y=60 — at
  or below this world's sea plane (y=62) — and its prefab metadata declares no
  `waterline_y`"*. Its third move: *"CHOOSE another horizon — a world of
  interior pieces that never meant to meet a sea wants `void`"*.
- `horizon: void` → **DW0885** (exit 3): *"its down side standing in open air
  the party can be in, and nothing answers for it"*. Its first move:
  *"BURY it — not under this horizon. `void` is the horizon that declares
  there is nothing outside the placed geometry, so it can bury nothing;
  `valley` is the one base that builds terrain"*.
- `horizon: valley` → **DW0855** (exit 1): *"builds terrain around the map,
  and this campaign never says how big the map is"*. Its move: *"Give the
  campaign a site plan, or set `horizon` to `void` or `ocean`"*.
- and the first half of that move is itself refused: adding a `site-plan.json`
  to an `areas[]` campaign is **DW0839** (exit 1), *"two placement authorities
  for one world"* (spec-0049 §6). Planted and observed, not inferred: the
  valley campaign above with the gallery's site plan copied in reds DW0839.

Each gate is right on its own. The pair rule in `CLAUDE.md` names what the
three of them are together: *a remedy one prescribes and the other refuses is
the pair's defect*, and *a gate that names a remedy owes a check that the
remedy is reachable*. Nothing in the engine holds that check, so the cycle was
found by walking it.

### 1.1 The pairing, measured

Four pools of the four in `campaigns/prefabs/pools.json`, against three bases
of the three `delvec schema --stage world` exports (`void`, `ocean`,
`valley` — the schema is the authority, and it declares **three**, not four).
Twelve cells of twelve measured, each a full `delvec build`:

| pool (members) | `void` | `ocean` | `valley` |
|---|---|---|---|
| `pool/stone-keep` (12) | **builds** (exit 0) | DW0344 (exit 3) | DW0855 (exit 1) |
| `pool/vertical-keep` (13) | **builds** (exit 0) | DW0344 (exit 3) | DW0855 (exit 1) |
| `pool/cave-shore` (13) | DW0885 (exit 3) | DW0344 (exit 3) | DW0855 (exit 1) |
| `pool/island` (4) | DW0318 (exit 3) | DW0885 (exit 3) | DW0855 (exit 1) |

**2 of 12 build.** The two keep pools are sealed shells whose air the party
never leaves, so DW0885 has nothing to judge and `void` seats them. Nothing
seats a cave or an island.

Two readings of that table are wrong and are recorded so they are not made:

- The `pool/island` row was first measured as DW0210 (exit 2) on both `void`
  and `ocean` — the island pieces are dark and the probe area declared no
  mitigation. That is the harness's defect, not the library's. The row above
  is the re-run with `mitigation: "night-vision"`, which is what the content
  campaign that seats this pool declares.
- `pool/island` on `void` reds DW0318 for a reason that is the contract
  working: the island pieces author real water, and under `void` a column the
  content did not build is bottomless, so 9787 fluid cells in 4813 columns run
  off the edge forever. An island piece **belongs** to an ocean. That is the
  whole subject of this spec stated by a diagnostic.

### 1.2 One of the moves cannot be taken, and the reason is the quantifier

DW0344's second move tells the author to *"RAISE the piece clear of the sea,
so its box starts above y=62 … a piece whose lower courses are solid plinth up
to local y=2 stands its walk plane on dry land"*. The check reads
`piece.bbox().0[1]` (`compiler::plan::check_ocean_waterline`) — the minimum y
of the **placement box**, which is the area origin, `OCEAN_BASE_Y` = 60, for
every piece in the world whatever its bytes contain. No plinth moves it. The
move is unreachable as written, and the gallery's own ocean point does not take
it: its four pieces satisfy DW0344 by move (1), declaring a waterline that is
true, and its build reports `waterline binding: 4 of 4`.

The box is a proxy, and it is the wrong one. What makes a delve wrong is a
**walk cell** under the sea — a party wading its critical path — and that is
the rule spec-0026 §2 wrote (*"any walk cell at or below flood level"*). A
piece's box reaches under the sea whenever its lowest course does, which is
every piece that has a floor; under the per-area datum of §3 it is every piece
in every ocean world, including the ones standing perfectly dry. So the second
arm of DW0344 over-refuses by construction, and no amount of authoring answers
it. §5 moves the quantifier onto the cells the rule is about.

### 1.3 Three of the five declarations are fictions

`DW0344`'s binding is a single optional metadata field. In the content
library, 36 prefab documents, **5 declare `waterline_y`** (all of them `2`),
and of those five:

| prefab | declared `waterline_y` | `minecraft:water` at local y=2 | verdict |
|---|---|---|---|
| `island-beach-camp` | 2 | 625 cells | true |
| `island-galley` | 2 | 221 cells | true |
| `island-greenfield` | 2 | none anywhere in the piece | **fiction** |
| `island-greenfield-bend` | 2 | none anywhere in the piece | **fiction** |
| `island-mountain` | 2 | none anywhere in the piece | **fiction** |

Cross-checked by a second instrument sharing no configuration with the first:
`delvec palette` finds a `minecraft:water[level=0]` entry in exactly
`island-beach-camp` and `island-galley`, and none in the other three
(`island-mountain`'s four `water` matches are `waterlogged=false` blockstates,
which are not water). Two readers, same two pieces.

The converse shape exists too and is equally unnamed: `cave-shore` writes 16
water cells at local y=1 and declares no waterline at all. An author copying
the tileset convention onto it would write `2` and manufacture a sixth
fiction.

DW0344 checks that `pos.y + waterline_y` equals the sea plane. It never asks
whether there is water at that plane. A declaration is a claim about the
bytes, and nothing reads the bytes.

### 1.4 The engine's own ocean proof does not generalise

`prefabs/gallery-generator` builds every gallery piece through `to_shore`,
which lifts it onto plinths, cuts a tide pool so its top water block lands on
the sea plane, declares `waterline_y`, and then asserts the result in
`assert_the_shore_is_standable`: *"a shore piece with no authored water
declares a waterline it cannot show"*, and *"the declared waterline is not the
top authored water block"*. That assertion is exactly DW0887 — living inside
one generator, reachable by one library, and by no piece any creator will ever
write. Measured: 5 of the 7 generated gallery pieces declare `waterline_y: 2`
and every one of them writes water at local y=2.

## 2. What a horizon provides

**Authored**, from the three bases the engine implements. Read it as the
horizon's half of the contract: the walk-plane datum a piece is seated
against, what stands outside the placed geometry, and therefore which of
DW0885's three burial moves exists.

| base | walk-plane datum | ambient outside the map | sea plane | what can bury an outward face | needs a `region` |
|---|---|---|---|---|---|
| `void` | `BASE_Y` = 64 | nothing; a column the content did not build is bottomless | none | another placed piece only | no |
| `ocean` | sea plane + 1 = **63** (§3) | pinned superflat: bedrock/stone to y=54, water 55..=62 | **62** | another piece, and the sea for everything at or below y=62 | no |
| `valley` | `BASE_Y` = 64; the surround's gap floor tops at 63 | built terrain — a rim annulus with a gap floor, real blocks in real templates | none | another piece, and the surround's terrain and moat | **yes** — a site plan's `region` (DW0855) |

Three notes the table cannot carry:

- `ocean`'s ambient is analytic and arrives through `nav::Ambient`; `valley`'s
  is blocks and arrives in the assembled map like any other piece's. DW0885
  asks all three the same question about the same cell, which is why a horizon
  that grows terrain changes its verdict without changing its code (cited:
  `compiler::burial` module note).
- `Ambient` has two variants, `Void` and `Ocean`, and `valley` maps to `Void`
  (cited: `compiler::nav`, `HorizonBase::Valley => Ambient::Void`). So the
  binding lines a valley build prints say ``horizon `void``` while the campaign
  declares `valley` — measured on the gallery's site-plan point. The line
  reports the **ambient**, and it should say so: a reader cannot tell an
  unbound check from a mislabelled one. (Authored: §10.7.)
- `valley` is the only base whose availability is a property of the
  **campaign** rather than of the pieces. That asymmetry is not a defect to
  remove; it is the fact DW0886 has to state.

## 3. The ocean datum, decided here

**Authored, against two records that disagree.**

spec-0013 says the sea sits at y=62 and *"areas sit at y=64+, so land reads as
islands"*. The implementation puts them at 60:
`OCEAN_BASE_Y = SEA_LEVEL - ISLAND_WATERLINE_Y`, and `ISLAND_WATERLINE_Y` is
documented in `compiler::plan` as *"the island tileset's authored waterline
(`prefabs/island-tileset.md`)"*. Neither number is a decision about the world.

**Today's constant is an accident**, and this spec says so plainly. It is one
tileset's authoring convention promoted to a world constant by being the only
convention that existed when the ocean was built. Its value is correct for
exactly those pieces whose walk plane is at local y=3, and it is why every
other piece in every library lands with its box bottom under the sea and gets
refused by a check whose second remedy cannot be performed (§1.2).

spec-0013's sentence is wrong in a more interesting way than being the wrong
number. It states a datum for the area **origin**. The relationship the player
experiences is about the **walk plane**: a body standing on a shore is one
block above the water it can swim in, so it can climb out. An origin datum
only produces that relationship for pieces that share one walk-plane
convention, and a general engine has no such convention to appeal to.

So, the ruling, adopting spec-0026 §2's mechanism by citation and fixing its
number:

1. **The ocean datum is a walk-plane datum.** An ocean world's walk plane is
   `SEA_LEVEL + 1` = **63**: one block above the water, the vanilla-normal
   beach relationship. This is the number `walk_ref_y` names in spec-0026 §2,
   and it is unchanged from that spec.
2. **The area origin is derived, never authored and never global.** An area's
   base y is `walk_ref_y − walk_y`, where `walk_y` is the piece set's own
   walk-plane local y, declared in prefab metadata (spec-0026 §2). A keep
   interior (`walk_y` 1) is seated at 62 and stands its walk plane at 63, dry,
   with no author action. An island piece (`walk_y` 3) is seated at 60 —
   byte-identical to today.
3. **`OCEAN_BASE_Y` and `ISLAND_WATERLINE_Y` are retired as world constants.**
   A tileset convention is a fact about a tileset; it lives in that tileset's
   pieces, which is where §4 puts it. `SEA_LEVEL` stays: the sea plane is a
   property of the horizon.
4. **spec-0013's "areas sit at y=64+" is retired**, not reinterpreted. Under
   this ruling the sentence names an origin that would put an island's walk
   plane four blocks above its own shore.

The bug class this closes is the one spec-0026 §2 named and never got to
build: an interior piece whose walk plane sits below the island convention
lands under sea level, floods on boot, and nothing looks.

## 4. What a piece owes

**Authored**, one row per declaration, with the base it is meaningful on. A
piece is a claim about the world it was built for, and these are the claims
the engine has to be able to read.

| declaration | owed on | what it states | who checks it |
|---|---|---|---|
| `walk_y` | **every base** | the piece's own walk plane, in local y — the number the origin is derived from (§3.2) | DW0886 at validation; the seating derivation consumes it |
| `waterline_y` | `ocean`, **and only if the piece authors water that meets the sea** | the local y of the piece's top authored water block | DW0887 (the bytes), DW0344 (the placement) |
| `shown_faces` | any base under which the party can be outside the piece | which sides are finished exterior surface | DW0885 (exists) |

And three rules about those rows:

1. **`walk_y` is not optional and has no default.** A default is the global
   datum wearing a different name: it would be right for the tileset it was
   copied from and silently wrong for every other. A piece placed on any base
   without it is DW0886 (spec-0026 §2 made this DW0367; this spec folds it
   into DW0886, because "this piece cannot be seated on this horizon" is one
   question and the missing number is one of its answers).
2. **`waterline_y` is a claim about the bytes and is checked against them.**
   A piece that declares one and authors no water at that plane is DW0887 —
   whatever base the campaign declares, because the fiction is in the library,
   not in the campaign. A piece that authors water and declares no waterline
   is not refused by this rule; under `ocean` its placement is DW0344's
   subject, and under `void` its runoff is DW0318's.
3. **`shown_faces` is meaningful on every base and reachable on all three.**
   It is the only one of DW0885's three moves that is a property of the piece,
   which is why it is the one a library can carry. §8 names the content work.

What a piece does **not** owe: any statement about which horizon it is for. A
piece that declares `walk_y`, declares a true `waterline_y` if it authors a
shore, and declares the sides it is meant to be seen from has said everything;
which bases those declarations admit is **derived** (§5), never typed. A
`horizon:` field in prefab metadata would be a design decision about what the
piece is for, encoded in a mechanism — the thing `CLAUDE.md` forbids a
primitive from doing.

## 5. When the creator learns

**Authored**, on the precedent this spec cites.

The pairing is a fact about documents and a library: the declared base, the
pools the world names, the members those pools hold, and each member's
metadata and bytes. Nothing has to be placed to know it. The precedent is
DW0855's own implementation note (cited, `crates/dsl/src/validate.rs`):
*"Refused here rather than at the build, because it is a fact about the
documents: nothing has to be placed to know that nothing states an extent."*
DW0855 therefore refuses at **validation, exit 1**, even though its `DwCode`
declares `ExitTier::Build` — measured: `delvec validate` on an `areas[]`
valley campaign exits 1 with DW0855 and places nothing.

**DW0886 — this pool cannot stand on this horizon.** Validation tier, exit 1
(within the brief's "at `analyze`, exit ≤ 2"; `analyze` implies `validate`).
Raised once per (area, pool) pair that cannot be seated on the declared base,
with the reason per member. Its shapes:

- a member declares no `walk_y` (any base);
- the base is `ocean` and a member's declared `walk_y` would put one of its own
  walk cells at or below the sea plane — the piece would be seated wading;
- the base is `ocean` and a member authors water in the sea's plane and
  declares no `waterline_y`, so nothing states where it meets the sea;
- the base is `void` and a member authors fluid that would run off an
  unburied face (the static form of DW0318's finding);
- the base builds terrain and the campaign states no `region` — the case
  DW0855 holds today. DW0855 **stays**, and DW0886 does not restate it: one
  rule, one code. DW0886's message names DW0855's case as a reason it did not
  fire, so the two read as one answer.

**DW0887 — a declared waterline is not in the bytes.** Validation tier, exit 1
when a campaign names the piece; also raised by `delvec prefab audit` over a
whole library, from one shared implementation. It is a property of the prefab
document and its `.nbt`, so it binds wherever those two are read together and
in no other way.

**DW0344 keeps its number and moves its quantifier.** Its first arm is
unchanged: a piece that declares a waterline lands with that waterline on the
sea plane. Its second arm stops asking whether the placement **box** reaches
the sea (§1.2, an over-refusal no author can answer) and asks the question
spec-0026 §2 wrote: is any **walk cell** of this placed piece at or below the
sea plane. That is a fact about the assembled world, so it stays at build tier,
where it is the backstop for whatever DW0886 could not know from the documents
alone — a piece the campaign edits after placement, above all.

DW0885 and DW0318 are unchanged. All three keep judging the assembled world,
because the assembled world is what they read; DW0886 exists so that a creator
never reaches them by a road that has no exit.

**Every remedy each of the four names becomes reachable**, and that is a
check, not a promise — §10.3.

## 6. The pairing, before anything is authored

**Authored.** `delvec prefab seating --horizon <base> [--prefabs DIR]`: the
subject is the piece set, so it joins the `prefab` family (ADR-0023 — one
binary, every capability a subcommand of it). It reads a library and a base
and prints, per pool, whether it can be seated and why.

```
$ delvec prefab seating --horizon ocean --prefabs campaigns/prefabs
pool/island        REFUSED  2 of 4 member(s) seatable
  island-greenfield        `waterline_y: 2` declared, no water block anywhere in the piece (DW0887)
  island-greenfield-bend   `waterline_y: 2` declared, no water block anywhere in the piece (DW0887)
  island-mountain          `waterline_y: 2` declared, no water block anywhere in the piece (DW0887)
  (every member is missing `walk_y`; the ocean datum cannot be derived — DW0886)
pool/stone-keep    REFUSED  0 of 12 member(s) seatable
  ...
seating binding: 4 pool(s) of 4 in this library examined over 42 member(s);
36 document(s) read, 36 `.nbt` opened; 5 waterline declaration(s) examined,
2 borne out by the bytes.
```

Three properties are load-bearing:

1. **It opens the bytes.** A verdict computed from declarations alone would
   report `pool/island` seatable on `ocean` — that is precisely today's
   green. The acceptance criterion (§10.4) is written so a declaration-only
   implementation fails it: the verdict must name the three fictions.
2. **It states a numerator and a denominator at every level** — pools of
   pools, members of members, declarations examined of declarations found —
   because a seating report with no denominator is the vacuity `CLAUDE.md`
   forbids, and because a library that has lost a field must read as a red,
   not as a small number.
3. **It is the same code the compiler runs.** One implementation, two entry
   points (this command and validation), so a library can never be seatable
   according to the tool and refused by the build.

## 7. Which pools survive, under this contract

**Authored**, from §1.1's measurement and §4's rules. `walk_y` and
`shown_faces` do not exist on any content piece today, so every row is stated
as: what the pool becomes once the content work in §8 is done.

| pool | `void` | `ocean` | `valley` |
|---|---|---|---|
| `pool/stone-keep` | **yes** (builds today) | **yes** once `walk_y` is declared — a keep interior is seated at 62 and stands dry at 63 (§3), and the sea buries its outward faces below the sea plane | yes, in a site-plan campaign |
| `pool/vertical-keep` | as above | as above | as above |
| `pool/cave-shore` | no — a cave is authored to stand inside rock, and `void` provides no rock; `shown_faces` on a cut hillside face would be the fiction DW0885's own message warns against | **yes** once `walk_y` is declared and `cave-shore` declares the waterline it really authors (local y=1, not the tileset's 2) | **yes** — this is the pool `valley` exists for: the surround is the hill the cave was cut out of |
| `pool/island` | no — the water runs off the world (DW0318), and that is correct | **yes** once the three fictions are repaired (§8) and `walk_y: 3` is declared | no — an island in a mountain valley is a different piece |

The general shape: **`void` is for sealed interiors, `ocean` is for pieces
that author a shore, `valley` is for pieces authored to be buried.** No pool
is for every base, and the contract's job is to say which is which before a
creator spends a build finding out.

## 8. What the content repository must do

Named, not done — this spec writes no content and touches no library.

1. **Declare `walk_y` on all 36 prefab documents.** It is a measurement of the
   piece, so it is derived by the generator that writes the piece and never
   typed by hand (`CLAUDE.md`: a census derivable from the object is never
   hand-written). The tilesets state their conventions already:
   `prefabs/island-tileset.md` (3), `prefabs/keep-tileset.md`,
   `prefabs/cave-tileset.md`.
2. **Repair the three fictions.** `island-greenfield`,
   `island-greenfield-bend` and `island-mountain` declare a waterline over no
   water. Each is *either* re-authored to carry the shore it claims *or* has
   the declaration deleted — and the second is the honest default for a piece
   whose whole extent is inland. Deleting it does not make the piece
   unseatable on `ocean`: under §3 it is seated by `walk_y`, and a piece with
   no water simply has no waterline to state.
3. **Correct `cave-shore`.** It authors water at local y=1 and says nothing.
   Declare `waterline_y: 1`, the number its bytes hold.
4. **Declare `shown_faces` on every piece a party can stand outside of** —
   the island set and the cave mouth/shore pieces. This is a judgement about
   the asset, not a mechanical sweep: a side is declared shown only if it is
   finished exterior surface.
5. **Retire nothing yet.** No pool becomes unbuildable under this contract;
   every one of them becomes buildable on at least one base once (1)–(4) are
   done. A pool that cannot be repaired is retired then, with the measurement
   that says so.

The two shipped campaigns are not part of this work. Both declare a
`dsl_version` this engine does not accept (`0.3.0` and `0.6.0` against
`0.21.1`), and nothing owes compatibility to anything already built; the drill
campaign is authored fresh through the skill.

## 9. spec-0026

**Authored.** spec-0026 is Proposed, and only its `valley` half was ever
built. It stays **Proposed**, for the three bases nobody has built (`sky`,
`flatland`, `summit`) and for the surround parameters it proposes.

Its **§2 is adopted and closed here**: the per-area datum, the declaration
that positions and the geometry that proves, and the retirement of
`OCEAN_BASE_Y`. This spec is the one authority for those rules; spec-0026 §2
is a citation, and its status line gains one clause saying so, so that a
reader who lands there is not left with two documents deciding one thing.

Its provisional numbers are **not** taken: spec-0026 wrote *"Provisional codes
DW0364–DW0368; numbers may shift at implementation"*, and DW0364..DW0368 are
long since allocated elsewhere. DW0367's rule (a piece with no `walk_y` on a
non-void horizon) becomes a shape of DW0886. DW0364's rule — a walk cell at or
below the flood level, with no exemption for a piece that declares no
waterline — becomes DW0344's second arm, which today asks the coarser
box question (§1.2) and is corrected under this spec's number rather than
gaining one; and DW0887 stands beside it for the half nobody built, that the
declaration is true.

## 10. Acceptance criteria

Machine-checkable; each names its instrument.

1. **The pairing measurement, with its binding count.**
   `delvec prefab seating --horizon <base>` run over each of the three bases
   `delvec schema --stage world` exports, against the content library, prints
   a verdict for every pool with numerator and denominator at pool, member and
   declaration level. A run whose examined-pool count is zero, or whose
   `.nbt`-opened count is less than its document count, exits non-zero. The
   committed expectation records **4 pools of 4** and **42 members**, and a
   run that disagrees with the library's own enumeration is a red.
2. **The analyze-tier refusal, red then green on one campaign.** A campaign
   seating `pool/stone-keep` under `horizon: ocean` exits 1 from
   `delvec analyze` with DW0886 naming the pool and the member reason, having
   placed nothing (asserted: no placement line in the output). The same
   campaign with the pool swapped for the gallery's shore pool analyzes and
   builds green. Both halves run in one test.
3. **Every remedy a diagnostic names is reachable.** A test enumerates the
   moves DW0344, DW0855, DW0885 and DW0886 name, and for each one builds the
   campaign that takes it and asserts it reaches a different verdict than the
   one that named it. The cycle of §1 is the first case: `ocean`→`void`,
   `void`→`valley`, `valley`→site plan, and DW0344's RAISE move — which fails
   this criterion at `c2a90ac2` (§1.2) and is either made observable or
   removed from the message before this criterion can pass. Adding a
   remedy-naming diagnostic without extending this test is a red
   (`tools/check-dw-codes.py` gains the cross-check: a message that names a
   base or a document as a move owes a row here).
4. **The waterline byte check.** DW0887 has a test per shape (declared with no
   water anywhere; declared with water at another plane) and a committed
   gallery probe. Run over the content library at `c2a90ac2`, the check
   reports **5 declarations examined, 2 borne out, 3 refused**, naming
   `island-greenfield`, `island-greenfield-bend` and `island-mountain`. A
   declaration-only implementation reports 5 of 5 and fails this criterion.
   The binding line prints both numbers on every run, including runs that find
   nothing.
5. **The drill campaign.** A campaign authored through `/new-delve` for an
   ocean either builds — with every placed piece's walk plane at y=63 and the
   sea-seepage binding reporting **0 submerged and 0 wading** over a non-zero
   walk-cell count — or is refused at `analyze` with DW0886 naming the pool
   set. There is no third outcome, and in particular no build that floods: a
   test asserts that no campaign reaching `build` has a walk cell at or below
   `SEA_LEVEL`.
6. **The gallery** (§11), including a non-zero exposure binding on the ocean
   point.
7. **Legibility.** Every binding line that prints a horizon prints the
   declared **base**, and where it reports the ambient it says "ambient". A
   `valley` build's lines no longer read ``horizon `void```.
8. **Determinism and the record.** Double builds of one fixture per base are
   byte-identical (ADR-0006); `docs/reference/compiler.md` carries the DW0886
   and DW0887 rows, the ocean datum of §3 and the piece contract of §4, in the
   PR that lands each; `tools/check-dw-codes.py` is green with zero new
   allowlist entries.
9. **`walk_y` reaches the surface it changes.** `delvec schema` exports it on
   the prefab-metadata document, every generator under `prefabs/` writes it as
   a measurement of the piece it just built, and a test perturbs one piece's
   `walk_y` and asserts an emitted byte moves.
10. A row for this contract's player-visible half — a shore a body can climb
    out onto — is queued in `docs/demo-levels.md` when the seating change
    lands.

## 11. The gallery's obligation

**Authored**, against what the gallery holds at `c2a90ac2`.

What is bound today: `void` on the `void-horizon` overlay, `ocean` on the
`ocean-horizon` overlay, `valley` on the `site-plan` overlay (which declares
`{base: valley, ratio: 2.5, rim_height: 48}` and builds 147 surround templates
around the plan's 64×64 region). Refusals are probed by
`a-horizon-with-no-map` (DW0855), `a-piece-that-says-nothing-about-the-sea`
(DW0344) and `a-face-nothing-stands-in-front-of` (DW0885).

Three obligations follow, and one thing that does not:

1. **An `areas[]` element per base is owed for `void` and `ocean`, and is
   impossible for `valley`** — DW0855 refuses it by design, and that refusal
   is the existing probe. So the obligation is stated per base as: **a bound
   element under the placement authority the base admits, plus a named refusal
   probe for the authority it does not.** `valley` is bound on the site-plan
   point; `a-horizon-with-no-map` is its `areas[]` refusal. Demanding an
   `areas[]` valley element would be demanding the engine contradict itself.
2. **DW0886 and DW0887 each owe a committed probe**, refusal-proven the way
   spec-0039 requires: a primary plus one declared edit the engine rejects
   with the named code. DW0886's probe seats a content-library pool on the
   gallery's ocean point; DW0887's declares a waterline on `gallery-yard`,
   which authors no water.
3. **The ocean point's exposure binding is currently zero and that is a
   finding.** Measured: `piece-exposure binding: horizon ocean; 4 of 4 placed
   piece(s) examined … 0 stand in air the party can be in … 0 `shown_faces`
   declaration(s) of which 0 are bound`. The gallery's ocean world is four
   sealed boxes, so the sea's power to bury an outward face — the thing that
   makes `ocean` seat a keep at all under §7 — is asserted nowhere. The ocean
   point gains a piece the party can walk outside of, and the criterion is a
   non-zero `judged` count with the sea doing the burying.

**The plinths stay, and the assertion moves.** The gallery generator's
`to_shore` — plinths, a cut tide pool, a declared waterline — is how a shore
piece is built, and it is the reference implementation of §4's contract; it is
not a workaround to be removed. What does move is
`assert_the_shore_is_standable`: once DW0887 exists, a generator-private copy
of the same rule is a second authority, and it is deleted in the PR that lands
the diagnostic. The gallery then proves the rule the same way every creator's
library does.

## 12. Out of scope, named

The `sky`, `flatland` and `summit` bases (spec-0026, still Proposed); any
change to the surround generator or to the boundary clock; retiring `areas[]`
(spec-0049 §6 leaves both authorities legal); the content work of §8; a
per-piece `horizon` declaration, refused on principle in §4.
