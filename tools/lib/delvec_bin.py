"""Resolving the `delvec` a tool runs, and NAMING it on every run.

## The defect this exists to end

Five tools in `tools/` run a `delvec` binary and derive their whole verdict from
it. Each resolved one privately and none said which:

- `check-gallery-coverage.py` searched `target/release/delvec` then
  `target/debug/delvec` and printed neither;
- `gallery-baseline.py`, `gallery-build.py`, `check-gallery-render.py` and
  `check-whole-map-render.py` each DEFAULTED to `target/release/delvec`.

A reviewer's worktree carried a release binary nine days old. The coverage gate
found it first, answered about the compiler that binary was built from, and
reported a false red about a schema field that had already landed. Nothing
errored, and the output named no instrument, so there was nothing in it to
disagree with.

That is `CLAUDE.md`'s frozen-measurement rule with the instrument *inside* the
thing being measured: the unit set these gates judge against is "the compiler in
THIS tree", so a stale binary does not make the answer noisy — it makes the
answer be about a tree that no longer exists, in the reassuring direction.

## What this module demands

1. **The instrument is named, every run**, on stderr: the resolved path, its
   mtime, and the `--version` line the binary itself answered. A measurement
   that names its instrument can be contradicted by a reader; one that does not
   cannot.
2. **A binary older than the tree it is supposed to be built from is a
   REFUSAL**, not a warning. The remedy is one command and it is printed.

The staleness key is the newest mtime among the files that reach the binary.
Cargo records them: every build writes a dep-info file beside the binary
(`target/<profile>/delvec.d`) listing each source and `include_str!` input the
build read, across the workspace's own crates. Those files, plus every tracked
`Cargo.toml` under `crates/` and the workspace manifest and lockfile (which
decide what the binary links against and are not in dep-info), are the key. A
file the build never read — an integration test under `tests/`, a
`#[cfg(test)]` module — cannot change the binary and cannot make it stale; a new
source file reaches the build only through a `mod` line in a file dep-info
already lists, so it moves that file's mtime.

Where no dep-info sits beside the binary (a binary copied out of `target/`),
the key falls back to every tracked file under `crates/`, plus the manifest and
lockfile: `git ls-files` rather than a directory walk because tracked-files-only
is git's ignore rule rather than a hand-written exclusion list, which `CLAUDE.md`
refuses ("never add an ignore list to make an audit green") — a build artifact
under a crate would otherwise make every run refuse forever. Where git cannot
answer, the walk is used and skips directories named `target`. The printed line
says which method decided.

There is deliberately **no override**. An opt-out here would be secured by
exactly the property the defect supplies — "I know my binary is fine" is what
the reviewer with the nine-day-old binary also believed.

## Why the refusal cannot fire spuriously in CI

Every CI caller checks out first and builds second, so the binary's mtime is
later than every source file's by construction. `Swatinem/rust-cache` prunes the
workspace's own artifacts before saving, so a restored cache never supplies the
binary; the `cargo build` step does, at the moment it runs.
"""

from __future__ import annotations

import datetime as _dt
import re
import subprocess
import sys
from pathlib import Path

# The sources whose mtime decides whether a built `delvec` is current. `crates/`
# is the compiler and everything it links; the manifest and lockfile decide what
# it links against.
SOURCE_ROOTS = ("crates", "Cargo.toml", "Cargo.lock")

# Searched in this order when no path is given, and the order is the whole
# hazard: a release binary that happens to exist wins over a debug one built
# five minutes ago. It is kept because a person who has built `--release` means
# it — what was missing was saying so and checking its age.
SEARCH = ("target/release/delvec", "target/debug/delvec")

BUILD_HINT = "cargo build -p delvec --bin delvec        (add --release for the release path)"


class StaleInstrument(Exception):
    """A resolved binary older than the sources it is supposed to be built from."""


