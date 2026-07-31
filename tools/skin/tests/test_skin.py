"""Determinism + contract tests for the skin toolchain (spec-0009 acceptance)."""

import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from delve_skin.compose import CastEntry, compose_png_bytes, compose_skin  # noqa: E402
from delve_skin.preview import PREVIEW_ANGLES, render_previews  # noqa: E402

FIXTURE = Path(__file__).parent / "fixtures" / "sample.cast.json"


def _entry():
    data = json.loads(FIXTURE.read_text())
    return CastEntry.from_dict(data["skins"][0])


def test_skin_is_64x64_rgba():
    img = compose_skin(_entry())
    assert img.size == (64, 64)
    assert img.mode == "RGBA"


def test_composition_is_byte_deterministic():
    a = compose_png_bytes(_entry())
    b = compose_png_bytes(_entry())
    assert a == b, "same cast entry must yield byte-identical PNG (ADR-0006)"


def test_previews_deterministic_four_angles(tmp_path):
    img = compose_skin(_entry())
    first = render_previews(img, tmp_path / "a", "s")
    second = render_previews(img, tmp_path / "b", "s")
    assert len(first) == len(PREVIEW_ANGLES) == 4
    for p, q in zip(sorted(first), sorted(second)):
        assert p.read_bytes() == q.read_bytes(), "previews must be deterministic"


def test_model_is_mandatory():
    with pytest.raises(ValueError, match="model"):
        CastEntry.from_dict({"texture_id": "x", "palette": {}})


def test_bad_model_rejected():
    with pytest.raises(ValueError, match="wide|slim"):
        CastEntry.from_dict({"texture_id": "x", "model": "chunky", "palette": {}})


def test_slim_not_silently_distorted():
    e = CastEntry.from_dict({"texture_id": "x", "model": "slim", "palette": {}})
    with pytest.raises(NotImplementedError):
        compose_skin(e)
