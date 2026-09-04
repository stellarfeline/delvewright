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

## A probe is the primary PLUS A DECLARED EDIT, and that is structural

An overlay ships whole stage documents on purpose: it is a parameter point of the
one campaign, and the documents it carries really are different documents. A
probe is a different object. It exists to show that the engine refuses ONE thing,
so everything in it except that one thing is meant to be the primary — and while
it shipped whole copies, nothing anywhere compared the copy to what it was a copy
OF. Measured across the fifteen probes on the tree this landed on, eight had
drifted: five quest probes carried a `quests.json` 113 paths away from the
primary and one path away from the primary *as it stood the day the probe was
written*; `sound-at-actor` had never been a copy of the primary at all, missing a
whole quest and every actor the gallery had gained since.

Drift there is not cosmetic. A probe that reds for a reason other than the one it
names proves nothing about the rule it claims — the vacuity mode the coverage
gate exists to refuse — and it is invisible right up until the day the stale copy
happens to trip a different diagnostic first.

So a probe's perturbation is a **declared edit** rather than a copy: `probe.json`
carries a `patch` array of JSON-pointer operations over the primary's documents,
applied here, at materialisation. Drift then cannot happen, because there is no
second copy to drift: everything the probe does not name comes from the primary
on every run, and an edit whose path the primary no longer has is a refusal
naming the probe and the path rather than a silent change of subject.

