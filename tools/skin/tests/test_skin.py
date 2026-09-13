"""Determinism + contract tests for the skin toolchain (spec-0009 acceptance).

The wardrobe tests do not assert "the bytes moved" and stop there. Changing any
wardrobe axis also changes how much of the seeded stream the composer draws, so
the bytes move even for an axis that paints nothing -- a green test bound to the
rng rather than to the garment. Every garment is therefore read back off the
composed skin at the part/face level, at the rows it is supposed to occupy and
at the rows it is supposed to leave alone.
"""

import json
import sys
from pathlib import Path

import pytest
from PIL import Image
from skinpy import Skin

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from delve_skin.catalog import catalog_card  # noqa: E402
from delve_skin.cli import _entry_surface_help  # noqa: E402
from delve_skin.compose import (  # noqa: E402
    ENTRY_KEYS,
    PALETTE_KEYS,
    CastEntry,
    compose_png_bytes,
    compose_skin,
)
from delve_skin.preview import PREVIEW_ANGLES, render_previews  # noqa: E402
from delve_skin.wardrobe import (  # noqa: E402
    FACIAL_HAIR,
    FOOTWEAR,
    LEGS,
    SLEEVES,
    Wardrobe,
)

FIXTURES = Path(__file__).parent / "fixtures"
FIXTURE = FIXTURES / "sample.cast.json"
WARDROBE_FIXTURE = FIXTURES / "wardrobe.cast.json"
GOLDEN = FIXTURES / "golden"

#: Every cast sheet in `fixtures/`, derived rather than listed, so a sheet added
#: beside them is swept with nothing here to edit.
CAST_SHEETS = sorted(FIXTURES.glob("*.cast.json"))

# Palette colours the wardrobe fixture declares, as RGB.
JACKET = (0x2F, 0x44, 0x36)
TROUSER = (0x3B, 0x3F, 0x46)
TROUSER_SHADOW = (0x29, 0x2C, 0x31)
BOOT = (0x33, 0x25, 0x1A)
GUIDE_SKIN = (0xC8, 0x9A, 0x74)
GUIDE_HAIR = (0x5E, 0x43, 0x30)  # `beard` falls back to `hair` in that palette


def _entries(path: Path):
    data = json.loads(path.read_text(encoding="utf-8"))
    return [CastEntry.from_dict(r) for r in data["skins"]]


def _entry():
    return _entries(FIXTURE)[0]


def _dressed(**wardrobe) -> CastEntry:
    """The wardrobe fixture, re-dressed. Its palette names every garment."""
    row = dict(json.loads(WARDROBE_FIXTURE.read_text(encoding="utf-8"))["skins"][0])
    row["wardrobe"] = {**row["wardrobe"], **wardrobe}
    return CastEntry.from_dict(row)


def _read_back(entry: CastEntry) -> Skin:
    """Compose, then address the result the way the composer addressed it."""
    img = compose_skin(entry)
    if img.mode != "RGBA":
        img = img.convert("RGBA")
    return Skin.from_image(img)


def _at(skin: Skin, part: str, face: str, x: int, y: int):
    f = skin.get_body_part_for_id(part).get_face_for_id(face)
    return tuple(int(c) for c in f.get_color(x, y))


def _near(actual, expected, tol: int = 8) -> bool:
    """Within the composer's cloth/skin jitter of a palette colour."""
    return all(abs(a - e) <= tol for a, e in zip(actual[:3], expected[:3]))


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


# --- the bytes every fixture sheet composes are pinned ----------------------


def test_every_fixture_sheet_composes_its_golden_bytes():
    """The anchor: a composer change that moves one pixel reds here by name.

    Regenerate deliberately, never to get green:
        python -m delve_skin build tests/fixtures/<name>.cast.json \
            --out-dir tests/fixtures/golden
    """
    assert CAST_SHEETS, "no cast sheets in fixtures/ -- this test binds nothing"
    checked = 0
    for sheet in CAST_SHEETS:
        for entry in _entries(sheet):
            golden = GOLDEN / f"{entry.texture_id}.png"
            assert golden.exists(), f"{sheet.name}:{entry.texture_id} has no golden"
            assert compose_png_bytes(entry) == golden.read_bytes(), (
                f"{sheet.name}:{entry.texture_id} no longer composes its golden bytes"
            )
            checked += 1
    assert checked == 2, f"expected 2 pinned entries, pinned {checked}"


def test_a_sheet_that_names_no_wardrobe_gets_the_default_one():
    assert "wardrobe" not in json.loads(FIXTURE.read_text())["skins"][0]
    assert _entry().wardrobe == Wardrobe()


# --- each garment is read back where it is supposed to be -------------------


