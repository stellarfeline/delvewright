"""The suite's time bounds, read once from `.config/nextest.toml` `[profile.ci]`.

spec-0126 §5. The bounds live in one place, the nextest profile every partition
runs under; the gates that state or judge them read them through `bounds()`,
never from a literal of their own, so a change to a bound is one edit to the
profile.

- `test_seconds`: the default per-test bound, `slow-timeout`'s period times
  its `terminate-after` (a test past it is terminated and fails);
- `run_seconds`: the partition's run bound, `global-timeout`;
- `overrides`: test name -> that test's own bound, from every
  `[[profile.ci.overrides]]` entry with `filter = 'test(=<name>)'`.

A bound the profile does not arm (no `terminate-after`, no `global-timeout`) is
`None`. Stdlib only.
"""

from __future__ import annotations

import pathlib
import re
import tomllib
from dataclasses import dataclass, field

PROFILE = ".config/nextest.toml"
_DURATION = re.compile(r"^\s*(\d+)\s*(ms|s|m|h)\s*$")
_FILTER = re.compile(r"^test\(=([^)]+)\)$")


def seconds(text: str) -> float:
    """A nextest duration (`"60s"`, `"78m"`, `"1h"`, `"500ms"`) in seconds."""
    m = _DURATION.match(text)
    if not m:
        raise ValueError(f"not a nextest duration this reader knows: {text!r}")
    n, unit = int(m.group(1)), m.group(2)
    return n * {"ms": 0.001, "s": 1, "m": 60, "h": 3600}[unit]


def _test_bound(slow: object) -> float | None:
    if isinstance(slow, str):
        return None  # a bare period marks SLOW and terminates nothing
    if not isinstance(slow, dict) or "terminate-after" not in slow:
        return None
    return seconds(slow["period"]) * int(slow["terminate-after"])


@dataclass(frozen=True)
class Bounds:
    test_seconds: float | None
    run_seconds: float | None
    overrides: dict[str, float | None] = field(default_factory=dict)

    def for_test(self, name: str) -> float | None:
        """The bound `name` runs under: its override's, else the default."""
        return self.overrides.get(name, self.test_seconds)


def bounds(root: pathlib.Path) -> Bounds:
    """The `[profile.ci]` bounds of the tree at `root`."""
    cfg = tomllib.loads((root / PROFILE).read_text(encoding="utf-8"))
    ci = cfg.get("profile", {}).get("ci", {})
    run = ci.get("global-timeout")
    overrides: dict[str, float | None] = {}
    for o in ci.get("overrides", []):
        m = _FILTER.match(str(o.get("filter", "")))
        if m:
            overrides[m.group(1)] = _test_bound(o.get("slow-timeout"))
    return Bounds(
        test_seconds=_test_bound(ci.get("slow-timeout")),
        run_seconds=seconds(run) if isinstance(run, str) else None,
        overrides=overrides,
    )
