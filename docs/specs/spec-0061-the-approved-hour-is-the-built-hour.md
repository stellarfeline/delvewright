# spec-0061: The approved hour is the built hour

- **Status**: Proposed
- **Ground**: written against engine `ba461b28` (`origin/main`; `delvec` /
  `dsl` 0.21.2) and the content repository at `8e09c30c` (its `main`), read
  only. The defect's instance is on the content branch `drill/page-walk-3`
  (`the-tidewatch`); its ledger row `drill3-01` is on the engine branch
  `chore/three-open-general-forms-are-queued`, carrier `null`. The page's
  present answer is on the content branch `docs/the-page-survives-a-second-run`
  (unmerged). Every count below names its instrument.
- **Diagnostics**: this spec needs **one** DW code, for the design record's
  four refusal shapes (§6). It is written `DW-SKY` below until the planner
  allocates it; no number is taken here. Spec number 0061 was verified free
  across all 88 engine remote refs (file names and text) at writing.
- **Research**: `docs/reference/approved-hour.md` — what established practice
  records, per rule cited or authored. §2 and §4 rest on it.
- **Non-goals**: reading a picture by machine (§12); the medium of the design
  walkthrough; binding an approval to image bytes; the `horizon` property
  (§8); any edit to the page, the ledger or the content repository — §10 names
  that work and stops.

## 1. The defect, as the record has it

**Cited**, from the drill record (`O-17`, `O-21`, §3 Gate B) and the ledger
row.

The third end-to-end drill authored `the-tidewatch`: four reference images
approved at step 4, every one a night scene under the style note *"cold night
palette … moonlight as the only warm value"*. `world.json` declared no `time`;
absent is `noon`. The build, the machine ladder and 36 emitted Chunky scenes
all ran under a blue noon sky, and the first thing that said so was the first
point-of-view frame at step 12 — after a ~1 min build and at a render cost of
299 s per frame on ten cores (`O-18`, measured on one scene at its own
`sppTarget: 500`).

Three facts make it a class and not a slip:

1. **The order of the page runs backwards from the decision.** `time` and
   `weather` sit in `world.json`, written at step 1; the art whose hour they
   must match is drawn at step 4. Nothing between the two compares them,
   because the reference image is style authority for a human and no check
   reads a picture.
2. **The default is a design decision inside a mechanism.** `WorldContent.time`
   is `Option<WorldTime>` with `Noon` as `#[default]`, and environment sealing
   emits `time set noon` for a campaign that said nothing (`compiler.md`,
   *Environment sealing*). "This delve is played at noon" is not a mechanism;
   it is what `CLAUDE.md` forbids a primitive from encoding. spec-0060 §4.1
   made the same ruling for `walk_y`: *a default is the global datum wearing a
   different name*.
3. **The obvious check binds to nothing.** A check reading an environment out
   of `design/README.md` and comparing it with `world.json` quantifies over
   campaigns that carry a `design/` directory. On the content repository's
   `main` there are **2** campaigns (`campaigns/*/world.json`) and **0** carry
   `design/` — measured by `git ls-tree -r origin/main`. Files under
   `campaigns/*/design/` exist on 18 of the repository's 84 remote refs, all
   unmerged. And the page's present remedy — record the approved environment in
   `design/README.md` "as the `time` and `weather` tokens `world.json` carries,
   not as prose" — is a page instruction, which is not a binding, over a prose
   file, which is not a document the engine reads.

The two measurements that key off the declared hour are the reason this is
more than a picture being wrong: `DW0210` judges every sky-open cell under the
darkest reachable `(time, weather)` — a night delve declared as noon has never
had its darkness measured — and `DW0496` stands only while the hour is a
pinned clear daytime one, so the same delve was also protected by a
daylight-burning gate whose premise was false.

## 2. What records the approval

**Authored**, against the rules in `docs/reference/approved-hour.md` §3.

