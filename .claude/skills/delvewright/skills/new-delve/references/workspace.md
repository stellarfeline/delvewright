# Step 1 — the workspace and its documents

## Contents

- [Where a campaign lives](#where-a-campaign-lives)
- [The document names](#the-document-names)
- [The envelope, and the number in it](#the-envelope-and-the-number-in-it)
- [Getting a document's shape](#getting-a-documents-shape)
- [`DESIGN.md` and `GENERATION.md`](#designmd-and-generationmd)
- [The campaign's own branch](#the-campaigns-own-branch)
- [Stubbing what is not written yet](#stubbing-what-is-not-written-yet)
- [`world.json`, written here](#worldjson-written-here)
- [The hour the delve is played at](#the-hour-the-delve-is-played-at)
- [The optional fields that commit something](#the-optional-fields-that-commit-something)


Create `campaigns/<campaign-id>/`. Everything of the campaign lives
there — the documents, the design record, the generation record, the storybook.
Build output goes beside them and is ignored by git there.

## Where a campaign lives

**`campaigns/` is the only place a campaign lives, and a demo level is a
campaign.** A level off the engine's `docs/demo-levels.md` queue — one mechanic
in the spotlight, ten to twenty minutes, minimum cast — is authored through this
page like any other and goes to `campaigns/<id>/`. It has to: a delve is what
somebody plays, and a campaign is the directory under `campaigns/` that carries
`world.json`. **`demos/` is not a second home for one.** What lives there is a
demonstration of a *generation-time* surface — a grammar program, the piece it
exports, its reports, its refusal transcripts — and it carries no campaign
document because no delve is built from it. One place each, and no pointer file
in either.

## The document names

**The document names, all of them.** `delvec validate` reads a whole campaign
and refuses one that is missing any of them with `DW0874`, which names **every**
document that is absent in a single run, gives the whole required set, and
prints what a stub looks like — so you never have to guess the names. Six are
always required:

```
world.json  npcs.json  classes.json  quest-plan.json  quests.json  dialogue.json
```

Six more are conditional, and every one of them is a real campaign document
with its own schema:

| document | when |
|---|---|
| `geometry-brief.json` · `layout-graph.json` · `site-plan.json` | a site-plan campaign — step 2B; a site-plan campaign has no `areas[]` |
| `design.json` | step 4, the moment the user approves the reference images — one row per approved picture, and the only home the approved sky has |
| `detail-plan.json` | step 13, optional, and only after the blockout has been walked |
| `world-edits.json` | whenever the map editor was used to fix terrain — see *Reference: tools by symptom* |

## The envelope, and the number in it

**Every document has the same envelope**, and it is four keys:

```json
{
  "dsl_version": "0.22.0",
  "campaign_id": "the-weighbridge",
  "stage": "world",
  "content": { }
}
```

`stage` is the document's own name — `world`, `npcs`, `classes`, `quest-plan`,
`quests`, `dialogue`, `world-edits`, `geometry-brief`, `layout-graph`,
`site-plan`, `detail-plan`. `content` is everything else.

**What number goes in `dsl_version`: the one `delvec --version` printed after
`dsl`.** A new campaign writes the engine's current number on every document.
The per-feature minimums this page states elsewhere ("needs `dsl_version`
0.10.0 on the quests stage") are the *floor* a surface became available at —
they exist so an old campaign keeps compiling unchanged, and a new campaign is
already above all of them. Write the current number and none of those sentences
applies to you.

## Getting a document's shape

**Get the shape from the engine, never from memory.** Before writing a
document:

```sh
delvec schema --stage world          # or npcs, quests, site-plan, walk-record, …
delvec schema --stage all            # every document at once
```

The schema is the authority on a document's form. Where anything else disagrees
with it, the schema is what parses. If you want a filled-in example rather than
a schema, `delvec metrics --gym .out/gym` writes nine complete documents that
build (see *Which placement model*).

## `DESIGN.md` and `GENERATION.md`

Two more files belong to the campaign and are prose, not documents the compiler
reads:

- **`DESIGN.md`** — the authoritative design record: layout, dramaturgy beats,
  the branch/ending table. Every later round is judged against it.
- **`GENERATION.md`** — the campaign's own decisions, in your words as their
  author: what the brief pinned down and what you invented, the `dsl_version`,
  this campaign's **posture note** (see *Reference: writing craft* §B), and later
  the findings ledger and the chronicle citation table. **Not the prompt
  verbatim and not a date.** Every artifact of this campaign is
  English-first and carries no personal information, no verbatim personal
  speech, and no record of who decided something or when — the engine
  constitution's Conventions name these generation logs explicitly. So write the
  constraint ("the brief pinned the tide as a story beat, never a clock"), never
  the sentence somebody typed; write the decision, never its date or its author.
  The record is for whoever authors the next round of this campaign, and a date
  tells them nothing a `git log` does not.

## The campaign's own branch

**Commit the campaign onto its own `campaign/<campaign-id>` branch**, as soon
as the documents are on disk — not onto `main`, and not held back until
everything is green. A campaign is in progress until somebody has walked it, and
everything of it lands on that branch: the documents, the design record, any
prefab it needs, the generation record, the storybook. Sort a file by which
artifact it belongs to: if abandoning the campaign would delete it, it is the
campaign. Conventional message; do not push unless asked. The documents are the
artifact of record: the delve must rebuild byte-identically from them with no
model in the loop.

**A campaign correctly stopped at the design gate does not build yet, and that
is not a defect.** Nothing in CI compiles it for you: the campaign is built by
you, with the engine this page pins, and it builds before the pull request
that publishes it — that is the run that decides whether it ships.

## Stubbing what is not written yet

**While you are authoring incrementally, stub the later documents** and mark the
stubs clearly, so `delvec validate` can run at all. A stub is the envelope plus
a `content` carrying only what its schema requires — `DW0874` prints the recipe.
Two things to expect while stubbing:

- **`quest-plan.json` is the one document the recipe cannot be followed for
  literally.** Its schema requires `finale` as well as `quests`, and `finale`
  has to name a member of `quests`, so an empty `quests` is `DW0131`. Its
  smallest stub is one planned quest with `finale` naming it.
- **Stubs owe things, and the diagnostics name them**: every stage-2 NPC needs
  a stage-6 dialogue tree (`DW0152`), a declared language needs a covering
  sidecar (`DW0180`), and a planned quest with no stage-5 expansion is
  `DW0150` — which is the ordinary state between step 3 and step 5 and is
  discussed there.

## `world.json`, written here

**`world.json` is written here, before placement**, because seven of its eight
required fields are not about placement at all: `title`, `theme`, `premise`,
`seed`, `target_minutes`, `time` and `weather`. Run `delvec schema --stage world`
and write them now. The eighth is `areas`, and which of the two ways it is filled
is step 2's whole question: `areas[]` on path 2A, and **empty** on a site-plan
campaign, where the plan is the placement authority and declaring both is
`DW0839`.

## The hour the delve is played at

**`time` and `weather` are the delve's hour, and the engine will not choose it
for you.** `time` takes `day`, `noon`, `dusk`, `night`, `midnight` or `dawn`
(`sunrise` is accepted as a synonym of `dawn`); `weather` takes `clear`, `rain`
or `thunder`. Neither has a default: "this delve is played at noon" is a design
decision, not a mechanism, so a `world.json` that omits either is `DW0100` like
any other missing required field. `DW0874`'s stub recipe is "its envelope, and a
`content` carrying only the fields its schema requires", so a stubbed
`world.json` carries both from the moment it is stubbed. Both are
dimension-global and frozen by environment sealing, so the state declared here is
the state for the whole delve.

**The order on this page runs backwards from the decision, and that is the
trap.** Step 4 confirms the design on concept art, and art has an hour in it — a
night sea, a storm, first light on a headland. So write the hour the brief and
the design are set in **here**, and expect step 4 to hold the pictures to it: at
approval each image's row in `design.json` states the sky it was drawn under, and
from step 5 onward `DW0890` refuses a campaign whose reachable skies and whose
approved rows are not the same two sets. Two measurements also key off the
declared hour: the dark-cell proof measures under the **darkest reachable**
`(time, weather)` sky, so a space lit only by the sky is judged at the night floor
once a night hour is declared (`DW0210`, and the lighting contract at 2A); and
`DW0496`, the daylight-burning refusal, stands only while the hour is a pinned
clear daytime one — so declaring `night` is a design decision that also switches
that gate off, which is why *Reference: authoring pitfalls* forbids reaching for
the hour to save a mob.

## The optional fields that commit something

**Some of `world.json`'s optional fields commit something you are not writing
yet.** They all sit in this document at this step, and the ones below are
refused, unbound or silently contradicted many steps later, so decide each one
here with the thing it obliges in front of you. The rest are described where
they are used: `languages` in *Reference: other languages*, `difficulty` in
*Reference: authoring pitfalls*, and `outro` — the delve's closing line, the
last player-visible sentence of the run, absent = the finale quest's `goal`.

- **`min_players`.** Absent = 1. Declaring `n ≥ 2` says the delve *requires* n
  bodies, and that is a claim about the QUEST GRAPH: the analyzer demands an
  objective with `n` `after` arms in `n` places — parallel work the party
  divides — and a delve that is one serial chain a single player walks is
  `DW0358` at **step 7**, after `quests.json` is written. So decide the number
  with the quest plan at step 3, not with the world at step 1: a brief that says
  "for two players" is a brief that has asked for a mechanism, and the design
  gains one or the number comes down.
- **`horizon`.** Absent = `void`, and that is the right answer unless the
  surround is part of the design. `valley` rings the map in generated mountain;
  both of those keep the area datum where every piece was authored for it.
  **`ocean` is different and it is paired with a piece set**: it swaps in a
  superflat sea at y=62 and DROPS the area datum to y=60 so a piece meets the
  water at its own declared `waterline_y`. Only pieces carrying that field are
  authored for it, and the invariant that proves the meeting (`DW0344`) examines
  only those — a piece without it is not checked and is not lifted. So the
  question is not "does the library have sea pieces", it is **how much of the
  pool I am about to place is authored for the sea**. Ask it, per pool, before
  you take `ocean`:

  ```sh
  python3 - <<'EOF'
  import json, os
  LIB = os.environ["DELVEWRIGHT_PREFABS"]
  pools = json.load(open(f"{LIB}/pools.json"))["pools"]
  for pid, p in sorted(pools.items()):
      names = [m["prefab"].split("/", 1)[1] for m in p["members"]]
      have = [n for n in names
              if json.load(open(f"{LIB}/{n}.json")).get("waterline_y") is not None]
      print(f"{pid}: {len(have)} of {len(names)} member(s) declare waterline_y  {sorted(have)}")
  EOF
  ```

  **Read what it prints, and do not carry a number off this page** — the
  library is a separate artifact on its own cadence, and a count written here
  describes whichever version of it somebody last looked at. Zero for your pool
  means `ocean` gives you a dropped datum, an invariant examining zero pieces
  and no lever to lift anything clear: that campaign takes `void` and puts the
  sea in the fiction. Short of every member means the unlisted ones stand in the
  water with nothing checking them, which is exactly the silence `DW0344`
  reports about itself.
- **`boundary`, which `ocean` obliges.** Absent = no boundary. It declares the
  playable region: the compiler derives one from the placed geometry plus a
  `margin` of blocks on every side (default 16, range `0..=64`), and a per-second
  clock returns anyone who leaves it to their last checkpoint, with an optional
  `message` on the actionbar. **`horizon: ocean` with no `boundary` is
  `DW0320`** — an infinite swimmable sea with no return rule — so those two are
  written together or neither is written.
