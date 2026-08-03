#!/usr/bin/env bash
# Fresh-volume guard (task #45). Tear a validation stack down and PROVE its world
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
#
# ## Two modes, and why the scoped one is the default posture
#
#   validation/fresh-volumes.sh --project dw-worker-<x>   # ONLY that compose project
#   validation/fresh-volumes.sh --all                     # daemon-wide sweep
#
# The daemon-wide sweep matches `server-data$` across EVERY compose project and
# force-removes the pinned `delvewright-*` container names — i.e. it reaches into
# the owner's session and into any other worker's stack. That is correct only for
# whoever owns the whole host. A worker (CLAUDE.md / mutex.sh "Worker isolation")
# runs in its own `-p dw-worker-<unique>` project and must tear down *only* its own
# project, so `--project` is what it runs — and running with neither flag is an
# error rather than the destructive default, because the cost of guessing wrong is
# somebody else's live world.
#
# `--project` also fixes the defect that motivated the split (island round 13): a
# `docker compose -p <proj> … down -v` leaves `<proj>_server-data` behind whenever an
# EXITED container of that project still holds it, and the stale volume carries the
# scoreboard — so the re-run starts with objectives already complete and the bot
# reports a false CONTENT failure. Three red runs were misattributed to the campaign
# before the volume was found. Here the removal is forced and then PROVEN, scoped to
# the one project.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

usage() {
  cat >&2 <<'USAGE'
usage: fresh-volumes.sh --project <compose-project>   # scoped: one project only
       fresh-volumes.sh --all                         # daemon-wide (owns the host)

A worker passes --project with the SAME name it gave `docker compose -p`
(COMPOSE_PROJECT_NAME is honoured when no flag is given). With neither, this
script does nothing: the daemon-wide sweep destroys other projects' worlds and
must be asked for by name.
USAGE
  exit 2
}

mode=""
project="${COMPOSE_PROJECT_NAME:-}"
if [ -n "$project" ]; then mode="project"; fi
while [ $# -gt 0 ]; do
  case "$1" in
    --project|-p)
      [ $# -ge 2 ] || usage
      mode="project"
      project="$2"
      shift 2
      ;;
    --all)
      mode="all"
      shift
      ;;
    -h|--help) usage ;;
    *)
      echo "fresh-volumes: unknown argument '$1'" >&2
      usage
      ;;
  esac
done
[ -n "$mode" ] || usage

# The mutex is not this script's to take, but `owner-play-session` is sacred: a
# human is playing on `delvewright-server`, and the daemon-wide sweep deletes
# exactly that container and its world volume. Refuse. (`--project` cannot reach the
# owner's project by construction, and a worker must always be able to clean up
# after itself, so the scoped mode only says so.)
# shellcheck source=validation/mutex.sh
. "$here/mutex.sh"
set -euo pipefail  # mutex.sh sets its own options when sourced; take ours back

if [ "$mode" = "all" ]; then
  dw_mutex_assert_not_owner_session || {
    echo "fresh-volumes: --all is a daemon-wide teardown; a worker wants" >&2
    echo "  --project <its own compose project> instead." >&2
    exit 1
  }

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
  echo "fresh-volumes: world volumes verified clean (daemon-wide)"
  exit 0
fi

# ---- scoped mode: this compose project and nothing else ----------------------

case "$project" in
  ""|*[!A-Za-z0-9_.-]*|[!A-Za-z0-9]*)
    echo "fresh-volumes: '$project' is not a compose project name ([A-Za-z0-9][A-Za-z0-9_.-]*)." >&2
    exit 2
    ;;
esac
if [ "$(dw_mutex_holder)" = "owner-play-session" ]; then
  echo "fresh-volumes: NOTE — the mutex reads owner-play-session. Tearing down only" >&2
  echo "  '$project'; nothing else here may be touched while a human is playing." >&2
fi

# Everything below selects by the compose project LABEL (plus, for a volume whose
# label did not survive, the `<project>_` name prefix compose stamps). No pinned
# `delvewright-*` container name and no bare `server-data$` match appears in this
# path: a scoped teardown that can match another project is not scoped.
label="com.docker.compose.project=$project"

project_containers() {
  docker ps -aq --filter "label=$label" 2>/dev/null || true
}
project_volumes() {
  {
    docker volume ls -q --filter "label=$label" 2>/dev/null || true
    docker volume ls -q 2>/dev/null | grep -E "^${project}_" || true
  } | sort -u
}

# Ordinary teardown first, so a running stack stops the way compose expects.
docker compose -p "$project" -f "$here/compose.yaml" down -v --remove-orphans >/dev/null 2>&1 || true
# Then force-remove what compose left behind: an EXITED container of this project
# still holds its volume open, which is exactly how `down -v` leaves
# `<project>_server-data` alive and silently poisons the next run.
containers="$(project_containers)"
if [ -n "$containers" ]; then
  # shellcheck disable=SC2086  # deliberate word splitting: one id per argument
  docker rm -f $containers >/dev/null 2>&1 || true
fi
volumes="$(project_volumes)"
if [ -n "$volumes" ]; then
  # shellcheck disable=SC2086
  docker volume rm -f $volumes >/dev/null 2>&1 || true
fi

# GUARD: same contract as the daemon-wide mode — prove it, never assume it. A
# surviving container is reported too: it is the reason a volume survives.
remaining_c="$(project_containers)"
remaining_v="$(project_volumes)"
if [ -n "$remaining_c" ] || [ -n "$remaining_v" ]; then
  echo "fresh-volumes: FAILED — project '$project' is not clean, next run is NOT fresh:" >&2
  # shellcheck disable=SC2086  # deliberate word splitting: one line per id
  if [ -n "$remaining_c" ]; then printf '  container %s\n' $remaining_c >&2; fi
  # shellcheck disable=SC2086
  if [ -n "$remaining_v" ]; then printf '  volume    %s\n' $remaining_v >&2; fi
  echo "  a surviving container holds its world volume open; stop it and retry" >&2
  echo "  (a stale world persists scoreboard state — completed objectives stay completed)." >&2
  exit 1
fi
echo "fresh-volumes: project '$project' verified clean (containers + volumes)"