The approval produces exactly one machine-readable artifact: the **design
record**, `design.json`, a stage document in the campaign's own envelope
(`stage: "design"`), holding one row per approved reference image.

```json
{
  "dsl_version": "0.21.2",
  "campaign_id": "the-tidewatch",
  "stage": "design",
  "content": {
    "references": [
      { "name": "concept/shore-far",
        "shows": "the Tidewatch on its headland from off the water, lamp window dark",
        "time": "night", "weather": "clear" },
      { "name": "concept/tower-near",
        "shows": "eye height in the lamp room, the cold lamp against the seaward window",
        "time": "night", "weather": "clear" }
    ]
  }
}
```

- **`name`** is the image's path stem relative to `design/`, under `concept/`
  (one scene) or `reference/` (a view of the whole map). The name resolves to
  exactly one file there; two files with one stem and two extensions are a
  candidate, not a match, and are refused.
- **`shows`** is one sentence, agent-facing (the page already demands it for
  `design/README.md`); it is not player-visible and is not l10n-inventoried.
- **`time`** and **`weather`** are the sky the picture was drawn in, typed as
  `WorldTime` and `WorldWeather` — the same two enums `world.json` uses. One
  authority for the vocabulary; a new hour added to the world is an hour a row
  can state, with no second table.

**Who writes it, and when.** The creator, at step 4, in the act that follows
the user's explicit yes: the image and its sidecar are copied to `design/`, and
the row is written beside it, the sky read off the picture. The tokens are a
creative judgement in the constitution's sense — the agent's reading of the
picture — and are the only input on this surface that is; everything else the
check needs it derives.

**What makes it authoritative rather than decorative.** Four things, none of
them a sentence on a page:

1. `delvec validate` refuses a world whose reachable skies disagree with the
   rows (§5, §6).
2. `delvec validate` refuses an approved image with no row and a row with no
   image — the record and the directory are held to each other, in both
   directions, the way `DW0887` holds a waterline to the bytes.
3. The staging gate refuses a campaign whose record has no rows (§7): a build
   the owner walks carries an approved design or is not staged.
4. `DESIGN.md` stays the prose design record for humans; `design.json` is its
   machine half, and the page's `design/README.md` sentence about tokens is
   retired the moment this lands (§10) — one authority for the sky.

**What was rejected, and why.**

