"""Unit tests for tools/zone-sheets.py — the refusal digest it reads back.

`delve-grammar sweep` prints the whole reading of a guard refusal on stderr.
The driver's obligation is the other half: a refusal reason a human can act on
and a tool cannot is half a diagnostic, so the structured `refusal` a refused
row carries in `sweep.json` has to survive into the summary the driver
assembles. These tests hold the digest to the clause that actually decided the
refusal, and to leaving alone a row that failed for a reason that is not a
guard.
"""

import importlib.util
from pathlib import Path

TOOL = Path(__file__).resolve().parents[1] / "zone-sheets.py"


def _load():
    spec = importlib.util.spec_from_file_location("zone_sheets", TOOL)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _cmp(lhs, op, rhs, holds, blocks=None):
    clause = {
        "cond": "cmp",
        "lhs": lhs,
        "op": op,
        "rhs": rhs,
        "holds": holds,
    }
    if blocks is not None:
        clause["shortfall"] = {
            "blocks": blocks,
            "lhs_must_reach": 0,
            "rhs_must_reach": 0,
        }
    return clause


def _report():
    """The real chapel-ward case: one clause of four decided the refusal."""
    return {
        "rows": [
            {
                "id": "great-hearth",
                "error": "no alternative of rule \"ward_plan\" applies …",
                "refusal": {
                    "symbol": "ward_plan",
                    "scope": {"size": [16, 9, 26]},
                    "alternatives": [
                        {
                            "index": 1,
                            "guard": {
                                "cond": "all",
                                "of": [
                                    _cmp(
                                        {"source": "strip_depth", "value": 9},
                                        "gt",
                                        {"source": "junction_run", "value": 8},
                                        True,
                                    ),
                                    _cmp(
                                        {
                                            "source": "Dimension.Z - junction_run - hearth_run",
                                            "value": 4,
                                        },
                                        "gt",
                                        {
                                            "source": "Dimension.X - strip_depth",
                                            "value": 7,
                                        },
                                        False,
                                        blocks=4,
                                    ),
                                ],
                            },
                        }
                    ],
                },
            },
            {"id": "as-designed"},
            {"id": "unwritable", "error": "cannot write unwritable.nbt: no space left"},
        ]
    }


def test_the_digest_names_the_clause_that_refused_and_not_the_ones_that_held():
    lines = _load().refusal_digest(_report())
    assert len(lines) == 1, lines
    line = lines[0]
    assert line.startswith("great-hearth: rule ward_plan")
    # The clause that decided it, as written, with both sides and the distance.
    assert "Dimension.Z - junction_run - hearth_run > Dimension.X - strip_depth" in line
    assert "(4 vs 7)" in line
    assert "4 short" in line
    # ...and not the three that held, which would bury it.
    assert "strip_depth > junction_run" not in line


def test_a_row_that_built_or_failed_for_another_reason_has_no_digest_line():
    lines = _load().refusal_digest(_report())
    assert not any("as-designed" in line for line in lines)
    assert not any("unwritable" in line for line in lines), (
        "a write failure is not a guard refusal and must not be dressed as one"
    )


def test_a_guard_that_declines_on_orientation_still_gets_a_line():
    """A refusal whose digest line is absent is the defect this digest removes.

    Not every guard is arithmetic: `orientation` is how a directional piece
    picks its facing, and a rule can decline on it alone. Reading only `cmp`
    clauses printed nothing at all for that refusal.
    """
    report = {
        "rows": [
            {
                "id": "turned",
                "refusal": {
                    "symbol": "stair_flight",
                    "alternatives": [
                        {
                            "index": 1,
                            "guard": {
                                "cond": "orientation",
                                "want": ["x", "y", "z"],
                                "got": ["z", "y", "x"],
                            },
                        }
                    ],
                },
            }
        ]
    }
    lines = _load().refusal_digest(report)
    assert len(lines) == 1, lines
    assert "orientation is x,y,z, the scope's is z,y,x" in lines[0]


def test_a_clause_under_none_of_is_named_for_holding_not_skipped_for_it():
    """`none_of` inverts what a clause had to do, and the digest must invert with
    it — otherwise the one clause that caused the refusal is the one clause left
    out, and every other clause is reported instead."""
    report = {
        "rows": [
            {
                "id": "too-wide",
                "refusal": {
                    "symbol": "nook",
                    "alternatives": [
                        {
                            "index": 1,
                            "guard": {
                                "cond": "none_of",
                                "of": [
                                    _cmp(
                                        {"source": "Dimension.X", "value": 12},
                                        "gt",
                                        {"source": "8", "value": 8},
                                        True,
                                    ),
                                    _cmp(
                                        {"source": "Dimension.Y", "value": 4},
                                        "gt",
                                        {"source": "9", "value": 9},
                                        False,
                                        blocks=6,
                                    ),
                                ],
                            },
                        }
                    ],
                },
            }
        ]
    }
    lines = _load().refusal_digest(report)
    assert len(lines) == 1, lines
    # The clause that HELD is the one that refused, under `none_of`.
    assert "Dimension.X > 8" in lines[0]
    assert "Dimension.Y" not in lines[0]


def test_a_clause_that_could_not_be_measured_says_so_rather_than_vanishing():
    report = {
        "rows": [
            {
                "id": "bad-arith",
                "refusal": {
                    "symbol": "wing",
                    "alternatives": [
                        {
                            "index": 1,
                            "guard": {
                                "cond": "unreadable",
                                "source": "Dimension.Y / 0 == 0",
                                "reason": "division or remainder by zero",
                            },
                        }
                    ],
                },
            }
        ]
    }
    lines = _load().refusal_digest(report)
    assert len(lines) == 1, lines
    assert "Dimension.Y / 0 == 0 cannot be measured here" in lines[0]
    assert "division or remainder by zero" in lines[0]


def test_the_operator_is_read_back_as_an_author_writes_it():
    module = _load()
    guard = _cmp(
        {"source": "Dimension.Y", "value": 3},
        "ge",
        {"source": "floor", "value": 6},
        False,
        blocks=3,
    )
    report = {
        "rows": [
            {
                "id": "short",
                "refusal": {
                    "symbol": "wing",
                    "alternatives": [{"index": 1, "guard": guard}],
                },
            }
        ]
    }
    assert "Dimension.Y >= floor" in module.refusal_digest(report)[0]


def _sized(program, regions, errors=()):
    return {
        "program": program,
        "rows": [
            {"id": f"c{i}", "region": list(r), **({"error": "refused"} if i in errors else {})}
            for i, r in enumerate(regions)
        ],
    }


def test_a_page_whose_candidates_differ_in_size_says_it_cannot_show_that():
    # Every cell is scaled to fill its own thumbnail, so length does not read
    # across the page. The driver states it rather than leaving the owner to
    # infer it from a picture that cannot show it.
    module = _load()
    notes = module.scale_notes([_sized("bell:gate-ward", [(20, 10, 84), (20, 10, 104)])])
    assert len(notes) == 1, notes
    assert "20x10x84 .. 20x10x104" in notes[0]
    assert "proportion, not size" in notes[0]


def test_a_page_whose_candidates_are_all_one_size_gets_no_note():
    module = _load()
    assert module.scale_notes([_sized("bell:hall-keep", [(20, 12, 60), (20, 12, 60)])]) == []


def test_a_refused_candidates_box_is_not_a_size_the_page_shows():
    # It was never rendered, so it cannot be what makes the page misleading.
    module = _load()
    report = _sized("bell:cliff-road", [(19, 6, 24), (99, 6, 24)], errors={1})
    assert module.scale_notes([report]) == []
