#!/usr/bin/env bash
# Fresh-volume guard (task #45). Tear the validation stack down and PROVE the world
# volumes are actually gone before a clean bot playthrough.
#
# Why this exists: the itzg `/data` world volume persists player scoreboard state —
# a completed objective stays completed — across runs. A stale `*server-data`
# volume therefore makes a "fresh" playthrough fail for reasons unrelated to the
# delve (e.g. a talk-to whose objective is already set never re-fires its transport).
# `docker volume rm` SILENTLY no-ops when the name's project prefix is wrong (it
# varies with the compose working directory) or the volume is still held by a
# not-fully-stopped container — so a teardown can look successful yet leave a stale
# world behind. Automate the pitfall out of existence: force the teardown, then
# assert the volume list is clean and fail loudly if it is not.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

# Best-effort compose teardown (inherits EULA from the caller's env like the run
# commands; failure here is non-fatal — the explicit cleanup + guard below is
# authoritative).
docker compose -f "$here/compose.yaml" down -v --remove-orphans >/dev/null 2>&1 || true
# Force-stop any lingering containers that would hold the world volume open (the
# usual cause of a silent `volume rm` no-op).
docker rm -f delvewright-server delvewright-bot >/dev/null 2>&1 || true
# Remove every world volume by suffix — the compose project prefix depends on the
# directory the stack was launched from, so match on the stable `server-data` tail.
docker volume ls -q | grep -E 'server-data$' | xargs -r docker volume rm >/dev/null 2>&1 || true

# GUARD: a remaining world volume means a remove silently no-op'd — the next run
# would NOT be fresh. Fail loudly rather than shipping a contaminated result.
remaining="$(docker volume ls -q | grep -E 'server-data$' || true)"
if [ -n "$remaining" ]; then
  echo "fresh-volumes: FAILED — world volume(s) still present, next run is NOT fresh:" >&2
  printf '  %s\n' $remaining >&2
  echo "  stop all delvewright containers and retry (a stale world persists scoreboard state)." >&2
  exit 1
fi
echo "fresh-volumes: world volumes verified clean"
