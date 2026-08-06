#!/usr/bin/env bash
# ONE live Mojang fetch per JOB, not one per server BOOT (task #41).
#
#   validation/server-bootstrap-cache.sh [--cache <dir>] [--force]
#
# ## The failure this removes
#
# The Mojang server jar is never baked into any image (ADR-0010, EULA), so every
# server container bootstraps it live at first boot. `tier 2` boots SEVEN servers
# over seven fresh `/data` volumes — the datapack-load check (VANILLA) plus six
# PackTest suites (FABRIC), each in its own compose project — so a `tier 2` run
# used to depend on seven independent live bootstraps, and any ONE of them failing
# reds a required status check. PR #312 died exactly there: the 5th of 6 suites hit
# `Downloading Minecraft server` -> `launchermeta.mojang.com ... Network is
# unreachable` before a single datapack was evaluated, on the same runner and image
# digest on which four suites had already passed.
#
# The problem was never speed (the runner pulls piston-data at >100 MB/s). It is the
# NUMBER OF INDEPENDENT CHANCES TO FAIL. This script collapses them to one.
#
# ## What it produces
#
# A cache tree that is a drop-in overlay for a server's `/data`, holding exactly the
# bootstrap state whose absence forces a network fetch:
#
#   <cache>/minecraft_server.<ver>.jar        VANILLA: the pinned Mojang jar
#   <cache>/.vanilla-manifest.json            VANILLA: itzg's "already installed" marker
#   <cache>/.fabric/server/<ver>-server.jar   FABRIC: the same jar, where Fabric looks
#   <cache>/.fabric/server/fabric-loader-server-<loader>-minecraft-<ver>.jar
#   <cache>/libraries/**                      FABRIC: the launch jar's Class-Path
#   <cache>/.dw-bootstrap.json                marker: version + sha256 + binding counts
#
# Callers COPY this tree into `/data` before the server starts — a copy, never a
# read-only bind mount. That is the whole difference from the jar cache removed on
# 2026-07-30 (see the note in ci.yml): itzg's installer replaces the jar by RENAME,
# and a rename cannot target a bind-mounted file ("Device or resource busy"). A copy
# is writable, so the rename path stays available and nothing is pinned open.
#
# ## Why single-fetch AND checksum, not either alone
#
# A single fetch removes six of the seven chances to fail. It does NOT make the
# remaining one honest: a truncated or half-written jar produces a server that dies
# for a reason no reader would connect to the network. Verifying `server_jar_sha256`
# from versions.toml turns that into a named red at the bootstrap step. Conversely a
# checksum alone leaves seven fetches. Together: one chance to fail, and when it
# fails it says so.
#
# Fetching the jar OURSELVES (rather than letting Fabric's installer do it) is also
# what shrinks the Fabric warm-up's own Mojang exposure: with a valid jar already in
# place Fabric prints "Existing server jar valid, not downloading" and only reads the
# small version manifest.
#
# ## When Mojang is genuinely unreachable for the whole run
#
# Every retry is scoped to the bootstrap fetch ALONE and is bounded. When they are
# exhausted this script exits non-zero with a `::error::` naming the host that could
# not be reached, BEFORE any server boots and before any datapack is evaluated. A
# reader never has to guess whether the network or the delve failed: a network
# outage reds a step named "bootstrap", a datapack failure reds a step named for the
# datapack. The gate keeps its teeth — nothing here can turn a real failure green.
#
# Idempotent: a cache whose marker matches the pinned version and sha256 performs NO
# fetch at all, so a second caller in the same job is free.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$here/.." && pwd)"
MANIFEST="$ROOT/versions.toml"

cache="$here/server-cache"
force=0
usage() {
  cat >&2 <<'USAGE'
usage: validation/server-bootstrap-cache.sh [--cache <dir>] [--force]

  --cache  Where the bootstrap overlay lives (default validation/server-cache).
  --force  Rebuild even if the marker already matches the pinned version+sha256.
USAGE
  exit 2
}
while [ $# -gt 0 ]; do
  case "$1" in
    --cache|-c) [ $# -ge 2 ] || usage; cache="$2"; shift 2 ;;
    --force)    force=1; shift ;;
    -h|--help)  usage ;;
    *) echo "server-bootstrap-cache: unknown argument '$1'" >&2; usage ;;
  esac
done

[ -f "$MANIFEST" ] || { echo "FATAL: $MANIFEST not found" >&2; exit 2; }

