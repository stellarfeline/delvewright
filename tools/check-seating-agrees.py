#!/usr/bin/env python3
"""The seating answer is the build answer, over every pool and every base.

## The property

spec-0060 §6.3 makes one property load-bearing: **a pool the seating command
calls seatable is not then refused by the build for a reason the command could
have known.** `delvec prefab seating` exists to answer *can this library stand on
this base* before a campaign is authored against either, and a verdict that
disagrees with the build is worse than no verdict — the creator spends the
assembly anyway and learns that the tool lied.

It was measured **false**. On an ocean, `pool/island` and `pool/cave-shore` both
printed `SEATABLE` and were then refused at build by `DW0886`: their members'
declared walk planes disagree with one another (`[2, 3]` and `[1, 2]`), which is
a fact about documents the command had already read, asked only after placement
because the rule lived in `compiler::plan::area_base_y` and nowhere else. Two
pieces were refused by `DW0344` for standing a walk plane level with their own
waterline, which is two numbers in one document.

## What this gate does

For every pool in the library it is pointed at and every horizon base the ENGINE
declares (`delvec schema --stage world`, never a list kept here):

1. run `delvec prefab seating --horizon <base> --json` and take that pool's
   verdict and the set of codes it named;
2. write a campaign that seats exactly that pool on that base, and run
   `delvec analyze` on it;
3. assert the two answers **agree**:
   - seatable  -> analyze raises none of the seating codes;
   - refused   -> analyze raises at least one code the command named.

`DW0855` is the one outcome that is neither, and it is enumerated rather than
shrugged at: a base that builds terrain refuses an `areas[]` campaign outright
(spec-0049 §6, two placement authorities), which is the case `DW0886`'s own
message names as the reason it did not fire. A cell that reds `DW0855` and
nothing else is recorded as `not-an-areas-base`, and a base is allowed at most
that.

## The perturbations

A green over a coherent library proves the gate runs, not that it can see. So
every base is also run over **two perturbed copies** of the library, each
carrying one of the two shapes measured in the tree before the repair:

- **two walk planes in one pool** — one member's `walk_y` moved by one, which is
  exactly `pool/island`'s `[2, 3]`. Before this round the command called that
  pool seatable and the build refused it: this cell reds unless BOTH answers
  refuse.
- **a shore level with its own sea** — one member's `waterline_y` raised to equal
  its `walk_y`, which is exactly `island-beach-camp` and `cave-shore`. Same
  shape, `DW0344`.

Each perturbation asserts it actually moved a declaration, so a copy that failed
to perturb is a red rather than a quiet pass.

## Binding

Every run states pools examined, bases examined, cells judged, perturbation
cells judged, and the codes seen. **Zero cells judged is a red**, and so is a
library that declares no pool: a green over an empty population is the unbound
vacuity mode.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from delvec_bin import resolve as resolve_delvec  # noqa: E402

REPO = Path(__file__).resolve().parent.parent

# The codes that are the SEATING answer — the ones both sides must agree about.
# Named as a set rather than matched by prefix: a gate that asked "did any DW
# code appear" would call every unrelated refusal a disagreement and go green on
# noise.
SEATING_CODES = {"DW0886", "DW0887", "DW0344"}

# The refusal that is a fact about the CAMPAIGN's placement authority rather than
# about the piece set: a base that builds terrain needs a site plan, and an
# `areas[]` campaign states none.
NOT_AN_AREAS_BASE = "DW0855"


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def run(cmd: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True)


def declared_bases(delvec: Path) -> list[str]:
    """Every horizon base the ENGINE declares, from its own schema export.

    Read through `$defs/HorizonBase` by name. A walk that matched string values
    would also match a default, and a base the engine stopped declaring would
    still be found.
    """
    r = run([str(delvec), "schema", "--stage", "world"])
    if r.returncode != 0:
        die(f"`delvec schema --stage world` exited {r.returncode}: {r.stderr.strip()}")
    doc = json.loads(r.stdout)
    definition = doc.get("$defs", {}).get("HorizonBase", {})
    bases: list[str] = []
    if isinstance(definition.get("enum"), list):
        bases += [b for b in definition["enum"] if isinstance(b, str)]
    if isinstance(definition.get("oneOf"), list):
        bases += [
            v["const"]
            for v in definition["oneOf"]
            if isinstance(v, dict) and isinstance(v.get("const"), str)
        ]
    bases = sorted(set(bases))
    if not bases:
        die(
            "the world schema declares no horizon base. The population this gate "
            "quantifies over comes from the engine and from nowhere else, so an "
            "empty one is a broken reading, not an empty world."
        )
    return bases


def pools_of(prefabs: Path) -> dict[str, list[str]]:
    """Every pool the library declares, with its members, from `pools.json`."""
    path = prefabs / "pools.json"
    if not path.is_file():
        die(f"{path} does not exist: this library declares no pools to judge")
    table = json.loads(path.read_text()).get("pools", {})
    out: dict[str, list[str]] = {}
    for pool, body in table.items():
        ids = sorted({m["prefab"] for m in body.get("members", [])})
        out[pool] = ids
    if not out:
        die(f"{path} declares zero pools — a green over an empty population is vacuous")
    return out


def seating(delvec: Path, prefabs: Path, base: str) -> dict:
    """`delvec prefab seating --json` — the machine form of the command's table."""
    r = run(
        [
            str(delvec),
            "--json",
            "prefab",
            "seating",
            "--horizon",
            base,
            "--prefabs",
            str(prefabs),
        ]
    )
    try:
        return json.loads(r.stdout)
    except json.JSONDecodeError:
        die(
            f"`prefab seating --horizon {base} --json` printed no JSON verdict "
            f"(exit {r.returncode}):\n{r.stdout}\n{r.stderr}"
        )
        raise


