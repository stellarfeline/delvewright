#!/usr/bin/env bash
# Fresh-volume guard. Tear ONE compose project down and PROVE its world
# volumes are actually gone before a clean run.
#
#   validation/fresh-volumes.sh --project dw-worker-<id>
#
# Why this exists: the itzg `/data` world volume persists player scoreboard state —
# a completed objective stays completed — across runs. A stale `*server-data`
# volume therefore makes a "fresh" playthrough fail for reasons unrelated to the
# delve (e.g. a talk-to whose objective is already set never re-fires its transport).
# `docker volume rm` SILENTLY no-ops when the volume is still held by a
# not-fully-stopped container — so a teardown can look successful yet leave a stale
# world behind. Automate the pitfall out of existence: force the teardown, then
# assert the volume list is clean and fail loudly if it is not.
#
# ## Networks are the third class, and they only bite the creator
#
# The teardown used to prove containers and volumes and say so — "verified clean
# (containers + volumes)" — which is `CLAUDE.md`'s own tell for a rule written
# against the cases its author had met: it carefully qualified two of the three
# things a compose project holds. `docker compose down` normally takes the
# network with it, but this script force-removes containers AFTER that `down`
# (which is the whole point of the previous paragraph), and a project whose
# `down` did not fully succeed leaves `<project>_default` behind with nothing
# reporting it.
#
# They accumulate silently, and CI never sees it: a runner is fresh every job.
# The creator's machine is not, and Docker's default address pool is finite —
# 29 leaked networks from four different rounds exhausted it, and the failure
# lands nowhere near the cause: `all predefined address pools have been fully
# subnetted`, on the tenth project of a sweep, reported as a PackTest failure.
# That is the unbounded-set-nobody-owns shape this project has already paid for
# once on disk, and it makes `CLAUDE.md`'s "every validation must be runnable on
# the creator's own machine" quietly false after enough runs.
#
# So the network is removed and PROVEN gone with the rest, by the same scoping
# rule: the compose project label, plus the `<project>_` name prefix compose
# stamps. Never a bare match, and the built-in `bridge`/`host`/`none` carry
# neither and cannot be selected.
#
# ## Images are the FOURTH class, and they were the whole leak
#
# The paragraph above is the shape repeating itself one class further out: the
# teardown proved containers, volumes and networks and said so, and a ladder
# creates a fourth thing nothing ever removed. Both image tags a run mints are
# project-scoped BY DESIGN — `delvewright/delve:<project>` (an image tag is
# global to the daemon exactly as a container name is, so the entry scripts give
# each ladder its own) and compose's own `<project>-bot:latest` — which is
# precisely what makes them a per-project LEAK rather than one shared tag being
# rebuilt. Nothing reuses them and nothing removed them.
#
# Measured on the creator's workstation before this: 260 images, 11.99 GB, 239 of
# them carrying a compose project label from 67 finished projects. CI never sees
# it, for the same reason it never saw the networks — a runner is fresh every job.
#
# The rule for what a project OWNS, and the safety that keeps a shared tag out of
# it, is `validation/lib/ladder-images.sh`; it is a library because the sweep over
# what earlier runs already left (`validation/reclaim-ladder-images.sh`) is the
# second caller, and the rule may not exist twice.
#
# One thing about it belongs here rather than there: an image that survives is
# NAMED, with the daemon's own reason, and does NOT fail the teardown. A stale
# image cannot poison the next run the way a stale volume does — `up --build`
# rebuilds the tag — so reddening a ladder over disk hygiene would manufacture
# exactly the false CONTENT failure the paragraph below is about. Containers,
# volumes and networks still fail it.
#
# It also fixes the defect that motivated the project scoping (island round 13): a
# `docker compose -p <proj> … down -v` leaves `<proj>_server-data` behind whenever an
# EXITED container of that project still holds it, and the stale volume carries the
# scoreboard — so the re-run starts with objectives already complete and the bot
# reports a false CONTENT failure. Three red runs were misattributed to the campaign
# before the volume was found. Here the removal is forced and then PROVEN.
#
# ## --project is REQUIRED
#
# There is no default and no daemon-wide mode. The old `--all` swept `server-data$`
# across EVERY compose project and `docker rm -f`'d the pinned `delvewright-*`
# names, i.e. it reached into the owner's session and into other workers' stacks —
# an operation no caller on a shared host is ever entitled to. It is gone. A
# teardown targets the project the caller launched, by name, or it does not run:
# every entry script (`bot-run.sh`, `packtest-run.sh`, `branch-runs.sh`) passes the
# same `--project` it gave `docker compose -p`.
#
# `COMPOSE_PROJECT_NAME` is deliberately NOT honoured: an inherited environment
# variable is exactly the kind of invisible default whose cost is somebody else's
# live world.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
# The ONE rule for which images a compose project owns, shared with the sweep.
# shellcheck source=validation/lib/ladder-images.sh
. "$here/lib/ladder-images.sh"

usage() {
  cat >&2 <<'USAGE'
usage: fresh-volumes.sh --project <compose-project>

Tears down exactly that compose project — containers, volumes, networks AND the
images it built — and proves each class gone. The project name is REQUIRED — pass
the same one you gave `docker compose -p`. There is no daemon-wide mode: a
teardown that can reach another project (or the owner's play session) is not a
teardown, it is an outage. For what EARLIER runs left behind, the sweep is
`validation/reclaim-ladder-images.sh` (dry-run by default).
USAGE
  exit 2
}

project=""
while [ $# -gt 0 ]; do
  case "$1" in
    --project|-p)
      [ $# -ge 2 ] || usage
      project="$2"
      shift 2
      ;;
    -h|--help) usage ;;
    *)
      echo "fresh-volumes: unknown argument '$1'" >&2
      usage
      ;;
  esac
