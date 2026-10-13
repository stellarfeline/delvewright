#!/usr/bin/env bash
# playtest-server.sh — build a thing and serve it locally for the owner to walk
# (Multiplayer -> Direct Connect localhost:25565), or tear it down.
#
#   tools/creator/playtest-server.sh up <path> [--lang LANG] [--prefabs DIR]
#                                [--delvec BIN] [--name NAME] [--out DIR]
#                                [--memory SIZE]
#                                [--run-report FILE --written-world FILE]
#                                [--stage-anyway "REASON" --acknowledge-red N]
#   tools/creator/playtest-server.sh down [--name NAME] [--prune]
#   tools/creator/playtest-server.sh status [--name NAME]
#
# ## A campaign is served from a kept image, and reopening is `docker run`
#
# `up` on a CAMPAIGN ends in a local OCI image of the admitted build with its
# world already placed, and serves by running it:
#
#   delvewright/playtest-build:<manifest sha256>  validation/Dockerfile.delve
#       over the build output: the shipped delve image (ADR-0010), unchanged.
#   delvewright/playtest:<manifest sha256>        validation/Dockerfile.playtest
#       over that: the world the shipped image placed at its first boot, the
#       staging gate's token, and the creator overlay and resource pack.
#
# Both carry `delvewright.playtest.*` labels: the INPUTS key (the delvec binary,
# the campaign tree, the prefab library, `--lang`, the findings ledger and the
# staging gate, the two Dockerfiles and this script, each hashed), the
# manifest's sha256, the admission token's sha256 and the pack's SHA-1.
#
# A later `up` of the same inputs finds the image by its inputs label and goes
# straight to `docker run`: no build, no gate, no placement. Before it serves,
# the image's own manifest and token are copied out and held to its labels and
# to each other (`validation/staging-admission.sh`, the verifier the other
# owner-facing path runs); a mismatch is refused, named. Any input that moved —
# a campaign edit, another engine, a new ledger row — is a different key, and
# the build is made and gated afresh.
#
# A reopen is a FRESH delve: every run starts from the placed world in the
# image, never from a previous session's progress.
#
# ## One command, one path, two kinds of thing
#
# This engine makes two kinds of artifact a person might want to stand inside: a
# CAMPAIGN, and a PREFAB — a building that exists before any campaign places it,
# and which for a zone past the 48-per-axis template cap ships as several `.nbt`
# tiles plus a manifest. There was a one-command path to the first and none at
# all to the second, so the answer to "can I walk it" depended on which kind the
# creator happened to be holding. It should not: they type one thing.
#
# So `up` takes whichever it is handed, and says which it read:
#
#   a directory holding `world.json`   -> a campaign. Built and gated exactly as
#                                         before, byte for byte.
#   a `.nbt`, a `.json` tile-set manifest, or a directory of `.nbt`
#                                      -> a prefab. Built into a browse world by
#                                         `delvec prefab gallery` and served.
#
# Neither shape is guessed at: a directory that is neither is refused with both
# shapes named, rather than being built as whichever branch happened to run.
#
# `up` on a CAMPAIGN builds, then runs the staging gate against that exact tree,
# and only then starts a container: no build reaches the owner while a past
# finding's general form is not a live, binding check on it
# (playtest-methodology.md rule 7). A refusal exits non-zero and prints the red
# list. The gate admits only with the two machine proofs of that build:
# `--run-report` (the bot ladder's `run-report.json`, critical path green) and
# `--written-world` (`tools/ci/check-written-world.py --record`, the server's
# world equal to the engine's model), each named by the build's manifest, so the
# build this script makes must be the one the ladder ran on — a deterministic
# rebuild of the same campaign is. `--stage-anyway "<reason>" --acknowledge-red
# <N>` overrides the ledger deliberately — it prints every class being
# overridden and stamps the reason into the build's admission token — and does
# not reach the proofs.
#
# ## What the prefab path does about the staging gate, and why that is honest
#
# It does not run it, and it says so in those words on every run rather than
# passing silently. The gate judges a CAMPAIGN against the findings ledger —
# `--campaign <dir>` is required, and every row it reads is a claim about a
# delve's content. A browse world has no campaign to judge: no quests, no NPCs,
# no player progression, and nothing that ships. Running the gate with an
# invented campaign argument, or with the rows it could not evaluate scored as
# passes, would be a green that binds to nothing — which is the exact shape
# CLAUDE.md forbids, and worse than no gate because it reads as one.
#
# What the prefab path is admitted by instead is stated, not implied, and it is a
# real check rather than a courtesy: `delvec prefab gallery` validates every
# `.mcfunction` line it emits against the pinned 1.21.11 command tree BEFORE it
# writes anything, refuses a lone tile of a tiled zone (`DW0739`), and refuses a
# manifest that does not tile its own zone. Then this script reads what the
# server says back — the pack's objectives, and one label entity per exhibit —
# so a datapack the server dropped at load cannot come up looking ready.
#
# The gate is never weakened, skipped or made optional on the campaign path to
# make this convenient. A campaign still gets the gate; a prefab was never in its
# scope.
#
# `--delvec BIN` names an engine outright. WITHOUT it, this script uses the
# `delvec` already on `PATH` when that binary IS this engine — its `--version`
# equal to `versions.toml` `[engine].version`, which is what a creator's `Init`
# puts there (ADR-0023) — and otherwise builds one from source. Either way it
# prints, in one line, which binary it chose and why, before it builds anything.
# `tools/lib/delvec-bin.sh` holds the rule and is shared with the other creator
# step that reaches for `delvec`, `validation/render-shots.sh`.
#
# Owner-facing contract: `up` ends by printing the connect address, how many
# seconds the run took from the command to joinable and, if the build ships a
# resource pack, where it is served and its SHA-1. `down` removes the
# containers, reclaims the session's directories and prints what it reclaimed
# (server lifecycle rule: tear down as soon as feedback arrives); it KEEPS the
# playtest images and lists them with their sizes. `down --prune` also removes
# every kept playtest image, printing each one's size. `status` lists the
# containers, what the session is holding on disk, and the kept images with
# their manifest and size.
#
# ## The delve's resource pack is SERVED, never installed (spec-0084 §11)
#
# (What follows is the prefab path's staging. On a campaign the sidecar serves
# the pack copied out of the playtest image, and the URL reaches the server as
# DELVE_RESOURCE_PACK_URL, which the delve image's entrypoint turns into
# `resource-pack` with the manifest's SHA-1.)
#
# A pack copied into a player's own `resourcepacks/` stays enabled in every world
# and server they open until they find the toggle — a delve that replaces the
# moon would replace it everywhere. Served, it applies while the player is
# connected and is gone when they leave. So a build that ships
# `resourcepack.zip` gets a SIDECAR, `<name>-pack`: a busybox httpd (spec-0009's
# verified pattern, pinned in versions.toml [images.pack_server]) over the build
# output, published on 127.0.0.1:25580 — the address a client on this machine
# reaches — and the staged `server.properties` carries `resource-pack`,
# `resource-pack-sha1` (the manifest's) and `resource-pack-prompt` (a JSON text
# component). They are written into the file, not passed as itzg's
# RESOURCE_PACK* variables, because this server runs with
# OVERRIDE_SERVER_PROPERTIES=false and itzg applies none of its variables to a
# file it was told not to touch. Before the server starts, the URL is fetched
# and its SHA-1 held to the manifest's, so a pack the client would fail to
# download refuses here. A campaign's own `require-resource-pack=true` arrives
# in the copied file as it stands. DELVEWRIGHT_RESOURCEPACKS_DIR is never
# written: when it is set, this says so and leaves it alone.
#
# A campaign's container runs its playtest image (above); a prefab's is a
# throwaway itzg/minecraft-server pinned to the project MC version over a
# staged /data. Either binds host 25565 — with `validation/owner-play.yaml`, one of the two
# sanctioned 25565 bindings; nothing else in the repo may publish that port
# (`tools/ci/check-compose-isolation.py`, and validation ladders are project-scoped
# with no host port at all). Because 25565 is the one genuinely shared resource
# left on this host, `up` TAKES the 25565 mutex as `owner-play-session` and `down`
# releases it: while it is held, no automation may bind the port, and the release
# is refused while any container still publishes it (validation/mutex.sh).
#
# The container's heap ceiling is `versions.toml` `[server].heap_max`, applied
# by `tools/lib/server-heap.sh` — the same default every server this engine
# starts gets — unless `--memory` says otherwise. itzg's OWN default is 1G,
# which is not enough for a large-ish campaign: 84 tiles plus 170 horizon
# templates OOM'd it
# (`java.lang.OutOfMemoryError: Java heap space`), NPCs never spawned, and the
# probe that checks for them died reporting a missing-NPC defect rather than
# an out-of-memory one. A probe failure now stops and removes the container
# and releases the mutex before this script exits — nothing is left running
# for a creator to discover later — and an OOM specifically is reported as
# what it is, not as whatever downstream symptom it caused.
set -euo pipefail

# `${BASH_SOURCE[0]}`, never `$0`: this file is executed directly in normal use
# but SOURCED under DW_PLAYTEST_SERVER_TEST_HOOK (below) so a test can reach
# the argv-building / OOM-detection functions without a docker or a delvec —
# `$0` under `source` is the interpreter's own path, not this file's.
# shellcheck source=validation/mutex.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/validation/mutex.sh"
set -euo pipefail  # mutex.sh sets its own options when sourced; take ours back
# The heap default and the OOM rule every server this engine starts shares.
# shellcheck source=tools/lib/server-heap.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/tools/lib/server-heap.sh"

MC_VERSION="1.21.11"
# The resource-pack sidecar (spec-0084 §11): the pinned image and the loopback
# port a client on this machine fetches the pack from. The port is under the
# same 25565 mutex as the server, so one session holds both at a time.
PACK_IMAGE="busybox:1.37@sha256:9532d8c39891ca2ecde4d30d7710e01fb739c87a8b9299685c63704296b16028"
PACK_PORT="25580"
NAME="dw-playtest"
LANG_ARG=""
PREFABS_ARG=""
DELVEC=""
OUT_DIR=""
STAGE_ANYWAY=""
ACK_RED=""
RUN_REPORT=""
WRITTEN_WORLD=""
RCON_PW="playtest"
# Empty = the shared default (`dw_server_heap_env`, versions.toml
# `[server].heap_max` as the ceiling). `--memory` is the escape hatch for a
# campaign that default is still not enough for — the probe-failure path below
# names an OOM as an OOM rather than a missing-NPC defect regardless, and leaves
# nothing running either way.
MEMORY_ARG=""

die() { echo "playtest-server: $*" >&2; exit 1; }

# Whether NAME is a container docker knows about — never inferred from `rm`'s
# own exit status, which still succeeds removing nothing on modern docker.
# Shared by `down` (a session ending normally) and `up_failed` (one that never
# got that far): both must ask this exact question about the exact resource.
dw_playtest_container_exists() {
  docker container inspect "$1" >/dev/null 2>&1
}

