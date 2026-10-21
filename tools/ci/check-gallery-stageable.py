#!/usr/bin/env python3
"""Every build this repository produces can be served — asked on every push.

## The gap this closes

`tools/creator/staging-gate.py` decides whether a built delve may be put in front of a
player, and until this ran its only consumer was `tools/creator/playtest-server.sh`:
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
true. The gate is run `--coverage-only`: it judges the ledger and writes no
admission token at all — a build is admitted only with its bot ladder and
written-world records, which a push does not make here — and each build tree is
then asserted to hold none. Judging a build is a question about coverage; it is
not an act of handing anything to anybody.

## The domain is enumerated from the objects, never typed

The points come from `gallery_domain.points()` — the one enumeration rule, read
off the gallery itself: the primary in `en` and in every language
`gallery/world.json` declares, plus each directory under `gallery/overlays/` in
`en`. A hand-written list here would be a second opinion on what the gallery is,
and it would go stale silently the first time an overlay was added: the job
would keep passing while judging less than the ladder builds, which is this
project's `unbound` vacuity wearing a green tick. Probes are counted and named
as not points: they exist to be refused, and `check-gallery-coverage.py` runs
them.

Each point is built by `tools/ci/gallery-build.py`, which proves its tree
against the manifests the same run's baseline measured (`--manifests`); a point
that run did not measure is refused there.

## Binding count

Every run states the points judged against the denominator the gallery
declares — languages, overlays, and probes excluded by name. Judging zero
points, or fewer points than the domain holds, is a red — a gate that examined
nothing is not a pass.
"""

from __future__ import annotations

import argparse
import contextlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
import parallel  # noqa: E402
import gallery_domain  # noqa: E402
from gallery_domain import build_id  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
GALLERY_BUILD = REPO / "tools" / "ci" / "gallery-build.py"
STAGING_GATE = REPO / "tools" / "creator" / "staging-gate.py"
PRIMARY = "primary"
# The one point that is judged to be red, and exactly how. The `demo-level`
# overlay declares `demo_level: true` and the gallery is not a queued demo
# (`docs/demo-levels.md` names campaigns, and the gallery is none), so the
# gate reads that declaration as one `DEMO-UNQUEUED` row. The point is bound
# only when its reds are EXACTLY these: the declaration removed (no red), or
# any other red beside it, is a refusal, so the element stays bound and the
# expectation can never absorb a second finding.
EXPECTED_REDS = {"demo-level.en": ["demo-declaration DEMO-UNQUEUED"]}
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


def domain_rows() -> list[tuple[str, str]]:
    """The `(point, lang)` pairs of the domain, from `gallery_domain.points()`.

    `build_id` owns the key format; the point name used here is the one it
    encodes, so a pair this tool builds re-encodes to the key the baseline
    run measured it under.
    """
    out: list[tuple[str, str]] = []
    for overlay, lang in gallery_domain.points():
        point = overlay or PRIMARY
        assert build_id(overlay, lang) == f"{point}.{lang}"
        out.append((point, lang))
    return out


def denominator() -> dict:
    """What the gallery declares, counted from the objects: languages, overlays, probes."""
    return {
        "languages": 1 + len([lang for lang in gallery_domain.languages() if lang != "en"]),
        "overlays": len(gallery_domain.overlays()),
        "probes": len(gallery_domain.probes()),
    }


def token_in_tree(out: Path) -> bool:
    """Whether a build tree carries an admission token (spec-0039 §2: never)."""
    return (out / ADMISSION).is_file()


def run_build(
    delvec: Path, prefabs: Path, manifests: Path, work: Path, point: str, lang: str
) -> tuple[Path, Path, subprocess.CompletedProcess]:
    """Materialise and compile one point, through the one tool that owns that act.

    It says nothing, so it may run on a worker of the shared pool; `built` reads it.

    `--src`/`--out` are named rather than defaulted because the default carries
    the point and not the language, so `primary.en` and `primary.zh-cn` would
    compile over each other and this gate would judge one tree twice. The same
    naming is what gives every concurrent point directories of its own.
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
            "--manifests",
            str(manifests),
            "--src",
            str(src),
            "--out",
            str(out),
        ],
        capture_output=True,
        text=True,
    )
    return src, out, r


def built(point: str, lang: str, src: Path, out: Path, r: subprocess.CompletedProcess) -> tuple[Path, Path]:
    """`(src, out)` of a finished build, or the refusal a failed one earns."""
    if r.returncode != 0:
        sys.stderr.write(r.stderr)
        die(
            f"`gallery-build.py --point {point} --lang {lang}` exited {r.returncode}. A point "
            "that does not build is not a point anything can be served from, and this gate has "
            "nothing to judge."
        )
    return src, out


def run_judge(
    ledger: Path, work: Path, point: str, lang: str, src: Path, out: Path
) -> subprocess.CompletedProcess:
    """The staging gate's run on one point, saying nothing; its files are named by the point."""
    key = build_id(None if point == PRIMARY else point, lang)
    report = work / "reports" / f"{key}.md"
    verdicts = work / "reports" / f"{key}.json"
    return subprocess.run(
        [
            sys.executable,
            str(STAGING_GATE),
            "--campaign",
            str(src),
            "--build",
            str(out),
            "--ledger",
            str(ledger),
            "--coverage-only",
            "--report",
            str(report),
            "--json",
            str(verdicts),
        ],
        capture_output=True,
        text=True,
    )


