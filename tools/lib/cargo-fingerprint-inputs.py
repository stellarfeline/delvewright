#!/usr/bin/env python3
r"""What two trees must AGREE about before one's `target/` is worth cloning into the other.

WHY THIS EXISTS

`tools/worktree-new.sh` clones a donor's `target/` into a new worktree with
`cp -c`. That is only a saving while the cloned units are still VALID, and cargo
decides validity by a fingerprint. So the script already refuses when the two
trees resolve different `rustc` — the trap that made the original measurement run
build with the wrong compiler, invalidate every cloned unit, rebuild all 140
packages and report that cloning saves nothing.

The rustc version is not the only fingerprint input, and it was the only one
compared. **Profile settings are fingerprint inputs too**, and this repository
moves them: `[profile.dev]` gained `debug = "line-tables-only"` after
`opt-level = 1`. A worktree cloned from a donor whose `target/` predates such a
change passes a rustc-only refusal, then invalidates every cloned unit anyway.

The wasted build is the small half. The large half is that it reproduces, exactly,
a symptom this repository has already recorded and MISDIAGNOSED — *"rebuilt all
140 packages and reported that cloning saves nothing"* — and it wears the costume
of a regression in the clone tool. The trap is laid for the next round, and the
last round that walked into it needed three independent confirmations to find the
cause.

WHAT IS COMPARED — 7 inputs, each established per tree

  1. `rustc`             `rustc -vV` run with the tree as CWD. Supersedes
                         `rustc --version`: it carries the host triple, the
                         commit hash and the LLVM version, and it is resolved the
                         way a build resolves it (rustup walks up from the CWD to
                         find `rust-toolchain.toml` — it has no manifest flag, and
                         that is the original defect in this area). Comparing the
                         resolved answer is strictly stronger than comparing
                         `rust-toolchain.toml`, which is only the question.
  2. `cargo`             `cargo -vV`, same way. Cargo owns the fingerprint FORMAT,
                         so its version decides what a stored fingerprint means.
                         Near-redundant with (1) because rustup resolves both from
                         one toolchain file — but that coupling is a convention,
                         not a guarantee, and `$CARGO` overrides it.
  3. `manifest_profiles` the `[profile.*]` tables of the tree's ROOT `Cargo.toml`
                         — the workspace whose `target/` is the thing being
                         cloned. Only these tables, never the whole manifest: a
                         donor is normally `main` and the new tree is a branch, so
                         hashing the manifest would refuse on any ordinary
                         dependency edit and the tool would degrade to
                         `--no-clone`, which is a loosening wearing a refusal's
                         clothes.
  4. `config_profiles`   the `profile` table of every `.cargo/config.toml`
  5. `config_build`      the `build` table  ...
  6. `config_target`     the `target` table ...
  7. `config_env`        the `env` table    ... on the tree root and on every
                         ancestor directory up to the filesystem root. These four
                         are the config sections a fingerprint can depend on
                         (`build.rustflags`, `[env]`, a per-target rustflags
                         override); `net`, `http`, `registries`, `source` and
                         `alias` cannot reach a fingerprint and are not read.

The ancestor chain is recorded as an ORDERED LIST OF TABLES, nearest first, and
never merged. Cargo's merge rules are their own subject (arrays join, scalars are
overridden) and re-implementing them here would be a second instrument with no
calibration. Order alone decides precedence, so comparing the ordered list is
faithful and conservative. Paths are deliberately dropped from that list: a donor
at `~/projects/engine` and a worktree at `/private/tmp/.../engine` sit at
different depths, and comparing depths would refuse on an accident of where the
tree was put.

WHAT IS DELIBERATELY NOT COMPARED, AND WHY

"Everything a fingerprint depends on" is not fully enumerable, and pretending
otherwise would be worse than a stated limit. What is left out, and the reason:

  * `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_PROFILE_*`, `CARGO_BUILD_*` and
    every other environment override. These are read from the environment of the
    BUILD, which happens later, in whatever shell the worker uses. The creation-
    time environment is one process and is therefore equal on both sides by
    construction — so comparing it would be a green that measures nothing, which
    is the vacuity this repository already names. A build launched with a
    different `RUSTFLAGS` than the donor's is outside what this can see, and no
    check here can close it.
  * `$CARGO_HOME/config.toml`. Read by cargo, but identical for both trees by
    construction — one process, one environment. Including it would inflate the
    stated count without adding power.
  * `Cargo.lock`. It differs legitimately whenever the new tree is a branch that
    touched a dependency, and a differing lock invalidates only the units whose
    dependency set actually moved. Refusing on it would refuse the common cheap
    case. `worktree-new.sh` reports it as a NOTE instead, which is what it was.
  * The build command itself — `--release`, `--all-targets`, `--features`. These
    are properties of an invocation that has not happened yet.
  * Environment variables a `build.rs` declares with `cargo:rerun-if-env-changed`.
    Enumerating them means running the build scripts, which is the build this
    exists to avoid. THIS is where the enumeration genuinely stops.
  * A `.cargo/config.toml` in a SUB-directory of the tree. Cargo reads it only for
    a build launched from that sub-directory, and `worktree-new.sh` closes by
    telling the reader to build from the tree root.
  * `[profile.*]` in the nested workspaces (`crates/render`, `prefabs/*-generator`,
    `docs/experiments/…`). Each keeps its own `target/` inside its own directory,
    and none of them is what `worktree-new.sh` clones.

Exit 0 when the two trees agree, 1 when they differ (naming every differing
input), 2 when agreement could not be ESTABLISHED — a missing `rustc`, an
unreadable manifest. Two is a refusal and not a pass: this fails closed, because
an unestablished agreement is the state the whole tool exists to distrust.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from pathlib import Path

# The order here is the order a difference is reported in, cheapest cause first.
INPUTS = (
    "rustc",
    "cargo",
    "manifest_profiles",
    "config_profiles",
    "config_build",
    "config_target",
    "config_env",
)

# The `.cargo/config.toml` sections a unit's fingerprint can depend on, mapped to
# the key this tool reports them under. Anything absent from this table is
# deliberately unread (see the module docstring).
CONFIG_SECTIONS = {
    "profile": "config_profiles",
    "build": "config_build",
    "target": "config_target",
    "env": "config_env",
}


class Unestablished(Exception):
    """Agreement could not be established — refuse rather than guess."""


def _tool_version(tree: Path, tool: str) -> str:
    """`<tool> -vV` resolved the way a build in `tree` resolves it.

    Run with `cwd=tree` rather than after a `cd`, so the caller's own working
    directory cannot drift — a `cd` in the first clause of a compound command
    persists through the rest of it, and this repository has made `git` and `gh`
    answer confidently about the wrong tree that way.
    """
    try:
        done = subprocess.run(
            [tool, "-vV"],
            cwd=tree,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as exc:
        raise Unestablished(f"{tool} could not be run in {tree}: {exc}") from exc
    if done.returncode != 0:
        raise Unestablished(
            f"{tool} -vV exited {done.returncode} in {tree}: "
            f"{(done.stderr or done.stdout).strip().splitlines()[:1]}"
        )
    return done.stdout.strip()


def _read_toml(path: Path) -> dict:
    try:
        with path.open("rb") as handle:
            return tomllib.load(handle)
    except OSError as exc:
        raise Unestablished(f"{path} could not be read: {exc}") from exc
    except tomllib.TOMLDecodeError as exc:
        raise Unestablished(f"{path} is not valid TOML: {exc}") from exc


def _manifest_profiles(tree: Path) -> dict:
    manifest = tree / "Cargo.toml"
    if not manifest.is_file():
        raise Unestablished(f"{manifest} does not exist — this is not a cargo tree")
    return _read_toml(manifest).get("profile", {})


def _config_chain(tree: Path) -> list[dict]:
    """Every `.cargo/config[.toml]` from the tree root upward, NEAREST FIRST.

    Cargo prefers `config.toml` to the extensionless `config` in one directory and
    reads at most one of them, so this does too.
    """
    chain: list[dict] = []
    for directory in [tree, *tree.parents]:
        for name in ("config.toml", "config"):
            candidate = directory / ".cargo" / name
            if candidate.is_file():
                chain.append(_read_toml(candidate))
                break
    return chain


def inputs(tree: Path) -> dict:
    """The 7 comparable fingerprint inputs of one tree."""
    tree = tree.resolve()
    if not tree.is_dir():
        raise Unestablished(f"{tree} is not a directory")
    chain = _config_chain(tree)
    result = {
        "rustc": _tool_version(tree, "rustc"),
        "cargo": _tool_version(tree, "cargo"),
        "manifest_profiles": _manifest_profiles(tree),
    }
    for section, key in CONFIG_SECTIONS.items():
        # Paths are dropped on purpose: two trees at different depths hold the
        # same ancestor configuration, and depth is not a fingerprint input.
        result[key] = [c[section] for c in chain if section in c]
    missing = set(INPUTS) - set(result)
    if missing:  # pragma: no cover - guards the INPUTS/collector pair
        raise Unestablished(f"collector produced no value for {sorted(missing)}")
    return result


def _render(value: object) -> str:
    if isinstance(value, str):
        return value
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def diff(donor: Path, new: Path) -> tuple[int, list[str]]:
    """Compare two trees. Returns (exit code, lines to print)."""
    try:
        a = inputs(donor)
        b = inputs(new)
    except Unestablished as exc:
        return 2, [
            f"UNESTABLISHED — {exc}",
            "Refusing rather than guessing: a clone whose validity was never "
            "established is the state this check exists to distrust.",
        ]

    differing = [k for k in INPUTS if a[k] != b[k]]
    if not differing:
        return 0, [
            f"all {len(INPUTS)} compared fingerprint inputs agree "
            f"({', '.join(INPUTS)})",
            f"toolchain on both sides: {a['rustc'].splitlines()[0]}",
        ]

    lines = [
        f"{len(differing)} of {len(INPUTS)} compared fingerprint inputs DIFFER:",
    ]
    for key in differing:
        lines.append(f"  {key}")
        lines.append(f"    donor: {_render(a[key])}")
        lines.append(f"    new  : {_render(b[key])}")
    return 1, lines


def main(argv: list[str] | None = None) -> int:
    sys.stdout.reconfigure(newline="\n")
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--json", metavar="DIR", help="print one tree's comparable inputs as JSON"
    )
    parser.add_argument(
        "--diff",
        nargs=2,
        metavar=("DONOR", "NEW"),
        help="compare two trees; exit 0 same, 1 differing, 2 unestablished",
    )
    args = parser.parse_args(argv)

    if args.json:
        try:
            payload = inputs(Path(args.json))
        except Unestablished as exc:
            print(f"UNESTABLISHED — {exc}", file=sys.stderr)
            return 2
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    if args.diff:
        code, lines = diff(Path(args.diff[0]), Path(args.diff[1]))
        for line in lines:
            print(line)
        return code

    parser.print_help()
    return 2


if __name__ == "__main__":
    sys.exit(main())
