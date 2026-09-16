#!/usr/bin/env python3
"""A tileset generator deletes nothing it did not write.

## The defect this gate exists to catch

Every prefab generator emits a whole metadata document and writes it over
whatever was there, so re-running one over a library deletes every key added
after generation: the anchors a campaign binds by name, an `"role": "entry"`
declaration, a `shown_faces` list, a lighting verdict measured at admission
rather than estimated at generation. Nothing bound the generators, so the rule
was transplanted by hand at every library round and the deletion surfaced only
as a campaign that stopped resolving an anchor it names.

Measured on the library as it stood: **88 leaf keys across 7 of 31
generator-written documents** — 40 anchors, 4 entry roles, 4 `shown_faces` and 5
lighting verdicts.

## The fix is in the generators; this proves it

`prefab_invariants::document::write_preserving` merges a generator's output onto
whatever is already on disk, so the safe path is the default and no library round
has to remember. A rule with no check is a sentence, and this is the check: it
runs every document-writing generator, plants into its own output exactly the
kinds of key a later step adds, runs it again, and refuses if any of them is
gone.

## Why it plants rather than reads a library

The committed prefab library lives in the content repository, reached through a
gitignored symlink that CI does not always have. A gate whose population is a
directory that may not be there is a gate that reports zero and passes, which is
the unbound vacuity mode. So the population is the generators themselves —
derived from `prefabs/Cargo.toml`'s own workspace members, never listed here —
and every document each one writes is decorated and re-checked. The denominators
are printed on every run.

## What it also asserts

No `.nbt` moves between the two runs. That is ADR-0006's determinism over the
generators, and it is free here: the second run is exactly the re-run this gate
already performs.

Exit 0 when every planted key survives and no structure byte moved; 1 otherwise;
2 on a usage or build failure.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PREFABS = ROOT / "prefabs"

# The kinds of key a later step adds to a generated document, and which this
# gate plants. Each one is a real surface with a real reader, named beside it —
# an invented key would prove only that unknown keys survive, and the keys that
# were actually deleted were modelled ones.
TOP_LEVEL = {
    # `DW0885` reads it; a piece authored to be buried writes nothing here.
    "shown_faces": ["north"],
    # spec-0036's contract, written by an exporter after the fact.
    "spatial_contract": {"entry": "hall", "spaces": {}, "no_body": {}, "edges": []},
    # spec-0050 §5, `DW0848`.
    "footprint_class": "size-class.room",
    # A key this engine does not model at all: kept as `PrefabMeta::extra`
    # (`DW0543`), which is the forward-compatibility half of the same rule.
    "x_added_by_a_newer_producer": {"note": "kept verbatim"},
}

# An anchor no generator knows about — the 40-of-88 half of the measurement.
NEW_ANCHOR = ("anchor/hand-added-by-a-library-round", {"pos": [1, 1, 1], "facing": "north"})

# Keys added INSIDE an anchor the generator does write, which is the half a
# top-level merge alone would still lose.
INSIDE_ANCHOR = {
    "role": "entry",
    "note": "added by a library round, for a person reading the piece",
    "resolves_to": "space:hall",
}


def workspace_members() -> list[str]:
    """Every crate in the generators' workspace, from the manifest itself."""
    text = (PREFABS / "Cargo.toml").read_text(encoding="utf-8")
    block = re.search(r"^members\s*=\s*\[(.*?)\]", text, re.S | re.M)
    if not block:
        sys.exit("prefabs/Cargo.toml declares no `members` — nothing to sweep")
    return re.findall(r'"([^"]+)"', block.group(1))


def binary_name(member: str) -> str:
    text = (PREFABS / member / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^name\s*=\s*"([^"]+)"', text, re.M)
    if not m:
        sys.exit(f"prefabs/{member}/Cargo.toml declares no package name")
    return m.group(1)


