#!/usr/bin/env bash
# The ONE rule for "which images does this compose project own", shared by the
# per-project teardown (`validation/fresh-volumes.sh`) and the sweep over what
# earlier runs left (`validation/reclaim-ladder-images.sh`).
#
# It lives here rather than inside the teardown because a correct rule living in
# one call site's teardown gives the next caller nothing to reuse, and the second
# caller — the sweep — is the one that has to get the SAFETY half exactly right.
#
# ## The class this exists for
#
# A ladder run creates FOUR classes of Docker object, not three. The teardown
# proved containers, volumes and networks and stopped there, so every ladder
# project leaked its images permanently:
#
#   * `delvewright/delve:<project>` — the shipped delve, tagged per project by
#     `bot-run.sh` / `world-save.sh` because an image TAG is global to the daemon
#     in exactly the way a container name is.
#   * `<project>-<service>:latest`  — compose's own generated name for a service
#     that has a `build:` and no `image:` (the harness `bot`).
#
# Both are project-scoped BY DESIGN, which is what makes them a per-project leak
# rather than one shared tag being rebuilt: nothing ever reuses them, and nothing
# ever removed them. Measured on the creator's workstation before this existed:
# 260 images, 11.99 GB, of which 239 carried a compose project label from 67
# finished projects.
#
# ## Selection is by LABEL, and that reaches the untagged half
#
# `docker compose build` stamps `com.docker.compose.project` and
# `com.docker.compose.service` into the config of every image it BUILDS — never
# onto an image it merely pulled, which is why the pinned toolserver digest and
# the itzg base can never be selected here. So images scope exactly like the
# other three classes, by the same project label, and `docker images -a` reaches
# the UNTAGGED ones too. That matters: every rebuild of `<project>-bot` leaves the
# previous build untagged and still labelled, and untagged images were 164 of the
# 260 above. A `dw-*` name glob cannot see any of them.
#
# ## …but REMOVAL is by tag, and the tag rule is the safety
#
# A label-only rule would be a disaster, and the proof is on the creator's
# machine: `delvewright/delve:local` — the default `DELVE_IMAGE`, the image
# `owner-play.yaml` publishes on 25565 and the one the `playtest` profile builds —
# carries the project label of whichever project built it last (measured:
# `dw-round-n`). Reclaiming that finished project by label alone would delete the
# owner's play image.
#
# So an image is this project's to remove only when every REPOSITORY TAG on it is
# a name this ladder itself minted:
#
#   * `<project>-<service>:latest`, with the service read off the image's OWN
#     service label rather than pattern-matched — `dw-m5-final-bot:latest` starts
#     with `dw-m5-`, and only the exact name can tell the two projects apart.
#   * any `<repo>:<project>` — the `DELVE_IMAGE` convention.
#
# Any other tag is somebody else's NAME for that image and is never touched. The
# judgement is per TAG, not per image: this project's own names go, the foreign
# ones stay, and the image itself survives exactly as long as one of those stays
# — which is what keeps the shared tag bootable while the finished project's name
# stops occupying it. An image with NO tag at all has no other claimant by
# construction and is removed by id.
#
# ## What is deliberately NOT reclaimed here
#
# **Build cache.** It is content-addressed and global by construction: a cache
# record carries no compose project (`docker buildx du --verbose` shows
# `Shared: true` and a `from local` description, and `docker builder prune` has no
# project filter), so a project cannot own one. A per-project tool that ran a
# global prune would be the `--all` shape this teardown already deleted once — an
# operation no caller on a shared host is entitled to. It is left alone, and said.
#
# **Another program's compose project.** The daemon is the creator's, not this
# repository's: the first sweep written here found `mimicat-app:latest`, a
# project belonging to unrelated software on the same machine, sitting in a
# population defined as "every compose project with an image". Two independent
# keys keep this repository's tools inside this repository's objects, and they
# are independent so that a collision in either one is caught by the other:
#
#   * the image's compose SERVICE must be one `validation/compose.yaml` itself
#     declares, read from compose's own parse of that file
#     (`docker compose --profile '*' config --services`) rather than a list
#     anybody has to maintain;
#   * the PROJECT must be named the way every entry script in this repository
#     names one — `--project dw-<id>` in `bot-run.sh`, `packtest-run.sh`,
#     `branch-runs.sh` and `world-save.sh`, `dw-noteflow-$$` / `dw-rehearsal-$$`
#     in the two host-driven flows — unless the caller names it explicitly.
#
# Neither is a substitute for the other and neither is a substitute for the tag
# rule above: they narrow the POPULATION, and the tag rule decides what inside it
# may go.
#
# **Pulled images.** No compose label, never selected: the pinned toolserver
# digest, `itzg/minecraft-server`, and every base layer.
#
# **An image a container holds.** Containers of ANY project are enumerated first
# and the images they reference are exempt by id, with the holder named. That
# exemption demands evidence the defect cannot supply — a live container
# referencing the image — rather than accepting a failed `docker rmi` as a reason
# to skip.

