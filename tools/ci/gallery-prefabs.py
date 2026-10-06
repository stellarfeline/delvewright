#!/usr/bin/env python3
"""The gallery's prefab directory, produced by the two committed acts that make it.

## What the gallery's pieces are

The gallery commits no piece. Its hand-built pieces — the hall, the annex kit,
the yard, the skins — are written by `prefabs/gallery-generator`; its
program-detailed place, `node/annex` of the site-plan overlay, is written by
`delvec detail` from `gallery/overlays/site-plan/programs/annex.json`
(spec-0058); its sculpted body, the carcass the `carcass` overlay walks, is
written by `delvec sculpt` from `gallery/forms/` (spec-0087). Every tool that builds the gallery takes `--prefabs <dir>` and
expects both to be there, and until this file existed the second half had no
committed act producing it: a directory holding only the generator's output
validates the site-plan overlay as `DW0842` — a piece the library does not hold.

## What it does, in order

1. Runs the generator into `--out`; the skins go to `gallery/skins` and the
   texture images to `gallery/textures` (spec-0084), which is where every
   materialised point carries them from (gitignored, generated), and the
   design records it reads stand at `gallery/design`.
2. Materialises the site-plan overlay into a scratch directory, exactly as the
   coverage gate, the baseline and `gallery-build.py` materialise it
   (`tools/ci/gallery_domain.py`), and runs `delvec detail <point> --all
   --prefabs <out>` over it.
3. **Asserts the row the verb wrote equals the row the overlay commits.** The
   committed `detail-plan.json` is what every other tool builds; the verb's
   output is what the program actually produces. A difference is the committed
   row standing where a measurement belongs, and the remedy is to commit what
   the verb wrote — never to edit the row by hand.
4. Sculpts every form under `gallery/forms/` into `--out` with `delvec sculpt`,
   then puts every form probe under `gallery/forms/probes/` through the same
   command: a probe is the form it names plus one declared edit
   (`gallery_domain.apply_patch`), and it must be refused with the code its
   `probe.json` names, writing nothing.

## Binding

Every run states how many pieces the generator wrote, how many places the verb
detailed, how many rows it compared to the committed document, how many forms it
sculpted and how many form probes were refused by name. Zero on any of them is a
red.
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
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from delvec_bin import resolve as resolve_delvec  # noqa: E402
from gallery_domain import GALLERY, PatchError, apply_patch, materialise  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
GENERATOR = REPO / "prefabs" / "gallery-generator" / "Cargo.toml"
POINT = "site-plan"


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def generate(out: Path, skins: Path, design: Path, textures: Path) -> int:
    out.mkdir(parents=True, exist_ok=True)
    skins.mkdir(parents=True, exist_ok=True)
    r = subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "--locked",
            "--release",
            "--manifest-path",
            str(GENERATOR),
            "--",
            str(out),
            "--skins",
            str(skins),
            "--design",
            str(design),
            "--textures",
            str(textures),
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


def sculpt(delvec: Path, out: Path) -> int:
    """Sculpt every form under `gallery/forms/` into `out`; return how many."""
    forms = sorted((GALLERY / "forms").glob("*.json"))
    for form in forms:
        r = subprocess.run(
            [str(delvec), "sculpt", str(form), "-o", str(out)], capture_output=True, text=True
        )
        sys.stderr.write(r.stderr)
        if r.returncode != 0:
            die(
                f"`delvec sculpt {form.relative_to(REPO)}` exited {r.returncode}: the gallery's "
                "sculpted body did not sculpt, so the prefab directory is incomplete."
            )
    return len(forms)


def form_probes(delvec: Path) -> tuple[int, int]:
    """Refuse every form probe by name; return (probes examined, edits applied).

    A form probe is `gallery/forms/probes/<name>/probe.json`: the `form` it
    perturbs (a file under `gallery/forms/`), the `code` `delvec sculpt` must
    refuse it with, the `why`, and the `patch` — the declared edit, applied by
    the same pointer rules a campaign probe's is. It must be refused with that
    code and write nothing.
    """
    probes = sorted((GALLERY / "forms" / "probes").glob("*/probe.json"))
    edits = 0
    for manifest_path in probes:
        name = manifest_path.parent.name
        manifest = json.loads(manifest_path.read_text())
        code, why, ops = manifest.get("code") or "", manifest.get("why") or "", manifest.get("patch") or []
        if not code.startswith("DW") or not why or not ops:
            die(f"form probe `{name}` needs a `DW` code, a `why` and a non-empty `patch`")
        form_path = GALLERY / "forms" / (manifest.get("form") or "")
        if not form_path.is_file():
            die(f"form probe `{name}` names form {manifest.get('form')!r}, which `gallery/forms/` does not hold")
        try:
            doc = apply_patch(json.loads(form_path.read_text()), ops)
        except PatchError as e:
            die(f"form probe `{name}` {e}")
        edits += len(ops)
        with tempfile.TemporaryDirectory(prefix="gallery-form-probe-") as tmp:
            perturbed = Path(tmp) / "form.json"
            perturbed.write_text(json.dumps(doc, indent=2) + "\n")
            out = Path(tmp) / "out"
            r = subprocess.run(
                [str(delvec), "sculpt", str(perturbed), "-o", str(out)], capture_output=True, text=True
            )
            codes = {line.split(" ", 1)[0] for line in r.stderr.splitlines() if " [error] " in line}
            if r.returncode == 0:
                die(f"form probe `{name}` was ACCEPTED by `delvec sculpt`; it must be refused with `{code}`")
            if code not in codes:
                die(
                    f"form probe `{name}` was refused, but not with `{code}` (got {sorted(codes) or 'no code'}):\n"
                    + r.stderr
                )
            if out.exists():
                die(f"form probe `{name}` was refused and still wrote `{out}`")
    return len(probes), edits


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
    pieces = generate(out, GALLERY / "skins", GALLERY / "design", GALLERY / "textures")
    if pieces == 0:
        die("the generator wrote ZERO pieces")
    places, compared = detail(delvec, out)
    forms = sculpt(delvec, out)
    if forms == 0:
        die("`gallery/forms/` holds ZERO forms, so nothing exercised `delvec sculpt`")
    probes, edits = form_probes(delvec)
    if probes == 0:
        die("`gallery/forms/probes/` holds ZERO form probes, so no sculpt refusal was re-run")
    print(
        f"gallery prefabs: {pieces} generated piece(s) into `{out}`, {places} place(s) "
        f"detailed by `delvec detail --all`, {compared} row(s) compared to the committed "
        f"detail plan, forms sculpted: {forms}, form probes refused by name: {probes} "
        f"({edits} declared edit(s))."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
