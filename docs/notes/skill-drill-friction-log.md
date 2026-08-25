# Friction log — walking `.claude/skills/new-delve/SKILL.md` end to end as a person

Instrument: engine worktree at `90cf2051f6b8c38f41e77577f1f58698f75dc363`
(`origin/main`, branch `docs/the-drill-friction-log`), macOS, clean shell.
Stance: I have this repository and that page and nothing else. Written as I go.

Subject invented for the drill: **"The Weighbridge"** — a disused customs post on
a road. Three places: the road gate, the weigh house, the ledger room. One spine,
no branching. Nothing to do with priories, tides or bells.

---

## F0 — Reading the page is itself the first cost
- **Page said**: (implicitly) read this and do it.
- **What happened**: `SKILL.md` is 1940 lines / 128,146 bytes. Read in full before
  Init, because the page's own Init step 6 turns out to be a hard stop that only
  makes sense if you have read as far as "The map pipeline" (line 1212) to know
  whether you are a site-plan campaign or an `areas[]` one.
- **A person concludes**: this is a manual, not a procedure. They will skim, and
  the skim will miss which of the two placement models they are on — the one
  decision the page says (line 1218) is "taken once, at the start".
- **Would have unstuck them**: a 20-line "the shape of the run" at the top:
  Init → pick placement model → stages → gates, with line references.
- **Severity**: costs an hour (of reading before anything runs).

## F1 — Init step 1: the page's own verification command does not exist
- **Page said** (Init 1): "from a pipeline checkout: `cargo build --release -p delvec` …
  Then `delvec --version` must print an engine version inside this skill's declared
  `requires.delvec` range. **It is a hard stop if it does not.**"
- **What happened**:
  ```
  $ cargo build --release -p delvec     # exit 0, 32s (warm target; see caveat)
  $ delvec --version
  (eval):1: command not found: delvec        EXIT=127
  $ ./target/release/delvec --version
  delvec 1.1.0, dsl 0.17.0, mc 1.21.11       EXIT=0
  ```
- **A person concludes**: read literally, `command not found` is the hard stop the
  page just declared, at Init step 1 of 6. Nothing on the page says the source
  build lands at `target/release/delvec`, or to put it on `PATH`. The page uses
  THREE spellings for running the binary and never reconciles them:
  `delvec …` (Init, and ~40 later commands), `cargo run -q -p delvec --bin delvec -- …`
  ("The loop", line 361), and `./target/release/delvec` (nowhere).
- **Would have unstuck them**: one line in Init — "a source build puts it at
  `target/release/delvec`; either `export PATH=$PWD/target/release:$PATH` or read
  every `delvec` below as `cargo run -q -p delvec --bin delvec --`."
- **Severity**: mild friction for someone who knows cargo; **stops the drill dead**
  for anyone taking "hard stop" at its word.

## F1b — Init and The loop disagree about `--release`
- Init 1 says build `--release`. Line 361 says "**Do not** reach for `--release`
  mid-loop: it is ~20s slower to rebuild after every edit and the output is
  byte-identical either way." So the page tells a person to build the binary twice,
  in two profiles, and never says the Init build is the throwaway.
- **Severity**: mild friction (one extra cold build ≈ minutes), but it is exactly
  the kind of thing that reads as "I did something wrong".

## Instrument caveat on every build time below
This worktree's `target/` was cloned from a donor (6.1 GB, populated before I
started). **Every build figure I report is a WARM figure and understates a fresh
clone**, which is the state the drill is actually run from. I did not measure cold.

## F2 — Init step 3: the second binary lands in a second, undisclosed target dir
- **Page said**: "`cargo build --release --manifest-path crates/render/Cargo.toml`
  (`--manifest-path`, not `-p`: it is its own cargo workspace). Then
  `delve-render fidelity-gate` must exit 0."
- **What happened**: build exit 0 (74s warm; pulls a git dependency, so network is
  required and the page never says so). Then:
  ```
  $ delve-render fidelity-gate
  (eval):2: command not found: delve-render
  $ ./crates/render/target/release/delve-render fidelity-gate
  fidelity gate PASSED: no missing-texture placeholder in the newest-block fixture   EXIT=0
  ```
- **A person concludes**: the page explains *why* `--manifest-path` is needed — that
  it is its own workspace — and then does not follow that thought to its
  consequence, which is that the binary is at `crates/render/target/release/`, NOT
  beside `delvec`. Someone who solved F1 by putting `target/release` on `PATH` has
  now solved nothing for this binary and will hit `command not found` a second time.
