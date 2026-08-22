"""Unit tests for tools/refscore.py (contact-sheet similarity scoring).

Two things are proven here, and the second is the one that matters.

1. The plumbing: config resolution and its hard errors, the capability refusals
   (a backend that cannot honour a flag says so instead of measuring something
   else), the stub backend's determinism, and `--dry-run` writing nothing. No
   test may touch the network or import a model — `conftest.py` blocks
   `urlopen`, and the real backends are lazily imported so nothing here loads
   PyTorch.

2. **The score RANKS; it never GATES** (spec-0028 §3). This tool's
   half of that is a *shape* obligation: it emits exactly one score per
   candidate and no verdict of any kind. A test asserts the emitted document
   scores every candidate and carries no threshold/keep/reject surface, so the
   drift — "just drop the ones below 0.2 here, the sheet will be cleaner" —
   lands as a red rather than as a shorter page nobody counted.
"""

import importlib.util
import json
import sys
from pathlib import Path

import pytest

TOOL = Path(__file__).resolve().parents[1] / "refscore.py"


def _load():
    spec = importlib.util.spec_from_file_location("refscore", TOOL)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["refscore"] = mod
    spec.loader.exec_module(mod)
    return mod


t = _load()


# ---------------------------------------------------------------------------
# fixtures
# ---------------------------------------------------------------------------


