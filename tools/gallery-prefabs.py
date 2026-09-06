#!/usr/bin/env python3
"""The gallery's prefab directory, produced by the two committed acts that make it.

## What the gallery's pieces are

The gallery commits no piece. Its hand-built pieces — the hall, the annex kit,
the yard, the skins — are written by `prefabs/gallery-generator`; its
program-detailed place, `node/annex` of the site-plan overlay, is written by
`delvec detail` from `gallery/overlays/site-plan/programs/annex.json`
(spec-0058). Every tool that builds the gallery takes `--prefabs <dir>` and
expects both to be there, and until this file existed the second half had no
committed act producing it: a directory holding only the generator's output
validates the site-plan overlay as `DW0842` — a piece the library does not hold.

## What it does, in order

1. Runs the generator into `--out`; the skins go to `gallery/skins`, which is
   where every materialised point carries them from (gitignored, generated).
2. Materialises the site-plan overlay into a scratch directory, exactly as the
   coverage gate, the baseline and `gallery-build.py` materialise it
   (`tools/gallery_domain.py`), and runs `delvec detail <point> --all
   --prefabs <out>` over it.
3. **Asserts the row the verb wrote equals the row the overlay commits.** The
   committed `detail-plan.json` is what every other tool builds; the verb's
   output is what the program actually produces. A difference is the committed
   row standing where a measurement belongs, and the remedy is to commit what
   the verb wrote — never to edit the row by hand.

## Binding

Every run states how many pieces the generator wrote, how many places the verb
detailed, and how many rows it compared to the committed document. Zero on any
of them is a red.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from delvec_bin import resolve as resolve_delvec  # noqa: E402
from gallery_domain import GALLERY, materialise  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
GENERATOR = REPO / "prefabs" / "gallery-generator" / "Cargo.toml"
POINT = "site-plan"


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def generate(out: Path, skins: Path) -> int:
    out.mkdir(parents=True, exist_ok=True)
    skins.mkdir(parents=True, exist_ok=True)
    r = subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "--locked",
            "--manifest-path",
            str(GENERATOR),
            "--",
            str(out),
            "--skins",
            str(skins),
        ],
        capture_output=True,
        text=True,
    )
    sys.stderr.write(r.stderr)
    if r.returncode != 0:
        die(f"the gallery generator exited {r.returncode}\n{r.stdout}")
    return sum(1 for p in out.iterdir() if p.suffix == ".nbt")


def detail(delvec: Path, out: Path) -> tuple[int, int]:
    """Detail every program-bound place of the site-plan point into `out`.

    Returns (places detailed, rows compared to the committed document).
    """
    overlay = GALLERY / "overlays" / POINT
    committed = json.loads((overlay / "detail-plan.json").read_text())
    with tempfile.TemporaryDirectory(prefix="gallery-detail-") as tmp:
        point = Path(tmp) / POINT
        n = materialise(point, overlay)
        if n == 0:
            die(f"materialising `{POINT}` wrote ZERO files")
        r = subprocess.run(
            [str(delvec), "--prefabs", str(out), "detail", str(point), "--all"],
            capture_output=True,
            text=True,
        )
        sys.stderr.write(r.stderr)
        if r.returncode != 0:
            die(
                f"`delvec detail {POINT} --all` exited {r.returncode}. The gallery's "
                "program-detailed place did not detail, so the prefab directory is "
                "incomplete.\n" + r.stdout
            )
        written = json.loads((point / "detail-plan.json").read_text())
    # Which rows the verb wrote: every row whose place has a program.
    programs = {p.stem for p in (overlay / "programs").glob("*.json")}
    by_place = {row["place"]: row for row in committed["content"]["details"]}
    compared = 0
    for row in written["content"]["details"]:
        stem = row["place"].split("/", 1)[1]
        if stem not in programs:
            continue
        compared += 1
        have = by_place.get(row["place"])
        if have != row:
            die(
                f"the row `delvec detail` writes for `{row['place']}` is not the row "
                f"`{overlay.relative_to(REPO)}/detail-plan.json` commits.\n"
                f"  verb:      {json.dumps(row, sort_keys=True)}\n"
                f"  committed: {json.dumps(have, sort_keys=True)}\n"
                "A committed row is what the program produces, never a hand edit: run "
                "the verb over the materialised point and commit `detail-plan.json` as "
                "it leaves it."
            )
    if compared == 0:
        die(
            f"`delvec detail --all` detailed a place but wrote no row for any program under "
            f"`{overlay.relative_to(REPO)}/programs/` — nothing was compared."
        )
    return len(programs), compared


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--delvec",
        help="the `delvec` this tool runs — resolved, NAMED on stderr, and refused when it "
        "is older than the compiler sources it was built from (`tools/lib/delvec_bin.py`).",
    )
    ap.add_argument("--out", required=True, help="the prefab directory to produce")
    args = ap.parse_args()
    delvec = resolve_delvec(args.delvec, repo=REPO, caller="gallery-prefabs")
    out = Path(args.out)
    if out.exists():
        shutil.rmtree(out)
    pieces = generate(out, GALLERY / "skins")
    if pieces == 0:
        die("the generator wrote ZERO pieces")
    places, compared = detail(delvec, out)
    print(
        f"gallery prefabs: {pieces} generated piece(s) into `{out}`, {places} place(s) "
        f"detailed by `delvec detail --all`, {compared} row(s) compared to the committed "
        "detail plan."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