- *Tokens in `design/README.md`* (the page's present instruction): prose, read
  by no tool, parsed only by a private rule a gate would have to keep, bound to
  0 of 2 campaigns, and a page line rather than a refusal.
- *The `refimg.py` sidecar*: it is the provider's response to a **draft**
  request, written before any approval exists, absent on Init path A (a
  campaign that arrives with its design already approved), and its `prompt`
  records what was asked for, not what came back and was approved. It stays
  what it is — the re-issue record of an image — and the design record points
  at the image by name, not at the sidecar.
- *One `environment` for the whole set*: a design whose finale is a sunrise
  has two skies, and a set-level field states one. Per-image rows are the
  general form (`approved-hour.md` §3.2), and they cost nothing more than the
  page already asks the creator to write per image.
- *Reading the picture*: no machine here reads a reference image, and a
  classifier would be a measurement with an unstated error rate standing where
  a refusal belongs — the shape spec-0028 §3 already forbids for `refscore`
  (a score ranks, it never gates).

## 3. Why the binding is not zero

**Authored.** The record is carried structurally, by three refusals that do
not ask a creator to remember anything:

1. **An image under `design/concept/` or `design/reference/` with no row is
   refused** (§6, shape c). The page makes approved images campaign files; a
   campaign that keeps its approved images therefore carries the record, or
   does not validate.
2. **A campaign whose record has no rows is not staged** (§7). The walk at
   step 9 is the first thing a creator's campaign is built *for*; it cannot
   happen without an admission token, and the token is refused on zero rows.
3. **The hour has no default** (§4). Every campaign states `time` and
   `weather`, so the comparison always has a world side; the record supplies
   the art side.

The object class is *approved reference images*, and on the current tree it
is empty: **0 of 2** campaigns on the content repository's `main` carry a
`design/` directory or a design record, and neither of the two builds on this
engine (`dsl_version` 0.3.0 and 0.6.0 against 0.21.2). **This is a recorded
debt, stated in those words**: every criterion in §11 that quantifies over the
class binds to zero until the first campaign authored through the page after
this lands. The first member is named — the drill campaign re-made through
`/new-delve`, whose four approved images all state `night` (§11.11) — and the
gallery carries the surface from the day it exists (§9), so the engine's own
proof never waits on content.

## 4. The hour has no default

**Authored**, on the precedent of spec-0060 §4.1 and the constitution's
general-engine rule.

`time` and `weather` become **required** fields of `world.json`. `Option` is
removed from both on `WorldContent`; the schema marks both required; a campaign
that omits either is refused the way every required field is (the schema
refusal, `DW0100`), not by a new code. `DW0874`'s stub recipe names both.

What does **not** change: the emitted bytes. `time set <kw>` was always
emitted; `weather <kw>` is emitted only for a declared non-`clear` weather,
because `clear` is vanilla's own state (`compiler.md`, *Environment sealing*),
and that rule stands. A campaign that already declares both builds
byte-identically. `DW0210` and `DW0496` read the same `reachable_time_weather`
scan as before (`compiler::light`), which now has no `unwrap_or_default` to
perform.

The cost, measured at `ba461b28` by opening every `world.json` under
`crates/`, `prefabs/` and `gallery/` outside `target/`: **41** world documents,
**34** declare neither field, **7** declare both (the gallery and its five
overlays among them). The 34 gain two lines each, mechanically; every world
document the engine itself writes (`delvec metrics --gym`, the gallery
generator's overlays) writes both. The two shipped content campaigns are not
touched: nothing owes compatibility to anything already built.

## 5. Which tier refuses, and what being wrong there costs

**Authored.** The refusal is at **validation, exit 1** — `delvec validate`,
and therefore everything built on it.

The reason is the one `DW0855` states in its own implementation note
(`crates/dsl/src/validate.rs`): *refused here rather than at the build,
because it is a fact about the documents: nothing has to be placed to know
it.* The rows are a document; the world's reachable skies are computed from
documents (`compiler::light::reachable_time_weather` — the initial state plus
every `set-time` / `set-weather` target, quest and dialogue effects both, every
root and every depth); the image files are names in a directory. Nothing in the
comparison needs a placed piece.

What each tier would have cost on the drill, from the drill's own timings:

| where the disagreement is caught | what has been spent when it is | drill measurement |
|---|---|---|
| the design step (a report beside the picture) | the argument | — the page's half (§10); a report, never a refusal, because the human is still choosing |
| **validation** | one `delvec validate` run | under a second (`fmt` and `analyze` on the same campaign: <1 s each) |
| build | placement, the solver, the light and nav proofs | ~1 min, 7 placed pieces |
| render | the build plus review frames | 299 s × 10 cores per frame, 36 scenes emitted, 82 min of a 109 min run |

A false refusal at validation costs one field edit on whichever side is wrong;
a false pass at validation is what the drill paid. Refusing at the design step
instead is refused on principle: the design is still being decided there, and
a refusal would fire on a record the human has not yet said yes to. So the
design step **reports** — the page prints each row's tokens beside its image
in the walkthrough, which is the content repository's half (§10) — and the
first place a disagreement is a *refusal* is the first `validate` after the
record is written, which on the page is step 5, before any content document
exists.

## 6. What the diagnostic says

**Authored**, in the shape this project's diagnostics take: what it refuses,
what it measured, and a remedy that is reachable. One code, four shapes, each
with its own message; every shape states the same binding line.

**The binding line, printed on every run whether or not it refuses:**

```
design record: 4 reference(s) recorded over 4 image file(s) under design/
  (concept/ 4, reference/ 0); skies stated: night+clear ×4;
  world reaches times {night} weathers {clear}
```

A run over a campaign with no `design/` and no `design.json` prints
`0 reference(s) recorded over 0 image file(s)` and does not refuse: that is a
measured zero of an optional surface, and staging (§7) is where a zero of this
class is a red.

**Shape a — the world reaches a sky no approved picture shows** (the drill's
shape, inverted: the rows say `night`, the world says `noon`):

> `DW-SKY` world: the approved design is drawn under a sky this world never
> reaches. 4 of 4 recorded reference(s) state `night`+`clear`; `world.json`
> declares `time: noon`, `weather: clear`, and no `set-time` reaches `night`,
> so every hour the party can be in is one nobody approved a picture of.
> Measured: world times {noon}, weathers {clear}; stated times {night},
> weathers {clear}. Either (1) declare `time: night` in `world.json` — the hour
> the approved set was drawn in — or (2) re-approve the design under `noon`
> and change the rows to say so. Never move the hour to satisfy a mob
> (`DW0496`'s rule).

The same shape fires the other way round — a `set-time dawn` on the finale
with no row stating `dawn` — with the remedies: approve a picture of the dawn
scene and add its row, or remove the effect. Both directions are one shape
because the fact is one: the set of skies the world reaches and the set the
approved pictures state are not equal. The two sets are compared **as the two
independent sets the engine already keeps** — times and weathers, not pairs —
because that is what `reachable_time_weather` returns and what `DW0210` and
`DW0496` read; a beat timeline that would make pairs meaningful is not
modelled, as `DW0496` says in its own row.

**Shape b — a row names an image that is not there:** `design.json` row 3
names `concept/tower-far`; nothing under `design/concept/` has that stem (files
present: …). Remedy: copy the approved image in, or delete the row. A row whose
stem resolves to two files (`tower-far.jpg` and `tower-far.png`) is the same
shape with the candidates listed: a resolve-by-name where names are not unique
yields a candidate, not a match.

**Shape c — an approved image nobody recorded:** `design/concept/shore-near.jpg`
has no row in `design.json` (2 row(s) present; 3 image file(s) found). Remedy:
add its row with the sky it was drawn in, or remove the file if it was not
approved. A campaign with image files under `design/concept/` or
`design/reference/` and **no** `design.json` at all is this shape over every
file, and its remedy names the document and `delvec schema --stage design`.

**Shape d — a row that cannot be read:** an empty `references` (the schema
refuses it — `minItems: 1`; a record of nothing is not a record), a `name`
outside `concept/` or `reference/`, a duplicate `name`. These are ordinary
schema and referential refusals and are listed only so the reader knows they
are not a fifth shape of `DW-SKY`.

**Every remedy each message names is reachable, and that is a test, not a
promise** — spec-0060 §10.3's rule, inherited: each move above (declare the
hour the rows state; add the row; delete the row; copy the file in; remove the
`set-time`) is a row in `crates/delvec/tests/remedy_reachability.rs`, and
`tools/check-dw-codes.py`'s cross-check refuses a message naming a move with no
row.

**Tier and subject.** `DW-SKY` is raised by `delvec validate` at exit 1. Its
`DwCode` declares `ExitTier::Build`, as every rule reported as an ordinary
validation diagnostic does (`compiler.md` §1: such a rule refusing with a build
under way stops the build), and `Subject::Campaign`.

**The artifact.** `delvec build` writes `validation/design-record.json`:

```json
{ "references": 4, "image_files": 4, "by_directory": {"concept": 4, "reference": 0},
  "skies_stated": [{"time": "night", "weather": "clear", "count": 4}],
  "world": {"times": ["night"], "weathers": ["clear"]},
  "unrecorded_files": [], "unresolved_rows": [] }
```

It is written on **every** build, including one with no record — `references:
0` is a number the staging gate reads (§7), and an absent file is "I could not
look", which is a different fact.

## 7. The staging event

**Authored.** The staging gate (`tools/staging-gate.py`) is the one entry point
to the owner's walk, and it is where "this campaign has an approved design" is
enforced.

1. `design.json` joins `Subject.STAGE_FILES`, so the gate holds a parsed copy
   of a document the compiler read; without this, a campaign carrying one reds
   `MISSING-CHECK` as format rot.
2. Ledger row `drill3-01` gains its carrier and binding — **named here, not
   edited here**:

   ```json
   "carrier": { "kind": "dw", "code": "DW-SKY" },
   "binding": { "kind": "artifact", "file": "design-record.json", "path": "references" },
   "applies_when": { "kind": "campaign", "glob": "world.json" }
   ```

   The binding counts rows in derived output, so it owes an `applies_when`,
   and the precondition is measured over the campaign source: every campaign
   is a member of the class, because every campaign owes a design gate. The
   shapes this produces, by the gate's own rules: zero rows on a campaign that
   exists is **`UNBOUND`, refused** — the class is present and nothing binds
   to it; one or more rows with `DW-SKY` in the engine is bound. `INAPPLICABLE`
   is unreachable for this row by construction, which is the point: a skipped
   design gate is not a design choice.
3. `tools/tests/test_staging_gate.py` drives both directions from fixtures
   (zero rows → `UNBOUND`; one row → bound; a design record the gate cannot
   parse → `MISSING-CHECK`).

The gallery is never staged, so its rows prove the compiler's half only; the
gate's half is proven by its own fixtures, which is how every other row is
proven.

## 8. Scope: a class, and its two members

**Authored.** The rule is not about `time` and `weather`; it is about the class
of properties **both the art and the world state**, and a property is a member
when all three hold:

1. it is **dimension-global** — declared once for the whole world in
   `world.json`, so a single token names it for every scene;
2. **every reference image is drawn under it** — a picture of any scene,
   interior or exterior, was made with it fixed, so a row can state it without
   inventing (a windowless corridor was still painted at night);
3. the world's **reachable set** for it is computable from the documents, so
   the comparison needs no placed piece.

`time` and `weather` are the members today, and they are the only two. The
nearest non-members, named so the boundary is visible:

- **`horizon`** passes 1 and 3 and fails 2: an interior scene is not drawn
  under an ocean, so most rows could state it only by repeating `world.json`.
  Its pairing is with the piece set (spec-0060), checked at validation
  already. If the design gate ever approves whole-map views as their own set
  (`design/reference/`), a `horizon` column on those rows is the same
  mechanism and needs no new one.
- **`difficulty`**, **`min_players`**, **`languages`**: not visible in a
  picture (fails 2).
- **a room's light** (`areas[].lighting`, `mitigation`, a piece's lighting
  record): per area, not global (fails 1); `DW0210` and spec-0054 own it. The
  page's line about "a room whose light the pictures show as a mood rather
  than as a lighting declaration" is that gate's subject, not this one's.

A row therefore has exactly the columns the members need, and adding a member
is adding a column to `Reference` and a set to the comparison, never a second
document.

## 9. The gallery's obligation

**Authored**, against `gallery/` at `ba461b28`: `world.json` declares `time:
day`, `weather: clear`; `quests.json` carries four `set-time` and two
`set-weather` effects and `dialogue.json` four more of the two kinds
(instrument: `grep -c` on the effect type tokens, so mentions, not distinct
targets); there is no `design/` and no image file anywhere under `gallery/`.

1. **A bound element**: `gallery/design.json` with one row per image under
   `gallery/design/concept/`, small original PNGs licensed with the code,
   enough rows that every time in the gallery's reachable set and every
   weather in it is stated by at least one row — so the gallery validates green
   under the equality of §6 shape a.
2. **Units**: the coverage gate enumerates them from `delvec schema --stage
   all` as it does every surface — `DesignContent.references`,
   `Reference.name`, `Reference.shows`, `Reference.time`, `Reference.weather`
   — and the coverage line reports 0 in neither state.
3. **Refusal probes**, a primary plus one declared edit each: shape a (a row's
   `time` moved to a value the world never reaches), shape b (a row's `name`
   moved to a stem no file has), shape c (a row removed while its file
   stays). Shape a's other direction is proven by a crate fixture if the probe
   format cannot add an effect; the spec does not decide that.
4. The emission baseline moves — a new validation artifact is written on every
   build — and is regenerated after the merge, never three-way merged.

No demo level is owed: nothing player-visible is added. The hour was already
a surface; what changes is that it is decided and held, not what it does.

## 10. What the content repository must do

Named, not done — this spec edits no page, no ledger row and no campaign.

1. **Step 1**: `time` and `weather` are required; the page says to decide them
   from the brief's fiction and names the enums; the stub recipe writes both.
2. **Step 4**: the row is written beside the image at the moment of approving,
   with `delvec schema --stage design` as the shape; the walkthrough presents
   each image **with its row's two tokens under it**, so the human's yes is a
   yes to the picture and the hour together. The sentence on the branch
   `docs/the-page-survives-a-second-run` that puts the tokens in
   `design/README.md` is retired in the same edit — two homes for one fact is
   the defect, and that branch has not merged.
3. **Step 5**: the first `delvec validate` after the gate is where `DW-SKY`
   can fire; the page's *Where step 3 ends* rule already frames pre-gate
   refusals, and this code is not one of them.
4. **Step 12**: "read the sky on the first frame" stays, and it reads
   `design.json`, not the README.
5. **The drill campaign**, re-made through the page as the standing drill,
   carries a `design.json` of four rows.

## 11. Acceptance criteria

Machine-checkable; each names its instrument and denominator. **Each was
checked against the tree at `ba461b28` before writing, and the verdict is
recorded per criterion**; a criterion the implementation cannot yet satisfy is
a debt, stated as such, never a pass.

1. **The document exists on the surface.** `delvec schema --stage design`
   exports `DesignContent` with `references` (`minItems: 1`) of
   `Reference{name, shows, time, weather}`, `time`/`weather` by `$ref` to the
   world's own enums; `delvec schema --stage all` lists `design` among the
   stages and `DW0874` names it among the optional documents.
   *Checked: debt — no such stage exists at `ba461b28`.*
2. **The hour has no default.** `delvec schema --stage world` lists `time` and
   `weather` in `required`; a fixture omitting either is refused at validate
   (schema tier), and `WorldContent` carries neither as `Option`. The 34 of 41
   engine world documents that declare neither (instrument: open every
   `world.json` under `crates/`, `prefabs/`, `gallery/` outside `target/`)
   declare both, and the 7 that already do build byte-identically before and
   after (instrument: the gallery baseline's manifests for the gallery and its
   overlays, compared on the emitted datapack only).
   *Checked: debt — both are `Option` with defaults `Noon` / `Clear`; count
   34/41 measured.*
3. **Red then green on one campaign, at validation, nothing placed.** A
   fixture with a design record stating `night`+`clear` over one image and a
   world declaring `noon` exits 1 from `delvec validate` with `DW-SKY` shape a
   and no placement line; the same fixture with `time: night` validates and
   builds green. Both halves in one test.
   *Checked: debt — no code, no fixture.*
4. **The record and the directory are held to each other both ways.** Tests
   for shape b (a row with no file; a row with two candidate files) and shape
   c (a file with no row; files with no `design.json`), each asserting the
   binding line's two counts. The image extension set is one constant in
   `compiler::load`, and a file with any other extension under `design/` is
   neither counted nor refused (the sidecars live there).
   *Checked: debt.*
5. **The reachable sets are read from one place.** `DW-SKY` consumes
   `compiler::light::reachable_time_weather` and no private scan; a test adds a
   `set-time` inside a `sequence` step and asserts shape a fires in the
   world-side direction; a second test removes the effect and asserts green.
   *Checked: debt.*
6. **Every named remedy is reachable.** `crates/delvec/tests/remedy_reachability.rs`
   gains one row per move `DW-SKY` names (§6), each building the campaign that
   takes it and asserting a different verdict; `tools/check-dw-codes.py`'s
   cross-check is green with the new code and refuses if a message names a
   move with no row.
   *Checked: debt — the test file and the cross-check exist at `ba461b28`
   (spec-0060 §10.3, landed); no rows for this code.*
7. **The artifact is always written.** `validation/design-record.json` is
   present after every `delvec build`, with `references` as an integer and the
   fields of §6; a build with no record writes `references: 0`.
   *Checked: debt.*
8. **Staging refuses a campaign with no approved design.** `design.json` is in
   `Subject.STAGE_FILES`; `docs/playtest-findings.json` row `drill3-01`
   carries the carrier, binding and `applies_when` of §7 (edited by the PR that
   lands the code, not here); `tools/tests/test_staging_gate.py` proves zero
   rows → `UNBOUND` (refused), one row → bound, and an unparseable record →
   `MISSING-CHECK`, from fixtures.
   *Checked: debt — the row carries `carrier: null` and the gate reds it
   `NO-GENERAL-FORM`, which is the honest state; `design.json` is not among the
   gate's 11 stage files.*
9. **The gallery** binds the surface (§9): the coverage line reports the five
   units bound, the three probes refusal-proven, 0 in neither state; the
   gallery validates green with its rows covering its reachable times and
   weathers.
   *Checked: debt — 0 units exist; the gallery has no `design/`.*
10. **Determinism and the record.** Double builds of the gallery and of the §3
    fixture are byte-identical (ADR-0006); `docs/reference/compiler.md` gains
    the `DW-SKY` row, a *Stage — `design`* section, and `required` on the
    world's `time`/`weather` rows, in the PR that lands each;
    `tools/check-dw-codes.py` is green with zero new allowlist entries.
    *Checked: debt.*
11. **The first binding.** The drill campaign, re-made through `/new-delve`
    after §10 lands, carries a `design.json` of four rows all stating
    `night`; its `world.json` declares `time: night`, `weather: clear`;
    `delvec validate` is green; perturbing `time` to `noon` reds `DW-SKY`
    shape a; and the staging gate reports `drill3-01` bound with
    `references = 4`.
    *Checked: debt, with the instance measured — on `drill/page-walk-3` the
    campaign holds 4 approved images, all one series under a night style note,
    `time: night`, no `weather`, and no `design.json`.*
12. **The class on the content tree.** Denominator: campaigns on the content
    repository's `main` (`campaigns/*/world.json`). The criterion is that
    every campaign merged to `main` after §10 lands carries a `design.json`
    whose rows cover every image under its `design/` — checked by the content
    repository's per-PR gate running `delvec validate` — and the number is
    reported with its denominator.
    *Checked: 0 of 2 today, and neither builds on this engine — recorded debt;
    the class is empty on the current tree.*

## 12. Out of scope, named

- **A machine reading the picture.** Nothing here opens image bytes; a row is
  a human's reading, made once, at approval. A model-based check of the hour
  is refused on the same ground as gating on `refscore` (spec-0028 §3).
- **Binding the approval to the bytes.** A content hash per image would make a
  swapped image detectable; no such defect is on record, the page attaches the
  approval to the file by name, and `design/` travels through git-lfs, where a
  pointer file is what a fresh checkout holds. If a swapped image is ever a
  finding, that is its row.
- **The walkthrough's medium.** The drill's Gate B records that the page never
  says what a walkthrough *is* at twenty scenes; §10.2 says only what each
  image is shown with, not where.
- **A beat timeline for skies.** Times and weathers are compared as two
  independent sets, as `DW0210` and `DW0496` already read them.
- **`horizon`** (§8), the sidecar's format, the two shipped content campaigns,
  and any change to `DW0210`, `DW0496` or the light model.
