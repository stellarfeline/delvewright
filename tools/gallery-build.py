#!/usr/bin/env python3
"""One point of the gallery domain, materialised and compiled to a tree on disk.

## The gap this closes

Every build of the domain except the primary in `en` existed only inside
`tools/gallery-baseline.py`, in a temp directory it deletes on the way out. The
baseline is right to do that — it keeps a hash, not a world — but it meant the
site-plan overlay, the map pipeline's first walkable whole, was a tree **nothing
in the repository could produce on demand**. The bot ladder walked it end to end
and the run was reported against a build that no committed act could bring back,
so the evidence named an object no later reader could re-make, and a round that
went looking for the materialisation read the coverage checker's validate-only
copy first.

This is the missing step, and it is deliberately small: materialise, build,
and prove the result is the one the baseline recorded.

## What it produces

Two persistent directories, both gitignored, both replaced on every run:

* `--src` — the campaign. `gallery/` is not one: it carries `baseline/`,
  `overlays/` and `probes/` beside its stage documents. What a point IS is
  decided by `tools/gallery_domain.py` and by nothing here, so this tool cannot
  become the third opinion on the question.
* `--out` — what `delvec build` emits from it: the tree `validation/bot-run.sh
  --output` boots and `delvec` re-reads.

## Why it checks the baseline, and why that is not a second determinism gate

The point of a regenerable tree is that it regenerates **identically**, and a
tool that only says "exit 0" has not established that. So the build's own
`manifest.json` — the compiler's SHA-256 index over every input and every output
path — is compared to the row `gallery/baseline/manifests.json` already holds for
this point. That row was written by a different tool, on a different machine, at a
different time, which is what makes it a real second observer rather than this
run agreeing with itself.

A mismatch is NOT classified here. `tools/gallery-baseline.py` decides what KIND of
finding a moved manifest is, and a second answer to that question is exactly the
defect this file exists to stop being repeated. This refuses, names the differing
paths, and sends the reader there. Which verdicts exist is deliberately not
restated: an enumeration written down twice goes stale on one side, and this one
already has — it gained a third verdict while both copies still said two.

## The gallery is never STAGED, and this tool does not change that

spec-0039 §2: the gallery is authored engine-test source, never released, never
staged; its build outputs are CI artifacts. What this makes regenerable is the
tree the BOT walks and CI measures. It is deliberately not wired into any staging
surface, and `tools/tests/test_gallery_not_shippable.py` is what keeps that true —
a build a human is handed comes from a content-repo campaign, and the human walk
spec-0049 §5.4 asks for is a campaign's gate, not the gallery's.

## Binding count

Every run states the files materialised, the emitted paths compared, and the
baseline row it compared them to. Materialising zero files, emitting zero paths,
or finding no baseline row for the point is a red — a rebuild that reproduced
nothing is vacuous, not a pass.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gallery_domain import GALLERY, build_id, materialise, overlays  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
BASELINE = GALLERY / "baseline" / "manifests.json"

PRIMARY = "primary"


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def point_dir(point: str) -> Path | None:
    """The overlay directory for `point`, or `None` for the primary."""
    if point == PRIMARY:
        return None
    known = overlays()
    if point not in known:
        die(
            f"`{point}` is not a point of the gallery domain. The points are "
            f"`{PRIMARY}` and the overlays: {', '.join(known) or '(none)'}. "
            "A probe is not a point — it exists to be REFUSED, and "
            "`tools/check-gallery-coverage.py` is what runs it."
        )
    return GALLERY / "overlays" / point


def baseline_row(point: str, lang: str) -> dict:
    """The manifest the baseline recorded for this build, or a refusal saying why not."""
    if not BASELINE.is_file():
        die(
            f"`{BASELINE.relative_to(REPO)}` is missing, so there is nothing to check this "
            "build against. Write it with `python3 tools/gallery-baseline.py --write`."
        )
    rows = json.loads(BASELINE.read_text())
    key = build_id(None if point == PRIMARY else point, lang)
    if key not in rows:
        die(
            f"the baseline holds no row `{key}`, so a build of it would be measured against "
            f"nothing. Recorded rows: {', '.join(sorted(rows))}. The domain is the primary in "
            "every declared language plus each overlay in `en` — a point outside it is not a "
            "build this repository has ever taken a baseline of."
        )
    return rows[key]


def manifest_delta(expected: dict, got: dict) -> list[str]:
    """Every way the two manifests disagree, named — over the WHOLE manifest.

    Not over `outputs` alone, and that is a repair rather than a preference. A
    manifest carries the compiler's index over its INPUTS as well as its outputs,
    so the first perturbation this refusal was tested against — one number in a
    stage document, emitting identical bytes — moved `inputs` and nothing else.
    An outputs-only delta printed a refusal with an empty
    evidence list under it, which is the shape that sends the next reader to
    rebuild the gallery to find out what the red was about.
    """
    lines: list[str] = []
    for k in sorted(set(expected) | set(got)):
        a, b = expected.get(k), got.get(k)
        if a == b:
            continue
        if isinstance(a, dict) and isinstance(b, dict):
            for name in sorted(set(a) | set(b)):
                if a.get(name) == b.get(name):
                    continue
                if name not in b:
                    lines.append(f"{k}: {name} — in the baseline, absent here")
                elif name not in a:
                    lines.append(f"{k}: {name} — emitted here, absent from the baseline")
                else:
                    lines.append(f"{k}: {name} — same path, different content")
        else:
            lines.append(f"{k}: baseline `{a}` vs this build `{b}`")
    return lines


def build(delvec: Path, src: Path, out: Path, prefabs: Path, lang: str) -> None:
    shutil.rmtree(out, ignore_errors=True)
    r = subprocess.run(
        [str(delvec), "--lang", lang, "build", str(src), "-o", str(out), "--prefabs", str(prefabs)],
        capture_output=True,
        text=True,
    )
    # stderr carries the compiler's own binding lines and its judgement-tier
    # warnings. They belong to whoever ran this, not to a log nobody reads: the
    # blockout battery's counts are the closest thing the walk has to a
    # description of what it is about to walk.
    sys.stderr.write(r.stderr)
    if r.returncode != 0:
        die(
            f"`delvec build` exited {r.returncode}. A tree that does not compile is not a "
            "tree anything can walk.\n" + r.stdout
        )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--point",
        required=True,
        help=f"`{PRIMARY}` or an overlay name. Required: a default would hide which of "
        "several campaigns you were handed.",
    )
    ap.add_argument("--lang", default="en")
    ap.add_argument("--delvec", default=str(REPO / "target/release/delvec"))
    ap.add_argument("--prefabs", required=True)
    ap.add_argument(
        "--src",
        default=None,
        help="where the campaign is written (default validation/gallery-src-<point>)",
    )
    ap.add_argument(
        "--out",
        default=None,
        help="where the build tree is written (default validation/delve-output-gallery-<point>)",
    )
    args = ap.parse_args()

    delvec = Path(args.delvec)
    if not delvec.is_file():
        die(f"no delvec at `{delvec}` — build one with `cargo build -p delvec --bin delvec`")
    prefabs = Path(args.prefabs)
    if not prefabs.is_dir():
        die(
            f"no prefab directory at `{prefabs}`. The gallery's piece is GENERATED and never "
            "committed: `cargo run --manifest-path prefabs/gallery-generator/Cargo.toml -- "
            "<dir> --skins gallery/skins`."
        )

    point = args.point
    overlay = point_dir(point)
    expected = baseline_row(point, args.lang)

    src = Path(args.src) if args.src else REPO / "validation" / f"gallery-src-{point}"
    out = Path(args.out) if args.out else REPO / "validation" / f"delve-output-gallery-{point}"

    n_src = materialise(src, overlay)
    if n_src == 0:
        die(f"materialising `{point}` wrote ZERO files — the campaign it built is empty")

    build(delvec, src, out, prefabs, args.lang)

    manifest_path = out / "manifest.json"
    if not manifest_path.is_file():
        die(f"`delvec build` exited 0 and wrote no `{manifest_path.name}` — nothing to compare")
    manifest = json.loads(manifest_path.read_text())
    n_out = len(manifest.get("outputs") or manifest.get("files") or {})

    key = build_id(None if point == PRIMARY else point, args.lang)
    print(
        f"gallery build `{key}`: {n_src} source file(s) materialised, {n_out} emitted path(s) "
        f"compared against baseline row `{key}`."
    )
    if n_out == 0:
        die(
            "the build emitted ZERO paths, so the manifest comparison below asserted nothing. "
            "A rebuild that reproduced nothing is vacuous, not a pass."
        )
    if manifest != expected:
        differing = manifest_delta(expected, manifest)
        die(
            f"this build of `{key}` is not the build `gallery/baseline/` recorded, so the tree "
            "written here is NOT the one the baseline measured and nothing downstream may cite "
            "it as such.\n"
            + "\n".join(f"  {line}" for line in differing[:40])
            + (f"\n  … and {len(differing) - 40} more" if len(differing) > 40 else "")
            + "\nWhat KIND of finding this is is decided by `python3 tools/gallery-baseline.py`, "
            "which reads both the diff and what the manifests hold. It is deliberately not "
            "decided here: one authority per question."
        )

    print(f"  campaign:   {src.relative_to(REPO) if src.is_relative_to(REPO) else src}")
    print(f"  build tree: {out.relative_to(REPO) if out.is_relative_to(REPO) else out}")
    print("  identical to the committed baseline for this point.")
    if out.is_relative_to(REPO / "validation"):
        rel = out.relative_to(REPO / "validation")
        print(
            f"  walk it:    EULA=TRUE validation/bot-run.sh --project dw-<id> --output ./{rel}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