# The label compose stamps on what it builds. Not a glob over names: a name glob
# misses every untagged image and cannot see a project called `validation`.
DW_IMG_LABEL_KEY='com.docker.compose.project'
DW_IMG_SERVICE_KEY='com.docker.compose.service'

# Where this library sits, so it can find the compose file it derives from.
dw_img_lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DW_IMG_COMPOSE="${DW_IMG_COMPOSE:-$dw_img_lib_dir/../compose.yaml}"

# How every entry script in this repository names a ladder project. It is a
# claim about THIS repository's callers, checked against them by
# `tools/tests/test_reclaim_ladder_images.py`, and it exists to keep a sweep from
# reaching software that merely shares the daemon.
DW_IMG_PROJECT_PREFIX='dw'

# The compose project a bare `docker compose -f validation/compose.yaml …` lands
# in (the basename of the compose file's directory). It is a real project like
# any other and this library treats it as one — but it is also where the OWNER's
# `owner-play.yaml` session lands, so the sweep requires it to be named
# explicitly rather than swept by default.
DW_IMG_DEFAULT_PROJECT='validation'

# The services `validation/compose.yaml` declares, from compose's OWN parse of
# it — a checker reads a document the way its consumer reads it, and a
# hand-written list of four names is a second authority that goes stale the first
# time a service is added. `--profile '*'` is what reaches the profiled ones;
# `EULA` is supplied only because the file refuses to interpolate without it, and
# nothing is started. A parse that yields nothing is a REFUSAL: an empty service
# set would silently make every image foreign and the sweep would reclaim
# nothing while reporting success.
DW_IMG_SERVICES=""
dw_ladder_services() {
  if [ -z "$DW_IMG_SERVICES" ]; then
    DW_IMG_SERVICES="$(EULA=enumerate-only docker compose -f "$DW_IMG_COMPOSE" \
      --profile '*' config --services 2>/dev/null || true)"
    if [ -z "$DW_IMG_SERVICES" ]; then
      echo "ladder-images: could not read the services of $DW_IMG_COMPOSE — refusing" >&2
      echo "  to judge any image foreign on an empty answer." >&2
      return 1
    fi
  fi
  printf '%s\n' "$DW_IMG_SERVICES"
}

# Is this image one THIS repository's compose file builds?
dw_service_is_ladder() {
  local svc="$1" known
  [ -n "$svc" ] || return 1
  known="$(dw_ladder_services)" || return 1
  case $'\n'"$known"$'\n' in
    *$'\n'"$svc"$'\n'*) return 0 ;;
  esac
  return 1
}

# Does this project name follow this repository's own ladder convention?
dw_project_is_ladder() {
  case "$1" in
    "$DW_IMG_PROJECT_PREFIX"*) return 0 ;;
  esac
  return 1
}

# Every image the daemon holds that compose stamped for this project, tagged or
# not. Full ids (`--no-trunc`), so they compare against a container's `.Image`.
dw_project_image_ids() {
  { docker images -a -q --no-trunc --filter "label=$DW_IMG_LABEL_KEY=$1" 2>/dev/null || true; } | sort -u
}

# Every compose project that has an image on this daemon, one per line. Derived
# from the objects, never from a name pattern — which is how `validation` and any
# future non-`dw-` project stay visible instead of being silently out of scope.
dw_image_projects() {
  local ids
  ids="$( { docker images -a -q --no-trunc --filter "label=$DW_IMG_LABEL_KEY" 2>/dev/null || true; } | sort -u)"
  [ -n "$ids" ] || return 0
  # shellcheck disable=SC2086  # deliberate word splitting: one id per argument
  { docker image inspect $ids --format "{{index .Config.Labels \"$DW_IMG_LABEL_KEY\"}}" 2>/dev/null || true; } |
    grep -v '^$' | sort -u || true
}

# id \t size(bytes) \t service \t comma-joined tags, one line per image.
# One batched inspect rather than one call per image: the sweep reads hundreds.
dw_project_image_table() {
  local ids
  ids="$(dw_project_image_ids "$1")"
  [ -n "$ids" ] || return 0
  # shellcheck disable=SC2086  # deliberate word splitting: one id per argument
  docker image inspect $ids \
    --format "{{.Id}}	{{.Size}}	{{index .Config.Labels \"$DW_IMG_SERVICE_KEY\"}}	{{join .RepoTags \",\"}}" \
    2>/dev/null || true
}

