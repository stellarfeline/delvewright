"""`tools/lib/anvil.py` reads a world's light, block entities and entities the
way the pinned server writes them (minecraft.wiki *Chunk format*, *Entity
format*): a section's `BlockLight`/`SkyLight` nibble arrays in `y`-`z`-`x`
order, low nibble first; a chunk's `block_entities` list; the `entities/`
region family's `Entities` lists.

Each world here is written by the module's own region writer, so a reader that
read another layout than the one it writes would answer the wrong cell."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ANVIL = ROOT / "tools" / "lib" / "anvil.py"
sys.path.insert(0, str(ROOT / "tools" / "lib"))
import anvil  # noqa: E402
from anvil import T_BYTE_ARRAY, T_COMPOUND, T_DOUBLE, T_INT, T_LIST, T_STRING, Tag  # noqa: E402


def nibbles(values: dict[int, int]) -> bytes:
    """A 2048-byte light array holding `values` (cell index -> level)."""
    out = bytearray(2048)
    for i, v in values.items():
        out[i // 2] |= (v & 0x0F) << (4 if i % 2 else 0)
    return bytes(out)


def make(tmp: Path) -> Path:
    w = tmp / "w"
    (w / "region").mkdir(parents=True)
    (w / "entities").mkdir(parents=True)
    (w / "level.dat").write_bytes(b"")
    # Section Y=4 (y 64..79) of chunk (0, 0): block light 15 at (3, 68, 3),
    # sky light 15 at (5, 70, 4), 0 everywhere else.
    i_block = (68 % 16) * 256 + 3 * 16 + 3
    i_sky = (70 % 16) * 256 + 4 * 16 + 5
    section = Tag(
        T_COMPOUND,
        {
            "Y": Tag(1, 4),
            "BlockLight": Tag(T_BYTE_ARRAY, nibbles({i_block: 15})),
            "SkyLight": Tag(T_BYTE_ARRAY, nibbles({i_sky: 15, i_block: 7})),
        },
    )
    sign = Tag(
        T_COMPOUND,
        {
            "id": Tag(T_STRING, "minecraft:sign"),
            "x": Tag(T_INT, 8),
            "y": Tag(T_INT, 65),
            "z": Tag(T_INT, 8),
            "front_text": Tag(
                T_COMPOUND, {"messages": Tag(T_LIST, [Tag(T_STRING, "KEEP")] + [Tag(T_STRING, "")] * 3, T_STRING)}
            ),
        },
    )
    anvil.region_write(
        w / "region" / "r.0.0.mca",
        {
            (0, 0): Tag(
                T_COMPOUND,
                {
                    "sections": Tag(T_LIST, [section], T_COMPOUND),
                    "block_entities": Tag(T_LIST, [sign], T_COMPOUND),
                },
            )
        },
    )
    body = Tag(
        T_COMPOUND,
        {
            "id": Tag(T_STRING, "minecraft:mannequin"),
            "Pos": Tag(T_LIST, [Tag(T_DOUBLE, p) for p in (10.5, 67.0, 5.5)], T_DOUBLE),
            "Tags": Tag(T_LIST, [Tag(T_STRING, "dw_npc_curator")], T_STRING),
        },
    )
    anvil.region_write(
        w / "entities" / "r.0.0.mca",
        {(0, 0): Tag(T_COMPOUND, {"Entities": Tag(T_LIST, [body], T_COMPOUND)})},
    )
    return w


def test_light_reads_the_nibble_of_the_cell_asked(tmp_path):
    w = make(tmp_path)
    assert anvil.light(w, (3, 68, 3)) == (15, 7)
    assert anvil.light(w, (5, 70, 4)) == (0, 15)
    assert anvil.light(w, (4, 68, 3)) == (0, 0)
    # A section the save does not carry: no arrays, so nothing to read.
    assert anvil.light(w, (3, 10, 3)) == (None, None)
    arrays = anvil.light_arrays(w)
    assert list(arrays) == [(0, 4, 0)]
    assert all(len(a) == 2048 for a in arrays[(0, 4, 0)])


def test_block_entities_and_entities_read_back_whole(tmp_path):
    w = make(tmp_path)
    bes = anvil.block_entities(w)
    assert list(bes) == [(8, 65, 8)]
    assert bes[(8, 65, 8)]["front_text"]["messages"][0] == "KEEP"
    ents = anvil.entities(w)
    assert len(ents) == 1
    assert ents[0]["id"] == "minecraft:mannequin"
    assert ents[0]["Pos"] == [10.5, 67.0, 5.5]
    assert ents[0]["Tags"] == ["dw_npc_curator"]


def test_the_cli_answers_what_the_functions_do(tmp_path):
    w = make(tmp_path)

    def run(*args):
        r = subprocess.run([sys.executable, "-I", str(ANVIL), *args], capture_output=True, text=True)
        assert r.returncode == 0, r.stderr
        return json.loads(r.stdout)

    assert run("light", str(w), "3", "68", "3") == {"block": 15, "sky": 7}
    assert run("light-census", str(w)) == {"sections": 1, "with_both": 1}
    assert run("block-entities", str(w))[0]["id"] == "minecraft:sign"
    assert run("entities", str(w))[0]["Tags"] == ["dw_npc_curator"]