# The docker-run argv for the throwaway server, one token per line — a seam:
# the flow below needs a real docker to run it, and this function does not, so
# a unit test can assert the exact flags (in particular the heap's `-e`)
# without a docker on PATH. <heap_env> is `dw_server_heap_env`'s output.
dw_playtest_docker_run_argv() {
  local name="$1" mc_version="$2" rcon_pw="$3" heap_env="$4" stage="$5"
  printf '%s\n' docker run -d --name "$name" -p 25565:25565 \
    -e EULA=TRUE -e TYPE=VANILLA -e VERSION="$mc_version" \
    -e "$heap_env" \
    -e RCON_PASSWORD="$rcon_pw" -e OVERRIDE_SERVER_PROPERTIES=false \
    -v "$stage:/data" itzg/minecraft-server:latest
}

# The pack sidecar's docker-run argv, one token per line — the same seam as
# the server's: a test asserts the flags without a docker. <out> is the build
# output directory the sidecar serves, read-only.
dw_playtest_pack_run_argv() {
  local name="$1" image="$2" port="$3" out="$4"
  printf '%s\n' docker run -d --name "$name-pack" -p "127.0.0.1:$port:8000" \
    -v "$out:/srv:ro" --entrypoint httpd "$image" -f -p 8000 -h /srv
}

# The three properties that serve the pack, appended to the staged
# server.properties — written into the file because this server runs with
# OVERRIDE_SERVER_PROPERTIES=false (see the header).
dw_playtest_pack_properties() {
  local url="$1" sha1="$2"
  printf 'resource-pack=%s\nresource-pack-sha1=%s\nresource-pack-prompt=%s\n' \
    "$url" "$sha1" '{"text":"This delve brings its own resource pack."}'
}

# ---- the playtest image: names, labels, and the seams that build its argv ----
#
# Two repositories in the daemon's image store, both keyed by the build's
# manifest sha256 (validation/Dockerfile.playtest's header says what each holds).
# Not a compose project's tag (validation/lib/delve-image.sh): a playtest image
# is one admitted build, wanted again by any later session of the same inputs,
# so it is named by the build and not by the session.
PLAYTEST_REPO="delvewright/playtest"
PLAYTEST_BUILD_REPO="delvewright/playtest-build"
PLAYTEST_LABEL="delvewright.playtest"

# dw_playtest_image_tag <repo> <manifest sha256>
dw_playtest_image_tag() {
  printf '%s:%s\n' "$1" "$2"
}

# The serving container's docker-run argv, one token per line — the same seam
# as the prefab path's: a test asserts the flags without a docker. <heap_env>
# is `dw_server_heap_env`'s output; <pack_url> is empty for a build with no
# resource pack (the delve image's entrypoint turns it into `resource-pack`
# with the manifest's SHA-1).
dw_playtest_serve_argv() {
  local name="$1" image="$2" rcon_pw="$3" heap_env="$4" stage="$5" pack_url="$6"
  printf '%s\n' docker run -d --name "$name" -p 25565:25565 \
    -e EULA=TRUE -e RCON_PASSWORD="$rcon_pw" -e "$heap_env"
  if [ -n "$pack_url" ]; then printf '%s\n' -e "DELVE_RESOURCE_PACK_URL=$pack_url"; fi
  printf '%s\n' -v "$stage:/data" "$image"
}

# The placement boot's argv: the shipped delve image, run once to place its
# world. NO host port — nobody can join a world that is still being built, and
# this boot does not need 25565 to be judged (rcon is `docker exec`). The
# creator overlay, when the build has one, is mounted read-only where the
# playtest image will carry it, and named in DATAPACKS, so the world is placed
# with the same packs it is later served with.
#
# The vanilla watchdog is OFF here (`max-tick-time=-1`, itzg's MAX_TICK_TIME).
# Placing a world is a few very long ticks by design, and how long a tick takes
# is the host's business: on a laptop running other builds a placement tick
# passed the default 60 s, the watchdog halted the server (`A single server tick
# took 60.00 seconds`, exit 1) and the placement failed with nothing wrong with
# the delve. A placement that truly hangs is still bounded, by name, by
# DW_PLAYTEST_PLACED_BUDGET_S. The serving boot keeps the shipped default.
DW_PLAYTEST_PLACE_TICK_ENV="MAX_TICK_TIME=-1"
dw_playtest_place_argv() {
  local name="$1" image="$2" rcon_pw="$3" heap_env="$4" stage="$5" creator="$6"
  printf '%s\n' docker run -d --name "$name-place" \
    -e EULA=TRUE -e RCON_PASSWORD="$rcon_pw" -e "$heap_env" -e "$DW_PLAYTEST_PLACE_TICK_ENV"
  if [ -n "$creator" ]; then
    printf '%s\n' -v "$creator:/delve/extras/creator-datapack:ro" \
      -e "DATAPACKS=/delve/datapack,/delve/extras/creator-datapack"
  fi
  printf '%s\n' -v "$stage:/data" "$image"
}

# dw_sha256_file <file> — bare hex, `sha256sum` where it exists, else `shasum`.
dw_sha256_file() {
  local out
  if command -v sha256sum >/dev/null 2>&1; then out="$(sha256sum "$1")" || return 1
  else out="$(shasum -a 256 "$1")" || return 1; fi
  printf '%s\n' "${out%% *}"
}

# The engine files an admitted image depends on beyond the build itself: the
# findings ledger and the gate that judged it, the two Dockerfiles the image is
# made from, the token's verifier, and this script (which places the world).
DW_PLAYTEST_INPUT_FILES="docs/playtest-findings.json tools/creator/staging-gate.py validation/Dockerfile.delve validation/Dockerfile.playtest validation/staging-admission.sh tools/creator/playtest-server.sh"

# dw_playtest_inputs_key <delvec> <campaign> <prefabs dir or empty> <lang> <repo root>
#
# The identity of everything a campaign build and its admission are made from,
# BEFORE building: an image whose inputs label equals this is the build these
# inputs make (ADR-0006: same inputs, same bytes) and the admission they get.
# Each component is printed, then the key, so what the key resolved to is on
# the page. A tree is hashed with `tools/ci/tree-digest.py` (relative paths
# beside content hashes, a symlink refused); a component that cannot be read
# fails the key rather than dropping out of it.
dw_playtest_inputs_key() {
  local scratch rc=0
  scratch="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-key.XXXXXX")" || return 1
  dw_playtest_inputs_list "$@" >"$scratch/inputs" || rc=1
  if [ "$rc" = 0 ]; then
    sed 's/^/  inputs: /' "$scratch/inputs" >&2
    dw_sha256_file "$scratch/inputs" || rc=1
  fi
  rm -rf "$scratch"
  return "$rc"
}

# dw_playtest_inputs_list — the components, one `<name> <sha256>` per line.
# A tree's digest file goes to a scratch directory of its own, removed on every
# path out.
dw_playtest_inputs_list() {
  local delvec="$1" campaign="$2" prefabs="$3" lang="$4" root="$5" scratch rel sum rc=0
  scratch="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-tree.XXXXXX")" || return 1
  if ! sum="$(dw_sha256_file "$delvec")"; then
    echo "playtest-server: inputs key: cannot read the delvec binary $delvec" >&2; rc=1
  fi
  [ "$rc" = 0 ] && printf 'delvec %s\n' "$sum"
  if [ "$rc" = 0 ]; then
    if python3 "$root/tools/ci/tree-digest.py" --root "$campaign" --out "$scratch/campaign" >/dev/null 2>&1; then
      printf 'campaign %s\n' "$(dw_sha256_file "$scratch/campaign")"
    else
      echo "playtest-server: inputs key: cannot digest the campaign tree $campaign" >&2; rc=1
    fi
  fi
  if [ "$rc" = 0 ]; then
    if [ -n "$prefabs" ] && [ -d "$prefabs" ]; then
      if python3 "$root/tools/ci/tree-digest.py" --root "$prefabs" --out "$scratch/prefabs" >/dev/null 2>&1; then
        printf 'prefabs %s\n' "$(dw_sha256_file "$scratch/prefabs")"
      else
        echo "playtest-server: inputs key: cannot digest the prefab library $prefabs" >&2; rc=1
      fi
    else
      printf 'prefabs absent:%s\n' "$prefabs"
    fi
  fi
  [ "$rc" = 0 ] && printf 'lang %s\n' "${lang:-default}"
  if [ "$rc" = 0 ]; then
    for rel in $DW_PLAYTEST_INPUT_FILES; do
      if [ -f "$root/$rel" ]; then
        printf '%s %s\n' "$rel" "$(dw_sha256_file "$root/$rel")"
      else
        echo "playtest-server: inputs key: $root/$rel is missing" >&2; rc=1; break
      fi
    done
  fi
  rm -rf "$scratch"
  return "$rc"
}

# dw_playtest_identity_check <dir> <tag manifest> <label manifest> <label admission>
#
# <dir> holds the image's own `manifest.json` and `staging-admission.json`,
# copied out of it. Four readings must agree: the manifest's sha256, the tag it
# is served under, its manifest label, and the token's `build_fingerprint`
# (read by validation/staging-admission.sh, the one verifier of a token); and
# the token's sha256 with the admission label. On a mismatch sets
# DW_IDENTITY_FAILURE to the readings that disagree and returns 1.
dw_playtest_identity_check() {
  local dir="$1" tag="$2" lmanifest="$3" ladmit="$4" actual admit verdict
  DW_IDENTITY_FAILURE=""
  [ -f "$dir/manifest.json" ] || { DW_IDENTITY_FAILURE="the image carries no /delve/manifest.json"; return 1; }
  [ -f "$dir/staging-admission.json" ] || { DW_IDENTITY_FAILURE="the image carries no /delve/staging-admission.json — it was never admitted"; return 1; }
  actual="$(dw_sha256_file "$dir/manifest.json")"
  if [ "$actual" != "$tag" ] || [ "$actual" != "$lmanifest" ]; then
    DW_IDENTITY_FAILURE="manifest mismatch: the image's manifest.json hashes to $actual, its tag says ${tag:-nothing}, its label says ${lmanifest:-nothing}"
    return 1
  fi
  admit="$(dw_sha256_file "$dir/staging-admission.json")"
  if [ "$admit" != "$ladmit" ]; then
    DW_IDENTITY_FAILURE="admission mismatch: the image's staging-admission.json hashes to $admit, its label says ${ladmit:-nothing}"
    return 1
  fi
  if ! verdict="$(bash "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/validation/staging-admission.sh" "$dir" 2>&1)"; then
    DW_IDENTITY_FAILURE="the image's token does not admit its own manifest: $verdict"
    return 1
  fi
  printf '%s\n' "$verdict"
}

# The line the vanilla watchdog logs as it halts a server whose tick ran past
# `max-tick-time`; read so a halt is reported as what it is, not as whatever
# probe it interrupted.
DW_SERVER_WATCHDOG_MARKER='A single server tick took'

# dw_server_log_shows_watchdog <text>
dw_server_log_shows_watchdog() {
  case "${1-}" in
    *"$DW_SERVER_WATCHDOG_MARKER"*) return 0 ;;
    *) return 1 ;;
  esac
}

