#!/usr/bin/env bash
# Install the pinned Chunky core by name — built from source at the pinned
# revision, and installed only when it is the pin.
#
# ## Why this builds rather than downloads
#
# Every emitted scene is written for ONE core, `versions.toml [render]
# chunky_core`. The launcher's `--update` takes a release CHANNEL, whose record
# names only its newest build, and the update site's `lib/<name>.jar` path serves
# today's jar whatever name it is asked for (`docs/reference/tools.md` §4a). No
# download names the pin. Chunky is GPL-3.0 and its source is public, so the pin
# is built the way its own snapshot job builds it: Chunky's Gradle wrapper, at
# `[render] chunky_revision`, under the JDK Chunky's toolchain declares at that
# revision (`[render] chunky_build_java`).
#
# ## What makes the result the pin
#
# Not its name and not its checksum: the jar's CONTENT digest
# (`tools/lib/chunky_core.py`), held to `[render] chunky_core_content_sha256`.
# Measured when the pin was recorded: a build at the revision and the core the
# launcher installed answer the same digest. A build that answers anything else
# installs nothing.
#
# ## What it writes, and what it leaves
#
# Into the Chunky home Chunky itself reads (`tools/lib/chunky-home.sh`): the core
# and the libraries its build names into `lib/`, and the version record naming
# them into `versions/<core>.json` — Chunky's own layout, the one
# `validation/chunky.sh` renders from. The clone and the Gradle cache live in a
# work directory (`--work`, default `~/.delvewright/chunky-build`), which is
# deleted after a verified install and kept, and named, after a failure. Nothing
# is written under `~/.gradle`.
#
# A home that already holds the pin is left alone, exit 0.
#
# Usage: validation/chunky-install.sh [--java-home DIR] [--work DIR]
#   --java-home  a JDK whose `java -version` answers `[render] chunky_build_java`
#                (default: `JAVA_HOME`, when its java answers that major)
#   --work       where the clone and the Gradle cache are built
#
# Needs `git`, `python3` and the network (the Chunky repository, the Gradle
# distribution, Maven Central). Exit 0 = the pin is installed; 1 = the build or
# the install failed; 2 = a precondition is missing, named.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"

java_home_arg=""
work=""
while [ $# -gt 0 ]; do
  case "$1" in
    --java-home) java_home_arg="$2"; shift 2;;
    --work) work="$2"; shift 2;;
    -h|--help) sed -n '2,43p' "$0"; exit 0;;
    *) echo "chunky-install: unknown argument \`$1\`" >&2; exit 2;;
  esac
done

pin() { python3 "$repo/tools/lib/versions.py" "render.$1"; }
core="$(pin chunky_core)" || exit 2
source_url="$(pin chunky_source)" || exit 2
revision="$(pin chunky_revision)" || exit 2
build_java="$(pin chunky_build_java)" || exit 2

# shellcheck source=tools/lib/chunky-home.sh
. "$repo/tools/lib/chunky-home.sh"
dw_resolve_chunky_home
home="$DW_CHUNKY_HOME"
echo "chunky home: $home (from $DW_CHUNKY_HOME_SOURCE)"

if python3 "$repo/tools/lib/chunky_core.py" verify --home "$home" 2>/dev/null; then
  echo "chunky-install: nothing to do — the pin is already installed."
  exit 0
fi
echo "chunky-install: $core is not installed at $home; building it."

# ---- the JDK the pinned revision builds under --------------------------------
java_home="${java_home_arg:-${JAVA_HOME:-}}"
major=""
if [ -n "$java_home" ] && [ -x "$java_home/bin/java" ]; then
  major="$("$java_home/bin/java" -version 2>&1 | awk -F'"' 'NR==1{split($2,v,"."); print v[1]}')"
fi
if [ "$major" != "$build_java" ]; then
  echo "chunky-install: the pinned Chunky revision builds under JDK $build_java (\`[render] chunky_build_java\`), and" >&2
  if [ -z "$java_home" ]; then
    echo "  neither --java-home nor JAVA_HOME names a JDK." >&2
  else
    echo "  $java_home answers ${major:-no version}." >&2
  fi
  echo "  Pass --java-home <a JDK $build_java home>. Chunky's Gradle build runs on it and compiles with it; another" >&2
  echo "  JDK cannot run that Gradle, and nothing is installed without it." >&2
  exit 2
fi

# ---- the build ---------------------------------------------------------------
work="${work:-$HOME/.delvewright/chunky-build}"
src="$work/src"
fail() {
  echo "chunky-install: $1" >&2
  echo "  The work directory is kept for inspection: $work" >&2
  exit 1
}
rm -rf "$src"
mkdir -p "$work"
echo "chunky-install: cloning $source_url at $revision into $src"
git clone --quiet --no-checkout "$source_url" "$src" || fail "could not clone $source_url"
git -C "$src" -c advice.detachedHead=false checkout --quiet "$revision" \
  || fail "the clone has no revision $revision"
[ "$(git -C "$src" rev-parse HEAD)" = "$revision" ] || fail "HEAD is not $revision after checkout"
timestamp="$(git -C "$src" show -s --format=%cI HEAD)"

# The snapshot tag is the pin's own: `chunky-core-<major>-<tag>.<count>.g<sha>`.
tag="${core#chunky-core-*-}"
tag="${tag%%.*}"
echo "chunky-install: building ${core} with JDK $build_java (Gradle output in $work/gradle.log)"
if ! (cd "$src" && JAVA_HOME="$java_home" GRADLE_USER_HOME="$work/gradle" \
      ./gradlew --no-daemon --quiet "-PprereleaseTag=$tag" \
        :chunky:jar :chunky:copyExternalDependencies) >"$work/gradle.log" 2>&1; then
  tail -20 "$work/gradle.log" >&2
  fail "the Gradle build failed"
fi

python3 "$repo/tools/lib/chunky_core.py" record --home "$home" \
  --libs "$src/chunky/build/libs" --timestamp "$timestamp" >/dev/null \
  || fail "the build is not the pin, so nothing was installed"
python3 "$repo/tools/lib/chunky_core.py" verify --home "$home" \
  || fail "the install did not verify"
rm -rf "$work"
echo "chunky-install: installed; render with validation/chunky.sh."
