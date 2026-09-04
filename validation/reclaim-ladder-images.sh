#!/usr/bin/env bash
# Reclaim the images that FINISHED ladder projects left on this daemon.
#
#   validation/reclaim-ladder-images.sh                # dry run — lists, removes nothing
#   validation/reclaim-ladder-images.sh --apply        # removes
#   validation/reclaim-ladder-images.sh --project dw-m5 --project dw-m1 --apply
#
# ## Why this exists at all, given the teardown now does it
#
# `validation/fresh-volumes.sh` reclaims a project's images from the moment it
# landed, so nothing new accumulates. This is the backstop for everything the
# ladder left BEFORE that — measured on the creator's workstation the day it was
# written: 260 images, 11.99 GB, 239 of them stamped by 67 compose projects that
# had not existed for weeks. The runnable-locally guarantee (`CLAUDE.md`) decays
# with use unless what the toolchain leaves behind is bounded, and a validation
# that fails on the thirtieth run is not runnable locally.
#
# It is a SEPARATE script rather than a `--sweep` flag on the teardown, on
# purpose: `fresh-volumes.sh --project` is per-project by construction and its
# header records why the old daemon-wide `--all` was deleted — a teardown that
# can reach another project is an outage. Growing a daemon-wide mode back onto it
# would re-open exactly that. Here, breadth is the whole subject and every
# safety is stated as its own rung.
#
# ## The rungs, and what each one demands
#
# A project's images are removed only when ALL of these hold. Each demands
# evidence the defect cannot supply (`CLAUDE.md`, the sixth vacuity mode); none
# of them is "it looks idle", "it is old" or "nobody is probably using it",
# because those are beliefs about who is running, and a sibling ladder is running
# on this host right now.
#
#   1. FINISHED — the project has NO container, NO volume and NO network. Those
#      are the three classes `fresh-volumes.sh` proves, so a project holding any
#      of them is mid-run or half-torn-down, and it is skipped with the class
#      named. This is checked per project, live, at the moment of the sweep.
#   2. THIS REPOSITORY'S — the project is named the way this repository's entry
#      scripts name one (`dw…`) and the image's compose service is one
#      `validation/compose.yaml` declares. The daemon belongs to the creator, not
#      to this repository: the first sweep written here would have removed
#      `mimicat-app:latest`, from unrelated software on the same machine.
#      Compose's default project `validation` is where the OWNER's
#      `owner-play.yaml` session lands, so it is swept only when named
#      explicitly with `--project validation`.
#   3. OWNED NAMES ONLY — an image is removed only if every repository tag on it
#      is a name that ladder minted (`<project>-<service>:latest` or
#      `<repo>:<project>`). A foreign tag keeps the image, and the shared
#      `delvewright/delve:local` is exactly that case.
#   4. NOT HELD — an image any container references, of any project, is exempt by
#      id with the holder named.
#
# Rungs 3 and 4 are `validation/lib/ladder-images.sh`, shared with the teardown,
# because the rule may not exist twice.
#
# Build cache is NOT touched: it is content-addressed and global by construction
# (`docker buildx du --verbose` reports every record `Shared: true`, and
# `docker builder prune` has no project filter), so no project owns one and a
# per-project tool has no standing to prune it. Said, never silently skipped.
#
# ## The figure this prints, and what it is a figure OF
#
# Bytes are the summed `.Size` of the images removed, and that is a figure of
# IMAGE SIZE, not of disk. Every delve image carries the whole 863 MB itzg base
# and every bot image the whole node base; the daemon stores each of those once
# and `.Size` reports it for each image, so the sum runs an order of magnitude
# above the disk that comes back (measured here: 165 GB of image size against a
# daemon holding 11.99 GB in total). It is reported because it is the only
# per-project figure there is. The DISK figure is `docker system df`, printed
# before and after under `--apply` — a second measurement whose failure mode is
# unrelated to the sum, and the two are expected to disagree by that much.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=validation/lib/ladder-images.sh
. "$here/lib/ladder-images.sh"

usage() {
  cat >&2 <<'USAGE'
usage: validation/reclaim-ladder-images.sh [--apply] [--project <name>]...

  (no flags)   DRY RUN: list every finished project's images and the bytes, and
               remove nothing. This is the default because the safe direction has
               to be the one that needs no argument.
  --apply      actually remove them.
  --project    restrict the sweep to this project (repeatable). Without it the
               sweep covers every project that has an image and no container,
               volume or network — EXCEPT compose's default project
               (`validation`), which is where the owner's play session lands and
               which is therefore swept only when named here.

A project holding any container, volume or network is skipped with the class
named: it is mid-run. Images a container holds, and images carrying a tag this
project did not mint (`delvewright/delve:local`), are kept and named.
USAGE
  exit 2
}

apply=""
restrict=""
while [ $# -gt 0 ]; do
  case "$1" in
    --apply) apply=1; shift ;;
    --project|-p)
      [ $# -ge 2 ] || usage
      case "$2" in
        *[!A-Za-z0-9_.-]*|[!A-Za-z0-9]*)
          echo "reclaim-ladder-images: '$2' is not a compose project name ([A-Za-z0-9][A-Za-z0-9_.-]*)." >&2
          exit 2
          ;;
      esac
      restrict="$restrict$2"$'\n'
      shift 2
      ;;
    -h|--help) usage ;;
    *) echo "reclaim-ladder-images: unknown argument '$1'" >&2; usage ;;
  esac
