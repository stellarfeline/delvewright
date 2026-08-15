#!/usr/bin/env python3
"""Compose isolation gate: the validation stack carries no Docker-GLOBAL name.

Container names and published host ports are global to the daemon: `docker
compose -p dw-worker-<x>` isolates containers, volumes and networks but NOT
those two. `validation/compose.yaml` carries neither, so a ladder is fully
described by its compose project and any number of them run side by side — no
mutex, no queueing. This gate is what keeps them out.

The rules, over `validation/*.yaml`:

  1. `compose.yaml` may not set `container_name:` and may not declare `ports:`
     on any service. (The predecessor of this gate merely required a matching
     `!reset` in a worker override — which meant the pin still existed and every
     caller had to remember to pass the override. It cost two runs: `server`
     and `bot` (the-drowned-bell round 2).)
  2. `owner-play.yaml` is the ONLY file that may publish a fixed host port, and
     only `127.0.0.1:25565:25565` — the owner's client address, the one genuinely
     shared resource, guarded by `validation/mutex.sh`.
  3. Every other file may publish only EPHEMERAL host ports (`127.0.0.1::<port>`
     or `::<port>`), where Docker picks a free number and two runs can never
     collide.
  4. Only `owner-play.yaml` may pin a `container_name:` — a human needs to find
     their own session by name; a ladder never does.

Deterministic, offline, no dependencies (Python 3 stdlib) — the compose files are
a fixed, flat two-level shape, so this reads them with an indentation scan rather
than pulling in PyYAML. Run from the repo root:
    python3 tools/check-compose-isolation.py
Exit 0 = isolated, 1 = a violation (see stderr), 2 = usage/IO error.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path("validation")
COMPOSE_NAME = "compose.yaml"
OWNER_NAME = "owner-play.yaml"

# A published port entry, in either compose short-syntax spelling:
#   "127.0.0.1:25565:25565" / "25565:25565"   -> FIXED host port
#   "127.0.0.1::25565" / "::25565" / "25565"  -> ephemeral (Docker chooses)
FIXED_PORT = re.compile(r"^(?P<ip>(?:\d{1,3}\.){3}\d{1,3}:)?(?P<host>\d+):(?P<container>\d+)(?:/\w+)?$")


def services(path: pathlib.Path) -> dict[str, dict[str, list[str]]]:
    """`{service: {key: [values]}}` for the `services:` block's 4-space keys.

    A key's values are its inline value plus any 6-space `- item` list entries,
    so `ports:` is seen whether it is written as a block list or inline.
    """
    out: dict[str, dict[str, list[str]]] = {}
    in_services = False
    current: str | None = None
    key: str | None = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        if raw.strip().startswith("#") or not raw.strip():
            continue
        indent = len(raw) - len(raw.lstrip(" "))
        line = raw.strip()
        if indent == 0:
            in_services = line == "services:"
            current = key = None
            continue
        if not in_services:
            continue
        if indent == 2 and line.endswith(":"):
            current = line[:-1]
            key = None
            out[current] = {}
        elif indent == 4 and current is not None and ":" in line:
            key, _, value = line.partition(":")
            key = key.strip()
            out[current].setdefault(key, [])
            value = value.strip()
            if value:
                out[current][key].append(value)
        elif indent >= 6 and current is not None and key is not None and line.startswith("- "):
            out[current][key].append(line[2:].strip())
    return out


def port_entries(values: list[str]) -> list[str]:
    """The individual port strings, from a block list or an inline `["a","b"]`."""
    entries: list[str] = []
    for value in values:
        for item in re.split(r"[,\[\]]", value):
            item = item.strip().strip("'\"")
            if item:
                entries.append(item)
    return entries


def main() -> int:
    if not ROOT.is_dir():
        sys.stderr.write(f"error: {ROOT} not found (run from the repo root)\n")
        return 2
    compose = ROOT / COMPOSE_NAME
    owner = ROOT / OWNER_NAME
    for p in (compose, owner):
        if not p.is_file():
            sys.stderr.write(f"error: {p} not found (run from the repo root)\n")
            return 2

    problems: list[str] = []

    # Rule 1: the base file carries nothing global.
    for name, keys in sorted(services(compose).items()):
        if "container_name" in keys:
            problems.append(
                f"  {compose}: service `{name}` pins `container_name` — container names are\n"
                f"    Docker-GLOBAL, so two ladders collide on it and one `down` reaches into\n"
                f"    the other. Drop it; the compose project (`-p <id>`) is the name.\n"
                f"    A name the OWNER must be able to find belongs in {owner}."
            )
        if "ports" in keys:
            problems.append(
                f"  {compose}: service `{name}` publishes `ports` — a host port is\n"
                f"    Docker-GLOBAL and 25565 is the owner's client address. Drop it; reach\n"
                f"    the server over the compose network or `docker exec … rcon-cli`.\n"
                f"    Owner binding: {owner}. Host-side bot: {ROOT / 'ephemeral-port.yaml'}."
            )

    # Rules 2-4: every other file in validation/.
    for path in sorted(ROOT.glob("*.yaml")):
        if path.name == COMPOSE_NAME:
            continue
        is_owner = path.name == OWNER_NAME
        for name, keys in sorted(services(path).items()):
            if "container_name" in keys and not is_owner:
                problems.append(
                    f"  {path}: service `{name}` pins `container_name` — only {owner} may,\n"
                    f"    because only a human needs to find their own session by name."
                )
            for entry in port_entries(keys.get("ports", [])):
                match = FIXED_PORT.match(entry)
                if not match:
                    continue  # ephemeral (`ip::container` / bare container port)
                if not is_owner:
                    problems.append(
                        f"  {path}: service `{name}` publishes the FIXED host port "
                        f"`{entry}`.\n"
                        f"    A fixed port is a shared resource under another name. Use the\n"
                        f"    ephemeral form (`127.0.0.1::<container-port>`) and read the number\n"
                        f"    back with `docker compose -p <id> port <service> <port>`."
                    )
                elif entry != "127.0.0.1:25565:25565":
                    problems.append(
                        f"  {owner}: service `{name}` publishes `{entry}`. This file exists for\n"
                        f"    exactly one binding — `127.0.0.1:25565:25565`, the owner's client\n"
                        f"    address. Anything else belongs in an ephemeral override."
                    )

    if problems:
        sys.stderr.write(
            "validation stack is not isolated by construction:\n"
            + "\n".join(problems)
            + "\n\nTwo ladders (or a ladder and the owner) would collide on these.\n"
            + "See validation/README.md 'Sharing the Docker host'.\n"
        )
        return 1
    print(
        f"validation stack is isolated by construction "
        f"({len(services(compose))} services in {compose} carry no global name)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
