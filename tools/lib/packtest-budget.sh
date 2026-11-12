#!/usr/bin/env bash
# The repo's ONE rule for how long a PackTest run may take before it is a hang,
# computed from what the build declares, never from a host's measurement.
#
#   . "$REPO_ROOT/tools/lib/packtest-budget.sh"
#   dw_packtest_budget <build-dir>   # -> "<seconds> <tests> <ticks>"
#
# Every test the build's PackTest datapack declares
# (`<build-dir>/packtest-datapack/data/<ns>/test/**/*.mcfunction`) is ended by
# the game at its `@timeout` — the directive PackTest reads from the comment
# lines that open the file, in game ticks — whether it passes or not; a test
# that states none takes PackTest's default (`DW_PACKTEST_DEFAULT_TIMEOUT_TICKS`).
# The suite therefore ends within the sum of its tests' timeouts run one after
# another at the game's 20 ticks a second, however many run side by side. The
# budget is that sum in seconds, rounded up, plus the start-up allowance
# (`DW_PACKTEST_START_SECONDS`): the server's boot, the datapacks' load and the
# test structures' placement, which no test's timeout covers. A run past the
# budget is a hang. A host whose server cannot hold 20 ticks a second does not
# meet the build's stated cost (CLAUDE.md, *Host hardware never caps a
# capability*).

# PackTest's timeout for a test that states none, in ticks (PackTest 2.4,
# `@timeout`: "defaults to 100").
DW_PACKTEST_DEFAULT_TIMEOUT_TICKS=100
# The start-up allowance, in seconds: the whole-run bound every suite measured
# before this rule ran inside, kept as the part of a run no test declares.
DW_PACKTEST_START_SECONDS=900
# Game ticks a second.
DW_GAME_TICKS_PER_SECOND=20

# dw_packtest_budget <build-dir> -> "<seconds> <tests> <ticks>" on stdout:
# the budget, the tests it counted and the ticks they declare. Fails (exit 1,
# named) when the build declares no PackTest datapack or no test in it — a
# budget over nothing binds nothing.
dw_packtest_budget() {
  local build="$1" root
  root="$build/packtest-datapack/data"
  if [ ! -d "$root" ]; then
    echo "packtest-budget: no PackTest datapack at $root" >&2
    return 1
  fi
  local tests=0 ticks=0 f t line
  while IFS= read -r -d '' f; do
    t="$DW_PACKTEST_DEFAULT_TIMEOUT_TICKS"
    while IFS= read -r line || [ -n "$line" ]; do
      case "$line" in
        '#'*)
          if [[ "$line" =~ ^#[[:space:]]*@timeout[[:space:]]+([0-9]+)[[:space:]]*$ ]]; then
            t="${BASH_REMATCH[1]}"
          fi
          ;;
        *[![:space:]]*) break ;;
      esac
    done < "$f"
    tests=$((tests + 1))
    ticks=$((ticks + 10#$t))
  done < <(find "$root" -path '*/test/*' -name '*.mcfunction' -type f -print0)
  if [ "$tests" -eq 0 ]; then
    echo "packtest-budget: no test under $root" >&2
    return 1
  fi
  local secs=$(( (ticks + DW_GAME_TICKS_PER_SECOND - 1) / DW_GAME_TICKS_PER_SECOND ))
  echo "$((DW_PACKTEST_START_SECONDS + secs)) $tests $ticks"
}
