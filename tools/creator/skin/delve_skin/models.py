"""Each model's own boxes, and the canvas the composer paints them through (spec-0097).

The boxes come from the model-part table the engine judges every sheet by,
`crates/delvec/data/model-parts-<version>.json`, measured from the pinned
client by `tools/maintenance/extract-model-parts.py`. Nothing here restates a
`texOffs`: a model's part table is read, and a composer part is matched to the
model's part by where skinpy-extended's own layout puts it, never by name.

Two layers per part:

* the **base** is the part's own box of grow 0 (`head` at 0,0);
* the **shell** is the child box of the same size and positive grow
  (`head/hat` at 32,0, grown 0.5 a side). On an outer-layer model -- one with
  no box of grow 0, such as `drowned_outer_layer` -- a part's own grown box is
  its shell and there is no base.

A composer part the model does not build, or builds only as a mirror of
another box's UV (a zombie's left arm reads the right arm's pixels), is
addressed into a discarded buffer: the composer's calls and its seeded stream
are the same for every model, and the paint lands nowhere.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Dict, List, Optional, Tuple

import numpy as np
from skinpy import Skin
from skinpy.skin import BodyPart

#: The repository root, from `tools/creator/skin/delve_skin/models.py`.
REPO = Path(__file__).resolve().parents[4]

#: The pinned Minecraft version whose table is read.
MINECRAFT = "1.21.11"

TABLE_PATH = REPO / "crates" / "delvec" / "data" / f"model-parts-{MINECRAFT}.json"

#: The composer's part ids, skinpy-extended's own (observer-relative: its
#: `left_arm` is on the observer's left, which is the model's right arm).
PART_IDS = ("head", "torso", "left_arm", "right_arm", "left_leg", "right_leg")

FACES = ("up", "down", "left", "front", "right", "back")


@dataclass(frozen=True)
class Box:
    part: str
    u: int
    v: int
    w: int
    h: int
    d: int
    grow: float
    mirror: bool
    posed: bool

    def face_rects(self) -> Dict[str, Tuple[int, int, int, int]]:
        u, v, w, h, d = self.u, self.v, self.w, self.h, self.d
        return {
            "up": (u + d, v, w, d),
            "down": (u + d + w, v, w, d),
            "left": (u, v + d, d, h),
            "front": (u + d, v + d, w, h),
            "right": (u + d + w, v + d, d, h),
            "back": (u + 2 * d + w, v + d, w, h),
        }


@lru_cache(maxsize=1)
def table() -> dict:
    return json.loads(TABLE_PATH.read_text(encoding="utf-8"))


def model_keys() -> List[str]:
    return sorted(table()["models"])


def boxes(key: str) -> List[Box]:
    models = table()["models"]
    if key not in models:
        raise ValueError(
            f"no model {key!r} in {TABLE_PATH.name}; the table carries "
            f"{', '.join(sorted(models))}"
        )
    return [
        Box(
            part=c["part"], u=c["u"], v=c["v"], w=c["w"], h=c["h"], d=c["d"],
            grow=c["grow"], mirror=c["mirror"], posed=c["posed"],
        )
        for c in models[key]["cubes"]
    ]


def texture_size(key: str) -> Tuple[int, int]:
    w, h = table()["models"][key]["texture_size"]
    return w, h


def mannequin_layers() -> List[str]:
    """The ids a mannequin's `hidden_layers` may name, read from the jar."""
    return list(table()["mannequin"]["layers"])


@lru_cache(maxsize=1)
def _skinpy_front_origins() -> Dict[str, Tuple[int, int]]:
    """Where skinpy-extended's own layout puts each part's front face.

    Read by addressing a sheet whose every pixel encodes its own coordinates,
    so the mapping is a measurement of the library, not a restatement of it.
    """
    img = np.zeros((64, 64, 4), dtype=np.uint8)
    for x in range(64):
        for y in range(64):
            img[x, y] = (x, y, 0, 255)
    skin = Skin.new(img)
    out = {}
    for pid in PART_IDS:
        front = skin.get_body_part_for_id(pid).get_face_for_id("front")
        _, fh = front.shape
        # The face's y=fh-1 is its top row; x=0 its observer-left column.
        c = front.get_color(0, fh - 1)
        out[pid] = (int(c[0]), int(c[1]))
    return out


