#!/usr/bin/env python3
"""Write the handout a fresh agent authors ONE place's program from (spec-0121).

A map cut into boxes is detailed one box at a time, and each box's program is
written by a new agent that holds nothing but its handout. The handout is
derived here from the campaign on every invocation and typed by nobody: the
place's own allocation as `delvec allocation` prints it, that place's rows of
the campaign's design, a summary of every adjacent box drawn from the
neighbours' design rows and the seam forms, the wave bound `delvec allocation
--all` states, and — on a repair — the refusal `delvec detail --check` gives
the program the last agent left.

    "$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/handout.py" <campaign-dir> <place> \\
        --engine "$DELVEWRIGHT_ENGINE" --prefabs "$DELVEWRIGHT_PREFABS"

WHAT IT REFUSES, SO A MISTAKE IS AN ERROR WHERE IT IS ENTERED

- a place the plan allocates no box to: `delvec allocation` says so, and this
  tool passes the refusal through (exit 1) — it never invents a frame;
- a campaign with no site plan, or one `delvec` cannot parse (exit 1, delvec's
  own words);
- a place whose program ALREADY passes `delvec detail --check` (exit 3):
  there is nothing to author, and a handout would send an agent to rewrite a
  program nobody refused — the orchestrator commits it with `delvec detail`;
- a `delvec` whose `allocation --all` states no concurrency line (exit 2): the
  wave bound is the engine's to state, never this tool's to assume;
- an engine tree or a prefab directory that is not there (exit 2).

The refusal a repair hands on is never passed in by hand: when the place has a
program, this tool runs `delvec detail --check` on it — the commit's own judge,
writing nothing — and the verdict it captures is the refusal: of this place, of
this program, against the whole as it stands on disk now.

Prints the handout's path and the one-line prompt the orchestrator launches the
agent with. Stderr carries the binding counts. Standard library only.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import shutil
import subprocess
import sys

SKILL_ROOT = pathlib.Path(__file__).resolve().parents[1]

EXIT_UNUSABLE = 2
EXIT_ALREADY_DETAILED = 3

HANDOUTS_DIR = pathlib.Path(".out") / "handouts"

# The line `delvec allocation --all` prints on stderr (spec-0122 §6.2): the
# number of place authors that run at once, with the three numbers it is
# computed from. Read, never recomputed.
CONCURRENCY_RE = re.compile(r"^concurrency: (\d+) = min\((\d+), max\(1, (\d+) / 2\)\)$", re.M)
PROGRAMS_DIR = "programs"

# The references a per-place author may read, by their path under the skill
# root and the engine tree. Everything else in the campaign and the skill is
# the whole's, and the handout says so.
SKILL_REFERENCES = (
    "references/detail.md",
    "references/new-pieces.md",
    "references/reference-works.md",
)
ENGINE_REFERENCES = (
    ("docs/reference/grammar.md", "§2 is the program surface, §2b `mark`, §2c the idiom index, §2d the spatial contract"),
    ("docs/reference/prefab-procedure.md", "the procedure a piece is made by; §3 the program, §4 the gates, §5 looking at it"),
)

# The starter generator copied beside every handout (spec-0122 §6.3): an
# empty-but-correct program for any allocation, so an author starts from a
# program that already passes `detail --check` rather than from research.
STARTER = SKILL_ROOT / "scripts" / "starter.py"


class Refusal(Exception):
    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code


def run(cmd: list[str], cwd: pathlib.Path | None = None) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=False)
    except FileNotFoundError as e:
        raise Refusal(EXIT_UNUSABLE, f"cannot run `{cmd[0]}`: {e}") from e


def load_json(path: pathlib.Path) -> dict | None:
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        raise Refusal(1, f"{path}: not a JSON document: {e}") from e


def stem_of(place: str) -> str:
    return place.removeprefix("node/")


def content(doc: dict | None) -> dict:
    return (doc or {}).get("content", {}) or {}


# --------------------------------------------------------------------------
# Rendering
# --------------------------------------------------------------------------


def cells(c: list) -> str:
    """An inclusive box as `[x0,y0,z0]..[x1,y1,z1]` (piece-local)."""
    lo, hi = c
    return f"[{lo[0]},{lo[1]},{lo[2]}]..[{hi[0]},{hi[1]},{hi[2]}]"


def q(s: object) -> str:
    return "`" + str(s) + "`"


def relative_plane(mine_world_y: int, other_world_y: int) -> str:
    d = other_world_y - mine_world_y
    if d == 0:
        return "level with yours"
    if d > 0:
        return f"{d} course(s) ABOVE yours"
    return f"{-d} course(s) BELOW yours"


def owed_answer(name: str) -> str:
    """How a program answers one owed name, by the name's synthesized shape."""
    stem = name.removeprefix("anchor/")
    if name == "spawn":
        return "`mark` `spawn` — the party's spawn point; a body stands there, on walkable floor"
    if stem.startswith("seam-"):
        return (
            f"the `barred` contract edge whose `bar.region` is `{stem}` — the gate region over exactly "
            f"the bar's cells, filled with a palette role of the bar's block; no `mark`"
        )
    if stem.startswith("unlock-"):
        return f"`mark` `{stem}` — where a body stands on this side to open the gate; walkable floor beside the bar"
    if stem.startswith("node-"):
        return f"`mark` `{stem}` — where this place's body stands: the cast and the objectives quoted above are put there"
    return f"`mark` `{stem}` — a station the graph declares; a body is put there"