- **Would have unstuck them**: name the two output paths once, in Init.
- **Severity**: mild friction, twice.

## F3 — Init step 4 is the one step with no way to confirm it
- **Page said**: "4. **Python 3** — the skin toolchain is a declared prerequisite
  too, and a missing skin is a build error rather than a silent skip."
- **What happened**: `python3 --version` → 3.14.7. Box ticked. But
  `python3 -c "import delve_skin"` → `ModuleNotFoundError`, and line 1088 later
  demands `python -m delve_skin all <cast.json> … **in its own venv**` with no
  instructions anywhere for creating that venv (`tools/skin/requirements.txt` and
  `pyproject.toml` exist; the page never mentions either).
- **A person concludes**: Init 4 is satisfied, because "Python 3" is satisfied. They
  discover at the skin step that it is not, which is precisely the failure mode
  Init exists to prevent and which step 5 explicitly calls out for `metrics`
  ("confirm it answers here rather than discovering at stage 3…").
- **Would have unstuck them**: give step 4 a confirmation command like every other
  step has — `python3 -m venv .venv-skin && .venv-skin/bin/pip install -r
  tools/skin/requirements.txt && .venv-skin/bin/python -m delve_skin --help`.
- **Severity**: costs an hour, at the moment an NPC needs a skin.
- Related, and reported honestly as NOT a finding for this drill: Init says nothing
  about `pytest`, but no step of this skill invokes `pytest`, so a person walking
  this page never needs it. The omission is real; the cost for this walk is zero.

## F4 — Init step 5: "must print the table" prints 333 lines of JSON
- **Page said**: "**The metrics standard** — `delvec metrics` must print the table.
  It is the single authority for the size classes, seam openings and stair pitches
  a layout graph and a site plan name, so read it before writing either; a name it
  does not define cannot compile (`DW0812`)."
- **What happened**: exit 0. stdout is 333 lines of JSON; a three-line human summary
  including `DW0813` goes to **stderr**, correctly separated.
- **Correction to my own first reading**: I first piped it with `2>&1`, got
  `JSONDecodeError: Extra data`, and was about to log "the output is not valid
  JSON". That was **my measurement failing, not the tool** — re-measured with the
  streams apart, stdout parses. Recording the withdrawal rather than deleting it.
- **The real friction**: the page calls it "the table" and says a layout graph
  "names an entry in" it. It does not say what the names LOOK like in a document.
  The JSON keys are `size-class.hall`, `opening.arch`; whether a `size_class` field
  takes `hall` or `size-class.hall` is not answerable from the page or from the
  output. I have not yet reached the document that needs it — carried forward.
- **Severity**: mild here; see F11 for where it lands.

## F5 — Init step 6 is a genuine hard stop, and its diagnostic is exemplary
- **Page said**: confirm `python3 tools/refimg.py --prompt "smoke test" --dry-run`;
  "Absent config exits 2 saying what to add".
- **What happened**, exactly as promised:
  ```
  refimg: no delvewright.local.toml — create it with a [refimg] section.
  See the commented convention block in delvewright.toml.        EXIT=2
  ```
- **A person concludes**: correctly, that they need an image-provider API key before
  they may start. This is the page working. Recorded as evidence of what good looks
  like: the message names the file, the section, and where the template is.
- **Consequence for this walk**: I have no key, so for a **site-plan** campaign the
  page's own rule ("If any step here cannot be completed, say so and stop") ends the
  drill at Init 6. I therefore took the `areas[]` branch, where the same wall stands
  one gate later at 4b — and I will hit it there. **Deviation declared.**
- **Severity for a keyless person**: stops the drill dead — by design, but the page
  never tells them at the TOP that a paid third-party API key is a prerequisite of
  the whole skill. It is disclosed at Init step 6 of 6, after two cargo builds.

## F6 — Init step 1's first sentence is false, and it is the sentence Init rests on
- **Page said** (Init 1): "**`delvec`** — **one binary, and it carries the whole
  authoring surface** including the CPU render arms".