@pytest.mark.parametrize("part", ["left_arm", "right_arm"])
def test_long_sleeve_reaches_the_wrist_and_stops(part):
    skin = _read_back(_dressed(sleeves="long"))
    for y in (3, 6, 11):
        assert _near(_at(skin, part, "front", 1, y), JACKET), f"no sleeve at y={y}"
    # The hand is not painted over: rows 0-1 stay skin (row 0 is its shadow).
    assert _near(_at(skin, part, "front", 1, 1), GUIDE_SKIN)


@pytest.mark.parametrize("part", ["left_arm", "right_arm"])
def test_short_sleeve_leaves_the_forearm_bare(part):
    skin = _read_back(_dressed(sleeves="short"))
    assert _near(_at(skin, part, "front", 1, 9), JACKET)
    for y in (1, 4, 6):
        assert _near(_at(skin, part, "front", 1, y), GUIDE_SKIN), f"sleeve at y={y}"


@pytest.mark.parametrize("part", ["left_arm", "right_arm"])
def test_bare_sleeve_puts_no_cloth_on_the_arm(part):
    skin = _read_back(_dressed(sleeves="bare"))
    for y in range(1, 12):
        assert not _near(_at(skin, part, "front", 1, y), JACKET), f"cloth at y={y}"
    assert _near(_at(skin, part, "up", 1, 1), GUIDE_SKIN), "a bare shoulder is skin"


@pytest.mark.parametrize("part", ["left_leg", "right_leg"])
def test_full_legs_are_trousers_to_the_ankle(part):
    skin = _read_back(_dressed(legs="full", footwear="none"))
    for y in (0, 4, 8, 11):
        assert _near(_at(skin, part, "front", 1, y), TROUSER), f"no trouser at y={y}"


@pytest.mark.parametrize("part", ["left_leg", "right_leg"])
def test_short_legs_leave_the_shin_bare(part):
    skin = _read_back(_dressed(legs="short", footwear="none"))
    assert _near(_at(skin, part, "front", 1, 11), TROUSER)
    for y in (2, 4, 8):
        assert _near(_at(skin, part, "front", 1, y), GUIDE_SKIN), f"cloth at y={y}"


@pytest.mark.parametrize("part", ["left_leg", "right_leg"])
def test_bare_legs_carry_no_leg_garment(part):
    skin = _read_back(_dressed(legs="bare", footwear="none"))
    for y in range(0, 12):
        assert not _near(_at(skin, part, "front", 1, y), TROUSER), f"cloth at y={y}"


def test_the_leg_garment_has_its_own_colour():
    """`legwear` is separate from `tunic`: the trousers move, the jacket does not."""
    skin = _read_back(_dressed(legs="full", footwear="none"))
    assert _near(_at(skin, "left_leg", "front", 1, 8), TROUSER)
    assert not _near(_at(skin, "left_leg", "front", 1, 8), JACKET)
    assert _near(_at(skin, "torso", "back", 4, 8), JACKET)


def test_a_leg_garment_with_no_colour_of_its_own_is_cut_from_the_torso_cloth():
    """Which is what every sheet written before `legwear` existed meant."""
    row = json.loads(WARDROBE_FIXTURE.read_text())["skins"][0]
    palette = {k: v for k, v in row["palette"].items() if not k.startswith("legwear")}
    entry = CastEntry.from_dict(
        {**row, "palette": palette, "wardrobe": {**row["wardrobe"], "footwear": "none"}}
    )
    assert _near(_at(_read_back(entry), "left_leg", "front", 1, 8), JACKET)


def test_footwear_height_is_what_the_axis_says():
    """Every named height, read back as the row it reaches. Flat, so exact."""
    for name, span in FOOTWEAR.items():
        skin = _read_back(_dressed(footwear=name, legs="bare"))
        if span is None:
            for y in range(0, 12):
                assert _at(skin, "left_leg", "front", 1, y)[:3] != BOOT
            continue
        top = span[1]
        assert _at(skin, "left_leg", "front", 1, top)[:3] == BOOT, f"{name} stops short"
        assert _at(skin, "left_leg", "front", 1, top + 1)[:3] != BOOT, f"{name} too tall"


def test_a_tall_boot_covers_the_knee_shadow():
    """The knee shadow is painted before the footwear, not over it."""
    skin = _read_back(_dressed(footwear="tall_boot", legs="bare"))
    assert _at(skin, "left_leg", "front", 1, 5)[:3] == BOOT


def test_trousers_get_a_cloth_knee_shadow_not_a_skin_one():
    bare = _read_back(_dressed(legs="bare", footwear="none"))
    trousered = _read_back(_dressed(legs="full", footwear="none"))
    knee_bare = _at(bare, "left_leg", "front", 1, 5)
    knee_cloth = _at(trousered, "left_leg", "front", 1, 5)
    assert knee_bare != knee_cloth
    assert _near(knee_cloth, TROUSER_SHADOW)