# dw_playtest_wait_done <container> <budget_s> <poll_s>
#
# Poll the container's log for the server's `Done (` line. A container that
# stopped, an OOM in the log, or a spent budget sets DW_DONE_FAILURE and returns
# 1. Each container here is fresh, so its log holds this boot and no other.
dw_playtest_wait_done() {
  local container="$1" budget="$2" poll="$3" waited=0 log state
  DW_DONE_FAILURE=""
  while :; do
    log="$(docker logs "$container" 2>&1 || true)"
    if [[ $log == *"Done ("* ]]; then return 0; fi
    if dw_server_log_shows_oom "$log"; then
      DW_DONE_FAILURE="booting: $(dw_server_oom_advice)"; return 1
    fi
    state="$(docker inspect -f '{{.State.Status}}' "$container" 2>/dev/null || true)"
    if [ "$state" = "exited" ] || [ "$state" = "dead" ]; then
      DW_DONE_FAILURE="the server stopped ($state) before it logged \`Done (\`"; return 1
    fi
    if [ "$waited" -ge "$budget" ]; then
      DW_DONE_FAILURE="the server did not log \`Done (\` within ${budget}s"; return 1
    fi
    sleep "$poll"
    waited=$((waited + poll))
  done
}

# dw_playtest_bake_context <world> <bake dir> <token> <creator dir or ""> <pack zip or "">
#
# Assembles validation/Dockerfile.playtest's context and prints the DATAPACKS
# value the image boots with. The world MOVES (no copy). Its `datapacks/` stay
# behind, because every boot copies them in from /delve, and so does the lock
# of a server now stopped. Every file is made readable by any user: the server
# writes level.dat 0600, a layer keeps that mode under root, and the image's
# server runs as another user, which then cannot read the world it is handed.
dw_playtest_bake_context() {
  local world="$1" bake="$2" token="$3" creator="$4" pack="$5" datapacks="/delve/datapack"
  mkdir -p "$bake/extras" || return 1
  mv "$world" "$bake/placed-world" || return 1
  rm -rf "$bake/placed-world/datapacks" "$bake/placed-world/session.lock"
  cp "$token" "$bake/staging-admission.json" || return 1
  if [ -n "$creator" ]; then
    cp -R "$creator" "$bake/extras/creator-datapack" || return 1
    datapacks="$datapacks,/delve/extras/creator-datapack"
  fi
  if [ -n "$pack" ]; then cp "$pack" "$bake/extras/resourcepack.zip" || return 1; fi
  chmod -R a+rX "$bake" || return 1
  printf '%s\n' "$datapacks"
}

# The server runtime a fresh /data needs before it can start without fetching:
# the pinned jar, itzg's "already installed" marker, and what the jar unpacks
# on its first start (`libraries/`, `versions/`). Kept on the creator's own
# disk, never in an image (ADR-0010), and copied — not bind-mounted — into each
# run's /data, because itzg's installer replaces the jar by rename.
DW_PLAYTEST_RUNTIME_ENTRIES=".vanilla-manifest.json libraries versions"

# dw_playtest_runtime_cache_dir <mc version>
dw_playtest_runtime_cache_dir() {
  printf '%s\n' "${DW_PLAYTEST_RUNTIME_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/delvewright/playtest-runtime}/$1"
}

# dw_playtest_runtime_keep <from /data> <cache dir> <mc version> <jar sha256>
# Copies the runtime out of a /data a server has started in, once the jar there
# is the pinned one. A jar that is not is refused into the cache, said, and
# left to be fetched again.
dw_playtest_runtime_keep() {
  local from="$1" cache="$2" ver="$3" want="$4" jar entry have
  jar="minecraft_server.$ver.jar"
  [ -f "$from/$jar" ] || { echo "runtime cache: $from holds no $jar — nothing kept"; return 0; }
  have="$(dw_sha256_file "$from/$jar")"
  if [ "$have" != "$want" ]; then
    echo "runtime cache: $from/$jar hashes to $have, versions.toml pins $want — NOT kept"
    return 0
  fi
  rm -rf "$cache"; mkdir -p "$cache"
  cp "$from/$jar" "$cache/$jar"
  for entry in $DW_PLAYTEST_RUNTIME_ENTRIES; do
    if [ -e "$from/$entry" ]; then cp -R "$from/$entry" "$cache/$entry"; fi
  done
  echo "runtime cache: kept $jar (sha256 $have) and its unpacked runtime at $cache"
}

# dw_playtest_runtime_seed <cache dir> <to /data> <mc version> <jar sha256>
# Seeds a fresh /data from the cache when its jar is the pinned one; otherwise
# says so, and the server fetches its own.
dw_playtest_runtime_seed() {
  local cache="$1" to="$2" ver="$3" want="$4" jar have
  jar="minecraft_server.$ver.jar"
  if [ ! -f "$cache/$jar" ]; then
    echo "runtime: no cached $jar at $cache — the server fetches it"
    return 0
  fi
  have="$(dw_sha256_file "$cache/$jar")"
  if [ "$have" != "$want" ]; then
    echo "runtime: cached $jar hashes to $have, versions.toml pins $want — not used; the server fetches it"
    return 0
  fi
  cp -R "$cache/." "$to/"
  echo "runtime: seeded /data from $cache ($jar sha256 $have)"
}

# The `up` EXIT trap. Defined here (above the test seam) rather than beside
# where it is armed, so a test can call it directly — with NAME, STAGE,
# SESSION_FILE and UP_OK set by hand and a fake docker on PATH — without going
# through a real build, staging gate or boot.
#
# A failed `up` keeps its STAGED WORLD on purpose — `$STAGE/logs/latest.log` is
# where a boot failure is diagnosed, and `die` already sends the reader to the
# logs. What it must not do is leave them unreachable, so it names the command
# that reclaims them. The record is written before the container exists, which
# is what makes that command work after any failure past this point.
#
# The CONTAINER is a different resource and is not kept: leaving it running
# after a probe failure (an OOM loading structures, an unreached objective, a
# missing NPC) would hold host 25565 behind a mutex this same failure already
# released, eating memory nobody is watching for. Every resource this failed
# session holds is named and released here — the container, then
# the mutex — before the directories are reported kept. `docker logs` is
# captured into the staged world FIRST, because it is the one thing that dies
# with the container: `$STAGE/logs/latest.log` is the server's own log and
# survives on disk regardless (it is a bind mount), but the wrapper's own
# stdout is not, and an early failure (before the JVM ever writes that file)
# has nothing else to diagnose from.
up_failed() {
  [ "${UP_OK:-0}" = 1 ] && return 0
  # The placement boot (a campaign's first `up`): its world is in the session's
  # place-dir, which `down` reclaims; its wrapper log is captured beside it.
  if dw_playtest_container_exists "$NAME-place"; then
    if [ -n "${PLACE:-}" ] && [ -d "${PLACE:-}" ]; then
      docker logs "$NAME-place" >"$PLACE/docker-boot.log" 2>&1 || true
    fi
    if docker rm -f "$NAME-place" >/dev/null 2>&1; then
      echo "playtest-server: removed the placement boot this failed session started ($NAME-place)" >&2
    else
      echo "playtest-server: could not remove $NAME-place — remove it by hand: docker rm -f $NAME-place" >&2
    fi
  fi
  if dw_playtest_container_exists "$NAME-pack"; then
    if docker rm -f "$NAME-pack" >/dev/null 2>&1; then
      echo "playtest-server: removed the pack sidecar this failed session started ($NAME-pack)" >&2
    else
      echo "playtest-server: could not remove $NAME-pack — remove it by hand: docker rm -f $NAME-pack" >&2
    fi
  fi
  if dw_playtest_container_exists "$NAME"; then
    if [ -n "${STAGE:-}" ] && [ -d "${STAGE:-}" ]; then
      docker logs "$NAME" >"$STAGE/docker-boot.log" 2>&1 || true
    fi
    if docker rm -f "$NAME" >/dev/null 2>&1; then
      echo "playtest-server: removed the container this failed session started ($NAME)" >&2
    else
      echo "playtest-server: could not remove $NAME — remove it by hand: docker rm -f $NAME" >&2
    fi
  fi
  dw_mutex_release
  if [ -n "${SESSION_FILE:-}" ] && [ -f "$SESSION_FILE" ]; then
    echo "playtest-server: this session left directories behind (see $SESSION_FILE)." >&2
    echo "  reclaim: tools/creator/playtest-server.sh down --name $NAME" >&2
  fi
  return 0
}

# The shared rejection rule and the shared "the world is placed" reading
# (tools/lib/rcon.sh), sourced above the test seam so a test reaches the
# verification below through the same functions `up` calls.
# shellcheck source=tools/lib/rcon.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/tools/lib/rcon.sh"

# The budget (seconds) from the server's `Done (` to the datapack's own
# "placed", and the pause between readings. Stated, not derived: the world build
# runs one block-write step a tick, and how long a tick takes is the host's
# business, not the build's. Measured on The Stranding (55 places): placed 15 s
# after `Done (` on the creator's laptop while other builds ran. Running out of
# it is a named failure, never a pass.
DW_PLAYTEST_PLACED_BUDGET_S=1200
DW_PLAYTEST_PLACED_POLL_S=5

# dw_playtest_wait_placed <container> <budget_s> <poll_s> — poll the shared
# reading until the server says the world is placed. Prints the first reading
# and the one that settled it; on a container that stopped, or a budget that ran
# out, sets DW_VERIFY_FAILURE to the reason and returns 1.
dw_playtest_wait_placed() {
  local container="$1" budget="$2" poll="$3" waited=0 reading="" first="" state
  while :; do
    state="$(docker inspect -f '{{.State.Status}}' "$container" 2>/dev/null || true)"
    if [ "$state" = "exited" ] || [ "$state" = "dead" ]; then
      DW_VERIFY_FAILURE="the server stopped ($state) before the datapack reported the world placed (last reading: ${reading:-none})"
      return 1
    fi
    reading="$(dw_placed_reading "$container")"
    if [ -z "$first" ]; then
      first="$reading"
      echo "placement: first reading after Done, at ${waited}s: $reading"
    fi
    if [ "$reading" = "placed" ]; then
      echo "placement: the datapack reports the world placed ($DW_PLACED_HOLDER $DW_PLACED_OBJECTIVE matches $DW_PLACED_DONE) after ${waited}s of a ${budget}s budget"
      return 0
    fi
    if [ "$waited" -ge "$budget" ]; then
      DW_VERIFY_FAILURE="the datapack did not report the world placed ($DW_PLACED_HOLDER $DW_PLACED_OBJECTIVE matches $DW_PLACED_DONE) within the ${budget}s budget; last reading: $reading"
      return 1
    fi
    sleep "$poll"
    waited=$((waited + poll))
  done
}

# dw_npcs_at_world_init <npcs.json> — how many NPCs the campaign has standing in
# the world once it is placed: every declared NPC that is not `deferred`
# (a deferred one is summoned only by a `spawn-npc` effect, which needs the
# story to move, so none is owed before a player has joined).
dw_npcs_at_world_init() {
  python3 -c 'import json, sys; sys.stdout.reconfigure(newline="\n"); print(sum(1 for n in json.load(open(sys.argv[1]))["content"]["npcs"] if not n.get("deferred", False)))' "$1"
}

# dw_saved_world_npc_count <world dir> — the `dw_npc`-tagged entities the world
# save holds, read through the one anvil parse rule (tools/lib/anvil.py). Asked
# of the save and not of `@e` so the answer does not depend on which chunks
# happen to be loaded: `@e` sees only loaded chunks, `setup_finish` releases
# forceloads, and nobody is online to load any, while a save holds every entity
# once `save-all flush` has run.
dw_saved_world_npc_count() {
  python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/tools/lib/anvil.py" count-tagged-entities "$1" dw_npc
}

