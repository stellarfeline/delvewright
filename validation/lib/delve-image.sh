#!/usr/bin/env bash
# The repo's ONE rule for naming the delve image a ladder builds.
#
#   . validation/lib/delve-image.sh
#   dw_export_delve_image "$project"     # exports DELVE_IMAGE for compose
#
# ## Why an image tag is the third Docker-global name
#
# `docker compose -p <project>` isolates containers, volumes and networks. It does
# NOT isolate container names, published host ports — or IMAGE TAGS. An image tag
# is a key in the daemon's single global image store, so two ladders that build
# different trees into one tag race: the second build re-points the tag while the
# first is still booting from it, and the loser runs the other ladder's delve.
# Nothing errors, and the run report describes a campaign that was never tested.
#
# `validation/compose.yaml` therefore templates the tag on every service that
# BUILDS the delve (`image: ${DELVE_IMAGE:-…}`), and every entry script scopes it
# to its own compose project — the same name the project already isolates
# everything else by.
#
# ## Why it lives here rather than in each script
#
# It was written inline in `bot-run.sh` with the race spelled out in a comment,
# and `world-save.sh` copied the line. `branch-runs.sh` — which runs the SAME
# `validate` profile with `up --build` — never set it at all and built into the
# shared `delvewright/delve:local`, as did the two `playtest`-profile flows,
# whose own headers claim they carry "no name either of them could collide
# with". A rule that has to be remembered at each new call site is the weakest
# form there is, so it is one function, and `tools/check-compose-isolation.py`
# fails any `validation/*.sh` that runs a compose `up --build` without calling it.
#
# The DEFAULT is unchanged: a bare `docker compose` that sets nothing still builds
# `delvewright/delve:local`, which is what `owner-play.yaml`'s single-session,
# mutex-guarded owner path has always used.

# dw_delve_image <compose-project> -> the tag that project owns.
dw_delve_image() {
  printf 'delvewright/delve:%s\n' "${1:?dw_delve_image needs the compose project}"
}

# dw_export_delve_image <compose-project>
# Export DELVE_IMAGE so `docker compose` tags this ladder's build with it.
dw_export_delve_image() {
  DELVE_IMAGE="$(dw_delve_image "${1:?dw_export_delve_image needs the compose project}")"
  export DELVE_IMAGE
}