# versions.toml is the single source of truth (spec-0005). Nothing below hardcodes a
# version, URL or checksum — validation/check-versions.sh asserts that, so this
# script cannot drift from the manifest the rest of the stack is pinned to.
eval "$(python3 - "$MANIFEST" <<'PY'
import sys, tomllib
d = tomllib.load(open(sys.argv[1], "rb"))
def emit(k, v): print(f'{k}={v!r}'.replace("'", '"'))
emit("MC_VERSION",     d["minecraft"]["version"])
emit("JAR_URL",        d["minecraft"]["server_jar_url"])
emit("JAR_SHA256",     d["minecraft"]["server_jar_sha256"])
emit("JAR_SIZE",       d["minecraft"]["server_jar_size"])
emit("FABRIC_LOADER",  d["fabric"]["loader_version"])
emit("FABRIC_LAUNCHER", d["fabric"]["launcher_version"])
emit("TOOL_IMAGE",     d["images"]["toolserver"]["repo"] + "@" + d["images"]["toolserver"]["digest"])
PY
)"

jar_name="minecraft_server.${MC_VERSION}.jar"
marker="$cache/.dw-bootstrap.json"

sha256_of() { # <file> -> bare hex
  if command -v sha256sum >/dev/null 2>&1; then
    set -- "$(sha256sum "$1")"; echo "${1%% *}"
  else
    set -- "$(shasum -a 256 "$1")"; echo "${1%% *}"
  fi
}

# ---------------------------------------------------------------- already warm --
# The marker is only trusted together with the bytes it describes: it names the
# version and sha256, and the jar it points at must still hash to that. A marker
# that outlives its jar is a stale cache, not a warm one.
if [ "$force" -eq 0 ] && [ -f "$marker" ] && [ -f "$cache/$jar_name" ]; then
  have="$(sha256_of "$cache/$jar_name")"
  if [ "$have" = "$JAR_SHA256" ] && grep -qF "\"minecraft_version\": \"$MC_VERSION\"" "$marker"; then
    echo "==> bootstrap cache already warm at $cache (no fetch: $MC_VERSION, sha256 verified)"
    exit 0
  fi
  echo "==> bootstrap cache at $cache is STALE (jar sha256 or version moved) — rebuilding"
fi

mkdir -p "$cache"
rm -f "$marker"

# ------------------------------------------------------------- 1. the Mojang jar --
# The ONE live Mojang fetch of the whole job. Bounded retry, scoped to this fetch
# and nothing else: it can turn a blip into a pass, and it can never turn a server
# that genuinely will not start into a pass, because it only ever retries the
# download of a file whose bytes are then checked against the pin.
fetch_jar() {
  local dest="$1" attempt=0 max=3 delay=5 rc got
  while :; do
    attempt=$((attempt + 1))
    rm -f "$dest.part"
    if curl -fSL --no-progress-meter --connect-timeout 20 --max-time 900 \
         --retry 0 -o "$dest.part" "$JAR_URL"; then
      got="$(sha256_of "$dest.part")"
      if [ "$got" = "$JAR_SHA256" ]; then
        mv -f "$dest.part" "$dest"
        echo "    fetched and verified ($JAR_SIZE bytes, sha256 $JAR_SHA256)"
        return 0
      fi
      echo "    attempt $attempt: sha256 MISMATCH (got $got, pinned $JAR_SHA256) — truncated or wrong object" >&2
    else
      rc=$?
      echo "    attempt $attempt: curl failed (exit $rc) fetching $JAR_URL" >&2
    fi
    rm -f "$dest.part"
    if [ "$attempt" -ge "$max" ]; then return 1; fi
    echo "    retrying the BOOTSTRAP FETCH in ${delay}s ($attempt/$max used)" >&2
    sleep "$delay"
    delay=$((delay * 3))
  done
}

echo "==> bootstrap fetch 1/1: the pinned Minecraft server jar ($MC_VERSION)"
if ! fetch_jar "$cache/$jar_name"; then
  echo "::error::BOOTSTRAP FETCH FAILED — could not obtain the pinned Minecraft server jar." >&2
  echo "  url:  $JAR_URL" >&2
  echo "  This is the NETWORK (or Mojang), not the datapack: no server has booted and" >&2
  echo "  no test has run yet. The jar is never baked into an image (ADR-0010, EULA), so" >&2
  echo "  a run cannot proceed without it. Nothing about this red implicates the delve." >&2
  exit 1
fi

