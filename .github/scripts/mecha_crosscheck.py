#!/usr/bin/env python3
"""mecha cross-check (ADR-0011).

Independently re-validate every emitted ``.mcfunction`` line against the pinned
1.21.11 command tree using mecha, as a CI-only check on our first-party
vendored-command-tree validator. Any disagreement (a line mecha rejects) fails
CI and is a bug in one of the two validators.

Usage: ``mecha_crosscheck.py <datapack-dir>`` (default ``out/datapack``).

Note: pins are `mecha==0.104.1` + `beet` (see the workflow). The mecha/beet
invocation below follows their documented programmatic API; it is exercised for
the first time on CI (the build host used to author this had Python 3.14 with no
mecha wheels), so if the entry points shift, adjust here — the contract is
"every emitted line parses against 1.21.11".
"""

import sys
from pathlib import Path


def main(datapack_dir: str) -> int:
    functions = sorted(Path(datapack_dir).rglob("*.mcfunction"))
    if not functions:
        print(f"no .mcfunction files found under {datapack_dir}", file=sys.stderr)
        return 1

    from beet import run_beet
    from mecha import Mecha

    failures = 0
    checked = 0
    # `minecraft` pins the command tree mecha validates against (ADR-0009).
    with run_beet({"require": ["mecha"], "minecraft": "1.21.11"}) as ctx:
        mc = ctx.inject(Mecha)
        for path in functions:
            for lineno, raw in enumerate(path.read_text().splitlines(), start=1):
                line = raw.strip()
                if not line or line.startswith("#"):
                    continue
                checked += 1
                try:
                    mc.parse(line, using="command")
                except Exception as exc:  # mecha.DiagnosticError and friends
                    failures += 1
                    print(f"FAIL {path}:{lineno}: {line}\n  {exc}", file=sys.stderr)

    print(f"mecha checked {checked} command line(s) across "
          f"{len(functions)} function(s); {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    target = sys.argv[1] if len(sys.argv) > 1 else "out/datapack"
    sys.exit(main(target))