def judged(work: Path, out: Path, point: str, lang: str, r: subprocess.CompletedProcess) -> dict:
    """What the staging gate decided for one point, read from what it wrote."""
    key = build_id(None if point == PRIMARY else point, lang)
    report = work / "reports" / f"{key}.md"
    verdicts = work / "reports" / f"{key}.json"
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
        "expected_reds": EXPECTED_REDS.get(key, []),
        "inapplicable": sum(1 for x in rows if x["verdict"] == "INAPPLICABLE"),
        "stderr": r.stderr,
        "report": report,
        "token_in_tree": token_in_tree(out),
    }


def is_refused(r: dict) -> bool:
    """A point is refused unless it is judged exactly as declared: stageable, or red in exactly its expected rows."""
    if r["expected_reds"]:
        return r["reds"] != r["expected_reds"] or r["exit"] != 1
    return r["exit"] != 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--delvec", help="the `delvec` under test (default: this tree's build)")
    ap.add_argument("--prefabs", required=True, help="the generated gallery prefab directory")
    ap.add_argument(
        "--manifests",
        required=True,
        type=Path,
        help="the head manifests this run's `gallery-baseline.py` measured, handed to `gallery-build.py`",
    )
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
            "--manifest-path gallery/prefab-generator/Cargo.toml -- <dir> --skins gallery/skins "
            "--design gallery/design --textures gallery/textures "
            "--terrain gallery/overlays/site-plan/terrain`."
        )
    if not args.ledger.is_file():
        die(f"no findings ledger at `{args.ledger}`")
    work = Path(args.work)
    delvec = Path(args.delvec) if args.delvec else REPO / "target" / "debug" / "delvec"

    rows = domain_rows()
    den = denominator()
    expected = den["languages"] + den["overlays"]
    print(
        f"gallery stageability: {len(rows)} point(s) of the domain — the primary in "
        f"{den['languages']} language(s) plus {den['overlays']} overlay(s) in `en` = {expected}; "
        f"{den['probes']} probe(s) are refusal points and are not built here.",
        flush=True,
    )
    if not rows or len(rows) != expected:
        die(
            f"the domain enumerated {len(rows)} point(s) against the {expected} the gallery "
            "declares, so this gate would judge a different set than the ladder builds."
        )

    def run_point(row: tuple[str, str]):
        point, lang = row
        src, out, b = run_build(delvec, prefabs, args.manifests, work, point, lang)
        if b.returncode != 0:
            return src, out, b, None
        return src, out, b, run_judge(args.ledger, work, point, lang, src, out)

    # Each point builds and is judged on the shared pool (`tools/lib/parallel.py`)
    # in directories named for it, and is READ in the ledger's order, so every
    # line and the first refusal are the serial walk's.
    results = []
    runs = parallel.ordered(run_point, rows)
    with contextlib.closing(runs):
        for (point, lang), outcome in zip(rows, runs):
            src, out, b, j = outcome.get()
            built(point, lang, src, out, b)
            r = judged(work, out, point, lang, j)
            results.append(r)
            mark = "REFUSED" if is_refused(r) else ("expected-red" if r["expected_reds"] else "stageable")
            print(
                f"  {r['key']:<22} {mark:<10} {len(r['reds'])} red / {r['findings']} finding(s), "
                f"{r['inapplicable']} inapplicable",
                flush=True,
            )
            for red in r["reds"]:
                print(f"      {red}", flush=True)

    if len(results) != len(rows):
        die(f"judged {len(results)} point(s) of {len(rows)} — the walk did not finish")

    refused = [r for r in results if is_refused(r)]
    missing = sorted(set(EXPECTED_REDS) - {r["key"] for r in results})
    if missing:
        die(f"an expected-red point is not in the domain: {', '.join(missing)}")
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
        f"\nall {len(results)} of {len(rows)} gallery build point(s) are judged as declared "
        f"({sum(1 for r in results if r['expected_reds'])} with its expected red, the rest stageable): "
        "every finding class each one contains carries a live, binding check on that build."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