done

if [ -z "$project" ]; then
  echo "fresh-volumes: --project <compose-project> is REQUIRED." >&2
  usage
fi
case "$project" in
  *[!A-Za-z0-9_.-]*|[!A-Za-z0-9]*)
    echo "fresh-volumes: '$project' is not a compose project name ([A-Za-z0-9][A-Za-z0-9_.-]*)." >&2
    exit 2
    ;;
esac

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

project_networks() {
  {
    docker network ls -q --filter "label=$label" 2>/dev/null || true
    docker network ls --format '{{.Name}}' 2>/dev/null | grep -E "^${project}_" || true
  } | sort -u
}

# The one hard seal left on a shared host: whatever the project label says, a
# container publishing host 25565 is an OWNER-FACING session (owner-play.yaml or
# tools/playtest-server.sh) and a human may be inside it. Refuse rather than
# reason about how it got that label. This can only ever fire on a mis-typed
# project name — a worker stack publishes no port at all (validation/compose.yaml).
owner_port_containers() {
  local ids id ports
  ids="$(project_containers)"
  [ -n "$ids" ] || return 0
  for id in $ids; do
    # Capture, then test. `| grep -qx` would exit at the first match and SIGPIPE
    # its producer; under pipefail that reads as NO MATCH, and this guard's whole
    # job is to refuse a teardown when a human may be inside the container.
    ports="$(docker inspect -f '{{range $p, $c := .NetworkSettings.Ports}}{{$p}}{{range $c}} {{.HostPort}}{{end}} {{end}}' \
      "$id" 2>/dev/null || true)"
    if [[ " $ports " == *" 25565 "* ]]; then
      echo "$id"
    fi
  done
}
guard="$(owner_port_containers)"
if [ -n "$guard" ]; then
  echo "fresh-volumes: REFUSING to tear down project '$project' — a container in it" >&2
  echo "  publishes host port 25565, so this is an owner-facing session and a human" >&2
  echo "  may be playing in it:" >&2
  # shellcheck disable=SC2086  # deliberate word splitting: one id per line
  docker ps -a --format '  {{.ID}}  {{.Names}}  {{.Ports}}' --filter "label=$label" >&2
  echo "  A worker stack publishes NO host port; check the --project you passed." >&2
  exit 1
fi

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
# Networks last: a network cannot be removed while a container is still attached,
# so this has to follow the container removal above rather than ride on `down`.
networks="$(project_networks)"
if [ -n "$networks" ]; then
  # shellcheck disable=SC2086  # deliberate word splitting: one id/name per argument
  docker network rm $networks >/dev/null 2>&1 || true
fi
# Images last: the container removal above is what releases them, and an image a
# container of ANOTHER project still holds is exempted by id rather than forced.
dw_reclaim_project_images "$project"

# GUARD: prove it, never assume it. A surviving container is reported too: it is
# the reason a volume survives.
remaining_c="$(project_containers)"
remaining_v="$(project_volumes)"
remaining_n="$(project_networks)"
if [ -n "$remaining_c" ] || [ -n "$remaining_v" ] || [ -n "$remaining_n" ]; then
  echo "fresh-volumes: FAILED — project '$project' is not clean, next run is NOT fresh:" >&2
  # shellcheck disable=SC2086  # deliberate word splitting: one line per id
  if [ -n "$remaining_c" ]; then printf '  container %s\n' $remaining_c >&2; fi
  # shellcheck disable=SC2086
  if [ -n "$remaining_v" ]; then printf '  volume    %s\n' $remaining_v >&2; fi
  # shellcheck disable=SC2086
  if [ -n "$remaining_n" ]; then printf '  network   %s\n' $remaining_n >&2; fi
  echo "  a surviving container holds its world volume open; stop it and retry" >&2
  echo "  (a stale world persists scoreboard state — completed objectives stay completed)." >&2
  exit 1
fi

# The image proof. A survivor is NAMED with its reason and does not red the
# ladder (see the header): a stale image is disk, not a poisoned world.
remaining_i="$(dw_project_images_remaining "$project")"
if [ -n "$DW_IMG_KEPT_LINES" ] || [ -n "$remaining_i" ]; then
  echo "fresh-volumes: project '$project' KEPT images (named, not silently skipped):" >&2
  [ -z "$DW_IMG_KEPT_LINES" ] || printf '%s' "$DW_IMG_KEPT_LINES" >&2
  if [ -n "$remaining_i" ]; then
    echo "  still carrying a name this project owns — the sweep will report them again:" >&2
    printf '%s\n' "$remaining_i" | sed 's/^/    /' >&2
  fi
fi
# BINDING (CLAUDE.md: a green gate that binds to nothing is VACUOUS). `examined`
# is how many images carried this project's label at all; zero is the honest
# answer for a project that never built one (the PackTest profile pulls a pinned
# digest and builds nothing), never a pass smuggled in as silence. The byte
# figure is summed IMAGE SIZE, not disk: every delve image carries the whole itzg
# base and the daemon stores that once, so the sum counts it per image.
echo "fresh-volumes: project '$project' verified clean (containers + volumes + networks + images:" \
  "$DW_IMG_EXAMINED examined, $DW_IMG_REMOVED removed, $(dw_img_human_bytes "$DW_IMG_BYTES") of image size," \
  "$DW_IMG_KEPT kept). Build cache is content-addressed and global — no project owns one, so none is pruned."
