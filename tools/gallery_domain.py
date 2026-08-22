#!/usr/bin/env python3
"""One authority on what a gallery build POINT is, and how it becomes a campaign.

A **build point** of the gallery domain (spec-0039 §3) is the primary campaign,
or the primary with one overlay's files laid over it, or the primary with one
probe's. Nothing downstream can use `gallery/` directly: the directory carries
`baseline/`, `overlays/` and `probes/` beside its stage documents, so it is the
domain's SOURCE and not a campaign directory. Materialising is what turns one
point of it into a campaign, and this is the only place that happens.

## Why this is a module rather than a function in each caller

It was a function in each caller, and the two had drifted into meaning different
things. `tools/gallery-baseline.py` held the one that BUILDS — it strips the
three non-campaign directories, so what `delvec` compiles is the campaign and
nothing else. `tools/check-gallery-coverage.py` held one that only ever had to
survive a schema walk, so it stripped nothing and skipped a different pair of
manifest files. Two consequences, and the second is the one that is structural:

* a round that needed to build a domain point read the coverage checker's copy
  first, took the validate-only one, and had to work the difference out of the
  repository by hand;
* the two tools were judging different objects. The coverage gate validates a
  materialised point; the baseline compiles one; and "the point" meant a tree
  with `overlays/` nested inside it in the first case and without in the second.
  Benign today, because `delvec` reads stage documents by name — and exactly the
  shape that stops being benign the moment anything walks a campaign directory.

A third copy is what this module exists to make unnecessary. It is also what
`tools/tests/test_gallery_domain.py` refuses.

## What the union costs each caller

Nothing either one was relying on. The strip set is `baseline`, `overlays`,
`probes`, none of which a campaign document ever names; the skip set is the union
`overlay.json` + `probe.json`, and an overlay carries no probe manifest nor a
probe an overlay manifest, so each caller skips exactly what it skipped before.
What both gain is that the tree the coverage gate validates is now, byte for
byte, the tree the baseline compiles and `tools/gallery-build.py` writes to disk.
"""

from __future__ import annotations

import shutil
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
GALLERY = REPO / "gallery"

# Beside the stage documents, and never part of a campaign: the committed hash
# baseline, the overlay set, the refusal probes. A point is the gallery MINUS
# these, plus at most one of the points inside them.
NOT_CAMPAIGN = ("baseline", "overlays", "probes")

# A point's own manifest — what it declares it binds, or which refusal it
# demonstrates — is tooling metadata, never a stage document.
POINT_MANIFESTS = ("overlay.json", "probe.json")


def overlays() -> list[str]:
    """The overlay names, derived from the directory — never a listed set.

    A listed set goes stale the first time an overlay is added, and goes stale
    silently.
    """
    d = GALLERY / "overlays"
    return sorted(p.name for p in d.iterdir() if p.is_dir()) if d.is_dir() else []


def build_id(overlay: str | None, lang: str) -> str:
    """The key one build of the domain is recorded under, in `gallery/baseline/`.

    Shared because `gallery-build.py` cross-checks its own build against the
    committed baseline row, and a second opinion about the KEY would let that
    cross-check silently compare a build against nothing.
    """
    return f"{overlay or 'primary'}.{lang}"


def materialise(dest: Path, point: Path | None = None) -> int:
    """Write the campaign for one build point to `dest`; return the file count.

    `point` is an overlay or probe directory, or `None` for the primary. `dest`
    is REPLACED, not merged: a materialisation that leaves a file from a previous
    point behind is a campaign nobody authored, and the merge semantics the two
    original copies shared (`dirs_exist_ok=True` onto whatever was there) is how
    that would happen without a word of output.

    The count is returned rather than printed so that each caller can state it in
    its own binding line; a point that materialised zero files is a finding for
    the caller to name.
    """
    dest = Path(dest)
    _refuse_dangerous_dest(dest)
    shutil.rmtree(dest, ignore_errors=True)
    shutil.copytree(GALLERY, dest)
    for junk in NOT_CAMPAIGN:
        shutil.rmtree(dest / junk, ignore_errors=True)
    if point is not None:
        for f in sorted(Path(point).iterdir()):
            if f.name in POINT_MANIFESTS:
                continue
            if f.is_dir():
                shutil.copytree(f, dest / f.name, dirs_exist_ok=True)
            else:
                shutil.copy2(f, dest / f.name)
    return sum(1 for p in dest.rglob("*") if p.is_file())


def _refuse_dangerous_dest(dest: Path) -> None:
    """This function deletes `dest`. Two families it must never be handed.

    Not defensiveness for its own sake: every caller passes a path built from a
    command-line default, and the cost of one wrong default here is the gallery
    source itself. Asked of the RESOLVED path, because a relative `--src gallery`
    and an absolute one are the same directory and only one of them looks alarming.
    """
    r = dest.resolve()
    if r == GALLERY or GALLERY in r.parents:
        raise SystemExit(
            f"error: refusing to materialise over `{r}` — that is the gallery source, "
            "which is the input every build point is copied FROM"
        )
    if r in GALLERY.parents:
        raise SystemExit(
            f"error: refusing to materialise over `{r}` — it contains the gallery"
        )