def allocation_json(a: dict) -> str:
    """The allocation verbatim, one line per ground column and perimeter cell.

    `ground.columns` and `ground.perimeter` are one entry per plot cell; pretty
    JSON spends five lines on each. Nothing is dropped and nothing is
    re-derived: every value is the one `delvec allocation` printed.
    """
    a = json.loads(json.dumps(a))  # a private copy to re-render
    ground = a.get("ground", {})
    columns = ground.pop("columns", None)
    perimeter = ground.pop("perimeter", None)
    fixed = ground.pop("fixed", None)
    out = [json.dumps(a, indent=2, ensure_ascii=False)]
    if columns is not None:
        out.append("")
        out.append(
            f"ground.columns ({len(columns)}; the terrain-shaped column under each plot cell, "
            f"piece-local `x z top blocks`):"
        )
        for c in columns:
            out.append(f"  {c['x']} {c['z']} {c['top']} {' '.join(c['blocks'])}")
    if perimeter is not None:
        out.append("")
        out.append(f"ground.perimeter ({len(perimeter)}; the terrain along the ring, piece-local):")
        out.append("  " + " ".join(f"[{p[0]},{p[1]},{p[2]}]" for p in perimeter))
    if fixed is not None:
        out.append("")
        out.append(
            f"ground.fixed ({len(fixed)}; ring cells the whole has already written — a void "
            f"to you, never paint one):"
        )
        for f in fixed:
            c = f["cell"]
            out.append(f"  [{c[0]},{c[1]},{c[2]}] {f['block']}")
    return "\n".join(out)


