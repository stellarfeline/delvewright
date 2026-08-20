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

## Two verdicts from one mismatch, because they mean opposite things

- the change touches `gallery/` or `crates/` — an **emission change**: regenerate
  the baseline in this change, or explain the drift;
- it touches neither — a **determinism finding** (ADR-0006), named as such. The
  baseline is thereby a standing cross-machine determinism probe, for free.

## The warnings ledger

Judgement-tier warnings are legitimate; *drifting* warnings are not. The emitted
warning set must equal the committed ledger exactly, so "still green" can never
quietly absorb "warns differently now".

## Binding count

Every run states builds compared, output paths compared, and warning rows
checked. Comparing zero builds or zero paths is a red: a baseline that matched
nothing is vacuous, not a pass.
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

REPO = Path(__file__).resolve().parent.parent
GALLERY = REPO / "gallery"
BASELINE = GALLERY / "baseline"

# The builds that make up the domain: the primary in every declared language,
# plus each overlay in `en`. Derived, never listed — a listed set goes stale the
# first time a language or an overlay is added, and goes stale silently.
WARNING_RE = re.compile(r"^(DW\d{4}) \[warning\] (\S+) (\S+):")


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


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


def overlays() -> list[str]:
    d = GALLERY / "overlays"
    return sorted(p.name for p in d.iterdir() if p.is_dir()) if d.is_dir() else []


def materialise(overlay: str | None, dest: Path) -> None:
    shutil.copytree(GALLERY, dest, dirs_exist_ok=True)
    for junk in ("baseline", "overlays", "probes"):
        shutil.rmtree(dest / junk, ignore_errors=True)
    if overlay:
        src = GALLERY / "overlays" / overlay
        for f in src.iterdir():
            if f.name == "overlay.json":
                continue
            if f.is_dir():
                shutil.copytree(f, dest / f.name, dirs_exist_ok=True)
            else:
                shutil.copy2(f, dest / f.name)


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
        m = WARNING_RE.match(line.strip())
        if m:
            rows.append({"code": m.group(1), "stage": m.group(2), "pointer": m.group(3)})
    return manifest, rows


def build_id(overlay: str | None, lang: str) -> str:
    return f"{overlay or 'primary'}.{lang}"


def header(delvec: Path, prefabs: Path) -> dict:
    v = delvec_versions(delvec)
    return {
        "delvec_version": v["delvec"],
        "dsl_version": v["dsl"],
        "gallery_source_sha256": tree_hash(GALLERY, {"baseline"}),
        "generator_input_sha256": tree_hash(prefabs, set()),
    }


def classify(paths: list[str]) -> str:
    """Emission change, or determinism finding — decided by what the diff touches."""
    r = subprocess.run(
        ["git", "-C", str(REPO), "diff", "--name-only", "origin/main...HEAD"],
        capture_output=True,
        text=True,
    )
    changed = r.stdout.splitlines() if r.returncode == 0 else []
    touched = [c for c in changed if c.startswith("gallery/") or c.startswith("crates/")]
    if touched:
        return "emission-change"
    return "determinism-finding"


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
    n_paths = sum(len(m.get("outputs") or m.get("files") or {}) for m in manifests.values())
    n_warn = sum(len(v) for v in warnings.values())
    print(
        f"gallery baseline: {len(manifests)} build(s), {n_paths} emitted path(s), "
        f"{n_warn} warning row(s)."
    )
    if not manifests or n_paths == 0:
        die(
            "the baseline compared ZERO builds or ZERO emitted paths. A baseline "
            "that matched nothing is vacuous, not a pass."
        )

    if args.write:
        old = json.loads((BASELINE / "manifests.json").read_text()) if (BASELINE / "manifests.json").is_file() else {}
        BASELINE.mkdir(parents=True, exist_ok=True)
        (BASELINE / "header.json").write_text(json.dumps(hdr, indent=2, sort_keys=True) + "\n")
        (BASELINE / "manifests.json").write_text(
            json.dumps(manifests, indent=2, sort_keys=True) + "\n"
        )
        (BASELINE / "warnings.json").write_text(
            json.dumps(warnings, indent=2, sort_keys=True) + "\n"
        )
        delta = compute_delta(old, manifests)
        (BASELINE / "delta.json").write_text(json.dumps(delta, indent=2, sort_keys=True) + "\n")
        if old and not any(delta[k] for k in ("added", "removed", "changed")):
            die(
                "the baseline was rewritten with an EMPTY delta — nothing about "
                "emission moved, so this is a noise commit. A baseline update is "
                "never split from the change that caused it (§5)."
            )
        print(
            f"wrote {BASELINE}: {len(delta['added'])} added, "
            f"{len(delta['removed'])} removed, {len(delta['changed'])} changed path(s)."
        )
        return 0

    for name in ("header.json", "manifests.json", "warnings.json"):
        if not (BASELINE / name).is_file():
            die(f"`gallery/baseline/{name}` is missing — run this with `--write`")
    committed_hdr = json.loads((BASELINE / "header.json").read_text())
    if committed_hdr != hdr:
        diffs = [
            f"  {k}: baseline `{committed_hdr.get(k)}` vs this tree `{hdr.get(k)}`"
            for k in sorted(set(committed_hdr) | set(hdr))
            if committed_hdr.get(k) != hdr.get(k)
        ]
        die(
            "the committed baseline was taken over DIFFERENT INPUTS, so a "
            "file-by-file diff would report noise rather than a finding:\n"
            + "\n".join(diffs)
            + "\nRegenerate it with `--write` in the same change that moved them."
        )
    committed = json.loads((BASELINE / "manifests.json").read_text())
    if committed != manifests:
        delta = compute_delta(committed, manifests)
        differing = delta["added"] + delta["removed"] + delta["changed"]
        verdict = classify(differing)
        if verdict == "determinism-finding":
            die(
                "DETERMINISM FINDING (ADR-0006): this change touches neither "
                "`gallery/` nor `crates/`, and the gallery's emission moved "
                "anyway. Same DSL + same seed must give byte-identical output.\n"
                + "\n".join(f"  {p}" for p in differing)
            )
        die(
            "EMISSION CHANGE: the gallery's emitted bytes moved. Regenerate the "
            "baseline in this same change (`--write`) and confirm every path "
            "class below is a consequence this change claims to have.\n"
            + "\n".join(f"  {p}" for p in differing)
        )
    committed_warnings = json.loads((BASELINE / "warnings.json").read_text())
    if committed_warnings != warnings:
        die(
            "the emitted warning set no longer equals the committed ledger. A new "
            "or vanished warning is a red: 'still green' must never quietly "
            "absorb 'warns differently now' (§4.3). Regenerate with `--write`."
        )
    print("baseline: header, manifests and warning ledger all match.")
    return 0


def compute_delta(old: dict, new: dict) -> dict:
    """Every added/removed/changed emitted path, grouped by output class.

    The review artifact, and it opens with the question its reader answers: *is
    every path class listed here a consequence this change claims to have?*
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
