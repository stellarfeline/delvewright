"""The crates.io sparse index, read one way — one authority for every caller.

## Why this is a library

`tools/crates-io-publish.sh` had the whole of it inline: the sparse-index path
scheme, the fetch, the JSON-lines scan for one version's `cksum`, and the bind
test that refuses to believe any of it until a version everyone knows exists
resolves. A second caller then needed exactly the same question answered —
`tools/check-dsl-version-published.py` asks whether the number a change moves
AWAY from is on the registry — and a private copy of a format reader is the shape
this repository has already paid for twice (`tools/lib/versions.py` on TOML,
`tools/lib/checksum.sh` on sha256). So the reading lives here and both callers
come through it.

Shell reads it the same way it reads a pin (`tools/lib/versions.py`'s own door):

    cksum="$(python3 tools/lib/crates_index.py cksum serde 1.0.0)"
    python3 tools/lib/crates_index.py bind-test || exit 1

## The layout, which is the registry's and not ours

crates.io serves one file per crate, JSON lines, one line per published version.
The path is by name length: 1 char `1/<n>`, 2 chars `2/<n>`, 3 chars
`3/<n[0]>/<n>`, otherwise `<n[0:2]>/<n[2:4]>/<n>`, all lowercase.

## ABSENT AND UNREACHABLE LOOK THE SAME, WHICH IS WHY THE BIND TEST EXISTS

A lookup answers "this version is not on the registry" by finding no row. Every
transport failure — no network, a blocked host, a changed path scheme, a changed
JSON shape — produces exactly the same answer, and a caller that acts on it is
acting on an unbound gate: for the publisher it turns a safe skip into a burned
version, and for a checker it turns a real question into a red with the wrong
reason. Neither caller may believe a "no" until [`bind_test`] has seen a version
that certainly exists resolve. `serde 1.0.0` is that version: it is on crates.io
and index rows are never deleted.

`DW_CRATES_INDEX` overrides the base URL for a test against a local fixture
index; it is printed the moment it fires, so an override can never survive
silently into a real answer.

Stdlib only. Exit codes for the CLI: 0 answered, 1 the bind test failed, 2 usage.
"""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request

REAL_INDEX = "https://index.crates.io"

# The version whose presence proves the lookup works. Not a connectivity check:
# what it binds is the whole chain — URL scheme, fetch, JSON shape — against a
# fact that cannot change.
BIND_CRATE = "serde"
BIND_VERSION = "1.0.0"

TIMEOUT_SECONDS = 30


def index_base() -> str:
    """The sparse-index base URL, and a loud line when it is not crates.io."""
    base = os.environ.get("DW_CRATES_INDEX") or REAL_INDEX
    if base != REAL_INDEX:
        print(
            f"crates_index: DW_CRATES_INDEX={base} — NOT the real crates.io index",
            file=sys.stderr,
        )
    return base.rstrip("/")


def index_path(name: str) -> str:
    """The sparse index's own path for a crate name."""
    n = name.lower()
    return {1: f"1/{n}", 2: f"2/{n}", 3: f"3/{n[0]}/{n}"}.get(len(n), f"{n[0:2]}/{n[2:4]}/{n}")


def _fetch(name: str) -> str | None:
    """The crate's index file, or None when the registry does not serve one.

    A 404 is a normal answer (an unpublished crate has no file at all). Every
    other failure is reported on stderr and also answered None — the caller may
    not tell them apart, which is the whole reason [`bind_test`] exists.
    """
    url = f"{index_base()}/{index_path(name)}"
    try:
        with urllib.request.urlopen(url, timeout=TIMEOUT_SECONDS) as response:  # noqa: S310
            return response.read().decode("utf-8")
    except urllib.error.HTTPError as exc:
        if exc.code != 404:
            print(f"crates_index: {url} -> HTTP {exc.code}", file=sys.stderr)
        return None
    except (urllib.error.URLError, OSError, TimeoutError) as exc:
        print(f"crates_index: {url} -> {exc}", file=sys.stderr)
        return None


def cksum(name: str, version: str) -> str:
    """The sha256 the registry records for `name` at `version`, or `""`.

    `""` means "no row was found", which is ABSENT and UNREACHABLE at once. Run
    [`bind_test`] before believing it.
    """
    body = _fetch(name)
    if body is None:
        return ""
    for line in body.splitlines():
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if row.get("vers") == version:
            return str(row.get("cksum", ""))
    return ""


def versions(name: str) -> list[str]:
    """Every version the index serves for `name`, in the order it lists them."""
    body = _fetch(name)
    if body is None:
        return []
    out: list[str] = []
    for line in body.splitlines():
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        vers = row.get("vers")
        if isinstance(vers, str):
            out.append(vers)
    return out


def bind_test() -> tuple[bool, str]:
    """Did the lookup resolve a version that certainly exists? `(ok, message)`."""
    got = cksum(BIND_CRATE, BIND_VERSION)
    if not got:
        return False, (
            f"the index lookup returned nothing for {BIND_CRATE} {BIND_VERSION}, which "
            f"certainly exists.\n"
            f"  'this version is absent' would therefore be an unbound answer for every "
            f"other crate too, so nothing that reads this lookup may act on one."
        )
    return True, f"{BIND_CRATE} {BIND_VERSION} resolves to sha256 {got}"


USAGE = """usage: crates_index.py <command> [args]

  path <crate>                the sparse-index path for a crate name
  cksum <crate> <version>     the registry's sha256, or nothing when absent
  versions <crate>            every version the index serves, one per line
  bind-test                   refuse (exit 1) unless a known version resolves
"""


def main(argv: list[str]) -> int:
    if not argv:
        print(USAGE, file=sys.stderr)
        return 2
    command, rest = argv[0], argv[1:]
    sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
    if command == "path" and len(rest) == 1:
        print(index_path(rest[0]))
        return 0
    if command == "cksum" and len(rest) == 2:
        got = cksum(rest[0], rest[1])
        if got:
            print(got)
        return 0
    if command == "versions" and len(rest) == 1:
        for vers in versions(rest[0]):
            print(vers)
        return 0
    if command == "bind-test" and not rest:
        ok, message = bind_test()
        if ok:
            print(f"  ok   {message}")
            return 0
        print(f"crates_index: {message}", file=sys.stderr)
        return 1
    print(USAGE, file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
