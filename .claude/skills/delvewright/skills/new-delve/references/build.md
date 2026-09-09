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
delvec --prefabs "$DELVEWRIGHT_PREFABS" build campaigns/<id> \
    -o "$DELVEWRIGHT_ENGINE/validation/delve-output"
```

Must exit 0. `$DELVEWRIGHT_ENGINE/validation/delve-output` is the
**zero-configuration** tree: every later step on this page boots it with no
variables set, which is why the command above names it.

**It is not the only place the tree may live.** `packtest-run.sh`,
`bot-run.sh`, `branch-runs.sh` and the play server all boot a tree anywhere,
including one inside your own working directory.

**If you put the tree in your own repository, TWO variables travel together.**
`dockerfile` is resolved relative to the build CONTEXT, so moving the context
without moving the dockerfile gives `failed to read dockerfile`, which reads as
a broken harness and is not one — it is the tree being somewhere the dockerfile
is not. The entry scripts export both for you; a hand-written
`docker compose … --profile play` does not:

```sh
export DELVE_OUTPUT="$PWD/.out/delve"                       # absolute
export DELVE_DOCKERFILE="$DELVEWRIGHT_ENGINE/validation/Dockerfile.delve"
delvec --prefabs "$DELVEWRIGHT_PREFABS" build campaigns/<id> -o "$DELVE_OUTPUT"
```

Compose cannot compute an absolute default, so this is a real pair and not
something a better default removes. Set neither and everything above is
unchanged.

The build writes more than a datapack. Four things to read, and **the last two
stand on site-plan campaigns only** — an `areas[]` build emits neither, and
their absence there is the placement model, not a run that went wrong:

- `critical-path.json`, at the **root** of the output — the playthrough the
  proof found, step by step. **Every campaign.** A step that crosses areas
  carries a `transport` key; step 2A is about what puts it there.
- `render-plan.json` — the deterministic shot list, each shot with the `expect`
  line step 12 checks it against. **Every campaign.**
- **Site-plan campaigns**: the three hashes and the engine revision it prints
  at the end. Step 13 copies them.
- **Site-plan campaigns**: `DW0822`, the pacing line — it stands beside
  `DW0813` on every site-plan build and on no other. It measures the critical
  path over the built
  blockout — so many blocks of route, and about so many minutes at the
  metrics table's blocks-per-minute rate — and it carries **no threshold and
  refuses nothing**, so nothing in the engine compares it to your
  `target_minutes`. **You do that.** A map that measures twelve minutes
  against a `target_minutes` of 150 is not a warning anyone will raise; it is
  a map whose walking is a twelfth of its billing, and the gap is either
  content you have not written yet or a design that is smaller than it says.
  **On an `areas[]` campaign there is no pacing measurement at all**, so
  `target_minutes` there is your own estimate and nothing reads it — do not go
  looking for this line, and do not read its absence as a build that skipped
  something.

**There is no blockout document and nothing to author early.** A site-plan
campaign's geometry is derived from the plan and the metrics table by this
command, which then runs the battery over the bytes it laid: every seam built
where it was allocated (`DW0836`), every place reached from the entry
(`DW0837`), and no crossing between places anywhere a seam was not allocated
(`DW0838`).

**`--perturb <knob>` asks the derivation for a named defect and shows you the
observer catching it** — `slide-openings`, `sink`, `short-walls`, `brick-up`,
`low-ceiling`, `wall-contacts`, one per run, each printing which code it expects
(`sink`, `brick-up` and `low-ceiling` also take `--perturb-place`, and it is
refused for the others). It writes nothing — `--out` is refused beside it and the
exit is always non-zero — so a perturbed tree does not exist to be shipped,
walked or admitted. Reach for it when a battery has been green on this campaign
from the first build and you want to know it is looking at the bytes rather than
replaying the arithmetic that laid them.
