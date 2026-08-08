#!/usr/bin/env python3
"""Harness/compiler `dsl_version` sync gate (task #157).

The compiler's DSL surface and the harness's critical-path allowlist are two
independent files that must agree on one number: the newest `dsl_version` a
campaign may declare. `crates/dsl/src/envelope.rs` names it once
(`SUPPORTED_DSL_VERSION`, the crate's own "latest version" identity constant);
`harness/src/critical-path.ts` names the harness's whole accepted set
(`SUPPORTED_DSL_VERSIONS`), which every artifact the bot tier parses
(critical-path, waypoints, combat plan) gates on.

Nothing in the repo ties these together, so a compiler version bump lands with
zero signal to the harness. That happened for real: spec-0026 raised
`SUPPORTED_DSL_VERSION` to `0.9.0` while `harness/src/critical-path.ts` still
listed `0.2.0 … 0.8.0` — the server booted, the bot connected, and the gate
refused the campaign before it took a single step (hollow-vigil ladder run,
task #157). Every other CI job was green.

The rule: the compiler's latest `dsl_version` must be a member of the
harness's `SUPPORTED_DSL_VERSIONS`. The harness is allowed to lag behind on
older-but-still-supported versions (this checks membership of the ceiling, not
set equality) but must never fall behind the compiler's own idea of "current".

Deterministic, offline, no dependencies (Python 3 stdlib). Run from the repo
root:
    python3 tools/check-harness-dsl-version.py
Exit 0 = in sync, 1 = the harness lags the compiler (see stderr), 2 = usage/IO
error (missing file or the source no longer matches the expected shape — fix
the regex, don't loosen the check).
"""

import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
ENVELOPE_RS = REPO_ROOT / "crates" / "dsl" / "src" / "envelope.rs"
CRITICAL_PATH_TS = REPO_ROOT / "harness" / "src" / "critical-path.ts"

# `pub const SUPPORTED_DSL_VERSION: &str = "0.9.0";` — the compiler's single
# "latest dsl_version this crate implements" identity constant.
COMPILER_VERSION_RE = re.compile(
    r'pub\s+const\s+SUPPORTED_DSL_VERSION\s*:\s*&str\s*=\s*"([^"]+)"\s*;'
)

# `export const SUPPORTED_DSL_VERSIONS = [ "0.2.0", ..., "0.8.0" ] as const;`
HARNESS_ARRAY_RE = re.compile(
    r"export\s+const\s+SUPPORTED_DSL_VERSIONS\s*=\s*\[(.*?)\]\s*as\s+const\s*;",
    re.DOTALL,
)
VERSION_LITERAL_RE = re.compile(r'"([^"]+)"')


def compiler_version(text: str) -> str | None:
    m = COMPILER_VERSION_RE.search(text)
    return m.group(1) if m else None


def harness_versions(text: str) -> list[str]:
    m = HARNESS_ARRAY_RE.search(text)
    if not m:
        return []
    return VERSION_LITERAL_RE.findall(m.group(1))


def main() -> int:
    for p in (ENVELOPE_RS, CRITICAL_PATH_TS):
        if not p.is_file():
            sys.stderr.write(f"error: {p} not found (run from the repo root)\n")
            return 2

    compiler_max = compiler_version(ENVELOPE_RS.read_text(encoding="utf-8"))
    if compiler_max is None:
        sys.stderr.write(
            'error: could not find `pub const SUPPORTED_DSL_VERSION: &str = "...";` '
            f"in {ENVELOPE_RS} — the constant was renamed/reshaped; update "
            "COMPILER_VERSION_RE in tools/check-harness-dsl-version.py\n"
        )
        return 2

    harness = harness_versions(CRITICAL_PATH_TS.read_text(encoding="utf-8"))
    if not harness:
        sys.stderr.write(
            "error: could not find `export const SUPPORTED_DSL_VERSIONS = [...] as "
            f"const;` (or it parsed empty) in {CRITICAL_PATH_TS} — the declaration was "
            "reshaped; update HARNESS_ARRAY_RE in tools/check-harness-dsl-version.py\n"
        )
        return 2

    if compiler_max not in harness:
        sys.stderr.write(
            "harness dsl_version allowlist lags the compiler:\n"
            f"  {ENVELOPE_RS}: SUPPORTED_DSL_VERSION = {compiler_max!r}\n"
            f"  {CRITICAL_PATH_TS}: SUPPORTED_DSL_VERSIONS = {harness!r}\n"
            f"  {compiler_max!r} is not in the harness allowlist.\n\n"
            "Every campaign at the compiler's current dsl_version would be refused "
            "at the bot-tier version gate before it takes a single step (task #157). "
            "Add the compiler's SUPPORTED_DSL_VERSION to harness/src/critical-path.ts's "
            "SUPPORTED_DSL_VERSIONS (additive addition only — do not remove older "
            "entries still in use), and confirm no version-conditional harness "
            "behavior needs updating for the new version's artifacts.\n"
        )
        return 1

    print(
        f"harness dsl_version allowlist covers the compiler ceiling "
        f"({compiler_max!r} in {harness!r})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