def _png(path: Path, byte: int) -> Path:
    """A file the tool only ever hashes — the stub backend reads bytes, not
    pixels, and nothing else in these tests decodes an image."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(bytes([byte]) * 64)
    return path


@pytest.fixture
def tree(tmp_path):
    """A contact-sheet manifest plus the candidate images it names."""
    renders = tmp_path / "renders"
    ids = ["alpha", "bravo", "charlie"]
    cells = []
    for i, cid in enumerate(ids):
        image = _png(renders / cid / f"{cid}-ext-se.png", 10 + i)
        cells.append({"rank": i + 1, "row": 0, "col": i, "id": cid,
                      "image": str(image), "score": None})
    sheet = tmp_path / "sheet.json"
    sheet.write_text(json.dumps({
        "schema": "delvewright.contact-sheet/1",
        "title": "t", "source": str(renders), "layout": "per-candidate-dir",
        "shot": None, "columns": 2, "thumb": 256,
        "rank_source": None, "rank_only_never_gates": True,
        "binding": {"candidates": 3, "scored": 0, "unscored": ids,
                    "unmatched_score_rows": []},
        "cells": cells,
    }))
    reference = _png(tmp_path / "reference.png", 99)
    return {"sheet": sheet, "reference": reference, "ids": ids, "root": tmp_path}


def run(args, capsys=None):
    return t.main([str(a) for a in args])


# ---------------------------------------------------------------------------
# the ruling: rank-only
# ---------------------------------------------------------------------------


def test_every_candidate_is_scored_and_nothing_is_ever_dropped(tree):
    out = tree["root"] / "scores.json"
    assert run(["--sheet", tree["sheet"], "--reference", tree["reference"],
                "--backend", "stub", "-o", out]) == 0
    doc = json.loads(out.read_text())
    scored = [row["id"] for row in doc["scores"]]
    assert scored == tree["ids"], "one score per candidate, in candidate order"
    assert len(doc["scores"]) == len(tree["ids"])
    assert doc["rank_only_never_gates"] is True


def test_the_document_carries_no_verdict_surface(tree):
    """No threshold, no keep/reject, no rank — only measurements.

    A gate cannot be smuggled in as data either: the moment this file could say
    "rejected", the sheet would have a filter it did not compute and could not
    refuse. The sheet's own DW0725 guard covers the ordering; this covers the
    input to it.
    """
    out = tree["root"] / "scores.json"
    assert run(["--sheet", tree["sheet"], "--reference", tree["reference"],
                "--backend", "stub", "-o", out]) == 0
    doc = json.loads(out.read_text())
    forbidden = {"threshold", "min_score", "cutoff", "keep", "reject", "excluded",
                 "filtered", "passed", "rank", "selected", "winner"}
    assert not (forbidden & set(doc)), f"verdict key in the document: {set(doc) & forbidden}"
    for row in doc["scores"]:
        assert set(row) == {"id", "image", "score"}, row


def test_the_sort_direction_is_stated_never_left_to_the_reader(tree):
    out = tree["root"] / "scores.json"
    assert run(["--sheet", tree["sheet"], "--reference", tree["reference"],
                "--backend", "stub", "-o", out]) == 0
    assert json.loads(out.read_text())["higher_is_better"] is True


# ---------------------------------------------------------------------------
# the stub backend
# ---------------------------------------------------------------------------


def test_stub_is_deterministic_and_distinguishes_candidates(tree):
    a, b = tree["root"] / "a.json", tree["root"] / "b.json"
    for out in (a, b):
        assert run(["--sheet", tree["sheet"], "--reference", tree["reference"],
                    "--backend", "stub", "-o", out]) == 0
    assert a.read_text() == b.read_text(), "the same inputs must score the same"
    values = [row["score"] for row in json.loads(a.read_text())["scores"]]
    assert len(set(values)) == 3, (
        "a stub that gave every candidate the same number would exercise the loop "
        "while proving nothing about the ordering"
    )
    assert all(0.0 <= v < 1.0 for v in values)


def test_stub_announces_itself_on_the_artifact(tree):
    out = tree["root"] / "scores.json"
    assert run(["--sheet", tree["sheet"], "--reference", tree["reference"],
                "--backend", "stub", "-o", out]) == 0
    doc = json.loads(out.read_text())
    assert doc["backend"] == "stub"
    assert "NOT a similarity measure" in doc["note"]


def test_dry_run_writes_nothing(tree, capsys):
    out = tree["root"] / "never.json"
    assert run(["--sheet", tree["sheet"], "--reference", tree["reference"],
                "--backend", "stub", "--dry-run", "-o", out]) == 0
    assert not out.exists()
    printed = capsys.readouterr().out
    assert "candidates : 3" in printed
    assert "never gates" in printed


# ---------------------------------------------------------------------------
# capability refusals — a flag the backend cannot honour is never dropped
# ---------------------------------------------------------------------------


def test_vqascore_without_a_prompt_is_refused(tree, capsys):
    assert run(["--sheet", tree["sheet"], "--backend", "vqascore",
                "-o", tree["root"] / "x.json"]) == 1
    assert "TEXT-conditioned" in capsys.readouterr().err


def test_vqascore_refuses_a_reference_it_would_ignore(tree, capsys):
    """The whole reason this repo distrusts silently-dropped inputs: a reference
    image handed to a text-conditioned metric produces a number the reference
    had no part in, and nothing anywhere would say so."""
    assert run(["--sheet", tree["sheet"], "--backend", "vqascore",
                "--prompt", "a sea-gate barbican", "--reference", tree["reference"],
                "-o", tree["root"] / "x.json"]) == 1
    assert "would IGNORE" in capsys.readouterr().err


def test_an_image_metric_without_a_reference_is_refused(tree, capsys):
    assert run(["--sheet", tree["sheet"], "--backend", "open-clip",
                "-o", tree["root"] / "x.json"]) == 1
    assert "--reference is required" in capsys.readouterr().err


def test_a_missing_reference_file_is_refused(tree, capsys):
    assert run(["--sheet", tree["sheet"], "--backend", "stub",
                "--reference", tree["root"] / "nope.png",
                "-o", tree["root"] / "x.json"]) == 1
    assert "not found" in capsys.readouterr().err


def test_a_missing_candidate_image_is_refused(tree, capsys):
    (tree["root"] / "renders" / "bravo" / "bravo-ext-se.png").unlink()
    assert run(["--sheet", tree["sheet"], "--reference", tree["reference"],
                "--backend", "stub", "-o", tree["root"] / "x.json"]) == 1
    assert "candidate image(s) not found" in capsys.readouterr().err


def test_a_foreign_manifest_is_refused(tree, tmp_path):
    other = tmp_path / "other.json"
    other.write_text(json.dumps({"schema": "something/else", "cells": []}))
    with pytest.raises(SystemExit) as exc:
        run(["--sheet", other, "--reference", tree["reference"],
             "--backend", "stub", "-o", tmp_path / "x.json"])
    assert "delvewright.contact-sheet/1" in str(exc.value)


# ---------------------------------------------------------------------------
# a missing real backend NEVER becomes the stub
# ---------------------------------------------------------------------------


def test_an_uninstalled_backend_errors_and_does_not_fall_back(tree, monkeypatch, capsys):
    """The failure this guards is silent, not loud: a stub number in a file
    labelled `open-clip` would order the owner's page by a hash and look exactly
    like a measurement."""
    def unavailable(*_args, **_kwargs):
        raise t.BackendUnavailable("No module named 'open_clip'")

    monkeypatch.setitem(t.SCORERS, "open-clip", unavailable)
    out = tree["root"] / "x.json"
    assert run(["--sheet", tree["sheet"], "--reference", tree["reference"],
                "--backend", "open-clip", "-o", out]) == 4
    err = capsys.readouterr().err
    assert "pip install open_clip_torch" in err
    assert "Not falling back to the stub" in err
    assert not out.exists()


# ---------------------------------------------------------------------------
# config
# ---------------------------------------------------------------------------


def _cfg(tmp_path: Path, body: str) -> Path:
    p = tmp_path / "delvewright.local.toml"
    p.write_text(body)
    return p


def test_absent_config_is_not_an_error_when_backend_is_a_flag(tmp_path):
    assert t.load_config(tmp_path / "nothing.toml") == {}


def test_a_section_less_config_is_not_an_error(tmp_path):
    assert t.load_config(_cfg(tmp_path, '[i18n]\nprovider = "x"\n')) == {}


def test_an_inline_api_key_is_a_hard_error(tmp_path):
    with pytest.raises(t.ConfigError, match="never live in a file"):
        t.load_config(_cfg(tmp_path, '[refscore]\napi_key = "sk-live"\n'))


def test_an_unknown_backend_is_a_hard_error(tmp_path):
    with pytest.raises(t.ConfigError, match="not supported"):
        t.load_config(_cfg(tmp_path, '[refscore]\nbackend = "clip-ish"\n'))


def test_config_supplies_the_backend_and_the_flag_overrides_it(tree, monkeypatch):
    monkeypatch.setattr(t, "load_config", lambda *_: {"backend": "stub", "model": "cfg-model"})
    out = tree["root"] / "scores.json"
    assert run(["--sheet", tree["sheet"], "--reference", tree["reference"], "-o", out]) == 0
    assert json.loads(out.read_text())["model"] == "cfg-model"
    assert run(["--sheet", tree["sheet"], "--reference", tree["reference"],
                "--model", "flag-model", "-o", out]) == 0
    assert json.loads(out.read_text())["model"] == "flag-model"


def test_no_backend_anywhere_says_what_to_add(tree, monkeypatch, capsys):
    monkeypatch.setattr(t, "load_config", lambda *_: {})
    assert run(["--sheet", tree["sheet"], "--reference", tree["reference"],
                "-o", tree["root"] / "x.json"]) == 2
    err = capsys.readouterr().err
    assert "delvewright.local.toml" in err
    assert "--backend" in err


def test_explicit_images_take_their_ids_from_the_stems(tmp_path):
    a = _png(tmp_path / "first.png", 1)
    b = _png(tmp_path / "second.png", 2)
    assert t.candidates_from_images([a, b]) == [("first", a), ("second", b)]


def test_every_declared_backend_has_a_scorer_and_an_install_line():
    """A backend named in the help but not implemented would be a flag that
    accepts and then does nothing."""
    assert set(t.BACKENDS) == set(t.SCORERS)
    for name, spec in t.BACKENDS.items():
        assert spec["needs_reference_image"] or spec["needs_prompt"], name
        if name != t.STUB:
            assert spec["install"], f"{name} must say how to install itself"
