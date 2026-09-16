# spec-0070: Every approved image is answered

- **Status**: Draft
- **Ground**: the engine at `origin/main` `9e101878` (`delvec` 1.5.0, `dsl`
  0.28.0), read only. It carries the design record (`design.json`,
  `compiler::design`, `DW0890`, spec-0061) and the showcase camera record
  (`design/cameras.json`, `compiler::view::camera`, `delvec cameras`,
  `delvec place-camera`, the build's `DW0724` second shape, spec-0069). The
  ledger row this spec closes is `drill3-03` on the engine branch
  `chore/the-drills-findings-are-queued` at `367b39aa`, carrier `null`. The
  content checkout's `main` at `7318202` is read for §1's class count only.
- **What it is for**: the phase goal is a 20-plus-scene delve of unified
  appearance authored through `/new-delve` alone. The design gate approves a
  set of pictures; the build seats whatever the layout seats; the only place
  the two are ever in one hand is a person looking at a rendered view of the
  built world beside the approved picture. An approved picture with no such
  view is that goal's own failure mode, and today nothing refuses it.
- **Research**: every rule below is marked **cited** (a document in the tree,
  read at the ground revision) or **authored** (this spec chooses). No craft
  question is open here: what a showcase camera is and how it is found is
  `docs/reference/showcase-shots.md`, and this spec adds no rule to it.
- **Numbers**: `spec-0070` is the only number taken, verified free by
  `tools/next-numbered-doc.py spec` (571 refs examined, 69 numbers claimed,
  next free 0070) and by listing `docs/specs/` at every `refs/remotes/origin`
  ref (highest 0069; no ref mentions `spec-0070`). §5 needs **one** DW code,
  written `DW-ANSWER` until the planner allocates it. No `dsl_version` moves:
  neither record's shape changes.
- **Non-goals**: reading a picture by machine (§4); judging whether a camera
  is a good picture (`showcase-shots.md`); whether the frame was rendered and
  reached the storybook (the hand-over's rule); any change to `DW0890`'s
  comparison, to the camera record's fields, or to `spec-0028`'s ranking.

## 1. The defect, measured

**Cited**, from the ledger row and re-measured on the ground revision.

`design.json` records each approved image as a row: `name` (the file's stem
under `design/concept/` or `design/reference/`), `shows`, `time`, `weather`.
`design/cameras.json` records each showcase camera as a row whose `answers`
names the `design.json` row the camera is judged against. Between the two
records the engine holds exactly one direction: a camera's `answers` must name
a row (`compiler::view::camera::bind_answers`, refused as `DW0721` by
`delvec cameras`, `--preview` and `delvec place-camera`). The other direction —
a row must have a camera — is computed by the same function, returned as a
list, and printed by `delvec cameras` at the end of a successful emission:

```
answers: 1 of 6 approved image(s) in design.json have a camera; none answers concept/morning-quay, …
```

It is a report at exit 0. `--preview` prints nothing about it. `delvec build`
reads the record as a hashed input and proves every camera photographs the
scene (`DW0724`), and reads no `answers` at all. The staging gate reads
`validation/design-record.json`, which carries the rows and the files and no
camera.

On the gallery's committed documents at `9e101878` (instrument: the six
`content.references[].name` of `gallery/design.json` against the
`cameras[].answers` of `gallery/design/cameras.json`): **6 rows, 2 cameras,
1 distinct image answered, 5 rows unanswered** — `concept/morning-quay`,
`concept/dusk-rampart`, `concept/night-quay`, `concept/midnight-crypt`,
`concept/dawn-approach`. The ledger row measured the same point building at
`design record: 6 reference(s) recorded over 6 image file(s)` and
`delvec cameras --preview` exiting 0. `overlays/valley-site/design/cameras.json`,
the one overlay carrying its own record, is in the same state: 2 cameras, 5
unanswered. Deleting either record leaves `delvec build` at exit 0.

On the content checkout's `main` at `7318202` (denominator: 5 campaign
directories under `campaigns/`): 1 carries a `design.json` (`doune-castle-tour`, 18 rows over 18
image files) and 0 carry a `design/cameras.json`.

Three facts make it a class and not a slip:

