#!/usr/bin/env python3
"""The file half of a playtest checkpoint: the quest table, the copy, the record.

`tools/creator/playtest-server.sh` owns the live half (the rcon flush around
the copy, the restore boot) and calls this for everything that is about files.

A checkpoint is a copy of a served world, taken while the server has stopped
saving, kept under the KEPT IMAGE it was played on:

    <root>/<image id hex>/image.json        the image's identity
    <root>/<image id hex>/quests.tsv        quest score, the name a checkpoint gets
    <root>/<image id hex>/<NN>/world/       the world, without datapacks/ and session.lock
    <root>/<image id hex>/<NN>/checkpoint.json

Subcommands (each prints what it resolved to; a refusal exits 1 with the reason):

    quests     --datapack DIR                 the quest table, read from the emitted pack
    copy       --src WORLD --dst DIR          copy a world and prove the copy equals it
    digest     --dir DIR                      the digest `copy` recorded, re-measured
    next-seq   --image-root DIR               the next checkpoint number
    write-meta --dir DIR ...                  write checkpoint.json
    list       --root ROOT                    every checkpoint, by image, with sizes
    resolve    --root ROOT --query Q [--image-id ID]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
from pathlib import Path

# A world's files that are not the world: the boot copies `datapacks/` in from
# the image every time (validation/Dockerfile.playtest), and `session.lock` is
# the running server's own lock.
EXCLUDED_TOP = ("datapacks", "session.lock")

# The emitted pack's own statements, read where the compiler wrote them:
#   `scoreboard objectives add dw.q_<x> dummy`        a quest's completion score
#   `execute if score #party dw.o_<a> matches 1 … unless score #party dw.q_<x>
#    matches 1 run function …`                         the quest's completion check
#   `scoreboard players set #party dw.o_<a> 1` beside the "Objective complete"
#    tellraw                                           what the player was told
QUEST_DECL = re.compile(r"^scoreboard objectives add (dw\.q_\S+) dummy\b")
CHECK_LINE = re.compile(r"\bunless score #party (dw\.q_\S+) matches 1 run function \S+$")
GUARD_TERM = re.compile(r"\bif score #party (dw\.o_\S+) matches 1\b")
OBJ_SET = re.compile(r"^scoreboard players set #party (dw\.o_\S+) 1$")
COMPLETE_KEY = "delvewright.ui.objective.complete"


def die(msg: str) -> int:
    print(f"playtest-checkpoint: {msg}", file=sys.stderr)
    return 1


def human(n: int) -> str:
    k = n / 1024
    if k >= 1048576:
        return f"{k / 1048576:.1f} GiB"
    if k >= 1024:
        return f"{k / 1024:.1f} MiB"
    return f"{int(k)} KiB"


# ---- the quest table -------------------------------------------------------


def objective_title(lines: list[str]) -> str | None:
    """The title in a function's "Objective complete: %s" tellraw, or None."""
    for line in lines:
        if not line.startswith("tellraw @a ") or COMPLETE_KEY not in line:
            continue
        try:
            comp = json.loads(line[len("tellraw @a "):])
        except json.JSONDecodeError:
            continue
        if not str(comp.get("translate", "")).endswith(COMPLETE_KEY):
            continue
        for arg in comp.get("with", []):
            if isinstance(arg, dict) and isinstance(arg.get("fallback"), str):
                return arg["fallback"]
    return None


def quest_table(datapack: Path) -> tuple[list[tuple[str, str]], list[str]]:
    """`(score, name)` per declared quest score, in declaration order, and the
    refusals. The name is the title of the LAST titled objective the quest's
    completion check requires: every one of them was completed, and announced
    to the player as completed, before the quest could complete. A quest whose
    objectives are all untitled is named by its place in declaration order."""
    files = sorted(datapack.glob("data/*/function/**/*.mcfunction"))
    declared: list[str] = []
    checks: dict[str, list[list[str]]] = {}
    titles: dict[str, list[str]] = {}
    for f in files:
        lines = f.read_text(encoding="utf-8").splitlines()
        for line in lines:
            m = QUEST_DECL.match(line)
            if m and m.group(1) not in declared:
                declared.append(m.group(1))
            m = CHECK_LINE.search(line)
            if m:
                checks.setdefault(m.group(1), []).append(GUARD_TERM.findall(line))
        sets = [m.group(1) for m in map(OBJ_SET.match, lines) if m]
        if sets:
            title = objective_title(lines)
            if title is not None:
                for score in sets:
                    titles.setdefault(score, []).append(title)
    table, refusals = [], []
    for k, score in enumerate(declared, 1):
        found = checks.get(score, [])
        distinct = {tuple(t) for t in found}
        if len(distinct) != 1:
            refusals.append(f"{score}: {len(distinct)} distinct completion check(s) in the pack, not 1")
            continue
        # An untitled objective is never announced, so it lends no name; the
        # quest's place in the pack's declaration order says nothing of the story.
        name = f"Quest {k} complete"
        for term in found[0]:
            seen = set(titles.get(term, []))
            if len(seen) > 1:
                refusals.append(f"{term}: {len(seen)} different completion titles in the pack, not 1")
            elif seen:
                name = seen.pop()
        table.append((score, name))
    return table, refusals


