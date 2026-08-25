# Friction log — walking the prefab procedure as a person

Written live, as it happened. Instrument: engine worktree at
`dd287282723cfd34bec87ca181afae4ab55ce4f0`, branch
`docs/the-prefab-procedure-friction-log`.

Stance: a person who has this repository, `.claude/skills/new-delve/SKILL.md`,
`docs/reference/prefab-procedure.md`, and nothing else. Subject: the smallest
plausible missing piece.

(entries appended below as they happen)

---
## Init (SKILL.md "Init", steps 1–2 and the finish checklist)

Ran every line of the Init checklist. All of it worked, first try, as written:
the symlink check, `cargo build --release --workspace` (exit 0, 38s),
`cargo build --release --manifest-path crates/render/Cargo.toml` (exit 0, 1m17s),
the `export PATH` line, `delvec --version` (`delvec 1.1.0, dsl 0.18.0, mc
1.21.11`), `delve-render fidelity-gate` (`PASSED`, exit 0), `delve-grammar list`
(36 programs), `command -v delve-admit`, and the client-jar ladder via
`delvec palette campaigns/prefabs/hello-room.nbt`.

**No friction.** Recorded because it is the control: the pages were followed
literally and the toolchain came up. Everything below happens on a working
install, so nothing below is an install problem.

One triviality, logged and not a finding: `delve-grammar list | head -50`
ends in

```
thread 'main' (22702219) panicked at .../stdio.rs:1166:9:
failed printing to stdout: Broken pipe (os error 32)
```

That is Rust's SIGPIPE default, and `list` still exits 0. A person paging a
36-entry list would see a panic trace and might think the tool crashed.
Severity: **mild friction**.

---

## §1 — Fix the scene description first

**What the page says** (`prefab-procedure.md` §1):

> One or two sentences, written down before any tool runs, naming: what a body
> does in the space (walks through / drops into / fights in / is watched from),
> the material feeling, and any element the campaign will need to attach to.

and

> **Fix the region in the same breath** … Record the chosen region and seed
> beside the program — in the campaign's `GENERATION.md`, since the program JSON
> does not yet carry its own region (queued engine surface).

**What I did.** Picked the dullest plausibly-absent piece. `delve-grammar list`
has `stair-flight`, `store-room`, `tee-passage`, `threshold-motif` — but no
corner. Subject:

> A plain corridor that turns one right angle. A body walks in at the north end,
> turns once, and walks out of the west face. Dry-laid stone, undressed, plain
> floor and plain ceiling, no light. The campaign attaches nothing but the two
> ends.

Region fixed in the same breath: **9×5×9** — a 3-wide walkway with a course of
wall each side, two legs of 9, headroom 3 under a 1-course ceiling on a 1-course
floor.

**Friction: one, small.** The page says to record the region and seed "in the
campaign's `GENERATION.md`". This whole section of the skill is titled *"when
the library has no piece you need"* — the piece may well be a **library** piece
with no campaign yet, and then there is no `GENERATION.md` to write into and the
page names no other home. A person would either invent a file or skip the
record; skipping is silent, and the region is exactly the fact the program JSON
does not carry.

*Would have unstuck them:* one clause naming where the record goes when the
piece is not a campaign zone.
Severity: **mild friction**.

---
## §2 — Choose the palette by measurement

**What the page says** (§2): three steps, "Do not stop after the first" —
screen, measure the mix, then look at the sheet.

**What happened.** All three ran first try and the page's own worked example
reproduced exactly:

```
$ python3 tools/block-appearance.py --screen --where full_cube --where 'L>=0.75' \
      --where 'L<=0.95' --where 'C_mean<0.02' --where 'texture_range<=0.30'
all candidate blocks   1146
...
14 survivor(s).
```

The page claims "The example above takes 1146 blocks to 14." It does.

My own screen (`L>=0.35 … L<=0.62`, neutral, quiet, `not gravity`) → 48
survivors. Mix measured:

```
$ python3 tools/block-appearance.py --mix 'cobblestone=6,stone=3,cobbled_deepslate=1'
binding: 1 of 1 declared paint(s) examined, 1 mix(es) with >= 2 members
   60.0%  minecraft:cobblestone …
   10.0%  minecraft:cobbled_deepslate …
  loudest_member minecraft:cobbled_deepslate at 10% of area
```

`--sheet --seed 7` wrote `.sheets/palette/swatches.png`; I opened it and the mix
tile reads as damp dry-laid cobble, which is the scene. Under a second, no GPU.