def design_rows(
    place: str,
    a: dict,
    graph: dict,
    plan: dict,
    brief: dict,
    quests: dict,
    design: dict,
    owed: set[str],
) -> tuple[list[str], int]:
    """This place's rows of the campaign's design, each quoted from its document."""
    rows: list[str] = []
    n = 0
    stem = stem_of(place)
    node = next((x for x in graph.get("nodes", []) if x.get("id") == place), None)
    if node:
        n += 1
        rows.append(
            f"- `layout-graph.json` node: intent {q(node.get('intent'))}"
            + (f"; note: {node['note']}" if node.get("note") else "; no note")
            + ("; **scenery** (`reached: false`): built to be seen, never entered" if node.get("reached") is False else "")
        )
        for s in node.get("stations", []) or []:
            n += 1
            rows.append(f"- station {q(s.get('anchor'))} ({s.get('kind')}): a named point this place owes")
    for e in graph.get("edges", []):
        if place in (e.get("a"), e.get("b")):
            n += 1
            extra = []
            if e.get("gating"):
                extra.append(f"gated {json.dumps(e['gating'], separators=(',', ':'))}")
            if e.get("shortcut"):
                extra.append("shortcut")
            if e.get("falls"):
                extra.append(f"falls {e['falls']}")
            rows.append(
                f"- connection {q(e.get('id'))}: {q(e.get('a'))} → {q(e.get('b'))}, class {q(e.get('class'))}"
                + (f" ({', '.join(extra)})" if extra else "")
            )
    cp = graph.get("critical_path", []) or []
    if place in cp:
        n += 1
        rows.append(f"- on the critical path: step {cp.index(place) + 1} of {len(cp)}")
    if graph.get("entry") == place:
        n += 1
        rows.append("- the ENTRY: the party spawns here (`spawn` is owed)")
    if graph.get("goal") == place:
        n += 1
        rows.append("- the GOAL: the whole ends here")
    for b in graph.get("beats", []):
        if b.get("node") == place:
            n += 1
            rows.append(f"- beat: objective {q(b.get('objective'))} of {q(b.get('quest'))} happens here")
    box = next((x for x in plan.get("boxes", []) if x.get("node") == place), None)
    if box:
        n += 1
        rows.append(f"- `site-plan.json` box: {json.dumps(box, separators=(',', ':'), sort_keys=True)}")
    facts = {f.get("id"): f for f in brief.get("facts", [])}
    for ident in plan.get("identities", []):
        measure = ident.get("measure") or {}
        if measure.get("node") != place:
            continue
        n += 1
        fact = facts.get(ident.get("fact"), {})
        rows.append(
            f"- the brief holds this box to a number (`DW0833`): `{ident.get('fact')}` {ident.get('cmp')} "
            f"{measure.get('of')}" + (f" on `{measure['axis']}`" if measure.get("axis") else "")
            + f" = {fact.get('value')} {fact.get('unit', '')}".rstrip()
            + (f" — {fact['note']}" if fact.get("note") else "")
            + "; the frame is the box plus its ring, so the box's own extent is what stands between the walls you draw on the ring"
        )
    for s in plan.get("seams", []):
        edge = s.get("edge")
        if any(x.get("edge") == edge for x in a.get("seams", [])):
            n += 1
            rows.append(f"- seam {q(edge)} is written as: {s.get('form', '(no form)')}")
    for quest in quests.get("quests", []):
        for npc, c in (quest.get("cast") or {}).items():
            if c.get("at") in owed:
                n += 1
                rows.append(
                    f"- cast: {q(npc)} stands at {q(c['at'])}"
                    + (f", {c['doing']}" if c.get("doing") else "")
                    + f" (quest {q(quest.get('id'))})"
                )
        for o in quest.get("objectives", []):
            if o.get("anchor") in owed:
                n += 1
                text = (o.get("happening") or {}).get("text")
                rows.append(
                    f"- objective {q(o.get('id'))} ({o.get('type')}) at {q(o['anchor'])}"
                    + (f": {text}" if text else "")
                )
    for r in design.get("references", []):
        name = r.get("name", "")
        shows = r.get("shows", "")
        if name == f"concept/{stem}" or stem in shows.split():
            n += 1
            rows.append(f"- `design.json` {q(name)}: {shows} (time {json.dumps(r.get('time'))}, weather {json.dumps(r.get('weather'))})")
    return rows, n


def neighbour_summary(
    a: dict,
    all_allocations: dict[str, dict],
    graph: dict,
    plan: dict,
    detailed: dict[str, str],
) -> tuple[list[str], int]:
    """One paragraph per adjacent box, drawn from its design rows and the seam forms."""
    out: list[str] = []
    mine_y = a["world_min"][1] + a["datum_y"]
    for nb in a.get("neighbours", []):
        other = nb["place"]
        node = next((x for x in graph.get("nodes", []) if x.get("id") == other), {}) or {}
        box = next((x for x in plan.get("boxes", []) if x.get("node") == other), {}) or {}
        theirs = all_allocations.get(other)
        out.append(f"### {other} — on your {nb['face']} side")
        what = f"intent {q(node.get('intent', '?'))}"
        if node.get("note"):
            what += f": {node['note']}"
        if node.get("reached") is False:
            what += " — scenery, never entered"
        out.append(f"- what it is: {what}")
        floor = f"its walk plane is {relative_plane(mine_y, nb['floor'])} (world y {nb['floor']})"
        if theirs:
            floor += f"; its frame is {theirs['extent'][0]}×{theirs['extent'][1]}×{theirs['extent'][2]}"
        if box.get("ceiling"):
            floor += f"; ceiling {json.dumps(box['ceiling'], separators=(',', ':'))}"
        out.append(f"- floor: {floor}")
        roof = nb.get("roof")
        roofline = (
            f"roofed, {roof[0]} course(s) of declared roof with {roof[1]} cell(s) of eaves"
            if roof
            else ("roofed, a flat lid at its ceiling (no declared roof zone)" if nb["kind"] == "roofed" else "open to the sky")
        )
        out.append(f"- roofline: {roofline}")
        voids = [v for v in a.get("voids", []) if v["owner"] == f"`{other}`"]
        if voids:
            out.append(
                "- facade: it draws its own wall into YOUR frame at "
                + ", ".join(cells(v["cells"]) for v in voids)
                + " — those cells are voids to you (listed under *Voids*); whatever it builds there shows through"
            )
        else:
            out.append("- facade: it owns no cell of your frame; the boundary between you is yours or the whole's")
        seams = [s for s in a.get("seams", []) if s["edge"] in nb.get("seams", [])]
        if not seams:
            out.append("- no seam joins you: the two claims meet, and nothing crosses")
        for s in seams:
            form = next((x.get("form") for x in plan.get("seams", []) if x.get("edge") == s["edge"]), None)
            who = "YOU own the plane: you draw the wall and cut the opening" if s["owns_plane"] else (
                f"{other} owns the plane: it draws the wall and the opening; you answer at your own first layer beside it"
            )
            line = (
                f"- seam {q(s['edge'])} ({s['class']}, on your {s['face']} face): {form or '(no form written)'}; "
                f"opening cells {cells(s['cells'])}, rise {s['rise']}, answer with {', '.join(q(c) for c in s['answer_with'])}; {who}"
            )
            if s.get("bar"):
                line += f"; the bar is yours at {cells(s['bar'])} in {q(s['bar_block'])}"
            elif theirs and any(x["edge"] == s["edge"] and x.get("bar") for x in theirs.get("seams", [])):
                line += "; the bar is theirs"
            out.append(line)
        if other in detailed:
            out.append(f"- already detailed: its piece is {q(detailed[other])}")
        else:
            out.append("- not detailed yet: it stands as the whole's stand-in until its own agent builds it")
        out.append("")
    return out, len(a.get("neighbours", []))