def cmd_quests(a) -> int:
    pack = Path(a.datapack)
    if not (pack / "pack.mcmeta").is_file():
        return die(f"{pack} is not a datapack (no pack.mcmeta)")
    table, refusals = quest_table(pack)
    if refusals:
        return die("the quest table cannot be read from the emitted pack:\n  " + "\n  ".join(refusals))
    for score, name in table:
        print(f"{score}\t{name}")
    print(f"quest table: {len(table)} quest completion score(s) declared by the pack, each named", file=sys.stderr)
    return 0


# ---- the copy --------------------------------------------------------------


def world_entries(root: Path) -> list[tuple[str, str]]:
    """`(relative path, sha256 | "dir")`, sorted, without the excluded top-level
    entries. Anything that is neither a file nor a directory is refused."""
    out = []
    for dirpath, dirnames, filenames in os.walk(root):
        rel_dir = Path(dirpath).relative_to(root)
        if rel_dir == Path("."):
            dirnames[:] = [d for d in dirnames if d not in EXCLUDED_TOP]
            filenames = [f for f in filenames if f not in EXCLUDED_TOP]
        dirnames.sort()
        for d in dirnames:
            p = Path(dirpath) / d
            if p.is_symlink():
                raise ValueError(f"{p} is a symlink")
            out.append(((rel_dir / d).as_posix(), "dir"))
        for f in sorted(filenames):
            p = Path(dirpath) / f
            if p.is_symlink() or not p.is_file():
                raise ValueError(f"{p} is not a regular file")
            out.append(((rel_dir / f).as_posix(), hashlib.sha256(p.read_bytes()).hexdigest()))
    return sorted(out)


def digest_of(entries: list[tuple[str, str]]) -> str:
    h = hashlib.sha256()
    for rel, sha in entries:
        h.update(f"{sha} {rel}\n".encode())
    return h.hexdigest()


def tree_bytes(root: Path) -> int:
    return sum(p.stat().st_size for p in root.rglob("*") if p.is_file() and not p.is_symlink())


def cmd_copy(a) -> int:
    src, dst = Path(a.src), Path(a.dst)
    if not (src / "level.dat").is_file():
        return die(f"{src} holds no level.dat — not a world")
    if dst.exists():
        return die(f"{dst} already exists")

    def ignore(d, names):
        return [n for n in names if Path(d) == src and n in EXCLUDED_TOP]

    shutil.copytree(src, dst, ignore=ignore)
    try:
        after_src, after_dst = world_entries(src), world_entries(dst)
    except ValueError as e:
        return die(str(e))
    if after_src != after_dst:
        a_map, b_map = dict(after_src), dict(after_dst)
        diff = sorted(k for k in set(a_map) | set(b_map) if a_map.get(k) != b_map.get(k))
        return die(f"the copy differs from the world it was taken from at {len(diff)} path(s), first {diff[0]} — "
                   "the server wrote during the copy")
    files = sum(1 for _, s in after_dst if s != "dir")
    print(f"{digest_of(after_dst)}\t{files}\t{tree_bytes(dst)}")
    return 0


def cmd_digest(a) -> int:
    try:
        print(digest_of(world_entries(Path(a.dir))))
    except (ValueError, OSError) as e:
        return die(str(e))
    return 0


# ---- the records -----------------------------------------------------------

SEQ_DIR = re.compile(r"^\d{2,}$")


def checkpoints(image_root: Path) -> list[Path]:
    if not image_root.is_dir():
        return []
    return sorted((p for p in image_root.iterdir() if p.is_dir() and SEQ_DIR.match(p.name)
                   and (p / "checkpoint.json").is_file()), key=lambda p: int(p.name))


def cmd_next_seq(a) -> int:
    root = Path(a.image_root)
    seqs = [int(p.name) for p in root.iterdir() if p.is_dir() and SEQ_DIR.match(p.name)] if root.is_dir() else []
    n = max(seqs, default=0) + 1
    print(f"{n:02d}")
    return 0


