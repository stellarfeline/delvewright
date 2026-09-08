#!/usr/bin/env bash
# The repo's ONE definition of "the sha256 of this file", portable across the
# two shapes a runner actually carries: `sha256sum` (GNU, every Linux runner)
# and `shasum -a 256` (macOS, which ships no `sha256sum` by default). Three
# call sites needed this before it moved here — `tools/crates-io-publish.sh`
# had it TWICE, once for the local tarball and once for the registry's
# download, and `tools/check-publishable.sh` needed a third copy the moment it
# started writing a verified sha256 beside each tarball it packages — and every
# private copy is one more place a fourth site gets the fallback wrong instead
# of reusing the one that is already right.
#
# Usage (source, then call):
#   . "$REPO_ROOT/tools/lib/checksum.sh"
#   sum="$(dw_sha256_file /path/to/file)"   # hex digest on stdout, nothing else

dw_sha256_file() { # <path>
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}