# dw_playtest_verify_campaign <container> <campaign dir> <world dir> <budget_s> <poll_s>
#
# Every command's response is READ (CLAUDE.md) — a world that booted is not a
# world that loaded its pack, and the difference is invisible from the outside:
# a single unparseable line drops a whole function and the server comes up empty
# and cheerful. Nothing about the world is judged before the server itself says
# the world is placed: `Done (` comes ticks before `setup` has even started the
# build, and the NPCs are summoned by `setup_finish`, its last step.
#
# Then: the pack's objectives exist, and every NPC the campaign has standing at
# world init is in the saved world — counted, against the campaign's own
# declaration. A campaign that declares none (a one-room demo level) is
# legitimate and owes none. On failure sets DW_VERIFY_FAILURE and returns 1.
dw_playtest_verify_campaign() {
  local container="$1" campaign="$2" world="$3" budget="$4" poll="$5"
  local objectives declared owed saved found
  DW_VERIFY_FAILURE=""
  dw_playtest_wait_placed "$container" "$budget" "$poll" || return 1
  if ! objectives="$(dw_rcon "$container" "scoreboard objectives list")"; then
    DW_VERIFY_FAILURE="the server refused \`scoreboard objectives list\`"; return 1
  fi
  if [[ $objectives != *"dw."* ]]; then
    DW_VERIFY_FAILURE="no dw.* objectives — datapack not loaded"; return 1
  fi
  if ! declared="$(python3 -c 'import json, sys; sys.stdout.reconfigure(newline="\n"); print(len(json.load(open(sys.argv[1]))["content"]["npcs"]))' "$campaign/npcs.json")"; then
    DW_VERIFY_FAILURE="cannot read the NPC count from $campaign/npcs.json"; return 1
  fi
  if [ "$declared" -eq 0 ]; then
    echo "npc probe: 0 NPC(s) declared in npcs.json, so no dw_npc entity is owed"
    return 0
  fi
  if ! owed="$(dw_npcs_at_world_init "$campaign/npcs.json")"; then
    DW_VERIFY_FAILURE="cannot read the deferred flags from $campaign/npcs.json"; return 1
  fi
  if ! saved="$(dw_rcon "$container" "save-all flush")"; then
    DW_VERIFY_FAILURE="the server refused \`save-all flush\`: $saved"; return 1
  fi
  if [[ $saved != *"Saved the game"* ]]; then
    DW_VERIFY_FAILURE="\`save-all flush\` did not answer \"Saved the game\": $saved"; return 1
  fi
  if ! found="$(dw_saved_world_npc_count "$world")"; then
    DW_VERIFY_FAILURE="cannot read the entities of the world save at $world"; return 1
  fi
  if [ "$found" != "$owed" ]; then
    DW_VERIFY_FAILURE="the saved world holds $found dw_npc entit(ies) where npcs.json has $owed standing at world init ($declared declared, $((declared - owed)) deferred)"
    return 1
  fi
  echo "npc probe: $found of $owed NPC(s) standing at world init are in the saved world ($declared declared, $((declared - owed)) deferred to a spawn-npc)"
}

# A seam for tests: sourcing this file with DW_PLAYTEST_SERVER_TEST_HOOK=1
# stops here, before any argument parsing or side effect, leaving every
# function above (and mutex.sh's, already sourced above) callable directly —
# with no docker, no delvec and no mutex directory the caller did not choose.
if [ "${DW_PLAYTEST_SERVER_TEST_HOOK:-}" = "1" ]; then
  return 0 2>/dev/null || exit 0
fi

cmd="${1:-}"; shift || true
case "$cmd" in up|down|status) ;; *) die "usage: up|down|status (see header)";; esac

CAMPAIGN=""
PRUNE=0
while [ $# -gt 0 ]; do
  case "$1" in
    --lang)    LANG_ARG="$2"; shift 2;;
    --prefabs) PREFABS_ARG="$2"; shift 2;;
    --delvec)  DELVEC="$2"; shift 2;;
    --name)    NAME="$2"; shift 2;;
    --out)     OUT_DIR="$2"; shift 2;;
    # Not campaign-only (unlike --lang/--prefabs/--stage-anyway below): the
    # container this bounds is the same one for a campaign and a prefab, so
    # this is accepted, and applies, on either subject.
    --memory)  MEMORY_ARG="$2"; shift 2;;
    # The deliberate override (playtest-methodology.md rule 7). Never a bare
    # flag: it needs a real reason AND the exact current red count, which moves
    # as the ledger does, so it cannot become the way this script is run.
    --stage-anyway)    STAGE_ANYWAY="$2"; shift 2;;
    --acknowledge-red) ACK_RED="$2"; shift 2;;
    # The two machine proofs the gate admits on, handed straight to it.
    --run-report)      RUN_REPORT="$2"; shift 2;;
    --written-world)   WRITTEN_WORLD="$2"; shift 2;;
    # `down` only: also remove the kept playtest images.
    --prune)           PRUNE=1; shift;;
    *) [ -z "$CAMPAIGN" ] && CAMPAIGN="$1" || die "unexpected arg: $1"; shift;;
  esac
done
if [ "$PRUNE" = 1 ] && [ "$cmd" != "down" ]; then die "--prune belongs to down"; fi

# Checked by default (tools/lib/rcon.sh, sourced above): a staging command the
# server refused must not pass for one that worked. `rcon_raw` is the unjudged
# form, for the one call whose failure is genuinely uninteresting.
DW_RCON_ARGS=(--password "$RCON_PW")
rcon() { dw_rcon "$NAME" "$1"; }
rcon_raw() { dw_rcon_probe "$NAME" "$1"; }

