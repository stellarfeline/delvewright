# Reference: drawing the map's reference

## Contents

- [The form, and the four steps](#the-form-and-the-four-steps)
- [The calls, per provider](#the-calls-per-provider)
- [What the sequence form costs, and what checks it](#what-the-sequence-form-costs-and-what-checks-it)
- [Confirmed views become campaign files](#confirmed-views-become-campaign-files)

Needed at step 2B on path B of Init I7. On path A the views are already in
`design/reference/` and this section is how to *extend* the series, not how to
start one.

## The form, and the four steps

**The form is several views of the one subject, each its own image, generated in
sequence.** Front, side, straight-down plan, a named angle — whatever the shape
needs. **Not several views divided into one canvas**: a fixed canvas cut into
four spends three quarters of its resolution on gutters and neighbours, and the
detail a reference exists to preserve is the first thing to go. It also makes
the unit of judgement wrong — one unusable panel forces the whole sheet to be
re-rolled, where one unusable view is re-rolled alone for the cost of one image.

1. Write the **style contract** once: what this place is, in what hand, plus the
   sentence that each image is ONE single full-frame view and never a sheet, a
   grid, a panel or an inset. It is one string, held constant across the series,
   and it is recorded in every sidecar, so a later round can extend the series
   instead of starting one. **Where it goes depends on the provider** — Init I7
   carries the row: `gemini-native` takes it in `--style-note`, and
   `ideogram-v3` has no system-instruction channel at all, so there it is
   prepended to every prompt instead. Same string either way.
2. **View 1 is generated from the prompt alone**, and confirmed for style before
   anything else is drawn. Frame it for what it shows.
3. **Every later view is generated from the prompt plus VIEW 1**, and how you
   pass view 1 is the provider's, not the tool's:
   - `gemini-native` — pass view 1's interaction id to `--chain-from`, read out
     of view 1's sidecar (`.id`). `.id` names the whole INTERACTION, and a
     chained call joins the one it was chained from — so every sidecar in the
     series carries the same `.id`, and reading it out of view 1's is how you
     name the series rather than a view.
   - `ideogram-v3` — pass view 1's **image file** to `--style-ref`. There is no
     interaction chaining on that provider and `--chain-from` is refused
     outright; the image is the anchor. `--style-ref` works on both providers,
     so a run that wants one method for both uses it.
   **Anchor every one of them on the FIRST image, never on the one before it**:
   chaining view to view compounds the drift instead of bounding it. Frame each
   for what it shows — a straight-down site plan is square and an elevation is
   not — which is per call and never a config edit, in that provider's own frame
   vocabulary (`--aspect-ratio`/`--image-size`, or `--resolution`).
4. **Read every returned view against the style contract before it joins the
   series** — including, and especially, the later ones. The contract is an
   instruction to a model, never a guarantee about what came back, and the anchor
   bounds drift rather than eliminating it. *You* are the judge and you judge by
   opening the file: a run that assembles the walkthrough from filenames has
   confirmed nothing, and binds the user's approval at step 4 to whatever
   arrived. A view that breaks the contract — a sheet, a grid, a panel, an inset,
   a caption, a mat or border, a different hand — is **re-drawn once**, the same
   prompt and the same anchor. If the second return breaks it too, stop: put both
   images in front of the user and say what the violation is. A second failure is
   a refusal, not a retry, and three calls for one view is a budget nobody
   approved.

## The calls, per provider

```bash

# the style contract, written once and held constant for the whole series
STYLE="Halgrave, in the same hand throughout: <palette, light, brushwork>. Each
image is ONE single full-frame view of that place, filling the frame edge to
edge: never a sheet, never a grid, never a panel, never an inset or a caption."

# --- on gemini-native ------------------------------------------------------

# view 1 — from the prompt alone, framed as an elevation
python3 "$DELVEWRIGHT_ENGINE/tools/refimg.py" --prompt-file v1-front.txt --style-note "$STYLE" \
    --aspect-ratio 16:9 --out .refimg/map-view1-front

# read view 1's interaction id out of its sidecar — this is the series anchor
V1=$(python3 -c 'import json;print(json.load(open(".refimg/map-view1-front.json"))["id"])')

# every later view: the same anchor, its own prompt, its own frame
python3 "$DELVEWRIGHT_ENGINE/tools/refimg.py" --prompt-file v2-west.txt  --style-note "$STYLE" \
    --chain-from "$V1" --aspect-ratio 16:9 --out .refimg/map-view2-west
python3 "$DELVEWRIGHT_ENGINE/tools/refimg.py" --prompt-file v3-plan.txt  --style-note "$STYLE" \
    --chain-from "$V1" --aspect-ratio 1:1  --out .refimg/map-view3-plan

# --- on ideogram-v3 --------------------------------------------------------
# no --style-note and no --chain-from: the contract is prepended to each prompt,
# view 1's IMAGE is the anchor, and the frame is named in pixels.

cat style.txt v1-front.txt > .refimg/v1-prompt.txt
python3 "$DELVEWRIGHT_ENGINE/tools/refimg.py" --prompt-file .refimg/v1-prompt.txt \
    --resolution 1344x768 --out .refimg/map-view1-front

cat style.txt v2-west.txt > .refimg/v2-prompt.txt
python3 "$DELVEWRIGHT_ENGINE/tools/refimg.py" --prompt-file .refimg/v2-prompt.txt \
    --style-ref .refimg/map-view1-front.png --resolution 1344x768 --out .refimg/map-view2-west
```

The anchor is the same in every later call, and that is where the "anchored on
view 1" claim is checked — the `chain_from` (or the style reference) recorded in
the sidecars, not a sentence in a report. An anchor or a frame the configured
provider cannot honour is **refused, never dropped**: the call stops and names
the provider, the missing capability and the flag to use instead, rather than
returning a correctly-framed picture with no anchor in it. Run the first call of
a new series with `--dry-run` if you are unsure which row you are on; it costs
nothing and answers immediately.

## What the sequence form costs, and what checks it

**The trade, stated because it is why this step has a check in it.**
Co-generating views in one canvas is what guaranteed they agreed about the
*geometry* of the subject; generating them in sequence guarantees only *style*.
So **the geometric facts live in the written brief, and a drift is checked
against text rather than eyeballed** — which is exactly what `geometry-brief.json`
is, and why reference imagery is style authority and never dimensional
authority. Read each view against the brief's facts, not against your memory of
the last picture.

The check this exists to make possible: a zone program once exported as a flat
chain of rooms with no climb, no belfry and no bell, under the name of the tower
its campaign was named after, and it survived until somebody held it against the
zone's own image. A silhouette drawn from three sides is legible enough that the
same collapse cannot pass.

## Confirmed views become campaign files

**When the views are confirmed they become campaign files** — copy them and their
sidecars to `campaigns/<id>/design/reference/`, named for the view, and commit
them with the campaign. An approval that lives only in a gitignored working
directory is bound to nothing, and the sidecar is what makes a view re-issuable
with one word changed: it carries the prompt, the style note, the resolved frame
and the anchor id.