def _stamp(ts: float) -> str:
    return (
        _dt.datetime.fromtimestamp(ts, _dt.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z")
    )


def tracked_sources(repo: Path) -> tuple[list[Path], str]:
    """Every file whose change could change the binary, and how they were found.

    Returns (paths, method). `method` is printed, so a reader can tell a git
    answer from the fallback walk without guessing.
    """
    r = subprocess.run(
        ["git", "-C", str(repo), "ls-files", "-z", "--", *SOURCE_ROOTS],
        capture_output=True,
        text=True,
    )
    if r.returncode == 0:
        paths = [repo / p for p in r.stdout.split("\0") if p]
        if paths:
            return paths, "git ls-files"

    paths = []
    for root in SOURCE_ROOTS:
        p = repo / root
        if p.is_file():
            paths.append(p)
        elif p.is_dir():
            for q in p.rglob("*"):
                if q.is_file() and "target" not in q.relative_to(p).parts:
                    paths.append(q)
    return paths, "directory walk (git could not answer)"


def _dep_info_paths(dep_info: Path) -> list[Path]:
    """The prerequisites of a Makefile-style dep-info file, as cargo writes it.

    Cargo writes `target: dep dep ...`, escaping a space in a path as `\\ `.
    Every target line's prerequisites are kept; a line continuation is joined.
    """
    text = dep_info.read_text(encoding="utf-8").replace("\\\n", " ")
    paths: list[Path] = []
    for line in text.splitlines():
        # The target ends at the first `: ` that is not escaped.
        m = re.search(r"(?<!\\): ", line)
        if not m:
            continue
        for tok in re.split(r"(?<!\\)\s+", line[m.end():].strip()):
            if tok:
                paths.append(Path(tok.replace("\\ ", " ")))
    return paths


def build_inputs(delvec: Path, repo: Path) -> tuple[list[Path], str]:
    """Every file whose change could change `delvec`, and how they were found.

    Cargo's dep-info beside the binary when it exists, plus the manifests and
    lockfile; otherwise every tracked compiler source (`tracked_sources`).
    """
    dep_info = delvec.with_name(delvec.name + ".d")
    if dep_info.is_file():
        # Cargo writes absolute paths unless `build.dep-info-basedir` is set,
        # which makes them relative to the workspace root.
        listed = [q if q.is_absolute() else repo / q for q in _dep_info_paths(dep_info)]
        if listed:
            r = subprocess.run(
                ["git", "-C", str(repo), "ls-files", "-z", "--",
                 "Cargo.toml", "Cargo.lock", "crates/**/Cargo.toml"],
                capture_output=True,
                text=True,
            )
            manifests = (
                [repo / p for p in r.stdout.split("\0") if p]
                if r.returncode == 0
                else [repo / "Cargo.toml", repo / "Cargo.lock"]
            )
            return listed + manifests, f"cargo dep-info `{dep_info.name}` plus manifests"
    return tracked_sources(repo)


def newest_source(delvec: Path, repo: Path) -> tuple[Path | None, float, int, str]:
    """The most recently modified build input, its mtime, the population, method."""
    paths, method = build_inputs(delvec, repo)
    newest, newest_ts = None, 0.0
    for p in paths:
        try:
            ts = p.stat().st_mtime
        except OSError:
            continue
        if ts > newest_ts:
            newest, newest_ts = p, ts
    return newest, newest_ts, len(paths), method


def _rel(p: Path, repo: Path) -> Path:
    try:
        return p.resolve().relative_to(repo.resolve())
    except ValueError:
        return p


def version_line(delvec: Path) -> str:
    r = subprocess.run([str(delvec), "--version"], capture_output=True, text=True)
    if r.returncode != 0:
        return f"(`--version` exited {r.returncode})"
    return " ".join((r.stdout + r.stderr).split()) or "(said nothing)"


def resolve(
    explicit: str | Path | None,
    *,
    repo: Path,
    caller: str,
    required: bool = False,
    stream=None,
) -> Path:
    """Resolve the binary, name it on `stream`, and refuse a stale one.

    `required=True` for a tool that must be handed its engine (the caller's
    whole verdict is about WHICH engine, so inferring one would be answering a
    question nobody asked). Everything else keeps the search and pays for it by
    printing what it found.

    Raises SystemExit(1) with a named reason: nothing found, not a file, or the
    stale-instrument refusal.
    """
    out = sys.stderr if stream is None else stream

    def die(msg: str) -> "Path":
        print(f"{caller}: FAIL — {msg}", file=out)
        raise SystemExit(1)

    if explicit:
        delvec = Path(explicit)
        if not delvec.is_file():
            die(f"--delvec `{delvec}` is not a file. Build one:\n    {BUILD_HINT}")
    elif required:
        die(
            "--delvec is required and is never inferred: this tool's verdict is "
            "about WHICH engine produced the artifact, so the caller names it.\n"
            f"    {BUILD_HINT}"
        )
    else:
        found = [repo / rel for rel in SEARCH if (repo / rel).is_file()]
        if not found:
            die(
                "no delvec binary found at "
                + " or ".join(f"`{rel}`" for rel in SEARCH)
                + ". The unit set is derived from the compiler in THIS tree and "
                "from nothing else, so there is no fallback:\n"
                f"    {BUILD_HINT}\n"
                "    or pass --delvec."
            )
        delvec = found[0]

    built = delvec.stat().st_mtime
    newest, newest_ts, population, method = newest_source(delvec, repo)

    print(
        f"{caller}: instrument {delvec} "
        f"(built {_stamp(built)}, {version_line(delvec)}); "
        f"newest of {population} build input(s) via {method}: "
        + (f"{_rel(newest, repo) if newest else '(none)'} at {_stamp(newest_ts)}"
           if newest
           else "(none found)"),
        file=out,
    )

    if newest is not None and built < newest_ts:
        die(
            f"STALE INSTRUMENT — `{delvec}` was built at {_stamp(built)} and "
            f"`{_rel(newest, repo)}` was modified at {_stamp(newest_ts)}, "
            "which is later.\n"
            "    This tool's answer is a property of the compiler in this tree. A "
            "binary older than the tree answers about a tree that no longer "
            "exists, and it does so in the direction that reads as a clean pass "
            "or as a confident false red — which is how a release binary nine "
            "days old once reported a missing schema field that had landed.\n"
            f"    Rebuild it:\n    {BUILD_HINT}"
        )

    return delvec