- **What happened** — measured by grepping the page itself for each binary it
  invokes (instrument: `grep -c` over `.claude/skills/new-delve/SKILL.md` at
  `90cf2051`, counting matching LINES not occurrences):

  | binary | lines on the page | established by Init? |
  |---|---|---|
  | `delvec` | 51 | yes (step 1) |
  | `delve-render` | 8 | yes (step 3) |
  | `delve-grammar` | 6 | **no** |
  | `delve-admit` | 4 | **no** |
  | `delve-harvest` | 2 | **no** |
  | `delve-schem` | 1 | **no** |

  Init mentions exactly two of the six. The four it omits are the entire
  "the layout needs a prefab the library doesn't have" procedure, which the page
  calls "**the procedure, and these are its mandatory steps, in order. Do not
  improvise around them**" (line 888).
- **A person concludes**: nothing, until step 3 of the prefab procedure tells them
  to run `delve-grammar list` and the shell says `command not found`. Then they must
  leave the page for `docs/reference/tools.md` — a document whose rows are
  single paragraphs running to 8,000+ characters — to learn the
  `cargo run -q -p <package> --bin <bin>` form. It is findable. It is not on this
  page, and Init's own promise says it should not need to be.
- **What I ESTABLISHED vs did not**: I established the counts above and that all
  six binaries exist as workspace targets. I did **not** establish that a fresh
  clone lacks them at that moment — my `target/` was donor-populated before I
  started, so `delve-grammar` was already sitting in it. The claim that a person
  hits `command not found` there follows from `cargo build -p delvec` building only
  the `delvec` package, which I did not separately demonstrate.
- **Would have unstuck them**: Init step 1 says "one binary" → make it
  "`cargo build --release --workspace` builds all five; `delve-render` is the sixth
  and its own workspace", and name the two target directories.
- **Severity**: costs an hour, on any campaign needing a prefab that does not exist
  — which the page treats as the normal case for a map that is the point.

---
# Part 2 — authoring the six stages

## F7 — the page contains ZERO complete stage documents
- **Page said** (The loop, step 1): "`… schema --stage <n>` — generate AGAINST the
  live schema, never from memory."