def write_campaign(dest: Path, pool: str, base: str) -> None:
    """A campaign that seats exactly `pool` on `base`, and nothing else.

    The stage documents come from the engine's own hello-world fixture so the
    campaign is one this `delvec` accepts whole; the only thing this function
    decides is the horizon and the piece set, which is the pair under test.
    """
    src = REPO / "crates/dsl/fixtures/valid/hello-world"
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(src, dest)
    world = json.loads((dest / "world.json").read_text())
    content = world["content"]
    content["horizon"] = base
    content["boundary"] = {"margin": 20}
    area = content["areas"][0]
    area.pop("prefab", None)
    area["prefab_pool"] = pool
    # The island pieces are dark and a probe that declared no mitigation reds
    # `DW0210` — the harness's defect, not the library's (spec-0060 §1.1).
    area["mitigation"] = "night-vision"
    (dest / "world.json").write_text(json.dumps(world, indent=2) + "\n")


def codes_in(text: str) -> set[str]:
    """Every code the engine RAISED, from the `code` field of its own diagnostics.

    Read out of `--json`'s one-object-per-line stream rather than grepped out of
    prose, and the difference is not tidiness: `DW0886`'s message names `DW0855`
    and `DW0839` in its own moves — the whole point of that message is that the
    three read as one answer — so a grep over the text reports three codes raised
    where one was. A gate that measured what a message MENTIONS would classify
    every `DW0886` cell as a site-plan base.
    """
    out = set()
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            doc = json.loads(line)
        except json.JSONDecodeError:
            continue
        code = doc.get("code")
        if isinstance(code, str) and doc.get("severity") == "error":
            out.add(code)
    return out


def analyze(delvec: Path, prefabs: Path, campaign: Path) -> tuple[int, set[str], str]:
    r = run([str(delvec), "--json", "analyze", str(campaign), "--prefabs", str(prefabs)])
    text = r.stdout + r.stderr
    return r.returncode, codes_in(text), text