def test_facial_hair_none_leaves_the_chin_clean():
    skin = _read_back(_dressed(facial_hair="none"))
    for y in range(0, 4):
        assert not _near(_at(skin, "head", "front", 3, y), GUIDE_HAIR), f"hair at y={y}"
    assert _near(_at(skin, "head", "down", 3, 3), GUIDE_SKIN), "chin underside"


def test_facial_hair_moustache_is_one_row():
    skin = _read_back(_dressed(facial_hair="moustache"))
    assert _near(_at(skin, "head", "front", 3, 3), GUIDE_HAIR), "no moustache"
    for y in (0, 1, 2):
        assert not _near(_at(skin, "head", "front", 3, y), GUIDE_HAIR), f"beard at y={y}"


def test_facial_hair_beard_covers_chin_jaw_and_underside():
    skin = _read_back(_dressed(facial_hair="beard"))
    for y in (0, 1, 2, 3):
        assert _near(_at(skin, "head", "front", 3, y), GUIDE_HAIR), f"no beard at y={y}"
    assert _near(_at(skin, "head", "down", 3, 3), GUIDE_HAIR, tol=9), "no underside"


# --- mistakes are refused where they are written ----------------------------


def test_unknown_wardrobe_key_refused():
    with pytest.raises(ValueError, match="unknown wardrobe key"):
        CastEntry.from_dict(
            {"texture_id": "x", "model": "wide", "wardrobe": {"sleeve": "long"}}
        )


def test_unknown_wardrobe_value_refused():
    with pytest.raises(ValueError, match="wardrobe.sleeves must be one of"):
        CastEntry.from_dict(
            {"texture_id": "x", "model": "wide", "wardrobe": {"sleeves": "3/4"}}
        )


def test_wardrobe_that_is_not_an_object_refused():
    with pytest.raises(ValueError, match="'wardrobe' must be an object"):
        CastEntry.from_dict({"texture_id": "x", "model": "wide", "wardrobe": ["long"]})


def test_unknown_entry_field_refused():
    """A misspelled `wardrobe` must not compose the default costume in silence."""
    with pytest.raises(ValueError, match="unknown field"):
        CastEntry.from_dict(
            {"texture_id": "x", "model": "wide", "wardrope": {"sleeves": "long"}}
        )


def test_unknown_palette_key_refused():
    with pytest.raises(ValueError, match="unknown palette key"):
        CastEntry.from_dict(
            {"texture_id": "x", "model": "wide", "palette": {"trousers": "#1a2b3c"}}
        )


# --- the surface a creator reads is the surface the code enforces -----------


def test_help_names_every_axis_value_and_every_key():
    help_text = _entry_surface_help()
    for key in ENTRY_KEYS:
        assert key in help_text, f"--help does not name entry field {key!r}"
    for key in PALETTE_KEYS:
        assert key in help_text, f"--help does not name palette key {key!r}"
    for axis in (SLEEVES, LEGS, FOOTWEAR):
        for value in axis:
            assert value in help_text, f"--help does not name wardrobe value {value!r}"
    for value in FACIAL_HAIR:
        assert value in help_text, f"--help does not name facial_hair {value!r}"


def test_readme_documents_every_axis_value_and_palette_key():
    readme = (Path(__file__).resolve().parents[1] / "README.md").read_text(
        encoding="utf-8"
    )
    for name, axis in (("sleeves", SLEEVES), ("legs", LEGS), ("footwear", FOOTWEAR)):
        assert f"`{name}`" in readme, f"README does not document wardrobe.{name}"
        for value in axis:
            assert f"`{value}`" in readme, f"README does not document {name}={value!r}"
    assert "`facial_hair`" in readme
    for value in FACIAL_HAIR:
        assert f"`{value}`" in readme, f"README does not document facial_hair={value!r}"
    for key in PALETTE_KEYS:
        assert f"`{key}`" in readme, f"README does not document palette key {key!r}"


def test_catalog_card_records_the_wardrobe():
    card = catalog_card(_entries(WARDROBE_FIXTURE)[0], b"", [])
    assert card["tags"]["wardrobe"] == {
        "sleeves": "long",
        "legs": "full",
        "footwear": "boot",
        "facial_hair": "none",
    }


def test_the_wardrobe_fixture_is_a_costume_the_old_composer_could_not_make():
    """The gap this surface closes, asserted rather than claimed."""
    w = _entries(WARDROBE_FIXTURE)[0].wardrobe
    default = Wardrobe()
    for axis in ("sleeves", "legs", "footwear", "facial_hair"):
        assert getattr(w, axis) != getattr(default, axis), f"{axis} is still the default"
    assert isinstance(compose_skin(_entries(WARDROBE_FIXTURE)[0]), Image.Image)
