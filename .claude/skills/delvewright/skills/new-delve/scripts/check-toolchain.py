#!/usr/bin/env python3
"""Is the toolchain on this machine the one the page beside this script pins?

Run on every run, right after I0 and I1, before any step reads a campaign
document or runs a `delvec` subcommand. It compares what is on disk with the
`versions.toml` beside it, prints every comparison it made with both sides, and
exits with a code the page binds to.

WHAT IT COMPARES

    the binary       `delvec --version`, as `. ~/.delvewright/env.sh && delvec`
                     resolves it — the binary every later command runs, not a
                     path this script guesses. Creator mode: equal to the pin's
                     `[engine].release`. Dev mode: equal to the checkout's own
                     `versions.toml` `[engine].version` (Init I3b).
    the engine tree  creator mode: `git -C <engine> rev-parse HEAD` equal to the
                     pin's `[engine].ref`. Dev mode: the checkout is the engine
                     under work, so HEAD is printed and not compared.
    env.sh           `DELVEWRIGHT_SKILL` equal to the skill root this script
                     lives in, and `DELVEWRIGHT_MODE` / `DELVEWRIGHT_ENGINE`
                     equal to this run's I0 — read by sourcing the file in `sh`,
                     the way every later command reads it.

The skill root is this script's own location, so it is invoked by the path I1
resolved in THIS run. A path read out of `env.sh` names whichever skill root
wrote that file, and its script would compare that root's pin with itself.

EXIT CODES, BECAUSE THE PAGE BINDS TO THEM RATHER THAN TO PROSE

    0  every comparison agrees: I2, I3 and I4 stand as they are
    2  this script cannot run at all (the pin, env.sh or a flag is unusable)
    3  a comparison disagrees: run the Init steps it names, in the order it
       prints them, then this script again — nothing else runs before it exits 0
    4  there is no toolchain on this machine (`env.sh` is absent): Init, I2-I8

    python3 scripts/check-toolchain.py --mode creator --engine ~/.delvewright/engine
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import pathlib
import subprocess
import sys
import tomllib

EXIT_UNUSABLE = 2
EXIT_MISMATCH = 3
EXIT_NO_TOOLCHAIN = 4

HERE = pathlib.Path(__file__).resolve().parent
SKILL_ROOT = HERE.parent

# The variables read back out of `env.sh`, and the ones cleared before sourcing
# it: a value inherited from the caller's shell must not stand in for a line the
# file does not carry.
ENV_KEYS = ("DELVEWRIGHT_MODE", "DELVEWRIGHT_ENGINE", "DELVEWRIGHT_SKILL")
CLEARED = ENV_KEYS + ("DELVEWRIGHT_PYTHON", "DELVEWRIGHT_PREFABS", "JAVA_HOME")

# The steps a disagreement is repaired by, in the order Init runs them.
ORDER = ("I2", "I3a", "I3b", "I4")


def _fetch_delvec():
    """`fetch-delvec.py`: its pin reader, its refusal and its version pattern."""
    spec = importlib.util.spec_from_file_location("fetch_delvec", HERE / "fetch-delvec.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


FD = _fetch_delvec()

# Sourced in `sh`, then one line per variable, then where `delvec` resolves and
# the first line it answers. Every value is printed even when empty, so the
# line count is fixed.
PROBE = (
    '. "$1" || exit 97\n'
    + "".join(f'printf "%s\\n" "${key}"\n' for key in ENV_KEYS)
    + 'p="$(command -v delvec)"; printf "%s\\n" "$p"\n'
    + 'v="$(delvec --version 2>/dev/null | head -n 1)"; printf "%s\\n" "$v"\n'
)


def read_env(env: pathlib.Path) -> tuple[dict[str, str], str, str]:
    """`(variables, delvec path, delvec --version line)` as `env.sh` makes them."""
    environ = {k: v for k, v in os.environ.items() if k not in CLEARED}
    proc = subprocess.run(
        ["sh", "-c", PROBE, "sh", str(env)], capture_output=True, text=True, env=environ
    )
    lines = proc.stdout.split("\n")
    if proc.returncode != 0 or len(lines) < len(ENV_KEYS) + 2:
        raise FD.Refusal(
            EXIT_UNUSABLE,
            f"{env} does not source cleanly in sh (exit {proc.returncode}): "
            f"{proc.stderr.strip()}",
        )
    values = dict(zip(ENV_KEYS, lines))
    return values, lines[len(ENV_KEYS)].strip(), lines[len(ENV_KEYS) + 1].strip()


def head(engine: pathlib.Path) -> str:
    if not (engine / ".git").exists():
        return ""
    proc = subprocess.run(
        ["git", "-C", str(engine), "rev-parse", "HEAD"], capture_output=True, text=True
    )
    return proc.stdout.strip() if proc.returncode == 0 else ""


def checkout_version(engine: pathlib.Path) -> str:
    path = engine / "versions.toml"
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))["engine"]["version"]
    except (OSError, tomllib.TOMLDecodeError, KeyError) as exc:
        raise FD.Refusal(
            EXIT_UNUSABLE,
            f"dev mode holds the binary to {path}'s `[engine].version`, and that "
            f"file is unusable: {exc}",
        ) from exc


def same_path(got: str, want: str) -> bool:
    return bool(got) and pathlib.Path(got).resolve() == pathlib.Path(want).resolve()


def run(mode: str, engine: pathlib.Path, env: pathlib.Path, pin: pathlib.Path) -> int:
    _repo, release, ref = FD.read_pin(pin)
    print(f"pin check: skill root {SKILL_ROOT}")
    print(f"pin check: its pin names release {release}, ref {ref}")

    if not env.is_file():
        print(f"pin check: {env} is absent — there is no toolchain on this machine")
        print("pin check: REFUSED — run Init from I2 through I8, then this check again")
        return EXIT_NO_TOOLCHAIN

    values, binary, answer = read_env(env)
    repairs: set[str] = set()

    def compare(what: str, found: str, want: str, agrees: bool, repair: str) -> None:
        verdict = "agrees" if agrees else f"DISAGREES, repaired by {repair}"
        print(f"pin check: {what}: found {found or 'nothing'}, want {want} — {verdict}")
        if not agrees:
            repairs.add(repair)

    # -- the engine tree -----------------------------------------------------
    tree = head(engine)
    if mode == "creator":
        compare(f"engine tree {engine} HEAD", tree, ref, tree == ref, "I2")
    else:
        print(
            f"pin check: engine tree {engine} HEAD {tree or 'nothing'} — dev mode, "
            f"recorded and not compared"
        )

    # -- the binary ----------------------------------------------------------
    m = FD.VERSION_RE.match(answer)
    found = m.group("version") if m else None
    if mode == "creator":
        want, repair = release.lstrip("v"), "I3a"
    else:
        want, repair = checkout_version(engine), "I3b"
    compare(
        f"`delvec --version` through env.sh, at {binary or 'no delvec on its PATH'}",
        answer,
        f"delvec {want}",
        found == want,
        repair,
    )

    # -- env.sh --------------------------------------------------------------
    got = values["DELVEWRIGHT_MODE"]
    compare("env.sh DELVEWRIGHT_MODE", got, mode, got == mode, "I4")
    got = values["DELVEWRIGHT_ENGINE"]
    compare("env.sh DELVEWRIGHT_ENGINE", got, str(engine), same_path(got, str(engine)), "I4")
    got = values["DELVEWRIGHT_SKILL"]
    compare("env.sh DELVEWRIGHT_SKILL", got, str(SKILL_ROOT), same_path(got, str(SKILL_ROOT)), "I4")

    if not repairs:
        print("pin check: ok — the toolchain on disk is the one this page pins")
        return 0
    # A new binary may sit in another directory (I3a's table can end at the
    # floor), and I4 is the step that names that directory on `PATH`.
    if repairs & {"I3a", "I3b"}:
        repairs.add("I4")
    steps = [s for s in ORDER if s in repairs]
    print(
        f"pin check: REFUSED — run {', '.join(steps)}, then this check again. "
        f"No document is read and no `delvec` subcommand is run until it exits 0"
    )
    return EXIT_MISMATCH


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--mode", required=True, choices=("dev", "creator"), help="this run's I0 mode")
    ap.add_argument(
        "--engine", required=True, type=pathlib.Path, help="this run's I0 engine path"
    )
    args = ap.parse_args(argv)
    try:
        return run(
            args.mode,
            args.engine.expanduser(),
            pathlib.Path("~/.delvewright/env.sh").expanduser(),
            SKILL_ROOT / "versions.toml",
        )
    except FD.Refusal as refusal:
        print(f"pin check: {refusal}", file=sys.stderr)
        return refusal.code


if __name__ == "__main__":
    raise SystemExit(main())