- The approval and the camera are written by different steps of the page —
  step 4 writes the rows, and a camera is written after step 8, because every
  writer of a camera row consumes a built world (`--preview` reads a build's
  `render-plan.json`; `--report` reads a stamp fired in a running server).
  Nothing between those steps counts the rows against the cameras.
- The report line is printed by the one command a creator runs last, on the
  path that renders, after the scenes are written.
- The record is optional and its absence is legal at every tier, so the
  cheapest way to satisfy every gate that exists today is to have no cameras.

## 2. What "answered" is

**Authored**, in terms of the records that exist; every term is **cited** to
its reader.

- **An approved image is a row of `design.json`.** The row is the approval;
  the file under `design/` is what it approves; `DW0890` holds the two to each
  other one-to-one at validation (a row resolves to exactly one file, a file
  has exactly one row). So the population this rule quantifies over is the
  rows, in the design's own order, and it never re-reads the directory.
- **A camera answers a row** when its `answers` equals the row's `name`
  (`bind_answers`, the one function; this spec adds no second comparison).
- **A row is answered** when at least one camera of `design/cameras.json`
  answers it. One image may have several cameras (a near and a far view, a
  bracket pick beside a hand camera); one camera answers exactly one image —
  the field is a string, and spec-0069 §4.4 keeps a row's `answers` fixed.
- **Both directories count the same.** A `reference/` row is a view of the
  whole map and a `concept/` row is one scene; a camera answers either.
  `delvec panorama` prints its solved camera as a record line, so a whole-map
  view has a camera the moment its panorama is solved. A rule that quantified
  over one directory would be a second bespoke field.
- **What "answered" asserts**: a stated view of the built world exists for
  the picture. The build's `DW0724` second shape asserts that view photographs
  the scene (lens clear, inside the build height, ray meets the loaded
  extent); `delvec cameras` emits its scene; `delvec contact-sheet` and the
  storybook put the frame beside the picture. This rule is the first of those
  four and asserts nothing the later three assert.

What each surface holds today, and what it will hold:

| surface | today | after this spec |
|---|---|---|
| `delvec validate` | rows ↔ files ↔ skies (`DW0890`); binding line | the same, plus the camera count and the answered count on the binding line (§5) |
| `delvec build` | record parsed; every camera proven (`DW0724`); `answers` unread | a present record leaving a row unanswered is refused before anything is placed (§3, §5); a camera answering no row is refused under the record's own code (§5) |
| `delvec cameras` | camera → row refused (`DW0721`); row → camera reported at exit 0 | scene emission refused while a row is unanswered; `--preview` draws any record and prints the count |
| `delvec place-camera` | camera → row refused | the same, and prints the count after the write |
| `validation/design-record.json` | rows, files, skies | plus `cameras`, `answered`, `unanswered_rows` |
| staging gate | `drill3-01`: `references` (rows) | plus `drill3-03`: `answered` |

## 3. Which tier refuses, and what the creator can still do afterwards

**Authored**; the tree facts are **cited** by path.

`DW0890` refuses at validation on `DW0855`'s ground: a fact about documents
needs nothing placed. The answered count is the same kind of fact — two
documents, one comparison, under a second — and this rule still refuses at
the **build**. The deciding question is what the creator can do after the
refusal, and the answer differs by tier because of how a camera row is made.

**A camera row is written against a built world.** `delvec cameras --preview`
reads `<build-dir>/render-plan.json` and draws the record's cameras over the
assembled world; `delvec place-camera --report` reads the pose a person fired
inside a running server of a build; `--candidates` picks from a bracket
`delvec cameras` emitted from a build. The one row that needs no build is a
first estimate typed by hand, and it is refined by the same `--preview`.
Every view command validates the campaign first
(`main::load_for_view` → `validate_stage` → `validate_loaded`), so a rule that
refused at validation would refuse `--preview` on the record it is asking the
creator to complete.