# The image id every container on this daemon references — of ANY project,
# running or exited. `.Image` is the resolved id, so a container started from a
# tag that has since moved still protects the image it actually holds.
dw_container_image_ids() {
  local ids
  ids="$(docker ps -aq 2>/dev/null || true)"
  [ -n "$ids" ] || return 0
  # shellcheck disable=SC2086  # deliberate word splitting: one id per argument
  { docker container inspect $ids --format '{{.Image}}' 2>/dev/null || true; } | sort -u
}

# Which container holds an image id (for the KEPT line's reason).
dw_container_holding() {
  local ids id
  ids="$(docker ps -aq 2>/dev/null || true)"
  [ -n "$ids" ] || return 0
  for id in $ids; do
    if [ "$(docker container inspect "$id" --format '{{.Image}}' 2>/dev/null || true)" = "$1" ]; then
      docker container inspect "$id" --format '{{.Name}} ({{.State.Status}})' 2>/dev/null || true
      return 0
    fi
  done
}

# Is this tag a name THIS ladder minted for THIS project? See the header: the
# service comes from the image's own label, never from a prefix match, because
# `dw-m5-final-bot:latest` starts with `dw-m5-`.
dw_tag_is_project_owned() {
  local project="$1" service="$2" tag="$3"
  [ -n "$tag" ] || return 1
  if [ -n "$service" ] && [ "$tag" = "$project-$service:latest" ]; then
    return 0
  fi
  # `<repo>:<project>` — the DELVE_IMAGE convention (`delvewright/delve:<project>`).
  case "$tag" in
    *:"$project") return 0 ;;
  esac
  return 1
}