# itzg's `install-vanilla` skips the download entirely when this manifest and the
# files it names are present (measured: it prints "Minecraft version <v> is already
# installed" and exits 0 under `--network none`). The timestamp is fixed rather than
# wall-clock — the compiler's determinism rule (ADR-0006) is about build output, but
# a cache artifact with a clock in it is a needless second source of variation.
cat > "$cache/.vanilla-manifest.json" <<JSON
{"@type":"me.itzg.helpers.vanilla.VanillaManifest","timestamp":"1970-01-01T00:00:00Z","files":["$jar_name"],"minecraftVersion":"$MC_VERSION","serverEntry":"$jar_name"}
JSON

# ------------------------------------------------------ 2. the Fabric bootstrap --
# The PackTest runner is a FABRIC server, and Fabric's own launcher — not itzg —
# bootstraps it: `ServerLauncher` returns early ONLY when the generated launch jar
# exists and every entry of its manifest `Class-Path` is present. Anything less and
# it re-installs, which reads launchermeta.mojang.com, the Mojang jar, and the loader
# libraries from maven.fabricmc.net. Six suites meant six of those.
#
# So the launch jar and its Class-Path are generated ONCE here, in a throwaway
# container of the pinned toolserver image, with the already-verified jar put where
# Fabric expects it (Fabric then reports "Existing server jar valid, not
# downloading" and never re-fetches 57 MB).
#
# The collection is by Class-Path, not by a hand-written file list: Fabric's own
# completeness predicate is "every Class-Path entry present", so seeding exactly
# that set is both minimal and self-updating if the loader's dependencies move.
fabric_warm() {
  local attempt=0 max=3 delay=5
  while :; do
    attempt=$((attempt + 1))
    rm -rf "$cache/.fabric" "$cache/libraries"
    if docker run --rm \
        -e MC_VERSION="$MC_VERSION" \
        -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
        -v "$cache:/out" \
        --entrypoint /bin/bash "$TOOL_IMAGE" -c '
set -euo pipefail
launcher="$(. /data/.install-fabric.env; echo "$SERVER")"
[ -f "$launcher" ] || { echo "toolserver image has no pre-baked Fabric launcher" >&2; exit 1; }

mkdir -p /warm/.fabric/server
cp "/out/minecraft_server.${MC_VERSION}.jar" "/warm/.fabric/server/${MC_VERSION}-server.jar"
cd /warm
# The launcher installs, then hands off to the Minecraft server, which stops at the
# EULA it was never given. We do not trust its exit code for that reason: what is
# asserted below is the ARTEFACTS, which is the same predicate Fabric itself uses.
java -jar "$launcher" nogui || true

launch="$(ls /warm/.fabric/server/fabric-loader-server-*.jar 2>/dev/null || true)"
[ -n "$launch" ] && [ -f "$launch" ] || {
  echo "Fabric produced no server launch jar — the loader bootstrap did not complete" >&2
  exit 1
}
# Unfold the manifest continuation lines (a wrapped header continues on a line
# starting with one space), then read Class-Path. No early-exiting consumer on the
# right of a pipe anywhere here (task #173).
cp_line="$(unzip -p "$launch" META-INF/MANIFEST.MF \
  | tr -d "\r" \
  | sed -e ":a" -e "N" -e "\$!ba" -e "s/\n //g" \
  | sed -n "s/^Class-Path: //p")"
[ -n "$cp_line" ] || { echo "Fabric launch jar has no Class-Path — nothing to seed" >&2; exit 1; }

n=0
for e in $cp_line; do
  abs="$(readlink -m "/warm/.fabric/server/$e")"
  [ -f "$abs" ] || { echo "Class-Path entry missing after install: $e" >&2; exit 1; }
  rel="${abs#/warm/}"
  mkdir -p "/out/$(dirname "$rel")"
  cp -a "$abs" "/out/$rel"
  n=$((n + 1))
done
mkdir -p /out/.fabric/server
cp -a /warm/.fabric/server/. /out/.fabric/server/
echo "CLASSPATH_ENTRIES=$n" > /out/.fabric-classpath-count
chown -R "$HOST_UID:$HOST_GID" /out 2>/dev/null || true
'; then
      return 0
    fi
    if [ "$attempt" -ge "$max" ]; then return 1; fi
    echo "    retrying the FABRIC BOOTSTRAP in ${delay}s ($attempt/$max used)" >&2
    sleep "$delay"
    delay=$((delay * 3))
  done
}