done

docker version >/dev/null 2>&1 || {
  echo "reclaim-ladder-images: the Docker daemon is not reachable." >&2
  exit 1
}

# The population, derived from the objects. Never a `dw-*` name glob: a glob
# cannot see the 164 untagged images a rebuild leaves behind, and it cannot see a
# project whose name is not `dw-` — which `validation`, the biggest single holder
# on the machine this was written on, is not.
if [ -n "$restrict" ]; then
  projects="$restrict"
else
  projects=""
  while IFS= read -r candidate; do
    [ -n "$candidate" ] || continue
    [ "$candidate" != "$DW_IMG_DEFAULT_PROJECT" ] || continue
    dw_project_is_ladder "$candidate" || continue
    projects="$projects$candidate"$'\n'
  done <<EOF
$(dw_image_projects)
EOF
fi
# The service half of the same question is asked per IMAGE, inside the shared
# rule — a project can only be judged by what it built.
dw_ladder_services >/dev/null

# One place decides the mode, and the SAFE value is the one a missing flag gives.
dry="dry"
mode="DRY RUN — nothing will be removed; add --apply to remove"
if [ -n "$apply" ]; then
  dry=""
  mode="APPLY — removing"
fi
echo "==> reclaim-ladder-images: $mode"
df_before=""
if [ -n "$apply" ]; then
  df_before="$(docker system df --format '{{.Type}} {{.TotalCount}} {{.Size}} {{.Reclaimable}}' 2>/dev/null || true)"
fi

examined_projects=0
swept_projects=0
skipped_projects=0
total_removed=0
total_kept=0
total_bytes=0

while IFS= read -r project; do
  [ -n "$project" ] || continue
  examined_projects=$((examined_projects + 1))
  label="com.docker.compose.project=$project"

  # Rung 1, live at the moment of the sweep. Not quiet, not mtime, not a belief
  # about who is running: the three classes a live run necessarily holds.
  live=""
  c="$(docker ps -aq --filter "label=$label" 2>/dev/null || true)"
  [ -z "$c" ] || live="$live container"
  v="$( { docker volume ls -q --filter "label=$label" 2>/dev/null || true
          docker volume ls -q 2>/dev/null | grep -E "^${project}_" || true; } | sort -u)"
  [ -z "$v" ] || live="$live volume"
  n="$( { docker network ls -q --filter "label=$label" 2>/dev/null || true
          docker network ls --format '{{.Name}}' 2>/dev/null | grep -E "^${project}_" || true; } | sort -u)"
  [ -z "$n" ] || live="$live network"
  if [ -n "$live" ]; then
    skipped_projects=$((skipped_projects + 1))
    echo "  SKIP $project — still holds:${live}; it is mid-run, not finished"
    continue
  fi

  dw_reclaim_project_images "$project" "$dry"
  [ "$DW_IMG_EXAMINED" -gt 0 ] || continue
  swept_projects=$((swept_projects + 1))
  total_removed=$((total_removed + DW_IMG_REMOVED))
  total_kept=$((total_kept + DW_IMG_KEPT))
  total_bytes=$((total_bytes + DW_IMG_BYTES))
  echo "  $project — $DW_IMG_EXAMINED image(s), $DW_IMG_REMOVED to go, $DW_IMG_KEPT kept, $(dw_img_human_bytes "$DW_IMG_BYTES")"
  [ -z "$DW_IMG_REMOVED_LINES" ] || printf '%s' "$DW_IMG_REMOVED_LINES"
  [ -z "$DW_IMG_KEPT_LINES" ] || printf '%s' "$DW_IMG_KEPT_LINES"
done <<EOF
$projects
EOF

echo
# BINDING (CLAUDE.md: a green gate that binds to nothing is VACUOUS). Every
# number here is computed from the objects; a zero examined-projects count is a
# finding — either the daemon holds nothing this tool can see, or the label it
# selects on has moved.
verb="would remove"
[ -z "$apply" ] || verb="removed"
echo "reclaim-ladder-images: $examined_projects project(s) examined," \
  "$skipped_projects skipped as mid-run, $swept_projects with images;" \
  "$verb $total_removed image(s), $(dw_img_human_bytes "$total_bytes") of IMAGE SIZE" \
  "(not disk — each image's shared base layers are counted in it; see docker system df)," \
  "kept $total_kept."
if [ "$examined_projects" -eq 0 ]; then
  echo "reclaim-ladder-images: examined ZERO projects — nothing on this daemon carries" >&2
  echo "  the label '$DW_IMG_LABEL_KEY'. That is a finding if a ladder has ever run here." >&2
fi
if [ -n "$apply" ]; then
  echo
  echo "  docker system df, before:"
  printf '%s\n' "$df_before" | sed 's/^/    /'
  echo "  and after — the second measurement, whose failure mode is unrelated to the sum above:"
  docker system df --format '{{.Type}} {{.TotalCount}} {{.Size}} {{.Reclaimable}}' | sed 's/^/    /'
  echo
  echo "  Build cache untouched: content-addressed and global, no project owns one."
else
  echo "  Re-run with --apply to remove them."
fi