| tier of the refusal | what the creator can still run after it | verdict |
|---|---|---|
| validation, exit 1 | nothing that reads the campaign: `analyze`, `build`, `snapshot`, `edit`, `cameras --preview` all go through the funnel. The only way out is to type every missing row blind and then look at them — a gate whose pass is a placeholder row | refused |
| **build, exit 3** | `validate`, `analyze`, `snapshot`, `cameras --preview` against the last built tree, `place-camera`, `contact-sheet`. The refusal names the pictures with no view, and the instrument that makes one is a command away | **this spec** |
| `delvec cameras` scene emission, exit 2 | the same set; the check runs where the record is read fresh, so a record edited after the build cannot emit a set with a hole in it | held as well, same function |
| staging, `UNBOUND` | everything; only the owner's hour and the hand-over are refused | holds the **absent** record (§6), which the build cannot |

**Where in the build.** After validation and analysis, before `Plan::build`
seats a piece and before anything is written under `-o` — the two documents
are already in hand at that point (`LoadedCampaign.design_files.cameras`,
`Campaign.design`), and a refusal there leaves the creator's previous build
tree on disk, which is what `--preview` needs. The `DwCode` declares
`ExitTier::Build` and means it.

**Why the build cannot hold the absent record.** The first build of every
campaign has no camera record: the record is made from that build. A build
that refused the absent record would refuse the state every campaign is in
between step 4 and step 8, and the only remedy would be the placeholder row
again. So the absent record is a measured zero at validation and at the build
(the binding line and the ledger say `0 of N answered`), and the event it is
refused at is the one that hands the build to a person: the staging gate,
which reads `validation/design-record.json` and reds a zero binding on a
campaign that declares the class (`drill3-01` is already that shape for the
rows themselves). This is the same division spec-0061 §7 draws for a campaign
with no approved design.

**The pair, read together.** The build's refusal names the remedy *write a
camera row against the last built tree*; the staging gate's refusal names the
same remedy. Neither refuses what the other prescribes. A creator who deletes
the record to get a build through arrives at the gate with `answered: 0` and
is refused there, so the hatch reaches nothing a person spends time on.

## 4. What the rule does not claim

**Cited**: spec-0028 §3, spec-0061 §12, `compiler::design`'s own header.

This rule holds that the approved picture and a view of the built world are
placed side by side. Whether they look alike is a person's reading, made at
the contact sheet and the storybook, and no machine's. `compiler::design`
opens no image bytes and no model reads a picture; that is standing, and this
rule keeps it: it reads two JSON documents and compares two lists of names.

## 5. What the diagnostic says

**Authored**, in the shape this project's diagnostics take: what it refused,
what it measured, a remedy that is reachable.

**One new code, `DW-ANSWER`**, for one fact — *an approved picture has no
view of the built world* — because its repair (write a camera) is unrelated to
`DW0890`'s (change the hour or the record) and to `DW0721`'s (fix the record's
own rules), and a creator looks a code up to find its repair. It is raised by
`delvec build` (exit 3) and by `delvec cameras` when emitting scenes (exit 2),
one code in two tiers, exactly as `DW0721` is raised by both today.

**The camera side at the build.** A camera whose `answers` names no row is
refused by `delvec cameras` and `delvec place-camera` today and accepted by
the build, whose showcase proof (`camera::prove_showcase`) parses the record
and never calls `bind_answers` (by reading; the implementing round measures
it). The build will hold the record to `design.json` through the same call,
under `DW0721`, as the remaining rule of the record it already enforces there.
A checker reads a document the way its consumer reads it; the build consumes
the record and will read all of it.

**The binding line**, appended to the design gate's line on every `validate`,
zeroes included:

```
design record: 6 reference(s) recorded over 6 image file(s) under `design/` (…) (DW0890);
  showcase cameras: 2 in design/cameras.json answering 1 of 6 approved image(s) (DW-ANSWER)
```

With no record: `showcase cameras: none (no design/cameras.json); 0 of 6
approved image(s) answered`. `delvec cameras --preview` and `delvec
place-camera` print the same `answered k of n` line, so a creator placing
cameras sees the count move without running the build.

**The refusal** (build, exit 3; `cameras`, exit 2), one diagnostic naming
every unanswered row with its `shows` sentence, because the sentence is what
tells the creator which picture it is:

> `DW-ANSWER` design: 5 of 6 approved image(s) have no showcase camera:
> `concept/morning-quay` (the quay from off the water in early light, the
> lamp still lit), `concept/dusk-rampart` (…), … `design/cameras.json` holds
> 2 camera(s) answering `concept/noon-hall`. An approved picture nobody has
> pointed a camera at is a picture the build is never held beside. Either
> (1) AUTHOR a camera for each: estimate the view from the picture
> (`showcase-shots.md` §4) and draw it with `delvec cameras <last build>
> --campaign <dir> -o <dir> --preview`, then write the row (`source:
> estimated`), or place it by hand in the running game and write it with
> `delvec place-camera --report`; or (2) DELETE the picture and its row from
> `design.json`, which is re-opening the design gate for that scene, and is
> said to the user in those words. Never re-aim an existing camera at a second
> picture: a row keeps its `answers`, and a camera for another picture is a
> new row. With no built tree to preview against, DELETE the record, build,
> and write it against that build.

**Every remedy it names is reachable, and that is a test** (spec-0060 §10.3):
each move — write the missing row; delete the picture and its row; delete the
record and build — is a row in `crates/delvec/tests/remedy_reachability.rs`
reaching a different verdict, and `tools/check-dw-codes.py`'s cross-check
refuses a message naming a document as a move with no row.

**The ledger.** `validation/design-record.json` gains three keys, written on
every build:

```json
{ "references": 6, "image_files": 6, …,
  "cameras": 2, "answered": 1,
  "unanswered_rows": ["concept/morning-quay", "concept/dusk-rampart", …] }
```

`cameras` is the record's camera count, `0` when there is no record (an
empty record is refused by its reader, so `0` means absent); `answered` is the
number of rows with at least one camera; `unanswered_rows` is in the design's
order. A build that reaches the ledger with a present record has
`answered == references`, because the build refuses otherwise; the ledger's
job is the absent case, for the staging gate. `render-plan.json` does not
move.

**The staging row.** `docs/playtest-findings.json` row `drill3-03` takes
carrier `DW-ANSWER` and binding `{"kind": "artifact", "file":
"design-record.json", "path": "answered"}`, `applies_when` as `drill3-01`'s
(`world.json`, every campaign), so a zero is `UNBOUND` and refuses. Edited by
the pull request that lands the code, on the branch that holds the row.

**Tier and subject.** Raised in `compiler::design`, beside `DW0890`, from the
same `DesignFiles`; `Subject::Campaign`; `ExitTier::Build`.

## 6. The empty and absent cases

**Authored.** Each state a campaign can be in, and the verdict at each tier.

| state | `validate` | `build` | `cameras` | staging |
|---|---|---|---|---|
| no `design/` images, no `design.json`, no record | passes; line says `0 reference(s) … 0 of 0 answered` | passes; ledger `references: 0, answered: 0` | `DW0721`: no record, nothing to emit (today) | refused, `drill3-01` `UNBOUND` (today) |
| no `design.json`, a record present | passes; line counts the cameras | `DW0721`: every camera answers a row that does not exist — a camera is an answer to a picture | `DW0721` (today) | never reached |
| rows, no record — every campaign between step 4 and its first build | passes; `0 of N answered` | passes; ledger `cameras: 0, answered: 0, unanswered_rows: all` | `DW0721` (today) | refused, `drill3-03` `UNBOUND` |
| rows, a record answering every row | passes; `N of N` | passes; every camera proven (`DW0724`); ledger `answered: N` | emits | bound, `answered = N` |
| rows, a record leaving some unanswered | passes; `k of N`, the line names the count | **`DW-ANSWER`, exit 3, before placement** | scenes refused, `DW-ANSWER`, exit 2; `--preview` draws and prints `k of N` | never reached |
| a camera answering no row | passes; the line says the record names a row that does not exist | `DW0721` | `DW0721` (today) | never reached |

`--only <name>` on `delvec cameras` selects which scenes are written and
exempts nothing: the record is judged, and a record with a hole in it emits
no scene. `--preview` writes no scene and is the instrument that closes the
hole, so it runs on any record its reader accepts.

## 7. The gallery's obligation, and the page's

**Authored**, against `gallery/` at `9e101878`.