@lru_cache(maxsize=1)
def part_names() -> Dict[str, str]:
    """Composer part id -> the model's own part name, matched on the player table."""
    fronts = _skinpy_front_origins()
    out = {}
    for pid, origin in fronts.items():
        hits = [
            b.part for b in boxes("player")
            if b.grow == 0 and (b.u + b.d, b.v + b.d) == origin
        ]
        if len(hits) != 1:
            raise RuntimeError(
                f"skinpy's {pid} front at {origin} matches {hits} in the player table"
            )
        out[pid] = hits[0]
    return out


def _own_box(bs: List[Box], name: str, grown: bool) -> Optional[Box]:
    own = [b for b in bs if b.part == name and ((b.grow > 0) if grown else (b.grow == 0))]
    return own[0] if own else None


def _reads_another(bs: List[Box], box: Box) -> bool:
    """A mirrored box that shares its UV with an unmirrored one reads that one's pixels."""
    return box.mirror and any(
        o is not box and not o.mirror and (o.u, o.v, o.w, o.h, o.d) == (box.u, box.v, box.w, box.h, box.d)
        for o in bs
    )


@dataclass(frozen=True)
class Layout:
    """Which box each composer part paints, base and shell, on one model."""

    key: str
    size: Tuple[int, int]
    outer: bool
    base: Dict[str, Optional[Box]]
    shell: Dict[str, Optional[Box]]


@lru_cache(maxsize=None)
def layout(key: str) -> Layout:
    bs = boxes(key)
    outer = not any(b.grow == 0 for b in bs)
    names = part_names()
    base: Dict[str, Optional[Box]] = {}
    shell: Dict[str, Optional[Box]] = {}
    for pid, name in names.items():
        if outer:
            own = _own_box(bs, name, grown=True)
            base[pid] = None
            shell[pid] = None if own is None or _reads_another(bs, own) else own
            continue
        own = _own_box(bs, name, grown=False)
        base[pid] = None if own is None or _reads_another(bs, own) else own
        kids = [
            b for b in bs
            if b.part.startswith(name + "/") and b.part.count("/") == 1 and b.grow > 0
            and own is not None and (b.w, b.h, b.d) == (own.w, own.h, own.d)
        ]
        shell[pid] = kids[0] if kids and base[pid] is not None else None
    return Layout(key=key, size=texture_size(key), outer=outer, base=base, shell=shell)


@lru_cache(maxsize=1)
def dressable() -> Tuple[str, ...]:
    """The models the composer's wardrobe fits: every part the player's size.

    Derived, not listed: a model qualifies when each composer part's own box
    (its base, or on an outer layer its grown box) has the player's base box's
    dimensions.
    """
    player = layout("player")
    out = []
    for key in model_keys():
        bs = boxes(key)
        outer = not any(b.grow == 0 for b in bs)
        ok = True
        for pid, name in part_names().items():
            own = _own_box(bs, name, grown=outer)
            want = player.base[pid]
            if own is None or (own.w, own.h, own.d) != (want.w, want.h, want.d):
                ok = False
                break
        if ok:
            out.append(key)
    return tuple(out)


class Canvas:
    """A sheet addressed as part, face, x, y through skinpy-extended's faces.

    ``base`` and ``shell`` are dictionaries of composer part id ->
    ``BodyPart``; a part the model does not paint resolves into ``sink``.
    """

    def __init__(self, key: str) -> None:
        self.layout = layout(key)
        w, h = self.layout.size
        # Column-major (x first), as skinpy-extended stores a sheet.
        self.image = np.zeros((w, h, 4), dtype=np.uint8)
        self.sink = np.zeros((64, 64, 4), dtype=np.uint8)
        player = layout("player")
        self.base = {pid: self._part(pid, self.layout.base[pid], player.base[pid]) for pid in PART_IDS}
        self.shell = {pid: self._part(pid, self.layout.shell[pid], player.base[pid]) for pid in PART_IDS}

    def _part(self, pid: str, box: Optional[Box], shape: Box) -> BodyPart:
        target, b = (self.image, box) if box is not None else (self.sink, shape)
        return BodyPart.new(
            id_=pid,
            skin_image_color=target,
            part_shape=(b.w, b.d, b.h),
            part_model_origin=(0, 0, 0),
            part_image_origin=(b.u, b.v) if box is not None else (0, 0),
        )

    def has_shell(self, pid: str) -> bool:
        return self.layout.shell[pid] is not None

    def to_rgba(self) -> Tuple[int, int, bytes]:
        w, h = self.layout.size
        return w, h, np.swapaxes(self.image, 0, 1).tobytes()