# ---- what a session HOLDS, and where `down` finds it -------------------------
#
# `up` holds three classes of resource, not one: the container, host 25565 (the
# mutex), and two temporary DIRECTORIES — the staged `/data` tree, which becomes a
# whole generated world, and the build output. `down` reclaimed the first two and
# nothing at all knew about the third, so every session left a world behind
# permanently: the paths came out of `mktemp -d`, so they were unguessable and
# `down` — a different shell, told only `--name` — could not have found them.
#
# A reclaimer must name EVERY class its subject holds and account for each, rather
# than the classes its author remembered. So `up` records the paths the moment it
# mints them, keyed by the one thing `down` is given, and `down` accounts for both.
#
# Not the mutex directory: that is keyed to the PORT, is shared with
# `owner-play.yaml`'s binding, and is removed on release — three reasons it is the
# wrong home for a per-session record. This is a plain file beside the directories
# it names, written before the container exists, so a session that dies during
# boot is still reclaimable by the next `down`.
case "$NAME" in
  ""|*/*|.|..) die "--name must be a plain container name, not '$NAME'";;
esac
case "$NAME" in
  *[!A-Za-z0-9_.-]*) die "--name may hold only [A-Za-z0-9_.-]: '$NAME'";;
esac
SESSION_FILE="${TMPDIR:-/tmp}/dw-playtest-session.$NAME"

# `key<TAB>path` lines, read field-wise — never sourced, because a path is data
# and `.` would execute whatever a stray edit put in there. One record per key.
session_record() {
  mkdir -p "${TMPDIR:-/tmp}"
  printf '%s\t%s\n' "$1" "$2" >>"$SESSION_FILE"
}

session_path() {
  [ -f "$SESSION_FILE" ] || return 0
  local key value
  while IFS=$'\t' read -r key value; do
    if [ "$key" = "$1" ]; then printf '%s\n' "$value"; fi
  done <"$SESSION_FILE"
}

# Size of a directory in KiB, or 0 if it is not there. `du -sk` is POSIX and reads
# the same on macOS and Linux; `du -sh` does not.
dir_kib() {
  if [ ! -d "$1" ]; then echo 0; return 0; fi
  du -sk "$1" 2>/dev/null | awk 'NR==1 {print $1+0} END {if (NR==0) print 0}'
}

human_kib() {
  awk -v k="${1:-0}" 'BEGIN {
    if (k >= 1048576) printf "%.1f GiB\n", k/1048576;
    else if (k >= 1024) printf "%.1f MiB\n", k/1024;
    else printf "%d KiB\n", k;
  }'
}

# The session's directories, every class `down` reclaims, in one list so
# `status`, `down` and `up`'s stale-record sweep cannot name different sets:
#   stage-dir  the serving container's /data — a whole world once it has run
#   place-dir  the placement boot's /data (a campaign's first `up` only)
#   image-dir  files copied out of the playtest image: its manifest, token,
#              and the resource pack the sidecar serves
DW_PLAYTEST_SESSION_DIRS="stage-dir place-dir image-dir"

# Bytes to the same units `human_kib` prints.
human_bytes() {
  human_kib "$(awk -v b="${1:-0}" 'BEGIN {printf "%d\n", b/1024}')"
}

# The kept playtest images, one per line: `<ref or id>\t<size bytes>\t<manifest>\t<campaign>`.
# Every image carrying the manifest label, dangling ones too (a rebake of the
# same build leaves the previous image untagged), so `--prune` reaches all of
# them and `status` hides none.
dw_playtest_kept_images() {
  local ids id info
  ids="$(docker images -q --no-trunc --filter "label=$PLAYTEST_LABEL.manifest-sha256" 2>/dev/null | sort -u)" || return 0
  for id in $ids; do
    info="$(docker image inspect -f "{{if .RepoTags}}{{index .RepoTags 0}}{{else}}{{.Id}}{{end}}	{{.Size}}	{{index .Config.Labels \"$PLAYTEST_LABEL.manifest-sha256\"}}	{{index .Config.Labels \"$PLAYTEST_LABEL.campaign\"}}" "$id" 2>/dev/null)" || continue
    printf '%s\n' "$info"
  done
}

if [ "$cmd" = "status" ]; then
  # The container half must not abort the disk half. An unguarded `docker ps`
  # exits 127 where docker is absent, and `set -e` then killed the script before
  # it could say what the session is still holding on disk — which is the half
  # that matters most on a machine that is not running the session. `-a`: a
  # container that stopped is still a container this session holds.
  docker ps -a --filter "name=$NAME" --format '{{.Names}}\t{{.Status}}\t{{.Ports}}' \
    || echo "docker unavailable — container state unknown" >&2
  # What the session is holding on disk, so a leak is visible without a teardown.
  if [ -f "$SESSION_FILE" ]; then
    echo "session record: $SESSION_FILE"
    for key in $DW_PLAYTEST_SESSION_DIRS build-dir; do
      path="$(session_path "$key")"
      if [ -z "$path" ]; then
        continue
      elif [ -d "$path" ]; then
        echo "  $key  $path  ($(human_kib "$(dir_kib "$path")"))"
      else
        echo "  $key  $path  (already gone)"
      fi
    done
    image="$(session_path image)"
    if [ -n "$image" ]; then echo "  image  $image"; fi
  else
    echo "session record: none at $SESSION_FILE"
  fi
  # The kept images are not this session's: any later `up` of the same inputs
  # runs one. Listed so their disk is visible; `down --prune` removes them.
  KEPT="$(dw_playtest_kept_images || true)"
  if [ -z "$KEPT" ]; then
    echo "kept playtest images: none"
  else
    echo "kept playtest images (sizes include the shared base layers; remove with: down --prune):"
    while IFS=$'\t' read -r ref size manifest campaign; do
      echo "  $ref  campaign ${campaign:-?}  manifest ${manifest:-?}  ($(human_bytes "$size"))"
    done <<<"$KEPT"
  fi
  exit 0
fi

if [ "$cmd" = "down" ]; then
  # `docker rm -f` on a name that never existed still exits 0 on modern docker —
  # that is not "I removed something", so the old one-liner printed "removed" for
  # a container that was never running. Ask first: existence is a different
  # question from removal, and `docker container inspect` is the one that
  # answers it. Where docker itself is not on PATH — this script's own test
  # fixtures run with none, and so does a host that never installed it — the
  # inspect call fails not-found (127) exactly the way `rm -f` already had to
  # tolerate, and is read the same as "no such container": the disk half below
  # must still run either way.
  if dw_playtest_container_exists "$NAME"; then
    docker rm -f "$NAME" >/dev/null 2>&1 && echo "$NAME removed" || echo "$NAME existed but could not be removed"
  else
    echo "$NAME was not running"
  fi
  # The pack sidecar is the session's third container class (spec-0084 §11):
  # named, and accounted for whether or not this session started one.
  if dw_playtest_container_exists "$NAME-pack"; then
    docker rm -f "$NAME-pack" >/dev/null 2>&1 && echo "$NAME-pack removed" || echo "$NAME-pack existed but could not be removed"
  else
    echo "no $NAME-pack sidecar to remove"
  fi
  # The placement boot exists only while a campaign's first `up` is placing its
  # world; one left here is an `up` that died mid-placement.
  if dw_playtest_container_exists "$NAME-place"; then
    docker rm -f "$NAME-place" >/dev/null 2>&1 && echo "$NAME-place removed" || echo "$NAME-place existed but could not be removed"
  fi
  # The session's directories. Each is `up`'s own — a /data tree, or files
  # copied out of an image — so nothing outside the session refers to it and
  # `down` is the moment it stops being wanted.
  RECLAIMED_ANY=0
  for key in $DW_PLAYTEST_SESSION_DIRS; do
    DIR_PATH="$(session_path "$key")"
    [ -n "$DIR_PATH" ] || continue
    RECLAIMED_ANY=1
    if [ -d "$DIR_PATH" ]; then
      DIR_KIB="$(dir_kib "$DIR_PATH")"
      rm -rf "$DIR_PATH"
      if [ "$key" = "stage-dir" ]; then
        echo "staged world reclaimed: $DIR_PATH ($(human_kib "$DIR_KIB"))"
      else
        echo "$key reclaimed: $DIR_PATH ($(human_kib "$DIR_KIB"))"
      fi
    else
      # Already gone is not a failure: a `down` after a `down`, or after a manual
      # sweep, must still release the port and finish clean.
      if [ "$key" = "stage-dir" ]; then
        echo "staged world already gone: $DIR_PATH (0 KiB reclaimed)"
      else
        echo "$key already gone: $DIR_PATH (0 KiB reclaimed)"
      fi
    fi
  done
  if [ "$RECLAIMED_ANY" = 0 ]; then
    echo "no staged world recorded at $SESSION_FILE — nothing to reclaim"
  fi
  # The build output is KEPT, and said rather than silently skipped: it holds
  # the resource pack, and its sibling `.gate` directory holds the staging-gate
  # report — which is what the session's findings are written against. The
  # report is a sibling and not a child because this gate measures the build
  # tree, and a report inside it is a file the next run counts. Both paths and
  # both sizes are printed so keeping them is a decision rather than a leak.
  BUILD_PATH="$(session_path build-dir)"
  if [ -n "$BUILD_PATH" ] && [ -d "$BUILD_PATH" ]; then
    echo "build output KEPT: $BUILD_PATH ($(human_kib "$(dir_kib "$BUILD_PATH")"))"
    echo "  it holds any resourcepack.zip; remove with: rm -rf $BUILD_PATH"
    GATE_PATH="${BUILD_PATH%/}.gate"
    if [ -d "$GATE_PATH" ]; then
      echo "staging-gate report KEPT: $GATE_PATH ($(human_kib "$(dir_kib "$GATE_PATH")"))"
      echo "  it holds staging-gate.md; remove with: rm -rf $GATE_PATH"
    fi
  fi
  rm -f "$SESSION_FILE"
  # The images. Kept by default — they are what makes the next `up` of the same
  # inputs a `docker run` — and listed with their sizes, so keeping them is a
  # decision. `--prune` removes every one; an image a running container still
  # uses is refused by docker and reported kept.
  KEPT="$(dw_playtest_kept_images || true)"
  if [ -z "$KEPT" ]; then
    echo "kept playtest images: none"
  elif [ "$PRUNE" = 1 ]; then
    # Playtest images first: each is a child of a build image, which docker
    # will not remove while the child stands.
    for pass in playtest build other; do
      while IFS=$'\t' read -r ref size manifest campaign; do
        [ -n "$ref" ] || continue
        case "$ref" in
          "$PLAYTEST_REPO:"*)       kind=playtest;;
          "$PLAYTEST_BUILD_REPO:"*) kind=build;;
          *)                        kind=other;;
        esac
        [ "$kind" = "$pass" ] || continue
        if docker rmi "$ref" >/dev/null 2>&1; then
          echo "image removed: $ref  campaign ${campaign:-?}  ($(human_bytes "$size"))"
        else
          echo "image KEPT (docker refused — a container still uses it?): $ref  ($(human_bytes "$size"))"
        fi
      done <<<"$KEPT"
    done
  else
    echo "playtest images KEPT (the next \`up\` of the same inputs runs one; remove with: down --prune):"
    while IFS=$'\t' read -r ref size manifest campaign; do
      echo "  $ref  campaign ${campaign:-?}  manifest ${manifest:-?}  ($(human_bytes "$size"))"
    done <<<"$KEPT"
  fi
  # Give 25565 back. Cross-shell by construction (`up` ran in another shell), so
  # release BY NAME — which refuses if anything still publishes the port, and
  # refuses outright if the holder is somebody else.
  dw_mutex_release_named "owner-play-session" || true
  exit 0
fi

# ---- up ---------------------------------------------------------------------
[ -n "$CAMPAIGN" ] || die "up needs a path: a campaign directory, or a prefab (a .nbt, a .json tile-set manifest, or a directory of .nbt)"
[ -e "$CAMPAIGN" ] || die "no such path: $CAMPAIGN"

# ---- which kind of thing is this? --------------------------------------------
#
# Read from the path, never asked for as a flag: a creator holding a castle
# should not have to know that this engine calls it a prefab rather than a
# campaign, and a flag they can get wrong is a flag they will get wrong.
#
# The marker for a campaign is `world.json` — the first document
# `compiler::load::load_campaign_dir` reads, and the one whose absence it already
# reports as a mistyped path. It is asked FIRST, so a campaign directory that
# also happens to hold a stray `.nbt` is still a campaign, and today's behaviour
# on every campaign is untouched.
#
# There is no third branch that guesses. A directory holding neither marker is
# refused with both shapes named, because "build it as whichever branch ran" is
# how a tool answers confidently about the wrong artifact.
SUBJECT_KIND=""
case "$CAMPAIGN" in
  *.nbt|*.json) [ -f "$CAMPAIGN" ] || die "not a file: $CAMPAIGN"; SUBJECT_KIND="prefab";;
  *)
    [ -d "$CAMPAIGN" ] || die "not a directory, and not a .nbt or .json prefab: $CAMPAIGN"
    if [ -f "$CAMPAIGN/world.json" ]; then
      SUBJECT_KIND="campaign"
    else
      # `find -maxdepth 1` rather than a glob, so a directory of 10 000 tiles
      # cannot overflow the argument list, and an unmatched glob cannot read as
      # a filename.
      FIRST_NBT="$(find "$CAMPAIGN" -maxdepth 1 -name '*.nbt' -print -quit 2>/dev/null || true)"
      [ -n "$FIRST_NBT" ] || die "$CAMPAIGN is neither a campaign nor a prefab:
  a campaign directory holds world.json (delvec build reads it first)
  a prefab is a .nbt, a .json tile-set manifest, or a directory holding .nbt
  this directory holds neither, and this script will not guess which you meant"
      SUBJECT_KIND="prefab"
    fi
    ;;
esac

# Flags that belong to one branch are REFUSED on the other rather than ignored.
# A flag silently dropped is a creator who believes it took effect — and
# `--prefabs` in particular reads as though it would point this at a prefab,
# which is the mistake this whole change exists to make impossible to make
# quietly.
if [ "$SUBJECT_KIND" = "prefab" ]; then
  [ -z "$LANG_ARG" ] || die "--lang is a campaign's translation, and $CAMPAIGN is a prefab"
  [ -z "$PREFABS_ARG" ] || die "--prefabs names the library a CAMPAIGN builds from; $CAMPAIGN is already the prefab to show — pass it as the path"
  [ -z "$STAGE_ANYWAY" ] || die "--stage-anyway overrides the staging gate, which judges a campaign; $CAMPAIGN is a prefab and the gate does not run on it (see this script's header)"
  [ -z "$ACK_RED" ] || die "--acknowledge-red belongs to --stage-anyway, which does not apply to a prefab"
  [ -z "$RUN_REPORT$WRITTEN_WORLD" ] || die "--run-report and --written-world are the staging gate's proofs of a campaign; $CAMPAIGN is a prefab and the gate does not run on it"
fi
echo "subject: $CAMPAIGN  ->  $SUBJECT_KIND"
# Capture, then test — never `cmd | grep -q`. Under `set -o pipefail` grep exits at
# the first match, `docker ps` dies of SIGPIPE (141), and pipefail promotes that to
# the pipeline: the guard silently fails to fire *because* it matched. CI keeps this
# out of the tree (tools/ci/check-shell-pipe-shortcircuit.py).
RUNNING="$(docker ps --format '{{.Names}}' || true)"
if [[ $'\n'"$RUNNING"$'\n' == *$'\n'"$NAME"$'\n'* ]]; then die "$NAME already running — 'down' first"; fi
BOUND_PORTS="$(docker ps --format '{{.Ports}}' || true)"
if [[ $BOUND_PORTS == *":25565->"* ]]; then die "host 25565 already bound by another container"; fi
if [[ $BOUND_PORTS == *":$PACK_PORT->"* ]]; then die "host $PACK_PORT (the resource-pack sidecar's port) already bound by another container"; fi
# Claim the port before building anything: if a human is already playing, this
# session must not start at all. Nothing is torn down before the lock is ours.
dw_mutex_acquire "owner-play-session" || die "another 25565 session holds the mutex"
# Hold it only while a session actually exists: a build or boot failure must give
# the port back, or the next `up` waits on a lock nothing is behind (the failure
# mode compose-project isolation removed everywhere else).
UP_OK=0
trap up_failed EXIT

# A previous session under this name that never reached `down` (a crash, or a
# boot that failed) left its record here. Reclaim it before minting new paths, or
# the record is overwritten and the old world becomes unreachable — which is the
# leak this file exists to close, one session further back.
if [ -f "$SESSION_FILE" ]; then
  echo "a previous '$NAME' session left a record — reclaiming it first"
  for key in $DW_PLAYTEST_SESSION_DIRS; do
    OLD_DIR="$(session_path "$key")"
    if [ -n "$OLD_DIR" ] && [ -d "$OLD_DIR" ]; then
      echo "  $key reclaimed: $OLD_DIR ($(human_kib "$(dir_kib "$OLD_DIR")"))"
      rm -rf "$OLD_DIR"
    fi
  done
  OLD_BUILD="$(session_path build-dir)"
  if [ -n "$OLD_BUILD" ] && [ -d "$OLD_BUILD" ]; then
    echo "  build output KEPT: $OLD_BUILD ($(human_kib "$(dir_kib "$OLD_BUILD")"))"
  fi
  rm -f "$SESSION_FILE"
fi

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# Which delvec, and why — one line, every run. `--delvec` still names one
# outright; without it, a `delvec` on PATH at the pinned engine version IS the
# toolchain the creator's Init established and is used as it stands, and
# anything else is built from source with the reason printed. The rule, and why
# the version equality is the whole guard, live in tools/lib/delvec-bin.sh.
# shellcheck source=tools/lib/delvec-bin.sh
. "$REPO_ROOT/tools/lib/delvec-bin.sh"
DELVEC="$(dw_resolve_delvec "$DELVEC" "$REPO_ROOT" "playtest-server")" || exit 1

# ---- a campaign: is its admitted image already kept? --------------------------
# The inputs key names everything the build and its admission are made from
# (dw_playtest_inputs_key, above). An image carrying it is the build these
# inputs make, already gated and placed, so nothing below the build is redone.
PLAYTEST_IMAGE=""
if [ "$SUBJECT_KIND" = "campaign" ]; then
  echo "inputs key: hashing what this build and its admission are made from"
  INPUTS_KEY="$(dw_playtest_inputs_key "$DELVEC" "$CAMPAIGN" "${PREFABS_ARG:-campaigns/prefabs}" "$LANG_ARG" "$REPO_ROOT")" \
    || die "cannot compute the inputs key (the line above names the input it could not read)"
  echo "inputs key: $INPUTS_KEY"
  FOUND="$(docker images --filter "reference=$PLAYTEST_REPO" --filter "label=$PLAYTEST_LABEL.inputs=$INPUTS_KEY" \
    --format '{{.Repository}}:{{.Tag}}' 2>/dev/null | sort -u || true)"
  FOUND_N="$(printf '%s' "$FOUND" | awk 'NF {n++} END {print n+0}')"
  if [ "$FOUND_N" -gt 1 ]; then
    die "the inputs key $INPUTS_KEY labels $FOUND_N playtest images with different manifests — the same inputs built different bytes (ADR-0006). Not choosing one:
$FOUND"
  elif [ "$FOUND_N" = 1 ]; then
    PLAYTEST_IMAGE="$FOUND"
    echo "kept image: $PLAYTEST_IMAGE carries these inputs — no build, no staging gate, no placement"
    [ -z "$OUT_DIR" ] || echo "  --out $OUT_DIR is not written: nothing is built"
    [ -z "$RUN_REPORT$WRITTEN_WORLD$STAGE_ANYWAY" ] || echo "  --run-report/--written-world/--stage-anyway are not read: this build was admitted when the image was made, and its token is checked below"
  else
    echo "kept image: none for these inputs — building, gating and placing"
  fi
fi

if [ -z "$PLAYTEST_IMAGE" ] && [ -z "$OUT_DIR" ]; then
  OUT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-out.XXXXXX")"
  # Recorded the instant it exists — an unguessable path nothing else knows about
  # is exactly as lost as an unrecorded one. A caller-supplied `--out` is NOT
  # recorded: it is the caller's directory, not this session's to account for.
  session_record build-dir "$OUT_DIR"
fi

if [ -n "$PLAYTEST_IMAGE" ]; then
  :
elif [ "$SUBJECT_KIND" = "campaign" ]; then
  BUILD_ARGS=(build "$CAMPAIGN" --out "$OUT_DIR")
  [ -n "$LANG_ARG" ]    && BUILD_ARGS+=(--lang "$LANG_ARG")
  [ -n "$PREFABS_ARG" ] && BUILD_ARGS+=(--prefabs "$PREFABS_ARG")
  echo "delvec ${BUILD_ARGS[*]}"
  # A caller-supplied `--out` that holds anything delvec did not write is
  # refused by the build itself (DW0967) and never cleared here.
  "$DELVEC" "${BUILD_ARGS[@]}" || die "build refused — the diagnostic above names the cause: the campaign, or (DW0967) an --out directory holding files no build wrote"

  # ---- the staging gate ------------------------------------------------------
  # A build compiling is not a build she should see. Every past finding's general
  # form must be a live, binding check ON THIS TREE (playtest-methodology.md rule
  # 7). This runs BETWEEN the build and the container, so a refusal costs a
  # container that never started rather than an hour of hers.
  #
  # It is an invocation, not a doc line, on purpose: the gate shipped with nothing
  # calling it, which is the UNRUN shape — a correct gate whose obligation to run
  # lived in prose. `validation/owner-play.yaml` requires the same admission for
  # the other 25565 binder.
  #
  # The report goes BESIDE the build tree, never into it. It names every ledger
  # row, so it prints the strings the ledger's own probes search that tree for —
  # and a run after it landed counted it, added a row, and moved the red count
  # between the run that prints N and the run handed `--acknowledge-red N`. The
  # gate refuses a `--report` inside `--build` outright now; this directory is
  # where the report the refusal asks for goes, and `down` prints its path.
  GATE_DIR="${OUT_DIR%/}.gate"
  mkdir -p "$GATE_DIR"
  GATE_REPORT="$GATE_DIR/staging-gate.md"
  GATE_ARGS=(--campaign "$CAMPAIGN" --build "$OUT_DIR" --report "$GATE_REPORT")
  [ -n "$RUN_REPORT" ]    && GATE_ARGS+=(--run-report "$RUN_REPORT")
  [ -n "$WRITTEN_WORLD" ] && GATE_ARGS+=(--written-world "$WRITTEN_WORLD")
  if [ -n "$STAGE_ANYWAY" ]; then
    [ -n "$ACK_RED" ] || die "--stage-anyway needs --acknowledge-red <N> (the gate prints N)"
    GATE_ARGS+=(--stage-anyway "$STAGE_ANYWAY" --acknowledge-red "$ACK_RED")
  fi
  echo "staging gate: $CAMPAIGN"
  echo "staging gate report: $GATE_REPORT"
  python3 "$REPO_ROOT/tools/creator/staging-gate.py" "${GATE_ARGS[@]}" || die \
    "staging gate REFUSED this build — not serving it (full table: $GATE_REPORT)"
else
  # ---- a prefab: the browse world --------------------------------------------
  # `prefab gallery` writes the same two things `build` writes — `datapack/` and
  # `server/server.properties` — so everything below this point is shared, and
  # there is no second staging path to keep in step with the first.
  GALLERY_ARGS=(prefab gallery "$CAMPAIGN" --out "$OUT_DIR")
  echo "delvec ${GALLERY_ARGS[*]}"
  "$DELVEC" "${GALLERY_ARGS[@]}" || die "could not build a browse world from $CAMPAIGN (the diagnostic above names the cause; DW0967 is an --out directory holding files no build wrote)"

  # Said out loud on every run, because a gate that does not run and is not
  # mentioned is indistinguishable from one that ran green. The reasoning is in
  # this script's header; this is the one-line form the operator sees.
  echo "staging gate: NOT RUN — it judges a campaign against the findings ledger,"
  echo "  and a browse world has no campaign to judge. This build is admitted by"
  echo "  what gallery emission already refused: every emitted line validated"
  echo "  against the pinned $MC_VERSION command tree, a lone tile of a tiled zone"
  echo "  refused (DW0739), and a manifest that does not tile its own zone refused."
  echo "  It is a building to look at, not a delve, and it ships to nobody."
fi

# A probe that would otherwise read as "the pack did not do X" is told the
# truth instead when the log shows why: an OOM loading structures or NPCs is
# not a content defect, and reporting it as one sends a creator chasing a
# datapack that is not broken. <container> defaults to the serving one.
die_or_oom() {
  local plain="$1" container="${2:-$NAME}" log
  log="$(docker logs "$container" 2>&1 || true)"
  if dw_server_log_shows_oom "$log"; then
    die "$plain — AND $(dw_server_oom_advice) Current: $HEAP_ENV. $LOG_HINT"
  fi
  if dw_server_log_shows_watchdog "$log"; then
    die "$plain — the vanilla watchdog halted it: one tick ran past max-tick-time (the host was too loaded to tick; the crash report is in crash-reports/ beside the log). $LOG_HINT"
  fi
  die "$plain. $LOG_HINT"
}

if [ "$SUBJECT_KIND" = "campaign" ]; then
  # The heap: the shared rule (tools/lib/server-heap.sh), read from the build's
  # own server/resources.properties — the build directory on a first `up`, the
  # copy taken out of the image on a reopen — so both boots get the value the
  # delve image's entrypoint would apply, and `--memory` overrides either.
  HEAP_ENV=""
  JAR_SHA256="$(python3 "$REPO_ROOT/tools/lib/versions.py" minecraft.server_jar_sha256)" \
    || die "cannot read [minecraft].server_jar_sha256 from versions.toml"
  RUNTIME_CACHE="$(dw_playtest_runtime_cache_dir "$MC_VERSION")"

  # ---- first `up` of these inputs: the image, and its placed world -----------
  if [ -z "$PLAYTEST_IMAGE" ]; then
    MANIFEST_SHA="$(dw_sha256_file "$OUT_DIR/manifest.json")" || die "the build wrote no manifest.json"
    [ -f "$OUT_DIR/staging-admission.json" ] || die "the staging gate passed and left no $OUT_DIR/staging-admission.json"
    ADMIT_SHA="$(dw_sha256_file "$OUT_DIR/staging-admission.json")"
    PACK_SHA1=""
    if [ -f "$OUT_DIR/resourcepack.zip" ]; then
      PACK_SHA1="$(python3 -c 'import json, sys; sys.stdout.reconfigure(newline="\n"); print(json.load(open(sys.argv[1])).get("resource_pack_sha1", ""))' "$OUT_DIR/manifest.json")" \
        || die "cannot read resource_pack_sha1 from $OUT_DIR/manifest.json"
      [ -n "$PACK_SHA1" ] || die "$OUT_DIR ships resourcepack.zip and its manifest records no resource_pack_sha1"
    fi
    LABEL_ARGS=(--label "$PLAYTEST_LABEL.inputs=$INPUTS_KEY"
                --label "$PLAYTEST_LABEL.manifest-sha256=$MANIFEST_SHA"
                --label "$PLAYTEST_LABEL.admission-sha256=$ADMIT_SHA"
                --label "$PLAYTEST_LABEL.pack-sha1=$PACK_SHA1"
                --label "$PLAYTEST_LABEL.campaign=$(basename "$CAMPAIGN")")
    BUILD_IMAGE="$(dw_playtest_image_tag "$PLAYTEST_BUILD_REPO" "$MANIFEST_SHA")"
    echo "image: the shipped delve image of this build -> $BUILD_IMAGE (validation/Dockerfile.delve)"
    docker build -q -f "$REPO_ROOT/validation/Dockerfile.delve" "${LABEL_ARGS[@]}" -t "$BUILD_IMAGE" "$OUT_DIR" >/dev/null \
      || die "docker build of the delve image refused (validation/Dockerfile.delve over $OUT_DIR)"

    HEAP_ENV="$(dw_server_heap_env "$MEMORY_ARG" "$OUT_DIR")" || die "cannot read the heap ceiling (the build's server/resources.properties, else versions.toml [server].heap_max)"
    echo "container heap (placement): $HEAP_ENV (the build's own statement; override with --memory)"
    # The placement boot: the shipped image, run once, with no host port.
    PLACE="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-place.XXXXXX")"
    session_record place-dir "$PLACE"
    LOG_HINT="placement log: $PLACE/logs/latest.log (docker-boot.log alongside it once cleanup runs)"
    CREATOR_DIR=""
    if [ -d "$OUT_DIR/creator-datapack" ]; then CREATOR_DIR="$OUT_DIR/creator-datapack"; fi
    dw_playtest_runtime_seed "$RUNTIME_CACHE" "$PLACE" "$MC_VERSION" "$JAR_SHA256"
    PLACE_ARGV=()
    while IFS= read -r dw_argv_line; do
      PLACE_ARGV+=("$dw_argv_line")
    done < <(dw_playtest_place_argv "$NAME" "$BUILD_IMAGE" "$RCON_PW" "$HEAP_ENV" "$PLACE" "$CREATOR_DIR")
    PLACE_T0=$SECONDS
    "${PLACE_ARGV[@]}" >/dev/null || die "could not start the placement boot $NAME-place"
    echo "placement: booting the shipped image to place its world (no host port)…"
    dw_playtest_wait_done "$NAME-place" 600 2 || die_or_oom "$DW_DONE_FAILURE" "$NAME-place"
    dw_playtest_verify_campaign "$NAME-place" "$CAMPAIGN" "$PLACE/world" \
      "$DW_PLAYTEST_PLACED_BUDGET_S" "$DW_PLAYTEST_PLACED_POLL_S" \
      || die_or_oom "$DW_VERIFY_FAILURE" "$NAME-place"
    # A clean stop is the server's own save: every chunk and entity written,
    # then the process exits 0. Anything else is not a world to bake.
    docker stop -t 120 "$NAME-place" >/dev/null || die "could not stop $NAME-place"
    PLACE_EXIT="$(docker inspect -f '{{.State.ExitCode}}' "$NAME-place" 2>/dev/null || echo '?')"
    docker logs "$NAME-place" >"$PLACE/docker-boot.log" 2>&1 || true
    [ "$PLACE_EXIT" = 0 ] || die "the placement boot exited $PLACE_EXIT on stop, not 0 — its world is not baked. $LOG_HINT"
    docker rm "$NAME-place" >/dev/null || die "could not remove $NAME-place"
    echo "placement: placed, probed, saved and stopped in $((SECONDS - PLACE_T0))s"
    dw_playtest_runtime_keep "$PLACE" "$RUNTIME_CACHE" "$MC_VERSION" "$JAR_SHA256"

    # The bake context, inside the place-dir so `down` reclaims it either way.
    # The world moves (no copy); its datapacks stay behind because every boot
    # copies them in from /delve, and so does the lock of a server now stopped.
    BAKE="$PLACE/bake"
    BAKE_PACK=""
    if [ -n "$PACK_SHA1" ]; then BAKE_PACK="$OUT_DIR/resourcepack.zip"; fi
    BAKE_DATAPACKS="$(dw_playtest_bake_context "$PLACE/world" "$BAKE" "$OUT_DIR/staging-admission.json" "$CREATOR_DIR" "$BAKE_PACK")" \
      || die "cannot assemble the bake context at $BAKE"
    PLAYTEST_IMAGE="$(dw_playtest_image_tag "$PLAYTEST_REPO" "$MANIFEST_SHA")"
    echo "image: the placed world ($(human_kib "$(dir_kib "$BAKE/placed-world")")) -> $PLAYTEST_IMAGE (validation/Dockerfile.playtest)"
    docker build -q -f "$REPO_ROOT/validation/Dockerfile.playtest" \
      --build-arg "PLAYTEST_BUILD_IMAGE=$BUILD_IMAGE" --build-arg "DATAPACKS=$BAKE_DATAPACKS" \
      "${LABEL_ARGS[@]}" -t "$PLAYTEST_IMAGE" "$BAKE" >/dev/null \
      || die "docker build of the playtest image refused (validation/Dockerfile.playtest over $BAKE)"
    rm -rf "$PLACE"
    echo "image: kept $PLAYTEST_IMAGE — the next \`up\` of these inputs runs it"
  fi

  # ---- the reopen proves it is serving the admitted build ---------------------
  # Copied out of the image itself, never read from the build directory: the
  # bytes that serve are the bytes judged. Held to the image's labels, to each
  # other, and — on a first `up` — to the build this run just gated.
  SERVE_T0=$SECONDS
  IMAGE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-image.XXXXXX")"
  session_record image-dir "$IMAGE_DIR"
  session_record image "$PLAYTEST_IMAGE"
  L_MANIFEST="$(docker image inspect -f "{{index .Config.Labels \"$PLAYTEST_LABEL.manifest-sha256\"}}" "$PLAYTEST_IMAGE")" \
    || die "cannot inspect $PLAYTEST_IMAGE"
  L_ADMIT="$(docker image inspect -f "{{index .Config.Labels \"$PLAYTEST_LABEL.admission-sha256\"}}" "$PLAYTEST_IMAGE")"
  L_PACK="$(docker image inspect -f "{{index .Config.Labels \"$PLAYTEST_LABEL.pack-sha1\"}}" "$PLAYTEST_IMAGE")"
  # Created to copy from, never started: the entrypoint named is the one no
  # server can come from.
  CID="$(docker create --entrypoint /bin/true "$PLAYTEST_IMAGE")" || die "cannot create a container from $PLAYTEST_IMAGE to read its files"
  COPY_OK=1
  mkdir -p "$IMAGE_DIR/server"
  docker cp "$CID:/delve/server/resources.properties" "$IMAGE_DIR/server/resources.properties" >/dev/null 2>&1 || true
  docker cp "$CID:/delve/manifest.json" "$IMAGE_DIR/manifest.json" >/dev/null 2>&1 || COPY_OK=0
  docker cp "$CID:/delve/staging-admission.json" "$IMAGE_DIR/staging-admission.json" >/dev/null 2>&1 || COPY_OK=0
  docker cp "$CID:/delve/server/server.properties" "$IMAGE_DIR/server.properties" >/dev/null 2>&1 || COPY_OK=0
  if [ -n "$L_PACK" ]; then
    docker cp "$CID:/delve/extras/resourcepack.zip" "$IMAGE_DIR/resourcepack.zip" >/dev/null 2>&1 || COPY_OK=0
  fi
  docker rm "$CID" >/dev/null 2>&1 || true
  [ "$COPY_OK" = 1 ] || echo "identity: a file could not be copied out of $PLAYTEST_IMAGE — judged as missing below" >&2
  IMAGE_TAG_MANIFEST="${PLAYTEST_IMAGE##*:}"
  dw_playtest_identity_check "$IMAGE_DIR" "$IMAGE_TAG_MANIFEST" "$L_MANIFEST" "$L_ADMIT" \
    || die "REFUSING to serve $PLAYTEST_IMAGE: $DW_IDENTITY_FAILURE. Remove it with: tools/creator/playtest-server.sh down --name $NAME --prune"
  if [ -n "${MANIFEST_SHA:-}" ] && [ "$MANIFEST_SHA" != "$L_MANIFEST" ]; then
    die "REFUSING to serve $PLAYTEST_IMAGE: it carries manifest $L_MANIFEST and this run gated $MANIFEST_SHA"
  fi
  session_record manifest-sha256 "$L_MANIFEST"
  session_record admission-sha256 "$L_ADMIT"
  echo "identity: $PLAYTEST_IMAGE serves manifest $L_MANIFEST, admitted by token $L_ADMIT"

  # ---- the delve's resource pack: served, never installed (spec-0084 §11) ----
  PACK_NOTE="no resource pack in this build"
  PACK_URL=""
  if [ -n "${DELVEWRIGHT_RESOURCEPACKS_DIR:-}" ]; then
    echo "resource pack: DELVEWRIGHT_RESOURCEPACKS_DIR is set and is NOT written — a delve's pack is served by the server, never installed into a player's own folder (spec-0084 §11)"
  fi
  if [ -n "$L_PACK" ]; then
    M_PACK="$(python3 -c 'import json, sys; sys.stdout.reconfigure(newline="\n"); print(json.load(open(sys.argv[1])).get("resource_pack_sha1", ""))' "$IMAGE_DIR/manifest.json")" \
      || die "cannot read resource_pack_sha1 from the image's manifest"
    [ "$M_PACK" = "$L_PACK" ] || die "REFUSING to serve $PLAYTEST_IMAGE: its pack label says $L_PACK and its manifest $M_PACK"
    PACK_URL="http://127.0.0.1:$PACK_PORT/resourcepack.zip"
    PACK_ARGV=()
    while IFS= read -r dw_argv_line; do
      PACK_ARGV+=("$dw_argv_line")
    done < <(dw_playtest_pack_run_argv "$NAME" "$PACK_IMAGE" "$PACK_PORT" "$IMAGE_DIR")
    "${PACK_ARGV[@]}" >/dev/null || die "could not start the resource-pack sidecar $NAME-pack"
    # The response is READ: the bytes a client will download, hashed, and held to
    # the manifest's SHA-1 — which is the hash the client checks them against.
    SERVED_SHA1=""
    for _ in $(seq 1 40); do
      SERVED_SHA1="$(python3 -c 'import hashlib, sys, urllib.request; sys.stdout.reconfigure(newline="\n"); print(hashlib.sha1(urllib.request.urlopen(sys.argv[1], timeout=5).read()).hexdigest())' "$PACK_URL" 2>/dev/null || true)"
      [ -n "$SERVED_SHA1" ] && break
      sleep 0.5
    done
    [ "$SERVED_SHA1" = "$M_PACK" ] || die "the pack sidecar serves sha1 '${SERVED_SHA1:-nothing}' at $PACK_URL, and the manifest records $M_PACK — a client would refuse the pack"
    PACK_NOTE="resource pack: served at $PACK_URL (sha1 $M_PACK) — accept the prompt when you join"
    if grep -q '^require-resource-pack=true$' "$IMAGE_DIR/server.properties"; then
      PACK_NOTE="$PACK_NOTE; this delve REQUIRES it (declining disconnects)"
    fi
  fi

  # ---- the serving run --------------------------------------------------------
  HEAP_ENV="$(dw_server_heap_env "$MEMORY_ARG" "$IMAGE_DIR")" || die "cannot read the heap ceiling (the image's server/resources.properties, else versions.toml [server].heap_max)"
  echo "container heap: $HEAP_ENV (the build's own statement, read from the image; override with --memory)"
  STAGE="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-data.XXXXXX")"
  session_record stage-dir "$STAGE"
  LOG_HINT="server log: $STAGE/logs/latest.log (docker-boot.log alongside it once cleanup runs)"
  dw_playtest_runtime_seed "$RUNTIME_CACHE" "$STAGE" "$MC_VERSION" "$JAR_SHA256"
  SERVE_ARGV=()
  while IFS= read -r dw_argv_line; do
    SERVE_ARGV+=("$dw_argv_line")
  done < <(dw_playtest_serve_argv "$NAME" "$PLAYTEST_IMAGE" "$RCON_PW" "$HEAP_ENV" "$STAGE" "$PACK_URL")
  RUN_T0=$SECONDS
  "${SERVE_ARGV[@]}" >/dev/null || die "could not start $NAME from $PLAYTEST_IMAGE"
  echo "waiting for the server…"
  dw_playtest_wait_done "$NAME" 600 1 || die_or_oom "$DW_DONE_FAILURE"
  DONE_S=$((SECONDS - RUN_T0))
  # The same readiness the first boot was judged by: the datapack reports the
  # world placed, its objectives exist, and every NPC standing at world init is
  # in the save. On a world the image already holds placed, the first reading
  # answers `placed`.
  dw_playtest_verify_campaign "$NAME" "$CAMPAIGN" "$STAGE/world" \
    "$DW_PLAYTEST_PLACED_BUDGET_S" 1 \
    || die_or_oom "$DW_VERIFY_FAILURE"
  READY_S=$((SECONDS - RUN_T0))

  UP_OK=1   # the session exists; the 25565 mutex stays held until `down`
  echo
  echo "READY — Multiplayer -> Direct Connect: localhost:25565"
  echo "joinable ${SECONDS}s after the command: $((SERVE_T0)) s to the image, $((RUN_T0 - SERVE_T0)) s identity + pack, ${DONE_S}s boot to \`Done (\`, $((READY_S - DONE_S))s placed + probes"
  echo "$PACK_NOTE"
  echo "teardown: tools/creator/playtest-server.sh down --name $NAME  (also removes $NAME-pack and frees the 25565 mutex; keeps $PLAYTEST_IMAGE)"
  exit 0
fi

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-data.XXXXXX")"
# Recorded before anything is copied into it, and before the container that will
# generate a whole world inside it exists — so `down`, which is a different shell
# given nothing but `--name`, can reclaim it however this session ends.
session_record stage-dir "$STAGE"
mkdir -p "$STAGE/world/datapacks"
cp "$OUT_DIR/server/server.properties" "$STAGE/server.properties"
printf 'enable-rcon=true\nrcon.password=%s\nrcon.port=25575\n' "$RCON_PW" >> "$STAGE/server.properties"

# ---- the delve's resource pack: served, never installed (spec-0084 §11) ------
PACK_NOTE="no resource pack in this build"
if [ -n "${DELVEWRIGHT_RESOURCEPACKS_DIR:-}" ]; then
  echo "resource pack: DELVEWRIGHT_RESOURCEPACKS_DIR is set and is NOT written — a delve's pack is served by the server, never installed into a player's own folder (spec-0084 §11)"
fi
if [ -f "$OUT_DIR/resourcepack.zip" ]; then
  PACK_SHA1="$(python3 -c 'import json, sys; sys.stdout.reconfigure(newline="\n"); print(json.load(open(sys.argv[1])).get("resource_pack_sha1", ""))' "$OUT_DIR/manifest.json")" \
    || die "cannot read resource_pack_sha1 from $OUT_DIR/manifest.json"
  [ -n "$PACK_SHA1" ] || die "$OUT_DIR ships resourcepack.zip and its manifest records no resource_pack_sha1"
  PACK_URL="http://127.0.0.1:$PACK_PORT/resourcepack.zip"
  PACK_ARGV=()
  while IFS= read -r dw_argv_line; do
    PACK_ARGV+=("$dw_argv_line")
  done < <(dw_playtest_pack_run_argv "$NAME" "$PACK_IMAGE" "$PACK_PORT" "$OUT_DIR")
  "${PACK_ARGV[@]}" >/dev/null || die "could not start the resource-pack sidecar $NAME-pack"
  # The response is READ: the bytes a client will download, hashed, and held to
  # the manifest's SHA-1 — which is the hash the client checks them against.
  SERVED_SHA1=""
  for _ in $(seq 1 20); do
    SERVED_SHA1="$(python3 -c 'import hashlib, sys, urllib.request; sys.stdout.reconfigure(newline="\n"); print(hashlib.sha1(urllib.request.urlopen(sys.argv[1], timeout=5).read()).hexdigest())' "$PACK_URL" 2>/dev/null || true)"
    [ -n "$SERVED_SHA1" ] && break
    sleep 1
  done
  [ "$SERVED_SHA1" = "$PACK_SHA1" ] || die "the pack sidecar serves sha1 '${SERVED_SHA1:-nothing}' at $PACK_URL, and the manifest records $PACK_SHA1 — a client would refuse the pack"
  dw_playtest_pack_properties "$PACK_URL" "$PACK_SHA1" >> "$STAGE/server.properties"
  PACK_NOTE="resource pack: served at $PACK_URL (sha1 $PACK_SHA1) — accept the prompt when you join"
  if grep -q '^require-resource-pack=true$' "$STAGE/server.properties"; then
    PACK_NOTE="$PACK_NOTE; this delve REQUIRES it (declining disconnects)"
  fi
fi
# The datapack's directory name. A campaign is a directory, so its basename is
# already a name; a prefab may arrive as `the-castle.json`, whose basename would
# put a `.json` in a datapack directory name. Strip the extension and nothing
# else, so a campaign's id is byte-identical to what it always was.
CAMP_ID="$(basename "$CAMPAIGN")"
if [ "$SUBJECT_KIND" = "prefab" ]; then CAMP_ID="${CAMP_ID%.*}"; fi
cp -R "$OUT_DIR/datapack" "$STAGE/world/datapacks/$CAMP_ID"
# The creator overlay too, when the build emitted one: it is what carries
# `/trigger dw.note`, and the walk this server exists for is the walk whose
# findings that trigger stamps. A server that serves the delve and not the
# overlay tells the creator to mark a finding with a command that is not there.
if [[ -d "$OUT_DIR/creator-datapack" ]]; then
  cp -R "$OUT_DIR/creator-datapack" "$STAGE/world/datapacks/$CAMP_ID-creator"
fi

HEAP_ENV="$(dw_server_heap_env "$MEMORY_ARG" "$OUT_DIR")" || die "cannot read the heap ceiling (the build's server/resources.properties, else versions.toml [server].heap_max)"
echo "container heap: $HEAP_ENV (the build's own statement for its view distance; itzg's own ceiling is 1G; override with --memory)"
# Not `mapfile`/`readarray`: bash 3.2 (macOS's shipped /bin/bash — the creator's
# own machine, CLAUDE.md) does not have them, and silently leaves the array
# empty rather than failing (tools/ci/check-shell-bash32.py).
DOCKER_RUN_ARGV=()
while IFS= read -r dw_argv_line; do
  DOCKER_RUN_ARGV+=("$dw_argv_line")
done < <(dw_playtest_docker_run_argv "$NAME" "$MC_VERSION" "$RCON_PW" "$HEAP_ENV" "$STAGE")
"${DOCKER_RUN_ARGV[@]}" >/dev/null

# Where every log this session can offer lives, stated once so every die()
# below can point at it — including after `up_failed` has already removed the
# container that a bare `docker logs $NAME` would otherwise need. The server's
# own log is the bind-mounted world directory, so it survives container
# removal; the wrapper's stdout does not, which is why `up_failed` captures it
# into the same directory first.
LOG_HINT="server log: $STAGE/logs/latest.log (docker-boot.log alongside it once cleanup runs)"

echo "waiting for world generation…"
READY=0
for _ in $(seq 1 60); do
  sleep 10
  BOOT_LOG="$(docker logs "$NAME" 2>&1 || true)"
  if [[ $BOOT_LOG == *"Done ("* ]]; then READY=1; break; fi
  if dw_server_log_shows_oom "$BOOT_LOG"; then
    die "booting: $(dw_server_oom_advice) Current: $HEAP_ENV. $LOG_HINT"
  fi
done
[ "$READY" = 1 ] || die "server did not come up after 600s. $LOG_HINT"

# rcon verification (dw_playtest_verify_campaign, above). What is asked differs
# by kind because what each kind CLAIMS differs. Both assertions are of the same
# strength: the pack's own objectives exist, and the thing the pack was supposed
# to put in the world is in it. Display slots are the campaign's: nothing here
# sets or clears one (tools/tests/test_playtest_server_leaves_the_display_slots.py).
if [ "$SUBJECT_KIND" = "campaign" ]; then
  dw_playtest_verify_campaign "$NAME" "$CAMPAIGN" "$STAGE/world" \
    "$DW_PLAYTEST_PLACED_BUDGET_S" "$DW_PLAYTEST_PLACED_POLL_S" \
    || die_or_oom "$DW_VERIFY_FAILURE"
else
  # `admit.sys` is created by `admit:load`, so its absence is a pack the server
  # dropped rather than a world that is merely slow.
  OBJECTIVES="$(rcon "scoreboard objectives list")"
  [[ $OBJECTIVES == *"admit.sys"* ]] || die_or_oom "no admit.sys objective — the gallery datapack did not load"
  # One label per exhibit, COUNTED against the layout the same build wrote.
  # `execute if entity` answers only pass/fail, and a pass over one label of nine
  # exhibits is the vacuity this project keeps paying for — so the count is
  # summed into a scoreboard and read back, and it must EQUAL the denominator.
  EXPECTED_EXHIBITS="$(python3 -c 'import json,sys; sys.stdout.reconfigure(newline="\n"); print(len(json.load(open(sys.argv[1]))["areas"]))' "$OUT_DIR/gallery-layout.json")"
  # `admit:finish` summons the labels at tick 6 and `admit:place` runs at tick 3,
  # so a label existing proves the placement function was reached. The forceloaded
  # chunks take a few ticks to arrive, so this waits rather than asking once.
  LABELS=0
  for _ in $(seq 1 30); do
    rcon "scoreboard players set #dwlabels admit.sys 0" >/dev/null
    rcon "execute as @e[tag=admit_label] run scoreboard players add #dwlabels admit.sys 1" >/dev/null
    LABEL_READ="$(rcon "scoreboard players get #dwlabels admit.sys")"
    LABELS="$(printf '%s' "$LABEL_READ" | awk '{for(i=1;i<=NF;i++) if ($i ~ /^[0-9]+$/) {print $i; exit}}')"
    LABELS="${LABELS:-0}"
    [ "$LABELS" -ge "$EXPECTED_EXHIBITS" ] && break
    sleep 2
  done
  [ "$LABELS" = "$EXPECTED_EXHIBITS" ] || die_or_oom \
    "the browse world has $LABELS labelled exhibit(s) where the layout declares $EXPECTED_EXHIBITS — the datapack did not finish placing"
  echo "browse world binding: $LABELS of $EXPECTED_EXHIBITS exhibit(s) placed and labelled"
fi

UP_OK=1   # the session exists; the 25565 mutex stays held until `down`
echo
echo "READY — Multiplayer -> Direct Connect: localhost:25565"
if [ "$SUBJECT_KIND" = "prefab" ]; then
  echo "creative, flight on; you spawn on a stone platform beside the building"
fi
echo "$PACK_NOTE"
echo "teardown: tools/creator/playtest-server.sh down --name $NAME  (also removes $NAME-pack and frees the 25565 mutex)"