# The compose `packtest` service asks itzg for FABRIC_LAUNCHER_VERSION by NAME rather
# than `LATEST`, so that its provisioning check is a local manifest match instead of a
# request to meta.fabricmc.net on every boot. That only holds while the name matches
# what the pinned image actually bakes — so read it off the image and say so loudly if
# it has moved, rather than letting six boots quietly go back to fetching.
echo "==> bootstrap: checking the pinned toolserver's baked Fabric launcher"
baked="$(docker run --rm --entrypoint /bin/bash "$TOOL_IMAGE" -c \
  'python3 -c "import json;print(json.load(open(\"/data/.fabric-manifest.json\"))[\"origin\"][\"installer\"])" 2>/dev/null \
   || sed -n "s/.*\"installer\":\"\([^\"]*\)\".*/\1/p" /data/.fabric-manifest.json')"
if [ "$baked" != "$FABRIC_LAUNCHER" ]; then
  echo "::error::the pinned toolserver bakes Fabric launcher '$baked', but versions.toml" >&2
  echo "  [fabric].launcher_version is '$FABRIC_LAUNCHER' (and compose asks itzg for that)." >&2
  echo "  A name itzg cannot satisfy locally sends it to meta.fabricmc.net on EVERY boot," >&2
  echo "  which is the per-boot fetch this cache exists to remove. Update versions.toml" >&2
  echo "  and validation/compose.yaml to '$baked'." >&2
  exit 1
fi
echo "    ok: launcher $FABRIC_LAUNCHER matches the pinned image"

echo "==> bootstrap: generating the Fabric launch jar + loader Class-Path (once)"
if ! fabric_warm; then
  echo "::error::BOOTSTRAP FAILED — could not generate the Fabric server bootstrap." >&2
  echo "  This step reads launchermeta.mojang.com and maven.fabricmc.net. It is the" >&2
  echo "  NETWORK (or one of those hosts), not the datapack: no PackTest suite has run" >&2
  echo "  and no datapack has been evaluated. Nothing about this red implicates the delve." >&2
  exit 1
fi

# ------------------------------------------------------------------- 3. binding --
# A cache that seeded nothing would leave every boot fetching live again while this
# script reported success — green, and bound to nothing (CLAUDE.md: a green gate
# that binds to nothing is VACUOUS). So the bindings are counted and asserted here,
# and the count is written into the marker for the callers to state.
cp_count="$(. "$cache/.fabric-classpath-count"; echo "$CLASSPATH_ENTRIES")"
rm -f "$cache/.fabric-classpath-count"
fabric_jar="$cache/.fabric/server/${MC_VERSION}-server.jar"
launch_jar="$(ls "$cache"/.fabric/server/fabric-loader-server-*.jar 2>/dev/null || true)"

fail=0
[ -f "$cache/$jar_name" ]            || { echo "::error::bootstrap cache lacks $jar_name" >&2; fail=1; }
[ -f "$cache/.vanilla-manifest.json" ] || { echo "::error::bootstrap cache lacks .vanilla-manifest.json" >&2; fail=1; }
[ -f "$fabric_jar" ]                 || { echo "::error::bootstrap cache lacks .fabric/server/${MC_VERSION}-server.jar" >&2; fail=1; }
[ -n "$launch_jar" ]                 || { echo "::error::bootstrap cache lacks the Fabric launch jar" >&2; fail=1; }
[ "${cp_count:-0}" -gt 0 ] 2>/dev/null || { echo "::error::bootstrap cache seeded ZERO Class-Path libraries — Fabric would re-install and fetch live" >&2; fail=1; }
if [ -f "$fabric_jar" ]; then
  got="$(sha256_of "$fabric_jar")"
  [ "$got" = "$JAR_SHA256" ] || { echo "::error::the Fabric-side server jar does not match the pinned sha256 ($got != $JAR_SHA256)" >&2; fail=1; }
fi
[ "$fail" -eq 0 ] || exit 1

seeded="$(cd "$cache" && find . -type f -not -name '.dw-bootstrap.json' | wc -l | tr -d ' ')"
cat > "$marker" <<JSON
{
  "minecraft_version": "$MC_VERSION",
  "server_jar_sha256": "$JAR_SHA256",
  "fabric_loader_version": "$FABRIC_LOADER",
  "fabric_classpath_entries": $cp_count,
  "seeded_files": $seeded
}
JSON

echo "==> bootstrap cache ready at $cache"
echo "    live Mojang fetches performed: 1 (the pinned jar) — every server boot seeded from here performs 0"
echo "    binding: $seeded files, of which $cp_count Fabric Class-Path libraries"
