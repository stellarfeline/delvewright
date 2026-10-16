#!/usr/bin/env bash
# ONE live Mojang fetch of the pinned client jar per JOB.
#
#   validation/client-jar-cache.sh [--cache <dir>] [--force]
#
# Every frame `delvec` draws is drawn with Minecraft's own textures and models,
# read out of the pinned client jar (spec-0112 §6.1 item 5). The jar is never
# committed, cached in the repository or redistributed (ADR-0010, ADR-0023 §9):
# a creator puts it on their machine at Init I5, and a CI job that draws frames
# fetches it here, once, in the shape of `validation/server-bootstrap-cache.sh`.
#
# ## What it produces
#
#   <cache>/minecraft-client-<ver>.jar   the pinned client jar, sha256-verified
#   <cache>/.dw-client-jar.json          marker: version + sha256
#
# and prints the jar's path on its last line, which a caller exports as
# `DELVEWRIGHT_CLIENT_JAR` — the second rung of the engine's jar ladder.
#
# ## Single fetch AND checksum
#
# The download is checked on its bytes against `[minecraft] client_jar_sha256`
# from versions.toml; a truncated or substituted jar is a named red here, before
# any frame is drawn, never a frame drawn from the wrong textures. Retries are
# bounded and scoped to this fetch alone. When Mojang is unreachable for the
# whole run this exits non-zero with a `::error::` naming the URL: a network red
# at a step named for the jar, never a frame red.
#
# Idempotent: a cache whose marker and bytes match the pin performs no fetch.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$here/.." && pwd)"
MANIFEST="$ROOT/versions.toml"

cache="$here/client-cache"
force=0
usage() {
  cat >&2 <<'USAGE'
usage: validation/client-jar-cache.sh [--cache <dir>] [--force]

  --cache  Where the jar is kept (default validation/client-cache).
  --force  Fetch even if the cache already holds the pinned bytes.
USAGE
  exit 2
}
while [ $# -gt 0 ]; do
  case "$1" in
    --cache|-c) [ $# -ge 2 ] || usage; cache="$2"; shift 2 ;;
    --force)    force=1; shift ;;
    -h|--help)  usage ;;
    *) echo "client-jar-cache: unknown argument '$1'" >&2; usage ;;
  esac
done

[ -f "$MANIFEST" ] || { echo "FATAL: $MANIFEST not found" >&2; exit 2; }

# versions.toml is the single source of truth: nothing below hardcodes a version,
# URL or checksum.
eval "$(python3 - "$MANIFEST" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/ci/check-python-shell-newlines.py
d = tomllib.load(open(sys.argv[1], "rb"))
def emit(k, v): print(f'{k}={v!r}'.replace("'", '"'))
emit("MC_VERSION", d["minecraft"]["version"])
emit("JAR_URL",    d["minecraft"]["client_jar_url"])
emit("JAR_SHA256", d["minecraft"]["client_jar_sha256"])
emit("JAR_SIZE",   d["minecraft"]["client_jar_size"])
PY
)"

jar="$cache/minecraft-client-${MC_VERSION}.jar"
marker="$cache/.dw-client-jar.json"

sha256_of() { # <file> -> bare hex
  if command -v sha256sum >/dev/null 2>&1; then
    set -- "$(sha256sum "$1")"; echo "${1%% *}"
  else
    set -- "$(shasum -a 256 "$1")"; echo "${1%% *}"
  fi
}

if [ "$force" -eq 0 ] && [ -f "$marker" ] && [ -f "$jar" ]; then
  if [ "$(sha256_of "$jar")" = "$JAR_SHA256" ]; then
    echo "==> client jar already cached at $jar (no fetch: $MC_VERSION, sha256 verified)" >&2
    echo "$jar"
    exit 0
  fi
  echo "==> client jar at $jar is STALE (sha256 moved) — fetching again" >&2
fi

mkdir -p "$cache"
rm -f "$marker"

attempt=0 max=3 delay=5
echo "==> client jar fetch 1/1: the pinned Minecraft client jar ($MC_VERSION)" >&2
while :; do
  attempt=$((attempt + 1))
  rm -f "$jar.part"
  if curl -fSL --no-progress-meter --connect-timeout 20 --max-time 600 \
       --retry 0 -o "$jar.part" "$JAR_URL"; then
    got="$(sha256_of "$jar.part")"
    if [ "$got" = "$JAR_SHA256" ]; then
      mv -f "$jar.part" "$jar"
      echo "    fetched and verified ($JAR_SIZE bytes, sha256 $JAR_SHA256)" >&2
      break
    fi
    echo "    attempt $attempt: sha256 MISMATCH (got $got, pinned $JAR_SHA256) — truncated or wrong object" >&2
  else
    rc=$?
    echo "    attempt $attempt: curl failed (exit $rc) fetching $JAR_URL" >&2
  fi
  rm -f "$jar.part"
  if [ "$attempt" -ge "$max" ]; then
    echo "::error::CLIENT JAR FETCH FAILED — could not obtain the pinned Minecraft client jar." >&2
    echo "  url:  $JAR_URL" >&2
    echo "  This is the NETWORK (or Mojang), not the delve: no frame has been drawn yet." >&2
    exit 1
  fi
  echo "    retrying the CLIENT JAR FETCH in ${delay}s ($attempt/$max used)" >&2
  sleep "$delay"
  delay=$((delay * 3))
done

printf '{"minecraft_version": "%s", "sha256": "%s"}\n' "$MC_VERSION" "$JAR_SHA256" > "$marker"
echo "$jar"
