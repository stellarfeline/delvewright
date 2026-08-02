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
prop() { sed -n "s/^$1=//p" "$props" | head -1; }
# v0.3: combat-wave campaigns need difficulty=easy (peaceful removes summoned
# mobs); wave-free delves stay peaceful. The seed is the compiler's (ADR-0006).
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
exec /image/scripts/start "$@"
