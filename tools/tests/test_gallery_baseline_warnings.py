"""The expected-warnings ledger must be able to READ every warning it compares.

The ledger's whole claim is that *the emitted warning set equals the committed
one exactly*, so "still green" can never quietly absorb "warns differently now".
That claim was false. `WARNING_RE` required the pointer to be a single non-space
token, and the parse site skipped any line that did not match — so every
build-tier diagnostic, whose pointer names a PHASE of the build rather than a
JSON pointer (`packtest watch coverage`, `packtest batch-state binding`), was
dropped from the comparison without a word. A warning class the ledger cannot
see is a warning class the ledger cannot gate, and it failed in the direction
that reads as a clean pass.

It was found because a newly added `DW0810` did not appear in the regenerated
ledger. Nothing would have reported it: the file was written, the run exited 0,
and the row simply was not there.
"""

from __future__ import annotations

import importlib.util
import pathlib
import sys

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]


def _load():
    path = REPO / "tools" / "gallery-baseline.py"
    spec = importlib.util.spec_from_file_location("gallery_baseline", path)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules["gallery_baseline"] = mod
    spec.loader.exec_module(mod)
    return mod


GB = _load()


@pytest.mark.parametrize(
    ("line", "code", "stage", "pointer"),
    [
        # The build-tier shape that was silently dropped: the pointer is a
        # phase of the build and contains spaces.
        (
            "DW0810 [warning] build packtest watch coverage: the generated suite drives one",
            "DW0810",
            "build",
            "packtest watch coverage",
        ),
        (
            "DW0807 [warning] build packtest batch-state binding: the invariant judged 0 of 4",
            "DW0807",
            "build",
            "packtest batch-state binding",
        ),
        # The stage-tier shapes that always worked, kept so the widened pattern
        # is proven not to have moved them.
        (
            "DW0527 [warning] quests /content/quests/1/x/requires_state: this compares state",
            "DW0527",
            "quests",
            "/content/quests/1/x/requires_state",
        ),
        (
            "DW0188 [warning] l10n l10n/zh-cn.json: 99 of 99 translated rows record no source",
            "DW0188",
            "l10n",
            "l10n/zh-cn.json",
        ),
    ],
)
def test_every_warning_shape_parses(line, code, stage, pointer):
    m = GB.WARNING_RE.match(line)
    assert m is not None, f"the ledger cannot read this warning, so it cannot compare it: {line}"
    assert (m.group(1), m.group(2), m.group(3)) == (code, stage, pointer)


def test_a_warning_line_is_recognised_as_one_even_when_it_does_not_parse():
    """The guard that turns the next silent drop into a red.

    `ANY_WARNING_RE` is deliberately weaker than `WARNING_RE`: anything that
    announces itself as a warning is a warning, whatever else about the line the
    strict pattern cannot handle. The parse site reds on exactly that
    difference, so a future diagnostic with a shape nobody anticipated stops the
    build instead of quietly leaving the comparison.
    """
    weird = "DW9999 [warning] no-colon-anywhere-in-this-line"
    assert GB.ANY_WARNING_RE.match(weird), "it is plainly a warning"
    assert GB.WARNING_RE.match(weird) is None, "and the strict pattern cannot read it"


def test_a_non_warning_line_is_not_mistaken_for_one():
    for line in [
        "DW0161 [error] world /content/areas/1/prefab_pool: area pool is not declared",
        "gallery coverage: 801 unit(s) enumerated",
        "",
    ]:
        assert GB.ANY_WARNING_RE.match(line) is None, line