A probe may still ship a whole FILE, and exactly one shape needs to: a document
the primary does not have at all (`site-plan.json`, `detail-plan.json`,
`walk-record.json` — the map-pipeline documents the primary cannot carry, since
`DW0839` refuses a campaign holding both `areas[]` and a site plan). There is
nothing for such a file to drift from. A file that SHADOWS a primary document is
refused: that is the copy this exists to end.
"""

from __future__ import annotations

import json
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

# The three edits a probe may declare. `add` and `replace` are kept apart rather
# than folded into one forgiving verb precisely because the distinction is what
# makes drift visible: an `add` onto a key the primary has gained since is a
# refusal here, where a permissive `set` would silently overwrite it.
PATCH_OPS = ("add", "remove", "replace")


class PatchError(Exception):
    """A declared edit that does not apply to the primary as it now stands.

    Raised rather than printed so the caller can name the probe, the document and
    the pointer in its own vocabulary — this module is a library and its refusals
    have to be catchable by the gate that reports them.
    """


def _unescape(token: str) -> str:
    """RFC 6901: `~1` is a `/` and `~0` a `~`, and the order of the two matters.

    The gallery's own keys need it — `obj/press-the-case` and `npc/marshal` are
    object KEYS with a slash in them, so a pointer that did not escape would name
    a path two levels deep that does not exist.
    """
    return token.replace("~1", "/").replace("~0", "~")


def _resolve(doc, pointer: str):
    """Walk a JSON pointer to the CONTAINER of its last token, and return both.

    Returns `(container, token)`. Everything above the last token must already
    exist: a probe declares an edit to a document it has read, so a pointer whose
    middle is missing is a probe that has lost track of the primary, not a
    request to create the intervening objects.
    """
    if pointer == "":
        raise PatchError("the empty pointer names the whole document, which no edit may replace")
    if not pointer.startswith("/"):
        raise PatchError(f"`{pointer}` is not a JSON pointer (RFC 6901 pointers start with `/`)")
    tokens = [_unescape(t) for t in pointer[1:].split("/")]
    node = doc
    for i, tok in enumerate(tokens[:-1]):
        where = "/" + "/".join(tokens[: i + 1])
        if isinstance(node, dict):
            if tok not in node:
                raise PatchError(f"`{pointer}` does not apply: the primary has no `{where}`")
            node = node[tok]
        elif isinstance(node, list):
            if not tok.isdigit() or int(tok) >= len(node):
                raise PatchError(
                    f"`{pointer}` does not apply: the primary's `{where}` is not an "
                    f"element of a {len(node)}-item array"
                )
            node = node[int(tok)]
        else:
            raise PatchError(
                f"`{pointer}` does not apply: the primary's `{where}` is a scalar, "
                "so nothing is nested under it"
            )
    return node, tokens[-1]


def apply_patch(doc, ops: list[dict]):
    """Apply one document's declared edits, in order, refusing anything that misses.

    In order, because `add` onto an array is positional and two adds at the same
    index are not the same patch in either sequence. Every refusal names the
    pointer, since the pointer is what a reader has to go and look at.
    """
    for op in ops:
        verb = op.get("op")
        pointer = op.get("path", "")
        if verb not in PATCH_OPS:
            raise PatchError(f"`{verb!r}` is not one of {', '.join(PATCH_OPS)}")
        node, token = _resolve(doc, pointer)
        has_value = "value" in op
        if verb in ("add", "replace") and not has_value:
            raise PatchError(f"`{verb}` at `{pointer}` carries no `value`")
        if verb == "remove" and has_value:
            raise PatchError(f"`remove` at `{pointer}` carries a `value`, which it cannot use")
        if isinstance(node, dict):
            present = token in node
            if verb == "add" and present:
                raise PatchError(
                    f"`add` at `{pointer}` does not apply: the primary already has that key "
                    "(an edit that means to overwrite says `replace`, so that a key the "
                    "primary GAINS is a red rather than a silent overwrite)"
                )
            if verb in ("remove", "replace") and not present:
                raise PatchError(f"`{verb}` at `{pointer}` does not apply: the primary has no such key")
            if verb == "remove":
                del node[token]
            else:
                node[token] = op["value"]
        elif isinstance(node, list):
            if verb == "add":
                idx = len(node) if token == "-" else None
                if idx is None:
                    if not token.isdigit() or int(token) > len(node):
                        raise PatchError(
                            f"`add` at `{pointer}` does not apply: `{token}` is not an "
                            f"insertion point in a {len(node)}-item array"
                        )
                    idx = int(token)
                node.insert(idx, op["value"])
            else:
                if not token.isdigit() or int(token) >= len(node):
                    raise PatchError(
                        f"`{verb}` at `{pointer}` does not apply: the primary's array holds "
                        f"{len(node)} item(s)"
                    )
                if verb == "remove":
                    del node[int(token)]
                else:
                    node[int(token)] = op["value"]
        else:
            raise PatchError(f"`{pointer}` does not apply: its parent is a scalar")
    return doc


def patch_ops(point: Path | None) -> list[dict]:
    """The edits a point declares, as an ordered list — empty for anything else.

    Read from `probe.json` rather than from a file beside it so that the thing a
    reader opens to learn what a probe TRIES is the same thing the machine
    applies. An overlay declares none: it ships whole documents by design.
    """
    if point is None:
        return []
    manifest = Path(point) / "probe.json"
    if not manifest.is_file():
        return []
    ops = json.loads(manifest.read_text()).get("patch") or []
    if not isinstance(ops, list):
        raise PatchError(f"`{manifest}` has a `patch` that is not a list of edits")
    return ops


def shadowed_documents(point: Path | None) -> list[str]:
    """Files a probe ships that the PRIMARY also holds — the copy shape, by name.

    A probe's whole content except its declared edit is meant to be the primary,
    so a file of the same name is a second copy of a document nothing compares:
    it is how eight of the fifteen probes came to be up to 113 JSON paths away
    from what they claimed to be a perturbation of. A file the primary does NOT
    hold is a different object and is left alone — the map-pipeline documents are
    genuinely new, and there is nothing for them to drift from.
    """
    if point is None:
        return []
    point = Path(point)
    if not (point / "probe.json").is_file():
        return []
    out = []
    for f in sorted(point.rglob("*")):
        if not f.is_file() or f.name in POINT_MANIFESTS:
            continue
        rel = f.relative_to(point)
        if (GALLERY / rel).is_file():
            out.append(str(rel))
    return out


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

    A probe's `patch` is applied HERE rather than in the gate that reads it, so
    that every consumer of a probe point sees the same campaign — the same reason
    this module exists at all. A patch that does not apply raises `PatchError`,
    which the caller names the probe in.
    """
    dest = Path(dest)
    _refuse_dangerous_dest(dest)
    shutil.rmtree(dest, ignore_errors=True)
    shutil.copytree(GALLERY, dest)
    for junk in NOT_CAMPAIGN:
        shutil.rmtree(dest / junk, ignore_errors=True)
    if point is not None:
        shadowed = shadowed_documents(point)
        if shadowed:
            raise PatchError(
                f"ships {', '.join(shadowed)}, which the primary also holds. A probe is "
                "the primary plus a declared edit: name the edit in `probe.json`'s "
                "`patch`, so nothing can drift from a copy nobody compares"
            )
        for f in sorted(Path(point).iterdir()):
            if f.name in POINT_MANIFESTS:
                continue
            if f.is_dir():
                shutil.copytree(f, dest / f.name, dirs_exist_ok=True)
            else:
                shutil.copy2(f, dest / f.name)
        by_doc: dict[str, list[dict]] = {}
        for op in patch_ops(point):
            by_doc.setdefault(op.get("doc") or "", []).append(op)
        for rel, ops in by_doc.items():
            target = dest / rel
            if not rel or not target.is_file():
                raise PatchError(
                    f"declares an edit to `{rel}`, which is not a document of the primary"
                )
            doc = json.loads(target.read_text())
            try:
                apply_patch(doc, ops)
            except PatchError as e:
                raise PatchError(f"`{rel}`: {e}") from None
            target.write_text(json.dumps(doc, indent=2, ensure_ascii=False, sort_keys=True) + "\n")
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
