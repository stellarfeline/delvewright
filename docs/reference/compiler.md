# delvec compiler — behavior reference

**Single authoritative record of *current* compiler behavior.** Specs
(`docs/specs/`) remain the historical decision records; this file is what the
compiler does today. A PR that changes compiler behavior updates this file in the
same PR (CLAUDE.md Methodology; CI enforces the DW-code subset — see
`tools/check-dw-codes.py`).

- Binary: `delvec` (`crates/compiler`, Rust-native, ADR-0011). The other binaries
  and scripts around it — `delve-schem`, `delve-admit`, `delve-render`,
  `delve-harvest`, `tools/`, `validation/` — are indexed in
  [`tools.md`](tools.md).
- Versions (as of this doc): `delvec 1.1.0`, `dsl 0.19.0`, `mc 1.21.11`.
  Supported campaign `dsl_version`: **`0.2.0`, `0.3.0`, `0.4.0`, `0.5.0`, `0.6.0`,
  `0.7.0`, `0.8.0`, `0.9.0`, `0.10.0`, `0.11.0`, `0.12.0`, `0.13.0`, `0.14.0`, `0.15.0`, `0.16.0`, `0.17.0`, `0.18.0`, `0.19.0`** (additive supersets; `0.2.0` output stays
  byte-identical across the later versions). This line is not prose: it is bound
  by equality to `crates/compiler/Cargo.toml`, `crates/dsl/src/envelope.rs`
  (`SUPPORTED_DSL_VERSION` + `SUPPORTED_DSL_VERSIONS` minus
  `RESERVED_DSL_VERSIONS`) and `versions.toml` by
  `tools/check-reference-versions.py`, in both directions.
- A `dsl_version` may be **reserved**: in the ledger, held for a surface a
  sibling change will land, and refused — `is_supported_version` says no and a
  stage document declaring it is `DW0102`, naming the versions the build
  accepts. Reserving is how a number an approved spec has allocated stops being
  free, since a skipped number cannot be filled afterwards (the ledger is
  append-only) and an unheld number is one a second change takes. **The list is
  empty today**, which does not retire the mechanism: a row belongs there for a
  surface whose change is *in flight*, because a row in this tree is the only
  claim a row can make. A number held for a surface with no scheduled work is a
  different thing — it stands in front of the live line on behalf of a road that
  may not be taken, and it made the ledger's own invariant (a reservation sits
  above everything implemented) unsatisfiable for the change that came next. That
  premise was removed rather than the invariant weakened, and a surface that lost
  its number takes a fresh one when it lands. The protection a standing row
  approximates lives where it can see the competing claim: the allocation scan
  over **every remote ref**, run before a round is dispatched.
  `tools/check-version-ledger-uniqueness.py` is that instrument's automatic half
  and diffs against `origin/main` alone, which is exactly why the scan exists; it
  also refuses a number a branch adds without a hand-written name — `is_v13` is
  computed from `0.13.0` and so cannot disagree with a second branch's claim on
  it, while `LAYOUT_GRAPH_SINCE` can.

---

## 1. Pipeline overview

### Pass order (`delvec build`)

| # | Pass | Crate/module | Fails with |
|---|------|--------------|-----------|
| 1 | Load campaign dir (6 required stage docs + the 5 optional documents + `walk-record.json` + `l10n/` sidecars) | `compiler::load` | internal (≥10), **naming the document that could not be read** |
| 2 | Parse (serde, `deny_unknown_fields`) | `dsl::parse_campaign` | `DW0100` (exit 1) |
| 3 | Validate stages 1–7 (schema + referential, full injected registries) | `dsl::validate_campaign_with` | `DW01xx` (exit 1); also `DW0455`, a body-family code refused at declaration time |
| 4 | l10n sidecar coverage + reserved channels + language-code mapping | `dsl::validate_l10n`, `dsl::validate_marker_channel`, `dsl::validate_tr_sigil`, `dsl::declared_mc_codes` | `DW0180`/`DW0181`/`DW0182`/`DW0183`/`DW0184` (exit 1) |
| 5 | Analyze (branch-coherent quest/dialogue reachability + critical-path replay) | `compiler::analyze` over `compiler::flow` | `DW02xx` (exit 2) |
| 6 | Solve jigsaw layout (per `prefab_pool` area, from seed); then read the settled draw back and report a pool that seats the same anchor-bearing prefab twice (`DW0498`, `compiler::pool`) | `compiler::solver`, `compiler::pool` | `DW030x` (exit 3); advisory `DW0498` |
| 7 | Assemble world model (placed pieces → voxel grid; ocean sea-level datum check) | `compiler::plan` | `DW030x`/`DW0344` (exit 3) |
| 8 | Replay the stage-7 edit script over the assembled model (spec-0017; per-batch invariant re-proofs — trap-hardware integrity, gravity, relight, walkability, fluid containment against the horizon, sea seepage into the built volume, boundary safety, block support; plus the advisory gate-region check). Skipped entirely for a campaign without one (byte-identical). | `compiler::edit` | `DW0318`/`DW0322`/`DW0323`/`DW0352`/`DW0354`/`DW0851` + reused invariant codes, batch-attributed (tier per code); advisory `DW0353`/`DW0354` |
| 9 | Assembled-light + relight (measure, place fixtures; over the **edited** model when a script exists) | `compiler::light` | `DW0210`/`DW0211` (**exit 2**) |
| 10 | Nav checks (**the ambient sea inside the built volume** (`DW0851`) and **boundary safety over the finished world** (`DW0322`, error tier) for every campaign that assembles one — the floor under the per-batch stage-8 call, which an edit-free campaign never reached; then A* `move-npc`/`move-actor` (footprint-aware, each walk routed over its **own timeline's** gate state), cutscene clip (authored polyline + rendered keyframe chords) + angular budget, critical-path walkability — incl. relight fixtures + water flood, and **per reachable branch** over each branch's own path under its own gate-seal step space; talk-to endpoint snap; waypoint self-check (critical path + per branch); v0.6 checkpoint no-stranding/placement + stealth-zone/onset + trap completability proofs; spec-0016 §6 TD lane polylines; spec-0016 §1 bonfire safe zone) — all over the **edited** model when a script exists | `compiler::nav` + `compiler::timeline` | `DW0307`/`DW0308`/`DW0311`/`DW0314`/`DW0315`/`DW0316`/`DW0318`/`DW0322`/`DW0325`/`DW0327`/`DW0342`/`DW0347`/`DW0355`/`DW0386`/`DW0410`/`DW0430`/`DW0478`/`DW0488`/`DW0851` (exit 3; `DW0342` → exit 2) |
| 11 | Referential + placement seals inside emission: every anchor-bearing effect resolves (`DW0360`), no generated name collides (`DW0361`), no body eclipses an interaction affordance (`DW0359`, `compiler::eclipse`), no body occupies block geometry at its anchor or on any walked leg (`DW0450`/`DW0451`, `compiler::clearance`), no walked leg contains a move its own body cannot make and no body's `traversal` declaration goes unexercised (`DW0452`/`DW0453`/`DW0454`, `compiler::traversal`), no two bodies the party clicks contest one crosshair in a scene the cast ledger declares (`DW0489`, `compiler::crosshair`), no daylight-burning body is staged for a fight whose walkable ground reaches open sky under a pinned daytime hour (`DW0496`, `compiler::daylight`, measured off the seated wave cells) | `compiler::emit` | `DW0359`/`DW0360`/`DW0361`/`DW0450`/`DW0452`/`DW0489`/`DW0496` (exit 3); advisory `DW0359`/`DW0451`/`DW0453`/`DW0489` |
| 12 | Emit (datapack incl. the `world_edits` function, packtest, server, critical-path, resourcepack, and the visual tier's `render-plan.json` — whose every camera is stood up in open air and then proven clear-eyed against the assembled world, `DW0724`) | `compiler::emit`, `compiler::render_plan` | `DW0300`+/`DW0724` (exit 3) |
| 13 | Emission self-checks over the **finished tree**: every affordance is visible and only its owner retires it (`DW0420`/`DW0421`), no engine fixture is reachable by a box-narrowed selector (`DW0545`), the call graph is closed — no `function <ns>:<name>` points at a function that was never emitted (`DW0497`) — and the score reads are closed: no `if score` / `unless score` / `scores={…}` reads a scoreboard entry the pack never creates (`DW0495`) | `compiler::affordance` + `compiler::integrity` + `compiler::seeding` | `DW0420`/`DW0421`/`DW0495`/`DW0497`/`DW0545` (exit 3) |

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
  AABB, each cell is one byte holding whether light passes it, whether the sky is
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
  Brigadier tree (`compiler::commands`, `data/commands-1.21.11.json`;
  structure-only — arity/paths, not arg values). mecha re-validates in CI
  (ADR-0011); disagreement fails CI. The match is exact and complete: the first
  token must be a known command root, `literal` nodes match verbatim, and
  `argument` nodes consume a fixed per-parser token count (`vec3`/`block_pos` 3,
  `vec2`/`column_pos`/`rotation` 2, `message` and greedy `string` the rest, else
  one balanced token). Tokenizing is brace/bracket/quote-aware, so an NBT
  compound, a block-state suffix and a selector are each one token. Matching
  **backtracks** across ambiguous argument branches and follows `redirect`s
  (`… matches N` → `execute`, `run <cmd>` → the tree root); a line is valid iff
  every token is consumed ending on an `executable` node. What it therefore
  catches is a misspelled command, a wrong argument count and a bogus subcommand
  path; what it does not judge is numeric coordinates, well-formed NBT/JSON, or
  whether a block or item id exists — those are mecha's cross-check and the DSL
  item/block registries.
  **Single-entity arity** (round-7 live finding, spec-0018): an entity argument
  the tree marks `amount: "single"` rejects `@a`/`@e` without `limit=1`.
  `damage @a[…] 40 minecraft:generic` is a well-shaped command that 1.21.11
  refuses to *load* ("Only one entity is allowed…") — taking the whole enclosing
  function down with it, silently. The tree already carries the fact, so the
  compiler enforces it rather than leaving it to folklore; the party form of
  `damage-players` is `execute as @a[…] run damage @s …`.
  **One value-level exception**: an SNBT integer literal in a
  `key:value` position whose suffix cannot hold it — NBT bytes and shorts are
  signed, so `text_opacity:255b` is structurally flawless and unparseable, and
  1.21.11 answers "Failed to parse number: Value out of range" by dropping the
  entire function. Quoted spans are skipped, and a bare standalone number is not
  examined, so it cannot mistake prose for a value. `delve-admit`'s gallery is the
  second consumer of this validator: it emits `.mcfunction` into a datapack
  exactly as `delvec` does, so it now runs the same tree over its own output
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
delvec schema   --stage <1..7|all>         # export JSON Schema
delvec metrics                             # export the metrics standard as JSON (§10)
delvec l10n-inventory <dir> [--lang <c>]   # l10n key inventory as JSON (translation input)
delvec snapshot <dir> [framing] [-o f.png] # draft frame + scene manifest (§7)
delvec blocking-chart <dir> [-o dir]       # per-elevation cutaway floor plans (§7)
delvec edit apply   <dir> [--batch f] [-o dir]  # replay edit script (+ candidate), persist on green (§7)
delvec edit preview <dir> [--batch f] [-o dir]  # same replay + renders, never persists
delvec calibrate <report> --layout <layout.json> [-o f.json]
                                           # harvested shot proposals -> anchor+offset DSL patch (§8)
delvec --version                           # "delvec 0.1.0, dsl 0.6.0, mc 1.21.11"
```

Global flags: `--json` (one JSON diagnostic object per line), `--prefabs <dir>`
(default `campaigns/prefabs`), `--lang <code>` (default `en`; affects `build`
only — `validate`/`analyze` are language-independent apart from coverage).

**Exit codes**: `0` ok · `1` validation failure · `2` analysis failure · `3`
build failure · `≥10` internal error. Undeclared `--lang` is a validation-class
rejection (exit 1). Codes are stable API; the CI fixture matrix asserts them.

**A failure that stops a build exits at its CODE's tier.** Which tier a rule
fails at is a property of the rule, so every `DwCode` declares it
(`dsl::diagnostic::ExitTier`) beside the version at which it starts binding, and
there is no constructor that leaves it unsaid. `Analysis` means the compiler did
its job and the CONTENT is the defect, and exits 2. `Build` means the compiler
could not produce a tree it will stand behind, and exits 3; it is also what a
rule reported as an ordinary validation diagnostic declares, because such a rule
refusing with a build under way is a build failure. The analysis-tier codes are
exactly these, and the second column says where the author's fix goes —
`tools/check-dw-codes.py` holds this table and the source declarations in
lockstep, in both directions:

| Code | What the author changes |
| --- | --- |
| `DW0201` | the quest graph — the finale is unreachable |
| `DW0202` | the quest graph — nothing that completes triggers the quest |
| `DW0203` | the objective / dialogue graph — the objective completes in no branch |
| `DW0204` | the beats on the critical path — the exported path is not one a player can walk |
| `DW0205` | the objective's dependency edge — an optional button already gates the mainline |
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
at exit 10, naming the path or document. The four verbs that read a campaign
directory — `validate` and everything built on it, `l10n-inventory`, `edit`,
`allocation` — answer both the same way, from one place.

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
and `DW0498`, where a pool repeating an anchored piece
is a legal shape shipping content relies on).

---

## 2. DSL surface (per stage)

Envelope (every stage): `{ dsl_version, campaign_id, stage, content }`,
`deny_unknown_fields`. IDs are type-prefixed kebab-case; all cross-stage refs are
**strictly backward**. Source of truth: `crates/dsl/src/stages.rs` (schemas
exported via `delvec schema`). Introduced-by column cites the spec.

### Stage 1 — `world`

| Field | Behavior | Since |
|-------|----------|-------|
| `title` | Player-visible; l10n key `world.title`. | 0.1 |
| `outro` (opt) | The closing line on the campaign-completion advancement — the last player-visible sentence of the delve. Player-visible, so it is l10n-inventoried as `world.outro` and sidecars translate it. Absent = the **finale quest's `goal`** (already campaign-derived and inventoried as `quest.<q>.goal`), so the line is never hardcoded English either way. Before this the description was the literal `"You left the keep."` on every delve ever built — the reference keep-crawl's line, shipped to campaigns with no keep and unaddressable by any sidecar key, because it never passed through a `Campaign` field. | 0.6 |
| `theme`, `premise` | Authoring context; **excluded** from l10n. | 0.1 |
| `seed` (u64) | Sole downstream randomness (layout PRNG). | 0.1 |
| `target_minutes` | Informational (pacing). | 0.1 |
| `languages[]` (opt) | BCP-47 codes; `en` implicit/never listed; drives l10n coverage + `--lang`. | 0.3 i18n |
| `areas[]` | 1..N. Each binds **exactly one** of `prefab` or `prefab_pool`+`pieces{min,max}` (else `DW0160`). Area origin = `[i·256, base_y, 0]`, where `base_y` is the **horizon datum**: `void` → 64, `ocean` → 60 (see `horizon`). Either way the placed pieces go through the same socket seal (`solver::seal_layout`, see *Assembled-world model*): a single-prefab area places one piece, so **every connector it declares is unmated and is walled**. A prefab with no connector yields no seal. | 0.1 / pool 0.2 |
| `areas[].lighting {fixture,min_light}` (opt) | spec-0010: relight pass guarantees `min_light` (1..=14, default 7; `DW0196` out of range) over reachable walkable cells by placing `fixture` (`torch`/`lantern`/`campfire`/`shroomlight`), else `DW0211`. | 0.5 |
| `areas[].mitigation` (opt) | `night-vision` — the first-class darkness declaration (v0.6). The compiler emits a self-rescheduling **1 s (20t)** `night_vision_tick` that runs `effect give @a[<this area's placed bounds>] minecraft:night_vision <lease> 0 true` (amplifier 0, particles hidden). The lease is `max(12, longest camera + 10 + 1)` seconds — **the camera-coverage guarantee**: a granted vision effect must outlast any authored camera it can overlap, plus vanilla's 10 s wind-down, so it can never begin ramping down on screen. 12 s is the floor and is what a campaign with no cutscene still emits (byte-identical to pre-0.6.1). The longest camera is measured from the ticks `camera::shot_ticks` really emits, and the campaign-wide max is used because the compiler cannot know which cutscene a player who steps out of a mitigated area will land in — the island's ending transports the party from the mitigated island to `area/open-sea` and immediately plays a 15 s camera, which the old 12 s lease could not survive. A player who leaves the area keeps sight for ≤ the lease: deliberate, since no vanilla primitive strips one effect on region exit without stripping effects the story granted, and the alternative is a visible flicker. Independent of `lighting`. This declaration is the **sole** `DW0210` night-vision mitigation. | 0.6 |
| `time` (opt) | `day`/`noon`/`dusk`/`night`/`midnight`/`dawn` (default `noon`; `sunrise` is accepted as a synonym of `dawn`). Dimension-global initial state, emitted in the sealing baseline. Vanilla's `/time set` takes either a keyword or a raw tick count, so the DSL is not limited to the four keywords: the four vanilla states emit their **keyword verbatim** (`time set night`) and the other two emit the equivalent **tick form** (`dusk` -> `time set 12000`, `dawn` -> `time set 23000`). One table maps every state to its argument and its `time query daytime` read-back (`WorldTime::spec`), so the sealed-state PackTest asserts the right value and no shipped campaign's bytes move. `dusk` is the **sunset onset** (12000, the sky visibly going orange), deliberately not 13000 — 13000 is the sun already down, which is exactly what the `night` keyword sets, so it would make `dusk` a synonym rather than its own beat; `dawn` (23000) is the sunrise onset. `dusk`/`dawn` count as night for the sky-light model (`DW0210`), which is the conservative direction (both skies are in fact brighter than midnight). Same enum for the `set-time` effect. | 0.5 / dusk+dawn 0.5 |
| `weather` (opt) | `clear`/`rain`/`thunder` (default `clear`; `clear` emits nothing — byte-identical to pre-0.5). Dimension-global, emitted after sealing (`weather <kw>`). Rain/thunder attenuate the assembled-light sky term. | 0.5 |
| `difficulty` (opt) | The delve's combat difficulty: `easy` / `normal` / `hard`. Absent = the compiler's historical **derivation** — `easy` when the campaign fields any wave, `peaceful` when it fields none — which is what keeps every pre-0.6 campaign byte-identical. Declaring it overrides the derivation in BOTH places a difficulty comes from: `server/server.properties` (what the shipped image and every compose profile boot from, via `validation/world-settings-entrypoint.sh`) and a `/difficulty <kw>` appended to the sealing baseline, so the declaration also holds when the datapack alone is dropped into another world. A declaring campaign also emits the `declared_difficulty` PackTest, which asserts the live world's difficulty via the bare `/difficulty` query command (vanilla returns `Difficulty#getId()`: peaceful 0, easy 1, normal 2, hard 3) — so properties, sealing and declaration are proven to agree on a real server. `peaceful` is refused (`DW0468`); fighting actors with no waves and no declaration is the advisory `DW0469`. **Retuning warning:** every combat number in every campaign written before this field was tuned under the implicit `easy`, which HALVES incoming player damage (`min(dmg / 2 + 1, dmg)`) — content that declares `normal` or `hard` must redo that arithmetic, not merely flip the keyword. | 0.6 |
| `horizon` (opt) | The ground and the sky the map stands in. Either a **string shorthand** — `void` (default/absent) or `ocean`, the pair that predates the horizon library and stays writable at 0.6 with byte-identical output — or the **object form** `{base, …params}` (0.16, `DW0141` below it). `void`: nothing outside the placed geometry. `ocean`: a pinned bedrock/stone/water superflat, sea level y=62, no structures or mobs; it drives `generator-settings` **and the area-origin datum**, placing ocean areas at y=60 = `sea_level − 2` so an island piece's authored waterline (local y=2) meets the world ocean and its walk plane is the vanilla-normal one block above the sea (`DW0344`). `valley`: the one base that BUILDS terrain — see *The horizon's surround* below. Params: `ratio` (2.0..=3.0, default 2.5) and `rim_height` (16..=128, default 48), both `valley`-only, both `DW0853` out of range or beside another base. Any horizon whose ambient a body can ENTER — the sea, and a valley's gap floor — needs a `boundary` (`DW0320`); `void` is the only one it cannot, because there is nothing out there to stand on. | 0.6 / library 0.16 |
| `min_players` (opt, 1..=4) | spec-0018: the party size the delve **requires**. Absent = 1 (a party of one is always legal; every pre-0.6 campaign reads as 1). `>= 2` emits the **lobby gate**: `tick` recomputes the live count into `#lobby dw.sys`, the class-selection dialog driver is prefixed `if score #lobby dw.sys matches <n>..` (so the delve cannot START short-handed), and unclassed players get a self-updating `x / n` actionbar (`{"score":{"name":"#lobby","objective":"dw.sys"}}` — one emitted line, no per-count strings; a compiler default, not an l10n key). Out of range = `DW0356`; a mandatory-n declaration with no n-way division of labour = `DW0358`. `min_players: 1` emits **nothing** (byte-identical). | 0.6 |
| `boundary {margin?,message?}` (opt) | spec-0013: declares a **derived** playable region (union of final placed-piece AABBs, inflated horizontally by `margin` (`0..=64`, default 16; else `DW0321`), unbounded up, floor = lowest placed block − 8). A 1s clock returns any player outside it to the last checkpoint (`dw:cp`) with an actionbar `message` (l10n `world.boundary.message`, English default when absent) + a soft sound; no damage, no item loss. `horizon:"ocean"` without a `boundary` = `DW0320`. | 0.6 |

### Stage 2 — `npcs` (casting sheets, stationary)

| Field | Behavior | Since |
|-------|----------|-------|
| `id`,`name`,`area`,`anchor`,`base_entity` | NPC body placed at resolved anchor; `name` → l10n `npc.<n>.name`. | 0.1 |
| `role` | Enum `quest-giver|flavor` — what the speaking part does. How hard a fight is billed is `tier` on the body that fights (a `waves[]` entry or a stage-5 actor), never a role here. | 0.2 |
| `persona{archetype,speech_style,motivation,…,relationships[]}` | Structured; **excluded** from l10n; relationship refs validated in-stage (`DW0112`). | 0.2 |
| `skin{texture_id,model}` (opt) | Switches body to `minecraft:mannequin`; PNG baked to resourcepack. Missing PNG → `DW0309`; bad/dup id → `DW0190`. The bake walks **bodies**, not one stage's list (`dsl::body_skin_sites`), so a stage-5 actor's `skin` is served and refused by exactly this rule. Every summon whose entity id comes from **content** rather than this switch (`npc.base_entity`, `actor.entity`, and the `unleash` twin, which has no skin branch at all) is spliced with `pose:"standing"` when that id names a mannequin (`emit::mannequin_pose_nbt`): a mannequin summoned without an explicit pose serializes it as `DYING`, which the server then fails to encode at save (`Failed to encode value 'DYING'` in a PackTest world's teardown). A non-mannequin entity gains nothing, so existing campaigns stay byte-identical. | 0.4 |
| `deferred` (opt, bool) | **Not** summoned at world init; the NPC's body + hitbox appear only when a `spawn-npc` effect fires, at this same `anchor` (the dual of `despawn-npc`). Default `false` = pre-0.6 behavior, byte-identical. Never spawned → `DW0197`; a `talk-to` provably ahead of every spawn → `DW0198`. | 0.6 |
| `traversal{locomotion}` (opt) | **What this body can do when it moves** (spec-0034; reserved `DW0141` pre-0.11, fenced on THIS stage's own `dsl_version`). The author's side of the traversal proof: by default the compiler derives locomotion from the entity id, and this overrides it for one body. **One shared type on every object class that has a body, a position and a compiler-emitted route** — the stage-2 NPC and the stage-5 actor (`dsl::body_traversal_sites`, a closed sum type, so a third body class is a compile error at every consumer until it is handled) — because traversal belongs to the body, not to the verb that first needed it. `ground|climber|flier`; `aquatic` is refused (`DW0455`). **A declaration is a claim, not an opt-out**: it must change a rule's verdict or the build fails (`DW0454`), and it can never reach the error tier (`opens_gates` is derived and unauthorable, and no class is exempt from `DW0452`). It changes which rules examine the body and nothing else — routing is unchanged, every body walks the same ground A*. Emission is byte-identical whatever is declared. | 0.11 |

### Stage 3 — `classes`

1..4 classes. `kit[]` = vanilla item id + count + optional display `name`
(→ l10n `class.<c>.kit.<i>.name`) + optional `carrier` (v0.6, spec-0018:
`all` (default) or `one`). A kit is per-player gear by construction; `carrier:
"one"` marks a **party-unique** kit item — `class_apply_<c>` guards that one
`give` behind a `#kit_<class>_<i> dw.sys` latch, so exactly one copy enters the
party (the first player to take the class) while the rest of the kit is unchanged. `name`/`blurb` player-visible.

**`flask`** (bool, DSL v0.8, spec-0016 §1; reserved
`DW0141` pre-0.8): this kit entry is the class's **recovery item**, and resting at
a bonfire replenishes it to exactly its declared `count`. A campaign that places a
`bonfire` and declares no flask anywhere is `DW0476`. Declaring one also makes
`class_apply_<c>` add a `dw_class_<c>` tag to the player — the pack has to remember
which class a resting player took, since `dw.class` is a trigger the apply resets
and `dw.classed` records only *that* a class was taken. Both are absent from a
campaign that declares no flask, so its class apply is byte-identical.

**`contents`** (obj, DSL v0.8, spec-0016 §1; reserved
`DW0141` pre-0.8): **what is in the bottle** — vanilla's
`minecraft:potion_contents` component, modelled field for field:

| Field | Behavior |
|-------|----------|
| `potion` (str, opt) | A 1.21.11 potion id (`minecraft:strong_healing`, `minecraft:long_night_vision`). Strength and duration are *part of the id* (`strong_`/`long_` prefixes) since 1.20.5 — not separate fields. Checked against the pinned `potion` registry (46 ids, inlined in `dsl::registry::POTION_IDS_1_21_11` — complete for the pinned version, so nothing is injected). |
| `effects[]{effect,duration?,amplifier?}` (opt) | The component's `custom_effects`. `effect` is checked against the same status-effect registry wave mobs use; `duration` is in **ticks** (20 = 1 s, 1–1 000 000) and is **required** for a lasting effect, **forbidden** on the two instantaneous ones (`instant_health`/`instant_damage`, applied once on drinking — a duration there is never read); `amplifier` is 0 = level I, 0–255 (vanilla's unsigned byte). Absent `amplifier` is emitted as absent and takes vanilla's own default. |
| `color` (str, opt) | Bottle colour override, `#rrggbb`, emitted as the packed int `custom_color`. Absent → the colour vanilla derives from the effects. |

Legal only on the four items that actually carry the component
(`minecraft:potion`, `splash_potion`, `lingering_potion`, `tipped_arrow` — read
off the pinned `item_components` summary); anywhere else the game would discard
it, so it is `DW0486`. At 0.8.0 one of those four items **without** `contents` is
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

Reserved kit
fields `lore`/`enchantments`/`attributes` are **not defined** → unknown-field
`DW0100`. Kit items carry **no semantics**: a night-vision potion in a kit is
flavor. The `DW0210` dark mitigation is the stage-1 `areas[].mitigation`
declaration only — the pre-0.6 heuristic that read a kit item's id/display name for
`night_vision` was **deleted** (see §4 "Semantics never key on player-facing text").
Because the signal is a declaration, the `DW0210` verdict is language-independent
by construction (ADR-0006) — nothing is threaded past the `--lang` localization
pass any more.

### Stage 4 — `quest-plan`

Quest DAG skeleton: `depends_on` acyclic (`DW0130`), `finale` declared
(`DW0131`), and the **partition** (spec-0051). `mandatory: false` is legal at
**0.17.0** and is refused below it (`DW0133`). A quest declares which half of
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
| `branch_points[]` | `{id, opens_at, forks_on[], branches[]}` (spec-0025, reserved `DW0141` pre-0.8) — the campaign's declared **story forks**. `forks_on` is the flag set this fork owns; `opens_at` the quest at which it opens. Each branch is `{id, flags[], leads_to}`: `flags` is the subset of `forks_on` this branch holds (the rest of `forks_on` is pinned **unset** on it — that half is what makes `DW0484` decidable), and `leads_to` is a **single** field whose id prefix says which kind of terminus it is — a `quest/<kebab>` the branches converge at, or an `ending/<kebab>` this branch runs to. One field rather than two mutually exclusive ones, so "exactly one of them" is an unrepresentable state instead of a rule some diagnostic polices: a value with neither prefix is the ordinary `DW0110`, one naming nothing is the ordinary `DW0112`. Enumerated branches are the **product of the declared points**, so the branch set is authored and small. Empty/absent = a campaign claiming to have no branch, which the compiler then *verifies* rather than assumes (`DW0480`). Proofs: `DW0480`–`DW0485`. | 0.8 |

### Stage 5 — `quests` (+ v0.3/v0.4 gameplay surface)

| Element | Fields → behavior | Since |
|---------|-------------------|-------|
| `trigger` | `campaign-start` \| `quest-complete{quest}`. | 0.1 |
| Objective `talk-to` | `{npc}`; completes via a stage-6 dialogue option (backward). | 0.1 |
| Objective `reach-anchor` | `{anchor,radius}`; completes inside a block box of half-extent `max(1, radius)` at the anchor cell (v0.3+; v0.2 = sphere). | 0.1 |
| Objective `kill` | `{wave,after?,requires_flags?}`; completes when wave countdown hits 0. | 0.3 |
| Objective `collect` | `{item,count,anchor,container?,item_name?,fill_count?,…}`; chest at anchor, `inventory_changed` advancement. **Container adoption (v0.8):** `container` names the anchor whose assembled-world cell holds a `chest`/`trapped_chest`/`barrel` the PREFAB placed — the objective fills that furniture where it stands and the compiler places nothing (the same division of labour `loot` and a trap's dispenser keep; a cell with no container is `DW0438`). `item_name` gives the collected item a display name as a vanilla `custom_name` component — player-visible, so it is l10n-inventoried (`obj.<quest>.<obj>.item_name`) and translated like any other line; adjudication is unaffected because both the completion advancement and the per-tick held check match on ITEM ID. `fill_count` pads the container so it READS full (vanilla fullness is occupied slots, not stack size): the objective's own stack lands in `container.0` and each padding stack repeats it in `container.1`, `container.2`, …, positionally and totally (ADR-0006). Ceiling `1 + fill_count ≤ 27` (`DW0432`, the `loot` rule); a container claimed by both a `loot` entry and a `collect` — or by two collects — is `DW0435`, since positional fills overwrite each other slot-for-slot. An adopted container also joins the layout solver's **required-anchor** set (a pool draw that omitted its carrier would leave the objective nothing to fill) and becomes the `critical_path` step position, because the bot's job is to open *that* block. All three absent = the pre-0.8 emission, byte for byte; declaring any of them below 0.8.0 is `DW0141`. **Drop gating (v0.9):** `dropped_by` names the **wave** whose declared `drops[]` provide this item, and provisioning moves from the world to the fight — no chest is placed and no fill is written, because the item does not exist until the boss dies. Waves only: an actor's death is observable by no objective, so an actor-gated collect would be an unprovable claim and is excluded per the no-hack doctrine (an actor may still declare drops; they just cannot gate a quest). Mutually exclusive with `container` (`DW0492`). Two proofs make "kill the boss -> take its key -> open the door" a chain the compiler checks rather than an authoring intention: the wave really declares an `{item}` drop of this item, in at least the count asked for (`DW0492`), and a `kill` objective for that wave provably precedes this collect — through the intra-quest `after` graph or a quest this one `depends_on` (`DW0493`). The `critical_path` step points at the wave's own anchor and carries `dropped_by`, so the bot walks the ground the fight ended on instead of opening a block that is not there. Absent = the pre-0.9 emission byte for byte; declaring it below 0.9.0 is `DW0141`. | 0.3 / adoption 0.8 / drop gating 0.9 |
| Objective `interact` | `{anchor,requires_item?,missing_item_hint?,prop?,…}`; interaction entity. **`requires_item` means HELD, not possessed**: `execute if items entity @s weapon.mainhand <item>`. Presenting the item IS the action — a player who right-clicks a sleeping giant with the stake stowed in their pack has not stabbed anything, and an inventory-wide reading (`container.*`) would fire the moment the item was picked up anywhere, whatever the hands were doing. It is a **global semantics, not an opt-in flag**: every `requires_item` in every campaign means held. `missing_item_hint` (v0.7) is the diegetic answer to a click that arrives without the item in hand — one guarded per-player `tellraw`, carrying the objective's own activation guard so an inactive or finished interaction stays silent, emitted before the trigger reset so one click yields exactly one line. Absent = the pre-0.7 silence, byte-identical. Requires `requires_item` (`DW0437`); l10n key `obj.<quest>.<obj>.missing_item_hint`. `prop{block}` `setblock`s the affordance (v0.4); `block` accepts a verbatim blockstate suffix `id[key=value,…]` (v0.6). | 0.3 / prop 0.4 / held + hint 0.7 |
| `after[]` | Ordering (acyclic → `DW0140`). | 0.1 |
| `requires_flags[]` | AND-gate on set flags (puzzle primitive). | 0.3 |
| `state[]` | `{id, scope, initial?, note?, name?}` (spec-0031/spec-0032, reserved `DW0141` pre-0.10) — **runtime state**: a named, integer-valued datum the campaign sets, adds to and clears while the delve is played. What `FlagId` is not: a flag is boolean, party-wide and monotonic (no verb clears one), which is right for "this has happened" and useless for a balance, a floor number, or "a ride is in progress". Unlike a flag a datum **is declared**, because two facts about it cannot be recovered from its use sites: `scope` (`player` = each player holds their own, `party` = one shared value on the `#party` holder, spec-0018) and `initial` (the value it starts at, and the value `clear-state` returns it to — one field, not two, so a datum can never be un-returnable to its own start). `note` is authoring prose, never machine-checked and never shown (the forcing function `cast[].doing` plays for a scene). Emission: one `dw.s_<local>` scoreboard objective per datum; a `party` datum seeded in `setup` (world init is exactly its lifetime), a `player` datum seeded on each player's first tick by `state_seed`, tagged `dw_state` so a relog does not re-seed. Absent = no objective, no function, no tick clause — byte-identical to pre-0.10. | 0.10 |
| `requires_state[]` | `[{state, op, value}]`, `op` ∈ `equals` \| `not-equals` \| `at-least` \| `at-most` (spec-0031, reserved `DW0141` pre-0.10) — the **numeric third field of the gate**, accepted at every one of the 36 sites `requires_flags`/`forbids_flags` are: all five objective kinds, all twenty-six gatable effect verbs, `triggers[]`, `traps[]`, dialogue options, cast placements and **shop offers** — where it is the price. The count is re-derived from the schema at each move, never adjusted to fit: 28 at spec-0031 §1, 30 with `fill-region`/`clear-region`, 33 with `give-effect`/`clear-effect`/`teleport`, 35 with `drop-stake` and the shop offer, 36 with `open-way` — each new verb carrying the whole gate at its FIRST site, which is where generality is decided. It lives in the gate and not in any verb because the comparison's consumers are exactly the gate's consumers — "this door opens at 500", "this line is withheld below 200", "this lever does nothing while the car is moving" — and generality is decided at the FIRST site (CLAUDE.md). Emission: ` if score <holder> dw.s_<local> matches <range>`, spliced into the guard each consumer already builds, where `<holder>` is `#party` or `@s` per the datum's declared scope. Four operators, not six: over integers `less-than n` is `at-most n-1`, and a second spelling of one thing is a second emission path to keep honest. Absent = no clause — byte-identical to pre-0.10. Enforced total by `crates/dsl/tests/gate_consumers.rs`, which enumerates the consumers from the **generated JSON Schema** (i.e. from the types) and reds when any gate-declaring object carries only part of the gate. | 0.10 || Effect `set-state{state,value}` | Writes a declared datum to an absolute value (spec-0031). `scoreboard players set <holder> dw.s_<local> <value>`. | 0.10 |
| Effect `set-state{state,value}` | Writes a declared datum to an absolute value (spec-0031). `scoreboard players set <holder> dw.s_<local> <value>`. | 0.10 |
| Effect `add-state{state,amount}` | Moves a declared datum by a **signed** amount (spec-0031); negative counts down. One verb, not an `add`/`subtract` pair: a purse a shop debits and a stake a death forfeits are one operation with the sign flipped. Lowers via vanilla's `scoreboard players remove` (its `add` takes an unsigned operand). | 0.10 |
| Effect `clear-state{state}` | Returns a declared datum to its declared `initial` (spec-0031) — the verb a flag has never had, and the reason a datum is not a flag. It **writes** the initial rather than `reset`ting the score: a reset score is *absent*, and an absent score makes `unless … matches` true, so a cleared datum would silently satisfy a `not-equals` comparison against its own starting value. | 0.10 |
| Effect `give-effect{effect,seconds,amplifier?,hide_particles?,in?}` | Grants a vanilla **status effect** for a stated duration (spec-0031). The engine has emitted status effects since v0.6 — the `mitigation: "night-vision"` clock is a self-rescheduling, region-scoped `effect give` — and exposed none, so "blind the party for the ride" or "slowness in the deep water" had no surface at all. `effect` is any id in the pinned 1.21.11 `mob_effect` registry (`DW0192`, the same registry and the same code a wave mob's `effects[]` answers to; a bare `blindness` normalizes to `minecraft:blindness`). `in` narrows to players inside an anchor-centred box — the SAME `StealthZone` a `begin-stealth` zone, a `damage-players` `in` filter and a `lethal_volumes[]` region use, through the one `Plan::zone_box` and the one `emit::box_selector_args` — which is what makes "blind whoever is riding" expressible without blinding the delve. **`seconds` is required and there is no `infinite` spelling**: a grant whose only removal is a later command is one the player keeps forever whenever that command does not run (a logout, a crash, an interrupted chain), so the hazard is made inexpressible rather than diagnosed, `1..=50000` (`DW0541`, derived from `MAX_POTION_DURATION_TICKS`, not picked again). Emission: `effect give <audience>[<box>] <effect> <seconds> <amplifier> <hideParticles>` — vanilla's full five-token form, from the one formatter the night-vision clock now also uses, so nothing is left to a vanilla default a future version could re-pick. No `tag=!dw_cutscene` guard, deliberately: a status effect is not inherently harm, and the engine's own region-scoped grant has never carried one. | 0.10 |
| Effect `clear-effect{effect?,in?}` | Vanilla's `effect clear` (spec-0031). **Not** how a `give-effect` is meant to end — a duration is — so it exists for effects this campaign did not grant: a potion the player drank, a `wither` a mob applied, the whole set at a bonfire. `effect` is therefore optional: absent clears everything, exactly as `effect clear <targets>` does. Pairing it with a still-live grant of the same effect in the same bundle is `DW0540`. Emission: `effect clear <audience>[<box>] [<effect>]`. | 0.10 |
| Effect `teleport{from,to}` | Moves **everything inside a declared volume** to an anchor (spec-0031). The selector is a **region, never a block**: "whoever is standing on this block" has three different answers for a player half a foot over the edge, a player mid-jump and a player sneaking on the lip, and a volume has one. `from` is the anchor-centred `StealthZone`; `to` resolves to a literal cell at build time, so the emitted command carries absolute coordinates and does no runtime search. Emission is a call into a generated `teleport_<content-key>` function whose whole body is exactly one line: `tp @e[x=…,dx=…,y=…,dy=…,z=…,dz=…,tag=!dw_fixture] <x> <y> <z>` — a named function for the same reason `volley` and `collapse` have one (the body is compiler-proven geometry, and a body that only ever exists spliced into a `seq_<hash>` is a body no runtime test can call). **The selection is total over bodies** — the six box terms plus the one class exclusion every box-narrowed entity selector in the engine carries (`tag=!dw_fixture`, `DW0545`), and no `type=`, no `limit=`, no `sort=`; the effect's own audience is ignored (a box has no party). A machinery-type exemption of the kind `lethal_volumes[]` must carry was considered and **rejected**: a stage-2 NPC is a body plus a co-located `minecraft:interaction` carrying its dialogue, so exempting that type would move the speaker and leave the thing players click behind, in silence — and everyone inside the volume travels, players and entities alike. What stands in its place is a CLASS the object declares (`DW0545`): an engine place whose position is engine state carries `dw_fixture` and is skipped, while an NPC's dialogue hitbox carries `dw_borne` and rides whatever its speaker rides. A place whose cell the compiler knows is refused outright at compile time (`DW0542`), which is available here and was not available to the lethal volume: a volume damages whatever *wanders* in, which the compiler cannot enumerate, while a teleport's harm is to what the compiler itself **placed**. **A teleport is not a rescue**: accumulated fall distance carries across one unchanged (measured Δ `0.0000` in 46/46 trials on the pinned 1.21.11, including teleports 143 and 157 blocks straight *up*; landing damage `floor(fall_distance) − 3`) and is charged in full at the destination, so a platform arriving under a falling player past ~20 blocks of fall is the surface they die on. No fall-distance reset is emitted: what *does* reset it was explicitly NOT measured (`docs/notes/death-and-teleport-spike.md` §5), and a mechanism invented from recall is the folklore this project forbids. The runtime half is a generated PackTest per teleport: it puts a `zombie`, an `interaction`, a `marker`, a `text_display` and an `item` in the volume — the four an exemption list of `LETHAL_EXEMPT_TYPES`'s shape would have dropped, beside a content body — asserts all five are inside the box, calls the campaign's own `teleport_<key>`, and asserts the box is then empty. That half cannot be a Rust test: whether vanilla's `@e[<box>]` really reaches every entity type is vanilla's fact, not the compiler's. Measured red→green on the pinned toolserver — with `,type=!minecraft:interaction` added to the emitted selector the template fails *Expected #tp_left 0, but got 1*. **Completability is not modelled**: nav reasons about walked routes and knows nothing of this verb, so a route that exists only through a teleport still fails `DW0311`. That is sound but incomplete (a teleport can only add reachability), and the lift's own completability lands with the lift. | 0.10 |
| `forbids_flags[]` | Negative gate, accepted **everywhere `requires_flags` is** (objectives, `triggers[]`, per-effect, dialogue options, `traps[]`): the element is suppressed while ANY listed flag is set. Per-player sites emit `unless score @s dw.f_<flag> matches 1` clauses (unset-safe — flag scores are never pre-initialized, so a `scores={…=..0}` selector would wrongly fail on unset); trigger arming uses the any-player form `unless entity @a[scores={dw.f_<flag>=1..}]` (a positive selector inside a negation). Unknown flags get the same `DW0172` treatment as `requires_flags`. Reserved (`DW0141`) pre-0.6 at every site. | 0.6 |
| `waves[]` | `{id,anchor,mobs[{entity,count,name?,attributes?,effects?,equipment?}]}`; entity validated (`DW0173`); `attributes`/`effects` are v0.4 (`DW0192`). `equipment{head?,chest?,legs?,feet?,main_hand?,off_hand?}` is v0.6 (reserved `DW0141` pre-0.6): slot item ids validate against the pinned 1.21.11 item registry (`DW0143`, the give-item family). Each slot is **either a bare item id string or `{item, enchantments{<id>: <level>}}`** (spec-0021) — the plain string stays the plain string, which is what keeps every pre-enchantment campaign byte-identical on re-serialisation; enchantments emit as the 1.21 `minecraft:enchantments` item component inside the slot compound, ids validated (`DW0433`) and levels range-checked (`DW0434`); emitted as component-era `equipment`/`drop_chances` summon NBT (never legacy `ArmorItems`/`HandItems` — 1.21.11 ignores them) with **drop chance 0 on every slot** (no-grind: wave gear is never lootable). Explicit slots merge over the armed-mob main-hand default (a helmeted skeleton keeps its bow; explicit `main_hand` overrides). A helmet is the sanctioned daylight-undead fix — never `set-time` — and that rule is **enforced**, not merely offered: a burning species staged for a fight whose ground reaches open sky under a pinned daytime hour is `DW0496`. **`drops[]` (v0.9; reserved `DW0141` pre-0.9)** names the DECLARED SUBSET this mob leaves behind — usually one piece, never automatically everything. Two entry forms: `{slot}` (a worn piece; the slot must be one the same mob's `equipment` really fills, and each slot at most once — `DW0490`) and `{item, name?}` (a quest token the fight yields rather than wears; id validated `DW0143`, `name` l10n-inventoried as `wave.<wave>.mob.<i>.drop.<n>.name`). Only an `elite`/`boss` wave may declare drops (`DW0491`) — rank-and-file gear stays unfarmable by construction. | 0.3 / tuning 0.4 / equipment 0.6 / drops 0.9 |
| `loot[]` | `{id,anchor,items[{item,count?,name?,enchantments?}]}` (spec-0021, reserved `DW0141` pre-0.6) — contents for a container the **prefab already placed**, the same division of labour a trap has with its dispenser. The compiler never places the container; `DW0431` proves one is really there. Slot assignment is **positional and deterministic**: the nth declared stack lands in `container.<n>` (ADR-0006 — no loot tables, no RNG, no seeded shuffle). Emitted in `setup_finish` as `item replace block … container.<n> with <item>[components] <count>`, so a campaign with no `loot` is byte-identical. `name` enters the l10n inventory as `loot.<id>.item.<i>.name`, exactly like a class kit item's name. Item ids validate against the pinned registry (`DW0143`), anchors against prefab metadata (`DW0142`); `DW0432` caps a fill at 27 stacks and `DW0435` rejects two fills of one container. | 0.6 |
| `lethal_volumes[]` | `{id,region{anchor,extent},message,damage_type?}` (spec-0031, DSL v0.10; reserved `DW0141` pre-0.10) — a declared box that **kills whatever enters it**, worded by the campaign's own strings. A mechanism, not a fiction: the commissioning case was a cliff whose fall must be fatal, and the same declaration is a lava pit, an acid pool, an out-of-bounds plane or the bottom of a lift shaft. The alternative considered and **rejected** for the cliff was making the world's `horizon` void so the fall kills anyway — that changes approved art to obtain a behaviour, and it serves exactly one fiction. `region` is the existing anchor-centred box (`anchor ± extent`), the SAME `StealthZone` type a `begin-stealth` beat and a `damage-players` `in` filter use, resolved through the one `Plan::zone_box`; a private twin with the same two fields would be `tools/check-capability-ownership.py` check C by construction. `message` is required (`DW0512` rejects a blank one) and is l10n-inventoried as `lethal.<id>.message`. `damage_type` is the curated `DamageKind` shared with `damage-players` (default `generic`), so a volume can no more void a held totem than a scripted hit can; it is what words **vanilla's own** death broadcast (`fall` → *fell from a high place*) while `message` says what the place was. Emission: one `function <ns>:lethal_<id>` line on the tick, and a two-line body — `execute as @a[<box>,tag=!dw_cutscene] run function <ns>:lethal_<id>_kill` (which `damage @s 1000 <type>`s, reads the player's health back into `#leth_hp dw.sys`, and `tellraw`s the wording as a `{translate,fallback}` component **only if that health reached 0**), plus one `execute as @e[<box>,type=!minecraft:player,type=!…] run damage @s 1000 <type>` for everything else. The engine's own machinery types (`interaction`, `marker`, `item_display`, `block_display`, `text_display`) are excluded — a volume drawn across a cutscene dolly must not erase the camera. Content bodies (wave mobs, actor puppets, NPCs) are deliberately NOT excluded: a mob that walks into the lava dies, which is the mechanism working. The kill is an ordinary `/damage`, so the vanilla `deathCount` edge (`dw.deaths`/`dw.death_ack`), the checkpoint re-seat (`cp_respawn_check`) and `keep_inventory` see the death they already handle — there is **no second death detector**. Completability: the volume's cells are impassable in the shared nav `World` (`DW0510`) and no place the campaign POSTS something — a respawn seat, an NPC anchor, a `cast` placement, an actor anchor — may sit in one (`DW0511`). A campaign that declares none emits no tick line, no function and no ledger. | 0.10 |
| `timed_gates[]` | `{id,gate,open_ticks,closed_ticks,phase?,crush?,disarm?}` (spec-0016 §4 + addendum, reserved `DW0141` pre-0.6) — a gate region on a deterministic open/close clock, so passage is a timing read rather than a permanent state. Emission is a **self-sustaining two-function ping-pong** (`tgate_open_<id>` / `tgate_close_<id>`), each half doing its `fill` and scheduling the other; `schedule` is replace-mode so the clock can never double up, and a timed gate costs **nothing per tick**. The gate is sealed by the prefab at world-load, so the clock's first act is always an OPEN (`phase` holds it shut that many ticks first). Structural errors are `DW0377` (id, a half-cycle of 0, a `phase` at or beyond the cycle, two clocks on one region, or a gate a `shortcut` already owns — a clock would re-seal what `DW0372` forbids re-sealing); a gate anchor with no declared fill `block` is `DW0343`. The design proof is `DW0378`: **not** all-phase passability (a gate that punishes bad timing is the point) but ≥ 20% of the cycle admitting a crossing. **`crush`** (optional, default `false`) makes the closing edge a real portcullis judgement: every player whose position intersects the gate region when it shuts is dealt lethal `damage` by command. It is a *command*, not suffocation, because vanilla's in-wall damage is slow, gear-dependent and escapable — a portcullis that merely inconveniences teaches nothing, and `DW0378` has already proven the window fair, so the penalty may be absolute. Zero per-tick cost is preserved: the judgement rides the closing tick of the ping-pong that already runs. Defaulting to `false` keeps every pre-addendum campaign byte-identical. **`disarm`** (optional — souls dossier §5.2) is the ladder's third rung: readable, avoidable, and finally *disable-able*, the way Smouldering Lake's ballista and the Fringefolk chariot can be removed for good. Its shape is `{via, sets_flag}`, **exactly** a trap's `disarm`, and it carries the same obligations: the `via` anchor gets a compiler-owned interaction entity **plus visible hardware** (`DW0420`), it may not be the gate anchor itself (`DW0377`), and it must be reachable from the campaign entry while the gate is SHUT (`DW0393`). Interacting with it suppresses the clock **permanently with the gate resting OPEN** — a jammed portcullis stays up — and permanence is structural exactly as a shortcut's is: no emitted function re-arms the clock and no `close-gate` may name the gate (`DW0389`). A disarmed gate therefore **can never crush**: the judgement rides the closing tick, and the closing tick is inside the suppressed clock. `DW0378`'s 20% duty-cycle proof and `DW0388`'s observability proof are unchanged and apply identically — observability is about the *pre-disarm* read, which is how the party decides the jam is worth the walk. Defaulting to absent keeps a campaign that declares no `disarm` byte-identical. | 0.6 |
| `on_death[]` | `[<effect>, …]` (spec-0031, reserved `DW0141` pre-0.10) — the campaign's **death beat**: effects run at the moment a player dies, for that player. Effect root **R7**, so it is visited by `for_each_effect_root` and therefore by every walk defined over it (l10n inventory, the flag model, the timeline, `DW0360`'s anchor seal, the completability model). One bundle per campaign, not one per checkpoint: *where you come back* is a property of a checkpoint, *that you died* is not, and a bundle repeated on each checkpoint would be N copies with N chances to forget one. Phase-specific behaviour uses the ordinary per-effect `requires_flags`/`forbids_flags` gate every root already carries — there is no second gating surface. It exists so that a delve's death consequence ("the purse is dropped where you fell") is ordinary content in a general mechanism rather than an engine feature. Optional in the strongest sense the completability model has — nobody is forced to die — so it registers `close-gate`s only, never an `open-gate` the proof could lean on, and nothing inside it is credited as a flag producer (a mainline reachable only by dying is not reachable). Emission and the death edge: see the `on_death` row under "Effect verbs". | 0.10 |
| `state[].name` | A datum's **player-visible name** (spec-0032, DSL v0.10; reserved `DW0141` pre-0.10). **A named datum is a currency**, and there is deliberately no separate `currencies` section: a purse is a runtime datum the player can see, and "the player can see it" is a property of the datum, not a different object class — a second struct carrying `id`+`scope`+`initial`+`name` would be a private copy of this one. Present ⇒ the datum **announces its new balance whenever it changes, from any cause** — a purchase, a death's forfeit, a stake collected, a plain `set-state` — on its holder's action bar as `<name>: <value>`, with the value carried by vanilla's own `{"score":…}` component so the line is the live balance rather than a number baked at emit time. One tick driver and one `st_show_<id>` function per named datum, keyed on a shadow score seeded beside the datum itself (so joining a world announces nothing). **The announcement belongs to the datum, not to the verbs that write it**, and that is a correctness property rather than tidiness: a readout emitted inside a gated effect carries that effect's gate, and the gate is evaluated AFTER the write it reports — spend your last coin behind `at-least 1` and the balance moves to 0, so the inherited guard stops holding and the one change the player most needs to see is the one they are never told about. Found by reading the generated `shop_pick_0_0` of this feature's own first shop. Absent ⇒ the datum is silent bookkeeping and emission is byte-for-byte what spec-0031 shipped. l10n-inventoried as `state.<id>.name`. |
| `shops[]` | `{id,anchor,title,marker_item?,offers[{label,tooltip?,requires_flags?,forbids_flags?,requires_state?,effects[]}]}` (spec-0032, DSL v0.10; reserved `DW0141` pre-0.10) — an interaction point that opens a list of gated offers. **It is the bonfire rest flow with different buttons**: a `minecraft:interaction` hitbox plus a glowing `minecraft:item_display` armed at world init, a `player_interacted_with_entity` advancement whose reward runs `shop_open_<i>` AS the clicking player (the entity's own `interaction` record names no player, so nothing else can say *who* is buying), a `minecraft:multi_action` dialog whose buttons run `/trigger dw.shop set <n>`, and tick dispatch on `dw.shop`/`dw.shop_at` — the same pair `dw.rest`/`dw.rest_at` uses, because `/trigger` is the only command a non-op player may run. **There is no `price` field**: an offer is the seventh `Gate` consumer and a price is its `requires_state`, so every rule that already governs a numeric comparison (`DW0500`–`DW0503`) governs a price for free. Villager `Offers` is excluded for three independent reasons (a trade cost can only ever be an item, right-click on a villager body is already dialogue's, and the data-driven trade registry post-dates 1.21.11). An offer's own gate makes its `/trigger` handler inert (`return fail`), exactly as a dialogue option's does, so a bot chatting the trigger cannot buy what the gate refuses; **a refusal that speaks is authored as a gated effect** — the purchase behind `at-least <price>`, the apology behind `at-most <price − 1>` — so the engine adds no `refused` field. `effects[]` is effect root **R8**, run with the buying player as `@s`. Strings: `shop.<id>.title`, `shop.<id>.offer.<i>.label`, `shop.<id>.offer.<i>.tooltip`. |
| `stakes[]` | `{id,state,forfeit?,max_live?,on_full?,collect_by?,collected_message,marker_item?}` (spec-0032, DSL v0.10; reserved `DW0141` pre-0.10) — **what a death leaves behind, and the one chance to get it back.** A mechanism, not a genre: "souls" is one setting of it. `state` must be a `player`-scoped datum (`DW0520`) — a stake is a personal wager. `forfeit` is `all` (default) / `{proportion,percent}` / `{fixed,amount}` / `none`, computed in integer scoreboard arithmetic and clamped at zero so a death can never hand the player money. `max_live` (default 1) is how many live stakes one player may hold — `0` is the **no-death-cost** configuration (nothing forfeited, nothing placed, no machinery emitted at all) and a larger number with `on_full: keep` is the **memorial at every death site**; `on_full` is `replace` (default; retire the oldest and place a new one — the souls loop) or `keep` (leave the wager alone, forfeit nothing). `collect_by` is `owner` (default) or `anyone`. The marker is an invisible `minecraft:interaction` plus a glowing `minecraft:item_display`, deliberately **not** an item entity (which despawns after 6000 ticks, burns in lava, sinks in the void and can be picked up by anyone), so it inherits `DW0420`/`DW0421`. The ledger is per-player scoreboards — amount, live flag and marker position per slot — because a scoreboard survives death, logout, restart AND chunk unload, which an entity in an unloaded chunk does not. Collection is idempotent under a double right-click in one tick by construction: taking a slot clears its live flag as part of taking it. Strings: `stake.<id>.collected`. |
| Effect `drop-stake{stake}` | Leaves a declared stake for the acting player (spec-0032, DSL v0.10; reserved `DW0141` pre-0.10): apply the retention policy, forfeit the declared share, and place the marker at the anchor the **compile-time placement table** gives for where they are. Nothing about the verb says "death" — it is written in `on_death` because that is where a souls-shaped delve wants it, but the mechanism is "leave a recoverable cache where the acting player stands". What *is* death-specific is a property of the `on_death` root, not of this verb: the corpse stands on the death position for every cause measured, so `execute at @s` inside the beat is positioned at the death point with no capture at all (`emit::death_position_capture` emits nothing). Emits one `function <ns>:stk_drop_<id>` call. |
| `ambushes[]` | `{id,at,actors[],trigger,telegraph[]?}` (spec-0016 §3, reserved `DW0141` pre-0.6) — **sugar**, not a new runtime mechanism. `parse_campaign` desugars each ambush into an ordinary one-shot `EnvTrigger` named `trigger/<local id>` at `at`, whose effects are the `telegraph` bundle, then a `spawn-actor` per listed actor, then an `unleash-actor` per listed actor. Everything downstream — validation, l10n, the flag/wave producer scans, nav, emission — sees only that trigger, so the sugar has no second code path to drift down and an ambush is exactly as debuggable as the trigger an author would otherwise type. The canonical form of a campaign is therefore its **desugared** form (the section is never serialized), which is what keeps the canonical round-trip idempotent. `telegraph` is **optional and stays optional**: the un-telegraphed ambush is core souls vocabulary and nothing in the compiler asks for a tell. Declaration errors are `DW0375`; the counterplay obligation is `DW0376`. | 0.6 |
| `waves[].respawns_on_rest` | `true` re-seats the wave on every bonfire rest **and** on every respawn at a bonfire (spec-0016 §1) — the souls contract: progress is kept, the enemies come back. Emission: `spawn_<wave>` additionally sets a seated sentinel `#wseat_<wave> dw.sys`, and `wave_reseat_<wave>` kills every survivor carrying `dw_wave_<id>` then re-runs the wave's own spawn (authored composition, DW0312-proven cells). A rest only re-seats waves the party has actually met — an unmet wave is never conjured. **Stationed re-seat**: a re-seated wave returns to the state it was FIRST seated in, never to the state the party last left it in — a lane wave re-enters its routed patrol from the lane start (`Patrolling:1b` re-applied, `patrol_target` back on waypoint 0, `#lane_<wave>` back to 0, the clock re-armed through the same replace-mode `schedule`), a non-lane wave stands at its anchor under vanilla-local AI with no patrol NBT at all. **Nothing re-seated may pursue across the map.** This holds because `wave_reseat_<wave>` re-enters through the wave's own `spawn_<wave>` and everything stationing a wave is written there and nowhere else, so the spawn state and the stationed state are the same bytes — an invariant the tests pin (`wave_reseat_<wave>` is exactly two lines) rather than a coincidence of the current emission. What earns it is `DW0478`: a bonfire may not stand where a re-seated force can perceive it. Generated PackTests `souls_reseat_stationed` (a rest, driven from the squad hauled onto the party and released to native AI) and `souls_td_lane_reseat` (the re-summon alone, for a lane campaign with no rest point beside its lane). Declaring the field with **no** `bonfire` in the campaign is inert, so it is `DW0370`, not a silent no-op. Reserved `DW0141` pre-0.6. | 0.6 |
| `waves[].tier` | `ordinary` (default) \| `elite` \| `boss` — what the content **bills** the encounter as (spec-0023). A declaration, never a knob: the compiler is forbidden from *scaling* content from it. Its main consumer is the bot ladder's **inverted floor gate** — an `elite`/`boss` encounter the UNASSISTED bot beats on its first attempt is reported as too easy for its billing (warning tier, advisory, content decides). Marking is authored rather than inferred because "this stack looks tuned, so it must be an elite" is exactly the downstream folklore the no-hack rule forbids. Through spec-0016 §1's **undefeated re-seat** the tier also reaches emission in exactly one place: in a campaign with a `bonfire`, a billed `elite`/`boss` wave that does NOT declare `respawns_on_rest` is refreshed by a rest *while it is still standing* — see the `bonfire` row in §3. Billing a wave `boss` **and** `respawns_on_rest` is `DW0499`. Absent ⇒ `ordinary` and omitted from serialisation, so every pre-0.7 campaign is byte-identical. Reserved `DW0141` pre-0.7. | 0.7 |
| `waves[].lane` | `{waypoints[],aggro_radius}` (spec-0016 §6, reserved `DW0141` pre-0.6) — **routed while distant, feral once aggroed**, on vanilla's Raider patrol system (the intended primitive; live-verified 1.21.11, `docs/notes/td-routing-spike.md`). The squad spawns `Patrolling:1b` with one `PatrolLeader:1b` and the **snake_case int-array** `patrol_target:[I;x,y,z]`; a per-wave clock (`lane_tick_<wave>`, 30t, self-terminating) advances a shared waypoint index and per mob releases `Patrolling:0b` whenever a player is inside `aggro_radius`. `aggro_radius` is emitted verbatim as each lane mob's `follow_range` attribute — release radius and perception radius MUST be one number, so a contradicting per-mob override is `DW0381`. Lanes are raider-family only (`DW0382`: vanilla's `#minecraft:raiders` — evoker / illusioner / pillager / ravager / vindicator / witch), squad ≥ 2 (`DW0383`: a lone patroller self-cancels), and a lane pillager must keep its crossbow (`DW0384`: its only attack goal is crossbow-gated, so an otherwise-armed one deadlocks on target acquisition). Declaration errors (no waypoints, an invented waypoint anchor, a repeated consecutive waypoint, `aggro_radius` outside `4..=64`, `lane` + `summon: aggro-edge` together) are `DW0381`; lane geometry is the build-tier `DW0386`. Lane waypoints join the wave's spawn anchor in the layout solver's **required-anchor** set for the wave's area, so a prefab-pool area is guaranteed to draw a piece providing each one — without that a pool draw can legally omit a waypoint's carrier and the lane fails `DW0386` for a reason the author cannot act on. |
| `waves[].summon` | `anchor` (default, the pre-0.6 behaviour) or `aggro-edge` (spec-0016 §6, reserved `DW0141` pre-0.6). **Aggro-edge = spirit-summoned at the edge of perception**: species without patrol AI never march a lane, so each mob instead materializes on the ring at its own `attributes.follow_range` from the wave `anchor` — which in this mode is the **defended point**, not the spawn point. Candidate cells are standable, walk-reachable and in line of sight of that point, on the one-sided band `[follow_range - 2, follow_range - 1]`, ordered outermost-first: one full block INSIDE the mob's own perception, because ladder evidence (the drowned bell, runs 10/12) showed a mob seated exactly AT the radius acquires a defender at the anchor only marginally — vanilla target acquisition at the boundary is a coin flip, and a summoned mob that acquires nobody stands idle forever, timing out its kill objective. Never beyond perception, never on top of the party. `follow_range` is mandatory here (`DW0385`) — the ring radius is authored, never guessed from a vanilla defaults table the compiler cannot verify. A ring with too few valid cells is `DW0387`, not a silent short spawn. |
| `shortcuts[]` | `{id,gate,unlock,on_unlock[]?}` (spec-0016 §2, reserved `DW0141` pre-0.6) — the souls loop-back. **`on_unlock` is effect root R6** (spec-0031): it always was one — a `Vec<QuestEffect>` emission lowers into `shortcut_open_<id>` — but it was outside every enumeration until then, so a `narrate` in it was never l10n-inventoried, a `set-flag` was invisible to the flag model, and a `sequence` would have called a function nothing generated. Deliberately made a root rather than desugared into a trigger the way an `ambush` is: the unlock is polled behind a once-only `#sc_<id>` sentinel and it clears the gate region, so desugaring would have introduced a second detector for one event. **The sealed door is a pressable object:** `setup_finish` arms one `1.02f` interaction per doorway cell, tagged `dw_ws_<id>`, standing in the open air on the **sealed side** (`compiler::wrongside`), and `shortcut_open_<id>` kills them as the bars go up. A `strike`/`use` trigger the author anchors on the `gate` rides those bodies, which is how a wrong-side press gets an answer at all — the compiler supplies the body, the campaign supplies the words. The placement is also the whole side mechanism and needs no player test: a near-side ray reaches a body standing in front of the bars, a far-side ray hits the door and stops. This matters because the answer is typically *"the door cannot be opened from this side"*, which said on the opening side is false, and a false player-facing line is worse than silence. An underivable side is `DW0425`. The `gate` is **sealed from world-load** (the prefab carries the physical fill), and the `unlock` anchor on the FAR side opens it **permanently**. Declaration errors are `DW0371` (malformed/duplicate id, an anchor no prefab provides, or an `unlock` equal to its own `gate`); a gate anchor with no declared fill `block` is `DW0343` (the same rule `close-gate` obeys); a `close-gate` anywhere targeting a shortcut gate is `DW0372` — permanence is structural, there is no re-seal verb to reach for. Geometry proofs: `DW0373` (the long route exists while the gate is sealed) and `DW0374` (opening it strictly shortens the walk to the unlock — the anti-leak proof that makes `unlock` a far-side anchor rather than a label). Every shortcut gate is additionally **sealed for the whole completability model** (`Plan::build` registers it as a `close-gate` at step 0), so `DW0311`/`DW0315`/`DW0342` all prove the delve finishable with no shortcut ever taken. | 0.6 |
| `happening` | `{verb, text, subject?}` (spec-0025, reserved `DW0141` pre-0.8) — what this node does to the story. Declared on a **quest**, an **objective**, a **story-weight dialogue option** (one carrying a `set-flag`), and the **eleven story-node effects** (`spawn-npc`/`despawn-npc`/`move-npc`, `spawn-actor`/`despawn-actor`/`move-actor`/`unleash-actor`, `spawn-wave`, `open-gate`/`close-gate`, `campaign-complete`) — and nowhere else, so a `happening` on a `narrate` is an unknown field (`DW0100`) rather than a beat nobody reads. `verb` is the closed ten-word vocabulary `dies` / `survives` / `departs` / `arrives` / `learns` / `believes` / `gains` / `loses` / `opens` / `seals`; `text` is one line of prose the compiler never interprets; `subject` names an `npc/`, `actor/` or `wave/` id (validated, `DW0112`), an `anchor/`, or an `item/<kebab>` label for a story token the campaign tracks by hand. Required at 0.8.0 (`DW0481`) — the forcing function, generalizing the cast ledger's `doing` from NPC presence to event flow. **Never player-visible**, so it is excluded from the l10n inventory exactly like `doing`, and it is deliberately absent from `QuestEffect`'s hand-written `Debug` — a content key can never move because a beat gained a line of prose. | 0.8 |
| `cast` | `{ "<npc id>": <entry>, … }` (spec-0020, reserved `DW0141` pre-0.7) — the **scene ledger**: for every NPC live during this quest, where they are, what they are doing, and what their right-click offers *for this quest's duration*. An entry is the bare keyword `"dead"`/`"offstage"`, one placement object, or a **list** of placements (per-branch casts, each gated by `requires_flags`/`forbids_flags`). A placement is `{at, doing, dialogue, requires_flags?, forbids_flags?}`: `at` is an anchor or `"offstage"`/`"dead"`; `doing` is free prose the compiler never checks (required anyway — it is the forcing function, and stage 6 writes the NPC's lines against it); `dialogue` is a stage-6 root id, `{"barks": [...]}`, `"none"`, or `"unchanged"`. **The declaration is the gate** — see "Cast-ledger dispatch" in §3. Barks enter the l10n inventory as `cast.<quest>.<npc>.<branch>.bark.<i>`; `doing` deliberately does not (it is never shown to a player). A cast-declared root counts as a **dialogue entry point**, so `DW0120` reachability is measured from the tree `root` plus every ledger root — without that, retiring a premise root by swapping to a later one would make the later one unreachable. Proofs: `DW0460`–`DW0467`, plus `DW0846` and `DW0858`. | 0.7 |
| `triggers[]` | `{id,at?,on:strike\|use\|approach{range}\|strike-npc{npc},requires_flags?,forbids_flags?,once?,effects[]}` (v0.4; `forbids_flags` and `strike-npc` v0.6, reserved `DW0141` earlier). `at` names a **place** and is required for `strike`/`use`/`approach`; `strike-npc` names a **character** and takes no `at` at all — either mismatch is `DW0194`, because an ignored anchor reads as meaningful and does nothing. A `strike-npc` target that stage 2 does not declare is `DW0112` (the trigger's tag would ride nothing). Bad/dup/`range 0` → `DW0194`. A trigger is armed while every `requires_flags` flag is held by some player AND no `forbids_flags` flag is set by anyone — e.g. a retaliation trigger armed by `flag/sealed` that stands down the moment `flag/asleep` is set (the wake beat takes over), with no re-arm plumbing. | 0.4 / forbids 0.6 |
| Effect `open-gate` | Fills gate anchor to air. | 0.1 |
| Effect `close-gate{anchor,sealed_hint?}` | The physical dual of `open-gate` (v0.6): fills the gate anchor's region with the block the anchor's prefab metadata declares (basalt boulder, iron bars), re-sealing an opened threshold into a wall. A gate anchor that declares no `block` is `DW0343`. Same anchor-existence check as `open-gate` (`DW0142`). Per-effect `requires_flags` like the other per-`@s` verbs. **`sealed_hint` (v0.8, reserved `DW0141` pre-0.8)** is what the seal *says* when a player right-clicks it: a seal is a wall the party walks back to and presses, and the press has to answer. **Who owes the answer depends on the declared version.** Below 0.11.0 it is the compiler's: unauthored, the canonical English `The way is sealed.` is baked at emit time exactly as `world.boundary.message` is. At 0.11.0 and above it is the campaign's — a seal nothing answers is `DW0429`, discharged by this field or by a `use` trigger on the gate. Authored, the line is l10n-inventoried under `<effect-key>.sealed_hint` and translates like a `narrate`. The answer belongs to the **anchor**, so two firings on one gate must agree (`DW0423`), and nothing else may hold a hitbox inside the sealed region (`DW0422`). Unlike `happening`, an authored hint prints in `QuestEffect`'s hand-written `Debug` — it changes emission, so it is part of the content key; an unauthored one does not, so no existing `seq_<hash>` moves. | 0.6 (`sealed_hint` 0.8) |
| Effect `campaign-complete` | Sets `dw.campaign`; finale fanfare. `ending` (opt, v0.8, spec-0025, reserved `DW0141` earlier) NAMES this ending — there is no separate `endings` section, the set of endings is exactly the set named here, the same rule flags follow — so a stage-4 branch can declare which ending it runs to and `DW0482` can state *which* ending a branch reached rather than merely that something ended. Validation metadata: never emitted, so a campaign that names none is byte-identical. | 0.1 / `ending` 0.8 |
| Effect `spawn-wave` | Summons wave mobs (AI on), tag `dw_wave_<id>`. | 0.3 |
| Effect `give-item{item,count,name?}` | Grants item (`name` v0.4). | 0.3 |
| Effect `set-flag{flag}` | Sets `dw.f_<flag>` (per-player). | 0.3 |
| Effect `narrate{text,style?,sound?}` | chat/title/subtitle/**art**/**actionbar**; `text` → l10n; `sound` validated (`DW0326`); `art` = the `delve:art` pixel-banner font, glyph-checked (`DW0328`), width-checked (`DW0330`); **`actionbar`** = the reply strip above the hotbar, the channel every compiler-written reply already used, not width-checked (vanilla neither wraps nor truncates it, and a reply is a fragment rather than a banner). | 0.4 / art 0.6 / actionbar 0.11 |
| Trigger `audience: party\|presser` | Who a trigger's bundle addresses. `party` (default, and what every trigger did before 0.11) is polled on the tick with no executor and addresses `@a`. `presser` is dispatched by a `minecraft:player_interacted_with_entity` advancement and runs **as the player who right-clicked**, so `@s` is the presser — which also makes a `player`-scoped `requires_state` legal there (`DW0503` asks the site, not the root class). Vanilla can attribute right-clicks only, so `presser` on a `strike` / `strike-npc` / `approach` is `DW0427`. | 0.11 |
| Effect `set-block{anchor,block}` | `setblock` at anchor; base block id validated (`DW0193`). `block` accepts a verbatim blockstate suffix `id[key=value,…]` (v0.6). | 0.4 / state 0.6 |
| Effect `fill-region{region,block}` | **Fill a declared region with a block at runtime** (spec-0031). `region` is the existing anchor-centred box (`anchor ± extent`, the same `StealthZone` a `begin-stealth` zone, a `damage-players` `in` filter, a `collapse` ceiling and a `lethal_volumes[]` region use, resolved through the one `Plan::zone_box`); `block` is registry-checked with the same `DW0193` `set-block` gets. Emission is one unfiltered `fill <lo> <hi> <block>`. This is the **general spelling** of the capability `open-gate`/`close-gate` carried privately: those two are this operation with the box and the block read off a prefab gate anchor instead of authored, `set-block` is the one-cell case at a point anchor, and `open-way` (v0.12) is the same operation with all three read off a placed piece's contract. All of them lower through `emit::fill_region_command` and are modelled by one completability rule (`plan::RegionEvent`), so a fourth consumer inherits the proof instead of re-deriving it. Completability: from the DAG point the effect fires at, the cells become what the **block** makes them — **solid** for a block that is a full cube, exactly as a `close-gate` seal is (a critical path that must cross afterwards fails `DW0311`), and **flooded** for `minecraft:water` / `minecraft:lava` (`assembled::is_fluid`): impassable, and never floor, because nothing stands on a fluid. A fill carries no `replace` filter, so a fluid fill over floor takes the floor away; a forced leg that needed that footing is `DW0544`. **Whose firing it is also counts**: a fill fired from a root the party can skip — a trap payload, a shop offer, a death bundle, a shortcut's far side — still SEALS the region (the proof must survive it) but does not lay footing the forced path may stand on, because the same block that walls a doorway floors the cell above it and only the first reading is conservative. A forced leg whose only footing comes from such a fill is `DW0546`. | 0.10 |
| Effect `clear-region{region}` | **Clear a declared region to air at runtime** (spec-0031) — the physical dual of `fill-region`, and the general spelling of what `open-gate` does to a gate anchor's box. Emission is the same `fill` with `minecraft:air` and, unlike `open-gate`, **no `replace` filter**: an author's clear empties the box rather than scrubbing one block id out of it. Completability: the cells are **passable** from the DAG point the effect fires at — the half no gate could ever exercise, because the assembled model already holds every gate cell open unconditionally. Two limits are stated rather than discovered. (1) A cleared cell the model already floods stays impassable: clearing a block does not remove water, it lets the water in. A clear that *opens* a dry box into adjacent water is not modelled — re-deriving the flood needs the block map the collision view does not carry — so that campaign's route proof is optimistic there, and a runtime `fill-region` of a fluid is a second way to put water next to such a clear. (2) A clear never removes cells another proof has forced solid (`nav::World::pinned`): a `collapse`'s debris, an ambush's occupied cells, a timed gate's shut span. Clearing a region says the blocks the campaign put there are gone, not that another proof's hazard never happened. | 0.10 |
| Effect `open-way{piece,way}` | **Open a placed piece's contingent way at runtime** (spec-0042). A piece's spatial contract may declare a traversal edge whose crossability depends on a named region — `laid` (empty as built, opening fills it with the way's block) or `cleared` (built in that block, opening voids it). The prefab checker proves, on the bytes as shipped, that the edge is severed and that applying the delta joins it; this verb is what applies it. **It carries no region, no block and no direction**: all three are read from the carrying piece's exported `spatial_contract.edges[].way`, so the effect and the building cannot disagree about what a way is — two authorities plus an equality check is the defect the shape avoids, not a variant of the fix. `piece` is a prefab id and must name exactly one PLACED way: a piece placed twice puts two breaks in the world at different coordinates, and a reference matching none or several is `DW0547`. Emission is one unfiltered `fill <lo> <hi> <block>` per box of the way (air for a `cleared` one), through the same `emit::fill_region_command` `fill-region` / `close-gate` / `set-block` lower through. Completability: the way is **shut until this fires**, and from that DAG point the cells are what the block makes them — the same one `plan::RegionEvent` rule, fed from prefab metadata instead of an authored box, so the forced-footing rule (`DW0546`) applies unchanged rather than being restated. Required content standing beyond a way no forced opening precedes is `DW0548`; every staged way's disposition is enumerated in `validation/ways.json` and a way the placed world declares but cannot stage is `DW0549`. | 0.12 |
| Effect `requires_flags[]` / `forbids_flags[]` (any effect) | Per-effect gates (v0.6): `requires_flags` wraps the effect's command(s) in a per-player `execute if score @s dw.f_<flag> matches 1 … run …`; `forbids_flags` adds `unless score @s dw.f_<flag> matches 1` clauses to the same guard (suppressed once any listed flag is set for the acting player). Valid on any `on_objective_complete` / `on_complete` / trigger effect **except** terminal `campaign-complete`; refs resolve like objective flags (`DW0172`). | 0.6 |
| Effect `despawn-npc{npc}` | Removes NPC + hitbox. | 0.4 |
| Effect `spawn-npc{npc}` | The dual of `despawn-npc` (v0.6): summons a stage-2 NPC — body + interaction hitbox + name display — at its declared anchor, via the **same** `npc_summon_commands` authority world init uses. Idempotent (per-entity tag guards), so a re-fire never doubles a body. Also a dialogue effect. World-global staging → no per-effect `requires_flags`. **`spawn_npc_<id>` is emitted for every NPC any `spawn-npc` site names**, not only `deferred` ones: the registration walk IS the call walk (quest/trigger/trap effect trees at any nesting depth, plus every dialogue option's `spawn-npc`), so a call and its callee can never disagree. It used to be deferred-only, which made `spawn-npc` on a non-deferred NPC — the legal, meaningful way to bring a character back after a `despawn-npc` — compile a call against nothing, so the character stayed gone (found by `DW0497` the day it landed). For an NPC already standing at its mark the entrance is exactly the no-op it reads as; a campaign that fires no `spawn-npc` and defers nobody emits nothing here (byte-identical to pre-0.6). | 0.6 |
| Effect `move-npc{npc,to_anchor,speed?,on_arrive[]?}` | A*-planned per-tick tp through walkable space; unroutable → `DW0307`. `on_arrive[]` (v0.6, reserved `DW0141` earlier) fires once on the driver's final-waypoint tick — **exact parity with `move-actor.on_arrive`**: same arrival detection, same execution context (`mv_arrive_<key>` mirrors `ma_arrive_<key>`), and every deep effect walker (flag/wave producer scans, consumer-ref checks, checkpoint/stealth collector, l10n inventory + localization, nav flattening, emission) recurses into it via the shared `nested_effect_lists` authority. Lets content gate a beat on walk *completion* (`on_arrive` → `set-flag`) instead of fire-and-forgetting the walk. | 0.4 / `on_arrive` 0.6 |
| Effect `cutscene{shots[]}` / `cutscene{path[],seconds,look_at?}` | Two-camera spectator dolly; clip → `DW0308` (checked **per shot**, over both the authored polyline and the client-rendered keyframe chords); a shot panning over the 6°/tick angular budget → `DW0347`. Two mutually exclusive spellings, normalized to one shot list: multi-shot `shots: [{path[],seconds,look_at?}, …]` (v0.6) or the single-shot `path`+`seconds` fields (v0.4) — mixing/omitting both, or a shot with an empty `path`, is `DW0199`. Shots play back-to-back inside ONE save/restore bracket (hard cut). `look_at {anchor,offset?}` aims every dolly camera at that world point; absent = face along the direction of travel. **`shot_style` (v0.6, spec-0015)**: a shot may instead declare a style preset + `subject {anchor|npc|actor, offset?}` (+ optional `dist`, `bearing`, `degrees` (orbit only), `subject_b` (two-shot only)); the compiler expands the style deterministically into the dolly + aim + duration (see "Shot styles" below). Explicit `path`/`look_at`/`seconds` always override the corresponding expanded part. Style-shape violations are `DW0348`; a `side-track`/`low-follow` whose subject has no sibling `move-npc`/`move-actor` (same effect group or sequence) is `DW0349`; an unknown subject npc/actor is `DW0112`. | 0.4 / `look_at`+`shots`+`shot_style` 0.6 |
| Effect `set-time{time}` | Instantaneous dimension-global cut (`time set <kw>`, or `time set <ticks>` for `dusk`/`dawn` — see the stage-1 `time` row); persists (cycle frozen). | 0.5 |
| Effect `set-weather{weather}` | Instantaneous dimension-global cut (`weather <kw>`); persists (cycle frozen). | 0.5 |
| Effect `play-sound{sound,at?,volume?,pitch?}` | Plays a sound event; `sound` validated (`DW0326`); `at` = `{anchor}`\|`players` (default); `{actor}` parses and is refused (`DW0335` — no live-actor position resolves); positional or per-player. | 0.6 |
| Effect `damage-players{amount,in?,damage_type?}` | Deals `amount` half-hearts of damage over vanilla `/damage` — a real `on_caught`/souls consequence. **Audience (spec-0018)**: on a quest beat / trigger the hazard is a fact about the delve, so it hits the whole party (`execute as @a[…] run damage @s …` — `/damage` takes ONE entity, see §1); inside a solo `on_caught`/`on_respawn` bundle it hits exactly that player (`execute if entity @s[…] run damage @s …`). `amount ≥ 40` is lethal through golden apples. `in {anchor,extent}` narrows to acting players inside the anchor-centred box (same box model as a stealth zone; anchor `DW0142`). `damage_type` is a **curated enum** of vanilla types that respect `keepInventory` and do NOT bypass totems (no `out_of_world`/`generic_kill`), default `generic`; an unknown value is `DW0100` (needs no registry). Named `damage_type`, not `type`, since the effect enum is internally tagged on `type`. Per-effect `requires_flags` allowed (per-`@s` verb). Every form is guarded by `tag=!dw_cutscene` — a player watching a cutscene is never harmed (§4). | 0.6 |
| Effect `set-checkpoint{anchor,on_respawn?}` | Party-wide respawn point: `spawnpoint @a` at the anchor + `storage dw:cp pos` mirror + the active-checkpoint marker. Monotonic by quest order. `on_respawn[]` = per-player effects re-run on respawn while active (vanilla `deathCount` detection). A death at the active checkpoint **re-seats** the respawned player on its cell rather than trusting vanilla's respawn lookup, which silently falls back to the world spawn on a cell it dislikes (§emission). Proofs `DW0315`/`DW0316`. Also a dialogue effect. | 0.6 |
| Effect `bonfire{anchor,on_rest?,prompt?,rest_label?,save_label?}` | The souls sibling of `set-checkpoint` (spec-0016 §1). The effect only **arms** a rest affordance at the anchor (a `minecraft:interaction` the party right-clicks; the campfire is prefab dressing) — the respawn point moves when the party actually **rests**. Right-clicking opens a dialog with **exactly two options** (a campfire must be a real interaction, never a lazy "arrive" objective): *rest and save* runs the full loop (the resting player is restored, their flask refilled, the checkpoint moved, every `respawns_on_rest` wave re-seated, `on_rest[]` fired); *save only* moves the checkpoint and does nothing else. Every respawn at this bonfire runs the same `on_rest[]` scene reset and re-seat. Arming is idempotent (guarded summon), resting is deliberately repeatable (unlike the one-shot trap disarm). `prompt` / `rest_label` / `save_label` (v0.8) author the three dialog strings; absent, the compiler bakes its canonical English (`Bonfire` / `Rest and save` / `Save only`) exactly as `world.boundary.message` does — an authored string is inventoried (`fx.….rest_prompt` / `.rest_label` / `.save_label`), translates like any other player-visible line, and the two labels carry the `DW0331` button budget because they are drawn on the same button a dialogue option is. A campaign with a bonfire whose class kits declare no `flask` is `DW0476`. Proofs are inherited: a bonfire is collected as a checkpoint, so `DW0316` (standable) and `DW0315` (no stranding — rooted at the ARMING beat, the earliest rest) apply unchanged. Quest/trigger effect only (not a dialogue effect). | 0.6 / the two-option dialog + authored labels 0.8 |
| Effect `begin-stealth{zones[{anchor,extent}],on_caught?,grace_ticks?}` | Per-tick: every player must be inside some zone — zone presence alone = hidden (no sneak requirement, which collides with the spectator cutscene camera); exposed for `grace_ticks` (default 20) → `on_caught`. Zone standable/reachable proof `DW0327`; onset-survivability proof `DW0355` (a beat whose `on_caught` punishes must be escapable inside `grace_ticks` from where the player provably stands when it arms, and from every checkpoint that can respawn them into it). | 0.6 |
| Effect `end-stealth` | Ends the active stealth beat (clears the session marker). | 0.6 |
| Stage-5 `actors[] {id,entity,name?,skin?,anchor,facing?,vulnerable?,equipment?,attributes?}` | Scripted NoAI/Silent/no-loot puppets, tag `dw_actor_<id>` (+ puppet marker `dw_pup_<id>`); `Invulnerable` unless `vulnerable` (then knockback-immune); `skin` → mannequin, with its PNG baked into the resource pack and its absence refused (`DW0309`) exactly as a stage-2 npc's — a skin is a property of the body, and one walk (`dsl::body_skin_sites`) answers for both classes. Summoned by `spawn-actor`, not at load. `equipment` (spec-0021, reserved `DW0141` pre-0.6) takes **the same shape a wave mob's does** — one type, one rule set, so the two surfaces cannot drift — and is emitted into BOTH the puppet summon and the unleashed twin's NBT: unleashing swaps the body, not the costume, so the dormant elite the party has been circling is visibly the armoured thing that stands up. Every slot at drop chance 0 (no-grind: an actor's kit is never lootable). Unlike the wave path it deliberately does **not** fall back to the armed-mob default table — an actor is a directed set piece and wears exactly what was declared, which is also what keeps every pre-`equipment` campaign byte-identical. `attributes` (reserved `DW0141` pre-0.6) is likewise **the wave mob's v0.4 [`MobAttributes`] shape** — one type, one rule set, one renderer (`emit::attribute_entries`), so the two surfaces cannot drift — and rides both bodies for the same reason gear does: the twin is what actually fights. Before it, an actor was pinned to vanilla base values while every wave mob could be tuned, which is what blocked elite authoring. A `vulnerable` puppet's `knockback_resistance: 1.0` is compiler-owned, not authorable, and is emitted **first** in the list, so the no-`attributes` rendering is unchanged; the twin never inherits it (that is the caged creep's property, not the freed elite's). `drops` (v0.9, reserved `DW0141` pre-0.9) takes the same list a wave mob's does, under the same rules (`DW0490`/`DW0491`/`DW0143`, `name` inventoried as `actor.<actor>.drop.<n>.name`), and rides BOTH bodies for the same reason gear does. What the compiler adds is the removal rule: every removal it performs itself — the `unleash` that kills the cage, a `despawn-actor` of either style, a souls re-seat's re-caging — first strips the declaration off the body (`execute as @e[tag=…] run data merge entity @s {drop_chances:{…0.0f},DeathLootTable:"minecraft:empty"}`), so a declared drop is what a **player's kill** yields and nothing else. | 0.6 / equipment 0.6 / attributes 0.6 / drops 0.9 |
| `actors[].tier` | `ordinary` (default) \| `elite` \| `boss` — the SAME [`EncounterTier`] vocabulary `waves[].tier` uses, on the other shape an elite takes (spec-0023). A wave is not the only way to build a hard fight: the set-piece souls encounter — the armoured thing kneeling among the graves that stands up when you strike it — is an **actor**, staged by `spawn-actor`, given AI by `unleash-actor`, killed by hand rather than by a `kill` objective, and it was therefore *structurally invisible* to the validation ladder's inverted floor gate (which only ever read `waves[].tier`), so an empty finding list read as a pass over a fight nobody had. Same contract as the wave field: a declaration, never a knob — emission is byte-identical whichever tier an actor carries, and nothing about the puppet or the twin changes. A tiered actor enters `validation/combat-plan.json`'s `actors[]` with the anchor to walk to, the `dw_actor_<id>` tag its body wears, the beats that spawn and unleash it (trigger id, event kind, watched anchor / struck NPC) and its declared `attributes`; whether the floor gate can measure it, and the reason when it cannot, is stated per actor and in the plan's `floor_gate` ledger (`DW0477`). Absent ⇒ `ordinary` and omitted from serialisation. Reserved `DW0141` pre-0.8. | 0.8 |
| `actors[].traversal{locomotion}` (opt) | **What this body can do when it moves** (spec-0034; reserved `DW0141` pre-0.11, fenced on THIS stage's own `dsl_version`). The author's side of the traversal proof: by default the compiler derives locomotion from the entity id, and this overrides it for one body. **One shared type on every object class that has a body, a position and a compiler-emitted route** — the stage-2 NPC and the stage-5 actor (`dsl::body_traversal_sites`, a closed sum type, so a third body class is a compile error at every consumer until it is handled) — because traversal belongs to the body, not to the verb that first needed it. `ground|climber|flier`; `aquatic` is refused (`DW0455`). **A declaration is a claim, not an opt-out**: it must change a rule's verdict or the build fails (`DW0454`), and it can never reach the error tier (`opens_gates` is derived and unauthorable, and no class is exempt from `DW0452`). It changes which rules examine the body and nothing else — routing is unchanged, every body walks the same ground A*. Emission is byte-identical whatever is declared. | 0.11 |
| Effect `spawn-actor{actor}` | Idempotent puppet summon at the actor's anchor. | 0.6 |
| Effect `despawn-actor{actor,style}` | `kill` = vanilla death animation in place; `vanish` = relocate-then-kill (silent, out of view). Targets `dw_actor_<id>` (puppet or twin). **Per-actor drop (round-8, live-observed):** `vanish` emits `execute as @e[tag=dw_actor_<id>] at @s run tp @s ~ -128 ~`, not `tp @e[…] ~ -128 ~` — the bare form resolves `~ ~` against the **command source**, and every path that reaches a `despawn-actor` (a `move-actor`'s `on_arrive`, a `sequence` step, a trigger bundle) runs from the server source at world spawn, so the island's herdsman standing at `6.5,-55.5` died at `10.0,-128.0,9.0`. Masked by the `kill` on the next line, but wrong data. | 0.6 |
| Effect `move-actor{actor,to_anchor,speed?,on_arrive[]}` | Footprint-aware A*-planned per-tick tp of the puppet, yaw along the path tangent (§4 "A walked body faces where it is walking"); `on_arrive` fires at the destination cell; unroutable → `DW0325`. `move-npc` is a thin wrapper over the same planner (player footprint). **Chained origins (round-6, live-server proven):** an actor's (and NPC's) successive moves chain — the first leg plans from the declared anchor, every later leg from the previous leg's target. Planning every leg from the declared anchor degenerated a second consecutive move (island: mouth→fire-pit at t=260, whose declared anchor IS fire-pit; t=260 is the round-6 authoring — the shipped campaign now fires that leg at t=420) into a single-waypoint instant teleport — the giant snapped instead of walking on camera. Two moves sharing `(id, to_anchor)` still share one content-keyed driver, planned from the first occurrence's origin (documented limitation of the content key). **Handoff PackTest (round-6):** for the first `move-actor` whose `on_arrive` fires a `spawn-npc` (the walker→NPC scene handoff), a generated `v06_arrive_handoff` template seals every campaign gate (`close-gate` fill), drives the arrival tick, and asserts puppet gone / NPC body present / exactly one NPC hitbox — the beat a delve soft-locks on if the handoff half-fires; gates are re-opened and entities cleared afterwards (batch model). **Concurrent moves are independent (round-8):** each `(actor, to_anchor)` gets its own start function, per-tick driver, run latch `#arun_<bare>` and step counter `#at_<bare>`, and each driver teleports only its own `dw_pup_<id>` — so N moves in flight at once cannot starve one another whatever order they start in (the island cinematic runs four sheep plus the giant). Pinned by `concurrent_move_actors_share_no_state`. The owner's round-8 report of a scheduled move that appeared not to run was chased on a live server with and without a player joined, on clean and stale scoreboards: the driver ran correctly every time (`#at` 0→288 monotonic, latch set then cleared, puppet at the destination cell), so no engine change was made for it; the beat is invisible from the player's seat for a content reason (the walk is off-camera for its whole duration and the arrival lands after the `close-gate` seal). **Overlapping legs on ONE puppet supersede:** concurrency across DIFFERENT puppets is independence (above); two legs for the SAME puppet is a contest, and the later one wins — see §4 "One body, one live walk driver". A puppet with only one planned leg carries none of that machinery (byte-identical). | 0.6 |
| Effect `unleash-actor{actor}` | Replaces the puppet with a real-AI twin (same entity/pos/name/tag, no puppet marker). Re-caging = `despawn-actor` + `spawn-actor`. **Spawn finalization (round-8, live-proven):** `/summon <entity> <pos> <nbt>` — *any* NBT compound, even `{}` — makes vanilla skip `finalizeSpawn`; `/summon <entity> <pos>` does not. The compiler always passes NBT (tags are how it addresses everything it owns), so every mob it summons is un-finalized. For `minecraft:warden` that is fatal: `finalizeSpawn` is the only place the `minecraft:dig_cooldown` brain memory is seeded, and a warden without it enters the DIG activity on its first AI tick, burrows, and despawns ~5 s later (the owner's round-8 report: strike the sleeping giant, watch the warden dig itself back into the ground). A/B on the pinned server: bare summon → `Brain{memories:{"minecraft:dig_cooldown":{value:{},ttl:1200L}}}`; summon with `{}` → `Brain{memories:{}}`, gone. The twin summon now carries that memory verbatim (vanilla's own 1200-tick value — the awake warden refreshes it itself, verified present and roaming past 80 s). Only the **twin** needs it: a caged puppet is `NoAI` and never runs `customServerAiStep`, which is why a puppet warden can stand in a meadow indefinitely. Species needing no finalization data are unchanged (byte-identical). **Aggro lock:** an unleashed hostile targets the player who *struck* the trigger. The click trigger parks that player's UUID in `storage dw:strike player` (`data modify … set from entity <hitbox> attack.player` — vanilla's own record of who clicked) for the length of its own bundle and removes it after, so it can never go stale; `unleash_<id>` seeds the warden's vanilla `anger.suspects` from it at max anger (150), guarded on the storage holding a value. Live end-to-end: the warden left its spawn cell, closed on the seeded player and killed that player. Emitted only for a campaign whose click triggers actually unleash, so other campaigns' unleash functions are unchanged. **Limit:** the warden is the only species with a data-settable target that survives a tick on 1.21.11 — the `NeutralMob` pair (`AngerTime`/`AngryAt`) was tried against endermen, piglins, wolves and iron golems, with a real online player's UUID, and neither field reads back afterwards, so nothing is emitted for them and they acquire targets by vanilla's own nearest-player search. | 0.6 |
| Effect `sequence{steps[]{at_ticks,effects[]}}` | Deterministic timeline: one schedule chain firing effect groups at exact tick offsets. No nested `sequence` → `DW0329`. Effects nested in a step are **first-class**: the flag/wave producer scans, the checkpoint/stealth collector, the l10n inventory, and emission all descend into `sequence.steps` and every nested effect list (`on_respawn`/`on_caught`/`on_arrive`) via one shared traversal, so a `set-flag`/`set-checkpoint` nested in a step produces its flag / registers its indexed checkpoint exactly as at top level. A sequence is a **global timeline**: every step function (the inline `at_ticks: 0` one included) is emitted server-source-safe, so its per-player beats address the party rather than one acting player — §4 "A scheduled bundle has no `@s`". | 0.6 |
| `traps[]` | spec-0011 + **spec-0022**: `{id,at,trigger,effect?,payload?,lethality?,disarm?,reset?,requires_flags?,forbids_flags?}`. **Redstone keeps exactly one job — the trigger**; the consequence is commands (spec-0022). `payload` is an ordered effect list in the SAME vocabulary quests use, plus the two trap verbs `volley` and `collapse` (see below); it is what a trap's consequence should be authored as now. `effect` (the spec-0011 `dispense` wiring) stays valid and unchanged so existing campaigns build byte-identically, but is superseded — a trap must declare at least one of the two (`DW0440`). `at` binds an `anchor/trap` prefab marker (the trigger/hazard cell; its `dispenser` metadata cell holds the payload socket). `trigger` ∈ `pressure-plate`/`tripwire`/`trapped-chest` (all redstone-native; `trapped-chest` = the only player-distinct trigger). `effect` = `{dispense:{item,count}}` (item `DW0341`; a non-`dispense` key e.g. `tnt` is an unknown variant → `DW0100`). `lethality` ∈ `lethal`/`harmful`(default)/`nonlethal`. `disarm{via,sets_flag}` = a reachable affordance that turns the trap off. `reset` ∈ `once`/`rearm`(default). Structural errors `DW0340`; a lethal forced-path trap without discharge `DW0342`. `requires_flags`/`forbids_flags` are a **physical** gate (see §4 emission): the trigger block is removed from the world while the gate is shut and restored verbatim when it opens, so a gated trap is genuinely inert rather than nominally so — the trigger must be a plate/tripwire declaring `trigger_block` in its prefab metadata, else `DW0363`. Reserved (`DW0141`) before 0.6. | 0.6 |

Dialogue effects `set-flag` (v0.4), `set-time`/`set-weather` (v0.5),
`set-checkpoint`/`spawn-npc` (v0.6) and option `requires_flags` mirror the quest
forms. A dialogue **option** may carry a `happening` (v0.8), and must when it
sets a flag (`DW0481`) — a choice that forks the world is a story node.
Per-effect `requires_flags` is a v0.6 **quests-stage** surface only (dialogue
effects are not mirrored — a dialogue option's own `requires_flags` already gates
its whole effect bundle). Newer surface declared under an older `dsl_version` is
reserved → `DW0141`; the version each construct is gated at is the "Since" column
of the tables above, and the **one enumerated list** of every reserved construct
is the `DW0141` row of the diagnostics catalog (§5) — it is not restated here,
because two copies of that list is exactly how the two drifted apart.
The blockstate suffix on `set-block`/`prop` blocks is a lenient parse of an
existing field, not version-gated: the base id is registry-checked and the `[…]`
string is passed to `setblock` verbatim (vanilla validates the property
names/values); a malformed suffix (unbalanced `[]`, empty, non-`key=value`)
reuses `DW0193`.

### Stage 6 — `dialogue`

Exactly one tree per stage-2 NPC (`DW0152`/`DW0153`). Nodes reachable from `root`
(`DW0120`/`DW0121`); `complete-objective` effects target a `talk-to` on the same
NPC (`DW0122`); every `talk-to` has ≥1 reachable (`DW0123`) and ≥1 **ungated**
(`DW0191`) completing option — where "gated" means `requires_flags` OR (v0.6)
`forbids_flags`: either kind of flag gate can make the option unavailable exactly
when it is needed, and the static analysis does no temporal reasoning about which
flags end up set. Node `text` → l10n `dlg.<n>.<node>.text`, option
labels → `.opt.<i>.label`. An option label is a **button caption**: it is drawn on a
fixed 150-GUI-px dialog button and scrolls if it does not fit, so every label —
source and translation — is width-checked (`DW0331`, error).

**Option `tooltip` (v0.8, reserved `DW0141` pre-0.8) —
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
codec accepts the field: `class_select` has shipped each class's `blurb` in exactly
this slot since v0.1, and tier 2 boots it on the pinned vanilla server every PR.

**Display gating (v0.4+):** an option is
*shown* only when clicking it would fire — every `requires_flags` set and no
`forbids_flags` set (flag axes; the click handler mirrors both with fail-fast
guards, so a direct `/trigger` cannot bypass them) and every completed objective
active, i.e. `dw.qa_<quest>==1` and
`dw.o_<obj>!=1` (objective-state axis) — so `DW0191`'s ungated completing option
is visible exactly while its objective is active (the guarantee holds
automatically).

### The map pipeline — `detail-plan` (optional; v0.15, spec-0050)

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
a **site-plan revision**, which moves the plan hash, which re-opens the walk gate
(`DW0841`), which re-runs the whole's walk; a part that wants different
*traversal* revises the **layout graph** and pays the identical cost through the
other half of the walk record's freshness key. The cost is stated, not hidden.

**The frame** a piece must exactly fill is the box's play space grown one course
downward — the walk plane's own floor. Everything else the derivation writes
around a box is structure and stays whole-owned: every vertical party plane,
every wall, every unshared shell face, every seam frame in a vertical plane,
every derived stair in an unbound host and every bar in a vertical-plane seam.
Where boxes stack, the horizontal party plane **is** the upper box's floor
course and belongs to the upper piece — a seam frame lying in that course goes
with it. In the derivation this is one rule rather than six exemptions: a
bound frame is a hole in what the whole writes, so the floor accent, the interior
clear, the ceiling of the box underneath, a hosted stair and a bar in the box's
own floor course all stop by the same subtraction, and everything outside the
frame is written exactly as before.

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

**`walk-record.json`** is a campaign artifact, not a stage document — no
`dsl_version`, no `campaign_id`, no `stage`, because it records an event rather
than being authored against a schema. Its form is
`{site_plan_sha256, layout_graph_sha256, blockout_sha256, engine_revision,
verdict, findings[]}`, and every build of a site-plan campaign prints all three
hashes with the engine's **revision** beside them, so a record can name its
subject and its instrument literally. It is hand-authored and it is refused when
it is wrong, so it is schema-exportable like everything else a person writes:
`delvec schema --stage walk-record`, derived from the same struct `DW0841`
parses. Not being a stage document decides what the document CONTAINS; it never
decided whether its form is machine-readable.

**The engine revision** is stamped into the binary at compile time by
`crates/compiler/build.rs`. A source build reads it out of the checkout it is
built from — suffixed `-dirty` when that tree carries uncommitted changes, since
a build behind an uncommitted edit is not a build of that revision. A release
recipe or container build that has the revision and no `.git` passes
`DELVEC_ENGINE_REVISION` in the environment and that wins unchanged. Where
neither can be established — a source tarball such as crates.io serves — the
engine prints `unstamped` rather than claiming a revision it does not have. The
stamp reaches stderr and diagnostic text only and no emitted byte, so two
binaries differing only in it compile a campaign to identical output. The first two are its **freshness key**:
the whole a walk judges is derived from both authored documents, so an edit to
either re-opens the gate. The third is the derived massing, which is what the
drift advisory reads. It is not a build input: a re-recorded walk moves no
emitted byte.

### Stage 7 — `world-edits` (optional; v0.6, spec-0017)

The map editor's edit script (`world-edits.json`), the artifact of record for
L3 world detailing. **Optional**: absent = no edit stage, and the build is
byte-identical to pre-stage-7. Replayed deterministically by the compiler
after world assembly (§1 pass 8); editing sessions leave no state outside the
script. `note` fields are authoring context — machine-ignored and **excluded**
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
| `fragment` | Stamp a **library prefab**'s non-air cells at a frame-resolved `at` (+ optional quarter-turn `rotation`) — semantically a `/place template` whose bytes the compiler models (non-air overwrites; authored air never erases). Only admitted library prefabs can be stamped, so provenance/license ride the prefab's own metadata (ADR-0013); an id outside the library is `DW0323`. Stamped cells keep their **full blockstate** (`assembled::structure_cells_stateful`; properties in sorted key order): the stamp's writes ARE the runtime `setblock` lines, and reading bare ids turned an authored `lantern[hanging=true]` into a floor lantern in mid-air (found by `DW0354`). **`rotation` turns POSITIONS only, and the compiler REFUSES rather than warns.** There is no rotate-aware blockstate rewriter, so a quarter-turned stamp would keep every `facing`/`axis`/`shape`/connection value unrotated and ship visibly deformed geometry — the silently-deformed-map class. A `rotation` other than `none` on a prefab carrying any **yaw-dependent** property (`facing` except `up`/`down`, `axis` except `y`, `shape`, `rotation`, `orientation`, `hinge`, `north`/`south`/`east`/`west`) is a build error (`DW0323`) naming the block, its prefab-local cell and the offending property; the prescription is to stamp unrotated or admit a pre-rotated prefab variant, never to hand-fix facings downstream. It is a **collision test, not a blanket ban**: prefabs whose every state is yaw-invariant (`hanging`, `half`, `waterlogged`, `open`, `lit`, `type`, `level`, `thickness`, `vertical_direction`, `axis=y`, `facing=up|down`) rotate correctly and stay allowed — a stone box with a hanging lantern and an upright log stamps fine at every quarter-turn. **No piece in the shipped library is one**, and that is a property of the library rather than of the rule: every emitted state names the connections it joins, so each carries `north`/`south`/`east`/`west` (or a `facing`, or an `axis`) that a quarter-turn moves. The accepting half of this test is therefore exercised against a piece built for it, not borrowed from the library — a check whose "provably-correct output is accepted" half has no case left is a blanket ban nobody can tell apart from a rule. |
| `relight` | Run the spec-0010 fixture-placement pass over ONE region and **bake** the fixtures into the edit script's writes — authorial control of where fixtures land (the whole-area relight still re-proves after every batch). Fixture/target default to the area's declared `lighting`; `fixture` + `min_light` (1..=14) override, and are **required** when the area declares none (`DW0162`). An unlightable region is the area pass's own `DW0211`, batch-attributed; a region with no reachable walkable cell is `DW0323`. |
| L2 massing verbs | `swap-piece` (replace a piece with a library prefab that re-mates every mated socket at its exact world pose, any rotation, overlap-checked), `insert-piece` (attach at a specific **unmated** socket — the targeted form of the solver's frontier attach), `remove-piece` (a **leaf** only — exactly one mated socket, never the entry; the neighbour's socket unmates and re-seals), `rewire-socket` (`sealed` **unmates the doorway pair** — a graph operation: both planes wall up and the DW0306 connectivity proof loses the edge; `open` clears an unmated socket's fill — deliberately without granting the proof an edge, conservative), `reseed-piece` (seeded weighted re-pick among the area pool's compatible members, current excluded — a reseed always changes the piece or errors). All carry the `piece` index + `prefab` drift guard. Applied at **plan** time (`compiler::massing`, inside `Plan::build` right after `solve_area`): seals are regenerated from the massaged mated flags (`seal_layout`), and anchors, gate reachability, waterline, assembly, relight, nav and the L3 replay all run over the massaged layout — the full assembly validation re-runs by construction. Massing verbs live in **massing-only** batches ordered before every detailing batch (`DW0162`); an inapplicable verb is `DW0324`. `resize-piece` from the spec's initial list is **excluded**: the library has no size-parameterized piece primitive to express it through (no-hack doctrine); `swap-piece` covers the different-sized-variant case. |
| Seeding | Every seeded verb streams from `stream_seed(campaign_seed, "edits/<batch-id>/<edit-index>")` — renaming a batch (or moving an edit) deliberately reseeds it; nothing else does (ADR-0006). |
| Emission | The replay lowers to a `world_edits` function (x-run-coalesced `fill`/`setblock`), called from `setup_finish` after the socket seals and before the relight fixtures — the exact model order, and the reason `DW0352` exists (`trap_setup` runs later). `setup` additionally forceloads every batch's write AABB (an edit may write outside the piece bboxes — a leaning canopy, a stamped fragment — and a `setblock` on an unloaded chunk silently fails); those chunks then follow the **forceload lifecycle** below. `world-edits.json` is hashed into `manifest.json` inputs. |

### The map pipeline — `geometry-brief` and `layout-graph` (optional; v0.13, spec-0049)

Two documents that state a campaign's **space before any coordinate exists**.
Both are optional files in the campaign directory, named rather than numbered
into the 1–7 sequence: they belong to a different pipeline and a number would
assert an ordering between the two that does not exist. A campaign that ships
neither parses, validates and emits exactly as it did — there is no new field on
any existing type, so nothing older is judged against them. `delvec schema
--stage geometry-brief` and `--stage layout-graph` export them; `--stage all`
includes both.

`geometry-brief.json` is the whole map's written brief reduced to numbers:

| Element | Behavior |
|---------|----------|
| `facts[]` | `{id: fact/<kebab>, value, unit?, note}`. A number with a name, taken from the brief's own prose. Ids are unique (`DW0111`) and well-formed (`DW0110`). Nothing reads a fact at this version — the site plan's identity checks are the consumer — and the binding line states the count so the absence is a number rather than a silence. |

`layout-graph.json` states the campaign's space as a graph:

| Element | Behavior |
|---------|----------|
| `nodes[]` | `{id: node/<kebab>, intent, size_class? \| way_class?, note?, stations?}`. A **place**: a room, a courtyard, an arena, a stretch of shore, a cavern, a road. `intent` is a free non-empty label no check keys on — recorded judgement for the reviewer and for the later per-place brief, kept free-form because an enum of intents would be one genre wearing a schema's clothes; empty is `DW0814`. A place is classified **exactly once**, by one of the two vocabularies below; both or neither is `DW0875`. A name the metrics table does not define is `DW0812` for either kind, because `Metrics::resolve` is the one path from an authored name to an entry and the question is the same question. |
| `edges[]` | A connection, internally tagged on `class`: `walk`, `stair`, `drop`, `barred`, `vision`. All carry `{id: edge/<kebab>, a, b}`. `walk`/`stair`/`barred` carry `one_way` (`a-to-b` \| `b-to-a`; absent = both ways); a `drop` is one-way by construction and so carries a **required** `falls` instead. `barred` carries `opens_from` (`a` \| `b` \| `either`, default `either`) — the one-side-openable door, spelled as a property of the connection rather than of any campaign's fiction — and a **required** `gating`. `vision` carries a line of sight and no body, so it has no direction, no gating and no shortcut mark; stage 4 gives it a sightline rather than a seam. Because the class is the serde tag, a field the class does not read (an `opens_from` on a walk, a `drop` with no `falls`) is an ordinary `DW0100` and no rule has to police it. |
| `edges[].gating` | `{flags[]?, quest?}` — what a body must already hold to pass. **Deliberately not the campaign's `Gate`.** A gate is a runtime object emission evaluates against an acting player; a layout-graph edge is evaluated by nothing at run time, so making it a gate consumer would push a never-emitted object into machinery whose whole subject is emission. It is also narrower on purpose: the closure below is monotone, so a negative flag term and a numeric comparison are terms no proof here could honour, and a surface an author may write and nothing honours is worse than one that is absent. What it states is a **projection** of the campaign's runtime gating into topology, and `DW0818` keeps it a projection — every flag it names must be one the campaign really produces, and every quest must exist. |
| `entry` / `goal` | Node ids. Every proof over the graph starts or ends at one, so a name nothing defines is `DW0814`. |
| `critical_path[]` | An authored node sequence from `entry` to `goal`. **Authored rather than derived**, so that it is a claim the machine verifies (`DW0817`) rather than an answer with no author to disagree with. |
| `beats[]` | `{quest, objective, node}` — where each quest beat happens. **Every objective is place-bound**: a body has to be standing somewhere to talk, to reach, to fight or to take, so every objective in the quest documents binds to exactly one node, and one that does not is `DW0818`. |
| Reachability | Judged under a **monotone closure**: from `entry` holding nothing, mark every edge whose gating the obtained set satisfies, mark every node reachable over those edges respecting one-way direction, add what every beat bound to a reached node grants, iterate to fixpoint. A beat grants the flags the campaign sets when that objective completes and — for a `talk-to` — everything reachable in the spoken-to NPC's dialogue tree, because the conversation happens where the speaker stands; a quest grants itself and its `on_complete` flags once every one of its beats is somewhere a body can stand. The closure is **optimistic in every direction it cannot decide**, which is one property rather than a list of exceptions: it is branch-blind, so a campaign whose branch points set mutually exclusive flags can reach a node no single playthrough reaches. That can only under-report at graph time; the branch-aware battery over the assembled world is what stops it shipping. |
| `nodes[].stations[]` | `{anchor: anchor/<kebab>, kind, note?}` (v0.18, spec-0052) — **the named places INSIDE a place**. A station is a name and a shape, never a position: there is no coordinate, offset or hint field, and that absence is the design. Declared names join the campaign's anchor vocabulary at the same authority as every synthesized one, so a quest can name the fire pit in the camp rather than flattening it onto the camp's box centre. `note` is recorded judgement for the reviewer that no check keys on, exactly as `intent` is. Refusals: `DW0869` (a name in the engine's derived namespace), `DW0870` (two claims on one name, scoped to the area), `DW0871` (a reference demanding a shape the station is not), and the per-stage fence below `0.18.0`. A station no quest references is **legal** — that is the mid-authoring state, and the binding line counts it. |
| `nodes[].size_class` | A rung of the metrics table's ladder (`alcove`, `room`, `hall`, `arena`, `expanse`). Bounds the footprint on **both** horizontal axes and carries a nominal traverse the pacing projection sums. This is the vocabulary a place that is a BOX is stated in. |
| `nodes[].way_class` | (v0.19, spec-0053) A **way**: a place whose footprint is bounded in one axis and free in the other — a road, a causeway, a corridor, a duct. Names an entry of the metrics table's way vocabulary (`corridor`, `road`). It classifies the **cross-section** and nothing else, and there is no length standard here: a route's length is per-campaign geometry, so the site plan states the run by putting the box where it put it, `DW0832` demands only that the run EXCEED the class's widest cross-section, and the pacing measurement reads it. **Why it is a second kind and not another rung**: for a rung to admit a cut ledge one body wide climbing a whole seaward face, that rung would have to span 4..90 on an axis, and a class in which an alcove and an expanse are the same thing has stopped classifying. The failure is by kind, not by margin, so no calibration of the ladder reaches it. Refusals: `DW0875` (classified twice or not at all), `DW0812` (an unknown name), `DW0832`'s way branch, and the per-stage fence below `0.19.0`. |
| `stations[].kind` | `point` or `gate`. **The shape, never the purpose**: a bonfire, a camera subject and a shop counter are the same `point` to every check, for the same reason `intent` is free-form. `point` is a cell a body is put at; `gate` is a region that seals and clears, which is what `open-gate`, `close-gate`, a `shortcut` and a `timed-gate` address. spec-0052 §3 describes a third, `region`, and it is deliberately not built: this engine resolves an anchor to a point or a gate and to nothing else, every volume-shaped consumer is an anchor-centred box on a **point** plus an extent, and a bare `region` anchor is read as a gate filled with `minecraft:air`. A third variant would be declarable, bindable and consumable by no reference site. spec-0052 §11's falsifier decides it: the first campaign brief that cannot state its place without one. |
| A station while its place is **massed** | The derivation realizes every station of every box at a stand-in, from the same authority validation resolved the name against — so a name that validates cannot fail to exist in the built world, massed or detailed. A point lands on its own standable cell (the first not already taken, ordered by Chebyshev distance from the floor centre then lexicographically — the order `footing` searches by); a gate lands on a minimal region of the derivation's own bar, **written into the mass** so the world-load seal measures it shut rather than taking the anchor's word. The author cannot state where a stand-in goes. |
| A station once its place is **bound** | It joins the node's owed set beside its `anchor/node-…`, its `spawn` when it is the entry, and its `anchor/unlock-…` sides, and the `detail-plan` `anchors` map must bind it to an anchor of the piece. The map's shape and its gate are unchanged — it still refuses every key outside the owed set, so a binding cannot invent vocabulary and a typo cannot pass as intent. The bound anchor's **shape** is checked against the station's `kind` (`DW0842`), so the kind validation read off the graph and the kind the built world has cannot drift. Two owed names bound to one piece anchor is legal: one spot may carry two roles. |
| Binding | Every run that carries either document prints one line: places, connections (traversal, one-way, shortcut, gated), **stations and how many of them are gates**, beats and **how many of them are on the mandatory quest spine**, critical-path steps, metrics references and brief facts. Three zeroes are called out as findings rather than counted: a graph with no traversal connection (a set of places with no space between them), a graph none of whose beats belongs to a quest the finale depends on (a critical path over an unbound graph), and a brief with no fact. A station count of zero is stated rather than omitted — a campaign naming its places at node granularity is a fact about that campaign, not a silence. |

### The map pipeline — `site-plan` (optional; v0.14, spec-0049)

`site-plan.json` is the **geometric embedding** of the layout graph, and the
whole map's design of record. Optional, named rather than numbered, and reached
only through itself: a campaign that ships none parses, validates and emits
exactly as it did. `delvec schema --stage site-plan` exports it; `--stage all`
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
of a place: the cells a body can be in. The shell the blockout builds is not
inside it — it stands in the one cell between two neighbours, and on the course
under the floor and over the ceiling. So `extent` is the interior footprint the
size-class ladder judges directly, and two connected places sit **exactly one
cell apart** on the face they share, that cell being the wall they have in
common.

| Element | Behavior |
|---------|----------|
| `region` | `{min: [x,y,z], extent: [dx,dy,dz]}` in world coordinates — the whole map's one region, and the number the brief hands down. **Required, with no derived spelling**: there is no "compute this from the boxes", so extent-flows-up is unrepresentable rather than forbidden, and `DW0826` refuses a box that does not fit while naming the box. Extents are `NonZeroU32`, so a zero-volume region is a schema failure (`DW0100`) rather than a rule some check has to remember. The water plane is deliberately absent — `horizon: ocean` in the stage-1 world document already fixes sea level. |
| `datums[]` | `{id: datum/<kebab>, y, note?}` — named ground planes. Ids are the ordinary `DW0110`/`DW0111`; a `floor` naming an undeclared one is the ordinary `DW0112`. |
| `boxes[]` | `{node, min: [x,z], extent: [dx,dz], floor, ceiling}` — **exactly one per graph node** (`DW0824`). A box is a footprint standing on a plane, not a prism: its vertical position is `floor` alone, so the plane has one authority instead of a `y` inside `min` beside a declared floor with no rule about which the derivation believes. `floor` is `{"datum": <id>}` or `{"y": <n>}`; `ceiling` is `{"clearance": <cells>}` or `"open"`. Horizontal extents are on the kit grid (`DW0825`) and inside the region (`DW0826`); boxes are disjoint (`DW0827`) and built to their place's size class (`DW0832`). |
| `ceiling: "open"` | A sky-open place — a courtyard, a shore, a summit. It claims the ground and its size class's own minimum headroom and **nothing above that**, which is what makes a `clearance` volume over a courtyard the whole reserving sky rather than two authorities over one cell. |
| `seams[]` | `{edge, face, at: [u,v], opening? \| contact?, stair_in?}` — **exactly one per traversal edge** (`DW0824`). `face` is one of the engine's six face names (`east`/`west`/`up`/`down`/`south`/`north`), **of the edge's `a` box**; `at` is the crossing's low corner in world coordinates on that face's own two in-plane axes — `[along, y]` on a vertical face (the sill is the second), `[x, z]` on a horizontal one. A seam allocates **one of two kinds** of connection, and both or neither is `DW0876`. `stair_in` names which of the two boxes hosts the treads: required on a `stair` (`DW0830`) and refused on anything else (`DW0824`). |
| `seams[].opening` | **A PORTAL**: a named standard from the metrics table (`DW0812` on an unknown name, `DW0829` on one that does not fit or whose sill cannot be reached). A body crosses at exactly the cells `at` and the standard allocate, and **every one of them must be passable** over the built bytes (`DW0836`). |
| `seams[].contact` | (v0.19, spec-0053) **A CONTACT**: the two places simply meet along a front, rather than through a doorway. `{extent?: [u,v]}` — the span in cells on the face's own two in-plane axes, anchored at `at`; omitted, it runs from `at` to the far edge of the shared face, which is how a front along the whole of a face is written. **What it means**: the boundary is continuous ground — the derivation writes **no wall along the span**, and wall as ever outside it, and no frame ring, because a ring around a fifty-five-cell front is a wall drawn in a second block. **What the proof reads**: the author allocates *where* the places meet and the engine measures the crossing profile from assembled bytes, so *"this face is fine"* is never a declaration this engine accepts. Seams stay allocated, never discovered: the span is the edge's allocation set for `DW0838`, so a crossing outside it is still a refusal. **No door check applies** — a contact has no opening name to resolve and no single sill, and calling a wide front a door would make every downstream door check wrong. A contact carries `walk` or `drop` only; `stair`, `barred` and `vision` are excluded until a campaign brief demands one. Its span must be **wider than the broadest standard opening**, derived from the table rather than seeded: anything narrower could have been a portal. Refusals: `DW0876`, `DW0877`, and the per-stage fence below `0.19.0`. |
| A seam's **rise** | **Derived, never authored.** It is `floor(b) − floor(a)`, which the plan has already stated by putting the two places where it put them. Authoring it would be authoring arithmetic — unlike `critical_path`, which is authored precisely because it is a *choice* among many — and a second declaration of it could only agree or be a refusal teaching nothing the datums did not already say. `DW0830` and `DW0831` judge the derived number. |
| `volumes[]` | `{id: volume/<kebab>, region, role, note?}` — the mass the WHOLE owns: `massif` (the mountain a cave system is inside), `ground` (the plane under a village), `clearance` (the sky a silhouette needs kept empty). They stand beside places, under them and over them, never inside one (`DW0835`), and they answer to the region like anything else the plan places (`DW0826`). |
| `identities[]` | `{fact, measure, cmp}` — guarded comparisons binding the plan to the geometry brief's written numbers. `cmp` is `eq`/`lt`/`le`/`gt`/`ge`. `measure` is a tagged union over a **small fixed vocabulary**, not a parsed string: `{"of":"region-extent","axis":x\|y\|z}`, `{"of":"box-extent","node":…,"axis":x\|z}`, `{"of":"box-height","node":…}`, `{"of":"distance-xz","from":…,"to":…}` (Euclidean between footprint centres), `{"of":"datum-y","datum":…}`. An unknown measure is an ordinary `DW0100` and a node it names is checked like any other reference. **Marked judgement**: the vocabulary will grow, and the falsifier is the first brief fact a campaign cannot bind with it — at which point the missing measure is added as a variant, never worked around by binding a different fact. |
| `sightlines[]` | `{edge, from, to}` — **one per `vision` edge** (`DW0824`), the segment the stage-5 battery walks. A vision edge carries a sightline rather than a seam because a vista's two ends are routinely not adjacent — a tower seen from a shore shares no face with it — so the seam construct cannot state the one thing it asserts. Each end must lie inside the place its connection names (`DW0824`): the proof walks exactly this segment, so ends elsewhere would prove a different claim, green or red. |
| `views[]` | `{id: view/<kebab>, eye, look_at, note?}` — the named exterior vantages the walk judges the silhouette from, rendered beside the stage-2 reference sheet. Optional; a plan with zero views has that zero stated in the binding line. |
| `lighting` | `{fixture, min_light}` applied to every enclosed box, so a blockout interior is walkable at night without per-box surface. **The engine's existing area-lighting object**, not a twin of it, so it answers the same range rule with the same code (`DW0196`). |
| Binding | Every run that carries a plan prints a second line beside the layout-graph one: boxes and **the pairs compared**, seams (stair, drop), datums, whole-owned volumes, identities, sightlines and views. Two zeroes are called out as findings rather than counted: a plan with no view (the walk has no declared vantage) and a plan with no whole-owned volume (the rule keeping the whole's mass out of the places examined nothing). A plan with no identity is `DW0834` in its own right. |

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
what makes that a fact about the emitted palette rather than a claim, and it
fired on `pink_petals`, whose `flower_amount`/`facing` are an authored decision
and are now authored.

**Binding.** Every surround build prints its templates, biome bands, the
rectangle and **which authority stated it**, and the standable gap-floor cell
count the climb proof floods from — a flood that started from nowhere passes for
free and looks exactly like one that did not.

**Not yet reachable from the DSL**: the generator carries a second flora (cherry
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
headroom a sky-open place claims, which is why the walk record's freshness key
is over both. What a reader needs to know
about it is what it BUILDS, which is fixed:

| Thing | What the derivation makes of it |
|---------|---------|
| a box | A shell one cell thick around the play space — floor course, four walls to the top of the play space, and a ceiling unless the place is sky-open. The floor is the place's own **accent**, cycled deterministically over the plan's boxes, so the colour under a body's feet names the place it is standing in. Two connected places share the wall between them, written once by each and identical either way. |
| a seam | A frame of contrasting wall around the opening, and the opening itself cut to air — or filled with the bar, on a `barred` way, which the world-load seal model then measures shut exactly as it measures a prefab-authored gate. |
| a stair | A stepped run inside the box the plan named, at the **gentlest standard pitch the run really has room for** — chosen by `siteplan::gentlest_pitch` over the run `siteplan::stair_run` reports, which are the two calls `DW0830` refuses with, so the plan-time verdict and the built geometry cannot be about different numbers. Across a vertical face the treads start against the wall the seam is in and walk the whole footprint, so the span is the host's extent on that axis. Through a punched **floor or ceiling** it is not: the treads start at the hole and leave along one side of it, so what they have is the room on that side plus the hole's own width, and a pitch chosen against the whole extent is chosen against a run the host does not have. **A run is laid whole or not at all** — the courses that would fall off the far wall are never dropped in silence, because a stair with its bottom missing reads as a stair to every later reader and is not one: the body climbs in from above, stands on the treads, and the place counts as reached while nobody can stand on its floor. A climb no standard pitch fits is left unbuilt, which the observer sees as an unreached place (`DW0837`) — reachable only if `DW0830` let the plan through, which is why both read one function. Realized as whole blocks and bottom slabs rather than as the table's `realization` blocks: the occupancy model treats a stair block as a full cube (deliberately conservative), so treads built from stair blocks would present the navigation model a 16/16 jump per course where the table's own `step_16` says the body takes two 8/16 steps and never leaves the ground. The derivation builds the geometry the table describes out of blocks whose top faces the model measures exactly. |
| a volume | Plain mass — `massif` and `ground` in their own blocks, `clearance` kept empty. |
| the order | Volumes, then every shell, then every interior cleared, then every seam's frame, then every stair, and **the openings last**. Two passes are separated on purpose. The interiors: a neighbour's shell may legally stand in the cells over or under a place, and clearing every interior after every shell is what makes *the play space the plan allocated is air* an invariant of the derivation rather than a property of the order two boxes happen to be written in. The openings: a stair arrives AT its seam, so its top course sits directly under or beside the hole, and a course written after the hole was cut fills it back in — cutting last makes *the opening the plan allocated is open* an invariant too, so the massing may do what it likes and the hole is the last word. |
| lighting | The plan's one `lighting` setting, applied to every enclosed box by the ordinary relight pass. |

Its diagnostics and the battery that judges the result are `DW0821`/`DW0836`–`DW0839`.

### l10n sidecars (`l10n/<code>.json`)

Envelope `{dsl_version,campaign_id,kind:"l10n",lang,content,source?}`; `content` =
flat **stable key → translated string**, `source` = the same keys → the canonical
English each row was translated **from**. Key inventory derived from stage docs
(`world.title`, `world.outro`, `area.<a>.name`, `class.<c>.name/.blurb/.kit.<i>.name`,
`npc.<n>.name`, `actor.<a>.name` (a scripted puppet's nameplate, only when set),
`quest.<q>.goal`, `obj.<q>.<o>.title/.hint`,
`obj.<q>.<o>.missing_item_hint` (v0.7) and `obj.<q>.<o>.item_name` (a `collect`'s
collected-item display name, v0.8, only when authored),
`dlg.<n>.<node>.text/.opt.<i>.label/.opt.<i>.tooltip` (the tooltip v0.8, only when
authored), `wave.<w>.mob.<i>.name`) plus effect strings
`fx.<q>.oc.<o>.<i>.narrate|.give`, `fx.<q>.done.<i>.…`, `fx.trig.<t>.<i>.…`, and a
`bonfire`'s authored rest-dialog strings `fx.….rest_prompt|.rest_label|.save_label`
and a `close-gate`'s authored `fx.….sealed_hint` (all v0.8; unauthored ones are
absent because the compiler bakes its canonical English, the
`world.boundary.message` precedent), plus `lethal.<volume>.message` — a lethal
volume's death wording (v0.10, spec-0031). That one is **required rather than
defaulted** (`DW0512`): a player reading a raw key at the moment they die is the
worst place in a delve for a hole, and there is no compiler-owned English that
could be right for a cliff, a lava pit and an acid pool at once.
**Every effect root emission can lower** is inventoried, not just the quests
stage's three: `fx.trap.<trap>.<i>.…` for a `traps[].payload`
(spec-0022 — a trap that narrates is ordinary now that a trap's consequence is
commands) and `fx.dlg.<npc>.<node>.<opt>.<eff>.respawn.<j>.…` for a dialogue
option's `set-checkpoint` `on_respawn` bundle. A string in either used to be
neither demanded of a translator nor swapped at build time, i.e. it shipped
English-only in a translated build, silently. spec-0031 adds the last two:
`fx.sc.<shortcut>.<i>.…` for a `shortcuts[].on_unlock` beat (root 6, outside every
enumeration since spec-0016 §2 — the door's own line as the bars lift was
untranslatable, and no campaign had used it) and `fx.death.<i>.…` for the
campaign's `on_death` (root 7). `dsl::l10n::effect_roots` (immutable,
for the glyph/text-fit/sound consumer scans) and `effect_roots_mut` (for
`each_string`, hence `inventory` + `localize`) enumerate the same seven roots, so
what is measured and what is translated cannot drift; each ref carries the `stage`
it was authored in, so a dialogue-rooted `DW0326`/`DW0328`/`DW0330` names
`dialogue` rather than `quests`.
**Nested effects** (DSL v0.6): a `narrate`/`give-item` inside a `sequence` step or
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
somewhere else entirely). `source` is additive: an older sidecar parses unchanged
and its unguarded rows are **counted** by `DW0188` on every run, so an unadopted
sidecar never reads like a checked one. `tools/i18n-translate.py` writes it, so
adoption is a re-run with no retranslation.

**Every string field in the DSL is classified, or CI is red.** `DW0185` proves that
a string the inventory *knows about* reaches a component; it cannot see one the
inventory never met, which ships English silently — how `actors[].name` survived
twenty playtest rounds. `crates/dsl/tests/l10n_surface.rs` closes that half: it
enumerates every string-valued property of the seven stage schemas (derived from
the Rust types, so complete by construction — **78** today) and requires each to
be classified `Inventoried` / `Reference` / `Machine` / `NotPlayerVisible(<why>)`,
in both directions. A new `String` anywhere in the DSL fails it until somebody
records whether a player reads it. It is a test, not a `DW` code, because the
defect is in the compiler: no campaign input can produce it.

**`delvec l10n-inventory <dir> [--lang <code>]`** emits that inventory as one JSON
document on stdout — the work list a translator (in-agent, human, or an external
API via `tools/i18n-translate.py`) is handed up front, instead of discovering it by
writing an empty sidecar and reading the coverage diagnostics back:

```
{ campaign_id, dsl_version, lang, declared, sidecar_present, world_title,
  npcs:    [{id, name, archetype, speech_style, demeanor?, motivation}],
  entries: [{key, en, speaker?, existing?}] }
```

`entries` is the inventory itself (a CLI test asserts the key set equals what
`DW0180` demands, so the two cannot drift). `speaker` is the NPC whose dialogue
tree the key belongs to (`dlg.<npc>.…`, `npc.<npc>.name`; a `.opt.<i>.label` is the
player's reply *inside* that tree); `existing` is what `l10n/<lang>.json` already
translates, so a re-run fills only the gaps. Persona rows carry voice, never plot
(`secret`/`backstory`/`relationships` are excluded). Runs **before** validation
gating — an incomplete sidecar is the normal state when you ask — and needs no
prefab library; only an unparseable campaign fails (exit 1). See
[i18n.md](i18n.md).

### Language delivery — i18n v2 (spec-0029)

**A released delve ships every declared language; the client picks its own.**
`delvec build` (no `--lang`) emits every authored player-visible string as a
**translatable text component**

```json
{"translate": "<l10n key>", "fallback": "<English source>"}
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
| Key set | The existing l10n inventory, unchanged. `each_string` stays the single authority over what is translatable — no second key scheme, no second inventory. |
| Tagging | `dsl::l10n::tag_translatables` rewrites each inventoried string to `<U+E000><key><U+E000><English>` **once**, before `Plan::build`. From there the tag is the compiler's only evidence that a string is player-visible. Emitters lower it through `emit::tr` / `emit::snbt_component`; non-component consumers read it through `dsl::l10n::plain`. |
| Lang files | Flat `{key: string}` in `BTreeMap` order (ADR-0006). `en_us.json` **is** the live inventory; each other file is its sidecar's `content`. The key sets must be equal — a hole fails the build (`DW0180`/`DW0181` at emit time), because a hole is a player reading a raw key. |
| Language codes | `dsl::mclang::mc_lang_code` normalises (lowercase, `-`→`_`) and then **checks membership against the pinned client's own language set** — `CLIENT_LANGS`, 143 stems **derived** from Mojang's 1.21.11 asset index (`tools/derive-client-langs.py`; digests in the module header), never transcribed. The membership check is what makes normalisation safe: a bare rewrite alone would invent `de` from `de`, a filename no client asks for, and a lang file nobody loads is a language silently dropped. A bare language resolves to `<lang>_<lang>` if the client ships one, else to its sole file; ambiguous (`zh`, `sr`, `be`) and unknown codes are `DW0184`. Baked into the source — the compiler never reaches the network during a build (ADR-0006). |
| `--lang <code>` | **Unchanged**, and still the single-language bake (spec-0029 §4): strings are swapped before emission, nothing carries a translate key, and the build ships **no** lang files — there is nothing for a client to select between. For local dev and one-language artifacts; the release path does not use it. |
| Art titles | `emit_narrate` no longer `to_ascii_uppercase()`s an `art` string — a case transform is something a `{"translate": …}` component cannot express, since the client resolves the lang file after the compiler is gone. The `delve:art` font now carries a **second bitmap provider** over the same atlas addressed by the lowercase letters, so a lowercase letter renders through its uppercase bitmap: identical pixels, in every language. Cells with no lowercase form are `\u0000` (vanilla's unused-cell marker), so no char is claimed twice. |
| Width gates | `DW0330`/`DW0331` already checked source **and** every declared translation. Under v2 any declared language may be what a player sees, so those checks are load-bearing rather than belt-and-braces. Unchanged code, raised stakes. |
| Build inputs | Every `l10n/<code>.json` is now an input of **every** build (not just a `--lang` bake) and is hashed into `manifest.json` — the sidecar's bytes ship in the pack, so they are as much a build input as a stage document. |

No DSL change and no `dsl_version` bump: this is emission only. Every campaign's
emitted bytes change (literals become components); released delves reproduce
through their pinned engine (`versions.toml` + OCI), per the versioning discipline.

#### Compiler chrome — the strings the compiler writes itself

A delve's on-screen text has two authors. Everything
above concerns the **campaign's** strings. The compiler writes thirteen of its own
— `New objective: `, `Delve Complete`, `Choose your class`, the default a bonfire
shows when the campaign authors no label — and until this they had no key and no
override, so a player reading a fully translated delve still saw English chrome
wrapped around it.

They are **compiler-owned end to end** (`dsl::chrome`): the keys, the English, and
every translation live with the engine, and a campaign authors nothing. Eight of
them are *product chrome* (`objective.new`, `objective.complete`,
`campaign.complete`, `campaign.signature`, `campaign.banner`, `lobby.waiting`,
`class.title`, `class.body`) — no campaign author wants to write those, which is
why the answer is not to give them an override; that would move the engine's
maintenance cost onto content. The other five are *diegetic defaults*
(`boundary.message`, `gate.sealed`, `bonfire.title|rest|save`) whose authored
overrides already exist, are unchanged, and still win — what lives in `chrome` is
only what the compiler bakes when nothing is authored.

| Piece | Behaviour |
|---|---|
| Key space | `delvewright.ui.<area>.<name>`. Collision-proof both ways by construction: the l10n key scheme derives a fixed set of kinds and can never produce `delvewright.`, and vanilla never defines it either. A sidecar that writes one anyway is `DW0186`. |
| Delivery | Identical to an authored string: the chrome string enters emission as a translation tag, an emitter lowers it through `emit::tr`/`snbt_component`, and a site that fails to is `DW0185` — chrome inherits the whole invariant rather than getting a parallel path. |
| Sentences, not fragments | Four chrome strings frame a value and are **one key with `%s`**, carried by the component's `with` (`"%s — complete."`, `"New objective: %s"`, `"Waiting for the party — %s / %s"`). A concatenation freezes English word order into every language; `translate`+`with` is vanilla's own primitive for it. A unit test requires each language's placeholder count to equal the English's. |
| Lang files | Chrome is written into `en_us.json` and into each **declared** language's file — never into languages the delve does not already ship, or a French client on a Chinese-only campaign would read French chrome around English story. Partial-by-language reads as broken; uniform English does not. |
| The honest fallback | A language the compiler has no chrome table for gets **no chrome rows at all**: the client resolves through `en_us.json` (or, for a player who declined the pack, the component's own `fallback`) and reads English. Absent, never English written into `fr_fr.json` under a translated name. |
| Coverage | `dsl::chrome::TABLES` maps a client language stem to a table. Today: **30 tables covering 47 of the client's 143 locales**, plus the 5 English locales that need none. The rest render English. |
| `--lang` bake | A bake ships no lang files, so the fallback IS what the player reads: `Chrome::for_build` puts the baked language's text there, falling back to English. `%s` still substitutes — vanilla formats the fallback with the same `with` arguments. |

**The translations are unreviewed.** They are machine-produced from the canonical
English and have not been checked by native speakers; that is recorded in
`dsl::chrome`'s module header so nobody mistakes them for reviewed work, and a
correction is a one-line table edit. English stays canonical.

#### Named exclusions — where an authored string stays literal

An authored string that does not land in a text component cannot carry a translate
key. Every such site is named here and reads its string through
`dsl::l10n::plain`; none of them is rendered by a client. Anything **not** on this
list that emits an authored string outside a component fails the build with
`DW0185`, so this table cannot silently grow.

| Site | Artifact | Why it is not a component |
|---|---|---|
| `emit::artifact_title` | `packtest-datapack/**` test `#>` descriptions | A PackTest source is a generated test, read by the validation server and by a maintainer — never rendered to a player. |
| `emit::emit_packtest` (dialogue-mask tests) | `packtest-datapack/**/dlg_mask_<npc>_<node>.mcfunction` | Same: the node id appears in the test's own description line. |
| `combat::actor_json` (an actor's `name`) | `validation/combat-plan.json` | The validation ladder's own artifact, read by the bot and by a maintainer. It could not carry a tag before `actors[].name` was inventoried; now that it can, it reads the English through `plain`. |
| `render_plan::npc_name` / `area_name_of` / `first_clause` / the NPC shot `expect` | `render-plan.json` | The reviewer/vision artifact. Its `expect` prose is read by a vision model against a rendered frame, in English, regardless of what the delve ships. |

`delvec l10n-inventory`, `validate`, `analyze`, `snapshot` and `edit` never see a
tag at all: tagging happens inside `build`, after validation and analysis, so
every other subcommand reads the campaign exactly as authored.

`critical-path.json`, `validation/*.json`, `combat-plan.json` and `manifest.json`
carry **ids**, never authored prose, so they need no exclusion — the bot contract
was already language-neutral. A generated PackTest may still *write* a text
component (`collect_container.mcfunction` pre-loads the stack the objective
counts): that is emitted by the same helper the datapack uses, so the two cannot
drift, and it is an input to the test rather than an assertion about rendered text.
No generated PackTest asserts on rendered text at all.

---

## 3. Verb → emission mapping

Mechanism level (not full mcfunction). See `crates/compiler/src/emit.rs`.

| Verb / effect | Emitted mechanism |
|---------------|-------------------|
| `talk-to` / dialog option | `minecraft:villager` body (NoAI/Invulnerable/Silent) + co-located `minecraft:interaction` (tag `dw_npc_<n>`); click advancement + `/trigger dw.dlg_<n>` both feed one per-tick option handler. |
| dialog display gating (v0.4+) | A node with any display-gated option (`requires_flags`, `forbids_flags` (v0.6) and/or `completes`) emits one `__m<mask>` variant per per-node availability bitmask + a chooser: `dmask_<n>_<node>` sets `dw.dmask` (bit `i` = the node's i-th gated option is displayable — its required flags set, no forbidden flag set (`unless score @s dw.f_<flag> matches 1`, unset-safe), and every completed objective active: `if …qa_<q>==1 unless …o_<obj>==1`), then `show_<n>_<node>` `dialog show`s the matching variant. Ungated nodes/options `dialog show` directly (v0.2/v0.3 byte-identical). Click handler keeps its own guard (defense-in-depth for the `/trigger` path). Generated PackTest: `dlg_mask_<npc>_<node>`, **one per gated node** (`DW0811`'s `dialogue-mask` claim, one claim per NPC). Each drives EVERY gated option of its node through that option's own full display condition, asserts the bit shown, then breaks each term of the condition on its own and asserts the bit gone — so the axis is never chosen and every gate the DSL has (`requires_flags`, `forbids_flags`, `requires_state`, `completes`) is covered by construction. The assertion is always on the option's **isolated** bit (`(dmask>>bit)&1` via `%= 2^(bit+1)` then `/= 2^bit`), never the whole `dw.dmask` — sibling options in a node can share a `qa_<q>` score, so a whole-mask compare would read a sibling's bit as this option's. |
| dialogue trigger re-arm | `dlg_<npc>_<n>` consumes the trigger with `scoreboard players reset @s dw.dlg_<npc>` — which also **re-locks** it — and therefore re-arms it in the very next line (`scoreboard players enable @s dw.dlg_<npc>`), before the flag gate's `return fail` and before any `dialog show`. The per-tick `scoreboard players enable @a` stays as belt-and-braces but cannot close the window on its own: 1.21.9+ **freezes the integrated (singleplayer) server while a screen is open**, and the handler's last act is to show the next node, so ticking stops with the trigger locked and the player's next click is executed the instant ticking resumes — before the tick function — and vanilla rejects it ("You can't trigger this objective yet"), silently swallowing one dialogue choice. A dedicated server never pauses, so no rung of the validation ladder can reproduce it. The generated `dialogue_trigger_rearm` PackTest drives a terminal option and uses the trigger twice **with the tick function never run in between** — that suppression is the freeze. |
| cast-ledger dispatch (v0.7, spec-0020) | **The declaration is the gate.** `talk_<npc>` keeps its `advancement revoke @s only <ns>:<npc>_interact` (the interaction record is written by the click and consumed here), then calls `cast_<npc>` and dispatches on the per-player `dw.cast` selector: `execute if score @s dw.cast matches <i> run <action>`. `cast_<npc>` is pure scoreboard math — `set @s dw.cast 0`, then one `execute if score #party dw.qa_<quest> matches 1 [if/unless score #party dw.f_<flag> matches 1 …] run scoreboard players set @s dw.cast <scene>` per **declared placement**, in quest-DAG order then declaration order. A per-branch cast contributes one clause per branch carrying that branch's `requires_flags`/`forbids_flags`, so a branch-divergent NPC genuinely dispatches per branch rather than collapsing to its first placement; later clauses override earlier ones, so a per-branch entry lists its fallback first. Branch-gate flags are added to the setup objective declarations (`declared_flags`), since a *read* of an undeclared objective is a runtime command error and — unlike a `set-flag` write — nothing else guarantees the declaration. Because `dw.qa_<quest>` is set when a quest *begins* and is never cleared, the latest-begun beat wins and keeps winning: that is the whole retirement mechanism — once the escape beat opens, the premise root is unreachable because the ledger says so, not because an author remembered a flag. Scene `0` (no declaring quest begun) shows the stage-6 tree `root`. Actions: a root → the ordinary `show_node_cmd` (direct `dialog show` or the `dmask`/`show_` chooser); a bark pool → `function <ns>:bark_<npc>_<scene>`; `"none"` → **no clause at all** (the record is still consumed one line above, and nothing opens); `"unchanged"` → no new scene, the selector simply keeps pointing at the carried-forward one. Splitting the selector out of `talk_` mirrors `dmask_`/`show_` and for the same reason: a PackTest can drive it and assert which scene the ledger chose without opening a dialog a dummy player has no client for. `dw.cast` is declared only when some quest casts an NPC, and an NPC no quest casts emits the single pre-0.7 root line — so a campaign with no ledger is byte-identical. Generated PackTests: one `cast_ladder_<npc>` per ledger NPC — scene 0 asserted under the empty story, then EVERY clause asserted under a solved distinguishing drive (its own gate satisfied, every later same-quest clause's gate violated, all earlier quests still active — the retirement mechanism proven per clause), then every term of the clause's gate broken one at a time with the expected fallback scene computed by the compiler-side ladder model (`cast::eval_ladder`); plus `cast_bark_cycle` (every pool of every NPC) and `cast_none_silent` (every `"none"` scene: record written, scene selected, advancement re-armed). The walk is registered as `watch::Claim`s (`cast-ladder` over `cast_`, `cast-bark` over each NPC's `bark_<npc>_`), so a suite that drives fewer bodies than the authored ledger declares is a `DW0811` refusal. **The same resolution answers "where is this body?"**: `cast::station` walks those clauses in the same order under a playthrough's flag state and yields the governing placement's `at` — which is how a `talk-to` critical-path step gets its position (see `critical-path.json` in §5) and how `DW0483` decides a branch's placement. One model, so the cell the ladder walks to and the scene the datapack shows can never disagree. **Templates that assert dispatch pin EVERYTHING the ladder reads** (island r15): the batch is one shared server and three sibling verb templates legitimately end with a campaign flag set to 1, so any term left undriven is decided by batch order (expected `dw.cast 2`, got 3). Every dispatch drive is preceded by a pin of every quest-active score, every flag and every datum any clause of that NPC reads — and the values are not "requires → 1, all else → 0" but the **solved** assignment from `cast::distinguishing_drive`, because two clauses of one quest can differ only by `requires_state` and a flag pin cannot separate them: the drive satisfies the asserted clause's gate and provably violates every later same-quest clause's, or the clause is refused as dead (`DW0846`) before any template is written. This is what makes the per-clause assert a proof rather than a template that can name the wrong scene. |
| cast bark pool (v0.7) | `bark_<npc>_<scene>` advances `#bk_<npc>_<scene>` on the shared `dw.sys` objective by 1, wraps it with `matches <n+1>.. → set 1`, then `execute if score … matches <i> run tellraw @s [{name},": ",{line, italic}]`. An explicit clause ladder, never `%=` and never RNG (ADR-0006): the n-th right-click always yields the same line. Bark text is baked localized at emit time like every other player-visible string. |
| class select | Dialog button → `/trigger dw.class set <n>`, dispatched per tick to `class_apply_<c>` (kit, `dw.classed`, campaign-start party arming, teleport to the entry point). **One-shot per player**: the trigger is re-armed each tick only for a player who has not classed (`class_arm`), and the dispatch carries the same `unless score @s dw.classed matches 1` guard — see "The class trigger is ONE-SHOT per player" in §4 Hard invariants. Generated PackTests: `class_trigger_once` for the seal (a property of the trigger, not of any class), plus `class_apply_<c>` **per declared class** for that class's own kit, worn tag and entry warp (`DW0811`'s `class-apply` claim). |
| `reach-anchor` | Per-tick `execute if entity @s[…]` over the completion volume `reach::reach_completion` returns — a box of half-extent `max(1, radius)` at the anchor cell (v0.2: a `distance=..radius` sphere), formatted from that value rather than restated here; glowing `end_rod` `item_display` marker (tag `dw_r_<obj>`), labeled with the objective `title` — an **untitled** objective gets a nameless glowing marker, never a raw-id label. Completion despawns the marker (`kill @e[tag=dw_r_<obj>]`). |
| `kill` / `spawn-wave` | `spawn-wave` summons mobs (AI on) tag `dw_wave_<id>`, countdown `#<id> dw.wave`; `player_killed_entity` advancement decrements; `kill` completes at 0. Armed species get `equipment` NBT (drop 0): `wither_skeleton→stone_sword`, `skeleton`/`stray→bow`, `pillager→crossbow`, `vindicator→iron_axe` (the pillager row is load-bearing, not cosmetic — see `waves[].lane`/`DW0384`). **Arming assertion (generated `verb_kill`)**: the test picks the wave's first mob with an **effective** main hand — the author's `equipment.main_hand` when given, the default table otherwise (`emit::effective_mainhand`, the same source the summon NBT reads) — and asserts that exact item via `execute if items entity … weapon.mainhand <item>`. Deriving it from the default table alone shipped a self-contradicting delve: the-drowned-bell summoned `stone_axe` vindicators while its generated test demanded `iron_axe`, so a correct campaign failed on a real server; the override case also *extends* coverage to authored weapons on species the table calls unarmed. **Mob placement:** each mob is seated on a distinct compiler-validated standable cell (2-tall clearance, solid floor) chosen by a deterministic BFS outward from the wave anchor over the assembled occupancy world (`compiler::nav`), ordered by ascending BFS distance with a fixed `(y,z,x)` tie-break. The flood-fill is confined to the anchor's own assembled piece, so a flock never crosses a socket seam into a neighbouring room. A wave needing more footing than its room offers is `DW0312` (never `+x`-strung mobs piling into blocks or spilling toward void). **spec-0016 §6 changes where, not how:** a `summon: aggro-edge` wave is seated on per-mob perception RINGS across the whole area instead (`DW0387`), and a `lane` wave additionally carries the patrol NBT and starts its `lane_tick_<wave>` clock at the end of its own `spawn_<wave>` (so a wave that never spawns never ticks, and a bonfire re-seat re-arms the clock through the same replace-mode `schedule`). **Census probe:** every wave also gets `wave_census_<wave>`, `wave_census_one_<wave>`, `wave_brand_<wave>` and `wave_unbrand_<wave>`. The census zeroes `#wcen_n`/`#wcen_b`/`#wcen_d`, bumps `#wcen_seq`, runs the per-mob function `as @e[tag=dw_wave_<id>]`, and states the totals on the anchored marker channel as `[dw:census <ns> <wave> <seq> <present> <branded> <damaged>]`, one `[dw:censusmob <ns> <wave> <seq> <x> <y> <z> <health> <max>]` per mob first (all ×100 fixed-point, so nothing crosses chat as a float). `damaged` compares `data get entity @s Health` against `attribute @s minecraft:max_health get` — vanilla's own primitives, so it is never a table the compiler refuses to invent (`DW0475`) and never a value the client happened to be sent (an unmodified max health is not on the wire at all). `wave_brand_<wave>` stamps `dw_brand_<wave>` on the wave's living mobs and the unbrand clears it, which is how the die-retry ladder names a survivor **by identity**: a re-summon cannot carry the stamp. This exists because the ladder used to count silhouettes — every entity the client tracked, anything taller than half a block — and reported the drowned bell's ambush husks, 57 blocks away at another encounter, as wave mobs a re-seat had failed to remove. Generated PackTest `wave_census` proves the arithmetic live, including that a bystander of the wave's own species summoned on the wave's own anchor cell moves no count. **Which waves get machinery (uniform emission):** all of it is gated on the wave resolving a spawn AREA, and that resolution (`plan::wave_area`) walks every effect root **deep**, through `QuestEffect::nested_effect_lists` — the same nesting authority emission itself walks — so a `spawn-wave` inside a `sequence` step, a `set-checkpoint` `on_respawn`, a `bonfire` `on_rest`, a `begin-stealth` `on_caught`, a `move-npc`/`move-actor` `on_arrive` or a trap `payload` registers exactly like a top-level one. It used to scan the top-level chains only, which cost the island's round 21 two of its three storm waves: fired from step 7 of a `sequence`, they resolved no area, got no machinery at all, and the `seq_…` function shipped `function <ns>:spawn_…` pointing at nothing. `DW0497` is now the standing proof that no emitter can ship that shape again. A wave declared in `waves[]` that nothing fires anywhere is unchanged — it resolves an area only through the defensive `kill`-objective fallback, and otherwise emits nothing (`DW0171` owns the killed-but-never-spawned case, `DW0310` the spawned-but-unplaceable one). |
| `collect` | Chest at anchor pre-loaded `count×item`; `inventory_changed` advancement runs guarded completion. **v0.8 adoption:** with a `container`, `activate_<obj>` emits **no `setblock`** and fills the prefab's own chest/barrel at the container anchor's cell instead — `item replace block <x> <y> <z> container.<slot> with <item>[custom_name=…] <count>`, slot `0` the required stack and slots `1..=fill_count` the padding that makes it read full. The component suffix is rendered by the same helper `loot` uses (`emit::container_stack_components`), so a named quest item and a named loot stack cannot drift apart. Fill time is unchanged — **activation**, not world-init — which keeps gap 13's contract: a late objective's items are not lootable from minute one, and an item pocketed before activation still completes it via the per-tick held check. Generated PackTest `collect_container` (only when some collect adopts): clear the adopted slots, run the objective's own `activate_<obj>`, assert the filled item count across the container (`if items block … container.* <item>` = `count × (fill_count+1)` — a dropped fill reads 0, padding that overwrote slot 0 reads one stack short), then put the **named** stack in the player's inventory and tick, asserting completion. That last phase is the point: it proves on a live server that a `custom_name` component does not change what the adjudication sees. |
| `interact` | `minecraft:interaction` (tag `dw_i_<obj>`) + `player_interacted_with_entity` advancement + `/trigger dw.i_<obj>`. **`requires_item` = `execute … if items entity @s weapon.mainhand <item>` — HELD, not possessed**; a campaign that declares none is untouched. Optional `missing_item_hint` (v0.7) adds ONE line to `tick`: `execute as @a[scores={dw.i_<obj>=1..}]<same activation guard> unless items entity @s weapon.mainhand <item> run tellraw @s {"text":…}` — placed between the completion line and the trigger reset, so it rides the existing two-phase click handling (advancement reward sets the trigger, `tick` reads it and resets it) and one click narrates once. Guarded identically to the completion line, so a not-yet-active or already-finished objective answers a stray click with the old silence. Generated `verb_interact_held` PackTest proves the semantics live in two phases on one dummy — item in `inventory.0` with an empty hand must NOT complete (and asserts, via `if items entity @s container.*`, that the item really is carried, so the phase is not vacuous), then the same item in `weapon.mainhand` completes; the `tellraw` itself is asserted in Rust because a chat line leaves no game state for PackTest to look at. `packtest_preamble` therefore places a `requires_item` in `weapon.mainhand` rather than `give`-ing it (the old `give` only satisfied the old gate because a fresh dummy's first free slot happens to be its selected one). Glowing lantern `item_display` marker (also tag `dw_i_<obj>`, only when no `prop`), labeled with the objective `title` — untitled → nameless glow, never a raw-id label. `prop{block}` = `setblock` affordance. Completion despawns both entities (`kill @e[tag=dw_i_<obj>]`) so a finished objective is not clickable; the `prop` block persists as scenery. **Arming before adjudication.** The completion line is gated on `#party dw.qa_<quest>` and the very next line resets the trigger with NO guard at all, so a click is spent whether or not it landed. That pair is only safe because `tick`'s completion loop visits quests in **arming order** (`emit::quests_in_arming_order`, a stable topological sort over the `quest-complete` edges): the completion loop is the one place a quest is armed — a completion line runs `complete_<obj>` → `check_q_<q>` → `complete_q_<q>`, which writes `dw.qa_<next>` — so a quest's lines must precede the lines of any quest it arms, or a click already pending when its quest arms is adjudicated against an unarmed quest and then thrown away. Nothing in the DSL orders quest declarations, so before this the guarantee was an accident of the JSON array. The sort is stable, so a campaign already declared in arming order is byte-identical. The unconditional reset is deliberate and stays: a trigger fired long before arming is DISCARDED, never banked — a banked click would auto-complete the objective the moment the quest armed, with nobody having clicked. Losing input is a bug; fabricating it is worse. Pinned by `tests/tick_arming.rs` (the invariant over every fixture, plus a campaign deliberately declared out of order) and by the generated `verb_interact_arming` PackTest (premature click → no completion and no banked score; arming alone → still nothing; a real click after arming → completes). |
| stage-5 `loot[]` (spec-0021) | `setup_finish` emits one `item replace block <x> <y> <z> container.<slot> with <item>[components] <count>` per declared stack, slot = declaration index. `components` carries `custom_name` (localized) and `enchantments` when present. The container itself is never emitted — it is prefab furniture, proven present by `DW0431`. A campaign with no `loot` emits nothing here and stays byte-identical. |
| environment `triggers[]` (v0.4) | `setup_finish` gives each `strike`/`use` trigger a body at its `at` anchor (tag `dw_trig_<id>`); `approach` needs no entity. **The body is the shape of the object at that anchor, not a point.** `compiler::pressable::body_at` is the single authority and both this emitter and `compiler::eclipse` read it, so the two can never disagree about whether a body exists. Three outcomes: where a compiler-owned interaction set already covers the anchor — a `close-gate` seal, a sealed shortcut door — the trigger **rides** it and summons nothing (one cell, one hitbox; a second co-located box is the `DW0422` ray-pick tie); where the anchor names a **gate region**, one `1.02f` box is summoned per clickable **shell** cell of that region, exactly as a `close-gate` seal has always done; where it names a point in open space, the ordinary `1.0f x 2.0f` box, unchanged and byte-identical. **Why the region form exists:** a point body at a region anchor lands *inside* the solid block. Measured on the `souls-shortcut` fixture, a `use` trigger on the shortcut's gate emitted one body with AABB `[4,65,6]..[5,67,7]` inside a doorway slab occupying `[4,65,6]..[6,68,7]` — flush with the block on the faces it touched and interior on the rest. Vanilla bounds its entity raycast by the block hit and takes the entity only when it is *strictly* nearer, so that trigger was pressable from **no angle at all**, and it compiled with zero diagnostics; a doorway is also six cells, of which a point body covers one. `close-gate` had solved this privately inside one verb since v0.8 and nothing else could reach the machinery. A trigger whose anchor resolves to nothing at all is `DW0426`.  `tick`: `strike` fires on `nbt={attack:{}}`, `use` on `nbt={interaction:{}}`; `approach` is a `distance=..<range>` selector. **The click block is two phases, not one (round-8, island QA):** every click trigger's fire clause first, in declaration order, then every clear clause (`data remove entity @s <field>`). Emitting the pair inline per trigger is only sound while at most one trigger reads a given interaction entity, and several `strike-npc` triggers legitimately ride ONE NPC hitbox — the island's giant carried `wake-the-giant` (requires `flag/asleep`) and `his-house` (requires `flag/sealed`, forbids `flag/asleep`) on the same entity. Inline removal made the FIRST-DECLARED trigger consume the click even with its own gate shut, so `his-house` could never fire: a suppressed trigger starved its siblings and declaration order silently decided which of two legal triggers worked. Two phases make it order-independent — every trigger sharing a hitbox is offered the same click and fires exactly when its own gate says so — while consumption is unchanged (the record is gone by the end of the same `tick` pass, so a held click still fires once). Byte impact: a campaign whose click triggers are its last-declared triggers is unchanged; any other ordering moves the clear clauses to the end of the block. `once` guards on `#trig_<id> dw.sys`, which **every** trigger now writes on firing (not only `once` ones): the write is what makes dispatch observable at all — the starvation bug was a trigger that simply never fired, invisible to every automated check — and it is what the generated `v06_shared_hitbox` template reads. One added line per non-`once` trigger function. **Generated `v06_shared_hitbox` (round-8):** emitted for a campaign that has two click triggers on one NPC hitbox whose flags can tell them apart; it proves the hitbox really is shared, then writes the vanilla `attack` compound and runs the real `tick` twice — once with the later trigger's gate open and the earlier one's shut (the starvation case: the later one must fire, the earlier must stay silent, the record must still be consumed), once with the earlier one's gate open (so both are reachable). Players are shielded with Resistance V across each pass because a real `tick` runs real effects and a delve's effects include `damage-players`; flags, actors and NPCs are handed back untouched (batch model). **`strike-npc` — the body IS the target (v0.6, round-7):** `on: {on:"strike-npc", npc}` has **no anchor**. Its tag rides the interaction hitbox the named NPC already owns and `setup_finish` summons nothing for it, so it works wherever that NPC stands and whatever body it wears. This is the form that can express "hit the giant": a place-based `strike` summons its own entity at a *cell*, and a large NPC's body eclipses that cell (`DW0359`), so the click never reaches it — the owner's island round-7 finding, where striking Polyphemus did nothing. Right- and left-click stay separate all the way down because a `minecraft:interaction` records them in **two distinct NBT fields**: the dialogue advancement takes the right-click (`interaction`), the trigger takes the left-click (`attack`), and neither consumes the other's record. That separability is machine-proven, not assumed — the generated `v04_strike_npc` PackTest writes a right-click record on the shared hitbox, ticks, and asserts no `attack` record appeared and the trigger did not fire. **Strike on an NPC's anchor — one cell, one hitbox (round-6):** the pre-0.6 spelling of the same mechanism, kept working — when a `strike` trigger's `at` is also where an NPC stands, the NPC's own interaction hitbox carries `dw_trig_<id>` **and is the trigger's sole entity** — `setup_finish` suppresses the trigger's own summon. The NPC's body is `Invulnerable`, so without the shared tag a swing could land where nothing was watching and the trigger never fire (round-4 island QA); and with a *second*, exactly co-located hitbox (the round-4 form) the client's entity ray-pick is ambiguous — an exact tie resolves to whichever entity iterates first, in practice the world-init summon — so every right-click landed on an entity without `dw_npc_<n>` and the dialogue advancement never fired (round-6 island QA: Polyphemus untalkable after the boulder seal, proven on a live server). Consequences: the trigger's lifecycle follows the NPC's — a `deferred` NPC's strike trigger is armed only after its `spawn-npc` entrance, a `move-npc`'d NPC carries the strike target with it, and `despawn-npc` removes it entirely (which is the trigger's meaning: the thing being struck is the NPC). Scoped to left-clicks: right-click on an NPC already belongs to the dialogue advancement, so a co-located `use` trigger is rejected at validate time (`DW0350`) and again at build time (`DW0359`). Generated PackTests: `v04_strike_npc` writes the vanilla `attack` compound onto the NPC's hitbox and asserts the trigger fires, once, with the record consumed; `v04_strike_talk` pins the single-hitbox invariant — exactly one interaction entity wears the trigger tag, none wears it without the NPC tag, before and after an attack record is consumed (attack-then-talk must stay clickable); and **`env_trigger_<id>`, one per declared trigger** (`DW0811`'s `env-trigger` claim), which opens that trigger's own gate through `packtest_gate_drive`, calls `trig_<id>` the way that trigger's own dispatch route calls it — with no executor for a `party` bundle, `as` the test's dummy for a `presser` one — and asserts the `#trig_<id> dw.sys` marker every trigger writes on firing. |
| `set-flag` / `requires_flags` / `forbids_flags` | `dw.f_<flag>` scoreboard (per-player); required flags AND-ed into objective guards (layered on `after`), forbidden flags (v0.6) joined as `unless score @s dw.f_<flag> matches 1` clauses in the same guard. **Per-effect** gates (v0.6) wrap each of the effect's emitted commands in `execute if score @s dw.f_<flag> matches 1 [… per required] unless score @s dw.f_<flag> matches 1 [… per forbidden] run <cmd>`; these effect functions already run per-player (`complete_<obj>` / `trig_<id>` are entered `as @a`/`@s`), and an ungated effect is emitted verbatim (byte-identical). In a **scheduled** bundle (`on_arrive`, `sequence` steps) there is no acting player: a per-player effect's gate stays `if score @s …` but under the effect's own `as @a`, while a global effect's gate degrades to the any-player predicate `if entity @a[scores={dw.f_<flag>=1..}]` — §4 "A scheduled bundle has no `@s`". `unless … matches 1` is the deliberate unset-safe spelling: flag scores are never pre-initialized to 0, so a `scores={…=..0}` selector would not match an unset score. That is one instance of a rule the compiler now enforces over the whole emitted tree (`DW0495`): a missing entry is not zero, it is false to every question, so a comparison either reads an entry the pack creates or is spelled so the absence cannot change its answer. **Trigger-level** `forbids_flags` is any-player: the fire condition gains `unless entity @a[scores={dw.f_<flag>=1..}]` per flag (a positive selector inside a negation — flags are campaign state, so one player's wake beat stands the trigger down for everyone); a suppressed strike/use still consumes the interaction record. Generated PackTests: `verb_flag_gate` (requires) and `verb_forbid_gate` (forbids: set flag → drive → assert NOT complete; clear → drive → assert complete). |
| `open-gate` | `/fill … air replace <block>` over the gate region — one **region write** (`emit::fill_region_command`, shared with `close-gate`, `fill-region` and `clear-region`), `replace`-filtered to the block the anchor declares so it removes the gate and nothing else that has drifted into the box. **Plus `kill @e[tag=dw_seal_<anchor>]`** when the campaign ever seals that anchor (v0.8): the seal's answer comes down with the seal. An opened threshold that still says "the way is sealed" is a lie, and an invisible box left standing in a doorway swallows right-clicks aimed through it. |
| `close-gate` | The same **region write** with the anchor's declared fill block and no `replace` clause — `/fill <region> <block>` (the dual of `open-gate`), **plus `execute unless entity @e[tag=dw_seal_<anchor>] run function <ns>:seal_arm_<anchor>`** (v0.8 — a sealed boulder answers a right-click instead of standing there in silence). See [The seal answers](#the-seal-answers) below. |
| `fill-region` / `clear-region` (v0.10) | The **general** region write: one `fill <lo> <hi> <block>` over `Plan::zone_box(region)`, with `minecraft:air` and no `replace` filter for the clear. Same builder as the two gate verbs above; the only difference between all four is where the box comes from and whether the fill carries a filter. An unresolvable `region/anchor` emits nothing — that is `DW0142`/`DW0355` at validation, not a silently mis-aimed fill here. |
| `give-item` | Grants item to player (`name` → SNBT text component). |
| `narrate` | chat / `title` / `subtitle` (+ optional sound); `art` = `title` with a `{"font":"delve:art"}` text component, rendered uppercase in the pixel-banner font (6 font px/glyph → ~15 glyphs fit; see [The `delve:art` font](#the-delveart-font)); `actionbar` = `title <who> actionbar <component>`. |
| `play-sound` | `playsound <sound> master @s [<pos>] [<vol> [<pitch>]]` — effects run `as @a`, so `@s` is each player: `anchor` uses the resolved anchor pos (all hear it there), `players` uses `~ ~ ~`. |
| `damage-players` | `execute as <audience>[tag=!dw_cutscene] run damage @s <amount> <type>` for a party beat (`@a`), `execute if entity @s[…] run damage @s …` inside a solo `on_caught`/`on_respawn` (default type `minecraft:generic`). With `in`, the stealth-zone box (`x=…,dx=2·ext,…`) joins the same selector, so each player is judged on their own position — no double-hit. `/damage` takes a single entity, so the party form re-binds rather than widening the target (§1, single-entity arity). A generated `v06_damage` PackTest summons a tagged dummy, applies the declared amount+type, and asserts its `Health` strictly dropped. |
| `set-block` | `setblock` at resolved anchor. |
| `despawn-npc` | Kills body + interaction hitbox. The generated `v04_despawn` PackTest targets the campaign's first `despawn-npc` NPC; when that NPC is **deferred** it runs its `spawn_npc_<id>` entrance right after `setup_finish` (a deferred NPC is deliberately absent from world init, so the presence assertion would otherwise read 0). The assertions themselves — 2 entities present, 0 after the kill — are identical in both cases, and the entrance line is emitted only for a deferred target, so a campaign with no deferred NPC keeps byte-identical PackTest output. |
| `spawn-npc` | `function <ns>:spawn_npc_<npc>` — the generated entrance function, emitted once per **deferred** NPC. Its two lines are the world-init summons, each independently guarded: body by `unless entity @e[tag=dw_npc,tag=dw_npc_<n>]`, hitbox by `unless entity @e[tag=dw_npc_<n>,tag=!dw_npc]` (both carry the id tag, so a single shared guard would let the body's own summon suppress the hitbox). The `npc_summons` PackTest fires each deferred NPC's entrance after `setup_finish` and asserts exactly one body. |
| `move-npc` | Per-tick tp along A*-planned walkable waypoints (hitbox in lockstep), at cell **centres** with L-shaped vertical steps — see §4 "Entity placement". Every `tp` carries `<yaw> 0` — the **exact bearing of the segment that tick walks**; see §4 "A walked body faces where it is walking". `on_arrive` (v0.6): the driver's final-waypoint tick additionally runs `mv_arrive_<key>` (the bundle's effects), mirroring `ma_tick`/`ma_arrive_<key>` exactly; a bare move emits no hook (byte-identical). The arrive bundle runs with the **server** command source (the driver reached it through `schedule`), so its effects are split per-player / global — see §4 "A scheduled bundle has no `@s`". A later `move-npc` for the **same body** supersedes any walk still running for it — see §4 "One body, one live walk driver"; a body with only one planned walk carries none of that machinery (byte-identical). |
| `cutscene` | Per player: save gamemode+pos → spectator → alternate `spectate` between two co-located dolly cameras each tick (skipping any player actively holding sneak — `predicate=!<ns>:sneak_held`, see §4 "The `spectate` bounce is sneak-gated") → restore. **Keyframe dolly (`compiler::camera`)**: each shot's waypoint polyline is arc-length parameterized (equal distance per time, not equal segments) with baked smoothstep ease-in/ease-out, then emitted as a tick-0 snap + a `tp` every *N* ticks with display-entity `teleport_duration:N` armed via `data merge` — the **client** tweens position and rotation linearly between keyframes (spike-measured: one position-sync packet per keyframe, rotation interpolates, the `spectate` bounce cannot reset an in-flight tween, and a same-tick merge+`tp` applies the OLD duration because position syncs flush before metadata — which is exactly why the snap and its cadence merge may share a tick). Cadence *N* = the widest of {10, 5, 4, 2, 1} whose rendered chords stay within 0.25 blocks (perpendicular) and 2° (aim) of the exact eased path; a single-waypoint or 1-tick shot is a static snap (cadence 0, no merge). Each shot with a successor resets `teleport_duration:0` on its last owned tick so the next snap is a hard cut, not a glide. Every keyframe `tp` carries an explicit `<yaw> <pitch>` — **Minecraft** entity rotation (`yaw = atan2(-dx, dz)`, 0 = +Z south; `pitch = atan2(-dy, hypot(dx,dz))`, + = down), *not* the render-plan/Chunky yaw convention — computed at emission from the camera's own position: at the shot's `look_at` subject if it has one, else along the eased path's direction of travel. Never the summon default (yaw 0 = south). Positions and rotations rounded to 3 decimals, `-0.0` collapsed to `0.0`, so emission is byte-stable. The bracket also arms the `dw_cutscene` state on every player and releases it on restore — see §4 "A cutscene is pure observation". Multi-shot: all shots share one `#t_<bare>` counter — shot *k* owns `[offset_k, offset_k+len_k]` and the next starts at `offset_k+len_k+1` (hard cut); one marker, one `gamemode spectator @a`, one camera pair, one restore. Both single-shot spellings emit identical bytes. `critical-path.json`'s `cutscene_seconds` is the **total** across shots. Function key = `cs_<first anchor>_<seconds>_<waypoints>` (a pathless styled shot keys `cs_<style>_<subject>_…`), plus an 8-hex sha256 digest of the whole normalized shot list whenever the cutscene is not a bare single shot without `look_at`/`shot_style` (the key must be injective — two shots sharing a first waypoint must never collapse onto one function). Styled shots are expanded (`compiler::camera::expand_shot`) before keyframe planning; a moving subject's per-tick track comes from its sibling move's A* plan, aligned by effect-group/sequence timing. Deduplication stays DSL-content-keyed, so two byte-identical styled cutscenes in *different* move contexts plan from the first occurrence (documented limitation; give the shots distinguishing content to split them). |
| `campaign-complete` | `dw.campaign` = 1 (dummy objective, **never on the sidebar** — a raw internal id must not surface to players); broadcast `[dw:complete <campaign_id> campaign]` (dark-gray bot channel, the harness's completion signal — §4 "The completion-marker channel"); title fanfare. |
| objective lifecycle | Activation shows `title`+`hint`+`note_block.pling` once (flag `dw.ann_<obj>`); completion sets `dw.o_<obj>` = 1, immediately broadcasts the anchored marker `[dw:complete <campaign_id> obj/<id>]` (§4 "The completion-marker channel"), then plays `experience_orb.pickup`. The marker precedes the objective's effects deliberately: it timestamps *completion*, not the aftermath. **Marker cleanup:** completion despawns every entity the objective's activation summoned via the objective-scoped tag — `interact` hitbox + wayfinding marker (`dw_i_<obj>`), `reach` marker (`dw_r_<obj>`). Prop/affordance *blocks* (`interact.prop`, `collect` chest) are scenery and persist; `talk-to`/`kill` summon no per-objective marker. Gated on v0.3+ with a resolved activation, so v0.2 stays byte-identical. |
| `on_death` (DSL **v0.10**, spec-0031) | The campaign-wide death beat — **effect root R7**, at `/content/on_death`, one bundle per campaign. It rides the SAME detector `set-checkpoint` arms and adds no second one: `dw.deaths` (`deathCount`) is the only thing in the delve that notices a death, `cp_respawn_check` is the only function that reads it, and one `tick` line runs it per player. What it adds is a second **acknowledgement** of that one counter. `dw.death_ack` is deliberately withheld while a player is dead (the whole edge is held until they are alive again, so an unspent edge stays armed instead of burning on the corpse), which is exactly the window this beat wants, so the corpse side gets its own ack `dw.death_seen`: `execute if data entity @s {Health:0.0f} if score @s dw.deaths > @s dw.death_seen run function <ns>:on_death_fire`, then the matching `= @s dw.deaths` so it fires once per death rather than every tick of the death screen. The corpse side is emitted FIRST, in the order the player lives the two moments. `on_death_fire` is the bundle under **`Audience::Solo`** — the dying player's own, like `on_respawn` and `on_caught`; broadcasting one death to the party would duplicate their narration and their kit. A campaign declaring no death beat emits none of it (no branch, no function, no `dw.death_seen`) and a campaign with a death beat and no checkpoint arms `dw.deaths` and this branch alone — no `#cp` marker, no re-seat, no `dw.death_ack`. **The death POSITION is not captured**: `emit::death_position_capture` is a named, deliberately empty seam ahead of the dispatch, because which vanilla mechanism records it (the pre-respawn death advancement, or `LastDeathLocation`) is unverified for non-entity deaths — void, fall, drowning — and is being measured on a live pinned 1.21.11 server (spec-0031, "Unverified"). Nothing downstream reads a position yet. Tests: `v10_on_death.rs`. |
| `set-time` / `set-weather` | `time set <kw|ticks>` / `weather <kw>` (dimension-global, no selector) inline in the effect/dialogue-option function; instantaneous cut, persists (cycle frozen). |
| relight fixtures (`lighting`) | `setblock` per placed fixture in `setup_finish`, after structure placement + socket seals (spec-0010). Blocks: `torch`/`wall_torch`, `lantern[hanging=…]`, `campfire[lit=true]`, `shroomlight`. |
| `mitigation: "night-vision"` | `night_vision_tick`: one `effect give @a[x=…,dx=…,y=…,dy=…,z=…,dz=…] minecraft:night_vision <lease> 0 true` per declaring area — written since v0.10 by `emit::effect_give_command`, the same formatter the author-facing `give-effect` verb uses, so the engine's own grant is one configured use of the general verb's emission rather than a private copy of it (byte-identical: the line was already the full five-token form) (the lease is `max(12, longest camera + 11)` s — the camera-coverage guarantee, see §"world") (selector = the area's final placed bounds, compile-time literals), then `schedule function <ns>:night_vision_tick 20t` (vanilla replace-mode, so the clock can never double up). `setup_finish` arms it once. A generated `v06_night_vision` PackTest teleports a dummy into the declared bounds, runs one clock tick and asserts it holds the effect — then teleports it 1000 blocks out and asserts it does not. |
| `set-checkpoint` | Inline: `spawnpoint @a <x y z>` + `data modify storage dw:cp pos set value [x,y,z]` (the readable "last checkpoint" mirror) + `#cp dw.sys = <index>` (the active-checkpoint marker; emitted for **every** campaign that declares a checkpoint). `setup_finish` seeds `dw:cp` to the spawn cell. Any checkpoint arms the respawn machinery: a `deathCount` objective (`dw.deaths`) + per-player ack, and a `tick` line running `cp_respawn_check`. **`cp_respawn_check` seeds every score it compares first** (`scoreboard players add @s <obj> 0`, idempotent, and on a `deathCount` objective it does not disturb the criterion): a player who has never died has an entry in none of the three, and a comparison against a missing entry is FALSE on the pinned server — so before this the whole edge was dead on a player's FIRST death and worked only from the second onward. `DW0495` is the standing proof that no emitter can ship that shape again. **The re-seat.** `spawnpoint` is a *hint*, not a promise: vanilla re-validates the recorded cell at respawn time and, whenever that cell or the cell above it is solid or liquid, silently discards it and respawns the player at the **world spawn** — the campaign entrance. Measured live on pinned 1.21.11: a spawnpoint on a dry cell respawns at `cell + (0.5, 0.1, 0.5)`; the same spawnpoint on a water cell respawns at `setworldspawn`. Past a one-way transport that is not a lost checkpoint but an unrecoverable softlock (the owner's tide-mill playtest). So the delve stops delegating its own promise: `cp_respawn_fire` dispatches `cp_seat_<index>` (a bare `tp @s <cell centre>`, coordinates compiled in — no macro, no storage read) for the active checkpoint **before** any authored `on_respawn` beat. When vanilla honoured the spawnpoint the player is already there and the teleport is invisible; when vanilla dropped it, this is the only thing that puts them back. It is edge-triggered, never a leash. **Edge timing**: `deathCount` ticks up on the DEATH, while the player is still a corpse on the death screen, so `cp_respawn_check` holds *both* the fire and the acknowledgement behind `execute unless data entity @s {Health:0.0f}` — the whole bundle lands on a player who has actually come back, and an unspent edge stays armed. Generated PackTests: `v06_checkpoint_respawn` (the record) and `v06_checkpoint_reseat` (the landing — drive a real `deathCount` edge from the campaign entrance, assert the player ends on the checkpoint cell centre, assert the ack, then assert no second re-seat without a second death). |
| `timed-gate` (spec-0016 §4) | `setup_finish` starts the clock: `function <ns>:tgate_open_<id>` at `phase: 0`, else `schedule … <phase>t`. `tgate_open_<id>` = `fill … minecraft:air replace <block>` + `schedule function <ns>:tgate_close_<id> <open_ticks>t`; `tgate_close_<id>` = (when `crush: true`) `execute as @a[<gate region>,tag=!dw_cutscene] run damage @s 1000 minecraft:generic`, then `fill … <block>` + `schedule function <ns>:tgate_open_<id> <closed_ticks>t`. The judgement precedes the `fill` deliberately — after the seal the victim is already encased and vanilla suffocation, not the portcullis, would be what kills them. `/damage` takes ONE entity, so the party form re-binds via `execute as` rather than widening the target. Both halves are pure world edits naming no player, so the server command source they are re-entered under is irrelevant (§4). Generated PackTest `souls_timed_gate_<id>`, **one per declared gate**: re-seal, assert sealed, drive the real open, assert air, drive the real close, assert sealed again. Every scratch score carries the gate id too, because the suite is one batch on one server with no ordering between templates. With `crush: true`, one more for that gate — `souls_timed_gate_crush_<id>`: the emitted region selector holds the dummy standing in the gate and releases it two blocks clear. It asserts **scoping, not death**, because **PackTest fake players are immune to `/damage`** (measured on the pinned toolserver 2026-08-03: a `# @dummy` reports `playerGameType: 0` yet `damage @s 1000 minecraft:generic` leaves `Health` at 20.0, and an explicit `gamemode survival @s` first changes nothing — the same limitation that already put the `damage-players` PackTest on a zombie dummy, which cannot stand in here since the crush selects `@a`). Lethality and ordering are pinned by compiler unit tests, and the end-to-end death was verified against a real mineflayer client on pinned 1.21.11 (parked 2 blocks clear a player survives 30 s of repeated closing ticks at full health; one closing tick standing inside kills them). The test binds `@s`, never `@a`: PackTest runs the whole suite in ONE shared world, so a sibling template's dummy in the same fixture cell would otherwise be counted.  With a `disarm` every line of `tgate_close_<id>` — the judgement, the `fill` and the next hop — plus the open half's `schedule` is prefixed `execute unless score #tgdis_<id> dw.sys matches 1`; the open's own `fill` is deliberately NOT guarded, because a jam landing while the gate is shut leaves one already-scheduled open in flight and that open is what parks the portcullis in its resting position. `setup_finish` summons the jam affordance (interaction hitbox + `dw_hw_…` item_display, `DW0420`), the tick carries the same one-shot `#tgdis_<id>` poll a shortcut unlock uses, and `tgate_disarm_<id>` is four commands whose ORDER is the semantics: latch the sentinel, raise `sets_flag` party-wide, `fill … minecraft:air replace <block>` once, `kill` the hardware (the one function `DW0421` allows to). There is deliberately no `schedule clear`: a close already in flight fires into the guard and does nothing — including not scheduling the next open — so the ping-pong dies of its own accord within one hop. Generated PackTest `souls_timed_gate_disarm_<id>`: prove the clock really seals while armed, pull the real lever, then drive `tgate_close_<id>`/`tgate_open_<id>` across three former cycle boundaries and assert the span is air at each. |
| `shortcut` (spec-0016 §2) | `setup_finish` summons the far-side unlock affordance (`minecraft:interaction`, tag `dw_sc_<id>`) — Alongside the hitbox the compiler summons its **visible hardware** — a glowing, collision-free `minecraft:item_display` at the same cell, tagged `["dw_marker","dw_hw_<tag>"]` (a `minecraft:lever` icon). `minecraft:interaction` is invisible, so the hitbox alone is a right-click target the player cannot see: the drowned-bell soft-lock (`DW0420`). Visibility is the compiler's, never the tileset's. `shortcut_open_<id>` kills it as the bar is thrown: the affordance is spent, and it is the ONLY function permitted to retire it (`DW0421`). — and emits **nothing** for the gate, which the prefab already seals. `tick` polls the affordance's `interaction` record once, guarded by the `#sc_<id> dw.sys` sentinel → `shortcut_open_<id>`, then clears the record. `shortcut_open_<id>` latches the sentinel, clears the gate region (`fill … minecraft:air replace <block>`, the same command `open-gate` emits) and runs `on_unlock` server-source-safe. `on_unlock` is **effect root R6** since spec-0031 — the bundle emission lowers here is now the same bundle every proof and every l10n pass walks. No emitted function anywhere ever re-fills a shortcut gate — the runtime half of permanence, asserted by a test over the whole datapack. Generated PackTest `souls_shortcut`: sealed before, air after, still air after a second unlock pass. |
| `lane` / `summon: aggro-edge` (spec-0016 §6) | A lane wave's `spawn_<wave>` summons the squad with `,Patrolling:1b[,PatrolLeader:1b],patrol_target:[I;x,y,z]` (leader = the first summoned mob, also tagged `dw_lead_<wave>`), `follow_range` forced to `aggro_radius`, then sets `#lane_<wave> dw.sys 0` and schedules `lane_tick_<wave>` at 30t. **Only the snake_case int-array routes** — 1.21.11's strict codec silently drops the legacy `PatrolTarget:{X,Y,Z}` compound and the squad then patrols to vanilla-rolled random points (working-but-drunk); `Patrolling`/`PatrolLeader` keep their camelCase names. `lane_tick_<wave>`: advance guards in DESCENDING index order (so one cycle steps at most one waypoint) firing when ANY squad member is within 8 blocks of the current waypoint (any member, not the leader — a dead leader must not strand the warband); one `data merge entity @s {Patrolling:0b}` release for every mob with a player inside `aggro_radius`; one per-index re-assert `{Patrolling:1b,patrol_target:[I;…]}` for every mob with nobody inside it (this is what defeats vanilla's arrival re-roll and the lone-patroller self-cancel, and it is inert during combat because the patrol goal cannot restart while the mob has a target); then a `schedule … 30t` re-arm guarded on the squad still existing, so the clock stops by itself. An `aggro-edge` wave carries no patrol NBT at all — only its ring placement. Because a re-seat is `kill` + this same `spawn_<wave>`, everything above is also what re-stations a re-seated squad (spec-0016 §1). Generated PackTests `souls_td_patrol_nbt`, `souls_td_lane_march`, `souls_td_lane_release`, `souls_td_lane_reseat` (the squad hauled onto the party, released to native AI and its clock run to the lane's end, then re-summoned: routed again from waypoint 0, release gone), `souls_td_aggro_edge`. |
| `bonfire` (spec-0016 §1) | Inline at the arming beat: `execute unless entity @e[tag=dw_bonfire_<i>] run summon minecraft:interaction … Tags:["dw_bonfire_<i>"]` — Alongside the hitbox the compiler summons its **visible hardware** — a glowing, collision-free `minecraft:item_display` at the same cell, tagged `["dw_marker","dw_hw_<tag>"]` (a `minecraft:campfire` icon, under the same absence guard so a re-fired beat never stacks a second one). `minecraft:interaction` is invisible, so the hitbox alone is a right-click target the player cannot see: the drowned-bell soft-lock (`DW0420`). Visibility is the compiler's, never the tileset's. Never retired: a bonfire is rested at, not used up, so **nothing** may kill its hardware (`DW0421`). — nothing else; the checkpoint does not move. **The click opens a choice, it does not rest.** A per-bonfire advancement `bf_<i>` on the vanilla `player_interacted_with_entity` criterion rewards `bonfire_open_<i>`, which therefore runs **as the clicking player** — the interaction entity's own `interaction` record names no player a `dialog show` could target, which is why the poll was replaced by the same primitive every `interact` objective already uses. `bonfire_open_<i>` revokes its own advancement (a rest point is used, never consumed), sets `dw.rest_at = <i>`, resets then `enable`s the **trigger** objective `dw.rest`, and shows dialog `<ns>:bonfire_<i>`. A trigger because a dialog button runs its command as the player and `/trigger` is the only command a non-operator player may run. The two buttons write `2` (*rest and save*) or `1` (*save only*); `tick` turns each answer into its function — `execute as @a[scores={dw.rest=1,dw.rest_at=<i>}] run function <ns>:bonfire_pick_save_<i>` and the `=2` twin for `bonfire_pick_rest_<i>`. `dw.rest_at` is what keeps a multi-bonfire campaign from routing every answer to the first fire. Each pick resets `dw.rest` first, so one press is one rest. **`bonfire_save_<i>`** = exactly the three `set-checkpoint` lines (`spawnpoint @a`, the `dw:cp pos` mirror, `#cp dw.sys = <i>`) and nothing else. **`bonfire_pick_rest_<i>`** = `bonfire_restore` then `bonfire_rest_<i>`. **`bonfire_rest_<i>`** is unchanged from v0.6 — those three lines + the wave re-seats + the `on_rest` bundle, emitted **server-source-safe** (§4: player-facing effects re-bind to `as @a`, so the whole party rests together and party state fires once). **`bonfire_restore`** is the player-local half: `effect give @s instant_health 1 9 true`, `effect give @s saturation 1 9 true`, one `effect clear @s <id>` per harmful effect (enumerated, never a bare `effect clear @s`, which would also strip the per-area night-vision mitigation clock and any beneficial effect the story granted), then `bonfire_flask`. `instant_health`/`saturation` because vanilla has no `/health` or `/food` command and `/data merge entity` refuses players — those two effects ARE the primitive. **`bonfire_flask`** refills each `flask` kit entry for the class the player took: `execute if entity @s[tag=dw_class_<c>] run clear @s <item-predicate>` then the matching `give @s <item>[components] <count>` — both built by the same two helpers the class kit's own give uses, so the refilled item is poured-identical, and the clear names the flask's potion `contents` rather than a bare item id (§2 stage 3) so it cannot take an unrelated potion out of the bag. `clear`+`give` rather than `item replace` because a kit item has no fixed slot; replenishment is two-directional by construction (a hoarded stack comes back DOWN to the declared count — the flask is a per-rest budget, not a stockpile). The respawn path runs the same `on_rest` bundle through `cp_on_respawn_<i>` under the player executor, so a bonfire with an empty `on_rest` still dispatches when it owes a re-seat, and — since vanilla already returns a dead player at full health but not with a full flask — it calls `bonfire_flask` too, so retry never costs a second walk to the fire you just respawned at. The **exported critical path rests**: after the step that arms bonfire `<i>` the path gains `{"action":"rest","bonfire":<i>,"anchor":…,"pos":…,"command":"/trigger dw.rest set 2"}` (see `critical-path.json` below). Generated PackTests `souls_bonfire_rest` (the real rest function moves `dw:cp` to the bonfire cell), `souls_bonfire_reseat` (a met, wiped wave stands again at its authored count after a rest; an unmet one is not conjured; and a **chipped survivor**, branded with an ad-hoc tag no re-summon can carry, is gone after the rest while the wave stands full — the no-chip-through rule proven by identity, not arithmetic) `souls_bonfire_options` (save-only moves `dw:cp` and leaves the flask alone; rest replenishes it to the declared count) and `souls_reseat_stationed` (the **stationed** re-seat: the wave is dragged onto the party and — for a lane — released to native AI by the real clock with its march clock run to the lane's end, then the real rest runs, and the fresh squad must stand at its own seating footing, at the authored count, with no mob of the previous life left and the routed state re-applied). **The undefeated re-seat.** A rest re-seats two more things, and both are gated on the body still being there rather than on a sentinel, so "undefeated" is asked of the world: (a) every **billed `elite`/`boss` wave** that does not declare `respawns_on_rest` — `execute if entity @e[tag=dw_wave_<id>] run function <ns>:wave_reseat_<id>`, the same two-line kill-and-respawn the stationed re-seat uses, so a boss chipped one hit per life comes back at full count and full health; (b) every **hostile actor** — an actor the campaign `unleash-actor`s anywhere (`combat::hostile_actors`, the compiler's one "unleash or nothing" definition of an actor that is a fight) — as `execute unless entity @e[tag=dw_pup_<id>] if entity @e[tag=dw_actor_<id>] run function <ns>:actor_restand_<id>`. `actor_restand_<id>` is `kill @e[tag=dw_actor_<id>]` then the twin summon at the actor's **absolute origin cell**, byte-identical to the body `unleash_<id>` produces. Three deliberate asymmetries: it puts the elite back **freed, never re-caged** (the `unleash-actor` beat fires from a one-shot trigger the engine never re-arms, so a re-caged elite would be dormant `Invulnerable` scenery for the rest of the delve); it does **not** re-apply the striker aggro lock (nobody has provoked this body — it stands on its anchor under vanilla-local AI, inside the `follow_range` `DW0478` measured the fire against); and it leaves a **caged puppet alone** (a puppet is `NoAI`, knockback-immune and normally `Invulnerable`, so combat can neither damage nor move it, and re-seating one would only undo authored `move-actor` staging). A killed or `despawn-actor`ed body selects nothing, so a **defeated boss stays dead** by construction (spec-0016 §1) with no state to keep. Generated PackTests `souls_reseat_actor` (stage the elite, unleash it, drag it onto the party, chip it to 1 HP and brand it, then run the REAL rest: one body, unbranded, within 2 blocks of its origin, no puppet — then kill it, rest again, and nothing comes back) and `souls_reseat_undefeated` (the same claim for a boss wave: ground down to a branded survivor, restored whole and unbranded by a rest; killed outright, never conjured back). Both re-seats also ride `cp_on_respawn_<i>`, because a death at a bonfire owes the party the same scene. Where a bonfire may STAND is proven separately: `DW0478` forbids one inside any wave's or fighting actor's aggro range — seated cells and lane polyline alike — because the fire is where the party respawns and where every `respawns_on_rest` wave is put back on its feet. |
| `begin-stealth` / `end-stealth` | `begin` → `#stealth dw.sys = <session>` + reset per-player `dw.st_grace`. `tick` runs `stealth_tick_<session>` while active → per-player `stealth_eval_<session>`: safe iff inside some zone box (a pure position selector — **zone presence alone = hidden**; there is no sneak requirement, which would collide with the spectator cutscene camera); grace resets when safe, climbs when exposed, and at `grace_ticks` fires `stealth_caught_<session>` (`on_caught`). `end` → `#stealth dw.sys = 0`. The `v06_stealth` PackTest disarms `#stealth` (sets it 0) after each `stealth_begin` because it drives `stealth_eval` explicitly: an armed session would make the world `tick` loop run a *second* judge pass in the same tick, double-counting exposure and mis-accruing grace (this only isolates the test; runtime gameplay has the tick loop as sole caller). It pins its dummy by tag (see "PackTest batch model" below), drives hidden/exposed purely by teleporting the dummy in/out of the zone box, runs the spare (safe-player) section first and the `on_caught` trip LAST — the trip executes arbitrary campaign `on_caught` content (possibly lethal), so nothing state-dependent follows it and the closing assert reads the dummy through the tag, which keeps matching even if the trip killed it. |
| `give-item` `carrier` (v0.6, spec-0018) | Absent/`all` → `give @a <item> <count>`: a quest beat arms the whole party. `one` → `give @s …`, the single quest prop handed to the player whose action fired the effect, for the party to pass around physically. `one` inside a scheduler-only bundle has no acting player and is rejected at validate time (`DW0357`). |
| trap `payload` — detection (spec-0022) | The compiler owns the detection tick, because the consequence is now commands. Two primitives, both already in the compiler, **none of them block-power polling** (which spec-0011 excluded as folklore): a `pressure-plate`/`tripwire` is a POSITION test on the trigger cell (`execute … if entity @a[x=…,dx=0,…]`, the `reach-anchor` idiom), and a `trapped-chest` is the v0.4 interaction-entity `use` — the same primitive the disarm affordance uses. Edge-triggered on a `#trapfire_<trap>` sentinel so stepping onto a plate is ONE event; a `rearm` trap clears the sentinel when the cell is vacated, a `once` trap never does (which is exactly the survivability discharge `DW0342` reasons about). Guarded by the flag gate when the trap declares one, and by the disarm latch when it has a disarm — load-bearing in a way it was not for redstone, since a command payload has no ammunition to empty. `trap_fire_<trap>` then runs the bundle under `Audience::Scheduled`: a trap is the dungeon firing at the party, not at whoever touched the plate, so player-facing effects address `@a` and there is no `@s`. A trap with no `payload` emits none of this (byte-identical). |
| `volley` (spec-0022) | One start function fans out into one function per salvo via `schedule` — the `sequence` shape, so **a volley costs nothing per tick**. Each salvo is (1) the *saturation*: one projectile per standable kill-zone cell, unconditional, with the compile-time velocity that reaches that cell — this is the contract, and it is why moving between salvos does not help; and (2) the *aimed extra*: a second projectile toward whichever cells hold a player this tick, selected by a plain block-volume selector, so standing still costs double fire. Both use compile-time velocities: there is no runtime vector arithmetic and no scoreboard math, because vanilla has no primitive for a runtime-aimed projectile and inventing one would be exactly the folklore the no-hack doctrine forbids. Projectiles are `NoGravity` (so the flown path IS the proven segment — drag scales speed without turning the line), `crit:0b` (deterministic damage; a random crit bonus would make the PackTest flaky) and `pickup:0b` (no loot litter in adventure mode). Speed is 2.5 b/t: arrow impact damage is `ceil(|velocity| x damage)` with `damage` defaulting to 2.0, so each arrow lands 5 half-hearts — a real consequence that three saturating salvos can kill, without any single arrow being an instant death. Coverage is proven at compile time (`DW0442`) and the zone must be watchable from safe ground before the player walks into it (`DW0388`). |
| `collapse` (spec-0022) | Summon one `falling_block` per region cell that holds a block (`HurtEntities:1b` for impact damage), then `fill` the region to air — the buried-alive beat redstone cannot express at all. The debris is settled deterministically at compile time (each column onto the first solid cell beneath it) and that post-collapse world joins the completability model (`DW0445`). An authored `then_floor` paves the settled surface via a scheduled second function, delayed by the computed fall height, because the rubble is still in flight when the trap fires. |
| trap `dispense` (spec-0011) | `setup_finish`: `item replace block <disp> container.0 with <item> <count>` fills the prefab's pre-wired dispenser socket (the `anchor/trap` metadata `dispenser` cell) — a static, deterministic payload, the same mechanism as a `collect` chest. **No detection** is emitted for the harm: the plate/tripwire/trapped-chest → dispenser redstone is already in the prefab. Pressure plates and tripwire are modelled **passable** in the assembled occupancy (`crate::assembled::is_passable_trap_trigger`) so nav routes a player ONTO a trigger cell rather than around a "solid" plate. |
| trap `requires_flags` / `forbids_flags` (spec-0011) | A **physical** gate, because the compiler owns world mutation: `trap_gate_on_<trap>` restores the trigger block declared by the `anchor/trap` metadata's `trigger_block` (verbatim, blockstate and all) and `trap_gate_off_<trap>` clears the cell to air, so a shut gate means a player stepping on the trigger steps on nothing. Edge-triggered on a `#trapgate_<trap>` sentinel, so the `setblock` fires on a flag transition rather than every tick. The gate is **campaign state, not per-player state** — flags are set by whoever reaches the beat — so the `tick` guards use the any-player form (`if entity @a[scores={dw.f_<flag>=1..}]`), one shutting clause per gating flag ("not (all required and no forbidden)" is a disjunction) and one opening clause carrying the full conjunction. `setup_finish` seeds the sentinel to the world the campaign starts in: a `requires_flags` gate starts shut (no flag is set yet) and clears the cell immediately, a `forbids_flags`-only gate starts open on the prefab's own block. An **ungated** trap emits none of this (byte-identical). Only sound for a trigger whose whole state is the block — `DW0363` rejects the rest rather than shipping folklore. PackTest `v06_trap_gate`: flag set → the trigger cell is air; flag cleared → the authored trigger is back. |
| trap `disarm` (spec-0011) | `setup_finish` summons a `minecraft:interaction` at the disarm `via` cell (tag `dw_trapdis_<trap>`) — Alongside the hitbox the compiler summons its **visible hardware** — a glowing, collision-free `minecraft:item_display` at the same cell, tagged `["dw_marker","dw_hw_<tag>"]` (a `minecraft:lever` icon). `minecraft:interaction` is invisible, so the hitbox alone is a right-click target the player cannot see: the drowned-bell soft-lock (`DW0420`). Visibility is the compiler's, never the tileset's. `trap_disarm_<trap>` kills it as the lever is thrown (`DW0421`).; `tick` fires `trap_disarm_<trap>` once on a right-click (`nbt={interaction:{}}`, reusing the v0.4 `use` primitive). `trap_disarm_<trap>` sets the party-wide `dw.f_<flag>` and empties the dispenser (`data modify block <disp> Items set value []`) — the modeled, global disarm that actually stops a redstone dispense trap. |

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
v0.7 cast ledger (spec-0020): the per-player scene selector `dw.cast`, and one
`#bk_<npc>_<scene>` bark-pool counter per pool on `dw.sys`.
`CustomName` is a plain SNBT text component (not `'{"text":…}'`).
v0.6 checkpoints/stealth (spec-0012/0014): storage `dw:cp pos` (last-checkpoint
mirror, a `[x,y,z]` int list); scores `dw.deaths` (`deathCount`) + `dw.death_ack`
(+ `dw.death_seen`, the corpse-side ack, only when the campaign declares `on_death`),
`dw.st_grace`/`dw.st_safe` (no sneak-stat scores — the judge is position-only);
markers `#cp`/`#stealth` on `dw.sys`. A campaign with a cutscene also ships the
datapack predicate `<ns>:sneak_held` (the cutscene bounce's re-attach gate, §4).

---

## 4. Hard invariants

### A campaign is judged at its DECLARED `dsl_version` (the obligation fence)

The compiler processes a campaign according to the
`dsl_version` its stage documents declare. A campaign that compiled before keeps
its behaviour unchanged; a new engine requirement reaches it only when that stage
adopts the version which introduced the requirement.

This is **not** a promise of byte-identical emission forever. A released delve
reproduces through its pinned engine (`versions.toml` + the OCI image, ADR-0010).
The promise is about **verdicts and behaviour at a declared version**.

#### Why it needed a mechanism

Per-stage fences already guarded new **surface** — "you may not write this field
below version X" (`DW0141`). Nothing guarded new **obligations** — "you are now
required to have X" — so whether a check respected a declared version depended on
whether its author remembered. Measured cost: `dsl::l10n::each_string`
was widened onto an actor's own `name` with no version gate, `DW0180` compares key
SETS and had no version gate either, and the obligation reached every campaign at
every declared version on the next build. `nobodys-cave-island` (0.6.0/0.8.0) went
red mid-staging with nothing in its own documents changed.

#### Half 1 — every code declares when it starts binding

A `Diagnostic` can only be built from a `DwCode`, and a `DwCode` can only be built
by naming which kind of rule it is (`dsl::diagnostic::Binds`):

| Declaration | Meaning | Choose it when |
|---|---|---|
| `DwCode::every_version("DWxxxx")` | Applies at every declared `dsl_version`. | The rule judges what the document **says** — a malformed id, an unknown item, a surface used below the version that introduced it. It cannot go red on a campaign whose documents did not change. |
| `DwCode::since("DWxxxx", n)` | Applies at minor version `n` and above. | The rule **requires** the campaign to have something. Campaigns below `n` are grandfathered and adopt it in their own version round. |

There is no constructor for "did not say", so *forgot to fence* is not a mistake
that can be made. Both directions are wrong in different ways — fencing a
wellformedness rule would stop rejecting bad documents in old campaigns — so
neither is a default, and there is no `Default` impl.

The `&'static str` code constants that remain in `delve-schem`, `delve-admit` and
`delve-render` are deliberate: those diagnostics are about prefabs, schematics and
renders, artifacts that carry no `dsl_version`, so there is nothing to grandfather
against. `tools/check-dw-codes.py` resolves both forms.

#### Half 2 — the fence is the only exit

`dsl::fence::Fenced` is the type `delvec` prints and derives its exit code from,
and its only campaign-aware constructor is `Fenced::apply(&campaign, diags)`,
which withholds every `Since(n)` diagnostic whose **stage** declares less than
`n`. A diagnostic's stage names a stage document (`quests`, `dialogue`, …); any
other stage (`l10n`, `prefabs`, `build`, empty) is judged at the **minimum**
across the campaign's stage documents — a campaign is only as adopted as its
least-adopted stage, and the minimum is the reading that grandfathers.
`Fenced::structural(diags)` is the pre-parse path (a stage document that did not
parse, so there is no version to read); it cannot grandfather, so it refuses to
carry anything version-scoped.

Every run states the fence's **binding count** on stderr when it is non-zero
(`obligation fence: N finding(s) grandfathered …, DWxxxx xN`), so a green
campaign also says what it is not yet answerable for. A silent fence and an
absent one are indistinguishable (CLAUDE.md: a green gate that binds to nothing
is vacuous, not a pass).

Currently declared `Since`: `DW0480`–`DW0485` (spec-0025, `since 8`). `branch.rs`
holds no `is_v08` guard of its own any more — a rule with two fences is a rule
whose two fences can disagree.

#### The other granularity: a check whose BINDING widens

A per-code fence sees a *new check*. It cannot see an *existing check examining
more objects*, a shape that adds no code at all. Such a check versions its own
binding at whatever granularity that binding has. The worked
instance is the l10n inventory:

- `l10n::each_string` passes a `KeyEntry` at **every** emission site — the stage
  whose `dsl_version` governs the key, and whether the key has been inventoried
  since its surface existed (`KeyEntry::always`) or was added to the walk later
  over surface older campaigns already had (`KeyEntry::since`). There is no
  two-argument `f`, so a new site must answer.
- `l10n::inventory` is unchanged: every key, tagged and translated as before, so
  **emission does not move**.
- `l10n::required_inventory` is the subset a campaign's declared versions have
  reached. `DW0180` (missing) reads it; `DW0181` (orphan) still reads the full
  inventory, so a campaign that translated a not-yet-demanded key early is never
  told it is an orphan.

Currently declared `KeyEntry::since`: `actor.<actor>.name` at 0.10 — v0.6 surface
added to the walk at 0.10. A campaign below 0.10 is not asked for actor
nameplates; at 0.10 it is.

### A scheduled bundle has no `@s` (executor contract)

`schedule function <ns>:<f> <n>t` re-invokes `<f>` with the **server** command
source: no executor, so `@s` resolves to nothing and every `@s`-addressed command
in it *silently does nothing* — no error, no log line. Three generated bundles are
reached only that way, and all three used to be emitted verbatim:

| bundle | reached from |
|---|---|
| `mv_arrive_<key>` | `mv_tick_<key>`, itself re-scheduled every tick |
| `ma_arrive_<key>` | `ma_tick_<key>`, likewise |
| `seq_<key>_<i>` | `seq_<key>`'s `schedule … <at_ticks>t` chain |

The cost (round-6 island, AUDIT-P0): two `on_arrive` bundles set the flags
`obj/take-cover` gates on, so the party soft-locked at "Get Into the Shadows" —
and the whole seal cinematic's `title`/`tellraw`/`playsound` beats were dead.

**The rule.** A bundle is emitted for an explicit **audience**
(`emit::Audience::{Party, Scheduled, Solo}`), and each effect is classified
individually, never the bundle as a whole (`emit_quest_effect` takes the audience
selector; the executor match is exhaustive — a new effect verb must state its
scope or the compiler refuses to build):

- `Party` — a party event entered as one player (`complete_<obj>`,
  `complete_q_<quest>`, `trig_<id>`): `@s` exists and is the completing player;
- `Scheduled` — the three bundles above: **no `@s` at all**;
- `Solo` — a checkpoint `on_respawn` / a stealth `on_caught`: the bundle belongs
  to the one player it fired for, and stays `@s` throughout (re-broadcasting one
  player's death would re-gift and re-narrate at every survivor).

Under `Party`/`Scheduled`:

- **player-facing** (`narrate`, `give-item`, `play-sound`, `damage-players`) →
  the command names `@a` directly (`tellraw @a`, `give @a`, `damage @a[…]`,
  `playsound … @a`), so the whole party sees the beat **once**. The one
  listener-relative form (a `players` sound with an explicit volume/pitch, which
  forces a `~ ~ ~`) is wrapped `execute as @a at @s run …` so `~ ~ ~` resolves at
  each listener rather than at the command's own position;
- **party-fact** (`set-flag` — now a `#party` write, gates, `set-block`,
  `spawn-wave`, `spawn`/`despawn`/`move`/`unleash-actor`,
  `spawn`/`despawn`/`move-npc`, `cutscene`, `set-time`/`set-weather`,
  `set-checkpoint`, `begin`/`end-stealth`, `sequence`, `campaign-complete`) →
  emitted **bare**, so it fires exactly once. A blanket `execute as @a run
  function <bundle>` — the obvious fix — is wrong: it would fire every `fill`,
  `summon`, driver start and `schedule` once per player.

**spec-0018 narrowed this seam to nearly nothing, and that is the point.**
Progression moved to the `#party` holder, so a scheduled `set-flag` writes
`scoreboard players set #party dw.f_<flag> 1` and names no executor — the exact
soft-lock the AUDIT-P0 fix was written for **cannot recur for flags**. Every
player-facing effect addresses `@a`, which needs no executor either. Exactly one
construct is still executor-shaped: a `carrier: "one"` `give-item`, which needs
the acting player, and is therefore rejected at validate time inside a
scheduler-only bundle (`DW0357`).

Per-effect flag gates have **one spelling** everywhere now (`if score #party
dw.f_<flag> matches 1` / `unless score #party …`, unset-safe): flags are party
state, so there is no per-player variant to diverge and no "does some player hold
it" selector to approximate it with. These bundles previously dropped the gate
entirely (they called the ungated emitter), so a gated effect inside an
`on_arrive`/`sequence` step fired unconditionally.

**`sequence` is a global timeline.** *Every* step function is emitted
server-source-safe, the inline `at_ticks: 0` one included — a timeline whose first
beat behaved differently from its second would be a trap, and `seq_<key>` is itself
reachable from a scheduled bundle (a `sequence` nested in an `on_arrive`).
Consequence: a sequence's per-player beats address the **party**, not one acting
player, wherever the timeline is started from.

**Enforcement** (all three; never relax one):

1. `tests/scheduled_executor.rs` walks the emitted call graph from every
   `schedule` site — following `function` calls that do *not* re-bind the
   executor — and asserts no function in that closure names `@s` outside an `as`
   clause (`positioned as`/`rotated as` do not bind). Fails on pre-fix output
   with the exact dead commands listed. Post-spec-0018 it passes because nothing
   in those bundles addresses a player at all — which is the strongest form the
   lesson can take.
2. Two generated PackTests drive the **real scheduler** (never an inline
   `function` call — running the driver inline *as the dummy* supplies exactly
   the executor the scheduler withholds, which is how a green suite hid this bug
   for a milestone): `sched_executor` (unconditional, so every campaign proves
   the seam live — it schedules a probe function emitted by the real
   scheduled-bundle emitter and awaits the flag on its own dummy) and
   `sched_arrive_flag` (the content path: the first `move-npc` whose `on_arrive`
   sets a flag; runs the real start function and lets the driver walk itself to
   the end). Both verified to go red on pre-fix emission on a live 1.21.11 server.
   Both now `await` the flag on `#party` and are the **sole owner** of the score
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
campaign pick its shape (the-wake escalation — a `sequence`-scheduled finale
made the old same-tick assert structurally unreachable, and the template drove
both branches' terminal objectives in one tick, a state no playthrough reaches).

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
other than this template is exposed. Measured on `souls-bonfire` and on the
`lift` fixture, both with the identical message; and `DW0807` now refuses the
emission that omits it. Zeroing is safe because every term zeroed is written
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
  `unless dw.q_*` guarded `on_complete` from re-firing — this shape meets the
  latch on its own second phase, which is where the baseline was first written),
  then drives ONLY that branch's flow playthrough in
  path order, emulating each branch-scripted dialogue option's `set-flag`s
  immediately before its `talk-to` drive (the real playthrough sets them there;
  a UI click is not available to a dummy). It then schedules
  `pt_camp_check_<i>` at `tail_i + 20t`, which counts `#party <completion> ==
  <value>` into the template-owned `#camp_phase dw.sys` and starts the next
  phase's run. The template's single closing `await score #camp_phase dw.sys
  matches <n>` (timeout `100 + Σ(tail_i + 20)`) demands every phase's verdict —
  a missed ending leaves the count short and times out red, never weaker than
  the old assert and now quantified over branches. Campaigns without
  `branch_points` are untouched by this shape.

### Semantics never key on player-facing text

**No semantic verdict may key on player-facing free text** (item/NPC display names,
titles, blurbs, hints). Semantics live only in ids, structured schema fields, or
first-class declarations. The removed night-vision name heuristic (`light.rs`,
deleted in the v0.6 mitigation PR) is the cautionary precedent: it read a kit item's
display name for "night vision", so a renamed water bottle passed `DW0210` while
nothing in the shipped world granted night vision — a check that passed without the
feature existing. Player-facing text is also localizable, so keying on it makes a
verdict language-dependent (ADR-0006).

### The completion-marker channel (the bot's oracle)

The critical-path bot's ONLY evidence that something completed is a chat line of
the anchored form

```
[dw:complete <campaign_id> <token>]
```

`<token>` is `campaign` (the whole delve, from `campaign-complete`) or the
completing objective's own `obj/<kebab>` id (broadcast by `complete_o_<obj>`, as
the score flips, before that objective's effects run). Both are `tellraw @a`,
dark-gray. The harness matches the **whole line**, exactly (`harness/src/markers.ts`
mirrors `plan::marker_line`) — never a substring of a longer line.

Why this shape. Before it, the harness tested every chat line for the *substring*
`[Delvewright] complete <objective> <value>`, emitted only for the campaign. Two
holes, both observed live: nothing stopped authored or translated content from
containing that substring, and a `reach`/`interact`/`talk-to` step passed on
arrival/on the dialogue opening while its own objective never completed — a 22/22
green island run whose campaign had in fact completed at step 12, the last ten
steps hollow. Three properties now make a forged completion impossible rather than
merely unlikely:

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
they are watching, not playing. Current consumers:

- the **stealth judge** is skipped for them (`stealth_tick` selects
  `@a[tag=!dw_cutscene]`). The judge is the only writer of `dw.st_grace`, so
  skipping it freezes the clock — grace neither accrues nor expires, and
  `on_caught` cannot fire mid-cinematic. The restore deliberately leaves
  `dw.st_grace` alone: the beat resumes exactly where it paused (the judge is
  position-only, so there is no other stealth state to re-sync).
- **`damage-players`** skips them: every form of the verb is guarded by
  `tag=!dw_cutscene`.

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

**The `spectate` bounce is sneak-gated** (round-6 flicker fix). In spectator
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
stealth no longer asks players to sneak: holding sneak and spectator cinematics
are inherently in conflict, so no delve mechanic may require a held sneak.

Any future verb that *demands input* or *deals harm* joins this list. The origin
is a round-4 island playtest where the stealth clock kept running through a dolly
and the catch killed the owner mid-shot, desyncing the beat.

### Shot styles (`shot_style`, v0.6 — spec-0015, camera dossier §2)

A styled shot expands at compile time into the same dolly + aim geometry an
explicit `path` produces — a pure function of (style, params, subject
geometry). `dist` is the only "lens" control (vanilla has no in-game FOV);
durations default from the dossier's film-editing ranges; every expanded path
runs the same `DW0308` clip (authored + rendered chords) and `DW0347` angular
budget as a hand-authored one. Placement is rule-based (no world-aware
candidate scoring yet — dossier §4's compile-time ClearShot is future work);
`bearing` (compass degrees: 0 = camera south of subject, 90 = west) steers the
placement, and an explicit `path`/`look_at`/`seconds` overrides any part.
Entity subjects (`npc`/`actor`) aim one block above the feet cell (torso)
before `offset`; `anchor` subjects use the block centre exactly.

A `subject` is discriminated by its key — exactly one of `anchor` / `npc` /
`actor`, plus an optional `offset`, **and nothing else**. Each spelling is its own
`deny_unknown_fields` type (`AnchorSubject` / `NpcSubject` / `ActorSubject`), so
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
scores, own init** (round-5 + round-6 island reds; `pin_dummy` in `emit.rs`;
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
  forbidden: they hit every coexisting dummy (`verb_flag_gate`'s withheld flag
  arrived via `verb_interact`'s old `@a` preamble).
- **Own scores.** Fake-player scratch holders on `dw.sys` are batch-global, so
  every template suffixes its own (`#n_sidm_<actor>`, `#bx_bret`,
  `#dm_<npc>_<node>`, …); no two templates share a holder. Real runtime scores
  (`#stealth`, `#placed`, `#trig_<id>`, the
  `#mt_`/`#at_`/`#arun_`/`#mgen_`/`#mown_`/`#agen_`/`#aown_` move drivers,
  `#lane_<wave>`) are deliberately shared — tests drive them and initialize them
  explicitly. The line between the two is **who writes the holder**: a name an
  emitted campaign function owns is runtime state no template can suffix, so the
  census answers `#wcen_n`/`#wcen_b`/`#wcen_d` (written by `wave_census_<wave>`)
  and the party-unique kit latches `#kit_<class>_<k>` (written by
  `class_apply_<class>`) are runtime, not scratch — a template can only drive or
  reset them. Where a runtime answer is *asserted*, the template copies it into
  its own scratch first (`#wcn_<n>_<wave>`) so the assertion reads a holder it
  owns.
- **Own scores, extended to party state (spec-0018).** Progression now lives on
  the batch-global `#party` holder rather than on each test's dummy, so a
  template's baseline writes are visible to every sibling. Inside a template that
  is harmless — a template is one atomic mcfunction, so its baseline, its drive
  and its assert land in one tick with nothing in between. It stops being
  harmless the moment a template spans ticks: `party_state_across_ticks_is_owned`
  requires that any template containing an `await`/`schedule` be the **sole**
  template touching each `#party` score it uses (`sched_executor`'s probe flag is
  test-only for exactly this reason).
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
  writes saw none of that (19 of 216 fixture templates judged, every `campaign`
  among the unjudged). A template's own hoisted helpers (`pt_camp_drive`,
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
  an AND-join proves only that one player can do both arms in sequence — which
  was already true before the party holder existed. The generated
  `party_join_<obj>` template therefore drives **n different players**, one arm
  each, and asserts the join's REAL emitted `pending_guard` (materialized into
  `#pj_<obj> dw.sys`) in three phases: shut with no arm, **still shut after only
  one** (the negative half that makes it an AND, not an OR), open after all of
  them — and then has the LAST member, never the one who cleared the first arm,
  complete the successor. n = the join's arm count, raised to `world.min_players`
  and capped at 4 (the party maximum); arms are handed out round-robin.
- **`assert` does not abort the template, and the log names the LAST failing
  line.** Measured 2026-08-04 while proving a seal by mutation: a
  template with two failing asserts reported only the second, and flipping the
  first assert's expectation changed nothing about which line was reported. So
  the reported line is *a* failure, never "the first thing that broke" — read
  the whole template, and never conclude an earlier assert passed because the
  log did not name it. It also means every later assert still runs against
  post-failure state.
- **Drive the real mechanism, not a convenient stand-in.** A template that calls
  a *scheduled* driver inline (`function <ns>:mv_tick_<key>`) runs it **as its own
  dummy** — supplying exactly the executor the vanilla scheduler withholds, so the
  test passes while the shipped delve soft-locks (AUDIT-P0; §4 "A scheduled bundle
  has no `@s`"). Tests of scheduled machinery hand it to `schedule` and `await` the
  outcome (`sched_executor`, `sched_arrive_flag`); the pre-existing
  `v06_move_actor`/`v06_arrive_handoff` inline drives stay as entity-state
  assertions, which is all they ever claimed.

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
| — | `gamerule tnt_explodes false` (**v0.6-gated**, spec-0011): defense-in-depth against a stray primed-TNT source deforming the sealed world. No gamerule separates explosion block vs. entity damage, so TNT is excluded as a trap payload by the schema and belt-and-braces sealed here. Emitted **only** when the world stage is `dsl_version 0.6.0`, so pre-0.6 fixtures stay byte-identical. |
| — | `time set <kw>` (declared `world.time`, default `noon` = daytime 6000; the sole seal with a vanilla read-back) |
| — | `weather <kw>` (declared `world.weather`; emitted **only when declared** — `clear` is the vanilla default, so an undeclared campaign emits nothing and stays byte-identical to pre-0.5) |
| — | `difficulty <kw>` (declared `world.difficulty`, v0.6; emitted **only when declared**, so an undeclared campaign is byte-identical). The shipped `server/server.properties` already carries it, so this line is not what makes the delve *image* correct — it is what makes the DATAPACK correct wherever else it is loaded (the owner's own test save, a world whose properties someone edited). `/difficulty` is idempotent. |

- Gamerule *values* have no vanilla read-back → asserted at compile time only;
  PackTest asserts the two queryable seals: `time = daytime_ticks(world.time)`
  (e.g. 6000 for `noon`, 18000 for `midnight`) and, for a campaign that declares
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
  `generator-settings={"biome":"minecraft:the_void","layers":[]}`,
  `level-seed=<seed>`, `gamemode=adventure`. `difficulty` = the campaign's
  declared `world.difficulty` (v0.6) when it declares one, else the derivation:
  `peaceful` for wave-free campaigns, **`easy`** when any wave exists (peaceful
  removes summoned mobs). No server jar, no region files (ADR-0010) — the
  bootstrap `/place template`s prefabs, so byte-identity covers the whole tree.
  `view-distance=10` and `simulation-distance=10` (below). The file writes 15
  keys and the key set is asserted exactly
  (`crates/compiler/tests/server_properties.rs`): what a delve pins and what it
  leaves to the host is a reviewed decision, not a residue.
- **Chunk distances.** `view-distance=10` (160-block render radius) and
  `simulation-distance=10`, two keys answering two questions and pinned for two
  reasons. *View*: the largest shipped scene spans 114 × 165 blocks and the next
  35 × 115 (measured from the emitted piece `forceload` AABBs), so 160 blocks
  reaches the far side of either from any standpoint inside it, and on an `ocean`
  horizon it puts the fog line 160 blocks of open sea past the shore. It is also
  the radius `docs/notes/horizon-library-dossier.md` §3–4 and spec-0026 §6 do
  their vista arithmetic against. Perf is non-gating on the Pi, so prod does not
  force the number down; the absence of delve
  content past 160 blocks is what stops it going up. *Simulation*: not what makes
  a delve tick — `setup` force-loads every placed piece and never releases it, so
  scene chunks are entity-ticking wherever the party stands, and everything
  beyond them is inert backdrop. Its job is to make the ticking rim a **known**
  radius: with both pinned, the chunks that can tick or be seen are bounded by
  the force-loaded scene ∪ Chebyshev radius 10 (+1 loading margin) around any
  player — a bound a whole-plane proof can be written against, and one the
  compiler cannot state otherwise.
- **Unpinned keys.** The pinned server version writes 70 properties; the build
  pins 15. Every other key is left at the host default *on purpose*, and the
  distinction that matters is that a delve has two boot paths with two different
  default sources: the shipped image starts from the itzg base's own
  `/image/server.properties` template, the owner's playtest server
  (`tools/playtest-server.sh`, `OVERRIDE_SERVER_PROPERTIES=false`) copies the
  build's file in and lets the vanilla jar fill the rest. Measured on both paths
  against the pinned version, the 55 unpinned keys resolve as follows.
  - **Diverge between the paths, harmless**: `enable-rcon` (image on, playtest
    server off until `playtest-server.sh` appends its own), `rcon.password` and
    `management-server-secret` (both generated per boot). Operator transport,
    never world state; `management-server-enabled=false` on both, so the secret
    is inert.
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
    and the remaining transport/status/resource-pack keys — the delve's resource
    pack is installed client-side, never served through `resource-pack`.
- `horizon:"ocean"` (v0.6, spec-0013) swaps `generator-settings` for a pinned
  superflat `{"biome":"minecraft:ocean","layers":[bedrock×1, stone×118,
  water×8]}`: from the −64 build floor the top water block lands at **y=62** (sea
  level). Still no structures/mobs (`generate-structures=false` + gamerule
  `spawn_mobs false`). `void`/absent is byte-identical to v0.5.
- **Sea-level datum (ocean).** An ocean world places its areas at **y=60**
  (`plan::OCEAN_BASE_Y` = `SEA_LEVEL − ISLAND_WATERLINE_Y`), not at the void
  datum 64. The island tileset (`prefabs/island-tileset.md`) authors every piece
  with its waterline — the top authored water block — at **local y=2** and its
  walkable land plane at local y=3; placing the base at `sea_level−2` makes the
  authored water one body with the world ocean and puts the shore exactly one
  block above the sea, the vanilla-normal beach a swimming player can climb.
  Placed at 64 instead, the whole island floats ~4 blocks above the sea: the
  authored water pocket hangs in the air and open water becomes an inescapable
  moat. *Assumption:* the waterline height is a **library** constant, not a
  per-piece one — placement uses the single tileset convention (2) so every area
  of an ocean world shares one datum, and prefab metadata's optional
  `waterline_y` is a *declaration checked against* that datum (`DW0344`), never
  an input that moves it. Everything downstream (nav/critical path, boundary
  region, checkpoint storage, POV shots, PackTests) derives from placement and
  simply follows the new Y. The flood model is unaffected: it seeds only
  from authored free-fluid cells inside placed pieces and never climbs,
  so the walk plane one block above the waterline stays dry by construction —
  the world ocean is backdrop, not a flood source.
- Prefab metadata may declare **`waterline_y`** (optional, integer, local y of
  the piece's top authored water block). Island-tileset pieces declare `2`;
  pieces that author no sea (keep/cave interiors, `hello-room`) omit it and are
  not checked. Consumed only by `DW0344`, which reports how many placed pieces it
  examined: an ocean world where that count is zero is reported by `DW0344`
  itself rather than passing silently, because a check keyed off an optional
  field goes quiet when the field goes missing, and a check that examined nothing
  has proved nothing. There is no exemption — the only one an author could claim
  ("this piece needs no waterline") is the missing declaration under another name.
- Prefab metadata's `lighting.profile` takes a fourth value, **`unmeasured`**
  (spec-0027 §2): a *generated* prefab places blocks, not photons, so it declares
  that a probe is owed rather than fabricating one. It is distinct from an absent
  `lighting` block (which means metadata predating the field), and the
  measurement fields stay mandatory where they are claimed: a `lit`/`dim`/`dark`
  profile without `measured_min_light` + `measured` is refused at parse
  (surfacing as `DW0346` for a library file), and an `unmeasured` profile
  carrying either is refused too. Nothing gates on the profile (`DW0210` measures
  the assembled world); its one consumer is the interior shot's reviewer line,
  where `unmeasured` reads "verify readability", never "mitigation expected".
- `boundary` (v0.6, spec-0013) emits, in `setup_finish`: a `dw:region bounds`
  storage mirror (readable region contract), a `dw:cp pos` init to the spawn cell
  (shared with spec-0012 checkpoints — the last-checkpoint mirror the return
  reads; idempotent, gated once via `needs_cp_init`), and `schedule function
  <ns>:boundary_tick 20t`. `boundary_tick` (self-rescheduling 1s clock) ejects
  every `@a` outside the region via `boundary_return` (a macro `$tp @s $(x) $(y)
  $(z)` off `dw:cp`, + actionbar message + soft sound). The region selector is
  compile-time-derived literals; nothing is authored.
- **Entry point.** One cell per **area** — the cell a body arrives at when it
  enters that area. It is the anchor whose prefab metadata declares
  `"role": "entry"`. A campaign addresses every other anchor by name; this is the
  one the compiler has to *find*, so the piece says what it is for rather than
  being recognised by what it is called.
  Where **no** anchor in an area declares the role, the compiler falls back to an
  ordered name list (`plan::ENTRY_ANCHOR_NAMES` = `spawn`, then `entry`) — the
  two spellings the shipped tileset library uses (keep/cave/test say `spawn`, the
  island tileset says `entry`). That list is a **compatibility path for pieces
  admitted before the role existed**, not a second authoring surface: a piece
  written today declares the role, and an area that declares one is never
  reached by a name at all, so a piece cannot acquire the campaign's start by
  calling one of its anchors `entry` for its own reasons. Two anchors in one area
  declaring the role is `DW0804`.
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
  The prefab **viewer** asks the same question for a different purpose — which
  anchor a review page should open on — and prefers a declared role over its own
  wider list of name stems (`spawn`, `entry`, `entrance`, `threshold`), which is
  a guess about one piece and is consulted only when the piece does not say.
- **The class trigger is ONE-SHOT per player, sealed in the pack.**
  `class_apply_<c>` ends in `teleport @s <entry point>`, so a
  `dw.class` trigger left armed after a class is a live warp back to the start of
  the delve, usable at any point in a run by anything that can chat a command.
  `tick` used to `scoreboard players enable @a dw.class` unconditionally, every
  tick, forever; it now runs `execute as @a run function <ns>:class_arm`, whose
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
  the campaign entry point (the first area's `spawn` anchor — the same cell
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
  (`resource_pack_sha1` in manifest) for a skinned campaign.
- `<out>/manifest.json` carries exactly `campaign_id`, `delvec_version`,
  `dsl_version`, `mc_version`, `inputs` (SHA-256 per authored document the build
  read, l10n sidecars included) and `outputs` (SHA-256 per emitted path), plus
  `language` on a non-`en` bake and `resource_pack_sha1` on a skinned campaign.
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

  The harness **requires** the block and refuses a path without it. The only
  fallback available to it is a literal set of entity names living in the harness,
  which is right only for the campaigns whose author happened to pick those
  bodies — that fallback is the defect, not the safety net.

  **A `talk-to` step's `pos` is the CAST LEDGER's**, not the NPC's stage-2 anchor
  (island-release blocker). The stage-2 `anchor` is only where a body is first
  summoned; a `move-npc` walks it away and the quest's `cast` row records where it
  then stands (`DW0461` proves the row equals the effect history). Reading the
  anchor here made the bot contract a second, staler source of truth: on the
  island `npc/perimedes` is declared at `anchor/mouth` and cast at
  `anchor/alcove-2` for `obj/the-stone`, so the eye-ray bot walked to the mouth —
  behind the sealed boulder region's wall of interaction entities — and could not
  acquire him, while the emitted cast had the body in the alcove all along. The
  row is chosen by `cast::station`, the one model the emitted `cast_<npc>`
  selector and `DW0483` also read: clauses accumulate in quest-DAG order over the
  quests this playthrough activates, **later declarations win** (`dw.qa_<quest>`
  is never cleared), and within a quest the last placement whose
  `requires_flags`/`forbids_flags` gate holds under the flag state the party
  carries into that step. Consequences: a ledger that moves a body across areas
  moves the step's area too, so the inter-area `transport` map follows; a row
  resolving to `"offstage"`/`"dead"` is an internal-invariant error (`DW0195` /
  `DW0461` own the refusal upstream). A campaign with no ledger anywhere (pre-0.7)
  keeps the stage-2 anchor and stays byte-identical.

  **A `reach` step carries the volume the SERVER adjudicates in**, as
  `completion`, beside the authored `radius`. The two are different facts and the
  bot uses only the first. `reach::reach_completion` computes it once and all three
  readers take it from there — the `tick` line's `@s[…]` selector is formatted
  from the same value — so the artifact and the datapack cannot describe different
  regions. Shape: `{"kind":"cube","lo":[x,y,z],"hi":[x,y,z]}` at v0.3+ (inclusive
  block corners, half-extent `max(1, radius)`), `{"kind":"sphere","pos":[x,y,z],
  "radius":n}` for a v0.2 campaign, whose emission is untouched. Required, never
  optional: an optional field with a fallback is the harness keeping its own
  completion model, and that model is what went wrong. The harness derives its
  walk goal from `completion` and, on the failure path only, reports when the walk
  ended outside it — a positive precondition would false-fail every step whose
  completion legitimately teleports the player away. `tests/reach_completion.rs`
  asserts the emitted selector equals the exported volume for every reach
  objective of every campaign it builds, and drives the volume rule over every
  entry of `envelope::SUPPORTED_DSL_VERSIONS` rather than over named versions, so
  a new ledger row joins the proof with nothing to edit.

  **`ending_tail_ticks`**: the terminal `assert-complete` step carries
  the path's scheduled-ending tail — the compiler-computed maximum tick offset
  between the terminal objective completing and `campaign-complete` firing
  (`sequence` `at_ticks`, `move-npc`/`move-actor` walk durations; the-wake: 250t).
  The harness completion window becomes `max(15s, tail·50ms + 10s)` — widened,
  never narrowed. Omitted when the ending is synchronous, so a path with no scheduled tail
  is byte-identical. Emitted by the same computation the campaign PackTest's await
  timeout uses, and per branch on each `validation/branch-path-<branch>.json`
  (a branch waits out its OWN ending's tail).

  **`rest` steps** (spec-0016 §1, bell round-3 finding 2026-08-03). A bonfire arms
  an affordance and moves nothing until the party rests — souls-correct, and also
  invisible to a ladder that walked past every fire without touching one: the
  checkpoint never moved, so a die-retry trial respawned at world spawn (the beach)
  and blew the walk-back budget, judging the *campaign* for a *proof* that never
  performed the player loop. Resting is the intended loop, so the proven path
  performs it. After the step that arms bonfire `<i>` the path carries one
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
  sees exactly what it saw before, and a campaign with no bonfire is
  byte-identical. **That untouchedness is exactly what created two coordinate
  systems**, and they drift by one per bonfire armed earlier: internal indices
  (`fire_step`, `Encounter::step`, every nav proof) count `plan.critical_path`;
  exported indices count these `steps[]`. Every artifact a harness reads states
  the EXPORTED one, and `Plan::exported_step` is the single translation — a
  consumer that mixes them is a silent off-by-N, which is what the combat plan's
  `step` was until it was reconciled.
- `<out>/validation/critical-path-waypoints.json`: the DW0311-proven per-leg route
  thinned to sparse waypoints (`from`/`to` = the `critical-path.json` step
  positions; a waypoint at each corner/floor-height change **and the corridor commit
  cell one step past each corner**: a wide-room→corridor corner is
  range-1-satisfiable from an off-route pocket beside it, so the post-corner cell
  gives the harness a close corridor-axis target for its stall-recovery). A leg
  that walks through a closed fence gate carries a `use_gates` array:
  the gate cells the player right-clicks open (an adventure-legal USE), each also
  force-kept as an explicit waypoint (never thinned away mid-run); the field is
  omitted for gate-free legs, so gate-free campaigns stay byte-identical. The
  harness replays these as successive nearby pathfinder goals so no single distant
  A* solve strands the bot on a large open cave (its pathfinder's `canOpenDoors`
  performs the gate click, and the harness's fence-lip waypoint filter stands as
  defence-in-depth). **Validation metadata, not shipped gameplay** —
  excluded from the delve image (like `packtest-datapack/`); emitted only when a
  walked critical leg exists, so a fully-transported campaign stays
  byte-identical.
  A campaign with `timed_gates[]` (spec-0016 §4) additionally carries a top-level
  `timed_gates` table — one entry per declared gate, in declared order, with
  `id`, `region: {min, max}` (inclusive, canonical world-coordinate bbox),
  `block`, `open_ticks`, `closed_ticks`, `phase`, `crush` — and every leg whose
  proven route walks through one carries `timed_gates: [<id>, …]`. `crush`
  exports the §4-addendum fact that the closing edge KILLS a player
  caught inside the region: the first live crush gate (tide-mill's 36t/84t
  `timed-gate/tide`) killed the harness bot because its gate machinery was
  reactive — wait for a window only after a hop fails — which is safe when a
  closing gate merely aborts the path and lethal when it crushes. The harness
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
  inside one open window (18 blocks through a 5 s window on the-drowned-bell,
  which loses the race), where pinning the mouth splits it into an
  uninterruptible approach plus a short crossing — which is what `DW0378`
  actually proves admissible (the *span*, not an arbitrary run-up to it).
  **In-region cells are deliberately NOT pinned**: the harness treats
  every waypoint as an *arrive-at* goal, so a waypoint under the gate parks the
  bot there — and a `crush: true` gate then fills that cell with the bot in it,
  which is how the-drowned-bell round 2 killed its own bot at `[24, 63, -10]`.
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
  (spec-0025, DSL v0.8): the branch set — per branch, its flag assignment, its
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
  `gate_events` fire-steps and the strict-ancestor relation recomputed in the
  branch path's own step space (`Plan::branch_gate_model`) — a branch path is a
  different sequence, so default-path step indices are never carried across (the
  same trap `emit::rest_step_index` documents for bonfires). Each branch's
  routes also pass the `DW0314` standability self-check. Branch diagnostics are
  prefixed ``branch `<id>`:``. The harness derives the filename from the
  branch's `branch-path-<slug>.json` (one slug, one contract) and reports
  **loudly** — stderr + a run-report finding — when a branch must walk without
  it (single-goal fallback, terrain-flaky where waypointed navigation is
  deterministic: the failure mode that broke 3 of 4 island branch runs).
  Emitted only when the branch has walked legs and the campaign builds an
  occupancy model, so everything else stays byte-identical. Remaining per-branch
  proof scope (deliberately not yet quantified over branches): checkpoint
  no-stranding (`DW0315`/`DW0316`), stealth (`DW0327`/`DW0355`), traps
  (`DW0342`), shortcuts/ambush/timed-gate (`DW0373`–`DW0378`, `DW0388`), stair
  orientation (`DW0430`) — these still run on the default path only.
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
  **Emitted only for a campaign that declares a volume**, so a file that exists
  and reports zero is a finding rather than an absence.
- `<out>/validation/death-plan.json`: **the bot tier's contract for
  dying** (`compiler::deathplan`). A PackTest fake player is permanently
  undamageable — measured independently on 2026-08-03 and again on 2026-08-09 —
  so that tier cannot witness a player death at all, and every runtime claim
  about the death loop belongs to the mineflayer tier. This file is what lets it
  make one: the campaign's PROMISES, in the campaign's own terms.
  - `lethal_volumes[]` — each volume's inclusive box, its canonical-English
    `message` plus the `message_key` a localized run asserts instead, and its
    `damage_type`.
  - `on_death` — how many effects the bundle carries at every nesting depth
    (read through `QuestEffect::nested_effect_lists`, so a `sequence` is
    counted), and which stakes it drops.
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
  that declares a lethal volume, an `on_death` or a stake**, so every campaign
  written before spec-0031 is byte-identical.
- `<out>/validation/teleport-gate.json`: the `teleport` proof's **binding
  ledger** (`compiler::teleport`, spec-0031, playtest-methodology.md rule 1).
  `teleports.declared` vs `teleports.resolved` (a gap is an anchor no placed
  piece provides — already `DW0142`/`DW0360`), `cells` (the size of what the
  emitted selector sweeps), `affordances_examined` (every engine affordance
  tested against `DW0542` — `eclipse::affordances` plus the seal shells) and
  `packtest_templates` (the runtime half of totality, one per teleport; whether
  vanilla's `@e[<box>]` really reaches every entity type is vanilla's fact, not
  the compiler's, and a compile-time-only green over it is exactly the vacuity
  this number exposes). **Emitted only for a campaign that declares a
  teleport**, so a file that exists and reports zero is a finding rather than an
  absence.
- Every `snapshot` manifest carries a **`frame`** block: `targets_in_frame`,
  `targets_out_of_frame`, and `featureless` (`null`, or the distinct-colour count
  when the frame shows no scene at all). The binding rule applied to a picture —
  a render that succeeds, writes a file and is a rectangle of flat background is
  indistinguishable from one more shot taken to a directory listing, to a contact
  sheet and to a reviewer skimming, and the only thing that separates a camera
  aimed at the room from one aimed at a wall is the count. Judged by the arm that
  DREW the frame (`view::detect::is_featureless`), so a consumer never computes a
  second verdict on the same question. `tools/check-gallery-render.py` reads these
  back rather than re-deriving them.

- `<out>/validation/effect-roots.json`: the **effect-root walk's own binding
  ledger** (`dsl::RootBinding`, written by `emit::build_with_warnings`).
  `roots_enumerated` / `roots_total`, the total `bundles` and `effects`, a
  per-root `sites` count, and `unbound_roots` — the roots this campaign has no
  bundles at, listed rather than left to be derived. Most effect-shaped proofs in
  this compiler are only as good as the roots this walk reaches, and until this
  file existed the number was a **string on stderr**: nothing downstream could
  assert that a build's effect walk bound to anything. Emitted for every campaign,
  because the walk runs for every campaign; a zero at a root is not a failure (a
  campaign with no traps has no trap payloads) but it is the reason any proof over
  that root is unbound, which is what a reader needs and cannot infer. For scale:
  the gallery binds all eight roots, where the largest shipped campaign binds
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
- `<out>/validation/placement-gate.json`: the `DW0864` **binding ledger** (`compiler::edit`). One row per `scatter`/`plant` verb the build replayed — its batch, its index in that batch, what it `declared` (a `plant` count, or `null` where the verb states only a density), what it `delivered`, the `domain` of cells the author's region selected, and how many of those the verb could `act on` at all (`usable`). Rows are written whether or not any of them is short, because a rule that speaks only when it fires reads exactly like a rule that never looked. `binding` carries `verbs`, `scatter`, `plant`, `short`, `domain_cells` and `delivered`, every one of them derived from the rows rather than written beside them. Absent for a campaign with no edit script.
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
  nothing else). A second block, `declared` (DSL v0.11, spec-0034), states the
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
  both present and never contemporaneous. This exists because `DW0478` returned
  `Ok(())` on `nobodys-cave-island` for its entire life — three respawn points,
  six hostile forces, **zero comparisons** — and nothing anywhere said so.
- `<out>/validation/press-bodies.json`: the `DW0426` proof's **binding ledger**
  (`compiler::pressable::PressLedger`). One row per `strike`/`use` trigger the
  proof resolved a body for — `trigger`, `click`, `anchor`, and WHICH body it
  landed on (riding a `close-gate` seal's hitboxes, riding a shortcut door's,
  riding an NPC's dialogue hitbox, arming a region's clickable shell, or a point
  in open air) — plus `examined`, `unbound` and, exactly when unbound, a
  `reason`. `DW0426` is error-tier, so a build that ships proves no press lands
  on nothing; that sentence is equally true of a campaign that arms no press at
  all, and only the count tells the two apart.
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
  the watch ledger because `tools/check-gallery-coverage.py` reds on a top-level
  `examined: 0` and reads top-level keys only — nested, the count would have been
  written, committed and diffed and never judged. Zero on a campaign declaring
  none of the claimed mechanic is honest; zero on the gallery, which declares
  everything, is the finding.
- `<out>/validation/combat-plan.json` (spec-0023): the bot ladder's encounter
  table. A top-level `fights` block states the **binding count for the whole
  spec-0023 pass** — `waves`, `actors`, `total`, `unbound`, `reason`
  (`combat::mandatory_fights`): every fight the party cannot walk away from, of
  BOTH shapes, because reading `encounters` as "how much combat is in this
  delve" is what let a campaign that unleashes three wardens look combat-free
  and skip `DW0470`–`DW0475` entirely. Then one entry per **mandatory wave**
  encounter (a wave a `kill` step on the compiled critical path names), in path
  order, carrying `wave`, `objective`,
  `step`, `tier` (`ordinary`/`elite`/`boss`, from the wave's declaration), `pos`,
  `count`, `respawns_on_rest`, the `checkpoint` governing a death at that
  encounter, and the `census` probe naming the three functions that measure this
  wave by tag. The document also states the `difficulty` the run is verified AT.
  Three of those fields carry rules worth stating exactly, because two were once
  off by one and the third was once not stated at all:
  - `step` is an index into the **exported** `critical-path.json` `steps[]`
    (`Plan::exported_step`). Two coordinate systems came into existence with
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
  - `bodies` is what stands at this encounter and how long each body should take
    to fall: one entry per **entity kind** (client vocabulary), with `count` and
    `give_up_swings`. The budget is `check_winnability`'s own three numbers —
    declared `attributes.max_health`, the mob's resistance multiplier, and the
    best `attack_damage` weapon any class kit carries — as
    `max(GIVE_UP_SWING_FLOOR, GIVE_UP_SWING_MARGIN * ceil(effective_hp / hit))`.
    It replaces a fixed six-second melee timer in the harness, which was too long
    for a rat and too short for an elite and, when it fired, blacklisted the body
    and reported nothing. A body that outlives its budget is now a **finding the
    run report names** (`unkillable_findings`): either nothing in the party's kit
    can damage it, or the encounter's numbers are wrong.

    `give_up_swings` is `null` with a `reason` where the arithmetic cannot run —
    an undeclared `max_health` (Mojang publishes no per-entity default
    attributes, and this compiler refuses to invent a health table), or a party
    with no `attack_damage` item at all. The harness then gives up on nothing at
    that encounter and names the unbounded kinds if the step times out; it never
    substitutes a number of its own.

    Two stacks of one entity are **one** entry, because the bot reads a name off
    a body and never its NBT: the budget is the worst of them, and a single
    unproven stack makes the kind unproven. The margin and the floor are
    **authored, not cited** — no source gives the right multiple for a bot that
    never charges a swing — and are sanity bounds in the spirit of
    `TTK_BUDGET_HITS`: crossed by a body that is not dying at all, not by one that
    is merely tanky.
  - `census` is `{census, brand, unbrand}` — the fully-qualified ids
    of this wave's `wave_census_<wave>` / `wave_brand_<wave>` /
    `wave_unbrand_<wave>` functions. It exists so the harness calls what the
    plan NAMES: `safe_local` is a compiler naming rule, and a harness
    re-deriving it would be the downstream folklore CLAUDE.md forbids. Present
    on every encounter; a plan that cannot state it is refused by the harness
    rather than silently measured by silhouette.
  - `checkpoint` is the last checkpoint/bonfire fired **strictly before** the
    step, omitted when there is none. Strictly, not "at or before": a
    `fire_step` is the step whose COMPLETION arms the checkpoint, and a death
    *during* step `i` happens while step `i` is unfinished — so a checkpoint
    armed by step `i` does not exist yet at that death. The `<=` form handed an
    encounter a respawn point one beat in its own future, which the souls-bonfire
    fixture shows at its sharpest: bonfire 0 is armed by `obj/slay`'s completion,
    the very kill the encounter IS, and the plan claimed a mid-fight death would
    return the party to that fire when in truth it returns them to world spawn.
    Erring toward the stricter answer is deliberate — the die-retry stage asserts
    the party respawns at the governing checkpoint, so an over-generous claim
    here makes the proof measure the delve against a rest point the player never
    had. (A bonfire additionally only MOVES the respawn point when the party
    rests; the harness's own precondition covers that half.)

  Together these are what turn a `kill` step into a verified encounter for the
  harness: which fights get the die-retry stage, where a death is supposed to put
  the party back, and which fights are billed hard enough for the inverted floor
  gate to have an opinion.
  Two further top-level keys, both **additive** — nothing was moved
  or renamed in `encounters[]`, and nothing else may be poured into that array,
  because "there is a checkpoint a death here returns to" is a property only a
  critical-path `kill` step has:
  - `actors[]` — every **tier-declaring stage-5 actor**, in declaration order:
    `actor`, `entity`, `name?`, `tier`, `anchor`, `pos` (the resolved cell),
    `tag` (`dw_actor_<id>`, worn by both the puppet and the unleashed twin),
    `vulnerable`, the declared `attributes` (the body that actually fights), and
    `spawned_by` / `unleashed_by` — the beats that stage and unleash it, each
    naming its `site` (`trigger`/`quest`/`objective`/`trap`), `owner` id, JSON
    `path`, and for a trigger its `on` event kind plus the `at` anchor or `npc`
    body it watches. That last part is what makes an actor fight *runnable*: a
    wave encounter has a critical-path step the bot already knows how to start,
    while an actor fight starts because something got struck, used or walked
    into, and only this states which. The beats are collected by the one shared
    effect traversal (`for_each_campaign_effect`), so `sequence` steps,
    `on_arrive` reactions and desugared ambushes are all seen exactly as
    emission sees them.
  - `floor_gate` — the **coverage ledger**: `covered[]` and `not_covered[]`, each
    entry `{kind: wave|actor, id, tier}` and every uncovered one carrying its
    `reason`. This exists because of how the floor gate reports: it warns when
    the unassisted bot beats a billed elite first-try and says nothing
    otherwise, so an encounter that was never fought produces exactly the same
    silence as one that was fought and lost. The ledger makes "not covered
    (reason)" a fact the run report must print rather than an absence it can
    read as a pass. The same finding is raised to the author as `DW0477`.
    **Untiered hostiles are on the ledger too**: an EMPTY ledger
    read as "everything is covered" when what it meant was "nothing was even
    assessed", so every actor the campaign turns loose on the party while
    declaring no `tier` is a `not_covered` entry with `tier: null` (an explicit
    null, never a dropped key) and a `reason` naming it `UNTIERED`. Hostility is
    read off the campaign's own declarations by the same "unleash or nothing"
    rule the die-retry / assist machinery uses — an `unleash-actor` beat gives
    the body real AI and it swings back; a staged puppet is `NoAI` and
    knockback-immune, so it never attacks and is not a hostile — and is never
    inferred from the species (`DW0469`'s rule). A tier declared `ordinary` is a
    statement and stays off the ledger; an absent tier is not a statement, and
    that difference is the whole entry. **No `DW0477`** for these: that
    diagnostic is about a *billing* the gate cannot hold, and nothing was billed
    — the ledger line is the whole record.
  - `floor_gate.examined` / `.unbound` / `.reason`, and a sibling top-level
    `actors_gate` (`{examined, unbound, reason?}`) — the **binding counts**
    (playtest-methodology.md rule 1, task following the island's round-20
    finding). Purely additive, no new DW code: `nobodys-cave-island` shipped a
    green combat floor gate that examined zero enemies for nineteen rounds
    because `floor_gate.covered`, `.not_covered` **and** `actors[]` were all
    empty at once and nothing said so — an unbound gate and a satisfied gate
    are indistinguishable to a reader who is not counting. `floor_gate.examined`
    is `covered.len() + not_covered.len()`; `unbound` is `examined == 0`, with
    `reason` present exactly then, in prose naming what zero means here. This is
    reporting, not diagnosis — an empty ledger is often the honest answer (an
    all-`ordinary` delve binds nothing, on purpose), so nothing here fails a
    build. `actors_gate` states the SAME shape for `actors[]` itself, and it is
    a genuinely different question: `actors[]` holds every actor declaring ANY
    tier (`ordinary` included), while the floor ledger only ever holds
    `elite`/`boss` — so a campaign with one `ordinary`-tiered actor and nothing
    billed hard is `actors_gate.unbound: false` and `floor_gate.unbound: true`
    at once. `actors_gate`'s zero-reason points a reader at
    `floor_gate.not_covered`, because an untiered *hostile* actor
    never appears in `actors[]` at all — it is invisible there by design, so
    `actors_gate.unbound` does not by itself mean "no hostile actor exists".
  **Validation metadata, not shipped gameplay** — excluded from the delve image
  like `critical-path-waypoints.json`, and emitted when the campaign has a
  mandatory encounter, a tier-declaring actor **or** an untiered hostile actor (a
  campaign whose only billed elite is an actor would otherwise emit no plan at
  all — the exact silence this closes — and a campaign whose only fight is an
  unbilled actor would report `floor_gate.present: false`, "this build cannot
  tell you", over a live hostile), so a combat-free delve's output is unchanged.
  Declaring a wave or actor `tier` therefore cannot move a shipped byte.
  **One caveat worth stating plainly** (measured, spec-0023): `manifest.json` is
  the reproducibility index over the WHOLE output tree, `validation/` included, so
  a campaign that gains a validation artifact gains exactly one line in the
  manifest's `outputs` map. Every datapack, world, resource-pack and
  creator-overlay byte is unchanged (verified against the pre-spec-0023 compiler
  for hello-world, hollow-vigil and the-drowned-bell). This is the same, already
  precedented consequence `critical-path-waypoints.json` had when it landed; the
  manifest indexes what the build produced, and pretending otherwise would make it
  a worse index for a cosmetic win.
- `<out>/render-plan.json` **player-POV shots** (`crate::render_plan::pov_shots`):
  the visual tier the owner's concern demands — the *player's own eye*, not the
  overhead/orbit cameras of the other shot kinds. One first-person `pov` shot per
  corner-thinned critical-path waypoint (the same `thin()` list the harness
  replays), camera at eye height (`1.62` above the standing cell), oriented along
  the walk toward the next waypoint and — at each leg's final waypoint — toward the
  objective anchor it arrives at (approach-heading fallback when the anchor is
  underfoot, so an arrival never degenerates to a straight-down floor shot). Each
  shot carries `leg`, the served `objective`, `standing_cell`, a `camera` with the
  first-person `fov` (~70°), and an `expect` whose first entry is a one-sentence
  machine description composed from campaign data (area name + objective/anchor/NPC
  names + objective hint) — the (image ↔ expect) pair a vision model reviews.
  Deterministic (route order → waypoint order; no RNG/clock) and appended after the
  overhead kinds so the existing shot prefix is unchanged. Emitted only when a
  walked critical leg exists.
- **`lighting` stamp** (POV + interior shots, `crate::render_plan::area_lighting_stamp`):
  pure metadata derived from the shot's area's **stage-1 declarations** — never from
  measurement (the measured model gates via `DW0210`/`DW0211`). `lighting` declared
  → `{"profile": "lit"}`; only `mitigation: "night-vision"` declared →
  `{"profile": "dark", "mitigation": "night-vision"}`; both → lit profile plus the
  mitigation; neither → **no key** (absent, not null), so campaigns without lighting
  declarations build byte-identically. Purpose: a declared-dark scene is pure black
  to an honest path tracer (the first island Chunky run proved exposure boosts
  cannot reveal a sealed cave — no light, only amplified noise — while real
  emitters render), so the stamp tells `delve-render` exactly which shots need its
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
  `"camera_eye_proof": {"cameras": N, "pulled_in": M}`.
- **`horizon` fact** (`crate::render_plan::horizon_fact`): the world-generator
  ambient the render layer cannot see. A `horizon: ocean` campaign (spec-0013)
  ships a world save holding only the chunks its layout occupies — the sea around
  the island belongs to the level generator — so a renderer loading that save
  draws void past the shoreline unless it raises its own water plane, at exactly
  the compiler's sea-level datum (anywhere else leaves a two-tone seam against the
  authored block water). The plan therefore *states* it,
  `{"kind": "ocean", "sea_level": 62}`, rather than leaving `delve-render` to
  infer a generator setting from blocks. `horizon: void` (default/absent) emits
  **no key** (absent, not null), so every campaign that declares nothing keeps a
  byte-identical `render-plan.json`.

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

Byte impact on a single-template prefab is nil: one template, at the piece's own
position, is the line that was emitted before.

**A tiled zone's extent is the whole zone, and more than one proof depends on
it.** `PiecePlacement::size` stays the zone's — tiling is absorbed below that
line — so `bbox()` is the whole building, and everything that asks *where does
the content end* gets the same answer for a tiled zone as for a single-template
one: the `forceload` span, the piece AABB `DW0780` compares, massing's
footprint, and `nav::built_volume`, which is what `DW0318` asks whether a fluid
cell is inside. That last one is the load-bearing agreement between two features
that arrived separately, and it is invisible from either: had the extent become
one tile, a tiled zone's own indoor water would read as having run out of the
world. `crates/compiler/tests/integration_pairs.rs` holds it.

`DW0803` is the invariant that keeps the two halves honest — the metadata's
declared size against the bytes' own `size` tag.

### Assembled-world model (shared, gravity-settled)

`crate::assembled` builds the one authoritative cell→block map of the world the
shipped delve actually assembles — placed prefab structures (`/place template`),
socket seals, gate clears — **then settles gravity-affected blocks**.

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
is already correct once `facing` moves. Before this the model rotated positions
but kept blockstates verbatim, so a rotated piece's `facing` disagreed with the
world the server actually builds. Nothing consumed those properties (every
occupancy classifier reads only rotation-invariant ones — `type`, `layers`,
`open`, `waterlogged`, `bottom`), which is why it went unnoticed and why the
correction leaves nav, seating, relight, snapshots and every emitted command
byte-identical. The `DW0430` stair proof is the first consumer of `facing`, and
without this it would report a false defect on every rotated piece.

The map stores **full blockstates** (`minecraft:oak_slab[type=top]`), not bare
ids: waterlogging, slab halves and snow-layer counts are block *state*,
and the fluid and step models below are wrong without them. Consumers that need
the bare id call `crate::assembled::base_id`; state-sensitive rules read their
property with `state_value`.

The
delve ships into a `the_void` flat world (no natural floor), so a vanilla
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
single-model discipline as settle. This closes the water analogue of the gravity
divergence: a `cave-shore` pool floods `[261,66,1]`, a cell an unpatched model
routed a talk-to leg's step-up through.

**Both fluids, one answer.** A body stands on lava no more than on water, so the
classifier asks `is_fluid` and both land in `flooded`. Lava's own flow differs —
overworld lava decays over 3 cells and forms no new sources, and a delve ships into
an ordinary overworld (a superflat with the `minecraft:the_void` biome, never an
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
`occupancy_of` below, the grammar back end's `Voxels` impl, and `delve-admit`'s
light probe. `Collision::passes_body` / `supports_body` / `floor_top_16` are the
three answers a walk gets.

**Collision classes (`crate::assembled::Occupancy`).** The occupancy is
no longer a single every-non-air-block-is-a-1×1×1-cube solid set; cells are
classified:

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
| no-collision fixtures (`assembled::is_no_collision_fixture`) | 0/16 | torches (`torch`, `*_torch` — wall, soul and redstone alike), signs and banners (`*_sign`, `*_banner`), `lever`, `*_button`, rails (`rail`, `*_rail`), `redstone_wire`, `light`, `structure_void` — vanilla declares every one of them `noCollission()`. A wall torch occupies the air cell beside the wall it is fixed to, and a model that calls that cell a full cube severs a corridor. **Deliberately excluded, and not because they collide**: `fire`, `soul_fire`, `cobweb` and the portals also have empty collision boxes and stay full cubes here, because a body that *passes through* one is not a body that may be *routed* through one. Also excluded because their shapes are not read out of the pin: lanterns, chains, end rods, ladders. |
| no-collision vegetation (`assembled::is_no_collision_plant`) | 0/16 | grasses/ferns, every small and tall flower, `pink_petals`/`wildflowers`/`leaf_litter`, saplings, crops, mushrooms and nether flora, kelp/seagrass, vines/`glow_lichen` — vanilla gives them an **empty** collision shape. Modelling them as full cubes makes a plant cell a phantom standable surface, which refuses valid geometry (a tuft on a terrace splits a 2-block riser into two climbable 1-block steps) and, worse, accepts invalid: a walkability proof that stands a body ON a tuft is unsound, and a flower cell measures light 0 as if it were opaque. The list is the **class**, never the ids one generator happens to scatter; lookalikes that DO collide (`azalea`, `big_dripleaf`, `bamboo`, `cactus`, `pointed_dripstone`, `sea_pickle`, leaves, …) deliberately stay conservative full cubes. Fidelity consequence: plant cells no longer dam the water-flood model either — vanilla water flows into and breaks them. |
| everything else | 16/16 | the conservative default |

A block under 8/16 is **thin decoration**: its cell is passable and is never a
floor level of its own (a walker stands on whatever is below it, and a 2-high
corridor with a carpet in it stays walkable). At 8/16–15/16 the cell blocks
passage but its walkable face is recorded in `Occupancy::partial`, which is what
makes the nav step rule physical rather than cell-counting — see below.

Modelled **precisely**: fences, walls, fence gates (open vs closed), trap
triggers, thin decoration, free fluid and waterlogging, and partial floor heights.
Modelled **conservatively** — treated as a full solid cube, never as
walkable-through: stairs, doors, trapdoors, and every other partial-collision
block. The tall/gate classes close the owner-hit soundness hole:
the full-solid model proved the island pen leg by standing the player ON TOP of a
1.5-tall `oak_fence` (a "legal" +1 step no vanilla player or bot can perform),
and a gateless fence ring would have PASSED the completability proof while being humanly
impassable (now `DW0311`). A tall/gate cell is never valid floor, which also
models the barrier's upper half blocking same-level walk-overs for free. Closed
fence gates are **use-gate** edges: walkable for the player (adventure-legal
right-click, the same action a human performs), exported first-class per leg (see
`use_gates` above). They remain routable edges for scripted `move-npc`/`move-actor`
tp polylines too — but **that is no longer taken on trust** (island round 21):
routing a puppet over a player-only edge is now a build error, `DW0452`, because
a tp'd puppet performs no interaction and no runtime verb ever opens a gate, so
"the firing beat's fiction controls the gate" was an assumption nothing proved.
The island's pen gate shipped `open=false` with sixteen legs through it. The edge
stays available so the diagnostic can name the cell and the reason rather than
degenerating into an unroutable `DW0307`.

**A stage-7 edit can author an open gate** (same round). `Assembled::open_gates`
— the side set `occupancy_of` reads to tell a closed gate from an open one — was
populated only by the prefab palette read, and `edit::write_cell` cleared it on
every write. So an edit could write `minecraft:oak_fence_gate[open=true]`, ship
exactly that block in the world, and still have every proof downstream model the
cell as shut: the model contradicting the bytes it emitted, and the one available
fix for `DW0452` was unauthorable. `write_cell` now re-derives the marking from
the blockstate it just wrote. Autonomous placement (`spawn-wave`
seating) uses the no-gate-use view (`World::without_gate_use`): a spawned mob is
never seated in a gate threshold and the seating flood never spills through a
closed gate. Cutscene dolly clipping (`DW0308`) treats fence, wall, and gate
cells as solids — they contain visible geometry. Water flow is unaffected:
vanilla water flows only into air, so every non-air block (fences and gates
included) dams the flood exactly as before.

### Entity placement: cells are centred, blocks are not

Every entity the compiler **summons or teleports** is positioned at
`nav::cell_center(cell)` = `(x + 0.5, y, z + 0.5)` — the horizontal centre of its
proven-walkable cell. Block-targeting commands (`setblock`, `fill`, `place`,
`spawnpoint`) keep the bare integer cell, which is the coordinate space they take.

The distinction is load-bearing. A block cell `(x, y, z)` spans `[x, x+1)`, but an
*entity's* position is the centre of its AABB, so summoning at the bare integer
coordinate parks the body on the corner where four columns meet: a 0.6-wide villager
at `x = 7.0` occupies `[6.7, 7.3]`, i.e. **70 % of it inside column 6**. Against a
wall that is an NPC standing in the wall; along a walked path it was the owner's
"the NPC visibly passes through blocks" island finding — measured at **234 of 385**
waypoints on the beach→cave `move-npc` leg with the body AABB inside a solid, now 0.
Nav itself was never wrong (A* is strictly cardinal — `neighbors_fp` offers four
horizontal moves and no diagonal transition exists, so corner-cutting is structurally
impossible); the defect was entirely in cell→position conversion at emission.

Applies to: NPC bodies + interaction hitboxes, `spawn-npc` entrances, actor puppets,
wave mobs, `interact` hitboxes and `interact`/`reach` wayfinding markers,
environment-trigger and trap-disarm interactions, and every `move-npc`/`move-actor`
waypoint. Cutscene dolly cameras already used centred coordinates
(`nav::camera_points`). **Player** teleports keep integer coordinates: vanilla
resolves player-vs-block overlap by pushing out, and the `dw:cp` mirror is a
documented int-triple contract (spec-0013).

**Vertical steps interpolate as an L, not a diagonal.** A one-block step up rises
over the source column first, then crosses at the new height; a step down crosses at
the source height, then drops. A straight lerp between the two cell centres would
drag the body through the corner of the step block — the stair-shaped instance of the
same artifact. Both legs of the L stay inside cells `standable_fp` + the jump
head-clearance rule already proved clear.

### A walked body faces where it is walking

Every per-tick `tp` of a `move-npc` / `move-actor` driver carries an explicit
`<yaw> <pitch>`; `pitch` is always `0` (a walk is level by construction — the L of a
vertical step is still a walk, not a dive). The yaw is `nav::yaws_along`: for each
waypoint, the **exact MC bearing of the segment it is about to walk**
(`yaw = atan2(-dx, dz)`, 0 = +z south), rounded to whole degrees; the arrival
waypoint keeps the last leg's bearing rather than snapping south.

There is no smoothing. A corner turns on the tick it is taken, because a per-tick
`tp` polyline is what vanilla gives us and any easing between two bearings would be
invented motion the nav proof never made. (Vanilla has no server-side "turn over N
ticks" primitive for a teleported body — CLAUDE.md *No hacks at any layer* says that
excludes the feature, not that it licenses a polling hack.)

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
re-entry latch `#mrun_<bare>` is keyed per **(npc, to_anchor, gate)**: it stops a walk
from restarting *itself* and knows nothing about the body's other walks. So firing a
second `move-npc` at an NPC whose earlier walk was still running left **two** drivers
alive, both teleporting the same entity every tick; the interleave garbled the path
and whichever walk had more remaining ticks wrote the final position — the body
parked at the **first** walk's endpoint, not the last-fired one. Root-caused live on
the island (2026-08-06): a 408-tick beach→mouth walk overlapped by a 21-tick walk to
checkpoint-1 left eurylochus at the mouth, 3.0 blocks off his cast-ledger cell —
exactly on the harness's affordance radius.

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
`unless own = gen`: with both scores unset — a driver invoked directly, as the
`v04_move` PackTest does — a score comparison is *false*, and the `unless` spelling
would read that as "stale" and cancel a walk nothing superseded.

The new walk still starts at **its own first waypoint** (waypoints are precomputed
from the walk's declared start anchor, so "resume from wherever the body stands" is
not expressible), i.e. an instant snap onto the new route — the same snap single-walk
content already gets when a walk fires while its NPC stands elsewhere.

A body with only **one** planned walk can never be superseded, so its start and driver
carry none of this and pre-existing single-walk campaigns stay byte-identical
(ADR-0006).

**`move-actor` puppets carry the identical contract.** The `ma_tick_*`
drivers had the same defect for the same reason — `#arun_<bare>` is keyed per
(actor, to_anchor, gate) — so two overlapping legs on one puppet left two live drivers
fighting over the same body, and the longer leg parked it at the wrong endpoint
permanently. The scores are the same two under actor names: `#agen_<actor>` (the
puppet's leg generation, bumped by every start) and `#aown_<bare>` (the generation
this driver was started for), with the same generation-aware re-entry refusal in
`ma_<actor>_<to>` and the same two-line staleness prologue in `ma_tick_<actor>_<to>`.
The positive `if own < gen` spelling matters for the same reason here: `v06_move_actor`
and `v06_arrive_handoff` invoke `ma_tick_` directly with both scores unset. A puppet
with one planned leg carries none of it and stays byte-identical. No campaign authors
*temporally overlapping* legs today, so the defect was **latent** and the fixture is
synthetic — though five island puppets are `supersedable` on leg count and do carry the
machinery.

Proved by `crates/compiler/tests/move_supersede.rs`, which **executes** the emitted
commands: a small interpreter for the driver command subset runs the real start
functions through the real 1-tick scheduler loop and reads the body's final position
off the `tp` commands. Both verbs are covered there, and the single-leg puppet's two
functions are additionally pinned **verbatim, byte for byte** (`GOLDEN_ONE_LEG`,
captured from the pre-fix build).

### Press answers — what a pressable thing says back (v0.8; generalized v0.11)

There is no such thing as a seal with nothing to say: a sealed gate that answers a
right-click with **silence** is a defect, and the answer is the engine's
obligation rather than the campaign's. A co-located `use` trigger on the gate
anchor cannot supply it, because it summons a *second* `minecraft:interaction` at
the same cell as the existing hint's, and an exact ray-pick tie resolves by
iteration order: one of the two hints silently dies and the compiler builds green
either way (`DESIGN.md` §7 item 4).

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
seal answers with silence — which is the finding, reproduced. Coordinates are
built from integer hundredths, never from `f64` arithmetic, so the shipped text is
exactly what it reads as (ADR-0006).

**The answer is not a mechanism — it is an ordinary trigger** (v0.11). Until this
version the reply lived entirely inside one effect verb: `close-gate.sealed_hint`
had its own advancement (`seal_<anchor>`), its own reward function
(`seal_hint_<anchor>`), its own actionbar command and its own baked English. Every
one of those is a property of *being a thing a player can press* and none of them
has anything to do with closing a gate, so the second object that needed them — a
sealed `shortcut` door, which is the gate a souls loop-back most invites the party
to push on — had no surface at all and answered silence. Both those functions are
gone. A press answer is now:

```
EnvTrigger { at: <the body's anchor>, on: use, audience: presser, once: false,
             effects: [ narrate { style: actionbar, text: … } ] }
```

authored by the campaign, or synthesized by the compiler
(`plan::collect_press_answers`) for a sealed body the campaign leaves silent. It
is emitted by the same `env_trigger_setup` / `env_trigger_fns` /
`emit_advancements` that emit every author-written click, so there is one proof,
one l10n path and one diagnostic family for all of it. The two things v0.11 added
are exactly the two the general verb was missing: the **channel** (`actionbar`)
and the **addressee** (`presser`).

**The dispatch, which is what `presser` means.** `press_<trigger>` is rewarded by
a `player_interacted_with_entity` advancement keyed on the trigger's own
`dw_trig_<id>` tag; it revokes the grant (a wall is not consumed by being asked)
and calls `trig_<trigger>`, whose bundle is emitted under `Audience::Solo`. That
criterion is the one vanilla primitive that runs a function **as the player who
right-clicked** — the same one every NPC dialogue, `interact` objective, bonfire
rest and shop button already runs on — and it is chosen over polling the entity's
`interaction` NBT for two reasons: the record names no player a command could
target, and *reading* it would consume the press a co-located `use` trigger is
entitled to see (round-8: adjudicate conditionally, consume unconditionally). An
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
`strike` / `strike-npc` / `approach` is `DW0427` rather than an approximation
(CLAUDE.md: a capability with no vanilla primitive under it is excluded, never
faked downstream). The compiler reserves the `dw-` trigger-id prefix for the
triggers it synthesizes (`trigger/dw-press-seal-<anchor>`,
`trigger/dw-press-door-<shortcut>`); an authored id in that namespace is `DW0428`.

**Which bodies get an answer, and who words it.** One site
(`plan::press_answer_sites`) lists every pressable body — `close-gate` seals, then
sealed `shortcut` doors — and each carries a **silence policy**: what happens when
the campaign says nothing. **The policy is keyed to the `dsl_version`, not to the
verb.** Above the fence there is one rule for the whole class; the two arms below
it differ from each other only because the two classes historically differed, and
preserving that is exactly what a fence is for.

| Body | at `dsl_version` ≥ 0.11.0 | below 0.11.0 |
|---|---|---|
| `close-gate` seal | `Authored` — **`DW0429`**, the campaign does not compile | `Defaulted` — the `delvewright.ui.gate.sealed` chrome, as since v0.8 |
| `shortcut` door | `Authored` — **`DW0429`**, the campaign does not compile | `Silent` — nothing emitted, as before the version existed |

The first column is deliberately uniform and the second deliberately is not: one
rule above the fence, and below it each class keeps precisely what it already had.

Two objects of one class must not get two defaulting policies: that is precisely
the "capability keyed to the verb" defect CLAUDE.md's worked example describes,
and this surface **is** that worked example. The shared site is therefore
load-bearing rather than tidiness.

**Why a sealed body errors instead of defaulting.** The wording may well be
"The way is sealed.", and it
must be creator-customisable — but a baked default is the compiler making a
*design statement*, about tone and about what this specific thing is, on the
author's behalf, and then never telling them it did. An error makes the author say
it. That is the no-hacks rule at a new site: if content needs a thing, the DSL
exposes it and the author declares it, rather than a lower layer inventing it. It
is also the only end state where the docs, the code and the player agree — this
reference and `wrongside.rs` both claimed for two versions that a door's wording
"defaults", and no code defaulted anything, so the door said nothing. The repair
was never to make the claimed default real.

**Grandfathering is the fence's job, not a review's.** The same declared
`dsl_version` yields the same verdicts and the same behaviour, so an
already-approved campaign keeps what it was
approved with and is never re-reviewed for a rule written after it. That is not a
promise of byte-identity forever — a released delve reproduces through its pinned
engine and OCI image (`versions.toml`), not through eternal emission stability.

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
rule `strike`-on-an-NPC's-anchor has followed since round 6, and the merge
`DESIGN.md` asked for. The consequence is also its meaning: such a trigger is live
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
own it. Seven roots do — quest `on_objective_complete`, quest `on_complete`,
`triggers[].effects`, `traps[].payload` (spec-0022: a payload is an effect root),
a **dialogue option's `set-checkpoint` `on_respawn` bundle**, `shortcuts[].on_unlock`
and the campaign's `on_death` (both spec-0031). The dialogue one is the trap:
`DialogueEffect` carries no gate verb, so the older gate scans stopped at
the quests stage — but `on_respawn` is a plain `Vec<QuestEffect>` and a
`close-gate` inside it really is lowered, into `cp_on_respawn_<i>`. A seal the
compiler fills but never arms is the finding again, one effect root further out.
`shortcuts[].on_unlock` is the same class read the other way round: it *is* in
`quests.content`, it *was* lowered (`shortcut_open_<id>`), and it was still missed
for two versions because the walks were written against a remembered list rather
than against what emission reaches.
The seal planner, `DW0423` and the `close-gate` completability
model (`plan::collect_gate_events`, feeding `DW0311`/`DW0315`/`DW0342`/`DW0410`)
all walk it, so the checks, the proofs and the emission can never disagree about
which firings exist.

The roots themselves are enumerated exactly once, in
`plan::for_each_effect_root` (which yields each top-level effect *list*, with an
`EffectRoot` naming which of the seven it is and carrying its owner where it has
one). `for_each_gate_effect` is that enumeration flattened; `timeline::walk_campaign`
(→ `DW0410`, `nav::all_effects`), `emit::all_campaign_effects` (→ the
generated functions), **both halves of `compiler::flow`**
(the producer scan and `flow::gate_flags`, → `DW0201`/`DW0202`/`DW0203`/`DW0204`/
`DW0205` and the exported critical path) and
`emit::check_effect_anchors` (→ `DW0360`, the resolved-anchor seal over exactly
what those generated functions emit) and `emit::declared_flags` (→ the
`dw.f_<flag>` scoreboard objectives `setup` creates for the writes those
functions perform) are the other consumers. A root can no
longer be added to one walk and forgotten in another, which is the only reason
this class of finding kept coming back. Since spec-0031 that claim is also a
**test matrix**: `tests/effect_root_walkers.rs` iterates `EffectRootKind::ALL`,
builds one campaign per root from an exhaustive `match` (so an eighth root is a
compile error there), and asks six walkers plus `DW0360` about every one of them.
A per-walker test proves one walker against the roots its author remembered; the
matrix asks every walker about every root, and names both when it fails.

**When a firing happens** comes off the site's `EffectRoot`. A quest
`on_objective_complete`/`on_complete` fires at its objective's / the quest's
completion step — the player is *forced* through both, so both gate directions are
modelled. An environment trigger, a trap payload, a dialogue-hosted `on_respawn`
bundle, a shortcut's `on_unlock` and the campaign's `on_death` have no step of
their own (proximity, a sprung trap, a death, a bar thrown), so all five
root conservatively at step 0, which precedes every leg. The four **optional**
roots — trap payload, `on_respawn`, `on_unlock` and `on_death` — register their
`close-gate`s **only**: an
unguaranteed firing may be assumed to have happened exactly when assuming so is
conservative, so it can seal a region but never unseal one. That is the rule a
shortcut gate already obeys (sealed for the whole model, because the delve must be
finishable the long way). A later `open-gate` from a forced root still wins the
region, so the widening reads as a seal the proof must survive, never as a veto.

**Wording (`close-gate` only).** `sealed_hint` is optional below 0.11.0 and
**required at 0.11.0 and above** unless a `use` trigger answers the gate instead
(`DW0429`). Either way it is now **sugar, not machinery**: it is the wording of
the press answer the compiler synthesizes for that gate, and nothing else. Unauthored, the line is the `delvewright.ui.gate.sealed` chrome
string — compiler-owned, translated with the compiler, absent from the campaign's
inventory (spec-0029). Authored, the line is inventoried at
`<effect-key>.sealed_hint` and translates like any other player-visible string.
Both keys are exactly what they were before v0.11, deliberately: the synthesis
happens in the **plan**, below `localize`, so no sidecar key moves and the chrome
default never enters a campaign's inventory.

Generated PackTest `v08_seal_answers`: nothing armed at boot, exactly one hitbox
per shell cell after the seal, unchanged after a re-fire, none after the re-open —
staged and un-staged by the fixture itself (batch model). The press→actionbar
half needs a real client's right-click, which no PackTest can fire; that primitive
is exercised by the harness bot wherever it rests at a bonfire or talks to an NPC.

### Nav (compile-time, over the assembled voxel grid)

**A navigation world is geometry plus premises, and the premises are one value.**
`nav::World` carries the collision classes measured off the assembled bytes and
six things the *campaign* states about the world those bytes sit in: the
world-generator `Ambient` (spec-0013 `horizon`), the built volume (where the
content ends), the declared **lethal volumes** (impassable, `DW0510`), the
measured **world-load gate seals**, the **clocked gate regions** a `timed-gate`'s
own clock owns, and the **transit teleport** source volumes. Those six travel
together as `nav::Premises`, and `nav::Premises::of_plan` is the only way to
derive one from a campaign — a call site cannot state a subset, so it cannot
carry half a world. Every arm that builds a world from a campaign passes one to
`World::from_occupancy`, and that is the same set on **both** of `emit::build`'s
arms: the pristine assembly (`World::from_plan`) and the stage-7 edit replay,
which builds its world from the edited bytes. `edit::check_batch_invariants`,
which re-runs the completability proofs after every batch, uses the same set.

A world with no campaign behind it says `nav::Premises::geometry_only` by name,
with its reason in a comment at the call site, and every production decline is
enumerated by `nav::tests::premise_declines_are_enumerated`. There are five: the
stage-5 blockout battery's two massing worlds (they carry their own sealing
authority, from the quest graph's monotone closure); the two relight darkness
surveys (`light::relight_over` and a `relight` verb's own — applying the premises
would cut the reachable flood at a kill box and *shrink* the `DW0210` survey
rather than sharpen it, and a pit that kills is a pit the player must be able to
see before stepping into it); and `delvec snapshot`, where a camera is stood up
against blocks and a reviewer framing a shot down into a lethal volume is looking
at open air. `delvec blocking-chart` goes the other way: it re-derives
`critical_path_routes`, so it carries the full set and draws the corridor the
proof actually walks.

`move-npc` paths and the critical path are routed by A* over the placed-world
block data (obstacles per the collision classes above — full-cube solids, 1.5-tall
fence/wall barriers, closed fence gates for walkers that cannot use them;
**fluid-flooded cells — water and lava alike (`assembled::is_fluid`) — are
impassable and are never valid floor**; compiler gate
regions are passable). Steps are cardinal, one cell up or down.

**Step cost is terrain-shaped, not distance-only (round 8).** A step costs
`16 + 2 × |Δfeet|` in sixteenths of level walking: `STEP_COST_16 = 16` for the
block travelled, plus `ELEV_WEIGHT = 2` per sixteenth of height change, **up or
down alike**. The A* heuristic is horizontal Manhattan distance × `STEP_COST_16`,
which no step can undercut, so it stays admissible and consistent — A* still
returns a true minimum-cost path and never reopens a closed node.

*Why.* Under a distance-only cost every route of equal length is equally good, so
the planner walked the island's herd and giant along the straight line over
bumpy 1-step terrain — bobbing a block a dozen times — while the flat cleared road
two columns over cost the same two-step detour it always did and never won.
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
cell)`). Measured on the island: total staged-walk length 1096 → 976 cells and
cumulative elevation change 228 → 108 blocks, of which the part beyond the legs'
own net climb fell 128 → 8; the beach→pen walk moved off `x=7` onto the built path
spine at `x=9..11` and runs flat at `y=63` across the whole greenfield.

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
does `delvewright_schem::nav`, which is the walk a grammar expansion, an ingested
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

This corrects the rule in both directions. It **rejects** what the old full-cube
model proved: stepping off a bottom slab (feet at `y+0.5`) onto a ledge whose face
is at `y+2` is a **1.5-block** rise, which the old rule read as an ordinary "+1
cell" step and certified as a walkable route no player or mineflayer bot can
perform. It **admits** what the old model refused: stepping from a full floor onto
a bottom slab is a 0.5-block auto-step, legal even directly under a ceiling that
would block a jump. Vertical candidates stay `{0, −1, +1}` cells — a `+2`-cell hop
between two very thin floors can be physically legal, but omitting it only ever
refuses a route, never proves one.

Cutscene dollies must pass only non-solid cells; the clip test is an **exact 3-D
grid walk** (Amanatides–Woo DDA) visiting every cell each segment intersects, with
no error term — it replaced a ≤0.25-block sampler that could miss a cell a shot
only grazed through a corner and certify the shot clear.
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
already used. Before that measurement, a gate's state in the static model was a
function of what *sealed* it and never of what *opened* it: "passable unless a
`close-gate` seals it" can only fail to notice an obstruction, never invent one,
and the mistake an author makes is forgetting to open a door. A campaign missing
its one `open-gate` compiles clean under that default and the runtime bot then
says *"No path to the goal!"* — a symptom that names nothing.

The base occupancy model still clears every gate cell, and that is now a statement
about the **base** world only. It has to pick one state, and "open" is the one that
keeps `RegionWrite::Unseal` expressible — an `open-gate` is `replace`-filtered to
the gate's own block, so a base world holding the bars would need a block-aware
clear and would then wrongly delete a `collapse`'s debris (`DW0445`). The
world-load `Fill` supplies the other half, and an `Unseal` cancels it by ordinary
latest-write-wins.

Three things this deliberately does **not** decide, each of them a stated gap
rather than a silent one:

* **A `timed-gate`'s region** (spec-0016 §4) is measured and never modelled shut:
  its clock fills and clears it twice a cycle from world-load, so a permanent seal
  would refuse a campaign that plays. `DW0378`/`DW0388` own that region.
* **A leg whose start is inside a declared `teleport` source volume**
  (`Plan::transit_teleports`) is judged with the seals lifted — the party may be
  carried off that cell rather than walk away from it, and nothing in the critical
  path records an intra-area ride the way `transport_before` records an inter-area
  one. That restores exactly the pre-measurement verdict for such a leg, so
  `DW0311`'s binding is unchanged; the cost is that a genuinely-blocked leg with a
  teleport anywhere over its start is not judged.
* **A gate opened only by an optional firing** — a trap payload, `on_death`, a shop
  offer, a dialogue `on_respawn`, a shortcut's far-side unlock — is treated as
  never opened, by the pre-existing rule that an optional firing may seal a region
  and may never open one (`plan::collect_region_events`), which is also what keeps
  every shortcut gate sealed so the delve is finishable the long way. A delve whose
  only door-opener is a sprung trap is therefore refused; the first-class way to
  spell "the party walks/presses here and the door opens" is an environment
  `trigger`, which the model does credit.

The `foreign_blocks` count in the ledger below exposes a fourth, older gap the
measurement made visible: an `open-gate`'s fill is `replace`-filtered to the
anchor's **declared** block, so any other block authored inside the gate region
survives the opening, while this model credits the `Unseal` with the whole region.
`cave-mouth.nbt` really does author five `mossy_cobblestone` cells inside a gate
declaring `cobblestone` — latent, since no campaign in either repo places it today.

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
meeting the ambient sea: the two numbers reconcile against `delve-admit`'s
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
`nobodys-cave-island` — the campaign the finding came from — 1 beat, 1 judge, 3
per-player tests, vocabulary exactly the six box arguments.

**Binding ledger — `validation/sea-seepage.json`.** The other half of the same
question, and the one that had no answer at all before: `DW0318` measures water
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
`foreign_blocks`), and `unbound` when the model treats none of them as shut. A
campaign whose layout resolves no gate anchor emits no file at all, so a file that
exists and reports zero is a finding rather than an absence — `nobodys-cave-island`
is exactly that case, its one gate anchor being the boulder the campaign
`close-gate`s later.

**Runtime region solidity (v0.6 as `close-gate`, generalised in v0.10 by
spec-0031; DAG-causal).** The base occupancy model treats every
gate region as **passable** (the conservative "assume the gate the player needs is
opened" stance `DW0306` separately proves at the piece-connectivity level, and the
world-load seal above supplies the state that stance was standing in for) — so
`open-gate` does not dynamically flip cells at nav time, and an `open-gate`-only
campaign routes exactly as before. `close-gate` is the physical dual: the compiler
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
`hide` branch's boulder). The seal is applied only to a **causal leg** (its start
objective is itself a DAG ancestor of the arrival) — the lineariser concatenates
sibling branches into artifact "legs" the player never walks under the arrival's
gate state, and base `DW0311` already proved every leg walkable in the open world. A
genuinely-forced re-crossing (a causal leg whose sealed gate is never reopened
before it) still fails `DW0311` (`DW0315` from a checkpoint) with a message naming
the sealed gate — the "point of no return by geometry" the owner's staging vision
wants, provable at compile time.

**Close-gate solidity for *staged walks* (v0.6, timeline-local — `DW0410`, round
8).** The DAG-causal model above answers "which gates are shut while the **player**
walks a critical leg". It says nothing about two effects inside one bundle,
because across bundles there is no order to know. Inside a **single effect
timeline** there is, and `compiler::timeline` proves it.

The island defect: one `sequence` sealed the boulder at `at_ticks: 460` and walked
the giant across that region at `at_ticks: 700`. The walk was planned on the open
world (gates are modelled passable), so the build was green and the actor stepped
through solid basalt on the live server. The gate state at tick 700 was never in
doubt — nothing was looking.

`timeline::walk` replays each timeline and pairs every effect with the gate
regions an **earlier effect in that same timeline** provably sealed. A timeline is
one **effect root** — every list `plan::for_each_effect_root` enumerates, i.e. all
five emission can lower: `on_objective_complete[obj]`, `on_complete`, a trigger's
`effects`, a `traps[].payload` and a dialogue option's `set-checkpoint`
`on_respawn` bundle (declared order, one tick, so effect *j* finishes before
*i > j* starts); a `sequence`, ordered by `(at_ticks, declaration index)` — real
elapsed time, which is exactly what the island defect turned on; or an `on_arrive`
bundle, which inherits the state as of its move.

**Optional roots need no special case**. Two of the five have no
guaranteed firing — the party may never trip a trap, nobody is forced to die at a
checkpoint — and the DAG-causal model above has to rule on that (an unguaranteed
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
and `emit::all_campaign_effects` (the generated functions) also walk. Four
consumers, one root list: what the emitter lowers and what the proofs check are
the same set by construction, which is what stops a hand-rolled walk from
enumerating three of the five roots.

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

The PackTest counterpart is unchanged and deliberately so: the generated
`v06_arrive_handoff` still drives the arrival tick with every campaign gate
filled. What must be immune to sealed terrain is the **arrival machinery** (a tp
chain, not pathfinding); what may not be routed across a seal is the compiler's
*plan*.

**One leg model for every consumer.** The per-leg seal
(`nav::leg_seal`) and the routing that uses it (`nav::route_walked_legs`) are now
the single definition shared by the completability proof, the forced-cell set the
`DW0342` trap proof reasons about (`World::required_path_cells`), and the exported
harness waypoints (`nav::critical_path_routes`). Previously only the proof ran
under seals while the other two routed the fully-open world, so the compiler could
(a) hand the bot a route through a gate the campaign had already sealed and (b)
call a lethal trap "avoidable" when the player only walks its detour *because* a
`close-gate` shut the direct route. A trap's disarm-reachability search likewise
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

### The map editor edit stage (spec-0017, v0.6)

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
  trigger/hazard cell, dispenser socket or disarm-affordance cell. `setup_finish`
  runs `world_edits` **before** `trap_setup`, so a colliding edit lands first and
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
  the six premises `nav::Premises::of_plan` carries (see *Nav* above), so the
  pristine assembly, the edit replay and the stage-10 whole-world call all have
  it by construction rather than by three call sites remembering. It is read
  **only** by this proof: it never feeds the walkability sets, so routing,
  standability and every other proof stay byte-identical.
  - **Anchor seating (`nav::AnchorRoot`).** The walk region this proof examines
    is flooded from every resolved anchor, and an anchor is a declared *point*,
    not a floor cell — seating it is a nearest-standable-cell snap. That snap
    chooses by squared distance and **nothing else**: it does not care that solid
    geometry stands between the anchor and the cell it lands on. So each root
    carries the AABB of the piece that declares it, and the snap may not leave
    it. Without the confinement a ceiling anchor — which every spec-0022
    `collapse` payload must declare — is nearer the cell on top of the ROOF (Δy
    2) than its own floor (Δy 3), so the proof rooted itself on a bare platform
    the party can never reach and demanded a safe edge there, which no
    free-standing prefab in a void world can give it. Same leak, one layer down,
    that `World::confined_standable_cells` closed for wave seating.
    Only the **seating** is confined; the walk that follows is not, because a
    player who reaches a room reaches whatever it connects to — confining the
    flood would shrink the region examined, which is a weaker check, not a more
    correct one. An anchor whose piece offers no footing within `SNAP_RADIUS`
    seats nowhere and contributes no root, exactly as an unsnappable start
    always did.
  - **`Ambient::Void`** (`horizon: void`, the default and every pre-0.6 campaign)
    — unchanged: bottomless columns are the hazard, exactly as before.
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
  so one cycle erases it. It reads `plan.region_events`, so the v0.10 verbs
  inherited this warning without a line of their own. The proofs stay sound (the occupancy model
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
  persisting, so a script `apply` accepts is a script `build` accepts. Measured
  cost: ~0.3 s on the largest content campaign, against ~0.34 s for the snapshot
  render the same command already does — a cheaper tier has no reason to exist.
- **Atomic persist.** `world-edits.json` is written to a sibling `.tmp` and
  renamed into place: the artifact of record (ADR-0006) is never left truncated
  by a crash or a full disk.
- **Forceload lifecycle.** `setup` forceloads every piece bbox *and* every edit
  AABB. Each edit chunk that no piece bbox covers gets its own convergence
  sentinel in `place_verify` (`execute if loaded <cell>` folded into `#placeok`),
  so `setup_finish` — and therefore the one-shot `world_edits` — cannot run into
  a still-loading chunk and lose those writes forever; the tick retry loop
  converges on them exactly as it does on piece placement. Those same chunks are
  then released (`forceload remove`) at the very **end** of `setup_finish`, after
  every other write in the function. **Piece forceloads are never released** —
  the gameplay tick machinery (gate fills, wave spawns, checkpoint and trap block
  reads) keeps addressing those chunks for the whole session.

---

## 5. Diagnostics catalog (complete, as of current `main`)

Every DW code in `crates/**/*.rs`. Grouped by range. `tools/check-dw-codes.py`
verifies this catalog is bidirectionally exact against source (CI docs job).

**Test-coverage gated** (CLAUDE.md Conventions). The same
script also fails CI if any documented, landed code has no test asserting it —
either the literal code string or a symbolic diagnostic-code constant (e.g.
`DW_STRIP`) that resolves to it, scoped per crate to avoid cross-crate name
collisions (`DW_INPUT` names a different code in `delve-schem`, `delve-render`,
and `delve-admit`) — appearing in `crates/<crate>/tests/**/*.rs` or a
`#[cfg(test)]` module in `crates/<crate>/src/**/*.rs`. A code that is
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
message names it with an explicit "do NOT". The rows below summarize each code's
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
row here. `DW0813`, `DW0822` and `DW0781` are written that way.

**A secondary whose premise is an already-reported primary does not print N
times.** Either the findings are one diagnostic naming all of them (`DW0842` at a
zero box count, `DW0826` where more than one thing leaves the region,
`DW0150` where stage 5 is empty), or the line stands and gains a clause naming
what it is downstream of (`DW0818` where stage 5 declares no quests, `DW0843`
and `DW0844` where the site plan has already refused the frame or the seam set).
The rule and its two shapes are in `dsl::diagnostic`. No code loses its ability
to refuse alone: every fold arm is reachable only in the state that made the
copies identical.

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
| `SitePlan` (a `site-plan.json` is present) | exactly one id, `area/site` | the derivation: `anchor/node-<place>`, `anchor/seam-<edge>`, `anchor/unlock-<edge>`, `spawn` — **plus every `stations[]` name the layout graph's nodes declare** (v0.18), which is why this arm also offers *declaring* the name and not only correcting it: a station is the one anchor name in this engine an author writes by hand, so an unresolved one is as likely a missing declaration as a typo |
| `NoMap` (`areas[]` empty and no plan) | nothing — the campaign has no map | nothing — no anchor is placed |

The `Prefabs` arm is what each site printed before the authority existed and is
returned verbatim, so a prefab campaign's refusals are unchanged. The other two
arms **replace** it rather than appending to it, because every prefab
prescription (`declare it in stage-1 world.areas`, `bind a prefab/pool`, `anchor
names come from prefab metadata; do NOT invent one`) is refused by `DW0839` or
`DW0160` in a campaign carrying a site plan, or names prefab metadata a derived
map does not have. A `NoMap` refusal names **both** authorities, since which one
the author wants is a choice they have not made and a refusal must not make it
for them.

It binds at **twenty-eight message sites**: in `dsl::validate`, three area
refusals (`DW0112` — an npc's area, a planned quest's area, and a stage-7 edit
script's `batches[].area`) and seventeen anchor refusals (`DW0142` x11,
`DW0194`, `DW0340` x2, `DW0371`, `DW0377`, `DW0381`); in `compiler::emit`,
`DW0360`, `DW0426` and `DW0447`; in `compiler::gates`, `DW0343`; in
`compiler::edit`, the two `AnchorRelative` frame failures a stage-7 edit script
can raise; and in `compiler::light`, `DW0210` and `DW0211`. `crates/dsl/tests/v14_site_plan.rs`'s
`no_refusal_on_a_derived_map_prescribes_a_prefab_document` binds it over a
derived map's whole refusal set, keyed to the forbidden prescription rather than
to a list of codes.

### DW0874 — a campaign directory part-way through being written (`compiler::load`; error; exit 1)

| Code | Meaning |
|------|---------|
| `DW0874` | **A campaign directory is present and does not hold all six stage documents.** `compiler::load` (`missing_stage_documents_diagnostic`), validation tier (exit 1), raised through `Fenced::structural` because it exists before any document has parsed and so has no declared `dsl_version` to grandfather against. The state it names is the ordinary one: a campaign is written a document at a time, and the authoring skill tells an author to stub the stages they have not reached. The message names **every** missing document rather than the first — the loader reads in document order and stops at the first absence, so an author starting from `world.json` alone learned the remaining five filenames by running `validate` five more times — then the whole set of six, what a stub is (`dsl_version`, `campaign_id`, `stage`, and a `content` carrying only the fields its schema requires), and `delvec schema --stage <name>` for each one's exact shape. **The recipe is exact for five of the six and the sixth is named**: `quest-plan.json` requires `finale` as well as `quests`, and `finale` must name a member of `quests`, so the literal recipe is refused by `DW0131` — whose remedy cannot be performed without authoring a quest — and the message gives the smallest stub that satisfies the document's own rule instead, one planned quest with `finale` naming it. It also names `DW0150` beside `DW0100` as an ordinary consequence of stubbing, because a stubbed plan produces one and an author told the recipe was exact would otherwise meet it as a surprise. The optional documents are named as optional, so their absence is not mistaken for the next thing owed. **What it deliberately does not cover**: a path that is not a directory has six absent documents by arithmetic and a remedy that does nothing, so it stays `internal error` at exit 10, as does a document that is there and cannot be opened. Absence is probed by opening and treating only `NotFound` as absent — `is_file()` answers `false` for a directory standing in a document's place, which would call an unreadable document absent. Bound at every verb that reads a campaign directory, which is what `crates/compiler/tests/missing_stage_document.rs` enumerates. |

### DW01xx — validation (`dsl`; severity error; exit 1)

| Code | Meaning |
|------|---------|
| `DW0100` | Document does not conform to its stage schema (unknown field / wrong type / missing required field, incl. persona). Parse-time. |
| `DW0101` | `stage` field ≠ document slot. |
| `DW0102` | Unsupported `dsl_version` (not in `{0.2.0,0.3.0,0.4.0,0.5.0,0.6.0,0.7.0,0.8.0,0.9.0,0.10.0,0.11.0,0.12.0,0.13.0,0.14.0,0.15.0,0.16.0,0.17.0,0.18.0,0.19.0}`). |
| `DW0103` | `campaign_id` differs across stages. |
| `DW0110` | Malformed id syntax (not kebab-case / wrong-missing prefix). **The message names the form of the type it rejected**, derived from that id type's own `PREFIX` — `` `dlg/<kebab>` `` for a dialogue node, `` `class/<kebab>` `` for a class — rather than restating the general rule beside three fixed examples. One macro in `dsl::validate::syntax` is the single path every id type's syntax refusal goes through, so the answer comes from the type at every site: `ids::syntax_form`. The per-section refusals that spell their own prefix by hand (`wave/`, `trigger/`, `trap/`, `shortcut/`, `ambush/`, `timed-gate/`, `loot/`) are the same fact copied, which is why the general path did not have it. |
| `DW0111` | Duplicate id in namespace (incl. two dialogue trees for one NPC). |
| `DW0112` | Dangling / forward / undeclared reference (incl. persona relationship to unknown NPC). An **area** reference resolves against the campaign's placement authority and its prescription comes from there (see the remediation contract above): a `Prefabs` campaign is told to declare it in stage-1 `world.areas`, a `SitePlan` campaign is told its one area is `area/site` and explicitly told NOT to declare it, and a campaign with neither is told both branches. Three sets in `dsl::validate` resolve an area id — npc/quest-plan references, and a stage-7 edit script's `batches[].area` — and all three now prescribe from the same authority. |
| `DW0120` | Dialogue node unreachable from `root`. |
| `DW0121` | Dialogue `root`/`next` references unknown node. |
| `DW0122` | Dialogue effect targets an objective unknown / not `talk-to` / on a different NPC. |
| `DW0123` | A `talk-to` has no reachable completing option in its tree, measured from the stage-6 `root` and every `cast` ledger root at once (static half of `DW0203`). Whether that option is what right-click opens during the beat that needs it is `DW0858`. |
| `DW0130` | Quest `depends_on` cycle. |
| `DW0131` | `finale` is not a declared quest. |
| `DW0132` | `finale` is not the convergent sink (some quest is not a transitive dependency of finale). |
| `DW0133` | **`mandatory: false` below dsl_version 0.17.0**, where the surface is reserved (spec-0051 §9). `every_version` deliberately, and it is the first case the `Binds` doctrine names: the rule judges what the document SAYS against the version that document itself declares, so its verdict is a function of the campaign alone. Fencing it as `Since(17)` would *stop rejecting* `mandatory: false` in a 0.12 campaign — the exact inversion the doctrine warns about. Below the fence the partition is forced empty, so an off-closure quest raises `DW0132` alongside this, exactly as it did before the surface existed. Prescription: raise the **quest-plan** stage's `dsl_version` to 0.17.0, or set `mandatory: true`. |
| `DW0140` | Objective `after` cycle. |
| `DW0141` | Reserved enum value/field for the campaign's `dsl_version`. **This row is the single enumerated list of reserved surface** — §2 deliberately does not restate it (under 0.2.0 the v0.3 verbs/effects; under pre-0.4 the v0.4 surface; under pre-0.5 the v0.5 surface: `time`/`weather`/`lighting`, `set-time`/`set-weather`; under pre-0.6 the v0.6 surface: area `mitigation`, `close-gate`, `damage-players`, `set-checkpoint`, `begin-stealth`/`end-stealth`, `horizon`/`boundary`, the `play-sound` effect + `narrate` `style: art`, per-effect `requires_flags`, `forbids_flags` at every site, `move-npc.on_arrive`, stage-2 npc `deferred` + the `spawn-npc` effect, stage-5 `actors` + `spawn`/`despawn`/`move`/`unleash-actor`, `sequence`, the `traps[]` section, the `bonfire` effect, wave `respawns_on_rest`, wave `equipment`, `waves[].lane` / `waves[].summon`, the `shortcuts[]` / `ambushes[]` / `timed_gates[]` sections, the `loot[]` section, actor `equipment`, and the spec-0022 trap `payload` surface + its `volley` / `collapse` effects; under pre-0.7 the v0.7 surface: the stage-5 `cast` ledger, wave `tier`; under pre-0.8 the v0.8 surface: the stage-4 `branch_points` section, the per-node `happening` on a quest / objective / dialogue option / staging-or-gate-or-ending effect, and the named `campaign-complete` `ending` (spec-0025); the class-kit `flask`, a kit item's potion `contents` and the `bonfire` rest-dialog labels (spec-0016 §1); actor `tier` (spec-0023); the stage-6 dialogue-option `tooltip`; the `close-gate` `sealed_hint`; and the `collect` container-adoption trio `container` / `item_name` / `fill_count` (each field is reserved independently, and an explicit `fill_count: 0` declares nothing since it is the default); under pre-0.10 the v0.10 surface (spec-0031): the stage-5 `state[]`, `lethal_volumes[]` and `on_death` sections, `requires_state` at every gate site, and the effects `set-state` / `add-state` / `clear-state` / `fill-region` / `clear-region`; under pre-0.11 **both** v0.11 surfaces — the press-answer lift (the `narrate` `style: actionbar` and a trigger's `audience: presser`, fenced on the quests stage) and the per-body `traversal` declaration (spec-0034, fenced on the stage that declares it: the stage-2 npc on `npcs`, the stage-5 actor on `quests`, so one stage may adopt it while the other has not)). |
| `DW0142` | Anchor not provided by the area's bound prefab — or, on a site-plan campaign, not among the names the derivation places. The predicate is `AnchorProviders`, unchanged; the prescription is the placement authority's (see the remediation contract above), so a derived map is given the synthesized vocabulary instead of being sent to prefab metadata it does not have and told not to invent a name it is required to invent. |
| `DW0143` | Item id not in the pinned 1.21.11 registry (kit / `collect` / `interact.requires_item` / `give-item`). |
| `DW0150` | Planned quest (stage 4) has no stage-5 expansion. **Two readings, one code, and the discriminator is whether stage 5 declares any quests at all.** Where it declares some and this id is not among them, the refusal is per plan entry and names its two ordinary remedies — write the expansion, or drop the entry — plus how many quests stage 5 does declare, which is what says *mismatch* rather than *unwritten*. Where it declares **none**, the campaign is between the stage-4 plan and the stage-5 quests, every planned quest is unexpanded by construction, and the two remedies are both wrong: writing the expansions IS the next authoring step, and the plan is not a mistake to delete. That case is **one** diagnostic on `/content/quests`, on the model `DW0874` sets — it names every planned quest, says the state is an authoring state rather than a fault, says why the refusal still stands (a plan entry with no expansion has no trigger, no objective and no completion, so nothing of it is emitted), and says there is **no cheaper way out**: the schema-minimal stage-5 quest is refused again by `DW0481` once per quest and `DW0460` once per NPC live in it, so writing empty expansions raises the count instead of lowering it. That last sentence is a measurement and `crates/compiler/tests/plan_awaiting_expansion.rs` takes it; the wording is `crates/dsl/tests/dw0150_plan_awaiting_expansion.rs`. Severity, code and exit are identical in both readings — a plan awaiting expansion cannot build, and a warning would let an unbuildable campaign read as buildable at the step where the difference decides whether anyone writes stage 5. |
| `DW0151` | Stage-5 quest not planned in stage 4. |
| `DW0152` | Stage-2 NPC has no stage-6 tree. |
| `DW0153` | Stage-6 tree references an NPC not in stage 2. |
| `DW0160` | Area binds neither or both of `prefab`/`prefab_pool`. |
| `DW0161` | `prefab_pool` references a pool absent from `prefabs/` metadata. |
| `DW0856` | **A bare `prefab` names a piece the library does not hold** — the same obligation `DW0161` carries on the other arm of the binding. Asked of `AnchorRegistry::has_prefab`, which answers `Some(true)`/`Some(false)` from a registry that is the whole library and `None` from one that is not, so a subset registry or a test double refuses nothing on its word; `anchors_for` cannot answer it, because its `None` deliberately means *defer*. It is an error rather than a deferral because an area whose piece is absent contributes **no anchor set at all**, and every per-area anchor check reads a missing set as a deferral and skips: the anchor proof (`DW0142`) over every quest in that area then examines zero anchors and passes. A mistyped piece is therefore not merely accepted — it is **less checked than a correct one**, which is the unbound vacuity mode one keystroke away. Measured on the gallery: mistyping `prefab/gallery-hall` by one character turned seven `DW0142` refusals into an exit-0 run, and the per-area proof went from four objective anchors examined against twenty-eight declared to zero examined and four skipped. Prescription: correct the id, or add the piece to the prefabs dir. (A piece whose metadata file failed to parse is absent from the registry too and so is reported here as well as by `DW0346`; both name the same missing piece from the two ends.) |
| `DW0162` | Stage-7 edit script structurally invalid (v0.6, spec-0017): an edit names a region no earlier `select` in its batch defined (region refs are strictly backward within a batch), a `union`/`intersect` lists < 2 regions / a `subtract` removes nothing, a box `min` > `max` on an axis, a surface band `from` > `to`, a palette recipe is empty or carries a non-positive/non-finite `weight`/`scale`, a `matching` list is empty, or a morph `by`/`passes` is 0. (Unknown recipe/matching block ids reuse `DW0193`; id syntax `DW0110`; duplicate batch/region names `DW0111`; a `world-edits` doc under a pre-0.6 `dsl_version` `DW0141`.) |
| `DW0170` | `kill`/`spawn-wave` references an undeclared `wave/<id>`. |
| `DW0171` | A killed wave is never spawned by any `spawn-wave`. |
| `DW0172` | `requires_flags` references a flag no `set-flag` produces. The producer scan descends every nested effect list (`sequence` steps, `on_respawn`/`on_caught`/`on_arrive`), so a `set-flag` nested in a timeline still counts as a producer (no spurious fire). |
| `DW0173` | Wave-mob `entity` is not a known vanilla entity id. |
| `DW0180` | l10n sidecar absent / inconsistent envelope / under-covers inventory (also if `en` is declared). Compiler-level. The inventory it demands coverage of spans **every effect root emission can lower** — including `traps[].payload` and a dialogue option's `set-checkpoint` `on_respawn` bundle; a string in either used to ship English-only in a translated build, uncovered. |
| `DW0181` | l10n sidecar has an orphan key (over-coverage). Compiler-level. |
| `DW0182` | A player-visible string — authored English (the whole l10n inventory) or any sidecar translation — contains the reserved completion-marker sigil `[dw:complete`. That chat sequence is the validation bot's completion oracle (§4 "The completion-marker channel"); content carrying it could forge a passing critical-path step, so the sigil is **reserved**, not merely discouraged. Reword the line. |
| `DW0183` | (i18n v2, spec-0029) A player-visible string — authored or translated — contains a character from the reserved private-use block `U+E000..U+F8FF`. That block is how the compiler carries an l10n key from the stage docs to the text component the string is emitted into (`dsl::l10n::TR_SIGIL`), so content carrying it could impersonate a translation tag; it also has no glyph in any Minecraft font. Remove the character. |
| `DW0184` | (i18n v2, spec-0029) A declared `world.languages` code does not resolve to a language file the **pinned client actually loads** (`dsl::mclang::CLIENT_LANGS`, derived from Mojang's 1.21.11 asset index), so its `assets/delvewright/lang/<code>.json` would sit under a filename no client ever asks for and the language would ship invisible. Also fires on an ambiguous bare code (`zh`, `sr`, `be` — several regions, no `<lang>_<lang>`), because guessing the region is how a language ships invisible. Use a code the client loads. A language is never silently dropped. |
| `DW0190` | Mannequin `skin.texture_id` malformed or duplicated. |
| `DW0191` | A `talk-to` has no **ungated** completing option (all `requires_flags`-gated → deadlock risk). |
| `DW0192` | Wave-mob `effects[].effect` not a known 1.21.11 status-effect id. |
| `DW0193` | `set-block`/`interact.prop` block id not a known 1.21.11 block id (base id checked; a malformed blockstate suffix `id[…]` — unbalanced `[]`, empty, or non-`key=value` tokens — reuses this code). |
| `DW0194` | Environment-trigger id malformed/duplicated, or `approach` `range` 0. |
| `DW0195` | A `talk-to` targets an NPC despawned by a prerequisite quest. |
| `DW0196` | Area `lighting.min_light` out of range (must be 1..=14). v0.5, spec-0010. |
| `DW0197` | A stage-2 NPC declares `deferred: true` but **no** `spawn-npc` effect anywhere (quest, trigger, nested timeline, or dialogue) summons it — the NPC never enters the world, so its dialogue tree and any `talk-to` on it are unreachable content. v0.6; the staging dual of `DW0195`. Prescription: add the `spawn-npc` at the entrance beat, or drop `deferred`. (0197/0198 were *reserved* by spec-0011's draft and released when it renumbered to `DW0340`/`DW0341`; no code ever emitted them.) |
| `DW0198` | A `talk-to` on a `deferred` NPC provably activates before the NPC exists: every `spawn-npc` for it fires in a quest that is a **strict DAG descendant** of the objective's quest. Conservative by construction — a spawn from a trigger, from dialogue, or from the objective's own quest is not DAG-ordered and suppresses the proof rather than risking a false positive (see the gap note below). v0.6. |
| `DW0199` | A `cutscene` effect's shape is invalid: it mixes the multi-shot `shots` list with the single-shot `path`/`seconds` fields, declares neither, omits `seconds` on a single shot, or gives a shot with an empty camera `path`. The two spellings normalize to one shot list, so this is where the shape is policed and emission may then assume a well-formed, non-empty list. v0.6. |
| `DW0320` | `horizon:"ocean"` declared without a `boundary` (an infinite swimmable sea with no return rule). v0.6, spec-0013. Numbered in the 032x world/region family but **validation-tier (exit 1)**, not a DW03x build code. |
| `DW0321` | `boundary.margin` outside `0..=64`. v0.6, spec-0013. Validation-tier (exit 1). |
| `DW0340` | Trap declaration structurally invalid (v0.6, spec-0011): a malformed/duplicate `trap/<id>`, an `at`/`disarm.via` that no area's prefab provides, or a `disarm.via` that collides with the trap's own trigger anchor. Renumbered off the spec's stale reserved number (0197). |
| `DW0341` | A trap `dispense` payload item id is not in the pinned 1.21.11 registry (v0.6, spec-0011; mirrors `DW0143`). Renumbered off the spec's stale reserved number (0198). |
| `DW0343` | A verb that needs a gate anchor's **fill block** targets an anchor that declares none (or is not a gate region at all): `close-gate` (v0.6), which fills the region back in, or a stage-5 `shortcut` (spec-0016 §2), whose unlock clears exactly that block and whose gate is sealed by it from world-load. Compiler-side (needs prefab metadata the DSL anchor registry does not carry), reported at **validation tier (exit 1)** like the atmos `DW032x` checks. **The scan is over the pieces this campaign's areas can place** — each area's bare `prefab`, or every member of its `prefab_pool` — and **all** of them that declare the anchor as a gate must declare a fillable one. A piece the campaign binds no area to has no standing to answer: it cannot be placed, so what it says about a gate is a fact about a different building. What the anchor declares is asked of one authority, `PrefabMeta::gate_anchor`, so an anchor carrying an explicit `region` + `block` and one carrying a `resolves_to` of `bar:<region>` resolve identically; where that authority refuses (the two forms disagree, the named bar is not in the piece's contract, or the bar's boxes do not fill their own bounding box) its reason is quoted into this diagnostic. Prescription: the placement authority's (see the remediation contract above) — on a prefab campaign, declare the gate on an anchor of a piece an area binds, as a `region` plus a `block` or as a `bar:` the piece's spatial contract carries; on a site-plan campaign, name an `anchor/seam-<edge>` the derivation cuts over a barred connection the layout graph declares. Either way, or remove the verb. A gate whose block the derivation supplies never reaches this check at all — `siteplan::synthesized_gate_block` answers first. |
| `DW0857` | **A gate verb names an anchor more than one of the campaign's areas provides**, so nothing an author can see says which building it fills, clears or opens. `close-gate`, `open-gate`, `shortcut` and `timed-gate`; `compiler::gates::check_close_gates`, validation tier (exit 1). **The scope of uniqueness for an anchor name is the AREA**, and that is the scope `DW0142` already resolves every anchor reference in — it checks each reference against the anchors of the quest's own area and makes exactly one exception, a cutscene camera, which may fly anywhere. The compiler's by-name lookup honours none of it: it walks a map keyed by `(area, name)` and returns the first entry whose NAME matches, across every area, first match wins. While one area provides the name the two readings agree, which is why nothing noticed; when two do, the answer is whichever area id sorts first. Measured on a campaign of eight zones: five names collided, two on the critical path — a portcullis shadowed by a chapel door, and an escort beat whose destination resolved back to the cell the NPC already stood on. This is not the unbound vacuity mode — the check examined something and reported truthfully about it — but the computed-key family: the lookup asked the right question about the wrong object, and a green meaning *another building satisfies this* is indistinguishable at the call site from a green meaning *this one does*. **What is refused is the ambiguity, not the crossing**: an anchor exactly one area provides resolves from anywhere exactly as before, so a beat that legitimately reaches into another area still does and no unambiguous campaign moves a byte. Two pieces inside ONE area sharing a name is not this finding either — that is what a `prefab_pool` is for, and it has always resolved within the area. **Prescription, and who can perform it.** The anchor name is a key in the piece's exported metadata, shared by every campaign that binds that piece, so renaming it is a prefab-library change and is not something a campaign document can make — the message says that rather than prescribing it. What a campaign can change is the BINDING: in `world.areas[]`, give one of the named areas a `prefab` or `prefab_pool` that does not declare the name, after which the remaining single provider resolves from anywhere exactly as before. The message therefore names which piece each area provides the anchor through (`gates::gate_providers` keeps the carrier, not just the area), because an author cannot choose which area to change without it. Where both areas must keep their pieces, the names have to stop colliding in the pieces themselves and the message says so plainly — that is a remedy; a repair the reader is not allowed to perform is not. There is still no escape hatch, deliberately: one would have to be the author naming which area they meant, which is the area-scoped resolution this diagnostic exists because the compiler does not have. Only a stage-1 `areas[]` campaign can reach it — a campaign carrying a `site-plan.json` declares an empty `areas` list (`DW0839`), so two providers cannot exist. |
| `DW0859` | **An anchor reference no scope settles names a place more than one of the campaign's areas provides.** `DW0857` is this finding keyed to four gate verbs; this is the same rule keyed to the object class the property belongs to, because a name two buildings answer to is a fact about the name and not about the verb that said it. Build tier (exit 3), `compiler::plan`, because a pool area defers its anchors to the solver and the DSL tier cannot see the second provider. **The scope of uniqueness for an anchor name is the AREA**, so the answer is normally to resolve in the referring area — and for the two object classes the DSL gives an area to (`Npc`, `PlannedQuest`) that is what the compiler does: a cast beat stands in the area the BEAT plays in, then the NPC's declared home, then an unambiguous crossing. The other eleven anchor-bearing classes — traps, shortcuts, timed gates, loot, lethal volumes, ambushes, actors, waves, stealth zones, shops, environment triggers — are flat campaign-wide vectors that **never record an area at all**, so there is no scope to resolve them in. For those a by-name match is a *candidate*: one candidate is an answer and resolves from anywhere, two is a question the compiler cannot answer, and answering it by whichever area id sorts first is how a green meaning *another building satisfies this* became indistinguishable from a green meaning *this one does*. **What is refused is the ambiguity, not the crossing**, so no unambiguous campaign moves a byte. Prescription: it leads with the move that costs nothing and is entirely the campaign's — name a place the referring area itself provides — and then carries the same sentence `DW0857` prints, from the one writer (`gates::anchor_ambiguity_remedy`), because what an author may do about a name two of their buildings answer to is a fact about anchor names and not about the verb that said one. Renaming the anchor is named as a prefab-library change the campaign cannot make, not prescribed as the fix; the campaign-side binding change (`world.areas[]`) is. This site knows the provider areas but not which piece of each provides the name — the anchor table records `(area, name)` and no carrier — so the writer leaves the per-piece clause out rather than rendering it empty. There is no hatch, for the reason `DW0857` has none — an opt-out would have to be the author naming which area they meant, which is the area-scoped resolution the DSL cannot express for these classes, so the hatch and the missing capability are the same thing. |
| `DW0866` | **An optional quest inside the finale's dependency closure** (spec-0051 §8.1), including a `finale` that declares itself `mandatory: false`. Validation tier (exit 1), `every_version`, `dsl::validate`. The delve cannot be completed without the quest, so calling it optional is a claim the completability proof would then rest on — and the skip world, in which no optional objective is ever completed, is exactly the world where the finale never fires. The closure is asked of `QuestPlanContent::spine`, the ONE authority on it; the declaration is asked of `QuestPlanContent::optional`, the ONE authority on the other half. **Its mirror image is `DW0132`**, which keeps today's convergence refusal for a MANDATORY quest the closure does not reach: the two are opposite errors and a shared message could prescribe neither. **Co-fires with `DW0867` whenever a mandatory quest's `depends_on` names an optional one**, necessarily — such a dependency is inside the closure by construction — and the two are kept apart because they prescribe different repairs: `DW0867` names the edge to cut, this one names the claim to withdraw. Prescription: set `mandatory: true`, or cut the `depends_on` chain that puts it in the closure. |
| `DW0867` | **A mandatory quest whose `depends_on` edge or stage-5 `quest-complete` trigger names an optional quest** (spec-0051 §8.2). Validation tier (exit 1), `every_version`, `dsl::validate`. The party may never play elective content, so a mainline beat waiting on it stops the delve in the skip world. **Refused at the edge, naming the edge**, which is where an author can act. One rule over **two** edge kinds, so one code: `depends_on` orders the plan, and the stage-5 trigger is what actually arms the quest at runtime — nothing ties the two together (a `quest-complete` trigger resolves against the stage-5 quest set, never against stage 4), so a campaign can spell this edge with either alone. The trigger arm is the one that reaches this code by itself; see `DW0866` for why the `depends_on` arm always co-fires. **The reverse directions are both legal and deliberately unreported**: an optional quest may `depends_on` a mandatory one (that is a strand's attachment to the spine) and may be triggered by a mandatory completion (a skipped quest still activates — §5). Prescription: mark the named quest mandatory, or move the edge onto the spine. |
| `DW0868` | **A mainline key behind participation** (spec-0051 §8.3): a mandatory quest's objective whose `requires_flags` names a flag every producer of which is rooted in an optional quest. Validation tier (exit 1), `every_version`, `dsl::validate`. A party that plays only the mainline can never open that beat, so the delve is not completable with zero optional participation. **The producer partition is conservative in the safe direction**: one producer anywhere else — a mandatory quest, an environment trigger, a trap disarm, a dialogue option, `on_death` — takes the flag out of the set. Dialogue is counted as non-optional deliberately, because whether an option is reachable only inside an optional quest's scene is a cast-ladder question this rule cannot answer and answering it wrongly would refuse a correct campaign. **`DW0204` is the compensating stronger check behind it** — the participation-minimal replay credits only the exported path's own producers, so this shape fails there too; what the edge buys is a message that names the strand instead of a walk that stops. **The `requires_state` and `dropped_by` chains of §8.3 are NOT covered here** and reach only `DW0204`. A flag nothing produces at all is `DW0172`, not this. Prescription: move the `set-flag` onto a mandatory quest, mark the producing quest mandatory, or drop the gate. |
| `DW0869` | **A station takes a name the engine derives** (spec-0052 §7.1). Validation tier (exit 1), `every_version`, `dsl::layout`. A layout-graph node's `stations[]` entry whose `anchor` begins `anchor/node-`, `anchor/seam-` or `anchor/unlock-`, or equals `spawn`. **The prefix is the rule, not the collision**: `anchor/seam-vestry-door` is refused whether or not the graph has such an edge today, so adding the edge later cannot turn a legal graph into two claims on one name. `spawn` is reserved by its exact name because it is one name with no family. Prescription: name the station something of the author's own — the quest layer references a declared name exactly as it references a derived one. |
| `DW0870` | **Two stations claim one name** (spec-0052 §7.2). Validation tier (exit 1), `every_version`, `dsl::layout`. Two `stations[]` entries anywhere in the graph — one node or two — declaring the same `anchor`. **The scope of uniqueness is the AREA**, unchanged from the standing rule that every anchor reference resolves in an area, and a site-plan campaign has exactly one, so the campaign's whole vocabulary (synthesized ∪ declared) shares it. Piece anchor names stay piece-scoped and never enter this scope, which is why two pieces may both declare `anchor/door` and collide with nothing. The message names both nodes. Prescription: rename one, or declare it on one place only — a quest in either place may name a station of the other. |
| `DW0871` | **A reference demands a shape the station is not** (spec-0052 §7.3). Validation tier (exit 1), `every_version`, `dsl::validate`. Judged at the reference site **from the declaration**, with zero pieces bound, so the answer does not wait for a piece to arrive; when one does, `DW0842` demands the same shape of the piece anchor it binds to, so the two readings cannot drift. The demand travels with the reference in `QuestEffect::anchor_refs`, the ONE authority on the anchor-bearing effect surface, so a new anchor-bearing variant cannot be added without stating what it does with the anchor. **Gate-demanding sites**: `open-gate`, `close-gate`, a `shortcut`'s `gate`, a `timed-gate`'s `gate`. **Point-demanding sites**: every objective anchor, an NPC or actor station, a `move-actor`/`move-npc` destination, a trigger's `at`, a trap's `at` and disarm, a timed gate's disarm, a shop counter, a loot chest, a lane waypoint, every camera field, and **the centre of every anchor-centred volume** — a `lethal_volumes[]` region, `damage-players`'s `in`, a `volley` kill zone, `collapse`, `begin-stealth`, `fill-region`, `clear-region` — all of which are a `StealthZone`, resolved from a point plus an extent rather than from a region anchor. Returns nothing for a prefab campaign by construction: `AnchorRegistry` answers names only, so a piece's shape stays the compiler's to discover at placement. Prescription: change the station's `kind` in the layout graph, or name a station that is already the demanded shape. |
| `DW0348` | A `shot_style` declaration is semantically invalid (v0.6, spec-0015): a styled shot with no `subject`; style params (`subject`/`subject_b`/`dist`/`degrees`/`bearing`) on an unstyled shot; `subject_b` off `two-shot` (or a `two-shot` without one); `degrees` off `orbit-arc` or outside `45..=120`; `dist` outside `1..=48`; `bearing` outside `-360..=360`. Validation-tier (exit 1), `dsl::validate`. |
| `DW0349` | A `side-track`/`low-follow` shot whose subject provably cannot move: those styles dolly *with* a moving subject, so the subject must be an npc/actor with a matching `move-npc`/`move-actor` in the same effect group or the same `sequence` timeline (an `anchor` subject can never move; reaction lists `on_arrive`/`on_caught`/`on_respawn` start a fresh scope — their firing time is statically unknowable). Validation-tier (exit 1), `dsl::validate`. Prescription: add the move alongside the cutscene, or use a static style (`locked-off`, `push-in`). |
| `DW0356` | `world.min_players` outside `1..=4` (v0.6, spec-0018). A delve is played by ONE party of 1–4 (the product definition), so a declared mandatory size can never sit outside it. Absent = 1. Validation-tier (exit 1), `dsl::validate`. |
| `DW0357` | A `carrier: "one"` `give-item` sits in a bundle only the scheduler ever runs — a `sequence` step, or a `move-npc`/`move-actor` `on_arrive` (v0.6, spec-0018). Those run with the server command source and have no acting player, so the single prop would reach nobody. The walk **stops** at `set-checkpoint.on_respawn` / `begin-stealth.on_caught`: those are dispatched per player and do have an `@s`. Validation-tier (exit 1), `dsl::validate`. Prescription: drop `carrier` (arm the whole party), or move the hand-off onto the beat a player completes. |
| `DW0350` | A `use` trigger anchored where an NPC stands (round-6 island QA). Right-click on an NPC already belongs to its dialogue advancement; a second interaction hitbox in the same cell makes the client's entity ray-pick ambiguous, and whichever entity loses the tie is silently dead — the soft-lock class that starved the giant's dialogue of every right-click. Left-click triggers are exempt (a left-click has no dialogue meaning): they ride the NPC's own hitbox instead of summoning a second one. Validation-tier (exit 1), `dsl::validate`. Prescription: move the trigger to its own anchor, express the interaction as a dialogue option, or — if the NPC's body is genuinely the target — use `on: strike-npc`, which takes no anchor at all. |
| `DW0377` | A `timed-gate` declaration (spec-0016 §4) is structurally invalid: a malformed or duplicate `timed-gate/<id>`, an `open_ticks` or `closed_ticks` of 0 (a gate that never opens, or never closes — that is `open-gate`/`close-gate`, not a clock), a `phase` at or beyond the full cycle, two timed gates driving one region (two clocks race every tick and the region's state becomes emission order, not design), a gate a `shortcut` already owns (a clock would re-seal what `DW0372` exists to forbid re-sealing), or a `disarm.via` anchor no area's prefab provides / one that IS the gate anchor (the jam lever cannot stand inside the span the portcullis closes on). Validation-tier (exit 1), `dsl::validate`. |
| `DW0375` | An `ambush` declaration (spec-0016 §3) is structurally invalid: a malformed or duplicate `ambush/<id>`, an empty `actors` list (an ambush that springs nothing), or the same actor listed twice — `spawn-actor` is idempotent, so the second one is a silent no-op and the ambush is half the size it reads as. Validation-tier (exit 1), `dsl::validate`. Deliberately does **not** require a `telegraph`: the un-telegraphed ambush is core souls vocabulary. Everything else about an ambush is checked as the trigger it desugars to (`DW0194`, the anchor seals, `DW0350`). |
| `DW0371` | A `shortcut` declaration (spec-0016 §2) does not resolve: a malformed or duplicate `shortcut/<id>`, a `gate`/`unlock` anchor no area's prefab provides, or an `unlock` equal to its own `gate` — the mechanism belongs on the far side of the door it opens, which is the entire point of the pattern. Validation-tier (exit 1), `dsl::validate`; anchor resolution stays lenient for pool areas the compiler resolves later, like the trap and trigger checks. |
| `DW0372` | A `close-gate` effect targets a gate a `shortcut` owns (spec-0016 §2). A shortcut opens **permanently** — that is the pattern — so permanence is made structural rather than left to authoring discipline: there is simply no way to spell the re-seal. The scan descends nested effect lists, so a `close-gate` buried in a `sequence` step is caught. `close-gate` on any other gate (the point-of-no-return staging beat) is untouched. Validation-tier (exit 1), `dsl::validate`. |
| `DW0389` | A `close-gate` effect targets the gate of a `timed-gate` that declares a `disarm` (souls dossier §5.2). A disarm suppresses the clock **permanently with the gate resting OPEN** — a jammed portcullis stays up — so, exactly as for a `shortcut` (`DW0372`), permanence is structural rather than left to authoring discipline: there is no way to spell the re-arm. The scan descends nested effect lists, so a `close-gate` buried in a `sequence` step is caught. A `close-gate` on a timed gate with **no** `disarm` is untouched — that clock is still a clock and the point-of-no-return beat may seal it. Validation-tier (exit 1), `dsl::validate`. |
| `DW0381` | A wave's TD `lane` / `summon` declaration (spec-0016 §6) is structurally invalid or internally contradictory: an empty `waypoints` list, a waypoint anchor no area's prefab provides, a repeated consecutive waypoint (the squad would be sent where it already stands, and vanilla re-rolls a patrol target on arrival), an `aggro_radius` outside `4..=64`, a mob whose `attributes.follow_range` disagrees with `aggro_radius`, or `lane` together with `summon: aggro-edge`. The `follow_range` clause is the subtle one: release radius and perception radius must be the same number, because a patrolling raider that targets a player it cannot engage HOLDS GROUND instead of marching — the squad stalls mid-lane with every other proof green. Validation-tier (exit 1), `dsl::validate`; anchor resolution stays lenient for pool areas the compiler resolves later. |
| `DW0382` | A lane wave fields a non-raider species (spec-0016 §6). `Patrolling`/`patrol_target` are Raider NBT: on any other mob they are simply dropped and it stands where it spawned — the silent no-op class. **The lane roster is Mojang's, never ours**: it is vanilla's own `#minecraft:raiders` tag, read from the vendored entity-type tag table (`crates/dsl/data/entity-tags-1.21.11.json`, regenerate with `tools/extract-entity-tags.py`), the same rule `DW0496` follows for `#minecraft:burn_in_daylight`. For 1.21.11 it holds evoker, illusioner, pillager, ravager, vindicator and witch. Three independent readings of the pinned server jar agree on that six: the tag itself; the entity types whose constructed class is a `PatrollingMonster`; and the entity types whose class is a `Raider`. The three NBT keys are string constants of exactly one class an entity is built from, `PatrollingMonster`, whose own `registerGoals` adds the `LongDistancePatrolGoal` every subclass inherits — so honouring the NBT and having the goal are the same membership question. `tools/check-patrol-types.py` re-derives all of it from the pinned jar and refuses on any disagreement. Validation-tier (exit 1). Prescription: use `summon: aggro-edge`, which needs no patrol AI, for everything else. |
| `DW0383` | A lane wave fields fewer than 2 mobs (spec-0016 §6). A lone patroller sets `Patrolling:0b` on ITSELF when it finds no companion within its follow range (vanilla, live-verified), so a one-mob lane cancels its own routing. Validation-tier (exit 1). |
| `DW0384` | A lane `pillager` is not holding a crossbow (spec-0016 §6). Its only attack goal is the crossbow goal, so on acquiring a target it has nothing runnable to do — while the patrol goal is meanwhile blocked BY that target — and it freezes in place indefinitely (live-verified deadlock). The compiler arms pillagers by default, so this fires only on an explicit `equipment.main_hand` override, which is exactly the remaining way into the deadlock. Validation-tier (exit 1). |
| `DW0385` | A `summon: aggro-edge` wave mob declares no `attributes.follow_range` (spec-0016 §6). That radius IS the summon ring — the distance at which the mob perceives the party — so it is authored, never guessed: the compiler will not fabricate a vanilla default it cannot verify against the pinned server. Validation-tier (exit 1). |
| `DW0370` | A wave declares `respawns_on_rest: true` but the campaign declares **no** `bonfire` (spec-0016 §1) — nothing can ever fire the re-seat, so the field is a silent no-op, the defect class this compiler always makes loud. Validation-tier (exit 1), `dsl::validate`; the scan descends every nested effect list (a `bonfire` inside a `sequence` step counts) over quests and triggers. Prescription: add the bonfire the re-seat hangs off, or drop the field — never leave a dead declaration in the DSL. |
| `DW0499` | A wave declares **both** `tier: boss` and `respawns_on_rest: true` (spec-0016 §1, spec-0023; stage bosses never respawn on rest). `tier` and `respawns_on_rest` are two fields on the SAME wave declaration — the only place a "boss" billing and a "re-seat on rest" contract can land on one another: an actor carries `tier` too (spec-0023's "other shape an elite takes"), but has no `respawns_on_rest` field at all — an actor is killed by hand, never by a `kill` objective, and the bonfire re-seat machinery only ever re-summons **waves** — so an actor-shaped boss is structurally incapable of expressing this violation, and the check is scoped to the one shape that can. A rest-respawning boss re-fight breaks the retry economy that rule protects: a boss is the campaign's named fight, not trash pressure the party grinds back down every rest. Validation-tier (exit 1), `dsl::validate`; checked unconditionally of whether a `bonfire` exists — the combination is forbidden on its own terms, not merely inert like `DW0370`. Prescription: drop `respawns_on_rest` if the encounter really is the boss, or drop `tier: boss` (bill it `elite` instead) if it is meant to re-seat. |

### DW032x/033x — sound & art-title validation (`compiler::atmos`; error; exit 1)

v0.6 (spec-0014) content checks that need compiler-vendored data (the pinned
`sound_event` registry) or the `delve:art` font, run in the compiler's
`validate_stage` (so `validate`/`analyze`/`build` all catch them) and reported at
**validation tier (exit 1)** like the `DW01xx` codes — not build-tier. No-op for a
campaign that uses neither the `play-sound` effect nor the `narrate` `art` style.

**Nested-effect consumer recursion.** These scans (`sound_refs`/
`play_sound_actor_refs` → `DW0326`/`DW0335`, `art_narrates` → `DW0328`) descend
**every nested effect list** (`sequence` steps, `on_respawn`/`on_caught`/`on_arrive`
bundles) through the one `each_effect_ref` traversal, keyed by the same position
scheme as the l10n inventory — so a bad sound id / non-Latin art string buried in a
timeline is caught, not shipped unvalidated. The DSL-side effect-ref consumer scans
(`spawn-wave` → `DW0170`, `give-item`/`collect`/`requires_item` → `DW0143`,
`set-block`/`prop` → `DW0193`, `move-npc`/`despawn-npc` → `DW0112`, per-effect
`requires_flags` → `DW0172`) likewise recurse via `for_each_effect_deep` /
`for_each_trigger_effect_deep`. This matches how the flag/wave **producer** scans and
emission already descend; top-level
paths/keys are unchanged, so a nesting-free campaign validates byte-for-byte
identically.

| Code | Meaning |
|------|---------|
| `DW0326` | A `play-sound.sound` (v0.6) or `narrate.sound` (v0.4) id is not a known 1.21.11 sound event (validated against the vendored `sound_event` registry, `crates/compiler/data/sounds-1.21.11.json`; `minecraft:` prefix optional). |
| `DW0328` | An `art`-styled `narrate` string — the English source **or** any declared-language sidecar translation — uses a character outside the `delve:art` font's glyph inventory (A–Z, 0–9, space, `! " ' ( ) , - . / : ; ?`; lowercase folds to uppercase). Forces per-language art titles to stay ASCII/Latin — a `zh-cn` art translation must be an ASCII rendition. |
| `DW0335` | A `play-sound` targets `at: {actor: …}`. A sound plays at fixed coordinates or at each listener's own position, and the compiler resolves no position for a live actor, so the sound would be silent. Use `at: {anchor}` or `at: players`. |

#### The `delve:art` font

An original 5×7 pixel bitmap font authored in `compiler::atmos` (`ART_GLYPHS`), baked
into `resourcepack.zip` as `assets/delve/font/art.json` +
`assets/delve/textures/font/art.png` — and **only** when the campaign uses `style:
art`, so a non-art campaign's pack stays byte-identical. The PNG is written by a
hand-rolled deterministic encoder (stored DEFLATE, no compressor), like the pack's
hand-rolled ZIP/SHA-1.

| Constant | Value | Meaning |
|----------|-------|---------|
| `CELL` / `GW` / `GH` | 8 / 5 / 7 | atlas cell, glyph ink width, glyph ink height, in source px |
| `ART_SCALE` | **1** | the provider's source-pixel scale — the one knob for on-screen size |
| `ART_HEIGHT` / `ART_ASCENT` | 8 / 7 | provider `height` / `ascent` = `CELL·ART_SCALE` / `GH·ART_SCALE` |
| `ART_GLYPH_ADVANCE` | **6** | `GW·ART_SCALE + 1`, vanilla's `round(ink·height/cellHeight)+1` |
| `ART_SPACE_ADVANCE` | 4 | the `space` provider's advance, `4·ART_SCALE` |

**`ART_SCALE` must stay an integer** — the font atlas is sampled nearest-neighbour,
so a fractional scale splits a source pixel across screen pixels and the glyph edges
go ragged. It was **4** through v0.6, which is why art banners could not physically
fit: an art `narrate` renders in the vanilla **title** slot, so the provider scale
and the slot's ×4 pose scale multiply, and 21 font px/glyph against `DW0330`'s 90 px
budget left room for *four* glyphs. The island's `NOBODY` (126 px) and `HOMEWARD`
(168 px) ran off both edges on screen. Halving to 2 was not enough — 11 px/glyph fits
8, which `HOMEWARD` exactly exhausts — so `ART_SCALE` is **1**, the largest integer
scale that fits **15** glyphs. The title slot still draws it ×4, so an art title
remains a title-sized blocky all-caps banner; what changed is that it now occupies a
title's share of the screen instead of four times it. `ART_ASCENT = GH·ART_SCALE`
keeps the ink sitting exactly on the baseline at any scale.

The width model treats every glyph as a flat `ART_GLYPH_ADVANCE`. That is exact for
every letter and digit (all ink the full 5 columns) and deliberately **conservative**
for the few narrow punctuation glyphs (`'`, `(`, `!`), which vanilla advances less —
`DW0330` never under-measures a line.

### DW0330 — on-screen text fit (`compiler::textfit`; **warning**; exit 0)

An **advisory** code (with `DW0351`, one of the compiler's two). Vanilla draws a `title`, a `subtitle`
and an art title centred, on **one line, with no wrapping and no shrink-to-fit** —
text wider than the screen just runs off both edges, silently. `DW0330` measures
each on-screen `narrate` string's **rendered width in font pixels** and compares it
to the style's budget.

**Why measured, not counted.** `i` and `W` differ by 3× in the vanilla font, and a
Han glyph is 9 px against a Latin letter's 6 (1.5×, *not* the 2× a "CJK counts
double" rule assumes). A character count is unfair to whichever script it was not
tuned for, so the check sums real advances: the ASCII sheet's per-glyph widths, the
`unihex` full-width advance for CJK, and — for `art` — the `delve:art` font's own
glyph metrics, derived from the same constants that emit the font.

**Budget.** `Gui.renderTitle` renders a title at pose scale **×4** and a subtitle at
**×2**; an art title is a title, so it takes ×4 on top of the `delve:art` provider's
own scale (see [The `delve:art` font](#the-delveart-font)). Against a reference GUI
width of **426** scaled px (what Minecraft's auto GUI scale yields at 1280×720 and
2560×1440; 1920×1080 gives 480, and 320 is the auto floor) at **85%** usable width,
the budgets are **90** font px for `title` and `art` and **181** for `subtitle`.
`chat` has no budget — it wraps and scrolls. At the art font's 6 px/glyph that is
**15 art glyphs**; the lint reads the same `ART_GLYPH_ADVANCE` / `ART_SPACE_ADVANCE`
constants the font emission does, so the two cannot drift.

**Why warning, not error.** The true limit is a property of the player's window and
GUI scale, which the compiler cannot know; rejecting on it would dress a judgement
call as a fact, and would hard-block a translation for being honestly longer than its
English source. It reports, and the author shortens.

**Scope.** The canonical English source **and** every declared-language sidecar
rendition, walked by the same `each_effect_ref` traversal and l10n keying as
`DW0326`/`DW0328` — so a sidecar finding is reported at
`l10n/<lang>.json#/content/<key>`, naming the exact string to shorten. Nested
effects are covered.

| Code | Meaning |
|------|---------|
| `DW0330` | An on-screen `narrate` string (`title` / `subtitle` / `art`) — English source or any declared-language sidecar rendition — renders wider than fits on screen. Advisory (exit 0): shorten the line. Do **not** demote a title to `chat` to silence it, and do not assume a wider monitor fixes it — the overflow scales with GUI scale, not away from it. |

### DW0331 — dialogue option button fit (`compiler::textfit`; **error**; exit 1)

Same font metrics as `DW0330`, a harder limit, and the opposite severity — for a
reason that is worth stating precisely, because "follow the precedent" here means
following its *reason*, not copying its tier.

**A dialogue option is a button caption.** `emit::build_node_dialog` emits each node
as a `minecraft:multi_action` dialog with `columns: 1` and **no `width` override**,
so every option button is vanilla's default **150 GUI px**. Vanilla draws a button's
label via `AbstractWidget::renderScrollingString`, inset **2 px** per side: a label
wider than the remaining **146 px** neither wraps nor shrinks — it **scrolls back and
forth**, and a shelf of sliding captions is unreadable to choose from.

**Budget: 146 font px, no scale divisor.** Dialog buttons draw at the identity pose
(**×1**), so one font pixel is one GUI pixel — unlike `DW0330`'s titles at ×4/×2.
Rules of thumb from the advances, for authoring: ~24 Latin or ~16 Han characters at
the threshold, so author to **~20 / ~12** and leave a translation room to grow
(the `/new-delve` page, *Writing craft* §C, in the campaigns repository;
`docs/reference/i18n.md`).

**Why error, not warning.** `DW0330` warns because its reference GUI width is a guess
about the *player's window*, which the compiler cannot know, and rejecting a build on
a guess dresses a judgement call as a fact. That reasoning does not transfer: 150 px
is the button width because **this compiler emitted no `width`**, on every window at
every GUI scale. `width > 146` therefore *is* "this caption scrolls in game" — a
property of the datapack being built, so it rejects. The remedy is never a wider
button: move the content into the node's body text, which wraps, or into the NPC's
reply.

**Not the option `tooltip` (v0.8).** A `tooltip` is a sibling of `label` in
vanilla's button codec but is never drawn on the button: the client wraps it with
`Tooltip.create(…)` → `Font.split(message, 170)` into its own hover box. Wrapping
is the whole difference — the defect `DW0331` rejects is a caption *scrolling*
inside a fixed button, and nothing overruns a box that wraps. So a tooltip carries
no width budget, and inventing one would forbid exactly the pattern the field
exists for ("button = caption, tooltip = the full line"). The label on an option
that also has a tooltip is measured exactly as before.

**Scope.** Every `.opt.<n>.label` in the canonical English source **and** every
declared-language sidecar rendition, keyed by the same `dlg.<npc>.<node>.opt.<i>.label`
inventory keying as `validate_l10n` (`dsl::l10n::dialogue_option_labels`) — so a
`zh-cn` label that overflows where its English source fits is reported at
`l10n/<lang>.json#/content/<key>`, naming the language and the exact string. Display
gating (`requires_flags`/`forbids_flags`) decides *whether* a variant shows a label,
never how wide it renders, so gated options carry the same budget.

| Code | Meaning |
|------|---------|
| `DW0331` | A dialogue option `label` — English source or any declared-language sidecar rendition — renders wider than the 146 usable font px of the 150-GUI-px dialog button it is drawn on, so vanilla scrolls the caption instead of sitting it still. Error (exit 1): cut the label to a caption and move what it carried into the node's body text or the NPC's reply. Scope follows the **widget**, not the stage: a `bonfire`'s authored `rest_label` / `save_label` (v0.8) are drawn on exactly that button and are held to exactly that budget, reported under the `quests` stage. The compiler's own canonical English is measured once by a unit test rather than per campaign, since it cannot vary. |

### DW0379/DW0380 — souls pacing lints (`compiler::nav`; **warning**; exit 0)

spec-0016 §7 is the design-contract section, and both of its rules are things the
compiler can **measure** but must not overrule — so both warn and neither ever
fails a build. Computed over the same assembled nav model as the completability
proofs, so their numbers are the numbers every other proof uses.

| Code | Meaning |
| --- | --- |
| `DW0379` | **Retry cost**: the proven walk from a rest point (`set-checkpoint` or `bonfire`) to the first beat it can respawn the party into exceeds **60 s** (1200 ticks at the 4 t/block sprint model `DW0355` uses). Dying must be an investment, not a commute — past the budget the loop stops teaching and starts taxing. Measured to the FIRST beat after the rest point, not the last. A warning because a long walk back can be the authored point (a pilgrimage, a set-piece approach). **Known limitation**: at 60 s the budget is ~300 blocks of walking, which no box-garden delve currently approaches, so the lint is effectively inert in practice. It is implemented to the spec's threshold rather than retuned: the number is a design decision, not a compiler one. |
| `DW0380` | **Optional-elite bypass**: an enemy no `kill` objective requires has no route around it — with its aggro radius (declared `follow_range`, else vanilla's 16) forced solid, a forced critical-path leg that routed before no longer does. Every way forward runs through the fight, so "optional" is a lie. The Tree Sentinel pattern — a powerful optional enemy near the start, fight it or walk around it — is explicitly legitimate; this is its one obligation. Two deliberate exclusions keep it about the ROUTE: a leg with an endpoint **inside** the sphere is contested ground by design (the landed "live threat" pattern, a wave seated on an objective anchor), and a leg that never routed in the clean world belongs to `DW0311`. |

### DW0351 — NPC location-continuity lint (`compiler::continuity`; **warning**; exit 0)

Tracks each NPC's **staged location history** through the campaign timeline —
the stage-2 anchor (or off-stage while `deferred`), every `move-npc`
destination, every `despawn-npc`/`spawn-npc` pair (a `spawn-npc` always places
at the NPC's declared anchor) — and warns when an NPC materializes or vanishes
at a location discontinuous with where it was last staged, with no movement in
between (owner, island QA round 6: `npc/perimedes` popped into an alcove
mid-story having never been staged entering; `npc/antiphos` was "grabbed at the
cave mouth" while his body vanished at the beach camp).

Three shapes warn:

* **re-entry jump** — `spawn-npc` re-materializes an NPC at its declared anchor
  after it was last staged elsewhere;
* **unstaged entrance** — a never-yet-staged deferred NPC materializes
  mid-story with no staged arrival. The accepted staging shape is firing the
  `spawn-npc` from a `move-actor`/`move-npc` `on_arrive` whose destination IS
  the NPC's anchor (walk a stand-in to the spot, swap the npc in on arrival);
* **remote dismissal** — `despawn-npc` fires from a beat whose scene anchor
  (the completing objective's anchor; a `talk-to`'s scene is its target NPC's
  staged spot) differs from where the NPC's body stands.

**Conservative model (no temporal reasoning).** Locations are symbolic anchor
names (same name = same place; no geometry). The timeline is the quest-DAG
linearization (stage-4 `depends_on` topo order; objectives in `after` order;
bundles in declared order, descending into `sequence` steps and `on_arrive`
lists in place). Anything whose firing time is statically unknowable makes the
NPC **untracked** instead of guessed at: a lifecycle effect fired from an
environment trigger, a dialogue option, an `on_respawn`/`on_caught` reaction
bundle, or carrying a `requires_flags`/`forbids_flags` gate excludes that NPC
from the lint entirely.

**Why warning, not error.** Whether a jump reads as broken is authorial taste —
narrative cover ("he slipped away while you slept") legitimizes any of these.
The message names the discontinuity concretely and prescribes the remedy
(stage a walk / spawn at the last staged location / accept with narrative
cover); the author decides.

| Code | Meaning |
|------|---------|
| `DW0351` | An NPC materializes (`spawn-npc`) or vanishes (`despawn-npc`) at a location discontinuous with its last staged location, with no movement in between. Advisory (exit 0): stage the move, re-anchor, or accept with narrative cover. |
| `DW0353` | A world-edits batch writes inside a `close-gate` region (v0.6, spec-0017). The gameplay seal fills that region solid and `open-gate` clears it to **air**, so one close/open cycle erases the edit — the dressing `delvec snapshot` shows is not what players see after the beat fires. Advisory (exit 0), `compiler::edit`, one finding per colliding gate region: dressing the *sealed* state is a legitimate authorial intent, so this reports rather than rejects. |

### DW02xx — analysis (`compiler::analyze` reachability + `compiler::light` lighting; error; exit 2)

`DW0210`/`DW0211` are emitted by the assembled-world light model
(`crate::light`), surfaced through the build path but mapped to exit 2 (analysis
tier) in `main`; `DW0201`–`DW0205` come from `compiler::analyze` over the
branch-coherent flow model (`compiler::flow`).

**The emitter table never overestimates (`crate::light::emission`).** Both gates
are only sound if the modelled light is a *lower* bound on the game's — a block
modelled brighter than vanilla lets a genuinely dark area ship unmitigated. The
table is evaluated over each block's **actual blockstate** (the assembled map
carries full states). Blocks absent from the table emit 0 (an underestimate, the
safe direction). A state-dependent block is never collapsed onto its brightest
state: `sea_pickle` is `3 + 3·pickles` when waterlogged and **0 when dry**;
`redstone_ore` is **0** idle and 9 when `lit`; `respawn_anchor` is **0** at
`charges=0`; `amethyst_cluster` is 5 (buds 4/2/1); `brewing_stand` and
`brown_mushroom` are **1**; `glow_lichen` is 7 where it is attached to a face and
**0** in its faceless default state (what a bare `minecraft:glow_lichen` places);
`glow_item_frame` is **0** (it is an entity, not a block, and emits no block light
in Java — 7 is a Bedrock value); the furnace family reports 13 when `lit`. Blocks
whose `lit`/`charges`/`berries` state has a *bright* default (campfire, soul
campfire, redstone torch) still evaluate bright from a bare id, so the compiler's
own relight fixtures are unaffected.

**The values are measured against the pinned game, not cited.** Every entry is
checked by `crates/compiler/tests/emission_table.rs` against
`crates/compiler/tests/fixtures/light/emission-1.21.11.tsv` — 1419 rows that
collapse all **29,671** blockstates of the pinned 1.21.11 server jar onto the
properties that can change their light, every value being what the game's own
`BlockState.getLightEmission()` returns. `tools/dump-block-light.py` regenerates
the fixture and refuses any jar whose sha256 is not the `versions.toml` pin;
`--check` re-derives it and diffs. Three assertions: `emission ≤ game` over every
blockstate (the contract); exact equality everywhere the table is not
deliberately taking a minimum; and the set of blocks measuring *below* the game
equals the declared set, so a future Minecraft's new emitter reds here rather
than silently costing a designer their fixture.

**Every light-emitting block of 1.21.11 is modelled**, including the ones a
designer reaches for first. Candles are **3 per candle and only while `lit`**,
which defaults to false — so a shipped candle is dark at any count and four lit
ones are 12; all seventeen candle ids (plain and dyed) and all seventeen
candle-cake ids (3 when lit) behave the same. Copper bulbs are **15 / 12 / 8 / 4**
by oxidation stage and only while `lit` (default false), waxed and unwaxed alike;
copper lanterns are **15 at every stage**, which is not the same rule; `copper_torch`
and `copper_wall_torch` are 14. `sculk_catalyst` is 6, `nether_portal` 11,
`firefly_bush` 2, and `dragon_egg`, `end_portal_frame`, `sculk_sensor` and
`calibrated_sculk_sensor` are 1.

**Where the world re-derives the property at load, the entry is the minimum over
the states the world can reach.** The model evaluates the blockstate the world
*ships* with; it does not simulate redstone, block entities, weathering or player
action. `redstone_lamp` has no `onPlace` and its `neighborChanged` schedules an
unlight the first time any neighbour updates while no signal is present — which
structure assembly does — so a shipped `lit=true` lamp is not a stable
configuration and the entry is **0**. `trial_spawner` and `vault` have their state
owned by a block entity, giving **0** and **6** respectively. `copper_bulb` is not
in that set: its `onPlace` runs `checkAndFlip`, which returns without touching
`lit` whenever the neighbour signal already agrees with `powered`, so a bulb
shipped lit in a room with no redstone stays lit.

**The opacity table is coupled to nav passability
(`crate::light::passes_light`).** The opacity side defaults the other way — an
unlisted block is **opaque**, which under-measures light and is the safe direction
for a block a walker cannot enter anyway. It is *not* safe for a block
`assembled::occupancy_of` deliberately leaves **passable**: then a cell the player
really stands in is measured at light 0 while the game lights it normally, and the
gate manufactures a `DW0210` no amount of relighting can clear. The invariant is
therefore:

> every block class whose cell `occupancy_of` leaves player-occupiable must be
> light-passing in `passes_light`.

Three classes are player-occupiable by construction — trap triggers
(`is_passable_trap_trigger`: `*_pressure_plate`, `tripwire`, `tripwire_hook`, kept
walkable *on purpose* so `DW0342` can reason about a player stepping onto a
critical-path trap), thin decoration (`is_thin_decoration`: every `*_carpet`, and
`snow` at 1–4 layers), and fence gates (`*_fence_gate`: open = a passable
threshold, closed = passable-with-use). All of them are `filterLight = 0` in
vanilla 1.21.11 and all of them now pass light; before this fix only
`oak_fence_gate` did, so any roofed prefab carrying a plate, a tripwire, a carpet
or a non-oak gate failed `DW0210` on cells that are in fact lit. Verified against
the pinned `minecraft-data` block table (`.../pc/1.21.9/blocks.json`):
`filterLight = 0` for all 16 pressure plates, `tripwire`, `tripwire_hook`, all 20
carpets, `snow`, and all 12 fence gates; `filterLight = 15` for the control cubes
`stone`/`dirt`/`oak_planks`/`cobblestone`/`deepslate`/`sand`/`gravel`/`obsidian`
and for `snow_block` (a full cube — deliberately still opaque). Blocks vanilla also
calls transparent but that `occupancy_of` classifies **solid or tall** (fences,
walls, buttons, levers, rails, slabs, stairs, doors, trapdoors, chests, signs)
stay opaque here: their cells are never player-occupiable, so their opacity can
only make the gate stricter, never manufacture a false pass. The invariant is
CI-pinned by `light::tests::every_nav_passable_block_passes_light`, which drives
the real classifier — a future passability change that forgets the light table
fails there rather than in a campaign.

| Code | Meaning |
|------|---------|
| `DW0201` | Finale quest can never complete (unreachable finale). |
| `DW0202` | Quest can never be triggered (dead quest — its trigger source never completes). |
| `DW0203` | Objective can never be completed **in any branch** (deadlock: unsatisfiable `after` chain, an unproducible `requires_flags` gate, or a `talk-to` completing option unreachable through the trigger/`after`/dialogue graph). |
| `DW0358` | A declared `min_players: n` (n ≥ 2) has **no n-agent division of labour** (v0.6, spec-0018). Completability is proven with `min_players` agents: n = 1 is the unchanged single-agent proof, and n ≥ 2 additionally requires the proven playthrough to contain an AND-join with n arms that are *independently reachable at the join's frontier* — the replay state just before its earliest arm — with no arm waiting on a sibling, a flag a sibling sets, or a quest that is not active yet (`flow::Flow::divide`). Names the widest join and how many arms it actually offers, or says the campaign has no AND-join at all. Reported on `world`/`/content/min_players`, exit 2. Prescription: split one beat into n `after`-arms completable from the same frontier, or lower `min_players`. |
| `DW0204` | The exported critical path is not a playthrough any player can walk: some step is not activatable/completable at its position, or `campaign-complete` fires before the final step (the signature of two mutually exclusive endings sharing one path). Names the first incoherent step. |
| `DW0205` | **Optional participation gates the mainline**: the dialogue button that completes a mainline objective is already on screen at an earlier point of the participation-minimal walk, before that objective's own activation chain has happened — so a player can take it and walk past a load-bearing beat. Names the objective, the beat, the dependency edge (`after`, or the flag the beat is what sets), and what the skip costs the mainline (the wave the beat spawns, the flag it sets, the quest that then never opens). Reported per branch too (`branch::check_branches`), naming the branch, for skips the campaign's own critical path does not already admit. Prescription: put the beat's flag on the option (`requires_flags`), or move the option into a `cast` scene that opens only after the beat. |
| `DW0210` | **Measured** (spec-0010): a reachable walkable cell of an area is below light 3, under the darkest reachable (time, weather) sky, with no `lighting` declaration and no `mitigation` declaration. Judged over the assembled world (per-seam, sealed-cavity aware — unreachable cavities are never counted). Admission `LightingProfile` is no longer a gating input. Keys on the stage-1 `areas[].mitigation` declaration, so a renamed water bottle in a class kit does not pass the gate. **One diagnostic for the whole build**, because its remedy is re-arranging a room and a designer cannot re-arrange a room they were not told about: it names every dark area (worst first), each with its dark-cell count out of the cells measured in it and its dark cells grouped into contiguous regions — adjacency is `nav::World::neighbors`, symmetrised, so a region is a run a body can walk without leaving the dark — each region with its extent and its own darkest cell, plus the campaign's single darkest cell. Both lists are capped and state how many entries and cells they dropped. The prescription that closes the report names the document THIS campaign declares lighting in — `world.areas[].lighting` where prefabs place the world, the site plan's own `lighting` where the blockout is derived — and it offers `mitigation` only where that surface exists. It does not on a site-plan campaign: `mitigation` lives on an `areas[]` entry and `DW0839` requires that list to be empty, so a derived map is offered two ways rather than three, and the count in the message is computed from the list of ways rather than written beside it. **A derived map therefore has no night-vision mitigation at all** — a capability gap, recorded here rather than papered over: such a campaign lights its blockout or brightens the scene. **The report is split by WHOSE cells are dark**, because that is what decides the remedy. Cells the derivation wrote get the paragraph above, unchanged. Cells inside a frame a `detail-plan` row bound get their own: the fixture pass never enters a bound frame (spec-0050 §3, a bound place lights itself), so a plan-level `lighting` reaches none of them and prescribing it would prescribe an act that changes nothing. That paragraph names each dark place with the piece bound to it, and states the darkest cell twice — in world coordinates, where the build measured it, and in the piece's own, where the author edits it — and the remedy it offers is emitters in the piece, a different piece, leaving the place undetailed, or brightening what stands OUTSIDE the frame, since light does cross a frame boundary. One code and one diagnostic still: the fact is the same measured darkness and the same refusal, and what moved is which document the author is sent to. A build whose dark set holds no bound place prints what it printed before, byte for byte. |
| `DW0211` | An area's declared relight `fixture` cannot raise every reachable walkable cell to `min_light` — no valid placement site remains (spec-0010). "Fix in" names the same placement-authority field `DW0210` does. |

**The branch-coherent flow model (`compiler::flow`).** Reachability is not one
union fixpoint over "every `set-flag` anywhere". A **choice group** is a dialogue
node with ≥2 options that each set a flag — taking one means not taking its
siblings, so the options are XOR alternatives. A **world** picks one alternative
per group (the product over the *flag-reading* groups only, capped at 512;
groups past the cap stay unconstrained, i.e. exactly the pre-model behavior).
The fixpoint runs **per world**, and a quest/objective is reported unreachable
only when it is unreachable in **every** world — so the branch model makes
`DW0202`/`DW0203` strictly more precise, never looser.

A flag producer is conditional on its gating context:

| Producer | Available when |
|----------|----------------|
| `set-flag` in `on_objective_complete[o]` | `o` is completable **and** every `requires_flags` gate on the enclosing effect chain is satisfied |
| `set-flag` in a quest's `on_complete` | that quest completes, same gate rule |
| `set-flag` on a dialogue option | the option is reachable from **one of the roots the campaign can put a body in front of** (below) through options whose own gates are satisfied, and is the world's selected alternative of its group |
| `set-flag` in an environment trigger's `effects` | the trigger's `requires_flags` are satisfied — **ambient** (a `strike`/`use`/`approach` trigger is player-initiated and has no DAG position) |
| `set-flag` in a `traps[].payload` | the trap's `requires_flags` are satisfied (ambient, same reasoning — the party can always walk over and spring it) |
| a trap's `disarm.sets_flag` | the trap's `requires_flags` are satisfied (ambient, same reasoning) |
| `set-flag` in an `on_respawn` / `on_caught` reaction bundle | **never** — reaction bundles fire at statically unknowable times, so nothing inside one is a producer (the conservative stance `compiler::continuity` already takes) — whether the bundle is rooted in the quests stage or hung off a **dialogue option's** `set-checkpoint` |

Consequences worth stating plainly: a `set-flag` gated on the very flag it sets
(the "re-affirm the branch" idiom) produces nothing; a flag produced only on the
`flag/flee` branch cannot satisfy a gate on the `flag/wait` branch; and flags set
from dialogue, triggers, trap payloads and trap disarms are first-class
producers, so those legitimate shapes no longer die as spurious `DW0203`.

**A dialogue tree has more than one door, and the model walks from all of them.**
The roots a reachability walk is seeded from — `Flow::entry_roots` — are the
tree's declared stage-6 `root` **plus every node a quest's `cast` ledger names as
that NPC's scene**, and a ledger root counts once its quest is active and its
placement's `requires_flags` hold. A ledger root is not a shortcut into the tree:
right-click opens it directly for that quest's duration, which is what the ledger
is for (spec-0020) — an NPC's right-click being a different scene per quest. So a
node no `next` link reaches is reachable when the ledger opens it, its options'
flags are producible, and a branch that forks there is not `DW0482`. The
quantifier is **per world**: a per-branch cast clause carries the branch's flag,
so its root opens the node in the worlds holding that flag and nowhere else.
`forbids_flags` is ignored here for the same reason the option walk ignores it —
the model is monotone, and a negative gate that closes later cannot un-reach a
node the party has already stood in. `"unchanged"` needs no resolution: it
carries forward a root some earlier quest already declared, which is already in
the union. `Flow::scene_root` asks a narrower question — which ONE root a
right-click opens at a given instant, where later declaration wins — because it
models the emitted `dw.cast` dispatch; reachability is the union over the whole
playthrough. The DSL half has always asked the wider question:
`NpcDialogue::reachable_from` is the one authority for "what can this tree show,
entered here", and `DW0120`'s orphan walk and `DW0858` both seed it from a root
SET.

**Which effect lists those rows range over is not `flow`'s to decide.** Both
halves of the model — the producer scan in `Flow::new` and the
gate-flag inventory `flow::gate_flags`, which is what decides whether a choice
group is enumerated as XOR worlds or left unconstrained — walk
`plan::for_each_effect_root`, so the proof cannot believe in fewer firings than
the datapack performs. A hand-rolled walk enumerating three of the five is the
defect the single enumeration exists to prevent: a `set-flag` in a
`traps[].payload` is then a producer **nowhere** in the proof while the emitted
`trap_fire_<trap>.mcfunction` really sets it (an objective gated on it dies as a
spurious `DW0203`), and a `requires_flags` *inside* such a payload is not counted
as a flag read at all — so a branch choice that only such a gate reads never
splits the worlds, and one world holds two mutually exclusive branch flags at
once. The table above is a **policy per root**; the roots themselves are
inherited, and the match on them is exhaustive, so a sixth root cannot be added
without `flow` deciding what it means. The two new roots needed no new ruling:
the payload takes the ambient stance the environment trigger and the trap
`disarm` beside it already had, and the dialogue-hosted `on_respawn` bundle is
reached but never credited, which is the reaction-bundle rule the identical
quests-stage bundle already obeyed.

**The exported critical path is one branch (`DW0204`).** `compiler::plan` does
not walk the finale's whole stage-4 `depends_on` closure. It walks the
**playthrough** the flow model proves: the first world (deterministic
enumeration order, all-first-alternative first) whose finale quest completes,
restricted to the quests that complete in it, with each `talk-to` taking the
completing dialogue option that belongs to that branch. Before export, the
sequence is **replayed** step by step through the flag/objective/quest state
machine: every step's quest must be active, its `after` prerequisites completed,
its `requires_flags` set and its `forbids_flags` unset *at that position*, its
completing dialogue option reachable *at that position*, and `campaign-complete`
must fire exactly at the final step. The first violation is `DW0204`, naming the
step. `compiler::plan`'s gate-aware reachability (`DW0306`) judges the same
sequence, so the static proofs and the exported bot contract agree by
construction. When no world completes the finale the campaign is already
`DW0201`; the model then degenerates to the whole closure so the geometry-only
commands (`chart`, `snapshot`) still run on an unanalyzable campaign.

**Optional participation can never gate the mainline (`DW0205`).** The owner's
contract is that *the mainline must be completable with zero optional
participation*. Optionality is not a DSL declaration — it is **derived**: the
**mainline** is exactly the critical path above, the participation the campaign
requires to reach `campaign-complete`; every other act a player may take (a side
objective, a non-path dialogue option, an elective trigger/trap/wave) is
optional. The contract is proven in two halves on that one path.

*The producer half is `DW0204`.* The replay is already the participation-minimal
walk: it credits only the mainline's own producers — the taken option's flags,
on-path completion bundles, and the ambient trigger/trap flags any player can
fire — so a mainline objective gated on a flag only an off-path quest or an
unselected option sets fails the replay. Nothing further is needed there.

*The order half is `DW0205`* (`flow::Flow::skips`). Every objective driver the
compiler emits goes through `pending_guard` — quest active ∧ `after` complete ∧
`requires_flags` ∧ `forbids_flags` — **except** the dialogue button, whose
`complete-objective` is gated only on its quest being active and the objective
not yet complete. So the same walk asks, at each state, which mainline `talk-to`
buttons the campaign already has on screen: the NPC's live `cast` scene must open
a tree (barks/silence offer nothing), the option's node must be reachable from
that scene root through options whose gates hold, and its own gates must hold.
A button on screen for a step further down the path is a **skip**; a skip whose
skipped beats carry a dependency edge into that objective (an `after` edge, or a
flag the beat is what sets) is the error. Because the walk is the same
`advance`/`fire` state machine as the replay, event-driven activation —
quest-complete chains, NPC arrivals through `on_arrive`, staged `sequence` steps
— is walked under the skip rather than assumed. The island's owner-hit softlock
is the canonical instance: `"Lead on."` (completing `obj/climb-out`, `after
obj/surf`) sat beside `"We climb."` (completing `obj/muster`, whose bundle spawns
the drowned) from campaign start, so a player could climb before the surf beat
existed, `quest/shipwrecked` never completed, and one of three crewmen reached
the cave.

*The remedy is a path gate, never a button gate.* `DW0191` requires every
`talk-to` to keep an **ungated** completing option, so that it cannot deadlock
the moment it activates; `DW0205` requires that option not to be on screen too
early. The two meet at the way IN: `requires_flags` on the option that navigates
to the completing node, or a `cast` scene that opens that tree only after the
beat. The completing option stays ungated and is simply unreachable until its
turn.

**`forbids_flags` and producibility (v0.6, conservative).** The reachability
fixpoint models `requires_flags` producibility (a gating flag must be producible
by an already-completable producer on the same branch) but deliberately
**ignores** `forbids_flags`: whether a forbidden flag is set when an element is
needed depends on play order — full temporal reasoning the existence fixpoint
does not attempt. An element with a negative gate is therefore treated as
fireable, so `forbids_flags` can never cause a spurious `DW0202`/`DW0203`. The
**compensating stronger check** is the `DW0204` path replay, which does have a
concrete order and enforces every negative gate at its real position on the
exported path. The other static guarantees that hold: every `forbids_flags`
reference resolves to a produced flag (`DW0172`), and a completing dialogue
option gated only by `forbids_flags` still counts as gated for `DW0191`.

### DW0879 — a numeric gate the path has already cleared (`compiler::statepath` over `compiler::flow`; error; exit 2)

| Code | Meaning |
|------|---------|
| `DW0879` | **A forced-path numeric gate the path itself has made unsatisfiable.** An objective's `requires_state` term evaluated at that objective's own position on a walked path, against the value every state write the path performed has left the datum holding. Analysis tier (exit 2), `every_version` — it judges what the document says, and a campaign that declares no `state[]` reads no gate this rule can see. Names the objective, the datum, the comparison, the value held, the beat whose write left it there, and the two remedies: move the write past the beat that reads the datum, or move the gate. |

**The quantifier is the whole rule.** `DW0501` asks whether a datum is written
anywhere, `DW0502` whether it is read anywhere, `DW0847` whether the gate's own
terms are jointly satisfiable, and `DW0527` whether a comparison sits after a
write **in the same bundle's effect list**. None of them asks what the datum
holds at the moment a *later* beat reads it, and that question needs an order —
which the monotone fixpoint does not have and the path replay does. So this is
the replay's binding widened from flags to the whole gate, not a mechanism
beside it: `ReplayState` carries every declared datum's value, `Flow::fire`
honours a write's own `requires_state` where it stands (which is where vanilla
evaluates it) and applies the write, and `Flow::state_gates` reads each
objective's gate at its position.

**What is walked**: the exported critical path (`Flow::playthrough` — the
participation-minimal order `DW0204` already proves is a playthrough), and every
enumerated branch world's own whole path (`Flow::playthrough_in`, a `dag_order`
over everything that completes in that world), which reaches optional strands
the finale-rooted path never visits and branches it is not on. A finding already
named on the critical path is not named again; a branch finding names the branch
by the flags that distinguish it.

**Two refusals to over-claim, both counted rather than silent.** A term is
refused only where the failure is one **no play order avoids**: the emitter's
`pending_guard` lets a player complete any activatable objective at any moment,
so two beats with no `after` between them can be played either way round, and a
gate that fails under one of those orders and holds under the other is a path
this walk picked rather than a defect. The writes the walk applied must
therefore be chained into one order by the campaign's own `after` and
`quest-complete` relations, ending before the gate. And a **datum no ordered
walk can date** is never refused at all: an ambient producer (an environment
trigger, a trap payload, a shortcut's `on_unlock`, a shop offer) may be fired any
number of times at any moment, a reaction bundle (`on_death`, `on_respawn`,
`on_rest`, `on_caught`) fires at a moment nothing names, a `stakes[]` forfeit
moves the purse on a death, and a `player`-scoped datum under `min_players >= 2`
depends on which agent acts. A flag is monotone and so is credited
unconditionally in all four cases; a number is not. Undatable **absorbs**,
`set-state` and `clear-state` included — a write pins a value only until the next
undated write, which can land the beat after.

**Ordering.** It withholds itself entirely where `DW0201` (no branch completes
the finale) or `DW0204` (the exported path is not walkable) already names a
cause, and says which, rather than adding a second refusal about one break.

**Binding**: paths walked, steps, numeric gate terms read, state writes
replayed, terms withheld, declared data and how many of them are undatable —
stated on every validate, analyze and build of a campaign, including the zeroes,
because a walk that read no gate and a walk that read twenty and found nothing
look identical from outside otherwise.

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

Exit 3 except `DW0312` (wave-capacity), `DW0313` (gravity-despawn) and `DW0342`
(lethal-trap completability), which are analysis-tier and mapped to exit 2 in
`main` like the `DW02xx` codes — see their rows.

| Code | Meaning |
|------|---------|
| `DW0300` | Generic build/resolution failure (missing prefab metadata/`.nbt`, unresolved anchor, critical-path dependency cycle). |
| `DW0301` | Bound pool declares no `entry` piece (or no `connector` filler when needed). |
| `DW0302` | A campaign-referenced anchor is provided by no pool member. |
| `DW0303` | `pieces{min,max}` too small to fit entry + required anchor-bearing pieces. |
| `DW0304` | Solver could not place a required piece without overlap (after retry), or a branching layout's pool declares no branch piece (tee/cross). |
| `DW0305` | A campaign-referenced anchor is defined by >1 placed piece (ambiguous); or a required anchor's only carrier is the `entry` piece. Fires per anchor, at the **use** site, and only over the anchors the solver is required to guarantee. The pool that made them ambiguous is named once at its declaration by the advisory `DW0498` — which is emitted **alongside** this error, not instead of it, so the failure arrives with its cause. |
| `DW0306` | Gate-aware reachability deadlock (an anchor reachable only through a gate no earlier objective opens). Modelled by **splitting the carrying piece into two halves** along the gate plane, joined only by the gate cut-edge — so an in-piece bypass *around* the gate is invisible to it and a far-side objective always reads as a deadlock. **Shortcut gates are exempt by construction, not by a special case**: the heuristic's gate set comes from `open-gate` effect anchors only (`collect_open_gate_anchors`), and a stage-5 `shortcut` gate has no `open-gate` effect, so it never enters the piece-split graph. Its deadlock obligation is discharged by a strictly stronger proof instead — `Plan::build` seals every shortcut gate at step 0, so the cell-level `DW0311` critical-path proof must find the long route over real geometry (delete the long route and the build fails at `DW0311`, not `DW0306`). This is why a souls loop-back — whose `unlock` sits on the far side by definition — must be declared as a `shortcut` and not as a plain `open-gate` reward on the far-side objective; the latter is genuinely self-deadlocking and is still rejected. Pinned by `souls_shortcut::{a_shortcut_owned_gate_is_not_a_dw0306_deadlock, the_same_gate_as_a_plain_open_gate_is_still_dw0306, a_shortcut_with_no_long_route_is_rejected_by_the_critical_path_proof}` — the same geometry, green as a shortcut and red as a plain gate. |
| `DW0307` | `move-npc` destination unreachable by A* over the solved voxel grid. |
| `DW0308` | `cutscene` camera dolly clips a solid block (checked per shot; the message names the shot and segment). Checked over **both** the authored waypoint polyline and the client-rendered keyframe chord path (`compiler::camera` — the client tweens straight between emitted keyframes, so a chord can cut up to 0.25 blocks inside an authored corner; the chord message names the keyframe pair). |
| `DW0309` | A staged **body** — a stage-2 npc or a stage-5 actor alike — declares `skin.texture_id` but the campaign ships no `skins/<id>.png` to bake. The message names the declaring body, its stage and its JSON pointer. Enumerated from `dsl::body_skin_sites`, a filter over `dsl::body_sites` (`BodyRef`'s closed set), so both classes are baked and both refused by the same rule; a skin declared by a class outside that set is red in `crates/dsl/tests/body_skin_sites.rs`, which takes the population from the schema export. One texture is read once however many bodies name it. |
| `DW0310` | `spawn-wave` references a wave whose spawn anchor resolves in no assembled area (dangling spawn). |
| `DW0311` | Critical path has a leg — a consecutive pair of visited positions, the first pair beginning at the campaign's entry point — with no walkable A* connection and no inter-area transport (player stranded). **It quantifies over walks only.** A leg whose ends are in different areas is a crossing and is refused before this proof, by `DW0872` or `DW0873`, so the doorway/gap/fence prescription below is never offered about a route nobody was going to walk. Routed over the collision-classified occupancy, so a required anchor sealed behind an unbroken 1.5-tall fence/wall ring with no fence-gate opening fails here — the full-solid model wrongly proved such pens by standing the player on a fence-top. Each leg is routed under the `close-gate` seal state from `plan::collect_gate_events`, which walks **all five** effect roots emission fills a gate from (§4 "The seal answers"), so a seal fired from a `traps[].payload` or a dialogue-hosted `on_respawn` bundle is sealed in this proof exactly as the datapack fills it. |
| `DW0312` | A `spawn-wave` needs more standable spawn cells near its anchor than the anchor's own room provides. **Analysis-tier: exit 2**, like `DW02xx` — a content-design capacity mistake (shrink the wave or use a larger room), not a geometry defect; the message names the wave, area, and needed-vs-found count. |
| `DW0313` | A placed gravity block (`sand`/`gravel`/`concrete_powder`/anvil/`dragon_egg`) despawns into the void at placement — an unsupported gravity floor over the `the_void` world falls out on the first block update, holing the shipped map even off the critical path. The authoritative gravity-settle gate (`crate::assembled`), not a downstream DW0311/DW0312 side effect. **Analysis-tier: exit 2** — a prefab/generator defect; the message attributes despawned cells+counts per piece and prescribes a non-falling substrate. Blocks that fall but **land on support** are faithfully modelled by the settle pass (no diagnostic): the shipped geometry is exact for every consumer, and the generator's own zero-unsupported invariant catches an *unintended* fall at authoring. Anti-dodge: swapping the floor palette to non-falling blocks to silence this is explicitly rejected — gravity floors are a first-class content need; add the substrate. |
| `DW0314` | An exported critical-path waypoint is not standable in the FINAL assembled world (settled + water-flooded + relight fixtures) **as that leg's own runtime region writes leave it** — the build-time self-check that makes the water-flow / post-nav-mutation divergence class structurally impossible to ship. The leg carries the world it was proven over (`LegRoute::proven_world`), which is built by the one region-state construction, so floor a campaign lays at runtime counts here exactly as it counts for the route — **forcedness included**: footing from a root the party can skip is impassable-and-not-floor in both, and is `DW0546` before export. Routes come from A* over that same world, so this fires only if a later pass mutates a cell nav relied on or an endpoint resolves off the walkable set. The message names the offending cell and leg. Fix the prefab/water or the assembly — never nudge the waypoint. |
| `DW0315` | A `set-checkpoint` (spec-0012) strands the party: re-rooting the DW0311 reachability at the checkpoint cell, the first remaining required critical-path anchor is no longer walkable from it (a checkpoint behind a one-way drop the forward path can't re-cross after respawn). The message names the checkpoint and the first unreachable anchor and prescribes moving the checkpoint or adding a return route — never deleting the checkpoint to silence the proof. |
| `DW0316` | A `set-checkpoint` anchor has no standable footing within snap range on the final assembled model (a trap-trigger / hazard / mid-air cell) — the party would respawn into void or a wall (spec-0012). Because the relight pass already proves every reachable walkable cell meets the area's `min_light`, a checkpoint that clears this and DW0315 provably meets `min_light` too. |
| `DW0317` | **A gate the campaign never opens.** A forced critical-path leg has no collision-free path once the gates the placed prefabs author SHUT at world-load are solid, but routes fine without them — or a visited objective's only footing is inside such a gate's region. Build-tier (exit 3), `compiler::nav`. The message names the gate anchor, its area, its region, how many of its cells the world fills, and what the campaign does to it: nothing forced opens it, or it is opened only at a later / non-ancestor step. Derived by counterfactual, exactly like `DW0510`. Prescription: fire `open-gate` from an objective the party is FORCED to complete before the leg, or route the forced path off the gate — never delete the gate, and never strip the anchor's fill block out of the prefab. `every_version`: it asks for no new surface (`open-gate` is v0.4) and states a contradiction between what the campaign places and what it requires; fencing it would leave it vacuous on every live campaign, all of which declare below the current version. |
| `DW0378` | A `timed-gate` (spec-0016 §4) is a **coin flip, not a timing read**: the entry phases from which a walking player clears the span before it shuts cover less than **20%** of the cycle. All-phase passability is explicitly NOT the requirement — punishing bad timing is the point; punishing *every* timing is a slot machine no amount of learning the level makes fair. The crossing cost is the A* step count between the footings either side of the region with the gate open, charged at the same 4 t/block sprint model `DW0355` uses; the admitting window is `max(0, open_ticks − cross + 1)` of `open_ticks + closed_ticks`, computed in integers (no float rounding in a proof, ADR-0006) and rounded DOWN. `compiler::nav::check_timed_gates`, build-tier (exit 3). Prescription: lengthen `open_ticks`, shorten `closed_ticks`, or narrow the span — never lower the floor. The runtime counterpart is the waypoint artifact's `timed_gates` table + per-leg crossing marks (see above): the harness bot waits out the window instead of failing a leg the gate shut on. The window proof has a companion the dossier rates higher: `DW0388`, which proves the window can be **read** — that there is safe ground with a sightline to the span. |
| `DW0376` | An `ambush` (spec-0016 §3) with **no counterplay**: standing every ambusher on the cell it will occupy, no checkpoint, bonfire or campaign entry is walkable from the trigger cell any more — the party is sealed in a pocket with the ambush and can only trade blows blind. The `DW0342` trap-avoidability machinery generalized from one hazard cell to an occupied cell set. This is NOT a telegraph requirement: 初见杀 is legitimate and determinism guarantees the second attempt meets the same ambushers in the same cells; what this proves is that the second attempt has a *play* — a retreat, luring ground, an exit. `compiler::nav::check_ambushes`, build-tier (exit 3). |
| `DW0373` | A `shortcut` (spec-0016 §2) has **no long route**: with its gate sealed, the far-side `unlock` affordance is not walkable from the campaign entry, so the mechanism that opens the shortcut sits behind the shortcut and can never be pulled. `compiler::nav::check_shortcuts`, build-tier (exit 3). Prescription: connect the far side by a long route, or move the unlock onto one — never open the gate at world-load to silence it. |
| `DW0374` | A `shortcut` (spec-0016 §2) **leaks**: opening its gate does not strictly shorten the A* walk from the campaign entry to its own `unlock`, so the unlock is not on the far side of anything and the loop-back the shortcut exists for never happens. The classic form is an `unlock` placed on the NEAR side of its own gate — this is the proof that makes `unlock` a far-side anchor rather than a label. Both distances are measured over the same nav model, differing only in the gate. `compiler::nav::check_shortcuts`, build-tier (exit 3). |
| `DW0420` | A compiler-owned **interact affordance has no visible hardware** (the drowned-bell soft-lock). `minecraft:interaction` is an invisible hitbox, so an affordance built from one alone asks the player to right-click a point nothing marks. Reproduced live on 1.21.11: a `shortcut` unlock cell was bare air holding exactly one invisible entity, and the only thing visible there belonged to an unrelated `reach-anchor` objective which killed its own marker on completion — so the "lever" vanished at the moment of arrival and the delve soft-locked with the gate still sealed. The compiler now owns every affordance's visibility outright rather than leaving it to whether the tileset happened to dress the cell (CLAUDE.md no-hacks: no downstream folklore at a layer boundary). Emission self-check over the finished datapack, `compiler::affordance`, build-tier (exit 3). |
| `DW0421` | An affordance's **visible hardware is destroyed by machinery that does not own it**. Hardware may be retired by exactly one thing — the affordance's own consumption (`shortcut_open_<id>`, `trap_disarm_<id>`); a bonfire's is permanent and may be retired by nothing. Anything else reaching the `dw_hw_<tag>` (a cleanup pass whose selector widened, a `DW0361`-class name collision) leaves a live affordance invisible again — the same soft-lock by a different route. Tag matching is exact, not prefix, so `dw_hw_a` never matches a kill aimed at `dw_hw_ab`. Emission self-check over the finished datapack, `compiler::affordance`, build-tier (exit 3). |
| `DW0422` | A **pressable body's hitbox is contested** by another compiler-owned interaction affordance (v0.8; widened to the whole pressable class in v0.11). Every compiler-owned press hitbox set is in scope — a `close-gate` seal's shell cells and a sealed `shortcut` door's sealed-side approach cells — because a ray-pick contest is a property of *having hitboxes*, not of the verb that first had them. It bound to seals alone until v0.11, so on the `souls-shortcut` fixture it examined **zero** objects: green, and meaning nothing. The overlap is tested against the **cell**, not against the emitted `1.02f` box: the protrusion exists to beat the BLOCK the body stands in, and `SEAL_MARGIN`'s own contract is that *a hundredth of a block never reaches into a neighbouring cell's own affordances* — two boxes sharing a 1 cm sliver are at different ray distances and there is no tie. (Widening the binding without that correction produced a false `DW0422` on the fixture immediately: `npc/keeper`'s dialogue hitbox is edge-adjacent to a door cell.) Any other affordance whose own 1.0 × 2.0 box overlaps one of those cells is in an exact ray-pick contest with it, and the client resolves an exact tie by iteration order — one of the two silently stops receiving clicks and which one is not decidable from the campaign. This is the defect that made the island's boulder hint unshippable for three rounds (`DESIGN.md` §7 item 4: a co-located second hitbox meant either the existing left-click hint or the new right-click hint died, and the compiler built green either way). Pure box arithmetic over resolved cells, `compiler::eclipse::check_seal_collisions`, build-tier (exit 3), run beside `DW0359`. **Not a collision:** a click trigger anchored on the gate anchor **itself** — it rides the seal's own hitboxes and `env_trigger_setup` summons nothing for it, the same merge `strike`-on-an-NPC's-anchor has used since round 6. An affordance contesting another AFFORDANCE, with no pressable body on either side, is outside this rule and is `DW0878`. Prescription: move the affordance out of the sealed region, or — when the thing being clicked really is the gate — anchor the trigger on the gate anchor so it rides the seal. |
| `DW0423` | Two `close-gate` effects seal the **same** gate anchor with different `sealed_hint` wordings (v0.8). A seal's answer belongs to the PLACE: one anchor carries one set of `dw_seal_<anchor>` hitboxes and one reward function, so a second wording has nowhere to live and would be silently dropped — a line an author wrote and a player can never read, which is the same silence class the verb exists to close. A firing that authors no hint is compatible with anything (it asks for the compiler's canonical English); only two *authored, different* lines conflict. `compiler::gates::check_seal_hints`, validation tier (exit 1). Prescription: give both firings the same line, or seal two different gate anchors. **Since v0.11 this polices the SUGAR, not a mechanism**: `sealed_hint` is the wording of the press answer synthesized for that anchor, and one anchor still gets one answer. It has no shortcut analogue and needs none — a `shortcut` carries no wording field, so there is nothing for two declarations to disagree about. |
| `DW0425` | **The compiler cannot tell which side of a `shortcut`'s gate is the sealed one**. A shortcut door's clickable body is placed in the open air on the *sealed* side only, and that placement IS the side test — so the side has to be derivable or nothing may be placed. It is derived from the gate slab's thin axis plus which side of it the `unlock` cell lies on, and it fails when the region has no unique thinnest axis (a cube is not a doorway) or the `unlock` is level with the doorway on that axis rather than beyond it. **It binds to every `shortcut` in the campaign**, not to the ones that declared something: every door gets a clickable body — a door with no answer is still a door a player walks up to and pushes — and every body has to stand on a side, so there is nothing to opt into and nothing to forget. (Before v0.11 the message claimed the shortcut "declares an `on_wrong_side` answer", naming a field that has never existed in any schema; a diagnostic that names a phantom field sends the author to write something the schema rejects.) Withhold, never invent: bodies placed on a guess put the author's "this will not open" answer exactly where the door DOES open, and a false player-facing statement is worse than silence — silence teaches nothing, a lie teaches something wrong. `compiler::wrongside::derive` + `emit::check_shortcut_sides`, build-tier (exit 3), raised **before** the route proofs so an undecidable doorway is not reported under `DW0374`'s name. Prescription: put the `unlock` clear of the gate's span on the axis the door is thin on — which is where a far-side bar belongs anyway — or use a gate anchor whose region is a doorway slab rather than a volume. |
| `DW0426` | **A click trigger is anchored where a player can never click it**. The unbound-vacuity class as a check, and the rule that would have caught the gap it came from: the trigger declares an anchor, a click and a full effect bundle, validation passes, emission runs, and the press lands on nothing — so the beat never happens and every board stays green. Fires when a `strike`/`use` trigger's `at` resolves to no placed piece, so there is no cell to give it a body at. (`strike-npc` carries no anchor and rides its NPC's own hitbox; `approach` is a radius test with no entity — neither is in scope.) `compiler::pressable::body_at` + `emit::check_trigger_bodies`, build-tier (exit 3). It walks `Plan::emitted_triggers_unlocalized` — the campaign's own triggers **and** the press answers the compiler synthesizes — so a compiler-owned press is proven to land exactly as an authored one is. The bodies it resolved are published as `validation/press-bodies.json` (`examined`, `unbound`, `reason`, and per press the trigger, the click, the anchor and WHICH body it landed on — riding a seal, arming a region shell, or a point in open air): an error-tier proof that ships is equally silent on a campaign with no click triggers at all, and only the count separates the two. Prescription: anchor it on a place a prefab provides — anchor names come from prefab metadata, never invented — or drop the trigger. |
| `DW0427` | **A press answer addressed to a click vanilla cannot attribute** (v0.11). A trigger declares `audience: presser` on something other than an `on: use`. `minecraft:player_interacted_with_entity` is the only vanilla criterion that runs a function as the player who clicked, and it fires on right-clicks alone; a left-click is recorded in the interaction entity's `attack` NBT as a UUID no command can become, and an `approach` has no click at all. Approximating it — polling the record and assuming the nearest player is the striker — is exactly the downstream folklore CLAUDE.md's no-hack rule excludes, so the capability is refused rather than faked. `dsl::validate::press_answer_checks`, validation tier (exit 1). Prescription: make it an `on: use` trigger, or drop `audience` and let the beat address the party. |
| `DW0428` | **An authored trigger id in the compiler's reserved `dw-` namespace** (v0.11). The compiler synthesizes triggers of its own — today the press answer every sealed gate and shortcut door gives (`trigger/dw-press-seal-<anchor>`, `trigger/dw-press-door-<shortcut>`) — and two triggers sharing an id would share one `dw_trig_…` tag and one emitted function, so one of them would silently disappear. Reserving the prefix makes the collision impossible by construction rather than improbable. `dsl::validate::press_answer_checks`, validation tier (exit 1). Prescription: rename it; any kebab id not opening with `dw-` is the campaign's. |
| `DW0429` | **A sealed body the campaign never answers** (v0.11). A `shortcuts[]` door bars a gate from world-load, or a `close-gate` seals a wall, and nothing says what it answers when the party presses it — no `use` trigger anchored on it, and for a `close-gate` no authored `sealed_hint`. A player who walks the long way round, arrives at the wrong side of a door and pushes on it is told nothing; that is the press a shortcut loop most invites, and a sealed wall is the same defect one verb over. **One rule for both**, because two objects of one class with two defaulting policies is exactly the "capability keyed to the verb" defect this surface is CLAUDE.md's worked example of. The compiler had every ingredient to invent a line here and deliberately does not: a baked default decides the door's tone on the author's behalf and never discloses that it did, while an error makes the author say it (the no-hacks rule at a new site). **Fenced by the obligation fence**, because it is a tightening rather than new surface: the code declares `Binds::Since(11)` and `dsl::fence::Fenced` grandfathers it against a quests stage below that version, so the check itself tests no version. Below 0.11.0 a silent door still compiles and still emits nothing, which is byte-for-byte what it emitted before the version, and the fence prints how many findings it withheld. Discharged by ANY `use` trigger on the body — `QuestsContent::answers_press_at`, the same predicate the synthesis reads — or, for a `close-gate`, by an authored `sealed_hint`; not by a `strike`, which is a different gesture. `dsl::validate::press_obligation_checks`, validation tier (exit 1). Prescription: the message carries the trigger JSON verbatim, and a test parses that prescription and asserts it clears the diagnostic, so it cannot come to name a field the schema does not have (`DW0425` spent two versions prescribing `on_wrong_side`, which never existed). |
| `DW0386` | A TD `lane` (spec-0016 §6) does not survive contact with the assembled world: a waypoint anchor that resolves nowhere in the wave's area, a waypoint with no standable footing within 3 blocks, a leg the squad cannot walk (routed on the same **no-gate-use** view wave seating uses — lane mobs cannot right-click a fence gate open), or a leg of **10 blocks or less**. The spacing rule is not taste: vanilla re-rolls a patrol target to a random point once the patroller is within 10 blocks of it, so a tighter lane is one the engine quietly stops following — it reads as working-but-drunk, not as a bug. The spike's measured working default is 12. `compiler::nav::plan_lanes`, build-tier (exit 3); the message names the wave, both leg endpoints and the measured length. |
| `DW0387` | A `summon: aggro-edge` wave (spec-0016 §6) whose perception ring offers fewer valid cells than the stack has mobs. The ring is the standable, walk-reachable, line-of-sight cells on `[follow_range - 1, follow_range]` around the defended anchor, inside the area. An error rather than a silent short spawn on purpose: the round-1 lesson was a wave that never fully appeared, so its `kill` countdown could never reach zero and the delve soft-locked with every other proof green. `compiler::emit::plan_aggro_edge_spawns`, build-tier (exit 3). Prescription: give the arena room at that radius, lower the stack's `follow_range` to a ring the arena actually has, or move the defended anchor off the wall. |
| `DW0388` | **Hazard observability** (spec-0016 §4 addendum, souls dossier §5.3 / §2.2 axis 5): a timed hazard — a `timed-gate` span or a `volley` kill zone — that the player cannot **watch before committing to it**. The obligation is one standable **watch cell**: (a) at least **5 blocks** (Chebyshev box distance) clear of every cell of the lethal span — one second of sprint at the same `4 t/block` model `DW0355` and `DW0378` use, so sight from the lip of the span does not count as safety; (b) walkable from the campaign entry over the world with that span **sealed**, which is the load-bearing clause — a bay you can only reach by first surviving the hazard is not a bay; and (c) with an unobstructed sightline from eye height (1.62 above its floor) to the player-centre-mass point (1.0 above the floor, the exact point a volley aims at) of some cell the hazard judges, walked by the `DW0308` Amanatides–Woo traversal through the same `blocks_camera` sight predicate — so glass and a grate are transparent to an eye exactly as they are to a camera. Search is bounded to 32 blocks; candidates are tried nearest-first, ties on cell order (ADR-0006). Deliberately **not** required: sight to the whole span — a stair volley read from its foot is observable even though the treads occlude each other, and demanding total visibility would red legitimate geometry while proving nothing more. `collapse` is out of scope (it fires once, its region is a ceiling with no standable cell, and there is no cycle to watch — `DW0445` is its fairness proof); a region with no standable cell, and a campaign with no entry anchor, are left to `DW0444`/`DW0311`/`DW0345`. **Two tiers, one rule**: **error (exit 3)** when the campaign declares a `bonfire` — the same test the flask obligation `DW0476` uses to decide "is this spec-0016 content" — and **warning** otherwise, where the geometry is a design note rather than a broken promise. `compiler::nav::check_hazard_observability`. This is the dossier's gap G1: no source reports a duty cycle for any FromSoft periodic hazard, but every source attests the observe-from-safety rule, and the dossier's verdict is that if only one of the two proofs can be afforded it should be this one, not `DW0378`'s 20%. Prescription is always geometry — open the approach, or move the hazard off the blind side of the corner. Never shorten the standoff. |
| `DW0393` | A `timed-gate`'s `disarm` affordance is not usable **before** the gate is committed to: its `via` cell is not walkable from the campaign entry over the world with the gate span **sealed**. Same load-bearing clause as `DW0388`(b) and `DW0373`, stated for the third rung of the hazard ladder (souls dossier §5.2 — readable, avoidable, *disable-able*): a jam lever the party can only reach by first surviving the crossing disables nothing, it is a trophy for having beaten the hazard dressed as counterplay. Endpoints are snapped on the SEALED world (radius 3) so neither can land inside the span; a gate with no `disarm`, an unstandable entry or `via` cell, and a campaign with no entry anchor are left to the proofs that own them (`DW0345`, the anchor checks). `compiler::nav::check_timed_gate_disarms`, build-tier (exit 3). Prescription is geometry — put the lever on ground the approach already touches (the stair head above the run, the alcove beside the doorway) — or drop the `disarm`. Never open the gate at world-load to silence it. |
| `DW0324` | An L2 massing verb cannot apply to the solved layout (v0.6, spec-0017): the target area binds a single `prefab` (no jigsaw layout to mass), a `piece` index / `prefab` guard mismatches the placement (layout drift), a `swap-piece`/`reseed-piece` candidate cannot re-mate every mated socket without overlap (or the pool has no compatible variant), an `insert-piece` socket is already mated or nothing attaches without overlap, a `remove-piece` targets the entry piece or a non-leaf, or a `rewire-socket` names an out-of-range connector / seals an already-sealed (opens an already-open) socket. `compiler::massing`, build-tier (exit 3); every message names the batch and prescribes re-inspecting the layout with `delvec snapshot` — never deleting the drift guard or the sockets. |
| `DW0318` | **A body of fluid runs out of the built world.** A fluid cell of the assembled world lies outside every placed piece's AABB, under `horizon: void` — where a column the content did not build is bottomless, so that water is not a pond overhanging an edge but a waterfall running down forever, on the server's own clock, before any player arrives and in a delve nothing rendered it into. `compiler::nav` (`measure_fluid_escape` / `FluidEscape::finding`), build-tier (exit 3). It is also **the first thing `verify_boundary_safety` does**, so a world the water is still running out of never yields a boundary verdict at all — see the `DW0322` row for why that sequence is load-bearing and why it is structural rather than a call-site convention. Raised at **two** call sites: once per world-edits batch in the stage-8 replay, naming the batch; and once over the finished assembled world at stage 10, for every campaign that assembles one. **Stated against the world-generator ambient** (`nav::Ambient`, spec-0013 `horizon`), exactly as `DW0322` is, because what lies beyond the content is the generator's property: under **`ocean`** the pinned superflat puts water to sea level and stone below it in every unbuilt column, so a shore's water meets the sea it depicts and the rule is vacuous by premise; under **`void`** it is the finding. It is the fluid analogue of `DW0313`, which fails the build when a placed GRAVITY block falls out of a void world — the solid case was covered from the start and the fluid one was not, and that asymmetry is what made this a hole rather than a policy. It is also the thing the piece-level `DW0800` defers TO: that rule counts a run direction leaving a piece's outer face and deliberately judges nothing, because what is beyond a face is not in those bytes — only the placement knows, and this is the placement deciding. Reads the **shared** flood model (`assembled::occupancy_of`'s `flooded`), never a second one. **Runs BEFORE `DW0322`, and the order is load-bearing**: `boundary_void`'s per-column fall-arrest scan counts a flooded cell as arrest, so a bottomless column with a waterfall in it reads as supported and the boundary proof goes quiet on precisely the columns the water escaped through — escaping fluid is a false premise of the proof that runs next, the way an unsettled gravity block is of everything after `DW0313`. Measured on the shipped `island-beach-camp` piece placed under `void`: 9792 escaped cells, `DW0322` silent. Aggregates like `DW0322` — up to 6 cells named plus totals, so one dribble and a whole coastline are distinguishable without re-probing — and attributes the escape to the placed piece(s) whose AABB adjoins it. Binding: fluid cells examined and placed pieces examined, both stated in the message and in `validation/fluid-escape.json`; neither is the length of the finding list. Prescription: wall the face the water runs out of, pull the body back a cell, place a piece against that face, or declare `horizon: ocean` if the water is meant to be a sea — never delete the water, which is first-class content. |
| `DW0322` | Boundary safety (v0.6, spec-0017 invariant 4): the reachable walk region fails "one step off the proven ground is survivable **and recoverable**". `nav::verify_boundary_safety`, which **runs `DW0318` first and returns it if it fires** — escaping fluid is a false premise of this proof, because the per-column fall-arrest scan counts a flooded cell as arrest, so a bottomless column with a waterfall in it reads as *supported* and this check goes quiet on exactly the columns the water escaped through. That sequence lives inside the function rather than at its call sites, and the difference is not tidiness: held by source order it survived only until a third gate was inserted into the same function, and nothing would have said so. It cannot be pinned by a fixture either — under `void` the flood model has no floor to stop it, so escaped water masks essentially every hit and a world with both defects reports `DW0318` under either order. `nav::boundary_only` is the unsequenced proof, called by one test so the masking can be demonstrated rather than asserted. At **two** call sites: once per world-edits batch inside the stage-8 replay, which names the batch that broke the boundary; and once over the **finished assembled world** at stage 10, for every campaign that assembles one. The guarantee is a property of the assembled world, not of having edited it — keyed to the edit script, an edit-free campaign shipped a walk region that had never been proven at all. The whole-world call is an **error, unwindowed**. Note the reachability roots are `edit::anchor_starts` — every resolved anchor, seated on the nearest standable cell within `nav::SNAP_RADIUS` **inside the assembled piece that declares it** (`nav::AnchorRoot`) — not the party's spawn/checkpoints. One code, one rule — *stated against the world-generator ambient* (`nav::Ambient`, spec-0013 `horizon`), because what an unmodelled column contains is the generator's property, not the content's. **`horizon: void`**: a reachable walkable cell borders a **void drop** — a horizontally adjacent column the player can step (or open a gate) into with no fall-arrest of any kind below (no solid, no fence/wall/gate top, no water); one step off the proven ground falls out of the world. Prescription: extend the terrain under the exposed edge (fill/morph a slope or outcrop) or reinstate a barrier shape. **`horizon: ocean`**: the pinned bedrock/stone/water superflat puts ground under *every* column, so nothing can fall out of an ocean world and the void premise is vacuous — the rule is the **stranding** invariant instead (the hazard `plan::OCEAN_BASE_Y` already names): a reachable walkable cell lets the player into a body of water with no climb-out back into the reachable walk region. Prescription: give the shoreline a step at the waterline (a beach or a bank), or wall the edge so the water cannot be entered there. Both branches **aggregate**: one report per run listing up to 6 violations plus a total, so the scale of a breach (one cell vs. the whole coastline) is visible without re-probing. Build-tier (exit 3); the stage-8 message names the batch, the stage-10 one does not. Note also what does **not** silence it: a `boundary` declaration (spec-0013) is a runtime return clock the proof deliberately never reads — being teleported back after falling out is not the guarantee — so the only fixes are geometry or the `horizon` premise. Numbered in the 032x world/region family beside the spec-0013 boundary pair (`DW0320`/`DW0321` are validation-tier; this one is build-tier — it needs the assembled geometry). Never weaken the check or reroute the path around it. |
| `DW0323` | A stage-7 edit fails to **resolve** against the solved layout (v0.6, spec-0017): a piece-local frame's `piece` index is out of range or its `prefab` guard mismatches the placed piece (layout drift — the loud alternative to a silently misplaced edit), an `anchor-relative` frame names an anchor the batch's area does not resolve, or a verb's target region resolves to **zero cells** (a silent no-op is always a defect: the select drifted off the content it targeted). Also the `fragment` verb's own resolution failures: a prefab outside the admitted library, one decoding to zero non-air cells, and a `rotation` other than `none` on a prefab carrying yaw-dependent blockstate — rotate-aware stamping is not implemented, so the compiler refuses the stamp instead of shipping unrotated facings (see the stage-7 `fragment` row). `compiler::edit`, build-tier (exit 3); the message names the batch and prescribes re-inspecting the layout with `delvec snapshot` — never deleting the prefab guard or leaving a dead edit. |
| `DW0352` | A world-edits batch writes into a cell a trap's hardware occupies (v0.6, spec-0017 + spec-0011): its trigger/hazard cell, its dispenser socket, or its disarm-affordance cell. `setup_finish` runs `world_edits` **before** `trap_setup`, so the edit lands first and the trap is loaded into a block that is no longer there — vanilla's `item replace block … container.0` on a non-container fails with **no output**, so the delve ships a dead trap with every proof green (`DW0342` proves the *planned* hazard, not the surviving hardware; no geometry proof models "is this still a dispenser"). `compiler::edit`, checked first in the per-batch invariants, build-tier (exit 3). The message names the batch, the cell, the trap and which role the cell plays; prescription is to move the region off the trap's cells or re-anchor the trap — never to assume the edit leaves the redstone intact. |
| `DW0864` | **A placement verb did not deliver what it declared**, measured over the **whole domain it declared** rather than over the part of that domain which turned out to be usable. `compiler::edit`, `every_version`, one finding per verb. The owner's cherry-grove finding: a valley staged as a grove held a handful of trees and bare rock in every other direction, and the acceptance proxy that passed it was a **rendered shot** — one bearing, which cannot see a region, and nothing anywhere produced the count. `plant` declares a `count` and quietly seats whatever the ground allows once candidates run out; `scatter` can dress nothing at all and return success. **Two tiers, one code**, on the shape `DW0354` already uses. **Error (exit 3)** where the verdict cannot be argued with: a `plant` that misses its own declared `count` (the author's stated guarantee), or a verb that could act on **no cell at all** of the domain it declared. **Advisory (exit 0)** where it can: a `scatter` whose domain held cells it could have dressed and whose noise sample came up empty, since a low enough density legitimately does that. Every message states four numbers — declared, delivered, domain, usable — because the denominator is the whole point: a count whose denominator is the part that worked is the render shot in numeric clothes. **`DW0323` does not reach this.** It refuses a region that resolves to zero cells; the live instance this rule found on its first run resolved to 406 cells and none the verb could touch — the gallery's own lane scatter, aimed by a `surface-band` at the SOLID course when a scatter can only act on an air cell with something under it, silently dressing nothing for as long as it existed while the coverage gate counted the `scatter` unit as bound. Prescription: widen the selection, lower `spacing`, drop an `avoid` envelope, point the region at the course **above** the surface rather than at the surface itself, or ask for what the ground can carry — never leave the shortfall to a render. The same rule at the layer that has no diagnostics is `prefabs/invariants.rs::assert_scatter_reaches_its_target`, which every generator source-includes. |
| `DW0354` | A support-dependent block the edit script placed has no valid support in the post-batch world (v0.6, spec-0017): a torch/lantern/campfire/rail-family block with **nothing below it** after a later batch carved its support away, or flora rooted in a block flowers cannot stand on (a `scatter` over bare stone). Vanilla pops such a block off as an item on the first chunk tick, so the write silently vanishes from the delivered world while every snapshot still shows it. **Two tiers, one code**: advisory (exit 0) for decoration, aggregated per reason + block with a count and one example cell; **error (exit 3)** when the popped block is a fixture the script's own `relight` verb placed — that is a declared `min_light` guarantee the `DW0211` proof accepted, and losing it re-darkens the region. `compiler::edit`, evaluated at every batch close over the cumulative placement set. Deliberately conservative: blocks supported sideways or from above (`wall_torch`, `hanging=true` lanterns) are classified as needing no support, and "support removed" means removed to **air** — the check never guesses about a block it cannot classify. |
| `DW0325` | A `move-actor` destination is unreachable over the assembled geometry for the **actor's footprint** (per-entity dims table; warden 0.9×2.9 needs 3 cells of headroom, so it can be stranded where a player fits), or an actor spawn/destination anchor resolves to no world position (spec-0014). Build-tier (exit 3), `compiler::nav`; the message names the actor, the leg, and a best-effort first blocked cell. |
| `DW0327` | A `begin-stealth` (spec-0014) zone has **no** standable cell, or **no** standable cell of the zone is reachable from the player's position at the beat that activates the stealth check — a guaranteed-unwinnable stealth beat. Reachability is **reachable-any over every cell of the zone box**: testing only the cell nearest the zone centre raised a spurious `DW0327` whenever that one cell happened to snap into a walled-off pocket of an otherwise perfectly reachable zone. The message names the zone and prescribes placing it over reachable floor / within walkable reach of the activating beat. |
| `DW0355` | A **punishing** `begin-stealth` beat whose grace window cannot be beaten (spec-0014 + spec-0016): from a position a player legally occupies the instant the session arms, no zone is reachable within `grace_ticks` at sprint speed over the assembled geometry. DW0327 proves cover exists and is *connected*; this proves it is reachable **in time** — the gap that shipped the island's blinding beat, where the beat armed under the player's feet at the fire-pit and killed every player (bot and human alike) ~2 s later, on a first honest ladder run. Start positions: the activating objective's anchor **and** every `set-checkpoint` reigning inside the beat's active window `[fire_step, end_step]` — a respawn point that cannot beat the window makes the retry loop non-terminating rather than a souls retry. Cost model: 4 ticks/block (vanilla sprint 0.2806 blocks/tick, rounded up — no sprint-jump credit) + 6 ticks per block climbed + 10 ticks of standing-start reaction; routed over the same per-leg geometry DW0311/DW0315 use (gates causally sealed by the firing step forced solid). Build-tier (exit 3), `compiler::nav`. The message names the beat, the start, the nearest zone cell, the measured flee time and the tick deficit. Scope: only beats whose `on_caught` tree actually punishes (`damage-players` / `spawn-wave`) — a narrate-only beat has nothing to escape. Prescription: raise `grace_ticks` to at least the measured need plus a tension margin, put a zone within reach of where the beat starts, move the checkpoint into/beside a zone, or arm the beat from a less exposed objective. **Delaying the arm does not discharge it** (a `sequence` step buys drama, not proof: the clock still starts with the player free to be standing at the start cell), and deleting the `on_caught` consequence is explicitly not a fix. Numbered `DW0355`, not `DW0352`: this rule and the map editor's trap-hardware check were developed on parallel branches, each picking the next free code against its own branch point, and collided on merge — `tools/check-dw-codes.py` now gates one-code-one-rule so that class fails CI instead of shipping. |
| `DW0329` | A `sequence` effect is nested inside another `sequence` (directly, or reachable via a nested `move-actor` `on_arrive`) — timelines do not recurse (spec-0014). Validation-tier (exit 1), `dsl::validate`. Flatten the inner steps into the outer timeline (shift their `at_ticks`). |
| `DW0342` | A **lethal** trap (spec-0011) whose trigger cell lies on the forced critical path with no discharge — not avoidable (the trigger cell is a required path cell), not survivable (`rearm`, so a respawn walk-back re-triggers it → soft-loop), and not disarmable (no disarm affordance reachable before it, over the world with the trap cell blocked). The player is provably killed or soft-looped. **Analysis-tier: exit 2**, like `DW0312` — a content-design mistake, not a geometry defect; the message names the trap and prescribes moving it off the path, setting `reset: once`, or adding a reachable `disarm`. Renumbered off the spec's stale reserved number (0314 — since taken by the waypoint self-check). |
| `DW0344` | In a `horizon: ocean` world, a placed piece whose prefab metadata declares `waterline_y` does not land that waterline at sea level (`piece.y + waterline_y ≠ 62`) — the piece floats above the sea (its shore an unclimbable cliff, its authored water pocket hanging in the air) or is drowned under it. Build-tier (exit 3), `compiler::plan`, checked after placement. Nothing downstream can catch this: nav, boundary, POV and PackTest all derive from the very placement that is wrong, so a mis-datumed island validates green and ships unplayable. The message names the area, prefab, placed y and the signed offset, and prescribes correcting the declared `waterline_y` (the local y of the piece's top water block; the island convention is 2) or rebuilding the piece against the convention — ocean areas are placed at y=60 and a piece with a different waterline cannot share that datum. Pieces declaring no `waterline_y` author no sea and are not individually checked — but the check reports its **binding count**, and an ocean world that places pieces where *none* declares a `waterline_y` is reported under this same code rather than passing silently: the invariant examined zero pieces, so nothing proved that anything in the world meets the sea where the sea is. Emitted as a warning beside the build; the message names how many pieces were placed, how many were examined (zero), and how many stand at or below the sea plane. A zero binding is never a pass here because the field being optional makes a declaration that was **deleted** indistinguishable from one that was never needed, and those need opposite answers — a real path, since an admission step that read prefab metadata through a type not modelling the field deleted it on write from the five island prefabs that carry it. No exemption is offered: the only justification an author could write ("this piece needs no waterline") is the missing declaration wearing another name, and the only geometric one ("no piece reaches the sea") is unsatisfiable while every ocean area is placed at y=60 under a sea at y=62 — so every piece of every ocean world stands in the water, and an author has no lever to lift one out. That is also why the verdict a zero binding earns, a refusal, is not yet the verdict it gets: with no lever, a refusal would demand a declaration of water the piece does not author. The per-area datum that makes a dry ocean piece authorable is the change that raises this to a refusal, and a fixture assertion on the sea-plane count reds when it lands. |
| `DW0345` | The assembled world resolves **no entry anchor** — no area places a piece whose prefab metadata declares an anchor with `"role": "entry"`, and none carries the fallback spelling (`spawn`, `entry`) either; see §4 "Entry point". The compiler then has no cell to call the campaign's start: no `setworldspawn`, no class-apply teleport, no first-join placement, no `dw:cp` seed. Build-tier (exit 3), `compiler::emit`, and raised **before any model is built** — a world with no start does not have a walking problem, and letting it through meant the crossing fell to the critical-path walk and was reported as `DW0311` over a leg nothing was meant to walk, which sends a reader looking for a wedged doorway. Silent before either — the delve compiled clean and fell back to the vanilla spawn search, which a **dedicated** server resolves to the surface (so every rung of the validation ladder stayed green) and the **integrated singleplayer** server resolves to the build floor, i.e. inside solid stone. Prescription: give the piece the party arrives in an anchor at that cell and put `"role": "entry"` on it (in a `prefab_pool`, that is the prefab the layout is seeded from), or bind the area to a prefab that already has one. The two names are a compatibility path for pieces admitted before the role existed — a piece written today declares the role rather than being renamed to match a spelling. |
| `DW0872` | **A crossing into an area with nowhere to arrive.** A leg of the party's forced route changes area, and the destination declares no entry point — no anchor in its prefab carries `"role": "entry"` and none bears the fallback spelling (`spawn`, `entry`), or the one that does resolves to a region rather than to a cell. Areas sit `AREA_SPACING` (256) blocks apart across void, so a leg that changes area is never a walk: the party has to be put down inside the destination, and nothing says where. Build tier (exit 3), `compiler::plan`, raised while the critical path is built — so it precedes the walk proof, which is the whole point. Silent before: the crossing was simply not emitted, the leg fell through to `DW0311`, and the author was told *the player cannot walk … a wedged doorway seam, a void gap, an unbroken fence ring* — or, on a campaign with a gate anywhere, to reopen a `close-gate` it does not have. Every word of that is true about a walk and this leg was never one. `DW0345` is the same rule over the **whole world** (no area at all declares one) and `DW0872` over **the one area a body must be put down in**; different quantifiers, different remedies, two codes. Prescription, in spec-0046's vocabulary: give the piece the party arrives in an anchor at that cell and put `"role": "entry"` on it (in a `prefab_pool`, that is the prefab the layout is seeded from), or bind the area to a prefab that already has one. |
| `DW0873` | **The party's first leg is a crossing, and nothing can carry it.** The campaign spawn and the first objective on the critical path stand in different areas. A crossing rides on the completion of the objective the party leaves from, and at the spawn they have completed nothing — so the first leg can be neither ridden nor walked and the delve cannot be started. Build tier (exit 3), `compiler::plan`. This is the member the leg enumeration missed: it paired *consecutive objectives*, and the spawn is a leg's origin that is not an objective, so the party's opening move was in no pair — no crossing emitted for it, and `DW0311`, which walked the same pairs, never examined it either. Such a campaign compiled clean, passed every generated game test, and stranded the harness at the spawn with `No path to the goal!` and no code at all. Prescription: put the campaign's first beat where the party starts (the quest's `area` in the quest plan), or start the delve where the beat already is by listing that area first in `world.areas` — the delve starts in the first area declaring an entry point of its own, so that area needs one. There is no third way across, and that is deliberate: a delve whose opening move is a teleport out of the area it starts in has put its spawn in the wrong area, and the refusal says so rather than inventing a start-of-run crossing to carry it. |
| `DW0346` | A prefab metadata `*.json` (or `pools.json`) in the prefabs dir failed to read or parse. `PrefabRegistry::load_dir` records a per-file diagnostic naming the file and the serde error, folded into every `validate`/`analyze`/`build` at **validation tier (exit 1)**; loading continues for the other files (report-all, not fail-fast). Without it the prefab simply vanished from the registry and the run failed much later as a baffling `DW0300` "prefab not found" (or a `DW0160` binding error) with no hint of why. What reaches this code is a document that is **malformed for this delvec**: a value of the wrong type, an absent required block, unreadable bytes. A key this delvec does not model is deliberately not one of them — see `DW0543`. Prescription: fix the named field. **A tile-set manifest is not one of these**: metadata carrying `structure_set` instead of `structure` is an ordinary prefab document whose blocks arrive as several `.nbt` tiles, and it loads, is indexed under its own prefab id, and is placed (see §4 “A piece's blocks arrive as one template or as a tile set”). What is still `DW0346` about a manifest is the same thing that is `DW0346` about any document: it is malformed — tiles that do not cover the zone exactly (a reassembly with a hole or an overlap), a tile past the declared `part_max`, an empty parts list. A document declaring **neither** structure block, or **both**, is likewise `DW0346`, and the message names both keys: “which shape is this” has two answers and no third, and a bare serde `missing field \u0060structure\u0060` was a true statement about the bytes and a useless one about the situation. |
| `DW0347` | A `cutscene` shot's aim sweeps faster than the angular budget: over 6°/tick (120°/s) peak on the exact eased path — at 20 Hz that reads as a spin, not a shot (the camera dossier's comfortable band is ≤ 2°/tick; thresholds are the dossier's proposal — the spike rig has no rendering client to calibrate against footage). Typical cause: a `look_at` subject too close to a fast dolly, or a sharp travel-aim corner. Build-tier (exit 3), `compiler::nav`. An **error**, not a warning: the shot is provably nauseating before it ships, and the fix is always available — more camera distance, a longer `seconds`, or splitting the move into two shots (the hard cut between shots is the idiomatic fast reframe). |
| `DW0360` | An anchor-bearing campaign effect — at **every effect root**, at **any** nesting depth — names an anchor that resolves to no position in the assembled world. The single resolved-anchor-or-diagnostic seal over the whole effect surface, driven by `QuestEffect::anchor_refs` (the referential sibling of `nested_effect_lists`) over the roots `plan::for_each_effect_root` enumerates. **The roots are inherited, not re-listed**: this walk hand-listed three of the five, so a typo'd anchor in a `traps[].payload` or a dialogue option's `set-checkpoint` `on_respawn` bundle was never asked the question — the build stayed green and `trap_fire_<trap>.mcfunction` shipped with the `open-gate` simply absent, which is the silent-drop class this seal exists to end, live inside the seal itself. **Scope: the verbs that fail open, plus the corner where nothing else looks.** The spec-0022 payload verbs (`volley`, `collapse`) fail *closed* — `plan_payload_verbs` resolves their volumes with `?` and reports `DW0447`, which names the verb and the volume — so **where `DW0447` runs**, they keep their own diagnostic rather than being preempted by this generic one (see "Known spec ↔ code drift" for why that overlap exists at all). `plan_payload_verbs` lives inside the world block, so it runs only when the campaign assembles a world (`emit::assembles_world`, the one predicate the world block itself reads), and a payload verb does **not** imply that: nothing confines `volley`/`collapse` to `traps[].payload`. The deferral is therefore conditional on the proof running; in a campaign with no traps, no waves, no bodies and no walkable critical leg, this seal keeps the payload verbs itself. It exists because every anchor consumer in emission fails **open**: `open-gate`/`close-gate` scan `plan.anchors` for a name match and fall out of the loop, `set-block`/`set-checkpoint`/`play-sound`/`damage-players` bail out of an `if let Some(pos)`, and a cutscene waypoint silently degrades to `[0, BASE_Y, 0]`. One typo'd anchor therefore emitted **nothing** — a door that never opens, a checkpoint bound to nothing — in a delve that compiled clean. `DW0142` catches what the DSL can see (an area's declared anchor set); this re-asks the question of the *assembled* world, so pool areas and cross-area camera anchors are covered too. Build-tier (exit 3), `compiler::emit`, run **first** among the referential proofs: an unresolved waypoint degraded to the origin otherwise surfaces as a bogus `DW0308` camera clip, sending the author to move a shot that was never the problem. |
| `DW0361` | Two different generated artifacts (function / dialog / advancement) sanitize to the same name, so one would silently overwrite the other in the emitted pack. `plan::safe_local` is doubly lossy — it drops an id's `<kind>/` prefix and folds `-`, `/` and `.` all into `_` — so wave `wave/npc-x` and npc `npc/x` both name `spawn_npc_x`, and `move-npc npc/guard-a → anchor/post` collides with `npc/guard → anchor/a-post` (which also aliases their tick counters and re-entry sentinels: two live movement drivers sharing one score). The output map is a `BTreeMap`, so the loser used to vanish without a word — the wave simply never spawned. Re-emitting the **same bytes** under one name stays legal (the emitters dedup by content key); only a genuine divergence fails. Build-tier (exit 3), `compiler::emit`. Prescription: rename one of the colliding ids so their sanitized local parts differ. |
| `DW0362` | A dialogue node declares more than `MAX_GATED_DIALOGUE_OPTIONS` (10) conditionally-visible options (`requires_flags` / `forbids_flags` / a `complete-objective` effect). Vanilla cannot hide a `dialog` option, so the compiler encodes visibility by precomputing **every combination**: `n` gated options emit `2^n` dialog JSONs plus a `2^n`-clause dispatcher keyed on a `dw.dmask` bitmask. Ten is 1024 variants for one node — already an order of magnitude past anything authorable (the largest node in any shipped campaign gates four), and the point past which pack size rather than the author decides what the delve is. Behind the soft cap is a hard wall: the mask is built with `1u32 << i` (a debug-build **panic** at 32 — the original symptom) and compared against a Minecraft scoreboard, i.e. an `i32`. Build-tier (exit 3), `compiler::emit`; the message names the node and npc. Prescription: split the node into a short chain, or move some gating onto the objective that reaches it. |
| `DW0363` | A trap declares a flag gate (`requires_flags` / `forbids_flags`) whose trigger hardware the compiler cannot remove and restore. Trap flag-gating is a **physical** gate: the trigger block leaves the world while the gate is shut and is put back verbatim (blockstate and all) when it opens, so it is only sound for a trigger whose entire state is the block — a pressure plate or a tripwire. A `trapped-chest` trigger carries a block entity with an inventory that removal would destroy, and a gated trap whose `anchor/trap` metadata declares no `trigger_block` names nothing the compiler could put back. Rejecting the gating surface for those cases is deliberate: the alternative is shipping the documented behaviour as folklore, which is exactly what happened before (the flag lists were planned and `DW0172`-checked but read by **no** emission site, so "inactive while the flag is set" did not exist). Build-tier (exit 3), `compiler::emit`. Prescription: declare the plate/tripwire as `trigger_block` on the anchor's prefab metadata (with its blockstate, as a gate anchor declares its fill `block`), switch the trap to a `pressure-plate`/`tripwire` trigger, or gate the story beat that arms the trap instead. |
| `DW0359` | An NPC or actor **body** stands on, or immediately in front of, an interaction affordance the party has to click (owner island QA, round 7). Bodies are boxes: a mannequin wears its `base_entity`'s standing hitbox (`nav::entity_dims` — one dims table, shared with actor-footprint routing), or the player model's 0.6 × 1.8 when it declares a `skin`; every affordance the compiler summons is a `minecraft:interaction` of `width:1.0f,height:2.0f`, i.e. exactly its anchor cell's column two blocks tall. Five affordance sources, one shape: `interact` objectives, `use`/`strike` triggers, `bonfire` rest points, `shortcut` unlocks and trap `disarm` affordances. **Two tiers, one code**: **error (exit 3)** when the boxes overlap in all three axes — the client's ray-pick reaches the invulnerable body and the affordance can never be clicked, so a required objective is unreachable and the delve soft-locks; **advisory (exit 0)** when they are apart but within 1 block horizontally (Chebyshev) with overlapping vertical spans, because whether a neighbouring body actually shadows the crosshair depends on the approach angle the player takes, which the compiler cannot know. `compiler::eclipse`, run with the referential seals before any occupancy model (pure box arithmetic over resolved cells). **It runs there and no earlier, and the reason is the word *resolved*.** Its input is an anchor's CELL, which is not a document fact: on a site plan the synthesized vocabulary is read off the derived mass (`blockout::footing`, `blockout::station_cell`), because a box's floor centre is sometimes inside the massing — a stair the plan hosts in that box legitimately stands on it — and on a prefab map the cell is what placement produces. `validate` and `analyze` hold neither, so this rule at either tier would be a second implementation over positions nobody has computed yet. It is already the earliest point at which its own inputs exist, which is also why a slow build is not evidence against the tier: the time is spent producing the cells, upstream of every check that reads them. This is the geometric statement of `DW0350`, which is symbolic (same anchor *name*) and sees only `use` triggers — an NPC body over an *objective's* affordance, or a 1.95-wide ravager's shoulder reaching into the cell next door, passed silently. It is the check the round-7 island needed: `npc/polyphemus`, a 0.9 × 2.9 warden on `anchor/fire-pit`, hid `obj/harden` and `obj/blind` behind itself. Two exemptions, both about not inventing certainty: a `strike` trigger on an NPC's own anchor summons **no** entity (it rides the NPC's hitbox — nothing to eclipse), and a body the campaign ever **moves** (`move-npc`/`move-actor`, any depth) is skipped, because a declared anchor is only a walker's starting mark and deciding "is it still there when the affordance goes live?" needs a timeline the compiler will not guess (known blind spot: a body walked *onto* an affordance, which wants a destination rule of its own). Prescription: move the body's anchor or the interaction's anchor 2+ blocks apart — **never** make the body intangible, which trades a dead objective for a character the party cannot talk to. |

### DW039x — shot calibration (`delvec calibrate`; spec-0019)

`calibrate` is the only subcommand that reads no campaign and builds no world —
just a harvested `rehearsal-report.json` plus the build's
`creator-datapack/layout.json`. Its codes therefore carry their own exit
mapping, stated per row rather than by the DW03xx section default.

| Code | Meaning |
|------|---------|
| `DW0390` | A harvested shot proposal names a cell with **no declared anchor within the 16-block snap radius**, so it cannot be written back into the DSL at all — the DSL has no free-floating world coordinates (spec-0019 §5). Reported per offending cell with the nearest anchor and its distance; the whole shot is left un-patched (a half-snapped dolly would fly a path nobody authored), while every other shot of the same session still is. **Exit 3**, and the patch file is still written. Prescription: declare an anchor near that cell in the prefab's metadata and re-mark the shot, or move the shot to an anchored spot — do NOT widen the radius and do NOT write a raw coordinate into the stage document. |
| `DW0391` | The rehearsal report and the `--layout` manifest name **different campaigns**: the proposals would snap onto another delve's anchors and silently relocate every camera. Refused before any snapping. **Exit 1**. Prescription: point `--layout` at the `creator-datapack/layout.json` of the build that session actually played — do NOT reuse an older build's manifest. |
| `DW0392` | The rehearsal report is unreadable, is not a rehearsal report, or carries a schema `version` this `delvec` does not understand (likewise for an unreadable layout manifest). **Exit 1**. Prescription: re-run `delve-harvest` over the session log — the report is a machine artifact and is never hand-written or hand-edited. |
### DW04xx — staging-timeline proofs (`compiler::nav` + `compiler::timeline`; error; exit 3)

Proofs about the order of effects **inside one timeline** — what the DAG-causal
`DW03xx` gate model deliberately cannot see (it reasons about quest causality
between bundles; this reasons about position within a bundle).

| Code | Meaning |
|------|---------|
| `DW0410` | A staged walk (`move-actor` / `move-npc`) whose path is blocked by a gate an **earlier effect in its own timeline** sealed with `close-gate`. The round-8 island defect exactly: a `sequence` closed the boulder at `at_ticks: 460` and walked the giant across that region at `at_ticks: 700`; the walk was planned on the open world (gate regions are modelled passable), so the build shipped green and the actor stepped through solid basalt on the live server. Timelines are **every effect root** `plan::for_each_effect_root` enumerates — an `on_objective_complete`/`on_complete` bundle, a trigger's `effects`, a `traps[].payload`, a dialogue option's `set-checkpoint` `on_respawn` bundle (each declared order, one tick) — plus a `sequence` ordered by `(at_ticks, declaration index)` and an `on_arrive` bundle inheriting its move's state. A model seeing only three of the five roots emission reaches would lower a walk in a payload or an `on_respawn` bundle, never prove it, and (through `nav::all_effects`) never even plan it — its `function <ns>:ma_…` call would have no driver behind it. The two are **optional** roots (the trap may never spring), which needs no special case here: this proof is conditional on its own bundle running, so a firing that never happens never runs the walk either — see §4 "Close-gate solidity for staged walks". **The planner routes over the timeline-adjusted world first**, so a legal detour around a shut gate is simply taken and nothing is reported; this fires only when the sealed world admits no route *and* the open world does — which is precisely what distinguishes it from `DW0325`/`DW0307` (unwalkable on the open world at all). Build-tier (exit 3), `compiler::nav`; the message names the verb, the mover, the leg and every gate anchor sealed ahead of it. Prescription: move the walk before the `close-gate` (a lower `at_ticks`, or an earlier position in the bundle), reopen the gate with `open-gate` before the walk, or retarget the walk to a destination reachable on the sealed side — commonly the walk belongs *before* the seal, since the staging beat is "the walker crosses, then the boulder comes down behind it". Never silence it by deleting the `close-gate`: the seal is the point-of-no-return the staging wants. |

### DW043x — geometry & container proofs (stair orientation; spec-0021 loot; v0.8 `collect` container adoption)

Two unrelated families sharing a number block: proofs that a *block* is the
block the content meant, rather than proofs about quests or timelines.

| Code | Meaning |
|------|---------|
| `DW0430` | A stair block on a **proven route** whose `facing` contradicts the climb it carries. Build-tier (exit 3), `compiler::stairs`. A vanilla stair's full-height half sits on its `facing` side (verified against the 1.21.11 collision shapes: `facing=north` puts the upper box at `z ∈ [0.0, 0.5]`, the north half), so `facing` **is** the direction you ascend. Nav models a stair as a full cube (`collision_top_16` returns 16), which means a reversed stair reads as a legal one-block *jump* and every other proof passes — the delve ships green with a staircase the player must hop up tread by tread, which is exactly how whole tidal-keep runs reached a playtest backwards. Scope is deliberately narrow to stay free of false positives: only stairs that are the floor of the **higher** cell of a ±1-elevation step on a proven route are inspected, then widened laterally across the run's width, each lateral cell gated on its own approach-side riser test (so a spiral's turn cannot bleed into the flight at right angles to it). Keying on the higher cell is what makes a turning staircase safe — the tread you arrived on still legitimately points the old way. Decoration is never inspected: a stepped gable, a corbel or a chair has no climb semantics. The message groups defects **per prefab piece** (the fix list — one wrong literal in a generator produces a whole run) and then names individual cells. Prescription: fix the piece that authors the blocks and re-export its `.nbt`. Do NOT reroute the critical path around the staircase and do NOT widen the nav step rule — the route is correct, the geometry is not. |
| `DW0431` | A stage-5 `loot` anchor whose assembled-world cell does not hold a fillable container, or holds one with fewer slots than the declaration has stacks. Build-tier (exit 3), `compiler::loot`; evaluated over the **edited** model when a stage-7 script exists, since a batch may legitimately be what places the barrel. Fillable = `minecraft:chest`, `minecraft:trapped_chest`, `minecraft:barrel` (27 slots each); `ender_chest` (per-player), `shulker_box` (an item) and double chests (two block entities, so `container.<n>` is ambiguous) are excluded on purpose. This exists because the failure is **silent**: `item replace block … container.<n>` against a non-container produces no output at all — the same hazard `DW0352` documents for trap dispensers — so the delve would ship with a bare wall where the stores should be. **The prescription is computed, not phrased.** Telling an author to "point the entry at an anchor that already has one" is a search over a prefab library the campaign does not own — and measured over the library these refusals are met in, 5 of 36 pieces contain a container blockstate at all, 3 declare an anchor whose NAME says container, and the intersection is one piece; both pieces named `anchor/chest` carry no container anywhere. So the message names the anchors instead, from the assembled world the proof is already holding (`loot::container_anchors`), with the area, the block and the slot count. It has three arms and each is a different answer: **free containers exist** — they are listed and the fix is a campaign edit; **every container is already being filled** — they are listed with what claims each, and a second fill of one of them is `DW0435`, so the campaign needs another container rather than another reference; **the world holds none at all** — there is no campaign edit, and the message says so, then names the half that is the author's (bind an area to a piece that carries one) and the half that is not (have a piece export one, which is a prefab-library change through the piece's own admission). A claimed container is never offered, which is what makes the remedy reachable rather than merely plausible. Do NOT paper over it with a `set-block` effect: the container is furniture, and furniture belongs in the piece. |
| `DW0432` | A **positional container fill** declares more stacks than a vanilla chest or barrel has slots (27): a `loot` entry's `items`, or (v0.8) a `collect` whose own stack plus `fill_count` padding exceeds 27. Validation-tier (exit 1). Slots are assigned positionally, so every stack past the 27th would be dropped without a word. Prescription: split the contents across more than one container, or lower `fill_count` — a container that reads full does not need to overflow. |
| `DW0433` | An enchantment id — on an `equipment` piece or a `loot` stack — is not in the pinned 1.21.11 enchantment registry. Validation-tier (exit 1). The registry is the 43-id `enchantment` list from the same misode/mcmeta 1.21.11 summary the item registry comes from. The message calls out the classic trap explicitly: vanilla's curse ids are `minecraft:binding_curse` and `minecraft:vanishing_curse`, never `curse_of_binding`. |
| `DW0434` | An enchantment level outside `1..=255`, the range the `minecraft:enchantments` component can store. Validation-tier (exit 1). Levels **above an enchantment's survival maximum are deliberately allowed** — exceeding it from a command is legal vanilla and is precisely how a set-piece elite is built, so the compiler does not overrule that design call. `0` means "not enchanted" and is silently dropped by the game, which is why it is rejected rather than ignored. |
| `DW0437` | An `interact` declares `missing_item_hint` without a `requires_item`. Validation-tier (exit 1). The hint exists to answer a click that arrives without the required item **in hand**; with no item gate there is no such click, so the authored line is dead content that could never narrate — and an author who wrote one plainly meant to gate the interaction. Prescription: add the `requires_item` the hint is about, or drop the hint. |
| `DW0435` | Two **positional container fills** claim one anchor: two `loot` entries, or (v0.8) a `loot` entry and a `collect`'s adopted `container`, or two adopted collects. Validation-tier (exit 1). Slots are assigned positionally from `container.0`, so the later fill overwrites the earlier one slot-for-slot and the loser's items never reach the player — and for two collects it is worse: whichever activates second replaces the first objective's items with its own. Prescription: give each fill its own container anchor (prefabs may expose several), or fold the items into one — never rely on declaration order to combine them. |
| `DW0436` | A **single-slot fill**'s `count` exceeds the item's `minecraft:max_stack_size` in the pinned 1.21.11 registry. Validation-tier (exit 1). Covers every DSL surface that compiles to `item replace … container.<n> with <item> <count>`: a `loot[]` stack, a `collect` objective's prop chest, and a trap's `dispense` payload. The command fails **SILENTLY** above the cap — the slot ships empty, the server logs nothing — which is the same silent-failure class `DW0431` exists for, and it shipped: `minecraft:rabbit_stew` (cap 1) declared `count: 2` put nothing in a the-drowned-bell chest, caught only by the generated PackTest. The cap is Mojang's own data, vendored per MC pin as `crates/compiler/data/item-stack-sizes-1.21.11.json` (regenerate with `tools/extract-item-stack-sizes.py`; a test pins its key set equal to the item registry's) — never a hand-maintained table. 1.21.11 uses exactly three caps: 1, 16, 64. Skipped when the item id is unknown, since that is already `DW0143`. Prescription: lower the count, or add more entries/containers. Do NOT rely on the game splitting the stack — `give` does, `item replace` does not. |
| `DW0438` | A `collect` objective's adopted `container` (DSL v0.8) does not resolve to a fillable container in the assembled world, or resolves to one with fewer slots than the fill needs. Build-tier (exit 3), `compiler::loot` (`check_collect_containers`); evaluated over the **edited** model when a stage-7 script exists, since a batch can legitimately be what puts the barrel there — the same pass and the same world `DW0431` uses, so the two container proofs cannot disagree about what is in the room. The sibling of `DW0431` reached through the other door, and the same silent failure: `item replace block … container.<n>` against a non-container fails **without output**, so this would ship an uncompletable objective with nothing anywhere to pick up. The message names the objective, the container anchor, the cell and the block actually found. Prescription: the same computed one `DW0431` carries, from the same writer (`loot::container_remedy`) over the same list — the two are one question about one object class reached through two doors, so a second copy of the sentence would be two rules for one thing. It names the anchors of this campaign's assembled world that hold a container and are not already being filled, with area, block and slot count; or, where every one is claimed, names the claims and says a second fill of a claimed container is `DW0435`; or, where the world holds none, says plainly that no campaign edit can create one and names what is the author's (bind an area to a piece that carries a container) against what is not (a change to the piece, through the prefab library). **Anti-dodge:** dropping the `container` field to go green is explicitly not the fix — it silently returns the delve to a compiler chest floating beside the furniture, which is the defect the field exists to remove. That anti-dodge is only honest beside a remedy that exists, which is why the two landed together: a refusal that forbids the dodge and prescribes something unreachable leaves the dodge as the only move. |

### DW044x — command-driven trap payloads (spec-0022)

Redstone keeps exactly one job — the visible, learnable **trigger**. Everything
downstream of it is commands, so a trap's consequence is an authored effect
bundle (`traps[].payload`) rather than hidden wiring. Two tiers: `DW0440`,
`DW0441` and `DW0443` are structural (`dsl::validate`, exit 1); the rest are
geometry proofs over the assembled world (`compiler::nav` / `compiler::emit`,
exit 3).

| Code | Meaning |
|------|---------|
| `DW0440` | A trap declares **no consequence at all** — neither the legacy redstone `effect` (spec-0011 `dispense`) nor a spec-0022 command `payload`. A trigger with nothing downstream of it is scenery, but the completability proofs still model its cell as a hazard, so it is a content mistake rather than a deliberate no-op. Validation-tier (exit 1), `dsl::validate`. Prescription: give the trap a `payload` (`volley`, `collapse`, `damage-players`, `play-sound`, `narrate`, `set-flag`, `spawn-wave`, …). |
| `DW0441` | A payload verb's vanilla id is not in the pinned 1.21.11 registry, or is of the wrong kind: a `volley` `projectile` must be an **entity** id, a `collapse` `falling_block` / `then_floor` a **block** id. Validation-tier (exit 1), `dsl::validate`; mirrors `DW0143`/`DW0341`. |
| `DW0442` | **The saturation proof.** A `volley`'s `from_anchor` has no clear line of fire to a standable cell of its `kill_zone`, and the message names that cell and the block that stops the shot. A volley BLANKETS its kill zone across repeated salvos, so an uncovered cell is a pocket a player is safe in **by accident** — which turns dodging from a decision into luck. Coverage is therefore proven, not hoped: `nav::plan_volley` returns one shot per standable cell or this error, and the emitter writes exactly those shots, so there is no path by which a volley ships covering less than it declares. The ray uses the same `walk_cells` traversal as the cutscene clip, against a **projectile** predicate (`is_occupied`) rather than the camera one — glass is transparent to a camera and solid to an arrow, so reusing `blocks_camera` would prove coverage through a window. Because projectiles are summoned `NoGravity`, the segment checked here is exactly the segment flown. Build-tier (exit 3), `compiler::nav`. Prescription: clear the obstruction, move `from_anchor` where it sees the whole zone, or shrink `kill_zone` to the part it does cover. |
| `DW0443` | A `volley`'s `salvos` (1..=16) or `interval` (1..=200 ticks) is out of range. A volley fires its whole kill zone every salvo, so the entity count is `salvos x standable cells`; past the cap that is a server hazard rather than a trap, and salvos spread wider than the interval cap stop reading as one event. Validation-tier (exit 1), `dsl::validate`. |
| `DW0444` | A payload verb's volume is unusable: a `volley` `kill_zone` with **no standable cell** (nothing to saturate — the volley fires into geometry no player can occupy), a `collapse` region holding **no blocks** (nothing would fall), or a collapse whose debris finds nothing to land on within 64 blocks. Build-tier (exit 3), `compiler::nav`. |
| `DW0445` | The critical path is no longer completable once a `collapse` has fired — the debris buries the only route. A trap is proven in its **sprung** state, because a player will step on the trigger; this is the mirror of the `shortcut` seal, which proves the delve finishable with the shortcut never taken. The post-collapse world is modelled by settling each dropped column onto the first solid cell beneath it and adding the debris as solid geometry (`World::with_sealed`), leaving the deleted region in place — deliberately conservative, so the proof can only ever be stricter than the real world, never laxer. Build-tier (exit 3), `compiler::nav`. Prescription: leave a way through the rubble, drop fewer layers, or move the collapse off the forced path. |
| `DW0446` | A `volley`'s `from_anchor` cell is solid or flooded, so the projectile would be summoned inside geometry and never leave it. Build-tier (exit 3), `compiler::nav`. Prescription: put the anchor in the open air of the firing niche — it marks where the projectile spawns, not the wall it comes out of. |
| `DW0447` | A payload verb centres its volume (`kill_zone` / `region_anchor`) on an anchor no placed prefab piece provides, so the box cannot be resolved. Reported rather than silently degenerating to an empty — and therefore vacuously "covered" — zone. Build-tier (exit 3), `compiler::emit`. |
### DW045x — body clearance and body traversal (`compiler::clearance` + `compiler::traversal` + `dsl::validate`; error + advisory)

An entity is a box with a real size, and so is a block. These prove the two
never occupy the same space — the counterpart to `DW0359`, which proves a body
does not occupy the same space as an *affordance*.

| Code | Meaning |
|------|---------|
| `DW0450` | An NPC or actor **body is inside solid block geometry** — at the anchor it is summoned on, or at some tick of a walked leg. Build-tier (exit 3), `compiler::clearance`. The owner's island rounds 8/10/11 defect class, in its clearest instance: `actor/polyphemus-walker`, a `minecraft:warden` (0.9 × 2.9 blocks), is `spawn-actor`ed at `anchor/mouth-side`, which resolves to `[6, 69, -45]` — and `[6, 69, -45]`, `[6, 70, -45]`, `[6, 71, -45]` are all `minecraft:cobblestone`, the cliff face beside the cave mouth. The emitted command is `summon minecraft:warden 6.5 69.0 -44.5`, straight into the rock, and every other proof was green. **The asymmetry this closes**: a *walked* destination was already safe by construction — `move-npc`/`move-actor` snap their endpoints to a standable cell (`SNAP_RADIUS`) and A* only steps through passable cells — but a *placed* body was proven only to have an anchor that RESOLVES (`DW0325`), and `summon` does no snapping, so the anchor is exactly where the body lands. Model: the entity's standing hitbox from `nav::entity_dims` (the one dims table, shared with `DW0359` and actor-footprint routing), centred on the position, rising `height` from the feet; intersected against each cell's true collision volume (`nav::World::solid_top_16` — a bottom slab is `y..y+0.5`, a `dirt_path` `y..y+15/16`), over the same assembled world every other geometry proof reads (settled, sealed, stage-7-edited, relight fixtures in). Water is not geometry and is excluded. Positions checked: every NPC anchor (incl. `deferred`), every actor anchor (incl. spawn-and-unleash), and **every emitted waypoint of every planned leg** — the exact per-tick `tp` coordinates the datapack ships. A leg reports its first offending tick only (a body dragged through twenty blocks of rock is one defect); all error-tier violations are named in one message so a single build gives the whole fix list. Prescription: move the anchor to a cell with real clearance (the message states how many cells of headroom the body needs), or give the leg a corridor the body fits. Do **not** shrink the body: `move-npc` plans on the *player* footprint by construction, so a warden-bodied NPC walked down a 2-high corridor is a route that was never sized for it — fix the route or the body, never the dims table. |
| `DW0451` | Advisory (exit 0), same module: the hitbox is clear, but the body will still read as clipping. Two cases, both measurements the compiler can state and must not adjudicate. **(1) Model overhang** — a solid block lies within `MODEL_MARGIN` (0.2 blocks) of the hitbox horizontally, for a body **at rest** only. Vanilla mob models render past their collision box (a warden's arms, an iron golem's, a ravager's horns, a sheep's wool), so a flush body *looks* embedded although nothing overlaps; the true per-model extent is client render geometry the compiler has no data for, hence a named margin rather than a verdict. The margin is also what makes the tier discriminating: a body leaves `(1-width)/2` of its cell free per side, so 0.2 fires for a 0.9-wide warden or sheep (0.05 free) and stays silent for a 0.6-wide player-model humanoid (0.2 free) — an NPC standing against a wall, the most ordinary staging there is, produces nothing. It is restricted to bodies at rest deliberately: a body at rest is a composed pose the party looks at, while a walker in a one-block corridor is within a fraction of a block of both walls by construction, so flagging legs would report the map's dimensions once per leg. **(2) 1.5-tall barriers** — a fence, wall or closed fence-gate cell falls inside the body volume. Those fill their cell for pathing but are a narrow post or panel in reality, so whether the body interpenetrates depends on sub-block shape the occupancy model does not carry. Prescription: give the body a cell of clearance, or confirm the framing in playtest. |
| `DW0452` | A walked leg's route contains a **move the body walking it cannot make**. Build-tier (exit 3), `compiler::traversal`. The owner's island round-21 finding B: `[18, 73, -63]` shipped `minecraft:oak_fence_gate[facing=east,open=false]` in the mountain pen's south fence line, and sixteen `move-npc`/`move-actor` legs walked straight through it — while the owner's own character could not, and had to offset to squeeze past the leaf. **Why nothing stopped it**: `nav::World::is_occupied` deliberately excludes `use_gates`, because a closed fence gate *is* passable — for the PLAYER, who opens it with an adventure-legal right-click (`World::without_gate_use` exists precisely because an autonomous mob cannot), and scripted walks were routed on the player's rules on the stated ground that "the beat's fiction controls the gate". Nothing proved that fiction. A scripted walk is a compiler-emitted `tp` polyline whose puppet performs no interaction at all, and **no runtime verb changes a fence gate's block state**, so a gate that ships `open=false` is shut for the whole delve. Model: capabilities come from the entity (`traversal::Traversal::of_entity`) rather than from a global rule — `opens_gates` is false for every mob, since no vanilla mob opens a fence gate (villagers open *doors*). Routing itself is unchanged: the edge stays available and the build now fails on it, which names the cell and the reason instead of turning it into an unroutable `DW0307`. **No locomotion class is exempt from this rule** (owner correction, round 21). `DW0452` is a COLLISION-AND-INTERACTION question, not a locomotion one: the gate leaf spans the full cell across one axis, the planned route runs down the cell's centre line, and the puppet performs no right-click — and not one of those three facts changes because the body has wings or claws. A flying body may skip the *climbing/surmounting* checks; the *collision* check it still owes. The only thing that can excuse this rule is `Traversal::opens_gates`, which is why that is a per-body field and not a constant, and why the exemption is expressed **per rule** rather than as an early skip over the whole body — an earlier draft did the latter and let a flier walk through a closed gate in silence. A leg reports its first offending cell only, and all violations land in one message. Prescription: ship the gate OPEN (a stage-7 `world-edits` fill writing `open=true` on the cell — an open fence gate has no collision at all, so the same route becomes honest for puppet and player alike), or seal the threshold and let the route take the way a body can. |
| `DW0453` | Advisory (exit 0), same module: a walked leg goes **over a barrier line, across a full-cube course of it**. The route steps up onto a cell whose support is a full cube standing level with, and orthogonally beside, a 1.5-tall fence/wall cell, and comes back down within `traversal::SURMOUNT_WINDOW` (4) route steps — i.e. the body crossed a line the same line refuses to let it walk through. The owner's island round-21 finding A: the beach fold's ring is `minecraft:cobblestone_wall` down the east and west sides and at the north corners but full-cube `minecraft:mossy_cobblestone` along the middle of the north and south edges, so the model sees an enclosure at nine cells and an ordinary one-block ledge at five; the flock's shortest way out ran up the east face at `[7, 63, -9]`, over the north wall's top at `[7, 64, -10]` and down into the meadow, and the pen's real opening at `[6, 63, -6]` was never used. Twelve legs, all naming the same course. With `nav::resample`'s L-shaped step-up — a vertical translation in place, which is what keeps a body out of the step block's corner — this renders as an animal sliding up a stone wall. **Advisory, not an error**: the move itself is legal (a one-block rise is inside the player-class jump every body in the dims table has), and the compiler cannot tell a decorative kerb or a deliberate stile from an enclosure that was meant to hold. A partial floor (slab, `dirt_path`) beside a fence is never a course — that is floor detail, not a wall. **This is the rule locomotion legitimately governs**, and the only one: a `Locomotion::Climber` is exempt because going over is what a climber does, and a `Locomotion::Flier` because it makes no ground step-up in the first place. This advisory tier is also the only tier a hand-listed class is permitted to gate, so a misclassified species costs a missed advisory and never a missed error. Prescription: build the line out of ONE material so the model's barrier and the player's eye agree, and let the route use the opening — or, if this body really is meant to go over walls, DECLARE it (`traversal`, DSL v0.11 / spec-0034), which is not a way to switch this line off but a claim the build then holds the body to (`DW0454`). |
| `DW0454` | **A body's `traversal` declaration is INERT** — it changed no rule's verdict, so nothing in this build holds the body to it. Build-tier (exit 3), `compiler::traversal`, DSL v0.11 (spec-0034). **Why this exists.** Spiders really do climb, so `DW0453` cannot be absolute; the author's side of that is the per-body `traversal` declaration. But a declaration that only silences a diagnostic is worse than no declaration — it converts a check into an opt-out — so the declaration is required to be PAID FOR. Model: for every declared body the compiler computes the findings its legs earn under the DECLARED class and under the class its entity id implies, and the declaration is *exercised* only where the two differ. Written as a difference of verdicts rather than as "is it a climber", so a second locomotion-governed rule joins the test by existing. Three inert shapes, each named in the message because the fix differs: the declared class is the one the species already had; the body walks no leg at all; or every leg it walks earns identical verdicts either way (no route of its goes over a barrier line, the only move locomotion governs). **What it is deliberately NOT**: a way to reach the error tier. `DW0452` has no authorable exemption at all — `Traversal::opens_gates` is derived, never declared, and no locomotion class is exempt — so a declared climber walks into a closed fence gate and the build still stops. All violations land in one message. Prescription: remove the declaration, or build the world that needs it — give the body the route that really makes the move, and the declaration is then what makes that route legal instead of a finding. |
| `DW0455` | **A declared locomotion the engine cannot hold the body to** — today exactly `aquatic`. Numbered in the 045x body family but **validation-tier (exit 1)**, like `DW0320`: it is raised by `dsl::validate_campaign_with` at pipeline step 3, refused at declaration time rather than accepted and ignored. `aquatic` is the one class that carries no exemption and governs no rule (it is a ledger label read off vanilla's own `#minecraft:aquatic` tag), so declaring it could never change a verdict and would land in `DW0454` every time; a value whose only possible outcome is another diagnostic is a trap, not a surface. The message NAMES the gap rather than leaving it to folklore (CLAUDE.md no-hack rule): routing has ONE reachability model, standable ground, and water-flooded cells are impassable and never floor for every body, so there is nothing for an aquatic claim to feed. When routing grows a water model, this refusal is what has to be deleted to enable the value. Prescription: remove the declaration — a route that crosses water is already governed by the flooded-cell rules, and a body vanilla itself calls aquatic still reaches the binding ledger under its derived class. |
### DW0489 — crosshair disambiguation (`compiler::crosshair`; error + advisory)

| Code | Meaning |
|------|---------|
| `DW0489` | **Two bodies the party has to click stand close enough that the crosshair cannot tell them apart** (owner island QA, terminal finding). Two crew NPCs were staged onto one cell at the cave mouth; a human could not aim at the one carrying the decision, the beat never opened, and the delve soft-locked with the whole machine ladder green — green because the bot interacts by *entity id* and never casts the ray a player casts (the harness half of the same fix is `harness/src/crosshair.ts`). The campaign said it outright: `quest/follow-the-smoke` declares `npc/eurylochus` **and** `npc/antiphos` both `at: anchor/mouth`. **Why nothing saw it:** `DW0359` compares a body against an *affordance*, never against another body — an NPC's own dialogue hitbox is not in its affordance list — and it applies the parked-body rule, skipping every NPC the campaign `move-npc`s, which both of those crew are. **Model:** the DSL v0.7 cast ledger (`DW0461` already proves it equals the position the effect history produces) is a checked roster of who shares a scene, so co-presence is read, never inferred: two placements share a scene when they are declared in the same quest and no flag proves them exclusive (one's `requires_flags` meeting the other's `forbids_flags`). Widths come from `nav::entity_dims` over the body that ships (`nav::npc_body_entity` — a skinned NPC is a 0.6-wide `minecraft:mannequin`). Pairs whose vertical spans do not overlap are silent: aiming up or down separates them from every azimuth. **Threshold, derived from vanilla 1.21.11 geometry alone** — `GameRenderer.pick` traces from the eye to `player.entity_interaction_range` (3.0 blocks) and `ProjectileUtil.getEntityHitResult` returns the first box the ray meets, inflated by `Entity.getPickRadius()` = 0.0 for every staged body, so there is no tolerance to hide in. The player is a body too (0.6 wide), so its eye can never come nearer than `(0.6 + w_t)/2` to a target's centre; the other body is provably out of the ray, from *every* azimuth, only when the eye is nearer than its near face, `d < s − (w_t + w_o)/2`. Such a stance exists exactly when `s ≥ τ = (0.6 + max(w_t,w_o))/2 + (w_t + w_o)/2` — **1.2 blocks for two humanoid bodies**, and the stance it guarantees sits 0.6 blocks out, far inside reach, so "provably clear" and "close enough to click" never pull against each other. Below τ every stance that can reach the target lies at or beyond the other's near face on some azimuths, and whether a clear one survives depends on walls this proof does not model — which is exactly how the island's crew ended up unclickable. `s = 0` is the degenerate case: coincident boxes make the pick an exact tie the client resolves by iteration order, so *which* body answers is not decidable from the campaign at all. **Two tiers, one code**: **error (exit 3)** when either placement's right-click opens a dialogue **root** — the ledger's own word for a consequential tree, where every `talk-to` objective and branch choice lives; **advisory (exit 0)** when both are barks or `none`, since the bodies are just as ambiguous but no beat is riding on the click. `compiler::crosshair`, build-tier, run beside `DW0359`/`DW0422`. **Boundary:** NPC-vs-NPC over the ledger only, and therefore silent for a pre-0.7 campaign that declares no ledger — without a roster, co-presence is unknowable and the compiler withholds rather than guesses (`DW0465` owns that deprecation window). Body-vs-affordance at rest is `DW0359`'s rule and affordance-vs-affordance is `DW0878`'s; neither is re-litigated here (one code, one rule); actors carry no ledger entry, so a puppet parked in front of a speaker remains nobody's rule and is a named blind spot. Prescription: move one of the two cast anchors. **Never** make either body intangible or non-pickable to let clicks through — a body the party cannot click is a character they cannot talk to. |
| `DW0878` | **Two interaction affordances stand on one cell.** Two `minecraft:interaction` boxes the party clicks, `1.0 × 2.0` at the same cell centre, therefore identical: any pick ray enters both at exactly the same distance, the client resolves the tie by entity iteration order, and which of the two answers is not decidable from the campaign at all. **A left-click and a right-click do not divide the cell** — vanilla picks the entity before it reads the button, so a `strike` trigger sharing a cell with a rest point loses its swings to that rest point's entity exactly as two right-clicks would. **Why nothing saw it:** each of the three neighbouring rules needs one side of its pair to be something else — `DW0359` a standing body, `DW0422` a compiler-owned press set, `DW0489` two NPCs in the cast ledger (whose whole model is scenes, flag co-presence and a tier read off a dialogue root, none of which an affordance has). Affordance-against-affordance was nobody's rule, and the engine's own gallery shipped it: `obj/press-the-case` and `trigger/read-the-label` both on `anchor/pedestal`, plus a bonfire and a `strike` trigger on `anchor/hearth` and three affordances on `anchor/vantage`. The build was green; `validation/bot-run.sh` failed at step 2, before any combat, reporting that the crosshair could acquire neither box. **The predicate is exact coincidence and deliberately nothing wider:** boxes that merely overlap (two affordances a block apart vertically share a one-block band) are entered at different distances from every stance and a player aims past them, so refusing those would be a false certainty — the same line `DW0422` draws by testing the cell rather than the emitted box. **And a cell is not yet a contest: the pair must be able to share a MOMENT.** The first cut of this rule said sharing a cell was enough and refused the released `nobodys-cave-island`, whose two branch endings each hang an `interact` objective on the galley's deck — `obj/board-flee` in `quest/take-the-cheese`, `obj/board-nobody` in `quest/the-sail`, two arms of one story that no player walks together. It was asserting co-presence it had not established, and the content-build gate is what caught it. The arming windows are read off the emitter rather than modelled: a trigger body, bonfire, shortcut unlock, disarm lever or shop is placed at setup or by a beat that never retires it, so it shares a moment with anything; an `interact` objective's box is summoned under `emit::pending_guard` (quest active, `after` set complete, own gate open) and killed by `emit::completion_cleanup` on completion. So two objective boxes are judged only when one quest declares both — one quest is active as a whole — and then only if no flag proves them exclusive, the same test `DW0489` applies to two NPCs the cast ledger puts in one scene. Across quests, co-presence is **unestablished and the compiler withholds**: a named blind spot in the same family as `DW0359`'s parked-body rule, closing which needs a quest-co-activation model this compiler does not have. **Merging is not the remedy**, and that is a fact about lifetimes rather than a preference: a trigger rides an existing hitbox set only where the compiler owns that set for the whole run (a `close-gate` seal, a sealed shortcut door, an NPC's own dialogue body), while an `interact` objective's box is summoned when its quest activates and killed the moment the objective completes and a bonfire's is armed by the beat that places it — riding either would hand the trigger a lifetime its own declaration never asked for, silent at both ends. Pure cell arithmetic over `compiler::eclipse::affordances`, the same single authority `DW0359` and `DW0422` read, so an affordance class added to the engine enters this proof by existing. `compiler::eclipse::check_affordance_contests`, build tier (exit 3), `every_version`, run beside `DW0359`/`DW0422`/`DW0489`. **Binding: the affordance set it examined**, stated by `eclipse::affordance_contest_binding` — a campaign with one affordance has no pair and passes for free, which from outside is the same silence as one whose forty all stand clear. **Ordering:** `DW0425` is raised before it, because a shortcut whose sealed side is undecidable stops its `use` trigger riding the door and so manufactures a tie that is a consequence of the real fault. **Boundary:** affordance-against-affordance only; a body over an affordance stays `DW0359`'s rule and an affordance inside a pressable body's cells stays `DW0422`'s. Prescription: give one of the two its own anchor, one cell clear. Never make either box non-pickable to break the tie — an affordance the party cannot click is a beat they cannot reach. |

### DW0496 — daylight-burning staging (`compiler::daylight`; error)

| Code | Meaning |
|------|---------|
| `DW0496` | **A body vanilla burns in daylight is staged for a fight the sun can reach.** Every other rung stays green on that shape: `DW0312` proves the wave has footing, `DW0311` that the room is reachable, spec-0023 that the fight is winnable, and the liveness census lets a wave that dies to *anything* still close its objective — which closes the soft-lock and, deliberately, not the encounter. **Fires when all five hold:** (1) the entity type is in vanilla's own `#minecraft:burn_in_daylight` tag and is not fire-immune; (2) the party is meant to fight it — a `kill` objective adjudicates the wave, or it is an actor they can damage (`vulnerable`, or unleashed; an `Invulnerable` puppet takes no fire damage at all); (3) the delve is pinned to a **clear daytime** hour for its whole length; (4) open sky stands on walk-reachable ground within one aggro radius of where it is staged; (5) the head slot is empty. **The species list is Mojang's, never ours.** `#minecraft:burn_in_daylight` is a built-in vanilla `entity_type` tag — since 1.21 the thing the engine itself tests before running a mob's sun-burn tick — vendored verbatim from the same pinned misode/mcmeta summary as the item registry (`crates/dsl/data/entity-tags-1.21.11.json`, regenerate with `tools/extract-entity-tags.py`; `data/PROVENANCE.md`). For 1.21.11 it holds `skeleton`, `stray`, `bogged`, `wither_skeleton`, `zombie`, `zombie_villager`, `zombie_horse`, `drowned`, `zombie_nautilus`, `phantom` — and **not** `husk` or `zombified_piglin`. Writing that list by hand would be the invented vanilla data this codebase already refuses for mob health (`DW0475`) and aggro range (`nav::DEFAULT_FOLLOW_RANGE`). The tag says which types *run* the burn tick, not which types the fire then *hurts*: fire immunity is a hardcoded entity-type property no vanilla data branch publishes, and exactly one tag member has it — `minecraft:wither_skeleton`, a Nether native ("the Nether-native undead mobs, which are entirely immune to fire", minecraft.wiki/w/Undead) — so `daylight.rs` carries that single exclusion explicitly with its citation. **The hour.** Only `day` and `noon` burn: `dusk` (12000) is the sun already down-going and `dawn` (23000) is before sunrise, both held non-burning exactly as `light::effective_sky` holds them at the night floor. Only `clear` burns: vanilla's `isSunBurnTick` is gated on the mob not being in water or rain. The daylight cycle is frozen (spec-0010), so a campaign that declares a burning hour and never `set-time`s is burning at every beat and needs no per-beat timeline; a campaign that DOES cut its time or weather has such a timeline, is not modelled, and stays silent (withhold, never invent — the same direction `DW0489` takes on a missing ledger). **The reach.** The mob need not spawn in the light: it holds its target while the player stays inside its `follow_range`, so a retreating player drags it as far as the player walks. A moving chase is not modelled, so the compiler asks the weaker decidable question — *is there open sky within one aggro radius of where this stands, on ground it can walk to?* One radius is the shortest lure that provably exists, so the rule under-fires by construction. Radius = the stack's declared `attributes.follow_range`, else `nav::DEFAULT_FOLLOW_RANGE` (the same reading `DW0380` and `DW0478` take — one documented number, never a per-species table). Walk-reachability is `nav::World::reachable_walkable` from the **seated** spawn cells (`emit::plan_wave_spawns`, so the measurement starts where mobs actually land, not at an anchor they stand around) over the assembled + stage-7-edited world, and is deliberately **unbounded**: getting there is geometry, the radius is perception. Bounding the walk would have been green on the motivating incident — Barrowmere's yard is 15.6 blocks from the muster room but 21 steps of corridor away. Sky exposure is `light::LightModel::sky_open`, the same column test spec-0010's relight seeds sky light with, so "the sky is above this cell" has one definition in the compiler. **The helmet, and the one species it does not save.** Vanilla's burn tick checks the head slot first and damages the helmet instead of igniting (minecraft.wiki/w/Zombie), which is why `equipment.head` is the sanctioned remedy, recorded on the DSL field itself. `minecraft:phantom` is the exception and it is explicit — "They burn even when equipped with helmets through commands" (minecraft.wiki/w/Phantom) — so a helmeted phantom is still `DW0496` and its message prescribes roofing instead: a prescription that does not work is worse than none. `compiler::daylight`, build-tier (exit 3), run right after wave seating. **Boundary:** waves a `kill` objective adjudicates and actors the party can damage; a wave nobody is asked to kill is a difficulty question, not a broken encounter. Flight is not modelled — a phantom is tested over walkable ground, which can only under-fire. Prescription: give the stack `equipment.head` (any head item; drop chance 0 is emitted for you), or roof the ground the fight happens on. **Never `set-time`** — the delve's hour is a pacing decision the author made, and moving it to save a mob spends a beat. |

### DW0498 — pool double draw (`compiler::pool`; advisory)

| Code | Meaning |
|------|---------|
| `DW0498` | **A pool draw seats the same anchor-bearing prefab more than once, so every anchor that prefab declares has more than one carrier**. An anchor name belongs to a *prefab*, not to a placement: seat the prefab twice and the name stops picking out a place in the world. The compiler already refused the sharp end of that — `DW0305` fails the build when a campaign-referenced anchor resolves to two placed pieces — but `DW0305` fires **per anchor, at the use site**, and only over the anchors the solver is required to guarantee (NPC stands, `reach-anchor`/`collect`/`interact` targets, `open-gate`/`close-gate`/`set-block`/`move-npc` anchors, wave spawns, lane waypoints, cutscene subjects). Everything else — a `spawn-actor`, a `move-actor` destination, a block or light edit — resolves silently to the **first** carrier in placement order (`Plan::build`'s `or_insert_with`) and leaves the other copy empty. The pool that caused all of it said nothing at all, so a campaign author discovered the constraint one blocked placement at a time. The motivating case ships: on the island, `pool/island` (4 members: 1 `entry`, 2 `connector`, 1 `terminal`) at `pieces {min:4,max:4}` seats `prefab/island-greenfield` **twice**, which makes all nine of its anchors (`anchor/fold` … `anchor/meadow`) ambiguous and unusable for wave or reach placement. **What it asserts:** facts about *this build's assembled draw* — the pieces the pinned seed actually seated (ADR-0006), read **after** stage-7 massing so what is reported is the layout the player gets. It never claims a pool "always" repeats; the same pool at a different budget or member set may not, which is exactly why the prescription is to change the pool and never to reroll the seed. **Anchorless fillers are excluded by construction**: repeating an anchorless connector is *how* a jigsaw pool spans its `pieces` budget (`pool/stone-keep`'s corridors exist to be drawn over and over), and a prefab that declares no anchors can make no anchor ambiguous — warning on every campaign that uses fillers would be noise, not information. **Severity: advisory (warning, exit 0), deliberately.** A repeat with no ambiguous-anchor *use* is legal and shipping content relies on it, so this never turns a green campaign red; when such an anchor IS referenced, `DW0305` still fails the build at the use site and this warning is printed with it as the pool-level explanation (carried on `PlanError::warnings`). `compiler::pool`, run in `Plan::build` right after the solver and massing; reported through `Plan::warnings` → `emit::build_with_warnings`. **Boundary:** one diagnostic per pool area, naming every repeated anchor-bearing prefab and every anchor each one makes ambiguous. It says nothing about two *different* prefabs that happen to declare the same anchor name — that ambiguity exists at a single draw and is `DW0305`'s alone. Prescription: give the pool more DISTINCT variant members in the repeated role (same sockets, different prefab) so a draw of this size never has to reuse one piece, or accept those anchors as unusable and keep every placement off them. **Never reroll the seed** to change the draw. |

### DW0494 — branch-aware inter-area transport (`compiler::emit`; error)

`build_critical_path` derives an inter-area transport map for whatever
playthrough it is handed, so every branch already has one
(`Plan::branch_critical_path`), and `validation/branch-path-<slug>.json`
publishes it to the harness. Emission carries them too: the
**exported** path's crossings are emitted unconditionally in the objective's
`complete_<obj>` bundle, and every crossing that exists only on a BRANCH is
emitted beside them, gated on exactly that branch's flag assignment (`if score
#party dw.f_<set> matches 1` / `unless … dw.f_<unset> …`). Before this, a
branch-only crossing was promised by the artifact and performed by nothing — the
island round-21 branch run walked to a deck it was never carried to. The overlay
is empty, and the emission byte-identical, for a campaign with no `branch_points`
or one whose branches cross only where the exported path already does
(`emit::branch_transport_overlay`).

| Code | Meaning |
|------|---------|
| `DW0494` | **One objective, two destinations.** Completing a single objective would cross into a different area on the exported path than on a branch. Build-tier (exit 3), `compiler::emit::branch_transport_overlay`, raised before any function is emitted. The crossing lives in that objective's own completion bundle, so the two teleports would sit in one function body and command order — not the branch the party is actually playing — would decide where they land; and there is nothing to gate on, because the exported path's crossing is unconditional by construction. The message names the objective, both destinations and the branch that disagrees. Prescription: split the crossing into one objective per branch, each gated by that branch's flags (which is what the two branches' beats already are, everywhere else). Do NOT move the branch's destination onto the exported path to silence it — that ships the branch to the wrong area. |

### DW046x — the NPC scene ledger (`compiler::cast`; spec-0020, DSL v0.7)

The `cast` block declares, per quest, where every live NPC is, what they are
doing, and what their right-click offers; these proofs compare that declaration
against the effect history [`compiler::continuity`] replays, and the resolved
declaration is what the emitter dispatches on. Validation-tier (exit 1) except
`DW0465`/`DW0467`, which warn (exit 0). `DW0846` and `DW0858` were numbered later and belong to
this group.

The motivating defects are both owner playtest findings on the island: a crew NPC
still offering premise questions ("Tell me what he is.") after the climactic
escape, because an NPC had exactly one dialogue tree for the whole campaign
(round 12); and two crew NPCs left standing forgotten in the stealth alcoves
while the player escaped (round 8), because the compiler's per-NPC effect history
was never compared against anybody's stated intent.

| Code | Meaning |
|------|---------|
| `DW0460` | **Completeness (proof 1).** A stage-2 NPC that is live when a quest opens — on stage, or with a branch-dependent position — has no entry in that quest's `cast`. Prescription: say where they are and what they are doing, or remove them from the world with `despawn-npc` and declare them `"offstage"`/`"dead"`. An NPC nobody placed is exactly how the round-8 alcove crew survived every other check. |
| `DW0461` | **Placement consistency (proof 2).** A declared `at` contradicts where the replayed effect history actually leaves the NPC when the quest opens — an anchor mismatch, a declared anchor for somebody not in the world, or a declared `"dead"`/`"offstage"` for a body still standing. The message cites both anchors. Declaring a position does not teleport anybody: prescription is to stage the `move-npc`, or declare where the NPC actually stands. Skipped for a branch-divergent NPC, which `DW0462` handles instead. |
| `DW0462` | **Branch honesty (proof 4).** An NPC whose position when the quest opens is branch-dependent (its lifecycle is driven from a dialogue option, a flag-gated effect, an environment trigger or a reaction bundle — `continuity`'s exclusion set, and the message names which) carries a single flat cast entry. One declaration cannot hold on every reachable branch, and merging optimistically is how a ledger starts lying. Prescription: declare per-branch casts — a **list** of placements, each gated by the `requires_flags`/`forbids_flags` that select its branch. Note the exclusion is campaign-global, so an NPC any branch touches needs per-branch casts in *every* quest, not only after the fork. |
| `DW0463` | **The forcing function.** An on-stage placement omits `doing` or `dialogue`, or an `"offstage"`/`"dead"` placement declares them. `doing` is free prose the compiler never checks and is required anyway: you cannot fill it without deciding the character's business in this beat, and stage 6 writes their lines against it. `dialogue` is required so silence is always a *choice* — an omitted field is this diagnostic, never an implicit carry-forward. |
| `DW0464` | **Dangling cast ref.** The entry names an NPC stage 2 never declared, a `dialogue` root that is not a node of *that* NPC's stage-6 tree (right-click would open nothing), or an empty `barks` pool (silence dressed as an answer — write a line, or declare `"none"` if the silence is the point). |
| `DW0465` | **The pre-0.7 deprecation window** (**warning**; exit 0). A campaign below `dsl_version 0.7.0` declares no `cast` ledger anywhere. One finding per campaign, not per quest. It keeps building for one version window; then the requirement hardens into an error. Note the asymmetry: the window forgives the *absence* of a ledger, while *declaring* one below v0.7 is `DW0141` like any other newer construct. |
| `DW0466` | **`"unchanged"` with nothing to carry.** The `"unchanged"` keyword resolves to whatever the NPC's dialogue was at its previous appearance in the quest-DAG ordering, and it is used at that NPC's **first** appearance. Prescription: declare a real root id, a `barks` pool, or `"none"` here. The keyword exists so that carrying dialogue forward is a conscious, declared act rather than an implicit default — which is why it cannot bootstrap itself. |
| `DW0467` | **Dialogue staleness** (**warning**; exit 0). An NPC appears in 2+ quests' cast ledgers and offers the same thing in every one — the same root throughout, whether spelled as a repeated root id or as `"unchanged"`, or `"none"` throughout. Its right-click never learns that the story moved: the "one tree from beginning to end" shape the ledger exists to surface. Prescription: give it a scene that changes (a later root, retired options), or — if it really is a background character — a `barks` pool, which is **exempt** because a bark pool never claims to advance anything. Warning, not error: a genuinely static minor character is legal, the author just has to see the flag. Silence that makes an objective impossible is not this lint's question and is refused by `DW0858`. |
| `DW0858` | **An objective that cannot be completed.** A `talk-to` objective whose NPC's `cast` ledger, at every scene it can present while that objective is live, opens nothing that completes it — a silent scene, a bark pool, or a root whose tree never reaches an option with a `complete-objective` effect for it. Validation tier (exit 1), `every_version`, `compiler::cast::check_talk_answerable`. The player walks up to the beat, presses, and it does not pass: a `"none"` scene consumes the interaction and emits no clause, and a bark pool never claims to advance anything. **The refusal is the CONJUNCTION, never the silent scene.** An NPC with nothing to say during a quest whose beats do not go through them is ordinary and correct; what is refused is a quest that asks the player to talk to somebody it has itself declared silent. **Why the three checks next door are all green on it.** `DW0123` measures reachability from the tree's ENTRY POINTS — the stage-6 `root` plus every ledger root, all at once — so it answers *does a completing option exist in this tree* and never *is it what right-click opens during this beat*; the option really is there, which is why the coverage check passes. `DW0467` is a whole-story staleness lint and a warning, it fires only when an NPC offers the same thing in EVERY quest, and it says nothing about whether an objective depends on the conversation. And the deep fixpoint's `DW0203` is cast-blind by construction: it is monotone and has no notion of *when*, whereas the ledger is entirely about when — `compiler::flow` consults the ledger only in its replay (`skips`/`DW0205`), the one place a moment exists. **Which scenes count as live** is bounded by two facts about the emitted dispatch rather than guessed: clauses accumulate in `quest_dag_order` and the last begun one wins, so a clause ranked before the objective's own quest is always overridden by that quest's own; and `dw.qa_*` is set when a quest STARTS, so a quest that only opens once this objective's quest completes cannot have begun while the objective is pending. Everything else in the ledger counts, which keeps the rule permissive where it cannot see the branch. Inert on a campaign with no ledger — there is no scene to be wrong about, and `DW0123` keeps its old verdict. **Binding: `talk-to` objectives whose own quest declares their NPC.** Prescription: give the NPC a `dialogue` root in that quest's `cast` whose tree reaches the completing option, or drop the objective. |
| `DW0846` | **A clause no runtime state can select.** Its own gate is satisfiable, yet at every state that satisfies it some LATER clause of the SAME quest also passes and, later clauses winning (the retirement mechanism), always overrides it — so the scene it declares is unreachable **by construction**, at every point of its governing window (later quests can only override further, never help). Decided by the same complete solver the generated `cast_ladder_<npc>` proof drives its phases from (`cast::distinguishing_drive`): per later same-quest clause the only choice is *which* term to violate, the choices are walked exhaustively, and per-datum emptiness is exact interval arithmetic (`dsl::gate::DatumSet`) — so "no distinguishing state" is a theorem about the ladder, never a search giving up. The worked shape is ordering: a per-branch entry lists its fallback FIRST and its gated branches after; written the other way round, the unconditional fallback sits last and shadows every branch. Prescription: reorder the placements, or tighten the later gate so this branch has a state of its own. A clause whose own gate is self-contradictory is `DW0847`'s finding at the gate site and is not double-reported here. |

#### Declared difficulty (v0.6)

| Code | Meaning |
|------|---------|
| `DW0468` | `world.difficulty` is `peaceful`. Refused, not honoured: on peaceful the server calls `checkDespawn` on every entity as it ticks it and **discards every hostile-category mob** — being `/summon`ed, `NoAI` or `PersistenceRequired` saves none of them — so every wave, hostile actor and ambush in the campaign would silently cease to exist. The keyword parses (it is a variant precisely so this diagnostic can exist instead of a serde "unknown variant") and validation rejects it. Validation-tier (exit 1), `dsl::validate`. Prescription: declare `easy`, `normal` or `hard`; for a genuinely combat-free delve, omit `difficulty` entirely — a campaign with no waves already ships peaceful by derivation. |
| `DW0469` | (**warning**; exit 0) A campaign stages actors meant to **fight** — unleashed into a real-AI twin, or declared `vulnerable` — but declares no `waves[]` and no `world.difficulty`, so it ships the derived `difficulty=peaceful` and a monster among them is discarded on the tick it spawns. "Meant to fight" is read off the campaign's own declarations (`unleash-actor`, `vulnerable`), never guessed from the species: the pinned entity registry is a membership set with no mob-category data, so *is this a monster* is exactly the question the compiler cannot answer — which is why this is advisory. Prescription: declare `world.difficulty`. |

### DW0860–DW0863 — an objective keeps the promise its prompt makes (`compiler::promise`; error; exit 1)

An objective carries exactly two player-facing strings — `title` and `hint` — and
these four rules are the whole of what the compiler asserts about them. They exist
because four playtest findings on two campaigns turned out to be one defect class:
what the game tells the party and what the machine requires are not the same thing.

One emitter fact is load-bearing under three of the four and is not visible in the
schema: the activation announcement is emitted **only for a titled objective**, and
the hint's line is nested inside that guard. An objective with no `title` therefore
announces nothing at all — no chat line, no cue sound, and its wayfinding marker is
summoned nameless — and an objective with a `hint` and no `title` shows *neither*.

`DW0862` is `every_version`: it judges what the document says — a `hint` that
asks to be shown and an absent `title` that guarantees it will not be — which is
`Binds::EveryVersion`'s own category, *a contradiction between two authored
fields*, and a document with no hint cannot violate it at any version. The other
three **require the campaign to have something**, so they bind from
**`dsl_version` 0.8.0** (`promise::PROMISE_SINCE`) and every campaign below that
is grandfathered. 0.8.0 is not the number that made the fixtures pass — anything
above 0.6 would have — but the version at which `DW0481` first required an
objective to declare a `happening`, *what this beat does to the story*: these
three are the same forcing function turned to face the player, so from 0.8.0 an
objective owes an account of itself in both directions and below it owes neither.
Two documents in this repository are grandfathered by it and both are engine
fixtures that predate the announcement surface: `keep-vertical` at 0.3.0 and
`souls-td-lanes` at 0.6.0, each carrying an unsigned `kill` objective.

Every run prints a binding line naming what it examined, zeroes included.

| Code | Meaning |
|------|---------|
| `DW0860` | (binds from `dsl_version` 0.8.0) **A failure clock nothing explained.** A `begin-stealth` with a non-empty `on_caught` is a clock: a player outside every zone for `grace_ticks` is punished by that bundle. It is refused when no `narrate` fires at or before the arming in the same effect bundle, or when the interval between the last such line and the clock's first bite is shorter than that line takes to read. The interval is `(arming offset − prompt offset) + grace_ticks` on the arming's own timeline, so a `sequence` step's `at_ticks` counts toward it; the reading floor is `20 + 2 × characters` ticks — one second to appear, then ten characters a second, which at five characters a word is 120 wpm. **The prompt the clock races is the LAST one before it**, not the sum of the bundle and not the longest: three mutually exclusive branch retellings of one beat are ordinary authoring and must not accumulate into a refusal, and the line still being read when the clock starts is the instruction. Prose in `on_caught` does not satisfy it — that line is read after the punishment. **Binding: `begin-stealth` effects with a non-empty `on_caught`**, over every effect root (the population is enumerated from `dsl::for_each_effect_root`, which asserts it reached all of them, never from a remembered list of places effects live). A `timed-gate` is deliberately out of scope: it arms at world load, not at a beat, so no prompt can precede it — what the party is owed there is the chance to watch it (`DW0388`) and a window worth reading (`DW0378`). Prescription: put a `narrate` before the arming saying what is now being asked, and give the clock time for it. Do NOT shorten the line to fit the clock. |
| `DW0861` | (binds from `dsl_version` 0.8.0) **An adopted container nothing distinguishes.** A `collect` that sets `container` adopts a chest or barrel the prefab already placed, and is refused without both a `title` and an `item_name`. Adoption is what creates the ambiguity: the compiler's own chest at `anchor` is a new object that appears the tick the objective activates, whereas an adopted container is scenery the party has been walking past since the beat began, identical to every other barrel the piece put down, and the compiler adds nothing to it. The announcement and the name on the stack inside are then the only two things that can tell one box from its neighbours. **Binding: `collect` objectives with `container` set** — a `collect` that conjures its own chest is not this rule's subject. Prescription: give the objective a `title` and an `item_name`. Do NOT reach for `fill_count`: padding makes the right box read full, it does not say which box is right. |
| `DW0862` | **A prompt the emitter will never show.** An objective authors a `hint` and no `title`. Because the announcement is guarded on the title and the hint is nested inside it, the party is shown neither — while the hint is still inventoried for translation and rendered into every language sidecar, so nothing anywhere reports that a translated, shipped line is never on a screen. **Binding: every objective.** An objective with *neither* string is not refused: this is a rule about a prompt that was written and is not shown, not about one nobody wrote. Prescription: give the objective a `title`; the hint is an announcement's second line, not an announcement. Do NOT delete the hint to clear it — that silences the beat rather than fixing it. |
| `DW0863` | (binds from `dsl_version` 0.8.0) **A fight nothing points at.** A `kill` objective without both a `title` and a `hint`. A fight is the one objective kind the compiler leaves nothing in the world to find: `activation_commands` emits no command for it, no marker is summoned, no prop is placed, no name is written, and the render plan falls back to the literal phrase `the fight` because none exists. Every other kind leaves something standing — a `reach-anchor` a glowing end rod, an `interact` a lantern or its authored prop, a `collect` a chest, a `talk-to` a named body. So the objective's own two lines are the only thing that can say where the wave arrives. **Binding: `kill` objectives.** Prescription: write both lines, and let the hint say where. Do NOT rely on the wave's anchor being near the previous beat — nothing proves that, and nothing shows the party an anchor. |

**What these deliberately do not claim.** The ledger's general form behind `DW0862`
is *an objective's prompt names the place and the act that actually complete it*,
and that is not what is proven: a machine cannot read prose for whether it names
the right place. What is proven is the necessary condition — that the prompt
reaches a player at all. The stronger reading was built and measured: keying "the
place" to the objective's quest's declared `area` finds three objectives on a live
campaign, and all three are correct — a quest booked in one area whose objective is
*travelling to* the next names the destination on purpose. A quest's `area` is
where the beat is booked, not where each objective completes, so it is not a sound
proxy for the place. Likewise `DW0861` proves that the party is told what to look
for and what they have found, not that the target is visually distinguishable from
its neighbours; and the ledger's second disjunct — *or every copy satisfies it* —
is not expressible at all, because `container` names one anchor.

### DW047x — combat winnability (`compiler::combat`; spec-0023)

The arithmetic half of spec-0023's three combat proofs. The ruling behind them:
"the average player can win" was never a provable claim and is no longer
pretended — the machine proves a fight is REACHABLE, RETRIABLE and
**structurally winnable**, and leaves human skill open on purpose. These are the
structural half; the retry loop and the assist windows belong to the bot ladder
(spec-0003 / §8 below).

Runs only for a campaign with at least one `kill` step on the compiled critical
path, over the SEATED wave-spawn cells (`plan_wave_spawns`, so it reasons about
where mobs actually land rather than where the anchor is). Build-tier (exit 3)
except `DW0474`/`DW0475`, which warn (exit 0).

**Every number is Mojang's own, or the answer is "unproven".** Weapon damage,
armour and food nutrition come from the vendored `minecraft:attribute_modifiers`
/ `minecraft:food` default components (`data/item-combat-1.21.11.json`); a damage
type's armour behaviour and difficulty `scaling` come from
`data/damage-types-1.21.11.json` (the `#minecraft:bypasses_armor` tag + the
registry's own `scaling` field). Mojang publishes **no** per-entity default
attributes, so mob base health is genuinely unknowable at build time — the
numeric bound therefore runs only where `attributes.max_health` is declared, and
`DW0475` says so rather than inventing a health table (the same refusal as
`nav::DEFAULT_FOLLOW_RANGE` and `clearance::MODEL_MARGIN`).

**The Easy-halving trap, stated once.** `WorldDifficulty`'s doc comment gives the
Easy formula `min(dmg/2+1, dmg)`, and applying it here would be wrong by 2× in
the LENIENT direction. Difficulty scaling is a property of the damage TYPE, and
`damage-players` emits a bare `/damage <target> <amount> <type>` with **no
attacker** — so the eight types whose `scaling` is
`when_caused_by_living_non_player` (everything the DSL exposes except
`explosion`) are not scaled at all. The one type that does scale (`explosion`,
`always`) is also the one armour reduces, so it is not adjudicated either; a test
pins that pairing so a future MC pin breaking it fails loudly.

| Code | Meaning |
|------|---------|
| `DW0470` | A hostile the party is **required** to kill can never be damaged, so its `kill` objective can never complete and the delve soft-locks. Build-tier (exit 3), `compiler::combat`. Immunity is spelled one way on a wave mob: a `minecraft:resistance` effect at amplifier 4 (level V), which is 20%-per-level × 5 = 100% reduction against everything outside `#minecraft:bypasses_resistance` — the same fact the emitter already leans on for its PackTest scaffolding, so nothing in a player's kit can reach it. Only waves a critical-path `kill` step names are held to this; an optional wave may be as immortal as the content likes. (An `unleash-actor` twin is deliberately NOT covered: the twin summon carries no `Invulnerable` NBT whatever the actor's `vulnerable` flag says, so it is always killable.) Prescription: lower the amplifier to at most 3 (80% reduction — still an extremely tanky elite), or move the durability into `attributes.max_health`, where it becomes a number `DW0472` can bound. Do NOT delete the `kill` objective to silence it: an unkillable mob in the room is still an unkillable mob. |
| `DW0471` | A hostile the party is required to kill has **nowhere to be fought from** — no standable cell anywhere around its seated body, so no player can stand within reach and the `kill` objective can never complete. Build-tier (exit 3), `compiler::combat`. Deliberately **local**: a Chebyshev-1 ring around the columns the body's footprint occupies (widened by `nav::entity_dims`, the one dims table), over the elevations it spans. It says nothing about global connectivity, which is what keeps it free of the false positives a reachability flood would produce — a room legitimately shut behind a gate or a shortcut is not disconnected, and `check_critical_path` already owns that question. What it catches is what nothing else does: `DW0312` proves the spawn cell is standable, and a 1×1 pocket with a floor passes that while being unfightable. Prescription: move the wave `anchor` into open floor, or carve the pocket. Do NOT widen the wave spawn search — the mobs would simply be seated somewhere the author never staged. |
| `DW0472` | A mandatory encounter's **declared** effective HP outlasts the best kit the party can field. Build-tier (exit 3), `compiler::combat`. Effective HP = Σ `count × attributes.max_health ÷ resistance multiplier` over the stacks that declare health; the best hit is the largest `attack_damage` attribute across every class kit, **excluding the player's own base fist damage**, so the real fight is always at least as fast as the arithmetic says. The gate counts SWINGS (`ceil(EHP / hit) > 400`), not seconds, because swing damage is Mojang's data while timing depends on charge discipline the compiler cannot model; the message adds an indicative duration from the weapon's `attack_speed` for context only. 400 swings is deliberately enormous — an iron sword clearing eight 20-HP zombies is 32 — so crossing it means the numbers are wrong, not that the fight is hard. spec-0023 asks for "a sanity bound, not a balance opinion", and the compiler is forbidden from having balance opinions. Prescription: lower `max_health`, cut the stack `count`, or put a stronger weapon in a kit. Do NOT raise the budget. |
| `DW0473` | An **unavoidable** scripted hit on the critical path kills a full-health player outright (landed damage ≥ 20). Build-tier (exit 3), `compiler::combat`. Scope is what the party can do nothing about: `damage-players` in a quest's own `on_complete` / `on_objective_complete` bundle (descending `sequence` steps, which are the same unconditional bundle on a timeline). Everything with counterplay is outside it on purpose — trap payloads, stealth `on_caught` and `move`-reaction bundles, dialogue-option effects, and any `damage-players` carrying a `within` zone, since standing elsewhere IS the dodge. spec-0016/0022's telegraph and saturation rules govern those. Only armour-bypassing damage types are adjudicated (the default `generic` is one): for the rest, what lands depends on what the player wears at that beat, which a slotless kit list does not state. The message shows the arithmetic AND the rule it used, naming the damage type's `scaling` explicitly so nobody re-derives the Easy halving wrongly. Prescription: lower the `amount` below 20, or move the consequence onto a beat the party can play around. |
| `DW0474` | (**warning**; exit 0) A campaign with mandatory combat hands the party **no sustain at all**. **Mandatory combat is every fight of either shape** (`combat::mandatory_fights`): a critical-path `kill` step naming a wave, OR an actor the campaign `unleash-actor`s on the party. It used to be `kill`-a-wave alone — the verb, not the object class — which gated the WHOLE spec-0023 winnability pass (`DW0470`–`DW0475`) off for a delve whose combat is entirely actors: `nobodys-cave-island` turns three bodies loose, bills one `elite`, ships zero `kill` objectives, and ran none of those six proofs for twenty-two owner rounds while `combat-plan.json` reported `encounters: 0` with nothing saying that was a coverage fact. The fight count is now published as `combat-plan.json`'s `fights` block (`waves`, `actors`, `total`, `unbound`, `reason`). The finding itself: no class kit, `give-item` effect (any nesting depth) or `loot` container carries an item with a `minecraft:food` component. Natural regeneration stops once the hunger bar falls below 18, so after the first fight the party's health only goes down. Warning rather than error because the fight budget a party actually needs depends on play the compiler is forbidden to model (spec-0023 "Out of scope") — the finding is the literal zero, which is a design fact, not a balance opinion. Prescription: put food in the kits, or stock a container on the route. |
| `DW0475` | (**warning**; exit 0) The numeric time-to-kill bound **could not be computed** for one or more mandatory encounters, so they ship with the structural proofs only (damageable, reachable, wired) and no arithmetic. Two causes, both stated per encounter: a stack that declares no `attributes.max_health` (Mojang publishes no per-entity defaults, so its health is unknown — see the block header), or a party whose kits carry no item with an `attack_damage` attribute at all, which means the damage output is unknown rather than zero (a bow's damage is projectile code and appears in no vanilla data; absence in the item table is a fact about attributes, never a claim of harmlessness). One finding per campaign, listing every affected encounter. Prescription: declare `attributes.max_health` to opt the encounter into `DW0472`. Deliberately advisory: an encounter left on vanilla stats is legitimate — the author just has to see that nothing arithmetic was proven about it. |
| `DW0476` | **The flask** (spec-0016 §1). The campaign places a `bonfire` but at least one class kit declares no `"flask": true` entry. Validation-tier (exit 1), `dsl::validate`; the bonfire scan is the same nesting-deep one `DW0370` uses, so a `bonfire` inside a `sequence` counts. Resting replenishes every flask entry to its declared `count`, and that replenishment is the only thing separating *rest and save* from *save only* on the recovery side — with no flask declared, the expensive option recovers nothing the player can spend later and the souls loop has no consumable at its centre. Campaign-global on purpose: the flask is per-class gear, so one class without one is as broken as none, and the requirement lands on EVERY class. A campaign with **no** bonfire is untouched — a wave campaign owes the party no flask. Prescription: add a recovery item to each class kit and mark it `"flask": true` (needs `dsl_version` 0.8.0 on the classes stage). Do NOT drop the bonfire to silence it — the rest point is the design. |
| `DW0849` | **An item gate a class cannot bring.** An `interact` completes only for a player HOLDING a named item, and the item has no supply this campaign gives to a player of every class: its only source is another class's `kit`, or it has no source at all. Validation tier (exit 1), `every_version`, `dsl::validate::item_gate_class_checks`. A delve is played by one to four players who each pick one class, so a **solo player of any class is a supported party** — an item only one class carries makes some real party assembled unable to finish, and it learns that standing at the thing it cannot press. Quantified over EVERY class for the same reason `DW0476` is: one class that cannot bring it is as broken as none. **The class-blind supplies are an enumeration, not a list of the verbs one author remembered** (`dsl::validate::class_blind_item_sources`): a `give-item` at any effect root (walked through `dsl::stages::for_each_campaign_effect`, which is `dsl::effects::for_each_effect_root` underneath — the same closed enumeration emission lowers from), a `collect` objective's item, a `loot` container's stack, and a wave mob's `drops` — the quest-token form and the worn-piece form, whose item comes from that mob's own `equipment`. A trap's `dispense` payload is deliberately **not** a supply: a dispenser fires its stack at the party as a hazard, and being shot with a thing is not being handed it. **The approximation runs one way on purpose.** A flag-gated `give-item` late in the DAG counts, because the reachability model does not model items at all and a stricter rule without one would refuse correct campaigns. So this fires only where *nothing* class-blind supplies the item — which is the shape the finding had, and the shape a mistyped-but-real item id has. **Binding: `interact` objectives declaring `requires_item`.** Prescription: put the item in a `collect` or a `loot` container on the way to the gate, hand it out with a `give-item` (whose default `carrier` is `all`), or add it to every class kit. Never drop `requires_item` to silence it — presenting the item is the beat. |
| `DW0850` | **A `reach` the party can arrive at without completing.** Build tier (exit 3), `every_version`, `compiler::reach::check_reach_completion`, run at the same site and over the same final assembled world as `DW0314`. Two assertions per reach objective, one code, because the remedy is the same. **Occupiable:** some cell of the completion volume is standable — a volume no body can be in is an objective nothing completes, which is the reported instance (a point sphere too tight for a human standing on the altar cell) stated as a property instead of as one altar. **Delivered into:** where the critical path walks to the anchor, the cell the leg actually ends on is inside the volume. That half is the one no existing proof could see, and it is live arithmetic rather than a hypothetical: `nav::SNAP_RADIUS` is **3** and the v0.3+ completion cube's half-extent is `max(1, radius)`, so a reach whose only footing lies further out than its own volume reaches is routable (`DW0311`), standable-on-export (`DW0314`) and walkable — and the party, standing exactly where the campaign routed them, is outside the volume that fires the objective. **The volume has one authority** (`compiler::reach::reach_completion`): the emitter formats its `tick` selector from the value this proof judges, and `critical-path.json` carries that same value to the bot as `completion`, so the string and the check cannot come to disagree about a rule that is invisible in the DSL and shows up only as a beat that never happens. Membership is read conservatively — vanilla tests hitbox intersection, so a body one cell out may complete on a face-touching tie, and demanding the certain case is what makes a green here mean *the party completes this* rather than *might*. **Binding: `reach` objectives whose anchor the plan resolves**, quantified over the quests rather than over the exported path, because a reach on an optional quest is a reach a player can be standing at. Prescription is always geometry: move the anchor onto the footing, or give the anchor cell floor. Never nudge the waypoint, and never widen the volume — widening it once, for the reported instance, is what left the other number live. Note the arithmetic the authored radius sets: an endpoint snaps inside a box of half-extent `SNAP_RADIUS`, so at `radius >= 3` every arrival the route can deliver is inside the volume and the **delivered into** half has nothing left to catch, while **occupiable** binds at every radius. That is the defect being absent rather than the check being blind, and `tests/reach_completion.rs` pins it so a later narrowing of the volume cannot re-open the gap silently. |
| `DW0477` | (**warning**; exit 0) **Something billed `elite`/`boss` that the inverted floor gate cannot measure**. One diagnostic per finding, at the declaring node's own pointer (`/content/actors/<i>/tier` or `/content/waves/<i>/tier`), `compiler::combat`. Three uncovered shapes, each with its own reason text, carried verbatim into `combat-plan.json`'s `floor_gate.not_covered`: a tiered **actor** no `spawn-actor` beat ever summons; one staged but never `unleash-actor`ed and not `vulnerable` (the puppet is `Invulnerable` — scenery, not a fight); one only ever staged `vulnerable` (damageable but `NoAI` and knockback-immune, so it never attacks — anything that cannot fight back is beaten cold by construction, and a floor finding derived from it would be an artifact of the check rather than a fact about the encounter); plus a tiered **wave** no critical-path `kill` objective names. Why it exists: the floor gate warns when the unassisted bot beats a billed elite first-try and says **nothing** otherwise — so an encounter that was never fought produces exactly the same silence as one that was fought and lost, and before this the bell's actor-implemented Barrow Warden made an empty finding list read as a pass over a fight nobody had. Advisory tier because an unmeasurable elite is a legitimate design (set dressing the content also chose to name); what is not legitimate is nobody knowing. Prescription: add the `unleash-actor` beat (or the `kill` objective), or drop the tier. An **untiered** hostile actor is a `not_covered` ledger entry but NOT a `DW0477`: nothing was billed, so there is no billing to hold and no `tier` pointer to attach the diagnostic to. |
| `DW0478` | **The respawn-point safe zone** (spec-0016 §1). A cell the party comes back to life on sits inside some hostile force's aggro range. Build-tier (exit 3), `compiler::nav::check_respawn_safe_zone`, run after wave seating and lane resolution because it needs both. **The object class is every respawn point** — a `bonfire` and a plain `set-checkpoint` alike (they are siblings of one sum type, resolve to one `CheckpointPlan` distinguished only by `rest`, and vanilla returns a dead player to either by the identical `spawnpoint`). Keying it to `rest == true` made it a hook on one variant and not its sibling: `nobodys-cave-island` shipped three `set-checkpoint`s and six hostile forces while this proof examined ZERO objects and reported green. A plain checkpoint is **monotonic**, so it is measured only against forces that can be in the world while it still governs — its reign ends when a later `set-checkpoint` replaces it (`Plan::respawn_reign_ends`). A bonfire never stops reigning, so it is measured against everything. The window narrows WHAT IS COMPARED, never what is demanded of a compared pair. **What a red claims is what declarations can carry** (spec-0044): not *"this is a soft-lock"* — whether a retry loop is winnable is a combat question the compiler refuses to simulate (ADR-0006) — but *"nothing this campaign declares separates this retry from a soft-lock"*. Three evidence routes answer it, all in `compiler::respawn`, each demanding a fact vanilla structurally contradicts when the defect is real, and each falling to a **conservative zero** (the pair stays compared) when the evidence is missing or ambiguous. **(1) The reset.** The pair is credited when the fold, in emitted line order, of the **unconditional** effects of the respawn point's own `on_respawn` (a bonfire's `on_rest`) removes the force with no later re-stage; a despawn followed by a `spawn-actor` is a re-stage and is measured at the re-staged cells instead, its verdict then passing through dominance rather than around it. An effect behind any `requires_flags` / `forbids_flags` — its own, or any enclosing one — is **never** credited: the post-reset world must hold in every state a death can occur in. **(2) The onset bounds**, which narrow the comparison window. A force's staging onset is the earliest `spawn-wave`/`spawn-actor` beat that stages it, raised to the **flag bound** of every gate on that beat — the earliest step each required flag can be set, resolved recursively over its own producers' gates, with 0 on any cycle or unresolvable producer. A staging from a root with no beat of its own (a trigger, a trap payload) is otherwise step 0. The **bearer bound**: a trigger keyed to an entity (`strike-npc`) structurally cannot fire without its bearer, so a force staged only from such triggers is skipped when every bearer is unconditionally removed by a forced bundle at or before the seat, nothing anywhere stages a bearer again, and no instance staged before the reign survives into it. The **puppet bound**: a staged actor's body is emitted `NoAI:1b` and vanilla gives a `NoAI` mob no target acquisition at all, so an actor's perception onset is `max(staging, unleash bound)`, where a step-rooted unleash resolves at its step, a flag-gated one at its flag bound, a proximity-triggered one at the earliest critical-path entry into the trigger's own declared region, and anything else at 0 — which makes the bound strictly narrowing, since `max(s, 0)` is the pre-amendment answer. **(3) Dominance.** A pair still red is credited when the campaign's own **forced critical path** — the routed walk `DW0311` proves, per leg — reaches a step **strictly after** the seat is armed and inside its reign, at or after the force's perception onset, that comes no farther from the force's **stationary** cells than the seat itself stands. Lane cells never dominate: a marching squad's corridor is every cell it sweeps over time, and the path crossing it is not a proven meeting — crediting on it would re-ship this rule's own motivating death. What anchors the credit is the oldest invariant the product has: a dominated respawn can only be a soft-lock if that forced beat is unwinnable, i.e. the campaign is uncompletable, which the machine playthrough refuses on evidence (a finished run) no defect can supply. **One build reports EVERY violating pair**, first pair first then the full list; returning at the first is how six false verdicts hid behind one. The binding count is published as `validation/respawn-safety.json` (`examined`, `pairs`, `credits`, `unbound`, `reason`, plus per respawn point the forces it was and was not compared against, each skip carrying its kind and reason, and each credit its kind, reason and post-reset state) — a proof that examined nothing must not read as a pass, and a credit must be as auditable as a skip. The rule: for every wave and every fighting actor, the distance from the respawn cell to that force's occupied cells must **exceed** its `follow_range` — and for a **lane path cell**, `follow_range` **plus the measured marching drift** of 7.9 blocks (`nav::LANE_MARCH_DRIFT`): the td-routing-spike dossier measured a marching squad as a corridor around its polyline (followers mean ≤3.2, max 7.9 blocks off-lane), so a centre-line distance understates the squad's real aggro reach — a fire can clear the polyline by 2 blocks and still be perceived, which is exactly how run nine died at 17.7 blocks from a 16-`follow_range` lane. Stationary cells (seated spawns, staging anchors) keep the plain `follow_range` term. Occupied cells are the DW0312-proven **seated spawn cells** (where the datapack really summons it, not where its anchor is), plus — for a `lane` wave — every cell of the DW0386-proven **march polyline**, because a lane wave's whole design is that it walks that corridor while the party is elsewhere. Radius: a lane's `aggro_radius` (emitted verbatim as each lane mob's `follow_range`), else the largest declared `follow_range` among the wave's mobs, else the documented default 16 — one number, never a per-species table the compiler would have to invent (`DW0475`'s rule). An **actor** counts when the campaign declares it as a fighter — `unleash-actor`ed somewhere, or staged `vulnerable` — the same declaration-based test `DW0469` uses; species is never consulted, because the pinned entity registry is a membership set with no mob-category data. Why error tier and not a §7 pacing lint: a respawn point is where the party comes back after a death — and, for a bonfire, where every `respawns_on_rest` wave is put back on its feet — so a cell inside a perception radius delivers the party into contact on the tick they arrive — the retry loop the fire exists to make cheap becomes a soft-lock, and there is no reading of that geometry that is the authored point. The message names both sides, the closest offending cell, what kind of cell it is, the measured distance, and how many pairs this build condemns in total. Prescription, in this order: supply one of the three evidence routes — an unconditional reset that removes or re-places the force, a staging that cannot meet the reign, a forced in-reign beat that already delivers the same encounter no-more-gently — or move the respawn point out of the danger (a side room, past the threshold, beyond the end of the lane), or move the force's anchor / lane. **Never** shrink `follow_range` to buy the clearance, which retunes a fight to hide a placement bug. |

### DW0486/DW0487 — the flask's contents (`dsl::validate`; spec-0016 §1, DSL v0.8)

The kit `flask` marker landed with no way to declare what the bottle pours, so
every flask compiled to a `minecraft:potion` carrying no
`minecraft:potion_contents` component — vanilla's *Uncraftable Potion*, which
grants nothing however it is named. `contents` (§2, stage 3) closes that; these
two keep it honest, and both are classes-stage validation at 0.8.0 only.

| Code | Meaning |
|------|---------|
| `DW0486` | **Contents 1.21.11 cannot pour** (spec-0016 §1). A kit item's potion `contents` is not something the `minecraft:potion_contents` component can express. Validation-tier (exit 1), `dsl::validate::kit_potion_checks`, at 0.8.0 on the classes stage. Seven shapes, each at its own pointer: `contents` on an item that carries no such component (only `minecraft:potion`, `splash_potion`, `lingering_potion` and `tipped_arrow` do — anywhere else the game discards the data); contents that name no `potion` and list no `effects` (the bottle still pours nothing); a `potion` outside the pinned 1.21.11 `potion` registry (usually the pre-1.20.5 spelling — strength and duration are part of the id, `strong_healing` / `long_night_vision`, never separate fields); an unknown status-effect id; an `amplifier` past 255, the end of vanilla's unsigned byte; a `duration` of 0 or past 1 000 000 ticks (≈13.9 h, past the delve ceiling — the ceiling catches a duration typed in milliseconds); a lasting effect with **no** `duration`, which vanilla would default to zero ticks, i.e. to nothing; and its mirror, a `duration` on `instant_health`/`instant_damage`, which land once on the tick the potion is drunk and never read it — that last one exists because the author who writes it believes they have authored a heal over time. Prescription: fix the field the message names. |
| `DW0487` | **The placeholder flask** (spec-0016 §1). A potion-bearing kit item declares no `contents` at `dsl_version` 0.8.0. Validation-tier (exit 1), `dsl::validate::kit_potion_checks`. A `minecraft:potion` with no `minecraft:potion_contents` component is vanilla's *Uncraftable Potion*: a bottle a player can drink all day for nothing, however it is named — and naming it is exactly what a campaign does when the DSL gives it no way to say what is inside, which is how every flask shipped between the `flask` marker landing and this field. The requirement fires only at 0.8.0, the version that introduced `contents`: a 0.7 campaign has no way to comply, so demanding compliance would be a version break rather than a check. Scoped to the item, not the `flask` marker — a tipped arrow with no contents is the same uncraftable item. Prescription: declare `"contents": {"potion": "minecraft:strong_healing"}` or an `effects` list. Do NOT rename the bottle instead: semantics never key on player-facing text (§4). |

### DW048x — branch-complete narrative verification (`compiler::branch`; spec-0025, DSL v0.8)

"Provably completable by machine" quantifies over **branches**, not paths. The
ladder used to prove ONE critical path: a fork that decides who lives was
declared in the DSL, reachability-checked as a graph, and then never played. The
island round-13 defect is the whole blind class in one shape — the flee branch's
cast ledger said Antiphos lives while the staging still belonged to the death
branch: an NPC despawned himself, another held a cave the party had left, a third
mourned a man standing beside him. **The fork moved the ledger but never moved
the bodies**, and no check owned the gap.

**The model.** Stage 4 declares its `branch_points`: the flag set a fork owns
(`forks_on`), the quest it `opens_at`, and the branches it offers. An
**enumerated branch** is one point of the product over the declared points, so
the branch set is authored and small — never a combinatorial sweep of every flag.
Each branch carries a **flag assignment**: the flags it lists are pinned SET and
every other flag of its points' `forks_on` is pinned UNSET. That second half is
what makes leakage decidable rather than hopeful. An assignment is realized
against `compiler::flow`'s enumerated worlds — a world realizes a branch when its
solved flag set holds every pinned-set flag and no pinned-unset one — and the
branch's own playthrough is rooted at **the branch**, not at the stage-4
`finale` (a branch running to its own ending never completes the finale, so
rooting there would say the branch plays nothing).

Validation-tier (exit 1), like the `DW046x` ledger it extends. The whole module
is **fenced at `dsl_version 0.8.0`**: below it nothing here fires, which is
proven on bytes — stripping the entire v0.8 surface from a campaign and dropping
it to 0.7.0 produces a byte-identical `datapack/`
(`the_v08_surface_changes_no_datapack_byte`).

| Code | Meaning |
|------|---------|
| `DW0480` | **Undeclared story fork.** A flag that gates casts, staging, quest structure or a staging trigger, is set on some enumerated playthroughs and not others, and belongs to no declared branch point. "Forks" is decided, never guessed: a flag EVERY playthrough sets is ordinary sequencing and is silent. An undeclared fork is a branch nothing verifies — exactly how a campaign ships with the ledger on one branch and the bodies on the other. Prescription: declare the branch point (`forks_on`, `opens_at`, and each branch's `leads_to`). Do NOT silence it by ungating the content — the gate is the story. |
| `DW0481` | **A story node declares no `happening`** (0.8.0+). The forcing function, generalizing spec-0020's `doing` from NPC presence to event flow: a design that never got written down node by node cannot compile. Required on every quest, every objective, every one of the **eleven story-node effects** (`spawn-npc`, `despawn-npc`, `move-npc`, `spawn-actor`, `despawn-actor`, `move-actor`, `unleash-actor`, `spawn-wave`, `open-gate`, `close-gate`, `campaign-complete`) at any nesting depth, and every **story-weight dialogue option** — one carrying a `set-flag`, which is how a player's choice forks the world. An option that only walks the tree or completes an objective needs none (the objective already declares one). Prescription: state the beat with one of the ten verbs plus a line of prose. Do NOT fill it with a placeholder: the per-branch chronicle the narrative review reads is assembled from exactly these lines. |
| `DW0482` | **Branch terminality.** A declared branch reaches no ending: either **no playthrough realizes its flag assignment** (a branch nobody can take — commonly a branch declaring two mutually exclusive flags), or its playthrough fires no `campaign-complete`, or it fires an ending other than the one the branch declares, or the quest it declares it `converges_at` never completes. The message names the branch, the assignment, and the ending that really fires. |
| `DW0483` | **Cast continuity** — spec-0020 proof 4 (`DW0462`) extended from "the declaration exists" to "the selector resolves to THIS branch's cast at every quest after the fork". For each enumerated branch, at each quest **strictly after** its fork, an NPC declaring per-branch casts must have exactly one placement selected under the branch's flag state when that quest opens. Zero selecting means the NPC has no declared position on this branch; two or more means emission dispatches the last clause, which is how a placement left UNGATED (or gated on the other branch's flag) keeps governing long past the beat that wrote it — the round-13 defect. The fork quest itself is excluded on purpose: during it the flag state is by construction pre-fork, so a per-branch cast there could never select. Prescription: gate each placement on the flags of the branch it belongs to, every branch, every post-fork quest. Do NOT leave one ungated as a fallback. |
| `DW0484` | **Exclusive-content leakage.** Every playthrough that realizes a branch's set flags also produces a flag the branch pins UNSET — so content gated on a sibling's flag is reachable HERE. The mourning scene on the branch where nobody died, as a build error rather than a review note. The message names the leaked flag and where it is produced (an ambient environment trigger or trap disarm is called out explicitly, since those fire on every branch by construction). Prescription: make the producer exclusive to the branch that owns it. Do NOT relax the branch declaration to admit the leak. |
| `DW0485` | **Hard event contradiction**, per branch, over the chronicle order, with **both chronicle lines shown**. Four rules, each decidable from the structured verbs alone: `dies(S)` then any later act by `S`; `departs(S)` then an act by `S` with no `arrives(S)` between; `seals(S)` then any later beat about `S` that is not `opens(S)`; `loses(S)` then a second `loses(S)` with no `gains(S)` between. `learns`/`believes` are **epistemic** and never contradict — their subject is what the beat is *about*, and a living character may perfectly well believe something about a dead one; "Elpenor mourns a man standing beside him" is precisely the class spec-0025 leaves to the chronicle's human reader, because no verb makes it decidable. Ambient beats (environment triggers, trap payloads) are excluded: `flow` refuses to date them, so ordering them against the dated account would invent a sequence. Prescription: fix whichever beat is on the wrong branch. Do NOT reword the `happening` to hide the clash — the verbs are the only part of the chronicle a machine can check. |
| `DW0488` | **A shared walk driver with two origins.** One content-keyed `move-npc`/`move-actor` driver is reached by occurrences that do not stand in the same place when they fire, so its waypoint polyline is the wrong one for at least one of them and that occurrence opens by teleporting the body across the map. Build-tier (exit 3), `compiler::nav::plan_moves` / `plan_actor_moves`. Drivers are content-keyed by `(body, destination, branch gate)`; two beats on the **same** branch that walk one body to one mark from different places therefore still collide, and that collision is this diagnostic. The message names both origins and the branch each occurrence fires on. Prescription: give the two beats distinct destinations (a second anchor a step apart reads identically in play), or walk the body to a shared staging mark first so both occurrences start from the same cell. Never "fix" it by deleting one of the walks — the body has to get there on both branches. |

The rest of the `DW048x` block is unassigned, reserved for the spec-0025 harness
tier (scripted-choice branch runs) and for real needs as they arise.

### DW0490–DW0493 — declared drops (`dsl::validate`; DSL v0.9)

**A mob may wear many pieces, but what it leaves behind is a declared subset —
usually one piece, never automatically everything.** The
DSL says WHICH pieces drop; quest items may be declared as drops too. All four
codes are validation-tier (exit 1), in `dsl::validate::check_drops`, and the
whole surface is fenced at `dsl_version 0.9.0` (declaring any of it earlier is
`DW0141`). Below 0.9 nothing here fires and nothing here emits: an undeclared
slot keeps drop chance `0.0f`, which is byte-for-byte what pre-0.9 emission
wrote — proven on bytes by rebuilding an existing campaign with the pre-change
compiler (`nobodys-cave-island`: identical `datapack/`, `world/` and server
config; the only delta is the engine-version string stamped into the
creator-loop `layout.json`).

| Code | Meaning |
|------|---------|
| `DW0490` | **A drop nobody wears.** A `drops[]` `slot` entry does not name a distinct slot the same entity's own `equipment` fills — the slot is empty, or the same slot is declared twice. A body can only leave behind a piece it wore, and only once. The message names both sides: the slot asked for, and the slots actually filled. Prescription: equip the slot, or declare one the kit fills. |
| `DW0491` | **Drops on an untiered fight.** `drops[]` on a wave or actor that is not billed `elite` or `boss`. Only a named fight leaves anything behind; making rank-and-file gear lootable is grind, which the constitution forbids, and the failure would be silent (a farmable mob looks exactly like an unfarmable one in the DSL). Prescription: declare the encounter's `tier`, or remove the drops. |
| `DW0492` | **An unsourced drop-gated collect.** A `collect` `dropped_by` is not backed by the wave it names: the wave declares no `{item}` drop of this objective's item (the message lists what it *does* declare), the objective asks for more copies than the wave's mobs can yield, or the objective also adopts a `container` — the item comes off a body or out of a box, never both. Prescription: declare the drop on the wave's mob, lower the count, or drop whichever provisioning the beat does not use. |
| `DW0493` | **A prize that arrives before the fight.** A `collect` `dropped_by` is not ordered after a `kill` objective for that wave — not through the intra-quest `after` graph, not through a quest this one `depends_on`. Without that edge the objective reads as active from the campaign's first tick over an item that does not exist yet, and "kill the boss, take its key, open the door" is an authoring intention the quest graph cannot check. Prescription: add the `kill` and list it in this objective's `after`, or put the kill in a quest this one depends on. |

#### The vanilla primitives, and why these numbers

Both halves are vanilla, verified against the **pinned 1.21.11 jar** rather than
folklore:

- **Worn pieces** ride the `equipment` / `drop_chances` compounds the compiler
  already writes. A declared slot gets **`2.0f`**, not `1.0f`. Vanilla's
  `DropChances` record (class `cgi`) names both numbers itself:
  `withGuaranteedDrop(slot)` writes the constant `2.0f`, and `isPreserved(slot)`
  is `chance > 1.0f`. `Mob.dropCustomDeathLoot` (class `chn`) reads both — a slot
  at exactly `0.0f` is skipped outright, and a **preserved** slot both drops when
  the killing blow was not a player's *and* skips the durability randomization
  that a chance of `≤ 1.0` applies to a damageable item. At `1.0f` a boss axe
  would drop with a die-rolled amount of damage on it, which is not a
  deterministic drop. (The same `2.0f` is what vanilla's own
  `SaddleEquipmentSlotFix` datafixer writes for a saddle a horse always drops.)
- **Quest items** have no slot, and hanging one in an off-hand the author never
  dressed would be exactly the downstream workaround the no-hack rule forbids.
  1.21.11 answers the slot-less half with its own primitive: `Mob` reads
  `DeathLootTable` (and `DeathLootTableSeed`) straight off summon NBT through the
  `ResourceKey<LootTable>` codec, and `dropAllDeathLoot` rolls it on death. The
  compiler already wrote `DeathLootTable:"minecraft:empty"` on every actor; a
  declared item drop points the same field at
  `data/<ns>/loot_table/dw_drop/{actor_<id>|wave_<wave>_<i>}.json` — one pool,
  one roll, one `minecraft:item` entry per declared item, no RNG (ADR-0006). A
  declared display `name` becomes `minecraft:set_name` with `target:
  "custom_name"` (both targets confirmed in the jar), the **same component** a
  `collect`'s `item_name` writes into a container stack, so the key a boss leaves
  on the ground and the key a barrel hands over are the same item.

**Removal is not a death the player earned.** Every removal the compiler performs
itself goes through `/kill`, which is an ordinary death, and a preserved slot
survives a non-player kill — so an elite the story re-cages would shed its axe on
every rest. The `unleash` that kills the puppet and both `despawn-actor` styles
therefore strip the declaration off the body first, with two intended primitives
composed: `execute as @e[tag=…] run data merge entity @s` (single-entity by
construction, which is what `data merge` requires) writing `0.0f` on every slot
and an empty death loot table. Emitted only for actors that declare drops, so
every earlier campaign's removal is byte-identical.

### DW050x — runtime state (`dsl::validate`; spec-0031, DSL v0.10)

Runtime state is a **declared** datum: a name, a scope (`player` / `party`) and
an initial value, written by `set-state`/`add-state`/`clear-state` and compared
against by `requires_state` in any gate. All four codes are validation-tier (exit
1), in `dsl::validate::state_checks`, and the whole surface is fenced at
`dsl_version 0.10.0` (declaring any of it earlier is `DW0141`, per stage — a
dialogue option's comparison is fenced by the *dialogue* stage's version). Below
0.10 nothing here fires and nothing here emits: no scoreboard objective, no
`state_seed` function, no tick clause, no guard clause. Proven on bytes by
rebuilding an existing campaign with the pre-change compiler
(`nobodys-cave-island`: identical `datapack/`, `world/` and server config; the
only delta is the engine `dsl_version` string stamped into the creator-loop
`layout.json`).

Both directions of the read/write ledger are errors, because each is a **vacuous
binding** in the CLAUDE.md sense and each is silent — the campaign compiles, the
datapack loads, and the delve plays as though the mechanism were live.

| Code | Meaning |
|------|---------|
| `DW0500` | **An undeclared datum.** A `state/<kebab>` reference — in a `requires_state` comparison or in one of the three verbs — names a datum the stage-5 `state` list does not declare. Unlike a flag, whose set is exactly what some `set-flag` produces, a datum is declared because its scope and its initial value are facts no use site can supply: an undeclared reference is not "a datum that happens to start at zero", it is a datum with no defined multiplayer semantics at all. Prescription: declare it, or fix the id. |
| `DW0501` | **Read, never written.** A gate's `requires_state` reads a declared datum that no verb anywhere in the campaign ever writes, so it can only ever hold its declared `initial` and every comparison against it was decided when the campaign was written. The gate is a constant wearing a condition's clothes — the numeric form of the bot's combat floor examining zero enemies for nineteen island rounds. Prescription: write it somewhere, or drop the comparison and say what you meant unconditionally. Its emitted-layer sibling is `DW0495`, which asks the same question of the commands rather than of the campaign, and therefore reaches the engine-internal objectives no campaign can declare. |
| `DW0502` | **Never read.** A declared datum that no gate's `requires_state` anywhere in the campaign ever reads. Either some verb writes it and nothing ever asks (an inert write — a counter nobody consults), or nothing touches it at all (a dead declaration). Runtime state exists to be compared against; a datum with no reader is bookkeeping no player can observe. Prescription: gate something on it, or delete the declaration and its writes. |
| `DW0503` | **No acting player.** A `player`-scoped datum is read or written where emission has no `@s` to resolve it against. Every such place is a property of the SITE, never of the verb, and there are three kinds. (1) **The root.** Four of the seven effect roots run with an acting player and three do not — a trigger's `effects`, a trap's `payload` and a shortcut's `on_unlock` are polled on the tick from the server command source (`Audience::Scheduled`), while `on_objective_complete` / `on_complete` are dispatched `as @a` and `on_death` / a dialogue `on_respawn` are the dying-or-respawning player's own. The answer is `EffectRootKind::runs_with_acting_player`, bound by equality to `emit::root_audience` over the closed root set — **except that since v0.11 a trigger answers per declaration**: `audience: presser` is dispatched by the interaction advancement and does have an `@s`, so the check asks `EffectRootSite::runs_with_acting_player` (which consults the trigger) and the kind-level answer stays the class default. Asking the kind would refuse a `player`-scoped read the emitter can serve, which is the mirror of the bug this seed was added to fix. (2) **The seams inside a bundle**, latched exactly as `DW0357` latches them for `carrier: "one"`: a `sequence` step and a `move-npc`/`move-actor` `on_arrive` drop the actor, while a `set-checkpoint` `on_respawn` and a `begin-stealth` `on_caught` nested inside one restore it. (3) **The gates emission evaluates against the party holder** — an objective's activation guard, a trigger's arming gate, a trap's arming gate. Reads and writes are treated alike: a per-player score named from a sourceless function is `@s` with nothing to resolve it to, whether the command is a `scoreboard players set` or an `execute if score`. Prescription: declare the datum `party`-scoped if the whole party shares it, or move the read/write onto a site a player drives — a dialogue option, a cast placement, `on_death`, or an effect on a beat a player completes. |

#### Which sites can touch a per-player datum, and why it is decidable

Two closed sets answer it, and neither is a list anybody maintains.

`GateConsumer::evaluates_per_player` answers for a gate's own site, and returns
`Option<bool>`: a dialogue option's availability is computed per player into
`dw.dmask` and its `/trigger` handler runs `as @s` (`Some(true)`); a cast
placement selects a scene into a per-player `dw.cast` (`Some(true)`); an
objective's guard, a trigger's arming gate and a trap's arming gate are party
predicates by construction (`Some(false)`). **`Effect` answers `None`** — an
effect's gate is evaluated wherever its bundle runs, and that belongs to the
root. The `Option` is deliberate: an earlier version answered a plain `true` for
`Effect`, which is right for `on_objective_complete` and wrong for three of the
seven roots, silently. A seventh consumer class cannot compile without answering.

`EffectRootKind::runs_with_acting_player` answers for the root, exhaustively, and
`emit::root_audience` is the single place the emitter chooses a bundle's
audience — one function rather than seven literals at seven call sites, bound to
the DSL's answer by equality in `emit::tests::root_audience_matches_the_dsl`. A
root whose emitted audience moved without that answer moving with it would turn a
validated per-player read into an `@s` in a sourceless function, with every check
green; an eighth root fails the bind until both sides name it.

#### One gate, three fields

`requires_flags` / `forbids_flags` / `requires_state` are one object
(`dsl::gate::Gate`), and every consumer answers `gate()`. Two things keep that
from decaying:

- `crates/dsl/tests/gate_consumers.rs` enumerates the gate-declaring object
  schemas **from the generated JSON Schema** — derived from the Rust types, so
  the enumeration is complete by construction rather than by diligence — and
  fails when any of them declares part of the gate and not the rest. It states
  its binding count (28 sites, 6 consumer classes) and asserts it exactly, so a
  new gate consumer is a deliberate diff rather than a silent one.
- `tools/check-capability-ownership.py` carries `("QuestEffect",
  "requires_state")` in `MODIFIER_HOLES` as an **inherited** open finding: the
  comparison rides exactly the nineteen verbs the flag pair rides and is absent
  from exactly the same ten. That is the point, not an oversight — a gate is one
  object, and giving its comparison a different carrier set than its flags would
  make "which verbs are gatable" two different answers. All three fields lift
  together, in one `dsl_version`, or none do.

**Where a comparison IS evaluated, and where it is not.** A `requires_state`
comparison stays out of the monotone producibility fixpoint, exactly as
`forbids_flags` does: that fixpoint has no notion of *when*, and a comparison is
entirely about when. The compensating stronger check is the **path replay**,
which does have a concrete order — so a numeric gate is evaluated there, against
the value the path itself has produced by the time the gate is read, and
`DW0879` refuses one the path has already made unsatisfiable. `DW0501` is the
other half and asks a different question: whether the datum is driven at all.

### DW0847 — a gate that can never open (`dsl::validate`; every gate consumer)

| Code | Meaning |
|------|---------|
| `DW0847` | **A gate contradicts itself, so it can never open.** A flag on both `requires_flags` and `forbids_flags`, or `requires_state` terms on one datum that no integer satisfies (`at-least 5` with `at-most 3`, two different `equals`, a `not-equals` punching out the only pinned value). The thing carrying it — objective, effect, trigger, trap, dialogue option, cast placement, shop offer — is authored content that provably never happens. One rule over the whole closed consumer set (`dsl::gate::for_each_gate`), because satisfiability is a property of the **gate**, never of the verb that first needed the question answered — the first asker was the cast ladder's per-clause solver (`DW0846`), and a check written beside it would have left the other six classes with no surface. The arithmetic is `dsl::gate::DatumSet` (interval-with-holes intersection, exact emptiness), the same value-picker the solver drives generated `cast_ladder_*` phases from, so "can this open" and "at what value" have one authority. Validation tier (exit 1), `every_version` — it judges an authored contradiction, a fact of the campaign alone. Distinct from `DW0501` (a satisfiable comparison whose datum nothing writes) and from the flow proofs' flag reachability: this is emptiness of the gate itself, before any question about what the campaign does at runtime. Prescription: fix the gate, or delete the thing it makes unreachable. |

### DW0540–DW0542 and DW0545 — status effects, the region teleport, and the fixture class (`dsl::validate` / `compiler::teleport` / `compiler::affordance`; spec-0031, DSL v0.10)

`DW0540` is the one rule in this family that is about a *pattern* rather than a
value, and it is the reason the surface is shaped the way it is. `give-effect`
has no infinite form: `seconds` is required and bounded, so a grant always ends
by itself. That can still be defeated by two effects that are individually fine —
grant blindness for an hour, clear it four ticks later — and then the clear is
the real removal, so any path that does not reach it (a logout, a crash, a death
mid-chain, a `sequence` whose remaining `schedule` never runs) leaves the player
blind for the rest of the hour.

The rule therefore fires on exactly the grants that are **still live** when their
clear arrives. Where the duration expires first, the duration is the removal and
there is nothing to say. "The same sequence" is mechanical: a bundle's own
timeline, where a plain member runs at offset 0 and a directly-nested
`sequence`'s members run at their step's `at_ticks` (nested sequences are
`DW0329`, so the expansion terminates). Conditional continuations — `on_arrive`,
`on_caught`, `on_respawn`, `on_rest` — are separate bundles with their own
timelines and are not folded in; a clear hanging off an arrival is strictly more
fragile than one on a fixed tick, and it is the mandatory duration, not this
rule, that keeps that case survivable.

`DW0542` is what stands where a runtime exemption list would otherwise be. A
`teleport`'s selector is total over bodies, so a volume drawn over an affordance
the engine anchored to a *block* would move the entity and leave the hardware: a
campfire, a lever or a sealed door still visible, still reachable, answering
nothing. The affordance set is not enumerated by this proof — it is
`eclipse::affordances`, the same authority `DW0359` measures bodies against, plus
the seal shells `DW0422` owns — so an affordance added to the engine enters this
proof by existing. Content bodies (NPCs, actor puppets, wave mobs) are
deliberately not refused: moving them is the mechanism working, and it is what
the cargo-lift ruling asks for.

Binding: `validation/teleport-gate.json` states how many teleports were declared
and resolved, how many cells their volumes cover, how many affordances were
examined, and how many PackTest templates were generated — a compile-time-only
green over a runtime mechanism is the vacuity that last number exists to make
visible. A campaign that declares no teleport emits no file at all, so a file
that exists and reports zero is a finding rather than an absence.

#### DW0545 — the fixture class: what a region verb selects

`DW0542` reaches every place whose cell the compiler knows. **A recovery stake's
marker has no such cell** — its position is the death point, or a row of the
compile-time placement table picked by the respawn seat in force — so a lift and
a stake in one room shipped a silent defect: the ride carried the marker away
from the position its ledger recorded, and the next tick `stk_gc_<s>` found
nobody holding a wager there and retired it. The wager was not uncollectable, it
was deleted.

The two obvious fixes are both defects CLAUDE.md names. *Teleport exempts engine
machinery* re-implements a general mechanism privately inside one verb; *the
stake ledger survives its marker moving* keys a capability to the wrong object,
making the stake compensate for a selector that grabbed something it should never
have grabbed. The question is upstream of both — **what does a content-authored
region verb select?** — and the measurement answering it is short:

| region verb | what its emitted selector reaches |
|---|---|
| `teleport` (`from`) | every **entity** in the box — the only verb with no filter at all |
| `lethal_volumes[]` (`region`) | every entity in the box minus six **types** (`@e`), plus every player (`@a`) |
| `give-effect` / `clear-effect` (`in`), `damage-players` (`in`), stealth zones, the night-vision area grant | **players only** (`@a`/`@s`) — no engine entity is reachable |
| `fill-region`, `clear-region`, `collapse`, `close-gate` | **blocks**; no entity selector exists |

So exactly two verbs quantify over non-player entities, and only they had the
question to answer. They answered it differently, and one of them not at all.

The fix is a **class the object declares about itself**, not a roster any verb
holds. Every entity the engine summons carries one of two tags:

- **`dw_fixture`** — *a place.* Its position IS engine state: an affordance's
  `minecraft:interaction` hitbox, the `dw_marker` display beside it, a stake
  marker, a cutscene's return mark. Moving it does not move a thing, it rewrites
  a fact.
- **`dw_borne`** — *carried by a body.* Today exactly one: an NPC's co-located
  dialogue hitbox, which must ride whatever its speaker rides.

A cutscene *camera* declares neither and that is deliberate: its own driver
re-asserts its position every tick, so it is a body the engine flies rather than
a place it recorded. Neither tag is authorable, and no campaign JSON can turn
either off.

Every box-narrowed entity selector then carries `tag=!dw_fixture` — **one negated
tag for the whole engine, forever**, which is what a type roster can never be. A
type cannot answer this question at all: an NPC's hitbox and a stake's marker are
both `minecraft:interaction`, and a teleport must move the first and leave the
second. `lethal_volumes[]` keeps its type roster as well, because that roster
makes a different and still-true claim — *do not aim `/damage` at a thing that
cannot take it*.

The two arms of the rule divide by **who can act on the defect**: a place whose
cell is known at compile time is *refused* (`DW0542`), because the author can
move it; a place only the runtime puts down is *skipped by the selector*
(`DW0545`), because nobody can.

`DW0545` is an emission self-check over the shipped datapack, in the `DW0420` /
`DW0421` family — it is `DW0421`'s rule (*only the owner may disturb an
affordance's hardware*) one verb wider, since moving hardware is disturbing it,
and one binding wider, since a region verb selects by box where `DW0421` reads a
tag. It fires on two clauses, and both are compiler defects rather than authoring
ones: a summon that declares neither class (the exclusion then protects nothing),
and a box-narrowed `@e` selector with no exclusion (the class exists and this verb
does not read it). Because it can never be caused or fixed by campaign JSON it is
`every_version`: fencing an engine self-check by `dsl_version` would let an older
campaign ship the defect in silence.

**The runtime half is the only half that can witness the original defect**, and it
is generated rather than argued: one PackTest template per (`teleport` × `stake`)
pair leaves a real marker in a real volume through the campaign's own
`stk_fill_<s>`, rides the campaign's own `teleport_<key>`, and asserts a plain
body **left** the box while both halves of the marker stayed. The body assertion
is what stops it being one-directional — without it, an engine whose teleport did
nothing at all would pass.

Binding: `validation/fixture-gate.json` states how many entities declared each
class, how many box-narrowed selectors were examined, and how many runtime
templates were generated. Zero on either of the first two counts is reported as
`unbound` **with an `unbound_reason` naming which arm** — an empty class makes
every exclusion decorative, while zero selectors means the class is bound and the
clause the defect lives in is simply not exercised by this campaign. The two are
not the same finding and the ledger never makes a reader guess which one it is.

| Code | Meaning |
|------|---------|
| `DW0540` | **A grant whose removal is a later effect, not its own duration.** A `give-effect` is still live at the moment a `clear-effect` for the same effect fires in the same bundle. Validation-tier (exit 1), `dsl::validate`. The message carries both numbers the author needs — how long the grant runs, and how long the bundle actually needs it for. Prescription: set `seconds` to the span the effect should last and delete the `clear-effect`; a duration expires with no cooperation from anything. `clear-effect` is for effects this campaign did not grant. |
| `DW0541` | **A duration that is not a duration.** A `give-effect`'s `seconds` is zero or past `MAX_EFFECT_SECONDS` (50 000, derived from `MAX_POTION_DURATION_TICKS`), or its `amplifier` is past vanilla's unsigned byte. Validation-tier (exit 1), `dsl::validate`. Zero is the grant that never happens — the unbound-vacuity class as a number; the ceiling is vanilla's own field width, so a value above it is a duration typed in ticks or milliseconds. |
| `DW0542` | **A teleport volume over an affordance bound to hardware.** A `teleport`'s `from` volume covers an interaction affordance the engine placed on a block it also places — an interact objective, a click trigger, a bonfire, a shortcut unlock, a trap or timed-gate disarm, a sealed gate's answer. Build-tier (exit 3), `compiler::teleport`. The teleport moves the entity and not the block, so the player is left with something they can see and reach that answers nothing. Prescription: move the affordance out of the volume, or shrink the volume's `extent`; do NOT add a type exemption to the selector — that would tear an NPC's dialogue hitbox off its body. |
| `DW0545` | **An engine fixture is reachable by a box.** Either an engine-summoned hitbox, mark or display declares neither class tag (`dw_fixture` / `dw_borne`), or a selector narrowed by a positional box (`@e[x=…]`) does not carry `tag=!dw_fixture`. Build-tier (exit 3), `compiler::affordance`, emission self-check over the shipped datapack. **A compiler defect, never an authoring one** — no campaign JSON can cause it and none can fix it; the message is addressed to whoever is changing the engine. Prescription for a new affordance: summon it declaring the class. For a new region verb: negate the class, never a `type=…` roster — a type cannot tell an NPC's dialogue hitbox from a recovery stake's marker, and a moving verb must carry the first and leave the second. |

### DW0543 — a prefab metadata key this delvec does not model (`compiler::registry`)

The prefab metadata document (`docs/reference/prefab-procedure.md` §9) has one
definition, `delvewright_dsl::prefab`, and every reader — `delvec`,
`delve-admit`, `delve-grammar`, `delve-render` — uses that type rather than a
local copy of the shape. `delvec` reaches it through the DSL crate because it is
published to crates.io and may only depend on published crates.

That type does not carry `deny_unknown_fields`, and the decision is the point.
The attribute is right on a document whose reader is also its owner — every
campaign stage struct keeps it, because a typo there is the bug it catches and
`dsl_version` handles forward compatibility. It is wrong on a **consumer**: a
content library and an engine version move independently, so a key newer than
the reader is the normal state of a mixed-version pair, and refusing it turns a
forward addition into a hard failure at the layer with the least context. One
key would have stopped **every** campaign building.

Ignoring the key is the other wrong answer, because a misspelled key looks
exactly the same from here. So the piece loads, the key is preserved on any
rewrite, and the reader says what it saw.

| Code | Meaning |
|------|---------|
| `DW0543` | **A prefab metadata file carries a key this delvec does not model.** Warning, `compiler::registry::PrefabRegistry::load_dir`, reported per file at every `validate`/`analyze`/`build`. The message names every unknown key, at the document root and per anchor, and states the two things it can be: a library newer than this engine (upgrade `delvec` to consume the key), or a misspelling of a key the document does define, in which case whatever it was meant to say is not being said. The prefab **loads** — it is not skipped, and this is not `DW0346`. Binding: the pinned content library must produce zero of these, which is what makes it a tripwire rather than noise (`tests/registry_load.rs`). |

### DW0544 — a runtime write that fills a region with fluid (`compiler::nav`; DSL v0.10)

A runtime region write concludes from the **block it writes**, not from the fact
that it wrote. Only a block that is a full collision cube leaves floor behind;
`minecraft:water` and `minecraft:lava` leave a cell a body sinks through, so
those writes mark their cells **flooded** — impassable, and never standable —
which is the set the model already carries for prefab-authored water. Water, lava,
any block state (`water[level=3]` is water), and with or without the `minecraft:`
namespace: an author's `fill-region` block is a hand-written string, a bare `water`
passes block validation and is emitted verbatim, and vanilla resolves it, so the
classifier (`assembled::is_fluid`) is namespace-insensitive like every other one
beside it. A **waterlogged** block is not a fluid: its cell is occupied by its host
block and is genuine floor, while its water spreads to neighbours — two questions,
and only the first is asked here.

Because a fill carries no `replace` filter, it *replaces* what was in the box, so
a fluid fill over floor takes the floor away. Where a fluid fill and a solid fill
overlap the fluid wins: a flooded cell is everything a walled cell is and one thing
more, which also makes the answer independent of declaration order (ADR-0006).

`DW0544` is derived from a counterfactual, exactly like `DW0510`: the leg is
re-routed over the identical world with every runtime fluid fill treated as solid,
and if *that* world routes, the fluid is what closed the leg and the boxes are
named. So it fires on the case where the box supplied **footing** — the author is
looking at a box they filled on purpose and must be told the fluid took the floor,
not sent to hunt a wedged doorway. A fluid fill laid *across* a path rather than
under it is an ordinary `DW0311`, because it would block the leg whatever block it
held; that message names the fluid fill in its hint rather than blaming the prefab.

**What is not modelled**: the fluid's spread beyond the written box. Vanilla flows
a source outward at world-tick; this marks the written cells and no more, so the
wet set can be under-marked. It is the same missing input as the limitation
`World::with_cleared` carries — a runtime block map to re-derive the flood from —
and a runtime fluid fill is now a second way to reach it: a later `clear-region`
next to a filled box is credited as dry, and the server may flood it.

| Code | Meaning |
|------|---------|
| `DW0544` | **A forced leg stands where a runtime write leaves fluid.** A critical-path leg has no collision-free path once runtime fluid fills are impassable and unstandable, but routes fine when they are treated as solid — or a visited objective's only footing lies in one. Build-tier (exit 3), `compiler::nav`. The message names the boxes the fluid-free route needs footing in. Prescription: fill with a block that is floor, put the walkable surface in the cell below the fluid, or route the forced path around the box; never swap in a solid you do not want in the world just to get green. |

### DW0546 — footing laid by a beat nobody has to play (`compiler::nav`)

A runtime write is registered with the answer to one more question than the block
alone can settle: **is the party guaranteed to cause this firing?**
(`plan::RegionEvent::is_forced`.) It is computed from the effect's root — a quest
`on_objective_complete` bundle, a quest `on_complete`, an environment trigger and
the world's own load-time seals are forced; a `traps[].payload`, a
`set-checkpoint` `on_respawn` bundle, a `shortcuts[].on_unlock`, the campaign's
`on_death` and a `shops[].offers[].effects` are not. The DSL carries **no field on
which an author can assert it**, and a `RegionEvent` cannot be constructed without
stating it, so the answer is a property of where the effect sits in the document
and nothing else.

The distinction exists because a solid block answers two questions at once and
only one of them is conservative when the firing is uncertain:

- *Is the party blocked?* Assume it happened. Assuming a wall can only make the
  proof harder, so the **blocking half of an unforced write is credited in full** —
  a `close-gate` in a trap payload still seals, and a forced path that must
  re-cross it still fails.
- *Can the party stand there?* Assume it did not. Assuming floor is what makes the
  proof easier, and easier is the direction that ships.

So an unforced fill is carried as impassable **and not floor**
(`World::with_unforced`) — the pointwise-worst of the two futures, and sound in
both. **A cell the assembled world already holds solid is left alone**, which is
what binds the rule to laying NEW floor rather than to re-surfacing existing
floor: if the box was floor before the write, it is floor whether or not the beat
fires, and there is nothing uncertain to model.

A **flood** takes no such split. Impassable and never floor is already the worst
of "the water is there" and "it is not", so an unforced flood is judged exactly as
a forced one and stays `DW0544`. A **clear** and an **unseal** never reach the
model unforced at all: `plan::collect_region_events` drops them, because an
unforced firing may make a region impassable and may never make one passable —
which is the same rule that keeps every shortcut gate sealed so the delve is
finishable the long way. Latest-write-wins needs no special case either: the
winning write carries its own forcedness, so a forced fill landing later on the
same box restores ordinary footing by winning.

`DW0546` is derived by counterfactual, exactly like `DW0510`, `DW0317` and
`DW0544`: the leg is re-routed over the identical world with every unforced fill
credited as ordinary floor (`RegionState::as_if_forced`), and if *that* world
routes, the unforced footing is what closed the leg. It is asked before the gate
counterfactual, because an unforced fill over a gate region would otherwise be
reported as a missing `open-gate` the author has already written.

The reading reaches the **exported** route by construction rather than by a second
rule: a leg is judged by the `DW0314` self-check in the world it was proven over,
and that world comes from the same `World::with_region_state`. So the root of a
runtime fill decides one thing in two places at once — a delve whose only route
crosses floor laid from a forced beat builds and exports a proven waypoint across
it, and the identical fill moved to a beat nobody has to play is `DW0546`.

| Code | Meaning |
|------|---------|
| `DW0546` | **A forced leg stands on footing laid by a beat the party can skip.** A critical-path leg has no collision-free path once fills fired from unforced roots are impassable-and-not-floor, but routes fine when they are credited as floor — or a visited objective's only footing lies on one. Build-tier (exit 3), `compiler::nav`. The message names each box and the beat that lays it (the trap, the shop offer, the shortcut, the death bundle). Prescription: fire the fill from an objective the party is FORCED to complete before the leg, build the floor into the prefab, or route the forced path around the box; never leave the path depending on a beat that can be skipped. A leg the unforced fill *walled* rather than floored is an ordinary `DW0311` whose hint names the write, for the same reason `DW0544`'s counterpart is. |

### DW0547–DW0549 and DW0555 — a campaign's contingent ways (`compiler::ways`; spec-0042, DSL v0.12)

A **way** is a region a piece's spatial contract declares its own traversal edge
contingent on: `laid` — empty as built, and opening fills it — or `cleared` —
standing in the way's block as built, and opening voids it. The prefab checker
proves the geometry on the bytes as shipped: the edge really is severed, applying
the delta really joins it, and every space reachable only under an opening is
named with the opening it needs. What it cannot prove is that anything ever
*opens* it — "happens" exists only where effects exist. That half is the
campaign's, and `open-way` is the whole of the surface.

**Three questions, one pass, at plan time.**

1. **Staging.** Which ways the placed pieces actually put in the world, with their
   world cells, their block and their sign, read through the same placement
   transform the face contract uses. A way is a fact about a PLACEMENT, not about
   a prefab: a piece placed twice puts two breaks in the world, so a reference
   that matches none or several names no one of them (`DW0547`). Ways of one name
   on several edges of one piece union, exactly as the grammar's own reachability
   walk unions them.
2. **Disposition.** Per staged way: which effect opens it, at which quest-DAG
   point, and whether the party is forced to cause that firing — or that nothing
   opens it. **A door that never opens is content**, so a never-opened way is
   reported and is not a finding by itself. Which verdict a way gets is computed
   from what is staged behind it; the author picks nothing.
3. **The one red that follows.** Required content standing beyond a way no forced
   opening precedes (`DW0548`).

**Whose reachability decides "beyond".** The piece's own: the contract's declared
graph, rooted at the `entry` space it declares, `vision` edges excluded and a
`drop` traversed forward only. That is deliberately the reading the grammar's
reachability walk takes, because the claim being consumed is the piece's claim.
Two consequences follow and both are stated rather than discovered — a space a
neighbour could reach through a mated exterior face is not counted as reached
(seams are the face contract's business, `DW0780`, and spec-0042 keeps ways off
them), and a `barred` edge is traversed (its bar is opened through the anchor
surface, which this pass does not model). Both make the red rarer, never commoner.

**What "in time" means, and it is not a second opinion.** An opening counts for a
required element when it is FORCED and the quest DAG guarantees it has already
fired: the predicate is `Plan::gate_fired_before`, the same strict-ancestor
relation the region-write model orders the world by, handed over rather than
re-derived. So an `open-way` on an objective a parallel branch merely interleaves
ahead does not count, and neither does one on the objective the party has to reach
by crossing the break.

**The binding count is the artifact.** Every build whose placed world stages a way
emits `validation/ways.json`: placed pieces, pieces declaring a way, ways staged,
opened, unforced-only, never-opened, `open-way` effects, required elements
examined, and one row per way with its cells, its block, its sign and every
opening that names it. A campaign that stages no way emits no file — a file
reading zero is a finding, and an absent file is the honest statement that there
was nothing to enumerate.

| Code | Meaning |
|------|---------|
| `DW0547` | **An `open-way` reference does not name exactly one placed way.** Either no placed piece stages a way of that name on that piece, or several do (the same prefab placed in two areas, or drawn twice by one pool). Build-tier (exit 3), `compiler::ways`. The message prints what the world does stage, way by way, with the area and placement index of each. Prescription: bind the way-carrying piece to one area, or give the second placement its own piece; a way is a fact about a placement, so one reference cannot open two of them. |
| `DW0548` | **Required content stands beyond a way no forced opening precedes.** A campaign-referenced anchor — an objective's target, an NPC stand, a wave spawn, a lane waypoint — resolves into a space the carrying piece's contract reaches only through a way that is never opened, opened only from a root the party can skip (a trap payload, a shop offer, a death bundle, a shortcut's far side), or opened at a quest-DAG point that does not precede the element. Build-tier (exit 3), `compiler::ways`. The message names the way, the effect (if there is one) and the element, and states which of the three it is; a never-opened way also carries the cell count of the building standing behind it. Prescription: give the way a forced `open-way` on an objective the quest DAG puts before this one. Nothing else about a never-opened way is a finding. |
| `DW0549` | **A placed piece declares a way the staging could not put in the world.** The placed pieces declare more distinct ways than reached the enumeration — a way whose `boxes` resolve to no cells at all. Build-tier (exit 3), `compiler::ways`. A way's whole content is the cells its opening writes: one that resolves to none is a break nothing can repair, and every disposition, ledger count and reachability verdict past that point would be stated over a world smaller than the one being shipped. The message carries both numbers. Prescription: fix the piece's metadata. |
| `DW0555` | **The way-reachability check examined zero required elements** (advisory). Ways are staged and no objective anchor, body or campaign reference resolves into a declared space of any way-carrying piece, so the dispositions are reported and nothing proves an opening is needed for anything. Warning, `compiler::ways`. Not a refusal: content behind no way at all is ordinary, and a delve whose ways are scenery is a delve. What is not acceptable is for that to be indistinguishable from a proof. |

### DW0510–DW0512 — lethal volumes (`compiler::nav` / `compiler::lethal` / `dsl::validate`; spec-0031, DSL v0.10)

A lethal volume is **geometry that kills**, so most of its completability
reasoning is not a check of its own: [`nav::World`] carries its cells as
impassable — one of the six premises `nav::Premises::of_plan` applies to every
world built from a campaign, on every arm — and every route proof in the engine
inherits that for free: the critical path, the checkpoint no-stranding proof
(`DW0315`), the branch paths, the trap forced-cell set, the exported harness
waypoints. That is the same move `close-gate`'s seal makes, and for the same
reason: a fourth consumer inherits the proof instead of re-deriving it.

`DW0510` exists because the *fix* for a blocked route differs in kind. A generic
`DW0311` sends the author to look for a wedged doorway or a void gap; here the
geometry is walkable and a **declaration** closed it. So the failure is derived
from a counterfactual — the leg is re-routed over the identical world with
lethality removed — and names the volumes covering that route.

`DW0511` is the one obligation routing cannot see, and it is one rule because it
is one defect: **a body put here by declaration rather than by walking.** Two
families fall under it. A *respawn seat* (entry spawn, `set-checkpoint`,
`bonfire`) means the party dies on arrival and is re-seated to die again, forever
— `/spawnpoint` is only a hint and the engine re-seats on the death edge, so
nothing downstream can rescue it. A *posted body* (a stage-2 NPC's anchor, a
per-quest `cast` placement, a stage-5 actor's anchor) is deleted on the first
tick: the volume's entity sweep exempts the engine's own machinery types and
deliberately not content bodies, so the delve loses its speaker in silence while
every static proof stays green. The second family was found while writing this
feature's own CI fixture, whose first draft put the volume on the Keeper's post.

Binding (playtest-methodology rule 1): a campaign with a volume emits
`validation/lethal-gate.json` — volumes declared vs. resolved, world cells closed,
posted places examined (`respawn_seats_examined`, which counts both families),
critical-path legs routed, and PackTest templates generated (one per volume). A campaign with no volume emits **no file at all**, so
a file that exists and reports zero is a finding rather than an absence.

| Code | Meaning |
|------|---------|
| `DW0510` | **The only route runs through the volume.** A forced critical-path leg has no collision-free path once the declared lethal volumes are impassable, but routes fine without them — or a visited objective's only footing lies inside one. Build-tier (exit 3), `compiler::nav`. The message names the volumes the lethality-free route crosses. Prescription: move or shrink the volume, or give the party a route around it — never delete the volume to silence the proof. |
| `DW0511` | **A posted place inside the volume.** Somewhere the campaign requires the party or a declared body to BE lies inside a lethal volume: the entry spawn, a `set-checkpoint` / `bonfire` cell (the death loop), or an NPC anchor, `cast` placement or actor anchor (a body the volume deletes on the first tick). Build-tier (exit 3), `compiler::lethal`. The message names the post and the volumes covering it. Prescription: move the post out, or shrink the volume's `extent` so it does not cover it. |
| `DW0512` | **A volume that kills in silence.** A `lethal_volumes[]` entry's `message` is blank. Validation-tier (exit 1), `dsl::validate`. There is no compiler-owned default that could be right for a cliff, a lava pit and an acid pool at once, so a blank wording is refused rather than papered over — a volume that kills while the player learns nothing is the vacuous pass CLAUDE.md names. Prescription: write the line the player reads as they die. |

#### The wording is a consequence of the blow, never a prediction of it

**Vanilla refuses damage far more often than "the target is dead" suggests, and
it says so while doing nothing.** A player is invulnerable for **59 ticks (~3 s)
after respawning**, and `/damage` — like `/kill` — reports success and changes
nothing (spec-0031 spike). A totem, `resistance 5` and an already-dead entity
refuse it the same way.

So a volume that printed its wording *before* swinging would tell a player *the
undertow takes you* once per tick for three seconds while they stood in it alive
— the delve asserting an outcome that did not happen. That is the same defect as
the eight legacy camelCase gamerules the same spike found: **neither offending
site read the command's response.**

The obvious fix is wrong, and this was measured rather than reasoned about.
`execute store success ... run damage` is **inert here**: on the pinned 1.21.11
toolserver a PackTest dummy in `playerGameType: 0` with `Invulnerable: 0` and
`Health: 20f` took `damage @s 1000 minecraft:fall`, ended on `Health: 20f`, and
the command answered **success = 1**. Reading a response that does not carry the
answer is the same defect as reading none. The guard therefore reads the
**outcome** — the player's health after the blow (`#leth_hp dw.sys`, reset to a
sentinel first so a failed `data get` cannot leave a previous player's zero
behind) — which covers every refusal in one rule and needs no list of them.

**What the PackTest tier can and cannot witness.** A PackTest fake player is
**permanently undamageable**, not merely spawn-invulnerable: the same dummy stood
inside a volume whose loop swings every tick and was still at `Health: 20f` after
**202 ticks**, far past the 59-tick window, with `minecraft:generic` refused
identically. So a player *death* cannot be witnessed at this tier at all, and the
generated templates do not pretend to — that claim belongs to the bot tier, which
drives a real client. What the dummy is ideal for is the opposite direction: a
body that provably never dies must never produce the claim, which makes it a
standing fixture that never expires. The two generated templates split on exactly
that line, and each is red for its own reason (measured): `lethal_<id>` fails when
the damage amount is stripped, `lethal_<id>_claim` fails when the player line is
deleted from the driver. That second binding is why the claim template drives the
**driver** and not the kill function — calling the kill function directly passed
12/12 with the player path deleted.

#### Why the wording is a component, and not a custom damage type

Vanilla's own spelling for "a death message the pack wrote" is a datapack
`damage_type` with a `message_id`, whose key the client resolves from a lang
file. It is **rejected here**, and the reason is an existing invariant rather than
a preference: vanilla builds that message with no `fallback` field, so a player
who declines the resource-pack prompt would read a raw `death.attack.…` key —
which spec-0029 §3 makes the delve's playable-in-English guarantee against, and
which `DW0185` would not catch (the emitted literal is the key, not the authored
string). The wording therefore travels the one path every player-visible string in
this engine travels, `emit::tr` → `{"translate":…,"fallback":…}`, and vanilla's
own broadcast still fires, worded by the declared `damage_type`: the party reads
*who* died, the victim reads *what the place was*.

### DW0520–DW0527 — trade and the recovery stake (`dsl::validate` / `compiler::stake`; spec-0032, DSL v0.10)

**There is no price diagnostic here, and its absence is the design.** A price is a
[`Gate`] term — the numeric comparison spec-0031 put in the shared gate rather than
in the verb that first asked for it — so everything that could go wrong with one is
already `DW0500`–`DW0503`: the datum must be declared, must be written somewhere,
must be read somewhere, and must be reachable at the scope the site evaluates at. A
shop that had grown a `price` field would have needed all four rules written a
second time, and the fifth consumer would have needed a fifth copy. `ShopOffer` is
therefore the **seventh gate consumer**, carrying `requires_flags` /
`forbids_flags` / `requires_state` like the other six and nothing of its own;
`crates/dsl/tests/gate_consumers.rs` asserts the positive half from the generated
schema and `crates/compiler/tests/v10_economy.rs` the negative half (no `price`,
`cost` or `compare` field exists anywhere in the shop's types).

`DW0520`–`DW0524` and `DW0527` are declaration and authoring rules and live in `dsl::validate`. `DW0525` and
`DW0526` are the **placement table's** proofs and live in `compiler::stake`,
because where a stake lands is a question about the solved layout — the same split
a lethal volume's `DW0512` and `DW0510`/`DW0511` make.

#### The placement rule, and why it is a table rather than a search

The rule: *the stake anchor is the point, on the walkable path from the respawn
point in force at the moment of death to the death point under the quest state in
force at that moment, that minimises distance to the death point.*

Read literally that is a runtime search, which ADR-0006 forbids. Read as a function
of three compile-time quantities it is a table, and every quantity already has an
owner: **walkable** is the same `nav::World` the completability proof runs on
(including a lethal volume's impassable cells, so "the near lip of the hazard"
falls out rather than being a second rule); **the quest state** is the DAG-indexed
sealing `close-gate` established (`nav::seal_configurations`); and **the respawn
point in force** is engine state the runtime already keeps in `#cp dw.sys`.

The rule degenerates, which is why there is only one rule: a player who dies on
ground they can walk back to is at distance zero from themselves, so the anchor is
the death point. Only deaths whose position **cannot** host a stake need a row, and
there are exactly two kinds — a death inside a **lethal volume**, and a death on a
block **runtime can remove** (a lift car, a `close-gate` region, a `collapse`
floor; spec-0031's ruling that a stake left on the car would be deleted by the next
ride). Both are boxes, so the runtime lookup is a selector test on the corpse
(`@s[x=…,dx=…]`) rather than a search.

#### The three ways a stake can be pulled out from under itself

Two are `DW0526`'s, one is not, and the third is named rather than left silent.

| how | in scope? | why |
|---|---|---|
| **Runtime-mutable ground** — `close-gate`, `set-block`, `collapse`, a shortcut's or a timed gate's seal | yes | the case spec-0031's ruling was written for: a stake left on a lift car is deleted by the next ride. |
| **`fill-region` / `clear-region`** | yes, and it is *the same defect* | a `clear-region` deletes the block a marker stands on exactly as a departing car does. They enter through `QuestEffect::region_write` — the DSL's own answer to "which verbs rewrite a box" — so a later verb of that family is covered by existing rather than by being remembered. |
| **A `teleport`'s `from` box** | **no — a deliberate ruling; closed by `DW0545` one layer away** | a teleport moves *entities*, not blocks: the ground under the marker is untouched, and what moves is the marker itself, away from the position the collecting player's ledger recorded — after which `stk_gc_<s>` finds nobody holding a wager there and retires it, taking the wager with it. Different defect, different fix, and not one a box check on this axis could state — `DW0526` is about **footing**, and a marker's position is chosen at RUNTIME, so no compile-time geometry test knows where it will be. |

The teleport case cannot simply inherit the teleport's own `DW0542` either, and the
reason is the shape spec-0031 named when it refused to inherit `lethal_volumes[]`'s
exemption list into a verb that *moves* rather than *deletes*: `DW0542` tests the
affordance authority, which carries compile-time cells, and a stake has none to
offer it. Inheriting it would have produced a green that examined nothing.

**Neither of the two fixes the finding proposed was taken, and nothing in
`compiler::stake` changed.** "The teleport exempts engine machinery" builds a
roster into one verb; "the stake ledger survives its marker moving" makes the
stake compensate for a selector that grabbed something it should never have
grabbed. The question was upstream of both — *what does a region verb select?* —
and a marker being a **place** is a property of the marker, not of any verb. So
the class is declared where the marker is summoned and every box-narrowed
selector reads it (`DW0545` above). That a capability keyed to the object needed
no cooperation from this module is the point rather than a coincidence.

**Note the direction of the conservatism, because it is why this set is not
`Plan::region_events`.** The completability model deliberately drops a non-fill
write fired from an **optional** root — an optional firing may fill, never open —
because a route proof must not lean on a clear the party might never trigger. This
set needs the opposite: a `clear-region` in an `on_death` bundle the party may
never reach is still ground a stake must not stand on, because if they do reach it
the marker is gone. Same geometry, opposite direction, so the two lists cannot be
one.

**Two conservative simplifications, recorded rather than hidden.** spec-0032
already records one — *reachable under the quest state* stands in for *explored*,
which the engine does not track. The implementation adds a second: nothing
observable at runtime says which point of a respawn point's DAG span a death
happened at, so the reachable set used for a seat is the **intersection** over
every sealing configuration that can hold while that seat is in force. The anchor
is then reachable under all of them, which is strictly stronger than the rule as
written and needs no runtime discriminator for quest state at all. A campaign with
no `close-gate` has exactly one configuration and pays nothing.

#### What the runtime tiers can and cannot witness — stated, not implied

**A PackTest fake player is permanently undamageable and cannot die** (measured
twice independently, 2026-08-03 and 2026-08-09). So that tier cannot witness a
player death, and therefore cannot prove the edge from a death to a stake being
placed. Two templates are generated and both are honest about what they cover:
`v10_shop_purchase` drives an offer handler as its own dummy and proves the debit
and the refusal; `v10_stake_<id>` drives `stk_drop_<id>` and `stk_collect_<id>` and
proves that the declared share leaves the purse, that a marker really stands where
the drop put it, that collecting returns **exactly** what was taken, and that a
second collection in the same breath returns nothing more. No template is generated
for the death edge itself — a template that bound to nothing and reported green is
the vacuity CLAUDE.md names, and it is worse than an absence because review cannot
see it.

**The open obligation, stated the way spec-0031 stated its own.** spec-0032's
acceptance criterion 9 — *die in a lethal volume, respawn, walk back, collect, with
the amount asserted* — is a **bot-tier** claim and is NOT discharged by anything in
this repository today. The compile-time half (the table, its two proofs, the
binding ledger) and the runtime half either side of the death (the shop, the drop,
the collect) are proven; the death itself, and the walk back to the anchor the
table chose, are not. The first campaign to declare a `stakes[]` entry must carry
that bot-tier proof, exactly as spec-0031 obliged the first campaign declaring
`on_death` to carry a bot-tier proof of the corpse-side fire. It is not the
campaign author's discretion.

**That proof now exists**: the harness's `death-loop` stage walks a real
client into every declared lethal volume, dies there, and asserts the volume's
wording, the declared forfeit, the stake's presence at the table's own anchor, the
walk back, an exact restore under a double right-click in one tick, the retirement
of the collected hardware, and the respawn seat — all against
`validation/death-plan.json`, never against the emission. `.github/workflows/release.yml`
runs it over `crates/compiler/tests/fixtures/economy`.

**And the die-retry loop, which had never run.** That stage — death → respawn at
the governing checkpoint → walk back → re-engage — needs an ARMED checkpoint
before a mandatory encounter, and measured 2026-08-11 across both repos, no build
had one: `keep-trial` and `hollow-vigil` field encounters with no checkpoint
before them, `nobodys-cave-island` compiles to zero mandatory encounters, the
drowned bell has no stage documents. The stage had therefore reported green over
zero scripted deaths everywhere it had ever run.
`crates/compiler/tests/fixtures/die-retry` is the smallest campaign that lets it
bind, `crates/compiler/tests/die_retry_fixture.rs` holds it to that shape, and the
run report's `die_retry_binding` makes the zero loud on every other build.

**What it found on its first live run**, and what nothing else could have: `on_death`
and the checkpoint respawn dispatch **never fired on a player's FIRST death**.
`dw.death_seen` and `dw.death_ack` are `dummy` objectives, so a player who has never
died has no score in either — and `execute if score @s A > @s B` with B unset does
not fire (measured on the pinned 1.21.11 server; `scoreboard players add <e> <obj> 0`
is what creates the entry at zero). Both edges then worked from the second death
onward, which is why every compile-time shape proof and every manual test that dies
twice passed. `cp_respawn_check` now seeds each acknowledgement it reads, ahead of
the comparison; `v06_checkpoints` and `v10_on_death` assert the ORDER, not merely
the presence.

Binding (playtest-methodology rule 1): a campaign with a stake emits
`validation/stake-gate.json` — stakes declared, respawn seats and death regions the
table is keyed on (and how many of those regions are lethal volumes), quest-state
configurations enumerated, rows proved, distinct anchors resolved, runtime-mutable
cells excluded, and stranded cells found. A campaign with no stake emits **no file
at all**, so a file that exists and reports zero is a finding rather than an
absence.

| Code | Meaning |
|------|---------|
| `DW0520` | **A stake that is not a personal wager.** A `stakes[]` entry's `state` names a datum the campaign never declares, or one declared `party`-scoped. Validation-tier (exit 1), `dsl::validate`. The scope half is stated as a rule because it is the multiplayer decision most likely to be made by accident: a shared purse turns a teammate's death into a penalty on everyone and nothing in the JSON would say so. Prescription: declare the datum `player`-scoped, or point the stake at one that is. |
| `DW0521` | **`drop-stake` names no declared stake.** Validation-tier (exit 1), `dsl::validate`. Prescription: declare it in `stakes[]`, or fix the id. |
| `DW0522` | **A stake nothing ever drops.** A declared stake that no `drop-stake` effect anywhere in the campaign leaves: its forfeit rule, its retention policy and its whole compile-time placement table describe a mechanism no beat can fire. Validation-tier (exit 1), `dsl::validate`. The vacuity rule `DW0502` states for a datum with no reader, applied to a whole feature. Prescription: drop it from a beat (`on_death` is the usual one), or delete the declaration. |
| `DW0523` | **A shop button that cannot answer.** A `shops[].offers[]` entry with no `effects` — drawn, pressable, inert — or a `shops[]` entry with no offers at all, which is worse than an empty shop: vanilla's 1.21.11 dialog codec rejects an empty action list at pack load. Validation-tier (exit 1), `dsl::validate`. A **refusal counts as an answer**, so an offer whose only effect is a `narrate` gated on `at-most <price − 1>` satisfies it — which is exactly the shape spec-0032 asks for. Prescription: give the offer effects, or delete it. |
| `DW0524` | **A forfeit above the whole purse.** A `forfeit` of kind `proportion` whose `percent` exceeds 100. Validation-tier (exit 1), `dsl::validate`. Prescription: 0–100, or `{"kind": "all"}`. |
| `DW0525` | **No walkable route back.** From some respawn seat, under every quest state that can hold while it is in force, there is no reachable cell a stake could stand on for deaths in some region — or there are cells a player can walk to and die on that the seat cannot reach at all (the one-way drop). Build-tier (exit 3), `compiler::stake`. The message names the death region, the seat and how many quest states were examined. Prescription: give the drop a way back (a shortcut, a ladder), or declare the place a `lethal_volume` so the stake is projected to its near lip instead — never delete the stake to silence it. |
| `DW0527` | **A comparison read after the bundle changed what it compares.** An effect's `requires_state` names a datum that an earlier effect in the same bundle writes **behind a gate on that same datum** — so the comparison is made on the far side of the boundary the bundle just tested. Warning-tier (exit 0), `dsl::validate`. Found in the emitted output of this feature's own first shop: written "purchase, then apology", buying your LAST coin debits it and the `at-most` apology — evaluated after the debit — then holds too, so the player is charged AND told they cannot afford it. The fix is always local: put every reading effect ahead of the write. An **unconditional** write followed by a comparison is deliberately NOT diagnosed — `set-state toll 0` and then a door gated on `toll at-most 0` is the ordinary sequenced idiom and plainly means the value the bundle just produced. **Its scope is ONE bundle's own effect list, and that is what it does not cover**: a write and a read four beats apart are two bundles, so a `clear-state` that empties a datum a later objective's gate depends on is invisible here. `DW0879` is that question, asked over the path rather than over a list. Prescription: reorder, or gate on something this bundle does not change. |
| `DW0526` | **No safe footing.** Every cell reachable from the seat that a stake could be projected onto for some death region stands on a block the runtime removes — a lift car, a sealed gate region, a collapsed floor — so a marker left there would be destroyed by the next ride. Build-tier (exit 3), `compiler::stake`. Distinguished from `DW0525` because the prescription is the opposite: there IS a route back, and the ground it ends on is the problem. |

### DW0495 — emitted score-read integrity (`compiler::seeding`; error; exit 3)

**The runtime fact, measured on the pinned 1.21.11 server** (rcon plus a real
client joined; the transcript is in the PR that landed this): a scoreboard entry
does not exist until something **writes** it — joining creates nothing, a
`deathCount` objective has no entry until the player dies, a statistic objective
none until the statistic moves, a `trigger` objective none until it is enabled —
and **every** comparison against a holder with no entry is FALSE. `if score X O
matches 0` does not fire; nor does `matches 0..`; nor does `if score A oA > B
oB` with either side missing; `unless` is correspondingly always true, and
`scores={O=…}` matches no entity lacking an entry in `O`. `set`/`add`/`remove`/
`enable` create the entry, `operation` creates **both** its target's and its
source's, `execute store … score` creates it, and `reset` destroys it.

So an unwritten score is not zero. It is *false to every question*, including the
questions whose honest answer at zero is yes — which is why this is a defect
class and not one bug.

| Code | Meaning |
|------|---------|
| `DW0495` | **The compiler emitted a comparison against a scoreboard entry it never creates.** Build-tier (exit 3), `compiler::seeding::check_tree`, run last over the finished output tree, beside `DW0497` and the affordance self-check, on the same principle: judge the commands that ship. **The motivating instance**, latent since spec-0012 and found by the bot tier's first live death: `dw.death_ack` and `dw.death_seen` are `dummy` objectives, so a player who has never died has no entry in either, and `execute if score @s dw.deaths > @s dw.death_ack` did not fire. `on_death` and the checkpoint respawn dispatch therefore never ran on a player's **first** death — no forfeit, no recovery stake, no `on_respawn`, no engine re-seat — and both edges worked from the second death onward, which is exactly why it survived every shape proof and every manual test: anyone testing a death loop dies more than once. **Relation to `DW0501`.** That rule is the same insight one layer up and binds to a different object: a campaign-**declared** datum whose `requires_state` gate no verb ever writes, decided from the campaign JSON. Engine-internal objectives — `dw.death_ack`, `dw.cast`, `dw.dmask` — are declarable by nobody, so no campaign-layer rule could ever have reached them. This is the emitted-layer sibling, not a widening. **Evidence: a comparison is admitted on any one of four forms of one demand — the entry exists by the time this runs.** (1) *A write*: an unconditional write of that `(holder, objective)` earlier in the same body, or in a function the body calls unconditionally before it. (2) *A spelling*: the answer on a missing entry equals the answer at the baseline 0, i.e. any `matches` range that **excludes 0**, in either sense — the flag idiom §`set-flag` already documents, stated as a property instead of left to folklore; a range spanning the whole of `i32` is admitted separately, as the deliberate *does an entry exist* probe the generated PackTest suite is built on. (3) *A guard*: a conjunctive `if score <h> <O> matches <R>` with `0 ∉ R` — earlier in the same `execute` chain, or a sibling clause of the same `scores={…}` block — proves the entity has an entry in `O`, and therefore in every objective the pack always writes **alongside** `O`; that co-write group is computed from the tree (the bodies that can leave `O` holding a value `R` admits, intersected), never declared, which is what makes a stake ledger's `kx/ky/kz` provable behind its own `kl matches 1` and a shop's `shop_at` behind its `shop=1`. (4) *A driver*: the objective is written unconditionally, for an entity, by a function the `minecraft:tick`/`minecraft:load` chain reaches **without crossing a single condition** — the once-per-player seeding hooks (`state_seed`, `class_arm`), which land on a player's first tick before any player-driven site can ask. **Named limits, both of which admit rather than accuse:** ordering *within* one tick is not modelled; and a `#`-prefixed holder is not an entity (vanilla's own convention for a compiler-owned singleton, whose whole lifecycle is one emitter's arm/read pair), so for those the demand is only that something in the pack writes it. **Binding.** The check reports a census — total comparisons, entity reads, and which evidence admitted each — so a walker that stopped matching cannot read as a pass; `crates/compiler/tests/score_seeding.rs` floors both the totals and the entity reads per fixture, and separately floors the guard and driver rules on `economy`, the one fixture that exercises both. The message lists every unbacked read with its artifact path, line number, the whole command and the `<holder> <objective>` it reads. **Prescription: fix the emitter** — seed the entry on a path that reaches the comparison (`scoreboard players add @s <obj> 0` is idempotent and, on a `deathCount` objective, does not disturb the criterion), or write the comparison so a missing entry cannot change its answer. Never silence it by deleting the comparison. |

### DW0497 — emitted call-graph integrity (`compiler::integrity`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW0497` | **The compiler emitted a `function <ns>:<name>` call to a function it never emitted.** Build-tier (exit 3), `compiler::integrity::check_tree`, run last, over the finished output tree — beside the affordance-hardware self-check, and on the same principle: judge the commands that ship, not the intent behind them. **The class.** Nearly every verb compiles in two halves — the *call site*, lowered from the effect tree wherever the author put the verb, and the *machinery*, emitted from a per-feature registration walk. When those two walks disagree about what exists, the call site still emits, vanilla resolves an unknown function to nothing at all (no error, no log line, nothing a bot can observe), and the verb simply never happens. **The motivating build** is the island's round 21: `wave/storm-surf` was fired from a top-level effect chain and got its full machinery; `wave/storm-shore` and `wave/storm-fire` were fired from step 7 of a `sequence`, and the wave emitter — which resolved a wave's area only from top-level chains — produced no `spawn_…`, no census, no brand, no kill reward for either, while `seq_under_ram` shipped `function nobodys-cave-island:spawn_storm_shore` all the same. Two of three storm waves never spawned; every build-tier proof was green, and the only thing that noticed was the compiler's own generated census PackTest — which walks `waves[]` rather than the effect tree — failing on a live server four minutes into a ladder run. Landing this check surfaced a **second, independent instance** immediately: `spawn-npc` on a non-`deferred` NPC compiled `function <ns>:spawn_npc_<id>` against a function only ever emitted for `deferred` NPCs, so a character brought back after a `despawn-npc` stayed gone. **Model:** every emitted `.mcfunction` in every tree is scanned for calls in command position — bare, after `run`, after `schedule` — and each target in the campaign's own namespace must name an emitted `data/<ns>/function/**` body. Deliberately **feature-blind**: the rule is "a call has a callee", which needs no knowledge of waves or NPCs and therefore guards emitters not yet written. Scope: the campaign's own namespace only (`minecraft:…` belongs to a tree this compiler does not emit); functions, not function tags (`function #<ns>:<tag>` is skipped, tag membership being a separate artifact); and **tiered** — the shipped `datapack/` ships alone (ADR-0010) so it may only call itself, while `packtest-datapack/` and `creator-datapack/` load beside it and may call their own tier or the shipped one. PackTest `test/` bodies are callers but never callees. The message lists every dangling call with its artifact path, line number, the whole command, and the missing target. Prescription: **fix the emitter** so its call walk and its machinery walk derive from one traversal — this is a compiler defect, never content. Never silence it by deleting the call site: the call is what the author asked for. |

### DW0185 — untranslated player-visible literal (`compiler::emit`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW0185` | **An authored player-visible string reached the built tree outside a text component.** Build-tier (exit 3), `emit::check_untranslated_literals`, run last over the finished output tree — beside `DW0497` and the affordance self-check, on the same principle: judge the bytes that ship, not the intent behind them. **The class.** i18n v2 (spec-0029) makes every authored string a `{"translate": …, "fallback": …}` component so a client can render the player's own language. The risk that change carries is a string that *cannot* land in a component — it would ship as a literal no lang file can reach, silently untranslatable, which is exactly the defect v2 exists to remove. Rather than enumerate the emission sites once and trust the list to stay true, the compiler makes it an invariant: each inventoried string enters emission carrying its l10n key in a reserved private-use tag (`dsl::l10n::tag_translatables`), an emitter either lowers it through `emit::tr`/`emit::snbt_component` or reads it through `dsl::l10n::plain`, and **a tag still present in the finished tree is a site that did neither**. Deliberately feature-blind, so it guards emitters not yet written. **Scope:** every emitted file, plus the compiler-authored resource-pack assets before they are zipped. A file that is neither UTF-8 text nor a **classified** verbatim binary output (`.nbt`, `.png`, `resourcepack.zip` — byte copies of input assets the compiler writes no string into) also fails here, so a new binary artifact cannot quietly opt out of the scan. The message lists every offending artifact with the key and the line. **Prescription:** lower the string through the component helpers; or, if the site is genuinely not a component and never read by a player, read it through `dsl::l10n::plain` **and** add it to the named-exclusion table in §2 "Language delivery". Never silence it by dropping the string. |
| `DW0186` | (i18n v2 addendum) A campaign l10n sidecar defines a key in the reserved `delvewright.` **chrome** namespace. Those are the engine's own on-screen strings — `New objective: `, `Choose your class`, a bonfire's default labels — owned by the compiler, shipped translated with it, authored by no campaign; a sidecar row under that prefix would be written into the language file and silently replace product chrome for that language. `DW0181` also flags it as an orphan; this names the reason. |
| `DW0187` | (i18n v2) An l10n sidecar row was translated from English the campaign no longer holds: its `source` entry differs from the key's canonical English (or names a key the sidecar does not translate). The translation is present, applied and wrong, and no key-set check can see it — `DW0180`/`DW0181` compare key SETS and a rewritten line moves no key. Load-bearing for entity display names, whose key belongs to the first site declaring a given text: renaming one body hands its key to another, so the stale row is not the one the author edited. Fix by re-translating the key and updating its `source` — `tools/i18n-translate.py` does both. |
| `DW0188` | (i18n v2) An l10n sidecar records `source` provenance for only some of its rows, or none, so `DW0187` cannot see the rest. **Warning tier**, stating the unguarded row count: `source` is additive and this is the one-version deprecation window before it is required. The count exists so an unadopted sidecar is a reported number on every run rather than a silence that reads like a pass. Adopt by re-running `tools/i18n-translate.py` — it records provenance for rows it already has, and retranslates nothing. |

#### The branch artifacts (validation metadata)

Two outputs, emitted only for a campaign that declares `branch_points`, both pure
functions of the campaign document and therefore byte-identical across builds
(ADR-0006). They live under `validation/` and are hashed into `manifest.json`
like `critical-path-waypoints.json` — **never** part of the shipped datapack.

- **`validation/branch-plan.json`** — per branch: its id, the alternative taken
  at each point, its flag assignment (`set` / `unset`), where its fork opens,
  what it `leads_to`, whether it is reachable, the **dialogue choices that enter
  it**, the endings it reaches, its **critical path computed under that branch**
  (the flow-level `quest` / `objective` / `talk_option` step list), and the names
  of its two companion files (`chronicle`, `path` — `path` is `null` exactly when
  the branch is unreachable). This is what the harness scripts a per-branch run
  from.
  An **entry choice** carries `npc`, the option's 1-based index across that NPC's
  tree, and — the field the harness actually uses — the `command` that takes it:
  `/trigger dw.dlg_<npc> set <n>`. A 1.21.11 dialog button is drawn by the
  CLIENT, so no bot can click one; every option the compiler emits is backed by
  the trigger line the button itself runs, and chatting it is the player-legal
  primitive the button stands for — the same substitution the exported critical
  path has made for `talk-to` steps since spec-0002 was amended. The command is
  emitted rather than left to the harness because reconstructing it means
  reproducing `safe_local`, i.e. game logic in a harness that holds none. The
  option index is resolved against **the tree of the NPC the step's own
  `talk-to` names** — the same ordinal in another NPC's tree is a different
  option of a different speaker.
- **`validation/branch-path-<branch>.json`** — one branch's **executable** path,
  emitted per reachable branch, in the ordinary `critical-path.json` contract
  (`format_version` 2, the same steps, the same `transport`/`sneak`/
  `cutscene_seconds` markers, the same spliced bonfire `rest` steps). Built by
  the *same* `plan::build_critical_path` the exported path is built by, driven by
  the playthrough of the world that realizes the branch (`Plan::branch_critical_path`)
  — so the branch a campaign already exports gets a **byte-identical** file, and
  "branch coverage" is coverage of the contract the ladder already proves rather
  than of a second, less-tested one. The branch's scripted dialogue choices are
  *inside* it: each `talk-to` step carries the `/trigger` line of the option
  belonging to that branch — and, since `cast::station` reads the flag state THIS
  branch holds at that step, its **position** as well: two branches that stage the
  same NPC at different anchors get two different cells for the same beat. A bonfire's `fire_step` (an index into the exported
  path) is translated onto a branch path through the **objective** its firing
  beat names, because a fire is armed by a beat and not by a position; a beat
  that does not happen on a branch arms nothing there. A step's `transport` is a
  **contract with the datapack**, not just a harness hint: emission carries every
  branch-only crossing as a flag-gated `teleport` in that objective's
  `complete_<obj>` bundle (`DW0494` above). Not emitted for an
  unreachable branch — there is no world that plays it, and `DW0482` has already
  failed the build. **Waypoints are not yet per-branch**:
  `critical-path-waypoints.json` legs are consumed in lockstep with the exported
  path's walked positions, so a branch whose path differs walks under single-goal
  navigation and the run report says so.

**Known gap — a branch path is FLOW-proven, not NAV-proven.** `DW0204`'s replay
and the `DW048x` proofs judge a branch's *story*: its steps are ordered, its gates
satisfied, its cast selected, its ending reached. The **geometry** proof
(`nav::check_critical_path`, `DW0311`) still runs over the exported path only, so
a branch can be structurally perfect and physically unwalkable — its route may
cross a gate that only a *sibling* branch opens. The first live branch run found
exactly that in the reference fixture: `branch/bolt` ran for the exit through a
portcullis that only `obj/watch`, a hold-branch beat, ever lifted, and the bot
reported `No path to the goal!` on ground the compiler had never claimed. The
dynamic layer caught it, which is the two-layer split working — but the static
layer should own it. Extending `DW0311` (and the per-branch waypoint export it
would produce) to every enumerated branch is the follow-up; until it lands, a
branch's walkability is proven by running it, and `validation/branch-runs.sh` is
therefore not optional for a branching campaign.
- **`validation/branch-chronicle-<branch>.md`** — the 流水账: every reachable
  node's `happening` line in the order the compiled graph plays them, readable
  start to ending, followed by the undated ambient beats and the endings reached.
  The dated account carries four kinds of line — `quest`, `objective`, `choice`
  and `effect`. A **`choice` line is the dialogue option this branch takes to
  complete a `talk-to` beat**, and it is where a fork's divergence lives: a
  `DialogueEffect` carries no `happening` of its own, so the option is the only
  thing on that side of the campaign that says what the choice did to the story.
  The line names the option's own NPC, the node it stands in and its 1-based
  ordinal — `npc/marshal dlg/marshal-read#2` — so a citation table can point at
  it. The ordinal is resolved against **that NPC's tree**, the scope
  `flow::flatten_trees` assigns it in and `plan::plan_npc` emits it in; the same
  ordinal in another tree is a different option of a different speaker. An option
  carrying no `happening` contributes no line, and the compiler never invents one.
  The SKELETON (ordering, reachability, which nodes appear) is derived machine
  truth — it is exactly the order `Flow::journal` replays, which is exactly the
  order `Flow::replay` proves; only the flesh (each line's text) is authored,
  node-locally. This is the **decompilation principle** (spec-0025): the
  generation workflow is natural language → design doc → DSL, and whether the DSL
  matches the design is not something an LLM can check by simulating compilation
  in its head — so the compiler compiles the DSL *back* into natural language and
  the reviewer compares NL against NL. Narrative incoherence becomes a readable
  contradiction in sequence.

### DW0841–DW0845 and DW0848 — the detail plan (`compiler::detail` + `dsl::prefab`; spec-0050, DSL v0.15)

Stage 6 of the map pipeline: a place is detailed inside the box the whole gave
it. The document is `detail-plan.json` (§2); this is what judges it.

**What invokes each check, and what happens without it.** `DW0841`–`DW0845` run
in `validate_loaded`, the one funnel every `delvec` subcommand's validation goes
through — `build` included — so a defect cannot reach a datapack by skipping
`delvec validate`. `DW0841` runs again in `delvec allocation`, before that verb
prints a single number, because obtaining an allocation and compiling a binding
are the two events that begin detail work; there is no third, because no other
verb reads a `detail-plan`. `DW0848` runs at `delve-admit audit` — the admission
event, where the library's integrity lives — and again wherever a `details[]` row
consumes the piece, so a pre-check-era piece cannot be consumed unjudged. The
frame itself is computed in `Plan::build`, which is the only constructor a world
can be reached through.

**No opt-out exists.** A place is bound or unbound, and the kind is determined by
whether a row exists rather than chosen among demands: there is no
acknowledgement field, no exemption list and no severity an author selects. The
two soft edges are each secured by a property the defect cannot supply. The walk
record's freshness key is the **two authored documents the whole is derived
from** — the site plan and the layout graph — and the defect `DW0841` catches,
detailing a whole the walk never passed, is exactly what moves one of them. The
blockout-drift advisory is reachable only by toolchain movement, because both
keyed documents have been compared and found equal by the time it fires: the
derivation is a pure function of the plan, the graph, the metrics table and the
engine, so what is left to have moved is the engine or the table. The advisory
suppresses itself on a graph mismatch for that reason and not as a
duplicate-diagnostic nicety — a campaign edit reaching it would make its own text
false.

| Code | Rule |
|---|---|
| `DW0841` | **Detail without a passed walk of this whole.** A campaign carrying a `detail-plan` is refused unless `walk-record.json` exists, parses, carries `verdict: "passed"`, and names both this plan's `site_plan_sha256` and this graph's `layout_graph_sha256`. Missing, unparseable, stale-in-the-plan, stale-in-the-graph and `"findings"` are each named separately — the plan and the graph are different edits with different repairs — and a stale record's refusal prints both sides of the hash that moved. Each hash is over its document's **canonical** bytes, so a reformat is not a re-walk. Both documents are in the key because the derived whole is a function of both: a seam is cut to air or filled with the bar by its edge's `class`, the side an `anchor/unlock-…` stands on is its `opens_from`, and a sky-open box's headroom is its node's `size_class` — so a graph-only edit can move the walked bytes, and can move the walked *connectivity* while moving no byte at all. Stated plainly: the machine half of this gate is freshness and an explicit verdict — that a human really walked is the record author's assertion, held by operating practice, and no engine check can prove otherwise. A `blockout_sha256` mismatch under an unchanged plan **and** an unchanged graph is instead a **warning** naming both hashes and both engine revisions. Validation tier (exit 1), `every_version`. **Binding: walk records read, freshness hashes compared out of the two keyed documents, and `details[]` rows stood in front of.** |
| `DW0842` | **The binding does not bind.** A `detail-plan` in a campaign with no site plan (the limiting case, naming the missing document); a `place` naming no layout-graph node; two rows for one place; a `piece` the prefab library does not hold; an `anchors` key that is not a name this place owes; an `anchors` value naming no anchor of the piece. Validation tier (exit 1), `every_version`. **Binding: rows resolved, against the plan's box count.** **Folded at a zero box count**: when the site plan resolves no box every row misses by construction, so one line states the count, names every row and defers to the primary that already said it — `DW0824` when the campaign carries no `layout-graph.json`, and the plan's own emptiness otherwise. With a map to be wrong about it refuses per row exactly as it always has. |
| `DW0843` | **The piece is not the shape of its allocation.** The piece's structure size differs from the handed frame on any axis — the refusal prints both extents, the axis and the direction. **Undersize refuses exactly as oversize does**: the box is the footprint, so a smaller building means a smaller box, which is a site-plan edit and a re-walk, taken visibly. Also under this code: a bound piece declaring no spatial contract, because the equivalence instrument would have nothing to read and a place detailed with such a piece would be a hole in the proof rather than a finding in it. Validation tier (exit 1), metadata only, `every_version`. **Binding: pieces measured.** **Deferral**: a `details[]` row is judged against a frame and a seam set the SITE PLAN computed, so where the plan has already refused one of them — the place's box off the kit grid (`DW0825`), or a seam the plan writes on this place and does not resolve (`DW0828`/`DW0829`) — the line still refuses on its own terms and says what it stands downstream of. The primary is in another document, where the reader cannot otherwise see the relation. |
| `DW0844` | **The piece's openings are not the plan's seams.** Both directions, from metadata, before any byte assembles: a seam this box must answer with no aligned face opening of a compatible class, and a face of the piece answering no seam — the *discovered* seam, at the earliest tier there is. Alignment means the face's opening cells answer the seam's allocated cells across the party plane, or **at** them for a seam lying in the piece's own floor course. Deliberately redundant with `DW0836`/`DW0838` and **not** their replacement: this reads declarations and names the piece and the seam at validation, they read bytes at build and remain the independent observers, and a piece that lies in its metadata passes here and reds there. Validation tier (exit 1), `every_version`. **Binding: seams required, and declared faces examined.** **Deferral**: a `details[]` row is judged against a frame and a seam set the SITE PLAN computed, so where the plan has already refused one of them — the place's box off the kit grid (`DW0825`), or a seam the plan writes on this place and does not resolve (`DW0828`/`DW0829`) — the line still refuses on its own terms and says what it stands downstream of. The primary is in another document, where the reader cannot otherwise see the relation. |
| `DW0845` | **An owed anchor has no standing.** An owed name left unbound; one bound to a piece anchor that declares no cell (a region answers a gate, and a gate region is never owed by a place); or one bound to an anchor the piece's own contract resolves into something a body cannot be at — a `no_body` region, a bar, a transit volume. Validation tier (exit 1), `every_version`. **Binding: owed names checked over every bound place.** |
| `DW0848` | **A piece's declared footprint class disagrees with its bytes.** Prefab metadata gains an optional `footprint_class` naming a metrics `size-class.*` rung (`DW0812` refuses a name the table does not define, as for any document naming a table entry). A piece declaring one is refused when its own structure size could serve no box of that class: horizontal extents off the class's range, off the kit grid's quantum, or a height under the class's clearance plus the one floor course a piece owns. The field stays optional for the library at large — every piece predates it — and a piece bound by a `details[]` row is held to exact frame equality by `DW0843` whether or not it declares. Validation tier (exit 1) in the compiler, admission-failing in `delve-admit`, `every_version`. **Binding: pieces declaring a class, against declaration documents read.** |

**The owed names** are the subset of the synthesized vocabulary whose bearer is a
given box: its own `anchor/node-…`, `spawn` when it is the entry node, and each
`anchor/unlock-…` whose opening side it is. A gate region (`anchor/seam-…`) is
never owed — it stands in a party plane the whole owns. `dsl::siteplan::owed_anchors`
answers, beside `synthesized_anchors`, and a test proves the two partition rather
than agree. The `anchors` map re-binds each owed name to an anchor of the piece,
so a kit piece keeps its own vocabulary and a campaign keeps its own: the quest
layer bound those names to places at stage 3, before any detail existed, and
detailing must never force a quest edit.

**`delvec allocation <place>` / `--all`** emits the handed allocation as JSON:
the frame's extents, the datum in piece-local coordinates, every seam of the box
in piece-local coordinates with its face, cells, class, rise and the answering
class the table above requires, the owed anchor names, and the detail plan's
palette. It is derived from the site plan on every invocation and is **an input
to nothing** — no gate, no build step and no check ever reads what it prints, so
a file made of it is a copy with no consumer and its staleness has no vector into
the build. Every obligation is recomputed from the plan at every validation.

### DW07xx — workspace tooling (spec-0007; **not `delvec`**)

Separate binaries with their own exit-code schemes; diagnostics to **stderr**.
Catalogued here so the DW namespace is complete and CI-checked. Two ranges are
`delvec`'s own and are numbered by DOMAIN rather than by binary: `DW0724` (the
visual/render range) and `DW077x` (`delvec fmt`, §9) — a code names a rule, and
the rule's domain is the more useful thing for the number to say.

| Code | Tool | Meaning |
|------|------|---------|
| `DW0700` | `delve-schem` | Strip hook: a forbidden block/entity was removed. |
| `DW0701` | `delve-schem` | Oversize schematic tiled into structure parts. |
| `DW0702` | `delve-schem` | Source `DataVersion` ≠ pinned MC 1.21.11. |
| `DW0710` | `delve-schem` | Input unreadable / not a Sponge schematic. |
| `DW0720` | `delve-render` | Missing-texture (magenta) placeholder detected (fidelity gate; exit 4). |
| `DW0721` | `delve-render` | Input (`.nbt`/metadata/`render-plan.json`) unreadable, or a `--view` that cannot be rendered as asked (exit 2). A declared view is refused **before any frame**: a malformed spec, a bearing given twice or not at all, a subject the piece does not declare (the message lists the anchors it does), or a name a planned shot already holds — which would overwrite that shot's image and quietly regress a review set. A view is never dropped or silently re-aimed: a set missing the one camera the reviewer asked for still looks complete in a directory listing. |
| `DW0722` | `delve-render` | Output file could not be written (exit 3). |
| `DW0723` | `delve-render` | GPU renderer failed / textures absent (exit 5). |
| `DW0724` | `delvec` (visual tier) | **A render-plan camera's eye cell is occupied** (solid/water) in the FINAL assembled world — the frame would render the inside of a block, and a picture of the inside of a block is indistinguishable from a picture of a featureless room. `compiler::nav::verify_camera_eyes`, over **every** shot the plan holds: `spawn`, `interior`, `seam`, `npc`, `interact`, `gate` and `pov`. It is bound at the derivation, not at a call site — `render_plan::render_plan` is the only constructor of a plan document and it takes the world, and every kind enters the shot list through one `push` that records the eye from the same position it writes into the camera, so a kind added later is covered without anyone remembering. (It was bound to `pov` alone, which is the kind that happened to need it first; the identical defect on a seam camera standing inside a hung ceiling lantern was invisible to every build in the repository.) Two verdicts, decided by the object rather than by the author. **`pov`** is the player's own eye, 1.62 above a DW0314-proven-standable waypoint, so it is clear by construction and is never moved: a violation there is the derivation changing (or a later pass mutating the cell) and fails the build (exit 3) — fix the derivation, never the waypoint or the geometry. **Every other kind** states a fixed stand-off from a subject it frames, which is a preference and not a position: a camera whose own cell holds a block stands instead at the furthest clear point on its own sight line (`compiler::camera::stand_in_open_air`) and records `camera.requested_pos` + `camera.standoff` on its shot, because a displaced camera is invisible in its own frame. It yields to that one fact and nothing else — an interior shot's dollhouse eye is deliberately above the piece and is not pulled through the roof it looks past. The error survives for those kinds too: it fires when even the subject's own cell is buried, so there is no vantage on the sight line at all. Every plan states the proof's binding counts (`camera_eye_proof`: `cameras` examined, `pulled_in`), and a plan holding zero cameras is a warning under the same code rather than a silent pass. Numbered in the `DW072x` visual/render range. Scale, measured over every campaign and fixture that builds before this binding existed: **204 of 752 cameras stood inside a block** — 144 seam, 38 gate, 16 NPC, 6 interact, 0 POV — and every one of the 27 campaigns had at least one. |
| `DW0725` | `delve-render` | **Contact-sheet ordering is not a total order over the candidates** — indices dropped, duplicated or out of range (exit 10). The score RANKS the sheet and NEVER gates it (spec-0028 §3): cross-domain calibration between a painterly reference image and a voxel render is unproven, so a similarity number may decide where a candidate sits on the page and never whether it is on the page. `sheet::build_sheet` puts whatever its ordering function returns through `sheet::verify_total_order` before drawing a pixel, so every way rank-only can erode — a threshold shortening the order, a "best of" repeating an index, an off-by-one losing the last cell — lands here as one refusal instead of a silently shorter page. Promoting the score to a threshold requires its own owner-approved amendment backed by accumulated batch data; do not add one to satisfy this diagnostic. |
| `DW0726` | `delve-render` | A contact sheet's score set bound to fewer candidates than the sheet holds. **Zero binding is an error** (exit 2) — nothing was ranked, and a score file that matched no candidate must not read as a successful ranking run (CLAUDE.md: a green gate that binds to nothing is vacuous, not a pass). A partial binding is a **warning** naming the counts; the unscored candidates stay on the page, last, labelled unscored — a missing measurement is not a bad one. Score rows matching no candidate warn under the same code (usually an id typo or a stale run). |
| `DW0727` | `delve-render` | **An anchor's eye-level camera is not standing on the anchor's own cell**, or could not be stood up at all (warning; `piece`/`batch` still write every other shot). The per-prefab eye shots are the only cameras inside a piece, and a prefab is mostly solid — the motivating ward was 81% rock with an anchor inside a bank of iron bars — so an eye point taken from an anchor position alone lands inside a block often enough that assuming it would put a picture of the inside of a block in a review set, indistinguishable from a picture of a room. Three tiers, one code, because they are one fact the reviewer needs (*where is the body in this frame*): the camera **stepped back** along the facing to a cell where a body fits, naming the block that displaced it and the offset; **no body cell** was found within 3 blocks with the anchor still in front of it, so that anchor gets no eye shot at all; or the frame rendered **empty** — nothing but flat background, meaning the camera is aimed at nothing (measured on the pixels, `detect::is_featureless`, not inferred from geometry). The empty-frame tier is a property of a rendered frame, not of one camera kind, so it covers every shot in the set and says which one it is talking to: an anchor aimed at nothing or out of the piece, an author-declared `--view` whose zoom or cutaway left the model out of frame (the message repeats the spec, bearing and zoom that produced it), or a fitted planned shot with nothing to fit. Every case also rides `<stem>-shots.json`, since a displaced camera is invisible in its own frame. Fix the anchor's facing or the piece's geometry; never move the camera to make the picture nicer. Zero eye shots over one or more eye-eligible anchors is reported under the same code — a review set with no interior view cannot judge the scene, which is the whole job of `prefab-procedure.md` §5. |
| `DW0730` | `delve-admit` | Audit: a palette block is not in the allowlist. The allowlist is a list of names **at the pin**, so the id it judges is the one a 1.21.11 server will hold after datafixing — not the one the bytes spell. For a template at the pin those are the same id; for a pre-pin template whose palette names a block the game has since renamed, the entry is resolved through `BlockRegistry::loaded_id_at` first and the diagnostic names both ids (`minecraft:chain`, written at DataVersion 2975, is judged as `minecraft:iron_chain`). An id the pin does not have and the rename table cannot reach is judged as written, which refuses it: resolution never widens the list. |
| `DW0731` | `delve-admit` | Audit: a hard-forbidden code-injection vector (command/structure block, NBT spawner, embedded `Command`). |
| `DW0732` | `delve-admit` | Input error (unreadable `.nbt`/metadata/JSON). |
| `DW0733` | `delve-admit` | Audit: a palette block state does not exist in Minecraft 1.21.11, **in a template whose `DataVersion` is the pin (4671) or later** — no datafix runs on such a file, so the game loads the block as air (error). The game datafixes every structure it loads against the file's own `DataVersion`, so invalidity-at-pin alone is not the rule; the pre-pin case is `DW0734`. Rule lives in `delvewright_schem::blocks::BlockRegistry::judge_at`. |
| `DW0734` | `delve-admit` | Audit (warning): a **pre-pin** template carries a block state the pin does not know. Load-time datafixing is expected to migrate it (`hero-temple-ruin-arch.nbt` at DataVersion 2975 carries `minecraft:chain`, which the pinned game holds as `minecraft:iron_chain`; refusing this case is a false positive) — but an id no fixer maps (a typo) still loads as air at any DataVersion, so the audit says so out loud and passes. Where the vendored rename table (`crates/dsl/data/block-renames-1.21.11.json`) reaches the id, the warning **names the id the server will hold**, because a reader told only which block is missing still has to guess which of ten chain blocks replaces it. Defined in `delvewright_schem::blocks` with the rest of the family. |
| `DW0735` | `delve-admit` / `delve-grammar` | A block state omits a **shape-carrying** property: one named by a `multipart` selector in the block's own blockstate definition (`crates/dsl/data/blockstate-shape-props-1.21.11.json`, derived from the client jar by `tools/extract-shape-properties.py`). A `variants` property the state omits picks the complete default model — benign, the default is what the author meant (`waterlogged`, `snowy`, `powered`, a chain's `axis`) — but a `multipart` property *assembles* the model, so the omitted default drops geometry: a `cobblestone_wall`/`iron_bars`/`oak_fence` with none written places as an isolated post, silently. Error at admission (`delve-admit audit`), a red `shape-complete` gate plus an export refusal in the grammar back end. The class is the block's, derived from game data — never a hand-kept id list. |
| `DW0736` | `delve-grammar` | An **orientation-sensitive block state filled into a scope whose frame turns or reflects it, with no `orientation` guard**. A grammar frame permutes and reflects the *geometry* a rule describes and never rewrites block-state properties (`crates/grammar/src/orient.rs`), so a **world-frame** literal `facing`/`axis`/connection/`rotation` state lands however the scope was framed — and a pure reflection is the case that reads as safe to a check that looks only at the axis permutation, because a reflection HAS the identity permutation. Two mechanisms answer it: write the state in the scope's own axis frame (`Paint::Local`, resolved at fill time — one binding, every frame) or pin the frame with `Cond::Orientation` and write one alternative per frame, reflections included. This fires when a fill uses neither. Checked during expansion (where scope frames exist), sensitivity derived from the registry's own value vocabulary (`BlockRegistry::oriented_mismatch`; `rotation`, `hinge` and a handed `shape` are the documented residue and count as mismatched whenever the frame moves **or reflects** a horizontal axis). Surfaces as a red `oriented-fills` gate — whose detail states fills examined, fills carrying properties, and how many of those were resolved out of the local frame — plus an export refusal. A green from this rule is a green over the frame the scope actually had; a fill whose scope stood in the identity frame was not judged by it at all, which is `DW0742`. A passed guard licenses a fill only while the frame it asserted still holds: a reorientation below the guard voids it. |
| `DW0737` | `delve-grammar` | A placed block state **omits a property the block has**, so its geometry is whatever a 1.21.11 server derives from the block's default state and no reader upstream of the server can know which. Vanilla's `BlockState` codec fills an omitted property in, so a partial state is legal and the game resolves it correctly — but the review render, the navigation walk, the diff a reviewer reads and the machine gates each have to guess, and the guesses disagree. `DW0735` is the strict subset that also drops geometry outright; this is the whole class, and it fires on the benign-looking half too: an `oak_stairs[facing=east]` with no `half` and no `shape` is not "the author meant the default", it is a stair whose geometry no document states, and vanilla recomputes `shape` from its neighbours on every block update. Rule lives in `delvewright_schem::blocks::BlockRegistry::omitted_properties` with the rest of the family. Surfaces as a red `states-complete` gate, with the placed-state count as its binding. Not an export refusal: unlike `DW0735` the omission costs no geometry in the emitted template, so it is a gate on what was AUTHORED. |
| `DW0738` | `delve-grammar` | A block state written in the **scope's own axis frame** (`Paint::Local`) carries a property the pinned vocabulary cannot map onto the world frame the scope was given. A frame is a signed permutation — which world axis each local axis names, and whether it runs backwards along it — so a **direction** (by key, as a connection flag, or by value, as a `facing`), an **`axis`** and a `<dir>_<dir>` pair all have exact images under every frame, and a reflection is simply the sign. What has no image is the frame-relative residue. A 16-step `rotation` and a handedness (`hinge`, a stair's corner `shape`, a double chest's `type`) are stated against a fixed vertical AND a fixed handedness, so they are determined only under a **pure turn about the vertical** — the identity, or the horizontal transposition `x↔z`, which is itself a reflection of the horizontal plane and sends a yaw `r` to `(12 − r) mod 16` and left to right. Under any frame that reflects an axis, or moves the vertical, they are refused; so is a `top`/`bottom`/`upper`/`lower` half under a frame that moves **or reverses** the vertical, a horizontal connection turned onto a block with no `up` key, and a rail's direction-composed `shape`. A value that names no handedness (`straight`, `single`) and a `double` slab are their own image under every frame and are never refused. Refused at expansion, naming the state, the property and the frame (with a `-` on a reflected axis); the fill never writes a substitute. Rule lives in `delvewright_schem::blocks::BlockRegistry::permuted_properties` — the same classifier `DW0736` judges with, so a state one of them calls wrong is never one the other quietly rewrites. Fix: keep the scope's vertical on the world's and unreflected, or write the state in the world frame under an `orientation` guard. |
| `DW0739` | `delve-admit` | **A whole-piece command was handed ONE TILE of a tiled zone** (exit 2). A zone past the 48-per-axis cap ships as several `.nbt` plus one manifest; every command that takes a single `.nbt` will otherwise answer confidently about a fragment — `audit` returns `pass` over a fifth of a zone, `lighting` measures a fifth of a building, `socket` edits one tile of a set, `gallery` puts slices of one building on separate plinths. A tile is recognised by the name `<base>.x<i>y<j>z<k>.nbt` that `split::part_filename` gave it, which is carried by the bytes through any copy or move; the manifest beside it only enriches the message. Pass the manifest instead. **Which commands owe this refusal is enumerated from the parser, not listed**: `crates/admit/tests/fragment_doors.rs` walks `delvewright_admit::cli::Cli`, requires every command it finds to be classified exactly once — piece door, or exempt with its reason — and separately requires that **no** command exits 0 when handed a tile, so an exemption cannot hide a door. A command added and classified nowhere is a red. |
| `DW0740` | `delve-admit` | Catalog card schema/field validation failure. |
| `DW0741` | `delve-admit` | Catalog card license not in the ADR-0013 allowlist. |
| `DW0742` | `delve-grammar` | **A world-frame oriented block state that THIS REGION CANNOT DECIDE** — the third `oriented-fills` verdict, neither a pass nor a fail. `DW0736` returns "sound" for the identity frame before it reads a property, so a fill standing in the identity frame was skipped rather than judged. Whether that matters is not a fact about the frame: a scope no rule reorients has the identity at every region there will ever be, while a scope under `z: largest` has it only while this box's longest axis is already the one the request names — and at a region whose axes rank differently the same scope is turned and the same literal is refused by `DW0736`. So the expander carries, beside each scope's frame, the SET of frames that scope could stand in over every region the program could be expanded at (`orient::FrameSet` — the reorientation request evaluated over every weak ordering of the box's extents), and this fires when that set holds a frame that would land one of the state's properties wrong. Reported as `GateState::Undecided` with its own binding count (`undecided`, printed beside `bound`), a named finding, and a report verdict of `undecided` when nothing went red. **Refuses no artifact and reds no corpus sweep**: the program may be entirely correct, and no edit to it would make this region decide the question — a gate that reddened here would red on ordinary correct programs and be routed around within a week. Judging against the reachable set rather than all 48 frames of the cube is what keeps it that way: a request pinning the vertical with `y: world_y` leaves every `axis=y` pillar and every `facing=up` barrel decided, and judging against all 48 instead put six of the live campaign's eight zones into a state their authors could do nothing about. Fix with either mechanism `DW0736` names — `Paint::Local`, or a `Cond::Orientation` guard. |
| `DW0750` | `delve-admit` | Admission tooling (socket/anchor/lighting) failure. |
| `DW0751` | `delve-admit` | Lighting probe: a `dark` interior was measured (advisory; no longer gates — spec-0010). The message names the binding, the darkest cell and **the sky the measurement was taken at** — a clear night, the darkest state `light::effective_sky` models — so the finding is a place to put a light rather than a mood. Where the piece is one the sky reaches, it also gives the daylight minimum and says the interior needs a light only where the delve reaches night: a floor's light is not one number, and the middle of a pavilion is bright at noon and black at midnight. |
| `DW0752` | `delve-admit` | **The lighting probe bound to ZERO cells** (exit 1): nothing was measured, so there is no profile to declare and `--write` writes nothing. The probe's binding is the floor a body can walk to from a ground-level entrance, and each filter can empty it for a different reason — no standable cell at all, no entrance on any vertical face (a piece whose way in is a jigsaw socket must be socketed before it is probed), or nothing reachable. The diagnostic says which. Zero binding is a **finding, not a pass** (CLAUDE.md): a sealed pitch-dark crypt is precisely the piece that would otherwise escape `DW0751` by having nothing to grade. There is no *nothing roofed* case: roofedness was a proxy for indoors that only a probe without a sky term needs, and it made every open-air piece — a courtyard, a jetty, a meadow — ungradeable. |
| `DW0753` | `delve-admit` | **`lighting --write` has no prefab metadata to write the measurement into** (exit 2), and will not invent one. The skeleton it used to manufacture claimed `source: unknown`, `spdx: UNKNOWN`, no `generated_by` row and `anchors: {}` — a document asserting that nothing is known about an asset, written silently and afterwards indistinguishable from a real admission record; on a tiled zone it landed beside a manifest whose provenance row was sitting right there. A tool that cannot establish where something came from refuses. The measurement is still printed; create the metadata first, then re-run with `--write`. |
| `DW0760` | `delve-admit` | Gallery emission / curation failure. |
| `DW0770` | `delvec fmt` | Authored JSON is not valid JSON, located at `line:col` (exit 1). Reported instead of formatted — `fmt` never guesses at a repair. |
| `DW0771` | `delvec fmt` | **A duplicate object key.** JSON's grammar allows one and `serde_json` silently keeps the LAST, so one of the two values is already being discarded without a word; formatting would make that loss permanent and invisible, so `fmt` refuses and writes nothing (exit 1). Delete or rename whichever occurrence is wrong. |
| `DW0772` | `delvec fmt` | Internal error: the formatter's own output is not equivalent to its input, so nothing was written (exit 1). The self-check runs on **every** file `fmt` writes — it re-parses the rendered text and compares arrays index-wise, objects as maps. Its whole purpose is that an array reordering (which changes the game) fails here instead of shipping. A `DW0772` is a compiler bug; report it. |
| `DW0773` | `delvec fmt --check` | A file is not in canonical form, with the line of the first difference (exit 1). Fix by running `delvec fmt <path>` — never by hand. |
| `DW0774` | `delvec fmt` | The given paths matched **zero** JSON files (exit 1). A formatter or a `--check` that binds to nothing is vacuous, not a pass (CLAUDE.md), and a stale path in a CI step is exactly how this gate would rot into a green no-op. |
| `DW0780` | `compiler::faces` | **Two placed pieces whose declared exterior faces do not mate.** One piece declares a way in or out on the face it shares with its neighbour, and the neighbour declares nothing there, declares an opening elsewhere along the same wall, or declares a different class (a sightline where the first declares a walk). Build-tier (exit 3), `compiler::plan`, checked after placement. Both pieces pass every prefab gate individually — the pair is what is wrong, which is why no single-piece check can see it. The message names both areas, both prefabs, both faces with their world extents, and what the neighbour offers instead. Compared over the **declared** face contract (`spatial_contract.faces` in prefab metadata), never re-derived from the assembled voxels. A face that opens onto no placed piece is not a finding: a box garden has an outside. |
| `DW0782` | `delve-admit audit` | **A piece's declared spatial contract disagrees with its own blocks** (spec-0036 §2) — the second of the contract checker's two doors; the first is `delve-grammar expand`, which writes no `.nbt` when a gate is red. Each failed obligation is reported as one error line naming the gate, **its binding count** and what it found; each opt-out instance (an open envelope, a sightline, an out-of-walk region with its computed kind, a declared way with its sign and cell count, a way or bar the walk had to open, an exterior face) is reported as one advisory line, always, because a list is what a reviewer reads and a count is what a blind script satisfies. One enumeration, both doors: this is the same list `delve-grammar expand` prints, because it is the same checker. Raised for a piece that declares a contract, whichever way it is packaged: a single structure template, and a **tile-set manifest**, whose contract and anchors are zone-relative and are therefore judged against the assembled zone rather than against any tile — including a **contingent edge whose cells straddle a tiling seam**, which is judged over the reassembled zone and over no tile alone. A piece that declares no contract is not judged here and says so as `DW0783`. Exit 1. |
| `DW0783` | `delve-admit audit` | **The second door did not judge this piece**, with the count of what it therefore did not examine. One rule at two severities. A **warning** where the door was entitled to stay shut: the declaration document carries no `spatial_contract`, or there is no document beside the bytes yet (an ingested piece is audited before its metadata exists). An **error** (exit 1) where the door COULD NOT judge: the document does not parse — a piece whose contract nobody can read is not a piece without one — or it declares no contract while its own anchors carry the `resolves_to` an exporter writes only out of a contract, which means the declaration was dropped from the document rather than never made. That last is what keeps the "declares none" case from being an opt-out the defect can supply: a step that loses a top-level key on write leaves behind the anchors it modelled, and the corroboration states its own binding count, so a document with no anchors is visibly one that nothing here could have contradicted. Split from `DW0782` because "the contract and the blocks disagree" is a fact about a piece that HAS a contract, and a piece nothing was asked about is a different rule — sharing one code leaves the silence reading as the pass. Every outcome, including the two warnings, is also recorded in the machine-readable report as `contract.state` plus the door's binding counts (files, cells, declared spaces/regions/edges/anchors, obligations run, objects examined, anchors carrying a resolved element), so a report whose door never opened is not the same artifact as a report whose door opened and found nothing wrong. |
| `DW0781` | `compiler::faces` | Advisory: the piece-mating check examined **zero** abutting faces, so `DW0780` proved nothing about this world. Raised when no declared face of any placed piece touches another placed piece. The verdict states two counts, because they are two findings with two repairs: pieces carrying a `spatial_contract` at all (none means the library predates it and owes an adoption round), and how many of those declare a **face** (a contract whose every edge joins two of the piece's own spaces has made its claim and offers a neighbour no side to disagree with). Advisory rather than a red because old documents keep compiling (version-adoption discipline); the adoption round that gives the library contracts is what turns this binding into a number. **The message is one line — the counts and the object class — and the reading is here.** A site-plan world allocates its ways at stage 4, on faces two boxes already share, and proves them over the built bytes by `DW0836`, so a zero there is the honest count of a question that world does not ask rather than a proof that went missing. In a prefab world a zero means nothing here proves the pieces fit together: a piece without a contract makes no claim about its own sides at all, and a contract declaring only interior edges has made its claim and offers a neighbour nothing to disagree with. Subject: the campaign, so it prints among the campaign advisories. |
| `DW0790` | `delve-render` | **A blockstate has no definition in the pinned asset source** (`viewer` / `palette`): the id does not exist at this version, or its model or one of its textures is absent. A **warning**, with the cell count and the offending resource — the reviewer still needs the rest of the building, and the block is listed on the page rather than silently drawn as if it had resolved. It says what the page cannot draw and nothing about what a server would load: whether an id the client jar lacks is also a defect in the world is decided by the template's own `DataVersion`, which is `DW0734`'s question and not this one. `minecraft:chain` is the live instance — the pinned game holds `minecraft:iron_chain` instead, the prefab naming the old id declares `DataVersion` 2975, and load-time datafixing places `iron_chain` correctly, so the finding here is that the review page draws a placeholder where the game draws a chain. Also carries the block-entity texture case when the asset source is an ordinary resource pack rather than the pinned game (see `DW0792`). |
| `DW0791` | `delve-render` | **A palette entry leaves properties unwritten that its blockstate definition selects a model with** (warning), naming the properties, the values the pinned version's default state supplies, and the cell count. The state is legal — vanilla completes it from the default state on load — but the file then means something only a running server can work out, and what the reviewer is looking at is not what the file says. Worst on a `multipart` definition, where an unwritten property matches no case at all: a `minecraft:cobblestone_wall` with nothing written drew a **solid cube** where a wall post stands, and every tool in the repo reported it resolved. Measured over the library at the pinned content SHA: **15 palette entries across 7 of the 36 prefabs**, which is 7 distinct blockstates — a barrel's `open`, a grass block's and a podzol's `snowy`, a button's `powered`, a fence gate's `in_wall`. No connection class is among them: an omitted connection is `DW0735`, and the generators write those states from the piece's own neighbours, so the library carries none. Counting *every* unwritten property instead of only the selecting ones gives 84 entries across 20 prefabs; the difference is `waterlogged` and the non-selecting residue beside it (a trapdoor's `powered`, `signal_fire`, `cracked`, `facing`, `distance`), real and invisible. This is what stops the review page reporting a clean resolution over a building whose walls are the wrong shape. The predicate is the block's own definition — a property the variant keys or the `multipart` `when` clauses test — so it is a superset of `DW0735`'s shape-carrying class and a subset of "every unwritten property": `distance` on a leaf block changes no picture and is not reported. Fix by writing the property at the value the message names; completion never overrides a written value, so the BlockState is unchanged. |
| `DW0792` | `delve-render` | **The review page's resources do not hold together** (exit 10, no page written) — a finding about this toolchain rather than about the prefab, and one that is silent by construction in both of its forms. Either the vendored renderer has lost its local texture-id patch (deepslate asks for `entity/banner/banner_base` and `entity/shield/shield_base_nopattern`, paths no Minecraft version ships; 1.21.11 has both at the jar's top level, and unpatched every banner and shield renders as the missing-texture checker), or a **block-entity texture id** the emitter asks for is absent from an asset source that declares itself to be the pinned game. Those ids are named by the renderer's code and by no model file, so the emitter must keep a second copy of the table and a wrong entry in it is invisible — a texture that does not exist and a texture nobody asked for look identical in a finished picture. What an absence MEANS is decided by the source rather than chosen: a jar carrying `version.json` with the pinned id is complete by definition, so a texture it lacks is the table and the version disagreeing; any other source is a resource pack, entitled to be partial, and gets `DW0790`. Rebuild the bundle with `tools/build-deepslate-bundle.sh`, or fix the table in `crates/render/src/viewer/resources.rs`. |
| `DW0800` | `delve-grammar` / `delve-admit` | **A body of fluid in a piece does not stay where it was written.** Water and lava are the only blocks an author places that move: they run down first and then sideways, on the server's own clock, before any player arrives — and nothing upstream of a server can see it, because the `.nbt`, the review render and the contact sheet the owner approves all draw still water. One rule with two ways to break it, under one code because they are one fact the reviewer needs (*this pond is not a pond*): **saturation** — every fluid cell must be a source, since a `level` other than 0 is a state vanilla derives from a cell's neighbours and re-derives on its own clock, and a piece cannot pin one (spec-0038's ruling; ADR-0006, since a world that heals no longer matches the bytes that built it); and **containment** — no source may have an open cell beside or below it. Open means AIR, and only air: a block written `waterlogged=false` is a wall, and a block written `waterlogged=true` is a still cell that spreads nothing. Both of those are measured on the pinned server rather than reasoned out (`tools/spike-block-settling/`, nine rigs including a grate and a stair in a wall, a stair with a source on each side, and a waterlogged block given a block update) — the plausible opposite, *a grate is a hole because iron bars are waterloggable*, is a claim about placing water in that cell and not about a body beside it. Fluid never runs upward, so an open top is not a leak. A run direction that leaves the piece's own outer face is **counted and never judged**: what is beyond a face is not in these bytes (a shoreline piece's water is the sea), and only the placement knows what is on the other side. **`DW0318` is where that is decided** — it takes the assembled world and refuses any fluid cell outside every placed piece under a void horizon, so "whatever this piece is placed against decides where that water goes" names something that does. Binding: fluid cells examined; a piece with none gets no gate at all and the count stands as a measurement. Emitted as a red `fluid-contained` gate (no `.nbt` is written), as an `audit` error, and — at the third emitter of structure bytes, which had nothing asking it — as `prefabs/invariants.rs::assert_fluid_is_contained`, run by every `prefabs/*-generator` before it writes, printing its binding per piece. |
| `DW0801` | `delve-grammar` / `delve-admit` | **A stair claims a `shape` the game does not derive at its cell.** A stair's `shape` is not a stored fact: vanilla recomputes it from the stair's own neighbours on every horizontal block update at that cell, so an authored value is a *claim about the four cells around it* — and a wrong claim is corrected by the world the first time anything is placed, broken or flooded beside it. This is the one property that can be right in every tool this project owns and wrong in the game: the render draws what the bytes say, the reviewer approves the picture, the world draws something else. The live instance is a mitred kerb pointed across its run instead of along it, which survives every render and flattens to `straight` in play. The derivation — straight unless a stair of the same `half` sits across the facing axis in front (outer corner) or behind (inner corner), suppressed when the cell beyond the turn already carries a stair of this facing and half, and *any* stair block counts, not the same one — is **measured, not read**: a field of 758 random stairs was placed, settled and read back on the pinned 1.21.11 server (`tools/spike-block-settling/`) and `crates/schem/tests/stair_shape_measured.rs` replays every cell of it through `delvewright_schem::stairs::derive_shape`. A stair that writes no `shape` at all makes no claim here, so nothing can disagree with it. Binding: stairs examined; a piece with none gets no gate and the count stands as a measurement. Emitted as a red `stair-shape` gate (no `.nbt` is written) and as an `audit` error. |
| `DW0803` | `compiler::emit` | **A placed structure template is not the size its prefab metadata says it is.** Two documents claim the same fact — the `.nbt`'s own `size` tag and the metadata's `structure.size` (or a tile's `structure_set.parts[].size`) — and *every pass but the placement itself reads the metadata's*: the forceload span, the piece AABB `DW0780` compares, massing's footprint, and the offset arithmetic that puts a tile in the world. When they disagree the world is built around a shape that is not the one whose blocks arrive, and no other check can see it because each half is internally consistent. Tiling is what makes it reachable: a manifest and its tiles are several files a `cp`, a partial re-export or a hand edit can leave at different ages, and a stale tile then lands at the offset the manifest gives it, sliding part of a building through the rest. A single-template prefab has the same exposure and had the same silence. Build tier (exit 3): the world would be wrong, so it is not built. Prescription is never “adjust the declared size” — the two sizes are one fact and the fix is to make them one export again. **Binding**: `TemplateExtentBinding { placed, checked }`, templates placed vs templates whose bytes decoded; a world where none decoded reports the zero binding as a `DW0803` warning rather than a clean pass over an empty comparison. **Bound at two entry points, deliberately**: `emit::build_with_warnings`, which protects the datapack, and `main::read_structures`, the one place every CLI consumer of prefab bytes passes through — `build`, `snapshot`, `viewer`, `blocking-chart`. The second is the one that matters: a review artifact drawn from a stale tile is a picture that lies, and it is what a reviewer checks the world against. The check is pure, so the build path runs it twice for the cost of a walk. |
| `DW0804` | `compiler::plan` | **Two anchors in one area declare the entry role.** An area has one place the party arrives at, and two claims to it is a question the compiler cannot answer: picking first-wins would settle it by piece order, or by the `BTreeMap` order of two anchor names nobody chose for their sort, and a spawn that moved for that reason is a mystery nothing in the build output mentions. Build tier (exit 3), raised while the anchors are being resolved, so it precedes every check that would have been computed from the wrong start. The message names the area and **both** claimants. Only reachable through a declared `role`: two anchors merely *named* `spawn` and `entry` in one area are the pre-role compatibility case and stay ordered by `plan::ENTRY_ANCHOR_NAMES`, because that ordering is what every shipped piece was admitted under. Prescription is to drop the `role` key from the anchor that is not the arrival cell — the anchor itself stays and content still binds it by name — and, in a `prefab_pool`, to leave the role on the piece that seeds the layout. Never resolved by renaming an anchor to `spawn` or `entry`: the name list is the fallback for pieces that predate the role, and it is not consulted at all once an area declares one. |
| `DW0807` | `compiler::emit` | **A generated PackTest template drives the outcome it asserts on, and does not write the `#party` state the gates on that outcome's path read.** The suite runs as ONE batch on ONE shared server in an order PackTest RANDOMISES (`compiler::batchstate`), and `#party` is batch-global progression state (spec-0018), so the contract is *own dummy, own scores, own init*. A term a template only READS is decided by whichever sibling ran last — and one template is not atomic: the campaign-playthrough template drives the real campaign, `schedule`s its next phase twenty ticks out and `await`s, so for those ticks the party ledger holds whatever the campaign's own completion functions wrote, and the last phase's leavings persist to the end of the run. Two worked instances. A gate IN `tick`: the gallery's `trigger/skip-the-label` forbids `flag/hall-sealed`, `complete_q_far_hall` SETS that flag and nothing clears it, so `v04_strike_npc` passed or failed on nothing but whether it ran before or after — the SAME BYTES producing both verdicts, which is why byte-identity against the baseline could not exonerate the emission. A gate the campaign template reaches through its OWN drive: `check_q_<q>`'s `unless score #party dw.q_<q> matches 1`, one dispatch below the `complete_o_*` it calls, deciding a score written two dispatches below in `campaign_complete` — a sibling that runs the real `tick` completes the terminal quest, the drive becomes a silent no-op, and the assert reads 0 on tick 0 (measured on `souls-bonfire` and on `lift`). Such a template does not fail; it lands green most of the time, which is the shape that gets re-run instead of read. Judged from the shipped bytes and feature-blind, so it guards templates not yet written. Build tier (exit 3). **Scope**: `#party` only (other global sentinels are reached through the machinery that owns them, so demanding a literal write would red correct templates); templates that DRIVE their own outcome, with their hoisted helpers (`pt_camp_drive`, `pt_camp_run_<i>`) inlined first; and only gates on the path from the drive to the outcome, judged against the assertions taken AFTER the drive (without that window, `verb_kill`'s wave count — asserted immediately after it summons the wave itself — would charge it with every gate that can touch a wave counter). **Binding**: `BatchStateBinding { templates, driving, judged }`; a suite whose templates drive but whose outcomes pass through no `#party` gate reports the zero binding as a `DW0807` warning rather than a clean pass over an empty comparison. Prescription: a one-gate template drives the whole gate through `packtest_gate_drive`, which covers `requires_flags`, `forbids_flags` and `requires_state` together; the campaign-playthrough template opens with `campaign_progression_baseline`, the entire party ledger set to the campaign's start state and re-written by its own drive inside the same atomic mcfunction. Never hand-roll the terms beside the template — three sites did, each dropping a different axis, which is how this arrived — and never silence it by relaxing the assertion or retrying the test. |
| `DW0810` | `compiler::emit` | **The generated PackTest suite drives one declared object's own emitted body but not a sibling's.** A mechanic whose runtime body is emitted PER OBJECT — a timed gate's `tgate_open_<id>`/`tgate_close_<id>`, an actor's `unleash_<id>`, a wave's `wave_census_<id>` — has one body per declared object, over its own region, with its own blocks and its own judgement, so a template that drives one of them proves nothing whatever about the next. The worked instance: the timed-gate emitter bound `plan.timed_gates.first()` and wrote one template per campaign, so a level declaring three gates with the lethal `crush` flag on the THIRD shipped its only player-killing mechanic with a compile-time proof and no runtime proof at all — the suite green throughout, while that level's own generation record showed six critical-path bot attempts, every one exit 1, the lethal gate never reached in any run. This is the unbound-gate vacuity mode wearing a subtler surface: the gate was not unbound, it bound ONE object and reported honestly about that one while the set it covered had N members — the same shape `CLAUDE.md` names one layer down as a hand-rolled walk enumerating 3 of 5 effect roots. **Warning tier**, and the reason is a limit of the reading rather than a schedule: nothing in a finished tree separates *the emitter meant to prove every member and skipped some* from *the suite drives one exemplar by design*, so a refusal drawn here would need a per-family allowlist and an allowlist is an opt-out the defect itself can supply. The refusal a per-object mechanic owes is `DW0811`, which judges the emitter's own registered claim rather than the bytes alone; read the two together. Judged over the shipped bytes and **naming no mechanic anywhere** — a table of watchable classes would be the very defect it checks. Declared ids come from a generic walk over the authored stage documents (every `id` at any depth), a function is object `i`'s body in family `prefix` when it equals `prefix + i` with `prefix` ending in `_` (longest id wins, so `tgate_open_side_door` is `side_door`'s and not `door`'s), and a body is **watched** only when a template invokes it directly — a mention in a comment is not a proof. **Scope**: at least one sibling must already be watched, because the rule is *the suite claims to watch this mechanic, so it must watch all of it*; a family nothing drives is a far broader question and is counted rather than diagnosed. Sub-bodies are not siblings (`lethal_east_pit_kill` does not end in a declared id), which is what drops eight of the sixteen families a naive prefix match reports. **Binding**: `WatchBinding { declared_ids, campaign_functions, invoked, families, multi_object_families, unwatched_families, unwatched_family_objects, unwatched_family_members, watched_objects, unwatched_objects }`, written to `validation/watch-ledger.json` so the counts travel with the build instead of living only in a stderr string. The families this rule passes over are reported there **by name, with their members and their object count** — a bare total states the limit's size and nothing an operator can act on, and `unwatched_family_objects` is the population `examined` excludes, stated so the examined figure is never read as the whole per-object surface. Prescription: drive every member of the family — loop the emitter over the declared collection and give each template a per-object path and per-object scratch scores (the suite is one batch on one server, so a shared score is written by one template and asserted by another). There is deliberately **no way for a campaign to declare a body unwatched**: the proof such an opt-out would demand — *this object has no template* — is precisely the condition that makes it a finding. |
| `DW0811` | `compiler::emit` | **A suite emitter claims per-object runtime proof over a declared list, and the shipped suite drives only some of the bodies it wrote for that list.** The refusal half of `DW0810`, drawn one step in from it because nothing in a finished tree separates *the emitter meant to prove every member and skipped some* from *the suite drives one exemplar by design* — both are a family with a watched member and an unwatched one. A refusal read off the bytes alone would therefore need a per-family allowlist, which is an opt-out the defect itself can supply. The distinction lives in the **emitter**, so a suite emitter that walks a declared collection to write per-object templates registers a `watch::Claim` over that collection, and the claim is judged against the shipped bytes. Neither half can be faked by the defect: `declared` is taken from the plan's own authored list, so a walk that stops at `first()` still declares every member, and the driven set is read off the emitted suite rather than from the emitter's own bookkeeping. **The rule over a claim is total but is only over bodies that EXIST**: `tgate_disarm_<id>` is emitted only for gates declaring a disarm, so an optional affordance is never read as a breach — what is judged is *written for a declared object, therefore driven*. **Build tier (exit 3)**, unlike `DW0810`: the emitter's own claim is the proof obligation, and a strict subset does not discharge it. **Stated limit**, because a silent one is worse than a narrow one: an emitter that registers no claim is outside this refusal — `DW0810` still names every sibling it leaves unwatched, and the gallery's warning ledger is a set-equality that reds on the new row. **Binding**: `ClaimBinding { claims, declared_objects, bodies_judged, bodies_watched }`, written to its own `validation/watch-claims.json` with `bodies_judged` repeated as a top-level `examined` — the spelling `tools/check-gallery-coverage.py` already reds on. Nested inside the `DW0810` ledger the count would have been written, committed and diffed but never judged; hung off an invocation that already exists, a claim that quietly stops binding on the gallery is a red rather than a number nobody reads. A campaign declaring none of the mechanic honestly reports zero, which is why the rule is the gallery's: the gallery declares everything. **Registered claims** (`mechanic` → families): `timed-gate` → `tgate_open_`/`tgate_close_`/`tgate_disarm_`; `actor` → `spawn_actor_`/`unleash_`; `wave-census` → `wave_census_`; `kill-reward` → `k_reward_`; `objective-activation` → `activate_o_`; `class-apply` → `class_apply_`; `npc-talk` → `talk_`; `env-trigger` → `trig_` over **every** declared environment trigger; `press-answer` → `press_` over the `audience: presser` subset that owns one; `cast-ladder` → `cast_` (declared: every NPC the authored ledger casts, from `cast::npc_casts` — the same authority `cast_dispatch` keys off, so an emitter that skips an NPC still declares it); and one `dialogue-mask` claim **per NPC** over `dmask_<npc>_`; and one `cast-bark` claim **per NPC** over `bark_<npc>_` (declared: that NPC's bark scene indices). The per-NPC families are why `Claim::families` is owned rather than a static list — a family prefix can carry an object id and cannot be a literal. **Which families get a claim** is decided by whether a sibling's proof could cover a member's body, read off the emitted bodies rather than asserted: two actors summon different entity types with different NBT at different cells, two classes carry different kits and different party-unique latches, two NPCs dispatch a different number of cast clauses through different mechanisms. The rule is deliberately not *the bodies differ today* — a family whose members happen to be identical modulo their id is one authored field away from not being, and a claim narrowed to what the emitter currently does is the defect this machinery exists to prevent, arriving from inside. **A claim's declared set is never the emitted set.** Several loops must skip members the campaign declares but the compiler emitted no machinery for — a wave whose spawn anchor resolves in no assembled area has no `spawn_<wave>` to drive, and a template calling it is what `DW0497` refuses. Such a loop reads the same traversal the machinery emitter reads, while the claim still declares every authored member; `check_claims` judges only bodies that exist, so an unplaceable wave is silently fine and a placed one the loop skipped is a breach. Prescription: loop the emitter over the declared collection, never over a prefix of it. |

`delve-render` exit codes: `0` ok · `2` input · `3` output · `4` fidelity-gate
failure · `5` renderer/GPU · `10` internal.

#### `delve-render` dark-shot REVIEW POLICY (night-vision emulation)

For shots stamped `{"profile": "dark", "mitigation": "night-vision"}` — and only
those — `delvec scene` emits the Chunky scene with a review-only
`materials` override: every non-light-emitting block of the build's shipped
structure palettes (union over `datapack/data/*/structure/*.nbt`, sorted,
deduped, state brackets stripped) gets a low uniform emittance
(`scene::REVIEW_EMITTANCE` = 0.05), and the scene carries
`"delvewrightReviewPolicy": "night-vision-emulated — review only"` (Chunky
ignores unknown keys). `delvec index` marks the same shots with
`review_policy` and passes the `lighting` stamp through. **This is an honest
approximation, not ground truth**: faint uniform self-glow is the closest
Chunky analogue of Minecraft night vision (which renders every block at full,
flat brightness), chosen after the exposure-boost route failed on the island
cavern; real emitters are deny-listed out of the override so a placed fixture
still reads as a genuine glow. Legibility of geometry/layout in an emulated
frame is reviewable; its *lighting* is not — the compiler's measured light
model remains the only light-truth. Lit-stamped and unstamped shots are
byte-untouched; a dark-stamped shot with no structure palette available is a
`DW0721` error (a silently-black "reviewable" scene would re-blind the review).
Deterministic throughout (`BTreeMap`-sorted override keys, sorted file walk).

### DW0812/DW0813 — the metrics standard (`dsl::metrics`; error + advisory)

| Code | Meaning |
|------|---------|
| `DW0812` | **A document names a metrics entry the table does not define.** A size class, a seam opening, a stair pitch or a storey height that resolves to nothing. Validation tier (exit 1), `every_version`. Raised at `Metrics::resolve`, which is the **only** path from a name to an entry — the map behind it is private — so a name the table does not define cannot be resolved, cannot compile, and no check downstream ever has to cope with one. That is what makes the table the single authority for this vocabulary rather than a suggestion, and it is the reason a second lookup written beside it would be a defect rather than a convenience. The refusal names the bad name **and the whole defined set of that kind**, because the author's next action is choosing a real one and a refusal that only says *no* sends them to read the compiler. **Binding: references resolved.** At the current version that count is **zero documents**, stated here rather than implied: no authored surface names a metrics entry until the layout-graph and site-plan stages exist, so the code lands with its resolver and its tests and no document call site. It is not the UNRUN shape for the reason that shape is about — nothing has to *remember* to call this; the round that adds those stages has no other way to read a name. |
| `DW0813` | **A verdict rests on a standard the metrics gym has not walked.** One warning per run (exit 0), `every_version`, naming every uncalibrated building metric some check above actually read. It asks the campaign for nothing — it reports a property of the ENGINE's own table — which is why no fence grandfathers it and no campaign can adopt its way out. The checks still ran and still refuse: a provisional number is a number, and what the line adds is that the green rests on a seed. **Bound to the READ, not to a call site.** `BuildingEntry::value` is the only way to reach a building metric's number and it takes the run's `Reads` ledger, so a check that consumes a seed has recorded that it did; `Metrics::notice` turns the ledger into the line. The residual is stated rather than implied: a caller that constructs its own ledger, reads through it and drops it has bypassed the notice — a deliberate act, not the omission the rule exists to catch. On the campaign path it is closed rather than merely narrow: `validate_campaign_with` constructs ONE ledger and threads it through the stage-3 and stage-4 checks together, so every building metric either of them rests a verdict on lands in the ledger the notice reads, and there is no second ledger for a read to disappear into. **Binding: building metrics read, and how many of them are provisional, stated every run whether or not the line prints.** Zero provisional reads means the line does not print, which is the calibrated end state and not a vacuity; zero reads at all is a different fact and is the one the binding count exposes. Its live binding today is `delvec metrics`'s own self-check — the table's consistency verdict is a real verdict on real seeds. Walking the gym is what retires it, one entry at a time. **Subject: the ENGINE** (`Subject::Engine`, the only code that declares it today), so it prints once per run as one line, after every diagnostic that is the author's — nothing they write moves it, and it reads identically on every campaign this engine compiles. The message carries the count and the names; this row is the reasoning's home. |

The table itself, its two halves and its export are §10.

### DW0840 — the metrics gym (`compiler::gym`; advisory)

| Code | Meaning |
|------|---------|
| `DW0840` | **The gym leaves a building metric unwalked.** One warning per `delvec metrics --gym` run (exit 0), `every_version`, naming every building entry the generated gym is built from nothing of. The gym's whole argument is that walking it settles the standard, so an entry no bay instantiates is a number the walk cannot rule on however carefully it is walked — a finding about the authoring vocabulary rather than about the run. **Bound to the READ, and stated against the whole table.** The numerator is `Reads` — the same ledger `DW0813` uses — so an entry counts as instantiated only when the generator actually consumed its value to decide a footprint, an opening, a pitch, a datum or the walk's length; the denominator is every entry in the table. A hand-maintained list of "what the gym covers" would be exactly the drift this measures, so there is none, and a table entry added tomorrow and reached by nothing is named the first time anyone regenerates. There is no exemption and no acknowledgement: an entry is reached or it is reported. **Binding: entries instantiated, of entries defined.** Zero unreached entries means the line does not print, which is the calibrated end state and not a vacuity, because the count is taken against the whole table either way. |

At this version the line names **one** entry:

- `pacing.walk-only-blocks-per-minute` is a ceiling for the route coefficient
  beside it, published so the ratio between them is what a playtest measures. No
  verdict reads it and nothing is built from it.

It named three, and the two that left are the reason this measure was worth
having. `corridor.min-width` and `corridor.min-clearance` described a place
narrower than any rung of the size-class ladder admits, and the site plan had no
surface for a place that is not a box with a size class — so a two-wide corridor
was not a thing the pipeline could spell, and there was nothing for a walker to
stand in. The gap was in the **vocabulary**, not in the gym, which is what the
line said and what a bigger gym could not have fixed. It is closed: those two
numbers are the corridor way class's own fields, the generator instantiates a bay
per way class at each width the kit grid admits, and the walk can rule on them.

### DW0814–DW0822 — the layout graph (`dsl::layout`; error + advisory)

Stage-3 of the map pipeline: the campaign's space checked as an object of its
own, **cheaply, before geometry exists to make it expensive**. Every code is
`every_version` for the same reason `DW0812` is — there is no field below
`dsl_version` 0.13.0 in which to write any of it, so no campaign can go red on a
document it did not change.

**One tier, and it is validation (exit 1)** — referential wellformedness,
agreement with the mission, and reachability alike. `dsl::layout::check` is
called from `validate_campaign_with` whenever either document is present, and it
is the only caller of `dsl::layout::reachability`, so there is one battery in one
place, no second copy of any rule, and no step anyone has to remember. `delvec
analyze` and `delvec build` both validate first, so a graph fault still cannot
reach a built world.

**What decides a tier here is when a check can fire, not what kind of question it
asks.** The reachability trio was sorted by kind — `DW0816`/`DW0817`/`DW0819` are
reachability questions like `DW0202`–`DW0204`, so they were raised from
`compiler::analyze::analyze_campaign` — and that put them somewhere no verb could
reach them at the step they exist for. The analysis pass runs only on a campaign
that already validates, and a campaign at the graph step carries `DW0150` by
construction (the plan is written and stage 5 is not), so `delvec analyze`
returned at the validation gate and the graph went unchecked until stage 5 was
written. That is past the design gate, and this section's own first sentence says
the graph is checked *cheaply, before geometry exists to make it expensive*. The
battery is a function of the campaign documents — it reads no plan, no prefab and
no block — which is what let it move.

**The proofs read the mission, so they say so while the mission is unwritten.**
`Grants::of` derives a flag grant from stage-5 quest effects and stage-6 dialogue,
so a campaign between the plan and the quests has no `set-flag` anywhere and every
flag-gated way in its graph is shut to the closure. A **quest**-gated way is not
affected: a quest is credited once every one of its beats sits at a reached place,
and beats are the graph's own document. So a reachability refusal computed while
stage 5 declares no quests carries a caveat naming that state and pointing at
`DW0150` — attached only where the mission's absence could be the cause, which is
a graph that gates on a flag at all, and worded as a caveat rather than a
dismissal, because a place can be unreached for reasons the mission has nothing to
do with and those are findings now. `DW0818`'s clause is the same predicate and a
different consequence: there the absent mission is the whole of the fault, so it
says the refusal clears; here it says which of two readings the author must
choose between.

| Code | Meaning |
|------|---------|
| `DW0814` | **The graph is not a graph.** A duplicate node or edge id, an end naming no declared place, an edge with both ends in one place (at every class — a self-loop states nothing a place does not already state, and would later be a seam with no face to sit on), a `critical_path` step or a `beats[]` entry naming no place, an `entry` or `goal` that is not a node, a place with an empty `intent`. Referential wellformedness, refused before any semantic check runs, because every check below reads node ids and a dangling one would make each of them answer about a place that is not there. Malformed ids are the ordinary `DW0110` and duplicates in the brief the ordinary `DW0111`: an id is an id. Validation tier (exit 1). |
| `DW0816` | **A node the closure never reaches.** Under the monotone closure (§2), a place unreachable from `entry` respecting gating and one-way direction. The message names the place, **the nearest place a body can stand** — so the missing link is visible rather than searched for — and how many of the graph's places are reachable at all. Validation tier (exit 1). While stage 5 declares no quests and the graph gates on a flag, the message carries the unwritten-mission caveat described above. **Binding: places examined, stated.** |
| `DW0817` | **The critical path does not hold.** Four faults under one code, because they are one claim: it does not run `entry` → `goal`; a step names two places no connection joins, or joins them the other way; a step crosses a connection **not open yet at that point in the walk**, judged stepwise against what the beats bound to places already visited have granted (quest-legal order, not merely eventual satisfiability); or it never visits a place where a beat of the **mandatory quest spine** happens, so a body walking it would reach the goal without doing the mission. The spine is the finale and everything its `depends_on` chain demands. Validation tier (exit 1). The unwritten-mission caveat rides the **not-open-yet** fault alone, and only when that connection waits on a flag: the other three are judgements about the graph by itself, which an absent mission has nothing to say about. **Binding: path steps checked and spine beats required, both stated in the binding line**, and a zero on the second is reported there as a finding — a critical path over an unbound graph is a route through nothing. The count is stated rather than raised as a diagnostic on purpose: a line saying *this bound to nothing* is not a fault, and the binding line is where a count belongs. |
| `DW0818` | **The graph names quest-side state that does not exist, or a beat has no place.** Four shapes of one referential rule between the graph and the quest documents: a `beats[]` entry naming a quest or objective the mission does not declare; a `gating` naming a flag no producer sets or a quest that does not exist; a `barred` connection whose `gating` is **empty**, which is passable from world load and therefore not barred; and the reverse direction — an objective in the quest documents bound to no node, or to two. The flag half reads `dsl::validate::produced_flags`, the one producer inventory, so the answer here is the same answer `DW0172` gives. **The reverse direction is the ordering tooth between the mission and the space** (spec-0049 §7): a graph may be authored at any point without touching the quests, and it may not coexist with a mission it ignores. Validation tier (exit 1). **Where stage 5 declares no quests at all**, every name this rule borrows from the mission is absent at once — every beat and every quest-gated way — so each refusal carries a clause saying so and pointing at `DW0150`, which is the one diagnostic that names that state. The clause is attached to both borrowing sites rather than to the beat alone: they are the same borrowing, and a clause on one of them would be a binding narrower than the rule. |
| `DW0819` | **A one-way edge strands.** For every one-way traversal edge `u → v`, some path from `v` back to the critical path must exist over edges passable under the obtained set with which `u` was **first** reached. A body can only be at `v` having been at `u` holding at most that much; if it cannot rejoin the spine, the drop is a softlock. **Marked judgement**: the set at `u` is the maximal one available at that round, and a player may arrive holding less — the residual is covered over bytes by the branch-aware battery, and a walked blockout demonstrating a strand this called green is the evidence that moves it to a gate-state lattice. Validation tier (exit 1). Carries the unwritten-mission caveat on the same terms `DW0816` does. **Binding: one-way edges examined, stated.** |
| `DW0820` | **A shortcut closes no loop.** An edge marked `shortcut` must lie on a cycle: its ends stay connected with it removed. **Direction-blind and gating-blind** — the loop a shortcut closes is spatial, and a long way round that is gated or one-way is still the long way round. A shortcut that closes nothing is a corridor wearing a shortcut's name, and the graph is where that claim is cheap to refuse. Validation tier (exit 1). **Binding: shortcut edges examined, stated.** |
| `DW0875` | **A place is classified twice, or not at all** (spec-0053). A node declares **exactly one of** `size_class` and `way_class`. Both is two answers to one question with nothing to choose between them — every geometric rule below would have to pick, and there is no rule to pick by. Neither is a place with no standard at all: `DW0832` has nothing to hold its extents to and the pacing projection has nothing to cross it in. The refusal names the offending place and, on the second shape, **both defined vocabularies**, because the author's next action is choosing from one of them. Validation tier (exit 1), `every_version` — the rule judges what the document SAYS, and below `0.19.0` there is no `way_class` to write. |
| `DW0822` | **The pacing measurement.** Per critical-path leg, the nominal traverse length from the metrics table's size-class ladder, summed and multiplied by the pacing coefficient into a projected route-minutes figure. **Warning, exit 0, with no threshold anywhere**: the coefficient is uncalibrated until the first walked blockout and the first full playtest, and a threshold on a number that uncertain would be defending nothing. It is printed so the projection and the measurement taken over the built world can be set side by side, which is how the coefficient gets calibrated at all. **Binding: places crossed and steps measured, both in the message.** A **way** leg is measured, never looked up: a way class bounds a cross-section and leaves the run free, so what crossing one costs is its box's LONG horizontal extent, read off the site plan — there is no `nominal_traverse_blocks` on a way class and there is not going to be one, because a route's length is per-campaign geometry and never a standard. Where the campaign carries no site plan yet, a way leg has no geometry and the line says so: the leg is stated **unprojected** in its own binding rather than given an invented number, which is the ordinary state of a graph authored before its embedding and not a fault. **The message is one line**: the figure and the world's own `target_minutes` beside it, so the reader has something to read it against, with the reasoning above left here rather than reprinted every run. Subject: the campaign — it measures THIS graph. |

### DW0824–DW0835 — the site plan (`dsl::siteplan`; error + one advisory)

Stage 4 of the map pipeline: the geometric embedding of the layout graph, judged
**upstream of any geometry**. No block exists when these run and none of them
reads one; what is being judged is whether the plan is a plan — whether the boxes
fit in the region and not in each other, whether two places that claim to connect
really touch, and whether the numbers the brief fixed still hold once the boxes
are drawn.

Every code is `every_version`, for the reason `DW0812` is: there is no field
below `dsl_version` 0.14.0 in which to write any of it, so no campaign can go red
on a document it did not change.

**One tier, and it is validation (exit 1)**, as it is for the layout graph beside
it. Every rule below is a property of the document in front of it, so a plan that
is wrong is wrong before anything is analyzed.

**What invokes them**: `dsl::siteplan::check`, from
`dsl::validate::validate_campaign_with`, whenever the campaign directory holds a
`site-plan.json` — the same event-bound shape stages 2, 3 and 7 use. That
function is what every `delvec` subcommand's validation stage calls, so there is
no path from a campaign directory to a verdict, a world or a datapack that goes
round it, no flag to pass and no step to remember.

**No opt-out exists.** Not one rule here has an acknowledgement, an override or
an exemption field, which is the cheapest available answer to the question
`CLAUDE.md` asks of every escape hatch — *could the defect this hatch exists to
catch supply the hatch's own proof obligation?* A hatch that does not exist
cannot be supplied.

**What is deliberately absent, and where it went.** Three obligations of this
stage are only decidable once the blockout exists: whether a built seam is the
opening the plan allocated, whether every node's floor is reached, whether a
crossing was *discovered* outside a seam; `DW0833`'s second call site over
assembled bytes; and whether a declared sightline is unobstructed. All three read
blocks. A version of them written here would be the derivation's arithmetic
replayed against itself — the opposite of an independent observer — so they
belong to the round that builds the blockout, and this stage states the plan-side
half they will be checked against.

| Code | Meaning |
|------|---------|
| `DW0824` | **The graph and the plan do not agree exactly.** Six correspondences and three references under one claim: everything the graph declares is embedded exactly once, and everything the plan embeds is something the graph declared. A place with no box or with two; a box naming no place; a traversal connection with no seam or with two; a seam naming no connection, or naming a `vision` one (which carries a sightline — a vista's two ends are routinely not adjacent, so the seam construct cannot state what it asserts); a `vision` connection with no sightline; a sightline naming a traversal connection, or whose end does not lie in the place its connection names (the stage-5 proof walks exactly that segment, so ends elsewhere prove a different claim); `stair_in` on a non-stair or naming a third place; an identity naming a fact the brief does not state or a place the plan does not embed. **And the ordering tooth**: a site plan present with no layout graph, or with no geometry brief, is refused naming the missing document — one refusal, not one per dangling name inside the plan. The type carries the other half, which no check could: a box's `node` is required, so a plan that describes a space without naming the place it is the space of does not parse. This is also the **two-artifact question's instrument** (spec-0049 §10): how often it fires *alone* — a graph edit with no plan edit, or the reverse — is the CI-visible number that decides whether graph and plan stay two documents. |
| `DW0825` | **A box leaves the kit grid.** A horizontal extent that is not a multiple of the metrics table's quantum `q`, named per box with the extent, the quantum and the two nearest multiples — the numbers a plan edit needs. A zero extent is not a case here: extents are `NonZeroU32`, so the schema refuses one as `DW0100`. |
| `DW0826` | **Something the plan places leaves the region.** A box cell or a whole-owned volume cell outside `region`, named with the offending span against the region's on each axis. The prescription is deliberate and is in the message: **the region is the brief's number flowing down, and a box is never grounds to grow it** — move the box, shrink it, or change the brief's fact and re-derive so the change is visible in the document that owns it. Volumes answer to it too, because a `massif` outside the region is the whole owning mass beyond its own declared extent, which is extent-flows-up arriving through the back door. **One region is one finding**: where more than one box leaves it, the boxes are one line at `/content/boxes` stating the count, the region and every offender with its own overrun, and the volumes are one line at `/content/volumes`; a single offender still gets its own line at its own index. |
| `DW0827` | **Two boxes overlap.** Boxes are disjoint; shared **faces** are the only permitted contact, because a seam needs one — and a shared face is a one-cell gap rather than a touch. Named with both places and the intersection on all three axes. Overlapping boxes are two owners for one block and the derivation would have to pick between them with no rule to pick by. |
| `DW0828` | **A seam is not on a shared face.** The declared face is not shared by the edge's two boxes — they are not one cell apart across it (the message states the gap it measured, and an overlap is stated as one), their spans miss each other on an in-plane axis, or a horizontal seam names a sky-open place with no ceiling or floor plane to sit in — or the seam's `at` corner lies off the shared area. **Seams are allocated on faces both boxes already have**: the two-places-cannot-mate failure class is resolved here, where both boxes are still free to move, and never later between two finished buildings. The refusal prints both boxes' cell ranges and floors. |
| `DW0829` | **A seam's opening does not fit, or its sill cannot be reached.** Two halves of one claim that the opening is usable. Geometric: the named standard's `width × height` anchored at `at` runs past the shared face, named with both rectangles — the standard set is the vocabulary, so an opening is never quietly cropped to fit. Physical: on a vertical face, a `walk` or `barred` seam's sill is more blocks over the floor of a side the connection is entered from than a body reaches by jumping, so the connection the graph declares is not one; the prescription is to drop the sill or declare a `stair` and let the treads carry the climb. (An opening name the table does not define is `DW0812` — that is the table being the single authority for the vocabulary, and this code is the geometric half.) |
| `DW0830` | **A stair seam cannot be built.** Three shapes of one claim. The seam declares no `stair_in`, so massing that has to stand somewhere stands nowhere. The two floors are the same plane, so the stair climbs nothing — which is a walk that has been called a stair, and is catchable precisely *because* the rise is derived rather than authored. Or the **climb** needs a longer run than the hosting box really affords at **every** standard pitch. Climb and run are both read from `siteplan::stair_run`, the one function the derivation lays treads by, so a plan that reaches green here is a plan the derivation can build. The climb is what the OPENING asks for — the seam's own sill across a vertical face, the pierced plane through a floor or ceiling — which is the rise on an ordinary plan and is not the rise when the sill sits somewhere other than the far floor. The run is the host's whole extent on the seam's normal across a vertical face, and beside a pierced floor it is the room on the roomier side of the hole plus the hole's own width, because the treads leave along one side of it. The message names the rise, the climb, the tightest standard's required run and the run available; the prescription is a longer host, a hole with more room beside it, the other host, or closer floors — never a steeper pitch, because the pitches are standards. |
| `DW0831` | **A drop seam falls outside the drop policy.** The derived fall along the edge's declared direction is zero or negative — a drop that rises is a mislabelled stair, and a drop is one-way only because a body cannot climb back up the way it came — or it exceeds the metrics table's designed-drop cap. A **policy** cap, deliberately far tighter than the survivability fact stored beside it in the player half: a drop is a decision about the shape of the map and should not also be a decision about the party's health. |
| `DW0832` | **A box violates its node's class** — either kind. For a **size class**: interior footprint on either horizontal axis, or declared headroom, outside the class's range. For a **way class** (spec-0053): the box's *shorter* horizontal extent is the cross-section and must lie in the class's range; the *longer* is the run and must **strictly exceed** the class's `max_width`. That third demand is the elongation, and it is derived from the class's own widest cross-section rather than seeded because it is exactly what a room cannot supply — **a square box can never satisfy it**, since its run equals its width and one number cannot both be `<= max_width` and exceed it. So "declare a room a way to escape the ladder" is refused by the object's own shape rather than by a rule the author could satisfy by choosing differently, which is the property an opt-out owes. There is no maximum run and there is not going to be one. This is the one place a classification becomes geometry; its playtime weight stays thresholdless (`DW0822`). A sky-open place is not judged on headroom — it has exactly the class minimum by construction. |
| `DW0876` | **A seam does not declare a connection this engine builds** (spec-0053). **Four shapes of one claim**, because a seam exhibiting any of them has no crossing for any rule below to judge and the author's next action is the same in every case: it declares neither an `opening` nor a `contact`, or both; its contact's span leaves the face `DW0828` established; its contact's span is **not wider than the broadest standard opening**; or it is a contact on a `stair`, `barred` or `vision` connection. The width floor is **derived from the standard opening set, not seeded**, and that is what makes it honest: anything at or under it could have been a portal, so a doorway declared a contact to dodge the standard set is refused by its own width — a demand the defect cannot supply. The class exclusion is the standing falsifier re-armed rather than an oversight: a stair needs a run and a pitch, a barred door needs a gate region that seals and clears, and a sightline is not a crossing at all. (A `vision` connection carrying a seam at all is `DW0824`, which reaches it first.) Validation tier (exit 1), `every_version`. |
| `DW0833` | **A brief identity does not hold.** An `identities[]` comparison is false. The refusal names **both numbers** and quotes the brief's own sentence, because the author's next action is deciding which of the two to move — and the prescription says where: change the fact *in the brief*, where the design is written down, so the change is a decision somebody took rather than a plan that drifted. Raised here over the plan; **its second call site is the built world**, where the same rule recomputes the same measures from assembled bytes so a derivation defect that moved a datum cannot hide behind a plan-time green. That site's own prescription is different and is stated in §5.3: the mass is what disagrees, and the plan's own massing is as likely to be the cause as the derivation. That site belongs to the round that builds the blockout, and nothing here approximates it. |
| `DW0834` | **The identity gate binds nothing.** Zero `facts[]` in the brief, or zero `identities[]` in the plan: the binding that holds the whole to its written design is empty, which is the vacuity the whole stage exists to prevent — with either side empty the plan may say anything at all and every rule above still passes, because none of them has an opinion about how big the map was meant to be. **Warning (exit 0)**, naming the empty side, so a deliberately minimal plan stays compilable; printed every run, so the emptiness is never quietly a pass. |
| `DW0835` | **A whole-owned volume enters a box.** A `volumes[]` region intersecting a box's play space, named with both and the intersection. The whole's mass may stand beside a place, under it and over it; inside it, the volume and the place are two authorities writing one cell, which the derivation must never be asked to arbitrate. |

### DW0821/DW0836–DW0839 — the derived blockout (`compiler::blockout`; error + two advisories)

Stage 5 of the map pipeline: the whole map's mass, derived from the site plan and
the metrics table, and then **judged against the plan it was derived from**.

**There is no authored form.** No document, no schema, no file. The only path to
blockout bytes is `compiler::blockout::derive`, whose input is a validated site
plan, and the only caller is `Plan::build` — which is the only constructor
`build`, `analyze`, `snapshot`, `viewer`, `blocking-chart` and `edit` can reach a
world through. That is what makes *blockout before site plan* uncompilable rather
than forbidden: there is nothing to author early.

**The mass is region writes, not a structure template.** A blockout box is a
shell — six faces of one uniform block around a volume of air — so a `.nbt`
packaging of it would be tens of thousands of mostly-air cells split across tiles
past vanilla's 48-per-axis cap, and would need a template writer this crate
cannot reach. A derived piece is therefore a `PiecePlacement` with **no
templates** whose blocks live in `AreaPlacement::mass`, applied in
`assembled::placed_blocks` one step ahead of the socket seals that already had
that shape. Nothing downstream special-cases it: `bbox()` answers for forceload
and relight off `pos`/`size` exactly as before, and a piece the prefab registry
has never heard of contributes no face contract and no anchors — which is
correct, because a derived box makes no claim about mating with anything. Each
write is split at the point of writing so none exceeds what one vanilla `fill`
will accept; a `fill` the server refuses fails in a function nobody reads.

**No seed reaches the derivation.** It takes the plan and the table and nothing
else — no RNG, no clock, no hash-order iteration — so the same plan derives the
same mass, and changing `world.seed` changes no blockout byte.

**The synthesized vocabulary** (what makes the unchanged quest layer land on
massing nobody authored):

| Name | What it is |
|---|---|
| `spawn` | The entry place's footing. The derivation **declares** it, exactly as a prefab does: the anchor arrives carrying `"role": "entry"`, so the graph's own `entry` node is what decides, and the spelling is a name content may address rather than the thing resolution reads. |
| `anchor/node-<place>` | A place's own footing. `node/near-hall` becomes `anchor/node-near-hall`, because a campaign reaches an anchor through `anchor/<kebab>` and `node/<id>` is not a name any document could write. |
| `anchor/seam-<edge>` | A `barred` seam's gate region, filled at world load with the bar and measured shut by the same gate-seal model a prefab-authored gate is. `open-gate` and `shortcut` address it. |
| `anchor/unlock-<edge>` | The far-side affordance's footing, on the side a one-sided `barred` seam opens from. Absent on an `either` seam, which needs none. |

Each is read off the mass **after** it is laid, not computed from the plan: a
stair the plan hosts in a box legitimately stands on that box's centre, so an
anchor placed by arithmetic alone lands inside the massing and `summon` does no
snapping. `dsl::siteplan::synthesized_anchors` is the one authority for the
names — validation resolves a campaign's anchor references against it and the
derivation places exactly it, so a name that validates cannot fail to exist.

**What invokes the battery**: `emit::build_with_warnings`, the one function that
turns a `Plan` into a datapack. A campaign with no site plan gets nothing and its
output does not move. There is no flag, no subcommand and no line in a document
to remember, and someone building a site-plan world without it would have had to
emit a datapack without `build_with_warnings`.

**Why the battery is an observer and not a replay.** Every verdict compares what
the plan declares with what the assembled bytes are. It does not know where the
derivation put a floor course, how it chose a pitch, or which cells it cleared —
it knows the plan, resolved by the same `dsl::siteplan` code stage 4 judged, and
it knows the world. That claim is demonstrated rather than asserted: each of
`DW0833`, `DW0836`, `DW0837` and `DW0838` is reddened in test by a **deliberately
perturbed derivation** (`blockout::Perturb`), never by hand-authored bytes, and the
production path passes `Perturb::none()` as a literal with a test asserting it.
`low_ceiling` closes one place a course under its plan's ceiling and moves nothing
else — no datum, no opening, no footing — so the whole error list under it is
`DW0833` alone, which is what makes it a defect only the headroom measure can see.

**And the demonstration is reachable from the command line.** `delvec build
<dir> --perturb <knob>` asks the derivation for one named defect and runs the
observer over the result, so the claim above is something a creator watches
happen on their own campaign rather than something they take from a test
transcript. The knobs are every field of `Perturb`, offered under
`--help` from `blockout::Knob` — `slide-openings`, `sink`, `short-walls`,
`brick-up`, `low-ceiling`, `wall-contacts` — and the three that damage one place
(`sink`, `brick-up`, `low-ceiling`) take `--perturb-place <place>`, checked
against the site plan's own boxes before anything derives. One knob per run: two
at once and the code that fires says nothing about which defect it saw.

**A perturbed build writes nothing, and cannot.** `--out` and `--perturb` are
declared as conflicting arguments, so the invocation carrying both is refused by
the parser; a perturbed run has no output path at all, rather than a decision not
to use one. No tree means no `manifest.json`, which is the file
`tools/staging-gate.py` hashes to give a build the identity an admission token
binds to — so a perturbed tree is unadmittable by construction. The exit is the
build tier whatever happens, and a perturbed derivation that NOTHING refuses is
itself a refusal, in those words: that outcome is precisely the failure the
facility exists to be able to see, and it also occurs honestly when a campaign
has nothing for that defect to damage (the metrics gym allocates no contact, so
`wall-contacts` reaches nothing there).

**A build stops at its first refusal and the battery states all of them.**
`BuildFailure` carries one code and one message, as every check in this compiler
does; one derivation defect is routinely seen by two of these rules, so the
battery prints a refusal line counting every code it raised and the messages
after the first, capped at twenty. Walls a course tall are the standing example:
they open every wall above its allocation (`DW0836`, which stops the build) and
join two places nothing connected (`DW0838`).

The step rule is the compiler's own (`nav::World::neighbors`), whose visibility
was widened for this rather than copied — a second step rule would make this the
one proof in the compiler taken under different physics.

| Code | Rule |
|---|---|
| `DW0836` | **A built seam disagrees with its allocation.** Three claims over the bytes. *Every allocated cell is passable* — a hole the derivation failed to cut is a connection the graph declares and the world does not have. *No other cell of the shared wall is passable*, asked per **wall** rather than per seam, because two connections may legitimately pierce one wall and the union of their openings is what it is allowed to have. *The realized rise equals the declared rise*, measured as the lowest cell a body can stand on inside each place — so a floor course laid at the wrong height disagrees with the plan that put the two places at those datums. Build tier (exit 3), `every_version`. **Binding: seams proven, and shared walls examined.** |
| `DW0837` | **A node's floor is unreached.** Per-cell reachability from the campaign's own spawn over the assembled world, with every way the graph's monotone gating closure never opens sealed as the plan sealed it — the base assembled model holds gate regions open, so a proof taken over it would walk through a door nothing unlocks. A declared `drop` is **seeded**, not walked: the step rule models no free fall (a router that could fall would prove routes a body cannot come back from), so the far side of a fall whose near side is already stood in is handed a starting cell and the closure iterates. That is the graph's own declaration carried into the bytes, never a widening of the step rule. The graph's `DW0816` proved this over topology; this proves the derivation preserved it in blocks. Build tier (exit 3), `every_version`. **Binding: places proven.** |
| `DW0877` | **A contact nothing can cross** (spec-0053). The measured crossing profile of a contact's span — the columns of it a body crosses over the assembled bytes, under `nav::World::neighbors`, the same step rule every route proof in this compiler is taken under and unwidened — holds no unbroken run of body width. The author allocated a front and the massing walled it, so the graph declares a hand-off the world does not have. It is the **contact's half of `DW0836`'s first claim, and a different claim rather than that one widened**: a portal is a hole and *every* cell the plan allocated must be clear, while a contact is continuous ground and massing standing on part of it is content — a rim with a boulder on it is still a rim. Asking a portal's question of a front would refuse correct content. **The one asymmetry**: a `walk` contact's column must let a body step out on both sides, because walking ground is two-way; a `drop` contact's only on the high side, because the far side of a fall is precisely what the step rule does not model and `DW0837` already seeds rather than walks it — one policy about drops in this engine, not two. Build tier (exit 3), `every_version`. **Binding: contacts examined, and crossable columns measured**, both stated beside the seam count, because zero columns over zero contacts and zero over three are different facts and only the pair separates them. Reddened in test by a **perturbed derivation** (`Perturb::wall_contacts`), never by hand-authored bytes. |
| `DW0838` | **A connection nothing allocated.** Delete every allocated opening from the world and no two places may still be walk-connected. **Departure from spec-0049 §5.3, recorded**: the spec states this as *every legal step between a cell owned by one box and a cell owned by another*, and read literally that rule is vacuous by construction — the plan's own `DW0828` puts exactly one cell between any two boxes, so no cell of one is ever a cardinal neighbour of a cell of the other and the rule quantifies over an empty set forever. The claim is therefore made over **paths**, which is the same claim and can fail: it catches the crossings the step form was reaching for and could not see — a wall the massing left low, a corner two shells did not close, a roof one open place lets a body onto — none of which is a single step between two owned cells either. Build tier (exit 3), `every_version`. **Binding: standable cells classified, and place pairs tested.** |
| `DW0821` | **A sightline is blocked.** The DDA walk of a declared `vision` edge's segment — `nav::walk_cells`, the same exact grid traversal the cutscene clip is proven with — naming **every** blocking cell rather than the first, because a walk sheet that names one cell of a wall has not said where the wall is. **Warning (exit 0) while any box is unbound; an error (exit 3) once `details[]` binds every graph node.** Derived massing has no landform, so a vista the detail pass will carve a ridge for is blocked here by the shells themselves, and refusing it then would force hand-shaped massing into a derivation whose whole property is that nobody shapes it. A fully detailed map has nothing left to carve, so the same world becomes a refusal. The severity is **computed from the artifact** — `compiler::detail::fully_detailed` — rather than set by a stage marker or an author flag, so there is nothing to set, nothing to forget, and no author who can choose the lenient reading. `every_version`. **Binding: sightlines walked.** |
| `DW0839` | **Two placement authorities in one campaign.** A stage-1 world declaring `areas[]` in a campaign that also carries a site plan. `areas[]` seats prefab pieces on the compiler's fixed stride and the site plan seats the derived blockout in its own declared region, so a world with both has two answers to every question about where something is and nothing says which. One or the other, per campaign; both surfaces stay legal at 0.14.0. Validation tier (exit 1), `every_version`. |
| `DW0851` | **The ambient sea covers the walk region.** A reachable, standable cell of a `horizon: ocean` world whose **head** cell the ambient sea fills once the world loads — the build proves a body stands there and the game gives it a body that swims. `compiler::nav` (`measure_sea_seepage` / `SeaSeepage::finding`), build-tier (exit 3), at the same **two** call sites as `DW0318`/`DW0322`: once per world-edits batch in the stage-8 replay, naming the batch, and once over the finished assembled world at stage 10. **The gap it closes.** The world model holds water in two disjoint places and only one of them reached walkability: `assembled::Occupancy::flooded` is seeded from the assembled BLOCK MAP — prefab-authored sources and waterlogged blocks — while the ambient sea is not in that map at all, and `nav::World::ambient_water` (the only predicate that knew about it) had exactly one reader, the stranding proof's sea *surface*. So the sea never reached `flooded`, never reached `is_occupied`, and never reached `is_standable`: a cell inside a placed piece that the sea was about to fill was proved standable and dry, and the route proof, the wave seating and the exported waypoints all stood on it. **The model**, in four steps. *Seeds*: every non-blocking cell INSIDE the built volume, in the sea's own band (`floor_top < y ≤ level`), 6-adjacent to an ambient sea cell — the contact face. *Flow*: `assembled::flood`, the same function the block map's water runs through (infinite-water source formation, then 7-level decay with infinite downward fall) — deliberately not a second physics, so one room cannot be judged wet by one model and dry by the other. *Confinement*: the one-cell skin of non-built cells around the built volume becomes barrier, so the flow stays inside the content; what leaves it is `DW0318`'s question. *Verdict*: a reachable standable cell whose head cell the flow reaches. **Why the head cell and not the feet.** Feet wet with a dry head is *wading* and vanilla lets a body walk it — the map says ground, the game says shin-deep water, and both are true. A wet head is *swimming*: the map says a body stands here and the game says it cannot, which is a contradiction about one fact and is what a proof may refuse. The line was chosen against a measurement, not in the abstract — the released `nobodys-cave-island` walks a 26-cell strip of its west bank at exactly sea level, every reachable cell it has at or below the waterline, and not one head-deep; a feet-cell verdict refuses that shoreline, which is how a diagnostic gets weakened later by somebody who needs it green. **Runs after `DW0318` and before `DW0322`**, inside `verify_boundary_safety` so no caller has an order to get wrong, and for the same reason the fluid proof runs first: a walk region the sea is about to fill is a false premise of the stranding proof, which then prescribes a shoreline step for a room nobody can walk in. The walk region is computed once and handed to both, so the two cannot judge different sets of cells. **Direction of error**: the seeds enter the flow as *sources* where vanilla would start them one level down, so a wide contact face fills further than the game would — over-marking, which can only turn a proof red, never let a wet cell ship as proven dry. **Not the shoreline**: a shore piece that authors its own water to the waterline (`DW0344`, spec-0048) has it in the block map already, so those cells are `flooded`, not standable, and never reachable. **Binding**: pieces examined, open contact-face cells, cells the sea reaches, walk cells examined, and the wading count — all in the message and in `validation/sea-seepage.json`; none is the length of the finding list, and `contact_face_cells: 0` is a watertight hull saying so rather than a silence. Prescription: close the face, raise the floor clear of sea level, or author the water so the room IS flooded and every proof downstream knows it — never move the path around it. |
| `DW0852` | **A stealth judge asks a player for something other than where they are.** A `stealth_eval_*` function's per-player `if entity @s[…]` test uses a selector argument outside `x`/`dx`/`y`/`dy`/`z`/`dz`. `compiler::emit` (`audit_stealth_judges` / `StealthJudgeAudit::finding`), build-tier (exit 3), run over the **final** emitted function list — after every emitter has had its say, so a later pass that rewrote a judge cannot slip past a check that ran earlier. A stealth beat is hiding and hiding is a place; `emit_stealth_functions` has said so in its own doc comment since v0.6 (*zone presence alone = hidden*), and a playtester still met a beat that quietly demanded a crouch the fiction had never asked for. A promise in a doc comment is a doc line, and a doc line is not an invocation — this is the invocation. **An allowlist, not a denylist**: the rule names the six arguments a position test may use, because a list of forbidden ones answers *not one of the ones I knew about* to the next demand, honestly and wrongly. **Scoped to the judge, not the dispatcher**: `stealth_tick_*`'s `@a[tag=!dw_cutscene]` is non-positional and correct — skipping a player watching a cinematic freezes the grace clock rather than asking that player for anything — so the rule distinguishes *who is judged* from *what the judgement asks for* and only owns the second. **What stops it going quiet**: the judges it found must equal the beats the plan holds, so a rename that made them invisible reds as an internal-invariant violation instead of examining zero and passing — the truncated-input mode, where the count is neither zero nor wrong but is about a smaller world than the check claims to cover. Binding: beats, judges, per-player tests examined and the argument vocabulary actually used, in the message and in `validation/stealth-judge.json`. A campaign that declares no stealth beat audits nothing and ships no ledger, so a file that exists and reports zero examined tests is a finding rather than an absence. Prescription: if a beat genuinely needs a posture or a held item, that is a DSL surface to propose — never a predicate to add to the judge, and never a widened allowlist. |
| `DW0853` | **A horizon param is out of range, or belongs to another base.** `delvewright_dsl::validate` (`reserved_v16`), validation tier (exit 1), `Since(16)`. The `horizon` object form is one flat schema rather than one per base, because a tagged union per base would make the common case — a base and nothing else — the awkward one. The price of a flat shape is that a param can sit beside a base that reads nothing from it, and this is that price paid rather than absorbed: an `ocean` carrying a `rim_height` parses perfectly, and the author who wrote it believes something is reading it. The range half is `ratio` (2.0..=3.0) and `rim_height` (16..=128), checked on the **resolved** view so a shorthand is judged by the same rule as the object form it desugars to, with both bounds and the default in the message and the reason for each bound stated — under the `ratio` floor the annulus has no room for a gap floor and a slope run both, over the ceiling it is mostly terrain no body reaches at a cost that is all shipped bytes. **Restated at build time under the same code**, from the generator's own range guard, because one rule with two names is two rules that will disagree. Binding: campaigns declaring a `horizon`, of which those declaring the object form are param-checked. |
| `DW0854` | **The surround's inner slope has grown a standable staircase.** `compiler::emit`, build tier (exit 3), `Since(16)`, run inside the world block beside the boundary proofs. A walk flood starting on the surround's own gap-floor cells reached a column outward of the crest line, so the landform no longer bounds the map and a body can walk out of the delve over the mountains. **Why it is a check and not an argument.** The generator already guarantees this by construction: no surround column stands exactly one block above the gap-floor datum, so the floor's own walkable component is bounded above by the datum and the first thing outward of it is a two-block riser, which vanilla's auto-step and jump cannot take. But that is a property of what the generator wrote, and between the generator and the world there is a gravity settle, possibly a stage-7 edit script, and a palette whose blocks may be a different height — any of which can put back the riser the generator never wrote. So the proof reads BYTES: it floods the same `nav::World` every route proof uses and asks the same step rule, sharing none of the generator's arithmetic, which is what makes it an observer of the derivation rather than a restatement of it. The generator runs its own flood too, over its finished tile contents, so a violation dies at generation as well — two proofs of one property at two moments, and the later one is the one that is about the shipped world. Binding: standable gap-floor cells the flood starts from, stated on every surround build, because a flood that started from nowhere passes for free and looks exactly like this one. |
| `DW0855` | **A horizon that builds terrain, on a campaign with no map to build it around.** `delvewright_dsl::validate` (`reserved_v16`), validation tier (exit 1), `Since(16)`, restated at build time under the same code. A surround rings a **declared** extent, and the only statement of a whole map's extent this engine has is a site plan's `region` — which is required, non-derivable, and which no box may grow. A campaign that seats its pieces with `areas[]` states no extent at all. **The substitute is the whole point of the refusal**: the union of whatever `areas[]` happens to place looks like an extent and is not one, because areas sit on the compiler's fixed 256-block stride, so that union is mostly the void between them and ringing it builds a mountain range around empty space. Measured rather than argued: the same surround around the gallery site plan's declared 64x64 region is fourteen templates in about ninety seconds, and around the union of the gallery primary's two hand-placed areas it had not finished in ten minutes. The fast answer and the correct answer are the same answer, which is usually the sign that the substitute was never the thing. Prescription: give the campaign a site plan, or declare `void` or `ocean`, which need no map to be a horizon of — never widen the union. |

`DW0833` and `DW0822` run their **second call sites** here, and the pair is the
point of each:

- `DW0833` re-measures the brief's identities off the assembled bytes, so a
  derivation defect that moved a datum cannot hide behind a plan-time green — a
  floor laid one block low satisfies every stage-4 rule, because stage 4 never
  saw a block. **`box-height` is measured over the whole footprint**: the tallest
  stack of clear cells standing over the place's realized walk plane at any
  column, capped at the declared clearance. The maximum, because every place this
  derivation builds has a flat ceiling — an unmassed column answers the height,
  and massing the plan itself put here can only ever answer less, so it cannot
  inflate the reading. `box-extent` is measured at the **top course** of the play
  space for the same reason on the horizontal axes: a stair the plan hosts here
  legitimately stands on the floor, and a measurement taken there reports the
  room as smaller than it is. **The prescription names both repairs**: a
  disagreement between plan and bytes is a disagreement about the MASS, and only
  one of the two things that put mass in a place is a defect — the derivation may
  have built it wrong, or the plan may have given the place a run of treads it was
  never given the room for, in which case the repair is to move the seam, host the
  stair in the other place, or widen the host. **Departure, recorded**: four of the five measures have a
  byte-side referent (a box's built footprint, its built headroom, the distance
  between two built places, a datum's realized walk plane) and `region-extent`
  does not. A region is a declaration the plan's contents must fit inside
  (`DW0826`); nothing is required to reach its edges and the derivation builds no
  object whose extent it is, so re-measuring it as *the extent of whatever got
  built* would refuse every plan that leaves a margin. Such an identity is
  evaluated once and counted as **declaration-only** in the binding line rather
  than passed over in silence.
- `DW0822` prints the **measured** A* route along the layout graph's own critical
  path, beside the projection stage 3 made of it. Neither carries a threshold;
  they exist to be set side by side, which is the only way the pacing coefficient
  gets calibrated at all.

**Binding line.** Every build of a site-plan campaign prints two: what the
derivation massed (places, seams with their stair and barred counts,
whole-owned volumes, anchors synthesized, region writes and the cells they cover)
and what the battery examined (seams, walls, places, standable cells, place
pairs, sightlines, identities with the declaration-only count, and critical-path
legs).

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
| Assembled-relight, measured `DW0210`, `DW0211`/`DW0196`, stage-1 `lighting`/`time`/`weather`, `set-time`/`set-weather` (all v0.5) | spec-0010 (landed) |
| Stage-1 `horizon` (ocean superflat), `boundary` (derived playable region + 1s return clock), `dw:region`/`dw:cp` mirrors, `DW0320`/`DW0321` (all v0.6) | spec-0013 (landed) |
| Sound + art-title surface (`play-sound`, `narrate` `art`, `delve:art` font, `DW0326`/`DW0328`/`DW0335`) | spec-0014 (v0.6) |
| Traps: stage-5 `traps[]`, `anchor/trap` dispenser fill + disarm emission, `tnt_explodes` seal, passable plate/tripwire model, `DW0340`/`DW0341`/`DW0342` (all v0.6) | spec-0011 (landed) |
| Visual authoring loop: `delvec snapshot` + `delvec blocking-chart`, the voxel raycaster, scene manifest and cutaway floor plans (§7) | spec-0015 (P1+P2 landed) |
| Souls-mode timed gates: stage-5 `timed_gates[]`, the two-function schedule clock, `DW0377`/`DW0378` (≥20% of cycle) (v0.6) | spec-0016 §4 |
| Hazard observability: every `timed-gate` span and `volley` kill zone needs a watch cell — standoff, pre-commit reachability, sightline (`DW0388`; error for a bonfire campaign, warning otherwise) | spec-0016 §4 addendum (dossier G1) |
| Timed-gate `disarm`: the hazard ladder's third rung — a jam lever that stops the clock with the gate resting open, permanently (`DW0377` structural, `DW0389` no re-arm, `DW0393` reachable while shut, `DW0420` visible) | souls dossier §5.2 |
| Timed-gate `crush`: the closing edge kills players caught in the region, by command (default off, byte-identical when unset) | spec-0016 §4 addendum |
| Affordance hardware: every compiler-owned right-click target carries its own visible, glowing `dw_hw_<tag>` display; `DW0420`/`DW0421` | spec-0016 §2 (drowned-bell playtest) |
| Souls-mode ambushes: stage-5 `ambushes[]` (parse-time desugaring to a trigger), `DW0375`/`DW0376`, optional telegraph (v0.6) | spec-0016 §3 |
| Souls-mode TD lanes: wave `lane{waypoints,aggro_radius}` + `summon: aggro-edge`, the Raider-patrol clock, `DW0381`–`DW0387`, `pillager`/`vindicator` added to the armed-mob default table (v0.6) | spec-0016 §6 |
| Souls-mode shortcut doors: stage-5 `shortcuts[]`, `DW0371`/`DW0372`/`DW0373`/`DW0374`, shortcut gates sealed for the whole completability model (v0.6) | spec-0016 §2 |
| Souls-mode pacing lints: retry cost `DW0379`, optional-elite bypass `DW0380` (both warning tier) (v0.6) | spec-0016 §7 |
| Souls-mode bonfires: `bonfire{anchor,on_rest?}`, wave `respawns_on_rest`, `DW0370` (v0.6); the two-option rest dialog + authored labels, the class-kit `flask` + `DW0476`, the flask's potion `contents` + `DW0486`/`DW0487`, the critical path's `rest` step (v0.8); the stationed re-seat + the bonfire safe zone `DW0478`, whose lane term includes the measured marching drift; the **undefeated re-seat** — a still-standing actor elite / billed wave is deleted and re-seated fresh at its origin on rest and on death-respawn, a defeated one stays defeated, `DW0489` | spec-0016 §1 |
| The map editor: stage-7 `world-edits.json`, the full L3 verb set (`select`/`fill`/`replace`/`carve`/`morph`/`scatter`/`plant`/`fragment`/`relight`), the L2 massing verbs (`swap`/`insert`/`remove`/`rewire-socket`/`reseed`; `resize` excluded — no size primitive), per-batch invariant re-proofs, `DW0162`/`DW0322`/`DW0323`/`DW0324`, `delvec edit apply|preview` (all v0.6) | spec-0017 |
| Map-editor audit fixes: trap-hardware integrity `DW0352`, gate-region + block-support advisories `DW0353`/`DW0354`, out-of-bbox edit-chunk load convergence + forceload release, `edit` running the full build-tier proof set, blockstate-preserving `fragment` stamps | map-editor audit |
| Party-shared progression: the `#party` holder, party-addressed UI, `world.min_players` + lobby gate, `give-item`/kit `carrier`, the n-agent division proof and the n-dummy `party_join_<obj>` PackTests, `DW0356`/`DW0357`/`DW0358` (all v0.6) | spec-0018 (landed) |
| The NPC scene ledger: stage-5 `cast` (DSL **v0.7**), the four build proofs `DW0460`–`DW0462`, the forcing function `DW0463`, dangling refs `DW0464`, the pre-0.7 deprecation window `DW0465`, the `"unchanged"` sugar `DW0466`, the staleness lint `DW0467`, the dead-clause proof `DW0846` and the unanswerable-objective refusal `DW0858`; the `dw.cast` scene dispatch + bark pools; cast roots as dialogue entry points | spec-0020 |
| Combat verification: wave `tier` (DSL **v0.7**) and actor `tier` (DSL **v0.8**), the winnability arithmetic `DW0470`–`DW0473` + the advisories `DW0474`/`DW0475`/`DW0477`, the vendored `item-combat` / `damage-types` tables, `validation/combat-plan.json` (encounters + tiered actors + the floor-gate coverage ledger), and the bot ladder's die-retry stage / assist windows / inverted floor gate | spec-0023 |
| Branch-complete narrative verification: stage-4 `branch_points`, the per-node `happening`, the named `campaign-complete` `ending` (DSL **v0.8**); the six proofs `DW0480`–`DW0485`; `validation/branch-plan.json` + the per-branch chronicle + the per-branch executable path, and the harness's scripted-choice branch runs (`DELVEWRIGHT_BRANCH`/`DELVEWRIGHT_BRANCHES`, `validation/branch-runs.sh`) | spec-0025 (the `from-diff` PR tier still needs a compiler-side diff→branches map) |
| Asset-pipeline tooling `DW07xx` (schem/render/admit) | spec-0007 |
| Determinism invariants | ADR-0006 |

### Known spec ↔ code drift (current, for maintainers)

- **Effect-root drift is CLOSED, structurally, except one named finding.** An
  *effect root* is a `Vec<QuestEffect>` emission can lower. There are seven; six
  hang off the quests stage and one off dialogue. Six walks were found blind
  to a root and fixed one at a time, by six unrelated investigations; a sweep after
  the sixth found **thirteen more**, and the sweep's own guard then found **three
  the sweep had missed**. Not one was ever red — a walk that visits four of five
  roots produces correct-looking output over any campaign that does not use the
  fifth.

  **And then a seventh instance, of a different shape** (spec-0031).
  `shortcuts[].on_unlock` (spec-0016 §2) had been a `Vec<QuestEffect>` emission
  lowered since it was written, and it was not in the enumeration at all — so
  every walk that correctly inherited the roots still could not see it, and
  `tools/check-effect-roots.py` could not either, because that gate greps for the
  roots it knows. The enumeration protects against forgetting a KNOWN root; the
  gate for a root nobody knows is `tools/check-capability-ownership.py` check E,
  which reads the effect-bundle FIELDS out of `stages.rs` and fails on any it
  cannot account for. That is what surfaced this one. Zero campaigns had used
  `on_unlock`, which is the only reason it never shipped as a bug: a `narrate`
  inside it was never inventoried, a `set-flag` was invisible to the flag model,
  and a `sequence` would have emitted a call to a function nothing generated.
  It is root R6 now, and the campaign's `on_death` (also spec-0031) is R7.

  Fixing them one at a time was never going to close it, so the roots are now
  enumerated **once**:

  - `delvewright_dsl::effects::for_each_effect_root` is the single enumeration.
    `for_each_effect_root_mut` (the l10n write path) is generated from the **same
    macro body**, so the ref/mut pair cannot drift either.
  - `dsl::for_each_campaign_effect` = that walk + the single nesting authority
    (`QuestEffect::nested_effect_lists_labeled`). Both axes inherited, neither
    listed. Its `EffectSite` gained a `DialogueRespawn` variant — its absence was
    load-bearing, because root 5 was not *representable* in the callback — and
    `ShortcutUnlock` / `OnDeath` for roots 6 and 7.
  - `compiler::plan::for_each_effect_root` and `l10n::effect_roots` /
    `effect_roots_mut` are thin adapters over it. Four separate enumerations became
    one.
  - `EffectRootKind::ALL` is the closed set, and the walk **asserts on every call**
    that it enumerated all of them — in release builds too, because a root quietly
    dropping out of the enumeration has no other symptom.
  - Every walk reports a `RootBinding`: how many roots it enumerated and how many
    bundles each bound to on this campaign. *Enumerated the root* and *this
    campaign uses the root* are different facts, and a proof that conflates them
    reports a vacuous green as a pass.

  Adding a root is one edit in that macro, and every walk below inherits it —
  demonstrated by spec-0031, which added two.

  **The claim is now also a test matrix.** `tests/effect_root_walkers.rs` iterates
  `EffectRootKind::ALL`, builds one campaign per root from an exhaustive `match`
  (an eighth root is a compile error there), and asks six walkers — the
  enumeration, `for_each_campaign_effect`, the l10n inventory, `flow::gate_flags`,
  `emit::declared_flags` and emission itself — plus `DW0360` about every root. The
  per-walker tests it sits beside each prove ONE walker against the roots their
  author remembered, and stay green when a root is added; the matrix cannot.

  Walkers swept (13 from the audit + 3 the guard found): `emit::check_wave_spawns`
  (already closed before the sweep), `gates::check_close_gates`, `dsl::validate`'s
  three flag-producer answers (the inline pass scan, `collect_declared_flags`,
  `produced_flags`), `camera::cutscene_units`, `rehearsal::bundles`,
  `light::reachable_time_weather`, `eclipse::walkers`, `combat::actor_beats`,
  `validate::difficulty_checks`, `daylight::fightable_actor`, `nav::actor_fights`,
  plus `continuity::excluded_npcs`, `emit::first_damage_players` and
  `emit_v04_packtests`' despawn scan.

  Two of those were also **shallow** — no nesting descent at all — which mattered
  more than the root axis on real content: `nobodys-cave-island` sets four of its
  five `set-time`s, two of three `set-weather`s and two of three `spawn-wave`s
  inside nested bundles, and produces two flags (`flag/eury-hidden`,
  `flag/antiphos-posted`) that `collect_declared_flags` could not see. That
  campaign stayed green only because it gates an *objective* on those two flags
  rather than a *trigger*, and objectives are checked against a different, deeper
  inventory. Three inventories, three answers, and which one you hit decided
  whether legitimate content compiled.

  `tools/check-effect-roots.py` is the other half: nothing in the type system stops
  a fourteenth hand-rolled walk, because the root fields are ordinary public
  fields. It fails CI when a window of source names three or more distinct roots
  outside a reasoned allowlist. It is a proximity heuristic and says so — a tripwire
  for the shape the thirteen actually had, not a proof of absence.

  **Open finding: `plan::required_anchors_for_area`.** It collects the anchors an
  area's assembly must provide from R1+R2 (and R3 only when the campaign has a
  single area), so an anchor named only in a `traps[].payload` or a dialogue
  `on_respawn` bundle is never registered as required. Left unfixed deliberately:
  unlike every other walker in the sweep this is not a mechanical widening, because
  a trap payload has no area attribution — a trap carries an `at` anchor, not an
  area — and registering its anchors in every area is the over-provisioning that
  function's own comment warns against. `DW0360`/`DW0447` still catch the resulting
  unresolved anchor at build time, so this is a worse message rather than a silent
  drop. It needs its own round, with a layout diff. Recorded in the guard's
  allowlist so it stays visible on every CI run.

  **Sound by construction, not a walker: `validate::reserved_v06_world`.** It checks
  R1–R3 for v0.6-only *fields*; R4–R7 cannot exist below v0.6 at all
  (`/content/traps` and `/content/shortcuts` are reserved wholesale, a dialogue
  `set-checkpoint` by `v06_effect()`, and `/content/on_death` is reserved to 0.10.0
  by `reserved_v10`), so widening it would report the same campaigns with a worse
  message.

  **`DW0360` vs `DW0447` overlap** (unchanged by this sweep, still open for the
  registry owner). Widening the seal to R4 put the spec-0022 payload-verb anchors
  (`volley.from_anchor`, `volley.kill_zone.anchor`, `collapse.region_anchor.anchor`)
  in its reach, and `DW0447` already owns exactly that predicate, fails just as hard
  and says more. The seal therefore scopes itself to the verbs that fail **open**
  and lets the fail-**closed** payload verbs keep `DW0447` — but only where
  `DW0447` runs. That qualifier is the finding: **there is no rule confining
  `volley`/`collapse` to `traps[].payload`**, and `plan_payload_verbs` lives inside
  the world block, so `DW0447` is unreachable for a campaign with no traps, no
  waves, no bodies and no walkable critical leg. Measured, an unconditional
  deferral there did not merely lose the better message — the typo'd anchor
  surfaced as `DW0497`, whose message tells the author the *compiler* is defective
  and which fires identically when the anchor is correct. The deferral is therefore
  conditional on `emit::assembles_world(plan)`, the same predicate the world block
  itself reads. Pinned by
  `anchor_seal::the_worldless_fixture_really_does_skip_the_payload_proof` and
  `anchor_seal::typod_volley_anchor_without_a_world_is_dw0360`.

  **Adjacent, unfixed:** a `volley` on a quest's `on_complete` in a world-less
  campaign fails the build with `DW0497` **even when its anchor is valid** — the
  call site emits `function <ns>:volley_<key>` while `plan_payload_verbs` never runs
  to emit the machinery. A genuine call-walk/machinery-walk disagreement of the
  class `DW0497` exists to catch; the fix is either to confine the payload verbs to
  `traps[].payload` at the DSL layer or to make their machinery independent of the
  world block, and both are their own round.

- **spec-0002 CLI** lists stages `1..5`, `dsl 0.1.0`, and omits `--json`/
  `--prefabs`/`--lang`; code is stages `1..6`, `dsl 0.6.0`, all three flags.
  (Spec is the original record; addenda + code are current — this doc governs.)
- **`gamerule keep_inventory true`** is emitted by the sealing baseline but is
  **not** in spec-0002's environment-sealing list (added as box-garden death
  policy; recorded here).
- **spec-0018 runtime tier, partial by design.** The static half is complete
  (completability is proven with `min_players` agents; `DW0358`) and the runtime
  half is complete for AND-joins (the generated n-dummy `party_join_<obj>`
  templates). The **critical-path bot** is still single-bot: `critical-path.json`
  and its replay describe one abstract playthrough of party state, which is
  exactly right for `min_players: 1` and remains a *sound* (if not maximal) proof
  for a bigger party — one agent can always walk what n can divide. Running
  `min_players` bots is harness work, tracked as a follow-up, not a gap in this
  layer's contract.
- **Sky attenuation constants** (`crate::light::effective_sky`, spec-0010): the
  stored sky-light baseline (15 at a sky-open cell) and the `time`/`weather` set
  commands are live-verified (1.21.11 itzg VANILLA); the per-state *effective*
  attenuation follows the documented vanilla `getSkyDarken` surface model
  (noon/day 15, night/midnight 4, rain −3, thunder −8 by day) applied
  conservatively — the effective (time-attenuated) value is not directly
  command-readable, so it is not a live measurement. `delve-admit`'s per-piece
  probe reads its two sky levels out of this same function rather than restating
  them, so a change here reaches the probe with nothing there to edit. Noted for
  maintainers.

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
flattening of the assembled block map. Shading is flat block-palette colour ×
face brightness (top brightest, bottom darkest, the two horizontal axes
distinct — the "ambient occlusion by face orientation") × a block-edge relief
darkening, then a distance fade toward the horizon. Background is a sky
gradient; for an `ocean`-horizon campaign the world generator's sea plane is
drawn analytically at `SEA_LEVEL` (a world-generation backdrop, never part of
the voxel model, never occluding a manifest target).

Three properties worth stating explicitly:

- **There is no lighting model.** The raycaster sees geometry regardless of block
  light, so a pitch-black cavern renders as legibly as a noon meadow — which is
  exactly what makes this the right tier for reviewing dark areas. A frame that
  looks fine here and black in Chunky has a *lighting* defect, not a geometry
  one, and the two tiers now separate that. Emissive blocks (glowstone, lantern,
  campfire, torch, …) still render at full brightness so a fire pit reads as one.
- **Only blocks exist.** Entities (NPC mannequins, scripted actors, item
  displays) are not in the assembled model and are not drawn; their *posts* are
  in the manifest and, with `--labels`, stamped on the frame.
- **Unknown blocks render magenta** (`255,0,255`, the same missing-texture key
  `delve-render`'s fidelity gate scans for). The palette resolves exact vanilla
  ids first, then material-family substrings (`_planks`, `_wool`, `stone`, …); a
  unit test asserts every block the shipped prefab library places has a real
  colour, so magenta in a frame means "a prefab introduced a block the palette
  has never seen" — extend the palette.

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
  "delvec": "0.1.0",
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
  editor authoring a piece-local frame had to back-solve the index and the
  transform from the rendered geometry by hand.
- **Kinds**: `anchor` · `gate` · `npc-post` · `actor-post` · `interact` ·
  `stealth-zone` · `trigger`. A point target carries `pos` (an inclusive cell);
  a region target carries `box: {min,max}` (inclusive cells) — never both.
  `gate` is the gate region, `stealth-zone` a `begin-stealth` zone box
  (`stealth-<beat>/<anchor>`), `trigger` an `EnvTrigger` — a box of its `range`
  for `approach`, the single interaction cell for `strike`/`use`.
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
byte-identical PNG **and** manifest; `crates/compiler/tests/snapshot.rs` asserts
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
{ "chart_version": 1, "campaign_id": "…", "delvec": "0.1.0",
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
  are hashed into a shipped resource pack. Moved here verbatim from
  `compiler::atmos`; resource-pack bytes are unchanged.
- `encode_rgba` (DEFLATE at a pinned level, via the existing `flate2` dep) — the
  snapshot renders, which are megapixel review artifacts.

---

## 8. Cutscene rehearsal + shot calibration (spec-0019)

LLMs are bad at authoring camera positions as `anchor + offset` numbers — three
island QA rounds shipped shots that pointed the wrong way. spec-0019 moves the
judgement into the running game: the creator adjusts a **proposal** live and
harvests it once; the DSL stays the artifact of record.

**Landed (this reference describes only what `delvec` does today):** the shot
proposal in data storage, the calibration verbs that mutate it, the `dw.done`
harvest, `delve-harvest`'s `rehearsal-report.json`, and `delvec calibrate`.
**Not yet landed:** playback — the macro-function dolly, `dw.beat` / `dw.shot`
replay, `dw.free`, and the compiler-derived state-restore inverses.

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
- A campaign with no cutscene emits **no rehearsal artifacts at all** — the
  overlay is byte-identical to its pre-spec-0019 form, and no dead trigger is
  registered.

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

The overlay also `say`-stamps a one-line `[DelveShotRoster]` the first time each
player joins, mapping shot ids to their JSON pointers; without it the creator
has no way to know what `dw.mark set 3` addresses.

**A `trigger` objective is armed by its score entry, so `scoreboard players
reset` disarms it.** Vanilla stores "this player may `/trigger` this objective"
as a lock flag on the score entry itself; deleting the entry deletes the
permission, and `scoreboard players enable` re-creates it at `0`. A tick that
both `enable`s an objective and `reset`s it therefore leaves it permanently
unusable: every `/trigger` answers *"You cannot trigger this objective yet"* to
the player and writes **nothing** to the server log — so no report, no PackTest
assertion and no amount of reading the emitted commands makes it visible. This
cost a live debugging round: a per-tick hygiene clause clearing the no-op value
(`scores={dw.mark=0}`) matched the entry `enable` had just created, so every
adjust verb was silently refused while `dw.done` — which had no such clause —
worked. **A fired trigger is cleared inside its handler, never in the tick**;
the next tick's `enable` re-arms it. Pinned by
`rehearsal::the_tick_never_resets_a_trigger_it_arms`, which fails the build's
tests if any overlay function ever again arms and disarms the same objective.

### The harvest stamp

```text
[DelveShot] shot=<n> beat=<n> ptr=<json-pointer> idx=<n> seconds=<n> look_at=<x,y,z|none> path=<x,y,z;…>
```

`say`, not `tellraw` — the same channel and the same reason as `[DelveNote]`
(spec-0006 §3): a system message to players never reaches the server stdout log
the harvester reads. `shot`/`beat`/`ptr`/`idx` are compile-time constants, so a
harvested proposal always knows which DSL node its patch belongs on; only the
live values are macro-substituted.

### `rehearsal-report.json` (`delve-harvest`)

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

---

## 9. `delvec fmt` — canonical form for authored JSON

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
it must handle an l10n sidecar, a stage document, a prefab metadata card and
whatever stage 8 turns out to be, with no per-schema list to keep in step.

### Which files, and how they are found

A path argument may be a file (taken as given — you pointed at it) or a
directory, walked recursively for `*.json` with entries **sorted**, never
`read_dir` order (ADR-0006). Two things are skipped:

- dot-directories (`.git`, `.github`);
- any directory holding a `manifest.json` — the marker `delvec build` itself
  stamps on an output root. Emitted trees are not authored content, several are
  checked in (`campaigns/*/out/`), and rewriting one would break the
  byte-identity record it exists to hold.

Symlinked directories are not followed (`campaigns/` is a symlink to the content
repo in a dev tree; a walk that followed it would silently reach a second
repository).

### The repository-wide sweep

Pointing the formatter at a path is what an author does to one document. What
this repository *gates* is the whole of what it holds, and that set is **derived,
never listed**: `tools/check-json-canonical.py` takes its population from
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
`crates/compiler/tests/golden/`, whose files are recordings of emitter output
asserted byte-for-byte by `golden_scene_matches`, with the directory's membership
closed by `every_golden_is_emitter_output`. Getting a document in there means
making the emitter actually emit its bytes — which is why it is not a hatch that
"it's generated" can open. The check refuses if that exemption matches zero
tracked files, or if either pin has been deleted.

Generated JSON is otherwise not exempt: it is made canonical **at the writer**.
`tools/gallery-baseline.py` writes `gallery/baseline/` with `ensure_ascii=False`
for exactly that reason. Where a foreign program owns the file — npm's
`package.json` and `package-lock.json`, the agent harness's `.claude/settings.json`
— it is formatted anyway and re-formatted after that program rewrites it, the
same relationship `cargo fmt` has with hand-written Rust.

Every run states files swept **against the tracked-JSON population**, plus the
exempt count. A population of zero, a swept set of zero, and an exemption
matching zero files are all refusals rather than passes.

### What formatting does and does not change in a build

Proved end-to-end by `crates/compiler/tests/fmt.rs::formatting_a_campaign_changes_only_the_manifest_input_hashes`,
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
already claimed the name "canonical" and is what **`delvec edit apply` writes
`world-edits.json` with**. Its form was serde's — struct-declaration field order.
Had `fmt` shipped a second, key-sorted form, the compiler would have written a
file its own `fmt --check` immediately rejected, and an author running both in
one loop could not have satisfied both. So `to_canonical_string` now serializes
with serde and puts the bytes through `fmt`: one definition, two doors.

The pre-existing fixture gate `crates/dsl/tests/roundtrip.rs` is unchanged by
this and now proves two things at once — serde loses no field on a round trip,
**and** the fixture on disk is in `delvec fmt` canonical form.

### CI

`python3 tools/check-json-canonical.py`, a step of the
`rust (fmt, clippy, test)` job (a step, not a job: every job name in `ci.yml` is
a required status context). It runs `--check` over every JSON document git
tracks and states its binding count against that population on every run. A
creator runs the same one command on a fresh clone; `--delvec <path>` skips the
cargo build when a binary is already to hand.

**Not yet covering the content repo.** `campaigns/` is pinned by
`versions.toml [content].sha`, so a `--check` over it here could only go green
after a content-repo normalization merges and the pin moves — an ordering this
repo cannot perform. The same one-line `--check` belongs in the content repo's
own CI, which is where its files are gated; until then, the accident this tool
exists to prevent is prevented for engine fixtures only.

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
no walk can change them. What a walk decides is the **designed** minimum — a
way class's `min_width` and `min_clearance` — which is a comfort judgement and
can never be chosen below the physical floor. And the jump arc
(`jump.airborne`, `walk.ticks-per-block`) is the derivation behind the nav
model's elevation weight, which lived in a doc comment where nothing could read
it; it is data here, and the weight is **asserted** against it rather than
computed from it, because the weight is a tuned figure an owner playtest
settled and re-deriving it at run time would move every route in every campaign
the first time somebody edited a physics fact.

**Building metrics** are standards this project fixes: the kit grid's quantum
and datum convention, the standard seam opening set, the stair pitch standards,
storey heights, the size-class ladder, **the way vocabulary**, the designed-drop
policy cap, and the pacing coefficients. Every one carries `calibrated`, and
every one is `false` — the metrics gym has not been walked. The pacing
coefficients additionally carry **no threshold anywhere**: a threshold on a
number this uncertain would be defending nothing.

**Two vocabularies classify a place, and they answer different questions.** A
`size-class.<name>` bounds a footprint on both horizontal axes and carries a
nominal traverse; a `way-class.<name>` bounds a **cross-section** —
`{min_width, max_width, min_clearance}` — and says nothing about the run,
because a route's length is per-campaign geometry and never a standard. A place
declares one or the other (`DW0875`), and `Metrics::resolve` is the one path
from either name to its entry.

`way-class.corridor` **subsumes** two entries this table used to publish alone.
`corridor.min-width` and `corridor.min-clearance` are its `min_width` and
`min_clearance` rather than keys of their own, so there is one authority for the
narrow way instead of a class beside two loose numbers nothing could spell —
which is why the metrics gym reported them unreachable before there was a way
class: no bay can instantiate an entry no document can name. `self_check`
re-asserts both inherited floors over **every** way class, not over the one that
inherited them, since a designed minimum under the physical passable size is a
standard nothing can use whether it is a road or a corridor.

A way class's `max_width` is held to the kit quantum, and its `min_width`
deliberately is not. A box's horizontal extents are multiples of the quantum
(`DW0825`), so a `max_width` off it would make the widest member of the class
uninstantiable and the gym unable to rule on it. The narrow bound is a different
matter: the corridor's inherited floor of two sits under a quantum of four, so
the narrowest way any plan can currently DRAW is four cells. Both numbers are
provisional and the walk owns both — which of them moves is the gym's judgement,
and the entry's own note asks for it.

The drop cap and the survivable fall sit beside each other on purpose. The cap
is a **policy** and is deliberately far tighter than the physics, because a drop
edge is a topology decision and should not also be a health decision; the
self-check refuses a cap that reaches the physical ceiling, since a cap there is
not a policy at all.

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
`render::occupancy` import the body constants they used to declare. Rust
enforces the property harder than a test could — a module cannot both `use` a
name and declare it — so "one definition, not two agreeing" is a compile error
rather than an assertion.

`crates/compiler/tests/metrics_standard.rs` proves the part that is left: that
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
every size class sits on the kit grid with a clearance above the passable
floor, every storey leaves interior between its courses, the drop cap is under
the survivability ceiling, and the route pace is under the pure-walk one.

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
**spine** of bays, one per rung of the size-class ladder at each of its two
bounds, chained by seams that take the widest standard opening both faces admit,
so a body walks from the smallest place the ladder allows to the largest through
every doorway it defines; and off the spine, two climbs to the same rise whose
hosts differ only in the run they afford, so the derivation picks the gentlest
standard pitch for one and the steepest for the other, and a designed fall at
exactly the drop policy's cap with a stair back out of it.

Every one of those choices reads the table through the accessor `DW0813` binds
to, which is what makes the coverage count above mean something: the generator
decides nothing a table entry already states, so *how much of the standard the
gym instantiates* and *how much of the standard the generator read* are the same
number. Deciding a host pair with a hard-coded ratio instead of the pitches'
declared runs is what `DW0840` caught during implementation.

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

It is deliberately **not** registered with
`tools/check-version-ledger-uniqueness.py`. That gate compares the *fence
anchors* two branches attach to one number, and this ledger has none — nothing
grandfathers against a metrics version and no document declares one.
