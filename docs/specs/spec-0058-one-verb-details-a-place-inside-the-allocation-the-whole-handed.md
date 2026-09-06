# spec-0058: One verb details a place inside the allocation the whole handed

- **Status**: Proposed
- **Ground**: engine `66906e39` (`origin/main` at the time of writing). The
  stage-6 chain measured here is the one `docs/reference/tools.md` §1, §2a and
  §3 document at that revision and the content repository's skill page §13
  walks: `delvec allocation`, `delve-grammar expand`, `delve-admit audit`,
  `delve-admit lighting`, a file move, a hand-written `details[]` row. The
  one-binary refactor (ADR-0023, `crates/delvec/`) is the shape this verb is
  mounted in; §5 states what this spec takes from it.
- **DSL**: the `details[]` row gains one optional field, `program` (§2.6). A
  document-shape change moves `dsl_version` by ADR-0024's rule — one minor —
  in the same change that lands the field (§7).
- **Grammar**: no new grammar-program surface. The handing enters a program
  through `params`, the outermost binding frame the language already has,
  under one reserved prefix (§2.3). No grammar-ledger movement.
- **Diagnostics**: `DW0841`, `DW0843`, `DW0844`, `DW0845` and `DW0848` are
  re-raised by this verb, each with the message it already prints at
  validation, because the verb runs the same check over the same object.
  **`DW0882`** is allocated to the one refusal that has no existing name: a
  program that asks for a handed value the whole does not hand (§3).
- **Non-goals**: several pieces per place or a half-derived place (spec-0050
  §17, unchanged); a seam-agnostic program that serves any place unedited
  (§2.3 says how composition reaches it); jigsaw connectors; any change to what
  `delvec allocation` prints.

## 1. Every input of the chain, classified

The creator is an agent, and every input to a surface is either a **creative
judgement** (the agent's argument) or a **procedural derivation** (handed by
the tool, never typed). The stage-6 chain at the ground revision, per place:

| # | Input | Where it is entered today | Class | Under this spec |
|---|---|---|---|---|
| 1 | The place id (`node/<kebab>`) | `delvec allocation <place>` argument, then `--id`, then the row | procedural | the one argument of the verb; the tool derives everything else from it |
| 2 | The frame's extents | read off the allocation, re-typed as `--region WxHxD` | procedural | the verb expands at the frame it computed; nothing is typed |
| 3 | The datum (walk plane, piece-local `y`) | read off the allocation, honoured by hand | procedural | handed into the program as `handed/datum-y` |
| 4 | Each seam's cells | read off the allocation, re-typed as `--param`s | procedural | handed as `handed/seam/<edge>/{x0,y0,z0,x1,y1,z1}` |
| 5 | Each seam's class, face and answering class | read off the allocation, honoured by hand | procedural | proved by `DW0844` over the expansion, before any write |
| 6 | Each seam's rise | read off the allocation, re-typed | procedural | handed as `handed/seam/<edge>/rise` |
| 7 | The owed anchor names | read off the allocation, honoured by hand as `mark` stems, then re-typed into `anchors` | procedural | a program answers an owed name by marking its stem; the verb writes the `anchors` map |
| 8 | The whole's palette | read off the allocation, re-typed as `--role` | procedural | bound by the verb: each `role/<stem>` of the plan's `palette` that the program declares as `<stem>` is rebound |
| 9 | The piece id | typed as `--id`, then re-typed into the row | procedural | `prefab/<campaign_id>-<place stem>` |
| 10 | The output directory | `-o <dir>`, then a file move into the prefab directory | procedural | the campaign's prefab directory (`--prefabs`), written directly |
| 11 | The `details[]` row | hand-written | procedural | written by the verb |
| 12 | The expansion seed | `--seed` | procedural | derived from the place id (§2.4) |
| 13 | The lighting measurement | `delve-admit lighting --write` | procedural | taken by the verb and written into the piece's metadata |
| 14 | The `footprint_class` claim | absent from a grammar export | procedural | stamped from the node's `size_class` where the graph declares one |
| 15 | The program — its rules, splits, claims and marks | a JSON document the agent writes | **creative** | `--program <path>`, the artifact of record |
| 16 | The program's palette (its own material choices inside the whole's vocabulary) | the program document | **creative** | unchanged |
| 17 | The per-place free choices inside the frame — partitions, lofts, pits, furniture | the program document | **creative** | unchanged (spec-0050 §7, "free to change") |
| 18 | The style anchor the program is written against | a render, judged rank-only | **creative** | unchanged; no gate reads it (spec-0050 §10.4) |
| 19 | The verdict on the render | the agent's and the owner's eyes | **creative** | unchanged |

