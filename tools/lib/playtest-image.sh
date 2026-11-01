#!/usr/bin/env bash
# What makes a playtest image's bytes, sourced by
# `tools/creator/playtest-server.sh`: the image names and labels, the inputs
# key, the campaign build and staging gate, the placement boot and the bake.
# The inputs key hashes THIS file (DW_PLAYTEST_INPUT_FILES below), so an edit
# here makes every kept image a miss and the next `up` builds afresh; code that
# only serves an image lives in playtest-server.sh and stays out of the key.
#
# The two flow functions run inside `up` and read its state (NAME, CAMPAIGN,
# OUT_DIR, DELVEC, REPO_ROOT, the flags) and its helpers (`die`, `die_or_oom`,
# `session_record`, the readiness probes, the runtime cache).

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

# The engine files an admitted image's bytes depend on beyond the build itself
# (the delvec binary, the campaign and prefab trees, `--lang`, hashed below):
#   docs/playtest-findings.json     the ledger the staging gate judges against;
#                                   the admission token it mints is baked
#   tools/creator/staging-gate.py   the gate that mints that token
#   validation/Dockerfile.delve     the shipped delve image, and its entrypoint,
#                                   which boots the placement
#   validation/Dockerfile.playtest  the playtest image over it
#   validation/staging-admission.sh the verifier the token is held to; the
#                                   admission a kept image claims is its answer
#   tools/lib/playtest-image.sh     this file: the names and labels, this key,
#                                   the build and gate invocation, the placement
#                                   boot and the bake
# `tools/creator/playtest-server.sh` is NOT here: what it holds only serves an
# image (the serving argv, the readiness probes, the checkpoints, status,
# down), and an edit to it must not throw every kept image away.
DW_PLAYTEST_INPUT_FILES="docs/playtest-findings.json tools/creator/staging-gate.py validation/Dockerfile.delve validation/Dockerfile.playtest validation/staging-admission.sh tools/lib/playtest-image.sh"

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

# dw_playtest_build_and_gate — a campaign's `delvec build` into OUT_DIR, then
# the staging gate against that exact tree.
dw_playtest_build_and_gate() {
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
    GATE_ARGS+=(--stage-anyway "$STAGE_ANYWAY")
    [ -z "$ACK_RED" ] || GATE_ARGS+=(--acknowledge-red "$ACK_RED")
  fi
  echo "staging gate: $CAMPAIGN"
  echo "staging gate report: $GATE_REPORT"
  python3 "$REPO_ROOT/tools/creator/staging-gate.py" "${GATE_ARGS[@]}" || die \
    "staging gate REFUSED this build — not serving it (full table: $GATE_REPORT)"
}

# dw_playtest_make_image — the first `up` of these inputs: the shipped delve
# image of the build, its placement boot, and the playtest image baked over the
# placed world. Sets PLAYTEST_IMAGE, MANIFEST_SHA and HEAP_ENV.
dw_playtest_make_image() {
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
}

# dw_playtest_place_failed — the placement boot of an `up` that failed: its
# wrapper log captured beside its world in the session's place-dir (which
# `down` reclaims), then the container removed. Called by `up_failed`.
dw_playtest_place_failed() {
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
}