def render(
    *,
    campaign_dir: pathlib.Path,
    place: str,
    a: dict,
    all_allocations: dict[str, dict],
    docs: dict[str, dict],
    detailed: dict[str, str],
    engine: pathlib.Path,
    prefabs: pathlib.Path,
    attempt: int,
    refusal: str | None,
    program: pathlib.Path,
    version: str,
    handout_path: pathlib.Path,
    delvec: str,
    concurrency: str,
    starter: pathlib.Path,
    allocation_file: pathlib.Path,
) -> tuple[str, dict[str, int]]:
    stem = stem_of(place)
    world = content(docs.get("world"))
    graph = content(docs.get("layout-graph"))
    plan = content(docs.get("site-plan"))
    quests = content(docs.get("quests"))
    design = content(docs.get("design"))
    owed = set(a.get("owed_anchors", []))
    brief = content(docs.get("geometry-brief"))
    rows, n_rows = design_rows(place, a, graph, plan, brief, quests, design, owed)
    nbs, n_nbs = neighbour_summary(a, all_allocations, graph, plan, detailed)
    ex, ey, ez = a["extent"]
    work = handout_path.parent / "work"
    delvec_p = f'"{delvec}" --prefabs "{prefabs}"'
    lines: list[str] = []
    w = lines.append

    campaign_id = (docs.get("world") or {}).get("campaign_id", campaign_dir.name)
    w(f"# Handout: author `{place}` of `{campaign_id}` — attempt {attempt}")
    w("")
    w(f"This file is `{handout_path}`. It is everything you are handed, and it was written by a tool from the campaign's documents; nothing in it was typed. Instrument: {version}.")
    w("")
    w(f"You author ONE place: **{place}** of the campaign at `{campaign_dir}`, by writing its grammar program at `{program}`. Another agent authors every other place; none of them is yours to read or touch. The orchestrating session that launched you never edits a program, so the program is yours alone.")
    w("")
    w(f"Places are authored concurrently, up to the bound the engine states — `{concurrency}` — and committed one at a time by the orchestrating session. **Which neighbours are detailed is as of this handout**: your check reads the whole as it stands on disk when the check runs, so a neighbour may have landed since this file was written, and a check that passed may be answered differently once it has.")
    w("")
    generator = campaign_dir / "design" / "generator" / f"{stem}.py"
    w("## Step 1: start from the starter")
    w("")
    w(f"Beside this handout stand `{starter}` — a ready-to-run generator for this place — and `{allocation_file}`, this place's allocation as `delvec allocation` prints it. The starter reads every handed value from the allocation (datum, ground, each seam's cells and rise, the roof, the owed names) and emits an empty-but-correct program: the floor, the ring wall, the lid where the place is roofed, every seam's opening answered, every owed name marked, sconces for the light probe. **It passes `detail --check` as it stands.** So:")
    w("")
    w(f"1. Copy it to `{generator}` and run it: `\"$DELVEWRIGHT_PYTHON\" \"{generator}\" --allocation \"{allocation_file}\" --out \"{program}\"`.")
    w(f"2. Check it — `{delvec_p} detail --check \"{campaign_dir}\" {place}` — and see it green before you build anything.")
    w("3. Build the place at the points marked `# ── BUILD HERE` in your copy, each of which names the idiom (the grammar reference's §2c) and the step of the *new pieces* reference it reaches for; re-run the generator and check after every edit.")
    w("")
    w("**Nothing outside this handout's reading list is needed, and nothing outside it is read**: not the engine's source, not the engine's gallery, not another campaign. Everything the starter does not already do is in the references listed below.")
    w("")
    w("## What you may do, and what you may not")
    w("")
    w(f"- **Write** exactly one document: `{program}` (and, if you emit it from a script, that script beside it under `{campaign_dir / 'design' / 'generator'}` — the JSON is the artifact of record). Create the `{PROGRAMS_DIR}/` directory if it is absent.")
    w(f"- **Read**: this handout; your own program; the authoring references below; the engine's grammar corpus through `\"{delvec}\" grammar list` and `{delvec_p} grammar show --program <id>`.")
    w(f"- **Ask the tool again whenever you want it**: `\"{delvec}\" allocation \"{campaign_dir}\" {place}` prints the same allocation this handout carries; type nothing from it anywhere — your program reads every handed value through a `handed/…` parameter (below).")
    w(f"- **Check** with the one verb that judges a place: `{delvec_p} detail --check \"{campaign_dir}\" {place}`. It binds the handing into your program, expands it at the frame, voids every cell you do not own, runs every gate of the place and the bindings check over the whole as committed, and refuses with the cause named; it **writes nothing**. Run it as often as you like. Exit 0 says your program would commit against the whole as it stands now — that is your finished state. The commit, `delvec detail` without `--check`, is the orchestrating session's, never yours.")
    w(f"- **Look** with `{delvec_p} grammar expand --file \"{program}\" --region {ex}x{ey}x{ez} -o \"{work}/expand\"` (your `handed/…` defaults are the handed values, so no `--param` is needed) and the render commands the *new pieces* reference names. Scratch goes under `{work}/`, never `/tmp`.")
    w(f"- **Measure the palette, never guess it**: `python3 \"{engine / 'tools' / 'creator' / 'block-appearance.py'}\" --screen …` / `--mix …` / `--sheet` is yours to run (the *new pieces* reference, step 2); it needs the pinned client jar at `~/.delvewright/client/minecraft-1.21.11.jar` and says so by name when it is absent, in which case take role names from the corpus and record where each came from in your return.")
    w("- **The engine's operating half is absent here, and binds you in nothing**: the engine constitution names a `CLAUDE.local.md` beside it; on a creator's machine that file does not exist, and nothing in it governs a place program. Do not stop on its absence and do not ask about it.")
    w("- **Never**: read another place's program, the design record (`DESIGN.md`, `GENERATION.md`, `design.json`, `design/`), the site plan, the layout graph, the quest documents or another handout — what you need of them is quoted here, and the rest is the whole's; edit any document but your program; run `delvec detail` without `--check`, `delvec detail --all`, `delvec build`, a server, or `git`; retype a handed number as a literal; weaken a check or reroll a seed to get green.")
    w("- **A capability you need and the grammar lacks** is reported in your return in one sentence, with the closest thing you could build; it is never faked with adjacent constructs.")
    w("")
    w("## The authoring references")
    w("")
    for rel in SKILL_REFERENCES:
        w(f"- `{SKILL_ROOT / rel}`")
    for rel, gloss in ENGINE_REFERENCES:
        w(f"- `{engine / rel}` — {gloss}")
    w(f"- `{starter}` — your starting program's generator, and the shape every generator takes")
    w("")
    w("The *detail* reference's section *The program's contract* is the contract your program has to meet: every handed value used is declared as a parameter under the `handed/` prefix with the allocation's value as its default (`handed/datum-y`; `handed/ground/{min-y,max-y,bottom-y}`; `handed/roof/…` on a declared roof; `handed/seam/<edge stem>/{x0,y0,z0,x1,y1,z1,rise}`); every seam is answered with an opening at the handed cells and a contract edge to `exterior` whose `via` is that opening; every owed name is answered with a `mark` of that stem; a bar this place owns is a `barred` contract edge over exactly the bar's cells, its region named by the seam's stem; the piece is EXACTLY the frame's shape. `delvec detail --check` names every handed name it offers when your program asks for one it does not (`DW0882`).")
    w("")
    w("## The campaign, in one breath")
    w("")
    w(f"- title {q(world.get('title'))}; theme: {world.get('theme', '')}")
    w(f"- premise: {world.get('premise', '')}")
    w(f"- played at time {json.dumps(world.get('time'))}, weather {json.dumps(world.get('weather'))}")
    if a.get("palette"):
        w(f"- the whole's palette (role → block; a role your program declares under the same stem is rebound to it): {json.dumps(a['palette'], sort_keys=True)}")
    else:
        w("- the whole states no palette: your program's own material choices stand")
    w("")
    w(f"## This place's rows of the design ({n_rows})")
    w("")
    lines.extend(rows or ["- (no row of any document names this place beyond its box)"])
    w("")
    w("## The frame, read before anything is drawn")
    w("")
    w(f"- extent **{ex}×{ey}×{ez}** (x×y×z): the piece must be exactly this size — undersize is refused as oversize is (`DW0843`).")
    w(f"- datum (walk plane, piece-local y) **{a['datum_y']}**; play space {cells(a['space'])}; the frame stands in the world at {a['world_min']}.")
    g = a.get("ground", {})
    w(f"- ground: fill {q(g.get('fill'))}, base {json.dumps(g.get('base'))}; the claim's bottom is piece-local y {g.get('bottom_y')}, the floor course y {g.get('floor_y')}; terrain along the ring from y {g.get('min_y')} to {g.get('max_y')}; {len(g.get('fixed', []))} ring cell(s) the whole has fixed; {len(g.get('columns', []))} ground column(s) under the plot. A ground place draws its own ground down to the terrain; an aloft one (`base: {{\"aloft\": n}}`) is handed none.")
    roof = a.get("roof")
    if roof:
        w(f"- roof: **declared** — {roof['courses']} course(s), {roof['eaves']} cell(s) of eaves, lid at y {roof['lid_y']}, top at y {roof['top_y']}. **Read `roof.clipped`**: {len(roof.get('clipped', []))} eave run(s) the plan clipped at a neighbour's play space" + (": " + "; ".join(f"{cells(c[0])} at {c[1]}" for c in roof["clipped"]) if roof.get("clipped") else "") + ". A clipped cell is a void: draw no eave there.")
    elif a.get("brief", {}).get("kind") == "roofed":
        w("- roof: none declared; the place is roofed by a flat lid at its ceiling, which your program draws (the lid is the frame's top course).")
    else:
        w("- roof: none — the place is open to the sky; its declared courses of air above the play space are yours to leave open.")
    w("")
    w(f"### Seams ({len(a.get('seams', []))}) — every one is answered, none is discovered")
    w("")
    for s in a.get("seams", []):
        form = next((x.get("form") for x in plan.get("seams", []) if x.get("edge") == s["edge"]), None)
        line = (
            f"- `{s['edge']}` — {s['class']} to `{s['other']}` on your **{s['face']}** face; written as: {form or s.get('form') or '(no form)'}. "
            f"Opening cells {cells(s['cells'])} (inclusive; every cell between the corners is open), rise {s['rise']}, answer with a face of class {', '.join(q(c) for c in s['answer_with'])}. "
        )
        if s["owns_plane"]:
            line += "**You own the plane**: the wall this opening is cut in is yours to draw."
        else:
            line += f"**You do NOT own the plane**: `{s['other']}` draws that wall, so your frame stops at your own first layer beside it and the opening is answered there — the ring on that side is not yours."
        if s.get("bar"):
            line += f" **The bar is yours**: fill {cells(s['bar'])} with a palette role of `{s['bar_block']}` inside a `barred` contract edge whose `bar.region` is `seam-{s['edge'].removeprefix('edge/')}` — it exports as the gate anchor `anchor/seam-{s['edge'].removeprefix('edge/')}` (`DW0845`)."
        if s.get("ground_y") is not None:
            line += f" The ring's ground under this seam is at y {s['ground_y']}."
        w(line)
    w("")
    w(f"### Owed names ({len(a.get('owed_anchors', []))}) — each answered by a piece anchor of its stem")
    w("")
    for name in a.get("owed_anchors", []):
        w(f"- `{name}` → {owed_answer(name)}")
    w("")
    w(f"### Voids ({len(a.get('voids', []))}) — cells of your frame you do not own")
    w("")
    w("Whatever your program paints at a void is replaced by `structure_void` after expansion, so nothing you put there survives — not a wall, not a lamp, not air. Design as if the cell were not yours, because it is not: a neighbour's facade shows through it, the whole's fixed ground stands in it.")
    w("")
    for v in a.get("voids", []):
        w(f"- {cells(v['cells'])} — owned by {v['owner']}")
    w("")
    w(f"## The adjacent boxes ({n_nbs})")
    w("")
    lines.extend(nbs or ["(no neighbour: this place stands alone)", ""])
    if a.get("views"):
        w("## The plan views that see this place")
        w("")
        for v in a["views"]:
            w(f"- `{v['id']}`: {v.get('note', '')} — `{v['frame']}`")
        w("")
    if refusal is not None:
        w("## The refusal this attempt repairs")
        w("")
        w(f"A previous agent left the program at `{program}`. `delvec detail --check` refuses it against the whole as it stood when this handout was written; the verdict below is verbatim. Read the program, repair the cause the verdict names, and check again. Do not start from nothing unless the program is beyond repair.")
        w("")
        w("```text")
        lines.extend(refusal.rstrip("\n").split("\n"))
        w("```")
        w("")
    w("## The allocation, verbatim")
    w("")
    w("What `delvec allocation` hands this place. Every number your program needs is here and is read through a `handed/…` parameter, never copied.")
    w("")
    w("```json")
    lines.extend(allocation_json(a).split("\n"))
    w("```")
    w("")
    w("## Your return")
    w("")
    w(f"Report, then exit. The report is short: the place; the program's path; the exit status of your last `delvec detail --check` run and its binding line; what you built, in three lines a reviewer can picture; any capability you needed and the grammar lacks, in one sentence. Nothing else — no transcript, no design proposal for another place.")
    counts = {
        "concurrency": int(CONCURRENCY_RE.search(concurrency).group(1)),
        "design rows": n_rows,
        "neighbours": n_nbs,
        "seams": len(a.get("seams", [])),
        "voids": len(a.get("voids", [])),
        "owed names": len(a.get("owed_anchors", [])),
        "refusal lines": 0 if refusal is None else len(refusal.rstrip("\n").split("\n")),
    }
    return "\n".join(lines) + "\n", counts