**Fourteen procedural, five creative.** Every procedural input above is today
typed at least once, and eight of them (2, 3, 4, 6, 7, 8, 9, 11) are typed
*twice* — once into the expander, once into the row — with nothing comparing
the two copies until `DW0843`/`DW0844` refuse after the piece exists. Under this
spec a procedural input is never a flag the creator types: the verb's only
inputs are the campaign, the place and the program.

## 2. The verb: `delvec detail`

Named by the object it acts on — the stage is "detail per place" (ADR-0022
§2, stage 6) and the document is the `detail-plan` — and mounted as an ordinary
`delvec` subcommand beside `allocation`, exactly as `delvec grammar …` and
`delvec prefab …` are mounted (ADR-0023).

```
delvec detail <campaign-dir> <place> --program <path>   [--prefabs <dir>]
delvec detail <campaign-dir> --all                      [--prefabs <dir>]
```

### 2.1 What it reads

The campaign directory, as `validate` reads it; the walk record beside it;
the prefab directory the campaign builds with (`--prefabs`, the global flag,
which is what makes the written piece land where the build looks); the
program document, resolved through its `include` list by the grammar loader
exactly as `delvec grammar expand --file` resolves it.

### 2.2 The order of operations, per place

1. **The allocation**, computed from the site plan by the same function
   `delvec allocation` prints from. Its gate is the same gate: no passed,
   fresh walk record → `DW0841`, and nothing further happens. A place the plan
   allocates no box to is refused in the words `allocation` uses.
2. **The handing is bound into the program** (§2.3). A program that declares
   a handed name the allocation does not hand → `DW0882`.
3. **The expansion**, at the frame's extent, at the derived seed (§2.4), with
   the palette bound (§2.5). An expansion the program cannot make at that
   frame — an absolute split past the box, a rule with no applicable
   alternative — is the grammar's own refusal, printed with the place and the
   program named, and nothing is written.
4. **The grammar gates** (`gates::judge`, every contract obligation of
   `docs/reference/grammar.md` §2d, reachability included), exactly as
   `delvec grammar expand` runs them. A red writes nothing.
5. **The piece, in memory**: the export (`export_zone`) with `footprint_class`
   stamped from the node's `size_class` where the layout graph declares one,
   and the lighting profile measured by the admission probe under the sky the
   piece's own contract claims (`light::SkyClaim::of`, the one rule
   `delvec prefab lighting` reads).
6. **The bindings check, on the in-memory piece and the row this run would
   write**: `compiler::detail::check` over a registry holding the library plus
   this piece — so `DW0843` (a contractless piece), `DW0844` (both directions,
   naming the seam and the face), `DW0845` (an owed name no mark answers) and
   `DW0848` fire here, in the words they print at validation, **before any
   file is written**.
7. **The admission audit** (`audit::audit`, the palette and licence gate),
   red → nothing written.
8. **The write**: the structure file(s) and metadata into the prefab
   directory; the gate report as `<id>.report.json` beside them (§2.7); the
   `details[]` row into `detail-plan.json` in canonical form, creating the
   document when the campaign has none.
