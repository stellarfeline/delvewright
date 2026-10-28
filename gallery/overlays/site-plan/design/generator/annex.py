#!/usr/bin/env python3
"""A place from its allocation: the gallery's `node/annex`, generated.

The worked example of a per-place generator (`docs/reference/grammar.md` §2c,
*A place from its allocation*). It reads the allocation the whole hands the
annex and writes the annex's detail program. It runs no binary and names no
path: the caller saves the allocation and names both files.

    delvec allocation <campaign> node/annex > annex-allocation.json
    "$DELVEWRIGHT_PYTHON" annex.py --allocation annex-allocation.json \
        --out <campaign>/programs/annex.json

The annex is a roofed box: a lit hall entered through an arch seam on its
east side, a barred chute seam on the same side further along, a pitched roof
over the lid the plan reserves, and a `mark` for every name the place owes.
Each step below names the obligation of the program's contract
(`references/detail.md`, *The program's contract*) it meets.
"""

from __future__ import annotations

import argparse
import json
import sys

# The handed names this program reads. Contract: *it declares the handed
# values it uses as parameters under the `handed/` prefix, with the
# allocation's values as their defaults*, and *a script reads the handed values
# from the allocation and retypes none* — the body reads each one only through
# its parameter.
ARCH = "annex-arch"
CHUTE = "annex-chute"


def handed(allocation: dict) -> dict[str, int]:
    """Every handed value the program uses, keyed by its `handed/…` name."""
    roof = allocation.get("roof")
    if roof is None:
        sys.exit("annex.py: the allocation hands no roof; the annex is built under one")
    out = {
        "handed/datum-y": allocation["datum_y"],
        "handed/roof/eaves": roof["eaves"],
        "handed/roof/lid-y": roof["lid_y"],
        "handed/roof/top-y": roof["top_y"],
    }
    seams = {s["edge"].removeprefix("edge/"): s for s in allocation["seams"]}
    for stem in (ARCH, CHUTE):
        seam = seams.get(stem)
        if seam is None:
            sys.exit(f"annex.py: the allocation hands no seam `edge/{stem}`")
        (_, y0, z0), (_, y1, z1) = seam["cells"]
        for key, value in (("y0", y0), ("y1", y1), ("z0", z0), ("z1", z1)):
            out[f"handed/seam/{stem}/{key}"] = value
    return out


# ---- the expression and operation vocabulary, as the grammar spells it ----


def num(v: int) -> dict:
    return {"expr": "int", "value": v}


def par(name: str) -> dict:
    return {"expr": "param", "name": name}


def arith(lhs: dict, op: str, rhs: dict) -> dict:
    return {"expr": "arith", "lhs": lhs, "op": op, "rhs": rhs}


def blocks(e: dict) -> dict:
    return {"blocks": e, "size": "absolute"}


def rel() -> dict:
    return {"size": "relative", "weight": num(1)}


def fill(role: str) -> dict:
    return {"material": {"role": role}, "op": "fill"}


def call(symbol: str) -> dict:
    return {"op": "call", "symbol": symbol}


VOID = {"op": "void"}


def claim(region: str, body: dict) -> dict:
    return {"body": body, "op": "claim", "region": region}


def split(axis: str, sizes: list, children: list, repeat: bool = False) -> dict:
    out = {"axis": axis, "children": children, "op": "split", "sizes": sizes}
    if repeat:
        out["repeat"] = True
    return out


def rule(body: dict, when: dict | None = None) -> dict:
    out = {"body": body, "weight": 1}
    if when is not None:
        out["when"] = when
    return out


def span(stem: str, axis: str) -> dict:
    """A seam's run along `axis`, inclusive: `<axis>1 − <axis>0 + 1`."""
    hi, lo = par(f"handed/seam/{stem}/{axis}1"), par(f"handed/seam/{stem}/{axis}0")
    return arith(arith(hi, "sub", lo), "add", num(1))


def eaves_frame(axis: str, inner: list) -> dict:
    """The eaves run on both sides of `inner`, then a wall course each side."""
    eaves = blocks(par("handed/roof/eaves"))
    one = blocks(num(1))
    return split(axis, [eaves, one, rel(), one, eaves], inner)


def dims_at_least(**mins: int) -> dict:
    conds = [
        {"cond": "cmp", "lhs": {"dim": axis, "expr": "dim"}, "op": "ge", "rhs": num(v)}
        for axis, v in mins.items()
    ]
    return conds[0] if len(conds) == 1 else {"cond": "all", "of": conds}


OTHERWISE = {"cond": "otherwise"}