**No friction at all.** This is the strongest stretch of the procedure walked in
this round: the instructions are executable verbatim, the numbers in the prose
match the numbers the tool prints, and the "now LOOK at it" line is printed by
the tool itself rather than left to the page.

---
## §3 — Author the program as JSON

**What the page says** (§3): read the idiom index first (`grammar.md` §2c), then
`delve-grammar show --program <nearest> > my-piece.json`, edit, and
`delve-grammar check --file my-piece.json` after every edit.

**What happened.** `delve-grammar show --program tee-passage` printed a 365-line
runnable program. I read `grammar.md` §2b (`mark`), §2c (idioms + the split-order
section) and §2 (the paint / weighted-list form) — all three named by the page —
and wrote a four-rule program: one `z` split into approach band / turn band /
solid, each band an `x` split, one shared `lane_column` doing the `y` split.

```
$ delve-grammar check --file corner-passage.json
corner-passage: ok — 4 rule(s), 2 param(s), 1 role(s). Structure only: expand it
to learn whether it fits a region.
```

Green first try. **No friction.** The split-order section of §2c
("the last axis you split is the only axis on which two things are guaranteed to
meet") is the single most useful paragraph in the whole reference for a person
writing a first program, and it is where it needs to be.

One honest note about my own conduct: my two legs' lanes line up only because
`[relative 1, absolute 3, relative 1]` and `[relative 1, absolute 3]` both put
the lane at x 3..5 **at this region**. That is a computed constant, which §2c
says is exactly the thing nothing checks. The page warned me and I did it anyway
because it is the obvious way to write an L. Worth knowing that the warning does
not prevent the mistake it names.

---

## §4 — Expand, and let the machine judge — **THE STOP**

**What the page says** (§4, and SKILL.md step 4 in the same words):

> `--traversable` is opt-in because it is a claim about a *kind* of piece … **Pass
> it whenever the piece is a passage, a stair or a route** — that is most of them,
> and a route nobody proved walkable is the defect the gate exists for.

My piece is a passage. I passed it, with `--reachable-floor` as SKILL.md's own
command line shows.

**What happened** — command and output, verbatim:

```
$ delve-grammar expand --file corner-passage.json --region 9x5x9 --seed 1 \
      --traversable --reachable-floor --id corner-passage -o out/
corner-passage: fail
  blocks-exist    pass  bound 4 …
  shape-complete  pass  bound 4 …
  states-complete pass  bound 4 …
  oriented-fills  pass  bound 8 …
  non-empty       pass  bound 405    324 filled cell(s) of 405 in the region
  traversable     FAIL  bound 3      0 standable cell(s) at the approach end, 3 at
                  the exit end; walking does NOT connect them. This piece declares
                  no spatial contract, so the binding count is standable CELLS on
                  two faces of the region, not declared ways in — declare exterior
                  edges and it counts doors
  reachable-floor pass  bound 27     27 standable cell(s) under a roof; 0 of them
                  have no walking route from the 6 grade entry cell(s)
  …
  reachability   27 of 27 standable cell(s) reachable on foot from 6 grade entry
                 cell(s) (100.0%) · 27 sheltered · unreachable 0 …
  finding: this piece declares no spatial contract …
error: corner-passage: a machine gate went red; no prefab was written.
expand_exit=4
```

**Read those two lines together.** `reachability` says **100% of the floor is
reachable from 6 grade entry cells** — my corridor is completely, provably
walkable end to end, and the always-on measurement says so in the same report.
`traversable` says it is not, and a red gate writes **no `.nbt`**. Exit 4,
nothing on disk.

The reason is that `traversable` is *a claim about two opposite faces of the
region box*. My two ends are on **perpendicular** faces — the north face and the
west face — because the piece turns a corner. That is what a corner passage IS.
The gate picked an axis, found the far face solid, and failed.

**What a person concludes.** Not "my piece is wrong" — the report told them the
floor is 100% reachable one line below. They conclude **the engine is broken**,
or that they have misunderstood what a passage is. And the page gave them no
warning: §4 lists exactly one piece to leave a flag off for ("The one piece to
leave it off: a one-way descent", for `--reachable-floor`), which reads as an
exhaustive list of the exceptions. There is no such note for `--traversable`,
and the instruction is the unqualified "**pass it whenever the piece is a
passage**".

**Severity: stops her dead.** Exit 4, no `.nbt` written, on the smallest and
dullest piece anyone would ever make, following the page literally.

**What would have unstuck them:** one sentence in §4 saying `traversable` is a
claim about two *opposite* faces, so a piece whose ends are on perpendicular
faces cannot satisfy it without a spatial contract. That sentence does not exist
on either page.

*(Continued below: I follow the diagnostic's own prescription rather than
skipping to what I know.)*

---
### §4 continued — following the diagnostic's prescription literally

The `traversable` failure prescribes, in its own words:

> declare exterior edges and it counts doors

So I went and declared exterior edges. Two things about that instruction:

**(a) Neither page tells you how, or that this is a thing you do.**
`prefab-procedure.md` §3 — "Author the program as JSON", the step where you would
write one — never mentions `claim`, `contract`, or the spatial contract at all.
The contract appears on that page only at §7 (as something `delve-admit audit`
checks) and §9 (as a metadata field a *reader* consumes). SKILL.md's whole
"when the library has no piece you need" section never mentions it either. The
only authoring documentation is `grammar.md` §2d, which the procedure page names
once, at the top, as a general "behaviour reference".

A person following the two pages has, at this point, a red gate, a prescription
they cannot act on, and no pointer. **Severity: stops her dead** — this is the
same wall as the stop above, and it is the wall she is standing at *after* she
does what the message said.

**(b) "declare exterior edges" is not a small thing.** `grammar.md` §2d:

> It runs whenever a piece declares a contract; there is no flag.

Declaring one edge turns on **nine** gates — well-formed, coverage, closure,
edge-proof, no-body, reachability, anchors, exterior-faces, no-body-majority —
each with its own obligations (every standable cell in a declared element, every
boundary cell of an `enclosed` space non-passable except a claimed opening, every
anchor resolving, and so on). The prescription reads like "add two lines"; what
it actually asks is "adopt the spatial-contract subsystem".

**What I wrote.** One `claim` node on the shared `lane_column`'s void child, plus:

```json
"contract": {
  "entry": "passage",
  "spaces": { "passage": { "envelope": "enclosed" } },
  "edges": [ { "a": "exterior", "b": "passage", "class": "walk" },
             { "a": "passage",  "b": "exterior", "class": "walk" } ]
}
```

**It passed, first try, all nine gates green:**

```
corner-passage: pass
  traversable     pass  bound 4  4 declared way(s) in or out — west walk, west
                  walk, north walk, north walk — and a walk connects every pair
  contract-well-formed pass  bound 3
  contract-coverage    pass  bound 27
  contract-closure     pass  bound 123
  contract-reachability pass bound 27
  contract-anchors      pass bound 2
  contract-exterior-faces pass bound 2   2 exterior edge(s) export 4 face(s)
  contract-no-body-majority pass bound 27
out//corner-passage.nbt + corner-passage.json + corner-passage.report.json
expand_exit=0
```

That is a genuinely good outcome once you know to do it. **The whole cost of
this stop is documentation, not engine.** Two sentences in §3 — "a piece whose
ends are not on opposite faces declares a spatial contract; see `grammar.md` §2d"
— would have removed it entirely.

### A defect found on the way: two exterior edges export duplicate faces

Not what I was looking for; found because the report printed its list.

`grammar.md` §2d's own worked example declares one exterior edge per door:

```json
{ "a": "exterior", "b": "near", "class": "walk" },
…
{ "a": "far", "b": "exterior", "class": "walk" }
```

I copied that shape — two exterior edges, one per end. The report:

```
contract-exterior-faces pass  bound 2   2 exterior edge(s) export 4 face(s):
                                        west walk, west walk, north walk, north walk
```

Each edge exported **both** faces, so the shipped `<id>.json` carries four
entries in `spatial_contract.faces`, in two byte-identical pairs:

```json
{ "space": "passage", "class": "walk", "dir": "west",
  "opening": { "from": [0,1,3], "to": [0,3,5] } },
{ "space": "passage", "class": "walk", "dir": "west",
  "opening": { "from": [0,1,3], "to": [0,3,5] } },   <- identical
```

Cut back to **one** exterior edge (probe, same region and seed): also `pass`,
`2 face(s): west walk, north walk`, no duplicates.

So the edge declaration is not the authority on which face — the checker derives
faces from the geometry — and the edge *count* multiplies the export. One edge
and two edges are both green and produce different artifacts.
`docs/reference/prefab-procedure.md` §9 names `spatial_contract.faces` as
something `delvec` consumes.

**What a person concludes:** nothing. Every gate is green, so they ship it. This
is the silent-wrong-artifact direction.
*Would have unstuck them:* nothing on a page — this one wants a gate, or the
declaration wanting a `face`/`dir` so an author says which end they mean.
**Severity: costs an hour** — later, to whoever finds it downstream.

---
