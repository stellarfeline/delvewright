#!/usr/bin/env python3
"""Which CI job groups a pull request's change can reach — the one filter.

`.github/ci-reach.toml` is the table: for every job group, the paths the jobs
reading that group actually read, each with the reader that reads it. This module
is the only thing that interprets it. Two callers:

* the `changes` job in `ci.yml` runs `python3 tools/lib/ci_reach.py groups`,
  which writes one `<group>=true|false` line per group for `$GITHUB_OUTPUT`;
* `tools/check-ci-reach.py` imports `load_table` and `glob_regex` to hold the
  table to the jobs.

THE RULE

On `pull_request`, a group is on when a changed path matches one of its globs, or
when a changed path matches the `[all]` set (the workflow, `.github/**`, the lock
file and the pins every job reads — and this file, since it decides the rest).
On any other event every group is on, with no diff computed: `push` to `main` and
the plugin release's `workflow_dispatch` run everything.

The changed paths are `git diff --name-only --no-renames <base> <head>`. With
`--no-renames` a move lists both the old and the new path, so a job that read the
old path is reached by the move. A diff that cannot be computed is an error, never
an empty change set.

GLOBS

`**` matches any number of whole path segments (zero included), `*` matches
within one segment, `?` one character within a segment. Nothing else is special.
A glob is anchored at the repository root.

Usage:
  python3 tools/lib/ci_reach.py groups --event EVENT [--base SHA --head SHA]
      [--table .github/ci-reach.toml] [--summary FILE]

Prints `<group>=true|false`, one per group, to stdout; the reasoning (changed-path
count, and for every group that is on, the first path that turned it on) goes to
stderr and, with `--summary`, to that file as markdown.

Exit 0 on success, 2 when the table or the diff cannot be read.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass

REPO = pathlib.Path(__file__).resolve().parent.parent.parent
TABLE = REPO / ".github" / "ci-reach.toml"
GROUP_NAME = re.compile(r"^[a-z][a-z0-9-]*$")


class TableError(ValueError):
    """The table is unreadable or malformed."""


@dataclass(frozen=True)
class Input:
    glob: str
    why: str


@dataclass(frozen=True)
class Table:
    all: tuple[Input, ...]
    groups: dict[str, tuple[Input, ...]]


def glob_regex(glob: str) -> re.Pattern[str]:
    """Compile one table glob to an anchored regex over a repo-relative path."""
    if not glob or glob.startswith("/") or glob.endswith("/"):
        raise TableError(f"glob {glob!r}: must be a non-empty repo-relative path")
    out: list[str] = []
    segments = glob.split("/")
    for n, seg in enumerate(segments):
        last = n == len(segments) - 1
        if seg == "**":
            # zero or more whole segments; at the end it also matches a file
            out.append(".*" if last else "(?:[^/]+/)*")
            continue
        if "**" in seg:
            raise TableError(f"glob {glob!r}: `**` must be a whole segment")
        piece = ""
        for ch in seg:
            if ch == "*":
                piece += "[^/]*"
            elif ch == "?":
                piece += "[^/]"
            else:
                piece += re.escape(ch)
        out.append(piece if last else piece + "/")
    return re.compile("^" + "".join(out) + "$")


def _inputs(where: str, raw: object) -> tuple[Input, ...]:
    if not isinstance(raw, list) or not raw:
        raise TableError(f"{where}: `inputs` must be a non-empty array")
    got: list[Input] = []
    for n, item in enumerate(raw):
        if not isinstance(item, dict) or set(item) != {"glob", "why"}:
            raise TableError(f"{where}: input #{n + 1} must carry exactly `glob` and `why`")
        glob, why = item["glob"], item["why"]
        if not isinstance(glob, str) or not isinstance(why, str) or not why.strip():
            raise TableError(f"{where}: input #{n + 1} needs a string glob and a non-empty why")
        glob_regex(glob)
        got.append(Input(glob, why))
    globs = [i.glob for i in got]
    dupes = sorted({g for g in globs if globs.count(g) > 1})
    if dupes:
        raise TableError(f"{where}: glob(s) listed twice: {', '.join(dupes)}")
    return tuple(got)


def load_table(path: pathlib.Path = TABLE) -> Table:
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise TableError(f"{path}: {exc}") from exc
    extra = set(data) - {"all", "group"}
    if extra:
        raise TableError(f"{path}: unknown top-level key(s): {', '.join(sorted(extra))}")
    all_raw = data.get("all")
    if not isinstance(all_raw, dict):
        raise TableError(f"{path}: `[all]` is missing")
    everything = _inputs("[all]", all_raw.get("inputs"))
    groups: dict[str, tuple[Input, ...]] = {}
    raw_groups = data.get("group")
    if not isinstance(raw_groups, list) or not raw_groups:
        raise TableError(f"{path}: no `[[group]]` entries")
    for n, g in enumerate(raw_groups):
        if not isinstance(g, dict) or set(g) != {"name", "inputs"}:
            raise TableError(f"{path}: group #{n + 1} must carry exactly `name` and `inputs`")
        name = g["name"]
        if not isinstance(name, str) or not GROUP_NAME.match(name):
            raise TableError(f"{path}: group #{n + 1} has an invalid name {name!r}")
        if name in groups:
            raise TableError(f"{path}: group {name!r} is declared twice")
        groups[name] = _inputs(f"group {name!r}", g["inputs"])
    return Table(everything, groups)


def first_match(inputs: tuple[Input, ...], paths: list[str]) -> str | None:
    regexes = [glob_regex(i.glob) for i in inputs]
    for p in paths:
        if any(r.match(p) for r in regexes):
            return p
    return None


def decide(table: Table, event: str, changed: list[str] | None) -> dict[str, str | None]:
    """Group -> the reason it is on (a path or an event), or None when it is off."""
    if event != "pull_request":
        return {g: f"event `{event}`" for g in table.groups}
    assert changed is not None
    hit = first_match(table.all, changed)
    if hit is not None:
        return {g: f"`{hit}` (every job reads it)" for g in table.groups}
    return {g: first_match(inputs, changed) for g, inputs in table.groups.items()}


def rev_parse(rev: str, repo: pathlib.Path = REPO) -> str:
    r = subprocess.run(["git", "-C", str(repo), "rev-parse", "--verify", f"{rev}^{{commit}}"], capture_output=True, text=True)
    if r.returncode != 0:
        raise TableError(f"`{rev}` does not name a commit here: {r.stderr.strip()}")
    return r.stdout.strip()


def changed_paths(base: str, head: str, repo: pathlib.Path = REPO) -> list[str]:
    r = subprocess.run(
        ["git", "-C", str(repo), "diff", "--name-only", "--no-renames", "-z", base, head],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        raise TableError(f"`git diff {base} {head}` exited {r.returncode}: {r.stderr.strip()}")
    return sorted(p for p in r.stdout.split("\0") if p)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    g = sub.add_parser("groups", help="print <group>=true|false for $GITHUB_OUTPUT")
    g.add_argument("--event", required=True)
    g.add_argument("--base")
    g.add_argument("--head")
    g.add_argument("--table", type=pathlib.Path, default=TABLE)
    g.add_argument("--summary", type=pathlib.Path)
    args = ap.parse_args(argv)
    sys.stdout.reconfigure(newline="\n")

    try:
        table = load_table(args.table)
        changed: list[str] | None = None
        if args.event == "pull_request":
            if not args.base or not args.head:
                print("ci-reach: a pull_request needs --base and --head", file=sys.stderr)
                return 2
            base, head = rev_parse(args.base), rev_parse(args.head)
            changed = changed_paths(base, head)
    except TableError as exc:
        print(f"ci-reach: FATAL — {exc}", file=sys.stderr)
        return 2

    verdict = decide(table, args.event, changed)
    on = [grp for grp, why in verdict.items() if why is not None]
    lines = []
    if changed is None:
        lines.append(f"event `{args.event}`: every group runs, no diff computed.")
    else:
        lines.append(f"pull_request {base}..{head}: {len(changed)} changed path(s).")
    lines.append(f"{len(on)} of {len(verdict)} group(s) on.")
    for grp, why in verdict.items():
        lines.append(f"- `{grp}`: " + (f"runs — {why}" if why else "skipped — no changed path is an input"))
    for grp, why in verdict.items():
        print(f"{grp}={'true' if why else 'false'}")
    print("\n".join(lines), file=sys.stderr)
    if args.summary:
        with args.summary.open("a", encoding="utf-8", newline="\n") as fh:
            fh.write("## Which jobs this change reaches\n\n" + "\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