# Reclaim one project's images.
#
#   dw_reclaim_project_images <project> [dry]
#
# `dry` non-empty lists what it WOULD remove and touches nothing. Sets, in the
# caller's shell (never call this in a subshell — the counts are the binding):
#
#   DW_IMG_REMOVED       how many images/tags went (or would go)
#   DW_IMG_BYTES         their summed `.Size`, an UPPER bound on disk returned:
#                        layers shared between images are counted once per image
#   DW_IMG_KEPT          how many were kept, each with a reason
#   DW_IMG_EXAMINED      how many carried this project's label at all — the
#                        binding count, and a zero is a fact worth printing
#   DW_IMG_UNJUDGED      1 when the service set could not be read, so NOTHING was
#                        judged. Not the same as "nothing to remove", and the
#                        caller must say which — a tool that reclaims nothing
#                        while reporting clean is the vacuity this file is against
#   DW_IMG_REMOVED_LINES / DW_IMG_KEPT_LINES   one `  <name>  <why>` line each
dw_reclaim_project_images() {
  local project="$1" dry="${2:-}"
  local held held_test id size service tags tag owned foreign name rc err
  DW_IMG_REMOVED=0
  DW_IMG_BYTES=0
  DW_IMG_KEPT=0
  DW_IMG_EXAMINED=0
  DW_IMG_REMOVED_LINES=""
  DW_IMG_KEPT_LINES=""
  DW_IMG_UNJUDGED=0

  # Resolve the service set ONCE, up front. Asking per image would let a
  # transient failure read as "every image is foreign", i.e. reclaim nothing and
  # report success — so the failure is recorded as a state the caller must print.
  if ! dw_ladder_services >/dev/null; then
    DW_IMG_UNJUDGED=1
    return 0
  fi

  held="$(dw_container_image_ids)"
  local table
  table="$(dw_project_image_table "$project")"
  [ -n "$table" ] || return 0

  # A heredoc, not a pipe: a `while … | read` loop runs in a subshell and every
  # count above would be lost at the end of it — silently, reported as zero.
  while IFS=$'\t' read -r id size service tags; do
    [ -n "$id" ] || continue
    DW_IMG_EXAMINED=$((DW_IMG_EXAMINED + 1))
    case "$size" in ''|*[!0-9]*) size=0 ;; esac

    # Membership by CASE, never `printf … | grep -q`: grep exits at the first
    # match and SIGPIPEs its producer, which under the caller's `pipefail` reads
    # as NO MATCH — and this test is the one thing standing between the sweep and
    # an image a live container is holding.
    held_test=$'\n'"$held"$'\n'
    case "$held_test" in
      *$'\n'"$id"$'\n'*)
        DW_IMG_KEPT=$((DW_IMG_KEPT + 1))
        DW_IMG_KEPT_LINES="$DW_IMG_KEPT_LINES  ${tags:-${id#sha256:}}  held by container $(dw_container_holding "$id")"$'\n'
        continue
        ;;
    esac

    if ! dw_service_is_ladder "$service"; then
      DW_IMG_KEPT=$((DW_IMG_KEPT + 1))
      DW_IMG_KEPT_LINES="$DW_IMG_KEPT_LINES  ${tags:-${id#sha256:}}  service '${service:-<none>}' is not one $(basename "$DW_IMG_COMPOSE") declares — another program's project"$'\n'
      continue
    fi

    if [ -z "$tags" ]; then
      # No tag, so no other claimant: remove by id.
      if [ -n "$dry" ]; then
        DW_IMG_REMOVED=$((DW_IMG_REMOVED + 1))
        DW_IMG_BYTES=$((DW_IMG_BYTES + size))
        DW_IMG_REMOVED_LINES="$DW_IMG_REMOVED_LINES  ${id#sha256:}  untagged ${service:-?} build"$'\n'
      else
        err="$(docker rmi "$id" 2>&1 >/dev/null)" && rc=0 || rc=$?
        if [ "$rc" -eq 0 ]; then
          DW_IMG_REMOVED=$((DW_IMG_REMOVED + 1))
          DW_IMG_BYTES=$((DW_IMG_BYTES + size))
          DW_IMG_REMOVED_LINES="$DW_IMG_REMOVED_LINES  ${id#sha256:}  untagged ${service:-?} build"$'\n'
        else
          DW_IMG_KEPT=$((DW_IMG_KEPT + 1))
          DW_IMG_KEPT_LINES="$DW_IMG_KEPT_LINES  ${id#sha256:}  the daemon refused: $(printf '%s' "$err" | tr '\n' ' ')"$'\n'
        fi
      fi
      continue
    fi

    # Tagged: every tag is judged on its own. A foreign tag keeps the image
    # alive, and that is the point — see the header on `delvewright/delve:local`.
    owned=""
    foreign=""
    local old_ifs="$IFS"
    IFS=,
    for tag in $tags; do
      if dw_tag_is_project_owned "$project" "$service" "$tag"; then
        owned="$owned $tag"
      else
        foreign="$foreign $tag"
      fi
    done
    IFS="$old_ifs"

    for name in $foreign; do
      DW_IMG_KEPT=$((DW_IMG_KEPT + 1))
      DW_IMG_KEPT_LINES="$DW_IMG_KEPT_LINES  $name  not this project's name for it — kept, and it keeps the image"$'\n'
    done

    for name in $owned; do
      if [ -n "$dry" ]; then
        DW_IMG_REMOVED=$((DW_IMG_REMOVED + 1))
        DW_IMG_BYTES=$((DW_IMG_BYTES + size))
        DW_IMG_REMOVED_LINES="$DW_IMG_REMOVED_LINES  $name  ${service:-?}"$'\n'
      else
        err="$(docker rmi "$name" 2>&1 >/dev/null)" && rc=0 || rc=$?
        if [ "$rc" -eq 0 ]; then
          DW_IMG_REMOVED=$((DW_IMG_REMOVED + 1))
          DW_IMG_BYTES=$((DW_IMG_BYTES + size))
          DW_IMG_REMOVED_LINES="$DW_IMG_REMOVED_LINES  $name  ${service:-?}"$'\n'
        else
          DW_IMG_KEPT=$((DW_IMG_KEPT + 1))
          DW_IMG_KEPT_LINES="$DW_IMG_KEPT_LINES  $name  the daemon refused: $(printf '%s' "$err" | tr '\n' ' ')"$'\n'
        fi
      fi
    done
  done <<EOF
$table
EOF
}

# What is left carrying this project's label that this project still OWNS a name
# for — the proof, run after the removal. Prints one offender per line; empty is
# the pass. An image kept alive by a foreign tag is not an offender: it was never
# this project's to remove.
dw_project_images_remaining() {
  local project="$1" id size service tags tag table
  table="$(dw_project_image_table "$project")"
  [ -n "$table" ] || return 0
  while IFS=$'\t' read -r id size service tags; do
    [ -n "$id" ] || continue
    dw_service_is_ladder "$service" || continue
    if [ -z "$tags" ]; then
      printf '%s  (untagged %s build)\n' "${id#sha256:}" "${service:-?}"
      continue
    fi
    local old_ifs="$IFS"
    IFS=,
    for tag in $tags; do
      if dw_tag_is_project_owned "$project" "$service" "$tag"; then
        printf '%s\n' "$tag"
      fi
    done
    IFS="$old_ifs"
  done <<EOF
$table
EOF
}

# Bytes as a human reads them. `docker` prints GB/MB decimal, so this does too.
dw_img_human_bytes() {
  awk -v b="${1:-0}" 'BEGIN {
    if (b >= 1000000000) { printf "%.2f GB", b / 1000000000 }
    else if (b >= 1000000) { printf "%.1f MB", b / 1000000 }
    else { printf "%d B", b }
  }'
}
