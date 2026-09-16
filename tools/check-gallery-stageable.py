#!/usr/bin/env python3
"""Every build this repository produces can be served — asked on every push.

## The gap this closes

`tools/staging-gate.py` decides whether a built delve may be put in front of a
player, and until this ran its only consumer was `tools/playtest-server.sh`:
step 9 of the `/new-delve` page, reached by a creator with a built world in
hand. That binds the gate to the moment a delve is staged — which is right, and
which is also the whole of it. **Nothing asked the question on a push.** The
engine could grow a campaign the gate would refuse, and the first thing that
said so would be a person at the end of the pipeline.

The subject a push CAN ask it about is the gallery: spec-0039's engine-owned
campaign, built by every revision, holding one instance of every surface the DSL
declares. So the property this step holds is *the engine's own campaign, in
every point of its domain, carries a live binding check for every finding class
it contains* — the same question the staging gate asks of a content build, asked
of the only build a push has.

Measured when this landed: the site-plan point declared a `flask` recovery kit
and no `bonfire`, so `bell-01` read `UNBOUND` — a flask nothing refills — and
nothing had ever said so, because nothing had ever run the gate.

## The gallery is still never STAGED, and this does not change that

spec-0039 §2, and `tools/tests/test_gallery_not_shippable.py` is what keeps it
true. A pass writes an admission token, and a token inside a gallery build tree
is exactly what the compose staging path looks for — so every token this tool
mints goes to `--work`, never into a build tree, and each build tree is then
asserted to hold none. Judging a build is a question about coverage; it is not
an act of handing anything to anybody.

## The domain is enumerated, never typed

The points come from `gallery/baseline/manifests.json` — the ladder's own build
ledger, one row per build the baseline records, which is the primary in every
declared language plus each overlay in `en`. A hand-written list here would be a
second opinion on what the gallery is, and it would go stale silently the first
time an overlay was added: the job would keep passing while judging less than
the ladder builds, which is this project's `unbound` vacuity wearing a green
tick.

Cross-checked against the directory (`gallery_domain.overlays()` plus the
primary), which shares no configuration with the committed ledger: a point the
domain has and the ledger does not means the two have drifted, and this refuses
rather than judging the smaller set.

## Binding count

Every run states the rows enumerated, the points judged, and the denominator.
Judging zero points, or fewer points than the ledger holds, is a red — a gate
that examined nothing is not a pass.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gallery_domain import GALLERY, build_id, overlays  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
BASELINE = GALLERY / "baseline" / "manifests.json"
GALLERY_BUILD = REPO / "tools" / "gallery-build.py"
STAGING_GATE = REPO / "tools" / "staging-gate.py"
PRIMARY = "primary"
ADMISSION = "staging-admission.json"


def _staging_gate_module():
    """The gate itself, imported — so which verdicts REFUSE is asked of it.

    A second copy of that tuple here would be two authorities for one rule, and
    it would go stale in the direction that reads a red as a green: a verdict
    added to the gate and not to the copy would be counted as a pass by this
    step while the gate below it exited 1.
    """
    spec = importlib.util.spec_from_file_location("staging_gate", STAGING_GATE)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


RED_VERDICTS = _staging_gate_module().RED_VERDICTS


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def ledger_rows() -> list[tuple[str, str]]:
    """The `(point, lang)` pairs the baseline records, in the ledger's own order.

    `build_id` owns the key format, so the split here is checked against it
    rather than trusted: a key this tool parses into a pair that does not
    re-encode to the same string is a key shape nobody agreed on, and guessing
    at it would silently drop a build from the domain.
    """
    if not BASELINE.is_file():
        die(
            f"`{BASELINE.relative_to(REPO)}` is missing, so there is no build ledger to "
            "enumerate the domain from. Write it with `python3 tools/gallery-baseline.py --write`."
        )
    rows = json.loads(BASELINE.read_text())
    if not isinstance(rows, dict):
        die(f"`{BASELINE.relative_to(REPO)}` is not a mapping of build id to manifest")
    out: list[tuple[str, str]] = []
    for key in sorted(rows):
        point, dot, lang = key.rpartition(".")
        if not dot or not point or not lang:
            die(f"`{key}` is not a `<point>.<lang>` build id")
        if build_id(None if point == PRIMARY else point, lang) != key:
            die(f"`{key}` does not re-encode from the pair ({point}, {lang})")
        out.append((point, lang))
    return out


def domain_points() -> list[str]:
    """What the gallery directory says its points are — the second observer."""
    return [PRIMARY, *overlays()]


def uncovered_points(rows: list[tuple[str, str]], declared: list[str]) -> list[str]:
    """Points the gallery directory has and the build ledger does not record.

    The comparison is the reason the ledger may be the enumeration at all: a
    ledger that has fallen behind the directory would let this step pass while
    judging less than the ladder builds, which is a green over an unbound gate.
    """
    have = {point for point, _ in rows}
    return [p for p in declared if p not in have]


def admission_path(work: Path, key: str) -> Path:
    """Where the token for one point goes — outside every build tree, always.

    The staging gate's default is `<build>/staging-admission.json`, and that is
    exactly the file the compose staging path looks for. Taking the default here
    would make each green gallery point a servable one.
    """
    return work / "admission" / f"{key}.json"


def token_in_tree(out: Path) -> bool:
    """Whether a build tree carries an admission token (spec-0039 §2: never)."""
    return (out / ADMISSION).is_file()


def build_point(delvec: Path, prefabs: Path, work: Path, point: str, lang: str) -> tuple[Path, Path]:
    """Materialise and compile one point, through the one tool that owns that act.

    `--src`/`--out` are named rather than defaulted because the default carries
    the point and not the language, so `primary.en` and `primary.zh-cn` would
    compile over each other and this gate would judge one tree twice.
    """
    src = work / f"gallery-src-{point}-{lang}"
    out = work / f"delve-output-gallery-{point}-{lang}"
    r = subprocess.run(
        [
            sys.executable,
            str(GALLERY_BUILD),
            "--point",
            point,
            "--lang",
            lang,
            "--delvec",
            str(delvec),
            "--prefabs",
            str(prefabs),
            "--src",
            str(src),
            "--out",
            str(out),
        ],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        sys.stderr.write(r.stderr)
        die(
            f"`gallery-build.py --point {point} --lang {lang}` exited {r.returncode}. A point "
            "that does not build is not a point anything can be served from, and this gate has "
            "nothing to judge."
        )
    return src, out


def judge(ledger: Path, work: Path, point: str, lang: str, src: Path, out: Path) -> dict:
    """Run the staging gate on one built point and return what it decided."""
    key = build_id(None if point == PRIMARY else point, lang)
    admit = admission_path(work, key)
    report = work / "reports" / f"{key}.md"
    verdicts = work / "reports" / f"{key}.json"
    r = subprocess.run(
        [
            sys.executable,
            str(STAGING_GATE),
            "--campaign",
            str(src),
            "--build",
            str(out),
            "--ledger",
            str(ledger),
            "--admit",
            str(admit),
            "--report",
            str(report),
            "--json",
            str(verdicts),
        ],
        capture_output=True,
        text=True,
    )
    if r.returncode == 2:
        sys.stderr.write(r.stderr)
        die(f"the staging gate could not read `{key}` at all (exit 2), so it judged nothing")
    if not verdicts.is_file():
        die(f"the staging gate wrote no verdict file for `{key}`, so there is nothing to read")
    rows = json.loads(verdicts.read_text())["findings"]
    reds = [f"{x['id']} {x['verdict']}" for x in rows if x["verdict"] in RED_VERDICTS]
    return {
        "key": key,
        "exit": r.returncode,
        "findings": len(rows),
        "reds": reds,
        "inapplicable": sum(1 for x in rows if x["verdict"] == "INAPPLICABLE"),
        "stderr": r.stderr,
        "report": report,
        "admitted": admit.is_file(),
        "token_in_tree": token_in_tree(out),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--delvec", help="the `delvec` under test (default: this tree's build)")
    ap.add_argument("--prefabs", required=True, help="the generated gallery prefab directory")
    ap.add_argument(
        "--work",
        default=str(REPO / "validation" / "gallery-stageable"),
        help="where campaigns, build trees, reports and admission tokens are written",
    )
    ap.add_argument(
        "--ledger",
        type=Path,
        default=REPO / "docs" / "playtest-findings.json",
        help="the findings ledger the gate adjudicates against",
    )
    args = ap.parse_args()

    prefabs = Path(args.prefabs)
    if not prefabs.is_dir():
        die(
            f"no prefab directory at `{prefabs}`. The gallery's piece is GENERATED: `cargo run "
            "--manifest-path prefabs/gallery-generator/Cargo.toml -- <dir> --skins gallery/skins "
            "--design gallery/design`."
        )
    if not args.ledger.is_file():
        die(f"no findings ledger at `{args.ledger}`")
    work = Path(args.work)
    delvec = Path(args.delvec) if args.delvec else REPO / "target" / "debug" / "delvec"

    rows = ledger_rows()
    if not rows:
        die(
            f"`{BASELINE.relative_to(REPO)}` holds zero build rows, so this gate would judge "
            "nothing. A domain with no points is a finding, not a pass."
        )
    points = {p for p, _ in rows}
    declared = domain_points()
    missing = uncovered_points(rows, declared)
    if missing:
        die(
            "the build ledger does not cover the domain: "
            + ", ".join(missing)
            + " exist in `gallery/` and are recorded by no baseline row, so judging the ledger "
            "would judge fewer points than the gallery has. Regenerate the baseline "
            "(`python3 tools/gallery-baseline.py --write`)."
        )

    print(
        f"gallery stageability: {len(rows)} build row(s) of {len(rows)} in "
        f"`{BASELINE.relative_to(REPO)}`, over {len(points)} point(s) of the "
        f"{len(declared)} the gallery directory declares ({', '.join(declared)}).",
        flush=True,
    )

    results = []
    for point, lang in rows:
        src, out = build_point(delvec, prefabs, work, point, lang)
        r = judge(args.ledger, work, point, lang, src, out)
        results.append(r)
        mark = "REFUSED" if r["exit"] != 0 else "stageable"
        print(
            f"  {r['key']:<22} {mark:<10} {len(r['reds'])} red / {r['findings']} finding(s), "
            f"{r['inapplicable']} inapplicable",
            flush=True,
        )
        for red in r["reds"]:
            print(f"      {red}", flush=True)

    if len(results) != len(rows):
        die(f"judged {len(results)} point(s) of {len(rows)} — the walk did not finish")

    refused = [r for r in results if r["exit"] != 0]
    leaked = [r for r in results if r["token_in_tree"]]

    if leaked:
        die(
            "these build trees carry an admission token: "
            + ", ".join(r["key"] for r in leaked)
            + f". The gallery is never staged (spec-0039 §2), so no `{ADMISSION}` may be left "
            "where a staging path would find one."
        )

    if refused:
        for r in refused:
            sys.stderr.write(r["stderr"])
        print(
            f"\ncheck-gallery-stageable: REFUSED — {len(refused)} of {len(results)} gallery build "
            "point(s) carry a finding class with no live, binding check. The remedy is the one "
            "the gate names, per row: build the general form, or give the point the content its "
            "own declarations imply. Never the ledger, and never this gate.",
            file=sys.stderr,
        )
        return 1

    print(
        f"\nall {len(results)} of {len(rows)} gallery build point(s) are stageable: every finding "
        "class each one contains carries a live, binding check on that build."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