def cmd_write_meta(a) -> int:
    d = Path(a.dir)
    doc = {
        "seq": a.seq,
        "name": a.name,
        "kind": a.kind,
        "image_id": a.image_id,
        "completed": sorted(a.completed.split()) if a.completed else [],
        "world_digest": a.digest,
        "world_files": a.files,
        "world_bytes": a.bytes,
    }
    (d / "checkpoint.json").write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


def read_json(p: Path) -> dict:
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}


def label(ck: Path) -> str:
    return f"{ck.name} {read_json(ck / 'checkpoint.json').get('name', '?')}"


def identity_line(image_root: Path) -> str:
    ident = read_json(image_root / "image.json")
    return (f"image {ident.get('image_ref', '?')} (id {ident.get('image_id', image_root.name)}, "
            f"manifest {ident.get('manifest_sha256', '?')}, campaign {ident.get('campaign', '?')})")


def cmd_list(a) -> int:
    root = Path(a.root)
    roots = sorted(p for p in root.iterdir() if p.is_dir()) if root.is_dir() else []
    total = 0
    for r in roots:
        cks = checkpoints(r)
        if not cks:
            continue
        mark = "  <- the session's image" if a.image_id and r.name == a.image_id.removeprefix("sha256:") else ""
        print(f"checkpoints of {identity_line(r)}{mark}:")
        for ck in cks:
            size = tree_bytes(ck)
            total += 1
            print(f"  {label(ck)}  ({human(size)})")
    if total == 0:
        print(f"checkpoints: none at {root}")
    return 0


def cmd_resolve(a) -> int:
    """A checkpoint by number (`3`, `03`), by its label (`03 Find the way`), or by
    its name alone when exactly one carries it. With --image-id, the search is
    that image's; a match only under another image is refused with both."""
    root = Path(a.root)
    q = a.query.strip()
    want = a.image_id.removeprefix("sha256:") if a.image_id else None
    roots = sorted(p for p in root.iterdir() if p.is_dir()) if root.is_dir() else []

    def matches(r: Path) -> list[Path]:
        hits = []
        for ck in checkpoints(r):
            name = read_json(ck / "checkpoint.json").get("name", "")
            if (q.isdigit() and int(q) == int(ck.name)) or q == label(ck) or q == name:
                hits.append(ck)
        return hits

    scope = [r for r in roots if want is None or r.name == want]
    hits = [h for r in scope for h in matches(r)]
    if len(hits) == 1:
        print(hits[0])
        return 0
    if len(hits) > 1:
        return die(f"'{q}' names {len(hits)} checkpoints — name one by its number:\n  "
                   + "\n  ".join(f"{label(h)}  ({identity_line(h.parent)})" for h in hits))
    elsewhere = [h for r in roots if r not in scope for h in matches(r)]
    if elsewhere and want is not None:
        here = root / want
        return die(f"'{q}' is a checkpoint of another build, and this session serves another: REFUSING to restore "
                   f"it here.\n  checkpoint {label(elsewhere[0])} was taken on {identity_line(elsewhere[0].parent)}\n"
                   f"  this session serves {identity_line(here) if (here / 'image.json').is_file() else 'image id ' + want}")
    return die(f"no checkpoint '{q}'" + (f" of image id {want}" if want else "") + f" under {root}")


def main() -> int:
    sys.stdout.reconfigure(newline="\n")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("quests")
    p.add_argument("--datapack", required=True)
    p.set_defaults(fn=cmd_quests)
    p = sub.add_parser("copy")
    p.add_argument("--src", required=True)
    p.add_argument("--dst", required=True)
    p.set_defaults(fn=cmd_copy)
    p = sub.add_parser("digest")
    p.add_argument("--dir", required=True)
    p.set_defaults(fn=cmd_digest)
    p = sub.add_parser("next-seq")
    p.add_argument("--image-root", required=True)
    p.set_defaults(fn=cmd_next_seq)
    p = sub.add_parser("write-meta")
    for f in ("dir", "seq", "name", "kind", "image-id", "digest"):
        p.add_argument(f"--{f}", required=True)
    p.add_argument("--completed", default="")
    p.add_argument("--files", type=int, required=True)
    p.add_argument("--bytes", type=int, required=True)
    p.set_defaults(fn=cmd_write_meta)
    p = sub.add_parser("list")
    p.add_argument("--root", required=True)
    p.add_argument("--image-id", default="")
    p.set_defaults(fn=cmd_list)
    p = sub.add_parser("resolve")
    p.add_argument("--root", required=True)
    p.add_argument("--query", required=True)
    p.add_argument("--image-id", default="")
    p.set_defaults(fn=cmd_resolve)
    a = ap.parse_args()
    return a.fn(a)


if __name__ == "__main__":
    sys.exit(main())
