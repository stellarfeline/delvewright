# Step 4 — the design gate

## Contents

- [What a yes is](#what-a-yes-is)
- [Which pictures these are](#which-pictures-these-are)
- [The hour, read against the pictures](#the-hour-read-against-the-pictures)
- [Confirmed images become campaign files](#confirmed-images-become-campaign-files)
- [`design.json`, the machine half](#designjson-the-machine-half)
- [When a still image cannot answer](#when-a-still-image-cannot-answer)
- [Devices wait for their gate](#devices-wait-for-their-gate)


**Stop here, end your turn, and do not begin step 5 until the user has said
yes.** Steps 1–3 settle
*what the delve is*; step 5 is where the expensive authoring happens, and every
problem this gate would have caught gets paid for twice once it is written.

You arrive here holding a campaign that does not validate, carrying only
refusals whose messages name something `quests.json` or `dialogue.json` will
supply — the rule *Where step 3 ends* states, whether or not the code is one it
lists. That is the correct state to be at this gate in, and it is not something
to mention, apologise for, or try to fix before asking. What is being confirmed
is the design.

Deliver a **walkthrough of the whole design**: the complete story, every scene's
design, and each scene carrying **both a near view and a far view**. Near view is
the scene as a player stands in it; far view is the same scene in its
surroundings, so staging and sightlines read. Not a document with pictures in it
— a visual walkthrough, in the medium the review happens in. A design the
reviewer cannot see is a design they cannot approve.

## What a yes is

**A confirmation is an explicit yes, not the absence of an objection**, and it
is the user's — never yours, and never a reviewer you invented. If you were told
to run the whole thing uninterrupted, this gate still happens: an uninterrupted
run removes the per-step pauses, not the two gates whose whole purpose is the
user's judgement.

## Which pictures these are

**Which pictures these are.** At this gate they are **reference images**:
concept art drawn from the scene description *before any prefab exists*, so what
is confirmed is the design, not a build. A **render** is a candidate prefab
imaged by `delvec render`, and belongs to curation later. Two stages, two
producers; building prefabs first and rendering them inverts the gate.

**A derived blockout has no reference image to judge.** On a site-plan campaign
the pictures at this gate are still design art — the map's own reference views
from step 2B, and the scene concept art anchored on view 1 — and what they show
is the design. **None of them shows the blockout**, and none is drawn to: the
massing is not authored, it is derived from the plan and the metrics table at
step 8, so a picture of it could only be made by inventing what the derivation is
going to do, and it would carry a picture's authority while doing it. So this
gate confirms the design and stops there. **The blockout is judged at step 9, in
the walk** — a site-plan campaign's first real gate, where scale, pacing, route
legibility and the silhouette from the declared `views[]` are settled by somebody
standing in it. Never send anyone to compare the built map against a reference
image of the built map; there is not one.

- On path A of Init I7, the images are already in
  `campaigns/<id>/design/concept/` and already approved, with `design/README.md`
  naming the approved set. The gate is: present the design
  beside them and confirm the design still is what they show.
- On path B, `$DELVEWRIGHT_ENGINE/tools/refimg.py` draws them; prompt iteration is the work, and a
  subject needing more than one view is drawn as a **sequence of single
  full-frame views**, never one canvas cut into panels — the form is in
  *Reference: drawing the map's reference*.
  - **Anchor every scene on the map's own view 1, under the map series'
    style note.** The scenes are places in one map, so they are one series
    with it: pass view 1's interaction id — read out of its sidecar's
    `.id` — to `--chain-from` on every scene, with the same `--style-note`
    string the map views were drawn under. Anchor each on view 1 itself,
    never on the scene before it, for the reason the map series does it:
    chaining picture to picture compounds the drift instead of bounding
    it. Twenty scenes then come back in one hand, and a reviewer reads the
    set as one place rather than twenty unrelated pictures.

## The hour, read against the pictures

**Before you hand it over, read `world.json` against the pictures, and put each
image's sky under the image.** The images show an hour and a sky, and `time` and
`weather` are where that is declared — written at **step 1**, before these
images existed. **The walkthrough shows every image with its two tokens under
it**, in the words the document uses (`night` + `clear`, `dawn` + `rain`), so
the yes you are asking for is a yes to the picture and the hour together rather
than to the picture alone. Where a picture and the document disagree, make them
agree **now** — a one-field edit here, or a night's worth of art redrawn later.
The same reading applies to anything else in the images the documents have to
carry: a piece the library does not hold (step 2A says what you owe for one),
and a room whose light the pictures show as a mood rather than as a lighting
declaration.

## Confirmed images become campaign files

**The moment images are confirmed they become campaign files.** Copy them **and
their `.json` sidecars** to `campaigns/<id>/design/concept/`, one per scene,
named for the scene, and write `campaigns/<id>/design/README.md` carrying the
approved names, what each one shows, and the sentence every later round is held
to: *author from the image, judge against it, present every choice beside it.*
**No date and no approver** — this file is a repository artifact under the same
rule `GENERATION.md` is; what it records is that this set is the approved one,
not when or by whom.
Commit them with the campaign. Everything under `design/` is tracked with
git-lfs (`.gitattributes`), so an approved image commits as a small pointer and
its bytes travel out of band — `git lfs install` in that clone is all this
needs, and it is why carrying a campaign's whole design set in the repository
does not make the next person's clone of it a several-hundred-megabyte download. `$DELVEWRIGHT_ENGINE/tools/refimg.py` writes to a gitignored working
directory, which is right for a draft and wrong for an approved one — **an
approval that lives only in a published page is bound to nothing.** The sidecar
travels with the image because it is what makes the image re-issuable with one
word changed: prompt, style note, resolved frame, anchor id. An image whose
prompt is gone can only be replaced, never edited.

## `design.json`, the machine half

**Write `campaigns/<id>/design.json` in the same act, one row per image you just
copied.** It is the machine half of this approval and the only home the approved
sky has: a row is `{name, shows, time, weather}`, where `name` is the file's path
stem under `design/` with no extension (`concept/shore-far`), `shows` is the one
sentence the README carries for it, and `time` and `weather` are the sky you read
off *that* picture — a set whose finale is a sunrise has two skies, and each row
says its own. Run `delvec schema --stage design` for the exact shape. Those two
tokens are the only judgement on this surface; everything else the engine
derives, and nothing anywhere opens the image.

From step 5 on, `DW0890` holds the record and the directory to each other in both
directions — a row naming no file, an approved file with no row, a stem two files
answer to — and holds the skies the rows state to the skies the world can reach.
A campaign that ships no `design.json` has approved no design: validation
measures that as a zero and says so without refusing, and `"$DELVEWRIGHT_ENGINE/tools/staging-gate.py"`
is where the zero is a red, so a build nobody can stage is what an unwritten
record costs.

**Every later step that asks anyone to choose reads `design/` first**, and
presents the choice beside that scene's image, under the approved name, saying
which element of the image the thing on offer corresponds to. This binds hardest
on contact-sheet curation, which is the step most likely to run in a later
session that never saw this gate.

## When a still image cannot answer

Two tools help when a still image cannot answer the question:

- `delvec --prefabs "$DELVEWRIGHT_PREFABS" viewer <nbt|dir|manifest.json> -o <page.html>` — one self-contained
  page the reviewer drives: orbit, plan, a player point of view at eye height at
  every anchor, and a cutaway that takes the roof off. Every block is drawn from
  the pinned version's own models and textures, so a wall is a wall.
- `delvec --prefabs "$DELVEWRIGHT_PREFABS" contact-sheet <renders> -o <png>` — when candidate prefabs exist and
  someone is choosing between them, all of them on one page. `$DELVEWRIGHT_ENGINE/tools/refscore.py`
  can order the page by similarity to this gate's reference image; the score
  only **orders** the page, it never removes a candidate.

## Devices wait for their gate

**A structural device enters a campaign only behind a green machine gate.** If a
shortcut loop, a one-way drop, an ambush reversal or a multi-path interlock has
no machine gate proving its class, it does not go in yet. Never author it now
and prove it later. When a design wants a device whose gate does not exist, that
is a capability gap: report it, and either the gate lands first or the design
does without it.
