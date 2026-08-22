#!/usr/bin/env python3
"""The gallery's emission baseline and its expected-warnings ledger (spec-0039 §5).

## What is committed, and what is not

Never the output tree — only, per build in the domain, a copy of that build's
`manifest.json` (the compiler's SHA-256 index over its inputs and every output
file) under `gallery/baseline/`, plus one `warnings.json` ledger and one
`delta.json` review artifact.

Every manifest copy carries a **header**: the delvec version, the `dsl_version`,
the gallery source-tree hash and the generator-input hash. The comparison
asserts the header FIRST and refuses with its own message when it disagrees,
instead of diffing noise — a baseline taken by a different delvec is not a
regression, it is a measurement of two different things, and reporting it as a
file-by-file diff buries the one fact the reader needs.

## Three verdicts from one mismatch, because they mean different things

- **no emitted path moved at all**, and the manifest differs anyway — a
  **declared input moved**. A manifest is not its outputs: `content_sha`, the
  pinned versions and the whole `inputs` index are SIBLINGS of `outputs`, so a
  manifest can differ while every emitted byte is identical. Reported first
  because it is the one the other two cannot describe: a pure content re-pin
  came out as a determinism finding that then listed zero differing paths,
  because the delta walks `outputs` and the thing that moved was next to it.
- the change touches an input that can reach emission (`EMISSION_INPUTS`) — an
  **emission change**: regenerate the baseline in this change, or explain the
  drift;
- it touches none of them — a **determinism finding** (ADR-0006), named as such.
  The baseline is thereby a standing cross-machine determinism probe, for free.

## The two arms are ONE rule (CLAUDE.md: the defect belongs to the PAIR)

`--write` refuses a noise commit by asking exactly the question the verify arm
asks — `baseline_matches`: *do the three recorded documents already equal what
this run measured?* — and nothing else. So **`--write` refuses if and only if
verify passes**, and no tree can be refused by both arms.

That is a repair to the pair rather than to either half. The previous shape
ENUMERATED what to compare — emission, then the header, then, one fix later, the
warning ledger — and was unsatisfiable for anything the enumeration had not met.
It met one: a pure content re-pin moves the recorded `content_sha`, which is in
none of the three, so verify refused the tree and `--write` refused the
regeneration it had just prescribed. Enumerating is the defect; comparing the
documents themselves cannot go stale, and a fourth recorded document joins both
arms by being recorded.

The guard also runs BEFORE the write, so a refusal leaves the tree untouched and
the exit status and the effect agree.

## The warnings ledger

Judgement-tier warnings are legitimate; *drifting* warnings are not. The emitted
warning set must equal the committed ledger exactly, so "still green" can never
quietly absorb "warns differently now".

## Binding count

Every run states builds compared, output paths compared, warning rows checked,
and recorded manifest values compared — the last being the denominator for the
third verdict, which had no stated binding at all while it was invisible.
Comparing zero of any of them is a red: a baseline that matched nothing is
vacuous, not a pass.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gallery_domain  # noqa: E402
from gallery_domain import build_id, overlays  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
GALLERY = REPO / "gallery"
BASELINE = GALLERY / "baseline"

# The builds that make up the domain: the primary in every declared language,
# plus each overlay in `en`. Derived, never listed — a listed set goes stale the
# first time a language or an overlay is added, and goes stale silently.
# A warning line: code, stage, then the POINTER, which runs to the first
# `: ` that ends it. The pointer is not a single token — a build-tier
# diagnostic points at a phase of the build ("packtest watch coverage"),
# not at a JSON pointer — and a `(\S+)` here silently dropped every one of
# them, so the ledger that claims the emitted warning set must equal it
# exactly was blind to a whole class of warning. `ANY_WARNING_RE` is what
# stops that being silent again: a line that announces itself as a warning
# and does not parse is a red, never a skip.
WARNING_RE = re.compile(r"^(DW\d{4}) \[warning\] (\S+) (.+?): ")
ANY_WARNING_RE = re.compile(r"^(DW\d{4}) \[warning\] ")

# What this baseline RECORDS, in the order a reader meets it. Both arms compare
# exactly this set, so a fourth recorded document joins the verify comparison AND
# the noise-commit guard by being added here — which is the whole reason the set
# is a constant rather than three names written out at each site.
#
# `delta.json` is deliberately absent: it is the review artifact OF a write, not
# a record of the tree, so a rewrite that moves only `delta.json` has recorded
# nothing. It is also derived from the other two, which would make it a second
# vote for a fact already counted.
RECORDED = ("header.json", "manifests.json", "warnings.json")

# The paths whose content can reach a byte the compiler emits OR records, so a
# manifest mismatch alongside a change to one of them is an ordinary consequence
# rather than a violation of ADR-0006. Membership is decided by that question and
# by nothing else.
#
# `versions.toml` is here because the compiler READS it at build time and records
# `content_sha`, `dsl_version` and `mc_version` out of it into every manifest
# (see its `[content]` block). Without it a pure content re-pin was reported as a
# determinism finding — a change to a declared input, named as the one thing it
# is not.
#
# Widening this set makes nothing easier to ship: BOTH verdicts refuse, so it
# decides only what the reader is told the finding IS.
EMISSION_INPUTS = ("gallery/", "crates/", "versions.toml")


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def write_canonical(path: Path, obj) -> None:
    """Write one baseline file in `delvec fmt` canonical form.

    `ensure_ascii=False` is the load-bearing argument and it is why this is a
    function rather than four call sites. Python's default escapes every
    non-ASCII character, so an em-dash inside a warning pointer landed in
    `warnings.json` as `\\u2014` — and that single habit was the ONLY reason any
    of `gallery/baseline/` was outside canonical form, which in turn was the only
    reason anyone could argue that a generated artifact needs an exemption from
    `tools/check-json-canonical.py`. A generated file that is already canonical
    needs no exemption, and cannot be used as cover by a file that merely never
    was (CLAUDE.md: an opt-out must be secured by a property the defect cannot
    supply — so the better move is to leave no opt-out to secure).

    The header does not hash `baseline/` (see `header`), so making these bytes
    canonical moves no header field and is not a determinism finding to anyone
    reading the diff afterwards. The three sibling files were already canonical;
    only `warnings.json` carried non-ASCII at all.
    """
    path.write_text(
        json.dumps(obj, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def tree_hash(root: Path, skip: set[str]) -> str:
    """A deterministic hash of a source tree: relative path AND content, in path order.

    Both halves, and the pairing is the point. Hashing only the file bytes makes
    a rename invisible; hashing the paths alongside a `shasum` listing — the
    shape this project has been bitten by — hashes the PATHS as content and
    reports two identical trees under different names as different.
    """
    h = hashlib.sha256()
    for p in sorted(root.rglob("*")):
        if not p.is_file():
            continue
        rel = p.relative_to(root).as_posix()
        if any(rel == s or rel.startswith(s + "/") for s in skip):
            continue
        h.update(rel.encode())
        h.update(b"\0")
        h.update(p.read_bytes())
        h.update(b"\0")
    return h.hexdigest()


def delvec_versions(delvec: Path) -> dict:
    r = subprocess.run([str(delvec), "--version"], capture_output=True, text=True)
    if r.returncode != 0:
        die(f"`delvec --version` exited {r.returncode}")
    # `delvec x.y.z, dsl a.b.c, mc x.y.z`
    parts = dict(
        (k.strip(), v.strip())
        for k, v in (p.split(" ", 1) for p in r.stdout.strip().split(", "))
    )
    return {"delvec": parts.get("delvec", ""), "dsl": parts.get("dsl", "")}


def declared_languages() -> list[str]:
    world = json.loads((GALLERY / "world.json").read_text())
    return list(world["content"].get("languages") or [])


def materialise(overlay: str | None, dest: Path) -> None:
    """One point of the domain as a campaign directory — `gallery_domain` decides what that means."""
    gallery_domain.materialise(dest, GALLERY / "overlays" / overlay if overlay else None)


def build_one(delvec: Path, prefabs: Path, overlay: str | None, lang: str, work: Path):
    """One build of the domain: `(manifest, warning rows)`."""
    src = work / f"src-{overlay or 'primary'}-{lang}"
    out = work / f"out-{overlay or 'primary'}-{lang}"
    materialise(overlay, src)
    r = subprocess.run(
        [str(delvec), "--lang", lang, "build", str(src), "-o", str(out), "--prefabs", str(prefabs)],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        die(
            f"build `{overlay or 'primary'}` in `{lang}` exited {r.returncode}. "
            "A baseline is only meaningful over a green build.\n" + r.stdout + r.stderr
        )
    manifest = json.loads((out / "manifest.json").read_text())
    rows = []
    for line in (r.stdout + r.stderr).splitlines():
        stripped = line.strip()
        m = WARNING_RE.match(stripped)
        if m:
            rows.append({"code": m.group(1), "stage": m.group(2), "pointer": m.group(3)})
        elif ANY_WARNING_RE.match(stripped):
            # Never `continue`. A warning the ledger cannot read is a warning the
            # ledger cannot compare, and dropping it makes the set-equality claim
            # false in exactly the direction that reads as a clean pass.
            die(
                "a warning line was emitted that this ledger cannot parse, so it "
                "would have been dropped from the comparison in silence — the set "
                "equality this file claims would be false and green:\n"
                f"  {stripped[:200]}"
            )
    return manifest, rows


def coverage_counts(delvec: Path) -> dict:
    """Units total / bound / refusal-proven / unaccounted, for the header.

    spec-0039 §6 asks for the deterministic counts in the baseline header so
    growth is a diffable number rather than a complaint. It also makes the
    unaccounted count a **committed fact**, which is what lets
    `check-required-contexts.py` EVALUATE the gallery job's advisory entry
    rather than recite its expiry condition — a hatch whose end state only a
    comment describes is a hatch that never ends.

    Derived from the same enumeration the coverage gate uses, never a second one.
    """
    sys.path.insert(0, str(REPO / "tools"))
    from gallery_units import Binder, Enumerator, stage_files

    r = subprocess.run(
        [str(delvec), "schema", "--stage", "all"], capture_output=True, text=True
    )
    if r.returncode != 0:
        die(f"`delvec schema --stage all` exited {r.returncode}")
    export = json.loads(r.stdout)
    e = Enumerator(export)
    units = e.run()
    if not units:
        die("the schema export enumerated ZERO units; the header would record a lie")

    def bind_dir(root: Path, label: str, into: set) -> None:
        b = Binder(e)
        for stage, fn in stage_files(export).items():
            f = root / fn
            if f.is_file():
                b.walk(export[stage], json.loads(f.read_text()), label)
        into |= set(b.bound) & set(units)

    bound: set = set()
    bind_dir(GALLERY, "primary", bound)
    work = Path(tempfile.mkdtemp(prefix="gallery-header-"))
    try:
        for name in overlays():
            dest = work / name
            materialise(name, dest)
            bind_dir(dest, f"overlay:{name}", bound)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    proven: set = set()
    pdir = GALLERY / "probes"
    if pdir.is_dir():
        for d in sorted(x for x in pdir.iterdir() if x.is_dir()):
            m = json.loads((d / "probe.json").read_text())
            proven |= {u for u in (m.get("units") or []) if u in units}

    return {
        "units_total": len(units),
        "units_bound": len(bound),
        "units_refusal_proven": len(proven - bound),
        "units_unaccounted": len(set(units) - bound - proven),
    }


def header(delvec: Path, prefabs: Path) -> dict:
    v = delvec_versions(delvec)
    return {
        "coverage": coverage_counts(delvec),
        "delvec_version": v["delvec"],
        "dsl_version": v["dsl"],
        # `baseline/` is this file's own output, and `README.md` is prose that
        # cannot reach a byte of emission — hashing either would make an ordinary
        # documentation edit demand a baseline regeneration, and a gate that
        # fires on changes it cannot possibly be about is a gate people learn to
        # discharge without reading.
        "gallery_source_sha256": tree_hash(GALLERY, {"baseline", "README.md"}),
        "generator_input_sha256": tree_hash(prefabs, set()),
    }


def classify() -> str:
    """Emission change, or determinism finding — decided by what the diff touches.

    Took the differing paths as an argument and never read them; the argument
    said the verdict depended on what moved, and it depends only on what the
    change touched.
    """
    r = subprocess.run(
        ["git", "-C", str(REPO), "diff", "--name-only", "origin/main...HEAD"],
        capture_output=True,
        text=True,
    )
    changed = r.stdout.splitlines() if r.returncode == 0 else []
    touched = [c for c in changed if any(under(c, p) for p in EMISSION_INPUTS)]
    if touched:
        return "emission-change"
    return "determinism-finding"


def under(path: str, prefix: str) -> bool:
    """`path` IS `prefix`, or lies inside it — never merely starts with its text.

    A bare `startswith("versions.toml")` also claims `versions.toml.bak`, and a
    prefix rule that quietly widens is how a determinism finding would get
    renamed by a file nobody meant to name.
    """
    return path == prefix.rstrip("/") or path.startswith(prefix.rstrip("/") + "/")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--delvec", default=str(REPO / "target/release/delvec"))
    ap.add_argument("--prefabs", required=True)
    ap.add_argument("--write", action="store_true", help="regenerate the baseline")
    args = ap.parse_args()

    delvec, prefabs = Path(args.delvec), Path(args.prefabs)
    if not delvec.is_file():
        die(f"no delvec at `{delvec}` — build one with `cargo build --release -p delvec`")

    builds = [(None, "en")] + [(None, l) for l in declared_languages()]
    builds += [(o, "en") for o in overlays()]

    work = Path(tempfile.mkdtemp(prefix="gallery-baseline-"))
    manifests, warnings = {}, {}
    try:
        for overlay, lang in builds:
            m, rows = build_one(delvec, prefabs, overlay, lang, work)
            manifests[build_id(overlay, lang)] = m
            warnings[build_id(overlay, lang)] = rows
    finally:
        shutil.rmtree(work, ignore_errors=True)

    hdr = header(delvec, prefabs)
    measured = {"header": hdr, "manifests": manifests, "warnings": warnings}
    n_paths = sum(len(m.get("outputs") or m.get("files") or {}) for m in manifests.values())
    n_warn = sum(len(v) for v in warnings.values())
    n_fields = field_count(manifests)
    print(
        f"gallery baseline: {len(manifests)} build(s), {n_paths} emitted path(s), "
        f"{n_warn} warning row(s), {n_fields} recorded manifest value(s)."
    )
    if not manifests or n_paths == 0 or n_fields == 0:
        die(
            "the baseline compared ZERO builds, ZERO emitted paths or ZERO "
            "recorded manifest values. A baseline that matched nothing is "
            "vacuous, not a pass."
        )

    committed = read_recorded()

    if args.write:
        # The noise-commit guard, and it asks ONE question: do the documents this
        # baseline records already equal what this run measured? That is exactly
        # the question the verify arm asks (`baseline_matches`), so this arm
        # refuses if and only if that arm passes, and no tree is refused by both.
        #
        # It replaces an ENUMERATION — empty output delta, unchanged header,
        # unchanged warning ledger — which was correct about the three things its
        # authors had met and silent about the fourth. `content_sha` is a sibling
        # of `outputs`, in no header and no warning row, so a pure content re-pin
        # moved something this baseline records while all three qualifiers held:
        # verify refused the tree and prescribed `--write`, and `--write` refused
        # the regeneration as noise. CLAUDE.md: when one gate's prescription is
        # another gate's refusal, the defect belongs to the PAIR — so the repair
        # is one shared question, not a fourth qualifier.
        #
        # It runs BEFORE the write. The old order wrote all four files and then
        # refused, so the exit status and the effect on disk disagreed and a
        # caller under `set -e` could not tell; worse, a genuinely-noise rewrite
        # blanked `delta.json`, destroying the review artifact of the last real
        # change, and the operator's undo is the `git checkout` this project has
        # already been bitten by. Guarding first makes a refusal a no-op.
        if baseline_matches(committed, measured):
            die(
                "nothing this baseline RECORDS moved: header, manifests and "
                "warning ledger already equal what this run measured, so this "
                "rewrite is a noise commit. A baseline update is never split "
                "from the change that caused it (§5). Nothing was written."
            )
        old = committed["manifests"] if committed else {}
        old_warnings = committed["warnings"] if committed else {}
        BASELINE.mkdir(parents=True, exist_ok=True)
        write_canonical(BASELINE / "header.json", hdr)
        write_canonical(BASELINE / "manifests.json", manifests)
        write_canonical(BASELINE / "warnings.json", warnings)
        delta = compute_delta(old, manifests)
        write_canonical(BASELINE / "delta.json", delta)
        moved = warning_delta(old_warnings, warnings) if old_warnings != warnings else []
        fields = manifest_field_delta(old, manifests) if old else []
        print(
            f"wrote {BASELINE}: {len(delta['added'])} added, "
            f"{len(delta['removed'])} removed, {len(delta['changed'])} changed path(s); "
            f"{len(moved)} warning row(s) at a new count; "
            f"{len(fields)} recorded manifest value(s) moved."
        )
        for line in moved + fields:
            print(line)
        return 0

    if committed is None:
        missing = ", ".join(f"`gallery/baseline/{n}`" for n in RECORDED if not (BASELINE / n).is_file())
        die(f"the baseline is incomplete — {missing} missing. Run this with `--write`.")
    if not baseline_matches(committed, measured):
        report_mismatch(committed, measured)  # never returns
    print("baseline: header, manifests and warning ledger all match.")
    return 0


def baseline_matches(committed: dict | None, measured: dict) -> bool:
    """The ONE question both arms ask: does the committed baseline already record
    what this run measured?

    Verify passes exactly when this is true; `--write` refuses exactly when it is
    true. That is the whole of the pair's agreement, and it lives in one function
    so it cannot become two enumerations that drift apart — which is what it was,
    and what made a pure content re-pin unsatisfiable in both directions.

    A missing baseline is not a match: there is nothing to have recorded
    anything, so verify reds (asking for `--write`) and `--write` lands.
    """
    return committed is not None and committed == measured


def read_recorded() -> dict | None:
    """The documents `gallery/baseline/` records, or `None` if ANY is absent.

    All of `RECORDED` or none of it. A partial baseline cannot answer whether
    anything moved, and answering anyway would be a guard reporting a fact about
    a smaller world than the one it claims to cover.
    """
    if not all((BASELINE / n).is_file() for n in RECORDED):
        return None
    return {
        "header": json.loads((BASELINE / "header.json").read_text()),
        "manifests": json.loads((BASELINE / "manifests.json").read_text()),
        "warnings": json.loads((BASELINE / "warnings.json").read_text()),
    }


def report_mismatch(committed: dict, measured: dict) -> None:
    """Name the mismatch and refuse. NEVER RETURNS.

    Reached only when `baseline_matches` is false, and it must die on every path
    through it: a fall-through here is a verify that reds nothing while `--write`
    refuses to regenerate — the pair's unsatisfiable state re-entering through
    the reporting rather than through the rule.
    """
    hdr, manifests = measured["header"], measured["manifests"]
    warnings = measured["warnings"]
    if committed["header"] != hdr:
        diffs = [
            f"  {k}: baseline `{committed['header'].get(k)}` vs this tree `{hdr.get(k)}`"
            for k in sorted(set(committed["header"]) | set(hdr))
            if committed["header"].get(k) != hdr.get(k)
        ]
        die(
            "the committed baseline was taken over DIFFERENT INPUTS, so a "
            "file-by-file diff would report noise rather than a finding:\n"
            + "\n".join(diffs)
            + "\nRegenerate it with `--write` in the same change that moved them."
        )
    if committed["manifests"] != manifests:
        delta = compute_delta(committed["manifests"], manifests)
        differing = delta["added"] + delta["removed"] + delta["changed"]
        fields = manifest_field_delta(committed["manifests"], manifests)
        if not differing:
            # The third verdict. Every emitted path is present and byte-identical
            # and the manifest still differs, so this is neither an emission
            # change nor a determinism finding — a DECLARED INPUT moved, and the
            # manifest records it beside the outputs rather than inside them.
            # Said first, and said in its own words, because the two verdicts
            # below cannot describe it: reported as a determinism finding, this
            # printed the gravest message this file has over a list of zero paths.
            die(
                "A DECLARED INPUT MOVED: every emitted path is present and "
                "byte-identical, and the manifest differs anyway — a manifest "
                "records more than its outputs. This is NOT a determinism "
                "finding and NOT an emission change; the baseline has gone stale "
                "against an input this change declares. Regenerate it with "
                "`--write` in this same change.\n" + "\n".join(fields)
            )
        extra = ("\nAnd these recorded values moved with it:\n" + "\n".join(fields)) if fields else ""
        if classify() == "determinism-finding":
            die(
                "DETERMINISM FINDING (ADR-0006): this change touches none of "
                f"{', '.join('`' + p + '`' for p in EMISSION_INPUTS)}, and the "
                "gallery's emission moved anyway. Same DSL + same seed must give "
                "byte-identical output.\n"
                + "\n".join(f"  {p}" for p in differing)
                + extra
            )
        die(
            "EMISSION CHANGE: the gallery's emitted bytes moved. Regenerate the "
            "baseline in this same change (`--write`) and confirm every path "
            "class below is a consequence this change claims to have.\n"
            + "\n".join(f"  {p}" for p in differing)
            + extra
        )
    if committed["warnings"] != warnings:
        die(
            "the emitted warning set no longer equals the committed ledger. A new "
            "or vanished warning is a red: 'still green' must never quietly "
            "absorb 'warns differently now' (§4.3). Regenerate with `--write` "
            "only once every row below is a consequence this change claims to "
            "have.\n" + "\n".join(warning_delta(committed["warnings"], warnings))
        )
    die(
        "the committed baseline differs from what this run measured, and no "
        "component comparison could say how. That is this file failing to "
        "REPORT, never a pass — and it is the one state in which `--write` is "
        "still the right next act."
    )


def warning_delta(old: dict, new: dict) -> list[str]:
    """Every warning row whose COUNT moved, named, with both counts.

    Its sibling `compute_delta` lists every differing emitted path and the
    header branch lists every differing input; this branch used to list nothing,
    so a red here meant rebuilding the gallery to find out what it was about.

    Counted, never set-compared, and that distinction is the whole point rather
    than a detail: the first live difference this helper met was a warning the
    engine had been emitting TWICE and now emits once — six rows across six
    builds, one per build. As sets the two ledgers are identical, so a
    set-shaped delta printed "no row added or removed" beside a message saying
    they differ. A set is a lossy reading of a list, and the loss is exactly the
    class of change a duplicated pass produces.
    """
    def counted(m: dict) -> dict:
        out: dict = {}
        for bid, rs in m.items():
            for r in rs:
                key = (bid, json.dumps(r, sort_keys=True))
                out[key] = out.get(key, 0) + 1
        return out

    before, now = counted(old), counted(new)
    lines = []
    for key in sorted(set(before) | set(now)):
        was, is_ = before.get(key, 0), now.get(key, 0)
        if was == is_:
            continue
        bid, row = key
        mark = "+" if is_ > was else "-"
        lines.append(f"  {mark} {bid}  x{was} -> x{is_}  {row}")
    if not lines:
        # Same rows at the same counts: what moved is the shape of the
        # containing document — a build id gained or lost an empty list. Say so,
        # because an empty delta beside a message asserting a difference reads as
        # the check being wrong.
        lines.append(
            f"  (every row and count identical; the build id set moved: "
            f"{sorted(set(old))} -> {sorted(set(new))})"
        )
    return lines


def field_count(manifests: dict) -> int:
    """How many recorded-but-not-emitted manifest values this run compares.

    The denominator for the third verdict, stated for the same reason the other
    two are: a comparison that examined nothing is vacuous, not a pass — and this
    one examined nothing at all for as long as it did not exist. Counted at the
    leaf, so each `inputs` entry is its own recorded value, because that is the
    granularity `manifest_field_delta` names them at.
    """
    n = 0
    for man in manifests.values():
        for k, v in man.items():
            if k == "outputs":
                continue
            n += len(v) if isinstance(v, dict) else 1
    return n


def manifest_field_delta(old: dict, new: dict) -> list[str]:
    """Every recorded-but-not-emitted manifest value whose content moved, named.

    A manifest is not its outputs. `content_sha`, the pinned `delvec_version` /
    `dsl_version` / `mc_version`, `campaign_id`, `resource_pack_sha1` and the
    whole `inputs` index are SIBLINGS of `outputs` — so a manifest can differ
    while every emitted byte is identical, and `compute_delta`, which walks
    `outputs`, then correctly reports nothing at all. That empty list beside a
    real mismatch is how a pure content re-pin was announced as a determinism
    finding over zero differing paths.

    Named rather than dumped: two seven-build manifests printed whole is not a
    reading anyone takes a finding from. `inputs` is descended one level so an
    entry is named by the file it indexes, which is what the compiler recorded.
    """
    lines: list[str] = []
    for bid in sorted(set(old) | set(new)):
        if bid not in new:
            lines.append(f"  - build `{bid}`: in the baseline, not built by this run")
            continue
        if bid not in old:
            lines.append(f"  + build `{bid}`: built by this run, not in the baseline")
            continue
        a, b = old[bid], new[bid]
        for k in sorted((set(a) | set(b)) - {"outputs"}):
            av, bv = a.get(k), b.get(k)
            if av == bv:
                continue
            if isinstance(av, dict) and isinstance(bv, dict):
                for sub in sorted(set(av) | set(bv)):
                    if av.get(sub) != bv.get(sub):
                        lines.append(
                            f"  ~ {bid}.{k}[{sub}]: baseline `{av.get(sub)}` "
                            f"vs this tree `{bv.get(sub)}`"
                        )
                continue
            lines.append(f"  ~ {bid}.{k}: baseline `{av}` vs this tree `{bv}`")
    return lines


def compute_delta(old: dict, new: dict) -> dict:
    """Every added/removed/changed emitted path, grouped by output class.

    The review artifact, and it opens with the question its reader answers: *is
    every path class listed here a consequence this change claims to have?*

    It walks `outputs` and NOTHING else, deliberately — it is about emitted
    bytes. Every other thing a manifest records is `manifest_field_delta`'s, and
    the two are read together: this one empty while the manifests differ is not
    "no difference", it is "the difference is not in emission".
    """
    def files(m: dict) -> dict:
        out = {}
        for bid, man in m.items():
            for path, sha in (man.get("outputs") or man.get("files") or {}).items():
                out[f"{bid}:{path}"] = sha
        return out

    a, b = files(old), files(new)
    added = sorted(set(b) - set(a))
    removed = sorted(set(a) - set(b))
    changed = sorted(k for k in set(a) & set(b) if a[k] != b[k])

    def klass(p: str) -> str:
        tail = p.split(":", 1)[1]
        for pre, name in (
            ("datapack/data/", "datapack function"),
            ("packtest-datapack/", "PackTest"),
            ("creator-datapack/", "creator overlay"),
            ("validation/", "validation ledger"),
            ("server/", "server config"),
            ("structures/", "structure"),
        ):
            if tail.startswith(pre):
                return name
        return "other"

    classes: dict[str, int] = {}
    for p in added + removed + changed:
        classes[klass(p)] = classes.get(klass(p), 0) + 1
    return {"added": added, "removed": removed, "changed": changed, "classes": classes}


if __name__ == "__main__":
    raise SystemExit(main())