# --------------------------------------------------------------------------
# The run
# --------------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("campaign_dir", type=pathlib.Path, help="the campaign directory")
    ap.add_argument("place", help="the place, a layout-graph node id (`node/<kebab>`)")
    ap.add_argument("--engine", required=True, type=pathlib.Path, help="the engine tree ($DELVEWRIGHT_ENGINE)")
    ap.add_argument("--prefabs", required=True, type=pathlib.Path, help="the prefab directory ($DELVEWRIGHT_PREFABS)")
    ap.add_argument("--delvec", default="delvec", help="the `delvec` to run (default: the one on PATH)")
    args = ap.parse_args(argv)
    try:
        return go(args)
    except Refusal as r:
        print(f"handout: {r}", file=sys.stderr)
        return r.code


def go(args: argparse.Namespace) -> int:
    campaign_dir = args.campaign_dir.expanduser().resolve()
    engine = args.engine.expanduser().resolve()
    prefabs = args.prefabs.expanduser().resolve()
    place = args.place
    if not (campaign_dir / "world.json").is_file():
        raise Refusal(EXIT_UNUSABLE, f"`{campaign_dir}` carries no `world.json`; it is not a campaign directory")
    if not (engine / "docs" / "reference" / "grammar.md").is_file():
        raise Refusal(EXIT_UNUSABLE, f"`{engine}` is not an engine tree (no `docs/reference/grammar.md`)")
    if not prefabs.is_dir():
        raise Refusal(EXIT_UNUSABLE, f"`{prefabs}` is not a directory; `--prefabs` names the prefab directory `delvec detail` writes into")
    if not place.startswith("node/"):
        raise Refusal(EXIT_UNUSABLE, f"`{place}` is not a place: a place is a layout-graph node id, `node/<kebab>`")

    # The instrument is named by its absolute path, so the agent runs the very
    # binary this handout was derived with and not whatever its own shell finds.
    delvec = shutil.which(args.delvec) or args.delvec
    if not pathlib.Path(delvec).is_file():
        raise Refusal(EXIT_UNUSABLE, f"`{args.delvec}` is not a `delvec` this shell can run (source `.out/env.sh`, or pass `--delvec`)")
    delvec = str(pathlib.Path(delvec).resolve())
    args.delvec = delvec
    version = run([delvec, "--version"])
    if version.returncode != 0:
        raise Refusal(EXIT_UNUSABLE, f"`{delvec} --version` exited {version.returncode}: {version.stderr.strip()}")
    instrument = f"{version.stdout.strip()} at `{delvec}`"

    allocated = run([args.delvec, "allocation", str(campaign_dir), "--all"])
    if allocated.returncode != 0:
        sys.stderr.write(allocated.stderr)
        raise Refusal(1, f"`delvec allocation --all` exited {allocated.returncode}; no handout is written")
    try:
        all_list = json.loads(allocated.stdout)
    except ValueError as e:
        raise Refusal(1, f"`delvec allocation --all` printed something that is not JSON: {e}") from e
    bound = CONCURRENCY_RE.search(allocated.stderr)
    if bound is None:
        raise Refusal(
            EXIT_UNUSABLE,
            f"`{delvec} allocation --all` stated no `concurrency:` line on stderr; the wave bound "
            "is the engine's to state (spec-0122), so this `delvec` predates the page — run Init I1b",
        )
    concurrency = bound.group(0)
    all_allocations = {x["place"]: x for x in all_list}
    if place not in all_allocations:
        raise Refusal(
            1,
            f"the plan allocates no box to `{place}`. The {len(all_allocations)} place(s) it does: "
            + ", ".join(f"`{p}`" for p in all_allocations),
        )
    a = all_allocations[place]

    docs = {
        name: load_json(campaign_dir / f"{name}.json")
        for name in ("world", "layout-graph", "site-plan", "geometry-brief", "quests", "design", "detail-plan")
    }
    detailed = {
        row["place"]: row["piece"] for row in content(docs.get("detail-plan")).get("details", [])
    }

    stem = stem_of(place)
    program = campaign_dir / PROGRAMS_DIR / f"{stem}.json"
    refusal: str | None = None
    if program.is_file():
        verdict = run([args.delvec, "--prefabs", str(prefabs), "detail", "--check", str(campaign_dir), place])
        if verdict.returncode == 0:
            print(
                f"handout: `{place}` already passes `delvec detail --check` from `{program}`; nothing is "
                f"owed and no handout is written — commit it with `delvec detail`.",
                file=sys.stderr,
            )
            return EXIT_ALREADY_DETAILED
        refusal = f"$ {delvec} --prefabs {prefabs} detail --check {campaign_dir} {place}\n{verdict.stdout}{verdict.stderr}exit status {verdict.returncode}\n"

    out_dir = campaign_dir / HANDOUTS_DIR / stem
    out_dir.mkdir(parents=True, exist_ok=True)
    attempt = len(list(out_dir.glob("handout-*.md"))) + 1
    handout_path = out_dir / f"handout-{attempt}.md"
    starter = out_dir / "starter.py"
    shutil.copyfile(STARTER, starter)
    allocation_file = out_dir / "allocation.json"
    allocation_file.write_text(json.dumps(a, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    text, counts = render(
        campaign_dir=campaign_dir,
        place=place,
        a=a,
        all_allocations=all_allocations,
        docs=docs,
        detailed=detailed,
        engine=engine,
        prefabs=prefabs,
        attempt=attempt,
        refusal=refusal,
        program=program,
        version=instrument,
        handout_path=handout_path,
        delvec=delvec,
        concurrency=concurrency,
        starter=starter,
        allocation_file=allocation_file,
    )
    handout_path.write_text(text, encoding="utf-8")
    if refusal is not None:
        (out_dir / f"refusal-{attempt}.txt").write_text(refusal, encoding="utf-8")
    binding = "; ".join(f"{k} {v}" for k, v in counts.items())
    print(
        f"handout binding: `{place}` attempt {attempt}, {binding}; "
        f"{'a repair of the program the last agent left' if refusal is not None else 'a first authoring (no program yet)'}; "
        f"instrument {instrument}",
        file=sys.stderr,
    )
    print(f"handout: {concurrency}", file=sys.stderr)
    print(str(handout_path))
    print(f"Read and execute {handout_path}. Report, then exit.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