- **What happened**: `grep -n 'dsl_version"' .claude/skills/new-delve/SKILL.md`
  returns **nothing**. In 1940 lines there is not one example of a stage document.
  The envelope every stage needs — `{dsl_version, campaign_id, stage, content}` —
  appears nowhere on the page; I learned it from `schema --stage 1`'s `required`.
  The page names `dsl_version` ~20 times, always as a *fence* ("needs
  `dsl_version` 0.10.0 on the quests stage") and never as a field you write.
- **A person concludes**: they write `{"areas": [...]}` and get a schema error.
  Worse — nothing tells them **what number to put in `dsl_version`**. The page
  states minimum fences per feature (0.8.0, 0.9.0, 0.10.0, 0.11.0, 0.12.0, 0.15)
  and never says whether you write the minimum, the maximum, or the engine's.
  I wrote `0.17.0` (from `delvec --version`'s `dsl 0.17.0`) on a guess. It worked.
  I do not know that it was right and the page cannot tell me.
- **Would have unstuck them**: one 30-line `world.json` at the top of "The loop".
- **Severity**: costs an hour; **stops the drill dead** for a person who does not
  think to read `required` out of a 26 KB JSON schema.

## F8 — the six file NAMES are never stated; the tool spells them one at a time
- **Page said**: "Create `campaigns/campaigns/<campaign-id>/` with **the six stage
  JSONs**" — and never names one of them.
- **What happened**: I guessed `world.json` from the stage list. Then:
  ```
  $ delvec validate <dir>
  internal error: cannot read campaign dir: npcs.json: No such file or directory (os error 2)
  EXIT=10
  ```
- **A person concludes**: two things, and the second is the finding. (1) The path
  IS discoverable — the tool names the next missing file, so six runs of `validate`
  spell the six filenames out. That is a real, if grudging, affordance. (2) It is
  reported as "**internal error**" with **no DW code** and exit 10, for the state
  the page explicitly tells you to be in ("When authoring stages incrementally,
  stub the later stages"). "internal error" is what you print when the compiler is
  broken, not when the author has done what the manual said.
- **Would have unstuck them**: name the six files once; and make the missing-stage
  message an ordinary diagnostic that says "stage documents are `world.json`,
  `npcs.json`, `classes.json`, `quest-plan.json`, `quests.json`, `dialogue.json`".
- **Severity**: mild friction, but it is the first thing the tool ever says to a
  new author and it says "internal error".

## F9 — the page's `happening` rule over-reaches, and the schema is the one that is right
- **Page said** (line 668): "Required on every quest, every objective, every
  staging / wave / gate / `campaign-complete` effect, **and every dialogue option
  that sets a flag**".
- **What happened**: reading "every dialogue option that sets a flag needs one", I
  put a `happening` on a quest-level `set-flag` effect. Refused:
  ```
  DW0100 [error] quests: … unknown field `happening`, expected one of `flag`,
  `requires_flags`, `forbids_flags`, `requires_state` at line 27 column 9.
  Fix the offending field … run `delvec schema --stage <1..7>` to see the exact shape.
  ```
- **A person concludes**: correctly, and fast. **This diagnostic is the best thing
  I met all round** — it names the field, enumerates the legal alternatives, gives
  `line:column`, and names the command that shows the truth. Recorded as the
  standard the others should be held to.
- **Cost**: one loop iteration. Mild friction — but note the page's enumeration is
  loose enough to produce it, and a looser reader would try it on more effects.
- **Severity**: mild friction.

## F10 — the page says six stages; the engine has seven, and the seventh is a campaign document
- **What happened**: that same diagnostic says `--stage <1..7>`. And:
  ```
  $ delvec schema --stage 8
  unknown stage `8` (want 1..7, `geometry-brief`, `layout-graph`, `site-plan`, `detail-plan`, or `all`)
  $ delvec schema --stage 7   →  content: WorldEditsContent
  ```
  Stage 7 is `world-edits.json`. The page mentions that filename **once** (line
  869, inside a symptom row about terrain fixes) and never as a stage, never with a
  schema command, and the "Campaign workspace" section says "**the six** stage
  JSONs".
- **A person concludes**: nothing bad immediately — but a campaign that needs a
  terrain fix has a seventh document whose existence the page hides.
- **Severity**: mild friction. Noted also: the page uses "stage 6" for two
  different things — `dialogue` (the loop) and `detail-plan.json` (the map
  pipeline, line 1388, "**Stage 6 — `detail-plan.json`**").

## F11 — **the biggest one: `collect` + `container` is unsatisfiable in the tileset the page recommends**
- **Page said** (line 450): "**A `collect` takes its item from the room's own
  furniture, and the item has a name.** Point the objective's `container` at the
  anchor of a chest/barrel the prefab already placed… The container must really be
  there in the piece (`DW0438`)". And earlier (line 386): "Areas: prefer
  `prefab_pool` (**stone-keep tileset**) for real layouts".
- **What happened**: I bound `container: "anchor/chest"` — the anchor
  `keep-room-small-a`'s own metadata declares, under that exact name. Build:
  ```
  DW0438 [error] build: 1 `collect` objective(s) adopt a container that is not there.
    collect `obj/take-ledger` -> container anchor `anchor/chest` at [259, 65, 4]
    holds `minecraft:air`, not a container
  ```
- **I then measured the library** (instrument: `delvec palette` over all 36
  `campaigns/prefabs/*.nbt` at the `campaigns/` symlink's current head, plus a
  JSON scan of the 36 metadata files):
  - **5 of 36** prefabs contain a chest/barrel/shulker blockstate at all:
    `hero-galleon-oak`, `hero-standing-monolith`, `island-beach-camp`,
    `island-galley`, `island-mountain`.
  - **3 of 36** declare a container-named anchor: `cave-room-small`
    (`anchor/chest`), `keep-room-small-a` (`anchor/chest`), `island-mountain`
    (four `anchor/cheese-barrel*`).
  - **The intersection is exactly ONE prefab**: `island-mountain`. Both prefabs
    named `anchor/chest` — the obvious thing an author reaches for — contain **no
    container anywhere in the piece**. `keep-room-small-a` holds four blockstates
    total: `chiseled_stone_bricks`, `glowstone`, `jigsaw`, `stone_bricks`.
  - So in the **stone-keep tileset the page recommends, the page's `collect`
    instruction cannot be followed at all**, and the one library piece where it can
    be followed is a `dark`-profile mountain cave with 33 anchors.
- **The diagnostic's prescription, followed literally**: it offers three moves and
  forbids the third. (a) "put a `minecraft:chest` … at the anchor's cell and
  re-export the `.nbt`" — the page itself forbids this ("Never hand-patch `.nbt`"),
  and re-exporting means the whole `delve-grammar` + `delve-admit` chain Init never
  built. (b) "point `container` at an anchor whose cell already has one" — measured
  above: for a keep campaign there is none. (c) "Dropping the `container` field to
  make this go away is **NOT** the fix". **I did (c)**, because it was the only
  move left, and the build accepted it.
- **A person concludes**: that the engine is contradicting itself. They will do (c),
  which the message tells them is wrong, and ship the floating chest the field
  exists to prevent — believing they were forced to.
- **Would have unstuck them**: a line in the page saying which prefabs actually
  carry containers, or a `DW0438` that names the ones that do.
- **Severity**: **stops the drill dead** — or worse, produces the exact defect the
  diagnostic was written to catch, with the author's full knowledge.

## F12 — **the one I could only answer from the compiler source**: multi-area transport is silently skipped, and DW0311 blames the geometry
This is the entry the brief asked for: *what I used that a person does not have.*

- **Page said** (Supported techniques, line 809): "**Multi-area + automatic
  inter-area transport is a physically enforced point of no return.** Placing beats
  in separate areas (256 blocks apart across void, no walkable link) **makes the
  compiler emit a one-way teleport on the objective that crosses areas** — the
  player *cannot* walk back."
- **What happened** on an ordinary three-area campaign with one objective per area:
  ```
  DW0311 [error] build: critical path: the player cannot walk from [259, 65, 4]
  to [513, 65, 2] over the assembled geometry — no collision-free path.
  A same-area leg must be walkable end to end; this is a wedged doorway seam, a
  void gap in the assembled layout, or an unbroken 1.5-tall barrier (fence/wall)
  ring … (or, if the jump is intended, a missing inter-area transport).
  ```
  The two anchors are 254 blocks apart in different areas. The page says the
  teleport is automatic. The build says it is missing. The message's own guesses —
  wedged doorway, void gap, fence ring — are all wrong, and its phrase "**A
  same-area leg**" tells the author the compiler thinks these are one area.
- **What a person has to work with**: I grepped the entire live schema export for
  all six stages. `"transport"` — **zero hits**. `"inter-area"` — **zero hits**.
  The page's own instruction is "generate AGAINST the live schema"; the schema has
  no spelling for this. `docs/reference/compiler.md`'s `DW0311` row repeats the
  phrase "no inter-area transport" and names no remedy.
- **What I ruled out first, as a person would**: both areas declared an anchor
  named `anchor/npc-stand`, so I suspected a name collision. I rebuilt with unique
  anchor names in every area. **Identical DW0311.** Not the cause.
- **Then I read `crates/compiler/src/plan.rs`. A person cannot.** The rule is at
  `build_critical_path`, line ~4086:
  ```rust
  if prev_area != next_area
      && let Some(ResolvedAnchor::Point { pos, .. }) = anchors.entry_anchor(next_area)
  { transport.insert(prev_id.clone(), *pos); … }
  ```
  **The transport fires only if the DESTINATION area's prefab declares an
  entry-point anchor.** If it does not, the `if` is false, no teleport is emitted,
  nothing is said, and the leg is then judged as a walk — which is DW0311.
- **Proof, single variable**: I changed exactly one thing — the third area's prefab,
  from `keep-room-small-b` (no entry anchor) to `hello-room` (declares `spawn`) —
  and moved the NPC to an anchor that piece has. **`BUILD EXIT=0`.** Nothing else
  changed.
- **How rare the qualifying pieces are** (instrument: JSON scan of all 36
  `campaigns/prefabs/*.json` for an anchor named `spawn`/`entry`/`entrance`/
  `threshold` or carrying a declared role): **5 of 36** — `cave-shore`,
  `hello-room`, `island-beach-camp`, `island-galley`, `keep-spawn-hall`. In the
  stone-keep tileset the page recommends, **exactly one piece of eleven** can be
  transported INTO. So a two-area keep campaign fails unless its second area is the
  spawn hall again.
- **A person concludes**: "the engine is broken", or "my prefabs don't fit
  together", and starts moving areas around, checking for fences, and re-reading
  the jigsaw docs. Nothing they can read tells them the answer. **They give up
  here**, and it is the second real thing they tried to build.
- **What a person without engine knowledge would have done instead**: collapsed the
  campaign into one area — which silently deletes the "physically enforced point of
  no return" the page sold them, and they would never know that is what happened.
- **Would have unstuck them**: `DW0311` naming the real condition —
  "areas differ and `<area>`'s prefab declares no entry anchor (`spawn`/`entry`/
  `entrance`/`threshold`), so no transport was emitted" — instead of guessing at
  fences. This is the same class of defect `plan.rs`'s own comment at line 1905
  records as having happened before: "*the island-tileset area was silently never
  transported into, never framed*".
- **Severity**: **stops the drill dead.** Highest-ranked item in this log.