def judge(
    delvec: Path, prefabs: Path, pool: str, base: str, verdict: dict, work: Path
) -> tuple[str, str]:
    """One cell: the command's answer and the engine's, compared.

    Returns `(state, note)`. `state` is `agree`, `not-an-areas-base` or
    `DISAGREE`; the note is what to print beside it.
    """
    said = {r["code"] for r in verdict["reasons"]} & SEATING_CODES
    seatable = verdict["verdict"] == "SEATABLE"
    camp = work / f"camp-{pool.replace('/', '-')}-{base}"
    write_campaign(camp, pool, base)
    code, raised, text = analyze(delvec, prefabs, camp)
    raised_seating = raised & SEATING_CODES

    if seatable:
        if raised_seating:
            return (
                "DISAGREE",
                f"the command called `{pool}` SEATABLE on `{base}` and the engine "
                f"refused it with {sorted(raised_seating)} — a fact about the "
                f"documents, asked only after the command had read them:\n{text}",
            )
        if NOT_AN_AREAS_BASE in raised:
            return (
                "not-an-areas-base",
                f"seatable; {NOT_AN_AREAS_BASE} — a base that builds terrain, on a "
                f"campaign that states no extent. This sweep's campaigns bind POOLS, and a "
                f"pool states none; an `areas[]` campaign reaches this base by being one "
                f"area bound to one prefab. Codes {sorted(raised)}",
            )
        return ("agree", f"seatable, analyze exit {code}, codes {sorted(raised)}")

    if not said:
        return (
            "DISAGREE",
            f"the command REFUSED `{pool}` on `{base}` and named no seating code, "
            f"so nothing says what the build should agree with",
        )
    if not (said & raised_seating):
        return (
            "DISAGREE",
            f"the command refused `{pool}` on `{base}` with {sorted(said)} and the "
            f"engine raised {sorted(raised)} — a pool refused for a reason it was "
            f"not given:\n{text}",
        )
    return ("agree", f"refused by both, {sorted(said & raised_seating)}")


