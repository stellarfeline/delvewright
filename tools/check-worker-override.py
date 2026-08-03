#!/usr/bin/env python3
"""Worker-isolation coverage gate: `validation/worker-override.yaml` must reset
every Docker-GLOBAL name `validation/compose.yaml` pins.

Container names and published host ports are global to the daemon: `docker
compose -p dw-worker-<x>` isolates volumes and networks but NOT those. So a
service that pins `container_name:` or publishes `ports:` and is missing from the
worker override is a collision waiting for the second concurrent worker — and, on
teardown, a `docker compose down` that reaches into somebody else's stack.

This class has now cost two runs: the `server` container name (#190) and then the
`bot` container name, which killed a the-drowned-bell round-2 playthrough. Every
other gate is green while it happens, because nothing in the repo relates the two
files. This one does.

The rule, per service in `compose.yaml`:

  * pins `container_name:`  -> the override must carry `container_name: !reset null`
  * declares `ports:`       -> the override must carry `ports: !reset []`

`!reset` (not `null`) is required by name: plain `container_name: null` is
silently ignored by compose and the container comes up with its pinned name
anyway (validation/README.md).

Deterministic, offline, no dependencies (Python 3 stdlib) — the compose files are
a fixed, flat two-level shape, so this reads them with an indentation scan rather
than pulling in PyYAML. Run from the repo root:
    python3 tools/check-worker-override.py
Exit 0 = covered, 1 = a gap (see stderr), 2 = usage/IO error.
"""

import pathlib
import sys

COMPOSE = pathlib.Path("validation/compose.yaml")
OVERRIDE = pathlib.Path("validation/worker-override.yaml")

# key -> the exact override value that neutralizes it.
GLOBAL_KEYS = {"container_name": "!reset null", "ports": "!reset []"}


def services(path: pathlib.Path) -> dict[str, dict[str, str]]:
    """`{service: {key: value}}` for the `services:` block's 4-space keys."""
    out: dict[str, dict[str, str]] = {}
    in_services = False
    current: str | None = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        if raw.strip().startswith("#") or not raw.strip():
            continue
        indent = len(raw) - len(raw.lstrip(" "))
        line = raw.strip()
        if indent == 0:
            in_services = line == "services:"
            current = None
            continue
        if not in_services:
            continue
        if indent == 2 and line.endswith(":"):
            current = line[:-1]
            out[current] = {}
        elif indent == 4 and current is not None and ":" in line:
            key, _, value = line.partition(":")
            out[current][key.strip()] = value.strip()
    return out


def main() -> int:
    for p in (COMPOSE, OVERRIDE):
        if not p.is_file():
            sys.stderr.write(f"error: {p} not found (run from the repo root)\n")
            return 2
    compose = services(COMPOSE)
    override = services(OVERRIDE)
    if not compose:
        sys.stderr.write(f"error: no services parsed from {COMPOSE}\n")
        return 2

    problems: list[str] = []
    for name, keys in sorted(compose.items()):
        for key, reset in GLOBAL_KEYS.items():
            if key not in keys:
                continue
            got = override.get(name, {}).get(key)
            if got is None:
                problems.append(
                    f"  {COMPOSE}: service `{name}` sets `{key}` (Docker-global), but "
                    f"{OVERRIDE} does not reset it.\n"
                    f"    Add under `{name}:`   {key}: {reset}"
                )
            elif got != reset:
                problems.append(
                    f"  {OVERRIDE}: service `{name}` has `{key}: {got}`, which does not "
                    f"neutralize the pin.\n"
                    f"    compose merges a plain value; use   {key}: {reset}"
                )
    # A stale override entry for a service compose no longer has is dead weight
    # that reads as coverage.
    for name in sorted(set(override) - set(compose)):
        problems.append(
            f"  {OVERRIDE}: service `{name}` does not exist in {COMPOSE} — drop the "
            "stale override block."
        )

    if problems:
        sys.stderr.write(
            "worker-isolation override does not cover every globally-named service:\n"
            + "\n".join(problems)
            + "\n\nTwo workers (or a worker and the owner) would collide on these names.\n"
        )
        return 1
    print(f"worker override covers every globally-named service ({len(compose)} checked)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