9. **Traversal equivalence against the blockout**: the campaign is validated
   and built in memory — `Plan::build` and the emission battery, the same
   observers `delvec build` runs (`DW0836`–`DW0838`, the brief's identities,
   the pacing measurement) — and the verdict is printed with the place named.
   This runs once per invocation, after every place of the run is written,
   because the battery is a property of the whole. A red here is the state
   `delvec build` would report and is left standing, not rolled back: the
   broken intermediate is a real, lookable object (spec-0050 §1), and the
   creator repairs the program and re-runs the verb.

`--all` walks every `details[]` row that names a `program`, in site-plan box
order, through steps 1–8, stopping at the first refusal with the place named,
then runs step 9 once. A row with no `program` — a kit piece bound by hand —
is not touched. `--all` over a campaign with no program row is refused as a
zero binding, not reported as a pass.

### 2.3 The handing, bound: the `handed/` prefix

A program reads a handed value through the language's own outermost binding
frame, `params`, under one reserved prefix. The names, all integers:

| Name | Value |
|---|---|
| `handed/datum-y` | the walk plane's piece-local `y` (the allocation's `datum_y`) |
| `handed/seam/<edge stem>/x0`, `y0`, `z0`, `x1`, `y1`, `z1` | the seam's answering cells, piece-local, inclusive, as the allocation prints them |
| `handed/seam/<edge stem>/rise` | the seam's rise, signed from this place |

`<edge stem>` is the layout-graph edge id without its `edge/` prefix. The
prefix uses `/` because that is how the language already qualifies a name
(`compose::include` prefixes rules, parameters and roles the same way).

**A program declares the handed names it consumes**, with a default — and the
default is what lets the same document expand standalone under `delvec grammar
expand` for the render loop, at the values the agent read off the allocation.
Under `detail` the verb sets every declared `handed/…` parameter from the
allocation; a declared `handed/…` name the allocation does not hand — a seam
this place does not have, a misspelling — is refused (`DW0882`) naming the
parameter and listing every name the allocation hands. A handed value the
program does not declare is not a refusal: a program may hard-code a cell,
and the price of that is paid the day the plan moves it (`DW0844`, §4).

A seam is keyed by its edge rather than by its face and an ordinal, because
the edge is the campaign's stable identity for the connection: a plan edit
that moves a seam along a face keeps the program's binding, and one that moves
it to another face is refused by `DW0844` naming the face. A program meant to
serve several places is written once and `include`d by a thin per-place
document that binds the edge-keyed names — the composition surface that
exists for exactly this (`grammar.md` §5c).

### 2.4 The seed

Derived from the place id — the first eight bytes of SHA-256 over the node id,
read big-endian — and recorded in the piece's provenance row like any seed.
A derivation, not a judgement: two places detailed from one included program
draw different texture; the same place draws the same texture on every
regeneration; no flag exists to re-roll.

### 2.5 The palette

The plan's `palette` is `role/<stem>` → block state. For each entry whose
`<stem>` the program declares as a palette role, the verb rebinds the role as
`--role` would — a restyle that keeps the binding's axis frame. An entry the
program does not declare is not applied and not a refusal: the whole's
vocabulary is handed, never gated (spec-0050 §4, §10.4). A program role the
plan does not name keeps the program's own paint.

### 2.6 The row, and the one new field

The verb writes `{place, piece, anchors, program}`:

- `place` — the argument.
- `piece` — `prefab/<campaign_id>-<place stem>`. Procedural, so two campaigns
  sharing one prefab directory cannot collide on `prefab/annex`.
- `anchors` — for every owed name (`dsl::owed_anchors`), the piece anchor
  `anchor/<stem>` where the stem is the owed name with a leading `anchor/`
  removed (`spawn` → `anchor/spawn`). A program answers an owed name by
  marking that stem; an owed name no mark answers is `DW0845`, raised at step
  6 in the words validation prints.
