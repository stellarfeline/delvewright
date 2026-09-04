#!/usr/bin/env python3
"""Every surface the DSL declares is written somewhere in the gallery (spec-0039).

## The defect this exists to end

A surface no campaign exercises is a surface **nothing has ever compiled end to
end**. That is not a hypothetical: on the tree this gate landed on, the entire
authored corpus — four campaigns and twenty-eight fixtures — bound 524 of 800
declared units, and the 276 it missed had never been written by anything, ever.
Two of them turned out to be unbuildable at any version the moment somebody
tried; nobody had, for as long as they had existed.

The engine's only real build gate before this one pointed at the **content
repo**, at a pinned SHA. That pin lags the engine by construction, so it
structurally cannot cover a surface landed in the pull request under review —
and the campaigns behind it are approved creative artifacts no engine change may
edit. The gallery is engine-owned and same-repo, so an element lands in the same
commit as the surface it exercises.

## The verdict

Every unit is one of exactly two things (§3 — there is no third kind, and no
free-text exemption):

- **bound** — written somewhere in the binding domain (primary + overlays);
- **refusal-proven** — a committed probe writes it and `delvec validate` refuses
  it with the ledgered code at the gallery's `dsl_version`.

A probe may also declare **no unit at all**, and then it is not an exemption but
a **refusal demonstration**: a document the engine says no to, committed so that
the refusal is re-run rather than remembered. The distinction is real and worth
the two lines it costs. Some rules are about a *combination* of surfaces both of
which are perfectly writable — `DW0839` refuses a campaign that carries `areas[]`
AND a site plan, and every unit involved is bound — so there is no unit for such
a probe to discharge, and demanding one would either force it to claim a unit the
domain already binds (which this tool refuses by name, correctly) or leave the
rule with no committed artifact at all. Such a probe is held to the same two
obligations as any other: it must be refused, and it must be refused with the
code it names.

Anything else is a **red naming the unit**. The refusal half is the
vacuity-mode-6 hardening: the escape hatch demands a machine-produced refusal,
which "nobody authored it" — the defect this gate exists to catch — cannot
supply.

## Legibility

Coverage that is complete and unreadable satisfies the tool and fails the
artifact. `--index` writes a reader-facing map from every element to the units it
binds and back, and the refusal probes are rendered with the *reason* the engine
refuses them, because a probe is the clearest demonstration the gallery can make
of what the engine checks — each one is a thing the engine correctly says no to.

## Binding count

Every run prints units enumerated, bound, refusal-proven, and the number of
authored documents walked. **Enumerating zero units is a red** and so is walking
zero documents: a gate that matched nothing is vacuous, not a pass (CLAUDE.md).
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
import gallery_domain  # noqa: E402
from delvec_bin import resolve as resolve_delvec  # noqa: E402
from gallery_units import Binder, Enumerator, stage_files  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
GALLERY = REPO / "gallery"


def die(msg: str) -> "None":
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def schema_export(delvec: Path) -> dict:
    r = subprocess.run(
        [str(delvec), "schema", "--stage", "all"], capture_output=True, text=True
    )
    if r.returncode != 0:
        die(f"`delvec schema --stage all` exited {r.returncode}: {r.stderr.strip()}")
    return json.loads(r.stdout)


def load_stage_docs(campaign: Path, export: dict) -> dict[str, dict]:
    out: dict[str, dict] = {}
    for stage, fn in stage_files(export).items():
        p = campaign / fn
        if p.is_file():
            out[stage] = json.loads(p.read_text())
    return out


def materialise(base: Path, overlay: Path, dest: Path) -> None:
    """A build of the domain: the primary, with the overlay's or probe's files on top.

    An overlay is a **parameter point of the one gallery**, never a second
    gallery (§3): it ships only the stage files it changes, so a drift in the
    primary reaches it automatically and it cannot quietly become a fork.

    What a point IS lives in `gallery_domain`, which is also what
    `tools/gallery-baseline.py` builds and what `tools/gallery-build.py` writes
    to disk. This tool used to answer that question itself and answered it
    slightly differently, so the tree it validated was not the tree anything
    compiled — a divergence nothing could have reported.
    """
    assert base == GALLERY, f"the domain has one source and it is `{GALLERY}`, not `{base}`"
    gallery_domain.materialise(dest, overlay)


def bind(enumerator: Enumerator, export: dict, docs: dict[str, dict], label: str):
    b = Binder(enumerator)
    for stage, doc in docs.items():
        b.walk(export[stage], doc, label)
    return b


def _codes(r: subprocess.CompletedProcess) -> list[str]:
    codes = []
    for line in (r.stdout + r.stderr).splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            codes.append(json.loads(line).get("code"))
        except json.JSONDecodeError:
            continue
    return [c for c in codes if c]


def run_probe(delvec: Path, campaign: Path, prefabs: Path) -> tuple[int, list[str], str]:
    """Put a probe through the engine and report how it was refused.

    **`validate` first, then `build`** — and the second half is not an
    optimisation, it is what makes the probe mechanism able to express a whole
    class of rule at all. A probe naming a code the DOCUMENT-level phase cannot
    reach — every geometry and emission refusal, which needs resolved anchor
    cells and therefore a plan — is accepted by `validate` and was then failed
    here as *"the engine no longer refuses what this code says it refuses"*: a
    false red, in the one direction that reads as a finding about the engine.
    So the refusal hatch was silently unavailable to `DW0359`, `DW0422`,
    `DW0878` and their whole family, and nothing said so.

    A campaign `validate` already refuses never reaches the build, so the
    fifteen probes that predate this pay nothing. The phase that produced the
    refusal is returned with it, so the index says which one looked.
    """
    v = subprocess.run(
        [str(delvec), "validate", str(campaign), "--prefabs", str(prefabs), "--json"],
        capture_output=True,
        text=True,
    )
    if v.returncode != 0:
        return v.returncode, _codes(v), "validate"
    out = Path(tempfile.mkdtemp(prefix="gallery-probe-build-"))
    try:
        b = subprocess.run(
            [
                str(delvec),
                "build",
                str(campaign),
                "-o",
                str(out / "delve"),
                "--prefabs",
                str(prefabs),
                "--json",
            ],
            capture_output=True,
            text=True,
        )
        return b.returncode, _codes(b), "build"
    finally:
        shutil.rmtree(out, ignore_errors=True)


# The probe contract has exactly two kinds and no third (§3). They are named
# here so that every site — this tool, its tests, the report — spells them once.
EXEMPTION = "exemption"
DEMONSTRATION = "demonstration"


def probe_kind(name: str, manifest: dict) -> tuple[str, str, list[str], str]:
    """What a probe manifest DECLARES: its kind, code, claimed units and reason.

    This is a function rather than six lines inside `main` because the contract
    had **two authorities and they drifted**. When the demonstration kind landed
    here, `tools/tests/test_gallery_coverage.py` went on asserting that every
    probe claims at least one unit — a rule this tool had stopped implementing —
    and the red that produced was the test, not the gallery. A test that DRIVES
    this function cannot restate the contract wrongly, because there is nothing
    left to restate.

    The kind follows from what the manifest claims: units means **exemption**,
    no units means **refusal demonstration**. That is a choice the author makes
    by writing an empty list, which is the shape CLAUDE.md warns about — an
    opt-out with several kinds is only as strong as the weakest. What answers it
    is not this function but `probe_discharges`: the weaker kind grants NOTHING,
    so the disjunction's weak branch buys an author no coverage. See there.
    """
    code = manifest.get("code") or ""
    if not code.startswith("DW"):
        die(
            f"probe `{name}` must name the `DW` code it is refused with "
            f"(got {code!r}). An exemption is only as good as the diagnostic it "
            "names, and a demonstration is only the re-run of a named refusal."
        )
    why = manifest.get("why") or ""
    if not why:
        die(
            f"probe `{name}` carries no `why`. A probe is the clearest thing a "
            "creator can read to learn what this engine checks, and one that "
            "cannot say what it tries renders an empty row in the index."
        )
    claimed = manifest.get("units") or []
    return (EXEMPTION if claimed else DEMONSTRATION), code, claimed, why


def assert_refused(
    name: str, kind: str, code: str, rc: int, codes: list[str], phase: str
) -> None:
    """The obligation BOTH kinds carry: refused, and refused with the code named.

    This is the vacuity-mode-6 hardening and it is where it lives for every
    probe. The defect the coverage gate exists to catch is a unit nothing has
    ever written; an unwritten unit produces silence, never a refusal, so it
    cannot supply what either kind of probe is asked for here.

    `phase` is the phase that produced the verdict — see [`run_probe`]. It is
    named in the refusal so an accepted probe says which phases looked, rather
    than naming one and leaving the reader to assume it was the only one.
    """
    if rc == 0:
        die(
            f"probe `{name}` was ACCEPTED by `delvec validate` AND by "
            f"`delvec build`. A probe is a "
            "refusal's whole proof: it must be refused. "
            + (
                "An accepted exemption probe proves its units are writable — "
                "which makes it a missing element, not an exemption."
                if kind == EXEMPTION
                else "An accepted demonstration proves the engine no longer "
                f"refuses what `{code}` says it refuses."
            )
        )
    if code not in codes:
        die(
            f"probe `{name}` was refused by `delvec {phase}`, but not with `{code}` "
            f"(got {sorted(set(codes)) or 'no machine-readable code'}). A probe "
            "refused for an unrelated reason proves nothing about the rule it "
            "names."
        )


def probe_discharges(
    name: str, kind: str, code: str, claimed: list[str], why: str, units: dict, bound: dict
) -> dict[str, dict]:
    """What this probe takes OFF the coverage requirement — the discharge half.

    **A demonstration discharges nothing**, and that single fact is what makes
    the author's choice of kind safe rather than an escape hatch. Some rules are
    about a *combination* of surfaces each of which is perfectly writable
    (`DW0839`), so there is no unit for such a probe to exempt; the alternative
    would be to force it to claim a unit the domain already binds, which the
    `already binds` refusal below correctly forbids.

    The verdict only ever subtracts this function's return value, so an author
    who empties `units` moves a unit from *discharged* back to *unaccounted* —
    never the other way. The two refusals below exist to keep a CLAIM honest,
    not to guard coverage: a claim on a non-unit subtracts nothing from the unit
    set anyway, and a claim on an already-bound unit subtracts something already
    accounted for. Dropping such a claim is therefore the honest repair, and it
    buys the author no coverage in either case.
    """
    if kind == DEMONSTRATION:
        return {}
    out: dict[str, dict] = {}
    for unit in claimed:
        if unit not in units:
            die(f"probe `{name}` claims `{unit}`, which is not a unit")
        if unit in bound:
            die(
                f"probe `{name}` claims `{unit}`, which the domain already "
                "binds. A unit that is written and compiles needs no exemption."
            )
        out[unit] = {"probe": name, "code": code, "why": why}
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--delvec", help="the delvec whose schema export defines the units")
    ap.add_argument(
        "--prefabs",
        required=True,
        help="the generated prefab directory (spec-0039 §6: built, never committed)",
    )
    ap.add_argument(
        "--build-out",
        help="a `delvec build` output tree of the gallery; its `validation/*.json` "
        "binding ledgers are carried into the report and a zero binding on "
        "machinery the gallery writes is a red (spec-0039 criterion 6)",
    )
    ap.add_argument("--report", help="write the machine report here (JSON)")
    ap.add_argument("--index", help="write the reader-facing element index here (Markdown)")
    args = ap.parse_args()

    delvec = resolve_delvec(args.delvec, repo=REPO, caller="check-gallery-coverage")
    prefabs = Path(args.prefabs)
    if not prefabs.is_dir():
        die(
            f"--prefabs `{prefabs}` is not a directory. The gallery's piece is "
            "GENERATED (spec-0039 §6) — run "
            "`cargo run --release --manifest-path prefabs/gallery-generator/Cargo.toml "
            "-- <dir> --skins gallery/skins` first."
        )

    export = schema_export(delvec)
    enumerator = Enumerator(export)
    units = enumerator.run()

    # Vacuity guard on the gate itself (§3): zero units means the export moved
    # or emptied, and every assertion below is then universally quantified over
    # nothing and passes.
    if not units:
        die(
            "the schema export enumerated ZERO surface units. Nothing below "
            "examined anything, so a green here would mean nothing. Either "
            "`delvec schema --stage all` changed shape or the walk stopped "
            "matching it."
        )

    primary_docs = load_stage_docs(GALLERY, export)
    if not primary_docs:
        die(f"the gallery at `{GALLERY}` holds no stage documents")
    docs_walked = len(primary_docs)

    primary = bind(enumerator, export, primary_docs, "primary")
    bound: dict[str, list[str]] = {k: list(v) for k, v in primary.bound.items()}

    # ---------------------------------------------------------------- overlays
    overlay_rows = []
    overlays_dir = GALLERY / "overlays"
    tmp = Path(tempfile.mkdtemp(prefix="gallery-domain-"))
    try:
        for od in sorted(p for p in overlays_dir.iterdir() if p.is_dir()) if overlays_dir.is_dir() else []:
            manifest_path = od / "overlay.json"
            if not manifest_path.is_file():
                die(f"overlay `{od.name}` has no `overlay.json` declaring what it binds")
            manifest = json.loads(manifest_path.read_text())
            declared = manifest.get("binds") or []
            if not declared:
                die(
                    f"overlay `{od.name}` declares an EMPTY `binds` set. An overlay "
                    "exists to reach a unit the primary cannot (§3); one that "
                    "claims nothing is the redundant overlay the rule forbids."
                )
            dest = tmp / od.name
            materialise(GALLERY, od, dest)
            ov = bind(enumerator, export, load_stage_docs(dest, export), f"overlay:{od.name}")
            for unit in declared:
                if unit not in units:
                    die(f"overlay `{od.name}` declares `{unit}`, which is not a unit")
                if unit not in ov.bound:
                    die(
                        f"overlay `{od.name}` declares it binds `{unit}` and does "
                        "not. The declaration is the overlay's whole reason to "
                        "exist; an unbacked one is a gate that binds to nothing."
                    )
                if unit in primary.bound:
                    die(
                        f"overlay `{od.name}` declares `{unit}`, which the PRIMARY "
                        "already binds. A redundant overlay is a red (§3) — it is "
                        "what stops the overlay set growing by reflex."
                    )
            for k, v in ov.bound.items():
                bound.setdefault(k, []).extend(v)
            overlay_rows.append(
                {"id": od.name, "binds": sorted(declared), "note": manifest.get("note", "")}
            )
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    # ------------------------------------------------------------------ probes
    refusal: dict[str, dict] = {}
    # Probes that name no unit: refused, re-run, and discharging nothing.
    demonstrations: dict[str, dict] = {}
    probes_dir = GALLERY / "probes"
    tmp = Path(tempfile.mkdtemp(prefix="gallery-probes-"))
    try:
        for pd in sorted(p for p in probes_dir.iterdir() if p.is_dir()) if probes_dir.is_dir() else []:
            manifest_path = pd / "probe.json"
            if not manifest_path.is_file():
                die(f"probe `{pd.name}` has no `probe.json`")
            manifest = json.loads(manifest_path.read_text())
            kind, code, claimed, why = probe_kind(pd.name, manifest)
            dest = tmp / pd.name
            materialise(GALLERY, pd, dest)
            rc, codes, phase = run_probe(delvec, dest, prefabs)
            assert_refused(pd.name, kind, code, rc, codes, phase)
            if kind == DEMONSTRATION:
                demonstrations[pd.name] = {"code": code, "why": why, "phase": phase}
            refusal.update(
                probe_discharges(pd.name, kind, code, claimed, why, units, bound)
            )
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    # ----------------------------------------------------------------- verdict
    bound_ids = {u for u in bound if u in units}
    proven = set(refusal)
    unaccounted = sorted(set(units) - bound_ids - proven)

    print(
        f"gallery coverage: {len(units)} unit(s) enumerated, {len(bound_ids)} bound, "
        f"{len(proven)} refusal-proven, {len(unaccounted)} in NEITHER state."
    )
    if demonstrations:
        print(
            f"refusal demonstrations: {len(demonstrations)} probe(s) refused with the code "
            f"they name and discharging no unit ({', '.join(sorted(demonstrations))})."
        )
    print(
        f"binding domain: {docs_walked} primary stage document(s), "
        f"{len(overlay_rows)} overlay(s), {len(refusal)} unit(s) behind "
        f"{len({r['probe'] for r in refusal.values()})} probe(s)."
    )

    # ------------------------------------------- compiler-stated bindings (§8.6)
    ledgers, zero_bindings = read_build_ledgers(Path(args.build_out)) if args.build_out else ({}, [])
    if ledgers:
        print(
            f"compiler-stated bindings: {len(ledgers)} ledger(s) read from the "
            f"gallery's build, {len(zero_bindings)} reporting a ZERO binding."
        )

    report = {
        "compiler_bindings": ledgers,
        "compiler_zero_bindings": zero_bindings,
        "units_total": len(units),
        "units_bound": len(bound_ids),
        "units_refusal_proven": len(proven),
        "units_unaccounted": len(unaccounted),
        "unaccounted": unaccounted,
        "overlays": overlay_rows,
        "refusal_proven": {k: refusal[k] for k in sorted(refusal)},
        "primary_documents": docs_walked,
    }
    if args.report:
        Path(args.report).write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    if args.index:
        write_index(
            Path(args.index),
            units,
            bound,
            refusal,
            overlay_rows,
            unaccounted,
            demonstrations,
        )

    if unaccounted:
        print(
            "\nThese units are neither written anywhere in the gallery nor proven "
            "to be refused. Each is a surface nothing has compiled end to end:",
            file=sys.stderr,
        )
        for u in unaccounted:
            doc = units[u].doc
            print(f"  {u}" + (f" — {doc}" if doc else ""), file=sys.stderr)
        print(
            "\nDischarge each by writing it into the gallery (an element, or one "
            "field line), or — only when the compiler really refuses it — by a "
            "probe under `gallery/probes/` naming the code. A prose justification "
            "is not an exemption here (spec-0039 §3).",
            file=sys.stderr,
        )
        return 1

    if zero_bindings:
        # `read_build_ledgers` has always said "the caller reds on the ones the
        # gallery writes", and the caller did not: the list was computed,
        # printed in the summary line, written into the report — and never
        # gated. A verdict nothing acts on is the UNRUN shape wearing a
        # measurement's clothes, and it is worse than an absent check, because
        # the printed count reads as a check that ran. Every ledger the gallery
        # emits binds today (7 of 7 non-zero), so this reds on drift and on
        # nothing else.
        print(
            "\nThese compiler-stated bindings are ZERO on a campaign that "
            "declares everything, so each is a proof that has stopped reaching "
            "what the gallery plainly writes — vacuous, not a pass:",
            file=sys.stderr,
        )
        for z in zero_bindings:
            print(f"  {z}", file=sys.stderr)
        print(
            "\nFix the check that stopped binding. A ledger whose count is "
            "honestly zero here means the gallery no longer writes that "
            "machinery, which is itself the finding.",
            file=sys.stderr,
        )
        return 1
    return 0


def read_build_ledgers(out: Path) -> tuple[dict, list[str]]:
    """The build's own binding ledgers, and which of them bound to nothing.

    Criterion 6: *a zero binding on machinery the gallery writes is a red.* The
    qualifier is doing the work. A campaign that fields no traps has no trap
    payloads, and `press-bodies.json` reporting `examined: 0` there is an honest
    measurement, not a failure — CLAUDE.md's own worked example says so. What is
    a failure is a zero on a proof over machinery **this** campaign declares,
    because the gallery declares everything: a zero there means the proof stopped
    reaching what the document plainly writes.

    So the zeroes are reported by name, and the caller reds on them.
    `effect-roots.json` is the sharpest of them: the gallery binds all eight
    roots, where the largest shipped campaign binds three.
    """
    vdir = out / "validation"
    if not vdir.is_dir():
        die(f"`{out}` is not a delvec build output tree (no `validation/`)")
    ledgers: dict = {}
    zeroes: list[str] = []
    for f in sorted(vdir.glob("*.json")):
        try:
            doc = json.loads(f.read_text())
        except json.JSONDecodeError:
            die(f"`{f}` is not readable JSON — a binding ledger nobody can read is not one")
        ledgers[f.name] = doc
        if not isinstance(doc, dict):
            continue
        if doc.get("unbound") is True:
            zeroes.append(f"{f.name}: {doc.get('reason') or doc.get('unbound_reason') or 'unbound'}")
        for k in ("examined", "bundles", "gates_examined", "pieces_examined"):
            if doc.get(k) == 0:
                zeroes.append(f"{f.name}: `{k}` is 0")
        for root in doc.get("unbound_roots") or []:
            zeroes.append(f"{f.name}: no bundle at root `{root}`")
    return ledgers, zeroes


def write_index(
    path: Path,
    units: dict,
    bound: dict[str, list[str]],
    refusal: dict[str, dict],
    overlays: list[dict],
    unaccounted: list[str],
    demonstrations: dict[str, dict] | None = None,
) -> None:
    """The reader-facing half: element → surface, and surface → element.

    Written for a creator asking "what can this engine build, and what does it
    check", not for the gate. The gate reads the JSON report.
    """
    lines = [
        "# What the gallery covers",
        "",
        "Generated by `tools/check-gallery-coverage.py --index`. Every row is a",
        "surface the DSL declares and the place the gallery writes it.",
        "",
        f"- **{len(units)}** surface units declared by `delvec schema --stage all`",
        f"- **{len(bound)}** written in the gallery",
        f"- **{len(refusal)}** proven by a refusal the engine really performs",
        f"- **{len(unaccounted)}** in neither state",
        "",
    ]
    if refusal:
        lines += [
            "## What the engine refuses",
            "",
            "Each row is a thing a creator might reasonably try to write, and the",
            "diagnostic that stops them. The probe under `gallery/probes/` is a",
            "committed document that really is refused — run it yourself.",
            "",
            "| Surface | Code | Why |",
            "| --- | --- | --- |",
        ]
        for unit in sorted(refusal):
            r = refusal[unit]
            lines.append(f"| `{unit}` | `{r['code']}` | {r['why']} |")
        lines.append("")
    if demonstrations:
        lines += [
            "## What the engine refuses to COMBINE",
            "",
            "Some rules are not about a surface at all — both halves are perfectly",
            "writable, and what the engine says no to is holding them at once. Such",
            "a probe discharges no unit; it exists so the refusal is re-run rather",
            "than remembered.",
            "",
            "| Probe | Code | Refused by | Why |",
            "| --- | --- | --- | --- |",
        ]
        for name in sorted(demonstrations):
            d = demonstrations[name]
            phase = d.get("phase", "validate")
            lines.append(f"| `{name}` | `{d['code']}` | `delvec {phase}` | {d['why']} |")
        lines.append("")
    if overlays:
        lines += [
            "## Parameter points",
            "",
            "Some settings are mutually exclusive within one world, so the gallery",
            "carries them as overlays — the same campaign at another parameter",
            "point, never a second gallery.",
            "",
            "| Overlay | Reaches | Why |",
            "| --- | --- | --- |",
        ]
        for o in overlays:
            lines.append(
                f"| `{o['id']}` | {', '.join(f'`{u}`' for u in o['binds'])} | {o['note']} |"
            )
        lines.append("")
    lines += ["## Every surface, and where it is written", "", "| Surface | Written at |", "| --- | --- |"]
    for uid in sorted(units):
        if uid in refusal:
            where = f"refused — `{refusal[uid]['code']}` (probe `{refusal[uid]['probe']}`)"
        elif uid in bound:
            sites = bound[uid]
            where = f"`{sites[0]}`" + (f" (+{len(sites) - 1} more)" if len(sites) > 1 else "")
        else:
            where = "**nowhere**"
        lines.append(f"| `{uid}` | {where} |")
    path.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    raise SystemExit(main())
