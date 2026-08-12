# Making a prefab — the procedure

The one way a prefab is produced in this project, written as steps an agent
executes. Every command below was run end to end, start to finish, on
2026-08-11 — one scene description to an admitted, rendered `.nbt`. Nothing here
is a plan, and no step names a tool that cannot be run today.

Behaviour references: [`grammar.md`](grammar.md) (what the back end does),
[`tools.md`](tools.md) (every binary and flag), [`compiler.md`](compiler.md)
(diagnostics).

## 0. Which back end

**The box-split grammar back end** (`crates/grammar`, spec-0027) — owner
decision, 2026-08-04. It is the default and this procedure is written for it.

When the scene is not a grammar scene, the route is decided by this table —
derived from the full technique survey (every approach this project researched,
probed, or rejected, with its tested evidence), not from recollection. A scene
that matches no row is **escalated, not improvised**.

| The scene is… | Route | Why this route — and what is proven about it |
|---|---|---|
| a new structural piece — a building, room, passage, stair; generic (T1) **or** a specific named referent (T2) | **grammar program** (this procedure, §1–§8) | The adopted production route for both tiers (owner, 2026-08-04). T2 is an input-modality property — the program is authored *against the referent* from the library corpus; the 2026-08-04 probe proved named referents recognizable this way, and the grammar's IR is what makes iteration converge where freehand geometry regresses. |
| a variation inside an existing hand-built tileset (another keep room in the keep's own conventions) | **grammar program**, matching the tileset's palette/conventions | The five Rust generators are maintained, not extended (§9). A generator's conventions are data to imitate, not a surface to grow. |
| a named referent that plausibly exists as a **community build** | check licence-gated ingestion first (`delve-schem` + `delve-admit`, spec-0007); fall back to grammar | Availability is luck — the corpus audit found most schematic sources unverifiable or NC-tainted, so ingestion is an opportunistic shortcut, never the plan of record. Everything ingested passes the same audit/socket/lighting admission as generated pieces. |
| organic or curved — a cave interior, a natural rock face, anything the grammar's axis-aligned boxes cannot state (§6) | **in-house generator work** (Rust: value-noise fields, 4-5-rule cellular automata, the ported craft passes) — an engine task, not an authoring step | The grammar has no curve, no noise, no terrain, by design. This route costs a worker dispatch and says so up front; it is the one row where "make a prefab" becomes "extend the engine". |
| terrain or backdrop *around* the playable scene | **surround layer** (`horizon` — ocean/void today; the spec-0026 library when it lands), never a prefab | A surround is analytically known so the proofs can read it; modelling vanilla worldgen was evaluated and rejected (the compiler cannot see server terrain without re-implementing its noise — a folklore hack). |
| an atmosphere-tier set-piece whose power is composition — scale contrast, approach framing, lighting (T3) | **no current route — escalate to the owner** | Measured, not assumed: the landmark probe showed T3's missing layer is presentation (lighting/fog/camera), not geometry. Composition is art direction (owner ruling) and is M4 design work. Attempting it through any row above produces the geometry and loses the scene. |

And the composition rules — how the routes combine into an area:

- **The one assembly chain**: piece (any route above) → `delve-admit` admission
  → pool declaration → **compiler-as-jigsaw layout** (the compiler is the
  jigsaw; sockets are a connectivity vocabulary, not runtime mechanics) →
  edit scripts for L2/L3 fixes → relight → validation renders. There is no
  second chain.
- **Grammar pieces enter areas by direct placement only, today**: the export
  emits no jigsaw connectors, so a grammar prefab cannot join a
  `prefab_pool` until a socket is carved (§7) — and connector *emission* is an
  open design, flagged, not improvised.
- **Intent flows one way**: a reference image (`tools/refimg.py`) conditions
  the human and the program; a similarity score (`tools/refscore.py`) RANKS
  candidate expansions on the contact sheet and can never gate or veto —
  structurally enforced (`DW0725`/`DW0726`). Generate N seed-varied
  candidates, machine-order, **human picks one**. No image is ever
  voxelized into geometry (red line).
- **Fixes go through edit scripts, not regeneration**, once a piece is placed:
  the edit script is versioned, deterministic, and re-proves walkability and
  lighting after replay; hand-editing world files is not an authoring surface.

## 1. Fix the scene description first

One or two sentences, written down before any tool runs, naming: what a body
does in the space (walks through / drops into / fights in / is watched from),
the material feeling, and any element the campaign will need to attach to.

It is written first so that §5 has something to judge against. A description
invented after the render is a description of the render.

**Fix the region in the same breath, and check it against the 48-block
template cap then** — not after the program is authored. A scene whose route
needs more than 48 blocks on any axis cannot ship as one prefab (§6), and
discovering that at export costs the whole authoring loop: five of the eight
drowned-bell zone programs were designed at zone scale (60–145 blocks of
travel) and hit the cap only when first exported. Record the chosen region and
seed beside the program — in the campaign's `GENERATION.md`, since the program
JSON does not yet carry its own region (queued engine surface).

## 2. Choose the palette by MEASUREMENT

**Never name a block from memory.** Block names are not descriptions of block
appearance and repeatedly are not close: `packed_mud` is orange (142, 107, 80),
`lightning_rod` is signal orange (197, 111, 83), `dried_kelp_block` is a woven
olive-green (46, 55, 36).

```sh
python3 tools/block-appearance.py --near '#3a4038' -n 10 --full-cube-only
python3 tools/block-appearance.py --id minecraft:packed_mud     # check one
```

Rules:

- Pick the target colour from the fiction, then take candidates from the ranked
  list. Record the measured hex beside each role in the program.
- `--full-cube-only` for anything structural: a wall made of a block whose model
  is mostly air is not a wall.
- The tool ranks; it cannot choose. A mean colour cannot see pattern or scale,
  and it has no idea what a block *is* — it will rank `structure_block` next to
  deepslate. Technical blocks are excluded by default; everything else you
  **see** at step 5 before believing.
- Biome-tinted blocks (`*_leaves`, grass, water) are flagged: their number is
  the untinted texture and the world will not look like it.

## 3. Author the program as JSON

Start from the corpus, never from the schema:

```sh
delve-grammar list                                # what exists
delve-grammar show --program store-room > my-piece.json
```

The library is a few-shot corpus this project legally owns (spec-0027 §2).
Editing the nearest rule is what made the worked example pass its first check;
writing the IR from its documentation is the slower path.

Then edit. The IR surface is `grammar.md` §2. Two things worth knowing before
you write:

- **Two guards that can both hold are a probability, not a priority.** A
  decision needs mutually exclusive guards.
- **`rounding` matters wherever a piece is load-bearing.** The default truncates
  and never writes the remainder, and an unwritten cell is air — a floor with a
  hole at the far end. Use `"rounding": "start"` on anything a body stands on.

```sh
delve-grammar check --file my-piece.json          # structure only; fast
```

`check` finds unknown rules, unknown roles, split/child mismatches and
unmatchable guards without a region or a seed. Run it after every edit.

## 4. Expand, and let the machine judge

```sh
delve-grammar expand --file my-piece.json --region 9x6x21 --seed 1 \
    --traversable -o out/
```

Writes `<id>.nbt`, `<id>.json` (prefab metadata) and `<id>.report.json`.

**Gates** (a red gate writes no `.nbt`; exit 4):

| Gate | Claim |
|---|---|
| `blocks-exist` | every block state the model paints exists in 1.21.11, properties and values included |
| `non-empty` | the expansion built something |
| `traversable` (`--traversable`) | a body can walk from the approach end to the exit end; add `--allow-falls` for a piece entered by stepping off a ledge |

`--traversable` is opt-in because it is a claim about a *kind* of piece: a room
with one door has no far end and would fail it correctly and uselessly. **Pass
it whenever the piece is a passage, a stair or a route** — that is most of them,
and a route nobody proved walkable is the defect the gate exists for.

Every gate reports a **binding count**. A gate that examined zero objects is
printed as a finding, not folded into the pass; so is a program that declared no
anchors. Read the findings.

**Measurements** (numbers, no verdict — deliberately not dressed as gates): fill
ratio, distinct states, standable cells, footprint area and perimeter,
silhouette complexity (1.00 is a plain box), and the five commonest blocks with
their shares. Use the shares to see monoculture; the craft gates that would
*fail* on it are not built (see §6).

If the region is wrong the tool refuses. A refusal is the correct outcome — a
region too small never yields a smaller building. Two refusal shapes today:
a **sized** rule that does not fit names itself and its requirement; a
**guard-exhaustion** refusal (`no alternative of rule "…" applies`) names only
the rule — the guards it tried are read from the program, not from the error.
Making that refusal print each alternative's failed inequality with its
evaluated operands is queued engine work; until it lands, budget one
read-the-rule round-trip per guard refusal.

## 5. See it before believing it

```sh
delve-render piece out/<id>.nbt -o shots/ --size 640
```

Four exterior three-quarters, a plan cutaway, one shot per socket, and one from
**each declared anchor's own eye height** — which is the shot that shows whether
an anchor is looking at the thing it is about.

Compare against the description from §1. The gates prove the piece is buildable
and walkable; they say nothing about whether it is the scene that was asked for.

## 6. What the grammar cannot do — escalate, do not improvise

Each of these was established by running it, except the two marked otherwise:

- **No block entities and no NBT.** No chest loot, no sign text, no spawner, no
  banner. Anything with a payload is bound by the *campaign* against an anchor
  (spec-0021), never by the piece. So the piece must **declare an anchor** for
  every such thing.
- **No jigsaw connectors.** The export emits none. A grammar prefab is usable as
  a single-`prefab` area as it stands; for a `prefab_pool` a socket is carved
  afterwards (§7).
- **No light.** The export declares `unmeasured` and it means it. §7 probes.
- **48 blocks per axis** — the vanilla structure cap. Bigger is several socketed
  pieces, which is a jigsaw design decision, not a parameter.
- **Axis-aligned boxes only** — no curve, no diagonal, no mirror (an orientation
  is a permutation without reflection). A round tower or an organic cave wall is
  not this back end's shape. *Read from `crates/grammar/src/orient.rs`, not
  probed.*
- **No terrain** — no noise, no heightfield; height variation comes from splits
  and recursion. *Same source.*
- **No craft gate.** spec-0027 §4's palette-role budget, gradient and depth rules
  are still not built, and what blocks them is named in
  `crates/grammar/src/gates.rs`: the budget is defined per *material family* and
  nothing here can decide what family a block is in. Until it exists, monoculture
  and flatness are caught by looking (§5), not by the machine.

## 7. Admit it

```sh
delve-admit audit    out/<id>.nbt
delve-admit socket   out/<id>.nbt --pos X,Y,Z --facing <dir> --opening 3,3 \
                     --name <ns>:<name> --target <ns>:<name> --pool pool/<name>
delve-admit lighting out/<id>.nbt --write
delve-admit audit    out/<id>.nbt          # again, after the edits
```

`audit` is the gate that runs on the bytes rather than on the expansion:
hard-forbidden blocks (`DW0731`), blocks the pinned version does not have
(`DW0733`), and the palette allowlist (`DW0730`). A grammar prefab passes it
by construction for `DW0733` — the export already refused — but a *hand-built*
or ingested piece does not, so `audit` is where that class is caught for
everything else.

`lighting --write` is a **static** estimate, not a live probe; it says so in the
metadata it writes. A piece it calls `dark` is dark because the program placed
no light — the grammar cannot warn you, so this step is where you find out.

## 8. Where the files go

Generated `.nbt` + metadata live in the **content repo**
(`campaigns/prefabs/`), never in this one (ADR-0007). The grammar **program**
is the artifact of record and lives beside the campaign that uses it; the
`.nbt` is a snapshot of one expansion of it, and its metadata carries the
program hash and seed that regenerate those exact bytes (ADR-0006 — verified:
same inputs twice gives byte-identical `.nbt` and metadata).

## 9. Hand-written Rust generators

`prefabs/*-generator` are five standalone Cargo workspaces that predate the
grammar back end. They are maintained, not extended: a new piece is a grammar
program. Running one is `cargo run --release --manifest-path
prefabs/<gen>/Cargo.toml -- campaigns/prefabs/`, and every piece it emits goes
through `prefabs/invariants.rs` — including the block-registry check, so the
`DW0733` class is refused at that emitter too.