- `program` — **new, optional** — the program's path relative to the campaign
  directory. This is what makes regeneration creator-free: `--all` reads it.
  A program outside the campaign directory is refused where entered, because
  the row could not name it. A row written by hand for a kit piece leaves the
  field absent, and the schema's shape is otherwise unchanged: still no
  coordinate, no region, no extent, no datum, no seam, no offset.

An existing row for the place is replaced. The document is written canonical
(`delvec fmt`'s form), so a re-run that changes nothing moves no byte.

### 2.7 The report, and the prefab directory

`delvec grammar expand -o <dir>` writes `<id>.report.json` beside the piece,
and `PrefabRegistry::load_dir` reads every `*.json` as metadata, so the
expander's output directory is refused by the compiler (`DW0346`). **One
rule: a file named `*.report.json` is not prefab metadata and the registry
skips it by name**, as it already treats `pools.json` by name. `detail` writes
its report beside the piece under the same rule. The alternative — the report
outside the prefab directory — was not taken because it would leave the
`expand` output directory refused for every kit piece made that way.

### 2.8 What is printed

Per place: the piece id, the frame, every handed name bound with its value,
the seams answered (count over count), the owed names bound (count over
count), the lighting profile and its binding, and the files written. Then the
whole's verdict (step 9) with its binding lines. `--json` carries the same as
one object per place. Zero seams, zero owed names and zero measured light cells
are each printed as the number they are, and the last is a refusal in the
probe's own words.

## 3. Refusals, where entered

| Refusal | Code | Fires at | Files written |
|---|---|---|---|
| No walk record, or a stale one, or `findings` | `DW0841` | step 1, before the program is opened | none |
| The place has no box in the plan | (the words `delvec allocation` prints) | step 1 | none |
| The program is outside the campaign directory, or does not load | (the grammar loader's own words) | step 2 | none |
| A `handed/…` parameter the allocation does not hand | **`DW0882`** | step 2 | none |
| The program cannot expand at the frame | (the expander's own words, place and program named) | step 3 | none |
| A contract obligation fails | (the gate's own name and words) | step 4 | none |
| The program declares no contract | `DW0843` | step 6 | none |
| A seam no face answers; a face answering no seam | `DW0844`, naming the seam and the face | step 6 | none |
| An owed name no mark answers | `DW0845` | step 6 | none |
| The stamped `footprint_class` disagrees with the bytes | `DW0848` | step 6 | none |
| The palette audit reds | (the audit's own words) | step 7 | none |
| The whole's battery reds | `DW0836`–`DW0838` and the rest, as `build` prints them | step 9 | the piece and the row stand; the verdict names the place |
| `--all` finds no program row | (a zero binding, stated) | before step 1 | none |

`DW0882`'s message names the parameter, the place, and every handed name the
allocation offers this place, so the repair is a rename in the program and
never a number.

## 4. Regeneration

After a site-plan or layout-graph edit and a re-walk, `delvec detail --all`
re-details every program-bound place with no creator input: the frame, the
seams, the datum, the owed names and the palette are recomputed and rebound;
the seed is the same; the row is rewritten. A program whose frame moved and
no longer fits is refused by name — the expander's overflow at step 3, or
`DW0844` at step 6 naming the seam and face that no longer align — and the
creator edits the program, never a number, because there is no number to
edit: the row still has no coordinate in it.

## 5. The dependency direction, preserved

spec-0050 §10.3 and §4: the compiler crate never depends on the grammar back
end, and the handing is computed on demand, proved by recomputation, never
copied. Both hold:

- The verb lives in the **binary** (`crates/delvec`, ADR-0023), which already
  depends on every library crate. `delvewright-compiler` gains no dependency;
  it exposes what it already has (`detail::allocation`, `detail::check`,
  `Plan::build`, the emission battery) and the verb composes them with the
  grammar's `expand`/`judge`/`export_zone` and the admission crate's audit and
  light probe. The compiler still consumes a frozen piece and reads nothing
  of the program.
- The allocation is still an input to nothing on disk: the verb computes it
  in memory and binds it into an expansion; what reaches the build is a
  frozen piece plus a row, and every obligation is recomputed from the plan at
  every validation exactly as before. A committed allocation file remains a
  copy with no consumer.

## 6. Gallery, probes, baseline

- **The element**: the site-plan overlay details a second place through the
  verb — `node/annex`, a 16×5×16 frame with two `walk` seams on its east face
  and one owed name — from a program committed under the overlay at
  `programs/annex.json`. The row carries `program`, which is the new coverage
  unit (`Detail.program`), bound. The piece is generated at build time, never
  committed, by running the verb over the materialised point before the
  build, in the same step that runs the gallery generator. `node/exit` keeps
  its hand-built kit piece, so both ways a place is bound stay demonstrated.
- **Probes**, in the primary-plus-one-edit form: a program declaring
  `handed/seam/no-such-edge/x0` (`DW0882`); a program whose east openings sit
  one cell off the handed seam (`DW0844`); a program with no `mark` for
  `node-annex` (`DW0845`). Each is refused by the verb with the named code,
  and the coverage gate reports 0 units in neither state before and after.
- **Baseline**: a detailed gallery place moves emitted bytes; the baseline
  regenerates with every moved row attributed to this element. A detailed
  gallery place is player-facing and enters the owner's playtest batch.
- **Demo row**: `docs/demo-levels.md` gains the mechanic's row — a two-place
  plan, one place detailed by program, regenerated after a plan edit.

## 7. Version discipline

- The `details[]` row gains `program` (optional). Under ADR-0024 the engine
  accepts one `dsl_version`; a document-shape change bumps it by one minor
  and moves every document this repository holds in the same change. Where the
  change lands before ADR-0024's implementation, the same field lands under a
  per-stage fence at the next free number, allocated by the planner.
- Prefab metadata: unchanged. `footprint_class` and `lighting` are existing
  fields the verb fills.
- Grammar documents: unchanged; `handed/…` is a naming convention over
  `params`, fenced by nothing because it adds no construct.

## 8. The skill page, §13, as this spec leaves it

Applied by the content round when the authoring pin moves; drafted here so the
two surfaces are written against one another.

> ## 13. Detail — site-plan campaigns only, and only after the walk
>
> Optional. A blockout is walkable and legible and made of concrete; detailing
> replaces one place's massing with a real building, one place at a time —
> every unbound box is still massed, so the map builds, walks and renders at
> every point between none detailed and all of them.
>
> 1. **Record the walk the user did at step 9.** `walk-record.json`, its
>    shape from `delvec schema --stage walk-record`, its four values copied
>    out of the build output. Nothing about detail runs without it
>    (`DW0841`); editing the plan or the graph re-opens it.
> 2. **Read the allocation**: `delvec allocation <campaign-dir> <place>`. It
>    is what the whole hands the place — frame, datum, every seam with its
>    cells and the face class that answers it, the owed anchor names, the
>    palette. Read it; write nothing from it by hand.
> 3. **Write the program** at `programs/<place stem>.json` inside the
>    campaign. Declare the handed values you use as parameters under the
>    `handed/` prefix, with the allocation's values as their defaults;
>    answer each seam with an opening at the handed cells and a contract edge
>    to `exterior`; answer each owed name with a `mark` of that stem; declare
>    a spatial contract. Iterate with `delvec grammar expand --file … --region
>    <the frame>` and a render until it reads as the place.
> 4. **Run one verb**: `delvec --prefabs prefabs detail <campaign-dir> <place>
>    --program programs/<place stem>.json`. It binds the handing, expands,
>    runs every gate, writes the piece into `prefabs/` and the row into
>    `detail-plan.json`, and builds the whole to prove the map still walks.
>    Read the verdict. A refusal names what to change in the program; there is
>    no number to edit anywhere else.
> 5. **After any plan edit and re-walk**: `delvec --prefabs prefabs detail
>    <campaign-dir> --all`. Every program-detailed place is re-made; one that
>    no longer fits is refused by name.
>
> Only when `details[]` binds every node does a declared vista stop being an
> advisory and become a refusal (`DW0821`).

## 9. Acceptance criteria

Machine-checkable; each names its instrument.

1. **Classification.** §1's table has a row for every flag, argument and
   hand-written field of the chain at the ground revision (`tools.md` §1
   `allocation`, §2a `expand`, §3 `audit`/`lighting`, the skill's §13.1–13.4),
   each classified; the verb's argument list contains no procedural input —
   asserted by `delvec detail --help` naming exactly `campaign-dir`, `place`,
   `--program`, `--all` beside the global flags.
2. **The verb.** On the gallery site-plan point with its walk record fresh,
   `delvec detail <point> node/annex --program programs/annex.json --prefabs
   <dir>` exits 0, writes `gallery-annex.nbt`, `gallery-annex.json` and
   `gallery-annex.report.json` into `<dir>`, writes the `node/annex` row with
   `program: "programs/annex.json"`, and a following `delvec build` exits 0
   with the battery's binding lines. Running it again moves no byte of the
   piece or the document (`ADR-0006`, asserted by hash).
3. **Refuse where entered.** Each row of §3's table has a test that produces
   exactly that code (or, for the uncoded rows, the named words) and asserts
   that the prefab directory and `detail-plan.json` are byte-identical before
   and after, for every row marked *none*. `tools/check-dw-codes.py` is green
   in both directions with zero new allowlist entries; `DW0882` has a test and
   a probe.
4. **Same words.** The `DW0843`, `DW0844`, `DW0845` and `DW0848` messages the
   verb prints are produced by `compiler::detail::check` — one function, no
   private copy — asserted by the tests of 3 matching on the message text the
   validation-tier test of the same code matches on.
5. **Regeneration.** Editing one box extent of the gallery site-plan point
   (and re-recording the walk) then `detail --all` either re-fits every
   program-bound piece (exit 0, the piece's structure size equal to the new
   frame) or refuses naming the place and the seam or face — both branches
   pinned: an extent change along the seam-free axis re-fits; one that moves
   the east face's seams off the program's openings is `DW0844`.
6. **Count.** A 24-place fixture — a chain of boxes, one included program
   and 24 per-place documents binding their edge-keyed handed names — is
   detailed by **one** `detail --all` invocation; the test asserts 24 pieces
   written, 24 rows carrying `program`, and one build green. The same fixture
   under the ground revision's chain is 96 commands plus a 24-row document,
   recorded in the commit body.
7. **`*.report.json`.** A prefab directory holding `x.report.json` loads with
   zero `DW0346`; a directory holding `x.json` that is malformed still raises
   it — the skip is by the full suffix, not by any `report` substring.
8. **Gallery.** `tools/check-gallery-coverage.py` is green with
   `Detail.program` bound in the overlay and the three probes of §6 refused
   with their codes; the unit total and the zero-in-neither count are stated
   before and after in the commit body. `tools/gallery-baseline.py` is green
   with every moved row attributed.
9. **Docs.** `docs/reference/tools.md` carries the `detail` row and says of
   `allocation` that it is what `detail` reads and remains for reading;
   `docs/reference/compiler.md` carries `DW0882`, the `program` field and the
   `*.report.json` rule; the docs job and `tools/check-diagnostic-messages.py`
   are green.

## 10. Not settled here

- **Whether `program` becomes required** for every row — a kit piece bound by
  hand has no program and spec-0050 §1 admits it deliberately; decided if a
  campaign ever binds one.
- **A seam-agnostic program surface** (a program answering "the seams on my
  faces, whatever they are called"): composition reaches the 24-place case
  today (§2.3); a first-class surface waits for a campaign whose programs
  cannot be written that way.
- **Running the whole's battery per place** rather than once per invocation:
  a cost question (review C §D2) with no answer in the ground revision's
  data, revisited on the first campaign with more than twenty program rows.