def rules() -> dict:
    datum = par("handed/datum-y")
    one = blocks(num(1))
    return {
        # The piece is expanded at the frame, so the start symbol splits the
        # whole frame on y at the handed datum and the handed lid: the floor
        # stands under the walk plane and the roof starts at the lid.
        "annex": [
            rule(
                split(
                    "y",
                    [blocks(datum), blocks(arith(par("handed/roof/lid-y"), "sub", datum)), rel()],
                    [fill("floor"), call("storey"), call("roof")],
                )
            )
        ],
        # The storey: the eaves the plan reserves on every side stay air, then
        # a wall course; the east wall is the party wall that carries the chute.
        "storey": [
            rule(
                eaves_frame(
                    "x", [VOID, fill("wall"), call("inner"), call("party"), VOID]
                )
            )
        ],
        "inner": [rule(eaves_frame("z", [VOID, fill("wall"), call("rooms"), call("street"), VOID]))],
        # The street face (spec-0123 §3.5, the census element): a counter
        # window and a door, both under one head course. The window is one
        # course high over a one-course sill, its iron shutter folded down
        # as the counter — an iron trapdoor laid closed on the sill, which a
        # body leans on and never climbs through and no hand lifts; the door is
        # iron too and stays shut: the shop is shut, and the face opens no way
        # the plan did not allocate. A course of wall stands over both and
        # frames each with jambs.
        "street": [
            rule(
                split(
                    "x",
                    [blocks(num(4)), blocks(num(3)), one, one, rel()],
                    [fill("wall"), call("counter_bay"), fill("wall"), call("door_bay"), fill("wall")],
                )
            )
        ],
        "counter_bay": [
            rule(split("y", [one, one, rel()], [fill("wall"), fill("counter"), fill("wall")]))
        ],
        "door_bay": [
            rule(split("y", [one, one, rel()], [fill("street-door"), fill("street-door-top"), fill("wall")]))
        ],
        "rooms": [rule(split("x", [rel(), one], [call("room_main"), call("east_col")]))],
        # Contract: *it answers each seam with an opening at the handed cells
        # … and a contract edge to `exterior` whose `via` is that opening* —
        # the arch is the `arch-way` region, cut at the handed y and z runs.
        "east_col": [
            rule(split("y", [blocks(span(ARCH, "y")), rel()], [call("east_band"), fill("wall")]))
        ],
        "east_band": [
            rule(
                split(
                    "z",
                    [
                        blocks(
                            arith(
                                par(f"handed/seam/{ARCH}/z0"),
                                "sub",
                                arith(par("handed/roof/eaves"), "add", num(1)),
                            )
                        ),
                        blocks(span(ARCH, "z")),
                        blocks(
                            arith(
                                arith(par(f"handed/seam/{CHUTE}/z0"), "sub", par(f"handed/seam/{ARCH}/z1")),
                                "sub",
                                num(1),
                            )
                        ),
                        blocks(span(CHUTE, "z")),
                        rel(),
                    ],
                    [fill("wall"), claim("arch-way", VOID), fill("wall"), claim("hall", VOID), fill("wall")],
                )
            )
        ],
        # Contract: *a gated seam whose bar this place owns* gets *a `barred`
        # contract edge whose `bar` is over exactly those cells, filled with a
        # palette role of that block, its region named by the same stem* — the
        # chute is the `seam-annex-chute` region, filled with the `bar` role,
        # and its leaf is marked `anchor/annex-chute-leaf`, where the gate's
        # own answer is pressed (spec-0117).
        "party": [
            rule(
                split(
                    "z",
                    [blocks(par(f"handed/seam/{CHUTE}/z0")), blocks(span(CHUTE, "z")), rel()],
                    [fill("wall"), call("chute"), fill("wall")],
                )
            )
        ],
        "chute": [
            rule(
                split(
                    "y",
                    [
                        blocks(arith(par(f"handed/seam/{CHUTE}/y0"), "sub", datum)),
                        blocks(span(CHUTE, "y")),
                        rel(),
                    ],
                    [
                        fill("wall"),
                        {
                            "body": claim(f"seam-{CHUTE}", fill("bar")),
                            "mark": {"anchor": "annex-chute-leaf", "at": "floor_center"},
                            "op": "mark",
                        },
                        fill("wall"),
                    ],
                )
            )
        ],
        # The hall, three cells high, under a ceiling that carries its own
        # light — a lamp every `lamp_period` cells — for the light probe
        # `delvec detail` runs over the piece.
        "room_main": [
            rule(split("y", [blocks(num(3)), rel()], [claim("hall", call("room")), call("ceiling")]))
        ],
        "ceiling": [
            rule(
                split(
                    "x",
                    [one, blocks(arith(par("lamp_period"), "sub", num(1)))],
                    [call("lamp_row"), fill("wall")],
                    repeat=True,
                )
            )
        ],
        "lamp_row": [
            rule(
                split(
                    "z",
                    [one, blocks(arith(par("lamp_period"), "sub", num(1)))],
                    [fill("lamp"), fill("wall")],
                    repeat=True,
                )
            )
        ],
        # Contract: *it answers each owed name with a `mark` of that stem* —
        # `anchor/node-annex` and the bench station `anchor/annex-bench`.
        "room": [
            rule(
                {
                    "body": {
                        "body": split("x", [rel(), blocks(num(2)), rel()], [VOID, call("plinth_row"), VOID]),
                        "mark": {"anchor": "annex-bench", "at": "offset", "facing": "west", "x": num(12), "y": num(0), "z": num(12)},
                        "op": "mark",
                    },
                    "mark": {"anchor": "node-annex", "at": "offset", "facing": "east", "x": num(2), "y": num(0), "z": num(6)},
                    "op": "mark",
                }
            )
        ],
        "plinth_row": [rule(split("z", [rel(), blocks(num(2)), rel()], [VOID, call("plinth"), VOID]))],
        "plinth": [rule(split("y", [one, rel()], [fill("furniture"), VOID]))],
        # The roof zone the plan reserves over the lid: claimed as the `roof`
        # region the contract declares no body walks, a pitched gable above.
        "roof": [rule(claim("roof", split("y", [one, rel()], [call("lid_course"), call("gable")])))],
        "lid_course": [rule(split("x", [one, rel(), one], [fill("roof_e"), fill("ceiling"), fill("roof_w")]))],
        "gable": [rule(split("x", [one, rel(), one], [VOID, call("slope"), VOID]))],
        "slope": [
            rule(split("y", [one, rel()], [call("course"), call("slope_up")]), dims_at_least(y=1, x=2)),
            rule(VOID, OTHERWISE),
        ],
        "slope_up": [
            rule(split("x", [one, rel(), one], [VOID, call("slope"), VOID]), dims_at_least(x=4)),
            rule(VOID, OTHERWISE),
        ],
        "course": [rule(split("x", [one, rel(), one], [fill("roof_e"), call("gable_row"), fill("roof_w")]))],
        "gable_row": [rule(eaves_frame("z", [VOID, fill("wall"), VOID, fill("wall"), VOID]))],
    }


