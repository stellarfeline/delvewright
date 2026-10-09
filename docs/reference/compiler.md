# delvec compiler — behavior reference

`delvec` is the delve creator: the one binary that turns campaign documents into
a provably completable Minecraft adventure map and carries every tool that
authors, admits and renders its rooms (ADR-0023). **This file is the single
authoritative record of the *current* behavior of its compiler surface** —
validate, analyze, build and what they emit. Specs (`docs/specs/`) remain the
historical decision records; this file is what the compiler does today. A PR
that changes compiler behavior updates this file in the same PR (CLAUDE.md
Methodology; CI enforces the DW-code subset — see `tools/ci/check-dw-codes.py`).

- The compiler is the module `delvec::compiler` of the one package
  `crates/delvec` (Rust-native, ADR-0011; one package, ADR-0025), whose
  `src/main.rs` is the binary. The other surfaces of the same binary — `delvec schem`,
  `delvec prefab`, `delvec grammar`, `delvec render`, `delvec harvest` and every
  other subcommand `delvec --help` lists — and
  the scripts around it (`tools/`, `validation/`) are indexed in
  [`tools.md`](tools.md).
- Versions (as of this doc): `delvec 1.11.0`, `dsl 0.38.0`, `mc 1.21.11`.
  The `dsl` number is the **one** `dsl_version` this engine accepts (ADR-0024):
  every stage document, map-pipeline document and l10n sidecar declares it, and
  any other number is refused at the envelope with `DW0102`, which names it. The
  number says which surface a document was written against and promises
  nothing about any other engine — a released campaign is built by the engine
  it pins (`versions.toml`), and a surface change bumps this number and moves
  every document in this repository with it in the same change. The number is
  the `delvewright-dsl` crate's own package version (a format change bumps the
  minor, a Rust-API-only change the patch), and a patch bump is a new accepted
  number too — the full number is declared and the full number is judged. This line is
  not prose: it is bound by equality to the root `Cargo.toml`
  (`[workspace.package] version`), `crates/dsl/Cargo.toml` (`[package] version`,
  which `DSL_VERSION` reads through `env!("CARGO_PKG_VERSION")`) and
  `versions.toml` by `tools/ci/check-reference-versions.py`, in both directions —
  and it is WRITTEN by that tool's `--write`, never retyped.

---

## 1. Pipeline overview

### Pass order (`delvec build`)

| # | Pass | Crate/module | Fails with |
|---|------|--------------|-----------|
| 1 | Load campaign dir (6 required stage docs + the 5 optional documents + `l10n/` sidecars) | `compiler::load` | internal (≥10), **naming the document that could not be read** |
| 2 | Parse (serde, `deny_unknown_fields`) | `dsl::parse_campaign` | `DW0100` (exit 1) |
| 3 | Validate stages 1–7 (schema + referential, full injected registries) | `dsl::validate_campaign_with` | `DW01xx` (exit 1); also `DW0455`, a body-family code refused at declaration time |
| 4 | l10n sidecar coverage + reserved channels + language-code mapping + inline style markup | `dsl::validate_l10n`, `dsl::validate_marker_channel`, `dsl::validate_tr_sigil`, `dsl::declared_mc_codes`, `dsl::textstyle::validate_inline_styles` | `DW0180`/`DW0181`/`DW0182`/`DW0183`/`DW0184`/`DW0975`/`DW0976` (exit 1) |
| 5 | Analyze (branch-coherent quest/dialogue reachability + critical-path replay) | `compiler::analyze` over `compiler::flow` | `DW02xx` (exit 2) |
| 6 | Solve jigsaw layout (per `prefab_pool` area, from seed); then read the settled draw back and report a pool that seats the same anchor-bearing prefab twice (`DW0498`, `compiler::pool`) | `compiler::solver`, `compiler::pool` | `DW030x` (exit 3); advisory `DW0498` |
| 7 | Assemble world model (placed pieces → voxel grid; ocean sea-level datum check) | `compiler::plan` | `DW030x`/`DW0344` (exit 3) |
| 8 | Replay the stage-7 edit script over the assembled model (spec-0017; per-batch invariant re-proofs — trap-hardware integrity, gravity, relight, walkability, fluid containment against the horizon, sea seepage into the built volume, boundary safety, block support; plus the advisory gate-region check). Skipped entirely for a campaign without one (byte-identical). | `compiler::edit` | `DW0318`/`DW0322`/`DW0323`/`DW0352`/`DW0354`/`DW0851` + reused invariant codes, batch-attributed (tier per code); advisory `DW0353`/`DW0354` |
| 9 | Assembled-light + relight (measure, place fixtures; over the **edited** model when a script exists) | `compiler::light` | `DW0210`/`DW0211` (**exit 2**) |
| 10 | Nav checks (**the ambient sea inside the built volume** (`DW0851`) and **boundary safety over the finished world** (`DW0322`, error tier) for every campaign that assembles one — the floor under the per-batch stage-8 call, which an edit-free campaign never reached; then A* `move-npc`/`move-actor` (footprint-aware, each walk routed over its **own timeline's** gate state), cutscene clip (authored polyline + rendered keyframe chords) + angular budget, critical-path walkability — incl. relight fixtures + water flood, and **per reachable branch** over each branch's own path under its own gate-seal step space; talk-to endpoint snap; waypoint self-check (critical path + per branch); v0.6 checkpoint no-stranding/placement + no place a body gets into and not out of (`DW0921`) + stealth-zone/onset + trap completability proofs; spec-0016 §6 TD lane polylines; spec-0016 §1 bonfire safe zone) — all over the **edited** model when a script exists | `compiler::nav` + `compiler::timeline` | `DW0307`/`DW0308`/`DW0311`/`DW0314`/`DW0315`/`DW0316`/`DW0318`/`DW0322`/`DW0325`/`DW0327`/`DW0342`/`DW0347`/`DW0355`/`DW0386`/`DW0410`/`DW0430`/`DW0478`/`DW0488`/`DW0851`/`DW0921` (exit 3; `DW0342` → exit 2) |
| 11 | Referential + placement seals inside emission: every anchor-bearing effect resolves (`DW0360`), no generated name collides (`DW0361`), no body eclipses an interaction affordance (`DW0359`, `compiler::eclipse`), no body, destination or cast row stands at a mark whose offset leaves its anchor's piece (`DW0897`, `compiler::mark`), no two bodies whose lifetimes overlap are declared on one cell (`DW0896`, `compiler::cohabit`), no body occupies block geometry at its anchor or on any walked leg (`DW0450`/`DW0451`, `compiler::clearance`), no walked leg contains a move its own body cannot make and no body's `traversal` declaration goes unexercised (`DW0452`/`DW0453`/`DW0454`, `compiler::traversal`), no two bodies the party clicks contest one crosshair in a scene the cast ledger declares (`DW0489`, `compiler::crosshair`), no daylight-burning body is staged for a fight whose walkable ground reaches open sky in an hour and weather it can stand in that burn it (`DW0496`, `compiler::daylight`, measured off the seated wave cells), no body whose AI takes no land target while the level is bright is staged as a fight that is bright until its `kill` completes with no water for the party's feet in reach (`DW0920`, `compiler::engage`, the same seated cells and the same clock; prints `engagement binding:`) | `compiler::emit` | `DW0359`/`DW0360`/`DW0361`/`DW0450`/`DW0452`/`DW0489`/`DW0496`/`DW0896`/`DW0920` (exit 3); advisory `DW0359`/`DW0451`/`DW0453`/`DW0489` |
| 12 | Emit (datapack incl. the world build — the mass, seal and stage-7 writes in steps across ticks, packtest, server, critical-path, resourcepack, and the visual tier's `render-plan.json` — whose every camera is stood up in open air and then proven clear-eyed against the assembled world, `DW0724`) | `compiler::emit`, `compiler::render_plan` | `DW0300`+/`DW0724` (exit 3) |
| 13 | Emission self-checks over the **finished tree**: every affordance is visible and only its owner retires it (`DW0420`/`DW0421`), no engine fixture is reachable by a box-narrowed selector (`DW0545`), the call graph is closed — no `function <ns>:<name>` points at a function that was never emitted (`DW0497`) — and the score reads are closed: no `if score` / `unless score` / `scores={…}` reads a scoreboard entry the pack never creates (`DW0495`) — and no shipped function's command chain, with every function it calls in the same tick, can pass the game's `max_command_sequence_length` (`DW0984`, `compiler::chain`) | `compiler::affordance` + `compiler::integrity` + `compiler::seeding` + `compiler::chain` | `DW0420`/`DW0421`/`DW0495`/`DW0497`/`DW0545`/`DW0984` (exit 3) |

- `build` ⟹ `validate` + `analyze`; `analyze` ⟹ `validate`. A validation failure
  short-circuits (exit 1) before analysis; analysis failure (exit 2) before build.
- **Every load failure names the document it is about**, and keeps its
  `ErrorKind`, so *absent* and *unreadable* stay distinguishable
  (`quests.json: No such file or directory` vs `classes.json: Permission
  denied`). The directory itself is established before any document is read, so
  an absent campaign directory names the directory rather than reporting a
  missing `world.json`. An **optional** document is absent only when it is not
  there: one that exists and cannot be read is a refusal, never a silent
  absence, because a build that skipped it would be byte-identical to one whose
  campaign never declared it.
- The assembled-light gate (`DW0210`/`DW0211`) runs inside the build (it needs the
  placed geometry) but is analysis-tier: `main` maps a `DW02xx` build diagnostic to
  **exit 2**. Its relight fixtures feed both `setup_finish` emission and the nav
  re-verification in pass 9.
- The light field itself (`compiler::light`) is **dense**: over the assembled
  AABB plus the layer of open air above it (the cell a body stands in on the
  tallest block, which reads the sky and block light it really has), each cell is one byte holding whether light passes it, whether the sky is
  above it and what it emits, resolved from its block id once. Sky exposure is one
  top-down sweep per column; the frontier drains brightest-first, so each cell is
  relaxed once. The greedy relight loop floods once and **extends** the field per
  fixture — a fixture written into a cell whose passability it does not change,
  emitting at least as much as what it replaced, can only make the field
  brighter, so the new field is the old one with that seed flooded into it; a
  write failing either half floods again from nothing. Determinism does not rest
  on the walk order: the field is the pointwise maximum over every seed of
  `seed − distance`, one value per cell however the frontier drains. The one
  order that IS a decision — the darkest deficient cell, ties by ascending
  `(y, z, x)` — is an explicit sort.
- Every emitted `.mcfunction` line is checked against the vendored 1.21.11
  Brigadier tree (`compiler::commands`, `crates/delvec/data/commands-1.21.11.json`;
  structure-only — arity/paths, not arg values). mecha re-validates in CI
  (ADR-0011); disagreement fails CI. The match is exact and complete: the first
  token must be a known command root, `literal` nodes match verbatim, and
  `argument` nodes consume a fixed per-parser token count (`vec3`/`block_pos` 3,
  `vec2`/`column_pos`/`rotation` 2, `message` and greedy `string` the rest, else
  one balanced token). Tokenizing is brace/bracket/quote-aware, so an NBT
  compound, a block-state suffix and a selector are each one token. **The
  separator between two tokens is exactly one space**, read the way the server
  reads it: 1.21.11 trims the line and skips one `' '` between nodes, so a
  second space or a tab before a literal or inside a fixed-arity argument is a
  refusal naming the token it stands before, while whitespace inside a balanced
  or quoted span, or anywhere in a greedy tail (`say  hello`), is the span's own.
  Measured on the pinned server from one probe datapack: `execute  if …`,
  `… matches 1 run  say …`, `tp @s 0  64 0` and a tab after `execute` fail to
  load ("Incorrect argument for command"); `say  hello`, `say hello  world`, a
  doubled space inside a JSON string and inside a selector load. Matching
  **backtracks** across ambiguous argument branches and follows `redirect`s
  (`… matches N` → `execute`, `run <cmd>` → the tree root); a line is valid iff
  every token is consumed ending on an `executable` node. What it therefore
  catches is a misspelled command, a wrong argument count and a bogus subcommand
  path; what it does not judge is numeric coordinates, well-formed NBT/JSON, or
  whether a block or item id exists — those are mecha's cross-check and the DSL
  item/block registries.
  **Single-entity arity** (spec-0018): an entity argument
  the tree marks `amount: "single"` rejects `@a`/`@e` without `limit=1`.
  `damage @a[…] 40 minecraft:generic` is a well-shaped command that 1.21.11
  refuses to *load* ("Only one entity is allowed…") — taking the whole enclosing
  function down with it, silently. The tree already carries the fact, so the
  compiler enforces it rather than leaving it to folklore; the party form of
  `damage-players` is `execute as @a[…] run damage @s …`.
  **Value-level exceptions** — bounds the command's own handler keeps *after* the
  parse has succeeded, which the tree cannot express because Brigadier describes
  only how a line parses. Such a line passes the tree and is refused by the
  running server, and because a refused command is not a parse failure the rest
  of the function still runs and nothing reads the reply. Each such bound that
  has cost something is one named function in `compiler::commands`; there are two.
  (1) An SNBT integer literal in a
  `key:value` position whose suffix cannot hold it — NBT bytes and shorts are
  signed, so `text_opacity:255b` is structurally flawless and unparseable, and
  1.21.11 answers "Failed to parse number: Value out of range" by dropping the
  entire function. Quoted spans are skipped, and a bare standalone number is not
  examined, so it cannot mistake prose for a value.
  (2) A `forceload` rectangle naming more than `FORCELOAD_MAX_CHUNKS` (256)
  chunks. `forceload add -76 -76 176 176` is a perfectly good `forceload add
  <column_pos> <column_pos>`; 1.21.11 answers "Too many chunks in the specified
  area (maximum 256, but specified 289)" and marks **nothing**, so the world
  boots with the placement's chunks unloaded while every static gate stays green.
  The ceiling is on the **area**, not on a side — a 1 × 257 strip is refused too
  — and `forceload remove` over a rectangle goes to the same handler; both facts
  were read off the pinned server rather than a wiki. Judged only where all four
  coordinates are plain integers, so a relative coordinate is left to the server
  like every other value. The emission side is under **forceload lifecycle**
  below.
  **A chat message's length** is a parse bound, and the walk applies it: a
  `minecraft:message` argument (`say`, `me`, `msg`, `teammsg`) holds at most
  `MESSAGE_MAX_CHARS` (256) characters, counted as the server counts them, and a
  longer one makes 1.21.11 refuse the WHOLE function at load — "Chat message was
  too long (707 > maximum 256 characters)", measured on the creator overlay's
  shot roster the first time a PackTest server loaded it. The line is refused
  naming its length. A macro line is judged on its literal text with each
  `$(name)` counted as nothing: a floor, which a substituted value can still
  carry past the bound at run time. `delvec prefab`'s gallery is the
  second consumer of this validator: it emits `.mcfunction` into a datapack
  exactly as `delvec` does, so it runs the same tree over its own output
  before writing anything (`gallery::validate_functions`, `DW0760`) rather than
  carrying a private copy of the rule.
- Determinism (ADR-0006): all map/set iteration is `BTreeMap`/sorted; the only
  randomness is stage-1 `seed` → a named splitmix64 per-area stream.

### CLI contract

```
delvec validate <dir>                      # stages 1–7 schema + referential
delvec analyze  <dir>                      # + quest-graph reachability
delvec build    <dir> -o <out>             # full deterministic build
delvec build    <dir> --perturb <knob> [--perturb-place <place>]
                                           # ask the derivation for a named defect and
                                           #   watch the observer; writes NO tree (§5.3)
delvec fmt      <path>… [--check]          # canonical form for authored JSON (§9)
delvec schema   --stage <1..7|name|all>    # export JSON Schema (named documents: §2)
delvec metrics  [--gym <dir>]              # export the metrics standard as JSON (§10)
delvec codes                               # every DW code this binary declares, one JSON line each (§5)
delvec prefab anchors [--pool <id>]        # which anchors does a pool guarantee (library only)
delvec l10n-inventory <dir> [--lang <c>]   # l10n key inventory as JSON (transcreation input)
delvec l10n-apply <dir> --lang <c> --table <f>
                                           # write the sidecar from an English → translation table
delvec allocation <dir> [<place>|--all]    # the handed allocation for a site-plan place
delvec detail   <dir> [<place>|--all]      # detail a place inside its allocation (spec-0058)
delvec snapshot <dir> [framing] [-o f.png] # draft frame + scene manifest (§7)
delvec blocking-chart <dir> [-o dir]       # per-elevation cutaway floor plans (§7)
delvec edit apply   <dir> [--batch f] [-o dir]  # replay edit script (+ candidate), persist on green (§7)
delvec edit preview <dir> [--batch f] [-o dir]  # same replay + renders, never persists
delvec calibrate <report> --layout <layout.json> [-o f.json]
                                           # harvested shot proposals -> anchor+offset DSL patch (§8)
delvec rig describe <rig> [--facing <f>]   # check a library rig (DW0935) and print its parts, every
                                           #   clip's length in ticks and per clip the last-frame
                                           #   footprint relative to the mark (spec-0082)
delvec --version                           # "delvec x.y.z, dsl a.b.c, mc x.y.z"
```

Global flags: `--json` (one JSON diagnostic object per line), `--prefabs <dir>`
(default `campaigns/prefabs`), `--lang <code>` (default `en`; affects `build`
only — `validate`/`analyze` are language-independent apart from coverage).

**Threads.** `DELVEC_THREADS=<n>` (a positive integer) sets how many threads
`delvec` spreads independent work over; unset or unparseable, it is the host's
available parallelism. `DELVEC_THREADS=1` runs everything on the calling
thread. The count changes how long a build takes and never a byte it emits
(§4 "Determinism").

**The output directory** holds exactly what the last build emitted: a build into a directory a previous build wrote removes every file that build emitted and this one does not, and a directory holding anything no build wrote is refused untouched (`DW0967`, §5).

**Exit codes**: `0` ok · `1` validation failure · `2` analysis failure · `3`
build failure · `≥10` internal error. Undeclared `--lang` is a validation-class
rejection (exit 1). Codes are stable API; the CI fixture matrix asserts them.

**A failure that stops a build exits at its CODE's tier.** Which tier a rule
fails at is a property of the rule, so every `DwCode` declares it
(`dsl::diagnostic::ExitTier`), and
there is no constructor that leaves it unsaid. `Analysis` means the compiler did
its job and the CONTENT is the defect, and exits 2. `Build` means the compiler
could not produce a tree it will stand behind, and exits 3; it is also what a
rule reported as an ordinary validation diagnostic declares, because such a rule
refusing with a build under way is a build failure. The analysis-tier codes are
exactly these, and the second column says where the author's fix goes —
`tools/ci/check-dw-codes.py` holds this table and the source declarations in
lockstep, in both directions:

| Code | What the author changes |
| --- | --- |
| `DW0201` | the quest graph — the finale is unreachable |
| `DW0202` | the quest graph — nothing that completes triggers the quest |
| `DW0203` | the objective / dialogue graph — the objective completes in no branch |
| `DW0204` | the beats on the critical path or on a branch's path — an exported path is not one a player can walk |
| `DW0210` | the area's `lighting` or `mitigation` declaration, or the prefab it is measured over |
| `DW0211` | the area's declared fixture, or the place it has to reach |
| `DW0312` | the wave's size, or the room it spawns in |
| `DW0313` | the prefab — a gravity floor over the void needs a substrate |
| `DW0342` | the trap's placement or `rearm`, or a disarm the party can reach first |
| `DW0879` | where the write sits relative to the gate that reads it — the path clears the datum before the beat that needs it |

Every other code is build tier.

**Reading a campaign directory has two failure states, and they are different
findings.** A directory that is present and does not hold all six stage
documents is a campaign part-way through being written — the state an author is
in for as long as it takes to write six documents — and it is refused as
`DW0874` at exit 1, naming every document that is missing. Anything else about
the directory (a document that cannot be opened, a directory standing where a
document belongs, a path that is not a campaign directory) is `internal error`
at exit 10, naming the path or document. The verbs that read a campaign
directory — `validate` and everything built on it, `l10n-inventory`,
`l10n-apply`, `edit`, `allocation`, `detail` — answer both the same way, from
one place.

**`--json` diagnostic shape**:
`{ "code":"DW####", "severity":"error|warning", "stage":"<stage>",
"path":"<json-pointer-ish>", "message":"…" }`.

**Severity is load-bearing.** `delvec` exits non-zero only on `error`. A `warning`
is printed and emitted in `--json` exactly like an error but never fails
`validate`/`analyze`/`build`. The tier is reserved for rules whose verdict depends
on something outside the campaign — `DW0330`, where how much text fits depends on
the player's window size and GUI scale; `DW0359`'s crowding tier, where whether a
neighbouring body shadows an affordance depends on the approach angle a player
takes; `DW0451`, where how far a mob model renders past its hitbox is client
geometry the compiler has no data for; `DW0489`'s barks tier, where two bodies
really are ambiguous but the campaign has declared that neither right-click
carries a consequence — or on authorial judgement the compiler
may measure but must not overrule (`DW0351`, `DW0353`, `DW0354`'s decoration
tier, `DW0379`, `DW0380`, `DW0453`, where a one-block course of a wall line may
be a decorative kerb, a deliberate stile or an enclosure that was meant to hold,
`DW0498`, where a pool repeating an anchored piece
is a legal shape shipping content relies on, and `DW0889`, where an anchor
carried only by a pool's filler pieces is one the draw may well seat and
shipping content does rest on).

---

## 2. DSL surface (per stage)

Envelope (every stage): `{ dsl_version, campaign_id, stage, content }`,
`deny_unknown_fields`. IDs are type-prefixed kebab-case; all cross-stage refs are
**strictly backward**. Source of truth: the stage-surface modules under
`crates/dsl/src/`, one per object class (schemas exported via `delvec schema`). Introduced-by column cites the spec.

### Stage 1 — `world`

| Field | Behavior | Since |
|-------|----------|-------|
| `title` | Player-visible; l10n key `world.title`. | 0.1 |
| `outro` (opt) | The closing line on the campaign-completion advancement — the last player-visible sentence of the delve. Player-visible, so it is l10n-inventoried as `world.outro` and sidecars translate it. Absent = the **finale quest's `goal`** (already campaign-derived and inventoried as `quest.<q>.goal`), so the line is never hardcoded English either way. | 0.6 |
| `theme`, `premise` | Authoring context; **excluded** from l10n. | 0.1 |
| `seed` (u64) | Sole downstream randomness (layout PRNG). | 0.1 |
| `target_minutes` | Informational (pacing). | 0.1 |
| `languages[]` (opt) | BCP-47 codes; `en` implicit/never listed; drives l10n coverage + `--lang`. | 0.3 i18n |
| `areas[]` | 1..N. Each binds **exactly one** of `prefab` or `prefab_pool`+`pieces{min,max}` (else `DW0160`). Area origin = `[i·256, base_y, 0]`, where `base_y` is the **horizon datum**: `void` → 64, `ocean` → 60 (see `horizon`). Either way the placed pieces go through the same socket seal (`solver::seal_layout`, see *Assembled-world model*): a single-prefab area places one piece, so **every connector it declares is unmated and is walled**. A prefab with no connector yields no seal. | 0.1 / pool 0.2 |
| `areas[].lighting {fixture,min_light}` (opt) | spec-0010: relight pass guarantees `min_light` (1..=14, default 7; `DW0196` out of range) over reachable walkable cells by placing `fixture` (`torch`/`lantern`/`campfire`/`shroomlight`), else `DW0211`. | 0.5 |
| `areas[].mitigation` (opt) | `night-vision` — the first-class darkness declaration. The compiler emits a self-rescheduling **1 s (20t)** `night_vision_tick` that runs `effect give @a[<this area's placed bounds>] minecraft:night_vision <lease> 0 true` (amplifier 0, particles hidden). The lease is `max(12, longest camera + 10 + 1)` seconds — **the camera-coverage guarantee**: a granted vision effect must outlast any authored camera it can overlap, plus vanilla's 10 s wind-down, so it can never begin ramping down on screen. 12 s is the floor and is what a campaign with no cutscene emits. The longest camera is measured from the ticks `camera::shot_ticks` really emits, and the campaign-wide max is used because the compiler cannot know which cutscene a player who steps out of a mitigated area will land in — the island's ending transports the party from the mitigated island to `area/open-sea` and immediately plays a 15 s camera, which a 12 s lease could not survive. A player who leaves the area keeps sight for ≤ the lease: deliberate, since no vanilla primitive strips one effect on region exit without stripping effects the story granted, and the alternative is a visible flicker. Independent of `lighting`. This declaration is the **sole** `DW0210` night-vision mitigation. | 0.6 |
| `time` (**required**) | **A keyword or a celestial statement** (spec-0081). A keyword — `day`/`noon`/`dusk`/`night`/`midnight`/`dawn` (`sunrise` is accepted as a synonym of `dawn`) — states vanilla's word with vanilla's meaning: the hour, **on day 0**, so a keyword night shows a **full moon**. A celestial statement — `{"sun"|"moon": <position>, "phase"?: <phase>}` — names exactly one body, one of six positions (*Celestial time*, below) and the moon's phase, one of the eight names the pinned `moon.json` gives, kebab-cased (`full-moon`, `waning-gibbous`, `third-quarter`, `waning-crescent`, `new-moon`, `waxing-crescent`, `first-quarter`, `waxing-gibbous`); the engine computes the tick count, day included. `phase` is **required** on `world.time` wherever the moon is at or above the horizon and **refused** wherever it is below (`DW0931`). **No default, and no `Default` impl** (spec-0061 §4): "this delve is played at noon" is a design decision. An omitted `time` is `DW0100` like any other missing required field; so is a phase outside the eight or a position outside the six. It is also the world half of `DW0890`'s comparison against the approved design. Dimension-global initial state, emitted in the sealing baseline through one token (`WorldTime::token`): a keyword on day 0 emits its table argument — the four vanilla keywords **verbatim** (`time set night`), `dusk` and `dawn` as ticks (`time set 12000`, `time set 23000`) — so no keyword campaign's bytes move; any other clock emits the integer `day × 24000 + daytime` (`{"moon": "just-risen", "phase": "new-moon"}` → `time set 108959`). `dusk` is the **sunset onset** (12000), deliberately not 13000 — that is what `night` sets; `dawn` (23000) is the sunrise onset. Every reader of the hour reads the clock: the sky-light model (`DW0210`) judges 15 on the day plateau of the vendored `sky_light_level` track and 4 elsewhere, so a twilight is judged as night (the conservative direction); `DW0496` reads the vendored `monsters_burn` window by tick. Same type for the `set-time` effect, a design row and a camera's `sky.time`. | 0.5 / dusk+dawn 0.5 / celestial 0.35 |
| `weather` (**required**) | `clear`/`rain`/`thunder`. Dimension-global, emitted after sealing (`weather <kw>`). Rain/thunder attenuate the assembled-light sky term. **Required, with no default**, for the reason `time` gives. The declared state is emitted. | 0.5 / required 0.22 |
| `difficulty` (opt) | The delve's combat difficulty: `easy` / `normal` / `hard`. Absent = the compiler's **derivation** — `easy` when the campaign fields any wave, `peaceful` when it fields none. Declaring it overrides the derivation in BOTH places a difficulty comes from: `server/server.properties` (what the shipped image and every compose profile boot from, via `validation/world-settings-entrypoint.sh`) and a `/difficulty <kw>` appended to the sealing baseline, so the declaration also holds when the datapack alone is dropped into another world. A declaring campaign also emits the `declared_difficulty` PackTest, which asserts the live world's difficulty via the bare `/difficulty` query command (vanilla returns `Difficulty#getId()`: peaceful 0, easy 1, normal 2, hard 3) — so properties, sealing and declaration are proven to agree on a real server. `peaceful` is refused (`DW0468`); fighting actors with no waves and no declaration is the advisory `DW0469`. **Retuning warning:** the derived `easy` HALVES incoming player damage (`min(dmg / 2 + 1, dmg)`) — content tuned under it that declares `normal` or `hard` must redo that arithmetic, not merely flip the keyword. | 0.6 |
| `horizon` (opt) | The ground and the sky the map stands in. Either a **string shorthand** — `void` (default/absent) or `ocean` — or the **object form** `{base, …params}`. `void`: nothing outside the placed geometry. `ocean`: a pinned bedrock/stone/water superflat, sea level y=62, no structures or mobs; it drives `generator-settings` **and the area-origin datum**, placing ocean areas at y=60 = `sea_level − 2` so an island piece's authored waterline (local y=2) meets the world ocean and its walk plane is the vanilla-normal one block above the sea (`DW0344`). `valley`: the one base that BUILDS terrain — see *The horizon's surround* below. Params: `ratio` (2.0..=3.0, default 2.5) and `rim_height` (16..=128, default 48), both `valley`-only, both `DW0853` out of range or beside another base. Any horizon whose ambient a body can ENTER — the sea, and a valley's gap floor — needs a `boundary` (`DW0320`); `void` is the only one it cannot, because there is nothing out there to stand on. | 0.19 |
| `atmospheres[]` (opt) | spec-0080: **the skies a place can stand under**, declared once on the campaign. Each is `{id: atmosphere/<kebab>, attributes?, tint?, precipitation, climate?}` and ships as a datapack biome `<ns>:atmosphere/<kebab>` (see *World / build output*). `attributes` maps an environment-attribute id (`visual/sky_color`; the `minecraft:` prefix optional) to a value in its shape — the ids, scope, shapes and the range the pinned codec rejects outside of are vendored data (`crates/delvec/data/environment-attributes-1.21.11.json`, read from the pinned jar by `tools/maintenance/extract-environment-attributes.py`), so the one DSL unit is the map. **20 ids are admitted**; the 5 the overworld day cycle overrides (`sun_angle`, `moon_angle`, `star_angle`, `sunrise_sunset_color`, `moon_phase`) and the 20 `gameplay/` ids are `DW0928`. A float may also be written in vanilla's modifier form `{"argument": <n>, "modifier": add\|subtract\|multiply\|minimum\|maximum\|override}` (`alpha_blend` takes a `FloatWithAlpha` and is not admitted). `tint` is `{grass?, foliage?, dry_foliage?, water?}`, each `#rrggbb`. `precipitation` (`none`/`rain`/`snow`, required) derives `has_precipitation`, `temperature` and `downfall` (`rain` → true, 0.5, 0.5; `snow` → true, 0.0, 0.5; `none` → false, 0.5, 0.5), and is the fact `DW0496` reads at a cell; `climate {temperature, downfall}` overrides the derived pair and must agree with it (`DW0930`: snow below 0.15, rain at or above). The day timeline stacks over a biome: colours and `sky_light_factor` survive darkened at night, `star_brightness` takes the maximum (stars at noon are possible), and the sun cannot be moved from a place. One no place carries and no beat paints is `DW0930`. | 0.35 |
| `areas[].atmosphere` (opt) | spec-0080: the atmosphere this place stands under **from the first tick** — painted by the bootstrap `atmosphere_bootstrap` function over the area's placed bounds (`AreaPlacement::bounds`) grown by the client's blend reach on every face, sideways within the area's claim (*The blend*, under *World / build output*). The volume is the placement's, never typed. Absent: the horizon's biome. The site plan's `boxes[].atmosphere` is the same capability on the other class of place with a world box. | 0.35 |
| `min_players` (opt, 1..=4) | spec-0018: the party size the delve **requires**. Absent = 1 (a party of one is always legal). `>= 2` emits the **lobby gate**: `tick` recomputes the live count into `#lobby dw.sys`, the class-selection dialog driver is prefixed `if score #lobby dw.sys matches <n>..` (so the delve cannot START short-handed), and unclassed players get a self-updating `x / n` actionbar (`{"score":{"name":"#lobby","objective":"dw.sys"}}` — one emitted line, no per-count strings; a compiler default, not an l10n key). Out of range = `DW0356`; a mandatory-n declaration with no n-way division of labour = `DW0358`. `min_players: 1` emits **nothing** (byte-identical). | 0.6 |
| `respawn_wait {seconds, alone?}` (opt) | spec-0077: a fallen player **waits** before rejoining. `seconds` (`1..=120`) is counted from the *Respawn* click; `alone` (default `false`) decides whether a player who comes back with nobody else present waits too — absent or `false`, a party of one never waits. In a party, the player waits when somebody else is in play and they were not part of a party wipe; while waiting they are a spectator under the observation tag `dw_cutscene`, watch the nearest teammate in play (sneak frees the view while held), see the chrome countdown `delvewright.ui.respawn.wait`, and count as **down** for the party wipe — so a wipe ends every wait at once. Needs a `set-checkpoint` or `bonfire` (else `DW0925`). Emission: the `respawn_wait` row of §3 (verb → emission mapping). Absent = no wait, and every datapack byte is unchanged (the resource pack's lang files carry the chrome row either way). | 0.35 |
| `view_distance` (opt) | spec-0091: **how far a player must be able to see**, in chunks — the server's `view-distance`, declared by the campaign whose far views need it. Absent = the engine's floor, **10** (160 blocks), which every proof is written against; declared in `10..=32` (vanilla serves at most 32; outside the range is `DW0956` on the field). The served radius is `16 × chunks` blocks in every direction — the pinned client draws `min(its render-distance setting, the server's view-distance)` sections around the camera's section, so the declared number is what a player who set their client to it sees, and the extra ring the server sends is margin nobody draws. Every far view is judged against it (`DW0956`): a site-plan `sightlines[]` or `views[]` entry longer than it (validation tier, `dsl::viewdistance`), a showcase camera whose subject — the first solid cell on its central ray, else where that ray enters the loaded scene — is past it, and a cutscene keyframe farther from its aim than it (both build tier). `simulation-distance` never moves with it (stays 10: the scene is force-loaded, and the ticking rim is a proof bound, not a view). Emission: `server.properties` `view-distance=<n>`, and `server/resources.properties` stating the cost (`heap-max`, `players`) — see *World / build output*. Two binding lines: `delvec validate` prints `view distance binding: N chunk(s) (declared | the engine's floor, undeclared) serve R blocks in every direction; S sightline(s) and V view(s) judged against it, B beyond it.`; `delvec build` prints the same head with the showcase cameras and cutscene shots judged and `stated to the host: heap-max <size> for 4 players × <k> chunks each`. | 0.36 |
| `boundary {margin?,message?,returns?}` (opt) | spec-0013: declares a **derived** playable region (union of final placed-piece AABBs, inflated horizontally by `margin` (`0..=64`, default 16; else `DW0321`), unbounded up, floor = lowest placed block − 8). A 1s clock returns any player outside it to the last checkpoint (`dw:cp`) with an actionbar `message` (l10n `world.boundary.message`, English default when absent) + a soft sound; no damage, no item loss. The clock skips a player watching a cutscene (`dw_cutscene`) and a creator flying with the overlay's free camera (`dw_free`). `returns: false` (spec-0092 §10, default `true`) keeps the region and emits no clock: the creator's switch for a world nobody can leave, legal only where the build proves no body walks out of the region or into the open sea (`DW0960`). `horizon:"ocean"` without a `boundary` = `DW0320`. | 0.6 |
| `textures[] {id,replaces,license}` (opt) | spec-0084: the vanilla textures this delve replaces. Each row names one texture the pinned client ships (`replaces`, a `minecraft:` resource location without `textures/` and `.png`, resolved against the census `crates/delvec/data/textures-1.21.11.json`) and the campaign's image for it, `textures/<id>.png` (with an optional `textures/<id>.png.mcmeta` animation sidecar), under a `license` recorded in `LicenseEvidence` — the catalog card's shape, `delvewright_dsl::license`. The build copies the bytes verbatim into `resourcepack.zip` at `assets/minecraft/textures/<path>.png` (and `.png.mcmeta`), through `emit`'s one pack funnel; the files are manifest inputs. Refusals: `DW0190` (id malformed or duplicated), `DW0939` (a path off the census, or two rows replacing one texture), `DW0940` (not an image that texture can be replaced by), `DW0741` (the licence), `DW0309` (no file), and — where the replaced texture is one a humanoid entity model is drawn with in the model-part table — `DW0978` (paint no box of that model samples) and `DW0979` (paint only on faces nobody standing level with it sees) — all at `delvec validate`, and again at build over the same bytes. See [A delve wears its own textures](#a-delve-wears-its-own-textures) and [a sheet is drawn to its own model's boxes](delvec/compiler/skinparts.md#dw0978dw0979dw0980--a-sheet-is-drawn-to-its-own-models-boxes-compilerskinparts--dslnpc-error). | 0.35 |
| `require_resource_pack` (opt, bool) | spec-0084 §11: `true` writes `require-resource-pack=true` into `server/server.properties` (absent/`false` writes nothing, so a campaign that does not declare it is byte-identical). Every server honours the file: the playtest server copies it, and the delve image's entrypoint exports itzg's `RESOURCE_PACK_ENFORCE=TRUE` from it unless the operator named `RESOURCE_PACK_ENFORCE`, which is obeyed in either direction. A player who declines a required pack is disconnected by the server. | 0.35 |

#### Celestial time (spec-0081)

The game's clock is one number, `dayTime`: `time set` writes it absolutely, `time query daytime` reads it modulo 24000, `time query day` divides by it, and the moon's phase is that day modulo 8 (`full-moon` on day 0, `new-moon` on day 4). A time value resolves to one **clock** `{day, daytime}`, and two values are equal when their clocks are. The six positions are points of a body's arc, defined by its **drawn disc** — the client's sky quad (`SUN_SIZE` 30, `MOON_SIZE` 20 at height 100) of which the celestial textures' full-brightness disc is 8 of 32 pixels, half-angles 4.29° (sun) and 2.86° (moon) — and the horizon. The moon stands exactly opposite the sun, so `{"moon": "rising"}` and `{"sun": "setting"}` are one clock.

| Position | Definition | Sun tick | Moon tick |
|---|---|---|---|
| `rising` | the disc's centre on the eastern horizon | 23218 | 12782 |
| `just-risen` | the disc wholly clear of the eastern horizon | 23486 | 12959 |
| `high` | the zenith | 6000 | 18000 |
| `setting` | the disc's centre on the western horizon | 12782 | 23218 |
| `just-set` | the disc wholly below the western horizon | 13047 | 23397 |
| `below` | the nadir — the other body is `high` | 18000 | 6000 |

The twelve ticks are integer constants (`delvewright_dsl::celestial::position_tick`; no trigonometry on the emission path) and `the_position_table_is_the_curve` re-derives each from the vendored `sun_angle` track. **The day rule**: a celestial statement is on the day its `phase` names, or the world's day where it states none; a keyword is day 0 where it states a sky (the world, a design row, a camera) and the world's day where it is a `set-time` cut, which changes the hour and keeps the moon. The world's day is its phase's index, or 0 for a keyword or a phase-less statement. **One authority**: `crates/dsl/data/timeline-day-1.21.11.json` and `timeline-moon-1.21.11.json` are the pinned jar's `data/minecraft/timeline/day.json` and `moon.json`, byte for byte plus one trailing newline (the canonical-form gate's demand), checked against the jar by `tools/maintenance/extract-timelines.py`; the phase names, the sun track and its ease, the sky-light and burn keyframes are read from them. **Binding**: every run prints `clock: <world|set-time> <spelling> -> day D daytime T [(dayTime N)]; sun …, moon …; sky light judged J, game G; burns: yes|no` per stated time and `clocks: 1 world + N cut(s); C celestial, K keyword; phases stated {…}; days {…}` (`compiler::clock`); a keyword campaign prints `0 celestial` and `days {0}` with the full moon spelled on the world's line.

### Stage 2 — `npcs` (casting sheets, stationary)

| Field | Behavior | Since |
|-------|----------|-------|
| `id`,`name`,`area`,`anchor`,`base_entity` | NPC body placed at resolved anchor; `name` → l10n `npc.<n>.name`. | 0.1 |
| `offset` (opt, `[x, y, z]`) | **A body stands at a mark** (spec-0066): the anchor's resolved cell plus this integer block offset, in world axes, default `[0, 0, 0]`. `Plan::body_point` adds it for every body `dsl::body_sites` walks, so the summon, `DW0450`, `DW0511`, `DW0896`, `DW0359`, the camera's static aim at an `npc` subject and the critical path's `talk-to` position all read the mark. A rank of bodies is one anchor and an offset apiece. A mark's cell stays inside the placed piece its anchor belongs to (`DW0897`). A `strike` / `use` trigger at the anchor rides the NPC's hitbox only while the offset is zero, since only then does the body stand on the anchor's own cell. | 0.27 |
| `role` | Enum `quest-giver|flavor` — what the speaking part does. How hard a fight is billed is `tier` on the body that fights (a `waves[]` entry or a stage-5 actor), never a role here. | 0.2 |
| `persona{archetype,speech_style,motivation,…,relationships[]}` | Structured; **excluded** from l10n; relationship refs validated in-stage (`DW0112`). | 0.2 |
| `skin{texture_id,model,hidden_layers?}` (opt) | Switches body to `minecraft:mannequin` whose `profile.texture` is `delvewright:npc/<campaign_id>/<texture_id>`, served by the delve's own resource pack at `assets/delvewright/textures/npc/<campaign_id>/<texture_id>.png` — the delve's directory is stamped on at emission ([One delve, one face](#one-delve-one-face)) and nothing a creator writes carries it. The PNG is read from the campaign dir's `skins/<texture_id>.png`. Missing PNG → `DW0309`; a malformed id → `DW0190` — both read the id as authored. Two bodies may wear one `texture_id`: it names a file, read once and served at one pack texture, while `model` and `hidden_layers` ride each body (spec-0097 §4.3). The PNG is held to the player model it is worn on (`player` for `wide`, `player_slim` for `slim`, from the model-part table): paint no box samples → `DW0978`, paint only on faces nobody standing level with it sees → `DW0979`, both at `delvec validate` (the loader reads `skins/`), judged once per `(texture, model)` pair, and again over the baked bytes at build. `hidden_layers` (spec-0097 §5) lists the overlay layers the mannequin does not draw — `SkinLayer`: `cape`, `jacket`, `left_sleeve`, `right_sleeve`, `left_pants_leg`, `right_pants_leg`, `hat`, the pinned `PlayerModelPart` ids, held equal to the table's `mannequin.layers` by `crates/delvec/tests/skin_parts.rs`; absent or empty emits nothing (a mannequin starts with every layer shown), otherwise both mannequin summons (`emit::mannequin_hidden_layers_nbt`) write `hidden_layers:["<id>",…]` after `profile`, in authored order; a layer named twice → `DW0980`. The bake walks **bodies**, not one stage's list (`dsl::body_skin_sites`), so a stage-5 actor's `skin` is served and refused by exactly this rule. **The generated gear PackTest looks at the body a campaign DRESSES, not the one it typed.** `v06_actor_equipment_<body>` is emitted once per body kind among the equipped actors, taking the first actor of each kind, and the body is in the file name so two kinds give two files. Every summon whose entity id comes from **content** rather than this switch (`npc.base_entity`, `actor.entity`, and the `unleash` twin, which has no skin branch at all) is spliced with `pose:"standing"` when that id names a mannequin (`emit::mannequin_pose_nbt`): a mannequin summoned without an explicit pose serializes it as `DYING`, which the server then fails to encode at save (`Failed to encode value 'DYING'` in a PackTest world's teardown). A non-mannequin entity gains nothing. | 0.4 |
| `deferred` (opt, bool) | **Not** summoned at world init; the NPC's body + hitbox appear only when a `spawn-npc` effect fires, at this same `anchor` (the dual of `despawn-npc`). Default `false`. Never spawned → `DW0197`; a `talk-to` provably ahead of every spawn → `DW0198`. | 0.6 |
| `traversal{locomotion}` (opt) | **What this body can do when it moves** (spec-0034). The author's side of the traversal proof: by default the compiler derives locomotion from the entity id, and this overrides it for one body. **One shared type on every object class that has a body, a position and a compiler-emitted route** — the stage-2 NPC and the stage-5 actor (`dsl::body_traversal_sites`, a closed sum type, so a third body class is a compile error at every consumer until it is handled) — because traversal belongs to the body, not to the verb that first needed it. `ground|climber|flier`; `aquatic` is refused (`DW0455`). **A declaration is a claim, not an opt-out**: it must change a rule's verdict or the build fails (`DW0454`), and it can never reach the error tier (`opens_gates` is derived and unauthorable, and no class is exempt from `DW0452`). It changes which rules examine the body and nothing else — routing is unchanged, every body walks the same ground A*. Emission is byte-identical whatever is declared. | 0.11 |

### Stage 3 — `classes`

1..4 classes. `kit[]` = vanilla item id + count + optional display `name`
(→ l10n `class.<c>.kit.<i>.name`) + optional `carrier` (spec-0018:
`all` (default) or `one`). A kit is per-player gear by construction; `carrier:
"one"` marks a **party-unique** kit item — `class_apply_<c>` guards that one
`give` behind a `#kit_<class>_<i> dw.sys` latch, so exactly one copy enters the
party (the first player to take the class) while the rest of the kit is unchanged. `name`/`blurb` player-visible.

**`flask`** (bool, spec-0016 §1): this kit entry is the class's **recovery item**, and resting at
a bonfire replenishes it to exactly its declared `count`. A campaign that places a
`bonfire` and declares no flask anywhere is `DW0476`. Declaring one also makes
`class_apply_<c>` add a `dw_class_<c>` tag to the player — the pack has to remember
which class a resting player took, since `dw.class` is a trigger the apply resets
and `dw.classed` records only *that* a class was taken. Both are absent from a
campaign that declares no flask, so its class apply is byte-identical.

**`contents`** (obj, spec-0016 §1): **what is in the bottle** — vanilla's
`minecraft:potion_contents` component, modelled field for field:

| Field | Behavior |
|-------|----------|
| `potion` (str, opt) | A 1.21.11 potion id (`minecraft:strong_healing`, `minecraft:long_night_vision`). Strength and duration are *part of the id* (`strong_`/`long_` prefixes) since 1.20.5 — not separate fields. Checked against the pinned `potion` registry (46 ids, inlined in `dsl::registry::POTION_IDS_1_21_11` — complete for the pinned version, so nothing is injected). |
| `effects[]{effect,duration?,amplifier?}` (opt) | The component's `custom_effects`. `effect` is checked against the same status-effect registry wave mobs use; `duration` is in **ticks** (20 = 1 s, 1–1 000 000) and is **required** for a lasting effect, **forbidden** on the two instantaneous ones (`instant_health`/`instant_damage`, applied once on drinking — a duration there is never read); `amplifier` is 0 = level I, 0–255 (vanilla's unsigned byte). Absent `amplifier` is emitted as absent and takes vanilla's own default. |
| `color` (str, opt) | Bottle colour override, `#rrggbb`, emitted as the packed int `custom_color`. Absent → the colour vanilla derives from the effects. |

Legal only on the four items that actually carry the component
(`minecraft:potion`, `splash_potion`, `lingering_potion`, `tipped_arrow` — read
off the pinned `item_components` summary); anywhere else the game would discard
it, so it is `DW0486`. One of those four items **without** `contents` is
`DW0487`: with no component it is the *Uncraftable Potion*, which grants nothing
however it is named — the placeholder flask, as a build error. Everything the
component cannot express is `DW0486` (empty contents, unknown potion/effect id,
out-of-range amplifier/duration, a missing or forbidden duration, a malformed
colour).

Emission: `class_apply_<c>`'s `give` carries
`[custom_name=…,potion_contents={…}]` (fixed field order `potion`,
`custom_effects`, `custom_color`, compact SNBT). **`bonfire_flask` clears and
re-gives through the same two helpers** (`emit::kit_item_predicate` /
`emit::kit_item_components`), so the replenished bottle is the poured-identical
item — the clear is `clear @s <item>[potion_contents={…}]` rather than a bare item
id, which both stops one rest from deleting an unrelated potion in the bag and
guarantees the clear names exactly the stack the next line gives back. If the two
sites ever disagreed the failure would be silent: the clear misses the carried
bottle, the give adds another, and the per-rest budget becomes a stockpile. The
`souls_bonfire_options` PackTest counts through the same predicate and asserts the
bare-id count too, so a rest that hands over a differently-filled bottle fails on
a live server.

**The flask's empties.** A flask whose item has a vanilla remainder (the pinned
1.21.11 `item_components` report gives exactly seven items a default
`minecraft:use_remainder`: `potion` and `honey_bottle` → `glass_bottle`,
`milk_bucket` → `bucket`, the four stews → `bowl`; inlined as
`emit::USE_REMAINDERS_1_21_11`) is given with that remainder restated plus a
`custom_data` mark: `use_remainder={id:"minecraft:glass_bottle",count:1,components:{"minecraft:custom_data":{dw_flask_empty:1b}}}`.
The marked stack is what drinking the flask leaves, so `bonfire_flask` first runs
`clear @s minecraft:glass_bottle[custom_data={dw_flask_empty:1b}]` — for every
flask's remainder item, unguarded by class, since an empty handed over by a party
member is still a flask's. A bottle the player obtained any other way carries no
mark and is never taken; a non-flask potion in a kit keeps vanilla's plain
remainder; a flask with no vanilla remainder (a splash potion) marks and clears
nothing. Cited: the remainder is the component (item_components report,
misode/mcmeta `1.21.11-summary` @ `c976eb3b`, SHA-256 `51b191e1…`). Measured
once on the pinned toolserver (`ghcr.io/stellarfeline/delvewright-toolserver@sha256:31ed5d5a…`,
PackTest 2.4.0) by a probe template: a dummy drinking a potion whose
`use_remainder` carries the mark holds exactly one marked bottle and no potion,
five plain bottles beside it are untouched by the marked clear, and a potion
without the override leaves exactly one plain bottle — each assertion perturbed
red. The per-build `souls_bonfire_mend` template does not drink (see below).

Reserved kit
fields `lore`/`enchantments`/`attributes` are **not defined** → unknown-field
`DW0100`. Kit items carry **no semantics**: a night-vision potion in a kit is
flavor. The `DW0210` dark mitigation is the stage-1 `areas[].mitigation`
declaration only — no kit item's id or display name is read for
`night_vision` (see §4 "Semantics never key on player-facing text").
Because the signal is a declaration, the `DW0210` verdict is language-independent
by construction (ADR-0006) — nothing is threaded past the `--lang` localization
pass.

### Stage 4 — `quest-plan`

Quest DAG skeleton: `depends_on` acyclic (`DW0130`), `finale` declared
(`DW0131`), and the **partition** (spec-0051). A quest declares which half of
the plan it is in, and the derivation keeps the declaration honest in both
directions: the MANDATORY quests must be exactly the finale's `depends_on`
closure, so a mandatory quest the closure does not reach is the convergence
refusal `DW0132` and an optional quest the closure does reach is `DW0866`.
A mandatory quest may not wait on an optional one, by `depends_on` or by
stage-5 `quest-complete` trigger (`DW0867`); the reverse directions are the
ordinary way a strand attaches to the spine. A mainline objective keyed on a
flag only optional content produces is `DW0868`. The closure is
`QuestPlanContent::spine` and the declaration is `QuestPlanContent::optional`
— one authority each. `goal` → l10n `quest.<q>.goal`.

| Element | Fields → behavior | Since |
|---------|-------------------|-------|
| `branch_points[]` | `{id, opens_at, forks_on[], branches[]}` (spec-0025) — the campaign's declared **story forks**. `forks_on` is the flag set this fork owns; `opens_at` the quest at which it opens. Each branch is `{id, flags[], leads_to}`: `flags` is the subset of `forks_on` this branch holds (the rest of `forks_on` is pinned **unset** on it — that half is what makes `DW0484` decidable), and `leads_to` is a **single** field whose id prefix says which kind of terminus it is — a `quest/<kebab>` the branches converge at, or an `ending/<kebab>` this branch runs to. One field rather than two mutually exclusive ones, so "exactly one of them" is an unrepresentable state instead of a rule some diagnostic polices: a value with neither prefix is the ordinary `DW0110`, one naming nothing is the ordinary `DW0112`. Enumerated branches are the **product of the declared points**, so the branch set is authored and small. Empty/absent = a campaign claiming to have no branch, which the compiler then *verifies* rather than assumes (`DW0480`). Proofs: `DW0480`–`DW0485`. | 0.8 |

### Stage 5 — `quests` (+ v0.3/v0.4 gameplay surface)

| Element | Fields → behavior | Since |
|---------|-------------------|-------|
| `trigger` | `campaign-start` \| `quest-complete{quest}`. | 0.1 |
| `guidance` | `{markers?, announcements?}`, each `shown` (default) or `hidden` (spec-0093) — the campaign's default for every objective's `marker` and `announcement`. Omitted = both `shown`, byte-identical to every campaign written before the block existed. **An objective is announced** when it has a `title` and its resolved `announcement` (its own, else `guidance.announcements`) is `shown`; the activation announcement (`New objective: <title>`, the hint's line, the cue), its once-flag `dw.ann_<obj>`, its `tick` line and the `Objective complete` line exist for exactly the announced objectives; the bot's `[dw:complete …]` marker is broadcast either way. **An objective is marked** when its kind summons a marker — a `reach-anchor`, an `interact` with no `prop` — and its resolved `marker` (its own, else `guidance.markers`) is `shown`. Every objective kind carries `announcement`; `marker` rides `interact` and `reach-anchor` only. A `hint` on an unannounced objective is `DW0862`; `announcement: shown` with no `title` is `DW0961`; `marker` beside a `prop` is `DW0962`. A title on an unannounced objective still names the marker's nameplate, the critical path, the render plan and the l10n inventory. | 0.36 |
| Objective `talk-to` | `{npc}`; completes via a stage-6 dialogue option (backward). | 0.1 |
| Objective `reach-anchor` | `{anchor,radius,marker?,announcement?}`; completes inside a block box of half-extent `max(1, radius)` at the anchor cell. **`radius` is a tolerance around the place, and the volume it draws must not reach a second one**: vanilla adjudicates the selector against a body's whole hitbox, so a standing body one course under the box already reaches into it, and every cell of floor the box covers is a cell the objective completes from. Every one of them has to be able to walk to the anchor's own footing without leaving the box — a stair inside it passes, a hall floor three courses down does not (`DW0881`). `marker` (spec-0093) says whether the glowing end rod is summoned; the volume adjudicates either way. | 0.1 / marker 0.36 |
| Objective `kill` | `{wave,after?,requires_flags?}`; completes when wave countdown hits 0. | 0.3 |
| Objective `collect` | `{item,count,anchor,container?,item_name?,fill_count?,…}`; chest at anchor, `inventory_changed` advancement. **Container adoption:** `container` names the anchor whose assembled-world cell holds a `chest`/`trapped_chest`/`barrel` the PREFAB placed — the objective fills that furniture where it stands and the compiler places nothing (the same division of labour `loot` and a trap's dispenser keep; a cell with no container is `DW0438`). `item_name` gives the collected item a display name as a vanilla `custom_name` component — player-visible, so it is l10n-inventoried (`obj.<quest>.<obj>.item_name`) and translated like any other line; adjudication is unaffected because both the completion advancement and the per-tick held check match on ITEM ID. `fill_count` pads the container so it READS full (vanilla fullness is occupied slots, not stack size): the objective's own stack lands in `container.0` and each padding stack repeats it in `container.1`, `container.2`, …, positionally and totally (ADR-0006). Ceiling `1 + fill_count ≤ 27` (`DW0432`, the `loot` rule); a container claimed by both a `loot` entry and a `collect` — or by two collects — is `DW0435`, since positional fills overwrite each other slot-for-slot. An adopted container also joins the layout solver's **required-anchor** set (a pool draw that omitted its carrier would leave the objective nothing to fill) and becomes the `critical_path` step position, because the bot's job is to open *that* block. **Drop gating:** `dropped_by` names the **wave** whose declared `drops[]` provide this item, and provisioning moves from the world to the fight — no chest is placed and no fill is written, because the item does not exist until the boss dies. Waves only: an actor's death is observable by no objective, so an actor-gated collect would be an unprovable claim and is excluded per the no-hack doctrine (an actor may still declare drops; they just cannot gate a quest). Mutually exclusive with `container` (`DW0492`). Two proofs make "kill the boss -> take its key -> open the door" a chain the compiler checks rather than an authoring intention: the wave really declares an `{item}` drop of this item, in at least the count asked for (`DW0492`), and a `kill` objective for that wave provably precedes this collect — through the intra-quest `after` graph or a quest this one `depends_on` (`DW0493`). The `critical_path` step points at the wave's own anchor and carries `dropped_by`, so the bot walks the ground the fight ended on instead of opening a block that is not there. | 0.3 / adoption 0.8 / drop gating 0.9 |
| Objective `interact` | `{anchor,requires_item?,missing_item_hint?,prop?,marker?,announcement?,…}`; interaction entity. **`requires_item` means HELD, not possessed**: `execute if items entity @s weapon.mainhand <item>`. Presenting the item IS the action — a player who right-clicks a sleeping giant with the stake stowed in their pack has not stabbed anything, and an inventory-wide reading (`container.*`) would fire the moment the item was picked up anywhere, whatever the hands were doing. It is a **global semantics, not an opt-in flag**: every `requires_item` in every campaign means held. `missing_item_hint` is the diegetic answer to a click that arrives without the item in hand — one guarded per-player `tellraw`, carrying the objective's own activation guard so an inactive or finished interaction stays silent, emitted before the trigger reset so one click yields exactly one line. Absent = silence. Requires `requires_item` (`DW0437`); l10n key `obj.<quest>.<obj>.missing_item_hint`. `prop{block}` `setblock`s the affordance; `block` accepts a verbatim blockstate suffix `id[key=value,…]`; a block a step fires (a pressure plate, the tripwire) is `DW0957`. `marker` (spec-0093) says whether the glowing lantern is summoned beside the hitbox when there is no `prop`; `marker` beside a `prop` is `DW0962`. **A prop vanilla reports the use of** (spec-0093 §6.5: the lever, every button, the bell — `dsl::blockshape::is_hand_pressed`) **is the whole affordance**: `activate_<obj>` places the block and summons nothing, the objective's `i_<obj>` advancement is `minecraft:default_block_use` at the block's cell (rewarding `i_reward_<obj>` as the player who pressed, into the same `dw.i_<obj>` trigger the tick adjudicates with `requires_item` and the pending guard), the exported `interact` step carries `block` for the harness to right-click, and the generated `verb_interact` and `obj_activate_<obj>` PackTests grant the advancement and assert the block. Any other prop is placed with the hitbox standing in its cell. An unmarked interact with no prop on open air is `DW0963`. | 0.3 / prop 0.4 / held + hint 0.7 / marker 0.36 |
| `after[]` | Ordering (acyclic → `DW0140`). | 0.1 |
| `requires_flags[]` | AND-gate on set flags (puzzle primitive). | 0.3 |
| Effect `when` | **The effect's whole gate, as one object** — `{requires_flags?, forbids_flags?, requires_state?}`, declared once as `Guard` and carried by an effect rather than by a verb. Absent = the always-open gate. Every verb takes it, the staging and souls vocabulary included (`spawn-actor`, `unleash-actor`, `move-actor`, `despawn-actor`, `spawn-npc`, `set-checkpoint`, `bonfire`, `begin-stealth`, `end-stealth`, `sequence`, `campaign-complete`). A gate that can never open on a `campaign-complete` is judged by the completability replay, which applies an effect's gate before it looks at the verb, not by a rule about which verbs may be gated. The verb is `#[serde(flatten)]`-ed beside it, so an effect is still `{"type": "…", …}` with the guard under `when`; `deny_unknown_fields` rides the flattened enum, so a typo in a verb's own field is still `DW0100`. Diagnostics about a gate point at `<effect>/when/<field>`. | 0.20 |
| Effect `audience` (any player-facing effect) | **Who the effect addresses** (spec-0085): `party` (`@a`) or `actor` (`@s` — the one player whose act fired the root: the completing player, the presser, the dying or respawning player, the buyer, the credited killer). An envelope field beside `when`, so it reaches every verb the emitter addresses to players (`Verb::addresses_players`: `narrate`, `give-item`, `play-sound`, `particle`, `give-effect`, `clear-effect`, `damage-players`). **Absent = the root's own answer**: a quest completion `@a`, a `presser` trigger, `on_death`, a respawn, a shop offer and an `on_kill` `@s`, a polled trigger, a trap payload and a shortcut `@a` with no actor — so a document that never writes it emits exactly what it did before the field existed. On a party-fact verb it is `DW0942`; `actor` where emission has no acting player is `DW0503`. | 0.35 |
| Effect `in` (any player-facing effect) | **Narrows the audience to players inside an anchor-centred box** (`{anchor, extent}`, `anchor ± extent`) at the moment the effect fires — the same `StealthZone` a `begin-stealth` zone and a `lethal_volumes[]` region take, through the one `Plan::zone_box` and `emit::box_selector_args`. An envelope field (spec-0085), not a field of any verb; the JSON of the three verbs that carried it before is byte-identical. Composes with `audience` (`actor` + `in` is the actor, if they stand in the box). On a party-fact verb it is `DW0942`. The box is the bodies standing in it, so its selector excludes `dw_cutscene` — a watcher's camera is not a body (`DW0926`) — save on `give-effect`/`clear-effect`, the status-effect site `observer::ALLOWED` names. | 0.35 |
| Effect `happening` | What the beat does to the story (spec-0025), declared on the effect for the same reason. Which verbs are **story nodes** — the eleven `DW0481` demands a `happening` from — stays a property of the verb. | 0.20 |
| `state[]` | `{id, scope, initial?, note?, name?, display?}` (spec-0031/spec-0032/spec-0076) — **runtime state**: a named, integer-valued datum the campaign sets, adds to and clears while the delve is played. What `FlagId` is not: a flag is boolean, party-wide and monotonic (no verb clears one), which is right for "this has happened" and useless for a balance, a floor number, or "a ride is in progress". Unlike a flag a datum **is declared**, because two facts about it cannot be recovered from its use sites: `scope` (`player` = each player holds their own, `party` = one shared value on the `#party` holder, spec-0018) and `initial` (the value it starts at, and the value `clear-state` returns it to — one field, not two, so a datum can never be un-returnable to its own start). `note` is authoring prose, never machine-checked and never shown (the forcing function `cast[].doing` plays for a scene). Emission: one `dw.s_<local>` scoreboard objective per datum; a `party` datum seeded in `setup` (world init is exactly its lifetime), a `player` datum seeded on each player's first tick by `state_seed`, tagged `dw_state` so a relog does not re-seed. Absent = no objective, no function, no tick clause. | 0.10 |
| `requires_state[]` | `[{state, op, value}]`, `op` ∈ `equals` \| `not-equals` \| `at-least` \| `at-most` (spec-0031) — the **numeric third field of the gate**, accepted at every one of the 11 gate-declaring objects in the schema: all five objective kinds, the effect's own `Guard`, `triggers[]`, `traps[]`, dialogue options, cast placements and **shop offers** — where it is the price. The count is re-derived from the schema, never adjusted to fit; the effect counts once because it declares its gate once, as one `Guard` under `when`, so every verb carries the gate and a verb carrying two of the three fields is unspellable. It lives in the gate and not in any verb because the comparison's consumers are exactly the gate's consumers — "this door opens at 500", "this line is withheld below 200", "this lever does nothing while the car is moving" — and generality is decided at the FIRST site (CLAUDE.md). Emission: ` if score <holder> dw.s_<local> matches <range>`, spliced into the guard each consumer already builds, where `<holder>` is `#party` or `@s` per the datum's declared scope. Four operators, not six: over integers `less-than n` is `at-most n-1`, and a second spelling of one thing is a second emission path to keep honest. Absent = no clause. Enforced total by `crates/dsl/tests/gate_consumers.rs`, which enumerates the consumers from the **generated JSON Schema** (i.e. from the types) and reds when any gate-declaring object carries only part of the gate. | 0.10 |
| Effect `set-state{state,value}` | Writes a declared datum to an absolute value (spec-0031). `scoreboard players set <holder> dw.s_<local> <value>`. | 0.10 |
| Effect `add-state{state,amount}` | Moves a declared datum by a **signed** amount (spec-0031); negative counts down. One verb, not an `add`/`subtract` pair: a purse a shop debits and a stake a death forfeits are one operation with the sign flipped. Lowers via vanilla's `scoreboard players remove` (its `add` takes an unsigned operand). | 0.10 |
| Effect `clear-state{state}` | Returns a declared datum to its declared `initial` (spec-0031) — the verb a flag has never had, and the reason a datum is not a flag. It **writes** the initial rather than `reset`ting the score: a reset score is *absent*, and an absent score makes `unless … matches` true, so a cleared datum would silently satisfy a `not-equals` comparison against its own starting value. | 0.10 |
| Effect `give-effect{effect,seconds,amplifier?,hide_particles?}` | Grants a vanilla **status effect** for a stated duration (spec-0031): "blind the party for the ride", "slowness in the deep water". The `mitigation: "night-vision"` clock is the same region-scoped `effect give`. `effect` is any id in the pinned 1.21.11 `mob_effect` registry (`DW0192`, the same registry and the same code a wave mob's `effects[]` answers to; a bare `blindness` normalizes to `minecraft:blindness`). The envelope's `in` narrows it to players inside a box — what makes "blind whoever is riding" expressible without blinding the delve. A grant of a blinding effect (`blindness`, `darkness`) owes the blind-reach proof wherever it is written (`DW0943`); a sight grant (`night_vision`, `blindness`, `darkness`) in a timeline beside a cutscene owes `DW0944`. **`seconds` is required and there is no `infinite` spelling**: a grant whose only removal is a later command is one the player keeps forever whenever that command does not run (a logout, a crash, an interrupted chain), so the hazard is made inexpressible rather than diagnosed, `1..=50000` (`DW0541`, derived from `MAX_POTION_DURATION_TICKS`, not picked again). Emission: `effect give <audience>[<box>] <effect> <seconds> <amplifier> <hideParticles>` — vanilla's full five-token form, from the one formatter the night-vision clock also uses, so nothing is left to a vanilla default a future version could re-pick. No `tag=!dw_cutscene` guard, deliberately: a status effect is not inherently harm, and the engine's own region-scoped grant has never carried one. | 0.10 |
| Effect `clear-effect{effect?}` | Vanilla's `effect clear` (spec-0031). **Not** how a `give-effect` is meant to end — a duration is — so it exists for effects this campaign did not grant: a potion the player drank, a `wither` a mob applied, the whole set at a bonfire. `effect` is therefore optional: absent clears everything, exactly as `effect clear <targets>` does. Pairing it with a still-live grant of the same effect in the same bundle is `DW0540`. Emission: `effect clear <audience>[<box>] [<effect>]`. | 0.10 |
| Effect `teleport{from,to{anchor,offset?}}` | Moves **everything inside a declared volume** to a mark (spec-0031, spec-0066: `to` is a mark — the anchor's cell plus `offset`, refused by `DW0897` when it leaves the anchor's piece). The selector is a **region, never a block**: "whoever is standing on this block" has three different answers for a player half a foot over the edge, a player mid-jump and a player sneaking on the lip, and a volume has one. `from` is the anchor-centred `StealthZone`; `to` resolves to a literal cell at build time, so the emitted command carries absolute coordinates and does no runtime search. Emission is a call into a generated `teleport_<content-key>` function whose whole body is exactly one line: `tp @e[x=…,dx=…,y=…,dy=…,z=…,dz=…,tag=!dw_fixture] <x> <y> <z>` — a named function for the same reason `volley` and `collapse` have one (the body is compiler-proven geometry, and a body that only ever exists spliced into a timeline is a body no runtime test can call). **The selection is total over bodies** — the six box terms plus the one class exclusion every box-narrowed entity selector in the engine carries (`tag=!dw_fixture`, `DW0545`), and no `type=`, no `limit=`, no `sort=`; the effect's own audience is ignored (a box has no party). A machinery-type exemption of the kind `lethal_volumes[]` must carry was considered and **rejected**: a stage-2 NPC is a body plus a co-located `minecraft:interaction` carrying its dialogue, so exempting that type would move the speaker and leave the thing players click behind, in silence — and everyone inside the volume travels, players and entities alike. What stands in its place is a CLASS the object declares (`DW0545`): an engine place whose position is engine state carries `dw_fixture` and is skipped, while an NPC's dialogue hitbox carries `dw_borne` and rides whatever its speaker rides. A place whose cell the compiler knows is refused outright at compile time (`DW0542`), which is available here and was not available to the lethal volume: a volume damages whatever *wanders* in, which the compiler cannot enumerate, while a teleport's harm is to what the compiler itself **placed**. **A teleport is not a rescue**: accumulated fall distance carries across one unchanged (measured Δ `0.0000` in 46/46 trials on the pinned 1.21.11, including teleports 143 and 157 blocks straight *up*; landing damage `floor(fall_distance) − 3`) and is charged in full at the destination, so a platform arriving under a falling player past ~20 blocks of fall is the surface they die on. No fall-distance reset is emitted: what *does* reset it was explicitly NOT measured (`docs/notes/death-and-teleport-spike.md` §5), and a mechanism invented from recall is the folklore this project forbids. The runtime half is a generated PackTest per teleport: it puts a `zombie`, an `interaction`, a `marker`, a `text_display` and an `item` in the volume — the four an exemption list of `LETHAL_EXEMPT_TYPES`'s shape would have dropped, beside a content body — asserts all five are inside the box, calls the campaign's own `teleport_<key>`, and asserts the box is then empty. That half cannot be a Rust test: whether vanilla's `@e[<box>]` really reaches every entity type is vanilla's fact, not the compiler's. Measured red→green on the pinned toolserver — with `,type=!minecraft:interaction` added to the emitted selector the template fails *Expected #tp_left 0, but got 1*. **Where the route proof leans on it** (spec-0083). A `teleport` hosted in a `triggers[]` entry declared `once: false` — in its bundle or one of its `sequence` steps — whose `from` and `to` anchors stand in one area is a **link**: a carry a straggler can take again. Every other teleport is a **gather** — whoever is in the box travels, once — and no proof leans on one. `compiler::link::collect` is the one enumeration (`Plan::links`, `Plan::gathers`). A link is **live** at a path step when the trigger's flag gate and every `when` on the way to the teleport hold under the flags the path holds walking up to the step, and every numeric term compares true against the writes the path has performed by then (the `Flow::walk` replay `DW0879` reads). Legs are proven in path order and a link is taken only where a walk fails: the leg becomes *walk to a stand cell · carry · walk on from `to`*, the last segment retried through the links not yet used, the first decomposition that routes is the leg's route, and a `trigger` step carrying `stand` and `transport` is spliced into the plan's one path directly in front of the leg's end (`nav::take_links`, `Plan::relinked`) — so every proof and artifact reads the same steps, and a campaign whose walks all route takes nothing and builds byte-identically. A link's **stand cell** is a standable cell inside `from` from which the act reaches the body — an eye within `strand::STRIKE_REACH` (3.0) of the body's box for a click, the rule `DW0924` reads; within `range` of the anchor for `approach` — the nearest to the leg's start by route length, ties by cell order. A trigger the path already performs for its openings or its flags that is a link the party can stand in is performed from its stand cell and carries all the same; pressed from outside its box, it carries nobody. A link's geometry is `DW0932`; a teleport, link or gather, that fires while its root's cutscene is still playing is `DW0933`; a link and the layout graph's `carry` edges disagreeing is `DW0934`. | 0.10 |
| `forbids_flags[]` | Negative gate, accepted **everywhere `requires_flags` is** (objectives, `triggers[]`, per-effect, dialogue options, `traps[]`): the element is suppressed while ANY listed flag is set. Every site emits `unless score #party dw.f_<flag> matches 1` clauses (flags are party state and live only on the party holder; unset-safe — flag scores are never pre-initialized, so a `matches ..0` read would wrongly fail on unset). Unknown flags get the same `DW0172` treatment as `requires_flags`. | 0.6 |
| `waves[]` | `{id,anchor,mobs[{entity,count,name?,attributes?,effects?,equipment?}]}`; entity validated (`DW0173`); `name` is a name tag, which marks one body, so a name a wave spawns more than one body under is refused wherever it is worn (`DW0983`); `attributes`/`effects` are validated (`DW0192`). `equipment{head?,chest?,legs?,feet?,main_hand?,off_hand?,body?,saddle?}` — the pinned game's eight equipment slots (`EquipSlot::ALL`; `body` carries horse armour, wolf armour, a llama's carpet, a nautilus's armour or a happy ghast's harness, `saddle` the saddle): slot item ids validate against the pinned 1.21.11 item registry (`DW0143`, the give-item family), and every piece is held to **what the body shows** (`DW0898`, spec-0067): the body table `crates/dsl/data/entity-slots-1.21.11.json` says which slots each of the 92 living entity types draws and for which kind of piece (`armour`, `wings`, `animal`, `item`), and the item table `crates/delvec/data/item-equippable-1.21.11.json` says which slot an item declares and which entities it admits. Each slot is **either a bare item id string or `{item, enchantments{<id>: <level>}}`** (spec-0021) — the plain string stays the plain string on re-serialisation; enchantments emit as the 1.21 `minecraft:enchantments` item component inside the slot compound, ids validated (`DW0433`) and levels range-checked (`DW0434`); emitted as component-era `equipment`/`drop_chances` summon NBT (never legacy `ArmorItems`/`HandItems` — 1.21.11 ignores them) with **drop chance 0 on every slot** (no-grind: wave gear is never lootable). Explicit slots merge over the armed-mob main-hand default (a helmeted skeleton keeps its bow; explicit `main_hand` overrides). A helmet is the sanctioned daylight-undead fix — never `set-time` — and that rule is **enforced**, not merely offered: a burning species staged for a fight whose ground reaches open sky in an hour it burns in is `DW0496`. **`drops[]`** names the DECLARED SUBSET this mob leaves behind — usually one piece, never automatically everything. Two entry forms: `{slot}` (a worn piece; the slot must be one the same mob's `equipment` really fills, and each slot at most once — `DW0490`) and `{item, name?}` (a quest token the fight yields rather than wears; id validated `DW0143`, `name` l10n-inventoried as `wave.<wave>.mob.<i>.drop.<n>.name`). Only an `elite`/`boss` wave may declare drops (`DW0491`) — rank-and-file gear stays unfarmable by construction. | 0.3 / tuning 0.4 / equipment 0.6 / drops 0.9 |
| `loot[]` | `{id,anchor,items[{item,count?,name?,enchantments?}]}` (spec-0021) — contents for a container the **prefab already placed**, the same division of labour a trap has with its dispenser. The compiler never places the container; `DW0431` proves one is really there. Slot assignment is **positional and deterministic**: the nth declared stack lands in `container.<n>` (ADR-0006 — no loot tables, no RNG, no seeded shuffle). Emitted in `setup_finish` as `item replace block … container.<n> with <item>[components] <count>`, so a campaign with no `loot` is byte-identical. An `enchanted_book` stack's `enchantments` are written as `stored_enchantments` (spec-0075; see `give-item` below). `name` enters the l10n inventory as `loot.<id>.item.<i>.name`, exactly like a class kit item's name. Item ids validate against the pinned registry (`DW0143`), anchors against prefab metadata (`DW0142`); `DW0432` caps a fill at 27 stacks and `DW0435` rejects two fills of one container. | 0.6 |
| `lethal_volumes[]` | `{id,region{anchor,extent},message,damage_type?,shown_by?,when?}` (spec-0031; `shown_by` spec-0062; `when` spec-0088) — a declared box that **kills whatever enters it**, worded by the campaign's own strings. A mechanism, not a fiction: the commissioning case was a cliff whose fall must be fatal, and the same declaration is a lava pit, an acid pool, an out-of-bounds plane or the bottom of a lift shaft. The alternative considered and **rejected** for the cliff was making the world's `horizon` void so the fall kills anyway — that changes approved art to obtain a behaviour, and it serves exactly one fiction. `region` is the existing anchor-centred box (`anchor ± extent`), the SAME `StealthZone` type a `begin-stealth` beat and a `damage-players` `in` filter use, resolved through the one `Plan::zone_box`; a private twin with the same two fields would be `tools/ci/check-capability-ownership.py` check C by construction. `message` is required (`DW0512` rejects a blank one) and is l10n-inventoried as `lethal.<id>.message`. `damage_type` is the curated `DamageKind` shared with `damage-players` (default `generic`), so a volume can no more void a held totem than a scripted hit can; it is what words **vanilla's own** death broadcast (`fall` → *fell from a high place*) while `message` says what the place was. Emission: one `function <ns>:lethal_<id>` line on the tick, and a two-line body — `execute as @a[<box>,tag=!dw_cutscene] run function <ns>:lethal_<id>_kill` (which `damage @s 1000 <type>`s, reads the player's health back into `#leth_hp dw.sys`, and `tellraw`s the wording as a `{translate,fallback}` component **only if that health reached 0**), plus one `execute as @e[<box>,type=!minecraft:player,type=!…] run damage @s 1000 <type>` for everything else. The engine's own machinery types (`interaction`, `marker`, `item_display`, `block_display`, `text_display`) are excluded — a volume drawn across a cutscene dolly must not erase the camera. Content bodies (wave mobs, actor puppets, NPCs) are deliberately NOT excluded: a mob that walks into the lava dies, which is the mechanism working. The kill is an ordinary `/damage`, so the vanilla `deathCount` edge (`dw.deaths`/`dw.death_ack`), the checkpoint re-seat (`cp_respawn_check`) and `keep_inventory` see the death they already handle — there is **no second death detector**. Completability: the volume is impassable in the shared nav `World`, **widened by the walker's own half-width** because vanilla selects on hitbox intersection and a body standing in the cell beside a face reaches into the box (`DW0510`; the widening is `metrics::keep_out_box`, one cell horizontally for every body up to two blocks wide); and no place the campaign POSTS something — a respawn seat, an NPC anchor, a `cast` placement, an actor anchor, a wave's seated cell — may hold a body whose hitbox meets one (`DW0511`). **And the volume may not reach a cell the player would read as safe floor** (`DW0891`, spec-0062): the same keep-out, intersected with the cells the party can walk to over the world with lethality removed, is the set a body can be caught from, and every one of those cells must stand on — or in — one of the blocks `shown_by` names. `shown_by` is an optional list of block ids, held to `waterline_y`'s standard: checked per caught cell against the assembled bytes, refused when no caught cell bears one out, and refused at validation when it names a block vanilla does not hurt a body with (`blockshape::HURTING_BLOCKS_1_21_11`; an id the pinned version does not have at all is `DW0193`). It does **not** exempt a cell from the walk graph — a visible hazard is still a hazard, and the router refuses the whole keep-out either way. **`when` is the one `Guard` an effect's `when` is** (spec-0088): absent, the volume is live from world-load to the end; present, it is **live while its gate holds**, and the volume is the eighth gate consumer (`GateConsumer::LethalVolume`). `when: {}` and a `requires_state` term on a `player`-scoped datum are `DW0953`. A staged volume ticks through `function <ns>:lethal_<id>_tick`, whose one line is `execute <terms> run function <ns>:lethal_<id>` with `<terms>` = `Plan::gate_terms` rendered by `GateTerm::clause(false)`; an unstaged volume's tick line is unchanged. The proofs read its gate two ways at each quest configuration (see *A volume live from a story stage* under `DW0510`–`DW0512`): **may be live** (route proofs and visibility) and **is live** (the forced route, `DW0954`). A campaign that declares none emits no tick line, no function and no ledger. | 0.10 |
| `loops[]` | `{id,region{anchor,extent},to{anchor,offset?},requires_flags?,forbids_flags?,requires_state?,counts?,on_cross?}` (spec-0086) — **an endless corridor**: a slab `region` that moves every body crossing it by one fixed offset `d = cell(to) − cell(region.anchor)`, so a hall goes on until the story says it ends. **The gate is the release**: the loop holds while its `requires_flags`/`forbids_flags`/`requires_state` hold (the one `Gate` every consumer reads, `GateConsumer::Loop`), and stands down for good when they stop; a loop with no gate term, a `requires_state` or `counts` naming a `player`-scoped datum, and a `teleport` inside `on_cross` are `DW0949` — the release is a fact about the party, so there is no configuration in which one player is looped and another is not. `counts` names a `party` datum the move raises by one before `on_cross` runs; `on_cross` is effect root R10 (`loop on_cross`, l10n key `fx.loop.<local>`), run under `Audience::Scheduled` from the server source, each effect keyed to a count by its own `when`. Who is moved: every entity in the slab but `dw_fixture` and `dw_cutscene` bodies, each on its own crossing, with its facing, velocity and in-cell position kept (a relative `tp`). The engine proves the move invisible (*Seamlessness*, §4) and the release reachable (the slab is a gated seal, *Runtime region solidity*); the forced route that meets a holding loop is exercised by a `loop` step (`critical-path.json`). A campaign that declares none emits no tick line, no function, no PackTest and no ledger. | 0.35 |
| `timed_gates[]` | `{id,gate,open_ticks,closed_ticks,phase?,crush?,disarm?}` (spec-0016 §4 + addendum) — a gate region on a deterministic open/close clock, so passage is a timing read rather than a permanent state. Emission is a **self-sustaining two-function ping-pong** (`tgate_open_<id>` / `tgate_close_<id>`), each half doing its `fill` and scheduling the other; `schedule` is replace-mode so the clock can never double up, and a timed gate costs **nothing per tick**. The gate is sealed by the prefab at world-load, so the clock's first act is always an OPEN (`phase` holds it shut that many ticks first). Structural errors are `DW0377` (id, a half-cycle of 0, a `phase` at or beyond the cycle, two clocks on one region, or a gate a `shortcut` already owns — a clock would re-seal what `DW0372` forbids re-sealing); a gate anchor with no declared fill `block` is `DW0343`. The design proof is `DW0378`: **not** all-phase passability (a gate that punishes bad timing is the point) but ≥ 20% of the cycle admitting a crossing. **`crush`** (optional, default `false`) makes the closing edge a real portcullis judgement: every player whose position intersects the gate region when it shuts is dealt lethal `damage` by command. It is a *command*, not suffocation, because vanilla's in-wall damage is slow, gear-dependent and escapable — a portcullis that merely inconveniences teaches nothing, and `DW0378` has already proven the window fair, so the penalty may be absolute. Zero per-tick cost is preserved: the judgement rides the closing tick of the ping-pong that already runs. **`disarm`** (optional — souls dossier §5.2) is the ladder's third rung: readable, avoidable, and finally *disable-able*, the way Smouldering Lake's ballista and the Fringefolk chariot can be removed for good. Its shape is `{via, sets_flag}`, **exactly** a trap's `disarm`, and it carries the same obligations: the `via` anchor gets a compiler-owned interaction entity **plus visible hardware** (`DW0420`), it may not be the gate anchor itself (`DW0377`), and it must be reachable from the campaign entry while the gate is SHUT (`DW0393`). Interacting with it suppresses the clock **permanently with the gate resting OPEN** — a jammed portcullis stays up — and permanence is structural exactly as a shortcut's is: no emitted function re-arms the clock and no `close-gate` may name the gate (`DW0389`). A disarmed gate therefore **can never crush**: the judgement rides the closing tick, and the closing tick is inside the suppressed clock. `DW0378`'s 20% duty-cycle proof and `DW0388`'s observability proof are unchanged and apply identically — observability is about the *pre-disarm* read, which is how the party decides the jam is worth the walk. Defaulting to absent keeps a campaign that declares no `disarm` byte-identical. | 0.6 |
| `on_death[]` | `[<effect>, …]` (spec-0031) — the campaign's **death beat**: effects run at the moment a player dies, for that player. Effect root **R7**, so it is visited by `for_each_effect_root` and therefore by every walk defined over it (l10n inventory, the flag model, the timeline, `DW0360`'s anchor seal, the completability model). One bundle per campaign, not one per checkpoint: *where you come back* is a property of a checkpoint, *that you died* is not, and a bundle repeated on each checkpoint would be N copies with N chances to forget one. Phase-specific behaviour uses the ordinary per-effect `requires_flags`/`forbids_flags` gate every root already carries — there is no second gating surface. It exists so that a delve's death consequence ("the purse is dropped where you fell") is ordinary content in a general mechanism rather than an engine feature. Optional in the strongest sense the completability model has — nobody is forced to die — so it registers `close-gate`s only, never an `open-gate` the proof could lean on, and nothing inside it is credited as a flag producer (a mainline reachable only by dying is not reachable). Emission and the death edge: see the `on_death` row under "Effect verbs". | 0.10 |
| `state[].name` | A datum's **player-visible name** (spec-0032). **A named datum is a currency**, and there is deliberately no separate `currencies` section: a purse is a runtime datum the player can see, and "the player can see it" is a property of the datum, not a different object class — a second struct carrying `id`+`scope`+`initial`+`name` would be a private copy of this one. Present ⇒ the datum **announces its new balance whenever it changes, from any cause** — a purchase, a death's forfeit, a stake collected, a plain `set-state` — on its holder's action bar as `<name>: <value>`, with the value carried by vanilla's own `{"score":…}` component so the line is the live balance rather than a number baked at emit time. One tick driver and one `st_show_<id>` function per named datum, keyed on a shadow score seeded beside the datum itself (so joining a world announces nothing). **The announcement belongs to the datum, not to the verbs that write it**, and that is a correctness property rather than tidiness: a readout emitted inside a gated effect carries that effect's gate, and the gate is evaluated AFTER the write it reports — spend your last coin behind `at-least 1` and the balance moves to 0, so the inherited guard stops holding and the one change the player most needs to see is the one they are never told about. Absent ⇒ the datum is silent bookkeeping and nothing is announced. l10n-inventoried as `state.<id>.name`. |
| `state[].display` | A datum's **standing display** (spec-0076). `sidebar` is the one value: the datum's objective is headed with its translated `name` (`scoreboard objectives modify <obj> displayname <component>` — the same `{translate, fallback}` component and the same `state.<id>.name` key the announcement uses, so no new l10n key exists), its value is painted the gold the action bar already paints it (`numberformat styled {"color":"gold"}`), and the objective is put in the slot (`setdisplay sidebar <obj>`) — three consecutive `setup` lines after every datum's objective is declared, once, at world init, which is the slot's lifetime. Nothing per tick: the sidebar reads the objective's own scores, which `state_seed` seeds and every state verb moves. What a player sees: the name as a heading on the right of the screen and one line per player, each showing that player's own balance under their name — the party's purses side by side, which is the reading a per-player purse has in a party. **Declared, never automatic**: a creator may keep a named tally that is spoken only when it moves, and the engine does not decide which of two named datums is the purse. Requires `name` (the heading is the display name) and a `player` scope (`#party` is hidden from the sidebar), and one datum per campaign may stand — all three are `DW0919`. Absent ⇒ none of the three lines, byte-identical to spec-0032. The harness never takes the plain `sidebar` slot (its die-retry ledger reads use `sidebar.team.<colour>`), so a standing display survives the ladder on the validation server; `dw.campaign` stays off every slot. |
| `shops[]` | `{id,anchor,title,marker_item?,offers[{label,tooltip?,requires_flags?,forbids_flags?,requires_state?,effects[]}]}` (spec-0032, DSL v0.10) — an interaction point that opens a list of gated offers. **It is the bonfire rest flow with different buttons**: a `minecraft:interaction` hitbox plus a glowing `minecraft:item_display` armed at world init, a `player_interacted_with_entity` advancement whose reward runs `shop_open_<i>` AS the clicking player (the entity's own `interaction` record names no player, so nothing else can say *who* is buying), a `minecraft:multi_action` dialog whose buttons run `/trigger dw.shop set <n>`, and tick dispatch on `dw.shop`/`dw.shop_at` — the same pair `dw.rest`/`dw.rest_at` uses, because `/trigger` is the only command a non-op player may run. **There is no `price` field**: an offer is the seventh `Gate` consumer and a price is its `requires_state`, so every rule that already governs a numeric comparison (`DW0500`–`DW0503`) governs a price for free. Villager `Offers` is excluded for three independent reasons (a trade cost can only ever be an item, right-click on a villager body is already dialogue's, and the data-driven trade registry post-dates 1.21.11). An offer's own gate makes its `/trigger` handler inert (`return fail`), exactly as a dialogue option's does, so a bot chatting the trigger cannot buy what the gate refuses; **a refusal that speaks is authored as a gated effect** — the purchase behind `at-least <price>`, the apology behind `at-most <price − 1>` — so the engine adds no `refused` field. `effects[]` is effect root **R8**, run with the buying player as `@s`. Strings: `shop.<id>.title`, `shop.<id>.offer.<i>.label`, `shop.<id>.offer.<i>.tooltip`. |
| `stakes[]` | `{id,state,forfeit?,max_live?,on_full?,collect_by?,collected_message,marker_item?}` (spec-0032, DSL v0.10) — **what a death leaves behind, and the one chance to get it back.** A mechanism, not a genre: "souls" is one setting of it. `state` must be a `player`-scoped datum (`DW0520`) — a stake is a personal wager. `forfeit` is `all` (default) / `{proportion,percent}` / `{fixed,amount}` / `none`, computed in integer scoreboard arithmetic and clamped at zero so a death can never hand the player money. `max_live` (default 1) is how many live stakes one player may hold — `0` is the **no-death-cost** configuration (nothing forfeited, nothing placed, no machinery emitted at all) and a larger number with `on_full: keep` is the **memorial at every death site**; `on_full` is `replace` (default; retire the oldest and place a new one — the souls loop) or `keep` (leave the wager alone, forfeit nothing). `collect_by` is `owner` (default) or `anyone`. The marker is an invisible `minecraft:interaction` plus a glowing `minecraft:item_display`, deliberately **not** an item entity (which despawns after 6000 ticks, burns in lava, sinks in the void and can be picked up by anyone), so it inherits `DW0420`/`DW0421`. The ledger is per-player scoreboards — amount, live flag and marker position per slot — because a scoreboard survives death, logout, restart AND chunk unload, which an entity in an unloaded chunk does not. Collection is idempotent under a double right-click in one tick by construction: taking a slot clears its live flag as part of taking it. Strings: `stake.<id>.collected`. |
| Effect `drop-stake{stake}` | Leaves a declared stake for the acting player (spec-0032, DSL v0.10): apply the retention policy, forfeit the declared share, and place the marker at the anchor the **compile-time placement table** gives for where they are. Nothing about the verb says "death" — it is written in `on_death` because that is where a souls-shaped delve wants it, but the mechanism is "leave a recoverable cache where the acting player stands". What *is* death-specific is a property of the `on_death` root, not of this verb: the corpse stands on the death position for every cause measured, so `execute at @s` inside the beat is positioned at the death point with no capture at all (`emit::death_position_capture` emits nothing). Emits one `function <ns>:stk_drop_<id>` call. |
| `ambushes[]` | `{id,at,actors[],trigger,telegraph[]?}` (spec-0016 §3) — **sugar**, not a new runtime mechanism. `parse_campaign` desugars each ambush into an ordinary one-shot `EnvTrigger` named `trigger/<local id>` at `at`, whose effects are the `telegraph` bundle, then a `spawn-actor` per listed actor, then an `unleash-actor` per listed actor. Everything downstream — validation, l10n, the flag/wave producer scans, nav, emission — sees only that trigger, so the sugar has no second code path to drift down and an ambush is exactly as debuggable as the trigger an author would otherwise type. The canonical form of a campaign is therefore its **desugared** form (the section is never serialized), which is what keeps the canonical round-trip idempotent. `telegraph` is **optional and stays optional**: the un-telegraphed ambush is core souls vocabulary and nothing in the compiler asks for a tell. Declaration errors are `DW0375`; the counterplay obligation is `DW0376`. | 0.6 |
| `waves[].respawns_on_rest` | `true` re-seats the wave on every bonfire rest **and** on the first respawn at a bonfire after a party wipe (spec-0016 §1) — the souls contract: progress is kept, the enemies come back. Emission: `spawn_<wave>` additionally sets a seated sentinel `#wseat_<wave> dw.sys`, and `wave_reseat_<wave>` removes every survivor carrying `dw_wave_<id>` unseen (§4 "A body the story removes is never seen to die"; the compiler's one removal, `removal_lines`: a wave any of whose mobs declares `drops` has the declaration stripped off every body first, so a rest yields nothing — see the actors row) then re-runs the wave's own spawn (authored composition, DW0312-proven cells). A rest only re-seats waves the party has actually met — an unmet wave is never conjured. **Stationed re-seat**: a re-seated wave returns to the state it was FIRST seated in, never to the state the party last left it in — a lane wave re-enters its routed patrol from the lane start (`Patrolling:1b` re-applied, `patrol_target` back on waypoint 0, `#lane_<wave>` back to 0, the clock re-armed through the same replace-mode `schedule`), a non-lane wave stands at its anchor under vanilla-local AI with no patrol NBT at all. **Nothing re-seated may pursue across the map.** This holds because `wave_reseat_<wave>` re-enters through the wave's own `spawn_<wave>` and everything stationing a wave is written there and nowhere else, so the spawn state and the stationed state are the same bytes — an invariant the tests pin (`wave_reseat_<wave>` is the removal and one `function <ns>:spawn_<wave>` call, nothing else) rather than a coincidence of the current emission. What earns it is `DW0478`: a bonfire may not stand where a re-seated force can perceive it. Generated PackTests `souls_reseat_stationed` (a rest, driven from the squad hauled onto the party and released to native AI) and `souls_td_lane_reseat` (the re-summon alone, for a lane campaign with no rest point beside its lane). Declaring the field with **no** `bonfire` in the campaign is inert, so it is `DW0370`, not a silent no-op. | 0.6 |
| `waves[].tier` | `ordinary` (default) \| `elite` \| `boss` — what the content **bills** the encounter as (spec-0023). A declaration, never a knob: the compiler is forbidden from *scaling* content from it. Its consumers are the health-bar advisory (`DW0912`, a `boss` that shows the player nothing of how the fight is going) and the drop rule (only a billed fight leaves anything behind). Whether the fight can be WON is nothing the machine asserts and nothing this field feeds: the ladder verifies mechanism at a combat step and taste stays with the owner's playtest. Marking is authored rather than inferred because "this stack looks tuned, so it must be an elite" is exactly the downstream folklore the no-hack rule forbids. Through spec-0016 §1's **undefeated re-seat** the tier also reaches emission in exactly one place: in a campaign with a `bonfire`, a billed `elite`/`boss` wave that does NOT declare `respawns_on_rest` is refreshed by a rest *while it is still standing* — see the `bonfire` row in §3. Billing a wave `boss` **and** `respawns_on_rest` is `DW0499`. Absent ⇒ `ordinary` and omitted from serialisation. | 0.7 |
| `waves[].health_bar` | `{title?, range, color?, style?}` (spec-0073) — **a fight shows its health.** A named vanilla custom boss bar over the wave's total health, drawn for every player within `range` blocks (4..=64, required; outside is `DW0100`) of a live body of the wave, from the moment they enter that range until the last body falls. One type, shared with `actors[].health_bar`. **Declared, never derived from `tier`** — `tier` stays a declaration the compiler scales nothing from; a `boss`-billed fight with no bar is advised (`DW0912`, warning) and no other tier is. `title` absent ⇒ the `name` of the wave's **one** mob entry, under that name's own l10n key; a wave of two entries or more, a nameless entry, or a blank `title` is `DW0910`. A stated `title` is inventoried as `wave.<w>.health_bar.title`. `color` and `style` are strings held to the literals the pinned command tree lists under `bossbar set <id> color\|style` (`DW0911`); absent ⇒ no line, the game's defaults (white, progress) stand. **A boss with adds** is two waves spawned by the same beat, the bar on the boss's; **a body that is many** (`count` > 1 or several entries) is one bar over the sum; **two fights** are two bars, drawn at once. Emission: see *health bar* in §3. Absent ⇒ no bar and byte-identical emission. | 0.31 |
| `waves[].on_kill` | `{fires?: "first-kill" \| "every-kill", effects: [<effect>, …]}` (spec-0074) — what happens **each time a player is credited with killing one of the wave's bodies**: effect root **R9**, one type shared with `actors[].on_kill`, run as the credited player (`Audience::Solo`), so a `player`-scoped datum pays the killer and a `party`-scoped one pays the party once. `effects` is non-empty (`minItems: 1`, refused as `DW0100`). `fires` is the creator's judgement on whether a body that comes back pays again, and has no default: **required** where the fight comes back after the party has met it (`DW0915`) — a `respawns_on_rest` or billed `elite`/`boss` wave in a campaign with a `bonfire`, or a wave whose `spawn-wave` beat can fire more than once (a trigger with `once: false`, a trap that re-arms, `on_death`, a dialogue `on_respawn`, a shop offer, another fight's `on_kill`, a bonfire's `on_rest`, a checkpoint's `on_respawn`, a stealth `on_caught`, or a second seating beat) — and refused as inert where it does not (`every-kill`, `DW0914`); `first-kill` may always be stated. One derivation, `onkill::fight_comes_back`, decides both. A wave no beat seats cannot carry one (`DW0913`). A death nobody is credited with — a fall, a hazard, another mob, a `/kill` — pays nothing and the fight still clears. Absent = no bundle, byte-identical emission. |
| `waves[].lane` | `{waypoints[],aggro_radius}` (spec-0016 §6) — **routed while distant, feral once aggroed**, on vanilla's Raider patrol system (the intended primitive; live-verified 1.21.11, `docs/notes/td-routing-spike.md`). The squad spawns `Patrolling:1b` with one `PatrolLeader:1b` and the **snake_case int-array** `patrol_target:[I;x,y,z]`; a per-wave clock (`lane_tick_<wave>`, 30t, self-terminating) advances a shared waypoint index and per mob releases `Patrolling:0b` whenever a player is inside `aggro_radius`. `aggro_radius` is emitted verbatim as each lane mob's `follow_range` attribute — release radius and perception radius MUST be one number, so a contradicting per-mob override is `DW0381`. Lanes are raider-family only (`DW0382`: vanilla's `#minecraft:raiders` — evoker / illusioner / pillager / ravager / vindicator / witch), squad ≥ 2 (`DW0383`: a lone patroller self-cancels), and a lane pillager must keep its crossbow (`DW0384`: its only attack goal is crossbow-gated, so an otherwise-armed one deadlocks on target acquisition). Declaration errors (no waypoints, an invented waypoint anchor, a repeated consecutive waypoint, `aggro_radius` outside `4..=64`, `lane` + `summon: aggro-edge` together) are `DW0381`; lane geometry is the build-tier `DW0386`. Lane waypoints join the wave's spawn anchor in the layout solver's **required-anchor** set for the wave's area, so a prefab-pool area is guaranteed to draw a piece providing each one — without that a pool draw can legally omit a waypoint's carrier and the lane fails `DW0386` for a reason the author cannot act on. |
| `waves[].summon` | `anchor` (default) or `aggro-edge` (spec-0016 §6). **Aggro-edge = spirit-summoned at the edge of perception**: species without patrol AI never march a lane, so each mob instead materializes on the ring at its own `attributes.follow_range` from the wave `anchor` — which in this mode is the **defended point**, not the spawn point. Candidate cells are standable, walk-reachable and in line of sight of that point, on the one-sided band `[follow_range - 2, follow_range - 1]`, ordered outermost-first: one full block INSIDE the mob's own perception, because ladder evidence showed a mob seated exactly AT the radius acquires a defender at the anchor only marginally — vanilla target acquisition at the boundary is a coin flip, and a summoned mob that acquires nobody stands idle forever, timing out its kill objective. Never beyond perception, never on top of the party. `follow_range` is mandatory here (`DW0385`) — the ring radius is authored, never guessed from a vanilla defaults table the compiler cannot verify. A ring with too few valid cells is `DW0387`, not a silent short spawn. |
| `shortcuts[]` | `{id,gate,unlock,on_unlock[]?}` (spec-0016 §2) — the souls loop-back. **`on_unlock` is effect root R6** (spec-0031): a `Vec<QuestEffect>` emission lowers into `shortcut_open_<id>`, and every effect-root walk reaches it — the l10n inventory, the flag model, `sequence` generation. Deliberately made a root rather than desugared into a trigger the way an `ambush` is: the unlock is polled behind a once-only `#sc_<id>` sentinel and it clears the gate region, so desugaring would have introduced a second detector for one event. **The sealed door is a pressable object:** `setup_finish` arms one `1.02f` interaction per doorway cell, tagged `dw_ws_<id>`, standing in the open air on the **sealed side** (`compiler::wrongside`), and `shortcut_open_<id>` kills them as the bars go up. A `strike`/`use` trigger the author anchors on the `gate` rides those bodies, which is how a wrong-side press gets an answer at all — the compiler supplies the body, the campaign supplies the words. The placement is also the whole side mechanism and needs no player test: a near-side ray reaches a body standing in front of the bars, a far-side ray hits the door and stops. This matters because the answer is typically *"the door cannot be opened from this side"*, which said on the opening side is false, and a false player-facing line is worse than silence. An underivable side is `DW0425`. The `gate` is **sealed from world-load** (the prefab carries the physical fill), and the `unlock` anchor on the FAR side opens it **permanently**. Declaration errors are `DW0371` (malformed/duplicate id, an anchor no prefab provides, or an `unlock` equal to its own `gate`); a gate anchor with no declared fill `block` is `DW0343` (the same rule `close-gate` obeys); a `close-gate` anywhere targeting a shortcut gate is `DW0372` — permanence is structural, there is no re-seal verb to reach for. Geometry proofs: `DW0373` (the long route exists while the gate is sealed) and `DW0374` (opening it strictly shortens the walk to the unlock — the anti-leak proof that makes `unlock` a far-side anchor rather than a label). Every shortcut gate is additionally **sealed for the whole completability model** (`Plan::build` registers it as a `close-gate` at step 0), so `DW0311`/`DW0315`/`DW0342` all prove the delve finishable with no shortcut ever taken. | 0.6 |
| `happening` | `{verb, text, subject?}` (spec-0025) — what this node does to the story. Declared on a **quest**, an **objective**, a **story-weight dialogue option** (one carrying a `set-flag`), and the **eleven story-node effects** (`spawn-npc`/`despawn-npc`/`move-npc`, `spawn-actor`/`despawn-actor`/`move-actor`/`unleash-actor`, `spawn-wave`, `open-gate`/`close-gate`, `campaign-complete`) — and nowhere else, so a `happening` on a `narrate` is an unknown field (`DW0100`) rather than a beat nobody reads. `verb` is the closed ten-word vocabulary `dies` / `survives` / `departs` / `arrives` / `learns` / `believes` / `gains` / `loses` / `opens` / `seals`; `text` is one line of prose the compiler never interprets; `subject` names an `npc/`, `actor/` or `wave/` id (validated, `DW0112`), an `anchor/`, or an `item/<kebab>` label for a story token the campaign tracks by hand. Required (`DW0481`) — the forcing function, generalizing the cast ledger's `doing` from NPC presence to event flow. **Never player-visible**, so it is excluded from the l10n inventory exactly like `doing`, and it is deliberately absent from `QuestEffect`'s hand-written `Debug` — a content key can never move because a beat gained a line of prose. | 0.8 |
| `cast` | `{ "<npc id>": <entry>, … }` (spec-0020) — the **scene ledger**: for every NPC live during this quest, where they are, what they are doing, and what their right-click offers *for this quest's duration*. An entry is the bare keyword `"dead"`/`"offstage"`, one placement object, or a **list** of placements (per-branch casts, each gated by `requires_flags`/`forbids_flags`). A placement is `{at, doing, dialogue, requires_flags?, forbids_flags?}`: `at` is an anchor, a mark `{anchor, offset}` (spec-0066 — the spelling for a body standing at an offset, which `DW0461` compares anchor and offset both), or `"offstage"`/`"dead"`; `doing` is free prose the compiler never checks (required anyway — it is the forcing function, and stage 6 writes the NPC's lines against it); `dialogue` is a stage-6 root id, `{"barks": [...]}`, `"none"`, or `"unchanged"`. **The declaration is the gate** — see "Cast-ledger dispatch" in §3. Barks enter the l10n inventory as `cast.<quest>.<npc>.<branch>.bark.<i>`; `doing` deliberately does not (it is never shown to a player). A cast-declared root counts as a **dialogue entry point**, so `DW0120` reachability is measured from the tree `root` plus every ledger root — without that, retiring a premise root by swapping to a later one would make the later one unreachable. Proofs: `DW0460`–`DW0467`, plus `DW0846` and `DW0858`. | 0.7 |
| `triggers[]` | `{id,at?,prop?,on:strike\|use\|approach{range}\|step\|strike-npc{npc}\|strike-assembly{assembly},requires_flags?,forbids_flags?,once?,effects[]}`. `at` names a **place** and is required for `strike`/`use`/`approach`/`step`; `strike-npc` names a **character** and `strike-assembly` an **assembly** (spec-0082), and neither takes an `at` — either mismatch is `DW0194`, because an ignored anchor reads as meaningful and does nothing. A `strike-npc` target that stage 2 does not declare, or a `strike-assembly` target `assemblies[]` does not, is `DW0112` (the trigger would ride nothing); a `strike-assembly` on an assembly with no `hitbox` is `DW0936`. A `strike-assembly` is **melee only**: the hitbox is a `minecraft:interaction`, and an arrow passes through one without writing its `attack` record (spec-0082 §8 row 5). Bad/dup/`range 0` → `DW0194`. A trigger is armed while every `requires_flags` flag is set AND no `forbids_flags` flag is set — e.g. a retaliation trigger armed by `flag/sealed` that stands down the moment `flag/asleep` is set (the wake beat takes over), with no re-arm plumbing. `once: false` is what makes a `teleport` the trigger hosts a **link** the route proof may take (spec-0083, effect `teleport`); a teleport in a `once` trigger is a gather. | 0.4 / forbids 0.6 **`step`** fires on a player stepping onto the anchor's cell, which must hold a block a step fires — a pressure plate or the tripwire string (`dsl::stepped_blocks`); the piece places it, and a cell holding anything else is `DW0917`. **`prop`** (spec-0093 §6.5): the visible block a `use` or `strike` trigger acts on, placed at `at` in `setup_finish` by `setblock`. A `use` on a block vanilla reports the use of — the lever, every button, the bell (`dsl::blockshape::is_hand_pressed`) — has the block as its whole body: no `minecraft:interaction` is summoned, the tick polls nothing, and the trigger's `press_<id>` advancement is `minecraft:default_block_use` at the block's cell, rewarding `press_<id>` (the presser dispatch, whichever audience the trigger addresses — a party bundle run as the clicker still addresses `@a`). Any other block, and every `strike` (a left-click vanilla reports no block for), is placed with the hitbox fitted over it in its cell. A prop on an event with no cell of its own is `DW0964`; a `use` or `strike` with no prop on open air is `DW0963`. The exported `trigger` step carries `block` for the harness to right-click. |
| Effect `open-gate` | Fills gate anchor to air. | 0.1 |
| Effect `close-gate{anchor,sealed_hint?}` | The physical dual of `open-gate`: fills the gate anchor's region with the block the anchor's prefab metadata declares (basalt boulder, iron bars), re-sealing an opened threshold into a wall. A gate anchor that declares no `block` is `DW0343`. Same anchor-existence check as `open-gate` (`DW0142`). Per-effect `requires_flags` like the other per-`@s` verbs. **`sealed_hint`** is what the seal *says* when a player right-clicks it: a seal is a wall the party walks back to and presses, and the press has to answer. **The answer is the campaign's**: a seal nothing answers is `DW0429`, discharged by this field or by a `use` trigger on the gate. Authored, the line is l10n-inventoried under `<effect-key>.sealed_hint` and translates like a `narrate`. The answer belongs to the **anchor**, so two firings on one gate must agree (`DW0423`), and nothing else may hold a hitbox inside the sealed region (`DW0422`). `Debug` is derived: nothing about an effect's rendering names a function. | 0.6 (`sealed_hint` 0.8) |
| Effect `campaign-complete` | Sets `dw.campaign`; finale fanfare. `ending` (opt, spec-0025) NAMES this ending — there is no separate `endings` section, the set of endings is exactly the set named here, the same rule flags follow — so a stage-4 branch can declare which ending it runs to and `DW0482` can state *which* ending a branch reached rather than merely that something ended. Validation metadata: never emitted, so a campaign that names none is byte-identical. | 0.1 / `ending` 0.8 |
| Effect `spawn-wave` | Summons wave mobs (AI on), tag `dw_wave_<id>`. | 0.3 |
| Effect `give-item{item,count,name?,carrier?,enchantments?}` | Grants item. `enchantments` (spec-0075) is a `loot` stack's field — id → level, checked by `DW0433`/`DW0434` at every effect root and any nesting depth — so a shop offer can sell an enchanted sword and a quest can hand over an enchanted book. | 0.3 |
| Effect `set-flag{flag}` | Sets `dw.f_<flag>` (per-player). | 0.3 |
| Effect `narrate{text,style?,sound?}` | chat/title/subtitle/**art**/**actionbar**; `text` → l10n; `sound` validated (`DW0326`); `art` = the `delve:art` pixel-banner font, glyph-checked (`DW0328`), width-checked (`DW0330`); **`actionbar`** = the reply strip above the hotbar, the channel every compiler-written reply already used, not width-checked (vanilla neither wraps nor truncates it, and a reply is a fragment rather than a banner). | 0.4 / art 0.6 / actionbar 0.11 |
| Trigger `audience: party\|presser` | Who a trigger's bundle addresses. `party` (default) is polled on the tick with no executor and addresses `@a`. `presser` is dispatched by a `minecraft:player_interacted_with_entity` advancement and runs **as the player who right-clicked**, so `@s` is the presser — which also makes a `player`-scoped `requires_state` legal there (`DW0503` asks the site, not the root class). A `presser` **`step`** is polled `as` each player in the plate's cell, so `@s` is the player who stepped on (see *A step names its actor*). Vanilla names the player for a right-click and a step only, so `presser` on a `strike` / `strike-npc` / `strike-assembly` / `approach` is `DW0427`. | 0.11 |
| Effect `set-block{anchor,block}` | `setblock` at anchor; base block id validated (`DW0193`). `block` accepts a verbatim blockstate suffix `id[key=value,…]`. | 0.4 / state 0.6 |
| Effect `fill-region{region,block}` | **Fill a declared region with a block at runtime** (spec-0031). `region` is the existing anchor-centred box (`anchor ± extent`, the same `StealthZone` a `begin-stealth` zone, a `damage-players` `in` filter, a `collapse` ceiling and a `lethal_volumes[]` region use, resolved through the one `Plan::zone_box`); `block` is registry-checked with the same `DW0193` `set-block` gets. Emission is one unfiltered `fill <lo> <hi> <block>`. This is the **general spelling** of the capability `open-gate`/`close-gate` carried privately: those two are this operation with the box and the block read off a prefab gate anchor instead of authored, `set-block` is the one-cell case at a point anchor, and `open-way` is the same operation with all three read off a placed piece's contract. All of them lower through `emit::fill_region_command` and are modelled by one completability rule (`plan::RegionEvent`), so a fourth consumer inherits the proof instead of re-deriving it. Completability: from the DAG point the effect fires at, the cells become what the **block** makes them — **solid** for a block that is a full cube, exactly as a `close-gate` seal is (a critical path that must cross afterwards fails `DW0311`), and **flooded** for `minecraft:water` / `minecraft:lava` (`assembled::is_fluid`): impassable, and never floor, because nothing stands on a fluid. A fill carries no `replace` filter, so a fluid fill over floor takes the floor away; a forced leg that needed that footing is `DW0544`. **Whose firing it is also counts**: a fill fired from a root the party can skip — a trap payload, a shop offer, a death bundle, a shortcut's far side — still SEALS the region (the proof must survive it) but does not lay footing the forced path may stand on, because the same block that walls a doorway floors the cell above it and only the first reading is conservative. A forced leg whose only footing comes from such a fill is `DW0546`. | 0.10 |
| Effect `clear-region{region}` | **Clear a declared region to air at runtime** (spec-0031) — the physical dual of `fill-region`, and the general spelling of what `open-gate` does to a gate anchor's box. Emission is the same `fill` with `minecraft:air` and, unlike `open-gate`, **no `replace` filter**: an author's clear empties the box rather than scrubbing one block id out of it. Completability: the cells are **passable** from the DAG point the effect fires at — the half no gate could ever exercise, because the assembled model already holds every gate cell open unconditionally. Two limits are stated rather than discovered. (1) A cleared cell the model already floods stays impassable: clearing a block does not remove water, it lets the water in. A clear that *opens* a dry box into adjacent water is not modelled — re-deriving the flood needs the block map the collision view does not carry — so that campaign's route proof is optimistic there, and a runtime `fill-region` of a fluid is a second way to put water next to such a clear. (2) A clear never removes cells another proof has forced solid (`nav::World::pinned`): a `collapse`'s debris, an ambush's occupied cells, a timed gate's shut span. Clearing a region says the blocks the campaign put there are gone, not that another proof's hazard never happened. | 0.10 |
| Effect `open-way{piece,way}` | **Open a placed piece's contingent way at runtime** (spec-0042). A piece's spatial contract may declare a traversal edge whose crossability depends on a named region — `laid` (empty as built, opening fills it with the way's block) or `cleared` (built in that block, opening voids it). The prefab checker proves, on the bytes as shipped, that the edge is severed and that applying the delta joins it; this verb is what applies it. **It carries no region, no block and no direction**: all three are read from the carrying piece's exported `spatial_contract.edges[].way`, so the effect and the building cannot disagree about what a way is — two authorities plus an equality check is the defect the shape avoids, not a variant of the fix. `piece` is a prefab id and must name exactly one PLACED way: a piece placed twice puts two breaks in the world at different coordinates, and a reference matching none or several is `DW0547`. Emission is one unfiltered `fill <lo> <hi> <block>` per box of the way (air for a `cleared` one), through the same `emit::fill_region_command` `fill-region` / `close-gate` / `set-block` lower through. Completability: the way is **shut until this fires**, and from that DAG point the cells are what the block makes them — the same one `plan::RegionEvent` rule, fed from prefab metadata instead of an authored box, so the forced-footing rule (`DW0546`) applies unchanged rather than being restated. Required content standing beyond a way no forced opening precedes is `DW0548`; every staged way's disposition is enumerated in `validation/ways.json` and a way the placed world declares but cannot stage is `DW0549`. | 0.12 |
| Effect `when.requires_flags[]` / `when.forbids_flags[]` (any effect) | Per-effect gates, declared under the effect's `when` (above): `requires_flags` wraps the effect's command(s) in `execute if score #party dw.f_<flag> matches 1 … run …`; `forbids_flags` adds `unless score #party dw.f_<flag> matches 1` clauses to the same guard (suppressed once any listed flag is set). Valid on every verb, `campaign-complete` included; refs resolve like objective flags (`DW0172`). | 0.6 |
| Effect `despawn-npc{npc}` | Removes NPC + hitbox, unseen: a call to `despawn_npc_<id>`, one function per NPC a `despawn-npc` names (§4 "A body the story removes is never seen to die"). | 0.4 |
| Effect `spawn-npc{npc}` | The dual of `despawn-npc`: summons a stage-2 NPC — body + interaction hitbox + name display — at its declared anchor, via the **same** `npc_summon_commands` authority world init uses. Idempotent (per-entity tag guards), so a re-fire never doubles a body. Also a dialogue effect. **`spawn_npc_<id>` is emitted for every NPC any `spawn-npc` site names**, not only `deferred` ones: the registration walk IS the call walk (quest/trigger/trap effect trees at any nesting depth, plus every dialogue option's `spawn-npc`), so a call and its callee can never disagree. `spawn-npc` on a non-deferred NPC is the legal way to bring a character back after a `despawn-npc`. For an NPC already standing at its mark the entrance is exactly the no-op it reads as; a campaign that fires no `spawn-npc` and defers nobody emits nothing here. | 0.6 |
| Effect `move-npc{npc,to{anchor,offset?},speed?,on_arrive[]?}` | The destination is a mark (spec-0066): `to.anchor` resolves through `plan::body_station` (`BodyScope::Beat`: the area the firing quest bundle plays in, then the npc's home, then an unambiguous crossing) — a name two areas answer to and neither settles is `DW0859`; the snap to a standable cell starts from the mark's cell, the anchor's plus `to.offset`, and a mark leaving the anchor's piece is `DW0897`. The walk driver is named `mv_<npc>_<anchor>` and, for a non-zero offset, `mv_<npc>_<anchor>_o<x>_<y>_<z>` (a negative component spelled `m<n>`), so two walks to one anchor at two offsets are two drivers. A*-planned per-tick tp through walkable space; unroutable → `DW0307`. `on_arrive[]` fires once on the driver's final-waypoint tick — **exact parity with `move-actor.on_arrive`**: same arrival detection, same execution context (`mv_arrive_<key>` mirrors `ma_arrive_<key>`), and every deep effect walker (flag/wave producer scans, consumer-ref checks, checkpoint/stealth collector, l10n inventory + localization, nav flattening, emission) recurses into it via the shared `nested_effect_lists` authority. Lets content gate a beat on walk *completion* (`on_arrive` → `set-flag`) instead of fire-and-forgetting the walk. | 0.4 / `on_arrive` 0.6 |
| Effect `cutscene{shots[]}` / `cutscene{path[],seconds,look_at?}` | Two-camera spectator dolly; clip → `DW0308` (checked **per shot**, over both the authored polyline and the client-rendered keyframe chords); a shot panning over the 6°/tick angular budget → `DW0347`. Two mutually exclusive spellings, normalized to one shot list: multi-shot `shots: [{path[],seconds,look_at?}, …]` or the single-shot `path`+`seconds` fields — mixing/omitting both, or a shot with an empty `path`, is `DW0199`. Shots play back-to-back inside ONE save/restore bracket (hard cut). `look_at {anchor,offset?}` aims every dolly camera at that world point; absent = face along the direction of travel. **`shot_style` (spec-0015)**: a shot may instead declare a style preset + `subject {anchor|npc|actor, offset?}` (+ optional `dist`, `bearing`, `degrees` (orbit only), `subject_b` (two-shot only)); the compiler expands the style deterministically into the dolly + aim + duration (see "Shot styles" below). Explicit `path`/`look_at`/`seconds` always override the corresponding expanded part. Style-shape violations are `DW0348`; a `side-track`/`low-follow` whose subject has no sibling `move-npc`/`move-actor` (same effect group or sequence) is `DW0349`; an unknown subject npc/actor is `DW0112`. **`party` (spec-0095)**: `present` (default) or `absent`, in both spellings — `present` shows every player in play by a stand-in where they stood for the cutscene's length (see "The party stays in the scene" below); `absent` places none, and the cutscene's functions are named `cs_<bare>_absent`. | 0.4 / `look_at`+`shots`+`shot_style` 0.6 / `party` 0.36 |
| Effect `set-time{time}` | Instantaneous dimension-global cut to a keyword or a celestial statement (the stage-1 `time` row). The cut changes the hour and keeps the world's moon: a keyword resolves on the world's day, a celestial statement on its `phase`'s day or the world's; `time set <kw>` for a keyword on day 0, `time set <ticks>` for `dusk`/`dawn`, `time set <day × 24000 + daytime>` otherwise (`noon` in a world whose moon is new → `time set 102000`). A `phase` equal to the world's is `DW0931`; persists (cycle frozen). | 0.5 / celestial 0.35 |
| Effect `set-weather{weather}` | Instantaneous dimension-global cut (`weather <kw>`); persists (cycle frozen). | 0.5 |
| Effect `play-sound{sound,at?,volume?,pitch?}` | Plays a sound event; `sound` validated (`DW0326`); `at` = `{anchor, offset?}`\|`{players, offset?}` (default `players`) — the anchor form is a mark (spec-0066), playing at the anchor's cell plus `offset`; the `players` form's `offset` (spec-0085) is integer blocks in **the listener's own frame** — `+x` left, `+y` up, `+z` the way the listener faces, the pitch flattened so behind stays at ear height (`[0, 0, -3]` is three blocks behind); `{actor}` parses and is refused (`DW0335` — no live-actor position resolves); positional or per-player. | 0.6 |
| Effect `firework{at,flight?,explosions}` | Fires a `minecraft:firework_rocket` from a mark (spec-0068): `at` is the same `{anchor, offset?}` `play-sound` takes, `flight` is 1, 2 or 3 (default 1 — the three the game crafts), `explosions` is 1–7 bursts of `{shape, colors, fade_colors?, trail?, twinkle?}` with `shape` one of the game's five (`small_ball`, `large_ball`, `star`, `creeper`, `burst`) and every colour a `#rrggbb` literal. Out-of-range flight, an explosion list outside 1–7, an empty `colors` and a malformed colour are each `DW0100` — the bound the exported schema states, restated at the document tier because serde does not enforce it. The burst stands 8, 18 or 32 blocks over the mark and `DW0899` proves it bursts in open air, clear of every posted body. A display of many rockets is a `sequence` of these. | 0.29 |
| Effect `particle{particle,at,count?,spread?,speed?}` | Spawns particles (spec-0085), the next one-shot point effect after `firework`. `particle` is a vanilla particle-type id (`minecraft:` optional) validated against the pinned registry `crates/dsl/data/particles-1.21.11.json` (115 types, derived from the server jar by `tools/maintenance/extract-particle-registry.py`); an unknown id, or one whose type takes options (18 of them — `dust`, `block`, `item`, `flash`, …), is `DW0941`. `at` is a mark (`{anchor, offset?}`) or the literal `players` — at each addressed player. `count` (default 1, minimum 1 — `0` is `DW0100`, vanilla's spelling of one particle with a velocity), `spread` (`[x, y, z]` standard deviations, default `[0, 0, 0]`), `speed` (default 0). Always emitted in **`force`** mode, its viewers the effect's audience; a `minecraft:elder_guardian` at `players` is the full-screen face. | 0.35 |
| Effect `lightning{at}` | Strikes a `minecraft:lightning_bolt` at a mark (spec-0092): `at` is the same `{anchor, offset?}` the other point effects take, and is the verb's one field. The bolt stands at the mark's cell centre on the mark's plane, so it strikes the block under the mark. It is the real bolt — every client in tracking range draws it and the sky flash and hears the thunder — and it hurts: `DW0958` refuses one whose reach holds a posted body and `DW0959` one whose struck block the game rewrites (a lightning rod, weathering copper). It lights no fire, because every delve seals `fire_spread_radius_around_player` at `0`. A party fact: `audience` and `in` on it are `DW0942`. A storm is a `sequence` of strikes. | 0.36 |
| Effect `damage-players{amount,damage_type?}` | Deals `amount` half-hearts of damage over vanilla `/damage` — a real `on_caught`/souls consequence. **Audience (spec-0018)**: on a quest beat / trigger the hazard is a fact about the delve, so it hits the whole party (`execute as @a[…] run damage @s …` — `/damage` takes ONE entity, see §1); inside a solo `on_caught`/`on_respawn` bundle it hits exactly that player (`execute if entity @s[…] run damage @s …`). `amount ≥ 40` is lethal through golden apples. The envelope's `in` narrows to acting players inside its box (anchor `DW0142`). `damage_type` is a **curated enum** of vanilla types that respect `keepInventory` and do NOT bypass totems (no `out_of_world`/`generic_kill`), default `generic`; an unknown value is `DW0100` (needs no registry). Named `damage_type`, not `type`, since the effect enum is internally tagged on `type`. Per-effect `requires_flags` allowed (per-`@s` verb). Every form is guarded by `tag=!dw_cutscene` — a player watching a cutscene is never harmed (§4). | 0.6 |
| Effect `set-checkpoint{anchor,on_respawn?}` | Party-wide respawn point: `spawnpoint @a` at the anchor + `storage dw:cp pos` mirror + the active-checkpoint marker. Monotonic by quest order. `on_respawn[]` = per-player effects re-run on respawn while active (vanilla `deathCount` detection). A death at the active checkpoint **re-seats** the respawned player on its cell rather than trusting vanilla's respawn lookup, which silently falls back to the world spawn on a cell it dislikes (§emission). Proofs `DW0315`/`DW0316`. Also a dialogue effect. | 0.6 |
| Effect `bonfire{anchor,on_rest?,prompt?,rest_label?,save_label?,rest_tooltip?,save_tooltip?}` | The souls sibling of `set-checkpoint` (spec-0016 §1). The effect only **arms** a rest affordance at the anchor (a `minecraft:interaction` the party right-clicks; the campfire is prefab dressing) — the respawn point moves when the party actually **rests**. Right-clicking opens a dialog with **exactly two options** (a campfire must be a real interaction, never a lazy "arrive" objective): *rest and save* runs the full loop (**every living player** is restored, whoever sat down — `execute as @a unless data entity @s {Health:0.0f} run function <ns>:bonfire_restore`: healed, fed, cured, every carried item mended, the flask refilled and its empties taken back; a body on its death screen is skipped and comes back at this fire through the respawn's flask refill — the checkpoint moved, every `respawns_on_rest` wave re-seated, `on_rest[]` fired); *save only* moves the checkpoint and does nothing else. **Mending** (`bonfire_restore`): each carried slot — `armor.head`/`chest`/`legs`/`feet`, `weapon.offhand`, `container.0`–`35` (41 lines, `emit::CARRIED_SLOTS`) — gets `execute if items entity @s <slot> *[damage~{damage:{min:1}}] run item modify entity @s <slot> <ns>:bonfire_mend`, and the build emits `datapack/data/<ns>/item_modifier/bonfire_mend.json` = `{"function":"minecraft:set_damage","damage":1.0}` whenever a bonfire exists. It repairs what the player carries now, in place — never a re-kit — and the guard keeps empty slots, stacks and items without durability away from the modifier (unguarded, `set_damage` leaves them unchanged but logs `Couldn't set damage of loot item` per slot, measured). A death-respawn at a bonfire refills the flask but does not mend. Live proof: the `souls_bonfire_mend` PackTest damages a shield (off-hand), a chestplate (armour), a sword (hotbar) and a bow (`container.35`), adds an undamaged sword, seven bread, five plain glass bottles, a kit flask (its marked `use_remainder` read back through the server's own component predicate) and two marked empties, runs the real `bonfire_pick_rest_<i>`, and asserts four mended, the rest untouched, zero marked empties, five plain bottles, and the flask at its declared count; perturbing the build (mend lines removed; take-back widened to bare `glass_bottle`; take-back removed) reds it on the matching assertion. It does not drink: across 42 full-suite runs of drinking variants of this template, 17 read the potion still undrunk in the hand (cause not found), so the consume half is the one-off probe recorded under Stage 3 `flask`. **A respawn at this bonfire resets the scene only after a party wipe.** The tick, just before the death edge, counts the living (`#alive dw.sys`, by the edge's own predicate `unless data entity @s {Health:0.0f}`); when it is 0 and `@a` (which matches a corpse and never a disconnected player) is non-empty, it latches `#wipe dw.sys 1` and tags every body `dw_wiped`. `cp_on_respawn_<i>` refills the respawning player's own flask and runs `cp_reset_<i>` only for a `dw_wiped` body: `party_reseat` (every re-seat line) while `#wipe` is 1, then `on_rest[]` at the solo audience. `cp_respawn_fire` ends by spending `#wipe` and the body's `dw_wiped`, so the first respawn after a wipe re-seats once and a death in a party still fighting re-seats nothing. Alone, every death is a wipe. `#wipe` is seeded 0 at setup; a campaign without a bonfire emits none of it. Tests: `souls_bonfire` (`a_respawn_resets_the_scene_only_after_a_party_wipe`, `a_rest_restores_every_living_player`), `v06_checkpoints` (`a_campaign_without_a_bonfire_carries_no_party_wipe_latch`). Arming is idempotent (guarded summon), resting is deliberately repeatable (unlike the one-shot trap disarm). `prompt` / `rest_label` / `save_label` (v0.8) author the three dialog strings; absent, the compiler bakes its canonical English (`Bonfire` / `Rest and save` / `Save only`) exactly as `world.boundary.message` does — an authored string is inventoried (`fx.….rest_prompt` / `.rest_label` / `.save_label`), translates like any other player-visible line, and the two labels carry the `DW0331` button budget because they are drawn on the same button a dialogue option is. `rest_tooltip` / `save_tooltip` (spec-0078) are each button's optional hover tooltip, emitted by the shared `dialog_button` (see *Dialog buttons* below) beside its label and inventoried as `fx.….rest_tooltip` / `.save_tooltip`; absent, the button carries no `tooltip` key. A campaign with a bonfire whose class kits declare no `flask` is `DW0476`. Proofs are inherited: a bonfire is collected as a checkpoint, so `DW0316` (standable) and `DW0315` (no stranding — rooted at the ARMING beat, the earliest rest) apply unchanged. Quest/trigger effect only (not a dialogue effect). | 0.6 / the two-option dialog + authored labels 0.8 / button tooltips spec-0078 |
| Effect `begin-stealth{zones[{anchor,extent}],on_caught?,grace_ticks?}` | Per-tick: every player must be inside some zone — zone presence alone = hidden (no sneak requirement, which collides with the spectator cutscene camera); exposed for `grace_ticks` (default 20) → `on_caught`. Zone standable/reachable proof `DW0327`; onset-survivability proof `DW0355` (a beat whose `on_caught` punishes must be escapable inside `grace_ticks` from where the player provably stands when it arms, and from every checkpoint that can respawn them into it). | 0.6 |
| Effect `end-stealth` | Ends the active stealth beat (clears the session marker). | 0.6 |
| Stage-5 `actors[] {id,entity,name?,skin?,anchor,offset?,facing?,vulnerable?,equipment?,attributes?}` | Scripted NoAI/Silent/no-loot puppets, summoned at the mark `anchor` plus `offset` (spec-0066, default `[0, 0, 0]`; the same resolution a stage-2 npc's `offset` takes, through `Plan::body_point`, and the same `DW0897` bound), tag `dw_actor_<id>` (+ puppet marker `dw_pup_<id>`); `Invulnerable` unless `vulnerable` (then knockback-immune); `skin` → mannequin, with its PNG baked into the resource pack and its absence refused (`DW0309`) exactly as a stage-2 npc's — a skin is a property of the body, and one walk (`dsl::body_skin_sites`) answers for both classes. Summoned by `spawn-actor`, not at load. `equipment` (spec-0021) takes **the same shape a wave mob's does** — one type, one rule set, so the two surfaces cannot drift — and is emitted into BOTH the puppet summon and the unleashed twin's NBT: unleashing swaps the body, not the costume, so the dormant elite the party has been circling is visibly the armoured thing that stands up. **A `skin` is a costume, not a different object**: `vulnerable`, `attributes` and `equipment` are properties of the body and are computed once, above the mannequin/mob branch in `emit::actor_puppet_summon`, so a skinned actor carries every one of them (live-verified on the pinned server — a mannequin summoned with them reads `equipment` back verbatim and merges `attributes` over its defaults, `max_health` 40 ⇒ `Health: 40.0f`). Every slot at drop chance 0 (no-grind: an actor's kit is never lootable). Unlike the wave path it deliberately does **not** fall back to the armed-mob default table — an actor is a directed set piece and wears exactly what was declared. `attributes` is likewise **the wave mob's v0.4 [`MobAttributes`] shape** — one type, one rule set, one renderer (`emit::attribute_entries`), so the two surfaces cannot drift — and rides both bodies for the same reason gear does: the twin is what actually fights. A `vulnerable` puppet's `knockback_resistance: 1.0` is compiler-owned, not authorable, and is emitted **first** in the list, so the no-`attributes` rendering is unchanged; the twin never inherits it (that is the caged creep's property, not the freed elite's). `drops` takes the same list a wave mob's does, under the same rules (`DW0490`/`DW0491`/`DW0143`, `name` inventoried as `actor.<actor>.drop.<n>.name`), and rides both bodies that can carry it. **The loot half is where vanilla, not the compiler, draws a line**: `DeathLootTable` and `drop_chances` are `Mob` save data and a `minecraft:mannequin` is a `LivingEntity`, so a skinned actor's caged puppet carries neither (live A/B on the pinned server: both read back `Found no elements matching`, and a mannequin damaged to death wearing a full kit drops nothing at all, so the no-grind zeros are held by the body rather than by a field it ignores). `emit::body_carries_loot_nbt` is the one authority, keyed on the body a summon actually dresses, so the same is true of a twin whose `entity` an author spelled `minecraft:mannequin` by hand. A skinned actor's declared `drops` therefore reaches the player through the unleashed twin and not through the puppet. What the compiler adds is the removal rule: every removal it performs itself — the `unleash` that kills the cage, a `despawn-actor` of either style, a bonfire's re-seat of an unleashed actor (`actor_restand_<id>`) and of a wave (`wave_reseat_<wave>`, both kinds; a wave mob's `drops` ride the same two fields) — first strips the declaration off the body (`execute as @e[tag=…] run data merge entity @s {drop_chances:{…0.0f},DeathLootTable:"minecraft:empty"}`), so a declared drop is what a **player's kill** yields and nothing else: vanilla `/kill` is an ordinary death, and a guaranteed slot and a death loot table roll whoever the killer was. Every such removal is built by one function, `removal_lines(ns, tag, declares_drops, exit)`, which puts the strip in front — an unseen removal's body is killed later under the world by `unseen_sweep`, and that `kill` is an ordinary death too; `every_compiler_removal_strips_declared_loot_first` (`crates/delvec/tests/souls_reseat.rs`) reads the loot-bearing tags off the emitted summons and fails on any removal in the shipped datapack — a `kill` of one, or the retag that hands one to the sweep — without the strip before it. A `dropped_by` `collect` is provisioned by nothing but that loot table, so the rule is also what keeps such an objective from completing without the fight. | 0.6 / equipment 0.6 / attributes 0.6 / drops 0.9 |
| `actors[].tier` | `ordinary` (default) \| `elite` \| `boss` — the SAME [`EncounterTier`] vocabulary `waves[].tier` uses, on the other shape an elite takes (spec-0023). A wave is not the only way to build a hard fight: the set-piece souls encounter — the armoured thing kneeling among the graves that stands up when you strike it — is an **actor**, staged by `spawn-actor`, given AI by `unleash-actor`, killed by hand rather than by a `kill` objective. Same contract as the wave field: a declaration, never a knob — emission is byte-identical whichever tier an actor carries, and nothing about the puppet or the twin changes. Its consumers are the health-bar advisory (`DW0912`) and the drop rule; the ladder asserts nothing about an actor fight's difficulty, and no artifact claims to. Absent ⇒ `ordinary` and omitted from serialisation. | 0.8 |
| `actors[].health_bar` | The same `HealthBar` a wave declares (spec-0073). It reads the bodies whose health can move: the **unleashed twin** (`tag=dw_actor_<id>,tag=!dw_pup_<id>` — the invulnerable puppet is not yet a fight), or, for a `vulnerable` actor, the puppet itself (`tag=dw_actor_<id>`). An actor that is neither `vulnerable` nor named by any `unleash-actor` is `DW0909`: its only body is an `Invulnerable` puppet and the bar could never move. `title` absent ⇒ the actor's `name` (a nameless actor is `DW0910`); a stated one is inventoried as `actor.<a>.health_bar.title`. A `boss`-billed actor with no bar is advised (`DW0912`) exactly as a wave is. Absent ⇒ byte-identical. | 0.31 |
| `actors[].on_kill` | The same `OnKill` a wave declares (spec-0074), over the actor's one body (`dw_actor_<id>`). It comes back where an `unleash-actor` turns it loose in a campaign with a `bonfire` (the undefeated re-stand) or where its `unleash-actor` beat can fire again, so `fires` is owed there (`DW0915`) and inert elsewhere (`DW0914`). Refused on an actor no `unleash-actor` names that is not `vulnerable` — its body is `Invulnerable` for the whole delve (`DW0913`). A re-stand only happens while the body stands, so a one-body fight pays once whichever value is stated. |
| `actors[].traversal{locomotion}` (opt) | **What this body can do when it moves** (spec-0034). The author's side of the traversal proof: by default the compiler derives locomotion from the entity id, and this overrides it for one body. **One shared type on every object class that has a body, a position and a compiler-emitted route** — the stage-2 NPC and the stage-5 actor (`dsl::body_traversal_sites`, a closed sum type, so a third body class is a compile error at every consumer until it is handled) — because traversal belongs to the body, not to the verb that first needed it. `ground|climber|flier`; `aquatic` is refused (`DW0455`). **A declaration is a claim, not an opt-out**: it must change a rule's verdict or the build fails (`DW0454`), and it can never reach the error tier (`opens_gates` is derived and unauthorable, and no class is exempt from `DW0452`). It changes which rules examine the body and nothing else — routing is unchanged, every body walks the same ground A*. Emission is byte-identical whatever is declared. | 0.11 |
| Effect `spawn-actor{actor}` | Idempotent puppet summon at the actor's anchor. | 0.6 |
| Effect `despawn-actor{actor,style}` | `kill` = the author's on-screen death: vanilla `/kill` in place, with its death animation. `vanish` = the body leaves unseen (§4 "A body the story removes is never seen to die"). Targets `dw_actor_<id>` (puppet or twin). | 0.6 |
| Effect `move-actor{actor,to{anchor,offset?},speed?,on_arrive[]}` | The destination is a mark (spec-0066), snapped from its own cell and named `ma_<actor>_<anchor>[_o<x>_<y>_<z>]` exactly as `move-npc`'s. Footprint-aware A*-planned per-tick tp of the puppet, string-pulled over level ground at the puppet's own footprint (§4 "A walked path goes straight where the ground allows"), yaw along the path tangent, with the walked body's arrival turn on the final waypoint (§4 "A walked body faces where it is walking"); `on_arrive` fires at the destination cell; unroutable → `DW0325`. `move-npc` is a thin wrapper over the same planner (player footprint). **Chained origins (live-server proven):** an actor's (and NPC's) successive moves chain — the first leg plans from the declared anchor, every later leg from the previous leg's target. Planning every leg from the declared anchor would degenerate a second consecutive move into a single-waypoint instant teleport. Two moves sharing `(id, to)` still share one content-keyed driver, planned from the first occurrence's origin (a limitation of the content key). **Handoff PackTest:** for the first `move-actor` whose `on_arrive` fires a `spawn-npc` (the walker→NPC scene handoff), a generated `v06_arrive_handoff` template seals every campaign gate (`close-gate` fill), drives the arrival tick, and asserts puppet gone / NPC body present / exactly one NPC hitbox — the beat a delve soft-locks on if the handoff half-fires; gates are re-opened and entities cleared afterwards (batch model). **Concurrent moves are independent:** each `(actor, to)` gets its own start function, per-tick driver, run latch `#arun_<bare>` and step counter `#at_<bare>`, and each driver teleports only its own `dw_pup_<id>` — so N moves in flight at once cannot starve one another whatever order they start in (the island cinematic runs four sheep plus the giant). Pinned by `concurrent_move_actors_share_no_state`. **Overlapping legs on ONE puppet supersede:** concurrency across DIFFERENT puppets is independence (above); two legs for the SAME puppet is a contest, and the later one wins — see §4 "One body, one live walk driver". A puppet with only one planned leg carries none of that machinery (byte-identical). | 0.6 |
| Effect `unleash-actor{actor}` | Replaces the puppet with a real-AI twin (same entity/pos/name/tag, no puppet marker); the puppet leaves unseen (§4 "A body the story removes is never seen to die"), so no second body dies beside the one standing up. Re-caging = `despawn-actor` + `spawn-actor`. **Spawn finalization (live-proven):** `/summon <entity> <pos> <nbt>` — *any* NBT compound, even `{}` — makes vanilla skip `finalizeSpawn`; `/summon <entity> <pos>` does not. The compiler always passes NBT (tags are how it addresses everything it owns), so every mob it summons is un-finalized. For `minecraft:warden` that is fatal: `finalizeSpawn` is the only place the `minecraft:dig_cooldown` brain memory is seeded, and a warden without it enters the DIG activity on its first AI tick, burrows, and despawns ~5 s later. A/B on the pinned server: bare summon → `Brain{memories:{"minecraft:dig_cooldown":{value:{},ttl:1200L}}}`; summon with `{}` → `Brain{memories:{}}`, gone. The twin summon carries that memory verbatim (vanilla's own 1200-tick value — the awake warden refreshes it itself, verified present and roaming past 80 s). Only the **twin** needs it: a caged puppet is `NoAI` and never runs `customServerAiStep`, which is why a puppet warden can stand in a meadow indefinitely. Species needing no finalization data get no memory. **Aggro lock:** an unleashed hostile targets the player who *struck* the trigger. The click trigger parks that player's UUID in `storage dw:strike player` (`data modify … set from entity <hitbox> attack.player` — vanilla's own record of who clicked) for the length of its own bundle and removes it after, so it can never go stale; `unleash_<id>` seeds the warden's vanilla `anger.suspects` from it at max anger (150), guarded on the storage holding a value. Live end-to-end: the warden left its spawn cell, closed on the seeded player and killed that player. Emitted only for a campaign whose click triggers actually unleash, so other campaigns' unleash functions carry none of it. **Limit:** the warden is the only species with a data-settable target that survives a tick on 1.21.11 — the `NeutralMob` pair (`AngerTime`/`AngryAt`) was tried against endermen, piglins, wolves and iron golems, with a real online player's UUID, and neither field reads back afterwards, so nothing is emitted for them and they acquire targets by vanilla's own nearest-player search. | 0.6 |
| Effect `sequence{steps[]{at_ticks,effects[]}}` | Deterministic timeline: one schedule chain firing effect groups at exact tick offsets. **Named by position**: the start function is `seq_<root>_<n>` and step `i` is `seq_<root>_<n>_<i>`, where `<root>` is the effect root's own key with its `fx.` prefix dropped (`fx.near-hall.oc.take-the-emeralds` → `seq_near_hall_oc_take_the_emeralds_0`) and `<n>` counts timelines within that root's deep walk, so a reader of the pack can see which bundle a timeline came from. Uniqueness is asserted over the collected names. The names come from one enumeration built on `for_each_effect_root` and read by both the generator and the caller, so the two cannot disagree; identity is by the timeline's value, since emission reads a trap's payload from a clone, and the first declaration wins, so two identical timelines still share one function. No nested `sequence` → `DW0329`. Effects nested in a step are **first-class**: the flag/wave producer scans, the checkpoint/stealth collector, the l10n inventory, and emission all descend into `sequence.steps` and every nested effect list (`on_respawn`/`on_caught`/`on_arrive`) via one shared traversal, so a `set-flag`/`set-checkpoint` nested in a step produces its flag / registers its indexed checkpoint exactly as at top level. **A timeline keeps its actor** (spec-0085): started where there is an acting player and using it, it tags that player and runs every step as them — §4 "A scheduled bundle has no `@s`". | 0.6 |
| Stage-5 `assemblies[] {id,rig,at,facing?,initial?,hitbox?,strikes?}` | **A fixed thing that can be hit and hits back** (spec-0082): an object built of display entities at a mark, moving through the clips of a library **rig**, struck in melee through an optional hitbox, and striking a player who stands in its arming region. Not a fight class: no health, equipment, traversal, health bar or kill credit, and it never dies. `rig` is `rig/<name>`, resolved against the prefab library's `rigs/<name>/rig.json` the way `prefab/<name>` resolves against a piece (a rig is a generator's output, never campaign JSON — `prefabs/rig-generator`, `prefabs/gallery-generator`); a missing, malformed or rule-breaking rig, or a clip name the rig lacks, is `DW0935`. `at` is a `Mark` (the rig's origin: the mark cell's centre at its floor plane; held in its piece by `DW0897`, resolved by `DW0360`). `facing` (default `south`) is applied to every frame by the compiler, so the entities stand at yaw 0. `initial` names the clip playing from spawn; absent, the parts stand in the rig's rest pose (`parts[].rest`, else the first frame of the first clip). `hitbox {width, height, offset?}` is a `minecraft:interaction` whose bottom centre is the mark cell's centre plus `offset`: width over 6 or height over 22 is `DW0936` (vanilla registers an attack only within 3.3 blocks toward −X/−Z and 22.6 above), and so is a box that meets none of the cells the spawned parts stand in. `strikes {while_in, pattern[] {windup, hold, strike, ticks_per_frame?, lock?, on_land[]}, aim?}`: `while_in` is the anchor-centred box `StealthZone` is; the pattern runs, repeating from its first step, while some player's body is in it (a player in a cutscene neither arms it nor is struck), and stops at the end of the step in flight when nobody is. A step plays `windup`, holds its last frame `hold` ticks, plays `strike`, and runs `on_land` — effect root R10, with no acting player — on the tick a client has drawn the strike clip's last frame whole, one cadence after it is applied (`Clip::landing_ticks`); a step with no `on_land` is a feint. **The wind-up's length is the creator's two knobs**: the wind-up clip's `1 + (frames − 1) × cadence` ticks, then `hold`. `ticks_per_frame` (1–20, else `DW0935`) is the step's pace: its wind-up and strike play as emitted clips after the rig's own (`assembly::paced_index`) at that cadence, and the record times the wind-up and the landing by it. `aim {facings}` (spec-0082 §5.7): at the start of every wind-up the root is turned, by `tp` (every riding part takes the root's change of yaw, stays seated and keeps its interpolation — spec-0082 §8 rows 3, 10), to the one of `facings` evenly spaced turns from `facing` nearest the bearing of the player in `while_in` nearest the mark — chosen by `assembly::target_lines`, every player in the box tagged and then `execute positioned <mark> run tag @a[tag=…,sort=nearest,limit=1]`, because a selector's own `x`/`y`/`z` are the origin its `sort` measures from (spec-0094 §2.3) — (`execute … facing entity`, the yaw read as a whole number of `1/facings` degrees, rounded with the scoreboard's floor division); only facings a player in `while_in` can draw are emitted (`assembly::drawn_facings`), a pick outside them resolves to the nearest drawn one, and every `damage-players` box at the top of `on_land` is turned with it (`assembly::turned_region`) and dealt through one tag per caught player (`assembly::region_damage_lines`). Where a blow lands is judged (`DW0938`); when it lands and how hard are the creator's (spec-0016 §3). **`lock {within, pick, reaches?}` on a step** (spec-0094): at the start of the step's wind-up one player in `within` (a `StealthZone`) is chosen by `pick` — `nearest`, `furthest` or `random`, vanilla's selector orders, measured from the mark — and the cell their feet stand in is read (`data get entity … Pos[a]`, the floor of the coordinate), made relative to `within`'s low corner and clamped to it, and dispatched (`asm_lock_<s>_<j>` → `asm_lockat_<s>_<j>` → one `asm_lockc_<s>_<j>_<ix>_<iy>_<iz>` per cell of `within`, so every macro call names a function that exists): the root is `tp`'d to the yaw proved for that cell and the cell's pose and landing are remembered in `dw:asm <s>.r` / `.q`; the swing plays `$function …:asm_play_<s>_$(r)`, the landing `$function …:asm_land_<s>_<j>_$(q)`. A display does not bend to a point, so a lock is a continuous **turn** plus a choice among **poses** — the step's `strike`, then each of `reaches` — and both are proved per standable cell of `within` (`assembly::lock_plan`, `DW0968`); a non-standable cell resolves to the proved cell in its column below it, else the nearest. The blow is **derived**: a top-level `damage-players` in a locked step declares no `in` (`DW0969`) and lands, through `region_damage_lines`, on the cells the chosen pose comes down on at the locked turn — the area shape 2 demands, by construction. A locked step winds up only while a player is in its `within`; the plans travel from `assembly::check` to the emitter (`assembly::Locks`), never re-derived. **Arming**: a wind-up begins only while `#asm_<s>_armed` is 1 — set by the summon and by `arm-strikes`, cleared by every `play-clip` — so a clip the story plays completes and holds whoever stands in `while_in`. A hit count is an ordinary `state` datum a `strike-assembly` trigger with `once: false` adds to, and what happens at a count is an effect behind the ordinary gate. | 0.35 |
| Effect `spawn-assembly{assembly}` | Summons the assembly (root, parts riding it, hitbox) at its mark, playing `initial`. Idempotent: guarded on the root's absence. | 0.35 |
| Effect `despawn-assembly{assembly}` | Removes every entity of the assembly. A display entity has no death, so there is no `style`: the parts and the hitbox leave unseen. | 0.35 |
| Effect `play-clip{assembly,clip}` | Makes `clip` the clip the assembly plays and returns to; its first frame is applied on the next tick. While a strike step is in flight the switch waits for the step to land, so a story beat never cuts a strike at the frame before it lands. **It stands the strike pattern down** (spec-0094 §3.3): `asm_cue_<s>_<k>` writes `#asm_<s>_armed` 0, so the clip completes and holds (or loops) with a player in `while_in` until an `arm-strikes`. A clip the rig lacks is `DW0935`. | 0.35 |
| Effect `arm-strikes{assembly}` | Re-arms the assembly's strike pattern a `play-clip` stood down (spec-0094 §3.3): `asm_rearm_<s>` writes `#asm_<s>_armed` 1 and the step index 0, so the pattern resumes from its first step on the next tick a player is in its arming region. Re-arming is a rule the author writes — a `sequence` step after the clip's length, a trigger, a rest — never a property of standing still. On an assembly with no `strikes` it is `DW0970`. | 0.36 |
| `traps[]` | spec-0011 + **spec-0022**: `{id,at,trigger,effect?,payload?,lethality?,disarm?,reset?,requires_flags?,forbids_flags?}`. **Redstone keeps exactly one job — the trigger**; the consequence is commands (spec-0022). `payload` is an ordered effect list in the SAME vocabulary quests use, plus the two trap verbs `volley` and `collapse` (see below); it is how a trap's consequence is authored. `effect` (the spec-0011 `dispense` wiring) is the alternative — a trap must declare at least one of the two (`DW0440`). `at` binds a **point anchor** an area's prefab provides — the trigger/hazard cell, under whatever name the piece gave it (`anchor/trap` is what the shipped pieces call it, and a name is all it is). **A `payload` trap needs nothing of the piece but that cell and the trigger block standing in it** — the plate, tripwire or trapped chest the `trigger` names, which the piece places and the compiler proves is there (`DW0917`): the compiler emits the detection, an edge-latched per-tick `execute … if entity @a[<cell>]` (a trapped chest's is an interaction hitbox over the chest). The anchor additionally needs a `dispenser` socket cell only for a legacy `effect` (which the prefab's own redstone fires, and which is the one case where no detection is emitted), and a `trigger_block` only for a flag-gated trap. `trigger` ∈ `pressure-plate`/`tripwire`/`trapped-chest` (all redstone-native; `trapped-chest` = the only player-distinct trigger). `effect` = `{dispense:{item,count}}` (item `DW0341`; a non-`dispense` key e.g. `tnt` is an unknown variant → `DW0100`). `lethality` ∈ `lethal`/`harmful`(default)/`nonlethal`. `disarm{via,sets_flag}` = a reachable affordance that turns the trap off. `reset` ∈ `once`/`rearm`(default). Structural errors `DW0340`; a lethal forced-path trap without discharge `DW0342`. `requires_flags`/`forbids_flags` are a **physical** gate (see §4 emission): the trigger block is removed from the world while the gate is shut and restored verbatim when it opens, so a gated trap is genuinely inert rather than nominally so — the trigger must be a plate/tripwire declaring `trigger_block` in its prefab metadata, else `DW0363`. | 0.6 |

Dialogue effects `set-flag`, `set-time`/`set-weather`,
`set-checkpoint`/`spawn-npc` and option `requires_flags` mirror the quest
forms. A dialogue **option** may carry a `happening`, and must when it
sets a flag (`DW0481`) — a choice that forks the world is a story node.
A per-effect `when` is a **quests-stage** surface only (dialogue
effects carry none — a dialogue option's own `requires_flags` already gates
its whole effect bundle).
The blockstate suffix on `set-block`/`prop` blocks is a lenient parse of the
field: the base id is registry-checked and the `[…]`
string is passed to `setblock` verbatim (vanilla validates the property
names/values); a malformed suffix (unbalanced `[]`, empty, non-`key=value`)
reuses `DW0193`.

### Stage 6 — `dialogue`

Exactly one tree per stage-2 NPC (`DW0152`/`DW0153`). Nodes reachable from `root`
(`DW0120`/`DW0121`); `complete-objective` effects target a `talk-to` on the same
NPC (`DW0122`); every `talk-to` has ≥1 reachable (`DW0123`) and ≥1 **ungated**
(`DW0191`) completing option — where "gated" means `requires_flags` OR
`forbids_flags`: either kind of flag gate can make the option unavailable exactly
when it is needed, and the static analysis does no temporal reasoning about which
flags end up set. **A completing option is drawn, and its click completes, under the
objective's whole pending guard** (spec-0093 §6.3) — quest active ∧ every `after`
complete ∧ `requires_flags` ∧ `forbids_flags` ∧ `requires_state` ∧ not yet
complete — the same `pending_guard` every other objective driver goes through, so
a button cannot complete a beat before the beats it declares `after`; that guard
is the engine's, opens exactly when the objective activates, and is what `DW0191`'s
"ungated" does not count. The generated `dialogue_mask` PackTest breaks each of
its terms on its own. Node `text` → l10n `dlg.<n>.<node>.text`, option
labels → `.opt.<i>.label`. An option label is a **button caption**: it is drawn on a
fixed 150-GUI-px dialog button and scrolls if it does not fit, so every label —
source and translation — is width-checked (`DW0331`, error).

**Option `tooltip` —
"button = caption, tooltip = the full line".** An option may carry an optional
`tooltip` beside its `label`: the sentence the character actually says, shown in a
hover box while the button keeps a caption. This is vanilla's own primitive, not a
workaround — a dialog action button is `ActionButton(CommonButtonData,
Optional<DialogAction>)` and `CommonButtonData`'s codec is exactly
`fieldOf("label")` + `optionalFieldOf("tooltip")` + `optionalFieldOf("width", 150)`
(read off the pinned 1.21.11 client jar), so the compiler emits `tooltip` as a
sibling of `label` inside the `actions[]` entry. The client hangs it on the button
via `Tooltip.create(…)`. **`DW0331` does not apply**: `Tooltip` wraps its text with
`Font.split(message, 170)`, so a tooltip never scrolls and has no button budget to
overrun — the format declares no other limit on it, so the compiler enforces none.
Player-visible, so it is inventoried and translated like the label
(`dlg.<n>.<node>.opt.<i>.tooltip`); an unauthored tooltip emits no key at all, so
a campaign that uses none is byte-identical. Precedent, and the live proof the
codec accepts the field: `class_select` carries each class's `blurb` in exactly
this slot, and tier 2 boots it on the pinned vanilla server every PR.

**Dialog buttons (spec-0078) — one shape for every button the player sees.**
Every dialog button the engine emits is built by one function,
`emit::dialog_button(label, tooltip, command)`, and by nothing else: the label
component, an optional `tooltip` component (`tr(tooltip)`, emitted only when
stated), and a `minecraft:run_command` action running a `/trigger`. The sites, all
inside `minecraft:multi_action` dialogs, so all on the `CommonButtonData` codec
above:

| Dialog | Button | Tooltip source | Key |
|---|---|---|---|
| `class_select` | one per class | the class's required `blurb` (always present) | `class.<c>.blurb` |
| `bonfire_<i>` | *rest and save*, *save only* | `bonfire.rest_tooltip` / `.save_tooltip` (optional) | `fx.….rest_tooltip` / `.save_tooltip` |
| `shop_<i>` | one per offer | `offers[].tooltip` (optional) | `shop.<s>.offer.<i>.tooltip` |
| `<npc>_<node>[__m<mask>]` | one per visible option | `options[].tooltip` (optional) | `dlg.<n>.<node>.opt.<i>.tooltip` |

A node with no visible option is a `minecraft:notice` with no authored `action`, so
vanilla draws its own default button and the engine states none. A campaign that
states no optional tooltip emits no `tooltip` key on any button.

**Display gating:** an option is
*shown* only when clicking it would fire — every `requires_flags` set and no
`forbids_flags` set (flag axes; the click handler mirrors both with fail-fast
guards, so a direct `/trigger` cannot bypass them) and every completed objective
active, i.e. `dw.qa_<quest>==1` and
`dw.o_<obj>!=1` (objective-state axis) — so `DW0191`'s ungated completing option
is visible exactly while its objective is active (the guarantee holds
automatically).

### The map pipeline — `detail-plan` (optional; spec-0050)

Stage 6: **which piece stands in which of the plan's places.** One document,
`detail-plan.json`, and two fields.

| Field | What it states |
|---|---|
| `palette` | Role name → block: the whole's material vocabulary, handed into every allocation and **gated by nothing**. Materials are style, style authority is rank-only, and a piece exported against a stale palette is a render finding rather than a machine one — the piece's own provenance row already freezes what it was built from. Absent means the whole states no vocabulary, which is a different claim from an empty map. |
| `details[]` | `{place, piece, anchors}`. `place` is a layout-graph node; `piece` is a prefab; `anchors` maps each synthesized name that place **owes** to an anchor of the piece. |

**There is no coordinate, no region, no extent, no datum, no seam and no offset
in it** — absent fields, not optional ones. A detail document is *structurally
unable* to move its box, its datum or its seams, because the schema has no
spelling for any of them, and the only path from a `details[]` row to placed
bytes runs through the compiler computing the frame from the site plan inside
`Plan::build` — the one constructor every world-reaching verb goes through. That
is the same tooth the blockout's is: inversion is not forbidden, it is
uncompilable. The escalation path a part that wants different *space* takes is
a **site-plan revision**; a part that wants different *traversal* revises the
**layout graph**, and either revision costs a re-detail (`delvec detail --all`).

**The frame** a piece must exactly fill is its place's claim (spec-0098 §2): the
bounding box of the cells the place owns — the ground under its plot, its floor
course, its play space, its ring above the fixed ground and, roofed, its lid and
declared roof zone. Every other cell of the frame is a **void** — a neighbour's
cell, the ring's fixed ground, nobody's — where the piece holds
`minecraft:structure_void` and the owner's block shows through, in the game and
in the model alike (`DW0987`, `DW0990`). In the derivation this is one rule: the
cells a bound place owns are a hole in what the whole writes, and the whole's
stand-ins stand only in unbound places' owned cells. A seam whose plane the
place owns is the piece's to cut, and a `barred` one's shut state the piece's to
ship, bound to the gate region it owes; a seam a neighbour owns is answered on
the place's own first layer beside the plane. Two seams answered in the first
layer meet at the play space's corner column, which then lies in both openings
— or, with one plane owned, the owned opening's end cell touches the room only
through the other opening; either way the piece answers both exactly as
allocated (`DW0844` compares cell for cell) and `contract-well-formed` takes a
cell beside another of the space's exterior openings as touching the room
(`compiler::detail` `two_contacts_at_a_corner_answer_whoever_owns_each_plane`).
Which spaces are enclosed is the piece's own declaration, and the closure gate
confirms exactly those; a piece declaring none — a street, a pavilion, a
covered market — passes, its zero stated in the enumeration with the count of
spaces.

**The fixture pass applies to derived interiors only.** A bound place lights
itself; its cells leave the relight pass's deficiency set and go to the
undeclared-darkness measurement instead, so a dark detailed place is a finding
rather than a silence. The measurement is kept **per place**, not folded into the
area's, because the two have different remedies and `DW0210` says which: a dark
bound place is named with its piece and told to light itself, and is never sent at
the plan's `lighting`, which does not reach it.

**Detail is per-place and partial by construction**: every unbound box is massed
exactly as at stage 5, so a campaign with one detailed place builds, walks and
renders like any other. The broken intermediate is a real, lookable object at
every point between "no detail" and "fully detailed".

**Traversal equivalence** is proved by the stage-5 battery running unchanged over
a world with pieces standing where massing stood — `DW0836`, `DW0837` and
`DW0838` share no arithmetic with the derivation and do not care what wrote the
blocks. What is deliberately free to change is the interior: partitions, stairs,
lofts and pits inside a place, its materials and its light. What is not: the
seams, their cells, their rises, and the absence of any way out the plan did not
allocate.

### Stage 7 — `world-edits` (optional; spec-0017)

The map editor's edit script (`world-edits.json`), the artifact of record for
L3 world detailing. **Optional**: absent = no edit stage. Replayed
deterministically by the compiler after world assembly (§1 pass 8); editing
sessions leave no state outside the script. `note` fields are authoring context — machine-ignored and **excluded**
from l10n (no stage-7 string is player-visible).

| Element | Behavior |
|---------|----------|
| `batches[]` | Ordered `{id: batch/<kebab>, area, note?, edits[]}`. Batch ids are unique (`DW0111`), the seed-stream label and the snapshot name; `area` must be an area the campaign declares (`DW0112`) — a stage-1 `areas[]` entry, or `area/site` on a **site-plan** campaign, whose one place is the site the plan lays out and whose `areas[]` is required to be empty (`DW0839`). After EVERY batch the invariants re-prove (§4). |
| `select` | `{name: region/<kebab>, shape}` — defines a named region for later verbs **in the same batch** (strictly backward; dangling/forward = `DW0162`, duplicate = `DW0111`). Shapes: `box` (inclusive `min`/`max` in a declared frame), `surface-band` (`over` + `from..=to` offsets from each column's surface), `palette-match` (`within` + base-id `blocks`), `union`/`intersect` (`of`, ≥2), `subtract` (`base` − `remove`). |
| Frames | `piece-local` (`piece` placement index + `prefab` drift-guard — mismatch is `DW0323`) or `anchor-relative` (a resolved anchor of the batch's area) — never raw world coordinates, so a script survives placement moves. |
| `fill` / `replace` | Seeded palette-recipe write over a region (`replace` only rewrites cells whose base id is in `matching`). A recipe is weighted `blocks[]` (+ optional noise `scale`, default 0.35 blocks⁻¹) sampled by smooth value noise — picks cluster into strata/patches, never a uniform fill. Block ids validate against the pinned registry with optional verbatim blockstate suffix (`DW0193`); weights/scale finite > 0 (`DW0162`). |
| `carve` | Clear a region to air. Sealing-aware by construction: the carved region re-enters relight + walkability + boundary proofs. |
| `morph` | Surface reshape per region column: `raise{by,recipe}`, `lower{by}`, `smooth{passes,recipe}` (±1/pass relaxation toward the cardinal-neighbour mean). The region gives the footprint + where the surface is read; `raise`/`smooth` may add cells above the region top. |
| `scatter` | Seeded dressing over a region's **standable** cells (air over an occupied cell): weighted `items[]` (blockstate suffixes allowed), per-candidate white-noise `density` gate in `(0, 1]` (dressing wants speckle, not the fill verbs' clustered patches), keep-clear `avoid[]` region envelopes (matched by `(x, z)` column), optional both-axes `spacing` rule and `limit` cap taken in descending noise order — the greenfield generator's spread idiom, ported. |
| `plant` | Structural flora via the **lean-or-grow** canopy rules (ported from the island terrain generator): up to `count` trees on the region's highest-noise standable cells (both-axes `spacing`, default 4; trunks never on `avoid[]` columns). A canopy that would cover an `avoid` column leans one block directly away; if that still covers it, the tree grows tall instead — its whole ball arched 3 above the trunk's floor. **No leaf is ever sliced**; leaves write only into air, so near walls/ceilings the ball may extend past them — review via the batch snapshot. `tree: oak` (per-species rule sets, extensible). |
| `fragment` | Stamp a **library prefab**'s non-air cells at a frame-resolved `at` (+ optional quarter-turn `rotation`) — semantically a `/place template` whose bytes the compiler models (non-air overwrites; authored air never erases). Only admitted library prefabs can be stamped, so provenance/license ride the prefab's own metadata (ADR-0013); an id outside the library is `DW0323`. Stamped cells keep their **full blockstate** (`assembled::structure_cells_stateful`; properties in sorted key order): the stamp's writes ARE the runtime `setblock` lines, so an authored `lantern[hanging=true]` stays a hanging lantern. **`rotation` turns POSITIONS only, and the compiler REFUSES rather than warns.** There is no rotate-aware blockstate rewriter, so a quarter-turned stamp would keep every `facing`/`axis`/`shape`/connection value unrotated and ship visibly deformed geometry — the silently-deformed-map class. A `rotation` other than `none` on a prefab carrying any **yaw-dependent** property (`facing` except `up`/`down`, `axis` except `y`, `shape`, `rotation`, `orientation`, `hinge`, `north`/`south`/`east`/`west`) is a build error (`DW0323`) naming the block, its prefab-local cell and the offending property; the prescription is to stamp unrotated or admit a pre-rotated prefab variant, never to hand-fix facings downstream. It is a **collision test, not a blanket ban**: prefabs whose every state is yaw-invariant (`hanging`, `half`, `waterlogged`, `open`, `lit`, `type`, `level`, `thickness`, `vertical_direction`, `axis=y`, `facing=up|down`) rotate correctly and stay allowed — a stone box with a hanging lantern and an upright log stamps fine at every quarter-turn. **No piece in the shipped library is one**, and that is a property of the library rather than of the rule: every emitted state names the connections it joins, so each carries `north`/`south`/`east`/`west` (or a `facing`, or an `axis`) that a quarter-turn moves. The accepting half of this test is therefore exercised against a piece built for it, not borrowed from the library — a check whose "provably-correct output is accepted" half has no case left is a blanket ban nobody can tell apart from a rule. |
| `relight` | Run the spec-0010 fixture-placement pass over ONE region and **bake** the fixtures into the edit script's writes — authorial control of where fixtures land (the whole-area relight still re-proves after every batch). Fixture/target default to the area's declared `lighting`; `fixture` + `min_light` (1..=14) override, and are **required** when the area declares none (`DW0162`). An unlightable region is the area pass's own `DW0211`, batch-attributed; a region with no reachable walkable cell is `DW0323`. |
| L2 massing verbs | `swap-piece` (replace a piece with a library prefab that re-mates every mated socket at its exact world pose, any rotation, overlap-checked), `insert-piece` (attach at a specific **unmated** socket — the targeted form of the solver's frontier attach), `remove-piece` (a **leaf** only — exactly one mated socket, never the entry; the neighbour's socket unmates and re-seals), `rewire-socket` (`sealed` **unmates the doorway pair** — a graph operation: both planes wall up and the DW0306 connectivity proof loses the edge; `open` clears an unmated socket's fill — deliberately without granting the proof an edge, conservative), `reseed-piece` (seeded weighted re-pick among the area pool's compatible members, current excluded — a reseed always changes the piece or errors). All carry the `piece` index + `prefab` drift guard. Applied at **plan** time (`compiler::massing`, inside `Plan::build` right after `solve_area`): seals are regenerated from the massaged mated flags (`seal_layout`), and anchors, gate reachability, waterline, assembly, relight, nav and the L3 replay all run over the massaged layout — the full assembly validation re-runs by construction. Massing verbs live in **massing-only** batches ordered before every detailing batch (`DW0162`); an inapplicable verb is `DW0324`. `resize-piece` from the spec's initial list is **excluded**: the library has no size-parameterized piece primitive to express it through (no-hack doctrine); `swap-piece` covers the different-sized-variant case. |
| Seeding | Every seeded verb streams from `stream_seed(campaign_seed, "edits/<batch-id>/<edit-index>")` — renaming a batch (or moving an edit) deliberately reseeds it; nothing else does (ADR-0006). |
| Emission | The replay lowers to x-run-coalesced `fill`/`setblock` lines at the end of the **world build** (§4 *One tick's command chain stays under the game's limit*), after the socket seals and before `setup_finish`'s relight fixtures — the exact model order, and the reason `DW0352` exists (`trap_setup` runs later). `setup` additionally forceloads every batch's write AABB (an edit may write outside the piece bboxes — a leaning canopy, a stamped fragment — and a `setblock` on an unloaded chunk silently fails); those chunks then follow the **forceload lifecycle** below. `world-edits.json` is hashed into `manifest.json` inputs. |

### The design record — `design` (optional; spec-0061)

`design.json`, the machine half of an approved look. **Optional**: absent = a
campaign that has not approved a design, which validation measures and prints
as a zero and which `tools/creator/staging-gate.py` refuses — a build the owner plays
carries an approved design or is not staged. Present = parsed, validated and
hashed into `manifest.json` inputs like any other stage document. Nothing in it
is player-visible, so nothing in it is l10n-inventoried.

A human approves a look by looking at a picture, and nothing in this toolchain
reads a picture. What a machine can hold is the token written beside the
picture at the moment of approving.

| Element | Behavior |
|---------|----------|
| `references[]` | **At least one** (`minItems: 1`; an empty list is `DW0100`). A record of nothing is not a record, and it is not how a campaign says it has approved no design — that campaign ships no `design.json`. |
| `references[].name` | The image's path **stem** relative to `design/`, under `concept/` (one scene) or `reference/` (a view of the whole map): `concept/<kebab>` or `reference/<kebab>`, else `DW0110`. Duplicated across rows = `DW0111`. The extension is absent on purpose — the row names the picture, not one encoding of it, and a stem two files answer to is a candidate rather than a match (`DW0890`). |
| `references[].shows` | One sentence saying what the picture shows. Agent-facing: never on a player's screen, never inventoried. |
| `references[].time` / `.weather` | The sky the picture was drawn under, typed as `WorldTime` / `WorldWeather` — the world's own two enums, so an hour added to the world is an hour a row can state with no second table. These two tokens are the only **creative judgement** on this surface; everything else the check derives. |
| The directory | `design/concept/` and `design/reference/` are the two directories an approved image lives in. A file there whose extension is one of `jpeg`, `jpg`, `png`, `webp` (`dsl::design::IMAGE_EXTENSIONS`, one constant) is an approved image and owes a row; anything else — the re-issue sidecars `tools/creator/refimg.py` writes, a creator's notes — is neither counted nor refused. |
| The comparison | `DW0890`, at validation: the set of skies the world can reach and the set the rows state must be equal, and every row must resolve to exactly one file and every file must have a row. |
| The camera | Every row is answered by a camera of `design/cameras.json` — `DW0900`, at the **build** (§7 `delvec cameras`), because a camera is written against a built world. A campaign with rows and no record at all builds, and is refused at the staging gate. |
| The ledger | `validation/design-record.json`, written by **every** build — `references`, `image_files`, `by_directory`, `skies_stated`, `world`, `unrecorded_files`, `unresolved_rows` (`DW0890`) plus `cameras`, `answered` and `unanswered_rows` (`DW0900`). |

### The map pipeline — `geometry-brief` and `layout-graph` (optional; spec-0049)

Two documents that state a campaign's **space before any coordinate exists**.
Both are optional files in the campaign directory, named rather than numbered
into the 1–7 sequence: they belong to a different pipeline and a number would
assert an ordering between the two that does not exist. A campaign that ships
neither is unaffected: neither document adds a field to any other type. `delvec schema
--stage geometry-brief` and `--stage layout-graph` export them; `--stage all`
includes both.

`geometry-brief.json` is the whole map's written brief reduced to numbers:

| Element | Behavior |
|---------|----------|
| `facts[]` | `{id: fact/<kebab>, value, unit?, note}`. A number with a name, taken from the brief's own prose. Ids are unique (`DW0111`) and well-formed (`DW0110`). A fact is read by the site plan's `identities[]`, and the binding line states the count so an absence is a number rather than a silence. |

`layout-graph.json` states the campaign's space as a graph:

| Element | Behavior |
|---------|----------|
| `nodes[]` | `{id: node/<kebab>, intent, note?, stations?, reached?}`. `reached: false` (spec-0098 §14) declares the place **scenery** — built to be seen and never entered, a tree's crown over a treehouse — and the checks confirm it both ways: the closure must not reach it (`DW0816`) and no body may get into it in the built world (`DW0837`). Scenery still owns its outside and stitches its ground ring; it owes no node anchor and no play light, and its piece is judged sealed (no way in claimed, and its floor gates excuse every standable cell of the piece — a crown's leaf tops included — stating the count, since none is stood in; `delvec detail`'s light probe states the cells it excludes with theirs and grades none). Absent means reached. A **place**: a room, a courtyard, an arena, a stretch of shore, a cavern, a road. `intent` is a free non-empty label no check keys on — recorded judgement for the reviewer and for the later per-place brief, kept free-form because an enum of intents would be one genre wearing a schema's clothes; empty is `DW0814`. A place carries no size class: its size is its box's `extent`, the author's declaration in the site plan, and the piece detailed into it may not exceed it (`DW0843`). |
| `edges[]` | A connection, internally tagged on `class`: `walk`, `stair`, `climb`, `drop`, `barred`, `carry`, `vision`. All carry `{id: edge/<kebab>, a, b}`. A `climb` (spec-0098 §2c) is a way a body climbs on a ladder or a vine the lower place hangs (spec-0099): a hole through a floor or a door high in a wall, with no treads and no sill; the rise is the two floors' difference, and a climb between two places on one plane is `DW0992`. A climb **inside one place** is the piece's own: its spatial contract declares the two floors as two spaces and a `climb` edge between them, proved over the body's climb moves (`docs/reference/grammar.md`, the contract's edge classes). `walk`/`stair`/`climb`/`barred` carry `one_way` (`a-to-b` \| `b-to-a`; absent = both ways); a `drop` is one-way by construction and so carries a **required** `falls` instead. `barred` carries `opens_from` (`a` \| `b` \| `either`, default `either`) — the one-side-openable door, spelled as a property of the connection rather than of any campaign's fiction — and a **required** `gating`. `carry` (spec-0083 §7) is a connection a body is **carried** over: `one_way` as `walk` carries it (absent = both ways, each direction then owed a link) and a **required** `gating` (empty is `DW0818`, as for `barred`: a carry live from world load is a hole in the graph's claim); it has no shortcut mark, no seam, no sill and no sightline, and the derivation writes nothing for it. The closure crosses it like any gated edge, and stage 5 matches each link to the `carry` edge joining the node of its `from` station to the node of its `to` mark in its direction, and each `carry` direction to at least one link (`DW0934`). `vision` carries a line of sight and no body, so it has no direction, no gating and no shortcut mark; stage 4 gives it a sightline rather than a seam. Because the class is the serde tag, a field the class does not read (an `opens_from` on a walk, a `drop` with no `falls`) is an ordinary `DW0100` and no rule has to police it. |
| `edges[].gating` | `{flags[]?, quest?}` — what a body must already hold to pass. **Deliberately not the campaign's `Gate`.** A gate is a runtime object emission evaluates against an acting player; a layout-graph edge is evaluated by nothing at run time, so making it a gate consumer would push a never-emitted object into machinery whose whole subject is emission. It is also narrower on purpose: the closure below is monotone, so a negative flag term and a numeric comparison are terms no proof here could honour, and a surface an author may write and nothing honours is worse than one that is absent. What it states is a **projection** of the campaign's runtime gating into topology, and `DW0818` keeps it a projection — every flag it names must be one the campaign really produces, and every quest must exist. |
| `entry` / `goal` | Node ids. Every proof over the graph starts or ends at one, so a name nothing defines is `DW0814`. |
| `critical_path[]` | An authored node sequence from `entry` to `goal`. **Authored rather than derived**, so that it is a claim the machine verifies (`DW0817`) rather than an answer with no author to disagree with. |
| `beats[]` | `{quest, objective, node}` — where each quest beat happens. **Every objective is place-bound**: a body has to be standing somewhere to talk, to reach, to fight or to take, so every objective in the quest documents binds to exactly one node, and one that does not is `DW0818`. |
| Reachability | Judged under a **monotone closure**: from `entry` holding nothing, mark every edge whose gating the obtained set satisfies, mark every node reachable over those edges respecting one-way direction, add what every beat bound to a reached node grants, iterate to fixpoint. A beat grants the flags the campaign sets when that objective completes and — for a `talk-to` — everything reachable in the spoken-to NPC's dialogue tree, because the conversation happens where the speaker stands; a quest grants itself and its `on_complete` flags once every one of its beats is somewhere a body can stand. The closure is **optimistic in every direction it cannot decide**, which is one property rather than a list of exceptions: it is branch-blind, so a campaign whose branch points set mutually exclusive flags can reach a node no single playthrough reaches. That can only under-report at graph time; the branch-aware battery over the assembled world is what stops it shipping. |
| `nodes[].stations[]` | `{anchor: anchor/<kebab>, kind, note?}` (spec-0052) — **the named places INSIDE a place**. A station is a name and a shape, never a position: there is no coordinate, offset or hint field, and that absence is the design. Declared names join the campaign's anchor vocabulary at the same authority as every synthesized one, so a quest can name the fire pit in the camp rather than flattening it onto the camp's box centre. `note` is recorded judgement for the reviewer that no check keys on, exactly as `intent` is. Refusals: `DW0869` (a name in the engine's derived namespace), `DW0870` (two claims on one name, scoped to the area), `DW0871` (a reference demanding a shape the station is not). A station no quest references is **legal** — that is the mid-authoring state, and the binding line counts it. |
| `stations[].kind` | `point` or `gate`. **The shape, never the purpose**: a bonfire, a camera subject and a shop counter are the same `point` to every check, for the same reason `intent` is free-form. `point` is a cell a body is put at; `gate` is a region that seals and clears, which is what `open-gate`, `close-gate`, a `shortcut` and a `timed-gate` address. spec-0052 §3 describes a third, `region`, and it is deliberately not built: this engine resolves an anchor to a point or a gate and to nothing else, every volume-shaped consumer is an anchor-centred box on a **point** plus an extent, and a bare `region` anchor is read as a gate filled with `minecraft:air`. A third variant would be declarable, bindable and consumable by no reference site. spec-0052 §11's falsifier decides it: the first campaign brief that cannot state its place without one. |
| A station while its place is **massed** | The derivation realizes every station of every box at a stand-in, from the same authority validation resolved the name against — so a name that validates cannot fail to exist in the built world, massed or detailed. A point lands on its own standable cell (the first not already taken, ordered by Chebyshev distance from the floor centre then lexicographically — the order `footing` searches by); a gate lands on a minimal region of the derivation's own bar, **written into the mass** so the world-load seal measures it shut rather than taking the anchor's word. The author cannot state where a stand-in goes. |
| A station once its place is **bound** | It joins the node's owed set beside its `anchor/node-…`, its `spawn` when it is the entry, and its `anchor/unlock-…` sides, and the `detail-plan` `anchors` map must bind it to an anchor of the piece. The map's shape and its gate are unchanged — it still refuses every key outside the owed set, so a binding cannot invent vocabulary and a typo cannot pass as intent. The bound anchor's **shape** is checked against the station's `kind` (`DW0842`), so the kind validation read off the graph and the kind the built world has cannot drift. Two owed names bound to one piece anchor is legal: one spot may carry two roles. |
| Binding | Every run that carries either document prints one line: places, connections (traversal, one-way, shortcut, gated, carry), **stations and how many of them are gates**, beats and **how many of them are on the mandatory quest spine**, critical-path steps and brief facts. Three zeroes are called out as findings rather than counted: a graph with no traversal connection (a set of places with no space between them), a graph none of whose beats belongs to a quest the finale depends on (a critical path over an unbound graph), and a brief with no fact. A station count of zero is stated rather than omitted — a campaign naming its places at node granularity is a fact about that campaign, not a silence. |

### The map pipeline — `site-plan` (optional; spec-0049)

`site-plan.json` is the **geometric embedding** of the layout graph, and the
whole map's design of record. Optional, named rather than numbered, and reached
only through itself: a campaign that ships none is unaffected.
`delvec schema --stage site-plan` exports it; `--stage all`
includes it.

**Every document answers to its own name.** `delvec schema --stage <name>` takes
any stage's name — the same string `DW0100` prints when that stage's document
will not parse — as well as `1`..`7` for the numbered campaign stages, so the
refusal's own prescription is a command that works. The names are enumerated
from `Stage::ALL` rather than a second list, so a document added later answers
the day it exists. **The exported schema is the authority on a document's
form**; where a spec disagrees with it, the spec is the stale one.

**Its one ordering obligation is not advice.** A plan validates only against a
layout graph and a geometry brief: `DW0824` refuses a plan whose graph or brief
is absent, naming the missing document, and a box carries a **required** `node`,
so there is no site plan — well formed or otherwise — that describes a space
without naming the place it is the space of. The inversion does not compile.

**The model, because every rule below rests on it.** A box is the **play space**
of a place: the cells a body can be in. So `extent` is the interior footprint the
author declares, and two connected places sit **exactly one
cell apart** on the face they share, that cell being the wall they have in
common. **A place owns its outside** (spec-0098), and its **claim** is a
cuboid: the ground under its plot from the claim's bottom, its floor course,
its play space, the one-cell **ring** its walls may stand in and, roofed, its
lid and declared roof zone. A ground place's bottom is per column — the lower
of its floor course and the terrain in that column, stopping one course over a
place stacked under that column; an aloft place's is its declared underside
(`base: {"aloft": n}`, `floor − 1 − n`) everywhere. The ring's cells at or
under the column's ground height are **fixed ground** on a ground place — the
site's terrain continued to the plot's edge, the whole's (rule 0); an aloft
place has none. Every other cell several
claims share goes to the place whose floor course it is (3a), else to the one
roofed place among them (3b), else to the `a` of the seams across that plane
(3c — at a corner where connections led by different places meet, the first in
seam order), else, where exactly one claimant is aloft and every other claims
the cell only as **sky ring** — an open ground place's ring above its floor
course, the air its headroom carries — to the aloft place hung in it (spec-0098
departure 35, `Site::is_sky_ring`; a ground place's ground, floor course and play space never
yield); a cell no rule awards is `DW0827`. A cell no claim covers holds the
declared volume or the site's `fill`. `siteplan::Site::owner` is the one
derivation, and the frame, the stand-ins, the handout and the checks all read
it.

**Where a box stands is derived** (spec-0059). A box states its extent, its plane and its headroom; a seam states which face of its `a` box it sits on and where along that face the crossing is; the compiler packs the graph onto the grid from the boxes whose corner the author pinned, one cell beyond each named face. The seam carries no rise and no sill: both are the two floors' business.

| Element | Behavior |
|---------|----------|
| `region` | `{min: [x,y,z], extent: [dx,dy,dz]}` in world coordinates — the whole map's one region, and the number the brief hands down. **Required, with no derived spelling**: there is no "compute this from the boxes", so extent-flows-up is unrepresentable rather than forbidden, and `DW0826` refuses a box that does not fit while naming the box. Extents are `NonZeroU32`, so a zero-volume region is a schema failure (`DW0100`) rather than a rule some check has to remember. The water plane is deliberately absent — `horizon: ocean` in the stage-1 world document already fixes sea level. |
| `datums[]` | `{id: datum/<kebab>, y, note?}` — named ground planes. Ids are the ordinary `DW0110`/`DW0111`; a `floor` naming an undeclared one is the ordinary `DW0112`. |
| `boxes[]` | `{node, extent: [dx,dz], floor, ceiling, base?, min?: [x,z]}` — **exactly one per graph node** (`DW0824`). **A box states what it is; where it stands is derived** (spec-0059). **A box is a cuboid** (spec-0098 §14, correction 3): its footprint is `min`/`extent` and its vertical extent three declared planes. `floor` is the one authority for the walk plane, `{"datum": <id>}` or `{"y": <n>}`. `ceiling` is `{"clearance": <cells>}` (a lid at `floor + cells`) or `{"open": <cells>}` (sky-open: exactly that many courses of air claimed, nothing above). `base` is `"ground"` (the default: the claim reaches down, column by column, to the lower of the terrain and the floor course, stopping one course over any place stacked under that column, and the whole hands that ground and fixes the ring) or `{"aloft": <n>}` (the place hangs: its claim stops `n` underside courses under the floor course everywhere, it is handed no ground and no fixed ring, and terrain reaching the claim is `DW0990`). The claim is the footprint grown by the one-cell ring, from that bottom to the open top, the lid or the roof zone; two places conflict only where their cuboids overlap (`DW0827`), and a cell no claim covers is the site's fill — on an `open` site, the commons under and between aloft places. `min` is a **pin**, optional: a creative choice of the corner, verified against the packing (`DW0883` when the two disagree); every connected component of the seam graph pins at least one box, or nothing places it (`DW0883`). Horizontal extents are any whole number of blocks; the packed box stands inside the region (`DW0826`, naming how its corner was obtained); boxes are disjoint (`DW0827`). A box's extent and headroom are the author's declaration, and no class refuses them. |
| `boxes[].atmosphere` | (spec-0080) The atmosphere this place stands under from the first tick, painted at world setup over the box's play space (`PlacedBox::space`) grown by the client's blend reach on every face (*The blend*, under *World / build output*) — `areas[].atmosphere`'s capability on a box. Two carried boxes whose own 4-cells meet under different atmospheres are `DW0929`: a box and its neighbour sit one cell apart, so neighbours carrying two skies must be a whole 4-cell apart. |
| `boxes[].roof` | (spec-0098) `{courses, eaves}`, optional, on a roofed box: the roof zone the whole reserves — the shell footprint grown by `eaves` on every side, from the ceiling course up `courses` courses. Massed solid at stage 5 over a place no piece fills; drawn by the place's own piece once detailed. Refused on a sky-open box and where its courses rise into another place's play space or floor course (`DW0988`); an eave stops where a neighbour's shell begins. Both numbers are judgements the plan states against `docs/reference/roof-and-facade-craft.md`. |
| `ceiling: {"open": n}` | A sky-open place — a courtyard, a shore, a summit, a bridge's deck. It claims exactly `n` courses of air over its walk plane and **nothing above them**, which is what makes a `clearance` volume over a courtyard the whole reserving sky rather than two authorities over one cell, and lets a place hung over the courtyard stand in its sky: a climb from the courtyard into a place above is a hole through that place's floor course, which the courtyard's headroom reaches when its top is one course under it (one short is `DW0828`, naming the gap). |
| `base` | (spec-0098 §14, correction 3) `"ground"` (the default, not printed) or `{"aloft": n}`. A ground place stands on the site's ground and is handed it (§2c). An aloft place hangs: its claim's bottom is `floor − 1 − n`, it is handed no ground (`allocation`'s `ground.base` says which; `fixed` and `columns` are empty and no wall seam has a `ground_y`), its ring is never fixed, and the terrain is never consulted except to refuse terrain reaching into the claim (`DW0990`, third shape). The space under it is whoever claims it — a lower place's open headroom, a scenery box — else the site's fill: on `open`, the commons; on `solid`, rock up to its bottom. |
| `seams[]` | `{edge, face, at?, meets?, opening? \| contact?, stair_in?}` — **exactly one per traversal edge** (`DW0824`). `face` is one of the engine's six face names (`east`/`west`/`up`/`down`/`south`/`north`), **of the edge's `a` box**. `at` is where the crossing sits on `a`'s face and `meets` where it sits on `b`'s, each an **offset from that box's own low corner**, never a world coordinate — one integer along a wall face (cells along `z` for east/west, `x` for north/south), `[dx, dz]` through a floor or ceiling — and each defaults to **centred**, `max(0, (extent − width) div 2)` by the standard's width (a contact's `extent`, or `0` for a contact with none). **The packing** (spec-0059 §3): the pinned boxes seed it; then `seams[]` in document order, repeatedly, a seam with exactly one end standing places the other **one cell beyond the named face** (`x0(b) = x0(a) + dx(a) + 1` across `east`, and so on) with `corner(b) = corner(a) + at − meets` along it, until a pass places nothing. A seam whose two ends both stand places nothing and is **checked**: the cells it names from `a` must be the cells it names from `b` (`DW0828`, the loop that does not close; `DW0883` when `b`'s corner is a pin). An offset off its own face, or of the wrong shape for the face, is `DW0828`. **The sill is not written**: it is `max(floor(a), floor(b))`. `delvec validate` prints every box's corner and the seam that placed it — the derivation handed back, never typed. A seam allocates **one of two kinds** of connection, and both or neither is `DW0876`. `stair_in` names which of the two boxes hosts the treads: required on a `stair` (`DW0830`) and refused on anything else (`DW0824`). |
| `seams[].opening` | **A PORTAL**: a named standard from the metrics table (`DW0812` on an unknown name), or a size the seam declares itself, `{"width": w, "height": h}` on the face's two in-plane axes — the author's own opening, a one-cell rope-bridge end included. `DW0829` refuses either when it does not fit the shared face or its sill cannot be reached. A body crosses at exactly the cells `at` and the standard allocate, and **every one of them must be passable** over the built bytes (`DW0836`), and the hole must **lead somewhere**: a body standing in it steps onto ground on both sides, with every bar open (`DW0986`; a `drop` owes its high side only). |
| `seams[].contact` | (spec-0053) **A CONTACT**: the two places simply meet along a front, rather than through a doorway. `{extent?: [u,v]}` — the span in cells on the face's own two in-plane axes, anchored where `at`/`meets` put it; omitted, it runs from that corner to the far edge of the shared face, which is how a front along the whole of a face is written (and both offsets then default to `0`). **What it means**: the boundary is continuous ground — the derivation writes **no wall along the span**, and wall as ever outside it, and no frame ring, because a ring around a fifty-five-cell front is a wall drawn in a second block. **What the proof reads**: the author allocates *where* the places meet and the engine measures the crossing profile from assembled bytes, so *"this face is fine"* is never a declaration this engine accepts. Seams stay allocated, never discovered: the span is the edge's allocation set for `DW0838`, so a crossing outside it is still a refusal. **No door check applies** — a contact has no opening name to resolve and no single sill, and calling a wide front a door would make every downstream door check wrong. A contact carries `walk` or `drop` only; `stair`, `barred` and `vision` are excluded. Its width is the author's: a front one cell wide is as legal as a wide one. Refusals: `DW0876`, `DW0877`. |
| `seams[].form` | (spec-0098) **Required**: what the crossing is, in a few words — "a wooden arch bridge, 3 wide", "a stone stair, down 4". A creative judgement the plan states once; the handout gives it to **both** places the seam joins, so each designs its side knowing what meets it. Never player-facing, never inventoried. A connector that is itself a structure — a bridge, a long stair over a gap — is a place of its own with its own piece; its neighbours provide a landing or an opening at each seam. |
| A seam's **rise** | **Derived, never authored.** It is `floor(b) − floor(a)`, which the plan has already stated by putting the two places where it put them. Authoring it would be authoring arithmetic — unlike `critical_path`, which is authored precisely because it is a *choice* among many — and a second declaration of it could only agree or be a refusal teaching nothing the datums did not already say. `DW0830` and `DW0831` judge the derived number. |
| `fill` | (spec-0098 §2b) **Required, with no default**: what every cell no place claims and no volume covers becomes. `{"kind": "solid", "block"}` — the enclosed site, whose places are carved out of rock; or `{"kind": "open", "terrain", "surface", "below"}` — a natural ground surface under sky: at the terrain's height the `surface` block, under it `below`, above it air. `terrain` is `{"kind": "flat", "datum"}` (the datum is the terrain's walk plane, so its surface block stands at the datum's `y − 1`) or `{"kind": "heightmap", "heightmap", "base_y", "range"}` — a greyscale PNG in the campaign, exactly the region's `x × z` pixels, each pixel the surface `y` `base_y + value × range / 255` (integer division), read by the loader and attached to the campaign. A plan without `fill` does not parse (`DW0100`); an undeclared datum is `DW0112`; a heightmap unread, unreadable, the wrong size or outside the region's `y` span is `DW0826`; a block that is not a block state is `DW0193`. Per region it is overridden by `volumes[]`. |
| `volumes[]` | `{id: volume/<kebab>, region, role, note?, block?}` — the mass the WHOLE owns: `massif` (the mountain a cave system is inside), `ground` (the plane under a village), `clearance` (the sky a silhouette needs kept empty). They stand beside places, under them and over them, never inside one (`DW0835`), and they answer to the region like anything else the plan places (`DW0826`). A volume's block is its own `block`, else the fill's block of its kind (spec-0098 §2b): a `massif` is the `solid` fill's block or the `open` fill's `below`; a `ground` is the `open` fill's `surface` over `below`, or the `solid` fill's block; a `clearance` is air and names no block. |
| `identities[]` | `{fact, measure, cmp}` — guarded comparisons binding the plan to the geometry brief's written numbers. `cmp` is `eq`/`lt`/`le`/`gt`/`ge`. `measure` is a tagged union over a **small fixed vocabulary**, not a parsed string: `{"of":"region-extent","axis":x\|y\|z}`, `{"of":"box-extent","node":…,"axis":x\|z}`, `{"of":"box-height","node":…}`, `{"of":"distance-xz","from":…,"to":…}` (Euclidean between footprint centres), `{"of":"datum-y","datum":…}`. An unknown measure is an ordinary `DW0100` and a node it names is checked like any other reference. **Marked judgement**: the vocabulary will grow, and the falsifier is the first brief fact a campaign cannot bind with it — at which point the missing measure is added as a variant, never worked around by binding a different fact. |
| `sightlines[]` | `{edge, from, to}` — **one per `vision` edge** (`DW0824`), the segment the stage-5 battery walks. A vision edge carries a sightline rather than a seam because a vista's two ends are routinely not adjacent — a tower seen from a shore shares no face with it — so the seam construct cannot state the one thing it asserts. Each end must lie inside the place its connection names (`DW0824`): the proof walks exactly this segment, so ends elsewhere would prove a different claim, green or red. |
| `views[]` | `{id: view/<kebab>, eye, look_at, note?}` — the named exterior vantages the silhouette is judged from, rendered beside the stage-2 reference sheet. Optional; a plan with zero views has that zero stated in the binding line. |
| `lighting` | `{fixture, min_light}` applied to every enclosed box, so a blockout interior is walkable at night without per-box surface. **The engine's existing area-lighting object**, not a twin of it, so it answers the same range rule with the same code (`DW0196`). |
| `max_drop` | Optional: the deepest fall, in blocks, a designed `drop` in this plan may take — the author's own policy, which `DW0831` confirms every drop seam honours. Absent, no policy cap applies; the unarmoured survivable fall holds every drop either way. |
| Binding | Every run that carries a plan prints a second line beside the layout-graph one: boxes and **the pairs compared** (with how many are pinned, how many derived, and in how many components), seams (stair, drop), datums, whole-owned volumes, identities, sightlines and views — then one **placing** line per box with its corner and how it was obtained (spec-0059). Two zeroes are called out as findings rather than counted: a plan with no view (the visual review has no declared vantage) and a plan with no whole-owned volume (the rule keeping the whole's mass out of the places examined nothing). A plan with no identity is `DW0834` in its own right. |

### The horizon's surround (spec-0026)

A horizon is a base and that base's params. Two of the bases — `void` and
`ocean` — are **world-generator settings**: what lies outside the placed
geometry is an analytic fact, one superflat layer stack, modelled per column by
`nav::Ambient`. `valley` is not. Its ground is real blocks in real structure
templates, generated by `compiler::surround` and placed by the same bootstrap
that places every other piece — so its *ambient* is `void`, because there is
nothing analytic out there: everything out there was built.

That difference is the design, not an implementation detail. A surround entering
as a new `Ambient` variant would owe a new branch to gravity settling, the
occupancy model, relight, the fluid model, boundary safety and the snapshot
renderer, and each branch would be a second model of the same ground that could
disagree with the first. Entering as **placed blocks** it owes none of them:
every proof already knows how to read a block.

**What it rings.** A surround rings a **declared** extent, which means a site
plan's `region` — required, non-derivable, and which no box may grow. A campaign
that seats pieces with `areas[]` states no extent and is refused (`DW0855`); the
union of what it happens to place is not a substitute, because areas sit on the
compiler's fixed 256-block stride and that union is mostly the void between them.
Reading the region rather than the placed pieces is what keeps the landform
fixed: a part can never push a mountain outward, and space a plan reserved and
has not yet filled stays reserved instead of being eaten by terrain.

**The second declaration, and the one that lets a SITE be placed.** A campaign
whose `areas[]` holds exactly ONE area bound to exactly one `prefab` has stated
an extent too: its whole map is that piece, and the piece's declared structure
size is the number. `dsl::placement::Extent` is the one predicate — the
validation tier refuses on it and `plan::surround_rect` derives the rectangle
from it, so the tier that refuses and the tier that builds cannot disagree — and
the authority it chose is printed in the surround's binding line. Nothing flows
upward from a part in that case, because there is one part and it is the whole:
detailing the piece's interior cannot move its box, and enlarging its region is a
re-export of the asset. This is what a **site** is — a building together with its
island, its moat and its banks inside one box — placed through `areas[]`, since
`DW0839` refuses a plan beside a non-empty `areas[]`. Two or more areas, or one
area drawing from a pool, still state nothing: the stride argument is about the
first, and a pool's footprint is the solver's answer and would move with the seed.

**The shape.** A rectangular annulus whose total footprint is `ratio` times the
region's on each axis, with three zones outward from the region edge: a flat
walkable **gap floor**, an **inner slope** rising to the crest line, and the
**crest band and outer face**, which is where the trees are. The radial profile
keys on a domain-warped distance from the region rectangle and the rim is a
ridged multifractal over the compiler's own value noise, so the silhouette reads
as rock rather than as a box — the curve is in the warp, not in a diagonal
primitive.

**Un-climbable by construction, and proven anyway.** The construction is one
sentence: **no surround column stands exactly one block above the gap-floor
datum.** Everything is at the floor or below it — the floor and the hollows in
it — or at least two above it, which is the rim. A flood from the gap floor
climbs at most one block a step, so its component is everything at or below the
datum and is bounded above by it; the first thing outward stands two blocks
higher, and a two-block riser is the one thing vanilla's auto-step and jump
cannot take. Above the barrier the landform's shape is unconstrained, which is
what lets a hillside be broken ground rather than a two-valued surface.
`DW0854` proves it again over the assembled bytes, because gravity settling, a
stage-7 edit script and a palette of different-height blocks all happen after
the generator has finished.

**The rim's height is the height it is declared at.** `rim_height` is what the
crest reaches at the ridge peaks; a saddle falls to `RIDGE_FLOOR` times it, and
a typical crest sits near three quarters. The crest is clamped one under the
declared rim so the build-range fence is exact rather than approximate.

**The moat.** The surround rings the region a site plan DECLARES, and a plan
under-fills its own region while it is being built. Every region column no piece
FLOORS — no block at or below the gap-floor datum — receives the gap floor's own
ground and surface treatment, as row-strip tiles (`horizon/valley/m<n>`), so the
box garden's floor is continuous from the rim to every piece footprint. A column
whose content is entirely ABOVE the datum is filled too: the valley floor runs
on under an elevated storey. A floored column is untouched — the piece owns its
ground, holes and basements included.

**Not an `AreaPlacement`, deliberately.** `plan.areas` is what the boundary
region derives from, what relight lights, what anchors resolve against and what
analysis counts, and a mountain is none of those. The surround is
`plan.surround`, and the sites that need it opt in through
`Plan::placed_pieces` — the one iterator every PLACEMENT site reads (shipped
`.nbt`s, forceload spans, `place_all`, the placement sentinels, the extent check
and the voxel model).

**One piece, many templates.** The annulus is far past the vanilla 48-per-axis
template cap, so it ships as many `.nbt` files and is one `PiecePlacement` with
many `PlacedTemplate`s — the same absorption *A piece's blocks arrive as one
template or as a tile set* already describes. The bytes are synthesized at build
time and never exist on disk; the structure reader merges them before it reads
the prefab library, and a prefab that shared a filename would still win.

**Biome.** Per-band `/fillbiome` in `setup_finish`, where `place_verify` has
already proved the chunks exist. That is vanilla's own channel for grass and
foliage tint, water colour, ambience and sky, so the surround reads as its biome
with no resource pack anywhere in the delve. The modification cap is raised for
the pass and restored, because a band is painted in one command and a truncated
command leaves a horizon painted half one colour.

**Every state it writes is judged against the pin, at the emitter.** A
structure template carrying a block id 1.21.11 does not have loads that cell as
**air** — the `.nbt` is well-formed, the build exits 0, the double-build
byte-identity gate passes, and the terrain simply has holes in it. No count
moves either: templates, biome bands and gap-floor cells are all counted before
the game reads a byte. So `serialize_tile` runs every palette entry through
`delvewright_dsl::blocks::BlockRegistry` — the id, every property name and every
property value — and dies naming the id and how many cells carry it. It is in
the emitter rather than in a test for the reason the emitted-command rule gives:
the creator running the tool does not run `cargo test`. Measured on this
generator: one rock id changed to a plausible near-miss builds green, emits
fourteen templates, prints a byte-identical binding line, and ships 2087 cells
that come up as air.

The same line judges the **connection** half — whether a state omits a
shape-carrying property, which is what turns a bare fence into a lone post. The
surround's vocabulary is rock, ground, logs, leaves and ground cover and carries
no connection class, so it derives nothing from neighbours; the assertion is
what makes that a fact about the emitted palette rather than a claim;
`pink_petals`' `flower_amount`/`facing` are an authored decision and are
authored.

**Binding.** Every surround build prints its templates, biome bands, the
rectangle and **which authority stated it**, and the standable gap-floor cell
count the climb proof floods from — a flood that started from nowhere passes for
free and looks exactly like one that did not.

**Not reachable from the DSL**: the generator carries a second flora (cherry
over `minecraft:cherry_grove`) and a second surface palette, on one code path
with parallel id tables. Neither is exposed, because the gallery element a second
flora needs is a second whole-map campaign and a surface lands with its element
or it does not land.

### The blockout (derived — there is no document)

Stage 5 has no element table because it has no elements: the whole map's mass is
derived — a pure function of the site plan, the layout graph, the metrics table
and the engine — so there is nothing an author writes here and nothing an author
can get wrong here. Both authored documents reach it: the plan states where the
boxes and the seams' cells are, the graph states what those seams are and what
headroom a sky-open place claims. What a reader needs to know
about it is what it BUILDS, which is fixed:

| Thing | What the derivation makes of it |
|---------|---------|
| the fill | The site's declared `fill`, over the whole region first: a `solid` site's block everywhere, or an `open` site's terrain — the `surface` block at each column's height, `below` under it — merged into rectangles of equal height (spec-0098 §2b). |
| a box | **A stand-in, never shipped** (spec-0098 §8), and only for a place no piece fills: in the cells that place owns, a ring wall from the claim's bottom to the top of the play space, the floor course in the place's own **accent** (cycled deterministically over the plan's boxes, so the colour under a body's feet names the place), a lid unless sky-open, and a declared roof zone massed solid in the roof block. Nothing is written in the fixed ground, in a neighbour's cells or in a gap; a bound place's owned cells are written by its piece alone. |
| the ring's ground | For every place, bound or not, its fixed ground cells — the terrain continued to the plot's edge in the fill's `surface`/`below` (or `solid` block); at the columns of a seam at grade (its sill one course over the terrain or less) the ground is the sill minus one, flat across the opening; a seam aloft — a bridge's deck, a door high in a wall — fixes no earth under it, and the column between the terrain and the sill is the owner's. |
| a seam | Where the place owning the seam's plane is a stand-in: a frame of contrasting wall around the opening, in that place's own cells, and the opening itself cut to air — or filled with the bar, on a `barred` way, which the world-load seal model then measures shut exactly as it measures a prefab-authored gate. Where a piece owns the plane, nothing: the piece cuts its own opening and ships its own bar. |
| a stair | A stepped run inside the box the plan named, at the **gentlest standard pitch the run really has room for** — chosen by `siteplan::gentlest_pitch` over the run `siteplan::stair_run` reports, which are the two calls `DW0830`'s stand-in finding reads, so the plan-time finding and the built geometry cannot disagree; where no standard fits, the stand-in lays no treads and the place above is unreached (`DW0837`). Laid only in a host no piece fills. |
| a climb | A ladder in the lower place when it is a stand-in: through a floor, from the lower floor up into the hole's first cell, hung on a pillar the stand-in raises beside it (and, in the hole, on the floor course); up a wall, against the wall under the opening up to the sill's course. Proven by the climb moves of the nav model (spec-0099) — `DW0837` reaches over it and `DW0986` crosses the opening on it. Where the lower place is bound, its piece hangs the ladder up to its own ceiling; the rung in a floor's hole is the hole owner's, so a stand-in that cut the hole hangs it whatever the lower place's binding. |
| a stairwell | Over every run through a punched **floor**, the floor cut away wherever a body climbing the run needs it gone (`blockout::stairwell`). |
| a volume | Mass of its own `block`, else of the fill's block of its kind; a `clearance` kept empty. |
| the order | The fill, the volumes, every stand-in, every stand-in's interior cleared, every place's fixed ground, every seam's frame, every stair, then **the openings**, and the stairwells after them, because a stairwell is measured over the holes it opens off. |
| lighting | The plan's one `lighting` setting, applied to every enclosed box by the ordinary relight pass. |
| the record | `validation/blockout.json`: the binding line and every place still massed by name, which the staging gate reads — a stand-in never ships. |

Its diagnostics and the battery that judges the result are `DW0821`/`DW0836`–`DW0839`/`DW0877`/`DW0986`/`DW0990`.

### l10n sidecars (`l10n/<code>.json`)

**Two key spaces, and the difference is where each is read.** Everything in this
section is the **campaign's own** key space: `world.title`, `npc.<n>.name`, the
keys a sidecar answers, the keys `DW0180` counts, the keys `delvec l10n-inventory`
hands a translator. They are campaign-*relative*, because a sidecar is read inside
one campaign's directory. What the compiler emits — every `{"translate": …}` a
component carries and every row of every `assets/delvewright/lang/<mc>.json` — is
each of those keys under that delve's **pack namespace**, `delve.<campaign_id>.`:
`world.title` ships as `delve.doune-castle.world.title`. Chrome is namespaced the
same way (`delve.doune-castle.delvewright.ui.class.title`) — the pack is where it
lives too. `dsl::l10n::pack_key` is the one authority; nothing else builds the
prefix. See [One delve, one vocabulary](#one-delve-one-vocabulary) for why.

Envelope `{dsl_version,campaign_id,kind:"l10n",lang,content,source?}`; `content` =
flat **stable key → translated string**, `source` = the same keys → the canonical
English each row was translated **from**. Key inventory derived from stage docs
(`world.title`, `world.outro`, `area.<a>.name`, `class.<c>.name/.blurb/.kit.<i>.name`,
`npc.<n>.name`, `actor.<a>.name` (a scripted puppet's nameplate, only when set),
`quest.<q>.goal`, `obj.<q>.<o>.title/.hint`,
`obj.<q>.<o>.missing_item_hint` and `obj.<q>.<o>.item_name` (a `collect`'s
collected-item display name, only when authored),
`dlg.<n>.<node>.text/.opt.<i>.label/.opt.<i>.tooltip` (the tooltip only when
authored), `wave.<w>.mob.<i>.name`, `wave.<w>.health_bar.title` and
`actor.<a>.health_bar.title` (a stated health-bar title, spec-0073, only when
authored — a derived title emits the name's own key)) plus effect strings
`fx.<q>.oc.<o>.<i>.narrate|.give`, `fx.<q>.done.<i>.…`, `fx.trig.<t>.<i>.…`, and a
`bonfire`'s authored rest-dialog strings `fx.….rest_prompt|.rest_label|.save_label`
(and its button tooltips `fx.….rest_tooltip|.save_tooltip`, spec-0078)
and a `close-gate`'s authored `fx.….sealed_hint` (unauthored ones are
absent because the compiler bakes its canonical English, the
`world.boundary.message` precedent), plus `lethal.<volume>.message` — a lethal
volume's death wording (spec-0031). That one is **required rather than
defaulted** (`DW0512`): a player reading a raw key at the moment they die is the
worst place in a delve for a hole, and there is no compiler-owned English that
could be right for a cliff, a lava pit and an acid pool at once.
**Every effect root emission can lower** is inventoried, not just the quests
stage's three: `fx.trap.<trap>.<i>.…` for a `traps[].payload`
(spec-0022 — a trap's consequence is
commands), `fx.dlg.<npc>.<node>.<opt>.<eff>.respawn.<j>.…` for a dialogue
option's `set-checkpoint` `on_respawn` bundle,
`fx.sc.<shortcut>.<i>.…` for a `shortcuts[].on_unlock` beat (root 6) and
`fx.death.<i>.…` for the campaign's `on_death` (root 7); a shop offer's effects are `fx.shop.<shop>.<offer>.<i>.…`
(root 8), and spec-0074 keys a fight's `on_kill` by the fight itself —
`wave.<wave>.on_kill.<i>.…` / `actor.<actor>.on_kill.<i>.…` (root 9), because the
bundle belongs to the body, not to a beat; spec-0082 keys an assembly strike
step's `on_land` the same way, `assembly.<assembly>.strike.<step>.<i>.…` (root
10). `dsl::l10n::effect_roots` (immutable,
for the glyph/text-fit/sound consumer scans) and `effect_roots_mut` (for
`each_string`, hence `inventory` + `localize`) enumerate the same ten roots, so
what is measured and what is translated cannot drift; each ref carries the `stage`
it was authored in, so a dialogue-rooted `DW0326`/`DW0328`/`DW0330` names
`dialogue` rather than `quests`.
**Nested effects**: a `narrate`/`give-item` inside a `sequence` step or
an `on_respawn`/`on_caught`/`on_arrive` bundle is inventoried and localized too,
under a position-derived child key = parent `fx.…` key + a stable segment
(`seq.<step>` for a sequence step; `respawn`/`caught`/`arrive` for the bundles) +
the effect's list index + leaf, e.g. `fx.<q>.oc.<o>.0.seq.1.0.narrate` (nesting is
arbitrary-depth). Keys are purely position-derived → deterministic + byte-stable.
**Entity display names are keyed by their TEXT, not by their site.** An NPC
(`npc.<n>.name`) and a scripted actor (`actor.<a>.name`) are two DSL surfaces for
one thing a player reads — a nameplate over a body — and one character routinely
occupies both: a stage-2 NPC that stands and talks, plus one actor puppet per
cutscene pose. Per-site keys would ask a translator for `Polyphemus` five times and
let it be answered five ways, so **the first site (NPCs before actors) declaring a
given name owns the key and every later site carrying the byte-identical name
emits that same key**. The inventory asks once; two bodies a player reads as one
character cannot render as two. Deliberately scoped to that class: prose keeps one
key per site (two coinciding English strings may legitimately need different
renderings), and `wave.<w>.mob.<i>.name` is **not** merged — same shape, but
merging it retires keys live campaigns already translate, which is an owner call.

Coverage is **exact**: missing/absent/inconsistent → `DW0180`; orphan → `DW0181`;
a key in the compiler's reserved `delvewright.` chrome namespace → `DW0186`.
Excludes authoring context (theme/premise/persona).

**Coverage is about key SETS, and that is not the same as being up to date.**
Rewrite an authored line and its translation is present, applied and **wrong**,
with no key moved and every coverage check green. `source` closes that: it records
the English each row was translated from, so the compiler compares
(`DW0187`) instead of a human auditing. It is load-bearing for entity display
names in particular — their key belongs to the first site declaring a given text,
so renaming ONE body migrates the key to ANOTHER, and the row that goes stale is
not the row the author edited (`DW0180` points at the newly-required key, which is
somewhere else entirely). `source` is optional: a sidecar without it parses,
and its unguarded rows are **counted** by `DW0188` on every run, so an unadopted
sidecar never reads like a checked one. `tools/creator/i18n-translate.py` writes it, so
adoption is a re-run with no retranslation.

**Every string field in the DSL is classified, or CI is red.** `DW0185` proves that
a string the inventory *knows about* reaches a component; it cannot see one the
inventory never met, which ships English silently. `crates/dsl/tests/l10n_surface.rs` closes that half: it
enumerates every string-valued property of the seven stage schemas (derived from
the Rust types, so complete by construction; the test prints the count) and requires each to
be classified `Inventoried` / `Reference` / `Machine` / `NotPlayerVisible(<why>)`,
in both directions. A new `String` anywhere in the DSL fails it until somebody
records whether a player reads it. It is a test, not a `DW` code, because the
defect is in the compiler: no campaign input can produce it.

**`delvec l10n-inventory <dir> [--lang <code>]`** emits that inventory as one JSON
document on stdout — the work list a translator (in-agent, human, or an external
API via `tools/creator/i18n-translate.py`) is handed up front, instead of discovering it by
writing an empty sidecar and reading the coverage diagnostics back:

```
{ campaign_id, dsl_version, lang, declared, sidecar_present, world_title,
  npcs:    [{id, name, archetype, speech_style, demeanor?, motivation}],
  entries: [{key, en, kind, speaker?, situation?, existing?, stale?}] }
```

`entries` is the inventory itself (a CLI test asserts the key set equals what
`DW0180` demands, so the two cannot drift). `speaker` is the NPC whose dialogue
tree the key belongs to (`dlg.<npc>.…`, `npc.<npc>.name`; a `.opt.<i>.label` is the
player's reply *inside* that tree); `existing` is what `l10n/<lang>.json` already
translates, so a re-run fills only the gaps; `stale: true` marks an `existing`
translation whose recorded `source` differs from the English the line reads now
(the `DW0187` condition), so a re-run redoes it rather than re-recording the new
English against the old translation. `kind` (`dsl::key_kind`, from the key alone)
is the class of text — `name`, `title`, `description`, `objective`, `refusal`,
`dialogue`, `bark`, `option-label`, `button-tooltip`, `item-name`,
`item-tooltip`, `prompt`, `narration` — and a CLI test over the gallery asserts
every row has one. `situation` (`dsl::key_situations`) is the campaign context the
line is said in, derived from the stage documents: the quest goal and its
`happening`, the objective and what completing it does, a cast placement's
`doing`, the NPC line a dialogue option answers and what choosing it does, the
shop an offer sits in, and the nearest enclosing effect's `happening`. Together
they are the intent a transcreator writes from. Persona rows carry voice, never plot
(`secret`/`backstory`/`relationships` are excluded). Runs **before** validation
gating — an incomplete sidecar is the normal state when you ask — and needs no
prefab library; only an unparseable campaign fails (exit 1). See
[i18n.md](i18n.md).

### Language delivery — i18n v2 (spec-0029)

**A released delve ships every declared language; the client picks its own.**
`delvec build` (no `--lang`) emits every authored player-visible string as a
**translatable text component**

```json
{"translate": "delve.<campaign_id>.<l10n key>", "fallback": "<English source>"}
```

and writes one `assets/delvewright/lang/<mc_code>.json` per declared language,
plus `en_us.json`, into the resource pack the release already ships. A client
auto-selects the lang file matching its own locale; a locale we do not ship, a key
a translator missed, **and a player who declined the resource-pack prompt** all
resolve through the component's own `fallback`. That is why the fallback rides the
component and not the pack's `en_us.json`: a declined pack has no lang files at
all, and the delve must still be playable in English.

| Piece | Behaviour |
|---|---|
| Key set | The existing l10n inventory, unchanged. `each_string` stays the single authority over what is translatable — no second key scheme, no second inventory. The **pack** key is that key under `delve.<campaign_id>.` (below). |
| Tagging | `dsl::l10n::tag_translatables` rewrites each inventoried string to `<U+E000><pack key><U+E000><English>` **once**, before `Plan::build`. From there the tag is the compiler's only evidence that a string is player-visible. Emitters lower it through `emit::tr` / `emit::snbt_component`; non-component consumers read it through `dsl::l10n::plain`. The tag is where the delve's namespace is applied, so no emitter has to know about it and none can forget. |
| Lang files | Flat `{key: string}` in `BTreeMap` order (ADR-0006). `en_us.json` **is** the live inventory, `delve.<campaign_id>.`-prefixed; each other file is its sidecar's `content`, likewise. The key sets must be equal — a hole fails the build (`DW0180`/`DW0181` at emit time), because a hole is a player reading a raw key. |
| Language codes | `dsl::mclang::mc_lang_code` normalises (lowercase, `-`→`_`) and then **checks membership against the pinned client's own language set** — `CLIENT_LANGS`, 143 stems **derived** from Mojang's 1.21.11 asset index (`tools/maintenance/derive-client-langs.py`; digests in the module header), never transcribed. The membership check is what makes normalisation safe: a bare rewrite alone would invent `de` from `de`, a filename no client asks for, and a lang file nobody loads is a language silently dropped. A bare language resolves to `<lang>_<lang>` if the client ships one, else to its sole file; ambiguous (`zh`, `sr`, `be`) and unknown codes are `DW0184`. Baked into the source — the compiler never reaches the network during a build (ADR-0006). |
| `--lang <code>` | The single-language bake (spec-0029 §4): strings are swapped before emission, nothing carries a translate key, and the build ships **no** lang files — there is nothing for a client to select between. For local dev and one-language artifacts; the release path does not use it. |
| Art titles | `emit_narrate` does not `to_ascii_uppercase()` an `art` string — a case transform is something a `{"translate": …}` component cannot express, since the client resolves the lang file after the compiler is gone. The `delve:art` font carries a **second bitmap provider** over the same atlas addressed by the lowercase letters, so a lowercase letter renders through its uppercase bitmap: identical pixels, in every language. Cells with no lowercase form are `\u0000` (vanilla's unused-cell marker), so no char is claimed twice. |
| Width gates | `DW0330`/`DW0331` check source **and** every declared translation. Any declared language may be what a player sees, so those checks are load-bearing rather than belt-and-braces. A styled span is measured as it draws (`textfit::width_for`): its markup is not drawn, an obfuscated span is as wide as its text, a bold span adds one font pixel per character (`textfit::bold_widening`, every character counted — the estimate can only be wider). |
| Build inputs | Every `l10n/<code>.json` is an input of **every** build (not just a `--lang` bake) and is hashed into `manifest.json` — the sidecar's bytes ship in the pack, so they are as much a build input as a stage document. |

#### Inline styles — a span of a line carries a style (spec-0096)

Any player-facing string — every row `each_string` walks, so every text class
the inventory knows — may carry styled spans:

```
The ledger is kept by [[obfuscated|someone else]] at [[italic,color=dark_purple|night]].
```

| Piece | Behaviour |
|---|---|
| Grammar | `[[<styles>|<text>]]`; `<styles>` is one or more of `obfuscated`, `bold`, `italic`, `underlined`, `strikethrough`, `color=<c>` (`<c>` one of the sixteen vanilla names or `#rrggbb`, emitted lower-case), comma-separated, no spaces, each once. `<text>` runs to the first `]]`, is not blank and holds no `[[`. Outside a span `[[` and `]]` do not occur; a single bracket is prose. Spans do not nest. One parser, `dsl::textstyle::parse`; malformed markup in the English or a sidecar row is `DW0975`. |
| Emission | `emit::tr` / `tr_with` / `snbt_component` / `snbt_text_component` — the only paths a tagged string reaches the tree by (`DW0185`) — lower a styled line to `{"translate": K, "fallback": F, "with": [S…]}` with `S_i = {"translate": "K.span.<i>", "fallback": T_i, <style keys>}`. `F` is the line with span `i` replaced by `%<i+1>$s` and every other `%` doubled; `T_i` is the span's text, `%` doubled (`dsl::textstyle::lower`). The line's own emitter style (a bark's `italic`, a title's `color`) sits on the outer component and the span inherits it, overriding only the keys it names. A line with **no** span emits exactly the pre-surface component. The SNBT form (an NPC's `CustomName`) is the same compound through `emit::text::snbt_of`. A `tr_with` caller passing its own `with` over a styled line panics: chrome carries no span. |
| Lang files | Each language writes `K → F` and `K.span.<i> → T_i` (`emit::server::styled_rows`), `en_us` from the English as authored. A translation's spans are **aligned to the English by style** (`dsl::textstyle::lower_aligned`) — the k-th span of a style in the translation answers the k-th span of that style in the English — so a sidecar row may place spans in its own order (`%2$s…%1$s`) and the style, which rides on the component, always meets its own text. A row whose span multiset is not the English's is `DW0976` at validate, re-proved by the build with the same code. No inventory key has a `span` segment followed by an index, so a span key never shadows a row (asserted). |
| `--lang` bake | Strings are swapped and untagged, so a styled line is drawn from its own spans in its own order as `{"text": "", "extra": [{"text": …, <style>}…]}`. |
| Visible text | `dsl::l10n::plain` returns the line with each span replaced by its text (`dsl::textstyle::visible`); `dsl::textstyle::legible` is the same with an `obfuscated` span's characters unreadable (U+FFFD), for a reader that asks what the player was told (`DW0982`), so every named exclusion below, the death plan's `message` the bot watches chat for (`deathplan::worded`), the art-glyph check (`DW0328`), both width gates and the dialogue checks (`DW0981` a question, `DW0982` a name told, `DW0983` a name tag; `compiler::telling`) read what is drawn, never markup. |
| Binding | `validate` prints `inline-style binding: N of M player-facing line(s) carry S span(s); R sidecar row(s) held to their English's spans`. |

#### One delve, one vocabulary

**A delve's keys are its own, because the client's language table is not.** The
key a component references and the key a lang file defines are
`delve.<campaign_id>.<l10n key>`, and no other delve can produce it.

The reason is where those keys are read. A Minecraft client merges every applied
resource pack into **one** language table — per key, highest-priority pack wins,
a server-pushed pack on top of every client-enabled one — and
`TranslatableContents` reads a component's `fallback` **only when the key is
absent from that table** (`Language.getOrDefault(key, fallback)`). A pack a
player installs stays applied across worlds and servers, enabled in
`options.txt` until they turn it off; a delve's pack is never installed — every
server this engine starts serves it ([A delve wears its own
textures](#a-delve-wears-its-own-textures)) — but a player may hold other packs,
and a client that joined another delve earlier has applied its pack in the same
session. So a key that named only its row inside one campaign — `world.title`
— is a key any delve that has ever been played can answer for the delve being
played now: a finished tour would toast another campaign's title, in a
language that delve does not ship. `fallback` cannot protect against this; it is
reached only when nobody defines the key.

| Piece | Behaviour |
|---|---|
| The rule | Every key that leaves the delve carries `delve.<campaign_id>.`. Campaign strings and compiler chrome alike — chrome rides the same pack, so a globally-keyed chrome row renders one delve's `Delve Complete` in another delve's language. |
| One authority | `dsl::l10n::pack_key` / `pack_namespace`. Applied at exactly two places: `tag_translatables` (which is what every campaign string and, through `Chrome`, every chrome string travels on) and `emit::lang_assets` (which writes the pack). No emitter builds a key. |
| Not the sidecar | An `l10n/<code>.json` keeps the campaign-relative key. It sits inside one campaign's directory, so it has nothing to collide with, and a namespaced sidecar would re-key every row on a rename for no gain. |
| Grain | The campaign id — what the pack file is named after, so rebuilding a campaign replaces its own pack rather than joining it, and two builds of one campaign cannot both be applied. Ids are kebab tokens with no `.`, so one delve's namespace can never prefix another's key. |
| Proof | `crates/delvec/tests/i18n_v2.rs`: two campaigns that differ only in id and text are built and their key sets compared — referenced and defined, both disjoint, and equal once the namespaces are stripped. Beside it, over one delve: every key it references is under its own namespace and is defined by its own pack, with no exemption list. |

#### One delve, one face

**The same rule, in the other space a pack writes into.** A baked skin ships twice
— as `assets/delvewright/textures/npc/<id>.png` in the pack and as
`delvewright:npc/<id>` in the `summon` that stands the mannequin up — and a client
merges the textures of every applied pack exactly as it merges their language
tables. So two delves that both cast a `keeper` would wear each other's faces, by the
same mechanism and with the same reach (packs stay enabled across servers).

| Piece | Behaviour |
|---|---|
| The rule | Every texture that leaves the delve is `<campaign_id>/<texture_id>`. Same grain as the key namespace, and the same argument. |
| One authority | `dsl::pack_texture_id` / `pack_texture_dir`, beside `pack_key` in `dsl::l10n`. Not `pack_key`'s dotted prefix: a texture id is a resource-location **path**, whose namespace separator is `/` — vanilla nests assets by directory and has no precedent for a `.` inside a final path segment. |
| One funnel | `dsl::namespace_skin_textures`, applied once per build to the campaign itself over `dsl::body_skins_mut` (the mutable mirror of `body_skin_sites`). An emitter reads `skin.texture_id` off the body it is summoning, so after the rewrite there is no un-namespaced id left for a new emit site to find. It returns `pack id → authored id`, which is how the bake finds `skins/<authored>.png`. |
| Not the creator's space | `texture_id` in `npcs.json`/`quests.json` and `skins/<texture_id>.png` on disk are unchanged, and `DW0190` (malformed) and `DW0309` (missing PNG) both read the authored id — they run before the rewrite, and `validate`/`analyze` never reach it. `SKINS.md` prints both names. |
| Proof | `crates/delvec/tests/skin_namespace.rs`: two campaigns that differ only in id and face bytes are built and their texture sets compared — baked and referenced, both disjoint, equal once the directories are stripped, and each delve's references match its own bake. Beside it, over one delve: every texture it references is its own. `crates/dsl/tests/body_skin_sites.rs` pins the mutable mirror against the walk `DW0309` and the bake share, over a campaign carrying every body class. |

#### A delve wears its own textures

**A `world.textures[]` row replaces a vanilla texture for the whole delve**
(spec-0084). A `minecraft:` path cannot be namespaced — replacing vanilla's file
is the capability — so the rule that protects every other pack entry has no
analogue here, and the pack that carries one is the delve's to serve and to take
away again when the player leaves.

| Piece | Behaviour |
|---|---|
| The census | `crates/delvec/data/textures-1.21.11.json`: every `assets/minecraft/textures/**.png` of the pinned client jar, keyed `minecraft:<path>`, with `w`, `h`, `sha256` of vanilla's bytes, `mcmeta` (vanilla ships a sidecar) and, where that sidecar animates, `animated`, `fw`, `fh` (one frame, by vanilla's rule: `animation.width`/`height` where stated, else the square of the shorter side). Its header states the jar's sha256 (`client_jar_sha256`); 3517 rows. Derived by `tools/maintenance/derive-client-textures.py`, which refuses a jar whose sha256 is not `versions.toml` `[render] textures_sha256`; `--check` compares without writing. A build never reaches for a jar. |
| One resolution | `compiler::textures::resolve(campaign, file)`, called by `delvec validate` (every row's findings at once, and a `textures:` binding line — rows resolved of rows declared, against the census size) and by `emit` over the same bytes from the manifest inputs, so the rule a row is judged by and the rule its bytes ship by are one function. |
| The image rule | A PNG that decodes; width `k·w₀` and height `k·h₀` for one integer `k ≥ 1` of vanilla's frame; with a sidecar, height `n·k·h₀` for `n ≥ 1` frames and a sidecar that is exactly vanilla's animation metadata (`animation` with `frametime`, `interpolate`, `frames` of indices or `{index,time}`, every index `< n`; `width`/`height` and any other key refused). Bytes equal to vanilla's (the census sha256) replace nothing and are refused; a re-encoded copy of vanilla's pixels is not detected. A row replacing a texture vanilla animates with no sidecar ships a still image, and the pack note says *still*. |
| The licence rule | `dsl::license::image_license_refusals`: the ADR-0013 allowlist (`license_allowed`, the catalog card's own list); `original` requires `source: original`; any other licence requires `url`; `CC-BY-*` requires `attribution`. Code `DW0741`, declared once in `dsl::codes::LICENSE_REFUSED`, the catalog's `DW_LICENSE` reading it. |
| The pack | Each row adds `assets/minecraft/textures/<path>.png` (+ `.png.mcmeta`) to the `extra` map `resourcepack::build_pack` takes; the creator's bytes, never re-encoded. A campaign with one row and nothing else ships a pack of `pack.mcmeta` and that texture. |
| The manifest | `textures/<id>.png` and `textures/<id>.png.mcmeta` are `inputs` (the loader reads `textures/` beside `l10n/`). `resource_pack_overrides_vanilla` (bool) stands beside `resource_pack_sha1` on every build that ships a pack: true exactly when an archive path starts `assets/minecraft/`, read off the paths the pack was built from (`textures::overrides_vanilla`). Every serving script reads it, and the staging gate holds it to the zip. |
| Delivery | **Served, never installed** — every pack, whatever it carries (spec-0084 §11). See the `server.properties` record. |
| What the creator is shown | Block textures through every review tool, the delve's pack layered above the jar: `delvec viewer --pack`, `delvec palette --pack`, `delvec render --pack`, `tools/creator/block-appearance.py --pack`, and `validation/chunky.sh --pack` (`-textures <pack>:<jar>`, the core's first-listed-wins order). Every other texture on a comparison sheet: `delvec textures <campaign> [-o review/textures]`, vanilla on the left and the override on the right, each nearest-neighbour to 256 px wide, a one-pixel separator, a chequered ground — written by a verb of its own because it reads the jar, which a build never does. `SKINS.md`'s **Textures** section: one line per row — id → vanilla path, scale and frames, the licence with its URL and attribution, which tools show it (*Chunky, viewer, palette* for `block/…`; *sheet only* otherwise), the sheet path, and *still* where it applies. |
| Proof | `crates/delvec/tests/texture_overrides.rs` (the pack of one texture, byte identity, the manifest key and input, every refusal on its shape, an animation's sidecar, the sheet), `crates/delvec/tests/remedy_reachability.rs` (each refusal's named move admits the row), the gallery's `hall-stone` row and four probes. |

#### Compiler chrome — the strings the compiler writes itself

A delve's on-screen text has two authors. Everything
above concerns the **campaign's** strings. The compiler writes fourteen of its own
— `New objective: `, `Delve Complete`, `Choose your class`, the default a bonfire
shows when the campaign authors no label — and each is keyed and translated, so a
player reading a fully translated delve does not see English chrome wrapped
around it.

They are **compiler-owned end to end** (`dsl::chrome`): the keys, the English, and
every translation live with the engine, and a campaign authors nothing. Nine of
them are *product chrome* (`objective.new`, `objective.complete`,
`campaign.complete`, `campaign.signature`, `campaign.banner`, `lobby.waiting`,
`class.title`, `class.body`, `respawn.wait`) — no campaign author wants to write those, which is
why the answer is not to give them an override; that would move the engine's
maintenance cost onto content. The other five are *diegetic defaults*
(`boundary.message`, `gate.sealed`, `bonfire.title|rest|save`) whose authored
overrides win — what lives in `chrome` is
only what the compiler bakes when nothing is authored.

| Piece | Behaviour |
|---|---|
| Key space | `delvewright.ui.<area>.<name>` within the campaign's own space, emitted as `delve.<campaign_id>.delvewright.ui.<area>.<name>` like every other key. Collision-proof both ways by construction: the l10n key scheme derives a fixed set of kinds and can never produce `delvewright.`, and vanilla never defines it either. A sidecar that writes one anyway is `DW0186` — a sidecar holds campaign-relative keys, so the reserved prefix is unchanged there. |
| Delivery | Identical to an authored string: the chrome string enters emission as a translation tag, an emitter lowers it through `emit::tr`/`snbt_component`, and a site that fails to is `DW0185` — chrome inherits the whole invariant rather than getting a parallel path. `Chrome::for_build(<campaign id>, <language>)` is what binds a chrome key to the delve; the plan-time default (`ChromeString::tagged()`, for a `sealed_hint` or a bonfire label the compiler bakes before the language is known) carries the bare key and is bound by `Chrome::rebind` at emission. A site that forgot to rebind would emit a key no pack defines, which `i18n_v2.rs` fails the tree for. |
| Sentences, not fragments | Five chrome strings frame a value and are **one key with `%s`**, carried by the component's `with` (`"%s — complete."`, `"New objective: %s"`, `"Waiting for the party — %s / %s"`, `"Back in %s"`). A concatenation freezes English word order into every language; `translate`+`with` is vanilla's own primitive for it. A unit test requires each language's placeholder count to equal the English's. |
| Lang files | Chrome is written into `en_us.json` and into each **declared** language's file — never into languages the delve does not already ship, or a French client on a Chinese-only campaign would read French chrome around English story. Partial-by-language reads as broken; uniform English does not. |
| The honest fallback | A language the compiler has no chrome table for gets **no chrome rows at all**: the client resolves through `en_us.json` (or, for a player who declined the pack, the component's own `fallback`) and reads English. Absent, never English written into `fr_fr.json` under a translated name. |
| Coverage | `dsl::chrome::TABLES` maps a client language stem to a table; its rows are the covered locales out of `dsl::mclang::CLIENT_LANGS`, plus the 5 English locales that need none. The rest render English. |
| `--lang` bake | A bake ships no lang files, so the fallback IS what the player reads: `Chrome::for_build` puts the baked language's text there, falling back to English. `%s` still substitutes — vanilla formats the fallback with the same `with` arguments. |

**The translations are unreviewed.** They are machine-produced from the canonical
English and have not been checked by native speakers; that is recorded in
`dsl::chrome`'s module header so nobody mistakes them for reviewed work, and a
correction is a one-line table edit. English stays canonical.

#### Named exclusions — where an authored string stays literal

An authored string that does not land in a text component cannot carry a translate
key. Every such site is named here and reads its string through
`dsl::l10n::plain` — the visible text, a styled span reading as its own text
(spec-0096); none of them is rendered by a client. Anything **not** on this
list that emits an authored string outside a component fails the build with
`DW0185`, so this table cannot silently grow.

| Site | Artifact | Why it is not a component |
|---|---|---|
| `emit::artifact_title` | `packtest-datapack/**` test `#>` descriptions | A PackTest source is a generated test, read by the validation server and by a maintainer — never rendered to a player. |
| `emit::emit_packtest` (dialogue-mask tests) | `packtest-datapack/**/dlg_mask_<npc>_<node>.mcfunction` | Same: the node id appears in the test's own description line. |
| `combat::actor_json` (an actor's `name`) | `validation/combat-plan.json` | The validation ladder's own artifact, read by the bot and by a maintainer. It reads the English through `plain`. |
| `render_plan::npc_name` / `area_name_of` / `first_clause` / the NPC shot `expect` | `render-plan.json` | The reviewer/vision artifact. Its `expect` prose is read by a vision model against a rendered frame, in English, regardless of what the delve ships. |

`delvec l10n-inventory`, `validate`, `analyze`, `snapshot` and `edit` never see a
tag at all: tagging happens inside `build`, after validation and analysis, so
every other subcommand reads the campaign exactly as authored.

`critical-path.json`, `validation/*.json`, `combat-plan.json` and `manifest.json`
carry **ids**, never authored prose, so they need no exclusion — the bot contract
is language-neutral. A generated PackTest may still *write* a text
component (`collect_container.mcfunction` pre-loads the stack the objective
counts): that is emitted by the same helper the datapack uses, so the two cannot
drift, and it is an input to the test rather than an assertion about rendered text.
No generated PackTest asserts on rendered text at all.

---

## 3. Verb → emission mapping

Mechanism level (not full mcfunction). See `crates/delvec/src/compiler/emit/`, one file per object.

| Verb / effect | Emitted mechanism |
|---------------|-------------------|
| `talk-to` / dialog option | `minecraft:villager` body (NoAI/Invulnerable/Silent) + co-located `minecraft:interaction` (tag `dw_npc_<n>`); click advancement + `/trigger dw.dlg_<n>` both feed one per-tick option handler. |
| dialog display gating | A node with any display-gated option (`requires_flags`, `forbids_flags` and/or `completes`) emits one `__m<mask>` variant per per-node availability bitmask + a chooser: `dmask_<n>_<node>` sets `dw.dmask` (bit `i` = the node's i-th gated option is displayable — its required flags set, no forbidden flag set (`unless score #party dw.f_<flag> matches 1`, unset-safe), and every completed objective active: `if …qa_<q>==1 unless …o_<obj>==1`), then `show_<n>_<node>` `dialog show`s the matching variant. Ungated nodes/options `dialog show` directly. Click handler keeps its own guard (defense-in-depth for the `/trigger` path). Generated PackTest: `dlg_mask_<npc>_<node>`, **one per gated node** (`DW0811`'s `dialogue-mask` claim, one claim per NPC). Each drives EVERY gated option of its node through that option's own full display condition, asserts the bit shown, then breaks each term of the condition on its own and asserts the bit gone (numeric terms are driven jointly per datum: the option's own `requires_state` and every completed objective's are satisfied by one value meeting all of them, and a break moves one term while holding the rest where a value can) — so the axis is never chosen and every gate the DSL has (`requires_flags`, `forbids_flags`, `requires_state`, `completes`) is covered by construction. The assertion is always on the option's **isolated** bit (`(dmask>>bit)&1` via `%= 2^(bit+1)` then `/= 2^bit`), never the whole `dw.dmask` — sibling options in a node can share a `qa_<q>` score, so a whole-mask compare would read a sibling's bit as this option's. |
| dialogue trigger re-arm | `dlg_<npc>_<n>` consumes the trigger with `scoreboard players reset @s dw.dlg_<npc>` — which also **re-locks** it — and therefore re-arms it in the very next line (`scoreboard players enable @s dw.dlg_<npc>`), before the flag gate's `return fail` and before any `dialog show`. The per-tick `scoreboard players enable @a` stays as belt-and-braces but cannot close the window on its own: 1.21.9+ **freezes the integrated (singleplayer) server while a screen is open**, and the handler's last act is to show the next node, so ticking stops with the trigger locked and the player's next click is executed the instant ticking resumes — before the tick function — and vanilla rejects it ("You can't trigger this objective yet"), silently swallowing one dialogue choice. A dedicated server never pauses, so no rung of the validation ladder can reproduce it. The generated `dialogue_trigger_rearm` PackTest drives a terminal option and uses the trigger twice **with the tick function never run in between** — that suppression is the freeze. |
| cast-ledger dispatch (spec-0020) | **The declaration is the gate.** `talk_<npc>` keeps its `advancement revoke @s only <ns>:<npc>_interact` (the interaction record is written by the click and consumed here), then calls `cast_<npc>` and dispatches on the per-player `dw.cast` selector: `execute if score @s dw.cast matches <i> run <action>`. `cast_<npc>` is pure scoreboard math — `set @s dw.cast 0`, then one `execute if score #party dw.qa_<quest> matches 1 [if/unless score #party dw.f_<flag> matches 1 …] run scoreboard players set @s dw.cast <scene>` per **declared placement**, in quest-DAG order then declaration order. A per-branch cast contributes one clause per branch carrying that branch's `requires_flags`/`forbids_flags`, so a branch-divergent NPC genuinely dispatches per branch rather than collapsing to its first placement; later clauses override earlier ones, so a per-branch entry lists its fallback first. Branch-gate flags are added to the setup objective declarations (`declared_flags`), since a *read* of an undeclared objective is a runtime command error and — unlike a `set-flag` write — nothing else guarantees the declaration. Because `dw.qa_<quest>` is set when a quest *begins* and is never cleared, the latest-begun beat wins and keeps winning: that is the whole retirement mechanism — once the escape beat opens, the premise root is unreachable because the ledger says so, not because an author remembered a flag. Scene `0` (no declaring quest begun) shows the stage-6 tree `root`. Actions: a root → the ordinary `show_node_cmd` (direct `dialog show` or the `dmask`/`show_` chooser); a bark pool → `function <ns>:bark_<npc>_<scene>`; `"none"` → **no clause at all** (the record is still consumed one line above, and nothing opens); `"unchanged"` → no new scene, the selector simply keeps pointing at the carried-forward one. Splitting the selector out of `talk_` mirrors `dmask_`/`show_` and for the same reason: a PackTest can drive it and assert which scene the ledger chose without opening a dialog a dummy player has no client for. `dw.cast` is declared only when some quest casts an NPC, and an NPC no quest casts emits the single root line. Generated PackTests: one `cast_ladder_<npc>` per ledger NPC — scene 0 asserted under the empty story, then EVERY clause asserted under a solved distinguishing drive (its own gate satisfied, every later same-quest clause's gate violated, all earlier quests still active — the retirement mechanism proven per clause), then every term of the clause's gate broken one at a time with the expected fallback scene computed by the compiler-side ladder model (`cast::eval_ladder`); plus `cast_bark_cycle` (every pool of every NPC) and `cast_none_silent` (every `"none"` scene: record written, scene selected, advancement re-armed). The walk is registered as `watch::Claim`s (`cast-ladder` over `cast_`, `cast-bark` over each NPC's `bark_<npc>_`), so a suite that drives fewer bodies than the authored ledger declares is a `DW0811` refusal. **The same resolution answers "where is this body?"**: `cast::station` walks those clauses in the same order under a playthrough's flag state and yields the governing placement's `at` — which is how a `talk-to` critical-path step gets its position (see `critical-path.json` in §5) and how `DW0483` decides a branch's placement. One model, so the cell the ladder walks to and the scene the datapack shows can never disagree. **Templates that assert dispatch pin EVERYTHING the ladder reads**: the batch is one shared server and three sibling verb templates legitimately end with a campaign flag set to 1, so any term left undriven is decided by batch order (expected `dw.cast 2`, got 3). Every dispatch drive is preceded by a pin of every quest-active score, every flag and every datum any clause of that NPC reads — and the values are not "requires → 1, all else → 0" but the **solved** assignment from `cast::distinguishing_drive`, because two clauses of one quest can differ only by `requires_state` and a flag pin cannot separate them: the drive satisfies the asserted clause's gate and provably violates every later same-quest clause's, or the clause is refused as dead (`DW0846`) before any template is written. This is what makes the per-clause assert a proof rather than a template that can name the wrong scene. |
| cast bark pool | `bark_<npc>_<scene>` advances `#bk_<npc>_<scene>` on the shared `dw.sys` objective by 1, wraps it with `matches <n+1>.. → set 1`, then `execute if score … matches <i> run tellraw @s [{name},": ",{line, italic}]`. An explicit clause ladder, never `%=` and never RNG (ADR-0006): the n-th right-click always yields the same line. Bark text is baked localized at emit time like every other player-visible string. |
| class select | Dialog button → `/trigger dw.class set <n>`, dispatched per tick to `class_apply_<c>` (kit, `dw.classed`, campaign-start party arming, teleport to the entry point). **One-shot per player**: the trigger is re-armed each tick only for a player who has not classed (`class_arm`), and the dispatch carries the same `unless score @s dw.classed matches 1` guard — see "The class trigger is ONE-SHOT per player" in §4 Hard invariants. Generated PackTests: `class_trigger_once` for the seal (a property of the trigger, not of any class), plus `class_apply_<c>` **per declared class** for that class's own kit, worn tag and entry warp (`DW0811`'s `class-apply` claim). |
| `reach-anchor` | Per-tick `execute if entity @s[…]` over the completion volume `reach::reach_completion` returns — a box of half-extent `max(1, radius)` at the anchor cell, formatted from that value rather than restated here; glowing `end_rod` `item_display` marker (tag `dw_r_<obj>`) **when the objective is marked** (spec-0093: its `marker`, else `guidance.markers`, is `shown`), labeled with the objective `title` — an **untitled** objective gets a nameless glowing marker, never a raw-id label; an unmarked one summons nothing and adjudicates the same volume. Completion despawns the marker (`kill @e[tag=dw_r_<obj>]`). |
| `kill` / `spawn-wave` | `spawn-wave` summons mobs (AI on) tag `dw_wave_<id>`, countdown `#<id> dw.wave`; `kill` completes at 0. **The countdown is a measurement of what still stands, not a tally of kills.** `tick` recomputes it for every spawned wave, ahead of every gate that reads it: `execute store result score #wlive dw.sys if entity @e[tag=dw_wave_<id>,nbt=!{Health:0.0f}]` then `execute if score #<id> dw.wave matches 1.. run scoreboard players operation #<id> dw.wave = #wlive dw.sys`. Three parts carry the rule. The `Health` filter separates a standing body from one still playing its death animation, so the clear lands on the tick of the last death rather than twenty ticks later. The `matches 1..` guard means the line only ever CORRECTS a countdown a spawn has opened — an unspawned wave has no score at all and must not acquire one, or its `kill` objective would complete on tick one, and a `respawns_on_rest` re-seat writes its fresh total before this next reads it. And the scratch holder is shared across waves because `tick` is one atomic function call, the same argument `#wcen_*` makes. Vanilla has no trigger for “this entity died”, so the `player_killed_entity` advancement `k_<id>` and its reward `k_reward_<id>` can only ever see a CREDITED kill: they still decrement on the tick of the kill, and the recount is what makes a body that fell, burned, drowned, walked into a lethal volume or was cut down by another mob count the same as one the party felled. Generated PackTests `verb_kill` (the countdown drained through the reward) and `verb_kill_uncredited` (every body killed with no player credited, one tick, the objective complete) — the second never touches `k_reward_<id>`, which is what makes it able to fail. Armed species get `equipment` NBT (drop 0): `wither_skeleton→stone_sword`, `skeleton`/`stray→bow`, `pillager→crossbow`, `vindicator→iron_axe` (the pillager row is load-bearing, not cosmetic — see `waves[].lane`/`DW0384`). **Arming assertion (generated `verb_kill`)**: the test picks the wave's first mob with an **effective** main hand — the author's `equipment.main_hand` when given, the default table otherwise (`emit::effective_mainhand`, the same source the summon NBT reads) — and asserts that exact item via `execute if items entity … weapon.mainhand <item>`. Reading the override keeps a campaign that arms a vindicator with a `stone_axe` from failing a test that demands `iron_axe`, and *extends* coverage to authored weapons on species the table calls unarmed. **Mob placement:** each mob is seated on a distinct compiler-validated standable cell (2-tall clearance, solid floor) chosen by a deterministic BFS outward from the wave anchor over the assembled occupancy world (`compiler::nav`), ordered by ascending BFS distance with a fixed `(y,z,x)` tie-break. The flood-fill is confined to the anchor's own assembled piece, so a flock never crosses a socket seam into a neighbouring room. A wave needing more footing than its room offers is `DW0312` (never `+x`-strung mobs piling into blocks or spilling toward void). **spec-0016 §6 changes where, not how:** a `summon: aggro-edge` wave is seated on per-mob perception RINGS across the whole area instead (`DW0387`), and a `lane` wave additionally carries the patrol NBT and starts its `lane_tick_<wave>` clock at the end of its own `spawn_<wave>` (so a wave that never spawns never ticks, and a bonfire re-seat re-arms the clock through the same replace-mode `schedule`). **Census probe:** every wave also gets `wave_census_<wave>`, `wave_census_one_<wave>`, `wave_brand_<wave>` and `wave_unbrand_<wave>`. The census zeroes `#wcen_n`/`#wcen_b`/`#wcen_d`, bumps `#wcen_seq`, runs the per-mob function `as @e[tag=dw_wave_<id>]`, and states the totals on the anchored marker channel as `[dw:census <ns> <wave> <seq> <present> <branded> <damaged> <credited>]`, one `[dw:censusmob <ns> <wave> <seq> <x> <y> <z> <health> <max>]` per mob first (all ×100 fixed-point, so nothing crosses chat as a float). **`credited` is the only field about the FALLEN**, and it is what a run's attribution reads: `#wcred_<wave>` counts the wave's deaths a player was credited with since the seating in force — seeded to 0 in `setup`, set back to 0 by `spawn_<wave>` so a re-seat's own uncredited `kill @e[tag=…]` sweep is not charged to the next cohort, and incremented by `k_reward_<wave>`, which is where vanilla's only credit (`minecraft:player_killed_entity`) lands. `count - present - credited` is therefore how many of the wave the WORLD killed — a fall, a lethal volume, a trap, another mob — which the countdown deliberately does not distinguish. The `setup` seed is load-bearing: the census renders the holder as a `score` component, and a holder with no score renders as the empty string, producing a line no reader parses. The SEATING is deliberately not on the wire beside it — `spawn_<wave>` writes a compile-time constant that already reaches the harness as `combat-plan.json`'s `count`, and one fact by two routes is a pair that can disagree, not a second measurement. `damaged` compares `data get entity @s Health` against `attribute @s minecraft:max_health get` — vanilla's own primitives, so it is never a table the compiler refuses to invent (`DW0475`) and never a value the client happened to be sent (an unmodified max health is not on the wire at all). `wave_brand_<wave>` stamps `dw_brand_<wave>` on the wave's living mobs and the unbrand clears it, which is how the die-retry ladder names a survivor **by identity**: a re-summon cannot carry the stamp. Counting silhouettes instead — every entity the client tracks — would report another encounter's bodies as wave mobs a re-seat failed to remove. Generated PackTest `wave_census` proves the arithmetic live, including that a bystander of the wave's own species summoned on the wave's own anchor cell moves no count. **Muster probe:** beside the census, every wave gets `wave_muster_<wave>` plus one `wave_muster_one_<wave>_<i>` per declared entity kind, and the two staged functions `wave_strike_<wave>` (`execute as @e[tag=dw_wave_<id>,limit=1,sort=nearest] run damage @s 100000 minecraft:player_attack by @p`) and `wave_chip_<wave>` (one point of the same attributed damage to every body). The muster bumps `#wmus_seq`, counts every tagged body into `#wmus_all`, then runs the per-kind function `as @e[tag=dw_wave_<id>,type=<entity>]` and states `[dw:muster <ns> <wave> <seq> <counted> <tagged>]`; each body first states `[dw:musterbody <ns> <wave> <seq> <type> <mask> <max_health> <armor> <armor_toughness> <movement_speed> <attack_damage> <follow_range> <effective_attack_damage>]`, all ×1000 fixed-point. Every DECLARED attribute is read with `attribute @s <attr> base get 1000`, never the total: a summoned zombie carries vanilla's own random spawn bonus on `movement_speed` (0.23 base reads 0.276 total) and its weapon's `attack_damage` modifier (2.0 base reads 5.0 with a wooden sword), so comparing a declaration against a total is a check that fails on every correct body. `armor`/`armor_toughness` are the totals on purpose — they are not declared, and the question they answer is whether the declared gear reached the body. The identity half is a bitmask: one `execute if data entity @s {…}` per declared fact (a `CustomName` component, an `equipment.<slot>` item id), so a name and an item id are answered by the SERVER as bits and no player-visible text is ever parsed off chat. The plan's `muster` block names the facts in the same order (§ `combat-plan.json`). `damage … by @p` rather than `kill` is load-bearing: `on_kill`, the countdown's `k_reward_<id>` and a declared drop all pay on a PLAYER's kill, and a removal crediting nobody would skip every one of them and leave the step reading green over machinery that never ran. **Which waves get machinery (uniform emission):** all of it is gated on the wave resolving a spawn AREA, and that resolution (`plan::wave_area`) walks every effect root **deep**, through `QuestEffect::nested_effect_lists` — the same nesting authority emission itself walks — so a `spawn-wave` inside a `sequence` step, a `set-checkpoint` `on_respawn`, a `bonfire` `on_rest`, a `begin-stealth` `on_caught`, a `move-npc`/`move-actor` `on_arrive` or a trap `payload` registers exactly like a top-level one. `DW0497` proves no emitter ships a `function <ns>:spawn_…` call pointing at nothing. A wave declared in `waves[]` that nothing fires anywhere resolves an area only through the defensive `kill`-objective fallback, and otherwise emits nothing (`DW0171` owns the killed-but-never-spawned case, `DW0310` the spawned-but-unplaceable one). |
| `on_kill` (spec-0074) | **Wave:** `k_reward_<wave>` gains one line between the credited-kill ledger and the revoke — `function <ns>:on_kill_w_<wave>` for `every-kill`, `execute if score #kf_w_<wave> dw.sys matches ..<N−1> run function <ns>:on_kill_w_<wave>` otherwise (N = `wave_total`; an absent `fires`, admitted only where the fight never comes back, takes the guard). **Actor:** `advancement/ka_<actor>.json` (`minecraft:player_killed_entity` over `{Tags:["dw_actor_<actor>"]}`), `ka_reward_<actor>` (the call, guarded `..0` unless `every-kill`, then `advancement revoke @s only <ns>:ka_<actor>`), emitted only for an actor that declares a bundle and whose body resolves. **The bundle** `on_kill_w_<wave>` / `on_kill_a_<actor>`: `scoreboard players add #kf_<f> dw.sys 1` FIRST — so the ledger counts firings and nothing else — then the effects under `Audience::Solo`. **The ledger** `#kf_w_<wave>` / `#kf_a_<actor>` on `dw.sys` counts payments over the whole delve: seeded `0` by `setup` beside `#wcred_<wave>`, reset by no spawn, re-seat or rest (the census's `credited` counts credited kills per seating — two facts, two holders). **Ordering** within one credited kill: countdown decrement, credited ledger, the bundle, the re-arm; the `kill` objective completes on the next `tick`, so the last body's bundle fires before that objective's `on_objective_complete`. **A compiler removal never pays**: every one (`wave_reseat_<wave>`, `actor_restand_<actor>`, `unleash_<actor>`'s puppet swap, `despawn-actor` in either style) ends in `/kill` from the server source, which credits nobody. **Generated PackTests**, one set per fight that declares a bundle (`DW0811` claim `kill-pays` over `on_kill_w_`/`on_kill_a_`), all driven with the credited blow `execute as <body> run damage @s 1000 minecraft:player_attack by <dummy>` — measured on the pinned server to grant the dummy `player_killed_entity` (and `damage … minecraft:generic` with no `by` to grant nobody): `kill_pays_<f>` (one credited blow → the ledger is 1 and the bundle's first ungated `add-state` holder moved by its amount; the body then driven directly pays again), `kill_pays_uncredited_<f>` (every body dies uncredited, the real `tick` runs → the ledger is 0), `kill_pays_removed_<f>` (a rest's re-seat through the real `bonfire_rest_<i>` and, for an actor, both `despawn-actor` styles, each over branded standing bodies dragged onto the template's dummy; where the removal is unseen, at least one body must be waiting in the dummy's column at Y −128, and a kill of the `dw_unseen` bodies in that column in the same tick must leave none alive, so the ledger is read after the body's death under the world → every brand removed and the ledger 0), `kill_pays_across_rest_<f>` where a rest brings the fight back (kill, rest, kill again: a `respawns_on_rest` wave pays N for `first-kill` and 2N for `every-kill`; a fight re-seated only while it stands is left one body standing before the rest and pays N or 2N−1). |
| `collect` | Chest at anchor pre-loaded `count×item`; `inventory_changed` advancement runs guarded completion. **Adoption:** with a `container`, `activate_<obj>` emits **no `setblock`** and fills the prefab's own chest/barrel at the container anchor's cell instead — `item replace block <x> <y> <z> container.<slot> with <item>[custom_name=…] <count>`, slot `0` the required stack and slots `1..=fill_count` the padding that makes it read full. The component suffix is rendered by the same helper `loot` uses (`emit::container_stack_components`), so a named quest item and a named loot stack cannot drift apart. Fill time is **activation**, not world-init: a late objective's items are not lootable from minute one, and an item pocketed before activation still completes it via the per-tick held check. Generated PackTest `collect_container` (only when some collect adopts): clear the adopted slots, run the objective's own `activate_<obj>`, assert the filled item count across the container (`if items block … container.* <item>` = `count × (fill_count+1)` — a dropped fill reads 0, padding that overwrote slot 0 reads one stack short), then put the **named** stack in the player's inventory and tick, asserting completion. That last phase is the point: it proves on a live server that a `custom_name` component does not change what the adjudication sees. |
| `interact` | `minecraft:interaction` (tag `dw_i_<obj>`) + `player_interacted_with_entity` advancement + `/trigger dw.i_<obj>` — or, for a prop vanilla reports the use of (spec-0093 §6.5), no hitbox and a `default_block_use` advancement at the block's cell feeding the same trigger. **`requires_item` = `execute … if items entity @s weapon.mainhand <item>` — HELD, not possessed**; a campaign that declares none is untouched. Optional `missing_item_hint` adds ONE line to `tick`: `execute as @a[scores={dw.i_<obj>=1..}]<same activation guard> unless items entity @s weapon.mainhand <item> run tellraw @s {"text":…}` — placed between the completion line and the trigger reset, so it rides the existing two-phase click handling (advancement reward sets the trigger, `tick` reads it and resets it) and one click narrates once. Guarded identically to the completion line, so a not-yet-active or already-finished objective answers a stray click with silence. Generated `verb_interact_held` PackTest proves the semantics live in two phases on one dummy — item in `inventory.0` with an empty hand must NOT complete (and asserts, via `if items entity @s container.*`, that the item really is carried, so the phase is not vacuous), then the same item in `weapon.mainhand` completes; the `tellraw` itself is asserted in Rust because a chat line leaves no game state for PackTest to look at. `packtest_preamble` therefore places a `requires_item` in `weapon.mainhand` rather than `give`-ing it. Glowing lantern `item_display` marker (also tag `dw_i_<obj>`, only when no `prop` **and the objective is marked** — spec-0093: its `marker`, else `guidance.markers`, is `shown`; the hitbox is summoned either way), labeled with the objective `title` — untitled → nameless glow, never a raw-id label. `prop{block}` = `setblock` affordance. Completion despawns both entities (`kill @e[tag=dw_i_<obj>]`) so a finished objective is not clickable; the `prop` block persists as scenery. **Arming before adjudication.** The completion line is gated on `#party dw.qa_<quest>` and the very next line resets the trigger with NO guard at all, so a click is spent whether or not it landed. That pair is only safe because `tick`'s completion loop visits quests in **arming order** (`emit::quests_in_arming_order`, a stable topological sort over the `quest-complete` edges): the completion loop is the one place a quest is armed — a completion line runs `complete_<obj>` → `check_q_<q>` → `complete_q_<q>`, which writes `dw.qa_<next>` — so a quest's lines must precede the lines of any quest it arms, or a click already pending when its quest arms is adjudicated against an unarmed quest and then thrown away. Nothing in the DSL orders quest declarations, so the order is the sort's, never the JSON array's. The sort is stable, so a campaign already declared in arming order keeps its declaration order. The unconditional reset is deliberate and stays: a trigger fired long before arming is DISCARDED, never banked — a banked click would auto-complete the objective the moment the quest armed, with nobody having clicked. Losing input is a bug; fabricating it is worse. Pinned by `tests/tick_arming.rs` (the invariant over every fixture, plus a campaign deliberately declared out of order) and by the generated `verb_interact_arming` PackTest (premature click → no completion and no banked score; arming alone → still nothing; a real click after arming → completes). |
| stage-5 `loot[]` (spec-0021) | `setup_finish` emits one `item replace block <x> <y> <z> container.<slot> with <item>[components] <count>` per declared stack, slot = declaration index. `components` carries `custom_name` (localized) and `enchantments` when present. The container itself is never emitted — it is prefab furniture, proven present by `DW0431`. A campaign with no `loot` emits nothing here and stays byte-identical. |
| environment `triggers[]` | `setup_finish` gives each `strike`/`use` trigger a body at its `at` anchor (tag `dw_trig_<id>`); `approach` needs no entity. **The body is the shape of the object at that anchor, not a point.** `compiler::pressable::body_at` is the single authority and both this emitter and `compiler::eclipse` read it, so the two can never disagree about whether a body exists. Three outcomes: where a compiler-owned interaction set already covers the anchor — a `close-gate` seal, a sealed shortcut door — the trigger **rides** it and summons nothing (one cell, one hitbox; a second co-located box is the `DW0422` ray-pick tie); where the anchor names a **gate region**, one `1.02f` box is summoned per clickable **shell** cell of that region, exactly as a `close-gate` seal has always done; where it names a point in open space, the ordinary `1.0f x 2.0f` box, unchanged and byte-identical. **Why the region form exists:** a point body at a region anchor lands *inside* the solid block. Measured on the `souls-shortcut` fixture, a `use` trigger on the shortcut's gate emitted one body with AABB `[4,65,6]..[5,67,7]` inside a doorway slab occupying `[4,65,6]..[6,68,7]` — flush with the block on the faces it touched and interior on the rest. Vanilla bounds its entity raycast by the block hit and takes the entity only when it is *strictly* nearer, so that trigger was pressable from **no angle at all**, and it compiled with zero diagnostics; a doorway is also six cells, of which a point body covers one. The region form is the machinery a `close-gate` seal uses, reachable by every trigger. A trigger whose anchor resolves to nothing at all is `DW0426`.  `tick`: `strike` fires on `nbt={attack:{}}`, `use` on `nbt={interaction:{}}`; `approach` is a `distance=..<range>` selector; `step` needs no entity and is a player in the cell — `@a[x,dx=0,y,dy=0,z,dz=0,tag=!dw_cutscene]` (`emit::step_cell_terms`, the one selector a plate or tripwire trap's detection also uses). A party `step` is edge-latched on `#stp_<id> dw.sys`: `execute <once/forbid> unless score #stp_<id> dw.sys matches 1 if entity @a[<cell>] <flags>run function <ns>:step_<id>` (the gate fragments `trigger_poll_guards` writes, each space-terminated, the one authority every trigger dispatch reads), and `execute unless entity @a[<cell>] run scoreboard players set #stp_<id> dw.sys 0`; `step_<id>` sets the latch and calls `trig_<id>`, so a plate stood on fires once and fires again only after it is stepped off and on. A presser `step` is `execute as @a[<cell>,tag=!dw_stp_<id>] run function <ns>:step_<id>` and `execute as @a[tag=dw_stp_<id>] unless entity @s[<cell box>] run tag @s remove dw_stp_<id>`; `step_<id>` tags `@s` and dispatches `trig_<id>` behind the trigger's own gate (`once`, forbidden flags, required flags and state, spelled by `trigger_poll_guards`), so every player who steps on is dispatched once per step, as `@s`. A `step` writes no advancement and summons no hitbox; its `env_trigger_<id>` template also drives `step_<id>` and asserts the latch, and `DW0811`'s `step-trigger` claim covers every `step_<id>`. **The click block is two phases, not one:** every click trigger's fire clause first, in declaration order, then every clear clause (`data remove entity @s <field>`). Emitting the pair inline per trigger is only sound while at most one trigger reads a given interaction entity, and several `strike-npc` triggers legitimately ride ONE NPC hitbox — e.g. a giant carrying `wake-the-giant` (requires `flag/asleep`) and `his-house` (requires `flag/sealed`, forbids `flag/asleep`) on the same entity. Inline removal would make the FIRST-DECLARED trigger consume the click even with its own gate shut: a suppressed trigger would starve its siblings and declaration order would decide which of two legal triggers worked. Two phases make it order-independent — every trigger sharing a hitbox is offered the same click and fires exactly when its own gate says so — while consumption is unchanged (the record is gone by the end of the same `tick` pass, so a held click still fires once). `once` guards on `#trig_<id> dw.sys`, which **every** trigger writes on firing (not only `once` ones): the write is what makes dispatch observable at all — a trigger that never fires is otherwise invisible to every automated check — and it is what the generated `v06_shared_hitbox` template reads. One added line per non-`once` trigger function. **Generated `v06_shared_hitbox`:** emitted for a campaign that has two click triggers on one NPC hitbox whose flags can tell them apart; it proves the hitbox really is shared, then writes the vanilla `attack` compound and runs the real `tick` twice — once with the later trigger's gate open and the earlier one's shut (the starvation case: the later one must fire, the earlier must stay silent, the record must still be consumed), once with the earlier one's gate open (so both are reachable). Players are shielded with Resistance V across each pass because a real `tick` runs real effects and a delve's effects include `damage-players`; flags, actors and NPCs are handed back untouched (batch model). **`strike-npc` — the body IS the target:** `on: {on:"strike-npc", npc}` has **no anchor**. Its tag rides the interaction hitbox the named NPC already owns and `setup_finish` summons nothing for it, so it works wherever that NPC stands and whatever body it wears. This is the form that can express "hit the giant": a place-based `strike` summons its own entity at a *cell*, and a large NPC's body eclipses that cell (`DW0359`), so the click never reaches it. Right- and left-click stay separate all the way down because a `minecraft:interaction` records them in **two distinct NBT fields**: the dialogue advancement takes the right-click (`interaction`), the trigger takes the left-click (`attack`), and neither consumes the other's record. That separability is machine-proven, not assumed — the generated `v04_strike_npc` PackTest writes a right-click record on the shared hitbox, ticks, and asserts no `attack` record appeared and the trigger did not fire. **Strike on an NPC's anchor — one cell, one hitbox:** the place-based spelling of the same mechanism — when a `strike` trigger's `at` is also where an NPC stands, the NPC's own interaction hitbox carries `dw_trig_<id>` **and is the trigger's sole entity** — `setup_finish` suppresses the trigger's own summon. The NPC's body is `Invulnerable`, so without the shared tag a swing could land where nothing was watching and the trigger never fire; and with a *second*, exactly co-located hitbox the client's entity ray-pick is ambiguous — an exact tie resolves to whichever entity iterates first, in practice the world-init summon — so a right-click lands on an entity without `dw_npc_<n>` and the dialogue advancement never fires (proven on a live server). Consequences: the trigger's lifecycle follows the NPC's — a `deferred` NPC's strike trigger is armed only after its `spawn-npc` entrance, a `move-npc`'d NPC carries the strike target with it, and `despawn-npc` removes it entirely (which is the trigger's meaning: the thing being struck is the NPC). Scoped to left-clicks: right-click on an NPC already belongs to the dialogue advancement, so a co-located `use` trigger is rejected at validate time (`DW0350`) and again at build time (`DW0359`). Generated PackTests: `v04_strike_npc` writes the vanilla `attack` compound onto the NPC's hitbox and asserts the trigger fires, once, with the record consumed; `v04_strike_talk` pins the single-hitbox invariant — exactly one interaction entity wears the trigger tag, none wears it without the NPC tag, before and after an attack record is consumed (attack-then-talk must stay clickable); and **`env_trigger_<id>`, one per declared trigger** (`DW0811`'s `env-trigger` claim), which opens that trigger's own gate through `packtest_gate_drive`, calls `trig_<id>` the way that trigger's own dispatch route calls it — with no executor for a `party` bundle, `as` the test's dummy for a `presser` one — and asserts the `#trig_<id> dw.sys` marker every trigger writes on firing. |
| assembly body (spec-0082) | `asm_spawn_<s>` runs `asm_summon_<s>` unless a root tagged `dw_asm_<s>_root` stands: `kill @e[tag=dw_asm_<s>]` (residue), one `minecraft:item_display` root with no item at the mark cell's centre on its floor, one `minecraft:block_display` per rig part at the same point with the spawn pose's transform turned by `facing` and the initial clip's cadence as `interpolation_duration`, each followed by `ride <part> mount <root>` — **a star**: a `tp` of the root carries every passenger with it and turns each by the root's own change of yaw, and a passenger that is itself teleported leaves its vehicle (spec-0082 §8 row 3), so no part depends on another's seat — then the hitbox `minecraft:interaction` (`width`, `height`, `response:1b`, tag `dw_asm_<s>_hit`) when declared, and the state on `dw.sys`: `#asm_<s>_live 1`, `_sm 0`, `_step 0`, `_base` = the initial clip's index (`-1` for none), then `asm_resume_<s>`. Every entity is tagged `dw_fixture`, so a region `teleport` never carries the assembly away. `asm_despawn_<s>`: `kill @e[tag=dw_asm_<s>]`, `#asm_<s>_live 0`, `_sm 0`. |
| assembly clips (spec-0082) | Clips are numbered in name order. One function per frame, `asm_frame_<s>_<clip>_<frame>`: per part, `execute as @e[tag=dw_asm_<s>_p<i>,limit=1] run data merge entity @s {start_interpolation:0,interpolation_duration:<cadence>,transformation:{…}}`, the transform turned by `facing` (translation rotated about the mark, the yaw quaternion composed before the left rotation) and written to four decimals. `asm_play_<s>_<k>` sets `_clip k`, `_f -1`, `_t <cadence-1>`, `_tpf <cadence>`, `_done 0`, so the first frame is applied on the next tick. `asm_tick_<s>` (run from `tick` while `_live` is 1) counts `_t` up and, when it reaches `_tpf` on a clip that is not done, runs `asm_adv_<s>`: `_f` advances, a looping clip wraps, a non-looping clip sets `_done` on its last frame and holds it; the clip and frame are stored in `storage dw:asm <s>` and `asm_apply_<s>` dispatches one macro line, `$function <ns>:asm_frame_<s>_$(c)_$(f)`. `asm_cue_<s>_<k>` — what `play-clip` calls — sets `_base k` and plays it only while `_live` is 1 and `_sm` is 0; `asm_resume_<s>` plays `_base`, or `asm_rest_<s>` (the rest pose, `_clip -1`) for none. |
| assembly strikes (spec-0082) | Appended to `asm_tick_<s>` after the frame driver, for an assembly with a pattern: while `_sm` is 0, nobody in `while_in` (`@a[<box>,tag=!dw_cutscene]`) resets `_step` to 0 and somebody in it runs `asm_begin_<s>` (`_sm 1`, plays the step's `windup`); `_sm 1` with the clip done runs `asm_hold_<s>` (`_sm 2`, `_hold 0`, `_holdn` = the step's `hold`); `_sm 2` counts `_hold` and at `_holdn` runs `asm_swing_<s>` (`_sm 3`, plays the step's `strike`); `_sm 3` with the clip done runs `asm_land_<s>`: `asm_land_<s>_<j>` (the step's `on_land`, through the ordinary effect emitter with no acting player — a `damage-players{in}` is `execute as @a[<box>,tag=!dw_cutscene] run damage @s <amount> <type>`), `_lands +1`, `_step` advanced and wrapped, `_sm 0`, `asm_resume_<s>`. Measured on the spike's own shape (spec-0082 §8 row 6): wind-up begin to hold 36 ticks for 8 frames at cadence 5, the landing on the tick the strike's last frame is applied. |
| `strike-assembly` (spec-0082) | No body of its own and no tag added: the poll and the clear in `tick` read the assembly's hitbox, `execute … if entity @e[tag=dw_asm_<s>_hit,nbt={attack:{}}] … run function <ns>:trig_<id>` then, after every trigger has been offered the record, `execute as @e[tag=dw_asm_<s>_hit] run data remove entity @s attack` — once per hitbox however many triggers ride it (`emit::trigger_carrier_tag`). A critical path performs a counting trigger as often as its gate needs — the least count for which the datum's initial plus that many of the bundle's ungated `add-state` amounts opens the way-opening or flag-paying effect, 1 otherwise — each a `trigger` step at the assembly's mark carrying `assembly`, after the beat that spawns it; the harness performs each as a real attack and waits for a fresh marker per performance. Generated PackTests, each one atomic mcfunction: `asm_spawn_<s>` (the root carries one passenger per rig part, one hitbox stands, and despawn leaves no entity tagged `dw_asm_<s>`); `asm_land_<s>` (from `_sm 3`, `asm_land_<s>` counts the landing in `#asm_<s>_lands` and returns `_sm` and `_step` to 0 — the machine only: a PackTest dummy is undamageable, so the blow's damage is not witnessed here); and one `asm_hits_<s>_<trigger>` per `strike-assembly` trigger whose bundle has an ungated `add-state` on a party datum, which merges an `attack` record onto the hitbox and runs the trigger's own poll clause and clear exactly as `tick` spells them (`emit::click_trigger_poll`, the one authority for both) — never the whole `tick`, whose other gates read the whole progression ledger, and a sibling that zeroes that ledger inline runs between the branch-shape campaign template's phases. It writes the trigger's own gate (`DW0807`), asserts the datum moved by the amount, and, where a `play-clip` waits on the datum at a threshold, asserts the clip the count plays. |
| `set-flag` / `requires_flags` / `forbids_flags` | `dw.f_<flag>` scoreboard, written and read **only on the party holder** `#party` (spec-0018); required flags AND-ed into objective guards (layered on `after`) as `if score #party dw.f_<flag> matches 1`, forbidden flags joined as `unless score #party dw.f_<flag> matches 1` clauses in the same guard. **Per-effect** gates wrap each of the effect's emitted commands in `execute if score #party dw.f_<flag> matches 1 [… per required] unless score #party dw.f_<flag> matches 1 [… per forbidden] run <cmd>`, and an ungated effect is emitted verbatim (byte-identical). A flag read never names a player (`@s`, or a `scores={dw.f_…}` selector): nothing writes a flag there, so such a read matches nobody; `crates/delvec/tests/v06_traps.rs` scans the emitted tree for the selector form. `unless … matches 1` is the deliberate unset-safe spelling: flag scores are never pre-initialized to 0, so a `scores={…=..0}` selector would not match an unset score. That is one instance of a rule the compiler enforces over the whole emitted tree (`DW0495`): a missing entry is not zero, it is false to every question, so a comparison either reads an entry the pack creates or is spelled so the absence cannot change its answer. **Trigger-level** `forbids_flags`: the fire condition gains `unless score #party dw.f_<flag> matches 1` per flag (flags are campaign state, so one player's wake beat stands the trigger down for everyone); a suppressed strike/use still consumes the interaction record. Generated PackTests: `verb_flag_gate` (requires) and `verb_forbid_gate` (forbids: set flag → drive → assert NOT complete; clear → drive → assert complete). |
| `open-gate` | `/fill … air replace <block>` over the gate region — one **region write** (`emit::fill_region_command`, shared with `close-gate`, `fill-region` and `clear-region`), `replace`-filtered to the block the anchor declares so it removes the gate and nothing else that has drifted into the box. **Plus `kill @e[tag=dw_seal_<anchor>]`** when the campaign ever seals that anchor: the seal's answer comes down with the seal. An opened threshold that still says "the way is sealed" is a lie, and an invisible box left standing in a doorway swallows right-clicks aimed through it. |
| `close-gate` | The same **region write** with the anchor's declared fill block and no `replace` clause — `/fill <region> <block>` (the dual of `open-gate`), **plus `execute unless entity @e[tag=dw_seal_<anchor>] run function <ns>:seal_arm_<anchor>`** (a sealed boulder answers a right-click instead of standing there in silence). See "Press answers — what a pressable thing says back" below. |
| `fill-region` / `clear-region` | The **general** region write: one `fill <lo> <hi> <block>` over `Plan::zone_box(region)`, with `minecraft:air` and no `replace` filter for the clear. Same builder as the two gate verbs above; the only difference between all four is where the box comes from and whether the fill carries a filter. An unresolvable `region/anchor` emits nothing — that is `DW0142`/`DW0360`, not a silently mis-aimed fill here. |
| `give-item` | Grants item to player: `give <target> <item>[custom_name=…,enchantments={…}] <count>`, the component suffix rendered by the ONE renderer a `loot` fill uses (`container_stack_components`), so a given stack and a container stack of one item agree; a plain, unnamed, unenchanted give emits the bare line. **An enchanted book stores**: the component is the item's, by vanilla's rule (`EnchantmentHelper.getComponentType`), exposed as `delvewright_dsl::enchantment_component` — `minecraft:stored_enchantments` on `minecraft:enchanted_book` (what an anvil applies), `minecraft:enchantments` on everything else; both are in the pinned 1.21.11 `data_component_type` registry. Every surface that writes an enchanted stack asks it: a `give-item`, a `loot` fill, an equipped piece. |
| `narrate` | chat / `title` / `subtitle` (+ optional sound); `art` = `title` with a `{"font":"delve:art"}` text component, rendered uppercase in the pixel-banner font (6 font px/glyph → ~15 glyphs fit; see [The `delve:art` font](delvec/compiler/atmos.md#the-delveart-font)); `actionbar` = `title <who> actionbar <component>`. |
| `play-sound` | `playsound <sound> master @s [<pos>] [<vol> [<pitch>]]` — effects run `as @a`, so `@s` is each player: `anchor` uses the resolved anchor pos (all hear it there), `players` uses `~ ~ ~`. A `players` sound with a non-zero `offset` is `execute as <who> at @s rotated ~ 0 positioned ^<x> ^<y> ^<z> run playsound <sound> master @s ~ ~ ~ [<vol> [<pitch>]]` — the listener inside the loop is `@s`, never the audience selector, so each listener hears it once, at their own behind. |
| `firework` | `summon minecraft:firework_rocket <x>.5 <y> <z>.5 {LifeTime:<L>,FireworksItem:{id:"minecraft:firework_rocket",count:1,components:{"minecraft:fireworks":{flight_duration:<f>b,explosions:[…]}}}}` at the mark's cell centre. **`LifeTime` is written, never left to the game**: unset, vanilla randomises it at launch (`(flight + 1) × 10 + random(0..5) + random(0..6)` ticks [cited — *Firework Rocket*, entity data]), so two runs of one datapack would burst at two heights and nothing could be proven about where the burst is. The emitter writes `L = (flight + 1) × 10` — 20, 30, 40 — the floor of that range, which is deterministic (ADR-0006) and the conservative side of `DW0899`. The entity's item field is `FireworksItem` [cited — *Firework Rocket*]; colours are the packed integers the component reads (`#ffd700` → `16766720`), through `dsl::color`, which the potion bottle's `custom_color` also reads; an undeclared `trail`/`twinkle` is absent rather than false. Every spelling comes from `dsl::firework`, the one file the game facts are pinned in. |
| `particle` | At `players`: `execute as <who> at @s run particle <id> ~ ~ ~ <dx> <dy> <dz> <speed> <count> force @s` — spawned at, and shown to, each addressed player. At a mark: `particle <id> <x>.5 <y> <z>.5 <dx> <dy> <dz> <speed> <count> force <who>` at the cell's horizontal centre on the mark's plane. `force` is written, never chosen: `normal` sends 32 blocks and may be dropped at the client's Minimal particle setting; `force` sends 512 and is always drawn [cited — Minecraft Wiki, *Commands/particle*]. `<who>` is the effect's audience, narrowed by its `in` box. |
| `lightning` | `summon minecraft:lightning_bolt <x>.5 <y> <z>.5` at the mark's cell centre on its plane. Setup holds the strike's chunk loaded for the session (`forceload add`, beside an area's claim): a `summon` into a chunk nothing loads is refused by the server, and the beat would ship with no bolt. A generated `lightning_<n>` PackTest first waits for the strike's chunk — setup force-loads it on the template's own first tick, which loads nothing yet, so a scheduled probe (`lightning_wait_<n>`, in the PackTest datapack) sets `#lbl<n>` once `execute if loaded` holds at the cell and the template awaits it — then runs the beat's own line and asserts a `minecraft:lightning_bolt` within a block of the mark on the same tick. |
| `damage-players` | `execute as <audience>[tag=!dw_cutscene] run damage @s <amount> <type>` for a party beat (`@a`), `execute if entity @s[…] run damage @s …` inside a solo `on_caught`/`on_respawn` (default type `minecraft:generic`). With `in`, the stealth-zone box (`x=…,dx=2·ext,…`) joins the same selector, so each player is judged on their own position — no double-hit. `/damage` takes a single entity, so the party form re-binds rather than widening the target (§1, single-entity arity). A generated `v06_damage` PackTest summons a tagged dummy, applies the declared amount+type, and asserts its `Health` strictly dropped. |
| `set-block` | `setblock` at resolved anchor. |
| `despawn-npc` | `function <ns>:despawn_npc_<id>`: body + interaction hitbox leave unseen (§4 "A body the story removes is never seen to die"). One generated `v04_despawn_<id>` PackTest per NPC a `despawn-npc` names (each function is that NPC's own body, `DW0810`); when the NPC is **deferred** it runs its `spawn_npc_<id>` entrance right after `setup_finish`. It re-issues the NPC's own body summon with a fixed UUID, asserts 2 entities before and 0 after the real `despawn_npc_<id>`, then reads the body by UUID through its death (a selector never matches a dying body): not dying on the removal's tick, nor `UNSEEN_DELAY_TICKS - 1` ticks later, and at Y ≤ -127 on the first tick it is. |
| `spawn-npc` | `function <ns>:spawn_npc_<npc>` — the generated entrance function, emitted once per NPC any `spawn-npc` names (deferred or not). Its two lines are the world-init summons, each independently guarded: body by `unless entity @e[tag=dw_npc,tag=dw_npc_<n>]`, hitbox by `unless entity @e[tag=dw_npc_<n>,tag=!dw_npc]` (both carry the id tag, so a single shared guard would let the body's own summon suppress the hitbox). The `npc_summons` PackTest fires each deferred NPC's entrance after `setup_finish` and asserts exactly one body. |
| `move-npc` | Per-tick tp along A*-planned walkable waypoints (hitbox in lockstep), at cell **centres** with L-shaped vertical steps — see §4 "Entity placement". The route is **string-pulled** before it is resampled, so a body crosses open level ground in a straight line instead of tracing the four-connected grid; every rise, drop, stair edge and use-gate ends a straight run and is still rendered as a cardinal step — see §4 "A walked path goes straight where the ground allows". Every `tp` carries `<yaw> 0` — the **exact bearing of the segment that tick walks**; see §4 "A walked body faces where it is walking". **The ARRIVAL tick is the exception, and deliberately** — the walked body's arrival turn, one rule this verb shares with `move-actor`: see §4 "A walked body faces where it is walking". `on_arrive`: the driver's final-waypoint tick additionally runs `mv_arrive_<key>` (the bundle's effects), mirroring `ma_tick`/`ma_arrive_<key>` exactly; a bare move emits no hook. The arrive bundle runs with the **server** command source (the driver reached it through `schedule`), so its effects are split per-player / global — see §4 "A scheduled bundle has no `@s`". A later `move-npc` for the **same body** supersedes any walk still running for it — see §4 "One body, one live walk driver"; a body with only one planned walk carries none of that machinery (byte-identical). |
| `cutscene` | Per player: save gamemode+pos → spectator → alternate `spectate` between two co-located dolly cameras each tick (skipping any player actively holding sneak — `predicate=!<ns>:sneak_held`, see §4 "The `spectate` bounce is sneak-gated") → restore. **Keyframe dolly (`compiler::camera`)**: each shot's waypoint polyline is arc-length parameterized (equal distance per time, not equal segments) with baked smoothstep ease-in/ease-out, then emitted as a tick-0 snap + a `tp` every *N* ticks with display-entity `teleport_duration:N` armed via `data merge` — the **client** tweens position and rotation linearly between keyframes (spike-measured: one position-sync packet per keyframe, rotation interpolates, the `spectate` bounce cannot reset an in-flight tween, and a same-tick merge+`tp` applies the OLD duration because position syncs flush before metadata — which is exactly why the snap and its cadence merge may share a tick). Cadence *N* = the widest of {10, 5, 4, 2, 1} whose rendered chords stay within 0.25 blocks (perpendicular) and 2° (aim) of the exact eased path; a single-waypoint or 1-tick shot is a static snap (cadence 0, no merge). Each shot with a successor resets `teleport_duration:0` on its last owned tick so the next snap is a hard cut, not a glide. Every keyframe `tp` carries an explicit `<yaw> <pitch>` — **Minecraft** entity rotation (`yaw = atan2(-dx, dz)`, 0 = +Z south; `pitch = atan2(-dy, hypot(dx,dz))`, + = down), *not* the render-plan/Chunky yaw convention — computed at emission from the camera's own position: at the shot's `look_at` subject if it has one, else along the eased path's direction of travel. Never the summon default (yaw 0 = south). Positions and rotations rounded to 3 decimals, `-0.0` collapsed to `0.0`, so emission is byte-stable. The bracket also arms the `dw_cutscene` state on every player and releases it on restore — see §4 "A cutscene is pure observation". Multi-shot: all shots share one `#t_<bare>` counter — shot *k* owns `[offset_k, offset_k+len_k]` and the next starts at `offset_k+len_k+1` (hard cut); one marker, one `gamemode spectator @a`, one camera pair, one restore. Both single-shot spellings emit identical bytes. `critical-path.json`'s `cutscene_seconds` covers every shot, to the tick `cs_end` runs (see `critical-path.json` in §5). Function key = `cs_<first anchor>_<seconds>_<waypoints>` (a pathless styled shot keys `cs_<style>_<subject>_…`), plus an 8-hex sha256 digest of the whole normalized shot list whenever the cutscene is not a bare single shot without `look_at`/`shot_style` (the key must be injective — two shots sharing a first waypoint must never collapse onto one function). Styled shots are expanded (`compiler::camera::expand_shot`) before keyframe planning; a moving subject's per-tick track comes from its sibling move's A* plan, aligned by effect-group/sequence timing. Deduplication stays DSL-content-keyed, so two byte-identical styled cutscenes in *different* move contexts plan from the first occurrence (a limitation of the content key; give the shots distinguishing content to split them). |
| `campaign-complete` | `dw.campaign` = 1 (dummy objective, **never on the sidebar** — a raw internal id must not surface to players; the sidebar is a currency's slot, spec-0076, and what stands there is a datum's display name); broadcast `[dw:complete <campaign_id> campaign]` (dark-gray bot channel, the harness's completion signal — §4 "The completion-marker channel"); title fanfare. |
| objective lifecycle | **For an announced objective** (spec-0093: a `title` and a resolved `announcement` of `shown`) activation shows `title`+`hint`+`note_block.pling` once (flag `dw.ann_<obj>`, the `announce_<obj>` function and its `tick` line exist only for these); completion sets `dw.o_<obj>` = 1, immediately broadcasts the anchored marker `[dw:complete <campaign_id> obj/<id>]` (§4 "The completion-marker channel") whether or not the objective is announced, then — announced only — prints `Objective complete: <title>` and plays `experience_orb.pickup`. The marker precedes the objective's effects deliberately: it timestamps *completion*, not the aftermath. **Marker cleanup:** completion despawns every entity the objective's activation summoned via the objective-scoped tag — `interact` hitbox + wayfinding marker (`dw_i_<obj>`), `reach` marker (`dw_r_<obj>`). Prop/affordance *blocks* (`interact.prop`, `collect` chest) are scenery and persist; `talk-to`/`kill` summon no per-objective marker. Gated on a resolved activation. |
| `on_death` (spec-0031) | The campaign-wide death beat — **effect root R7**, at `/content/on_death`, one bundle per campaign. It rides the SAME detector `set-checkpoint` arms and adds no second one: `dw.deaths` (`deathCount`) is the only thing in the delve that notices a death, `cp_respawn_check` is the only function that reads it, and one `tick` line runs it per player. What it adds is a second **acknowledgement** of that one counter. `dw.death_ack` is deliberately withheld while a player is dead (the whole edge is held until they are alive again, so an unspent edge stays armed instead of burning on the corpse), which is exactly the window this beat wants, so the corpse side gets its own ack `dw.death_seen`: `execute if data entity @s {Health:0.0f} if score @s dw.deaths > @s dw.death_seen run function <ns>:on_death_fire`, then the matching `= @s dw.deaths` so it fires once per death rather than every tick of the death screen. The corpse side is emitted FIRST, in the order the player lives the two moments. `on_death_fire` is the bundle under **`Audience::Solo`** — the dying player's own, like `on_respawn` and `on_caught`; broadcasting one death to the party would duplicate their narration and their kit. A campaign declaring no death beat emits none of it (no branch, no function, no `dw.death_seen`) and a campaign with a death beat and no checkpoint arms `dw.deaths` and this branch alone — no `#cp` marker, no re-seat, no `dw.death_ack`. **The death POSITION needs no capture**: `emit::death_position_capture` is a named, deliberately empty seam ahead of the dispatch, because the corpse stands on the death position for every cause measured — void, fall, drowning, lava, a mob kill (`docs/notes/death-and-teleport-spike.md`) — so `execute at @s` inside the bundle is the death point. Tests: `v10_on_death.rs`. |
| `set-atmosphere` | spec-0080: `fillbiome <lo> <hi> <biome>` over `Plan::zone_box(region)` or the `place`'s paint (`horizon::place_paint`: an area's placed bounds or a site-plan box's play space, grown by the blend reach — the cells the place's own `atmosphere` paints; a `region` is painted exactly as sized), with `<ns>:atmosphere/<kebab>`, or the ground biome for `atmosphere: null`. Exactly one of `region` / `place` (`DW0929`). Lines come from `atmosphere::fillbiome_lines`, the one writer of `fillbiome` under `crates/delvec/src/` (the surround's bands and the bootstrap paint use it too), which splits the box at 4-cell boundaries so no command exceeds the default `max_block_modifications` (32768, measured the way `FillBiomeCommand.fill` measures: the spans of the box its corners quantize to) — a repaint fires mid-play, where raising a gamerule around it would be a second world-wide write. `fillbiome` paints every 4×4×4 cell the range touches, so the painted volume is the enclosing 4-aligned box. A hard cut: the client re-reads fog and sky every frame and re-meshes grass, foliage and water (`chunk_biomes`, no chunk reload — the spike's measurement, re-asserted by the bot tier). Fires from every quest-effect root and depth; a dialogue option reaches it through a flag a trigger reads, since `DialogueEffect` is its own closed vocabulary. Generated PackTest `atmosphere_repaint_<n>`, one per repaint with a resolvable volume: re-establish the first tick (the ground over every repaint volume of the build, then the carried places), read the first-tick biome inside (`execute if biome`), run exactly the lines the verb emits, read the new biome inside and the unchanged one just outside — a cell no OTHER repaint's volume holds, so a sibling that runs a beat and leaves its paint standing cannot be read (no such cell: no reading, a comment says so) — restore. |
| `set-time` / `set-weather` | `time set <kw|ticks>` / `weather <kw>` (dimension-global, no selector) inline in the effect/dialogue-option function, the time through `WorldTime::token` of its clock (a keyword on day 0 verbatim, any other clock the integer `day × 24000 + daytime`); instantaneous cut, persists (cycle frozen). |
| relight fixtures (`lighting`) | `setblock` per placed fixture in `setup_finish`, after structure placement + socket seals (spec-0010). Blocks: `torch`/`wall_torch`, `lantern[hanging=…]`, `campfire[lit=true]`, `shroomlight`. **What decides whether a fixture can be sited is its ATTACHMENT SURFACE, not its clearance**, and the four differ widely: `torch` takes an off-path air cell over a solid floor OR against any solid horizontal face (`wall_torch`), and `shroomlight` REPLACES a solid block that borders air, so those two site almost anywhere geometry exists at all; `lantern` needs a solid block ABOVE it (hanging), falling back to a floor only on a cell outside the reachable set; `campfire` needs a solid floor with air above AND is barred from every required-path cell, from all four of its horizontal neighbours, and from the reachable set, because it is a damage source — which makes it the narrowest of the four. **Leaves hold no fixture**: a mount — the floor under a torch, campfire or floor lantern, the face behind a wall torch, the ceiling over a hanging lantern — is a solid block that is not `*_leaves`, because the pinned server drops a fixture hung from leaves on the first block update (measured: every lantern the pass hung under `oak_leaves` in the gallery's save was gone, `tools/ci/check-written-world.py`), so light the proof would count is light the game never shows. `DW0211`'s remedy hint names this order, so a creator refused on one fixture is sent to the one the geometry can actually take. |
| `mitigation: "night-vision"` | `night_vision_tick`: one `effect give @a[x=…,dx=…,y=…,dy=…,z=…,dz=…] minecraft:night_vision <lease> 0 true` per declaring area — written since v0.10 by `emit::effect_give_command`, the same formatter the author-facing `give-effect` verb uses, so the engine's own grant is one configured use of the general verb's emission rather than a private copy of it (byte-identical: the line was already the full five-token form) (the lease is `max(12, longest camera + 11)` s — the camera-coverage guarantee, see §"world") (selector = the area's final placed bounds, compile-time literals), then `schedule function <ns>:night_vision_tick 20t` (vanilla replace-mode, so the clock can never double up). `setup_finish` arms it once. A generated `v06_night_vision` PackTest teleports a dummy into the declared bounds, runs one clock tick and asserts it holds the effect — then teleports it 1000 blocks out and asserts it does not. |
| `set-checkpoint` | Inline: `spawnpoint @a <x y z>` + `data modify storage dw:cp pos set value [x,y,z]` (the readable "last checkpoint" mirror) + `#cp dw.sys = <index>` (the active-checkpoint marker; emitted for **every** campaign that declares a checkpoint). `setup_finish` seeds `dw:cp` to the spawn cell. Any checkpoint arms the respawn machinery: a `deathCount` objective (`dw.deaths`) + per-player ack, and a `tick` line running `cp_respawn_check`. **`cp_respawn_check` seeds every score it compares first** (`scoreboard players add @s <obj> 0`, idempotent, and on a `deathCount` objective it does not disturb the criterion): a player who has never died has an entry in none of the three, and a comparison against a missing entry is FALSE on the pinned server — so before this the whole edge was dead on a player's FIRST death and worked only from the second onward. `DW0495` is the standing proof that no emitter can ship that shape again. **The re-seat.** `spawnpoint` is a *hint*, not a promise: vanilla re-validates the recorded cell at respawn time and, whenever that cell or the cell above it is solid or liquid, silently discards it and respawns the player at the **world spawn** — the campaign entrance. Measured live on pinned 1.21.11: a spawnpoint on a dry cell respawns at `cell + (0.5, 0.1, 0.5)`; the same spawnpoint on a water cell respawns at `setworldspawn`. Past a one-way transport that is not a lost checkpoint but an unrecoverable softlock (the owner's tide-mill playtest). So the delve stops delegating its own promise: `cp_respawn_fire` dispatches `cp_seat_<index>` (a bare `tp @s <cell centre>`, coordinates compiled in — no macro, no storage read) for the active checkpoint **before** any authored `on_respawn` beat. When vanilla honoured the spawnpoint the player is already there and the teleport is invisible; when vanilla dropped it, this is the only thing that puts them back. It is edge-triggered, never a leash. **Edge timing**: `deathCount` ticks up on the DEATH, while the player is still a corpse on the death screen, so `cp_respawn_check` holds *both* the fire and the acknowledgement behind `execute unless data entity @s {Health:0.0f}` — the whole bundle lands on a player who has actually come back, and an unspent edge stays armed. Generated PackTests: `v06_checkpoint_respawn` (the record) and `v06_checkpoint_reseat` (the landing — drive a real `deathCount` edge from the campaign entrance, assert the player ends on the checkpoint cell centre, assert the ack, then assert no second re-seat without a second death). |
| `timed-gate` (spec-0016 §4) | `setup_finish` starts the clock: `function <ns>:tgate_open_<id>` at `phase: 0`, else `schedule … <phase>t`. `tgate_open_<id>` = `fill … minecraft:air replace <block>` + `schedule function <ns>:tgate_close_<id> <open_ticks>t`; `tgate_close_<id>` = (when `crush: true`) `execute as @a[<gate region>,tag=!dw_cutscene] run damage @s 1000 minecraft:generic`, then `fill … <block>` + `schedule function <ns>:tgate_open_<id> <closed_ticks>t`. The judgement precedes the `fill` deliberately — after the seal the victim is already encased and vanilla suffocation, not the portcullis, would be what kills them. `/damage` takes ONE entity, so the party form re-binds via `execute as` rather than widening the target. Both halves are pure world edits naming no player, so the server command source they are re-entered under is irrelevant (§4). Generated PackTest `souls_timed_gate_<id>`, **one per declared gate**: re-seal, assert sealed, drive the real open, assert air, drive the real close, assert sealed again. Every scratch score carries the gate id too, because the suite is one batch on one server with no ordering between templates. With `crush: true`, one more for that gate — `souls_timed_gate_crush_<id>`: the emitted region selector holds the dummy standing in the gate and releases it two blocks clear. It asserts **scoping, not death**, because **PackTest fake players are immune to `/damage`** (measured on the pinned toolserver 2026-08-03: a `# @dummy` reports `playerGameType: 0` yet `damage @s 1000 minecraft:generic` leaves `Health` at 20.0, and an explicit `gamemode survival @s` first changes nothing — the same limitation that already put the `damage-players` PackTest on a zombie dummy, which cannot stand in here since the crush selects `@a`). Lethality and ordering are pinned by compiler unit tests, and the end-to-end death was verified against a real mineflayer client on pinned 1.21.11 (parked 2 blocks clear a player survives 30 s of repeated closing ticks at full health; one closing tick standing inside kills them). The test binds `@s`, never `@a`: PackTest runs the whole suite in ONE shared world, so a sibling template's dummy in the same fixture cell would otherwise be counted.  With a `disarm` every line of `tgate_close_<id>` — the judgement, the `fill` and the next hop — plus the open half's `schedule` is prefixed `execute unless score #tgdis_<id> dw.sys matches 1`; the open's own `fill` is deliberately NOT guarded, because a jam landing while the gate is shut leaves one already-scheduled open in flight and that open is what parks the portcullis in its resting position. `setup_finish` summons the jam affordance (interaction hitbox + `dw_hw_…` item_display, `DW0420`), the tick carries the same one-shot `#tgdis_<id>` poll a shortcut unlock uses, and `tgate_disarm_<id>` is four commands whose ORDER is the semantics: latch the sentinel, raise `sets_flag` party-wide, `fill … minecraft:air replace <block>` once, `kill` the hardware (the one function `DW0421` allows to). There is deliberately no `schedule clear`: a close already in flight fires into the guard and does nothing — including not scheduling the next open — so the ping-pong dies of its own accord within one hop. Generated PackTest `souls_timed_gate_disarm_<id>`: prove the clock really seals while armed, pull the real lever, then drive `tgate_close_<id>`/`tgate_open_<id>` across three former cycle boundaries and assert the span is air at each. |
| `shortcut` (spec-0016 §2) | `setup_finish` summons the far-side unlock affordance (`minecraft:interaction`, tag `dw_sc_<id>`) — Alongside the hitbox the compiler summons its **visible hardware** — a glowing, collision-free `minecraft:item_display` at the same cell, tagged `["dw_marker","dw_hw_<tag>"]` (a `minecraft:lever` icon). `minecraft:interaction` is invisible, so the hitbox alone is a right-click target the player cannot see: the drowned-bell soft-lock (`DW0420`). Visibility is the compiler's, never the tileset's. `shortcut_open_<id>` kills it as the bar is thrown: the affordance is spent, and it is the ONLY function permitted to retire it (`DW0421`). — and emits **nothing** for the gate, which the prefab already seals. `tick` polls the affordance's `interaction` record once, guarded by the `#sc_<id> dw.sys` sentinel → `shortcut_open_<id>`, then clears the record. `shortcut_open_<id>` latches the sentinel, clears the gate region (`fill … minecraft:air replace <block>`, the same command `open-gate` emits) and runs `on_unlock` server-source-safe. `on_unlock` is **effect root R6** since spec-0031 — the bundle emission lowers here is now the same bundle every proof and every l10n pass walks. No emitted function anywhere ever re-fills a shortcut gate — the runtime half of permanence, asserted by a test over the whole datapack. Generated PackTest `souls_shortcut`: sealed before, air after, still air after a second unlock pass. |
| `lane` / `summon: aggro-edge` (spec-0016 §6) | A lane wave's `spawn_<wave>` summons the squad with `,Patrolling:1b[,PatrolLeader:1b],patrol_target:[I;x,y,z]` (leader = the first summoned mob, also tagged `dw_lead_<wave>`), `follow_range` forced to `aggro_radius`, then sets `#lane_<wave> dw.sys 0` and schedules `lane_tick_<wave>` at 30t. **Only the snake_case int-array routes** — 1.21.11's strict codec silently drops the legacy `PatrolTarget:{X,Y,Z}` compound and the squad then patrols to vanilla-rolled random points (working-but-drunk); `Patrolling`/`PatrolLeader` keep their camelCase names. `lane_tick_<wave>`: advance guards in DESCENDING index order (so one cycle steps at most one waypoint) firing when ANY squad member is within 8 blocks of the current waypoint (any member, not the leader — a dead leader must not strand the warband); one `data merge entity @s {Patrolling:0b}` release for every mob with a player inside `aggro_radius`; one per-index re-assert `{Patrolling:1b,patrol_target:[I;…]}` for every mob with nobody inside it (this is what defeats vanilla's arrival re-roll and the lone-patroller self-cancel, and it is inert during combat because the patrol goal cannot restart while the mob has a target); then a `schedule … 30t` re-arm guarded on the squad still existing, so the clock stops by itself. An `aggro-edge` wave carries no patrol NBT at all — only its ring placement. Because a re-seat is `kill` + this same `spawn_<wave>`, everything above is also what re-stations a re-seated squad (spec-0016 §1). Generated PackTests `souls_td_patrol_nbt`, `souls_td_lane_march`, `souls_td_lane_release`, `souls_td_lane_reseat` (the squad hauled onto the party, released to native AI and its clock run to the lane's end, then re-summoned: routed again from waypoint 0, release gone), `souls_td_aggro_edge`. |
| `bonfire` (spec-0016 §1) | Inline at the arming beat: `execute unless entity @e[tag=dw_bonfire_<i>] run summon minecraft:interaction … Tags:["dw_bonfire_<i>"]` — Alongside the hitbox the compiler summons its **visible hardware** — a glowing, collision-free `minecraft:item_display` at the same cell, tagged `["dw_marker","dw_hw_<tag>"]` (a `minecraft:campfire` icon, under the same absence guard so a re-fired beat never stacks a second one). `minecraft:interaction` is invisible, so the hitbox alone is a right-click target the player cannot see: the drowned-bell soft-lock (`DW0420`). Visibility is the compiler's, never the tileset's. Never retired: a bonfire is rested at, not used up, so **nothing** may kill its hardware (`DW0421`). — nothing else; the checkpoint does not move. **The click opens a choice, it does not rest.** A per-bonfire advancement `bf_<i>` on the vanilla `player_interacted_with_entity` criterion rewards `bonfire_open_<i>`, which therefore runs **as the clicking player** — the interaction entity's own `interaction` record names no player a `dialog show` could target, which is why the poll was replaced by the same primitive every `interact` objective already uses. `bonfire_open_<i>` revokes its own advancement (a rest point is used, never consumed), sets `dw.rest_at = <i>`, resets then `enable`s the **trigger** objective `dw.rest`, and shows dialog `<ns>:bonfire_<i>`. A trigger because a dialog button runs its command as the player and `/trigger` is the only command a non-operator player may run. The two buttons write `2` (*rest and save*) or `1` (*save only*); `tick` turns each answer into its function — `execute as @a[scores={dw.rest=1,dw.rest_at=<i>}] run function <ns>:bonfire_pick_save_<i>` and the `=2` twin for `bonfire_pick_rest_<i>`. `dw.rest_at` is what keeps a multi-bonfire campaign from routing every answer to the first fire. Each pick resets `dw.rest` first, so one press is one rest. **`bonfire_save_<i>`** = exactly the three `set-checkpoint` lines (`spawnpoint @a`, the `dw:cp pos` mirror, `#cp dw.sys = <i>`) and nothing else. **`bonfire_pick_rest_<i>`** = `bonfire_restore` then `bonfire_rest_<i>`. **`bonfire_rest_<i>`** is unchanged from v0.6 — those three lines + the wave re-seats + the `on_rest` bundle, emitted **server-source-safe** (§4: player-facing effects re-bind to `as @a`, so the whole party rests together and party state fires once). **`bonfire_restore`** is the player-local half: `effect give @s instant_health 1 9 true`, `effect give @s saturation 1 9 true`, one `effect clear @s <id>` per harmful effect (enumerated, never a bare `effect clear @s`, which would also strip the per-area night-vision mitigation clock and any beneficial effect the story granted), then `bonfire_flask`. `instant_health`/`saturation` because vanilla has no `/health` or `/food` command and `/data merge entity` refuses players — those two effects ARE the primitive. **`bonfire_flask`** refills each `flask` kit entry for the class the player took: `execute if entity @s[tag=dw_class_<c>] run clear @s <item-predicate>` then the matching `give @s <item>[components] <count>` — both built by the same two helpers the class kit's own give uses, so the refilled item is poured-identical, and the clear names the flask's potion `contents` rather than a bare item id (§2 stage 3) so it cannot take an unrelated potion out of the bag. `clear`+`give` rather than `item replace` because a kit item has no fixed slot; replenishment is two-directional by construction (a hoarded stack comes back DOWN to the declared count — the flask is a per-rest budget, not a stockpile). The respawn path runs the same `on_rest` bundle through `cp_reset_<i>` (entered from `cp_on_respawn_<i>` only by a body dead at a party wipe) under the player executor, so a bonfire with an empty `on_rest` still dispatches when it owes a re-seat, and — since vanilla already returns a dead player at full health but not with a full flask — it calls `bonfire_flask` too, so retry never costs a second walk to the fire you just respawned at. The **exported critical path rests**: after the step that arms bonfire `<i>` the path gains `{"action":"rest","bonfire":<i>,"anchor":…,"pos":…,"command":"/trigger dw.rest set 2"}` (see `critical-path.json` below). Generated PackTests `souls_bonfire_rest` (the real rest function moves `dw:cp` to the bonfire cell), `souls_bonfire_reseat` (a met, wiped wave stands again at its authored count after a rest; an unmet one is not conjured; and a **chipped survivor**, branded with an ad-hoc tag no re-summon can carry, is gone after the rest while the wave stands full — the no-chip-through rule proven by identity, not arithmetic) `souls_bonfire_options` (save-only moves `dw:cp` and leaves the flask alone; rest replenishes it to the declared count) and `souls_reseat_stationed` (the **stationed** re-seat: the wave is dragged onto the party and — for a lane — released to native AI by the real clock with its march clock run to the lane's end, then the real rest runs, and the fresh squad must stand at its own seating footing, at the authored count, with no mob of the previous life left and the routed state re-applied). **The undefeated re-seat.** A rest re-seats two more things, and both are gated on the body still being there rather than on a sentinel, so "undefeated" is asked of the world: (a) every **billed `elite`/`boss` wave** that does not declare `respawns_on_rest` — `execute if entity @e[tag=dw_wave_<id>] run function <ns>:wave_reseat_<id>`, the same removal-and-respawn the stationed re-seat uses, so a boss chipped one hit per life comes back at full count and full health; (b) every **hostile actor** — an actor the campaign `unleash-actor`s anywhere (`combat::hostile_actors`, the compiler's one "unleash or nothing" definition of an actor that is a fight) — as `execute unless entity @e[tag=dw_pup_<id>] if entity @e[tag=dw_actor_<id>] run function <ns>:actor_restand_<id>`. `actor_restand_<id>` is the removal of `dw_actor_<id>` (its declared `drops` stripped first) then the twin summon at the actor's **absolute origin cell**, byte-identical to the body `unleash_<id>` produces. Three deliberate asymmetries: it puts the elite back **freed, never re-caged** (the `unleash-actor` beat fires from a one-shot trigger the engine never re-arms, so a re-caged elite would be dormant `Invulnerable` scenery for the rest of the delve); it does **not** re-apply the striker aggro lock (nobody has provoked this body — it stands on its anchor under vanilla-local AI, inside the `follow_range` `DW0478` measured the fire against); and it leaves a **caged puppet alone** (a puppet is `NoAI`, knockback-immune and normally `Invulnerable`, so combat can neither damage nor move it, and re-seating one would only undo authored `move-actor` staging). A killed or `despawn-actor`ed body selects nothing, so a **defeated boss stays dead** by construction (spec-0016 §1) with no state to keep. Generated PackTests `souls_reseat_actor` (stage the elite, unleash it, drag it onto the party, chip it to 1 HP and brand it, then run the REAL rest: one body, unbranded, within 2 blocks of its origin, no puppet — then kill it, rest again, and nothing comes back) and `souls_reseat_undefeated` (the same claim for a boss wave: ground down to a branded survivor, restored whole and unbranded by a rest; killed outright, never conjured back). `souls_unleash_yields_nothing` and `souls_reseat_yields_nothing`, emitted when a re-seated wave or hostile actor declares `drops` — one template per removal, because a failing `assert` does not abort a template and the log names only the last one (see "PackTest batch model"), so one template judging both removals reported a leaking unleash as the rest's failure. In each, the body is dragged onto the party and the real removal runs — `unleash_<id>` on each drop-declaring hostile actor's puppet; `bonfire_rest_<i>` on every drop-declaring re-seated wave and on each such actor's unleashed twin, after what the unleash parked is killed and cleared uncounted — followed in the same tick by a kill of the `dw_unseen` bodies in the party's column at Y −128 (see "A body the story removes" — a template never runs `unseen_sweep`); no item entity may lie within 3 blocks of Y −128 in the party's column, where an unseen removal's body dies. Then the body that removal takes, fresh — a puppet for the unleash, each wave and twin for the rest — is moved to that same place and killed by a bare `kill`, which must yield one there, so the zero is measured where the removal kills, against a body that carries the loot. Each template's title states its binding: the number of bodies it meets, by id. Both re-seats also ride the respawn path (`party_reseat`, entered from `cp_reset_<i>` while `#wipe` is latched), because a party wiped at a bonfire is owed the same scene; a death in a party still fighting re-seats nothing. Where a bonfire may STAND is proven separately: `DW0478` forbids one inside any wave's or fighting actor's aggro range — seated cells and lane polyline alike — because the fire is where the party respawns and where every `respawns_on_rest` wave is put back on its feet. |
| `loops[]` (spec-0086) | One tick line `function <ns>:loop_<id>_poll` per loop. `loop_<id>_poll` is one line: `execute<gate> as @e[<slab box>,tag=!dw_fixture,tag=!dw_cutscene] at @s run function <ns>:loop_<id>`, where `<gate>` is the loop's gate as `if`/`unless score #party …` terms (the same formatter every gate consumer uses) and the box is the slab from `Plan::zone_box` through `emit::box_selector_args`. `loop_<id>` = `scoreboard players add #party dw.s_<counts> 1` (when `counts` is declared), then `tp @s ~dx ~dy ~dz`, then `function <ns>:loop_<id>_cross`, the `on_cross` bundle emitted under `Audience::Scheduled`. The move is relative, so position-in-cell, facing and velocity carry (spec-0086 §2). Generated PackTests, synchronous: `loop_<id>` drives the gate to hold (each flag and datum at a value the gate admits), summons a tagged `NoAI` zombie in the slab, asserts it is selected by the slab box, runs the real `loop_<id>_poll`, and asserts its position moved by exactly the offset (read in thousandths of a block) and the count by one; `loop_<id>_released` drives one gate term shut and asserts the same poll moves nothing and counts nothing. |
| `respawn_wait` (spec-0077) | World-level, emitted only when declared. `setup` adds the per-player clock `dw.rwait` (dummy, held **from 1** exactly while a player waits, so `matches 1..` is "waiting" and a missing entry reads the same as a reset one — `DW0495`). The party-wipe detector (emitted for a bonfire **or** a declared wait) counts `#alive` as alive **and** unclocked, and counts `#present`; the wait is read off its clock, not the tag, because a cutscene tags every player and would otherwise latch a wipe. The respawn edge in `cp_respawn_check` calls **`rw_begin`** instead of `cp_respawn_fire`: with a second player present it starts the wait (`rw_start`) when `#alive − 1 ≥ 1` and the player is not tagged `dw_wiped`; with `alone: true` and nobody else present it starts it too; otherwise it fires `cp_respawn_fire` itself. `rw_start` sets the clock to 1, takes the player out of this tick's `#alive`, adds `dw_cutscene`, switches to spectator and runs `rw_lock`. **A waiting player answers nothing**: `rw_lock` is one `scoreboard players reset @s <t>` per trigger objective the finished `setup` declares (read off `setup` itself, so a channel an emitter adds later is locked by existing — the class pick, the bonfire, the shop, each NPC dialog, each interact), which clears the score and revokes the permission, so the waiting player's own `/trigger` is refused and no dispatch reads a stale answer; `rw_release` enables nothing, and each channel is re-armed by the normal flow as for any player (the per-tick `enable @a`, `class_arm`, a bonfire or shop opening its dialog). `tick`, after the wipe detector and before the edge: `execute if score #alive dw.sys matches 0 [if score #present dw.sys matches 2.. — alone only] as @a if score @s dw.rwait matches 1.. run function <ns>:rw_release` (a wipe ends every wait), then `execute as @a if score @s dw.rwait matches 1.. run function <ns>:rw_lock` — after every per-tick `enable @a`, and not held by a cutscene, so every tick ends with a waiting player's channels shut — then `rw_tick` for every clocked player (held whole while a cutscene plays: `unless score #cs_live dw.sys matches 1..`). `rw_tick` re-applies spectator and the tag (a relog comes back in adventure under `force-gamemode`; a cutscene's end restores adventure), releases at `matches <seconds×20>..`, shows the seconds left on the action bar (`delvewright.ui.respawn.wait`, `with` = `#rw_left dw.sys`), counts the clock, and runs `execute at @s unless predicate <ns>:sneak_held run spectate @p[tag=!dw_cutscene,nbt=!{Health:0.0f}] @s` — the cutscene bounce's sneak rule; the watcher stands where its target stands, so the nearest is the one it already watches. With `alone: true`, a watcher with nobody to watch is held on the active checkpoint cell (`rw_watch_fire`, the `cp_seat_<i>` dispatch). `rw_release` resets the clock, removes the tag, returns to adventure and runs `cp_respawn_fire` — the seat, the fire's per-player half (flask refill) and, after a wipe, the one re-seat. **A watcher is out of play everywhere** (`crate::compiler::observer`): in every campaign, with or without a wait (a cutscene viewer carries the tag too), the approach-trigger test, a lane's patrol test, the cutscene repair driver and the cutscene's return mark (`@p`) gain `tag=!dw_cutscene`, and every build censuses every positional player selector in the shipped tree (`@a`/`@r`/player-typed `@e` with a box or `distance` term; `@p`; player-typed `@n`) into *excludes the tag*, *at an allowed site* (`observer::ALLOWED`, each with its reason: boxed status effects, the ladder's staged blow) or *unguarded*; one unguarded selector refuses the build with `DW0926` (an engine self-check) and the count is printed as `observer binding: …` and written to `validation/observer-census.json`. Gallery primary: 119 positional player selectors over 348 functions, 110 guarded, 9 allowed, 0 unguarded. Live proof: `harness/probe/respawn-wait-party.ts`. |
| health bar (`waves[]`/`actors[]` `health_bar`, spec-0073) | One vanilla custom boss bar per declaring fight, id `<ns>:hb_<key>` where `<key>` is `wave_<safe id>` or `actor_<safe id>` (a wave and an actor sharing a local name never share a bar). **World init** (`setup`): `bossbar remove` then `bossbar add <id> <title component>` — an `add` on an existing id fails, and a bar left by an earlier build of the same world must not survive — then `bossbar set <id> color|style <lit>` only when declared, `bossbar set <id> players` (no audience) and the holders `#hbv_<key>`/`#hbm_<key>` zeroed on `dw.sys`. The title component is the i18n v2 `{translate, fallback}` form: a stated `title` under `wave.<w>.health_bar.title`/`actor.<a>.health_bar.title`, a derived one under the name's own key. **Every tick**: `function <ns>:hb_<key>`, whose first line is `execute unless entity @e[<bodies>,nbt=!{Health:0.0f},limit=1] run return run bossbar set <id> visible false` — no live body, hidden for everyone, on the tick of the last death. Otherwise it zeroes `#hbv_<key>`, runs `hb_acc_<key>` as every live body (`data get entity @s Health 1` added in), stores the value into the bar, restates the max from `#hbm_<key>` when it is `1..`, clears the player tag `dw_hb_<key>`, re-tags every player within `range` of a live body who is not in a cutscene and not dead (`execute as @e[<bodies>,nbt=!{Health:0.0f}] at @s run tag @a[distance=..<range>,tag=!dw_cutscene,nbt=!{Health:0.0f}] add dw_hb_<key>`), sets the bar's players to `@a[tag=dw_hb_<key>]` and `visible true`. `<bodies>` is `tag=dw_wave_<id>` for a wave; `tag=dw_actor_<id>,tag=!dw_pup_<id>` for an actor, without the narrowing when it is `vulnerable`. **Max capture**: `hb_max_<key>` zeroes `#hbm_<key>` and sums `attribute @s minecraft:max_health get 1` over the live bodies (`hb_macc_<key>`); it is the last line of every function that summons the fight's bodies — `spawn_<wave>` (so every re-seat, which re-runs it, recaptures), `spawn_actor_<id>`, `unleash_<id>` and `actor_restand_<id>` — so a fight of three with one dead reads two-thirds, and a re-seated fight reads full. `crates/delvec/tests/call_graph_integrity.rs` finds every summoner from the bytes and holds each to its capture. On a server restart the bar persists in `level.dat` (vanilla stores custom bars there) and the first tick re-derives value, max, audience and visibility from the world, so nothing stale outlives one tick. Generated PackTests: `health_bar_<key>` per declared bar (meet the fight through its real spawn/unleash, lift the dummy and the fight to a private altitude so the audience count is exact in a shared batch, then in range: one player, visible, value = max = the fight's summed `max_health` — the declared literal when every body declares it, else the server's own `attribute … base get` summed by `scoreboard players operation`; chip one body to 1 HP with `data modify` and the value moves by exactly what it lost; lifted out of `range`: no player; with a bonfire that re-seats the fight, the real `bonfire_rest_<i>` and value = max again; every body killed: hidden) and `health_bar_several` when two or more are declared (`bossbar list` returns the declared count after `setup`, and each id answers `bossbar get … max`). A campaign that declares no bar emits none of this. |
| `begin-stealth` / `end-stealth` | `begin` → `#stealth dw.sys = <session>` + reset per-player `dw.st_grace`. `tick` runs `stealth_tick_<session>` while active → per-player `stealth_eval_<session>`: safe iff inside some zone box (a pure position selector — **zone presence alone = hidden**; there is no sneak requirement, which would collide with the spectator cutscene camera); grace resets when safe, climbs when exposed, and at `grace_ticks` fires `stealth_caught_<session>` (`on_caught`). `end` → `#stealth dw.sys = 0`. The `v06_stealth` PackTest disarms `#stealth` (sets it 0) after each `stealth_begin` because it drives `stealth_eval` explicitly: an armed session would make the world `tick` loop run a *second* judge pass in the same tick, double-counting exposure and mis-accruing grace (this only isolates the test; runtime gameplay has the tick loop as sole caller). It pins its dummy by tag (see "PackTest batch model" below), drives hidden/exposed purely by teleporting the dummy in/out of the zone box, runs the spare (safe-player) section first and the `on_caught` trip LAST — the trip executes arbitrary campaign `on_caught` content (possibly lethal), so nothing state-dependent follows it and the closing assert reads the dummy through the tag, which keeps matching even if the trip killed it. |
| `give-item` `carrier` (spec-0018) | Absent/`all` → `give @a <item> <count>`: a quest beat arms the whole party. `one` → `give @s …`, the single quest prop handed to the player whose action fired the effect, for the party to pass around physically. `one` inside a scheduler-only bundle has no acting player and is rejected at validate time (`DW0357`). |
| trap `payload` — detection (spec-0022) | The compiler owns the detection tick, because the consequence is commands. Two primitives, both already in the compiler, **none of them block-power polling** (which spec-0011 excluded as folklore): a `pressure-plate`/`tripwire` is a POSITION test on the trigger cell (`execute … if entity @a[x=…,dx=0,…]`, the `reach-anchor` idiom), and a `trapped-chest` is the interaction-entity `use` — the same primitive the disarm affordance uses. Edge-triggered on a `#trapfire_<trap>` sentinel so stepping onto a plate is ONE event; a `rearm` trap clears the sentinel when the cell is vacated, a `once` trap never does (which is exactly the survivability discharge `DW0342` reasons about). Guarded by the flag gate when the trap declares one, and by the disarm latch when it has a disarm — load-bearing in a way it was not for redstone, since a command payload has no ammunition to empty. `trap_fire_<trap>` then runs the bundle under `Audience::Scheduled`: a trap is the dungeon firing at the party, not at whoever touched the plate, so player-facing effects address `@a` and there is no `@s`. A trap with no `payload` emits none of this. |
| `volley` (spec-0022) | One start function fans out into one function per salvo via `schedule` — the `sequence` shape, so **a volley costs nothing per tick**. Each salvo is (1) the *saturation*: one projectile per standable kill-zone cell, unconditional, with the compile-time velocity that reaches that cell — this is the contract, and it is why moving between salvos does not help; and (2) the *aimed extra*: a second projectile toward whichever cells hold a player this tick, selected by a plain block-volume selector, so standing still costs double fire. Both use compile-time velocities: there is no runtime vector arithmetic and no scoreboard math, because vanilla has no primitive for a runtime-aimed projectile and inventing one would be exactly the folklore the no-hack doctrine forbids. Projectiles are `NoGravity` (so the flown path IS the proven segment — drag scales speed without turning the line), `crit:0b` (deterministic damage; a random crit bonus would make the PackTest flaky) and `pickup:0b` (no loot litter in adventure mode). Speed is 2.5 b/t: arrow impact damage is `ceil(|velocity| x damage)` with `damage` defaulting to 2.0, so each arrow lands 5 half-hearts — a real consequence that three saturating salvos can kill, without any single arrow being an instant death. Coverage is proven at compile time (`DW0442`), the zone must be watchable from safe ground before the player walks into it (`DW0388`), and a body a salvo lands on must be able to walk out before the next one in at least 20% of the interval (`DW0918`). |
| `collapse` (spec-0022) | Summon one `falling_block` per region cell that holds a block (`HurtEntities:1b` for impact damage), then `fill` the region to air — the buried-alive beat redstone cannot express at all. The debris is settled deterministically at compile time (each column onto the first solid cell beneath it) and that post-collapse world joins the completability model (`DW0445`). An authored `then_floor` paves the settled surface via a scheduled second function, delayed by the computed fall height, because the rubble is still in flight when the trap fires. |
| trap `dispense` (spec-0011) | `setup_finish`: `item replace block <disp> container.0 with <item> <count>` fills the prefab's pre-wired dispenser socket (the `anchor/trap` metadata `dispenser` cell) — a static, deterministic payload, the same mechanism as a `collect` chest. **No detection** is emitted for the harm: the plate/tripwire/trapped-chest → dispenser redstone is already in the prefab. Pressure plates and tripwire are modelled **passable** in the assembled occupancy (`crate::assembled::is_passable_trap_trigger`) so nav routes a player ONTO a trigger cell rather than around a "solid" plate. |
| trap `requires_flags` / `forbids_flags` (spec-0011) | A **physical** gate, because the compiler owns world mutation: `trap_gate_on_<trap>` restores the trigger block declared by the `anchor/trap` metadata's `trigger_block` (verbatim, blockstate and all) and `trap_gate_off_<trap>` clears the cell to air, so a shut gate means a player stepping on the trigger steps on nothing. Edge-triggered on a `#trapgate_<trap>` sentinel, so the `setblock` fires on a gate transition rather than every tick. The clauses live in `trap_gate_tick`, which the tick calls. The gate is **campaign state** and every term is read where the campaign writes it, through the one rule every gate consumer uses (`Plan::gate_terms`): a flag as `score #party dw.f_<flag> matches 1`, a `requires_state` datum on its holder. One shutting clause per term ("not (every term holds)" is a disjunction) and one opening clause carrying the full conjunction. `setup_finish` calls `trap_gate_init` before the first tick: it arms each gated trap (`trap_gate_on_<trap>`) and runs the tick's own shutting clauses, so the trap starts in the state its whole gate says — a required flag nothing has set yet, or a `requires_state` datum whose declared initial fails its comparison, disarms it before any player can step on it. An **ungated** trap emits none of this. Only sound for a trigger whose whole state is the block — `DW0363` rejects the rest rather than shipping folklore. PackTest `v06_trap_gate` binds the first gated trap, whatever its axes: it drives the whole gate open on `#party` and runs `trap_gate_tick` (never the on/off halves directly), then for every term breaks that term alone, ticks, asserts the trigger cell is air, repairs it, ticks, and asserts the authored trigger is back; the same truth table runs first through `trap_gate_init`, the world-start seed; each assert has its own holder `#tgate_<n>`, so a red names its step. |
| trap `disarm` (spec-0011) | `setup_finish` summons a `minecraft:interaction` at the disarm `via` cell (tag `dw_trapdis_<trap>`) and beside it its **visible hardware** — a glowing, collision-free `minecraft:item_display` at the same cell, tagged `["dw_marker","dw_hw_<tag>"]` (a `minecraft:lever` icon); `minecraft:interaction` is invisible, so the hitbox alone would be a right-click target the player cannot see (`DW0420`). Visibility is the compiler's, never the tileset's, and `trap_disarm_<trap>` is the only function that kills it (`DW0421`). `tick` fires `trap_disarm_<trap>` once on a right-click (`nbt={interaction:{}}`, the `use` primitive). `trap_disarm_<trap>` sets the party-wide `dw.f_<flag>` and empties the dispenser (`data modify block <disp> Items set value []`) — the modeled, global disarm that actually stops a redstone dispense trap. |

**The party holder (`#party`, spec-0018).** Progress is a fact about the party,
not about a player. Every progression score — `dw.o_<obj>`, `dw.q_<quest>`,
`dw.qa_<quest>`, `dw.f_<flag>`, `dw.ann_<obj>` and `dw.campaign` — is read and
written on the single fake player `#party`, so any member's completing action
advances everyone, and `after: [obj/a, obj/b]` becomes a **division of labour**:
A clears one arm in one room, B the other in another, and the successor's guard
(every term a `#party` read) opens for both. A fake player needs no entity and
survives every join/leave, which is exactly the lifetime party state needs.

Consequences, all mechanical:

- the `announce_<obj>` / `activate_<obj>` tick drivers need no player context at
  all (their whole predicate is party state) and therefore fire **once for the
  party**; the completion drivers keep `as @a` because they still test a real
  player (proximity, held items, a fired trigger). Those stay single-fire because
  vanilla evaluates `execute as @a … if … run` per selected player *in turn*: the
  first player's `run` sets the party score and every later player's `unless score
  #party …` fails in the same tick;
- objective/quest/campaign UI addresses `@a` (`tellraw @a`, `title @a`,
  `playsound … @a`, `advancement grant @a`), so the party is told together;
- what stays **per-player** is exactly what belongs to a body: `dw.class` /
  `dw.classed` / `dw.dlg_shown`, `dw.dlg_<npc>` / `dw.i_<obj>` triggers,
  `dw.dmask` (this player's dialog screen — its *conditions* read `#party`),
  `dw.hold`, `dw.deaths` / `dw.death_ack` / `dw.death_seen`, `dw.st_grace` /
  `dw.st_safe`,
  inventory, position, and cinematic attach/restore.

CI-enforced by `tests/party.rs::no_per_player_progression_scoreboard_remains`, a
sweep over every emitted pack of every fixture family: a progression score may
appear only after the `#party` holder token (or in its `scoreboard objectives
add` declaration), and no selector may filter players by one. A *partial*
migration — player A's objective set, player B's guard still shut — is the
soft-lock that no single-player test can see.

Naming: `dw.o_<obj>`, `dw.q_<quest>`, `dw.qa_<quest>` (active), `dw.dlg_<npc>`,
`dw.f_<flag>`, tags `dw_npc_<npc>`/`dw_wave_<id>`/`dw_i_<obj>`/`dw_r_<obj>`.
Cast ledger (spec-0020): the per-player scene selector `dw.cast`, and one
`#bk_<npc>_<scene>` bark-pool counter per pool on `dw.sys`.
`CustomName` is a plain SNBT text component (not `'{"text":…}'`).
Checkpoints/stealth (spec-0012/0014): storage `dw:cp pos` (last-checkpoint
mirror, a `[x,y,z]` int list); scores `dw.deaths` (`deathCount`) + `dw.death_ack`
(+ `dw.death_seen`, the corpse-side ack, only when the campaign declares `on_death`),
`dw.st_grace`/`dw.st_safe` (no sneak-stat scores — the judge is position-only);
markers `#cp`/`#stealth` on `dw.sys`. A campaign with a cutscene also ships the
datapack predicate `<ns>:sneak_held` (the cutscene bounce's re-attach gate, §4).

---

## 4. Hard invariants

### One tick's command chain stays under the game's limit

The pinned server runs a function, and every function it calls in the same tick, inside one execution context whose command quota is the `max_command_sequence_length` gamerule (65,536 by default); past it the server logs `Command execution stopped due to limit` and drops the rest, and nothing reads the log. Each root — a `#minecraft:tick`/`#minecraft:load` entry, a scheduled function, an advancement reward — has its own context. How the quota is charged, and the cost model, are on [`delvec/compiler/chain.md`](delvec/compiler/chain.md).

- **The world build.** The world's own block writes — each area's derived mass and socket seals, then the stage-7 edit script's writes, in the compile-time model's order — grow with the campaign (one sculpted stamp is tens of thousands of lines), so they run as steps: `world_build`, `world_build_2`, …, each at most `chain::SETUP_STEP_BUDGET` (a quarter of the limit). `place_verify` enters `world_build` once every sentinel reports; its first line latches `#placed dw.sys` at **2**, and the tick's `place_all`/`place_verify` run only while `#placed` is **0**, so no template is placed back over a write. Each step ends `schedule function <ns>:<next> 1t`; the last schedules `setup_finish`, which latches `#placed` at **1** as its last line. A campaign with no mass, seal or edit write has no step, and `place_verify` calls `setup_finish` directly. Every reader of setup completion reads `#placed` = 1: `join_place`'s gate, `validation/world-save.sh` and `validation/reset-when-empty-run.sh` over rcon, and the bot through `join_place`. A generated PackTest that re-runs `setup_finish` re-runs no block build.
- **The refusal (`DW0984`).** After emission, over the finished tree, every shipped function's chain is measured and a build where any could pass the limit — or that reaches a call cycle no count bounds — is refused. `validation/chain-length.json` is its ledger.

### One `dsl_version` (ADR-0024)

The engine accepts exactly one `dsl_version`, `delvewright_dsl::DSL_VERSION`; a
document declaring any other is refused at the envelope with `DW0102`. Every
rule applies to every document the engine accepts — a `DwCode` carries its exit
tier and its subject and nothing about when it binds — so a finding is never
excused by a number: a campaign that needs relief from a rule needs the rule
changed or the document changed. What a campaign compiles to is a function of
its documents and the engine, never of a per-document number.

A surface change bumps `DSL_VERSION` and moves every document this repository
holds — fixtures, gallery, probes — in the same change. A released campaign
reproduces through its pinned engine (`versions.toml` + the OCI image,
ADR-0010) and moves to a new number by an adoption round that bumps its pin.

The `&'static str` code constants in `delvec schem`, `delvec prefab` and
`delvec render` are about prefabs, schematics and renders, artifacts that carry
no `dsl_version`; `tools/ci/check-dw-codes.py` resolves both forms.

### A scheduled bundle has no `@s` (executor contract)

`schedule function <ns>:<f> <n>t` re-invokes `<f>` with the **server** command
source: no executor, so `@s` resolves to nothing and every `@s`-addressed command
in it *silently does nothing* — no error, no log line. Three generated bundles are
reached only that way:

| bundle | reached from |
|---|---|
| `mv_arrive_<key>` | `mv_tick_<key>`, itself re-scheduled every tick |
| `ma_arrive_<key>` | `ma_tick_<key>`, likewise |
| `seq_<root>_<n>_<i>` | `seq_<root>_<n>`'s `schedule … <at_ticks>t` chain |

The cost of emitting them verbatim: an `on_arrive` bundle that sets a flag an
objective gates on soft-locks the party, and every `title`/`tellraw`/`playsound`
beat in it is dead.

**The rule.** A bundle is emitted for an explicit **audience**
(`emit::Audience::{Party, Scheduled, Solo}`), and each effect is classified
individually, never the bundle as a whole (`emit_quest_effect` takes the audience
selector; the executor match is exhaustive — a new effect verb must state its
scope or the compiler refuses to build):

- `Party` — a party event entered as one player (`complete_<obj>`,
  `complete_q_<quest>`, `trig_<id>`): `@s` exists and is the completing player;
- `Scheduled` — the three bundles above: **no `@s` at all**;
- `Solo` — a checkpoint `on_respawn` / a stealth `on_caught` / `on_death` / a
  shop offer / a fight's `on_kill` (the player credited with the kill): the
  bundle belongs to the one player it fired for, and stays `@s` throughout (re-broadcasting one
  player's death would re-gift and re-narrate at every survivor).

Under `Party`/`Scheduled`:

- **player-facing** (`Verb::addresses_players`: `narrate`, `give-item`,
  `play-sound`, `particle`, `give-effect`, `clear-effect`, `damage-players`) →
  the command names `@a` directly (`tellraw @a`, `give @a`, `damage @a[…]`,
  `playsound … @a`), so the whole party sees the beat **once**. The one
  listener-relative form (a `players` sound with an explicit volume/pitch, which
  forces a `~ ~ ~`) is wrapped `execute as @a at @s run …` so `~ ~ ~` resolves at
  each listener rather than at the command's own position;
- **party-fact** (`set-flag` — a `#party` write, gates, `set-block`,
  `spawn-wave`, `spawn`/`despawn`/`move`/`unleash-actor`,
  `spawn`/`despawn`/`move-npc`, `cutscene`, `set-time`/`set-weather`,
  `set-checkpoint`, `bonfire`, `begin`/`end-stealth`, `sequence`,
  `campaign-complete`, the region writes, `teleport`, `volley`, `collapse`,
  `firework`, `lightning`, the state verbs) → emitted **bare**, so it fires exactly once.
  The envelope's `audience` and `in` on one of these is `DW0942`; the set is
  `Verb::addresses_players`, bound to the emitted bytes by
  `tests/v35_perception.rs::every_verb_is_classified_by_what_the_emitter_does`. A blanket `execute as @a run
  function <bundle>` — the obvious fix — is wrong: it would fire every `fill`,
  `summon`, driver start and `schedule` once per player.

**The `#party` holder (spec-0018) narrows this seam to nearly nothing.**
Progression lives on the `#party` holder, so a scheduled `set-flag` writes
`scoreboard players set #party dw.f_<flag> 1` and names no executor — the
soft-lock above **cannot occur for flags**. Every
player-facing effect addresses `@a`, which needs no executor either. Exactly one
construct is still executor-shaped: a `carrier: "one"` `give-item`, which needs
the acting player, and is therefore rejected at validate time inside a
scheduler-only bundle (`DW0357`).

Per-effect flag gates have **one spelling** everywhere (`if score #party
dw.f_<flag> matches 1` / `unless score #party …`, unset-safe): flags are party
state, so there is no per-player variant to diverge and no "does some player hold
it" selector to approximate it with, and a gated effect inside an
`on_arrive`/`sequence` step is gated like any other.

**A timeline keeps its actor** (spec-0085). A `sequence` is keyed by its value
and by the audience it is started under, so the forms never share a body. Started
where there is an acting player (`Party` or `Solo`) **and using it** — some step,
emitted under that audience, says something it would not say from the server
source (an `actor` audience, a solo root's `@s`), or hands a `carrier: "one"`
prop, or touches a `player`-scoped datum — the start function tags `@s` with
`dw_seq_<root>_<n>`, every step `i` is reached through `seq_<root>_<n>_<i>_as`,
whose one line is `execute as @a[tag=dw_seq_<root>_<n>] at @s run function
<ns>:seq_<root>_<n>_<i>` (so inside the step `@s` is the actor, standing where
they stand), every step body is emitted under the timeline's audience, and the
last step removes the tag. Every step goes through the dispatch, the inline
`at_ticks: 0` one included — a timeline whose first beat behaved differently
from its second would be a trap. A timeline that does not use its actor — every
timeline of world facts — and every timeline started from the server source
(`Scheduled`: a polled trigger, a trap, a shortcut, an `on_arrive`, a `bonfire`'s
rest) is emitted untagged, its steps called and scheduled directly and its
player-facing beats addressing the party; so a timeline of spawns and flags never
waits on one player's connection. `schedule` is replace-mode, so a second start
before the first ends re-times the chain for every tagged player: the timeline
stays global, with a tag on it. The cost of the tag form, stated: a tagged
player who leaves the server before a step runs takes that step with them.

Which nested bundle has an acting player is one statement,
`QuestEffect::nested_effect_dispatch`: a `sequence` step inherits its parent's;
a `set-checkpoint`'s `on_respawn` and a `begin-stealth`'s `on_caught` always have
one; a `move-npc`/`move-actor` `on_arrive` and a `bonfire`'s `on_rest` never do.
`DW0357` and `DW0503` read it.

**Enforcement** (all three; never relax one):

1. `tests/scheduled_executor.rs` walks the emitted call graph from every
   `schedule` site — following `function` calls that do *not* re-bind the
   executor — and asserts no function in that closure names `@s` outside an `as`
   clause (`positioned as`/`rotated as` do not bind), listing every dead command
   it finds. It passes because nothing in those bundles addresses a player at
   all.
2. Two generated PackTests drive the **real scheduler** (never an inline
   `function` call — running the driver inline *as the dummy* supplies exactly
   the executor the scheduler withholds): `sched_executor` (unconditional, so every campaign proves
   the seam live — it schedules a probe function emitted by the real
   scheduled-bundle emitter and awaits the flag on its own dummy) and
   `sched_arrive_flag` (the content path: the first `move-npc` whose `on_arrive`
   sets a flag; runs the real start function and lets the driver walk itself to
   the end). Both `await` the flag on `#party` and are the **sole owner** of the score
   they await (`tests/packtest_batch.rs::party_state_across_ticks_is_owned`).
3. The suite datapack may therefore carry `data/<ns>/function/` mechanism
   functions beside `data/<ns>/test/`. PackTest only discovers `test/`, so
   every `function/` file must be **reachable from some template** — named by a
   template directly (`tests/emit.rs`), or through the packtest function graph
   (the campaign phase chain below; `tests/packtest_campaign.rs` walks the
   closure). An orphan there is a test PackTest would never run.

### The campaign mechanism test (scheduled endings, branches)

The `campaign` template drives every objective's `complete_o_*` on its dummy and
asserts the completion objective on `#party`. Two structural facts of the
campaign pick its shape: whether the ending is scheduled (a same-tick assert
cannot see a `sequence`-scheduled finale) and whether it branches (driving every
branch's terminal objective in one tick is a state no playthrough reaches).

**All three shapes open with the same full progression re-baseline**
(`campaign_progression_baseline`): completion objective, every declared flag,
every `dw.q_*` / `dw.qa_*` / `dw.o_*` to 0, then the campaign-start quests
active. The template plays the campaign **from its start**, not from wherever
the batch left it. That is not decoration: the chain it drives is latched —
`check_q_<q>` fires `complete_q_<q>` only `unless score #party dw.q_<q> matches
1` — so a quest a sibling already completed makes the whole drive a silent no-op
and the assert reads 0 on tick 0. `#party` is batch-global, PackTest randomises
the batch order, and a sibling running the campaign's real `tick` can complete a
terminal quest outright, so any campaign whose quests can advance by a route
other than this template is exposed. `DW0807` refuses the emission that
omits it. Zeroing is safe because every term zeroed is written
again by the drive that follows it inside the same atomic mcfunction, so no
sibling can observe the zeroed state.

**The baseline and drive are hoisted into `pt_camp_drive` in every non-branch
shape** (`pt_camp_run_<i>` is the branch shape's equivalent), so the template's
own body touches exactly one `#party` score — the one it asserts or awaits.
That is not only about spanning ticks: `packtest_batch::party_state_across_ticks_is_owned`
reads each template's OWN text and demands that a `#party` score awaited across
ticks be touched by one template alone, and a whole-ledger baseline written
inline would refuse any campaign whose suite also awaits one of those scores —
`sched_arrive_flag`, emitted for a `move-npc` whose `on_arrive` sets a flag,
awaits exactly that. Hoisting is the state that satisfies `DW0807` and that test
together, so it does not depend on the ending's shape.

- **Synchronous ending, no `branch_points`** — the single-TICK template: it
  `assert`s on the spot, `# @timeout 100`, and its exported critical path carries
  no tail field.
- **Scheduled ending, no `branch_points`** — the emitter computes the ending
  tail (`campaign_complete_tail`: max scheduled offset to a `campaign-complete`
  across all nesting — `sequence` steps add `at_ticks`, `move-npc`/`move-actor`
  `on_arrive` adds the planned walk; reaction bundles are skipped, `DW0204`
  proves the path's ending is not exclusively there) and the template `await`s
  the completion objective with `# @timeout 100 + tail`. `await` is never a
  weaker `assert`: it fails the test at timeout exactly as `assert` fails it on
  the spot.
- **Declared `branch_points`** — ONE template, one **phase per reachable
  realized branch**, serialized through the vanilla scheduler (two concurrent
  templates awaiting the shared completion objective would hand each other
  false verdicts in batch order). `pt_camp_run_<i>` opens with the same
  re-baseline (a prior phase's completed quest would otherwise keep its
  `unless dw.q_*` guarded `on_complete` from re-firing on this shape's second
  phase),
  then drives ONLY that branch's flow playthrough in
  path order, emulating each branch-scripted dialogue option's `set-flag`s
  immediately before its `talk-to` drive (the real playthrough sets them there;
  a UI click is not available to a dummy). It then schedules
  `pt_camp_check_<i>` at `tail_i + 20t`, which counts `#party <completion> ==
  <value>` into the template-owned `#camp_phase dw.sys` and starts the next
  phase's run. The template's single closing `await score #camp_phase dw.sys
  matches <n>` (timeout `100 + Σ(tail_i + 20)`) demands every phase's verdict —
  a missed ending leaves the count short and times out red, quantified over
  branches. Campaigns without
  `branch_points` are untouched by this shape.

### A body the story removes is never seen to die

Vanilla `/kill` on a living entity is a death — red flash, fall-over, death
particles, where the body stands. Every removal of a body the compiler placed (an
NPC's body and hitbox, an actor's puppet or twin, a wave's mobs) is built by one
function, `emit::removal_lines(ns, tag, declares_drops, exit)`, and the story
writes a death on screen in exactly one place: `despawn-actor` `style: kill`
(`Exit::OnScreen`, `kill @e[tag=<tag>]` in place). Every other removal —
`despawn-npc`, `despawn-actor` `vanish`, the puppet an `unleash` replaces, an
actor re-stand (`actor_restand_<id>`), a wave re-seat (`wave_reseat_<wave>`) —
is `Exit::Unseen`, four commands over `@e[tag=<tag>]`:

1. `execute if entity … run schedule function <ns>:unseen_sweep 5t replace` —
   only when a body is really leaving, and `replace`, so every body waits at
   least the full delay;
2. `execute as … on passengers run ride @s dismount` — a rider is set down,
   never carried out of the world;
3. `execute as … at @s run tp @s ~ -128 ~` — straight down the body's own column
   (`at @s`: a server-source `tp` resolves `~ ~` at world spawn), under the
   overworld's floor (`-64`) and not past vanilla's void line (below `-128`);
4. `execute as … run data merge entity @s {Tags:["dw_unseen"],NoGravity:1b,NoAI:1b,Silent:1b}`
   — every tag replaced, so from this command on no selector the datapack
   writes finds the body; frozen, so it takes no void damage.

`unseen_sweep` is `kill @e[tag=dw_unseen]`, emitted exactly when some function
schedules it. The 5-tick delay (`UNSEEN_DELAY_TICKS`) exceeds a living entity's
tracker update interval (at most 3 ticks), so a client is told where the body
went before any death packet is sent — the death plays at Y -128, out of every
player's sight. That interval figure and the client's handling of the move are
read from vanilla's behaviour, not measured here; what is measured is the
server side: the generated `v04_despawn_<id>` PackTests read the body by UUID
and prove it is not dying on the removal's tick nor 4 ticks later, and is at
Y ≤ -127 on the first tick it is. `tests/unseen_removal.rs` judges the shipped
function tree feature-blind: no line kills an NPC, actor, puppet or wave tag in
place except a `despawn-actor` `kill`.

**A generated PackTest never runs `unseen_sweep`.** The suite shares one world, so
the sweep a template ran would kill every body a sibling's removal parked in the
same batch, inside the delay the removal promises it — which is what
`v04_despawn_<id>` watches: on the pinned server, `kill_pays_removed_<f>`
templates that ran it killed `v04_despawn` bodies in the tick they were parked.
A template that needs a removal's death in the tick
it happens (`souls_unleash_yields_nothing`, `souls_reseat_yields_nothing`,
`kill_pays_removed_<f>`) drags the
bodies onto its own dummy before the removal and then kills the `dw_unseen`
bodies within 1 block of Y -128 in the dummy's column, and no others
(`emit::unseen_sweep_under`). `tests/unseen_removal.rs` refuses any template line
that runs the sweep.

### Semantics never key on player-facing text

**No semantic verdict may key on player-facing free text** (item/NPC display names,
titles, blurbs, hints). Semantics live only in ids, structured schema fields, or
first-class declarations. A check that read a kit item's display name for "night
vision" would pass `DW0210` on a renamed water bottle while nothing in the shipped
world grants night vision — a check that passes without the feature existing. Player-facing text is also localizable, so keying on it makes a
verdict language-dependent (ADR-0006).

### The completion-marker channel (the bot's oracle)

The critical-path bot's ONLY evidence that something completed is a chat line of
the anchored form

```
[dw:complete <campaign_id> <token>]
```

`<token>` is `campaign` (the whole delve, from `campaign-complete`), the
completing objective's own `obj/<kebab>` id (broadcast by `complete_o_<obj>`, as
the score flips, before that objective's effects run), or a fired environment
trigger's own `trigger/<kebab>` id (broadcast first thing in `trig_<id>` by every
trigger a path could perform — the proof a `trigger` step passes on). Both are `tellraw @a`,
dark-gray. The harness matches the **whole line**, exactly (`harness/src/markers.ts`
mirrors `plan::marker_line`) — never a substring of a longer line.

Why this shape. A substring match lets authored or translated content forge a
completion, and a `reach`/`interact`/`talk-to` step that passes on arrival or on
the dialogue opening passes while its own objective never completes. Three
properties make a forged completion impossible rather than merely unlikely:

1. player chat reaches a client as `<name> …`, so no player utterance can begin
   with the sigil;
2. the campaign id is part of the match, so a marker from other content cannot
   satisfy this campaign's step;
3. `DW0182` reserves the sigil in every player-visible string — authored English
   and every sidecar translation alike.

The harness side of the contract (`critical-path.json` `format_version` 2, the
per-step `objective` id, and the endgame rule that campaign completion belongs to
the last objective step) is described under "World / build output" below.

### A cutscene is pure observation (`dw_cutscene`)

While a cutscene plays, every player carries the entity tag `dw_cutscene` —
added by the cutscene `start` alongside `gamemode spectator @a`, removed by the
`end`/restore, so the state has exactly the cinematic's lifetime. **Campaign
machinery must neither require anything of a tagged player nor punish them:**
they are watching, not playing. Consumers:

- the **stealth judge** is skipped for them (`stealth_tick` selects
  `@a[tag=!dw_cutscene]`). The judge is the only writer of `dw.st_grace`, so
  skipping it freezes the clock — grace neither accrues nor expires, and
  `on_caught` cannot fire mid-cinematic. The restore deliberately leaves
  `dw.st_grace` alone: the beat resumes exactly where it paused (the judge is
  position-only, so there is no other stealth state to re-sync).
- **`damage-players`** skips them: every form of the verb is guarded by
  `tag=!dw_cutscene`.
- a **lethal volume** and a timed gate's **`crush`** skip them (`tag=!dw_cutscene`
  on the kill selector), and a **health bar** never adds them to its audience.

**A disconnect mid-cutscene must not strand the player.** The whole bracket is
`@a`-scoped, so `cs_end_<bare>` restores gamemode, teleports and untags exactly
*the players online when it ends*. A player who dropped during the shot is not
among them: they rejoin still tagged, still in spectator, and the marker they
would have been teleported to has already been killed — a ghost with no way back.
`join_place` cannot help, because it is gated on `dw_joined`, which survives a
relog exactly like the cutscene tag does. The repair is therefore its own `tick`
clause keyed on the **stuck state itself** — tagged while nothing is playing:

- the bracket refcounts itself on `#cs_live dw.sys` (`add 1` in `start`, *after*
  the re-entry `return fail` so a re-entrant start cannot inflate it; `remove 1`
  in `end`). A refcount, not a flag: nothing forbids two cutscenes overlapping,
  since each start only guards re-entry into itself. Never initialized, so the
  `unless … matches 1..` test reads correctly before the first cutscene runs.
- `execute unless score #cs_live dw.sys matches 1.. as @a[tag=dw_cutscene] run
  function <ns>:cs_repair` — a player tagged while a cutscene *is* playing is
  left alone, because `cs_end_` will collect them normally.
- `cs_repair` is strictly per-player (`@s`): `gamemode adventure`, drop the tag,
  and a macro `tp` to `storage dw:cp pos` (via `dw:cs at` + `cs_repair_tp`,
  the same shape the boundary return uses). The destination is the live
  checkpoint rather than the cutscene's own saved position because that marker
  is destroyed by `cs_end_` before this can ever run.

A cutscene-less campaign emits none of it (byte-identical).

**The `spectate` bounce is sneak-gated.** In spectator
mode the sneak key dismounts the spectated entity, so an unconditional per-tick
re-attach against a held key strobes: attach → client dismount → attach, every
tick. Both bounce lines therefore select
`@a[predicate=!<ns>:sneak_held]` — the vanilla `minecraft:player` `input`
sub-predicate (1.21.2+), which reads the client's raw input packet and so
reports the held key in every gamemode, spectator included. A player holding
sneak mid-cutscene settles into a stable detached spectator (frozen, staring at
the world — acceptable; strobing is not) and re-attaches on the first bounce
tick after release, resuming the shot. The predicate file
(`data/<ns>/predicate/sneak_held.json`) is emitted only for a campaign with at
least one cutscene; everything else stays byte-identical. This gate is also why
stealth never asks players to sneak: holding sneak and spectator cinematics
are inherently in conflict, so no delve mechanic may require a held sneak.

Any verb that *demands input* or *deals harm* joins this list.

**The party stays in the scene (spec-0095).** Spectator takes the bodies out of the world, so a cutscene declared `party: present` (the default) leaves a stand-in for each. Its `start`, before `gamemode spectator @a`, runs `execute as @a[tag=!dw_cutscene,gamemode=!spectator] at @s run function <ns>:cs_standin` — a player already watching (a respawn wait, another cutscene) or out of the body in spectator gets none — and claims the new stand-ins as `dw_standin_<bare>`. `cs_standin`, one shared function run as the player at the player, summons a `minecraft:mannequin` (`pose:"standing"`, `immovable`, `NoGravity`, `Invulnerable`, `Silent`, `hide_description`), turns it to the player's yaw with a level head, fills a `player_head` from the loot table `<ns>:standin_profile` (`fill_player_head`, entity `this`) into its head slot and copies the head's `minecraft:profile` onto its `profile` — the player's whole profile as the server holds it, signed `textures` property included, with no name typed and nothing fetched — then copies `armor.head`, `armor.chest`, `armor.legs`, `armor.feet`, `weapon.mainhand` and `weapon.offhand` from the player (`item replace … from entity @s …`; an empty slot copies as air). `cs_end_<bare>` removes them by the unseen exit (`removal_lines`, `Exit::Unseen`: down to y −128, swept five ticks later), so no death animation plays where the player has just returned; a killed mannequin drops none of its equipment [measured on the pinned server], so the copied gear never reaches the world. A stand-in is a **body**: it carries neither `dw_fixture` nor `dw_borne`. It stands — a crouch is not read. The `standin` PackTest runs the real `cs_standin` as its dummy and asserts the profile id is the dummy's UUID, the yaw is the dummy's, and the chestplate, the held item and the empty head slot are copied. The build proves the bracket over the shipped tree against the declarations (`DW0971`) and prints `stand-in binding: C cutscene(s) examined, P present (stand-ins placed and removed), A absent (none placed)`, writing `validation/stand-in-gate.json`. A campaign with no `present` cutscene emits no `cs_standin`, no loot table and no PackTest.

### Shot styles (`shot_style` — spec-0015, camera dossier §2)

A styled shot expands at compile time into the same dolly + aim geometry an
explicit `path` produces — a pure function of (style, params, subject
geometry). `dist` is the only "lens" control (vanilla has no in-game FOV);
durations default from the dossier's film-editing ranges; every expanded path
runs the same `DW0308` clip (authored + rendered chords) and `DW0347` angular
budget as a hand-authored one. Placement is rule-based (no world-aware
candidate scoring — dossier §4's compile-time ClearShot is not built);
`bearing` (compass degrees: 0 = camera south of subject, 90 = west) steers the
placement, and an explicit `path`/`look_at`/`seconds` overrides any part.
Entity subjects (`npc`/`actor`) aim one block above the feet cell (torso)
before `offset`; `anchor` subjects use the block centre exactly.

A `subject` is discriminated by its key — exactly one of `anchor` / `npc` /
`actor`, plus an optional `offset`, **and nothing else**. Each spelling is its own
`deny_unknown_fields` type (`Mark` / `NpcSubject` / `ActorSubject`), so
both serde and the exported JSON Schema (`additionalProperties: false`) reject a
typo'd key or a subject naming two discriminators at once with `DW0100`. An
untagged enum would instead silently *ignore* an unrecognised key: a mistyped
`ofset` would deserialize fine with the offset dropped, and
`{"anchor": …, "npc": …}` would quietly match the anchor and discard the npc —
shipping a shot framed somewhere the author never asked for.

| Style | Expansion (camera relative to subject S) | Aim | `dist` default | Default `seconds` | Notes |
|---|---|---|---|---|---|
| `insert` | Static at `dist` (3), +0.5 up | S | 3 | 2 | A prop, an inscription. Structurally judder-free. |
| `locked-off` | Static at `dist` (12) abeam the subject track's midpoint, +2 up | tracks S | 12 | 6 | Subject may be moving (aim pans) or static. |
| `push-in` | Dolly `dist` → `dist`/3 (min 2) along the bearing axis, +1 up | S | 12 | 4 | Dread; a line landing. |
| `pull-back-reveal` | Dolly `dist` → 4×`dist`, +1 up | S | 4 | 6 | "You are not alone." |
| `establishing-crane` | `dist`, +12 up → `dist`/2, +4 up (Δy −8) | S | 24 | 8 | First sight of an area. |
| `orbit-arc` | Arc of `degrees` (45–120, default 90) at radius `dist`, +2 up, from `bearing`; one waypoint per ≤10° | S | 12 | 8 | Constant angular speed via arc-length parameterization. |
| `side-track` | Per-tick camera = subject track + constant offset `dist` right of overall travel (`bearing` rotates it), +1 up — the Rockstar phantom-vehicle rig | tracks S | 8 | 8 | **Requires a moving subject** (`DW0349`); no easing — the subject's motion profile governs. |
| `two-shot` | Static on the AB perpendicular bisector nearest `bearing`, +1 up; d = (|AB|/2)/tan(α/2), α = 70°/3 (thirds framing), clamped 5–9; `dist` overrides | midpoint of A,B | Toric solve | 5 | Toric-space-inspired closed form (Lino & Christie, SIGGRAPH 2015 / SCA 2012 — ideas only). Needs `subject_b`. |
| `low-follow` | Per-tick camera = subject track + `dist` directly behind overall travel (`bearing` rotates), +0.5 up | tracks S | 4 | 5 | **Requires a moving subject** (`DW0349`). The dossier's worst-case style: the angular budget is the guard. |

### PackTest batch model (one dummy per test, one shared server)

PackTest runs the whole generated suite as **one batch on one shared server**:
every `# @dummy` test spawns its **own** dummy player, all dummies coexist, and
all test functions execute over the same server tick(s), sequentially in an
order the compiler does not control. The conversion is **total** and the rule is
hard: **every generated test is interleaving-independent — own dummy, own
scores, own init** (`pin_dummy` in `compiler/emit/packtest.rs`;
CI-enforced over every fixture family by `tests/packtest_batch.rs`):

- **Own dummy — `@p` is not "the test's player".** It re-resolves from the test
  structure origin on every command — the moment a template teleports its dummy
  to absolute campaign coordinates, `@p` retargets to a *neighbor test's* dummy
  and later writes/asserts land on the wrong player (`v06_stealth` read a
  foreign dummy's grace). A template that drives per-player state tags its
  dummy on its first post-setup line (`tag @p add dw_t_<test>` — while its own
  dummy, inside its own structure, is still the nearest player) and addresses
  it exclusively via `@a[tag=…,limit=1]`, which — unlike `@p` — also keeps
  matching a dummy that campaign content has killed. `@s` (the executing dummy)
  is equally safe — the binding survives teleports. Bare `@a` writes are
  forbidden: they hit every coexisting dummy.
- **Own scores.** Fake-player scratch holders on `dw.sys` are batch-global, so
  every template suffixes its own (`#n_sidm_<actor>`, `#bx_bret`,
  `#dm_<npc>_<node>`, …); no two templates share a holder. Real runtime scores
  (`#stealth`, `#placed`, `#trig_<id>`, the
  `#mt_`/`#at_`/`#arun_`/`#mgen_`/`#mown_`/`#agen_`/`#aown_` move drivers,
  `#lane_<wave>`) are deliberately shared — tests drive them and initialize them
  explicitly. The line between the two is **who writes the holder**: a name an
  emitted campaign function owns is runtime state no template can suffix, so the
  census answers `#wcen_n`/`#wcen_b`/`#wcen_d` (written by `wave_census_<wave>`),
  the wave recount's scratch `#wlive` (written by `tick`), the per-wave
  credited-kill ledger `#wcred_<wave>` (written by `setup`, `spawn_<wave>` and
  `k_reward_<wave>`), the `on_kill` payment ledgers `#kf_w_<wave>` /
  `#kf_a_<actor>` (written by `setup` and `on_kill_w_<wave>` /
  `on_kill_a_<actor>`, spec-0074) and the party-unique kit latches `#kit_<class>_<k>` (written by
  `class_apply_<class>`) are runtime, not scratch — a template can only drive or
  reset them. Where a runtime answer is *asserted*, the template copies it into
  its own scratch first (`#wcn_<n>_<wave>`) so the assertion reads a holder it
  owns.
- **Own scores, extended to party state (spec-0018).** Progression lives on
  the batch-global `#party` holder rather than on each test's dummy, so a
  template's baseline writes are visible to every sibling. Inside a template that
  is harmless — a template is one atomic mcfunction, so its baseline, its drive
  and its assert land in one tick with nothing in between. It stops being
  harmless the moment a template spans ticks: `party_state_across_ticks_is_owned`
  requires that any template containing an `await`/`schedule` be the **sole**
  template touching each `#party` score it uses (`sched_executor`'s probe flag is
  test-only for exactly this reason). That rule reads a template's own text, so it does not see a read the
  template makes in a helper it SCHEDULES: the branch-shape campaign template
  reads the completion score in `pt_camp_check_<i>`, 20 ticks after
  `pt_camp_run_<i>` drives the phase. `a_party_score_read_after_a_schedule_is_no_siblings`
  follows a template's helpers through `schedule function` and requires that
  every `#party` score read there is touched by no other template.
- **Own the gate you assert on (`DW0807`, `compiler::batchstate`).** The rule
  above binds a template to the scores it *writes*; a template is equally decided
  by the ones it only *reads*. A template that **drives** the outcome it asserts
  on — dispatches a campaign function which, transitively, writes a score the
  template asserts or awaits later in its own body — must therefore WRITE every
  `#party` term read by the gates on the path from that drive to the outcome.
  For a one-gate template those terms are `requires_flags`, `forbids_flags` and
  `requires_state` together, driven from the whole `Gate` through
  `packtest_gate_drive` rather than listed beside the template; for the
  campaign-playthrough template they are the whole party ledger, opened with
  `campaign_progression_baseline`. `tick` is one such campaign function and is
  not a special case — the campaign template calls `complete_o_*` directly and
  never ticks at all, is decided by a gate one dispatch below that in
  `check_q_<q>`, and asserts a score written two dispatches below in
  `campaign_complete`; a check reading `tick`'s own lines and one level of
  writes would see none of that. A template's own hoisted helpers (`pt_camp_drive`,
  `pt_camp_run_<i>`) are inlined before judging, because a baseline does not stop
  belonging to the template when the emitter moves it. All of it is decided at
  build time over the shipped bytes, not by a convention a future template can be
  written without.
- **Own members (spec-0018).** A division-of-labour template needs more than one
  player, and `# @dummy` gives exactly one. It spawns the rest itself
  (`/dummy <name> spawn`, PackTest's own command), addresses them by
  `@a[name=…,limit=1]` — as exclusive as a tag, and admitted alongside it by rule
  2 — and removes every one it spawned (`spawned_members_are_uniquely_named_and_removed`
  also checks the ≤16-char player-name limit and cross-template name uniqueness).
- **Own init.** "Never set" is not 0 and "fresh world" does not exist here:
  every score a template asserts on is actively initialized by that template
  (`packtest_preamble` with `with_flags: false` clears withheld flags to 0),
  and every entity tag it counts on is cleared on entry. Sibling residue is
  real: `v06_unleash`'s leftover real-AI twin carried `dw_actor_<id>` with no
  puppet marker, so `v06_spawn_idempotent`'s guarded spawns
  (`unless entity @e[tag=dw_actor_<id>]`) no-op'd and it counted 0 puppets —
  a pass/fail decided purely by batch order on byte-identical packs. Templates
  also leave no residue of their own (actor tests kill the actor tag on exit),
  and templates that re-run the unguarded `setup_finish` clear every planned
  NPC tag first (its summons would otherwise duplicate bodies + hitboxes).
  Each template is a single mcfunction and therefore atomic — nothing can
  interleave *within* it; these rules make the boundaries between templates
  order-free.
- **Division of labour is not simulable with one dummy.** A single-dummy test of
  an AND-join proves only that one player can do both arms in sequence. The
  generated
  `party_join_<obj>` template therefore drives **n different players**, one arm
  each, and asserts the join's REAL emitted `pending_guard` (materialized into
  `#pj_<obj> dw.sys`) in three phases: shut with no arm, **still shut after only
  one** (the negative half that makes it an AND, not an OR), open after all of
  them — and then has the LAST member, never the one who cleared the first arm,
  complete the successor. n = the join's arm count, raised to `world.min_players`
  and capped at 4 (the party maximum); arms are handed out round-robin.
- **`assert` does not abort the template, and the log names the LAST failing
  line.** Measured while proving a seal by mutation: a
  template with two failing asserts reported only the second, and flipping the
  first assert's expectation changed nothing about which line was reported. So
  the reported line is *a* failure, never "the first thing that broke" — read
  the whole template, and never conclude an earlier assert passed because the
  log did not name it. It also means every later assert still runs against
  post-failure state.
- **Drive the real mechanism, not a convenient stand-in.** A template that calls
  a *scheduled* driver inline (`function <ns>:mv_tick_<key>`) runs it **as its own
  dummy** — supplying exactly the executor the vanilla scheduler withholds, so the
  test passes while the shipped delve soft-locks (§4 "A scheduled bundle
  has no `@s`"). Tests of scheduled machinery hand it to `schedule` and `await` the
  outcome (`sched_executor`, `sched_arrive_flag`); the
  `v06_move_actor`/`v06_arrive_handoff` inline drives are entity-state
  assertions, which is all they claim.

### Determinism (ADR-0006)

- Same DSL + seed (+ `--lang`) → byte-identical `<out>/` tree. Gated by the
  double-build test (`tests/cli.rs`).
- All map/set iteration `BTreeMap`/explicit sort; JSON is `serde_json` pretty
  (sorted keys) + trailing newline.
- **No** wall-clock, hostname, locale, or absolute build path in any output byte.
- **No ambient state**: every emitted byte is a function of the campaign
  directory, the prefab directory and the flags named on the command line. The
  compiler reads nothing from above those paths and nothing from the working
  directory, so the same build from the engine checkout, from a content checkout
  and from a scratch directory produces the same tree. Gated by
  `tests/cli.rs::the_working_directory_cannot_reach_the_build`, which is the
  perturbation the double-build test cannot make — both of its builds share one
  working directory, so a value read out of the filesystem above the inputs is
  invisible to it.
- Only randomness = stage-1 `seed` → named splitmix64 per-area streams. Solver
  retry (≤32 attempts) is seed-deterministic; attempt 0 reproduces pre-M2 growth.
- **Parallel work merges in a fixed order.** Every multi-core site goes through
  `delvec::par` (`map`, and `try_for_each_ordered` for large results, which
  holds at most `2 × threads` results at once). Each result is computed from
  its own item alone and the results are folded sequentially in item order, so
  the output is the sequential loop's at any thread count. The sites: each
  placed template's decode (`assembled::placed_blocks`, applied in template
  order — a later template still wins a cell), its size (`DW0803`, judged in
  placement order), its placement sentinel, `burial`'s per-piece sides,
  `seating`'s and `settling::piece_bytes`'s per-template reads (the first
  failing template is the one named), the valley horizon's tile stacks (built
  per footprint, numbered `horizon/valley/t{n}` in footprint order), and the
  `DW0921` proof's quest configurations (counts summed and pockets listed in
  configuration order). Gated by `tests/thread_count.rs`, which builds one
  subject at `DELVEC_THREADS=1`, `3` and unset and compares trees and stderr.
- **A block state is interned, and reads as its text.** The assembled world's
  map holds `blockstate::BlockState` handles (one interned copy per distinct
  state for the life of the process); equality, order, hashing and both
  formats are the text's, so interning moves no comparison, sort or message.

### Environment sealing (bootstrap `#minecraft:load`, idempotent, `#init`-guarded)

**1.21.11 renamed every gamerule** (verified live 2026-07-30; legacy camelCase
and `minecraft:`-prefixed forms both rejected). Emitted sealing commands
(`emit::sealing_commands`):

| Legacy (spec text) | 1.21.11 accepted (emitted) |
|--------------------|----------------------------|
| `doMobSpawning` | `gamerule spawn_mobs false` |
| `doDaylightCycle` | `gamerule advance_time false` |
| `doWeatherCycle` | `gamerule advance_weather false` |
| `doFireTick` | `gamerule fire_spread_radius_around_player 0` (no boolean successor; radius 0 = no spread) |
| `mobGriefing` | `gamerule mob_griefing false` |
| `spawnRadius` | `gamerule respawn_radius 0` (spawn **scatter** off — vanilla otherwise scatters a first join / spawnpoint-less respawn uniformly in a square of this radius around world spawn; every scattered cell in a box garden is solid prefab or void, so the only correct radius is the exact compiler-chosen anchor) |
| — | `gamerule keep_inventory true` (box-garden death policy; **not in spec-0002** — see §6) |
| — | `gamerule send_command_feedback false` (the delve's own machinery must not narrate itself: a dialogue option is a `/trigger`, and vanilla would answer it in chat) |
| — | `gamerule tnt_explodes false` (spec-0011): defense-in-depth against a stray primed-TNT source deforming the sealed world. No gamerule separates explosion block vs. entity damage, so TNT is excluded as a trap payload by the schema and belt-and-braces sealed here. |
| — | `time set <kw|ticks>` (declared `world.time`, required, through `WorldTime::token`: a keyword on day 0 its table argument, `noon` = daytime 6000; a celestial world the integer `day × 24000 + daytime`, `{"moon": "just-risen", "phase": "new-moon"}` = `time set 108959`; the sole seal with a vanilla read-back) |
| — | `weather <kw>` (declared `world.weather`, **required**; the declared state is emitted, whatever it is — a campaign that declares `clear` emits `weather clear`) |
| — | `difficulty <kw>` (declared `world.difficulty`; emitted **only when declared**). The shipped `server/server.properties` already carries it, so this line is not what makes the delve *image* correct — it is what makes the DATAPACK correct wherever else it is loaded (the owner's own test save, a world whose properties someone edited). `/difficulty` is idempotent. |

- Gamerule *values* have no vanilla read-back → asserted at compile time only;
  PackTest asserts the two queryable seals: `time = daytime_ticks(world.time)`
  (e.g. 6000 for `noon`, 18000 for `midnight`) — and, for a **celestial**
  world, the second read-back `time query day` equal to the clock's day and,
  where its moon is up, `execute store success … if predicate <ns>:moon_<phase>`
  over a `minecraft:time_check` predicate the compiler emits at
  `packtest-datapack/data/<ns>/predicate/moon_<phase>.json` (`period` 192000, the
  phase's 24000-tick window), so the phase the creator wrote is proven on the
  server; a keyword world's `sealed_state` is unchanged — and, for a campaign that declares
  one, `difficulty = WorldDifficulty::id()` via the bare `/difficulty` query
  command, which vanilla answers with `Difficulty#getId()`. Regression asserts
  exact forms and that legacy names never appear.
- Time/weather freeze: cycles are frozen (`advance_time`/`advance_weather false`);
  a set state persists until the next explicit set. Stage-1 `time`/`weather` +
  `set-time`/`set-weather` (spec-0010) make these first-class. The assembled-light
  model judges sky-open cells under the **darkest reachable (time, weather)**
  combination (initial ∪ every `set-time`/`set-weather` target).

### World / build output

- `server/server.properties`: `level-type=minecraft:flat` +
  `generator-settings={"biome":"<ns>:void","layers":[]}` (see *The ground
  biome* below),
  `level-seed=<seed>`, `gamemode=adventure`. `difficulty` = the campaign's
  declared `world.difficulty` when it declares one, else the derivation:
  `peaceful` for wave-free campaigns, **`easy`** when any wave exists (peaceful
  removes summoned mobs). No server jar, no region files (ADR-0010) — the
  bootstrap `/place template`s prefabs, so byte-identity covers the whole tree.
  `view-distance=<the campaign's, floor 10>` and `simulation-distance=10` (below).
  The file writes 15 keys and the key set is asserted exactly
  (`crates/delvec/tests/server_properties.rs`): what a delve pins and what it
  leaves to the host is a reviewed decision, not a residue.
- **Chunk distances.** `view-distance=<n>` — the campaign's `world.view_distance`
  (spec-0091), or the engine's floor **10** (160-block radius) when it declares
  none — and `simulation-distance=10`, two keys answering two questions. *View*:
  the floor reaches the far side of the largest shipped scene (114 × 165 blocks,
  measured from the emitted piece `forceload` AABBs) from any standpoint inside
  it, and is the radius `docs/notes/horizon-library-dossier.md` §3–4 and
  spec-0026 §6 do their vista arithmetic against; a campaign whose far views
  need more declares more, every far view is judged against what is served
  (`DW0956`), and the build states the cost. *Simulation*: not what makes a
  delve tick — `setup` force-loads every placed piece and never releases it, so
  scene chunks are entity-ticking wherever the party stands, and everything
  beyond them is inert backdrop. Its job is to make the ticking rim a **known**
  radius: pinned, the chunks that can tick are bounded by the force-loaded scene
  ∪ Chebyshev radius 10 (+1 loading margin) around any player — a bound a
  whole-plane proof can be written against — which is why it never moves with
  the view distance: ticking farther renders nothing.
- **`server/resources.properties`** (spec-0091 §4): what the delve asks of its
  host, computed from the declared view distance at the player cap and read by
  the image's entrypoint and the playtest server as the JVM's `MAX_MEMORY`
  unless an operator names one. `heap-max=<size>` is `versions.toml`
  `[server].heap_max` (the floor, which covers the scene and the floor's view)
  plus twice the fitted live-heap increment of the declared distance over the
  floor, rounded up to whole GiB; `players=4`. The model is
  `compiler::served` — chunks sent per client are the pinned server's own set
  (`(max(0,|a|−2))² + (max(0,|b|−2))² < n²`), the live heap is linear in them —
  fitted to `crates/delvec/tests/measured/view-distance.json` and held equal to it
  by `served::tests`. `server/README.md` states the declaration, the client
  render-distance floor a player sets (the declared number), and the heap.
- **Unpinned keys.** The pinned server version writes 70 properties; the build
  pins 15. Every other key is left at the host default *on purpose*, and the
  distinction that matters is that a delve has two boot paths with two different
  default sources: the shipped image starts from the itzg base's own
  `/image/server.properties` template, the owner's playtest server
  (`tools/creator/playtest-server.sh`, `OVERRIDE_SERVER_PROPERTIES=false`) copies the
  build's file in and lets the vanilla jar fill the rest. Measured on both paths
  against the pinned version, the 55 unpinned keys resolve as follows.
  - **Diverge between the paths, harmless**: `enable-rcon` (image on, playtest
    server off until `playtest-server.sh` appends its own), `rcon.password` and
    `management-server-secret` (both generated per boot). Operator transport,
    never world state; `management-server-enabled=false` on both, so the secret
    is inert. `enable-status` and `hide-online-players` join them, and only on
    one path: with `DELVE_RESET_WHEN_EMPTY` set (spec-0064) the delve entrypoint
    exports `ENABLE_STATUS=true` and `HIDE_ONLINE_PLAYERS=false`, because the
    reset's whole event is the online player count read from a server-list ping,
    and a host that hid its player list would turn every reading into a failed
    ping, every failed ping into "one player", and the flag into a flag that does
    nothing. Neither key enters the pinned set: they are operator transport, not
    world state, which is the line this paragraph already draws — with the flag
    unset the entrypoint sets neither and the host decides both, as today.
  - **Agree, and load-bearing**: `function-permission-level=2` (the level every
    datapack command in the pack runs at — a host that lowered it would break the
    whole bootstrap), `initial-enabled-packs=vanilla` (an enabled experimental
    feature pack changes worldgen and content), `entity-broadcast-range-percentage=100`
    (how far NPCs and markers are sent to a client), `hardcore=false`,
    `allow-flight=false`. Safe at their defaults today and the first place to
    look if a host ever renders or ticks a delve differently.
  - **Agree, product-shaped**: `max-players=20` against a 1–4 player delve
    (`world.min_players` states the floor, nothing states the ceiling), `motd`,
    `white-list=false`, `player-idle-timeout=0`, `pause-when-empty-seconds=60`
    (an empty server stops ticking, including force-loaded chunks, and resumes on
    join — desirable for a delve).
  - **Agree, irrelevant to a delve**: `max-world-size` and
    `max-chained-neighbor-updates` (the boundary region and command-driven traps
    make both moot), `region-file-compression` (changes generated region bytes,
    which are never shipped or compared), `sync-chunk-writes`, `max-tick-time`,
    and the remaining transport/status keys.
- **The resource pack is served, never installed** (spec-0084 §11, as spec-0009
  and spec-0024 decided the serving). `require-resource-pack=true` is written
  only when the campaign declares `require_resource_pack`. Every other
  resource-pack key is set where the server starts, because only that place
  knows an address the client can reach: the shared entrypoint
  (`validation/world-settings-entrypoint.sh`, byte-identical to the
  `Dockerfile.delve` heredoc) obeys an operator's `RESOURCE_PACK` as given — a
  release bakes the GitHub Release asset URL and its SHA-1 — and otherwise
  applies `DELVE_RESOURCE_PACK_URL` when the build's `manifest.json` records a
  pack, with that manifest's `resource_pack_sha1`, defaults
  `RESOURCE_PACK_PROMPT` to a JSON text component, and turns the file's
  `require-resource-pack=true` into `RESOURCE_PACK_ENFORCE=TRUE` unless the
  operator named it. The validation compose serves the build output from a
  `pack` sidecar (busybox httpd, `versions.toml` `[images.pack_server]`) at
  `http://pack:8000/resourcepack.zip`; `tools/creator/playtest-server.sh`
  serves it from `<name>-pack` on `127.0.0.1:25580` and writes
  `resource-pack`, `resource-pack-sha1` and `resource-pack-prompt` into the
  staged file (it runs with `OVERRIDE_SERVER_PROPERTIES=false`, under which
  itzg applies none of its variables). `validation/owner-play.yaml` does not
  yet publish the sidecar on the loopback (`tools/ci/check-compose-isolation.py`
  admits one fixed binding there), so a host client joining through that hand
  path is pushed an address it cannot reach.
- **The ground biome** (`horizon::ground_biome`). A declared weather is the
  weather the party stands in: vanilla draws rain, counts a body as in rain and
  aims lightning only where the biome at the cell precipitates, and vanilla's
  `minecraft:the_void` does not. So a non-`ocean` delve's generator lays the
  delve's own biome, `<ns>:void`, emitted at
  `datapack/data/<ns>/worldgen/biome/void.json`: `minecraft:the_void` from the
  pinned jar field for field — sky `#7ba4ff`, water `#3f76e4`, no fog, grass or
  foliage override, temperature 0.5 and downfall 0.5, no carvers, no spawners,
  the one `void_start_platform` feature — except `has_precipitation: true`
  (`horizon::VOID_BIOME_PRECIPITATES`, which `DW0496` reads too). At temperature
  0.5 what falls inside the build height is rain, never snow. It also joins the
  one vanilla tag `the_void` belongs to,
  `#minecraft:without_wandering_trader_spawns`
  (`datapack/data/minecraft/tags/worldgen/biome/`). The datapack is in the
  world's `datapacks/` before first boot on every boot path, so the biome exists
  when the world is generated. `minecraft:river`, the vanilla biome closest to
  it, is not a substitute: it carries monster, water and ambient spawners,
  carvers, ore and lake features, underwater music and ten biome-tag memberships (`the_void` has one).
- **Atmosphere biomes** (spec-0080). Every `world.atmospheres[]` entry ships as
  `datapack/data/<ns>/worldgen/biome/atmosphere/<kebab>.json`, biome id
  `<ns>:atmosphere/<kebab>`: the void definition above with `features: []` (a
  painted biome never generates), the derived `has_precipitation` /
  `temperature` / `downfall`, `effects` = `water_color` `#3f76e4` overlaid by the
  declared `tint`, and every attribute under its `minecraft:` id in canonical
  form (colours lower-cased, floats through `f64`, so `1` and `1.0` emit one
  byte string). Each joins the void biome's tag file. Every declared atmosphere
  is emitted whether a place carries it or only a beat paints it, because the
  biome registry closes when the world opens. A campaign with none emits
  byte-identically.
- **The biome map** (`horizon::biome_map`, spec-0080 §4) — the one answer to
  which biome a cell stands in: the valley's bands (spec-0026, outside the map
  by construction), then every carried place in declaration order (areas, or
  site-plan boxes), over the ground biome; a later paint wins a cell, as a later
  `fillbiome` does. It is the only reader of the ground biome and the bands: the
  bootstrap pass, the generator settings, the biome files and `DW0496` all read
  it, and it answers precipitation for a vanilla biome, a declared atmosphere or
  the ground through one method. A cell is answered by the 4-cell it lies in;
  vanilla's own reading (`BiomeManager.getBiome`) jitters the sample, so a block
  within two of a 4-cell face may read its neighbour, and a block whose
  coordinates are all 2 mod 4 always reads its own cell — where the generated
  PackTests sample. `setup_finish` paints the bands (cap raised and restored, as
  before), then calls `atmosphere_bootstrap`, which paints each carried place's
  paint (*The blend*, below) through `fillbiome_lines`. Generated PackTest `atmosphere_places` first
  re-establishes the first tick (the ground over every repaint volume, then the bootstrap), then reads
  each carried place inside (`execute if biome`) and just outside (`execute
  unless biome`), at a cell in no repaint volume and in no other place's paint under the same sky. Every build prints `atmosphere binding: A declared; P of N
  place(s) carry one; R repaint effect(s) over V volume(s); Q quart cells
  painted at bootstrap of M paint(s) in the map; B biome file(s) emitted` —
  hello-world prints a measured zero.
- **The blend** (spec-0080 §2.2; `horizon::BLEND_REACH`, `place_paint`,
  `camera_weight`). Read from the pinned 1.21.11 client through Mojang's
  official mappings: the camera does not read a spatially interpolated
  attribute from the biome it stands in. `EnvironmentAttributeProbe.tick`
  hands `position.scale(0.25)` to `GaussianSampler.sample`, which weighs the
  6×6×6 4-cells from `floor(q − 0.5) − 2` to `+ 3` on each axis by the kernel
  `[0, 1, 4, 6, 4, 1, 0]`, lerped by the fraction, and
  `SpatialAttributeInterpolator` averages each biome's value by its weight —
  a linear mean for a distance. A camera in 4-cell `Q` reads `Q − 3 ..= Q + 3`:
  twelve blocks on every side, up and down included. Every `visual/`
  attribute is interpolated except `moon_phase`, `default_dripstone_particle`
  and `ambient_particles`; the `audio/` ones and those three are read from the
  one 4-cell the camera is in. The unpainted side's fog end is the attribute's
  default, 1024, so fog is the most fragile: a place painted over its own
  play space only — The Threshold's first build, a 16×20 hall eight blocks
  tall — gave a standing eye at most 55.8% of its atmosphere, a fog end of 467
  where 26 was declared, and its walker saw the sky, the stars, the ash and
  the silence change and no fog. Measured by three instruments that share no
  code: the client's own `GaussianSampler`, `SpatialAttributeInterpolator`
  and `EnvironmentAttributeMap` called from the client jar on a JVM, a
  Python transcription of the bytecode, and the port
  (`horizon::tests::the_camera_reads_what_the_pinned_client_reads` holds the
  client's own readings: 0.558105468750 for that hall, 0.994062500000 at the
  centre of the eldritch spike's approved 32×40×40 slab). So **a carried
  place is painted as far as a camera inside it reads**: its own 4-cells
  grown by `BLEND_REACH` (12) on every face, bounded up and down by the build
  height; sideways by the place's claim, the columns world setup
  force-loads for it: for a site-plan box, the plan's `region` and the chunks
  its own columns stand in (force-loaded with the piece that holds them); for
  an area, `horizon::area_claim`, its own 4-cells grown by the reach on every
  side, which world setup force-loads with the area's pieces (`forceload add`
  in `setup`, each claim chunk no piece covers waited for by an `execute if
  loaded` in `place_verify` and never released) and the map's extent counts,
  for every area a paint reaches (carried, or named by a `set-atmosphere`
  `place`) on a campaign without a site plan; and toward every
  other place under a different sky (an uncarried place stands under the
  horizon's) short of that place's own 4-cells grown by the same reach, along
  the axis that separates the two (`y`, then `x`, then `z`). It never cuts
  into the place's own bounds. Each place's paint is computed from the
  others' own cells, never their paints, so declaration order decides
  nothing. A `set-atmosphere` with `place` repaints the same cells, held back
  only from places whose declared sky differs from the one it paints; one
  with `region` paints exactly the volume the creator sized. What still
  mixes is what the rule cannot reach: the cells within twelve blocks of a
  neighbour under another sky (the gradient across a threshold), and a place
  too narrow for the kernel. Every build that carries one prints, per carried
  place, `atmosphere reach: <place> — W of E standing eye(s) read <biome>
  whole; the best reads B% of it, the worst L%` (through `camera_weight`, each
  eye at the block centre, 1.62 over its floor): a site-plan box stands one
  eye per footprint column over its floor; an area, which states no floor,
  stands one at every cell of the party walk
  (`World::reachable_walkable_rooted` from every anchor) inside its placed
  bounds. The line is a measurement and
  refuses nothing.
- **The cut in time** (spec-0080 §2.2, measured on The Thing Beyond the
  Fog). A repaint reaches a connected client as one `chunk_biomes` packet per
  chunk and no chunk reload (the eldritch spike's reading). The client then
  takes the new value on its next tick: `EnvironmentAttributeProbe$ValueProbe`
  keeps the previous tick's value and the current one, and between them
  applies the attribute type's partial-tick lerp, so a camera whose kernel the
  repaint covers whole moves from the old fog to the new in one client tick
  (50 ms), linearly. Read through the client's own classes — its
  `GaussianSampler`, `SpatialAttributeInterpolator`, `EnvironmentAttributeMap`
  and the `fog_end_distance` type's partial-tick `LerpFunction` — at the demo
  level's camera: the kernel reads the repainted volume at weight 1.0, and
  `fog_end` steps 32 → 280 → 528 → 776 → 1024 across the four quarters of that
  one tick. On top of the attribute, `AtmosphericFogEnvironment.setupFog`
  draws the fog from `fog_start − 160·m` to `max(min(96, fog_end), fog_end −
  256·m)`, where `m` is the rain fog multiplier: it eases toward `rain level ×
  clamp((sky light − 8) / 7) × (1 if the biome has precipitation else 0.5)` by
  a fifth of the gap per tick, so two atmospheres that differ in
  `precipitation` also move `m` over about a second. A creator who wants a
  flash rather than a fade repaints between two atmospheres with the same
  `precipitation`, over a volume that holds the camera's whole kernel.
- `horizon:"ocean"` (spec-0013) swaps `generator-settings` for a pinned
  superflat `{"biome":"minecraft:ocean","layers":[bedrock×1, stone×118,
  water×8]}`: from the −64 build floor the top water block lands at **y=62** (sea
  level). Still no structures/mobs (`generate-structures=false` + gamerule
  `spawn_mobs false`).
- **The ocean datum is a WALK-PLANE datum, and an area's origin is derived from
  it** (spec-0060 §3). An ocean world's walk plane is
  `SEA_LEVEL + 1` = **y=63** (`horizon::OCEAN_WALK_REF_Y`) — one block above the
  water, the vanilla-normal beach relationship a swimming body can climb out
  onto. An area's origin is then `walk_ref_y − walk_y`, where `walk_y` is the
  piece set's own declared walk plane: a keep interior declaring `1` is seated at
  62 and stands dry at 63; an island piece declaring `3` is seated at 60. The
  derivation is per AREA (`plan::area_base_y`), never per world.
- **No world constant seats a piece.** A tileset's walk-plane convention is a
  fact about that tileset, so it lives in its pieces (`walk_y`, below).
  `SEA_LEVEL` is a world constant because the sea plane is a property of the
  horizon.
- **`valley`'s gap floor is a WALK-PLANE datum too**, `VALLEY_WALK_REF_Y` =
  `VALLEY_GAP_FLOOR_TOP_Y + 1` = **y=64** — the y a body's feet occupy standing
  on the valley's own ground — so an area's origin there is `64 − walk_y` on the
  same rule the ocean uses. A base has a walk-plane datum exactly when it has
  ground of its own outside the piece. Seated at `BASE_Y` instead, a piece would
  stand `walk_y` courses proud of the gap floor, and the library declares
  `walk_y` ≥ 1, never 0. For a SITE that difference is the whole thing:
  seated by its walk plane the bank runs into the gap floor and the party walks
  from one onto the other, and seated on `BASE_Y` it is a cliff they drop off and
  cannot climb back.
- **`void` keeps its ORIGIN datum**, `BASE_Y` = 64 (`horizon::walk_ref_y`
  returns `None` for it alone). It declares nothing outside the placed geometry,
  so there is no outside relationship to hold a walk plane against.
- **What a piece owes** (spec-0060 §4), all three in prefab metadata, all three
  different claims about the same object rather than one claim written three
  ways:
  - **`walk_y`** — the local y of the cell a body's feet occupy on the piece's
    principal floor. Owed on **every** base. It has **no default**, because a
    default is the retired global datum wearing a different name. It is a
    MEASUREMENT of the piece, written by the generator that built it
    (`prefab_invariants::walkplane::measure_walk_y`, and
    `grammar::export::measured_walk_y` for the grammar back end — one rule, the
    lowest local y holding a standable cell, read through the engine's own
    standable predicate). A piece a campaign seats without one is `DW0886`.
  - **`waterline_y`** — the local y of the piece's top authored `minecraft:water`
    block. Owed only where the piece really writes water that meets a sea. It is
    a claim about the bytes and is checked against them (`DW0887`); its
    placement is checked against sea level (`DW0344`, first arm). A piece that
    authors no water has no waterline to state.
  - **`shown_faces`** — which sides are finished exterior surface (`DW0885`).
    Absent means none is, which is the strict answer. For a piece a grammar
    program produced it is **written by the program**, not typed here: `delvec
    grammar expand` rewrites the metadata on every run, so a hand-written value
    survives until the next expansion and no longer, and the program is where a
    building is said (`grammar.md` §2f, program version 1.9.0).
  A piece declares **nothing** about which horizon it is for: which bases its
  declarations admit is derived (`DW0886`), never typed. A `horizon:` field in
  prefab metadata would be a design decision about what the piece is for,
  encoded in a mechanism.
- The world ocean is **not** backdrop: `/place template` carries the fluid
  already in a cell onto the block it writes there, so a waterloggable block
  placed below the sea plane comes out `waterlogged=true` whatever the prefab
  said, and a waterlogged cell is a water source. That is what `DW0851`
  measures; the walk plane one block above the sea is what keeps a shore clear
  of it.
- **`delvec schema --stage prefab-metadata`** exports the prefab document's
  shape. It is deliberately absent from `--stage all`: the gallery's coverage
  gate enumerates its units from that export, and a library-asset document
  folded into it would demand a stage-document binding for every field of a file
  no stage document contains.
- **`delvec prefab seating --horizon <base>`** answers the pairing over a whole
  library before a campaign is authored, per pool, with a numerator and a
  denominator at pool, member and declaration level, and under `--json` as one
  object carrying the per-pool verdicts, every reason with its `code` and its
  `shape`, and every binding count. It asks the SET's own question as well as
  each member's — one origin per area, derived from one walk plane — because a
  pool every member of which is individually seatable is still unseatable when
  they disagree. `delvec prefab audit` runs `DW0887` and `DW0888` over a whole library when
  handed a directory, and over one asset when handed a `.nbt` or a tile-set
  manifest, which is the form every gate and the admission procedure actually
  use. All of them run `compiler::seating`, the implementation the compiler's own
  validation and its origin derivation run, so a library can never be seatable
  according to the tool and refused by the build.
  That includes the exposure question `DW0885` asks at build (`DW0886`'s
  *outside unanswered* shape, below). `tools/ci/check-seating-agrees.py` is where
  that sentence is a check: it enumerates every pool of a library against every
  base the schema declares, compares the command's verdict with `analyze` and,
  for a seatable pool, with the build's exposure check, and perturbs toward the
  disagreement shapes.
- **`delvec prefab planes <asset> [--write]`** measures a piece's own `walk_y`
  and, where it authors water, its own `waterline_y`, and declares them. Both are
  measurements of the bytes and every generator writes them by reading them back
  off the blocks it just laid; this is that measurement for a piece **no
  generator wrote** — an ingested hero asset, a hand-authored room — which
  otherwise had no way to state a `walk_y` except by typing one. It is the
  `lighting` verb's shape: it prints the measurement with its binding counts
  always, and `--write` persists it into the metadata beside the asset, refusing
  when there is no document there to write into (`DW0753`) and when the piece has
  no standable cell at all. A tile set is measured as one assembled building.
- Prefab metadata's `lighting.profile` takes a fourth value, **`unmeasured`**
  (spec-0027 §2): a *generated* prefab places blocks, not photons, so it declares
  that a probe is owed rather than fabricating one. It is distinct from an absent
  `lighting` block (which declares no lighting), and the
  measurement fields stay mandatory where they are claimed: a `lit`/`dim`/`dark`
  profile without `measured_min_light` + `measured` is refused at parse
  (surfacing as `DW0346` for a library file), and an `unmeasured` profile
  carrying either is refused too. Nothing gates on the profile (`DW0210` measures
  the assembled world); its one consumer is the interior shot's reviewer line,
  where `unmeasured` reads "verify readability", never "mitigation expected".
- `boundary` (spec-0013) emits, in `setup_finish`: a `dw:region bounds`
  storage mirror (readable region contract), a `dw:cp pos` init to the spawn cell
  (shared with spec-0012 checkpoints — the last-checkpoint mirror the return
  reads; idempotent, gated once via `needs_cp_init`), and `schedule function
  <ns>:boundary_tick 20t`. `boundary_tick` (self-rescheduling 1s clock) ejects
  every `@a[tag=!dw_cutscene,tag=!dw_free]` outside the region via
  `boundary_return` (a macro `$tp @s $(x) $(y) $(z)` off `dw:cp`, + actionbar
  message + soft sound): a cutscene is pure observation, and a creator out of the
  body is not fought by the player bound. `boundary_exempt_cutscene` and
  `boundary_exempt_free` PackTests hold each tag on its own. The region selector
  is compile-time-derived literals; nothing is authored. With `returns: false` the
  bounds are still written and no clock is scheduled, and the `v06_boundary_*`
  templates are not emitted.
- **Entry point.** One cell per **area** — the cell a body arrives at when it
  enters that area. It is the anchor whose prefab metadata declares
  `"role": "entry"`. A campaign addresses every other anchor by name; this is the
  one the compiler has to *find*, so the piece says what it is for rather than
  being recognised by what it is called.
  The role is the **whole** of it: an anchor's name is never consulted, so no
  spelling supplies an entry point and a piece cannot acquire the campaign's
  start by calling one of its anchors `entry` for its own reasons. Every producer
  writes the role where it writes the anchor — `"role": "entry"` on a `mark` for
  a grammar program (`grammar.md` §2b), `delvec prefab anchor --role entry` for a
  hand-built or ingested piece, and its own site-plan anchor for a derived area.
  Two anchors in one area declaring the role is `DW0804`, whose remedy is
  `delvec prefab anchor --no-role` (or dropping `role` from the `mark`) on the one
  that is not the arrival cell.
  The **campaign's** entry point is the first area that resolves one, and drives
  `setworldspawn`, the `class_apply_*` teleport, first-join placement, the
  `dw:cp` seed and the gate-deadlock proof's start node. Resolving **nothing** in
  **any** area is `DW0345`, and it is checked before any model is built — a world
  with no start does not have a walking problem, and reporting it as one
  (`DW0311` over a crossing nothing was meant to walk) sends a reader looking for
  a wedged doorway.
  Every consumer goes through one resolver — `AnchorTable::entry_anchor` /
  `Plan::entry_point` / `Plan::entry_point_facing` for one area's,
  `AnchorTable::entry_anchor_name` where the answer is needed as a name (the
  gate-deadlock proof reads its start node out of prefab metadata),
  `Plan::entry_points` for the whole start set — and no consumer matches a name
  itself. Besides the campaign-level uses above, the per-area entry point is what
  **inter-area transport** carries the party to, what the **POV shot planner**
  frames, and what the **trap-safety proof** counts as a place a player can start
  from.
- **Crossings, and what a leg is.** A **leg** is a move from where the party
  stands to where the next critical objective stands, and the first leg begins at
  the campaign's entry point — the party is standing there when the delve starts.
  `plan::build_critical_path` enumerates that population once, and it is the
  population `DW0311` walks.
  A leg whose two ends are in different areas is a **crossing**, never a walk:
  areas sit `plan::AREA_SPACING` (256) blocks apart across void with no walkable
  link. The compiler emits it as a one-way teleport fired by the completion of
  the objective the party leaves from — `Plan::transport`, keyed by that
  objective — and it is one-way by construction: nothing carries the party back.
  A crossing needs two things, and the build refuses rather than degrading into a
  walk judgement when either is absent. It needs somewhere to arrive: the
  destination area's own entry point, or `DW0872`. And it needs something to ride:
  a completed objective at the leg's origin, which the spawn cannot supply, so a
  campaign whose first beat is in another area is `DW0873`. **The practical rule
  an author needs is therefore that the campaign's first beat plays in the area
  the party starts in**, and every later beat may be anywhere that declares an
  entry point.
  A **link** is the other carry (spec-0083, effect `teleport`): an authored
  `teleport` in a repeatable trigger, within one area, which the route proof
  takes only where a walk fails. It rides on a press the path performs — a
  `trigger` step — so the leg into it ends at its stand cell and the party
  arrives at the link's own `to`, a visited position from which the next leg is
  walked and proven. A crossing and a link are marked by the same `transport`
  field and skipped as rides by the one enumeration of visited positions
  (`nav::positions_of`), which `DW0311`, the branch proofs and the waypoint
  export all read.
  The prefab **viewer** asks the same question for a different purpose — which
  anchor a review page should open on — and prefers a declared role over its own
  wider list of name stems (`spawn`, `entry`, `entrance`, `threshold`), which is
  a guess about one piece and is consulted only when the piece does not say.
- **The class trigger is ONE-SHOT per player, sealed in the pack.**
  `class_apply_<c>` ends in `teleport @s <entry point>`, so a
  `dw.class` trigger left armed after a class is a live warp back to the start of
  the delve, usable at any point in a run by anything that can chat a command.
  `tick` runs `execute as @a run function <ns>:class_arm`, whose
  whole body is
  `execute unless score @s dw.classed matches 1 run scoreboard players enable @s dw.class`
  — the vanilla trigger pattern of re-enabling only what is meant to be usable.
  The seal is **per-player**, because classing is (`dw.classed`): a second player
  still on the class screen keeps an armed trigger while the first is sealed, and
  the score survives death and relog. The dispatch carries the same guard
  (`unless score @s dw.classed matches 1`), so a score arriving by any other
  route is inert rather than a warp. The guard lives in `class_arm` rather than
  inline in the tick line so the generated `class_trigger_once` PackTest can
  drive the **real** arming path as its own dummy instead of restating it: it
  proves an unclassed player's trigger works, takes the class, then arms again
  and shows the `trigger` command failing, no score arriving, and the dummy
  neither re-classed nor moved (verified by mutation — with the guard removed the
  template goes red).
  The legitimate post-death re-arm is *nothing*: `gamerule keep_inventory true`
  keeps the kit, and `dw.classed` / `dw_class_<c>` are scoreboard and tag state
  that a death does not touch.
- **First-join placement is datapack-owned** (not the server's reading of
  level.dat). `tick` runs `execute if score #placed dw.sys matches 1 as
  @a[tag=!dw_joined] run function <ns>:join_place`; `join_place` teleports `@s` to
  the campaign entry point (the same cell
  `class_apply_*` uses) and then adds the `dw_joined` tag, so it fires exactly
  once per player and a relog keeps the player where they stood. **Respawn is
  untouched** (`spawnpoint @a` + the spec-0012 checkpoint machinery). The `#placed`
  gate makes the teleport land on real geometry — the prefabs are `/place
  template`d over the first ticks. *Why it exists:* the **integrated
  (singleplayer) server** does not reliably honour the emitted spawn state and
  drops the first join at the superflat floor (x/z of world spawn, y = build
  floor) — inside stone, unescapable except by dying. A dedicated server places
  the same world correctly, so no rung of the validation ladder can observe it;
  the assertion is therefore static (`first_join_placement_emitted`). The target
  is the entry point rather than the live `dw:cp` checkpoint deliberately: `dw:cp`
  is *seeded* to that same cell at setup, so they agree at world start and diverge
  only once a checkpoint has fired — at which point a *first*-joining player is a
  player who has not played, and the entry point is where the campaign begins.
- `datapack/pack.mcmeta`: `min_format`/`max_format` = `[94, 1]` (a bare
  `pack_format` is rejected for formats > 81).
- `resourcepack.zip` → `pack.mcmeta`: `min_format`/`max_format` = `[75, 0]`, and
  **no** bare `pack_format`. Resource packs and data packs share one
  `pack.mcmeta` codec; only the "must declare `min_format`/`max_format`"
  threshold differs — **64** for resource packs, 81 for data packs — and the
  codec cross-checks a bare `pack_format` against `max_format`, so emitting both
  risks a declaration-mismatch error. Formats are pinned in `versions.toml`
  (`[resourcepack] pack_format`) from the 1.21.11 client's `version.json`
  (`resource_major: 75, resource_minor: 0`). Getting this wrong is **client-side
  only**: the pack is refused whole ("Pack declares support for version newer
  than 64, but is missing mandatory fields min_format and max_format") and every
  baked skin silently never loads, while no server — and therefore no rung of the
  validation ladder — parses a resource pack at all.
- `<out>/`: `manifest.json`, `datapack/`, `packtest-datapack/`, `server/`,
  `critical-path.json`, plus `resourcepack.zip`+`SKINS.md`
  (`resource_pack_sha1` and `resource_pack_overrides_vanilla` in manifest) for a
  campaign whose pack carries anything — a skin, the art font, a language, a
  replaced texture.
- `<out>/manifest.json` carries exactly `campaign_id`, `delvec_version`,
  `dsl_version`, `mc_version`, `inputs` (SHA-256 per authored document the build
  read, l10n sidecars included) and `outputs` (SHA-256 per emitted path), plus
  `language` on a non-`en` bake and `resource_pack_sha1` +
  `resource_pack_overrides_vanilla` on a build that ships a pack; `inputs`
  covers `textures/<id>.png` (spec-0084).
  It names no repository, revision or checkout: a revision is a property of a
  checkout rather than of the bytes the compiler was handed, so which content
  commit a shipped delve was built from is stated by the party that knows it —
  the release image's `org.opencontainers.image.revision` and
  `ca.stellarfeline.delvewright.campaign-commit` labels.
- `<out>/critical-path.json`: the bot contract. `version` is the **campaign's DSL
  version**; `format_version` is the **contract's own** version, currently `4`
  (`plan::CRITICAL_PATH_FORMAT_VERSION`) — bumped when what the harness is told
  about proving the path changes, independently of the DSL. At format 2 every
  objective-bearing step (`talk-to`/`reach`/`kill`/`collect`/`interact`) carries
  `objective`: the `obj/<id>` that step must prove, and the harness passes the step
  only when **that** objective's anchored completion marker arrives (position
  arrival, an opened dialogue and an emptied chest are means, never proof). The
  harness rejects any other `format_version` outright rather than running a path it
  cannot verify. Endgame rule: campaign completion is due at the LAST objective step;
  the campaign marker arriving earlier fails the run on the spot, because every
  remaining step is then provably hollow.

  **Every cutscene a step can fire is on the step** (`compiler::hold`), as two
  optional positive integers. `cutscene_seconds` is the time from the step's
  completion to the end of the last cutscene its completion schedules: the
  bundles it fires (the objective's `on_objective_complete`, its quest's
  `on_complete` when it is the quest's last objective, a performed trigger's
  `effects`, a `rest` step's bonfire `on_rest`) walked along their own timeline,
  where a `sequence` step's effects play `at_ticks` after the sequence, every
  cutscene on it counted and the latest end winning (`link::cutscene_end_offset`,
  whole seconds rounded up). `en_route_cutscene_seconds` is the longest a cutscene
  fired by something the step's walk passes can hold the party: an `approach`
  trigger (an ambush desugars to one) whose range, read as the emitted selector
  reads it, a cell of the step's proven leg enters (every such trigger, for a walk
  the route proof did not route: a step with no proven leg, and the step after a
  `rest`, which the bot walks from the bonfire); a pressure-plate or tripwire
  trap the leg steps on (the same widening); a `loop` step's `on_cross`; and an `on_arrive` bundle of
  a body an earlier step set moving, which lands at a time no tick count states and
  is owed by every later step. The nested lists of the effect grammar are
  classified by `hold::Nesting` (a `sequence` step's timeline, `on_arrive`,
  `on_rest`, `on_respawn`, `on_caught`), and `crates/delvec/tests/hold_containers.rs`
  holds that classification equal to the containers `delvec schema` declares. A list
  fired by a death or a capture (`on_respawn`, `on_caught`, `on_death`) holds no step
  of the route, nor does an `on_kill`, an assembly's `on_land`, a shop offer or a
  shortcut's `on_unlock`. Gates are not read, so both numbers are upper bounds. The
  harness sleeps `cutscene_seconds` after the step; when a walk that began with the body in
  hand finds it out of adventure mode it abandons the path, waits for control up to the larger of the two plus its grace and
  walks on from where the cutscene returned it, and it **refuses** control taken during
  a step that declares neither, naming the step — the plan missed a cutscene. Both keys
  ride every `branch-path-<branch>.json`.

  **A `trigger` step that carries the party** (spec-0083 §4) carries two more
  keys: `transport: [x, y, z]`, the link's `to` (the field every carried step
  has), and `stand: [x, y, z]`, the cell inside the link's volume the act is
  performed from. Both are present exactly when performing the trigger carries
  the party. The bot walks to `stand` as a block goal, performs the act from
  there, passes on the trigger's fired marker and then awaits the landing as for
  every carried step. The same keys ride every `branch-path-<branch>.json`.

  **A `reach` step a landing completes** carries `completed_on_landing: true`
  (`plan::completed_on_landing`, the one place the rule lives). It is present
  exactly when the step before it carries the party — a crossing, a link or a
  loop, through its `transport` — and the landing lies in this reach's completion
  volume, read by `ReachCompletion::completes_on_landing`: the vanilla
  intersection test with the body centred on the cell and its feet on the cell's
  floor for a crossing and a link, which put a body on a fixed point, and the
  certain reading (the cell inside the cube) for a loop, whose landing keeps the
  body's place in the cell. The server completes such a reach on arrival, during
  the carrying step, so the path keeps the step and the bot walks nothing for it:
  it asserts the marker already arrived, and the endgame rule counts the campaign
  marker as due at the carrying step when the last objective step is such a
  reach. A sealed room reached only by a link, whose beat stands where the link
  lands, is the shape. The harness refuses the key on a step with no carrying step
  before it (a bonfire `rest` between the two is looked past).

  **`non_combatants` — who the bot may never swing at** (format 4,
  `combat::non_combatants`). A block of `kinds`, `ambiguous`, `examined`,
  `unbound` and (exactly when unbound) `reason`. `kinds` names the entity kinds,
  in the **client's** vocabulary (`mannequin`, `villager` — no namespace, because
  that is the only identity mineflayer exposes on 1.21.11), no body of which is
  ever a combat target. It is derived from the emitter's own NPC rule: a skinned
  NPC is a `minecraft:mannequin`, a plain one is its `base_entity`, and both
  branches summon `Invulnerable:1b`. It rides on the **path**, not on the combat
  plan, because it is a fact about the world the bot walks: a delve with NPCs and
  no combat ships no combat plan at all, and its bot must still know not to swing
  back at a quest-giver when a fall takes its health.

  A kind that is an NPC body **and** a wave mob or an actor entity cannot be
  excluded without making that fight unwinnable, so the fightable kind wins and
  the collision is stated in `ambiguous[]` (`kind`, `why`, naming the NPCs) — the
  one direction that cannot soft-lock a delve, said out loud instead of decided in
  silence. The harness prints every ambiguity at load.

  **`why` names only remedies the object can still take** (`combat::ambiguity_remedy`).
  The two NPC-side moves — a different `base_entity`, or a `skin` — are both inert
  once the colliding kind is `mannequin`: an NPC whose body is a mannequin got
  there by declaring a `skin`, and its `base_entity` is not the body it wears, so
  the message points at the other side of the collision instead (take the wave mob
  or actor off `minecraft:mannequin`). Where the collision is on some other kind
  and this campaign *also* fights mannequins, the `skin` move would trade one
  collision for another and is not offered either. **Open, and not this census's
  question:** the kind is the whole channel the bot has (entity tags are not
  readable from the client), so a delve that bodies every character as a mannequin
  cannot be separated by any quantifier over kinds — that belongs to how the
  harness identifies a body, not to who is counted here.

  The harness **requires** the block and refuses a path without it. The only
  fallback available to it is a literal set of entity names living in the harness,
  which is right only for the campaigns whose author happened to pick those
  bodies — that fallback is the defect, not the safety net.

  **A `talk-to` step's `pos` is the CAST LEDGER's**, not the NPC's stage-2 anchor.
  The stage-2 `anchor` is only where a body is first
  summoned; a `move-npc` walks it away and the quest's `cast` row records where it
  then stands (`DW0461` proves the row equals the effect history). Reading the
  anchor here would make the bot contract a second, staler source of truth. The
  row is chosen by `cast::station`, the one model the emitted `cast_<npc>`
  selector and `DW0483` also read: clauses accumulate in quest-DAG order over the
  quests this playthrough activates, **later declarations win** (`dw.qa_<quest>`
  is never cleared), and within a quest the last placement whose
  `requires_flags`/`forbids_flags` gate holds under the flag state the party
  carries into that step. Consequences: a ledger that moves a body across areas
  moves the step's area too, so the inter-area `transport` map follows; a row
  resolving to `"offstage"`/`"dead"` is an internal-invariant error (`DW0195` /
  `DW0461` own the refusal upstream). A campaign with no ledger anywhere
  keeps the stage-2 anchor.

  **A `reach` step carries the volume the SERVER adjudicates in**, as
  `completion`, beside the authored `radius`. The two are different facts and the
  bot uses only the first. `reach::reach_completion` computes it once and all three
  readers take it from there — the `tick` line's `@s[…]` selector is formatted
  from the same value — so the artifact and the datapack cannot describe different
  regions. Shape: `{"kind":"cube","lo":[x,y,z],"hi":[x,y,z]}` (inclusive
  block corners, half-extent `max(1, radius)`). Required, never
  optional: an optional field with a fallback is the harness keeping its own
  completion model — a second authority over one fact. The harness derives its
  walk goal from `completion` and, on the failure path only, reports when the walk
  ended outside it — a positive precondition would false-fail every step whose
  completion legitimately teleports the player away. `tests/reach_completion.rs`
  asserts the emitted selector equals the exported volume for every reach
  objective of every campaign it builds.

  **`ending_tail_ticks`**: the terminal `assert-complete` step carries
  the path's scheduled-ending tail — the compiler-computed maximum tick offset
  between the terminal objective completing and `campaign-complete` firing
  (`sequence` `at_ticks`, `move-npc`/`move-actor` walk durations).
  The harness completion window becomes `max(15s, tail·50ms + 10s)` — widened,
  never narrowed. Omitted when the ending is synchronous, so a path with no scheduled tail
  is byte-identical. Emitted by the same computation the campaign PackTest's await
  timeout uses, and per branch on each `validation/branch-path-<branch>.json`
  (a branch waits out its OWN ending's tail).

  **`rest` steps** (spec-0016 §1). A bonfire arms
  an affordance and moves nothing until the party rests — souls-correct, and also
  invisible to a ladder that walks past every fire without touching one: the
  checkpoint never moves, so a die-retry trial respawns at world spawn and blows
  the walk-back budget, judging the *campaign* for a *proof* that never performed
  the player loop. Resting is the intended loop, so the proven path
  performs it. After the step that arms bonfire `<i>` — or, when a crossing
  carries the party out of the fire's area at that step, after the first later
  step at which the party stands in that area again (the area of the last step
  naming a position, with the next one in the same area); none, if the route
  never returns — the path carries one
  `{"action":"rest","bonfire":<i>,"anchor":"anchor/…","pos":[x,y,z],
  "command":"/trigger dw.rest set 2"}`. The bot walks to `pos`, **right-clicks the
  `dw_bonfire_<i>` interaction** — which is what opens the dialog and, crucially,
  what *enables* the `dw.rest` trigger — and then sends `command`, the exact chat
  line the "rest and save" button runs. The click is not optional: a bot that only
  chats the command changes nothing, because the trigger is disabled until the
  opener enables it. A `rest` step carries no `objective` and proves none — it
  performs the loop the following steps are proven under. Several bonfires armed by
  one beat are spliced in bonfire order. This is a path *export* only:
  `plan.critical_path` is untouched, so every `fire_step` index and every nav proof
  sees the unspliced path. **That is what makes two coordinate
  systems**, and they drift by one per bonfire armed earlier: internal indices
  (`fire_step`, `Encounter::step`, every nav proof) count `plan.critical_path`;
  exported indices count these `steps[]`. Every artifact a harness reads states
  the EXPORTED one, and `Plan::exported_step` is the single translation — a
  consumer that mixes them is a silent off-by-N.

  **`witness-strike` steps** (spec-0082 §5.4, §5.7). Per assembly with a strike
  pattern, just before the first exported step that strikes it, the path carries
  `{"action":"witness-strike","assembly":…,"expect":"struck","pos":[x,y,z],
  "step":j,"facing":k,"facing_count":n,"yaw":y,"amount":a,"window_ticks":w}` then
  `{"action":"witness-strike","assembly":…,"expect":"spared","pos":[x,y,z],
  "window_ticks":w}` (`assembly::with_witness_steps`, spliced into the exported
  `steps[]` after the rest steps, so it moves exported indices exactly as `rest`
  does). `struck`'s cell is facing 0's stand cell — a landing cell under the
  limb, inside the arming region, from which a body draws that facing; the bot
  stands there until a blow takes health. `spared`'s is a walked cell outside the
  arming region's keep-out, nearest the region; the bot stands there and takes
  nothing. `w` is one cycle (wind-up + hold + strike) of every step plus the
  longest again plus `WITNESS_SLACK_TICKS` (40). Neither carries an objective. An
  assembly with no stand cell or spared cell, or never struck on the path, is not
  witnessed, and `validation/assembly.json` `witnessed` says which were.

  **`trigger` steps — the path performs the triggers it depends on**
  (`plan::path_triggers`). An environment trigger is a party act nothing on the
  quest DAG orders, and two proofs credit what it does: the region-write model
  credits the way it opens, the flow replay credits the flags it sets. A path
  credited with either owes the act, so it carries
  `{"action":"trigger","trigger":"trigger/…","on":"strike"|"use"|"approach"|"step"|"strike-npc","pos":[x,y,z]}`
  plus `anchor` (every kind but `strike-npc`), `npc` (`strike-npc` only) and
  `range` (`approach` only). A trigger is performed when its bundle **opens a
  way** (`open-gate` / `open-way`) — at the first path step where its
  `requires_flags` hold, none of its `forbids_flags` is set, and the party is in
  the area its target stands in — or when it **pays a flag debt**
  (`Flow::trigger_debts`) — at that same point if it comes no later than the step
  that reads the flag, else directly in front of the reader. A `clear-region` does
  not make a trigger performed by itself (a lift's car, a collapsing floor — not a
  threshold); it is credited when its trigger is performed for one of those two
  reasons. `pos` is the anchor cell the emitter summons the hitbox on, or the
  NPC's body at that beat (the cast ledger's station). Unlike `rest`, the step IS
  in `plan.critical_path`: it is a place the party must walk to, so the leg into
  it is a leg `DW0311`/`DW0317` prove, with the trigger's own opening not yet
  credited. It carries no `objective`; the bot does what a player does — a real
  attack for `strike`/`strike-npc`, a real right-click for `use`, a walk into
  range for `approach`, never a command — and passes only on the trigger's fired
  marker `[dw:complete <campaign> trigger/<id>]`, which every trigger whose bundle
  opens a way or sets a flag broadcasts first thing (`plan::trigger_may_be_performed`).
  A path whose trigger gate never holds, or whose target never shares an area
  with a step, does not perform it, and nothing it opens is credited. For these
  two reasons numeric gates (`requires_state`) are not evaluated: a press whose
  state gate is closed broadcasts no marker and the step fails where it stands.
  **A numeric gate only presses move is driven** (`plan::path::drive`,
  `DW0985`). A datum is *driven* when it is declared, no stake forfeits it, no
  loop counts it, it has one holder on the walk (`party`, or `player` with
  `min_players` 1), and every write to it anywhere is a top-level effect of a
  `use` or `strike` trigger on an anchor; such a trigger also broadcasts the
  fired marker. The plan replays, in path order, every press the path performs
  — the trigger's own gate (`once`, flags, numeric terms), then each effect in
  order behind its own `when`, read against the value the earlier lines of the
  same bundle produced, as the datapack runs them — and at each objective whose
  `requires_state` reads a driven datum and does not hold, searches
  breadth-first (candidates in declaration order) for the shortest press
  sequence that makes every such term hold, bounded at 64 presses and 16384
  distinct states. The sequence is performed as `trigger` steps directly in
  front of the objective, after any trigger already due there; the anchor is
  resolved in the objective's area first, elsewhere only when its name is
  unique. No sequence within the bound refuses the build with `DW0985`, naming
  the gate, the presses and every value they reach. A press whose line reads a
  datum the plan cannot name there is not taken, and a search that skipped one
  refuses nothing. Terms on data that are not driven are left to the other
  proofs.
  **`loop` steps** (spec-0086 §5.2, §6). A loop that holds where the forced
  route meets it is **exercised**: in front of the first step whose position is
  beyond the slab along its axis while the party, in the loop's area, is not,
  the path carries
  `{"action":"loop","loop":"loop/…","pos":[x,y,z],"cross":[x,y,z],"offset":[dx,dy,dz],"times":<n>,"transport":[x,y,z]}`.
  `pos` is the approach cell the party walks to, `cross` the slab's lowest
  course in the anchor's column, `offset` the move, `transport` the landing
  (`cross + offset`). `times` is the least number of crossings (at most 64)
  after which the loop's own `counts` and `on_cross` writes shut its gate, read
  from the loop replay over the flow model, else `1`; those crossings' writes
  are credited to the path as forced. The step is in `plan.critical_path`:
  `positions_of` walks `pos`, then marks the landing `transport_before`, so the
  leg after the step begins at the landing — one enumeration of visited
  positions for every consumer. The bot walks to `pos`, walks through `cross`,
  and requires the jump to equal `offset` to within 0.001 block per axis on every
  crossing; it fails the step when a crossing times out
  (`DELVEWRIGHT_LOOP_CROSS_TIMEOUT_MS`, default 20 s) and, outside a `loop`
  step, when any walk is moved by the offset of a loop the path exercises. It carries no
  `objective`.
- `<out>/validation/critical-path-waypoints.json`: the DW0311-proven per-leg route
  thinned to sparse waypoints (`from`/`to` = the `critical-path.json` step
  positions; a waypoint at each corner/floor-height change **and the corridor commit
  cell one step past each corner**: a wide-room→corridor corner is
  range-1-satisfiable from an off-route pocket beside it, so the post-corner cell
  gives the harness a close corridor-axis target for its stall-recovery). **No
  waypoint stands inside a timed gate's region** (`waypoints::leg_waypoints`): a
  gate's two mouths are force-kept, and a corner's commit cell that falls inside
  the gate is dropped, so a crush crossing is always one mouth-to-mouth hop and no
  waypoint asks the bot to stand where the fill closes. A leg
  that walks through a closed fence gate carries a `use_gates` array:
  the gate cells the player right-clicks open (an adventure-legal USE), each also
  force-kept as an explicit waypoint (never thinned away mid-run); the field is
  omitted for gate-free legs, so gate-free campaigns stay byte-identical. The
  harness replays these as successive nearby pathfinder goals so no single distant
  A* solve strands the bot on a large open cave (its pathfinder's `canOpenDoors`
  performs the gate click, and the harness's fence-lip waypoint filter stands as
  defence-in-depth). A leg whose route climbs (spec-0099) carries a `climbs`
  array, in route order: each `{from, to, bottom, top, block, facing?}` — where
  the body takes hold, where it lets go (both kept waypoints), the lowest and
  highest cell it holds in, the block, and a ladder's facing. The cells held
  between `from` and `to` are not waypoints (a waypoint is a place to stand), so
  a climb is one hop, and the harness drives it. Omitted for a leg that climbs
  nothing. **Validation metadata, not shipped gameplay** —
  excluded from the delve image (like `packtest-datapack/`); emitted only when a
  walked critical leg exists, so a fully-transported campaign stays
  byte-identical.
  A campaign with `timed_gates[]` (spec-0016 §4) additionally carries a top-level
  `timed_gates` table — one entry per declared gate, in declared order, with
  `id`, `region: {min, max}` (inclusive, canonical world-coordinate bbox),
  `block`, `open_ticks`, `closed_ticks`, `phase`, `crush` — and every leg whose
  proven route walks through one carries `timed_gates: [<id>, …]`. `crush`
  exports the §4-addendum fact that the closing edge KILLS a player
  caught inside the region: reactive gate handling — wait for a window only
  after a hop fails — is safe when a closing gate merely aborts the path and
  lethal when it crushes. The harness
  stages a crush crossing at the compiler-pinned mouth cell and enters only on
  an observed fresh closed→open edge with full margin; that decision needs to
  know WHICH gates crush, and the fact is compiler-owned (no-hack layering:
  export it, never make the harness infer a lethal mechanic). A leg **crosses** a
  gate iff at some cell of its full A* route the player's own 2-block occupancy
  (feet cell or the cell above) lies inside the region — i.e. closing the gate
  would land the fill on the walk. The test is stated over the *unthinned* route
  (a straight run through the gate thins to its endpoints) and is exact rather
  than proximity-based: a leg that merely walks *past* a gate is deliberately
  unmarked, because the mark is what licenses the harness to retry a failed leg
  and a looser mark would grant blanket retries that mask navigation
  regressions. The gate **mouth** — for each maximal run of in-region
  route cells, the route cell immediately BEFORE it and the one immediately
  AFTER, i.e. the pair flanking the crossing — is force-kept as waypoints,
  exactly as a `use_gates` cell is: corner-thinning would otherwise collapse a
  corridor through a gate to its endpoints and ask the bot to walk the whole run
  inside one open window, where pinning the mouth splits it into an
  uninterruptible approach plus a short crossing — which is what `DW0378`
  actually proves admissible (the *span*, not an arbitrary run-up to it).
  **In-region cells are deliberately NOT pinned**: the harness treats
  every waypoint as an *arrive-at* goal, so a waypoint under the gate parks the
  bot there — and a `crush: true` gate then fills that cell with the bot in it.
  The flanking pair says the same thing about the route without ever naming a
  lethal cell as a destination. A campaign whose route turns *inside* a gate
  region still keeps that corner (dropping it would let the polyline leave the
  proven path, which no waypoint rule may ever do). `DW0378` proves the window is *readable*; this
  export is what lets the runtime rung act on it — the harness stands off (only
  when caught inside the fill), waits for the closed→open edge and retries,
  bounded by two full cycles plus margin, instead of failing the leg when the
  gate fills mid-approach. Both keys are omitted entirely for a campaign with no
  gate clock, so such campaigns stay byte-identical.
- `<out>/validation/branch-plan.json` + `<out>/validation/branch-chronicle-<branch>.md`
  + `<out>/validation/branch-path-<branch>.json`
  (spec-0025): the branch set — per branch, its flag assignment, its
  critical path computed under that branch, and the dialogue choices that enter
  it — plus one per-branch chronicle (every reachable node's `happening` line in
  compiled play order) and, per REACHABLE branch, one **executable path** in the
  ordinary `critical-path.json` contract, which is what the harness's branch runs
  walk. **Validation metadata, not shipped gameplay**, excluded
  from the delve image like `critical-path-waypoints.json`, and emitted **only**
  for a campaign that declares `branch_points`, so nobody who has not opted in
  gains a file. Full description in §5 "DW048x — branch-complete narrative
  verification".
- `<out>/validation/branch-waypoints-<branch>.json`: per REACHABLE
  branch, the branch's own waypoint artifact, in exactly the
  `critical-path-waypoints.json` shape (same corner thinning, same `use_gates`
  force-keeps, same `timed_gates` table/marks) — its legs follow the branch's
  **own** exported path, in that path's step order. Backed by a **per-branch
  `DW0311`**: every walked leg of every reachable branch path is routed over the
  assembled world under the branch's own causal gate seals before export, with
  `gate_events` fire-steps, forcedness and the strict-ancestor relation recomputed
  in the branch path's own step space (`Plan::branch_gate_model`; a bundle whose
  objective or quest the branch path never plays is unforced there, so it opens
  nothing that path may cross) — a branch path is a
  different sequence, so default-path step indices are never carried across (the
  same trap `emit::rest_step_index` documents for bonfires). Each branch's
  routes also pass the `DW0314` standability self-check. Branch diagnostics are
  prefixed ``branch `<id>`:``. The harness derives the filename from the
  branch's `branch-path-<slug>.json` (one slug, one contract) and reports
  **loudly** — stderr + a run-report finding — when a branch must walk without
  it (single-goal fallback, terrain-flaky where waypointed navigation is
  deterministic).
  Emitted only when the branch has walked legs and the campaign builds an
  occupancy model, so everything else stays byte-identical. Proofs not
  quantified over branches: checkpoint
  no-stranding (`DW0315`/`DW0316`), stealth (`DW0327`/`DW0355`), traps
  (`DW0342`), shortcuts/ambush/timed-gate (`DW0373`–`DW0378`, `DW0388`), stair
  orientation (`DW0430`) — these run on the default path only.
- `<out>/validation/lethal-gate.json`: the lethal-volume proofs' **binding
  ledger** (`compiler::lethal`, spec-0031, playtest-methodology.md rule 1).
  `volumes.declared` vs `volumes.resolved` (a gap is an anchor no placed piece
  provides — already `DW0142`, restated so a reader of the ledger alone cannot
  mistake a dropped volume for a proven one), `cells` (what the navigation model
  actually made impassable), `respawn_seats_examined` (every posted place tested
  against `DW0511` — respawn seats and posted bodies alike),
  `critical_path_legs_examined` (legs routed with lethality applied) and
  `packtest_templates` (the runtime half, one per volume; a compile-time-only
  green over a runtime mechanism is exactly the vacuity this number exposes).
  `blind_reach` is `DW0943`'s (spec-0085 §6.3): `grants_examined`, `caught`, and
  per blinding grant its `path`, `effect`, `seconds`, `standing` (`|S|`),
  `reach_moves` (`n`), `reached` (`|R|`), `caught` (the cells) and `caught_by`.
  `danger_visibility` is `DW0891`'s half (spec-0062 §5): `population` (cells the
  party can walk to from everywhere the campaign PUTS it, over the world with
  lethality removed), `caught` / `shown` / `reads_as_safe_floor` over every
  volume, `declarations.examined` vs `declarations.borne_out`, and one row per
  volume carrying its `keep_out` box, its `caught` count and cells, its `shown`
  count and its `shown_by` as declared, and `reached_by`: the first body the
  engine models that gets its hitbox into the volume, in words, or `null` when
  none does (then counted in `unreached`, and warned as `DW0891`). **A volume
  that catches nothing prints its zero beside the population**, so a pit whose
  keep-out lies under the rim reads as *checked and clear* rather than as
  unbound. The build prints the same
  numbers as one line — `danger-visibility binding: …` — before the verdict is
  taken, so a refusal states the population it was measured against as fully as a
  pass does.
  **Emitted only for a campaign that declares a volume**, so a file that exists
  and reports zero is a finding rather than an absence.
- `<out>/validation/atmosphere-repaints.json` (spec-0080 §5.2): **what each
  repaint must tell a connected client** — per `set-atmosphere` with a
  resolvable volume, its `effect` path, the `biome` it paints, the `chunks`
  `[cx, cz]` of its painted 4-cell box, and `after`, the completion-marker token
  of the bundle that fires it (an objective's `obj/<id>`, or a trigger's
  `trigger/<id>` when it carries a marker), or `null` for a repaint nested in a
  later `sequence` step or another deferred list, gated by `when`, or fired from
  a root no marker announces. `harness/src/repaint.ts` reds a performed repaint
  whose held chunks no `chunk_biomes` named within 10 s, any held chunk a
  `map_chunk` resent, or one that binds no chunk held inside the view distance
  the client is served from where it stood when the bundle fired (see
  `docs/reference/tools.md` §8). Emitted
  only for a campaign that repaints.
- `<out>/validation/biome-map.json` (spec-0080 §5.3): the biome map's place
  paints — `{ground, places: [{place, atmosphere, biome, precipitates,
  cells}]}` — read by `delvec palette --build … --place …` to tint a scene
  inside a place under that place's biome. Emitted only when a place carries an
  atmosphere. **A PackTest world is not the delve's world**: the suite runs in
  the server's own test level, whose generator lays its own biome, so the
  generated atmosphere templates read a carried place as itself (the bootstrap
  painted it) and read the ground only as "not the repaint's biome", never by
  its id.
- `<out>/validation/death-plan.json`: **the bot tier's contract for
  dying** (`compiler::deathplan`). A PackTest fake player is permanently
  undamageable — measured twice, independently —
  so that tier cannot witness a player death at all, and every runtime claim
  about the death loop belongs to the mineflayer tier. This file is what lets it
  make one: the campaign's PROMISES, in the campaign's own terms.
  - `lethal_volumes[]` — each volume's inclusive box, the `keep_out` box a
    player's own body must stay out of (the volume widened by half a player
    width — read, never re-derived, so the cell-versus-hitbox rule has no
    second implementation in TypeScript), its canonical-English
    `message` plus the `message_key` a localized run asserts instead, its
    `damage_type`, and its **`gate.terms`** (format 4, spec-0088): the same
    term rows a `drop-stake`'s gates carry, `Plan::gate_terms` over the
    volume's `when`, empty for a volume live from world-load. The harness
    excludes a staged volume from a walk leg only while its gate reads open
    (asked before each leg; an unanswered term excludes it), enters it in the
    death loop only when it reads open, and refuses a row with no `gate`.
    `format_version` is `DEATH_PLAN_FORMAT_VERSION`, 4.
  - `on_death` — how many effects the bundle carries at every nesting depth
    (read through `QuestEffect::nested_effect_lists`, so a `sequence` is
    counted), and `drops_stake[]`: per stake the bundle can forfeit, **the gate
    that decides whether this death forfeits it**. A `drop-stake` carries a
    `when` like every other effect, so the promise is conditional: the death
    forfeits the stake when a gate is open and keeps it whole when every gate
    is shut, and the bot tier asserts both halves. Each entry is `{stake, gates[]}`:
    the gates are alternatives — one stake dropped by two effects is forfeited
    when either fires — and each alternative's `terms[]` is the conjunction of
    its own effect's gate with every enclosing effect's, so a gated `sequence`
    round an ungated drop gates the drop. A term is `{objective, holder, min,
    max, negate}`: the ledger, the selector that holds it (`#party` for a party
    datum or a flag, `@s` for a player datum — carried rather than derived from
    a scope, for the same reason `keep_out` is), the closed interval with open
    ends that `matches` spells, and whether the gate wants the range to fail.
    The terms are `Plan::gate_terms`' — the same reduction the emitter renders
    into the `execute` guard, so the datapack and the contract cannot disagree
    about when a death takes a purse; `tests/death_plan_gate.rs` holds every
    emitted `stk_drop_<stake>` guard in `on_death_fire` equal to one of the
    plan's alternatives, term for term, in both directions. An unconditional
    drop is one alternative with no terms.
  - `stakes[]` — the declared `forfeit` rule (`all` / `proportion` /
    `fixed` / `none`), `max_live`, `on_full`, `collect_by`, the
    `collected_message` and the `marker_item`, plus the wagered `currency`: its
    state id, its **scoreboard objective** (spec-0032 *decided* a currency is a
    ledger, so the objective is the declaration and not an implementation
    detail), its `initial`, its scope and its player-visible name.
  - `placement` — the recovery stake's compile-time table as the bot checks it:
    `seats[]` (`cp`, label, cell), `regions[]` (each carrying the lethal
    volume's **id** when it is one, so the harness matches by name rather than
    re-deriving `compiler::stake`'s region ordering) and `rows[]` as
    (seat, region) → anchor.
  - `binding` — what the contract lets the tier examine, and `unbound` +
    `reason` when it lets it examine nothing. A campaign with a volume and no
    `on_death`, or an `on_death` and no volume, is unbound: the bot has nowhere
    to cause a death from content, or no promised consequence to assert. That is
    reported, never walked.

  It carries **no emitted function name, no generated command and no objective
  the engine invented for its own bookkeeping** (`dw.kl0_*`, `#stk_amt`, …).
  That is the whole discipline of the file: an assertion written by reading the
  emitter cannot fail when the emitter is wrong. **Emitted only for a campaign
  that declares a lethal volume, an `on_death` or a stake**.
- `<out>/validation/teleport-gate.json`: the `teleport` proof's **binding
  ledger** (`compiler::teleport`, spec-0031, playtest-methodology.md rule 1).
  `teleports.declared` vs `teleports.resolved` (a gap is an anchor no placed
  piece provides — already `DW0142`/`DW0360`), `cells` (the size of what the
  emitted selector sweeps), `affordances_examined` (every engine affordance
  tested against `DW0542` — `eclipse::affordances` plus the seal shells) and
  `packtest_templates` (the runtime half of totality, one per teleport; whether
  vanilla's `@e[<box>]` really reaches every entity type is vanilla's fact, not
  the compiler's, and a compile-time-only green over it is exactly the vacuity
  this number exposes). The partition (spec-0083 §5): `teleports.links` and
  `teleports.gathers`, counted from the triggers, with `links + gathers =
  resolved`, and `legs_carried`, the default path's legs a link carries. A
  campaign that declares a teleport and reports zero links states a fact, not a
  finding. **Emitted only for a campaign that declares a teleport**, so a file
  that exists and reports zero is a finding rather than an absence.
- Every `snapshot` manifest carries a **`frame`** block: `targets_in_frame`,
  `targets_out_of_frame`, and `featureless` (`null`, or the distinct-colour count
  when the frame shows no scene at all). The binding rule applied to a picture —
  a render that succeeds, writes a file and is a rectangle of flat background is
  indistinguishable from one more shot taken to a directory listing, to a contact
  sheet and to a reviewer skimming, and the only thing that separates a camera
  aimed at the room from one aimed at a wall is the count. Judged by the arm that
  DREW the frame (`view::detect::is_featureless`), so a consumer never computes a
  second verdict on the same question. `tools/ci/check-gallery-render.py` reads these
  back rather than re-deriving them.

- `<out>/validation/effect-roots.json`: the **effect-root walk's own binding
  ledger** (`dsl::RootBinding`, written by `emit::build_with_warnings`).
  `roots_enumerated` / `roots_total`, the total `bundles` and `effects`, a
  per-root `sites` count, and `unbound_roots` — the roots this campaign has no
  bundles at, listed rather than left to be derived. Most effect-shaped proofs in
  this compiler are only as good as the roots this walk reaches, and this file is
  how something downstream asserts that a build's effect walk bound to anything. Emitted for every campaign,
  because the walk runs for every campaign; a zero at a root is not a failure (a
  campaign with no traps has no trap payloads) but it is the reason any proof over
  that root is unbound, which is what a reader needs and cannot infer. For scale:
  the gallery binds all ten roots, where the largest shipped campaign binds
  three.
- `<out>/validation/ways.json`: the contingent-way gate's **binding ledger**
  (`compiler::ways`, spec-0042, playtest-methodology.md rule 1). `pieces` and
  `pieces_with_ways` (what the placed world is, and how much of it makes a
  contingency claim), `staged` (ways with cells — measured against `declared`
  inside the gate, whose disagreement is `DW0549`), `opened` /
  `unforced_only` / `never_opened` (the disposition split), `open_way_effects`,
  and `elements_examined` — the required elements the reachability half judged
  against a way-carrying piece's contract, which is zero for a world whose ways
  are scenery and is then also said out loud as `DW0555`. One row per staged way
  carries its area, piece, placement index, name, sign, block, cell count and
  every opening that names it with that opening's DAG step and forcedness.
  **Emitted only for a world that stages a way**, so a file that exists and
  reports zero ways is a finding rather than an absence.
- `<out>/validation/fixture-gate.json`: the fixture-class proof's **binding
  ledger** (`compiler::affordance`, `DW0545`, playtest-methodology.md rule 1).
  `fixtures_declared` and `borne_declared` (every engine-summoned hitbox, mark
  and display, split by the class it declared), `box_selectors_examined` (every
  `@e[…]` selector narrowed by a positional box — the region verbs of this
  build), and `packtest_templates` (the runtime half, one per `teleport` × `stake`
  pair; the original defect has no compile-time form at all, so this is the only
  number that binds to it). Unlike the ledgers above this one is emitted for
  **every** campaign, because the class binds to any build that summons an
  affordance. `unbound` is true when either of the first two counts is zero, and
  it is always paired with an `unbound_reason` naming WHICH arm found nothing:
  most campaigns bind the class and not the clause (`nobodys-cave-island`
  declares no region verb at all — 47 fixtures, 5 borne, 0 box selectors), and a
  bare `true` over 47 examined objects is how a reader learns to skip the field.
- `<out>/validation/chain-length.json`: the `DW0984` **binding ledger** (`compiler::chain`). `limit` (the pinned default of `max_command_sequence_length`), `step_budget`, `examined` (shipped functions measured, every datapack of the tree), `counted_loops`, `steps` (the world-build steps the delve's datapack ships), `longest` (`function`, `chain`), `refused` (empty on any build that wrote the file) and the `quantifier` the counts hold under: per executing source. The build prints `chain binding: …` before the verdict.
- `<out>/validation/observer-census.json`: the `DW0926` **binding ledger** (`compiler::observer`, spec-0077 §5). `functions_read`, `positional_player_selectors`, `exclude_observation_tag`, `allowed` (one row per `observer::ALLOWED` site: its count and its reason) and `unguarded` (0 on every build that ships, since one unguarded selector refuses it). Emitted on every build: a cutscene viewer carries the tag in any campaign, and a declared `world.respawn_wait` adds a second state that does.
- `<out>/validation/placement-gate.json`: the `DW0864` **binding ledger** (`compiler::edit`). One row per `scatter`/`plant` verb the build replayed — its batch, its index in that batch, what it `declared` (a `plant` count, or `null` where the verb states only a density), what it `delivered`, the `domain` of cells the author's region selected, and how many of those the verb could `act on` at all (`usable`). Rows are written whether or not any of them is short, because a rule that speaks only when it fires reads exactly like a rule that never looked. `binding` carries `verbs`, `scatter`, `plant`, `short`, `domain_cells` and `delivered`, every one of them derived from the rows rather than written beside them. Absent for a campaign with no edit script.
- `<out>/validation/piece-mating.json`: the `DW0780`/`DW0781` **binding ledger**
  (`compiler::faces`). What the piece-mating check examined, over the denominator
  the placement itself supplies: `placed` pieces, how many are `touching` another
  piece, `examined` abutting pairs, `judged` pairs (pairs a declared face
  crosses — equal to `examined` on any build that succeeded, since a shortfall is
  a `DW0780` refusal), `faces_declared` and `faces_bound`, and the split of
  pieces judged `by_contract` against `by_socket`. `allocated` says the world's
  ways were allocated by a site plan, which is why its zero is a zero. `examined`
  counts pairs and not declarations deliberately: a ledger keyed on declarations
  reads a library that declares nothing as an honest zero.
  Written on every build.
- `<out>/validation/piece-exposure.json`: the `DW0885` **binding ledger**
  (`compiler::burial`). What the piece-exposure check examined, over a
  denominator taken from the placement and the bytes rather than from the
  declarations: `placed` pieces, `examined` pieces (those the compiler has a
  prefab document for — a site plan's derived massing volumes are placed boxes
  no author can write a `shown_faces` in, so they bury and are not judged),
  `boundary_cells` solid on the pieces' own box boundaries, `exposed_cells`
  with neither block, box nor horizon in front of them, `judged_cells` standing
  in air the party can be in, `sides_seen`, `faces_declared`, `faces_bound`,
  `party_air_cells` and `party_air_cut_off`. `exposed_cells` beside
  `judged_cells` is the named residual: the party's air is followed two cells
  outside the content, so a piece standing well clear of everything can be
  exposed and go unasked, and the gap between the two numbers is the size of
  that. `horizon` is carried so a zero can be read against what the world had
  to bury with.
- `<out>/validation/traversal-gate.json`: the `DW0452`/`DW0453` proof's **binding
  ledger** (`compiler::traversal`, playtest-methodology.md rule 1). States what
  the traversal proof actually examined — `legs`, `route_cells`, and
  `legs_by_class` per `Locomotion` (`ground`/`climber`/`flier`/`aquatic`, counted
  under the class the proof USED, i.e. the declared one where a body declares) —
  plus,
  per rule, the objects it bound to (`gate_use.cells`, `surmount.rises`), and
  `unbound` with a `reason` when the campaign plans no walked leg at all. The
  per-class count is the point: every class that carries an exemption is a class
  the proof does not examine, so a total alone would report green over exactly
  the bodies it understands least. **`Locomotion` membership follows one stated
  rule**: `Ground` is the default AND the checked class, so every id vanilla data
  does not positively answer lands there (unrecognised ids included, and
  `minecraft:breeze` deliberately — it hops, it does not fly); a class may carry
  an exemption only when its membership is vanilla's own answer (`Aquatic` =
  `#minecraft:aquatic`, which exempts nothing at all) or a closed, cited list
  whose exemption is advisory-tier (`Climber` = vanilla's `Spider` and its
  subclasses; `Flier` = a closed, cited list of the mobs that leave the ground
  under their own power — both `DW0453` only, never the error tier, because
  routing is identical for every body: this compiler walks a flying body down the
  same ground A* a sheep gets, so a class may excuse the *surmount* question and
  nothing else). A second block, `declared` (spec-0034), states the
  **author's** side on the same page: `bodies` carrying a `traversal`
  declaration, `by_class` under which token, `exercised` (how many changed a
  verdict — anything less is `DW0454` and the build failed), and
  `advisories_waived`, the `DW0453` findings those declarations removed. A
  declaration silences a rule for a body, so a ledger that counted only what the
  rules examined would report green over exactly the bodies an author asked to be
  treated differently. `rules.jump_reach`
  is carried **declared-unbound on purpose**: per-entity `JUMP_STRENGTH` is
  server-code attribute data rather than registry data the compiler reads, so
  every rise is measured against the *player's* apex (`nav::MAX_JUMP_RISE_16`)
  for every body, and the ledger says so rather than leaving a reader to infer
  it from silence. Emitted only when the campaign assembles a world — "assembled
  nothing" and "examined nothing" are different facts, so the artifact is
  omitted rather than emitted claiming a zero it never measured.
- `<out>/validation/respawn-safety.json`: the `DW0478` proof's **binding ledger**
  (`compiler::nav::RespawnSafetyLedger`, playtest-methodology.md rule 1). Every
  respawn point (`anchor`, `kind` — `bonfire` or `set-checkpoint` — `pos`,
  `fire_step`, `reign_end`), every hostile force, `examined`, `pairs` (the
  comparisons actually made) and `credits`. Per respawn point it also lists
  `compared_against`, `not_compared` — each skip carrying a `kind` (`onset`,
  `flag-bound`, `bearer-bound`, `puppet`) and the reason it could not meet the
  party there — and `credited`: every compared pair whose geometry overlaps and
  which the campaign supplies evidence for, with its `kind` (`reset`,
  `dominated`), `reason` and `post_reset_state`. A credit is as auditable as a
  skip, and both kinds are **computed from the object**: there is no field an
  author writes to claim one. `unbound` is `pairs == 0`, and a `reason` is present exactly then,
  naming which of the three zeros it is: no respawn point, no hostile force, or
  both present and never contemporaneous. Without it a `DW0478` that made **zero
  comparisons** would pass and nothing would say so.
- `<out>/validation/press-bodies.json`: the `DW0426` proof's **binding ledger**
  (`compiler::pressable::PressLedger`). One row per `strike`/`use` trigger the
  proof resolved a body for — `trigger`, `click`, `anchor`, and WHICH body it
  landed on (riding a `close-gate` seal's hitboxes, riding a shortcut door's,
  riding an NPC's dialogue hitbox, arming a region's clickable shell, or a point
  in open air; a `strike-assembly` is recorded as riding the assembly's hitbox,
  with its size and cell, the `anchor` column then naming the assembly's mark,
  spec-0082) — plus `examined`, `unbound` and, exactly when unbound, a
  `reason`. `DW0426` is error-tier, so a build that ships proves no press lands
  on nothing; that sentence is equally true of a campaign that arms no press at
  all, and only the count tells the two apart.
- `<out>/validation/leave-proof.json`: the `DW0921` proof's **binding ledger**
  (`compiler::nav::LeaveBinding`): `configurations` (distinct quest
  configurations judged), `route_cells` (the critical path's route cells the
  proof was rooted at, summed over them), `reached` (cells a body reaches from
  them by walking, falling, jumping, swimming or climbing), `afloat` (of those, cells
  where the body floats at the top of water — the population the water half
  judges), and `trapped` (zero on a build that ships). Written only when the
  proof holds.
- `<out>/validation/assembly.json`: the assemblies' **binding ledger and staging record** (`compiler::assembly::AssemblyBinding`, spec-0082 §5.6): `declared`, `parts`, `clips`, `hitboxes`, `steps`, `refused`, `locks` (cells locked steps can lock onto, each proved), `writes_per_tick` (each assembly's part count over its fastest clip's cadence, summed) and `facings` (facings judged, one per landing per facing a blow can take), per strike step `windup_ticks`, `hold`, `strike_ticks` (to the landing: the strike's last frame drawn whole), the landings' `amounts`, every `caught` cell, `facing_count`, and per facing a blow can take its `k`, root `yaw`, `caught` cells, the cells the limb `comes_down` on and the `stand` cell that draws it — for a `locked` step one row per lockable cell, `k` its index, with the `clip` that strikes it and the cell itself as its `stand`; `spared` (per assembly, a cell outside its arming region) and `witnessed` (the assemblies the critical path's bot witnesses) — what a round summary states about a blow, and what the engine does not judge. Written for a campaign that declares an assembly, when the proofs hold. The same counts are the line every build prints, zeroes included: `assembly binding: A assembl(ies) declared, P part(s), C clip(s), H hitbox(es) examined, S strike step(s) checked over F facing(s) and L locked cell(s), R refused`, followed by `assembly cost: P display part(s) in all; at most W keyframe write(s) per tick at the declared cadences` beside spec-0082 §8 row 7's measured rate — the host meets the number; it never caps the capability.
- `<out>/validation/strand.json`: the `DW0924` **binding ledger**
  (`compiler::strand::StrandBinding`): `waves`, `stacks` (also as `examined`) and
  `seats` judged, `reached` (cells a member reaches, summed over stacks),
  `in_reach` (of those, within a strike of a party cell), `party` (cells the
  party can walk to) and `stranded` (empty on a build that ships). Written for a
  campaign that seats a wave a `kill` objective names, when the proof holds.
- `<out>/validation/wave-lethal.json`: the `DW0922` / `DW0923` **binding
  ledger** (`compiler::lethal::WaveLethalBinding`): `waves` (waves the seating
  pass seated), `stacks` and `seats` (what was flooded from), `volumes`,
  `reached` (cells a member reaches as built, summed over stacks), `openable`
  (barrier cells a player can open in the assembled world) and `reached_open`
  (cells reached with all of them open), and the `as_built` / `opened` findings
  (empty on a build that ships). Written only for a campaign that declares a
  volume, and only when the proofs hold.
- `<out>/validation/watch-ledger.json`: the `DW0810` proof's **binding ledger**
  (`compiler::watch::WatchBinding`). `declared_ids`, `campaign_functions`,
  `invoked`, `families`, `multi_object_families`, `unwatched_families`,
  `unwatched_family_objects`, `unwatched_family_members`, `watched_objects`,
  `unwatched_objects`, and `examined` = watched + unwatched — the per-object
  bodies the rule could judge at all. Plus one `unwatched` row per finding, each
  naming the family, the declared id, the emitted body nothing drives, and the
  siblings that ARE driven. The `unwatched_family*` keys are the rule's own
  stated limit: a family with no runtime proof whatever is reported here rather
  than diagnosed, **by name and with its members** (`unwatched_family_members`
  maps family prefix → declared ids) rather than as a bare total, so the scope is
  actionable instead of merely visible. `unwatched_family_objects` is the
  population `examined` deliberately excludes, stated beside it so the examined
  count is never read as the whole per-object surface.
- `<out>/validation/watch-claims.json`: the `DW0811` refusal's **binding ledger**
  (`compiler::watch::ClaimBinding`). `claims`, `declared_objects`,
  `bodies_judged`, `bodies_watched`, `examined` = `bodies_judged`, and one
  `breaches` row per undischarged member. Its own file rather than a section of
  the watch ledger because `tools/ci/check-gallery-coverage.py` reds on a top-level
  `examined: 0` and reads top-level keys only — nested, the count would be
  written, committed and diffed and never judged. Zero on a campaign declaring
  none of the claimed mechanic is honest; zero on the gallery, which declares
  everything, is the finding.
- `<out>/validation/combat-plan.json` (spec-0023): the bot ladder's encounter
  table. A top-level `fights` block states the **binding count for the whole
  spec-0023 pass** — `waves`, `actors`, `total`, `unbound`, `reason`
  (`combat::mandatory_fights`): every fight the party cannot walk away from, of
  BOTH shapes, because reading `encounters` as "how much combat is in this
  delve" would let a campaign whose fights are actors look combat-free
  and skip the whole spec-0023 pass. Then one entry per **mandatory wave**
  encounter (a wave a `kill` step on the compiled critical path names), in path
  order, carrying `wave`, `objective`,
  `step`, `tier` (`ordinary`/`elite`/`boss`, from the wave's declaration), `pos`,
  `count`, `respawns_on_rest`, the `checkpoint` governing a death at that
  encounter, and the `census` probe naming the three functions that measure this
  wave by tag. The document also states the `difficulty` the run is verified AT.
  Two of those fields carry rules worth stating exactly:
  - `step` is an index into the **exported** `critical-path.json` `steps[]`
    (`Plan::exported_step`). Two coordinate systems exist because of
    spec-0016 §1's rest splice: `plan.critical_path` is the compiler's own list —
    what every `CheckpointPlan::fire_step`, every nav proof and every internal
    index means — while the exported path additionally carries one `rest` step
    after the beat arming each bonfire, so they drift by one per bonfire armed
    earlier. **Every artifact a harness reads states exported coordinates**;
    `Plan::exported_step` is the translation for the main path, and
    `the_combat_plan_step_indexes_the_exported_path` pins it against the real
    emitted documents (the step the plan points at must BE the encounter's kill)
    rather than against the arithmetic. It is deliberately **main-path only**:
    spec-0025's per-branch paths resequence the same steps, so an index cannot be
    carried across at all and `emit::rest_step_index` translates through the
    *objective* the arming beat names instead. On the main path that translation
    is the identity, which is exactly what makes the simple count valid there and
    nowhere else. There is one `combat-plan.json`, over the main path, so this
    never crosses the boundary.
  - `census` is `{census, brand, unbrand}` — the fully-qualified ids
    of this wave's `wave_census_<wave>` / `wave_brand_<wave>` /
    `wave_unbrand_<wave>` functions. It exists so the harness calls what the
    plan NAMES: `safe_local` is a compiler naming rule, and a harness
    re-deriving it would be the downstream folklore CLAUDE.md forbids. Present
    on every encounter; a plan that cannot state it is refused by the harness
    rather than silently measured by silhouette.
  - **`run_backs`** (spec-0016 §1, spec-0023 §3; `combat::run_backs`) — the
    re-seated fights the path walks past again. A `respawns_on_rest` wave the
    path has cleared comes back when the path rests at a bonfire, stationed at
    its seat; a leg the path walks after that rest, within the wave's aggro
    radius of a cell it occupies and in sight of it, meets the fight again. For
    a player that is the souls run-back — fight it or run past it — and a plan
    that exported the leg as a plain walk claimed a leg that does not exist.
    Each entry: `wave`, `objective` (the kill that first cleared it),
    `bonfire` (whose rest re-seats it), `before` (the token — `obj/…` or
    `trigger/…` — of the step whose leg re-crosses it; a token and not an index
    because every per-branch path carries the same beat at a different index and
    reads this one plan), `tier`, `pos`, `count`, `radius`, `crossing` and
    `distance` (the routed cell that makes it a crossing), and `paths` (the
    exported paths — `critical-path` or branch slugs — that carry it). Per path:
    for each cleared wave and each bonfire rest the path performs at or after the
    kill, the FIRST walked leg after the rest that crosses is the run-back; once
    fought, the wave is down until the next rest. The aggro model is the one
    `DW0478` measures with (`nav::aggro_sources`: seated spawn cells, a lane's
    marched corridor with its drift margin, the declared `follow_range` or the
    default) plus the sight gate the aggro-edge ring uses
    (`World::has_line_of_sight` — vanilla's nearest-attackable-target goal is
    sight-gated). The first leg after a rest is routed from the fire
    (`LegRoute::rerouted_from`), because a rest is not a step and the party sets
    off from where it rested. The list is always present; an empty list is a
    measurement. The ladder reads and clears each due run-back before the step it
    names (see `docs/reference/tools.md`).

    **Why an encounter, not a route or a sprint.** A leg that passes a standing
    fight meets it. Routing round it is not available to the engine: the only alternative
    route may be a shortcut, and the path never opens an optional one (spec-0016
    §2 — the delve must be finishable the long way). Running past is a player's
    choice whose success is movement skill, exactly the variable spec-0023 leaves
    to the human; a bot sprint proves nothing a machine can repeat.

    **What no static proof assumed.** No engine proof models hostiles on a walked
    leg — `DW0311`/`DW0317` route geometry only — so none assumed a re-seated
    wave absent. `DW0478` measures every re-seated wave as present at every rest
    point, and its dominance credit (`respawn::Evidence::dominance`) reads the
    path's own meeting with a force as a forced encounter the machine playthrough
    proves winnable; for a re-seated wave that meeting is a run-back, which the
    playthrough fights as one rather than walking.
  - `checkpoint` is the last checkpoint/bonfire fired **strictly before** the
    step, omitted when there is none. Strictly, not "at or before": a
    `fire_step` is the step whose COMPLETION arms the checkpoint, and a death
    *during* step `i` happens while step `i` is unfinished — so a checkpoint
    armed by step `i` does not exist yet at that death. The souls-bonfire fixture
    shows it at its sharpest: bonfire 0 is armed by `obj/slay`'s completion, the
    very kill the encounter IS, so a mid-fight death returns the party to world
    spawn, not to that fire.
    Erring toward the stricter answer is deliberate — the die-retry stage asserts
    the party respawns at the governing checkpoint, so an over-generous claim
    here makes the proof measure the delve against a rest point the player never
    had. (A bonfire additionally only MOVES the respawn point when the party
    rests; the harness's own precondition covers that half.)

  - **`muster`** (`compiler::muster`) — what the wave DECLARES, phrased as
    questions the live bodies can be asked, plus the two staged functions that
    follow the reading. This is the half of a combat step that is a
    verification: a mob's `max_health`, `attack_damage`, `equipment` and `name`
    went into one `summon` line and were never read back, and the pinned server
    silently drops NBT it does not recognise (`HandItems`, the legacy
    `PatrolTarget` compound), so a declaration that
    never reached the body shipped a delve that booted, played, and was not the
    document. Fields: `probe` / `strike` / `chip` (the function ids, in the
    census's convention, so no harness re-derives `safe_local`), `scale` (1000 —
    three decimal places, because `movement_speed` is declared in blocks per tick
    and two cannot tell `0.230` from `0.234`), `unread` (`-1`, what a holder
    carries where the probe deliberately did not ask), `bodies`, `checked` (the
    binding count: declared facts this probe puts a question to), `types[]` and
    `profiles[]`. Each `types[]` entry is one entity kind with its ordered
    `facts[]` — `name=<text>` and `equipment.<slot>=<item>`, bit `i` of a body's
    mask being `facts[i]` — plus `dropped_facts[]` (a wave with more than 30
    identity facts, so a short mask can never read as a passing check) and
    `reads_attack_damage` / `reads_follow_range`, true only where EVERY stack of
    that kind declares the attribute: `attribute … get` on a body that carries
    none is an error, and Mojang publishes no per-entity attribute table
    (`DW0475`'s rule). Each `profiles[]` entry is one declared stack: `type`,
    `count`, `mask`, `label`, the declared `max_health` / `attack_damage` /
    `movement_speed` / `follow_range` (`null` where the stack leaves it vanilla),
    and `armor_at_least` / `armor_toughness_at_least` — the points the declared
    gear contributes, counted only for a piece worn in its own `equippable` slot
    (`combat::worn_as_armour`, the rule `DW0472` reads too), from the vendored
    item table, which is a FLOOR under the
    body's own `armor` attribute and never an equality, because a mob's base
    armour is not in any published data. The equipment a profile is built from
    comes from `emit::wave_equipment_slots`, the one resolution the `summon` line
    is also written from, so the probe can never verify a copy of the
    declaration instead of the emitted one.

  Together these are what turn a `kill` step into a verified encounter for the
  harness: which fights get the die-retry stage, where a death is supposed to put
  the party back, what each wave's bodies are declared to be, and the two
  functions that remove them once they have been read.

  **Validation metadata, not shipped gameplay** — excluded from the delve image
  like `critical-path-waypoints.json`, and emitted when the campaign has a
  mandatory encounter, so a combat-free delve's output is unchanged. Declaring a
  wave or actor `tier` therefore cannot move a shipped byte.
  **One caveat worth stating plainly** (spec-0023): `manifest.json` is the
  reproducibility index over the WHOLE output tree, `validation/` included, so a
  validation artifact is one line in the manifest's `outputs` map and moves no
  datapack, world, resource-pack or creator-overlay byte. The manifest indexes
  what the build produced, and pretending otherwise would make it a worse index
  for a cosmetic win.
- `<out>/render-plan.json` **player-POV shots** (`crate::render_plan::pov_shots`):
  the visual tier of the *player's own eye*, not the
  overhead/orbit cameras of the other shot kinds. One first-person `pov` shot per
  corner-thinned critical-path waypoint (the same `thin()` list the harness
  replays), camera at eye height (`1.62` above the standing cell), oriented along
  the walk toward the next waypoint and — at each leg's final waypoint — toward the
  objective anchor it arrives at. **The arrival aim is derived from the objective
  and from nothing else** (`render_plan::arrival_aim`): an anchor 4+ blocks away
  is framed directly; a nearer one is aimed at *along its own direction*, out to
  4 blocks at the anchor's height, so the objective is centred and near with the
  room behind it rather than dragging the pitch at the floor; an anchor inside
  the eye's own column is underfoot, no aim shows it, and the camera keeps the
  walked heading. That third case is the only one where the `expect` sentence
  stops saying the objective is ahead — **a shot's `expect` is a claim the
  emitter has to be able to keep** (`render_plan::Arrival` decides both together,
  which is why it is not a bool). Each
  shot carries `leg`, the served `objective`, `standing_cell`, a `camera` with the
  first-person `fov` (~70°), and an `expect` whose first entry is a one-sentence
  machine description composed from campaign data (area name + objective/anchor/NPC
  names + objective hint) — the (image ↔ expect) pair a vision model reviews.
  **Each POV shot states the configuration its leg is walked in** (spec-0089
  §8): `after: {step, cells_moved}` — `cells_moved` the cells whose block the
  configuration the leg arrives under (`path_configurations`' `per_step`) holds
  differently from the load world, over the world as shipped
  (`view::beat::picture_base`), and `step` the latest step of `plan.critical_path`
  before the leg's that carries an id (`null` when none does). A review frame
  still renders the server save at load, and `delvec scene` says so on every run:
  `review frames render the world at load; <k> of <m> POV shots stand in a
  configuration other than load` (`k` counts the POV shots with `cells_moved`
  above 0). `delvec snapshot --shot` reads one shot's camera and writes no plan,
  so its in-memory plan carries no `after`.
  Deterministic (route order → waypoint order; no RNG/clock) and appended after the
  overhead kinds. Emitted only when a
  walked critical leg exists.
- **`lighting` stamp** (POV + interior shots, `crate::render_plan::area_lighting_stamp`):
  pure metadata derived from the shot's area's **stage-1 declarations** — never from
  measurement (the measured model gates via `DW0210`/`DW0211`). `lighting` declared
  → `{"profile": "lit"}`; only `mitigation: "night-vision"` declared →
  `{"profile": "dark", "mitigation": "night-vision"}`; both → lit profile plus the
  mitigation; neither → **no key** (absent, not null), so campaigns without lighting
  declarations build byte-identically. Purpose: a declared-dark scene is pure black
  to an honest path tracer (exposure boosts cannot reveal a sealed cave — no
  light, only amplified noise — while real emitters render), so the stamp tells `delvec render` exactly which shots need its
  night-vision review emulation (below) and guarantees it touches no others.
- **Every camera's eye cell is proven clear** (`DW0724`,
  `crate::nav::verify_camera_eyes`). A camera whose eye cell holds a block
  renders the inside of that block, which is a picture indistinguishable from a
  picture of a featureless room. `render_plan::render_plan` is the only
  constructor of a plan document and it takes the assembled world, so there is no
  render plan that skipped the proof; every kind enters the shot list through one
  `push` that records the eye from the same position it writes into the camera.
  Six of the seven kinds place their camera at a fixed stand-off from a subject,
  and such a camera whose own cell is occupied stands instead at the furthest
  clear point on its own sight line (`crate::camera::stand_in_open_air`), stating
  `camera.requested_pos` and `camera.standoff` on that shot. `pov` is never
  moved — that eye IS the player's — so a `pov` violation stays a build error
  against the derivation. The plan carries the proof's binding counts:
  `"camera_eye_proof": {"cameras": N, "pulled_in": M}`. An `interior`
  overview's eye stands three courses over the highest of: its piece's top, a
  horizon's ground, the site's declared fill at the eye's column, and every
  placement or site-plan place stacked over that column (its ceiling course and
  roof, out to its eaves) — a place over another is a stack the overview looks
  down into, never a block its eye stands inside.
- **Every showcase camera photographs the scene** (`DW0724`, second shape;
  spec-0069). When the campaign has `design/cameras.json` (read by the loader,
  hashed into the manifest's inputs), `render_plan` proves each of its cameras
  after the plan's own: the lens cell is clear by the same `World::is_clear`, the
  lens lies within the world's build height (−64..320), and the ray along its
  view meets the box the scene's chunk list is cut from
  (`scene::loaded_extent`, widened to whole blocks), read back through the plan
  just built. A showcase camera is not a plan shot — `delvec cameras` emits its
  scene from the record — so nothing moves it: a violation refuses the build
  naming the row, telling a hand camera to be placed again (`/trigger dw.cam`)
  and an estimate to be moved, and never the geometry. A record its reader
  refuses stops the build as `DW0721` (exit 3). The count is
  `camera_eye_proof.showcase`, present only when the record is.
- **`sky` fact** (`crate::render_plan::sky_fact`): **the hour and weather this
  delve is played at**, stated for the render layer exactly as `horizon` is —
  `{"time": "dusk", "daytime_ticks": 12000, "weather": "rain"}`, the author's
  spelling (the keyword, or the celestial object as written — no phase key,
  because no consumer of the plan can act on one), the vanilla `daytime` value it sets, and the declared initial
  `weather` keyword. Always present, because `world.json`'s `time` and `weather`
  are required. It is the **declared initial** hour — what the world save
  is written at, and therefore what every frame is of; a campaign that moves the
  clock with `set-time` reaches other hours at play (`DW0890` holds the design's
  rows equal to that whole reachable set), and a still frame has one sun and is
  not evidence about the beats after the cut. The renderer derives the sun from the **ticks** and never
  from the keyword, so a state vanilla does not name is worth as much as one it
  does. The **weather** reaches a frame as an overcast sky (`scene::sky_of`,
  below): the pinned Chunky core draws no rain, but it draws the sky's radiance
  and colour, the sun's intensity and disc, and fog. The panorama and the review
  frames take this sky; a showcase camera takes its picture's (§7 `delvec
  cameras`).
- **The Chunky sun** (`crate::view::scene::sun_at`, used by `delvec scene` **and**
  `delvec panorama`): an altitude of `90° − α` on the western half of the arc and
  `α − 270°` on the eastern half, and an azimuth of exactly east or exactly west,
  where α is the pinned day timeline's `visual/sun_angle` at the plan's
  `daytime_ticks` — two keyframes at 6000 (360 then 0) eased by `cubic_bezier
  [0.362, 0.241, 0.638, 0.759]`, read from the vendored file (spec-0081 §4.3), the
  same curve the celestial position table is derived from. minecraft.wiki's
  closed form (*Daylight cycle* §Sky angle) and the game's pre-timeline
  `DimensionType.timeOfDay` are kept as the second method in
  `the_two_published_sun_angle_formulas_agree`: the two closed forms agree to
  1e-9 at every tick, and the pinned track agrees with them to 0.056° (worst at
  tick 11457). The sun rises in the east and stands overhead at noon, so one
  angle fixes the whole position; the curve is not linear in ticks, which is the
  term that makes vanilla's sunrise and sunset linger near the horizon. A camera
  whose `sky.time` is celestial renders under the sun its position names; **the
  pinned Chunky core draws no moon** (its sky package holds no moon), so a night
  frame carries no phase and every `sky:` line whose moon is up says *moon not
  drawn by the renderer* — the phase is confirmed in the game, never on a
  render. Chunky's direction toward
  the sun is `(cos az·cos alt, sin alt, sin az·cos alt)` (pinned core's
  `Sun.initSun`) and nothing clamps the altitude, so `night`/`midnight` emit a sun
  below the horizon and those frames render dark — which is what a night delve
  looks like. **A plan with no `sky`, or a `sky` with no `weather`, is
  `DW0721`**, in `scene`, `panorama` and `cameras` (`scene::plan_sky`), naming
  the plan as written by an engine older than the running one: Chunky's own
  default is a 60° midday sun under a clear sky, so emitting anyway would hand
  back a noon frame of a midnight delve, or a clear frame of a rain delve, and
  say nothing.
- **The sky of a scene** (`crate::view::scene::sky_of`, spec-0079 §4): the one
  writer of every `sky`, `sun` and `fog` key under `compiler::view`; the review
  frames (`scene::scenes_from_plan`), the panorama and every showcase camera
  (`camera::world_scene`) take their sky through it. **`clear`** is the hour's
  sun direction and nothing more — Chunky's simulated sky with its default sun is
  a clear sky — so a clear scene's bytes are the bytes the engine wrote before
  the weather reached a scene (pinned by `clear_scenes_keep_their_base_bytes`
  against goldens emitted at spec-0079's base). **`rain`** and **`thunder`** write
  the whole overcast block on every scene: the sun's direction from `sun_at`,
  `sun.intensity`, `sun.color` and `sun.drawTexture false`; `sky.mode
  SOLID_COLOR`, `sky.color`, `sky.skyLight` (the light the sky casts) and
  `sky.apparentSkyLight` (the brightness the lens sees); `fog.mode UNIFORM`,
  `fog.uniformDensity`, `fog.color` and `fog.skyFogDensity 0` (so the fog colour
  never paints the sky). The block's values are a cell of a **look table** keyed
  by weather and by a **daylight class read off the emitted sun's altitude**,
  never the hour's name: `high` at or above 20° (`noon` 90°, `day` 27.55°), `low`
  from 0° up to 20° (`dusk` 12.37°), `below` under 0° (`night`, `dawn` −3.52°,
  `midnight` −90°). Every cell shares `sky.color` (0.10, 0.11, 0.14), `sun.color`
  (0.9, 0.75, 0.65) and `fog.color` (0.20, 0.22, 0.26):

  | Cell | `skyLight` | `apparentSkyLight` | `sun.intensity` | `fog.uniformDensity` |
  |---|---|---|---|---|
  | `high` × `rain` | 3.2 | 1.2 | 0.25 | 0.002 |
  | `low` × `rain` | 1.6 | 0.6 | 0.25 | 0.002 |
  | `below` × `rain` | 0.16 | 0.06 | 0 | 0.002 |
  | `high` × `thunder` | 1.92 | 0.72 | 0.125 | 0.003 |
  | `low` × `thunder` | 0.96 | 0.36 | 0.125 | 0.003 |
  | `below` × `thunder` | 0.096 | 0.036 | 0 | 0.003 |

  `low` × `rain` is the overcast dusk measured on one delve
  (`docs/reference/showcase-shots.md` §2b); the other five derive from it by two
  rules — `high` doubles both sky-light modifiers, `below` takes a tenth of them
  and puts the sun (under the horizon) at 0; `thunder` takes its class's `rain`
  cell at 0.6 of both modifiers, half its sun and 1.5 times its fog — asserted by
  `the_look_table_is_the_measured_cell_and_two_rules`. This table is the record
  of what the engine emits; the constants are `scene::{HIGH,LOW,BELOW}_{RAIN,
  THUNDER}`. A night-vision review emulation (`REVIEW_POLICY`) is a material
  override and independent of the sky: a declared-dark room under rain gets both. The panorama's
  bearing decides only which side is in shot, and a bearing that looks into the
  sun at the declared hour is a backlit frame the creator re-shoots from
  elsewhere.
- **`horizon` fact** (`crate::render_plan::horizon_fact`): the world-generator
  ambient the render layer cannot see. A `horizon: ocean` campaign (spec-0013)
  ships a world save holding only the chunks its layout occupies — the sea around
  the island belongs to the level generator — so a renderer loading that save
  draws void past the shoreline unless it raises its own water plane, at exactly
  the compiler's sea-level datum (anywhere else leaves a two-tone seam against the
  authored block water). The plan therefore *states* it,
  `{"kind": "ocean", "sea_level": 62}`, rather than leaving `delvec render` to
  infer a generator setting from blocks. `horizon: void` (default/absent) emits
  **no key** (absent, not null), so every campaign that declares nothing keeps a
  byte-identical `render-plan.json`. A `valley` states the `extent` of the
  landform it built: every scene loads it, and `delvec panorama` fits its frame
  to `layout_aabb` (or to the anchors `--subject` names) and never to the
  landform, so the ground is the built place's setting and never what the frame
  is sized to.

### A piece's blocks arrive as one template or as a tile set

Vanilla caps a structure template at **48 blocks per axis**. That is a limit on a
file format, never on a design, so a zone past it ships as several `.nbt` tiles
plus one manifest (`structure_set`, `delvewright_dsl::split::TileSet`) — and
**tiling is packaging, not authoring**. A campaign binds `prefab/<id>` the same
way whichever packaging the piece uses; its anchors, spatial contract and
connectors are in whole-zone coordinates, because a cut never moves a mark.
Nothing an author writes mentions 48.

The compiler absorbs the difference at exactly one place. `PrefabMeta::templates()`
answers with the templates a piece's blocks arrive in — one at offset `[0,0,0]`
for a single-file prefab, one per tile at its manifest offset for a zone — and
`plan::PiecePlacement` carries them as `templates: Vec<PlacedTemplate>`, already
resolved to world positions. Above that line a piece is one piece: one entry in
`AreaPlacement::pieces`, at the whole zone's `size`, with one rotation, one set of
seals, one pool draw, one row in the face-mating check. Below it, everything that
touches blocks loops over templates and never asks how many there were:

- `place_all` emits **one `place template` per template**, at
  `piece.pos + rotation(tile.offset)`. Vanilla rotates about the placement
  position, so that composes to the whole zone rotated about the piece origin.
- `place_verify` takes **one sentinel per template**. A `place template` can land
  for one tile and fail for another (an unloaded chunk is how), so a per-piece
  sentinel would report a zone placed when eight ninths of it was there.
- the datapack ships one `structure/<id>.nbt` per template;
- `forceload` spans the piece AABB, which is the whole zone;
- the assembled-world model reads each template's cells at that template's world
  position, so the model is the zone the cut never happened to;
- the stage-7 `fragment` verb stamps every template of the prefab it names.

A single-template prefab emits one template, at the piece's own position.

**A tiled zone's extent is the whole zone, and more than one proof depends on
it.** `PiecePlacement::size` stays the zone's — tiling is absorbed below that
line — so `bbox()` is the whole building, and everything that asks *where does
the content end* gets the same answer for a tiled zone as for a single-template
one: the `forceload` span, the piece AABB `DW0780` compares, massing's
footprint, and `nav::built_volume`, which is what `DW0318` asks whether a fluid
cell is inside. That last one is the load-bearing agreement between two features,
and it is invisible from either: had the extent become
one tile, a tiled zone's own indoor water would read as having run out of the
world. `crates/delvec/tests/integration_pairs.rs` holds it.

`DW0803` is the invariant that keeps the two halves honest — the metadata's
declared size against the bytes' own `size` tag.

### Assembled-world model (shared, gravity-settled)

`crate::assembled` builds the one authoritative cell→block map of the world the
shipped delve actually assembles — placed prefab structures (`/place template`),
socket seals, gate clears — **then settles gravity-affected blocks**.

**One assembly per build.** `emit::build` assembles the world once, before the
gravity check (`DW0313`), and every later pass borrows that `Assembled`; a
campaign with an edit script hands it to the replay (`edit::replay_taking`),
which edits it in place, and every pass after reads the edited copy. The map is
`Arc<BlockMap>` (`blockstate::BlockMap`, interned values) so a reader that needs
it whole — the light model — shares it instead of copying it; the replay is its
only writer. The geometry is classified once too: `light::geometry_world` is
the premise-free nav world relight surveys, and the campaign's own world is that
world's cells under `nav::Premises::of_plan` (`World::with_premises`) plus
relight's colliding fixtures. The nav `World`'s cell sets are copy-on-write
(`cellset::CellSet`/`CellMap`): a derived view — a quest configuration, a sealed
gate, a counterfactual — shares the bulk and stores only its own edits, and a
set that fills its bounding box is held as a bitset. Iteration over either is in
`[i32; 3]` order, as a `BTreeSet` iterates.

**A template places what the game places** (spec-0098 §7). Every cell a
template names is written — **air included**, which carves the mass and any
earlier block under it, exactly as `/place template` does — except a
`minecraft:structure_void`, which places nothing, so whatever stood there shows
through, as at a cell the template omits. This is what a piece's voids rely on:
a bound piece holds `structure_void` at every cell of its frame its place does
not own, and the model reads the neighbour's wall, the ring's fixed ground or
the fill standing there. (The surround's templates do not carve the map: the
map wins the argument, and the surround is read without its air.)

**Socket sealing is a property of a placed piece, not of the layout solver.**
`solver::seal_layout` runs over the placed pieces of **every** area, pool or
single-prefab: a mated socket's jigsaw block is cleared to air, leaving a clean
3×3 passage; an unmated one is walled with `minecraft:stone_bricks` over the
connector's whole opening, which also overwrites the `minecraft:jigsaw` marker
standing in its sill. A single-prefab area places exactly one piece and so has
nothing to mate with — every connector its prefab declares is unmated and is
walled. That is why the model never contains a jigsaw marker, and why `DW0322`
does not read a connector's doorway as a walkable cell one step from a void drop.
A prefab that declares no connector yields no seal at all.

**Blockstates rotate with their piece**. `/place template … <rotation>`
rotates a structure's blockstates as well as its cell positions, so
`placed_blocks` applies `assembled::rotate_state` to every palette name it
places: `facing` (horizontal only — `up`/`down` are yaw-invariant), `axis`
(`x` ↔ `z` on a quarter turn), the 16-step sign/banner `rotation` dial, the
`north`/`south`/`east`/`west` connection set (permuted simultaneously, so a
wall's `none`/`low`/`tall` values travel with their side), crafter/jigsaw
`orientation`, and **rail** `shape` only — a *stair's* `shape`
(`inner_left`, `outer_right`, …) is expressed relative to its own `facing` and
is already correct once `facing` moves. Every occupancy classifier reads only
rotation-invariant properties (`type`, `layers`, `open`, `waterlogged`,
`bottom`), so the rotation moves nothing in nav, seating, relight, snapshots or
emission; the `DW0430` stair proof is the consumer of `facing`, and without the
rotation it would report a false defect on every rotated piece.

The map stores **full blockstates** (`minecraft:oak_slab[type=top]`), not bare
ids: waterlogging, slab halves and snow-layer counts are block *state*,
and the fluid and step models below are wrong without them. Consumers that need
the bare id call `crate::assembled::base_id`; state-sensitive rules read their
property with `state_value`.

The
delve ships into a void flat world (no natural floor), so a vanilla
`FallingBlock` (`sand`/`red_sand`/`gravel`/`*_concrete_powder`/anvils/`dragon_egg`)
placed unsupported by `/place template` immediately falls out of the world and
leaves air. Settling reproduces this per `(x,z)` column: non-falling **solid**
blocks are immovable supports (stone floats), each falling block drops onto the
highest support at or below it, and a falling block with no support anywhere below
it despawns into the void. **Fluids are not supports**: vanilla's
`FallingBlock.isFree` counts a water/lava cell as free space, so a falling block
sinks straight through and lands on the first genuinely solid block, *displacing*
the fluid in the cell it rests in — and a gravity block over water with no floor
beneath still despawns.
`pointed_dripstone`/`scaffolding` attach upward / by
support-distance and are deliberately not settled by the below-support rule (a
ceiling stalactite must not be mistaken for an unsupported floor block). Both the
nav occupancy model (`crate::nav::World`) and the relight light model
(`crate::light::LightModel`) derive from this single settled map, so a `sand`
floor laid over void is a *hole* in every consumer — DW0311 walkability, DW0312
wave seating, the relight pass, and the waypoint export — exactly as in game, not
a phantom floor the model wrongly "proves" solid. Determinism
(ADR-0006): fixed placement/seal/gate order, `BTreeMap`-ordered column iteration,
bottom-up stacking.

The settle pass also feeds the **`DW0313` gravity-despawn gate**: a gravity block
that despawns (falls with no support anywhere below) is always a defect — no DSL
verb can intend it — so `crate::assembled::gravity_despawn_error` fails the build
directly at its start, before any consumer, listing the offending pieces/cells and
prescribing a substrate. This is the authoritative gate for the pitfall; a fall
that merely lands on support is left to the faithful settle model (no diagnostic),
and the tileset generator's own zero-unsupported invariant catches unintended
falls at authoring time (strongest-form defence, per the debug doctrine).

The settle pass is followed by a **fluid-flood pass**, the fluid peer of
gravity settling. Free-fluid cells — `minecraft:water` **and `minecraft:lava`**,
namespace- and state-insensitively (`crate::assembled::is_fluid`, the one predicate
a runtime `fill-region` is classified by too) — **and every `waterlogged=true`
block** seed a deterministic, **conservative superset** of vanilla flow (mirroring spec-0010's never-overestimate-walkability
stance): (1) infinite-water source formation — a supported air cell flanked by ≥2
source cells becomes a source, cascading, so a walled pool basin fills completely,
not just 7 cells from its seeds; (2) 7-level horizontal decay from the completed
source set plus infinite downward flow. Vanilla's drop-seeking *direction* rule is
omitted (spread goes every way), which only over-marks. Every flooded cell (any
fluid level, plus sources) is **impassable and never standable floor** for every
consumer — nav, wave seating, relight fixture placement, waypoint export — the same
single-model discipline as settle: a `cave-shore` pool floods `[261,66,1]`, and
no talk-to leg routes a step-up through it.

**Both fluids, one answer.** A body stands on lava no more than on water, so the
classifier asks `is_fluid` and both land in `flooded`. Lava's own flow differs —
overworld lava decays over 3 cells and forms no new sources, and a delve ships into
an ordinary overworld (a superflat over the delve's own void biome, never an
ultrawarm dimension) — so running it through the water flow above **over-marks** a
lava pool's reach, which is the permitted direction. A free fluid is deliberately
not a flood *barrier* either: water spreads through a lava cell rather than being
dammed by it, and the stone or obsidian vanilla would make of that meeting is a
solid the model declines to invent. The cell stays impassable and not floor.

**Waterlogging is water**. Since MC 1.13 a `waterlogged=true` block's
cell holds a genuine water *source* that ticks and spreads into adjacent air
exactly like a free source (place one waterlogged stair on dry land and water
flows around it). A model that stored bare ids and asserted the opposite
("waterlogging never spreads to a neighbour") would **under-mark** the flood —
the one direction the never-under-mark contract forbids, since an under-marked
cell ships as proven-dry and strands the bot. A waterlogged cell is therefore both
a flood source *and* its host block's normal collision class: nothing walks or flows
*into* it, and `flooded` stays disjoint from every block class (it means "a walker
would be in open water here").

Trap triggers (spec-0011): `*_pressure_plate`, `tripwire`, and `tripwire_hook`
(`crate::assembled::is_passable_trap_trigger`) are non-collidable in game, so
`collision_top_16` answers 0 for them and the occupancy model treats their cells
as **passable** rather than solid. The predicate keeps its own name because a
caller needs the *trap* fact as well as the collision fact, and the two cannot
disagree because one is computed from the other. This is the
faithful model — a plate rests on a solid support block below, so standability is
unchanged — and it is load-bearing for the `DW0342` trap proof: a player must be
routed *onto* a trigger cell (so the compiler can prove the trap avoidable or not),
never around a phantom "solid" plate that would call every trap avoidable.

**Where the collision table lives.** `delvewright_dsl::blockshape` holds it,
and `crate::assembled` re-exports its names by `pub use` — there is one
definition in the workspace and no second copy for anything to drift from. It is
in `delvewright-dsl` for the reason `metrics::step_allowed` is: `delvec` is
published and may depend only on published crates, so the one crate the grammar
back end, the admission pipeline and the compiler can all reach is that one. And
it is the right home by object class — a collision box is a fact about a vanilla
block state under the pinned game version, in the same sixteenths as the
auto-step and jump-apex budgets it feeds.

`blockshape::collision_class` is the rule, and every consumer asks it:
`occupancy_of` below, the grammar back end's `Voxels` impl, and `delvec prefab`'s
light probe. `Collision::passes_body` / `supports_body` / `floor_top_16` are the
three answers a walk gets.

**Collision classes (`crate::assembled::Occupancy`).** Cells are classified:

| Class | Blocks | Walk through? | Stand on top? |
|---|---|---|---|
| solid | every other non-air block (full-cube, the conservative default) | no | yes |
| tall barrier | `*_fence` (incl. `nether_brick_fence`), `*_wall` — 1.5-tall | no | **no** |
| use-gate | closed `*_fence_gate` (1.5-tall, right-click-openable) | player: yes (USE); autonomous mobs: no | **no** |
| passable | open `*_fence_gate` (block state `open=true`, read from the prefab palette **or written by a stage-7 edit** — see below), trap triggers, thin decoration (< 8/16 collision) | yes | no |
| flooded | fluid reach — `minecraft:water` / `minecraft:lava` (`is_fluid`), plus waterlogged sources | no | no |
| partial | a solid cell's true top-face height in sixteenths, when < 16 | no | yes, **at that height** |

**Partial floor heights.** `crate::assembled::collision_top_16` reports
a block's collision-box top face in sixteenths, against the 1.21.11 shapes:

| Block | Height | Note |
|---|---|---|
| `*_slab` `type=bottom` (**the default state**) | 8/16 | a half-step |
| `*_slab` `type=top` / `type=double` | 16/16 | the face is the cell top |
| `snow[layers=N]` | `(N-1)·2 / 16` | `layers=1` (the default) has **no** collision box; `layers=8` is 14/16 |
| `*_carpet`, `moss_carpet` | 1/16 | `pale_moss_carpet` only when `bottom=true`, else 0 |
| `dirt_path`, `farmland` | 15/16 | |
| `candle`, `*_candle` | 6/16 | every candle count has the same 6-pixel box, so a candle on a floor is stepped over |
| `flower_pot`, `potted_*` | 6/16 | |
| `lantern`, `soul_lantern`, `*copper_lantern` | 9/16, 10/16 `hanging=true` | two stacked boxes, the cap on top; a floor lantern is a step a mob takes onto a curb (content measurement: top at the cell floor + 0.5625). `sea_lantern` and `jack_o_lantern` are full cubes |
| no-collision fixtures (`assembled::is_no_collision_fixture`) | 0/16 | torches (`torch`, `*_torch` — wall, soul and redstone alike), signs and banners (`*_sign`, `*_banner`), `lever`, `*_button`, rails (`rail`, `*_rail`), `redstone_wire`, `light`, `structure_void` — vanilla declares every one of them `noCollission()`. A wall torch occupies the air cell beside the wall it is fixed to, and a model that calls that cell a full cube severs a corridor. **Deliberately excluded, and not because they collide**: `fire`, `soul_fire`, `cobweb` and the portals also have empty collision boxes and stay full cubes here, because a body that *passes through* one is not a body that may be *routed* through one. Also excluded because their shapes are not read out of the pin: lanterns, chains, end rods, ladders. |
| climbables (`blockshape::climbable`, every member of the pinned `#minecraft:climbable` but `scaffolding`) | — (class `Climbable`) | `ladder` (a 3/16 panel against its support), `vine`, `weeping_vines`, `twisting_vines`, `cave_vines` and their `_plant` bodies: a body passes the cell and never stands on it; what holds it there is the climb (spec-0099, below). `scaffolding` stays a full cube |
| no-collision vegetation (`assembled::is_no_collision_plant`) | 0/16 | grasses/ferns, every small and tall flower, `pink_petals`/`wildflowers`/`leaf_litter`, saplings, crops, mushrooms and nether flora, kelp/seagrass, vines/`glow_lichen` — vanilla gives them an **empty** collision shape. Modelling them as full cubes makes a plant cell a phantom standable surface, which refuses valid geometry (a tuft on a terrace splits a 2-block riser into two climbable 1-block steps) and, worse, accepts invalid: a walkability proof that stands a body ON a tuft is unsound, and a flower cell measures light 0 as if it were opaque. The list is the **class**, never the ids one generator happens to scatter; lookalikes that DO collide (`azalea`, `big_dripleaf`, `bamboo`, `cactus`, `pointed_dripstone`, `sea_pickle`, leaves, …) keep a box a body cannot walk through: a full cube, or the measured floor height of the next row (`cactus` 15, an upward dripstone tip 11). Fidelity consequence: plant cells no longer dam the water-flood model either — vanilla water flows into and breaks them. |
| every other block whose pinned collision box rests on the cell floor and tops out at 8–15/16 (`blockshape::measured_collision`) | the measured top | read from `crates/dsl/data/collision-tops-1.21.11.tsv`, the vertical extent of `BlockState.getCollisionShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO)` for every blockstate of the pinned server jar (`tools/maintenance/dump-collision-tops.py`; 29,671 states in 5,724 rows). A property the name leaves out is read at the block's pinned default, so a bare `pointed_dripstone` is the upward tip it places: 11/16, the feet of a body standing on it 0.6875 into the tip's cell. Chests, beds, soul sand, mud, honey, cactus, cakes, lecterns, skulls and the rest of the class come in at their own tops by the same rule |
| everything else | 16/16 | the conservative default: a box under 8/16 no row above names, a box that does not start at the cell floor (a downward dripstone tip is 5–16), a box over a block tall, or an empty box this table refuses to route through |

**Every height in this table is the jar's, or the default.** `blockshape::tests::every_height_is_the_jars_or_the_full_cube_default` asks `collision_top_16` of every measured row: it must equal the measured top (0 for an empty box) or be 16, and where it is 16 the measured box must not be a floor at 8–15/16 — the class a dripstone tip modelled as a full block was in. One exception is counted rather than passed: a wall hanging sign's bracket is a 14–16/16 box at the cell top that the `*_sign` fixture rule reads as empty (12 rows), the direction that admits a step the game refuses; it is an open defect, not a ruling.

A block under 8/16 is **thin decoration**: its cell is passable and is never a
floor level of its own (a walker stands on whatever is below it, and a 2-high
corridor with a carpet in it stays walkable). At 8/16–15/16 the cell blocks
passage but its walkable face is recorded in `Occupancy::partial`, which is what
makes the nav step rule physical rather than cell-counting — see below.

Modelled **precisely**: fences, walls, fence gates (open vs closed), trap
triggers, thin decoration, free fluid and waterlogging, and partial floor heights.
**A body climbs (spec-0099).** A climbable the assembled world **keeps** is a
fourth kind of place beside standing, floating and falling: a body whose feet
cell holds one, with its body's cells unoccupied, **holds** there
(`World::climb_cell_fp`), standable or not. Which climbables the world keeps is
read from each block's pinned `canSurvive` rule over the block map, to the fixed
point the game's shape updates cascade to (`assembled::climb_holds`,
`keep_climbs`): a ladder needs a sturdy face on the block behind it, a vine a
full face beside one of its set faces or the vine above carrying the same face
(its `up` face is not counted), a weeping, twisting or cave vine the same plant
or a sturdy face on the block it grows from. Faces come from
`crates/dsl/data/faces-1.21.11.tsv` (`dump-faces.py`). A climbable whose hold
fails is air to the model and is recorded for `DW0991`; a runtime write that
touches a climbable's cell or its hold takes the climb away (`World::drop_unheld_climbs`).
The climb's moves (`World::climb_moves_fp`) are up, down, off the side (level,
or one lower), over the top from the top rung onto a standable cell one higher,
off the bottom onto the floor one under, and in from a standable cell beside (or
one higher, the climbable catching the step off the brink). The route relation
(`neighbors_fp`) takes them, so every route proof does; a body holding in mid-air
walks nowhere. `body_moves` and `mob_moves` take them too, with a step off the
side, letting go at the bottom of a run, and the **catch**: a fall is stopped by
the first climb cell whose top is at most `metrics::climb_catch_fall_blocks()`
(7, derived from the fall law: past it a body moves a block a tick and can pass a
one-block cell between ticks) under the body's feet. `fatal_step_off` slides a
caught body to the bottom of its run and asks the fall below it. Scaffolding and
the open-trapdoor-over-ladder rule are not modelled (spec-0099 §3.3–3.4). A
ladder dams a flood; a vine does not, and a flooded cell holds no climb.

Modelled **conservatively** — treated as a full solid cube, never as
walkable-through: stairs, doors, trapdoors, and every other partial-collision
block whose box is not a floor at a measured height. The tall/gate classes keep the proof from standing a player ON TOP of a
1.5-tall `oak_fence` (a "legal" +1 step no vanilla player or bot can perform),
so a gateless fence ring around a required anchor fails `DW0311`. A tall/gate
cell is never valid floor, which also
models the barrier's upper half blocking same-level walk-overs for free. Closed
fence gates are **use-gate** edges: walkable for the player (adventure-legal
right-click, the same action a human performs), exported first-class per leg (see
`use_gates` above). They remain routable edges for scripted `move-npc`/`move-actor`
tp polylines too — but routing a puppet over a player-only edge is a build
error, `DW0452`, because a tp'd puppet performs no interaction and no runtime
verb ever opens a gate, so "the firing beat's fiction controls the gate" is an
assumption nothing proves. The edge stays available so the diagnostic can name the cell and the reason rather than
degenerating into an unroutable `DW0307`.

**A stage-7 edit can author an open gate.** `Assembled::open_gates` — the side
set `occupancy_of` reads to tell a closed gate from an open one — is filled from
the prefab palette, and `edit::write_cell` re-derives the marking from the
blockstate it writes, so an edit writing `minecraft:oak_fence_gate[open=true]`
is modelled open by every proof downstream (the authorable fix for `DW0452`).
Autonomous placement (`spawn-wave`
seating) uses the no-gate-use view (`World::without_gate_use`): a spawned mob is
never seated in a gate threshold and the seating flood never spills through a
closed gate. Cutscene dolly clipping (`DW0308`) treats fence, wall, and gate
cells as solids — they contain visible geometry. Every classified block except
thin decoration (the `Thin` class: carpets, plates, torches, no-collision plants
and fixtures) dams the flood — fences, walls and gates, open or closed, included
— and a thin cell is flooded like air (`assembled::occupancy_of`).

### Entity placement: cells are centred, blocks are not

Every entity the compiler **summons or teleports** is positioned at
`nav::cell_center(cell)` = `(x + 0.5, y, z + 0.5)` — the horizontal centre of its
proven-walkable cell. Block-targeting commands (`setblock`, `fill`, `place`,
`spawnpoint`) keep the bare integer cell, which is the coordinate space they take.

The distinction is load-bearing. A block cell `(x, y, z)` spans `[x, x+1)`, but an
*entity's* position is the centre of its AABB, so summoning at the bare integer
coordinate parks the body on the corner where four columns meet: a 0.6-wide villager
at `x = 7.0` occupies `[6.7, 7.3]`, i.e. **70 % of it inside column 6**. Against a
wall that is an NPC standing in the wall, and along a walked path it is a body
visibly passing through blocks. The conversion is made at emission; nav routes
cells. The *route* is strictly cardinal — `neighbors_fp` offers four horizontal
moves and no diagonal transition exists — but the **walked polyline** is not,
since it is string-pulled off that route (next section), so corner-cutting is
refused by an explicit swept test.

Applies to: NPC bodies + interaction hitboxes, `spawn-npc` entrances, actor puppets,
wave mobs, `interact` hitboxes and `interact`/`reach` wayfinding markers,
environment-trigger and trap-disarm interactions, and every `move-npc`/`move-actor`
waypoint. Cutscene dolly cameras use centred coordinates too
(`nav::camera_points`). **Player** teleports keep integer coordinates: vanilla
resolves player-vs-block overlap by pushing out, and the `dw:cp` mirror is a
documented int-triple contract (spec-0013).

**Vertical steps interpolate as an L, not a diagonal.** A one-block step up rises
over the source column first, then crosses at the new height; a step down crosses at
the source height, then drops. A straight lerp between the two cell centres would
drag the body through the corner of the step block — the stair-shaped instance of the
same artifact. Both legs of the L stay inside cells `standable_fp` + the jump
head-clearance rule already proved clear.

### A walked path goes straight where the ground allows

A walk is **routed** by A* over a four-connected voxel grid, so the route is always a
staircase of axis-aligned segments. That is invisible at room scale and unmistakable
across a courtyard or a wall-walk: a body zig-zagging along gridlines reads as a
machine tracing a floor plan rather than a person walking a yard.

So the route is **string-pulled** before it is resampled (`nav::smooth_walk`): from
each kept vertex, extend while the straight segment to the next route cell is
walkable in its own right, stop at the first cell it is not, keep that one, repeat.
The cells in between are dropped, and `resample_body` then lerps straight across.

The test that decides it (`World::segment_walkable_fp`) is the **same standability
rule A\* stepped with** — `standable_fp` per column, so the lethal-volume question and
the floor/headroom question are asked exactly as the route asked them — evaluated
over the columns the body's AABB actually **sweeps** rather than the columns it would
occupy centred on a cell. Off a cell centre a body straddles up to four columns, and
a diagonal is off-centre almost everywhere. The swept set is the exact Minkowski sum
of the segment with the body box (`nav::segment_meets_cell_16`, a 3-axis
separating-axis test in sixteenths of a block), **not** a sample grid: a sample grid
can step over the sliver of a doorway jamb, and a doorway corner is exactly where
this must not be wrong. Intervals are closed, so a hitbox that merely grazes a column
counts as entering it — the direction that can only refuse a diagonal, never admit
one.

**Smoothing is level-only, and that is the stated limit.** A run merges only while
both endpoints and every swept column share one cell `y` *and* one true feet height
(`feet_16_fp`). So a rise, a drop, a slab lip, a stair edge and a closed fence gate
each **end** a run and survive as their own vertex, rendered by the cardinal step
shape above exactly as before. A closed fence gate needs its own clause because it is
deliberately not `is_occupied` (the player opens it with a click), so without it a
diagonal could be routed through a shut gate — or past the one the traversal proof
recorded the leg as using. A route with no level run of three or more cells comes
back unchanged.

**A killing volume needs no clause here, and that is a property rather than an
oversight.** A smoothed body is off cell centre almost everywhere; solid geometry is
covered anyway, because whole cells tile the plane and every overlapped column is
asked. A lethal volume looks like the one question that could not be covered that way
— except that `cell_can_meet_volume` never asked about a *centred* body: a walker's
cell does not fix its position, so it already refuses every cell from which a body
standing anywhere inside it could reach the volume, and `standable_fp` on each swept
column inherits that reading whole. Restating the rule inside the swept test would be
a private re-implementation of a general mechanism, so the reliance is pinned by a
test instead (`the_swept_test_inherits_the_off_centre_reading_of_a_killing_volume`).

`MovePlan::cells` / `ActorMovePlan::cells` keep the **full A\* route**, untouched.
The traversal proof asks of them what move the body made — which cell it entered,
which it stepped up onto, which use-gate it passed — and a thinned route would
silently stop binding the surmount and gate-use rules. The string-pulled polyline is
what the body is *rendered* along; the proof keeps the cells.

Integer arithmetic throughout, in the sixteenths `feet_16_fp` already measures in, so
no float comparison decides whether a body may take a path (ADR-0006).

Smoothing moves only the walk drivers (and the manifest that indexes them): the
world, the critical-path export and every other validation artifact are
untouched by it.

### A walked body faces where it is walking

Every per-tick `tp` of a `move-npc` / `move-actor` driver carries an explicit
`<yaw> <pitch>`; `pitch` is always `0` (a walk is level by construction — the L of a
vertical step is still a walk, not a dive). The yaw is `nav::yaws_along`: for each
waypoint, the **exact MC bearing of the segment it is about to walk**
(`yaw = atan2(-dx, dz)`, 0 = +z south), rounded to whole degrees.

**The ARRIVAL waypoint is the exception, and it is one rule over one object class**
(`nav::apply_arrival_yaw`, called by the `move-npc` planner and the `move-actor`
planner alike). A body's own path tangent is the wrong answer at the end of a walk:
a body that walked away from the party would arrive with its back to them, which is
what a guide leading a tour must never do — and a puppet is as much a walked body as
a villager is. So the final waypoint carries the destination anchor's **declared
`facing`** when the piece declares one (an anchor is where the piece says a body at
that cell looks), and otherwise the reverse of the last leg, turning the body back
down the path it walked, which is where whoever followed it is standing. The next
leg chains from that yaw, so a body that turned to face the party does not moonwalk
out of the room. The two verbs differ only in the scope the destination anchor
resolves in — an NPC's own area (`nav::anchor_facing_yaw`), an actor's globally by
first match (`nav::actor_anchor_facing_yaw`, the same scan its position comes from).
Pinned by `tests/arrival_turn.rs`: both verbs, both branches.

**The bearing is read off the unrounded samples** (the exact half of
`resample_body`'s output), while the
emitted position is the rounded one. Emitted coordinates are rounded to 0.01 of a
block so the `tp` lines stay short and byte-stable, and on an axis-aligned step that
rounding cannot move a bearing at all. On a diagonal it can: two samples 0.15 apart,
each component rounded by up to 0.005, differ in bearing by a degree or two — so a
bearing taken off the rounded samples twitches a body's head on nearly every tick
of a straight diagonal. A yaw is a property of the segment being walked, not of
the difference between two printed coordinates.

There is no yaw **easing**. A corner turns on the tick it is taken, because a per-tick
`tp` polyline is what vanilla gives us and any interpolation between two bearings
would be invented motion the nav proof never made. (Vanilla has no server-side "turn
over N ticks" primitive for a teleported body — CLAUDE.md *No hacks at any layer* says
that excludes the feature, not that it licenses a polling hack.) What reduces the
corners to turn is the path itself, straightened where the ground allows (previous
section), so a long open run yaws once and holds.

A `tp` **without** rotation leaves the body's yaw at whatever its summon or previous
beat set, so a body routed the other way slides backwards for the whole walk. Both
the actor and the NPC driver therefore share one yaw source.

`yaws_along` takes a **seed** — the facing the body already has — used for any
leading waypoint with no horizontal motion of its own (a walk that opens on the
vertical leg of an L, or a degenerate zero-length move). The seed is the previous
leg's exit yaw when the body has walked before (facing chains exactly as position
does, including across a deduped repeat of a content-keyed driver), else the yaw its
summon gave it: the home anchor's declared `facing` for an NPC, the actor's declared
`facing` for a puppet. An authored facing is never overwritten with a fabricated
south.

### One body, one live walk driver

A `move-npc` compiles to a self-scheduling per-tick driver `mv_tick_<npc>_<to>` that
teleports `@e[tag=dw_npc_<id>]` along its precomputed waypoints. The driver's
re-entry latch `#mrun_<bare>` is keyed per **(npc, to, gate)**: it stops a walk
from restarting *itself* and knows nothing about the body's other walks. On the
latch alone, a second `move-npc` fired at an NPC whose earlier walk is still running
leaves **two** drivers alive, both teleporting the same entity every tick; the
interleave garbles the path and whichever walk has more remaining ticks writes the
final position — the body parks at the **first** walk's endpoint, not the
last-fired one.

The contract is **last fired wins**, carried by a per-NPC *walk generation* score:

| score | meaning |
|---|---|
| `#mgen_<npc>` | the body's current walk generation; every start bumps it by 1 |
| `#mown_<bare>` | the generation this driver was started for |

* **start** (`mv_<npc>_<to>`): `scoreboard players add #mgen_<npc> dw.sys 1`, then
  `#mown_<bare> = #mgen_<npc>`. Its re-entry refusal is generation-aware —
  `if score #mrun_<bare> matches 1 unless score #mown_<bare> < #mgen_<npc> run return fail`
  — so a latch left armed by a leg this body has already superseded does not block
  that leg being fired again (the re-fire is itself the later walk, and wins).
* **driver** (`mv_tick_<npc>_<to>`), first two lines: when
  `#mown_<bare> < #mgen_<npc>` it drops its own latch and `return fail`s — no
  teleport, no `mv_arrive_`, and crucially no reschedule, which is what ends it. The
  superseded driver therefore dies on the next tick the scheduler hands it.

The staleness test is written as the positive `if own < gen`, never as
`unless own = gen`: with both scores unset a score comparison is *false*, and the
`unless` spelling would read that as "stale" and cancel a walk nothing superseded.

**A template that invokes a guarded driver directly claims it first.** The scoreboard
is shared by the whole PackTest suite: a sibling template that fires two walks for the
body (a quest completion running both starts) leaves `#mown_<bare>` behind
`#mgen_<npc>`, and a driver invoked with its stamp behind returns before it
teleports. So `v04_move`, `v06_move_actor` and `v06_arrive_handoff` run
`scoreboard players operation <own> dw.sys = <gen> dw.sys` before the jump when the
body's drivers carry the guard (`walk_claim`). The generation is taken, not bumped, so
a walk a sibling is running is not superseded by the template. A body whose drivers
carry no guard gets no claim line.

The new walk still starts at **its own first waypoint** (waypoints are precomputed
from the walk's declared start anchor, so "resume from wherever the body stands" is
not expressible), i.e. an instant snap onto the new route — the same snap single-walk
content already gets when a walk fires while its NPC stands elsewhere.

A body with only **one** planned walk can never be superseded, so its start and driver
carry none of this.

**`move-actor` puppets carry the identical contract**, for the same reason —
`#arun_<bare>` is keyed per (actor, to, gate), so on the latch alone two
overlapping legs on one puppet leave two live drivers fighting over the same body,
and the longer leg parks it at the wrong endpoint permanently. The scores are the same two under actor names: `#agen_<actor>` (the
puppet's leg generation, bumped by every start) and `#aown_<bare>` (the generation
this driver was started for), with the same generation-aware re-entry refusal in
`ma_<actor>_<to>` and the same two-line staleness prologue in `ma_tick_<actor>_<to>`.
The positive `if own < gen` spelling matters for the same reason here, and
`v06_move_actor` and `v06_arrive_handoff` claim a guarded driver before invoking
`ma_tick_` directly, as `v04_move` does. A puppet
with one planned leg carries none of it. The fixture is synthetic: a puppet with
more than one planned leg is `supersedable` and carries the machinery whether or not
its legs ever overlap in time.

Proved by `crates/delvec/tests/move_supersede.rs`, which **executes** the emitted
commands: a small interpreter for the driver command subset runs the real start
functions through the real 1-tick scheduler loop and reads the body's final position
off the `tp` commands. Both verbs are covered there, and the single-leg puppet's two
functions are additionally pinned **verbatim, byte for byte** (`GOLDEN_ONE_LEG`).

### Press answers — what a pressable thing says back

There is no such thing as a seal with nothing to say: a sealed gate that answers a
right-click with **silence** is a defect, and the answer is the engine's
obligation rather than the campaign's. A co-located `use` trigger on the gate
anchor cannot supply it, because it summons a *second* `minecraft:interaction` at
the same cell as the existing hint's, and an exact ray-pick tie resolves by
iteration order: one of the two hints silently dies and the compiler builds green
either way.

**A sealed gate is armed, an open one is not.** `close-gate` calls
`seal_arm_<anchor>` under an `unless entity @e[tag=dw_seal_<anchor>]` guard (a
re-fired beat never stacks a second set); `open-gate` on the same anchor kills the
tag. The hitboxes therefore exist **exactly while the region is solid** — no
scoreboard mirrors the seal state, because the entities *are* the state.

**Geometry: the shell, and a centimetre of protrusion.** `seal_arm_<anchor>`
summons one interaction entity per **shell** cell — every region cell with at
least one axis-neighbour outside the region. A buried cell has six sealed
neighbours and no face a crosshair can reach, so arming it would ship an entity
nothing can click; for the thin slab a gate anchor usually is, the shell is the
whole region. Each entity is `width:1.02f,height:1.02f`, positioned at the cell
centre horizontally and one margin *below* the cell floor, so its box brackets the
block on all six sides. **The margin is the mechanism, not cosmetics:** vanilla
takes an entity hit over a block hit only when it is *strictly* nearer the eye, so
a box exactly coincident with the sealed block loses the pick to the block and the
seal answers with silence. Coordinates are
built from integer hundredths, never from `f64` arithmetic, so the shipped text is
exactly what it reads as (ADR-0006).

**The answer is not a mechanism — it is an ordinary trigger.** An advancement, a
reward function, an actionbar line and its wording are properties of *being a
thing a player can press*, and none of them has anything to do with closing a
gate — a sealed `shortcut` door, the gate a souls loop-back most invites the party
to push on, needs them as much as a seal. A press answer is:

```
EnvTrigger { at: <the body's anchor>, on: use, audience: presser, once: false,
             effects: [ narrate { style: actionbar, text: … } ] }
```

authored by the campaign, or synthesized by the compiler
(`plan::collect_press_answers`) for a sealed body the campaign leaves silent. It
is emitted by the same `env_trigger_setup` / `env_trigger_fns` /
`emit_advancements` that emit every author-written click, so there is one proof,
one l10n path and one diagnostic family for all of it. What the general verb
carries for it is the **channel** (`actionbar`) and the **addressee**
(`presser`).

**The dispatch, which is what `presser` means.** `press_<trigger>` is rewarded by
a `player_interacted_with_entity` advancement keyed on the trigger's own
`dw_trig_<id>` tag; it revokes the grant (a wall is not consumed by being asked)
and calls `trig_<trigger>`, whose bundle is emitted under `Audience::Solo`. A
gated trigger's call carries the trigger's own `once`, `forbids_flags` and
`requires_flags`/`requires_state` guard, read from `trigger_poll_guards` — the
one authority the tick clauses read, every fragment space-terminated — so a
press answer gates exactly as a polled trigger does
(`execute unless score #party dw.f_hall_sealed matches 1 if score #party dw.f_hall_open matches 1 run function <ns>:trig_<id>`;
the gallery's barred side door is the instance). An ungated answer calls its
bundle outright. That
criterion is the one vanilla primitive that runs a function **as the player who
right-clicked** — the same one every NPC dialogue, `interact` objective, bonfire
rest and shop button already runs on — and it is chosen over polling the entity's
`interaction` NBT for two reasons: the record names no player a command could
target, and *reading* it would consume the press a co-located `use` trigger is
entitled to see. An
advancement observes without consuming, so a press answer can never eat another
consumer's click. A `presser` trigger therefore emits **no tick clause at all**,
neither a fire nor a `data remove`.

The generated `env_trigger_<id>` template drives both bodies, separately, because
they fail differently: `trig_<id>` direct, then — after clearing the marker again,
or the second assert would read what the first wrote — `press_<id>` through a
granted advancement, asserting that the dispatch reached the bundle and that the
grant was revoked so the object answers the next press too. `DW0811`'s
`press-answer` claim is registered over the `audience: presser` list, so a
synthesized answer — whose id appears in no authored document and is therefore
invisible to `DW0810`'s byte reading — is still covered.

Vanilla offers no such criterion for a left-click, so `audience: presser` on a
`strike` / `strike-npc` / `strike-assembly` / `approach` is `DW0427` rather than an approximation
(CLAUDE.md: a capability with no vanilla primitive under it is excluded, never
faked downstream). The compiler reserves the `dw-` trigger-id prefix for the
triggers it synthesizes (`trigger/dw-press-seal-<anchor>`,
`trigger/dw-press-door-<shortcut>`); an authored id in that namespace is `DW0428`.

**A step names its actor.** A `step` trigger with `audience: presser` is the
second act vanilla can attribute, and it needs no advancement: the event is a
player whose hitbox is in the plate's cell, so the selector that detects the
event — `@a[<cell>,tag=!dw_cutscene]`, run `as` — iterates exactly the players
who satisfy it, and `@s` in `step_<id>` is one of them. Nothing is correlated
after the fact, which is what `DW0427` refuses for a left-click: there the event
(a record on an entity) and the player (whoever is nearest) are two facts joined
by a guess. Measured against the rules the actor rests on: `DW0503` asks the site
(`EffectRootOwner::runs_with_acting_player` answers `addresses_presser`), and the
emitter dispatches `step_<id>` `as` the player and `trig_<id>` under
`Audience::Solo`, so the validator and the emitter agree that `@s` exists; the
observer census (`DW0926`) reads the poll as a positional selector carrying
`tag=!dw_cutscene`, so a watcher in a respawn wait is never an actor, and the
re-arm reads `@a[tag=dw_stp_<id>]` and `@s[<cell box>]`, which are not positional.
Two players on one plate are two actors, each dispatched once on their own step.
`once` is read per player inside `step_<id>`, so it still fires the bundle once,
for the first player the poll iterates — of two who step on in the same tick,
that is the server's player-list order (`@a` sorts arbitrarily). Stated, not hidden: the cell box is the block, not
the plate's own touch box (vanilla presses a plate on a hitbox meeting the
plate's lower quarter, inset a sixteenth from each side), because a volume
selector is never smaller than one block; a body whose hitbox passes through the
cell above the plate's top quarter — a jump across it — is detected without the
plate going down. The trap detection the selector is shared with has the same
property. `approach` is not attributed by this argument and stays `DW0427`.

**Which bodies get an answer, and who words it.** One site
(`plan::press_answer_sites`) lists every pressable body — `close-gate` seals, then
sealed `shortcut` doors — and the campaign words every one of them: a body with
neither an authored wording nor a `use` trigger is **`DW0429`** and the campaign
does not compile. Two objects of one class must not get two defaulting
policies: that is precisely the "capability keyed to the verb" defect CLAUDE.md's
worked example describes, and this surface **is** that worked example. The shared
site is therefore load-bearing rather than tidiness.

**Why a sealed body errors instead of defaulting.** The wording may well be
"The way is sealed.", and it
must be creator-customisable — but a baked default is the compiler making a
*design statement*, about tone and about what this specific thing is, on the
author's behalf, and then never telling them it did. An error makes the author say
it. That is the no-hacks rule at a new site: if content needs a thing, the DSL
exposes it and the author declares it, rather than a lower layer inventing it.


**Two ways to discharge the obligation**, the same thing said at two layers: a
`use` trigger anchored on the body (the general verb, available to every pressable
object), or — for a `close-gate` — an authored `sealed_hint`, which *is* the author
defining the wording. The compiler lowering an authored wording onto the general
path is not the compiler putting words in a player's mouth; inventing one is.

"The campaign answers it" is `QuestsContent::answers_press_at`: **any** `use`
trigger anchored on that body, whatever it does. One predicate, read by both the
refusal and the synthesis, so they cannot disagree about what counts as an answer.
A `strike` does not discharge it — pressing a door is a right-click, and a
left-click reply is a gesture the player may never make. A `shortcut` has **no**
wording field and does not get one: writing the line is what the trigger is for.

**One cell, one hitbox — the merge, one layer out.** A `strike`/`use` trigger whose
`at` is the gate anchor is asking the player to hit *the gate*, and once sealed the
gate's own hitboxes are what a click reaches. Its `dw_trig_<id>` therefore rides
the seal's entities and `env_trigger_setup` summons nothing for it — the identical
rule `strike`-on-an-NPC's-anchor follows. The consequence is also its meaning: such a trigger is live
exactly while the gate is sealed. Everything *else* holding a hitbox in a
pressable body's cells is rejected (`DW0422`), and two firings that disagree about
the wording are rejected (`DW0423`).

**Lifetime is the body's lifetime, and that is why riding matters.** A press
answer summons nothing; it rides the hitboxes the sealed object owns. So a seal's
answer exists exactly while the region is solid (`open-gate` kills
`dw_seal_<anchor>`), and a shortcut door's exists exactly until it opens
(`shortcut_open_<id>` kills `dw_ws_<safe>`). Shortcut permanence is structural —
`DW0372` forbids a re-seal — so a door that kept saying it cannot be opened after
you opened it would be worse than the silence this closes, and nothing has to
remember not to do that: there is no answer left once the thing you pressed is
gone.

**Every site that can FILL a gate must also ARM it — and must be MODELLED.**
`plan::for_each_gate_effect` is the one traversal every gate consumer walks,
deliberately wider than `dsl::for_each_campaign_effect`: an effect list is a gate
site if `emit::emit_quest_effect` can reach it, not if the quests stage happens to
own it. Every `EffectRootKind` does — quest `on_objective_complete`, quest
`on_complete`, `triggers[].effects`, `traps[].payload` (spec-0022: a payload is an
effect root), a **dialogue option's `set-checkpoint` `on_respawn` bundle**,
`shortcuts[].on_unlock` and the campaign's `on_death` (both spec-0031),
`shops[].offers[].effects` (spec-0032) and a wave's or actor's `on_kill`
(spec-0074). The dialogue one is the trap: `DialogueEffect` carries no gate verb,
so a walk that stops at the quests stage misses it — but `on_respawn` is a plain
`Vec<QuestEffect>` and a `close-gate` inside it really is lowered, into
`cp_on_respawn_<i>`. A seal the compiler fills but never arms is the finding
again, one effect root further out. A walk written against a remembered list
rather than against what emission reaches misses roots in exactly this way.
The seal planner, `DW0423` and the `close-gate` completability
model (`plan::collect_gate_events`, feeding `DW0311`/`DW0315`/`DW0342`/`DW0410`)
all walk it, so the checks, the proofs and the emission can never disagree about
which firings exist.

The roots themselves are enumerated exactly once, in
`plan::for_each_effect_root` (which yields each top-level effect *list*, with an
`EffectRoot` naming which of the nine it is and carrying its owner where it has
one). `for_each_gate_effect` is that enumeration flattened; `timeline::walk_campaign`
(→ `DW0410`, `nav::all_effects`), `emit::all_campaign_effects` (→ the
generated functions), **both halves of `compiler::flow`**
(the producer scan and `flow::gate_flags`, → `DW0201`/`DW0202`/`DW0203`/`DW0204`
and the exported critical path) and
`emit::check_effect_anchors` (→ `DW0360`, the resolved-anchor seal over exactly
what those generated functions emit) and `emit::declared_flags` (→ the
`dw.f_<flag>` scoreboard objectives `setup` creates for the writes those
functions perform) are the other consumers. A root cannot be added to one walk
and forgotten in another. That claim is also a **test matrix**
(spec-0031): `tests/effect_root_walkers.rs` iterates `EffectRootKind::ALL`,
builds one campaign per root from an exhaustive `match` (so a new root is a
compile error there), and asks six walkers plus `DW0360` about every one of them.
A per-walker test proves one walker against the roots its author remembered; the
matrix asks every walker about every root, and names both when it fails.

**When a firing happens** comes off the site's `EffectRoot`. A quest
`on_objective_complete`/`on_complete` fires at its objective's / the quest's
completion step — the player is *forced* through both, so both gate directions are
modelled. An **environment trigger** is a party act nothing on the quest DAG
orders, so its **openings** fire at the `trigger` step the critical path performs
it in (`plan::path_triggers`, below) and a trigger no path step performs opens
nothing; its **fills** root at step 0, forced, so a wall a trigger raises is
assumed up from the start and never assumed down before somebody strikes it. A
trigger step precedes every later step of its path in the ancestry relation
(`compute_strict_ancestor_steps`). A trap payload, a dialogue-hosted `on_respawn`
bundle, a shortcut's `on_unlock`, the campaign's `on_death`, a shop offer's
effects and an `on_kill` bundle have no step of their own (a sprung trap, a death,
a bar thrown, a purchase, a kill), so all six root conservatively at step 0, which
precedes every leg (`plan::firing_of`). These six **optional** roots
register their `close-gate`s **only**: an
unguaranteed firing may be assumed to have happened exactly when assuming so is
conservative, so it can seal a region but never unseal one. That is the rule a
shortcut gate already obeys (sealed for the whole model, because the delve must be
finishable the long way). A later `open-gate` from a forced root still wins the
region, so the widening reads as a seal the proof must survive, never as a veto.

**Wording (`close-gate` only).** `sealed_hint` is **required** unless a `use`
trigger answers the gate instead (`DW0429`). It is **sugar, not machinery**: it
is the wording of the press answer the compiler synthesizes for that gate, and
nothing else. The line is inventoried at `<effect-key>.sealed_hint` and
translates like any other player-visible string; the synthesis happens in the
**plan**, below `localize`, so no sidecar key moves.

Generated PackTest `v08_seal_answers`: nothing armed at boot, exactly one hitbox
per shell cell after the seal, unchanged after a re-fire, none after the re-open —
staged and un-staged by the fixture itself (batch model). The press→actionbar
half needs a real client's right-click, which no PackTest can fire; that primitive
is exercised by the harness bot wherever it rests at a bonfire or talks to an NPC.

### Nav (compile-time, over the assembled voxel grid)

**A navigation world is geometry plus premises, and the premises are one value.**
`nav::World` carries the collision classes measured off the assembled bytes and
what the *campaign* states about the world those bytes sit in: the
world-generator `Ambient` (spec-0013 `horizon`), the built volume (where the
content ends), the declared **lethal volumes** (impassable, `DW0510`), the placed
**furniture regions** (never stood on, `DW0510`; spec-0065), the
measured **world-load gate seals**, the **clocked gate regions** a `timed-gate`'s
own clock owns, and the critical path's
**objective cells** (read only by the sea-seepage proof, to name the objective a
wet walk region drowns). They travel together as `nav::Premises`, and `nav::Premises::of_plan` is the only way to
derive one from a campaign — a call site cannot state a subset, so it cannot
carry half a world. Every arm that builds a world from a campaign passes one to
`World::from_occupancy`, and that is the same set on **both** of `emit::build`'s
arms: the pristine assembly (`World::from_plan`) and the stage-7 edit replay,
which builds its world from the edited bytes. `edit::check_batch_invariants`,
which re-runs the completability proofs after every batch, uses the same set.

A world with no campaign behind it says `nav::Premises::geometry_only` by name,
with its reason in a comment at the call site, and every production decline is
enumerated by `tests/premise_declines.rs`: the stage-5 blockout
battery's two massing worlds (they carry their own sealing authority, from the
quest graph's monotone closure) and its stairwell pass (one step between two
courses of a stair, a question about geometry inside the stair's own box); the
synthetic constructor every unit-test world goes through; the two relight darkness
surveys (`light::relight_over` and a `relight` verb's own — applying the premises
would cut the reachable flood at a kill box and *shrink* the `DW0210` survey
rather than sharpen it, and a pit that kills is a pit the player must be able to
see before stepping into it); and `delvec snapshot`, where a camera is stood up
against blocks and a reviewer framing a shot down into a lethal volume is looking
at open air. `delvec blocking-chart` goes the other way: it re-derives
`critical_path_routes`, so it carries the full set and draws the corridor the
proof actually walks.

**Furniture is not floor (spec-0065).** A prefab anchor with `role: furniture`
and a `region` names the piece's furniture blocks — a laid table, an altar, a
counter, a bed. `Plan::furniture` holds every placed piece's regions in world
space, in area → placed piece → anchor-name order (read off the placed pieces,
not the first-wins anchor table, so a piece seated twice keeps both tables), and
`World::standable_fp` refuses a cell when, for any column of the footprint, the
**support cell** under it lies in a furniture region and is solid. The clause sits
beside the lethal one in the one predicate, so every route proof, `move-npc` /
`move-actor` leg and its snap, flood, wave seat and waypoint export inherits it.
Air cells of a region contribute nothing, and a body beside a table, feet on the
floor, is untouched. **What is not judged**: a body *posted* on furniture (an
NPC's anchor, an actor's anchor) stands where it is declared — posts answer to
`DW0450` and `DW0511` as before — and a walk that starts from one snaps to the
nearest standable cell. `DW0891`'s population is taken over
`World::without_exclusions`, which lifts lethal volumes **and** furniture
together, so a declaration over the stone round a pit cannot hide caught floor;
the admission's own standable rule (`schem::nav::standable_cells`) does not read
the role. A furniture anchor is never a gate (`PrefabMeta::gate_anchor` answers
none, so the region is not voided from the modelled world) and resolves to a
point at its `pos`, else its region's `from`. **Binding**: every walked build
prints `furniture binding: F region(s) over S solid cell(s), W standable cell(s)
withheld from walking; L leg(s) proved, N standing on furniture.` — `S` counts the
region's cells the nav model holds as floor (a fence leg is a tall barrier, not
floor), `W` the cells a player could stand in on the bare geometry that the
exclusion withholds, `N` the cells of the proved critical-path, `move-npc` and
`move-actor` legs that rest on furniture, computed rather than typed — and writes
the same counts, with the placed anchors, to `validation/furniture-gate.json`
(`examined` = `F`). `tools/ci/check-gallery-coverage.py` reds the gallery when
`examined` or `withheld` is 0.

`move-npc` paths and the critical path are routed by A* over the placed-world
block data (obstacles per the collision classes above — full-cube solids, 1.5-tall
fence/wall barriers, closed fence gates for walkers that cannot use them;
**fluid-flooded cells — water and lava alike (`assembled::is_fluid`) — are
impassable and are never valid floor**; compiler gate
regions are passable). Steps are cardinal, one cell up or down.

**Step cost is terrain-shaped, not distance-only.** A step costs
`16 + 2 × |Δfeet|` in sixteenths of level walking: `STEP_COST_16 = 16` for the
block travelled, plus `ELEV_WEIGHT = 2` per sixteenth of height change, **up or
down alike**. The A* heuristic is horizontal Manhattan distance × `STEP_COST_16`,
which no step can undercut, so it stays admissible and consistent — A* still
returns a true minimum-cost path and never reopens a closed node.

*Why.* Under a distance-only cost every route of equal length is equally good, so
a planner walks a body along the straight line over bumpy 1-step terrain —
bobbing a block a dozen times — while the flat cleared road two columns over
costs a two-step detour and never wins.
Staged walks are photographed; a body that pogos over lumps reads as broken even
though every step is legal, and the built road exists to be walked.

*Why 2.* A rise past the auto-step budget is a jump, and vanilla's jump arc is
≈12 ticks airborne against ≈4.6 ticks to walk a block on the flat — so clearing a
1-block rise really costs about 2.5 blocks of walking time. Two is the integer
under that: enough that a 1-block bump is worth ~2 blocks of going around, not so
much that the planner invents absurd circuits to dodge a single step. It is
deliberately *under* the physical figure, the safe direction, since overpaying for
flatness is what would distort routes on legitimately sloped terrain. The weight
applies per sixteenth, so a slab or `dirt_path` lip costs proportionally less than
a full block and intentional slab stairs are not penalised like lumpy ground.

*Scope.* Cost shaping changes which of several **valid** routes is chosen, never
which routes exist: `DW0307`/`DW0311`/`DW0325` reachability semantics are
unchanged, a bump is a cost and never a wall, and a disconnected goal is still
unreachable. Determinism is unchanged (integer costs, frontier ordered `(f, g,
cell)`).

**The step rule is physical, not cell-adjacency.** Each standing cell
has a true **feet height** in sixteenths — the cell below's `partial` face height,
so standing on a bottom slab is `y - 0.5`, not `y` — and a candidate step is gated
on the **rise** between the two feet heights:

| Rise | Verdict |
|---|---|
| ≤ 9/16 (0.5625) | a walk-up. Vanilla `maxUpStep` is 0.6, so no jump — and therefore **no headroom** is required above the source cell |
| ≤ 20/16 (1.25) | a jump; the swept head cell above the source feet must be clear or the entity head-bonks |
| > 20/16 | **impossible** — a vanilla jump apex is ≈1.2522 blocks, so 1.3125 is unreachable |

**Those three arms are one function, in one crate, and every walk in this engine
asks it.** `delvewright_dsl::metrics::step_allowed` takes a rise in sixteenths and
a head-clear answer and returns the verdict above; `compiler::nav` asks it, and so
does `delvec::schem::nav`, which is the walk a grammar expansion, an ingested
structure template and a reassembled zone are read with, and which a prefab's
contract-reachability gate proves over. The site is `delvewright-dsl` because
`delvec` is published to crates.io and may only depend on published crates, so the
rule cannot live in the schem crate and be shared; `dsl` is the one crate both
already reach, and it is where the three constants already live.

What each caller keeps is the **measurement** of the rise, not the rule. This
model reads real collision tops and a footprint's highest supporting face. A box
of cells has neither, so it reads a step of one cell as a full block unless its
implementor overrides `Voxels::floor_top_16` — a coarser reading of the same
quantity, wrong only in the direction that **refuses** a step vanilla admits.

The rule cuts both ways against a cell-counting one. It **rejects** stepping off a
bottom slab (feet at `y+0.5`) onto a ledge whose face is at `y+2`: a **1.5-block**
rise, which a cell count reads as an ordinary "+1 cell" step no player or
mineflayer bot can perform. It **admits** stepping from a full floor onto a
bottom slab is a 0.5-block auto-step, legal even directly under a ceiling that
would block a jump. Vertical candidates stay `{0, −1, +1}` cells — a `+2`-cell hop
between two very thin floors can be physically legal, but omitting it only ever
refuses a route, never proves one.

Cutscene dollies must pass only non-solid cells; the clip test is an **exact 3-D
grid walk** (Amanatides–Woo DDA) visiting every cell each segment intersects, with
no error term, so a cell a shot only grazes through a corner is still visited.
Unroutable/clipping/stranded → `DW0307`/`DW0308`/`DW0311` at build (never a
runtime glitch).

**The world-load gate seal — what a gate's state is *before* any verb fires
(`DW0317`).** A gate region is not empty because it is a gate; it holds whatever
the prefab `.nbt` authors there, and both cases ship in the library:
`hello-room`'s `anchor/door` is six cells of `iron_bars`, `island-mountain`'s
`anchor/boulder` is twenty-seven cells of air. **Which one a gate is, is measured,
never defaulted** (`assembled::measure_gate_seals`, taken immediately before the
base model clears the gate cells), and a gate the world authors shut re-enters the
model as a `Fill` at step 0 — the identical shape a shortcut gate's world-load seal
already used. A gate state that is a function of what *sealed* it and never of
what *opened* it ("passable unless a `close-gate` seals it") can only fail to
notice an obstruction, never invent one, and the mistake an author makes is
forgetting to open a door: a campaign missing its one `open-gate` would compile
clean and the runtime bot would say *"No path to the goal!"* — a symptom that
names nothing.

The base occupancy model clears every gate cell, as a statement about the **base**
world only. It has to pick one state, and "open" is the one that
keeps `RegionWrite::Unseal` expressible — an `open-gate` is `replace`-filtered to
the gate's own block, so a base world holding the bars would need a block-aware
clear and would then wrongly delete a `collapse`'s debris (`DW0445`). The
world-load `Fill` supplies the other half, and an `Unseal` cancels it by ordinary
latest-write-wins.

Two things this deliberately does **not** decide, each of them a stated gap
rather than a silent one:

* **A `timed-gate`'s region** (spec-0016 §4) is measured and never modelled shut:
  its clock fills and clears it twice a cycle from world-load, so a permanent seal
  would refuse a campaign that plays. `DW0378`/`DW0388` own that region.
* **A gate opened only by an optional firing** — a trap payload, `on_death`, a shop
  offer, a dialogue `on_respawn`, a shortcut's far-side unlock — is treated as
  never opened, by the rule that an optional firing may seal a region
  and may never open one (`plan::collect_region_events`), which is also what keeps
  every shortcut gate sealed so the delve is finishable the long way. A delve whose
  only door-opener is a sprung trap is therefore refused; the first-class way to
  spell "the party walks/presses here and the door opens" is an environment
  `trigger`, which the model credits **at the `trigger` step the path performs it
  in** — so the critical path carries the press, and a trigger whose flag gate
  first holds after the leg through its door is `DW0317` like any late opener.

The `foreign_blocks` count in the ledger below exposes a fourth gap: an
`open-gate`'s fill is `replace`-filtered to the anchor's **declared** block, so any
other block authored inside the gate region survives the opening, while this
model credits the `Unseal` with the whole region. `cave-mouth.nbt` authors five
`mossy_cobblestone` cells inside a gate declaring `cobblestone`; no campaign in
either repository places it.

**Binding ledger — `validation/fluid-escape.json`.** What the `DW0318` proof
looked at: `horizon` (the ambient the verdict was stated against),
`pieces_examined`, `fluid_cells_examined`, `cells_outside_built_volume`,
`from_pieces` and `verdict`. Emitted by **every** campaign that assembles a
world, including one that holds no water at all — a dry delve ships a ledger
reading `"fluid_cells_examined": 0`, which is the reading that tells a reader
the check bound to nothing, as opposed to a silence nobody can tell apart from a
check that never ran. `fluid_cells_examined` is counted off the assembled
occupancy model and `pieces_examined` off the plan, so neither is the length of
the finding list. On `nobodys-cave-island` the ledger reads 19958 fluid cells
across 5 pieces, 18086 of them outside the built volume and every one of those
meeting the ambient sea: the two numbers reconcile against `delvec prefab`'s
independent per-piece count (1200 + 672 = 1872 authored fluid cells, and
19958 - 18086 = 1872).

**Binding ledger — `validation/stealth-judge.json`.** What the `DW0852` audit
read off the emitted datapack: `beats`, `judges`, `examined` (per-player tests),
`arguments` (the selector vocabulary those tests actually used), the
`allowed_arguments` they are held to, `offenders` and `verdict`. Emitted only by a
campaign that declares a stealth beat — the `gate-seal.json` rule, and for the
same reason: a campaign that fields no stealth has nothing to audit, so a file
that EXISTS and reports zero examined tests is a finding rather than an absence.
`judges` must equal `beats`; a disagreement is reported as an internal-invariant
violation rather than as a pass over whatever the scan happened to see. Readings:
`nobodys-cave-island` — 1 beat, 1 judge, 3
per-player tests, vocabulary exactly the six box arguments.

**Binding ledger — `validation/sea-seepage.json`.** The other half of the same
question: `DW0318` measures water
leaving the built volume, this measures the ambient sea coming into it.
`horizon`, `pieces_examined`, `contact_face_cells` (cells inside the built volume
the sea is directly touching — the seed set), `cells_the_sea_reaches`,
`walk_cells_examined`, `walk_cells_submerged`, `walk_cells_wading`, `in_pieces`
and `verdict`. Emitted by **every** campaign that assembles a world; a
`horizon: void` one ships `"horizon": "void"` and zeroes rather than nothing at
all, because "there is no sea here" and "nobody ran this" are different facts.
`contact_face_cells: 0` is the only honest way this proof passes without looking
at anything — a watertight hull — and it is a number rather than a silence.
`walk_cells_wading` is measured and deliberately not judged: it is the shoreline,
and how wide a shoreline a map has is a fact worth a reader's eye even though it
refuses nothing. Readings: on `nobodys-cave-island`, 5 pieces, 593 cells of open
contact face, 1831 cells the sea reaches inside them, 2007 walk cells examined,
20 wading and 0 submerged — a pass that says what it looked at. On the gallery's
`ocean-horizon` overlay, 4 pieces and 872 walk cells against a closed hull, so
zero contact face.

**Binding ledger — `validation/gate-seal.json`.** Every gate the layout resolved,
sealed or not: `gates_examined`, `sealed_at_world_load`, `modelled_as_sealed`, a
per-gate row (`area`, `anchor`, region, `cells`, `blocked_at_world_load`,
`foreign_blocks`, `sealed`, and `clocked` — whether a timed gate's clock owns the
region, so its blocks at any instant are the clock's phase, which
`tools/ci/check-written-world.py` counts as such), and `unbound` when the model
treats none of them as shut. A
campaign whose layout resolves no gate anchor emits no file at all, so a file that
exists and reports zero is a finding rather than an absence — `nobodys-cave-island`
is exactly that case, its one gate anchor being the boulder the campaign
`close-gate`s later.

**Runtime region solidity (spec-0031; DAG-causal).** The base occupancy model treats every
gate region as **passable** (the conservative "assume the gate the player needs is
opened" stance `DW0306` separately proves at the piece-connectivity level, and the
world-load seal above supplies the state that stance was standing in for) — so
`open-gate` does not dynamically flip cells at nav time. `close-gate` is the physical dual: the compiler
collects every runtime region write with its firing objective's critical-path step
(`plan.region_events`, content-ordered, **deep-walked through `sequence`/lifecycle
bundles** via the shared `visit_deep` authority — a write nested in a timeline is
collected exactly like a top-level one), and each walked critical leg — and each
checkpoint→forward-path leg — is routed with a region forced **solid** iff its
causally-latest write **among the leg-objective's DAG ancestors** is a fill (not
reopened by a later `open-gate` or `clear-region`), and **cleared** iff that write is
a `clear-region`.

Five verbs produce those writes and none of them owns the rule — `close-gate` /
`open-gate` (a box and a block off a prefab gate anchor), `fill-region` /
`clear-region` (an authored box), `open-way` (the cells, the block and the
direction off a placed piece's exported contract, spec-0042), plus a
`shortcut`'s world-load seal. What each
leaves behind is read off the command it emits (`plan::RegionWrite`), which is why
an `open-gate` is a third case and not a synonym for a clear: it is
`replace`-filtered to the gate's own block, so it removes nothing the model believed
was there, while an unfiltered `clear-region` does. Collapsing the two says an
`open-gate` deletes a `collapse`'s debris resting in the doorway — measured: the
`DW0445` burial test goes green, i.e. stops proving anything, the moment they are
collapsed. Where a fill and a clear overlap, the fill wins: a proof that survives the
seal is the conservative answer. The ordering is **DAG-causal, not linear**
(`plan.strict_ancestor_steps` / `Plan::gate_fired_before`): a gate only seals a leg
whose objective is a true causal descendant of the gate's firing objective, so a
gate on a **parallel quest branch** the lineariser merely interleaves ahead of a leg
does not falsely seal it (island `take-the-cheese` flee legs are not sealed by the
`hide` branch's boulder). A walked leg `from → to` credits exactly the writes the
party has necessarily fired while walking it (`World::leg_region_state`): those that
precede the arrival, the start step's own, and those that precede the start. For a
**causal leg** (the start is itself a DAG ancestor of the arrival) that is the
arrival's state. A leg the ancestry does not connect — the lineariser concatenating
sibling branches, or an ordering the relation fails to record — is judged by the
same rule and never over the open world: a write that neither end inherits does not
seal it, while the world-load seals and every write of the start's own branch do. A
genuinely-forced re-crossing (a causal leg whose sealed gate is never reopened
before it) still fails `DW0311` (`DW0315` from a checkpoint) with a message naming
the sealed gate — the "point of no return by geometry" the staging wants,
provable at compile time.

**A loop's slab is a gated seal** (spec-0086 §5.1). No write is invented: the
seal's state is the loop's gate's state, read per critical step from the one
`Flow` replay every gate consumer reads (`flags_at`, plus the numeric terms
replayed with the loop's own `counts` writes and the flags its `on_cross`
sets carried forward; a term the replay cannot date counts as holding). At the
step the gate opens the plan records `RegionWrite::Hold` (impassable, never
floor, joined to the unforced set so no route stands on it) and at the step it
shuts `RegionWrite::Unseal`. So a leg that must cross the slab routes only in
a configuration where the loop has stood down, and the loop stands down only by
writes the forced path performs; a forced leg across a holding slab is
`DW0311`, naming the loop, the open gate term and the step it was read at.

**Every arrival is keyed, not every objective.** The critical path is
`[select-class, objective…, assert-complete]`, and a consumer that sweeps quest
states — `nav::reachable_under_every_quest_state`, the seat model `DW0525` reads —
runs to `critical_path.len()` **inclusive**, one past the end. Neither the
completion assertion nor that sentinel is an objective, so `strict_ancestor_steps`
carries a row for each of them holding **every** objective on the path: the
exported path is rooted at the finale, so it holds exactly the quests campaign
completion depends on, and all of them are done before `dw.campaign` can be
asserted, in every valid play order. Those rows are what keeps the two arrivals at
which everything has fired from answering as if nothing has, and the answer there
matters both ways — a door the party is FORCED to open reads shut again at the end
of the delve (refusing a rest point behind it), and a `close-gate` fired by the
LAST objective goes missing (admitting a rest point sealed in).

**Forcedness is decided one layer earlier, and the set is deliberately every
objective on the path rather than the mandatory ones** (spec-0051).
`plan::collect_region_events` drops a write that does not FILL when its root is
unforced, so an `open-gate` hanging off a quest nobody has to play is not in
`plan.region_events` for any relation to credit; an unforced FILL is kept, because
a wall the party may find standing is one the proof must survive. A beat absent
from the path being proven is unforced on it, so a branch proof credits only the
openings its own path fires: an `open-gate` on another branch's objective, or one
behind a guard the path's flags never satisfy, leaves the door shut, and the leg
through it is `DW0317`. A second reading
of forcedness here would be a second authority, and for a fill it would be the
answer that ships. A **branch** path (`Plan::branch_gate_model`) is ordered over
what its world completes rather than by the finale's closure, so it can carry a
quest that world's player may skip; it is sound for the same reason, and its
consumers arrive only at objective steps.

**Close-gate solidity for *staged walks* (timeline-local — `DW0410`).** The DAG-causal model above answers "which gates are shut while the **player**
walks a critical leg". It says nothing about two effects inside one bundle,
because across bundles there is no order to know. Inside a **single effect
timeline** there is, and `compiler::timeline` proves it.

The shape it refuses: one `sequence` seals a boulder at `at_ticks: 460` and walks
an actor across that region at `at_ticks: 700`. Planned on the open world (gates
are modelled passable), the walk would build green and the actor would step
through solid rock on the live server, although the gate state at tick 700 is not
in doubt.

`timeline::walk` replays each timeline and pairs every effect with the gate
regions an **earlier effect in that same timeline** provably sealed. A timeline is
one **effect root** — every list `plan::for_each_effect_root` enumerates, i.e.
every `EffectRootKind` emission can lower (declared order, one tick, so effect *j*
finishes before *i > j* starts); a `sequence`, ordered by `(at_ticks, declaration
index)` — real elapsed time, which is exactly what the shape above turns on; or an `on_arrive`
bundle, which inherits the state as of its move.

**Optional roots need no special case**. Six roots have no guaranteed firing —
the party may never trip a trap, nobody is forced to die at a checkpoint — and the DAG-causal model above has to rule on that (an unguaranteed
firing registers its `close-gate` only). The staged walk does not, and the
asymmetry is deliberate: that model reasons *across* bundles about the route the
player is **forced** to walk, so whether a firing happens is load-bearing, while
this one reasons only *within* one bundle and its claim is conditional from the
start — *if this bundle runs, this walk starts after that seal landed*. A trap
that never springs never runs the walk either, so optionality cancels on both
sides of the implication. A payload's walk must be legal in the world its own
payload has already made, which is exactly what it must be whenever it fires.

Both walk planners (`plan_actor_moves`, `plan_moves`) then **route over that
timeline-adjusted world**, so a legal way around a shut gate is simply taken and
nothing is reported. `DW0410` fires only when the sealed world admits no route
*and* the open world does — which is what separates it from `DW0325`/`DW0307`
(unwalkable on the open world at all). A deduped repeat occurrence re-checks the
already-planned route against its own timeline's seals, since that is the path the
shared content-keyed driver actually walks. `nav::all_effects` is defined as this
same walk with the states dropped, so effect and attributed state cannot drift —
and the walk itself is defined over `plan::for_each_effect_root`, the **one**
enumeration of effect roots, which `plan::for_each_gate_effect` (the gate scans)
and `emit::all_campaign_effects` (the generated functions) also walk. One root
list: what the emitter lowers and what the proofs check are the same set by
construction, which is what stops a hand-rolled walk from enumerating a subset of
the roots.

**No false certainty** (the `compiler::continuity` stance): cross-bundle order is
never guessed — every timeline starts from "nothing provably sealed"; a
`close-gate` carrying `requires_flags`/`forbids_flags` may not fire and so adds no
seal, and a conditional `open-gate` likewise *drops* the region to unsealed rather
than asserting it open (both uncertainties collapse toward silence, the direction
that can only withhold an error, never invent one); and gate effects nested in an
`on_arrive` seal only within that bundle, since they are not ordered against the
enclosing bundle's later siblings. Symmetrically a walk may **rely** on a gate an
earlier effect opened — the occupancy model already treats gate regions as
passable, so `open-gate` needs no special case.

The PackTest counterpart deliberately does not model this: the generated
`v06_arrive_handoff` drives the arrival tick with every campaign gate
filled. What must be immune to sealed terrain is the **arrival machinery** (a tp
chain, not pathfinding); what may not be routed across a seal is the compiler's
*plan*.

**One leg model for every consumer.** The per-leg seal
(`World::walked_legs_sealed`) and the routing that uses it
(`nav::route_walked_legs`) are the single definition shared by the completability
proof, the forced-cell set the `DW0342` trap proof reasons about
(`World::required_path_cells`), and the exported harness waypoints
(`nav::critical_path_routes`). A consumer routing the fully-open world instead
could (a) hand the bot a route through a gate the campaign has already sealed and
(b) call a lethal trap "avoidable" when the player only walks its detour *because*
a `close-gate` shut the direct route. A trap's disarm-reachability search likewise
runs under the gate state of the earliest leg that crosses the trap cell, not the
fully-open world.

**Talk-to endpoint:** a talk-to leg's target anchor is the NPC's own
occupied cell — the cell the **cast ledger** stations it at for that beat (see
`critical-path.json` above), not its stage-2 anchor; the mannequin stands there
and its interaction hitbox fills it. The
leg's goal is snapped to the nearest standable cell *beside* the NPC — excluding
the NPC's own cell and any flooded cell — so a shore NPC resolves onto dry footing
within interaction range, never onto the mannequin or a water tongue.

**Deferred-NPC staging order (`DW0197`/`DW0198`, scope note).** The ordering proof
is the stage-4 `depends_on` closure (the same machinery `DW0195` uses), taken at DSL
validation tier — not the compiler's `plan.strict_ancestor_steps`. It therefore
proves only the **decidable** half: a `talk-to` whose every `spawn-npc` sits in a
strict DAG *descendant* quest is `DW0198`. Not proven, deliberately: a `spawn-npc`
fired from an environment trigger, a dialogue option, or the talk-to's own quest —
none of which has a position on the quest DAG — so those suppress the check rather
than risk a false positive on legitimate staging. `DW0197` (never spawned at all) is
total and covers the common defect.

**Waypoint self-check (`DW0314`):** after routing, every exported
critical-path waypoint is re-asserted standable in the FINAL world (settled +
water-flooded + relight fixtures) **as that leg's own runtime region writes leave
it**. A leg is not walked over the bare assembled world: a campaign may lay floor
at runtime — a repaired stair, a lowered bridge, a placed plank — and the leg
crossing it is routed over the world those writes produce, so the self-check
rebuilds that same world from the state the leg carries (`LegRoute::proven_world`).
There is one definition of a leg's world and both halves read it, which is what
makes "the exported route is the route the proof passed" a property of the type
rather than of two call sites agreeing. That world is built by the one region-state
construction (`World::with_region_state`), so the self-check reads **forcedness**
along with everything else: footing laid by a beat the party can skip is impassable
and not floor here exactly as it is for the route, and a forced leg that leans on it
is `DW0546` before any waypoint is exported. Since the routes come from A* over that
same world, this can only fire if a later pass mutates a cell nav relied on or an
endpoint resolves off the walkable set — making it structurally impossible to ship
a waypoint the game floods or walls (the water-flow / post-nav-mutation divergence
class), a loud build failure instead of a runtime strand. A terrain edit that
buries a cell a proven route uses is still caught: an edit is not a runtime region
write, and no leg state restores it.

### Seamlessness — an endless corridor (spec-0086, spec-0090)

**The claim**: at the moment a body is moved from `p` to `p + d`, everything it can see near it is what it could see from `p`, cell for cell, and anything far off that differs moves on screen by no more than station 4's far field moved anything. The server keeps the body's in-cell position, facing and velocity across a relative `tp`, and the client receives one relative update, so only the world can betray the move. `compiler::r#loop::check` proves it per loop over the assembled world:

- **The slab** (`DW0945`): one crossing axis, a move that carries the slab out of itself, thick enough for the poll (`dsl::metrics::POLL_HORIZONTAL_BLOCKS_PER_TICK`, `POLL_FALL_BLOCKS_PER_TICK`), a horizontal slab tall enough that a jump from its floor does not clear it, clear of other slabs, landings, teleport volumes and lethal keep-outs, and passable throughout. An engine-placed affordance in it is `DW0542`.
- **The eyes**, each written where it lands (`e′`; it was moved from `e′ − d`): for every standable cell of the landing slab `region + d`, the standing eye (`metrics::PLAYER_EYE_HEIGHT`) at the cell's centre and at ±0.3 toward each horizontal corner; and, for a horizontal slab, the **catch band** — the standing eye of a body the poll first finds, its centre half a body-width (0.3) outside the slab's approach face, at the middle of the passage cell and ±0.3 across it.
- **Sight**: a cell is in sight of an eye when the segment to its centre or to any of its eight corners is walked by `nav::walk_cells` and meets no cell `World::blocks_camera` holds for — the one definition of a view stopped by geometry.
- **Fog**: each eye has a fog end, read at one site (`fog_end_at`): the kernel-weighted mean, by `horizon::camera_mix` over the painted `BiomeMap`, of each nearby biome's `visual/fog_end_distance`, the attribute default (1024) for a biome that declares none, in each configuration the route stands under (the first tick's map, then after each `set-atmosphere` repaint in route order). A cell farther than the eye's fog end is closed to it.
- **The span** is found by a breadth-first walk from the eyes' own cells through every open cell an eye sees inside its fog; a seen cell that blocks the camera ends the walk there, an open cell no eye sees ends it by geometry, one seen only past the fog end ends it by fog. The span box covers every cell the walk asked about. **The view must close**: a seen open cell outside every placed piece, above the built volume into open sky, or `SPAN_REACH` (128) cells past the landing slab is `DW0947`. The visible set `V` is every seen cell.
- **The near field and the far field** (spec-0090). A cell **differs** when its full block state differs from `c − d`'s, or its light does (the model's flood at the campaign's darkest and brightest reachable sky, over the clip `S ∪ (S − d)` widened by 16). Its **shift** is the largest angle, over its nine points (centre and corners) and over the eyes that see it inside the farthest fog any configuration gives them, between the point's sightlines from `e′ − d` and from `e′` — the parallax of the offset, exactly how far the point moves on screen, since facing is kept (`r#loop::shift_at`). A far light difference is measured over its **lit area**: the air cell and every non-air neighbour whose face its light falls on. The **near range** is derived (`r#loop::near_range`): the distance at which a one-cell difference on the offset's line stops shifting past the threshold — 10.6 blocks for a 6-block offset, 13.8 for 12. A cell nearer than it to some eye, before or after the jump, is in the near field.
- **The near field is identical** (`DW0946`): no near cell of `V` differs, in any configuration the route passes through — every runtime write reaching the span applied first, and each exercise crossing's `on_cross` writes — and the message names the write and its root.
- **The far field moves less than station 4's** (`DW0947`): every far difference is measured, and a configuration with any whose shift exceeds `r#loop::FAR_FIELD_SHIFT_DEGREES` (1.2852°) is refused on the worst. The threshold is station 4's largest far-field shift — the eldritch spike's hall, its end visible 60 blocks past the slab with a 12-block jump, the one loop on record walked on a client and read as seamless — 1.285125° rounded up at the fourth decimal, read by `station_4_calibrates_the_far_field_threshold` and by the independent `crates/delvec/tests/measured/far_field.py` (spec-0090 §5). With it, a goal that never comes nearer stands about 60 blocks past the slab for a 12-block jump (66 lit) and about 48 for a 6-block one; spec-0090 §5.3 tabulates the rest.
- **Volumes and bodies**: a lethal keep-out or a `teleport` volume must equal its own image in the near field (`DW0946`); a compiler-placed body in the near field is `DW0948`; a body in the span past it is a far feature, its shift read over its feet and head cells from every eye, seen or not, and refused over the threshold (`DW0947`).

**Not checked, by name** (also the ledger's `unchecked` list): a client frame; particles and positional sounds alive in the span at the move; items on the ground and projectiles; a witness, who sees the mover's body jump; chunk streaming at the far ring and a client render distance below the server's; a body caught mid-jump, whose eye stands higher than the standing eye the view is judged from.

**The proof and the populations.** The slab is a gated seal (*Runtime region solidity*); the forced route that meets the loop while it holds is exercised (`critical-path.json`'s `loop` step), with the crossings' writes credited as forced; `DW0921` judges each configuration with each slab holding or clear; `DW0891`'s population roots gain every landing-slab cell and each exercise step's landing, and `validation/lethal-gate.json` records them under `danger_visibility.roots`; `DW0897` reads the landing as a destination.

### The map editor edit stage (spec-0017)

`crate::edit` replays the optional stage-7 script over the assembled model
(§1 pass 8) so **every** downstream consumer — relight, nav, wave seating,
waypoint/POV export, `snapshot`/`blocking-chart`, emission — sees the edited
world. Invariants:

- **Per-batch re-proofs.** After each batch the replay re-settles gravity
  (`DW0313` on a despawn, batch-attributed), re-runs the spec-0010 relight
  (`DW0210`/`DW0211`), re-proves critical-path + checkpoint walkability
  (`DW0311`/`DW0315`/`DW0316`, with the relight fixtures solid), and runs the
  **boundary-safety** check (`DW0322`, `nav::verify_boundary_safety`, stated
  per **horizon** — see the `DW0322` catalog row and *Boundary safety and the
  world-generator ambient* below). This is the guarantee the greenfield berm
  provided physically, made checkable so an edit script may reshape a boundary
  into natural landform. Reused codes keep their tiers; failures are prefixed
  `after world-edits batch `<id>``, and every violation of a run is aggregated
  into one report (bounded listing + total), never just the first.
- **Trap-hardware integrity (`DW0352`).** No batch write may land on a trap's
  trigger/hazard cell, dispenser socket or disarm-affordance cell. The world build
  runs **before** `setup_finish`'s `trap_setup`, so a colliding edit lands first and
  the trap is then wired into a block that is gone — vanilla's `item replace
  block … container.0` on a non-container fails with **no output**, shipping a
  dead trap while every geometry proof stays green (`DW0342` proves the *planned*
  hazard, not the surviving hardware). Structural, so it is checked first, before
  the geometry re-proofs.
- **Support validity (`DW0354`).** Every support-dependent block the script has
  placed (torch/lantern/campfire/rail family; flora) is re-checked at each batch
  close against the current world: a later batch that carved its support away, or
  a `scatter` that dropped flowers onto a non-soil block, leaves a block vanilla
  pops off as an item on the first chunk tick — the edit silently undone while
  every snapshot still shows it. **Advisory** for decoration (aggregated per
  reason + block, with a count and one example cell); **error** when the popped
  block is a fixture the script's own `relight` verb placed, since that is a
  declared `min_light` guarantee the `DW0211` proof accepted. Conservative by
  construction: a block whose support is sideways or above (`wall_torch`, a
  `hanging=true` lantern) is classified as needing none, and "support removed"
  means removed to **air** — the check never guesses about a block it cannot
  classify.
- **Boundary safety and the world-generator ambient (`DW0322`).** The check's
  premise is what a column the compiler modelled *nothing* into actually holds in
  the delivered world — a property of the level generator (`nav::Ambient`,
  spec-0013 `horizon`), not of the content. It rides on `nav::World` as one of
  the premises `nav::Premises::of_plan` carries (see *Nav* above), so the
  pristine assembly, the edit replay and the stage-10 whole-world call all have
  it by construction rather than by three call sites remembering. It never feeds
  the walkability sets, so routing and standability are untouched by it.
  - **Anchor seating (`nav::AnchorRoot`).** The walk region this proof examines
    is flooded from every resolved anchor, and an anchor is a declared *point*,
    not a floor cell — seating it is a nearest-standable-cell snap. That snap
    chooses by squared distance and **nothing else**: it does not care that solid
    geometry stands between the anchor and the cell it lands on. So each root
    carries the AABB of the piece that declares it, and the snap may not leave
    it. Without the confinement a ceiling anchor — which every spec-0022
    `collapse` payload must declare — is nearer the cell on top of the ROOF (Δy
    2) than its own floor (Δy 3), so the proof would root itself on a bare
    platform the party can never reach and demand a safe edge there, which no
    free-standing prefab in a void world can give it — the leak
    `World::confined_standable_cells` closes for wave seating, one layer down.
    Only the **seating** is confined; the walk that follows is not, because a
    player who reaches a room reaches whatever it connects to — confining the
    flood would shrink the region examined, which is a weaker check, not a more
    correct one. An anchor whose piece offers no footing within `SNAP_RADIUS`
    seats nowhere and contributes no root, as an unsnappable start does.
  - **`Ambient::Void`** (`horizon: void`, the default): bottomless columns are the
    hazard.
  - **`Ambient::Ocean`** (`horizon: ocean`) — the ambient is the pinned superflat
    (`plan::SEA_LEVEL` = 62 water top, `plan::SEA_FLOOR_TOP_Y` = 54 sea floor,
    bedrock below), present in every column **except** inside a placed piece's
    AABB (`/place template` writes the whole box, air included; the water *under*
    an island base is still ambient). Bedrock everywhere ⇒ the void premise is
    vacuous, and the real hazard is **stranding**, modelled as:
    1. **Entering.** A reachable walkable cell puts the player in the sea when a
       horizontally adjacent column is enterable at its level (feet + head clear
       of solids and 1.5-tall barriers — water does *not* block walking in) and
       that column is open, between that level and the sea surface, all the way
       to ambient water. Walking in, wading in and falling off a cliff are the
       same outcome: vanilla buoyancy leaves the player afloat at `sea_level`.
    2. **The sea.** A cell at `y == sea_level` is swimmable when it is neither
       solid nor tall and is either ambient water or *authored* water (a lagoon
       at sea level is physically the same plane). Swimmable cells 4-connect into
       **bodies**; a body reaching the edge of the search window (the placed
       geometry inflated by `nav::OPEN_SEA_MARGIN`) is the open sea, and all such
       bodies are one, since the ring beyond the window is untouched ambient
       water in every direction. Connectivity is taken on the surface plane only
       — a diver might swim under a land bridge into another body, which the
       model deliberately does not count on.
    3. **Climbing out.** A body is escapable when one of its surface cells is
       horizontally adjacent to a **proven reachable walkable** cell whose feet
       are at `sea_level` (a rim one block under the waterline: wade out of the
       shallows) or `sea_level + 1` (the canonical beach — land flush with the
       surface; this is the island tileset's own convention, waterline local y=2
       / walk plane local y=3). A lip two blocks above the surface is a wall to a
       swimmer, and adventure mode has neither boat nor blocks.

    A body the player can enter and cannot climb out of is the violation. The
    granularity is **per body**: an island with a perfect outer beach still fails
    on an inner pool with 2-high walls, which a global "is there a climb-out
    anywhere" test would pass. Requiring the climb-out cell to be in the
    *reachable* walk region is what makes it a return, not just a landing.
- **Gate-region collision (`DW0353`, advisory).** A world-edit inside a region a
  runtime write fills — a `close-gate`'s gate region, or a `fill-region`'s box — is
  overwritten with that write's block when it fires (a solid, or water/lava) and
  cleared to **air** when its dual does,
  so one cycle erases it. It reads `plan.region_events`, so every region-writing
  verb is covered without a line of its own. The proofs stay sound (the occupancy model
  already treats the region as gate-controlled), and dressing the *sealed* state
  is a legitimate intent — hence a warning, one per colliding gate region.
- **Determinism (ADR-0006).** Edit noise is position-addressed value noise
  (the island/cave generators' primitive family, ported into `crate::edit`)
  seeded per script position; the double-build gate covers the edited fixture
  (`tests/edit.rs`).
- **View mode.** `snapshot`/`blocking-chart` replay the script **without**
  enforcing invariants (`edit::replay_view`) — a broken state must be
  viewable; only region-resolution failures (`DW0323`) stop a view.
- **The loop** (`delvec edit apply|preview`, §7): full validation → replay
  with invariants → one labelled snapshot + manifest per batch (framing the
  batch's edited AABB over the final edited world) → **the whole build-tier
  proof set**. `apply --batch` appends a candidate batch and persists
  `world-edits.json` (canonical form) only when all of that is green; `preview`
  never writes to the campaign dir. A red candidate can never leave a broken
  script behind.
- **One proof tier, not two.** The per-batch invariants are a *subset* of what
  `build` proves — they miss cutscene clipping (`DW0308`), stealth zones
  (`DW0327`), trap completability (`DW0342`), wave seating (`DW0312`),
  `move-npc`/`move-actor` routability, and the exported-route/POV self-checks.
  `edit` therefore runs `analyze` + `emit::build` (output discarded) before
  persisting, so a script `apply` accepts is a script `build` accepts; the build
  costs about what the command's own snapshot render does, so a cheaper tier has
  no reason to exist.
- **Atomic persist.** `world-edits.json` is written to a sibling `.tmp` and
  renamed into place: the artifact of record (ADR-0006) is never left truncated
  by a crash or a full disk.
- **Forceload lifecycle.** `setup` forceloads every piece bbox *and* every edit
  AABB. Each edit chunk that no piece bbox covers gets its own convergence
  sentinel in `place_verify` (`execute if loaded <cell>` folded into `#placeok`),
  so the one-shot world build cannot run into a still-loading chunk and lose
  those writes forever; the tick retry loop
  converges on them exactly as it does on piece placement. Those same chunks are
  then released (`forceload remove`) at the very **end** of `setup_finish`, which
  runs after the world build's last step and after every other write of its own. **Piece forceloads are never released** —
  the gameplay tick machinery (gate fills, wave spawns, checkpoint and trap block
  reads) keeps addressing those chunks for the whole session.
  **A span is split, never refused.** Every `forceload add` this compiler emits —
  piece bboxes, edit AABBs and the gallery admission pack's own cells alike —
  goes out through `commands::forceload_add_lines`, and nothing writes the
  command by hand. A span inside `FORCELOAD_MAX_CHUNKS` (256) emits a single
  line, coordinates untouched; a span past it is cut into a grid of
  `ceil(w/16) × ceil(h/16)` tiles, each axis into runs of as near equal length as
  they divide, so no tile exceeds 16 chunks on a side and none is a one-chunk
  sliver beside a full one. Splitting rather than refusing is the ruling: the
  span is **derived** — a piece's own bbox, or the ring a horizon grows around
  one, which for a 101 × 101 piece under `valley` is 17 × 17 = 289 chunks — so a
  refusal would hand a creator a legal piece under a legal horizon and no act
  that clears it, while vanilla caps only what *one command* may name and never
  how many chunks a world may hold. The command validator's forceload exception
  above is what makes a site that forgets the helper impossible to ship.

---

## 5. Diagnostics catalog

Every DW code in `crates/**/*.rs` has one catalog row, and **the row lives on the
page of the module that declares the code**: a code declared in
`crates/<crate>/src/<path>.rs` (or `<path>/mod.rs`) has its row in
`docs/reference/<crate>/<path>.md` — strip the crate root, replace `::` with `/`,
append `.md` — so `compiler::nav`'s codes are in
[`delvec/compiler/nav.md`](delvec/compiler/nav.md) and `dsl::diagnostic`'s in
[`dsl/diagnostic.md`](dsl/diagnostic.md). `delvec codes` prints each code's
declaring module. `tools/ci/check-dw-codes.py` holds the catalog bidirectionally
exact against source, one row per code, and each row on its declaring module's
page, so a code whose declaration moves and whose row does not is a red;
`tools/lib/dwcatalog.py` is the one reader of the catalog every gate goes
through. A page may say more than its rows; nothing on it is a row for another
module's code.

**A code is declared inside `dw_code!`, and nowhere else.** The macro
(`delvewright_dsl::dw_code!`) expands to the constant as written —
`pub const NAME: DwCode = DwCode::new("DWxxxx", ExitTier::…);`, or
`pub const NAME: &str = "DWxxxx";` for a code of a verb with its own exit table
(`prefab`, `schem`, `render`, the view arms) — and registers it in
`delvewright_dsl::diagnostic::DECLARED`, a link-time distributed slice. There is
no list beside the declarations. `delvec codes` prints the registry, sorted by
code, one JSON object per line:

```
{"code":"DW0944","tier":"Build","subject":"Campaign","name":"PERCEPTION_SIGHT_UNDER_A_CAMERA","module":"delvewright_dsl::diagnostic::codes"}
{"code":"DW0721","tier":null,"subject":null,"name":"DW_INPUT","module":"delvec::compiler::view::diag"}
```

with the count on stderr and exit 0. It is what the binary can print, which a
scan of the binary's bytes is not: a code raised at one inlined site is written
by immediate stores and never spelled contiguously. `check-dw-codes.py --delvec
<binary>` holds the registry equal to the declarations its `CONST_RE` reads, by
`(code, constant)` in both directions and by exit tier, so a constant declared
outside the macro reds; `crates/delvec/tests/codes.rs` runs it against the
binary cargo built.

**Test-coverage gated** (CLAUDE.md Conventions). The same
script also fails CI if any documented, landed code has no test asserting it —
either the literal code string or a symbolic diagnostic-code constant (e.g.
`DW_STRIP`) that resolves to it — resolved per module, the way the compiler
resolves it: through the `use` line that imports it, a `use super::*` of the
module that declares it, or a qualified path, with `pub use` re-exports
followed, because modules of the one engine crate reuse a constant name for
different codes (`DW_INPUT` names a different code in `delvec::schem::diag`,
`delvec::compiler::view::diag` and `delvec::admit::diag`) — appearing in
`crates/<crate>/tests/**/*.rs` or a `#[cfg(test)]` module in
`crates/<crate>/src/**/*.rs`. A code that is
genuinely unreachable without external resources (e.g. `DW0720`, which needs a
GPU adapter + the never-committed 1.21.11 client jar) may be declared in the
script's `ALLOWLIST` with a one-line justification — kept minimal; writing the
test is always preferred.

**Remediation contract.** Every DW message is the repair protocol for a
zero-context author: it states **what** is wrong (with the offending name/coord/
count/limit interpolated), **where** to fix it (the campaign stage/field, the
prefab/tileset, or — for an invariant breach — "compiler bug, escalate"), and
**how** to fix it; where a tempting wrong fix exists (weaken a threshold, reroll
the `seed` against ADR-0006, widen a socket seam, bypass the allowlist) the
message names it with an explicit "do NOT". Each row summarizes its code's
*meaning*; the emitted message additionally carries the prescription. Gold
standards: `DW0312`, `DW0210`/`DW0211`, `DW0304`, `DW0306`.

**A run's lines are grouped, author-actionable first.** Every diagnostic
declares whose state its verdict is about (`dsl::diagnostic::Subject`): the
CAMPAIGN — every refusal, and every advisory that is a fact about the documents
in front of the author — or the ENGINE, meaning a table that is still seeded or a
standard nobody has calibrated, which reads the same on every campaign this
engine compiles. `delvec` sorts on that before printing, into three labelled
groups in this order: refusals, advisories about this campaign, notices about
this engine. The sort is stable, so within a group each pass's own order
survives, and it applies to `--json` as well, because a consumer reading the
first line should get the actionable one for the same reason a person should.
The run's binding counts follow all of it on stderr, under `-- what this run
examined`; every one is still stated, zeroes included. Nothing is suppressed by
the grouping — it decides order, never whether a line prints.

**A message states its finding; this catalog holds the reasoning.** Where an
advisory's message would otherwise be several paragraphs explaining why it
refuses nothing, the message is one line with its numbers and the essay is the
row on its module's page. `DW0813`, `DW0822` and `DW0781` are written that way.

**A secondary whose premise is an already-reported primary does not print N
times.** Either the findings are one diagnostic naming all of them (`DW0842` at a
zero box count, `DW0826` where more than one thing leaves the region,
`DW0150` where stage 5 is empty), or the line stands and gains a clause naming
what it is downstream of (`DW0818` where stage 5 declares no quests, `DW0843`
and `DW0844` where the site plan has already refused the frame or the seam set).
The rule and its two shapes are in `dsl::diagnostic`. No code loses its ability
to refuse alone: every fold arm is reachable only in the state that made the
copies identical.

**A rule whose whole input is missing does not report at all.** The third shape,
and the one no raise site can serve: a site-plan campaign's anchor vocabulary is
derived from `layout-graph.json`, so with that document gone the set is not
empty, it is unknown — and every rule resolving a name against it was refusing
correct names, each with a prescription (*write one of the names the graph
places*) that cannot be taken while there is no graph. `DW0142`, `DW0371` and
`DW0343` therefore ask
`dsl::placement::anchor_vocabulary_unknowable` first and stay silent, exactly as
they stay silent for a `prefab_pool` whose draw the compiler has not made; the
`DW0824` line says so in its own words, and every name is judged with the same
rules the moment the graph exists.

**A prescription is chosen by the campaign's placement authority, not by the
rule that raised it.** A campaign hands its space either to stage-1 `areas[]`,
which seats prefab pieces, or to a `site-plan.json`, which owns a derived
blockout — never both (`DW0839`) — and a campaign may also have declared
neither yet. Where a name does not resolve, the sentence saying what to write
instead is asked of `dsl::placement::Placement`, one authority for all three
answers, rather than written beside each refusal:

| Placement | An area id resolves against | An anchor name comes from |
|-----------|-----------------------------|---------------------------|
| `Prefabs` (`areas[]` non-empty, no plan) | the stage-1 `areas[]` entries | the bound prefab's metadata |
| `SitePlan` (a `site-plan.json` is present) | exactly one id, `area/site` | the derivation: `anchor/node-<place>`, `anchor/seam-<edge>`, `anchor/unlock-<edge>`, `spawn` — **plus every `stations[]` name the layout graph's nodes declare**, which is why this arm also offers *declaring* the name and not only correcting it: a station is the one anchor name in this engine an author writes by hand, so an unresolved one is as likely a missing declaration as a typo |
| `NoMap` (`areas[]` empty and no plan) | nothing — the campaign has no map | nothing — no anchor is placed |

The same authority answers the prior question — *can this be judged at all* —
because the `SitePlan` remedy is itself unreachable in one state:
`anchor_vocabulary_unknowable` is true for a site-plan campaign with no
`layout-graph.json`, where there are no places, ways or nodes to write a name
from, and every anchor rule stays silent rather than refusing a correct name
against an empty set.

The `Prefabs` arm is the prefab prescription each site prints. The other two
arms **replace** it rather than appending to it, because every prefab
prescription (`declare it in stage-1 world.areas`, `bind a prefab/pool`, `anchor
names come from prefab metadata; do NOT invent one`) is refused by `DW0839` or
`DW0160` in a campaign carrying a site plan, or names prefab metadata a derived
map does not have. A `NoMap` refusal names **both** authorities, since which one
the author wants is a choice they have not made and a refusal must not make it
for them.

It binds at every call of `Placement::area_remedy`, `Placement::anchor_remedy`
and `Placement::lighting_field`: in `dsl::validate`, the area refusals (`DW0112`
— an npc's area, a planned quest's area, and a stage-7 edit script's
`batches[].area`) and the anchor refusals (`DW0142`, `DW0194`, `DW0340`,
`DW0371`, `DW0377`, `DW0381`); in `compiler::emit`,
`DW0360`, `DW0426` and `DW0447`; in `compiler::gates`, `DW0343`; in
`compiler::edit`, the two `AnchorRelative` frame failures a stage-7 edit script
can raise; and in `compiler::light`, `DW0210` and `DW0211`. `crates/dsl/tests/v14_site_plan.rs`'s
`no_refusal_on_a_derived_map_prescribes_a_prefab_document` binds it over a
derived map's whole refusal set, keyed to the forbidden prescription rather than
to a list of codes.



---

## 6. Spec cross-reference

Which spec introduced or last amended each area (specs are historical records;
this doc is current behavior).

| Area | Spec |
|------|------|
| DSL schemas, stages 1–6, envelope, ids, l10n key scheme | spec-0001 (v0.1/0.2/0.3 + i18n addendum) |
| CLI, exit codes, build output, world config, environment sealing, critical path, gameplay-verb emission, jigsaw solver, `--lang` build | spec-0002 (v0 + v0.3 / M2 vertical / i18n addenda) |
| Validation ↔ runtime split; `DW02xx` analysis role | ADR-0005 / spec-0005 |
| v0.4 surface (dialogue state, props, narrate, wave tuning, NPC lifecycle, skins, triggers, cutscene, `DW0190`–`DW0195`, `DW0307`–`DW0311`) | spec-0008 |
| Skins toolchain, resourcepack bake (`DW0309`) | spec-0009 |
| v0.6 scripted actors + staging effects (`actors[]`, `spawn`/`despawn`/`move`/`unleash-actor`, `sequence`; footprint-aware nav; `DW0325`/`DW0329`) | spec-0014 |
| Assembled-relight, measured `DW0210`, `DW0211`/`DW0196`, stage-1 `lighting`/`time`/`weather`, `set-time`/`set-weather` (all v0.5) | spec-0010 |
| Stage-1 `horizon` (ocean superflat), `boundary` (derived playable region + 1s return clock), `dw:region`/`dw:cp` mirrors, `DW0320`/`DW0321` (all v0.6) | spec-0013 |
| Sound + art-title surface (`play-sound`, `narrate` `art`, `delve:art` font, `DW0326`/`DW0328`/`DW0335`) | spec-0014 (v0.6) |
| Traps: stage-5 `traps[]`, `anchor/trap` dispenser fill + disarm emission, `tnt_explodes` seal, passable plate/tripwire model, `DW0340`/`DW0341`/`DW0342` (all v0.6) | spec-0011 |
| Visual authoring loop: `delvec snapshot` + `delvec blocking-chart`, the voxel raycaster, scene manifest and cutaway floor plans (§7) | spec-0015 |
| Souls-mode timed gates: stage-5 `timed_gates[]`, the two-function schedule clock, `DW0377`/`DW0378` (≥20% of cycle) (v0.6) | spec-0016 §4 |
| Hazard observability: every `timed-gate` span and `volley` kill zone needs a watch cell — standoff, pre-commit reachability, sightline (`DW0388`; error for a bonfire campaign, warning otherwise) | spec-0016 §4 addendum |
| Timed-gate `disarm`: the hazard ladder's third rung — a jam lever that stops the clock with the gate resting open, permanently (`DW0377` structural, `DW0389` no re-arm, `DW0393` reachable while shut, `DW0420` visible) | spec-0016 §4 |
| Timed-gate `crush`: the closing edge kills players caught in the region, by command (default off, byte-identical when unset) | spec-0016 §4 addendum |
| Affordance hardware: every compiler-owned right-click target carries its own visible, glowing `dw_hw_<tag>` display; `DW0420`/`DW0421` | spec-0016 §2 |
| Souls-mode ambushes: stage-5 `ambushes[]` (parse-time desugaring to a trigger), `DW0375`/`DW0376`, optional telegraph (v0.6) | spec-0016 §3 |
| Souls-mode TD lanes: wave `lane{waypoints,aggro_radius}` + `summon: aggro-edge`, the Raider-patrol clock, `DW0381`–`DW0387`, `pillager`/`vindicator` added to the armed-mob default table (v0.6) | spec-0016 §6 |
| Souls-mode shortcut doors: stage-5 `shortcuts[]`, `DW0371`/`DW0372`/`DW0373`/`DW0374`, shortcut gates sealed for the whole completability model (v0.6) | spec-0016 §2 |
| Souls-mode pacing lints: retry cost `DW0379`, optional-elite bypass `DW0380` (both warning tier) (v0.6) | spec-0016 §7 |
| Souls-mode bonfires: `bonfire{anchor,on_rest?}`, wave `respawns_on_rest`, `DW0370` (v0.6); the two-option rest dialog + authored labels, the class-kit `flask` + `DW0476`, the flask's potion `contents` + `DW0486`/`DW0487`, the critical path's `rest` step (v0.8); the stationed re-seat + the bonfire safe zone `DW0478`, whose lane term includes the measured marching drift; the **undefeated re-seat** — a still-standing actor elite / billed wave is deleted and re-seated fresh at its origin on rest and on death-respawn, a defeated one stays defeated | spec-0016 §1 |
| The map editor: stage-7 `world-edits.json`, the full L3 verb set (`select`/`fill`/`replace`/`carve`/`morph`/`scatter`/`plant`/`fragment`/`relight`), the L2 massing verbs (`swap`/`insert`/`remove`/`rewire-socket`/`reseed`; `resize` excluded — no size primitive), per-batch invariant re-proofs, `DW0162`/`DW0322`/`DW0323`/`DW0324`, `delvec edit apply|preview` (all v0.6) | spec-0017 |
| Map-editor audit fixes: trap-hardware integrity `DW0352`, gate-region + block-support advisories `DW0353`/`DW0354`, out-of-bbox edit-chunk load convergence + forceload release, `edit` running the full build-tier proof set, blockstate-preserving `fragment` stamps | — |
| Party-shared progression: the `#party` holder, party-addressed UI, `world.min_players` + lobby gate, `give-item`/kit `carrier`, the n-agent division proof and the n-dummy `party_join_<obj>` PackTests, `DW0356`/`DW0357`/`DW0358` (all v0.6) | spec-0018 |
| The NPC scene ledger: stage-5 `cast` (DSL **v0.7**), the four build proofs `DW0460`–`DW0462`, the forcing function `DW0463`, dangling refs `DW0464`, the `"unchanged"` sugar `DW0466`, the staleness lint `DW0467`, the dead-clause proof `DW0846` and the unanswerable-objective refusal `DW0858`; the `dw.cast` scene dispatch + bark pools; cast roots as dialogue entry points | spec-0020 |
| Combat verification: wave `tier` (DSL **v0.7**) and actor `tier` (DSL **v0.8**), the combat proofs `DW0470`–`DW0473` + the advisory `DW0475`, the vendored `item-combat` / `damage-types` tables, `validation/combat-plan.json` (encounters + each wave's muster + run-backs), and the bot ladder's die-retry stage | spec-0023 |
| Branch-complete narrative verification: stage-4 `branch_points`, the per-node `happening`, the named `campaign-complete` `ending` (DSL **v0.8**); the six proofs `DW0480`–`DW0485`; `validation/branch-plan.json` + the per-branch chronicle + the per-branch executable path, and the harness's scripted-choice branch runs (`DELVEWRIGHT_BRANCH`/`DELVEWRIGHT_BRANCHES`, `validation/branch-runs.sh`; the `from-diff` PR tier has no compiler-side diff→branches map and refuses) | spec-0025 |
| Asset-pipeline tooling `DW07xx` (schem/render/prefab/grammar) | spec-0007 |
| Determinism invariants | ADR-0006 |

### Known spec ↔ code drift (current, for maintainers)

- **Effect roots are enumerated once.** An *effect root* is a `Vec<QuestEffect>`
  emission can lower. There are nine (`EffectRootKind::ALL`); eight hang off the
  quests stage and one off dialogue. A walk that visits four of five roots
  produces correct-looking output over any campaign that does not use the fifth,
  so no walk enumerates roots for itself:

  - `delvewright_dsl::effects::for_each_effect_root` is the single enumeration.
    `for_each_effect_root_mut` (the l10n write path) is generated from the **same
    macro body**, so the ref/mut pair cannot drift either.
  - `dsl::for_each_campaign_effect` = that walk + the single nesting authority
    (`QuestEffect::nested_effect_lists_labeled`). Both axes inherited, neither
    listed. Its `EffectSite` has a variant per root, so every root is
    *representable* in the callback.
  - `compiler::plan::for_each_effect_root` and `l10n::effect_roots` /
    `effect_roots_mut` are thin adapters over it.
  - `EffectRootKind::ALL` is the closed set, and the walk **asserts on every call**
    that it enumerated all of them — in release builds too, because a root quietly
    dropping out of the enumeration has no other symptom.
  - Every walk reports a `RootBinding`: how many roots it enumerated and how many
    bundles each bound to on this campaign. *Enumerated the root* and *this
    campaign uses the root* are different facts, and a proof that conflates them
    reports a vacuous green as a pass.

  Adding a root is one edit in that macro, and every walk inherits it.

  **The claim is also a test matrix.** `crates/delvec/tests/effect_root_walkers.rs`
  iterates `EffectRootKind::ALL`, builds one campaign per root from an exhaustive
  `match` (a tenth root is a compile error there), and asks six walkers — the
  enumeration, `for_each_campaign_effect`, the l10n inventory, `flow::gate_flags`,
  `emit::declared_flags` and emission itself — plus `DW0360` about every root. The
  per-walker tests beside it each prove ONE walker against the roots their author
  remembered, and stay green when a root is added; the matrix cannot.

  Two guards stand outside the type system, because the root fields are ordinary
  public fields. `tools/ci/check-effect-roots.py` fails CI when a window of source
  names three or more distinct roots outside a reasoned allowlist — a proximity
  heuristic and a tripwire for a hand-rolled walk, not a proof of absence. It
  protects against forgetting a KNOWN root; the gate for a root nobody knows is
  `tools/ci/check-capability-ownership.py` check E, which reads the effect-bundle
  FIELDS out of the DSL stage-surface modules (its `DSL_STAGE_MODULES`
  registry) and fails on any it cannot account for.

  **Open finding: `plan::required_anchors_for_area`.** It collects the anchors an
  area's assembly must provide from R1+R2 (and R3 only when the campaign has a
  single area), so an anchor named only in a `traps[].payload` or a dialogue
  `on_respawn` bundle is never registered as required. It is not a mechanical
  widening: a trap payload has no area attribution — a trap carries an `at`
  anchor, not an area — and registering its anchors in every area is the
  over-provisioning that function's own comment warns against. `DW0360`/`DW0447`
  catch the resulting unresolved anchor at build time, so this is a worse message
  rather than a silent drop. It is recorded in the guard's allowlist so it stays
  visible on every CI run.

  **Sound by construction, not a walker: `world::world_checks`.** It checks
  the stage-1 *fields* R1–R3 and never walks an effect root, so widening it would
  report the same campaigns with a worse message.

  **`DW0360` vs `DW0447` overlap** (open). The seal reaches R4, so the spec-0022
  payload-verb anchors (`volley.from_anchor`, `volley.kill_zone.anchor`,
  `collapse.region_anchor.anchor`) are in its reach, and `DW0447` owns exactly
  that predicate, fails just as hard and says more. The seal therefore scopes itself to the verbs that fail **open**
  and lets the fail-**closed** payload verbs keep `DW0447` — but only where
  `DW0447` runs. That qualifier is the finding: **there is no rule confining
  `volley`/`collapse` to `traps[].payload`**, and `plan_payload_verbs` lives inside
  the world block, so `DW0447` is unreachable for a campaign with no traps, no
  waves, no bodies and no walkable critical leg. An unconditional deferral there
  would surface a typo'd anchor as `DW0497`, whose message tells the author the
  *compiler* is defective and which fires identically when the anchor is correct.
  The deferral is therefore conditional on `emit::assembles_world(plan)`, the same
  predicate the world block itself reads.
  `anchor_seal::typod_volley_anchor_is_dw0447` pins the specific diagnostic on a
  single-objective campaign, which assembles a world for its first leg
  (`anchor_seal::a_single_objective_campaign_assembles_a_world_for_its_first_leg`).

  **Adjacent, open:** a `volley` on a quest's `on_complete` in a world-less
  campaign fails the build with `DW0497` **even when its anchor is valid** — the
  call site emits `function <ns>:volley_<key>` while `plan_payload_verbs` never runs
  to emit the machinery. A genuine call-walk/machinery-walk disagreement of the
  class `DW0497` exists to catch; the fix is either to confine the payload verbs to
  `traps[].payload` at the DSL layer or to make their machinery independent of the
  world block.

- **spec-0002 CLI** lists stages `1..5` and omits `--json`/`--prefabs`/`--lang`;
  code validates stages `1..7` plus the map-pipeline documents, declares the one
  `dsl_version` `delvec --version` prints, and has all three flags. (Spec is the
  original record — this doc governs.)
- **`gamerule keep_inventory true`** is emitted by the sealing baseline but is
  **not** in spec-0002's environment-sealing list (box-garden death policy).
- **spec-0018 runtime tier, partial by design.** The static half is complete
  (completability is proven with `min_players` agents; `DW0358`) and the runtime
  half is complete for AND-joins (the generated n-dummy `party_join_<obj>`
  templates). The **critical-path bot** is single-bot: `critical-path.json` and
  its replay describe one abstract playthrough of party state, which is exactly
  right for `min_players: 1` and is a *sound* (if not maximal) proof for a bigger
  party — one agent can always walk what n can divide. The harness does not run
  `min_players` bots; that is harness work, not a gap in this layer's contract.
- **Sky attenuation constants** (`crate::light::effective_sky`, spec-0010): the
  stored sky-light baseline (15 at a sky-open cell) and the `time`/`weather` set
  commands are live-verified (1.21.11 itzg VANILLA); the per-state *effective*
  attenuation follows the documented vanilla `getSkyDarken` surface model
  (noon/day 15, night/midnight 4, rain −3, thunder −8 by day) applied
  conservatively — the effective (time-attenuated) value is not directly
  command-readable, so it is not a live measurement. `delvec prefab`'s per-piece
  probe reads its two sky levels out of this same function rather than restating
  them, so a change here reaches the probe with nothing there to edit.

---

## 7. Visual authoring loop (spec-0015)

A **view-only** tier of `delvec`: draft renders of the assembled world plus a
structured description of the same frame, so an authoring agent can look at its
own build mid-authoring instead of waiting on a full build + Chunky pass. Two
commands — `snapshot` (a perspective viewport: what does it look like from here)
and `blocking-chart` (orthographic cutaway plans: is there room). Both add no DW
diagnostics, change no emission (build output is byte-identical), and never write
a datapack.

### `delvec snapshot`

```
delvec snapshot <campaign-dir>
    [--camera x,y,z,yaw,pitch[,fov]]      # explicit eye
    [--at <anchor> [--orbit <deg>] [--dist <n>]]   # frame a subject
    [--shot <render-plan id>]             # reuse a planned camera
    [-o out.png] [--labels]
    [--width 960] [--height 540] [--timing] [--json]
```

Framing precedence (the first three are mutually exclusive, enforced by clap):
`--camera` → `--at` → `--shot` → a default dollhouse overview of the whole
layout. Details:

- **`--camera`** — `x,y,z` is the eye in world coordinates; `yaw`/`pitch` are
  **Minecraft** degrees (`0` = south/+Z, `90` = west/−X, `180` north, `270`
  east; pitch positive looks **down**), the same convention the v0.6 cutscene aim
  uses. `fov` is optional (vertical, default `70`).
- **`--at <anchor>`** — accepts a bare anchor name (`anchor/fire-pit`, matched in
  the first declaring area) or `area:anchor` (`area/island:anchor/pen`) to
  disambiguate; a gate anchor resolves to its region centre. `--orbit` is a
  compass bearing in the same yaw sense (`0` = the camera stands due south of the
  subject looking north, `90` due west looking east); `--dist` is blocks (default
  `14`), with the eye raised `0.45 × dist`. **The eye is then pulled along its own
  sight line until it stands in open air**, so `--at` frames an interior (a
  cavern fire pit, an alcove) instead of rendering the inside of the mountain.
  The walk is `compiler::camera::stand_in_open_air`, shared with the render
  plan's own cameras: standing a camera up in open air is a property of a
  camera, so there is one of it.
- **`--shot <id>`** — reuses a `render-plan.json` camera by id (`interior/…`,
  `npc/…`, `interact/…`, `gate/…`, `seam/…`, `pov/leg{L}/wp{W}`). The render plan
  states cameras in its own Chunky yaw convention, so the bridge reads only its
  `pos`/`look_at` world points and re-derives Minecraft yaw/pitch. `pov/…` ids
  additionally compute the DW0311 critical-path routes; other ids do not.
  An unknown id lists the available ones. The plan is derived against the same
  edited assembled world this command rasterises, so a camera the plan stands up
  out of the rock is stood up identically here — `--shot` frames what the built
  plan states, never a second opinion about it.

**Pipeline stages required** — parse → `Plan::build` (placement) → read the
placed `.nbt` → `assembled::assembled_blocks`. That is all: no relight, no nav
proofs, no emission. Validation diagnostics are printed but **never gate** the
render (only an unparseable campaign, exit 1, or a placement failure, exit 3,
stops it), because the loop exists precisely to look at builds that are not
finished yet.

**Renderer** — a voxel DDA raycaster (`compiler::snapshot`) over a chunked
flattening of the assembled block map. Shading is block-palette colour ×
face brightness (top brightest, bottom darkest, the two horizontal axes
distinct — the "ambient occlusion by face orientation") × a **grain** whose
amplitude is the block's measured roughness × a block-edge relief darkening,
then a distance fade toward the horizon. Background is a sky
gradient; for an `ocean`-horizon campaign the world generator's sea plane is
drawn analytically at `SEA_LEVEL` (a world-generation backdrop, never part of
the voxel model, never occluding a manifest target).

Three properties worth stating explicitly:

- **There is no lighting model.** The raycaster sees geometry regardless of block
  light, so a pitch-black cavern renders as legibly as a noon meadow — which is
  exactly what makes this the right tier for reviewing dark areas. A frame that
  looks fine here and black in Chunky has a *lighting* defect, not a geometry
  one, and the two tiers separate that. Emissive blocks (glowstone, lantern,
  campfire, torch, …) still render at full brightness so a fire pit reads as one.
- **Only blocks exist.** Entities (NPC mannequins, scripted actors, item
  displays) are not in the assembled model and are not drawn; their *posts* are
  in the manifest and, with `--labels`, stamped on the frame.
- **The colour is the pinned client jar's, and a block the pin does not have
  renders magenta** (`255,0,255`, the same missing-texture key `delvec render`'s
  fidelity gate scans for). `snapshot::block_color` reads
  `crates/delvec/data/block-appearance-1.21.11.json` — every block of the pinned
  registry at its default state, `minecraft:plains` tints, derived by
  `compiler::view::blockcolor::Deriver` from the jar the GPU path textures with,
  so the draft and the render hold one opinion about what a block looks like.
  1161 of 1161 non-air blocks resolve; `crates/delvec/tests/preview_palette.rs`
  enumerates the registry and reds on any that does not, and holds the table's
  recorded `mc_version` to the engine's pin, so a pin bump that leaves the table
  behind reds with no jar in hand. The jar is EULA-bound and never committed; the
  derivation's output is, as for the shape-carrying property table and the font
  metrics. `python3 tools/maintenance/refresh-block-appearance.py <jar>`
  re-derives the table and proves every entry against that jar in the same run;
  the file is never edited by hand. The derivation it runs is a `cargo` example
  and not a `delvec` flag, because `delvec` is what an authoring session runs and
  a creator never holds this file. Keyed by block id: the grid draws every cell
  as a full cube, so a blockstate's own geometry has nowhere to go and
  `oak_slab[type=top]` shades as `oak_slab`. **What varies inside a face is the
  grain**, and the two halves of it are in different places. The PATTERN is this
  renderer's: an FNV-1a hash of the world cell, the face and a 4×4 sub-cell, so
  it belongs to the wall rather than to the camera and two runs give the same
  bytes (ADR-0006). The AMPLITUDE is the block's measured `roughness` — the
  standard deviation of its texture's brightness as a fraction of that texture's
  mean — scaled so the drawn face's spread is the spread the texture has, which
  is why smooth stone reads smooth and cobble reads rubbly. Without it a wall of
  one material is one rectangle of one value whatever the material: a draft worth
  as much as a paint swatch, and, for a dark stone, a frame the gallery render
  gate reads as showing no scene at all. **The table holds one number and no
  layout**, which is what lets it be committed while the jar cannot be: a
  reduced rendition of an asset would not be (ADR-0013).
  Magenta therefore means one of three
  things: `jigsaw` or `structure_block` reached the model (the solver strips
  both — the magenta is the alarm), a template carries an id 1.21.11 renamed
  (`chain` → `iron_chain`), or a datapack block outside `minecraft:`. **The
  fallback is never silent**: every surface that draws a grid prints `unpainted:
  N of M block kind(s) …` on stderr naming every id
  (`snapshot::unpainted_report`).

**`--labels`** burns in: a coordinate lattice tinted onto every visible **top**
face on a 16-block X/Z line (so it follows the terrain rather than an invented
flat plane), `x,z` readouts at the ten nearest visible lattice intersections, an
outline per in-frustum target (dim when occluded), and the target's name. Names
are placed **visible-first** and nudged down to avoid overlap; an occluded name is
stamped only where it lands clear on the first try. `--labels` changes the frame
and never the manifest.

**Output** — the PNG at `-o` (default `snapshot.png`) and a manifest sidecar at
the same path with its extension replaced: `shot.png` → `shot.manifest.json`.

### Scene manifest (`manifest_version: 2`)

```json
{
  "manifest_version": 2,
  "campaign_id": "nobodys-cave-island",
  "delvec": "<x.y.z>",
  "image":  { "path": "shot.png", "width": 960, "height": 540 },
  "camera": { "pos": [x,y,z], "yaw": 0.0, "pitch": 25.7, "fov": 70.0,
              "convention": "minecraft degrees: yaw 0 = south (+Z) …" },
  "world":  { "block_kinds": 48,
              "bounds": { "min": [x,y,z], "max": [x,y,z] },
              "sea_plane": 62 },
  "pieces": [
    { "area": "area/island", "index": 1, "prefab": "prefab/island-greenfield",
      "origin": [0, 60, -30], "size": [16, 12, 16], "rotation": "none",
      "box": { "min": [0, 60, -30], "max": [15, 71, -15] } }
  ],
  "targets": [
    { "id": "anchor/fire-pit", "kind": "anchor", "area": "area/island",
      "pos": [9, 69, -56],
      "screen_bbox": { "x": 466, "y": 264, "w": 28, "h": 36 },
      "occluded": false, "distance": 14.724 }
  ],
  "out_of_frame": [ { "id": "anchor/pen", "kind": "anchor", "area": "…",
                      "pos": [x,y,z] } ]
}
```

- **`pieces`** is the **layout** half of the scene, beside the point/region
  targets: every placed structure piece of the whole plan (not just the ones in
  frame), in plan order — areas as the plan holds them, pieces entry-first
  within each area. It carries exactly the inputs a `piece-local` edit frame
  resolves against (`edit::resolve_frame_point`: `origin + rotation(local)`
  against `area.pieces[index]`): the per-area `index` (the frame's `piece`
  field), the `prefab` guard value, the `/place template` `origin` + `rotation`
  token, the unrotated `size`, and the resulting inclusive `box`. Without it an
  editor authoring a piece-local frame would back-solve the index and the
  transform from the rendered geometry by hand.
- **Kinds**: `anchor` · `gate` · `npc-post` · `actor-post` · `interact` ·
  `stealth-zone` · `trigger`. A point target carries `pos` (an inclusive cell);
  a region target carries `box: {min,max}` (inclusive cells) — never both.
  `gate` is the gate region, `stealth-zone` a `begin-stealth` zone box
  (`stealth-<beat>/<anchor>`), `trigger` an `EnvTrigger` — a box of its `range`
  for `approach`, the single interaction cell for `strike`/`use`/`step`.
- **Deliberate duplication**: an `interact` objective's marker and the `anchor`
  it binds to are the same cell under two ids. They are different *things* —
  "the interact is occluded" and "the anchor is occluded" are different findings
  — so no deduplication is applied.
- **`screen_bbox`** is the projected inclusive cell box, clipped to the frame,
  in whole pixels with the origin top-left. This is the vocabulary spec-0015
  pillar 2 asks for: review feedback and edits address ids and boxes.
- **`occluded`** = every one of nine sight lines (the box centre plus the corners
  of a slightly inset box) meets a block that is **not part of the target**. Both
  refinements matter: a marker often *is* a block (`anchor/fire-pit` names the
  campfire), and a single centre ray grazing the rim of a platform would call the
  thing standing on it hidden.
- **`out_of_frame`** carries the same world-space fields, with no screen box, for
  every known target outside the frustum. It is what makes "the subject is absent
  entirely" machine-visible instead of something a reviewer has to notice.
- **Ordering** is `(kind, area, id)`; floats are rounded to 3 decimals.

**Determinism (ADR-0006)** — no RNG, clock, parallelism or hash-order iteration:
the voxel palette comes from a `BTreeMap` walk, targets are sorted, and the PNG
encoder (`compiler::png`) pins its DEFLATE level. Two runs on one input produce
byte-identical PNG **and** manifest; `crates/delvec/tests/snapshot.rs` asserts
both.

**Performance** (measured, `nobodys-cave-island`, release build, macOS/M-series,
single-threaded): assemble + voxel-grid flattening ≈ **30 ms**, a 960×540 frame
with `--labels` + manifest ≈ **190 ms**. `--timing` prints both to stderr (never
to the output, so it cannot affect byte-identity).

### `delvec blocking-chart`

```
delvec blocking-chart <campaign-dir> [-o <dir>] [--timing] [--json]
```

Per-elevation **cutaway** floor plans: one orthographic top-down PNG per
detected walkable band per area, plus `blocking-chart.json`. Default output
directory `blocking-chart/`. It answers the question a viewport structurally
cannot — *is there room* — so NPC crowding, a post blocking a doorway, or a
stealth zone lying across the only corridor are visible before the build exists.

**Cutaway, because there is no camera.** A roofed cavern cannot be photographed
from above, so the renderer simply excludes everything above the cut plane — a
dollhouse view straight from the voxel model. For a band whose walkable floor is
`Y`, each column of the area is drawn from the **topmost block in
`[Y-1, Y+3)`**: the floor a player stands on plus anything up to head height, so
a lintel and a waist-high obstacle both read, and no ceiling sneaks in.

**Bands are found, not declared.** Walkable cells (a BFS rooted at the area's
anchors, so a sealed void pocket never counts) are histogrammed by Y, and a band
is a local maximum of that histogram that

1. holds ≥ `BAND_MIN_CELLS` (6) cells, and
2. stands out from its neighbouring elevations by `BAND_RELIEF` (3×).

**Relief, not share** — this is the load-bearing choice. A share rule ("≥4% of
the area's walkable cells") makes a storey's status depend on how big the rest of
the *area* is, and the island's sheep pen — unambiguously a second floor — fails
it purely because the beach and meadow below are large. Relief asks the local
question instead: does walkable area *concentrate* here relative to what is
immediately above and below? A floor and a mezzanine do; a ramp, contributing one
or two transit cells per Y, never does. Maxima closer than `MIN_BAND_GAP` (3)
merge into the more populated one; at most `MAX_BANDS` (8) survive.

A **coverage pass** then guarantees the chart set is trustworthy: *every*
populated elevation must fall inside some band's cut. Relief finds storeys, but
rolling outdoor ground (the island's meadow climbing from beach to cave mouth)
has no storeys at all, and without this pass a walkable stretch would appear on
no chart at all with nothing to signal it. Uncovered elevations get fill-in
bands, lowest first, bypassing the merge rule — coverage outranks tidiness.

**Each slice is cropped** to its own band's walkable cells and markers (plus a
5-block margin), so a campaign whose single area runs from beach to mountain-top
gives the cavern its own tight frame at its own larger scale, instead of a small
drawing in a large field of void.

**Overlays**, in order: terrain flat-shaded by [`snapshot::block_color`] and
lightened with height within the cut (so a step reads as a step); a green wash on
the band's walkable cells; the DW0311-proven critical-path walk corridor as an
orange tint; then an outlined, labelled marker for every anchor, gate, NPC/actor
post, interact marker, stealth zone and trigger region whose elevation range
meets the cut. Labels use the same kind colours as `snapshot --labels` and the
same deterministic placer; a label that cannot be fitted on the plan is dropped
from the image but recorded in the index, never pushed off the edge. Routing is
best-effort — a campaign whose critical path does not route yet charts without
the corridor tint rather than refusing to chart.

Orientation is **+X right, +Z down (north up)**; each slice carries a title bar
naming its area, band index, floor Y and cut range.

**Index** (`blocking-chart.json`, `chart_version: 1`):

```json
{ "chart_version": 1, "campaign_id": "…", "delvec": "<x.y.z>",
  "orientation": "top-down orthographic; +X right, +Z down (north up)",
  "cut": "each slice draws world Y in [floor-1, floor+3) …",
  "areas": [ { "area": "area/island",
               "bounds": { "min": [x,y,z], "max": [x,y,z] },
               "walkable_cells": 1500,
               "bands": [ { "index": 0, "floor_y": 69, "walkable_cells": 420,
                            "y_range": [68, 71], "file": "island-band0-y69.png",
                            "width": 504, "height": 526,
                            "labelled": ["anchor/fire-pit", "…"] } ] } ] }
```

**Performance** (measured, `nobodys-cave-island`, release): **≈90 ms** for the
whole campaign's four slices, including the nav model and critical-path routing.

### `delvec cameras` — the showcase camera record

```
delvec cameras <build-dir> --campaign <campaign-dir> -o <dir> [--prefabs <dir>]
    [--only <name>]... [--bracket yaw=,pitch=,fov=,dolly=,truck=,rise=]
    [--draft | --preview]
```

`<campaign-dir>/design/cameras.json` is the one record of a campaign's showcase
cameras; `compiler::view::camera` is its one reader. At the top level, both
required: `campaign_id` — the campaign the record belongs to, held equal to the
build's, so a record cannot silently place cameras in another world — and
`cameras`. Per camera: `name`, `answers` (a `design.json` row), `pos` (the lens,
world blocks), `yaw`/`pitch` in the `--camera` convention above, vertical `fov`,
`exposure`, `width`, `height`, `source` (`estimated` or `hand`), `spp`, and two
optional fields: `sky` — an object of `time` and `weather`, both required
when the object is present, typed by the hour and weather enums `world.json` uses — the
sky the picture is taken under when it is not the sky of the row the camera
answers; and `after` — an object of `step`, required, and `path`, optional — the
step of an exported path the picture is taken after (spec-0089): `step` an
objective id (`obj/<id>`) or a trigger id (`trigger/<id>`), never an index, and
`path` a branch id `validation/branch-plan.json` declares, absent for the
critical path. **Every other field is required**: the reader denies unknown fields and
serde refuses a missing one, both as `DW0721`, naming the camera and the row it
answers. Keys are alphabetical, so the
record a tool writes is already canonical. This list is not hand-kept:
`crates/delvec/tests/hand_camera.rs` serialises the reader's own structs and
holds this paragraph to the field names that come out, in both directions. The record is not a stage document
and reaches neither the datapack nor the plan's shots; `delvec build` reads it
as a hashed input, proves every camera in it (`DW0724`, above) and holds it to
`design.json` in both directions — every camera answers a row (`DW0721`) and
every row is answered (`DW0900`) — so the only bytes of the build it moves are
`manifest.json`'s input hash, `render-plan.json`'s `camera_eye_proof.showcase`
and `validation/design-record.json`'s three camera keys. `delvec schema --stage
cameras` exports the record's JSON Schema (`camera::record_schema`), naming the
file it lives at under `x-delvewright-file`; it is not part of `--stage all`.

**The configuration a camera stands in** (spec-0089 §4). A camera with no
`after` stands at load: the critical path's configuration on arrival at its
first step, every world-load seal in place, no beat fired. A camera after step
`X` of path `P` (index `i`) stands in `P`'s configuration on arrival at `i + 1`
(`nav::configuration_at` — `World::region_state_at` over `P`'s own region
writes under `P`'s own ancestry; the critical path's is `Plan::gate_fired_before`,
a branch's `Plan::branch_gate_model`), the state the proofs route the leg leaving
`X` under; after the last step it is the end state. Its bytes are
`Configuration::blocks` — `RegionState::blocks_over`, the one derivation of a
configuration's block map, which `DW0891` reads too — laid over **the world as
shipped** (`view::beat::picture_base`): the assembly, every gate the placed
world authors shut written back with its anchor's block (the assembly clears a
gate's region; the datapack's setup holds it), the relight fixtures and then every trigger's `prop`
(`pressable::trigger_props`, the list `setup_finish` writes them from) in the
order `setup_finish` sets them (`assembled::shipped_blocks`, which the loop's
tiling reads too), and every gated trap's trigger removed whose gate is shut at load (a
`requires_flags`, or a `requires_state` term its datum's `initial` fails). An
unforced write is not laid. The path is the one the build's proofs read, with
the links the route proof takes spliced in (`nav::with_links_taken`), so a step
index counts `plan.critical_path` — the region model's step space, not
`critical-path.json`'s rows, which splice witness, rest and completion steps
into the export. The `after` rules are the record's, refused `DW0721` by
`delvec cameras`, `delvec place-camera` and `delvec build` alike through
`view::beat::stand`: a step no step of the path carries (the path's steps
listed), a step the path carries more than once (a candidate, not a match — the
steps carried once listed), a path the build does not declare (the declared
branches listed) or declares and no world plays, and a step after which the map
equals load byte for byte (the remedy is to remove the field or name the first
step after which a block moves that the path carries once; or the line says the
path moves none).

**The world the engine writes** (spec-0089 §5; `compiler::view::world`). One
world per distinct configuration a framed camera stands in, under
`<out>/worlds/<key>/` — `at-load`, or `after-<step>` (`/` as `-`) with
`-on-<branch slug>` for a branch — named in each scene by absolute path, since
Chunky resolves it against the rendering process's working directory. It holds
`level.dat` (gzip, mtime 0: `Data{DataVersion, LevelName, SpawnX/Y/Z,
version}`, the spawn the campaign's start cell) and `region/r.<x>.<z>.mca`,
nothing else: per chunk holding a cell, `DataVersion` (the pin's, 4671),
`xPos`, `zPos`, `yPos` (−4), `Status: minecraft:full`, and a section for each
section holding a cell — `Y` a byte, as the game writes it and the pinned core
reads it (an int `Y` loads as an empty chunk) — with `block_states{palette,
data}` and `biomes{palette, data}`, palettes in first-seen order over the
section's own index order, every state completed with the pinned defaults of
the properties it leaves out (what the game resolves them to on load), indices
packed at `max(4, ⌈log2 n⌉)` bits for blocks and `max(1, ⌈log2 n⌉)` for biomes,
never across a long, `data` absent for a one-entry palette. Biomes are the
build's biome map (`horizon::biome_map`) sampled at each 4-cell's centre. Every
region timestamp is zero, zlib at level 6, chunks in index order, sectors
contiguous; two writes of one map are byte-identical, and each world's sha-256
over its region files in name order is printed. No light arrays, heightmaps,
entities, block entities or ticks. **`--world` is gone**: a showcase frame has
one world source. The server save (`validation/world-save.sh`) is the
instrument that checks the writer (`DW0955`, `tools/ci/check-written-world.py`),
and stays the world source of `delvec scene` and `delvec panorama`.

**What a frame does not show, by name** (spec-0089 §6): entities (the pinned
core draws none of the kinds the engine summons, from any world); block-entity
content — a sign's text, a banner's pattern, a head's profile, a lectern's book,
counted on the binding line from the blocks of those kinds the world holds; a
`set-atmosphere` repaint by a beat (the biomes are the map at load, and the line
says `biomes: at load`); the configuration's clock (the frame's sky is the
picture's, spec-0079); a fluid's spread past the cells a `Flood` write lays; a
write only an unforced root lays; **a `collapse`** — a collapse reaches the
route proofs as `DW0445`'s settled debris, not as a region write, so no
configuration lays its debris or its `then_floor` and a frame after a collapse
beat shows the ceiling standing; and a gated trap's trigger re-armed by a later
beat (it is drawn as at load).

Emission writes one Chunky scene per camera (`<campaign>_camera_<name>.json`)
against a build's `render-plan.json`: the camera verbatim, rounded to six
decimals; the camera's sky (`scene::sky_of`, §4 above); the layout and its landform
in the chunk list and Y clip; the ocean plane on an ocean horizon; no review
emulation. `delvec panorama` emits through the same scene builder
(`camera::world_scene`), so a solved camera and a stated one are one scene shape,
and prints its solved camera as a record line. Refusals are `DW0721` (exit 2),
before any file: a record that does not parse or carries an unknown key, an
illegal name, two cameras of one name, an empty `answers` or one naming no row,
a non-positive exposure, a pitch outside −90..90, a field of view outside
(0, 180), a zero frame or sample target, an empty record, a record for another
campaign than the build, an `--only` name the record lacks, a `sky` with a
member missing or a keyword outside the enums, **a stated `sky` equal to the
answered row's** (a derivation typed where a judgement belongs; the remedy is to
remove the field), an `after` its rules refuse (above), and a plan whose `sky`
states no `weather`. A campaign that does not plan is refused with its own
build code (exit 3), because the world is assembled from it. **Scene emission is
also held to `DW0900`** (exit 2): a record leaving any approved image unanswered
emits no scene, `--only` included, because the record is what is judged and a
set with a hole in it is not a set. Every run prints how many approved images
have a camera and names the rest.

**Where each scene's sky comes from** (spec-0079 §5), one function, three callers:

| Scene | `time`, `weather` from |
|---|---|
| a showcase camera (`delvec cameras`) | the camera's `sky`, else the `design.json` row it `answers` |
| the whole-map panorama (`delvec panorama`) | `render-plan.json`'s `sky` |
| a review frame (`delvec scene`: POV, interior, seam, …) | `render-plan.json`'s `sky` |

A camera with no `sky` is a frame of its picture, so it takes its row's hour and
weather, not the world's initial ones — `DW0890` holds every row's sky to a sky
the party can be in. The record's sky rule (`camera::resolve_sky`) is asked by
`delvec cameras` (and `--preview`), `delvec place-camera` and `delvec build`
alike (`compiler::design::answered`, exit 3), one sentence wherever it is read.
**Binding line**, printed by every `delvec cameras` run, refused or not, beside
the `answers:` sentence: one line per camera this run frames — `sky: <name>
<time>+<weather> <derived from <row> | stated> class <high|low|below> — <the
renderer's own clear sky | overcast cell <class>×<weather>: skyLight …,
apparentSkyLight …, sun …, fog …>` — and `skies: D derived, S stated, over C
camera(s)[ (R refused)]; weathers emitted: {…}`, whose set names the non-clear
weathers (a clear scene emits no sky block). A record stating no sky prints `0
stated`; a rain delve rendering clear prints the empty set. Beside them, per
framed camera, `after: <name> at load` or `after: <name> after <step> (step <i>
of <n> on <the critical path | branch `<id>`'s path>): <c> cells moved from load,
<u> unforced write(s) not laid, <b> block entit(ies) omitted; biomes: at load;
clock: as the picture` — `<c>` the cells whose block differs from the load
world's (`Configuration::moved_from`); per written world, `world: <key> <chunks>
chunk(s), <cells> cell(s), sha256 <hash> -> <dir>`; and `configurations: <k>
written for <c> camera(s), <a> after a step, <l> at load`. A record with no
`after` prints `0 after a step` and one world, `at-load`.

`--bracket` appends, after each camera, the camera moved one field by one step
each way (`dolly` along the heading, `truck` to the frame's right, `rise` up),
skipping any candidate outside the legal ranges, and writes every emitted camera
to `candidates.json` in the record format; a moved candidate is `estimated`
whatever its camera was. `--draft` divides the frame by 4 and
caps samples at 128 under `<stem>_draft`. A candidate carries its camera's
`after`, so it stands in its camera's world. `--preview` writes no scene: it
assembles the world as `snapshot` does (it reads `--prefabs`) and rasterises each
camera, in the configuration it stands in, at half its frame as `<stem>_preview.png`, byte-identical to `snapshot
--camera` with the same numbers, and names each camera whose lens is inside or
within `LENS_CLEARANCE` (0.25 block) of a placed block, with a `lens:` binding
line — a report, since the grid counts every block as a full cube. The material
is the pinned jar's, off the same derivation the emitted scene is path-traced
with, so what the preview says a wall is made of is what the render will say; a
block the pin does not have is magenta and named on stderr (`unpainted:`, above).
It draws any
record its reader accepts and is never refused by `DW0900`: it is the instrument
that closes the hole, and it prints the count it is closing. Byte-deterministic (ADR-0006): the same record,
plan and options give the same scene, candidate and preview bytes.

### `delvec place-camera` — the record's one writer

```
delvec place-camera <campaign-dir> --name <row> [--answers <design.json row>]
    [--sky <time>,<weather>] [--after <step>[@<branch>]]
    (--report <camera-report.json> --slot <n> --fov <degrees>
     | --candidates <record-format file> --pick <camera>
     | --delete)
```

Writes one row of `design/cameras.json` (`camera::place` / `camera::delete`)
and nothing else. `--report` writes the pose `delvec harvest` read off slot `n`
— the eye as `pos`, the rotation as `yaw`/`pitch`, verbatim, since the stamp and
the record speak Minecraft's rotation — with the field of view the person
framed with, as `source: hand`; a new hand row takes `--answers`, a 1600×900
frame, 300 samples and exposure 1.0, and a replaced one keeps its frame and
exposure. `--candidates` writes the named camera of a record-format file
(`candidates.json`) as `source: estimated`. **An estimate over a `hand` row is
refused, naming the row**; only `--delete` frees the name. A row keeps its
`answers`: a different `--answers` for an existing row is refused (a camera
aimed at another picture is a new row). The written record is held to
`design.json` like `delvec cameras` holds it, and the same
`answers: K of N approved image(s) …` line is printed after the write, so the
count moves under the creator's hand. `--sky <time>,<weather>` (`noon,clear`, in
the two enums' keywords) writes the row's `sky`; without it a new row states
none, a replaced hand row keeps the sky it had, and `--candidates` copies the
candidate's verbatim. A `--sky` that does not parse, or that equals the answered
row's sky, is refused; a written row prints its `sky:` line. `--after
<step>[@<branch>]` writes the row's `after` (the step, and the branch whose path
it is on); without it a hand row keeps the step it had and `--candidates` copies
the candidate's verbatim — a pose stamped in the running game never carries one,
since the overlay cannot read the quest state. A row stating an `after` is held
to the campaign's planned path (the campaign is assembled from `--prefabs`), so
an `--after` that does not parse, or one the `after` rules refuse, writes
nothing; a written row prints its `after:` line. Refusals are `DW0721` (exit 2)
and write nothing.

### `delvec edit apply` / `delvec edit preview` (spec-0017)

The map editor's write half, closing the loop with the read half above: edit
verb → deterministic replay → snapshot. Both subcommands run full validation
(exit 1 on any error — unlike the view commands, an edit session must not
build on a broken campaign), `Plan::build`, the checked replay (§4 "The map
editor edit stage"), then render **one labelled snapshot + manifest per
batch** into `-o` (default `edit-shots/`): the camera frames the batch's
edited AABB over the final edited world, dollhouse-style, pulled into open
air like `--at` (so an interior edit is viewed from inside its room). File
names are the batch's kebab (`batch/dress-floor` → `dress-floor.png` +
`dress-floor.manifest.json`).

After the snapshots, both subcommands run the **entire build-tier proof set** —
the DW02xx reachability analysis and `emit::build` itself, output discarded. The
per-batch invariants are only a subset (they miss `DW0308` cutscene clipping,
`DW0327` stealth zones, `DW0342` trap completability, `DW0312` wave seating,
`move-npc`/`move-actor` routability and the exported-route/POV self-checks), and
persisting on a subset let `apply` write a script the very next `build` rejects.
There is one proof tier: what `apply` accepts, `build` accepts.

`--batch <file>` appends one candidate `EditBatch` object (the `delvec schema
--stage 7` shape's batch element) to the script in memory. `apply` persists
the augmented script to `world-edits.json` (canonical 2-space form, trailing
newline) **only after the replay AND the build-tier proofs are green** — a red
candidate exits with its diagnostic and writes nothing, so a session can never
leave a broken script behind. The write is tmp + rename, so a crash mid-write
cannot truncate the artifact of record. `preview` is byte-for-byte the same run
but never writes to the campaign directory. `apply` without `--batch` replays +
re-renders only. Exit codes: 0 green · 1 validation · 2/3 replay or build-proof
failure by the failing code's tier (same mapping as `build`).

### PNG writing

`compiler::png` is a hand-rolled 8-bit RGBA writer shared by two callers, for the
same reason `compiler::resourcepack`'s ZIP/SHA-1 is hand-rolled — byte-stability
must be a function of this repo:

- `encode_rgba_stored` (uncompressed) — the `delve:art` font atlas, whose bytes
  are hashed into a shipped resource pack.
- `encode_rgba` (DEFLATE at a pinned level, via the existing `flate2` dep) — the
  snapshot renders, which are megapixel review artifacts.

---

## 8. Cutscene rehearsal + shot calibration (spec-0019)

LLMs are bad at authoring camera positions as `anchor + offset` numbers — a shot
authored that way can point the wrong way. spec-0019 moves the judgement into the
running game: the creator adjusts a **proposal** live and harvests it once; the
DSL stays the artifact of record.

**What exists:** the shot proposal in data storage, the calibration verbs that
mutate it, the `dw.done` harvest, `delvec harvest`'s `rehearsal-report.json`,
`delvec calibrate`, and the `dw.free` toggle (spec-0069, below). **Not built:**
playback — the macro-function dolly, `dw.beat` / `dw.shot` replay, and the
compiler-derived state-restore inverses.

### The proposal (`dw:rehearsal` storage, creator overlay only)

`compiler::rehearsal` enumerates every rehearsable **beat** (an effect bundle
containing a `cutscene`, at any nesting depth) and every **shot** inside it, in
campaign declaration order, giving each a 1-based id and the **JSON pointer**
that names its `cutscene` **effect** in the `quests` stage document, plus its
0-based index within that effect. The pointer names the effect and not the shot
on purpose: the single-shot spelling (`{path, seconds}`) and its one-entry
`shots` equivalent are the same cutscene and must emit byte-identical output
(`v06_cutscene::single_shot_spellings_are_byte_identical`), so a shot's identity
cannot depend on which spelling was used. A patch applies at
`<pointer>/shots/<index>` under the multi-shot spelling and at `<pointer>`
itself under the single-shot one. `compiler::creator` bakes
that inventory into the overlay:

- `creator/rehearsal/defaults` writes the compiled values into
  `dw:rehearsal base` (immutable) and copies them to `dw:rehearsal shots` (the
  live proposal). It runs from `#minecraft:load` **guarded on
  `unless data storage dw:rehearsal shots`**, so a `/reload` does not discard a
  proposal the creator is midway through.
- A campaign with no cutscene emits **no rehearsal artifacts at all**, and no
  dead trigger is registered.

**Everything in the proposal is an integer block cell.** That is the DSL's own
granularity (a camera waypoint is `anchor + integer offset`, resolved by
`nav::anchor_offset_point` to `cell + 0.5`), so the write-back round trip is
lossless — the snap error is identically zero, not "small". It is also the only
NBT numeric type a function macro substitutes without a type suffix: a `double`
expands as `12.5d`, which is an unparseable argument to `say` and `tp`. Each
shot additionally carries `pstr`/`lstr`, the pre-formatted strings the harvest
stamp substitutes, maintained in lockstep with the numeric `path`/`look` by
every verb that writes them.

### Calibration verbs (trigger objectives, overlay only)

All take a **1-based** shot id (`-0` cannot express "reset shot 0"). All mutate
`dw:rehearsal` storage and nothing else — no datapack write, no world edit, no
campaign scoreboard — which is what lets adjust-and-replay cycle with no reload.

| Trigger | Effect |
|------|---------|
| `/trigger dw.mark set <s>` | Append the creator's **eye cell** as the next waypoint of shot `s`. The first mark after a (re)set *replaces* the compiled path (so "first call = start, second = end" reads true); later marks append. The eye cell is derived as `floor((Pos + eye height) × 1000 / 1000)` via scoreboard division, which floors correctly below `y=0`/`z=0` where plain `int 1` truncation would be off by one. |
| `/trigger dw.mark set -<s>` | Reset shot `s` to its compiled values (`base[s]`). |
| `/trigger dw.aim set <s>` | Set shot `s`'s `look_at` to the block the creator is looking at. A **bounded, one-shot** ray — `execute anchored eyes positioned ^ ^ ^0.25`, 256 steps ≈ 64 blocks, run on demand, never polled — whose hit cell is read back off a `marker` summoned and killed inside the same command chain (vanilla has no position→score primitive). |
| `/trigger dw.faster set <s>` / `dw.slower set <s>` | Scale `seconds` by ∓20 % with a floor of one whole second, clamped to 2..30. The one-second floor is why the step is `max(1, 20 %)`: plain integer scaling leaves a 2 s shot at its fixpoint forever. |
| `/trigger dw.done` | The single harvest — one `[DelveShot]` line per shot. |

The overlay also `say`-stamps the `[DelveShotRoster]` the first time each
player joins — `shots=<n>`, then one line `<id>=<pointer>#<index>` per shot, so no
line reaches the 256-character chat bound whatever the campaign's shot count —
mapping shot ids to their JSON pointers; without it the creator has no way to
know what `dw.mark set 3` addresses. `delvec harvest` reads the roster back
(`orchestrator::rehearsal::harvest_roster`) and says whether it is the roster
`layout.json` states, warning when the log came from another build.

**A `trigger` objective is armed by its score entry, so `scoreboard players
reset` disarms it.** Vanilla stores "this player may `/trigger` this objective"
as a lock flag on the score entry itself; deleting the entry deletes the
permission, and `scoreboard players enable` re-creates it at `0`. A tick that
both `enable`s an objective and `reset`s it therefore leaves it permanently
unusable: every `/trigger` answers *"You cannot trigger this objective yet"* to
the player and writes **nothing** to the server log — so no report, no PackTest
assertion and no amount of reading the emitted commands makes it visible: a per-tick hygiene clause clearing the no-op value
(`scores={dw.mark=0}`) matches the entry `enable` has just created, so every
adjust verb is silently refused. **A fired trigger is cleared inside its handler, never in the tick**;
the next tick's `enable` re-arms it. Pinned by
`rehearsal::the_tick_never_resets_a_trigger_it_arms`, which fails the build's
tests if any overlay function ever again arms and disarms the same objective.

### The harvest stamp

```text
[DelveShot] shot=<n> beat=<n> ptr=<json-pointer> idx=<n> seconds=<n> look_at=<x,y,z|none> path=<x,y,z;…>
```

`say`, not `tellraw` — the same channel and the same reason as `[DelveNote]` (whose objectives continue on `[DelveNoteQuests]` lines after the stamp when one chat message cannot hold them; the harvester appends them to the stamp before)
(spec-0006 §3): a system message to players never reaches the server stdout log
the harvester reads. `shot`/`beat`/`ptr`/`idx` are compile-time constants, so a
harvested proposal always knows which DSL node its patch belongs on; only the
live values are macro-substituted.

### `rehearsal-report.json` (`delvec harvest`)

The same harvest pass that writes `playtest-report.json` also parses
`[DelveShot]` lines into a versioned `rehearsal-report.json` **beside** it,
written only when the session actually stamped a proposal. Schema version
`0.1.0`; per shot: `shot`, `beat`, `pointer`, `shot_index`, `path`, `look_at`, `seconds`, the
stamp's `at` timestamp, and `stamps` (how many times that shot was stamped).
`dw.done` fired twice keeps the **last** reading — the creator's final word — so
a report can never silently mix an early and a late state of one loop.

### `delvec calibrate`

```
delvec calibrate <rehearsal-report.json> --layout <creator-datapack/layout.json> [-o shot-patch.json]
```

Snaps every proposal cell to the **nearest declared anchor** within
`SNAP_RADIUS` (16 blocks) and emits `anchor + integer offset` — a zero offset is
spelled as a bare `{"anchor": …}`, exactly as the DSL does. Ties break on anchor
id, so the converter is a pure function of its inputs (ADR-0006). The
resolved-anchor vocabulary comes from `creator-datapack/layout.json`, which
spec-0019 extended with an `anchors` array (`id`, `area`, `kind`, resolved
`pos`) and a `shots` roster; it lives there rather than in a new build output
because it is a creator-loop artifact and the shipped image never carries
`creator-datapack/`.

The patch is **never applied here**: nothing writes to a stage document from the
game. The agent applies it, reruns `delvec build`, and the normal proofs
(`DW0308` air corridors, `DW0347` angular budget) gate the result exactly as
they gate a hand-written shot.

### A showcase camera placed by hand (spec-0069)

Every overlay — a campaign with no cutscene included — registers two more
trigger objectives, armed by `creator/tick` under the never-reset rule above
(`rehearsal::the_tick_never_resets_a_trigger_it_arms` covers them):

| Trigger | Effect |
|------|---------|
| `/trigger dw.free` | Leave the body, and on the next fire come back to it. Leaving summons a `marker` tagged `dw_free_home` at the player's position and rotation, carrying their free-flight id (`dw.fid`), their `playerGameType` (`dw.fmode`) and whether this overlay force-loaded its chunk (`dw.fload`, set only when `forceload query` says nothing else forces it), then sets spectator. Returning restores the game mode, teleports to the marker (position and rotation), removes the force-load it added and kills the marker. `gamemode` and `forceload` run at the server's function permission level (default 2), so no op is needed. |
| `/trigger dw.cam set <n>` | Stamp slot `n` (`/trigger dw.cam` is slot 1). The eye is read off a `marker` summoned at `execute anchored eyes positioned ^ ^ ^` and killed in the same chain, in milli-blocks — the eye in any pose — and the rotation off the player's `Rotation` in centi-degrees, into `<ns>:camera` storage; `in` is `block` when the eye's cell is not `air`/`cave_air`/`void_air`. It refuses nothing. |

```text
[DelveCamera] slot=<n> eye=<x_mb>,<y_mb>,<z_mb> yaw=<centi-degrees> pitch=<centi-degrees> in=<air|block>
```

`delvec harvest` parses it into **`camera-report.json`** (`orchestrator::camera`,
schema `0.1.0`), written only when the log carries a stamp: per slot `slot`,
`eye` (blocks), `yaw`, `pitch` (degrees), `in`, the stamp's `at`, and `stamps`;
the last stamp of a slot wins. `delvec place-camera --report` writes a slot into
the record.

Every suite proves both handlers on a server (`creator::packtests`, beside the
suite's other templates; the PackTest compose service loads
`creator-datapack/`): `creator_camera_standing` (the stamp's storage holds the
eye within one milli-block of `Pos` + 1.62 and the rotation the dummy was
teleported to), `creator_camera_in_a_block` (a spectator whose eye is in a stone
block the template places: the same eye, `in` = `block`), and
`creator_free_returns` (spectator after the first fire; adventure, the same
position to the milli-block, the same yaw, and no place marker left after the
second). They stand at the corner of the campaign's first area at y 300 — a
column its setup force-loads — because PackTest places a batch millions of
blocks out, where a position in milli-blocks overflows a score. A sneaking eye
and the log line itself are proven live by `validation/rehearsal-flow.sh`.

---

## 9. `delvec fmt` — canonical form for authored JSON

**`delvec fmt` writes the number.** Every envelope it formats comes out declaring the one `dsl_version` this engine implements (ADR-0024), so adopting a campaign to a new surface is `delvec fmt` plus the edit the surface asks for, and `--check` reds a document that still declares the old one. Only the envelope's own top-level `dsl_version` is touched; a nested key of that name is content.

A file that is not canonically ordered turns a **three-key** insertion into a
**103-insertion / 100-deletion** diff the moment a writing tool's `sort_keys`
re-lays it out — measured on `nobodys-cave-island/l10n/zh-cn.json`. Canonical order makes
an insertion a one-line insertion, and makes two authors editing different keys a
non-conflict.

```
delvec fmt <path>…            # rewrite in canonical form
delvec fmt --check <path>…    # report; write nothing; exit 1 if anything is off
```

A formatter **and** a check, in that order and for a reason: a `--check`-only
gate makes an author hand-sort a 900-key sidecar, which nobody does twice, so the
gate ends up waived. `cargo fmt` is the shape that works.

### The hard constraint

**Only object keys may be sorted. Array order is semantic** — `quests[]`,
`objectives[]`, `effects[]`, `options[]`, `steps[]` are ordered, and reordering
one changes the game. So this is a correctness property, not a style property,
and it is *proved* rather than promised: `delvewright_dsl::fmt::format_text`
re-parses its own output and runs `fmt::equivalent`, which compares **arrays
index-wise** and objects as key→value maps. A renderer that sorted an array fails its own check
and writes nothing (`DW0772`). The guard is demonstrated firing — the unit test
`the_guard_catches_a_renderer_that_sorts_arrays` injects a deliberately
array-sorting renderer and asserts `DW0772`.

### The canonical form

| rule | why (argued from *minimal diff on insertion* / *no semantic change ever*) |
|---|---|
| object keys sorted by Unicode scalar value | an inserted key lands in exactly one place. UTF-8 byte order == code-point order, so Rust's `str` `Ord` and Python's `sorted()` agree and the existing Python authoring tools already emit this order. |
| 2-space indent, one value per line | the motivating file and every `tools/*.py` writer already use `indent=2` (also `serde_json`'s pretty default), so the one-time normalization is smallest exactly where the files are largest. One value per line makes an inserted array element a whole-line insertion, not a rewrite of a long line. |
| non-ASCII written raw, never `\uXXXX` | the campaigns are half Chinese; escaping would triple every sidecar and make its diffs unreadable — a direct defeat of the motivation. |
| control characters escaped in the shortest legal form (`\n`, `\t`, `\b`, `\f`, `\r`, else `\u00xx` lowercase) | required by JSON, and it is the form every other writer here already emits, so it is the fixed point. |
| **number literals preserved byte-for-byte** | the one rule that is not about diffs. Re-rendering through `f64` loses integers above 2^53 and can move a decimal's last digit — a silent semantic change. `9007199254740993`, `1.50`, `1e3` and `-0.0` all survive unchanged. |
| exactly one trailing newline | POSIX text, and without it appending anything rewrites the last line. |
| duplicate object keys refused (`DW0771`) | see the catalog row: the data loss is already happening silently; formatting would make it permanent. |
| empty containers stay on one line (`[]`, `{}`) | matches `serde_json` and `json.dumps`. |

Not canonicalized, deliberately: number literals (above), and Unicode
normalization of string contents (NFC vs NFD is the author's text, not the
formatter's). The formatter knows the **JSON grammar and nothing about the DSL** —
it must handle an l10n sidecar, a stage document, a map-pipeline document, a
prefab metadata card and any document added later, with no per-schema list to
keep in step.

### Which files, and how they are found

A path argument may be a file (taken as given — you pointed at it) or a
directory, walked recursively for `*.json` with entries **sorted**, never
`read_dir` order (ADR-0006). Two things are skipped:

- dot-directories (`.git`, `.github`);
- any directory holding a `manifest.json` — the marker `delvec build` itself
  stamps on an output root. Emitted trees are not authored content, and
  rewriting one would break the byte-identity record it exists to hold.

Symlinked directories are not followed (`campaigns/` is a symlink to the content
repo in a dev tree; a walk that followed it would silently reach a second
repository).

### The repository-wide sweep

Pointing the formatter at a path is what an author does to one document. What
this repository *gates* is the whole of what it holds, and that set is **derived,
never listed**: `tools/ci/check-json-canonical.py` takes its population from
`git ls-files -z -- '*.json'` and hands the whole of it to `fmt --check`. A
directory of authored JSON added next month is swept the moment it is committed,
with nothing to edit anywhere.

Using git rather than a walk is what makes the exclusions properties instead of
names. `target/`, `node_modules/` and every other build product are absent
because they are not tracked; `campaigns/` contributes one symlink entry rather
than a second repository's contents; `delvec build` output trees are absent for
the same reason `BUILD_OUTPUT_MARKER` skips them. None of those is a rule that
can go stale.

There is exactly **one** exemption, and it is a pointer rather than a judgement:
`crates/delvec/tests/golden/`, whose files are recordings of emitter output
asserted byte-for-byte by `golden_scene_matches`, with the directory's membership
closed by `every_golden_is_emitter_output`. Getting a document in there means
making the emitter actually emit its bytes — which is why it is not a hatch that
"it's generated" can open. The check refuses if that exemption matches zero
tracked files, or if either pin has been deleted.

Generated JSON is otherwise not exempt: it is made canonical **at the writer**.
`tools/ci/gallery-baseline.py` writes `gallery/baseline/` with `ensure_ascii=False`
for exactly that reason. Where a foreign program owns the file — npm's
`package.json` and `package-lock.json`, the agent harness's `.claude/settings.json`
— it is formatted anyway and re-formatted after that program rewrites it, the
same relationship `cargo fmt` has with hand-written Rust.

Every run states files swept **against the tracked-JSON population**, plus the
exempt count. A population of zero, a swept set of zero, and an exemption
matching zero files are all refusals rather than passes.

### What formatting does and does not change in a build

Proved end-to-end by `crates/delvec/tests/fmt.rs::formatting_a_campaign_changes_only_the_manifest_input_hashes`,
and measured on `nobodys-cave-island` in both languages: **every emitted file is
byte-identical** (609 EN / 594 ZH outputs). The single exception is stated rather
than smoothed — `manifest.json`'s `inputs` map is the sha256 of the **source**
bytes, i.e. provenance of exactly what the author checked in, so it *must* move
when the sources are rewritten. `manifest.json`'s `outputs` map, and every other
key, are unchanged. A formatter that left `inputs` alone would have broken the
provenance record instead of preserving it.

### One canonical form, one implementation

The formatter lives in `crates/dsl` (`delvewright_dsl::fmt`), not in the
compiler, because a canonical form belongs to the **format** rather than to
whichever writer needed it first — and `delvewright-dsl` is the crate whose
published description already is "the format the delvec compiler reads"
(ADR-0018 §4).

That placement is load-bearing, not tidiness. `delvewright_dsl::to_canonical_string`
is what **`delvec edit apply` writes `world-edits.json` with**. A second form
beside `fmt`'s would make the compiler write a file its own `fmt --check`
rejects, and an author running both in one loop could not satisfy both. So
`to_canonical_string` serializes with serde and puts the bytes through `fmt`:
one definition, two doors.

The fixture gate `crates/dsl/tests/roundtrip.rs` proves two things at once —
serde loses no field on a round trip, **and** the fixture on disk is in
`delvec fmt` canonical form.

### CI

`python3 tools/ci/check-json-canonical.py --delvec target/debug/delvec`, a step of
the `delvec binary (one build per run)` job, over the binary that job builds (a
step, not a job: every job name in `ci.yml` is a required status context). It runs `--check` over every JSON document git
tracks and states its binding count against that population on every run. A
creator runs the same one command on a fresh clone; `--delvec <path>` skips the
cargo build when a binary is already to hand.

**The content repo is not covered.** `campaigns/` is pinned by
`versions.toml [content].sha`, so a `--check` over it here could only go green
after a content-repo normalization merges and the pin moves — an ordering this
repo cannot perform. The content repo's own CI runs no `fmt --check`, so the
accident this tool exists to prevent is prevented for engine fixtures only.

---

## 10. The metrics standard (`delvec metrics`)

One machine-readable table of the numbers a level is built to — the engine's
own data, exported as JSON so a tool outside the engine reads the export and
never a copy. `dsl::metrics` is the module; `delvec metrics` is the door.

```
delvec metrics                # the table on stdout, the verdicts on stderr
delvec --json metrics         # the DW0813 notice as a JSON diagnostic object
delvec metrics --gym <dir>    # generate the metrics gym into <dir>
```

The values are deliberately **not** listed here. This page fixes the table's
shape and mechanism; the table itself is the authority for its numbers, and a
second copy of them in prose is the drift a single authority exists to prevent.
Run the tool.

### Two halves, because two kinds of number

**Player metrics** are facts of pinned Minecraft Java 1.21.11 — the collision
box, the eye height, the step rule's walk-up and jump bounds, the jump arc
against flat walking speed, fluid impassability, the fall-damage onset and the
unarmoured survivable fall. They are measured, never chosen, so they carry no
calibration flag: walking a level cannot make a player 0.7 blocks wide.

Two of them are worth naming because the obvious filing is wrong. The width and
clearance at which a body can **pass** (`passable.width`, `passable.clearance`)
are functions of the collision box and belong here, not among the standards —
no walk can change them. And the jump arc
(`jump.airborne`, `walk.ticks-per-block`) is the derivation behind the nav
model's elevation weight, which lived in a doc comment where nothing could read
it; it is data here, and the weight is **asserted** against it rather than
computed from it, because the weight is a tuned figure an owner playtest
settled and re-deriving it at run time would move every route in every campaign
the first time somebody edited a physics fact.

**Building metrics** are standards this project fixes: the datum convention,
the standard seam opening set, the stair pitch standards, storey heights and
the pacing coefficients. Every one carries `calibrated`, and every one is
`false` — the metrics gym has not been walked. The pacing coefficients
additionally carry **no threshold anywhere**: a threshold on a number this
uncertain would be defending nothing.

**The table states no size of a place.** A box's `extent` is the author's
declaration and the piece detailed into it may not exceed it (`DW0843`), so a
class range beside it would confirm nothing; there is no size-class ladder, no
way vocabulary and no footprint quantum. **Nor does it cap a designed drop**: a
cap on how far a declared fall may go is the author's to declare, on the site
plan's `max_drop`, and the physical ceiling — the unarmoured survivable fall —
holds every drop whether or not one is declared (`DW0831`).

### Provenance, per entry

Every entry says where its number came from, and the four values are not
interchangeable:

| Provenance | What it claims |
|---|---|
| `engine-constant` | The number **is** a constant `dsl::metrics` defines and the rest of the workspace imports. Nothing to drift from. |
| `vanilla-rule` | A stated rule of the pinned game that no engine constant held before the table, and that this repository has **not** measured on a running server. The note names the rule, so the claim is checkable. |
| `derived` | Computed from other entries, or taken from a convention the tree already carries; the note carries the arithmetic or names the source. |
| `provisional` | A seed for the gym's calibration walk. Chosen, not established. |

Provenance and `calibrated` are orthogonal, and the seam opening set is where
that shows: the three-by-three passage is `derived`, because `cave:socket` and
`tk:socket` are both that opening and the prefab library has been built against
it for as long as it has existed — and it is still uncalibrated, because what
the walk decides is whether the convention is right, not what it is.

### One authority, structurally

The player half is not a second table that agrees with the navigation model. It
**is** the model's constants: `compiler::nav` imports the step rule's bounds
from `dsl::metrics`, and `compiler::crosshair`, `compiler::render_plan`,
`compiler::view::viewer`, `compiler::creator`, `compiler::combat` and
`render::occupancy` import their body constants from it. Rust
enforces the property harder than a test could — a module cannot both `use` a
name and declare it — so "one definition, not two agreeing" is a compile error
rather than an assertion.

`crates/delvec/tests/metrics_standard.rs` proves the part that is left: that
the bounds the table **publishes** are the bounds the model **walks at**. It
builds its geometry from the exported figure and asks the model what it
reaches, so a step rule that stops honouring its own published number goes red.
What it cannot see is stated on the file: moving the table alone does not red
it, because the model imports the same constant, and a second method sharing
the first's calibration is not a second method.

### A provisional value cannot be consumed quietly

A building metric's number is reachable only through an accessor that takes the
run's read ledger. A verdict resting on an uncalibrated standard therefore
records that it did, and `DW0813` names exactly those entries — the obligation
is in the signature, not in a line of documentation.

### What every run states

Three things on stderr, each stated whether or not it found anything:

1. **What the table holds** — entries per half, and how many are unwalked.
2. **What the self-check bound to** — invariants evaluated, building entries in
   the table, entries read, and how many of those are provisional.
3. **`DW0813`**, when any verdict rested on a seed.

The self-check is the table checked against itself and against the player half:
every opening admits a standing body, every standard pitch presents a tread
inside the walk-up budget (so a "standard" pitch is *walked*, never jumped),
every storey leaves interior between its courses, and the route pace is under
the pure-walk one.

Two failures are **internal errors** (exit ≥10), not diagnostics. A table that
contradicts itself is a defect in `dsl::metrics` and not in anybody's campaign,
so there is no author to address a refusal to. And a self-check that bound to
nothing exits the same way, because a check that examined no entry is vacuous
rather than a pass.

### The gym — the campaign the table generates

`delvec metrics --gym <dir>` writes a complete site-plan campaign into `<dir>`:
nine stage documents, no authored geometry, built by the ordinary stage-5
derivation. It is what a walk calibrates the table on.

What it lays out is read out of the table rather than typed beside it — a
**spine** of bays chained by one seam per standard opening, in table order, so a
body walks through every doorway the table defines; and off the spine, two
climbs to the same rise of one low storey whose hosts differ only in the run
they afford, so the derivation picks the gentlest standard pitch for one and the
steepest for the other, and a designed fall of that storey with a stair back
out of it. The bays' footprints are the gym's own declaration, sized so each
opening fits its face and each host affords its pitch's run.

Every one of those choices reads the table through the accessor `DW0813` binds
to, which is what makes the coverage count above mean something: the generator
decides nothing a table entry already states, so *how much of the standard the
gym instantiates* and *how much of the standard the generator read* are the same
number. Deciding a host pair with a hard-coded ratio instead of the pitches'
declared runs is the kind of choice `DW0840` names.

The gym is **not committed to this repository**. A generated campaign is content
and the engine ships the generator, on the same footing as a prefab generator
whose `.nbt` library lives in the content repo. Regenerating it is the update
path: a walker's ruling edits the table entry and the bay that demonstrated it is
a different size the next time anyone runs the command.

### The version

`metrics_version` is bound to the table's exported bytes by a committed digest
(`crates/dsl/tests/metrics.rs`): change any number, note or calibration flag and
the test reds until the version moves with it. A consumer that pins a metrics
version is pinning values, and values that move under a fixed version are the
drift the pin was bought to prevent.

## 11. `delvec sculpt` — a prefab from a declared form (spec-0087)

`delvec sculpt <form.json> -o <dir> [--seed N] [--id <prefab-id>]` (`crates/delvec/src/sculpt/`) is the second prefab back end beside the grammar: a form states a body as implicit solids over its own ground, and the command writes the ordinary prefab — structure parts plus a metadata document — so nothing downstream of admission changes. `delvec schema --stage sculpt-form` exports the form (`x-delvewright-file: forms/*.json`); it is a library asset, absent from `--stage all`.

**Two frames.** Anchors are in the piece's frame and are written to the metadata as declared. Solids and lights are in the body's frame: `piece y = body y + ground.top + 1 − sink`; `x` and `z` agree.

**The form document's fields**

| Field | Where | What it is |
|---|---|---|
| `form_version` | top | `1.0.0`; any other is refused |
| `id` | top | `prefab/<kebab>`; `--id` overrides the output id, never the provenance |
| `box` | top | extent `[x, y, z]` in blocks |
| `sub` | top | sub-voxels per block per axis, 2 or 4 |
| `ground` | top | `{block, top}`: the apron, full blocks at piece `y ∈ [0, top]` wherever the fit leaves air; required |
| `block` | ground, light | a block state |
| `top` | ground | the apron's top course |
| `sink` | top | courses of the body below the apron's top |
| `noise` | top | `{amplitude, cell}`: weathering on `noisy` solids |
| `amplitude` | noise | blocks at one standard deviation |
| `cell` | noise | lattice spacing in blocks |
| `palette` | top | four tones, bleached to weathered |
| `full` | tone | `[[block, weight], …]` full blocks |
| `family` | tone | the stem of `<family>_stairs` and `<family>_slab` |
| `solids` | top | the body, stamped in order |
| `shape` | solid | `capsule`, `ellipsoid`, `disc`, `box` or `shelf` |
| `op` | solid | `add` or `cut` (not on a shelf) |
| `noisy` | solid | weathered by `noise` |
| `from` | capsule, box | one end or corner; also a region's or `within`'s low corner |
| `to` | capsule, box | the other; also a region's or `within`'s high corner |
| `radius_from` | capsule | radius at `from` |
| `radius_to` | capsule | radius at `to` |
| `stretch_y` | capsule | vertical stretch of the cross-section |
| `centre` | ellipsoid, disc | the centre |
| `radii` | ellipsoid | the three semi-axes |
| `axis` | disc | the cylinder's axis |
| `radius` | disc | its radius |
| `height` | disc | its full height |
| `path` | shelf | knots `[x, y_feet, z]` |
| `width` | shelf | the walkway's width |
| `clearance` | shelf | clear headroom over the feet surface |
| `depth` | shelf | solid under the feet surface |
| `material` | solid | the solid's own tone: a block takes the tone of the last solid with a material that reaches into it — its centre within half a block's diagonal of the shape, so the stairs and slabs on that shape's surface take it too (a shelf's tread as before); the palette when absent |
| `lights` | top | light placed where the room is designed: `{at, block}` by hand, or `{block, hull}` set into the inside surface by the sculpt; exactly one of `at` and `hull` |
| `at` | light | the cell, body frame |
| `hull` | light | `{within, on, spacing, mode, cover?}`: sources drawn over the inside surface (§11, hull light) |
| `within` | hull | `{from, to}`: the cells, body frame, inclusive |
| `on` | hull | the surfaces a source may sit in: `wall`, `vault`, `floor` |
| `spacing` | hull | the least distance in blocks between two sources, at least 2 |
| `mode` | hull | `embedded` (flush in the surface; a full cube) or `recessed` (behind a `cover`, with a slot) |
| `cover` | hull | a bare `<family>_stairs` or `<family>_slab` id a recessed source sits behind |
| `anchors` | top | prefab metadata's anchor shape: at least one `role: entry` |
| `pos` | anchor | the cell, piece frame |
| `facing` | anchor | cardinal |
| `role` | anchor | `entry` or `furniture` |
| `region` | anchor | `{from, to}` inclusive cell range |

**What the form arm refuses** is `DW0951` (§5). Nothing is fitted until the form passes.

**The pass order.** (1) stamp the solids into a `box × sub` grid, noise from the crate's seeded generator (`grammar::rng`); (2) octant fit — air, full, bottom or top slab, or a straight stair of any facing and half, by least occupancy error with a cost bias of 0.6 (stair) and 0.4 (slab); inside a shelf's tread a stair may only face the way the shelf climbs (`DW0430`'s rule, by construction); (3) thin-plate refit from the solid thickened by one sub-voxel, where the fit says air; (4) the apron; (5) islands under four blocks dropped and counted; (6) `lights[]` placed by hand (`at`), then each `hull` entry in order (below); (7) stair corners by `schem::stairs::derive_shape`; (8) tone from the smoothed normal, openness and seeded noise, block by tone, axis by longest local run, every state completed from the registry's defaults (never a shape-carrying property). Smoothing is a three-pass box-filter cascade, so no transcendental function touches the bytes.

**Hull light** (spec-0087 §9; `sculpt::hull`). A `hull` entry's candidates are the fitted body blocks (full, stair or slab) in `within` whose face meets air with the body over it — `vault` if the air is below, else `wall` if beside, else `floor` if above; the ground is never a host — kept on the surfaces `on` names, and for `recessed` only where the recess fits: cover and slot blocks are body surface, the room cells in front of both are air, and every block round the source and the cell behind the slot is solid. A wall's slot opens sideways first (its mouth at the source's height), then up; a vault's along the first cardinal that fits. The candidates are shuffled by the crate's seeded generator and drawn as a maximal Poisson-disk set: a candidate is accepted when no source of its entry lies within its `spacing`, nor an earlier entry's within the smaller of the two spacings. `embedded` replaces the surface block with the source; `recessed` puts the source one block behind it (`H − d`), the oriented cover in it (`H`; a stair with its full-height half toward the room, or for a vault its raised quarter away from the slot; a bottom slab), and opens the two blocks beside them along the slot (`H + u`, `H − d + u`). The finished piece is flooded by `compiler::light::LightModel` (block light only) and each source's room cell read; the run prints `lights[i] hull <mode>: N source(s) — w wall, v vault, f floor — of C candidate cell(s) (…), spacing s; room light lo..hi`.

**What it checks before writing.** The grammar's always-on gates through the grammar's own functions (`blocks-exist`, `shape-complete`, `states-complete`, `stair-shape`, `fluid-contained`, `non-empty`), sealed by `gates::seal_zero_bindings`; then `DW0952` (§5): anchors stand, the entry is in the walk from grade, and no pocket. Every run prints the gate lines with binding counts, the fit counts, the entry walk (`… reaches N, and k of n standing anchor(s)`), the pocket line, `walk_y` and `shown_faces`.

**What it writes.** Through `grammar::export::freeze_model` — the grammar export's palette, unknown-state refusal and 48-block tiling, over `convert::build_region`; one `<id>.nbt` when every axis is at most 48, else parts under a `structure_set` manifest. No schematic is constructed, so `DW0710` is not on the path. Metadata: `anchors` as declared; `lighting` **measured** by the piece light probe over the bytes (`unmeasured` only when it binds nothing — the grammar export's rule, which is what keeps the piece showable under `DW0894`); `walk_y` the lowest standable local `y`; `shown_faces` every side the bytes put a block on; `license` `original`, `GPL-3.0-or-later`, `generated_by {generator: "sculpt", program: <form id>, program_hash: sha256 over the form's canonical JSON, seed, region}`. Same form, seed and engine: byte-identical files.
