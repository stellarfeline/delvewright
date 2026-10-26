# Steps 6-8 — fmt, analyze, build

## Contents

- [`delvec fmt`](#delvec-fmt)
- [`delvec analyze`](#delvec-analyze)
- [`delvec build`](#delvec-build)

## `delvec fmt`


**Mandatory, for every campaign, whether or not it has a second language**, and
again after every later fix including every playtest-round repair.

```sh
delvec fmt campaigns/<id>
```

It rewrites every document and l10n sidecar in canonical form: object keys
sorted, two-space indent, non-ASCII raw, one trailing newline. It exists because
a three-key insertion into a non-canonical file produces a hundred-line diff
that nobody can review. **Array order is semantic and it never touches it**
(`quests[]`, `objectives[]`, `effects[]` are ordered), and it proves that on
every file it writes — so running it is never a risk to the campaign. It states
its binding count: `examined N file(s); reformatted M, 0 unparseable`.

Exit 1 means something is wrong with the JSON itself, not with its layout:
`DW0770` unparseable (it prints `line:col`), `DW0771` a duplicate object key —
which means one of the two values is already being silently discarded, so fix
the document rather than the formatter. Never hand-sort a file, and never "fix"
a `DW0773` by editing: re-run `fmt`. CI runs `delvec fmt --check`, so a campaign
that skips this reds there instead.

## `delvec analyze`


```sh
delvec --prefabs "$DELVEWRIGHT_PREFABS" analyze campaigns/<id>
```

Quest-graph reachability, deadlock, dark-room mitigation. Fix findings in the
documents — never by weakening the campaign. A dead quest is a design bug.

## `delvec build`


```sh
delvec --prefabs "$DELVEWRIGHT_PREFABS" build campaigns/<id> -o "$DELVE_OUTPUT"
```

Must exit 0. **`$DELVE_OUTPUT` is this session's build tree**, `.out/delve`
under the working directory, and every later step boots it. I4 writes it into
`.out/env.sh` together with `DELVE_DOCKERFILE`, the engine's
`validation/Dockerfile.delve`, because the two travel together: `dockerfile` is
resolved relative to the build CONTEXT, so a tree moved without its dockerfile
gives `failed to read dockerfile`, which reads as a broken harness and is not
one. `packtest-run.sh`, `bot-run.sh`, `branch-runs.sh`, `world-save.sh`, the
play server and a hand-written `docker compose … --profile play` all read the
pair; each working directory builds into its own tree, so two sessions on one
machine never boot each other's build.

The build writes more than a datapack. Four things to read, on every build:

- `critical-path.json`, at the **root** of the output — the playthrough the
  proof found, step by step.
- `render-plan.json` — the deterministic shot list, each shot with the `expect`
  line step 12 checks it against.
- `DW0822`, the pacing line — it stands beside `DW0813` on every build. It
  measures the critical path over the built blockout — so many blocks of
  route, and about so many minutes at the metrics table's blocks-per-minute
  rate — and it carries **no threshold and refuses nothing**, so nothing in
  the engine compares it to your `target_minutes`. **You do that.** A map that
  measures twelve minutes against a `target_minutes` of 150 is not a warning
  anyone will raise; it is a map whose walking is a twelfth of its billing, and
  the gap is either content you have not written yet or a design that is
  smaller than it says.

**Every campaign: no place a player can get into and not out of.**
The build moves a body the way a player does — walks, falls off edges, jumps
gaps, wades or drops into water and floats — from every cell of the proven
route, and refuses any cell from which no movement leads back. A garden bed
ringed by a hedge the player can jump onto, a pool whose rim stands two over
the water, a slot in a wall walk: each is named with the move that gets a body
in. A floor is judged by what holds a body, not by what holds a block: iron
bars or a pane laid through a floor are posts a body falls between, so a hatch
built of them is named here with the fall through it — build a floor's bar of
the block the allocation hands as `bar_block`. Repair the place — open a side,
lower the rim, add a step, move what the player jumps in from — and never
answer it with an invisible barrier. A room
the story shuts the party into is not this: its objective is inside it. The
build prints the leave-proof binding line either way; the counts are in
`validation/leave-proof.json`.

**Every campaign: every block the build writes is one the server keeps.** A
block that needs a neighbour to stay — a wall torch, a lever, a ladder, a
lantern, a flower, a carpet, a door's upper half — is judged by the pinned
server's own measured rule, over the world at load and over every
configuration the critical path passes. One that no neighbour holds is
`DW1002`, naming the block, its cell, the piece whose box holds it, and what
each neighbour would have to give. Repair it where the block is authored — in
the piece, the edit or the fill: give it the neighbour it asks for, hang it on a
face that holds it, or choose a block that needs nothing there. The build prints
`attached blocks (DW1002): …` either way, and a block the rule cannot judge is
counted there, never refused. Rule:
`$DELVEWRIGHT_ENGINE/docs/reference/delvec/compiler/attached.md`.

**Three binding lines name what step 10 will assert**, zeroes included:
`sculk walk: P of S sensor(s) predicted …` (a sculk device no leg predicts is
named, and is never asserted), `pulse binding: … H of N with a listening
station` (a pulse with no station is never heard by the ladder), and `watch
binding: … D drawable, R refused, U unobservable`. Read them against what you
declared before you run the ladder.

**There is no blockout document and nothing to author early.** A site-plan
campaign's geometry is derived from the plan and the metrics table by this
command, which then runs the battery over the bytes it laid: every seam built
where it was allocated (`DW0836`), every place reached from the entry
(`DW0837`), no crossing between places anywhere a seam was not allocated
(`DW0838`), every doorway leading a body through once its bar is open
(`DW0986`), and every plot stitched to the ground the whole gave it — no crack
along a plot's edge, no aloft place standing in the earth (`DW0990`).
A stair whose run no standard pitch fits is `DW0830` as a warning: the
stand-in lays no treads, and the place above stays unreached (`DW0837`) until
the host is detailed with its own stair at step 9. **A stand-in is not judged
for light**: nobody has placed its light yet, so the whole's `DW0210` gate
leaves out every cell inside a massed place's frame and says how many on its
binding line (`… E cell(s) under M stand-in(s) not judged`). A place's light is
judged when it is detailed, by its own piece's probe and then by the whole's
gate, so step 8 exits 0 over unlit stand-ins and the battery, the exposure gate
and the leave proof report here rather than places later.

**`--perturb <knob>` asks the derivation for a named defect and shows you the
observer catching it** — `slide-openings`, `sink`, `short-walls`, `brick-up`,
`low-ceiling`, `wall-contacts`, `open-stairwells`, `bury-barred`, `hollow-edge`, one per run, each printing which code it expects
(`sink`, `brick-up` and `low-ceiling` also take `--perturb-place`, and it is
refused for the others). It writes nothing — `--out` is refused beside it and the
exit is always non-zero — so a perturbed tree does not exist to be shipped,
walked or admitted. Reach for it when a battery has been green on this campaign
from the first build and you want to know it is looking at the bytes rather than
replaying the arithmetic that laid them.