1. **The bound element.** `gallery/design/cameras.json` gains one camera for
   each of its five unanswered rows, and `overlays/valley-site/design/cameras.json`
   the same for its own five; every point of the domain then builds with
   `answered: 6` of `references: 6` in its ledger and `camera_eye_proof.showcase`
   counting every camera. A point whose world differs enough that a shared
   camera stands inside rock or looks at sky carries its own record, as
   `valley-site` does today; the record is a per-point document.
2. **The probe — the perturbation only this rule can catch.**
   `gallery/probes/a-picture-nobody-looks-at/probe.json` re-aims one camera:
   `replace /cameras/<i>/answers` from a row that camera alone answers to
   `concept/noon-hall`, a row already answered. Every camera still answers a
   real row (`DW0721` green), no lens moves (`DW0724` green, `showcase` count
   unchanged), `design.json` is untouched (`DW0890` green), the record's size
   is unchanged; only the answered count moves, six to five. It is refused by
   `delvec build` with `DW-ANSWER` and by nothing else. A demonstration probe
   (`units: []`): the record is not a schema unit. A probe may name
   `design/cameras.json` as its `doc` (`gallery_domain.materialise` applies
   a patch to any document of the primary).
3. **The stageable gate.** `tools/check-gallery-stageable.py` runs the staging
   gate over every point; `drill3-03` reads `BOUND` with `answered = 6` on each.
4. **The baseline moves** — three new ledger keys on every point — and is
   regenerated after the merge commit exists, never three-way merged.

No demo level is owed: nothing a player meets changes.

**The page's half, named and not done here.** The `/new-delve` page has no
step that writes a camera, and its step 12 asks the agent to name, per POV
frame, the approved image it answers. After this spec the page states, between
step 8 and step 9: *one camera per approved image, estimated from the picture
and drawn with `--preview`, before the walk* — because the walk's staging gate
refuses without them; and `references/tools-by-symptom.md` carries the
`DW-ANSWER` line. Those lines land with the release that carries the code
(spec-0069 §8.8's debt shape: `tools/check-skill-page.py` rule 4 holds every
`delvec` subcommand the page names to the pinned release).

## 8. Acceptance criteria

Machine-checkable; each names its instrument and its binding with denominator.
**Each was checked against the tree at `9e101878` before writing, and the
verdict is recorded per criterion**; a criterion the implementation cannot yet
satisfy is a debt, stated as such, never a pass.

1. **The measurement is on every `validate`.** The design gate's binding line
   carries `showcase cameras: <c> … answering <k> of <n> approved image(s)`
   (or `none (no design/cameras.json); 0 of <n>`) on every run, zeroes
   included, computed from `bind_answers` over the loader's record bytes.
   Instrument: `crates/delvec/tests/design_record.rs`, asserting the line's
   three numbers on a fixture in each of §6's states. Denominator: rows.
   *Checked: debt — the line ends at `(DW0890)`.*
2. **Red then green at the build, nothing placed, nothing written.** A fixture
   with two rows and a record answering one: `delvec build` exits 3 with
   `DW-ANSWER` naming the unanswered row and its `shows` sentence, before any
   placement line, and a tree previously built into the same `-o` is
   byte-unchanged; the same fixture with a second camera builds green with
   `answered: 2`. Both halves in one test. Instrument: `design_record.rs`.
   *Checked: debt — measured on the gallery, 5 of 6 unanswered, build exit 0.*
3. **The placing instruments are not refused.** On criterion 2's partial
   fixture, `delvec validate` exits 0, and `delvec cameras --preview` against
   the last built tree exits 0, draws the one camera, and prints
   `answers: 1 of 2 …`. Instrument: `crates/delvec/tests/hand_camera.rs`.
   *Checked: debt — `--preview` prints no count; validation passes today.*
4. **The absent record is measured at the build and refused at the gate.**
   With no record the build exits 0 and the ledger reads `cameras: 0,
   answered: 0, unanswered_rows` = every row; `tools/tests/test_staging_gate.py`
   drives a `drill3-03`-shaped row from fixtures to `UNBOUND` on that ledger,
   `BOUND` with `answered = references` on a complete one, and
   `MISSING-CHECK` when the key is absent. *Checked: debt — the ledger has no
   `answered` key; `drill3-03` is on its branch with carrier `null`.*
