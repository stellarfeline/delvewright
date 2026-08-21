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
from gallery_units import Binder, Enumerator, stage_files  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
GALLERY = REPO / "gallery"


def die(msg: str) -> "None":
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def find_delvec(explicit: str | None) -> Path:
    if explicit:
        p = Path(explicit)
        if not p.is_file():
            die(f"--delvec `{p}` is not a file")
        return p
    for rel in ("target/release/delvec", "target/debug/delvec"):
        p = REPO / rel
        if p.is_file():
            return p
    die(
        "no delvec binary found. The unit set is derived from the compiler in "
        "THIS tree and from nothing else, so there is no fallback: build one "
        "with `cargo build -p delvec --bin delvec` (add `--release` for speed) "
        "or pass --delvec."
    )
    raise AssertionError("unreachable")


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
    """A build of the domain: the primary, with the overlay's files on top.

    An overlay is a **parameter point of the one gallery**, never a second
    gallery (§3): it ships only the stage files it changes, so a drift in the
    primary reaches it automatically and it cannot quietly become a fork.
    """
    shutil.copytree(base, dest, dirs_exist_ok=True)
    for src in overlay.iterdir():
        if src.name in ("overlay.json", "probe.json"):
            continue
        if src.is_dir():
            shutil.copytree(src, dest / src.name, dirs_exist_ok=True)
        else:
            shutil.copy2(src, dest / src.name)


def bind(enumerator: Enumerator, export: dict, docs: dict[str, dict], label: str):
    b = Binder(enumerator)
    for stage, doc in docs.items():
        b.walk(export[stage], doc, label)
    return b


def run_probe(delvec: Path, campaign: Path, prefabs: Path) -> tuple[int, list[str]]:
    r = subprocess.run(
        [str(delvec), "validate", str(campaign), "--prefabs", str(prefabs), "--json"],
        capture_output=True,
        text=True,
    )
    codes = []
    for line in (r.stdout + r.stderr).splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            codes.append(json.loads(line).get("code"))
        except json.JSONDecodeError:
            continue
    return r.returncode, [c for c in codes if c]


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

    delvec = find_delvec(args.delvec)
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
    probes_dir = GALLERY / "probes"
    tmp = Path(tempfile.mkdtemp(prefix="gallery-probes-"))
    try:
        for pd in sorted(p for p in probes_dir.iterdir() if p.is_dir()) if probes_dir.is_dir() else []:
            manifest_path = pd / "probe.json"
            if not manifest_path.is_file():
                die(f"probe `{pd.name}` has no `probe.json`")
            manifest = json.loads(manifest_path.read_text())
            code = manifest.get("code")
            claimed = manifest.get("units") or []
            if not code or not claimed:
                die(f"probe `{pd.name}` must name a `code` and at least one `unit`")
            dest = tmp / pd.name
            materialise(GALLERY, pd, dest)
            rc, codes = run_probe(delvec, dest, prefabs)
            if rc == 0:
                die(
                    f"probe `{pd.name}` was ACCEPTED by `delvec validate`. A probe "
                    "is an exemption's whole proof: it must be refused, and a "
                    f"probe the compiler accepts proves `{claimed[0]}` is writable "
                    "— which makes it a missing element, not an exemption."
                )
            if code not in codes:
                die(
                    f"probe `{pd.name}` was refused, but not with `{code}` "
                    f"(got {sorted(set(codes)) or 'no machine-readable code'}). An "
                    "exemption is only as good as the code it names — a probe "
                    "refused for an unrelated reason proves nothing about this unit."
                )
            for unit in claimed:
                if unit not in units:
                    die(f"probe `{pd.name}` claims `{unit}`, which is not a unit")
                if unit in bound:
                    die(
                        f"probe `{pd.name}` claims `{unit}`, which the domain "
                        "already binds. A unit that is written and compiles needs "
                        "no exemption."
                    )
                refusal[unit] = {
                    "probe": pd.name,
                    "code": code,
                    "why": manifest.get("why", ""),
                }
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
        write_index(Path(args.index), units, bound, refusal, overlay_rows, unaccounted)

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

    So the zeroes are reported by name rather than counted, and the caller reds on
    the ones the gallery writes. `effect-roots.json` is the sharpest of them: the
    gallery binds all eight roots, where the largest shipped campaign binds three.
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