def perturb(prefabs: Path, dest: Path, pool: str, members: list[str], kind: str) -> str:
    """A copy of the library carrying one of the two shapes measured in the tree.

    Returns what it moved, so a copy that perturbed nothing is a red rather than
    a quiet pass.
    """
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(prefabs, dest)
    for member in members:
        stem = member.split("/", 1)[-1]
        path = dest / f"{stem}.json"
        if not path.is_file():
            continue
        doc = json.loads(path.read_text())
        if kind == "two-walk-planes":
            if doc.get("walk_y") is None:
                continue
            was = doc["walk_y"]
            doc["walk_y"] = was + 1
            # The piece's own waterline moves with its floor, so the copy does
            # not ALSO carry `DW0344`'s shape — a shore whose walk plane is not
            # one course over its waterline is the other perturbation's subject
            # and would muddy this one. It does then declare a waterline its
            # unmoved bytes no longer bear out, which is `DW0887` saying the same
            # thing a second way; the cell asserts the code it is AIMED at rather
            # than the whole set, so that extra reading cannot green it.
            if doc.get("waterline_y") is not None:
                doc["waterline_y"] = doc["waterline_y"] + 1
            path.write_text(json.dumps(doc, indent=2) + "\n")
            return f"{stem}.walk_y {was} -> {was + 1}"
        if kind == "shore-level-with-its-sea":
            if doc.get("walk_y") is None:
                continue
            was = doc.get("waterline_y")
            doc["waterline_y"] = doc["walk_y"]
            path.write_text(json.dumps(doc, indent=2) + "\n")
            return f"{stem}.waterline_y {was} -> {doc['waterline_y']}"
    return ""


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--delvec", help="the `delvec` this gate runs (resolved and named)")
    ap.add_argument("--prefabs", required=True, help="the prefab library to judge")
    ap.add_argument("--work", help="scratch dir (default: a temp dir, removed)")
    args = ap.parse_args()

    delvec = resolve_delvec(args.delvec, repo=REPO, caller="check-seating-agrees")
    prefabs = Path(args.prefabs).resolve()
    if not prefabs.is_dir():
        die(f"--prefabs {prefabs} is not a directory")

    tmp = None
    if args.work:
        work = Path(args.work).resolve()
        work.mkdir(parents=True, exist_ok=True)
    else:
        tmp = tempfile.mkdtemp(prefix="seating-agrees-")
        work = Path(tmp)

    bases = declared_bases(delvec)
    pools = pools_of(prefabs)
    print(f"bases (from the engine's schema): {bases}")
    print(f"pools (from {prefabs}/pools.json): {sorted(pools)}")

    cells = 0
    disagreements: list[str] = []
    site_plan_cells = 0
    codes_seen: set[str] = set()

    for base in bases:
        verdicts = {p["pool"]: p for p in seating(delvec, prefabs, base)["pools"]}
        for pool in sorted(pools):
            if pool not in verdicts:
                disagreements.append(
                    f"{pool} on {base}: the command printed no verdict for a pool "
                    f"`pools.json` declares"
                )
                continue
            codes_seen |= {r["code"] for r in verdicts[pool]["reasons"]}
            state, note = judge(delvec, prefabs, pool, base, verdicts[pool], work)
            cells += 1
            if state == "DISAGREE":
                disagreements.append(f"{pool} on {base}: {note}")
            elif state == "not-an-areas-base":
                site_plan_cells += 1
            print(f"  {pool:<28} {base:<8} {state:<18} {note.splitlines()[0]}")

    # The perturbations: each is the shape the tree carried, planted in a copy,
    # and each must be refused by BOTH answers.
    perturbed_cells = 0
    # What each perturbation is AIMED at. A cell that reds on some other code has
    # not shown this gate can see the shape it exists to catch.
    aimed_at = {"two-walk-planes": "DW0886", "shore-level-with-its-sea": "DW0344"}
    for kind in ("two-walk-planes", "shore-level-with-its-sea"):
        for pool, members in sorted(pools.items()):
            if len(members) < 2 and kind == "two-walk-planes":
                continue
            copy = work / f"lib-{kind}-{pool.replace('/', '-')}"
            moved = perturb(prefabs, copy, pool, members, kind)
            if not moved:
                disagreements.append(
                    f"{kind} on {pool}: the perturbation moved NOTHING, so the cell "
                    f"below proves nothing — every member declares no `walk_y`?"
                )
                continue
            verdicts = {p["pool"]: p for p in seating(delvec, copy, "ocean")["pools"]}
            v = verdicts[pool]
            if v["verdict"] == "SEATABLE":
                disagreements.append(
                    f"{kind} on {pool}: the command still calls it SEATABLE after "
                    f"{moved} — this is the exact shape it was measured missing"
                )
                continue
            named = {r["code"] for r in v["reasons"]}
            codes_seen |= named
            if aimed_at[kind] not in named:
                disagreements.append(
                    f"{kind} on {pool}: after {moved} the command refused it with "
                    f"{sorted(named)}, which does not include {aimed_at[kind]} — the "
                    f"shape this perturbation is aimed at"
                )
                continue
            state, note = judge(delvec, copy, pool, "ocean", v, work)
            perturbed_cells += 1
            if state != "agree":
                disagreements.append(f"{kind} on {pool}: {note}")
            print(f"  {kind:<28} {pool:<20} {state:<10} ({moved}) {note.splitlines()[0]}")

    print(
        f"seating-agreement binding: {len(pools)} pool(s) x {len(bases)} base(s) = "
        f"{cells} cell(s) judged, of which {site_plan_cells} are a base this sweep's "
        f"POOL-bound campaigns cannot reach ({NOT_AN_AREAS_BASE}: a pool states no "
        f"extent); "
        f"{perturbed_cells} perturbation cell(s) judged; "
        f"codes named by the command: {sorted(codes_seen) or 'none'}."
    )
    if tmp:
        shutil.rmtree(tmp, ignore_errors=True)

    if cells == 0:
        die("ZERO cells judged — a green over an empty population is not a pass")
    if perturbed_cells == 0:
        die(
            "ZERO perturbation cells judged, so nothing here has been shown able to "
            "see the defect it exists to catch"
        )
    if disagreements:
        for d in disagreements:
            print(f"error: {d}", file=sys.stderr)
        print(
            f"error: {len(disagreements)} disagreement(s) between the seating command "
            f"and the engine. A pool the command calls seatable must not be refused "
            f"for a reason the command could have known.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