def contract() -> dict:
    """The spatial contract. Contract: *it declares a spatial contract
    (`DW0843`)*, and *the enclosed spaces are the program's to declare* — the
    hall is the one; the walk in is by the arch, the barred edge by the chute,
    and the roof is a zone no body walks."""
    return {
        "edges": [
            {"a": "exterior", "b": "hall", "class": "walk", "via": "arch-way"},
            {
                "a": "hall",
                "b": "exterior",
                "bar": {"block": "bar", "region": f"seam-{CHUTE}"},
                "class": "barred",
            },
        ],
        "entry": "hall",
        "no_body": {"roof": {"reason": "the roof and the loft under it, which no one is meant to walk"}},
        "no_body_majority_ack": (
            "a building that owns its outside owns its roof, and a pitched roof is more "
            "standable surface than the room under it; nobody walks it"
        ),
        "spaces": {"hall": {"envelope": "enclosed"}},
    }


PALETTE = {
    "bar": "minecraft:iron_bars[east=false,north=false,south=false,waterlogged=false,west=false]",
    "ceiling": "minecraft:spruce_planks",
    "floor": "minecraft:stone_bricks",
    "furniture": "minecraft:chiseled_stone_bricks",
    "lamp": "minecraft:sea_lantern",
    "counter": "minecraft:iron_trapdoor[facing=south,half=bottom,open=false,powered=false,waterlogged=false]",
    "street-door": "minecraft:iron_door[facing=south,half=lower,hinge=left,open=false,powered=false]",
    "street-door-top": "minecraft:iron_door[facing=south,half=upper,hinge=left,open=false,powered=false]",
    "roof_e": {"local": "minecraft:dark_oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]"},
    "roof_w": {"local": "minecraft:dark_oak_stairs[facing=west,half=bottom,shape=straight,waterlogged=false]"},
    "wall": "minecraft:stone_bricks",
}


def program(allocation: dict) -> dict:
    params = handed(allocation)
    # A design choice of this place, not a handed value: one lamp in four.
    params["lamp_period"] = 4
    return {
        "contract": contract(),
        "name": "gallery-annex",
        "palette": PALETTE,
        "params": params,
        "rules": rules(),
        # The faces the piece shows the world.
        "shown_faces": ["north", "south", "up", "west"],
        "start": "annex",
        "version": "1.9.0",
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--allocation", required=True, help="`delvec allocation <campaign> node/annex`, saved")
    ap.add_argument("--out", required=True, help="where the program is written")
    args = ap.parse_args()
    with open(args.allocation, encoding="utf-8") as f:
        allocation = json.load(f)
    if allocation.get("place") != "node/annex":
        sys.exit(f"annex.py: the allocation is for `{allocation.get('place')}`, not `node/annex`")
    text = json.dumps(program(allocation), indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    with open(args.out, "w", encoding="utf-8") as f:
        f.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