5. **The camera side at the build.** A record whose one camera answers
   `concept/a-view-nobody-drew` is refused by `delvec build` with `DW0721`,
   naming the camera and listing the rows, before placement. Instrument:
   `hand_camera.rs`. *Checked: debt — by reading `prove_showcase`, the build
   accepts it; the test is the measurement.*
6. **Scene emission is held.** On the partial fixture with a built tree and a
   world save, `delvec cameras` (with and without `--only <the one camera>`)
   exits 2 with `DW-ANSWER` and writes no scene; on the complete fixture it
   emits every scene and prints `answers: 2 of 2`. Instrument:
   `crates/delvec/tests/view_cli.rs`. *Checked: debt — exit 0 with the report
   line.*
7. **Every named remedy is reachable.** `remedy_reachability.rs` gains one row
   per move of §5's message, each reaching a different verdict;
   `tools/check-dw-codes.py` is green with the new code and zero new allowlist
   entries; every `DW-ANSWER` and new `DW0721` shape is asserted by a test.
   *Checked: debt — the file and the cross-check exist; no rows for this code.*
8. **The gallery binds it.** Every point's `validation/design-record.json`
   reads `answered: 6` of `references: 6` (denominator: the points enumerated
   from `gallery/baseline/manifests.json`, 8 today); `camera_eye_proof.showcase`
   is at least 6 on each; `check-gallery-stageable.py` reports `drill3-03`
   `BOUND` on each; the coverage line reports 0 in neither state.
   *Checked: debt — `answered` would read 1 of 6 on every point.*
9. **The perturbation.** The probe of §7.2 is accepted by `delvec validate`
   and refused by `delvec build` with `DW-ANSWER`; `tools/check-gallery-coverage.py`
   runs it and reds if it is ever accepted. A second, engine-side perturbation
   proves the probe measures this rule and no other: with the re-aim applied,
   `DW0721`, `DW0724` and `DW0890` are each asserted green on the same
   materialised point. *Checked: debt — no probe.*
10. **Determinism.** Two builds of the complete fixture and of the gallery
    are byte-identical, `validation/design-record.json` included (ADR-0006).
    Instrument: `design_record.rs`, `tools/gallery-baseline.py`. *Checked:
    debt for the new keys; the existing ledger is byte-stable.*
11. **The record.** `docs/reference/compiler.md` gains the `DW-ANSWER` row,
    the `DW0721` build shape, the three ledger keys under the `design` stage
    table and `DW0890`'s artifact, and the `cameras` section's refusal;
    `docs/reference/tools.md` §4a states the count every command prints;
    `docs/specs/README.md` carries this spec's row
    (`tools/check-numbered-doc-index.py`). *Checked: the index row is in this
    pull request; the rest is debt landing with the code.*
12. **The class on the content tree.** Denominator: campaigns on the content
    repository's `main` carrying a `design.json`. Every one merged after the
    page's half lands carries a record answering every row, checked by the
    content repository's per-PR `delvec build`, and the number is reported
    with its denominator. *Checked: 1 of 1 carries rows (`doune-castle-tour`,
    18) and 0 of 1 carries a record — recorded debt; a released campaign is
    never rebuilt by a later engine, so the class is empty on the current
    tree.*

## 9. Decisions

1. **The rule.** A campaign whose camera record leaves any approved picture
   without a camera does not build; a campaign with no camera record at all
   builds and is refused at the staging gate, before anyone walks it.
2. **What it refuses that builds today.** The gallery, on every point: its
   record answers one of six pictures, and the pull request that lands the
   code adds the five cameras. No campaign on the content repository's `main`
   carries a camera record, so none is refused at the build; `doune-castle-tour`,
   with eighteen approved pictures and no cameras, would be refused at the
   gate if it were staged again, and a released campaign is never rebuilt by a
   later engine.
3. **What it does not do.** No picture is read by any machine. The rule says a
   view exists for every approved picture; whether the view looks like the
   picture stays a person's judgement at the contact sheet and the storybook.
4. **What the creator owes, and when.** One camera per approved picture,
   estimated from the picture and drawn with `--preview` after the first
   build and before the walk; a hand camera replaces an estimate on the same
   row, as spec-0069 already has it.
5. **Numbers.** One DW code, allocated by the planner. No `dsl_version` moves.
