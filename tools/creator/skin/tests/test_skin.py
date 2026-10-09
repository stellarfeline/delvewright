"""Determinism + contract tests for the skin toolchain (spec-0009 acceptance).

The wardrobe tests do not assert "the bytes moved" and stop there. Changing any
wardrobe axis also changes how much of the seeded stream the composer draws, so
the bytes move even for an axis that paints nothing -- a green test bound to the
rng rather than to the garment. Every garment is therefore read back off the
composed skin at the part/face level, at the rows it is supposed to occupy and
at the rows it is supposed to leave alone.
"""

import io
import json
import struct
import sys
import zlib
from pathlib import Path

import pytest
from PIL import Image
from skinpy import Skin

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from delve_skin.catalog import catalog_card  # noqa: E402
from delve_skin import png as png_mod  # noqa: E402
from delve_skin.cli import _entry_surface_help  # noqa: E402
from delve_skin.compose import (  # noqa: E402
    ENTRY_KEYS,
    FACE_BROW,
    FACE_CHIN,
    FACE_EYES,
    FACE_HAIRLINE,
    FACE_LIP,
    FACE_MOUTH,
    PALETTE_KEYS,
    CastEntry,
    compose_png_bytes,
    compose_skin,
)
from delve_skin import models  # noqa: E402
from delve_skin.cli import parts_table  # noqa: E402
from delve_skin.compose import compose_preview_skin, entities  # noqa: E402
from delve_skin.preview import PREVIEW_ANGLES, render_previews  # noqa: E402
from delve_skin.wardrobe import (  # noqa: E402
    COLLAR,
    FACIAL_HAIR,
    FOOTWEAR,
    GREYING,
    HAIR,
    HOOD,
    LEGS,
    OVERCOAT,
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


def test_slim_is_drawn_to_the_slim_arms():
    """A slim skin is composed on `player_slim`'s boxes: 3-pixel arms, so the
    wide arm's fourth column and back-face tail stay clear."""
    e = CastEntry.from_dict({"texture_id": "x", "model": "slim", "palette": {}})
    px = compose_skin(e).load()
    assert px[44 + 2, 25][3] and not px[55, 25][3], "the slim arm ends at 54"
    assert e.model_key() == "player_slim"


# --- the pixels every fixture sheet composes are pinned ---------------------


def test_every_fixture_sheet_composes_its_golden_file():
    """The anchor: the committed golden is the composed FILE, byte for byte.

    The PNG is written by `delve_skin.png` (stored deflate, no compressor), so
    its bytes are a function of the pixels alone and this comparison holds on
    every machine -- CI composes on Linux what was committed from wherever it
    was regenerated. The pixels are asserted on their own as well, so a red
    says which of the two moved: a composer change moves both, an encoder
    change moves only the file.

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
            assert compose_png_bytes(entry) == golden.read_bytes(), (
                f"{sheet.name}:{entry.texture_id} composes its golden pixels into "
                "a different file -- the encoder moved"
            )
            checked += 1
    assert checked == 11, f"expected 11 pinned entries, pinned {checked}"


def test_the_png_file_is_byte_stable_within_one_run():
    """An intervening composition of a different entry does not move the bytes."""
    first = compose_png_bytes(_entry())
    other = compose_png_bytes(_entries(WARDROBE_FIXTURE)[0])
    assert compose_png_bytes(_entry()) == first, "an intervening entry moved the bytes"
    assert other != first


def test_no_linked_compressor_writes_the_committed_file(monkeypatch):
    """The committed file reaches neither zlib's deflate nor Pillow's encoder.

    Either one would put the machine's zlib build back into the bytes, so both
    are made to raise and the composition must still produce the golden file.
    """
    import zlib

    def refuse(*_a, **_k):
        raise AssertionError("a linked compressor was reached")

    monkeypatch.setattr(zlib, "compress", refuse)
    monkeypatch.setattr(zlib, "compressobj", refuse)
    monkeypatch.setattr(Image.Image, "save", refuse)
    entry = _entry()
    assert compose_png_bytes(entry) == (GOLDEN / f"{entry.texture_id}.png").read_bytes()


def _stored_blocks(stream: bytes):
    """Walk a zlib stream that claims to be stored blocks; yield (bfinal, len)."""
    assert stream[:2] == b"\x78\x01"
    i = 2
    while True:
        header = stream[i]
        assert header & 0b110 == 0, f"block at {i} is not stored (BTYPE 00)"
        length, nlength = struct.unpack("<HH", stream[i + 1 : i + 5])
        assert length ^ 0xFFFF == nlength
        yield header & 1, length
        i += 5 + length
        if header & 1:
            break
    assert len(stream) == i + 4, "trailing bytes after the Adler-32"


def _idat(png: bytes) -> bytes:
    assert png[:8] == png_mod.SIGNATURE
    i, kinds, idat = 8, [], b""
    while i < len(png):
        (n,) = struct.unpack(">I", png[i : i + 4])
        kind, data = png[i + 4 : i + 8], png[i + 8 : i + 8 + n]
        (crc,) = struct.unpack(">I", png[i + 8 + n : i + 12 + n])
        assert crc == zlib.crc32(kind + data) & 0xFFFFFFFF, f"{kind} CRC"
        kinds.append(kind)
        if kind == b"IDAT":
            idat += data
        i += 12 + n
    assert kinds == [b"IHDR", b"IDAT", b"IEND"], kinds
    return idat


@pytest.mark.parametrize(
    "size, block_lengths",
    [((64, 64), [16448]), ((1, 1), [5]), ((200, 90), [65535, 6555])],
)
def test_the_encoder_is_read_back_by_two_independent_decoders(size, block_lengths):
    """Pillow and the stdlib's zlib, neither of which wrote it, read the pixels.

    (200, 90) is 72090 scanline bytes -- two stored blocks, so the block split
    and the BFINAL flag on the last one are exercised, not just the one-block
    case a skin needs.
    """
    w, h = size
    rgba = bytes((x * 7 + y * 13 + c * 31) % 256 for y in range(h) for x in range(w) for c in range(4))
    png = png_mod.encode_rgba(w, h, rgba)
    with Image.open(io.BytesIO(png)) as im:
        assert im.mode == "RGBA" and im.size == (w, h)
        assert im.tobytes() == rgba
    stream = _idat(png)
    raw = zlib.decompress(stream)
    assert raw == b"".join(b"\x00" + rgba[y * w * 4 : (y + 1) * w * 4] for y in range(h))
    blocks = list(_stored_blocks(stream))
    assert [f for f, _ in blocks] == [0] * (len(blocks) - 1) + [1]
    assert [n for _, n in blocks] == block_lengths, "blocks are filled to 65535 bytes"
    assert png_mod.adler32(raw) == zlib.adler32(raw)


def test_the_encoder_refuses_a_buffer_that_is_not_the_image_it_names():
    with pytest.raises(ValueError, match="RGBA"):
        png_mod.encode_rgba(2, 2, b"\x00" * 15)


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
    for y in range(FACE_CHIN, FACE_EYES):
        assert not _near(_at(skin, "head", "front", 3, y), GUIDE_HAIR), f"hair at y={y}"
    assert _near(_at(skin, "head", "down", 3, 3), GUIDE_SKIN), "chin underside"


def test_facial_hair_moustache_is_one_row():
    skin = _read_back(_dressed(facial_hair="moustache"))
    assert _near(_at(skin, "head", "front", 3, FACE_LIP), GUIDE_HAIR), "no moustache"
    for y in (FACE_CHIN, FACE_MOUTH):
        assert not _near(_at(skin, "head", "front", 3, y), GUIDE_HAIR), f"beard at y={y}"


def test_facial_hair_beard_covers_chin_jaw_and_underside():
    skin = _read_back(_dressed(facial_hair="beard"))
    for y in (FACE_CHIN, FACE_MOUTH, FACE_LIP):
        assert _near(_at(skin, "head", "front", 3, y), GUIDE_HAIR), f"no beard at y={y}"
    assert _near(_at(skin, "head", "down", 3, 3), GUIDE_HAIR, tol=9), "no underside"


# --- the lower face, which a beard used to be the only thing on -------------
#
# The defect these pin: the composer painted a brow, eyes and a nose on the
# upper rows and nothing at all below them, so half of every clean-shaven head
# was the fill colour and read as an enormous jaw. A beard hid it by occupying
# exactly those rows, which is why bearded characters looked right and
# clean-shaven ones did not. The rows themselves are a measurement, recorded in
# `docs/reference/face-craft.md`.

#: The amount `_build_head` textures bare skin with. A pixel within this of the
#: fill colour is fill, not modelling.
SKIN_NOISE = 6


def _face_row(skin: Skin, y: int):
    return [_at(skin, "head", "front", x, y) for x in range(8)]


def _lum(c) -> float:
    return 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]


def test_a_clean_shaven_face_is_modelled_below_the_eyes():
    """Every row under the eyes that is not the bare lip row carries a face."""
    skin = _read_back(_dressed(facial_hair="none", hair="short"))
    for y in (FACE_MOUTH, FACE_CHIN):
        row = _face_row(skin, y)
        modelled = [c for c in row if not _near(c, GUIDE_SKIN, tol=SKIN_NOISE + 2)]
        assert modelled, f"row y={y} of a clean-shaven face is bare fill"
    mouth = _face_row(skin, FACE_MOUTH)
    for x in (3, 4):
        assert not _near(mouth[x], GUIDE_SKIN, tol=SKIN_NOISE + 2), f"no mouth at x={x}"
    for x in (0, 1, 2, 5, 6, 7):
        assert _near(mouth[x], GUIDE_SKIN, tol=SKIN_NOISE + 2), (
            f"the mouth is wider than the two pixels every default skin gives it "
            f"(x={x})"
        )


def test_the_jaw_narrows_toward_the_chin():
    """The taper, asked as a shape rather than as a list of colours."""
    skin = _read_back(_dressed(facial_hair="none", hair="short"))
    chin = _face_row(skin, FACE_CHIN)
    centre = (_lum(chin[3]) + _lum(chin[4])) / 2
    inner = (_lum(chin[1]) + _lum(chin[6])) / 2
    outer = (_lum(chin[0]) + _lum(chin[7])) / 2
    assert outer < inner < centre, (
        "the chin row does not step down toward its corners: "
        f"centre={centre:.1f} inner={inner:.1f} outer={outer:.1f}"
    )
    for side in ("left", "right"):
        jaw = _lum(_at(skin, "head", side, 3, FACE_CHIN))
        cheek = _lum(_at(skin, "head", side, 3, FACE_EYES))
        assert jaw < cheek - 10, (
            f"the {side} of the head does not taper: jaw={jaw:.1f} cheek={cheek:.1f}"
        )


def test_a_beard_covers_the_mouth_rather_than_sitting_beside_it():
    beardless = _read_back(_dressed(facial_hair="none"))
    bearded = _read_back(_dressed(facial_hair="beard"))
    for x in (3, 4):
        assert not _near(_at(beardless, "head", "front", x, FACE_MOUTH), GUIDE_HAIR)
        assert _near(_at(bearded, "head", "front", x, FACE_MOUTH), GUIDE_HAIR)


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


def test_a_bald_head_gets_no_band_where_a_fringe_would_have_shadowed_it():
    """A hairline shadow with no hairline over it is a headband."""
    bald = _read_back(_steward(hair="bald", greying="none"))
    for x in range(8):
        assert _near(_at(bald, "head", "front", x, FACE_HAIRLINE), STEWARD_SKIN), (
            f"a band across a bald forehead at x={x}"
        )
    cropped = _read_back(_steward(hair="crop", greying="none"))
    assert not _near(_at(cropped, "head", "front", 3, FACE_HAIRLINE), STEWARD_SKIN), (
        "a fringe with no shadow under it"
    )


def test_the_brow_is_a_pair_of_eyebrows_and_not_a_band():
    """A full-width dark row across the face is a headband, not a brow."""
    skin = _read_back(_steward(hair="crop", greying="none"))
    brow = [_at(skin, "head", "front", x, FACE_BROW) for x in range(8)]
    for x in (1, 2, 5, 6):
        assert not _near(brow[x], STEWARD_SKIN), f"no eyebrow at x={x}"
    for x in (0, 3, 4, 7):
        assert _near(brow[x], STEWARD_SKIN), f"the brow runs across the face at x={x}"


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
    for group in (FACIAL_HAIR, COLLAR, HOOD, OVERCOAT, GREYING):
        for value in group:
            assert value in help_text, f"--help does not name wardrobe value {value!r}"


def test_readme_documents_every_axis_value_and_palette_key():
    readme = (Path(__file__).resolve().parents[1] / "README.md").read_text(
        encoding="utf-8"
    )
    axes = (
        ("sleeves", SLEEVES), ("legs", LEGS), ("footwear", FOOTWEAR),
        ("hair", HAIR), ("facial_hair", FACIAL_HAIR), ("collar", COLLAR),
        ("hood", HOOD), ("overcoat", OVERCOAT), ("greying", GREYING),
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
        "hood": "none",
        "overcoat": "none",
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


# --- the overlay shell (spec-0097) -------------------------------------------

OVERLAY_FIXTURE = FIXTURES / "overlay.cast.json"


def _overlay(texture_id: str) -> CastEntry:
    return next(e for e in _entries(OVERLAY_FIXTURE) if e.texture_id == texture_id)


def _sheet(entry: CastEntry) -> "models.Canvas":
    """Compose, then address the result through the model's own base and shell."""
    import numpy as np

    canvas = models.Canvas(entry.model_key())
    img = compose_skin(entry)
    canvas.image[:] = np.swapaxes(np.asarray(img), 0, 1)
    return canvas


def _shell_at(canvas, part: str, face: str, x: int, y: int):
    return tuple(int(c) for c in canvas.shell[part].get_face_for_id(face).get_color(x, y))


def _opaque(px) -> bool:
    return px[3] > 0


def test_a_beard_stands_off_the_jaw_on_the_hat():
    sheet = _sheet(_dressed(facial_hair="beard"))
    for y in (FACE_CHIN, FACE_MOUTH):
        for x in range(1, 7):
            assert _near(_shell_at(sheet, "head", "front", x, y), GUIDE_HAIR, tol=16), (x, y)
    for x in range(2, 6):
        assert _opaque(_shell_at(sheet, "head", "front", x, FACE_LIP))
    assert not _opaque(_shell_at(sheet, "head", "front", 3, FACE_EYES)), "the eyes are covered"
    assert _opaque(_shell_at(sheet, "head", "left", 1, FACE_CHIN)), "the jaw side"
    shaven = _sheet(_dressed(facial_hair="none", hair="bald"))
    hat = [_shell_at(shaven, "head", f, x, y) for f in models.FACES
           for x in range(8) for y in range(8)]
    assert not any(_opaque(p) for p in hat), "a bald clean-shaven head paints no hat"


def test_hair_has_a_lip_at_the_fringe_and_long_hair_falls_on_the_jacket():
    sheet = _sheet(_dressed(hair="long", facial_hair="none"))
    for x in range(8):
        assert _near(_shell_at(sheet, "head", "front", x, 7), GUIDE_HAIR, tol=16), x
    assert not _opaque(_shell_at(sheet, "head", "front", 3, FACE_EYES))
    sy0, sy1 = SHOULDER_HAIR
    for y in range(sy0, sy1 + 1):
        assert _opaque(_shell_at(sheet, "torso", "back", 3, y)), y
    short = _sheet(_dressed(hair="short", facial_hair="none"))
    assert not any(
        _opaque(_shell_at(short, "torso", "back", x, y)) for x in range(8) for y in range(12)
    ), "only long hair reaches the jacket"


def test_a_hood_frames_the_face_and_falls_to_the_shoulders():
    sheet = _sheet(_dressed(hood="up", facial_hair="none"))
    for y in range(0, FACE_HAIRLINE):
        for x in range(2, 6):
            assert not _opaque(_shell_at(sheet, "head", "front", x, y)), (x, y)
    for y in range(8):
        assert _opaque(_shell_at(sheet, "head", "front", 0, y))
        assert _opaque(_shell_at(sheet, "head", "front", 7, y))
    for face in ("up", "back", "left", "right"):
        assert all(_opaque(_shell_at(sheet, "head", face, x, y)) for x in range(8) for y in range(8))
    assert _opaque(_shell_at(sheet, "torso", "up", 0, 0)), "the fall on the shoulders"
    assert _opaque(_shell_at(sheet, "torso", "back", 3, 10))


def test_a_high_collar_rings_the_neck_and_a_closed_one_does_not():
    high = _sheet(_dressed(collar="high"))
    for face in ("front", "back", "left", "right"):
        for y in (10, 11):
            assert _opaque(_shell_at(high, "torso", face, 0, y)), (face, y)
        assert not _opaque(_shell_at(high, "torso", face, 0, 9)), face
    closed = _sheet(_dressed(collar="closed"))
    assert not any(
        _opaque(_shell_at(closed, "torso", f, x, y))
        for f in ("front", "back", "left", "right") for x in range(4) for y in range(12)
    )


def test_the_shell_is_painted_after_the_base_and_moves_none_of_it():
    """The base of every fixture is the base it composed before the shell existed:
    the shell draws from the stream only after the base is done, so the one
    difference a shell makes to the base faces is none."""
    entry = _dressed(hood="up", collar="high", hair="long")
    flat = _dressed(hood="none", collar="closed", hair="long")
    a, b = _sheet(entry), _sheet(flat)
    for pid in models.PART_IDS:
        for face in models.FACES:
            fa = a.base[pid].get_face_for_id(face)
            fb = b.base[pid].get_face_for_id(face)
            w, h = fa.shape
            assert all(
                tuple(fa.get_color(x, y)) == tuple(fb.get_color(x, y))
                for x in range(w) for y in range(h)
            ), (pid, face)


def test_a_zombie_is_drawn_to_its_own_boxes():
    entry = _overlay("zombie-farmer")
    assert entry.model_key() == "zombie"
    img = compose_skin(entry)
    assert img.size == (64, 64)
    px = img.load()
    for (u, v) in ((32, 48), (16, 48)):  # the player's left limbs; a zombie mirrors
        assert not any(px[x, y][3] for x in range(u, u + 16) for y in range(v, v + 16)), (u, v)
    for (u, v) in ((16, 32), (40, 32), (48, 48), (0, 32), (0, 48)):  # no jacket, sleeves, pants
        assert not any(px[x, y][3] for x in range(u, u + 16) for y in range(v, v + 16)), (u, v)
    assert any(px[x, y][3] for x in range(32, 64) for y in range(0, 16)), "the hat shell"


def test_an_outer_layer_is_painted_at_the_base_positions():
    entry = _overlay("drowned-sailor")
    img = compose_skin(entry)
    px = img.load()
    assert any(px[x, y][3] for x in range(0, 32) for y in range(0, 16)), "the outer head box"
    assert any(px[x, y][3] for x in range(16, 40) for y in range(20, 22)), "the collar on the outer body"
    for (u, v) in ((16, 32), (40, 32), (48, 48), (0, 32), (0, 48)):
        assert not any(px[x, y][3] for x in range(u, u + 16) for y in range(v, v + 16)), (u, v)


def test_a_body_the_wardrobe_does_not_fit_is_refused_by_name():
    row = {"texture_id": "v", "entity": "zombie_villager", "palette": {}}
    with pytest.raises(ValueError, match="could not be measured"):
        CastEntry.from_dict(row)
    with pytest.raises(ValueError, match="head's overlay shell"):
        CastEntry.from_dict({"texture_id": "p", "entity": "piglin", "palette": {},
                             "wardrobe": {"hood": "up"}})
    with pytest.raises(ValueError, match="torso's overlay shell"):
        CastEntry.from_dict({"texture_id": "z", "entity": "zombie", "palette": {},
                             "wardrobe": {"collar": "high"}})
    with pytest.raises(ValueError, match="has one model"):
        CastEntry.from_dict({"texture_id": "z", "entity": "zombie", "model": "wide",
                             "palette": {}})
    assert "player" not in entities() and "mannequin" in entities()
    assert set(entities()) == {"mannequin"} | set(models.model_keys()) - {"player", "player_slim"}


def test_hidden_layers_are_the_jars_and_named_once():
    base = {"texture_id": "h", "model": "wide", "palette": {}}
    assert CastEntry.from_dict({**base, "hidden_layers": ["hat"]}).hidden_layers == ["hat"]
    with pytest.raises(ValueError, match="unknown hidden layer"):
        CastEntry.from_dict({**base, "hidden_layers": ["helmet"]})
    with pytest.raises(ValueError, match="twice"):
        CastEntry.from_dict({**base, "hidden_layers": ["hat", "hat"]})
    with pytest.raises(ValueError, match="mannequin's"):
        CastEntry.from_dict({"texture_id": "z", "entity": "zombie", "palette": {},
                             "hidden_layers": ["hat"]})


def test_every_model_prints_its_own_table():
    piglin = parts_table("piglin")
    assert "hat" not in piglin and "jacket" in piglin
    assert "texOffs 0,38" in parts_table("villager"), "the villager's robe"
    for key in models.model_keys():
        assert parts_table(key).startswith(f"{key}: ")


def test_the_preview_shows_what_the_shell_covers():
    entry = _dressed(hood="up", facial_hair="none")
    flat = compose_preview_skin(entry)
    assert flat.size == (64, 64)
    head = Skin.from_image(flat).get_body_part_for_id("head").get_face_for_id("up")
    hood = (0x2F, 0x44, 0x36)  # the wardrobe fixture's tunic, which a hood defaults to
    assert _near(tuple(int(c) for c in head.get_color(3, 3)), hood, tol=8)
    for e in _entries(OVERLAY_FIXTURE):
        assert compose_preview_skin(e).size == (64, 64), e.texture_id


def test_a_villager_wears_its_robe_and_a_piglin_its_jacket():
    """spec-0097 §6.3: the vanilla overlay boxes of every model are dressed —
    the villager's 8x20x6 robe at 0,38, the piglin's jacket, sleeves and pants."""
    v = _sheet(CastEntry.from_dict({"texture_id": "v", "entity": "villager", "palette": {},
                                    "wardrobe": {"overcoat": "coat", "hair": "short"}}))
    robe = v.shell["torso"].get_face_for_id("front")
    assert robe.shape == (8, 20)
    assert all(_opaque(tuple(int(c) for c in robe.get_color(0, y))) for y in range(20)), "to the hem"
    assert _opaque(_shell_at(v, "head", "up", 3, 3)), "hair on the villager's hat shell"
    nose = next(part for box, part in v.extras if box.part == "head/nose")
    assert _opaque(tuple(int(c) for c in nose.get_face_for_id("front").get_color(0, 0)))
    pg = _sheet(CastEntry.from_dict({"texture_id": "p", "entity": "piglin", "palette": {},
                                     "wardrobe": {"overcoat": "long_coat", "sleeves": "long"}}))
    for part, (x, y) in (("torso", (0, 6)), ("left_arm", (0, 6)), ("left_leg", (0, 8))):
        assert _opaque(_shell_at(pg, part, "front", x, y)), part
    assert pg.layout.shell["head"] is None, "a piglin has no hat"


def test_a_zombies_preview_draws_both_arms_and_both_legs():
    """The mirrored limbs a zombie's model reads from the right ones are drawn in
    the preview, reflected — not left empty."""
    flat = Skin.from_image(compose_preview_skin(_overlay("zombie-farmer")))
    for pid in ("left_arm", "right_arm", "left_leg", "right_leg"):
        face = flat.get_body_part_for_id(pid).get_face_for_id("front")
        assert all(face.get_color(x, y)[3] for x in range(4) for y in range(12)), pid
