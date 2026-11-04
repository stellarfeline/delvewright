#!/usr/bin/env python3
"""One point of the gallery domain, materialised and compiled to a tree on disk.

## The gap this closes

Every build of the domain except the primary in `en` exists otherwise only
inside `tools/ci/gallery-baseline.py`, in a temp directory it deletes on the way
out — it keeps a hash, not a world. So the site-plan overlay, the map pipeline's
first walkable whole, would be a tree nothing in the repository could produce on
demand, and a bot run over it would name an object no later reader could re-make.

This is that step, and it is deliberately small: materialise, build, and prove
the result is the one the baseline run measured.

## What it produces

Two persistent directories, both gitignored, both replaced on every run:

* `--src` — the campaign. `gallery/` is not one: it carries `baseline/`,
  `overlays/` and `probes/` beside its stage documents. What a point IS is
  decided by `tools/ci/gallery_domain.py` and by nothing here, so this tool cannot
  become the third opinion on the question.
* `--out` — what `delvec build` emits from it: the tree `validation/bot-run.sh
  --output` boots and `delvec` re-reads.

## Why it checks the baseline, and why that is not a second determinism gate

The point of a regenerable tree is that it regenerates **identically**, and a
tool that only says "exit 0" has not established that. So the build's own
`manifest.json` — the compiler's SHA-256 index over every input and every output
path — is compared to the row the baseline run measured for this point: the
`manifests.json` that `tools/ci/gallery-baseline.py` wrote under its `--work`,
handed here by `--manifests` (in CI, by artifact from the baseline job). That row
was written by a different tool, in a different job, from a prefab directory a
different job generated — a real second observer, never this run agreeing with
itself, and never an assumption that the same binary over the same input agrees.

The document names the commit it measured (`tree`), and a document measured at
any other commit than this checkout's HEAD is refused before any row is read: a
stale report from an earlier run is not the run's.

A mismatch is NOT classified here. `tools/ci/gallery-baseline.py` decides what KIND of
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
or finding no row for the point is a red — a rebuild that reproduced nothing is
vacuous, not a pass.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from delvec_bin import resolve as resolve_delvec  # noqa: E402
from gallery_domain import GALLERY, build_id, materialise, overlays  # noqa: E402

REPO = Path(__file__).resolve().parents[2]

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
            "`tools/ci/check-gallery-coverage.py` is what runs it."
        )
    return GALLERY / "overlays" / point


def head_commit() -> str:
    r = subprocess.run(["git", "-C", str(REPO), "rev-parse", "HEAD"], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else ""


def baseline_row(manifests: Path, point: str, lang: str, head: str | None = None) -> dict:
    """The manifest the baseline run measured for this build, or a refusal saying why not."""
    if not manifests.is_file():
        die(
            f"`{manifests}` is missing, so there is nothing to check this build against. "
            "It is written by `python3 tools/ci/gallery-baseline.py` (under its `--work`, "
            "`report/manifests.json`) in the same run."
        )
    doc = json.loads(manifests.read_text())
    if not isinstance(doc, dict) or "tree" not in doc or "manifests" not in doc:
        die(f"`{manifests}` is not a baseline manifests document (`tree`, `manifests`)")
    head = head_commit() if head is None else head
    if doc["tree"] != head:
        die(
            f"`{manifests}` was measured at `{doc['tree'][:12] or '?'}` and this checkout is "
            f"`{head[:12] or '?'}`. Those manifests are not this run's, so a build agreeing "
            "with them would prove nothing about this tree."
        )
    rows = doc["manifests"]
    key = build_id(None if point == PRIMARY else point, lang)
    if key not in rows:
        die(
            f"the measured manifests hold no row `{key}`, so a build of it would be measured against "
            f"nothing. Measured rows: {', '.join(sorted(rows))}. The domain is the primary in "
            "every declared language plus each overlay in `en` (`gallery_domain.points`) — a "
            "point outside it is not a build the baseline run measured."
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


def scratch_within_stated(stderr: str, ledger: dict) -> list[str]:
    """The stores' scratch file a build used, against the cost it stated.

    ADR-0037 §3: `validation/resource-cost.json` states, computed before the
    whole-map stages start, the most bytes the stores' scratch file can take
    (`scratch_bytes.stores`, one region per array alive at once); the
    `world store pages:` line states, as the run ends, what the file took.
    A stated cost below the measured one lets a host provision too little, so
    a build whose file passed its stated bound is red here. Each line read is
    required: a build that printed no pages line, or a ledger without the
    figure, binds nothing and is red too. Returns the red lines, none when
    the file fits; the binding is stated on stdout by the caller.
    """
    reds: list[str] = []
    pages = [l for l in stderr.splitlines() if l.startswith("world store pages: ")]
    if len(pages) != 1:
        return [f"{len(pages)} `world store pages:` line(s) on stderr; one is read"]
    m = re.search(r"; (\d+) byte\(s\) in the scratch file", pages[0])
    if not m:
        return [f"the pages line names no scratch file size: {pages[0]}"]
    used = int(m.group(1))
    try:
        stated = ledger["scratch_bytes"]["stores"]
        computed = ledger["computed_never_measured"] is True
    except (KeyError, TypeError):
        return ["`validation/resource-cost.json` carries no `scratch_bytes.stores`"]
    if not computed:
        reds.append("the ledger does not say `computed_never_measured`: the bound is not the plan's")
    if not isinstance(stated, int) or stated <= 0:
        reds.append(f"the stated scratch bound is `{stated}`, not a positive byte count")
    elif used > stated:
        reds.append(
            f"the stores' scratch file took {used} byte(s) against the {stated} the cost stated: "
            "a host provisioning the stated figure runs out of disk (ADR-0037 §3)"
        )
    return reds


REACH_FILES = ("the lists' and tables'", "the snapshot's")
BANK_FILE = "the bank's"


def reach_file_longest(line: str, name: str) -> int | None:
    """The `at most L long` figure of file `name` on a `reach scratch:` line; None when absent."""
    m = re.search(re.escape(name) + r" (\d+) written, at most (\d+) long", line)
    return int(m.group(2)) if m else None


def reach_scratch_within_stated(stderr: str, ledger: dict) -> tuple[list[str], str]:
    """The reach graph's scratch files a build used, against the cost it stated.

    ADR-0037 §3: `validation/resource-cost.json` states, computed before the
    whole-map stages start, the most bytes the reach graph's scratch files
    the plan bounds can take (`scratch_bytes.reach_graph`: the snapshot's
    pages, the node tables and one closure's lists, per configuration); the
    `reach scratch:` line states, as the run ends, the longest each file was.
    The lists' and tables' file and the snapshot's are held to the figure,
    summed; the bank's file is not bounded by the plan (it holds what the
    launches' simulation read) and is reported beside. Returns the red lines
    and the binding to print: a build that printed no `reach scratch:` line,
    a line naming no such file, or a ledger without the figure binds nothing
    and is red.
    """
    lines = [l for l in stderr.splitlines() if l.startswith("reach scratch: ")]
    if len(lines) != 1:
        return [f"{len(lines)} `reach scratch:` line(s) on stderr; one is read"], ""
    longest = {name: reach_file_longest(lines[0], name) for name in REACH_FILES}
    missing = [name for name, n in longest.items() if n is None]
    if missing:
        return [f"the reach scratch line names no longest for {', '.join(missing)}: {lines[0]}"], ""
    used = sum(n for n in longest.values() if n is not None)
    bank = reach_file_longest(lines[0], BANK_FILE)
    try:
        stated = ledger["scratch_bytes"]["reach_graph"]
        computed = ledger["computed_never_measured"] is True
    except (KeyError, TypeError):
        return ["`validation/resource-cost.json` carries no `scratch_bytes.reach_graph`"], ""
    reds: list[str] = []
    if not computed:
        reds.append("the ledger does not say `computed_never_measured`: the bound is not the plan's")
    if not isinstance(stated, int) or stated <= 0:
        reds.append(f"the stated reach scratch bound is `{stated}`, not a positive byte count")
    elif used > stated:
        reds.append(
            f"the reach graph's scratch files took {used} byte(s) at most on disk against the "
            f"{stated} the cost stated: a host provisioning the stated figure runs out of disk (ADR-0037 §3)"
        )
    parts = "; ".join(f"{name} {n}" for name, n in longest.items())
    binding = (
        f"scratch within the stated cost: the reach graph's files {used} byte(s) at most on disk "
        f"({parts}) against {stated} stated; the bank's "
        f"{bank if bank is not None else 'none'} beside, not bounded (ADR-0037 §3)."
    )
    return reds, binding


def build(delvec: Path, src: Path, out: Path, prefabs: Path, lang: str) -> str:
    # Not cleared first: `delvec build` replaces a tree it wrote and refuses
    # (`DW0967`) a directory holding anything else, which a silent `rmtree` of
    # a caller-supplied path would have deleted.
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
    return r.stderr


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--point",
        required=True,
        help=f"`{PRIMARY}` or an overlay name. Required: a default would hide which of "
        "several campaigns you were handed.",
    )
    ap.add_argument("--lang", default="en")
    ap.add_argument("--delvec", help="the `delvec` this tool runs. Default: `target/release/delvec` then `target/debug/delvec` in this tree — resolved, NAMED on stderr, and refused when it is older than the compiler sources it was built from (`tools/lib/delvec_bin.py`).")
    ap.add_argument("--prefabs", required=True)
    ap.add_argument(
        "--manifests",
        required=True,
        type=Path,
        help="the head manifests the baseline run measured (`gallery-baseline.py`'s "
        "`<work>/report/manifests.json`). Required: the proof is against this run's "
        "measurement and nothing else.",
    )
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

    delvec = resolve_delvec(args.delvec, repo=REPO, caller="gallery-build")
    prefabs = Path(args.prefabs)
    if not prefabs.is_dir():
        die(
            f"no prefab directory at `{prefabs}`. The gallery's pieces are GENERATED and never "
            "committed: `python3 tools/ci/gallery-prefabs.py --out <dir>` runs the generator and "
            "`delvec detail --all` over the site-plan point (spec-0058)."
        )

    point = args.point
    overlay = point_dir(point)
    expected = baseline_row(args.manifests, point, args.lang)

    src = Path(args.src) if args.src else REPO / "validation" / f"gallery-src-{point}"
    out = Path(args.out) if args.out else REPO / "validation" / f"delve-output-gallery-{point}"

    n_src = materialise(src, overlay)
    if n_src == 0:
        die(f"materialising `{point}` wrote ZERO files — the campaign it built is empty")

    stderr = build(delvec, src, out, prefabs, args.lang)

    ledger_path = out / "validation" / "resource-cost.json"
    if not ledger_path.is_file():
        die(f"`delvec build` exited 0 and wrote no `{ledger_path.relative_to(out)}` — no stated cost to hold the run to")
    ledger = json.loads(ledger_path.read_text())
    reds = scratch_within_stated(stderr, ledger)
    if reds:
        die("the build's scratch file against its stated cost: " + "; ".join(reds))
    print(
        f"scratch within the stated cost: the stores' scratch file "
        f"{re.search(r'; (\d+) byte\(s\) in the scratch file', stderr).group(1)} byte(s) against "
        f"{ledger['scratch_bytes']['stores']} stated (ADR-0037 §3)."
    )
    reds, binding = reach_scratch_within_stated(stderr, ledger)
    if reds:
        die("the build's reach scratch files against its stated cost: " + "; ".join(reds))
    print(binding)

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
            f"this build of `{key}` is not the build the baseline run measured, so the tree "
            "written here is NOT the one the baseline measured and nothing downstream may cite "
            "it as such.\n"
            + "\n".join(f"  {line}" for line in differing[:40])
            + (f"\n  … and {len(differing) - 40} more" if len(differing) > 40 else "")
            + "\nWhat KIND of finding this is is decided by `python3 tools/ci/gallery-baseline.py`, "
            "which reads both the diff and what the manifests hold. It is deliberately not "
            "decided here: one authority per question."
        )

    print(f"  campaign:   {src.relative_to(REPO) if src.is_relative_to(REPO) else src}")
    print(f"  build tree: {out.relative_to(REPO) if out.is_relative_to(REPO) else out}")
    print("  identical to the baseline run's measurement of this point.")
    if out.is_relative_to(REPO / "validation"):
        rel = out.relative_to(REPO / "validation")
        print(
            f"  walk it:    EULA=TRUE validation/bot-run.sh --project dw-<id> --output ./{rel}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
