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
    COLLAR,
    FACIAL_HAIR,
    FOOTWEAR,
    GREYING,
    HAIR,
    LEGS,
    SHOULDER_HAIR,
    SLEEVES,
    Wardrobe,
)

FIXTURES = Path(__file__).parent / "fixtures"
FIXTURE = FIXTURES / "sample.cast.json"
WARDROBE_FIXTURE = FIXTURES / "wardrobe.cast.json"
HAIR_FIXTURE = FIXTURES / "hair.cast.json"
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


# --- the pixels every fixture sheet composes are pinned ---------------------


def test_every_fixture_sheet_composes_its_golden_pixels():
    """The anchor: a composer change that moves one pixel reds here by name.

    The golden is compared as PIXELS, not as file bytes, because those are two
    different questions and only one of them is about this tool. Composition is
    deterministic and portable: the same cast entry yields the same 64x64 image
    everywhere. PNG *serialisation* is not. Pillow hands the scanlines to
    whatever zlib it is linked against, and deflate output differs between zlib
    builds -- measured with the same Pillow 12.3.0 and numpy 2.5.3 on either
    side, varying only zlib: macOS (1.2.12) and Linux (1.3.1) agree on every
    pixel of both fixtures and disagree on the file at compress_level 1, 6 and 9
    alike. Pinning file bytes therefore pins the zlib build of whoever last
    regenerated and reds on every other machine while the composer is innocent,
    which is exactly what it did.

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
            with Image.open(golden) as g:
                want = g.convert("RGBA").tobytes()
            got = compose_skin(entry).convert("RGBA").tobytes()
            assert got == want, (
                f"{sheet.name}:{entry.texture_id} no longer composes its golden pixels"
            )
            checked += 1
    assert checked == 3, f"expected 3 pinned entries, pinned {checked}"


def test_the_png_file_is_byte_stable_within_one_build():
    """The half of determinism that IS a property of the file, stated on its own.

    Within one zlib build the same entry serialises to the same bytes, and an
    intervening composition of a different entry does not move them. Across
    builds only the pixels carry -- see the golden test.
    """
    first = compose_png_bytes(_entry())
    other = compose_png_bytes(_entries(WARDROBE_FIXTURE)[0])
    assert compose_png_bytes(_entry()) == first, "an intervening entry moved the bytes"
    assert other != first


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


def test_nothing_painted_after_the_torso_erases_it():
    """The belt and the collar survive the parts composed after the torso.

    The legs used to repaint the whole torso FRONT flat on their way past --
    `noise(..., amount=0)` returns before drawing, so it consumed no rng and was
    not the ordering guard it was written as; what it did was erase the belt,
    the hem shadow, the cloth texture and the collar on every skin this tool has
    ever made. It is a face carrying one colour where the opposite face carries
    twenty, so that is what this asks.
    """
    entry = _entries(WARDROBE_FIXTURE)[0]
    skin = _read_back(entry)
    front = [_at(skin, "torso", "front", x, y) for x in range(8) for y in range(12)]
    back = [_at(skin, "torso", "back", x, y) for x in range(8) for y in range(12)]
    assert len(set(front)) > 1, "the torso front is flat -- something repainted it"
    assert len(set(front)) >= len(set(back)) - 4, "the front lost most of its detail"
    belt = (0x1B, 0x24, 0x1D)
    assert any(_near(c, belt, tol=0) for c in front), "the belt is not on the front"
    assert any(_near(c, GUIDE_SKIN, tol=0) for c in front), "no collar at the neck"


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


# --- the head: hair length, the face it frames, the collar, the grey --------

STEWARD_HAIR = (0x7D, 0x76, 0x69)
STEWARD_GREY = (0xA9, 0xA2, 0x9A)
STEWARD_CUT = (0x56, 0x4F, 0x42)
STEWARD_SKIN = (0xD0, 0xA1, 0x7C)
STEWARD_JACKET = (0x39, 0x49, 0x5E)


def _steward(**wardrobe) -> CastEntry:
    """The hair fixture, re-styled. Its palette names hair, its grey and its cut."""
    row = dict(json.loads(HAIR_FIXTURE.read_text(encoding="utf-8"))["skins"][0])
    row["wardrobe"] = {**row["wardrobe"], **wardrobe}
    return CastEntry.from_dict(row)


@pytest.mark.parametrize("side", ["left", "right"])
def test_every_hair_length_reaches_the_row_it_names(side):
    """Each named length, read back on the side of the head it comes down."""
    for name, span in HAIR.items():
        skin = _read_back(_steward(hair=name, greying="none"))
        if span is None:
            for y in range(0, 8):
                assert not _near(_at(skin, "head", side, 3, y), STEWARD_HAIR, tol=12), (
                    f"bald, but hair at y={y}"
                )
            continue
        y0, y1 = span
        # A length long enough to show an edge spends its bottom row on the cut
        # line, so the hair proper starts one above it.
        probe = y0 + 1 if Wardrobe(hair=name).hair_has_a_cut_line() else y0
        assert _near(_at(skin, "head", side, 3, probe), STEWARD_HAIR, tol=14), (
            f"{name} does not reach y={probe}"
        )
        assert _near(_at(skin, "head", side, 3, y1), STEWARD_HAIR, tol=14), (
            f"{name} does not reach its own top row y={y1}"
        )
        if y0 > 0:
            assert _near(_at(skin, "head", side, 3, y0 - 1), STEWARD_SKIN), (
                f"{name} hangs below y={y0}"
            )


def test_a_bald_head_carries_no_hair_anywhere():
    skin = _read_back(_steward(hair="bald", greying="none"))
    for face in ("up", "back", "front", "left", "right"):
        for x in range(8):
            for y in range(8):
                assert not _near(_at(skin, "head", face, x, y), STEWARD_HAIR, tol=10), (
                    f"hair on a bald head at {face} ({x},{y})"
                )


def test_hair_past_the_ear_frames_the_face_and_shorter_hair_does_not():
    """The four rows of blank skin a clean-shaven head used to show."""
    framed = _read_back(_steward(hair="jaw", greying="none"))
    for y in (2, 4, 7):
        for x in (0, 7):
            assert _near(_at(framed, "head", "front", x, y), STEWARD_HAIR, tol=14), (
                f"no frame at front ({x},{y})"
            )
    # The face itself is still a face: skin between the frames.
    assert _near(_at(framed, "head", "front", 3, 2), STEWARD_SKIN)
    cropped = _read_back(_steward(hair="short", greying="none"))
    assert _near(_at(cropped, "head", "front", 0, 2), STEWARD_SKIN), "short hair frames"


def test_hair_long_enough_to_show_an_edge_gets_a_cut_line():
    for name in ("jaw", "long"):
        skin = _read_back(_steward(hair=name, greying="none"))
        y0 = HAIR[name][0]
        assert _near(_at(skin, "head", "left", 3, y0), STEWARD_CUT), f"{name}: no cut line"
    for name in ("crop", "short"):
        skin = _read_back(_steward(hair=name, greying="none"))
        y0 = HAIR[name][0]
        assert not _near(_at(skin, "head", "left", 3, y0), STEWARD_CUT), (
            f"{name}: a cut line where there is no edge to read"
        )


def test_long_hair_falls_onto_the_shoulders_and_shorter_hair_does_not():
    """The one place hair goes that is not the head."""
    sy0, sy1 = SHOULDER_HAIR
    long_ = _read_back(_steward(hair="long", greying="none"))
    assert _near(_at(long_, "torso", "back", 4, sy1), STEWARD_HAIR, tol=14)
    assert _near(_at(long_, "torso", "back", 4, sy0), STEWARD_CUT)
    assert _near(_at(long_, "torso", "back", 4, sy0 - 1), STEWARD_JACKET, tol=12), (
        "hair below the shoulder rows"
    )
    jaw = _read_back(_steward(hair="jaw", greying="none"))
    assert not _near(_at(jaw, "torso", "back", 4, sy1), STEWARD_HAIR, tol=12)


def test_a_closed_collar_puts_cloth_at_the_throat_and_an_open_one_skin():
    """No palette key could ever have done this: the V is painted from `skin`."""
    closed = _read_back(_steward(collar="closed"))
    open_ = _read_back(_steward(collar="open"))
    for x, y in ((3, 11), (4, 11), (3, 10), (4, 10), (4, 9)):
        assert _near(_at(open_, "torso", "front", x, y), STEWARD_SKIN), (
            f"open collar has no skin at ({x},{y})"
        )
        assert not _near(_at(closed, "torso", "front", x, y), STEWARD_SKIN), (
            f"closed collar still bare at ({x},{y})"
        )
        assert _near(_at(closed, "torso", "front", x, y), STEWARD_JACKET, tol=12)


def _grey_count(skin: Skin, part: str, face: str, x1: int, y1: int) -> int:
    return sum(
        1
        for x in range(x1)
        for y in range(y1)
        if _near(_at(skin, part, face, x, y), STEWARD_GREY, tol=6)
    )


def test_greying_reaches_the_hair_and_only_when_it_is_asked_to():
    """The gap that gave a twenty-year veteran brown hair."""
    none = _read_back(_steward(greying="none"))
    hair = _read_back(_steward(greying="hair"))
    beard_only = _read_back(_steward(greying="beard", facial_hair="beard"))
    assert _grey_count(none, "head", "up", 8, 8) == 0, "grey with nothing greying"
    assert _grey_count(hair, "head", "up", 8, 8) > 2, "greying=hair left the crown alone"
    assert _grey_count(beard_only, "head", "up", 8, 8) == 0, (
        "greying=beard reached the hair"
    )


def test_greying_both_reaches_hair_and_beard_at_once():
    both = _read_back(_steward(greying="both", facial_hair="beard"))
    assert _grey_count(both, "head", "up", 8, 8) > 2, "the crown is not greying"
    chin = [_at(both, "head", "front", x, y) for x in range(1, 7) for y in range(0, 3)]
    # The beard greys against `beard_grey`, which this palette derives from hair.
    assert len({c for c in chin}) > 1, "the beard is one flat colour"


def test_the_older_spelling_of_greying_still_means_the_beard():
    """No sheet written before this axis existed changes."""
    e = CastEntry.from_dict(
        {"texture_id": "x", "model": "wide", "features": {"greying": True}}
    )
    assert e.wardrobe.greying == "beard"
    assert e.wardrobe.greys_beard() and not e.wardrobe.greys_hair()
    plain = CastEntry.from_dict({"texture_id": "x", "model": "wide"})
    assert plain.wardrobe.greying == "none"


def test_saying_what_is_greying_twice_is_refused():
    """Two ways to say one thing, and the one that errors is the one to have."""
    with pytest.raises(ValueError, match="both set what is going grey"):
        CastEntry.from_dict(
            {
                "texture_id": "x",
                "model": "wide",
                "features": {"greying": True},
                "wardrobe": {"greying": "hair"},
            }
        )


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
    for axis in (SLEEVES, LEGS, FOOTWEAR, HAIR):
        for value in axis:
            assert value in help_text, f"--help does not name wardrobe value {value!r}"
    for group in (FACIAL_HAIR, COLLAR, GREYING):
        for value in group:
            assert value in help_text, f"--help does not name wardrobe value {value!r}"


def test_readme_documents_every_axis_value_and_palette_key():
    readme = (Path(__file__).resolve().parents[1] / "README.md").read_text(
        encoding="utf-8"
    )
    axes = (
        ("sleeves", SLEEVES), ("legs", LEGS), ("footwear", FOOTWEAR),
        ("hair", HAIR), ("facial_hair", FACIAL_HAIR), ("collar", COLLAR),
        ("greying", GREYING),
    )
    for name, axis in axes:
        assert f"`{name}`" in readme, f"README does not document wardrobe.{name}"
        for value in axis:
            assert f"`{value}`" in readme, f"README does not document {name}={value!r}"
    for key in PALETTE_KEYS:
        assert f"`{key}`" in readme, f"README does not document palette key {key!r}"


def test_catalog_card_records_the_wardrobe():
    card = catalog_card(_entries(WARDROBE_FIXTURE)[0], b"", [])
    assert card["tags"]["wardrobe"] == {
        "sleeves": "long",
        "legs": "full",
        "footwear": "boot",
        "hair": "short",
        "facial_hair": "none",
        "collar": "open",
        "greying": "none",
    }


def test_the_wardrobe_fixture_is_a_costume_the_old_composer_could_not_make():
    """The gap this surface closes, asserted rather than claimed."""
    w = _entries(WARDROBE_FIXTURE)[0].wardrobe
    default = Wardrobe()
    for axis in ("sleeves", "legs", "footwear", "facial_hair"):
        assert getattr(w, axis) != getattr(default, axis), f"{axis} is still the default"
    w = _entries(HAIR_FIXTURE)[0].wardrobe
    for axis in ("hair", "collar", "greying"):
        assert getattr(w, axis) != getattr(default, axis), f"{axis} is still the default"
    assert isinstance(compose_skin(_entries(WARDROBE_FIXTURE)[0]), Image.Image)