def leaf_paths(value, at: str = "") -> list[str]:
    """Every leaf key path, as `a.b[0].c`.

    The same shape `prefab_invariants::document::leaf_paths` computes, because
    the gate and the writer must count the same thing.
    """
    out: list[str] = []
    if isinstance(value, dict) and value:
        for k, child in value.items():
            out += leaf_paths(child, f"{at}.{k}" if at else k)
    elif isinstance(value, list) and value:
        for i, child in enumerate(value):
            out += leaf_paths(child, f"{at}[{i}]")
    else:
        out.append(at)
    return sorted(out)


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def decorate(doc_path: pathlib.Path) -> list[str]:
    """Plant the post-generation keys into one document; return their paths."""
    doc = json.loads(doc_path.read_text(encoding="utf-8"))
    if not isinstance(doc, dict):
        return []
    planted: list[str] = []
    # `pools.json` is a pool declaration, not a prefab document: it has no
    # anchors and no faces, so only the unmodelled key is planted there. The
    # object decides, not a filename list.
    is_prefab = "prefab_id" in doc
    for key, value in TOP_LEVEL.items():
        if not is_prefab and key != "x_added_by_a_newer_producer":
            continue
        # A key the generator writes ITSELF is not this rule's subject: the
        # generator owns what it measures, so overwriting one here would measure
        # whether it re-measures rather than whether it preserves.
        if key in doc:
            continue
        doc[key] = value
        planted += [f"{key}{p}" if p.startswith("[") else (f"{key}.{p}" if p else key)
                    for p in leaf_paths(value)]
    if is_prefab:
        anchors = doc.setdefault("anchors", {})
        name, body = NEW_ANCHOR
        anchors[name] = body
        planted += [f"anchors.{name}.{p}" for p in leaf_paths(body)]
        for existing, anchor in list(anchors.items()):
            if existing == name or not isinstance(anchor, dict):
                continue
            for key, value in INSIDE_ANCHOR.items():
                if key in anchor:
                    continue
                anchor[key] = value
                planted.append(f"anchors.{existing}.{key}")
    doc_path.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return planted


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--scratch",
        type=pathlib.Path,
        required=True,
        help="a directory this gate owns and replaces; never /tmp on a developer machine",
    )
    ap.add_argument(
        "--no-build",
        action="store_true",
        help="use the release binaries already on the shelf (the caller has just built them)",
    )
    args = ap.parse_args()

    if not args.no_build:
        build = subprocess.run(
            ["cargo", "build", "--release", "--workspace",
             "--manifest-path", str(PREFABS / "Cargo.toml")],
            cwd=PREFABS,
        )
        if build.returncode != 0:
            print("the generators do not build, so nothing was measured", file=sys.stderr)
            return 2

    shelf = PREFABS / "target" / "release"
    members = workspace_members()
    swept = documents = planted_keys = 0
    lost: list[str] = []
    moved: list[str] = []
    silent: list[str] = []

    scratch = args.scratch
    if scratch.exists():
        shutil.rmtree(scratch)
    scratch.mkdir(parents=True)

    for member in members:
        binary = shelf / binary_name(member)
        if not binary.exists():
            # `invariants` is a library and has no binary: a member with nothing
            # to run writes no document and is not this gate's subject.
            continue
        out = scratch / member
        out.mkdir(parents=True)
        run = subprocess.run([str(binary), str(out)], capture_output=True, text=True)
        if run.returncode != 0:
            print(f"{member}: exited {run.returncode}\n{run.stderr}", file=sys.stderr)
            return 2
        docs = sorted(out.glob("*.json"))
        if not docs:
            # A generator that writes no document cannot delete a key; it is
            # reported rather than skipped in silence.
            silent.append(member)
            continue
        swept += 1
        before_nbt = {p.name: digest(p) for p in sorted(out.glob("*.nbt"))}
        expected: dict[pathlib.Path, list[str]] = {}
        for doc in docs:
            expected[doc] = decorate(doc)
            documents += 1
            planted_keys += len(expected[doc])
        rerun = subprocess.run([str(binary), str(out)], capture_output=True, text=True)
        if rerun.returncode != 0:
            # The writer asserts the rule itself, so a non-zero exit here IS the
            # finding rather than an error beside it.
            print(f"{member}: re-run over its own decorated output exited "
                  f"{rerun.returncode}\n{rerun.stderr}", file=sys.stderr)
            lost.append(f"{member}: the generator refused to write (see above)")
            continue
        for doc, keys in expected.items():
            if not doc.exists():
                lost.append(f"{doc.name}: the document itself is gone")
                continue
            have = set(leaf_paths(json.loads(doc.read_text(encoding="utf-8"))))
            gone = [k for k in keys if k not in have]
            if gone:
                lost.append(f"{doc.name}: {len(gone)} key(s) deleted — {', '.join(gone[:6])}")
        after_nbt = {p.name: digest(p) for p in sorted(out.glob("*.nbt"))}
        for name, before in before_nbt.items():
            if after_nbt.get(name) != before:
                moved.append(f"{member}/{name}")

    print(f"generator preservation binding: {len(members)} workspace member(s) declared, "
          f"{swept} document-writing generator(s) swept, {documents} document(s) decorated "
          f"and re-generated, {planted_keys} planted key(s) checked, "
          f"{len(lost)} deletion finding(s), {len(moved)} `.nbt` moved.")
    if silent:
        print(f"  members that write no document (nothing to preserve): {', '.join(silent)}")
    if swept == 0 or planted_keys == 0:
        print("REFUSED: this run examined nothing — a green verdict over an empty population "
              "is the unbound vacuity mode, not a pass.", file=sys.stderr)
        return 1
    for line in lost:
        print(f"  DELETED  {line}", file=sys.stderr)
    for line in moved:
        print(f"  MOVED    {line} — a re-run changed a structure byte (ADR-0006)", file=sys.stderr)
    if lost or moved:
        print("REFUSED: a generator may not delete a key it did not write. Every generator "
              "writes through `prefab_invariants::document::write_preserving`, which merges "
              "its output onto whatever is already there; a document written any other way "
              "is what this reds on.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
