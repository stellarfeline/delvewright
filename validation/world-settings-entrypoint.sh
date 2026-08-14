#!/usr/bin/env bash
# Derive the campaign-varying world settings from the compiler's server.properties
# and hand off to the itzg entrypoint. Used by EVERY server that boots a build
# output: the shipped delve image (baked by Dockerfile.delve) and the validation
# PackTest runner (mounted by compose.yaml). One source, so no server can run a
# different world than the compiler declared.
#
# CONTRACT: this file and the heredoc body in validation/Dockerfile.delve are
# byte-identical; validation/check-world-settings.sh fails CI if they drift.
set -e
props="${DELVE_SERVER_PROPERTIES:-/delve/server/server.properties}"
if [ ! -f "$props" ]; then
  echo "FATAL: $props is missing - cannot derive the world the compiler declared;" >&2
  echo "refusing to boot a world that is not this campaign's." >&2
  exit 1
fi
# First match wins, and `sed` stops by itself rather than being cut off by a
# reader. It used to be `sed -n "s/^$1=//p" "$props" | head -1`, which is the
# SIGPIPE+pipefail shape: `head` exits at line one, `sed` dies of
# SIGPIPE, and a pipeline status of 141 becomes the function's. This script sets
# `set -e` WITHOUT `pipefail`, so that was latent rather than live — which is
# exactly why it is worth removing. Adding one `set -o pipefail` here would have
# turned every property read on the player-facing boot path into a coin flip,
# and nothing about the change would have looked dangerous.
prop() { sed -n "/^$1=/{s///;p;q;}" "$props"; }
# Difficulty. v0.6: `world.difficulty` is DECLARED by the campaign (easy/normal/
# hard) and the compiler writes it here; absent, it derives easy for a wave
# campaign and peaceful for a wave-free one (peaceful removes summoned mobs).
# Either way the file is the authority: the image's DIFFICULTY env is a fallback
# for an absent file only, and this line is what stops it from deciding how hard
# the delve is. The seed is the compiler's (ADR-0006).
d=$(prop difficulty);   if [ -n "$d" ];  then export DIFFICULTY="$d";        fi
s=$(prop level-seed);   if [ -n "$s" ];  then export SEED="$s";              fi
# World generator (spec-0013 v0.6 `horizon`): void by default, ocean superflat for
# horizon=ocean. Sourced here so every server builds the SAME world the compiler
# declared. Without this a fixed void default wins and an ocean delve boots as a
# void: content sits at y>=60 but there is no terrain, so the joining player (and
# the validate-profile bot) falls out of the world - and the PackTest runner would
# exercise a world the delve never ships.
lt=$(prop level-type);        if [ -n "$lt" ]; then export LEVEL_TYPE="$lt";        fi
gs=$(prop generator-settings); if [ -n "$gs" ]; then export GENERATOR_SETTINGS="$gs"; fi
# Chunk distances. Left underived, the two boot paths take their values from two
# different sources - the itzg base's own /image/server.properties template, and
# the vanilla jar's built-in defaults - neither of which is ours, so the delve
# renders and ticks at whatever radius the host happens to choose. Pinned in the
# compiler and read here, every server that boots this build sees the same chunks
# and ticks the same chunks (ADR-0006). itzg maps both to properties via
# /image/property-definitions.json (view-distance -> VIEW_DISTANCE,
# simulation-distance -> SIMULATION_DISTANCE).
vd=$(prop view-distance);       if [ -n "$vd" ]; then export VIEW_DISTANCE="$vd";       fi
sd=$(prop simulation-distance); if [ -n "$sd" ]; then export SIMULATION_DISTANCE="$sd"; fi
# Offline op seeding. itzg's OPS env resolves EVERY name through Mojang's
# PlayerDB - even with ONLINE_MODE=FALSE - so an offline-only name (the
# validation bot) aborts the boot: "Could not resolve user from Playerdb".
# DELVE_OPS_OFFLINE takes a name and writes /data/ops.json directly with the
# SAME deterministic UUID an offline server assigns that name on join (Java
# nameUUIDFromBytes: MD5 of "OfflinePlayer:<name>", version 3, RFC variant) -
# the name never reaches the network, and with OPS unset itzg leaves the file
# alone. Offline servers only: a real account's online UUID differs, so an
# ONLINE_MODE=TRUE server keeps using itzg's OPS env instead.
if [ -n "${DELVE_OPS_OFFLINE:-}" ]; then
  n="$DELVE_OPS_OFFLINE"
  h=$(printf 'OfflinePlayer:%s' "$n" | md5sum | cut -c1-32)
  case "${h:16:1}" in
    [048c]) v=8;; [159d]) v=9;; [26ae]) v=a;; *) v=b;;
  esac
  u="${h:0:8}-${h:8:4}-3${h:13:3}-${v}${h:17:3}-${h:20:12}"
  printf '[{"uuid":"%s","name":"%s","level":4,"bypassesPlayerLimit":false}]\n' \
    "$u" "$n" > /data/ops.json
  echo "[init] Seeded offline op $n ($u) into /data/ops.json"
fi
exec /image/scripts/start "$@"
