#!/usr/bin/env python3
"""Write the handout a fresh agent reviews ONE campaign's script from (step 5b).

A script the creator invented is read once more before anything is built, by a
reader who never held the design: a fresh agent handed nothing but what a
player meets. This tool derives that reading from the campaign on every
invocation and types none of it: the script in play order (`script.md`), and
the handout beside it (`handout.md`) carrying the brief the creator was given,
the questions and the review's form, both read from the engine's
`docs/reference/script-review.md` so that a question edited there is the
question asked.

    "$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/script-review.py" <campaign-dir> \\
        --brief <file> --engine "$DELVEWRIGHT_ENGINE"

THE SCRIPT IS WHAT THE PLAYER MEETS, AND NOTHING ELSE

`script.md` carries the title and premise, the class blurbs, every quest's goal,
every objective's title, hint, visibility and answers, every dialogue tree as
the cast ledger shows it during that quest, every narration and every
player-facing effect, then everything pressed or passed in the world: triggers,
shops, signs, counters, the containers' notice, pulses. It carries nothing of
`DESIGN.md`, `GENERATION.md`, a persona or a `happening`: those are what the
author meant, and the reviewer judges what the player gets.

THE ORDER IS THE DOCUMENTS' DECLARED ORDER, AND SAYS SO

Quests are laid out by `depends_on` depth (the hub, then every line it opens,
then the finale), siblings in the plan's file order; objectives by `after`,
file order among those open together. That is a static reading of the
documents, not the compiler's replay (`Flow::journal`, which the build writes
as `validation/world-chronicle-*.md` and which does not exist before step 8);
where several lines or objectives are open together the script says so, and the
reviewer reads them as a party that may take them in any order.

WHAT IT REFUSES, SO A MISTAKE IS AN ERROR WHERE IT IS ENTERED

- a campaign without `quests.json` or `dialogue.json` (exit 1): step 5 has not
  been written, and there is no script to read;
- a campaign without `quest-plan.json`, `world.json` or `classes.json` (exit 1);
- a brief file that is missing or empty (exit 2): a review that does not know
  what the delve was asked to be judges against nothing;
- an engine tree whose `docs/reference/script-review.md` is missing, or whose
  questions or review sections yield nothing (exit 2): the questions are the
  page's to state, never this tool's to assume.

Prints the handout's path and the one-line prompt the creator launches the
agent with. Stderr carries the binding counts. Standard library only.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

EXIT_UNUSABLE = 2

OUT_DIR = pathlib.Path(".out") / "script-review"
ENGINE_PAGE = pathlib.Path("docs") / "reference" / "script-review.md"
QUESTIONS_HEADING = "## 2. The questions"
REVIEW_HEADING = "## 3. The review"

REQUIRED = ("world.json", "classes.json", "quest-plan.json", "quests.json", "dialogue.json")

LINK_RE = re.compile(r"\[([^\]]+)\]\([^)]+\)")
TAIL_RE = re.compile(r"\s\*\*(?:Cited|Authored)\b")


def die(message: str, code: int) -> "NoReturn":  # noqa: F821 — typing only
    sys.stderr.write(f"script-review: {message}\n")
    sys.exit(code)


def load(path: pathlib.Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        die(f"{path}: {e}", 1)


def content(doc: dict, path: pathlib.Path) -> dict:
    inner = doc.get("content")
    if not isinstance(inner, dict):
        die(f"{path} carries no `content` object", 1)
    return inner


# --- the engine page: the questions and the review's form -------------------


def section(text: str, heading: str) -> list[str]:
    lines = text.split("\n")
    try:
        start = lines.index(heading)
    except ValueError:
        return []
    body: list[str] = []
    for line in lines[start + 1 :]:
        if line.startswith("## "):
            break
        body.append(line)
    return body


def items(body: list[str], opener: re.Pattern[str]) -> list[str]:
    """Each list item opened by `opener`, with its continuation lines joined."""
    out: list[str] = []
    current: list[str] | None = None
    for line in body:
        if opener.match(line):
            if current:
                out.append(" ".join(current))
            current = [line.strip()]
        elif current is not None and line.startswith("  ") and line.strip():
            current.append(line.strip())
        else:
            if current:
                out.append(" ".join(current))
            current = None
    if current:
        out.append(" ".join(current))
    return out


def plain(markdown: str) -> str:
    return LINK_RE.sub(r"\1", markdown)


def read_page(engine: pathlib.Path) -> tuple[list[str], list[str]]:
    page = engine / ENGINE_PAGE
    if not page.is_file():
        die(f"{page} is not there: the engine tree carries no script-review page", EXIT_UNUSABLE)
    text = page.read_text(encoding="utf-8")
    questions = [
        plain(TAIL_RE.split(q, maxsplit=1)[0]).rstrip()
        for q in items(section(text, QUESTIONS_HEADING), re.compile(r"^- \*\*Q\d+"))
    ]
    form = [plain(f) for f in items(section(text, REVIEW_HEADING), re.compile(r"^\d+\. "))]
    if not questions:
        die(f"{page} yields no question under `{QUESTIONS_HEADING}`", EXIT_UNUSABLE)
    if not form:
        die(f"{page} yields no step under `{REVIEW_HEADING}`", EXIT_UNUSABLE)
    return questions, form


# --- the script ----------------------------------------------------------------


class Script:
    def __init__(self, campaign: pathlib.Path) -> None:
        self.campaign = campaign
        self.world = content(load(campaign / "world.json"), campaign / "world.json")
        self.classes = content(load(campaign / "classes.json"), campaign / "classes.json")
        self.plan = content(load(campaign / "quest-plan.json"), campaign / "quest-plan.json")
        self.quests = content(load(campaign / "quests.json"), campaign / "quests.json")
        dialogue = content(load(campaign / "dialogue.json"), campaign / "dialogue.json")
        npcs_path = campaign / "npcs.json"
        npcs = content(load(npcs_path), npcs_path).get("npcs", []) if npcs_path.is_file() else []
        self.npc_name = {n.get("id"): n.get("name", n.get("id")) for n in npcs if isinstance(n, dict)}
        self.trees: dict[str, dict] = {}
        self.node_tree: dict[str, str] = {}
        for tree in dialogue.get("dialogues", []):
            self.trees[tree["npc"]] = tree
            for node in tree.get("nodes", []):
                self.node_tree[node["id"]] = tree["npc"]
        self.rendered_roots: set[tuple[str, str]] = set()
        self.rendered_nodes: set[str] = set()
        self.lines = 0  # player-facing lines counted
        self.out: list[str] = []
        self.talk_roots = self.find_talk_roots()

    # -- helpers ---------------------------------------------------------------

    def name(self, npc: str | None) -> str:
        if not npc:
            return "?"
        return f"{self.npc_name.get(npc, npc)} (`{npc}`)"

    def say(self, text: str) -> str:
        self.lines += 1
        return text

    @staticmethod
    def gates(obj: dict) -> str:
        parts: list[str] = []
        for f in obj.get("requires_flags", []) or []:
            parts.append(f"only if `{f}`")
        for f in obj.get("forbids_flags", []) or []:
            parts.append(f"not if `{f}`")
        for s in obj.get("requires_state", []) or []:
            parts.append(f"`{s.get('state')}` {s.get('op')} {s.get('value')}")
        when = obj.get("when")
        if isinstance(when, dict):
            inner = Script.gates(when)
            if inner:
                parts.append(inner)
        return "; ".join(parts)

    def effect_lines(self, effects: list | None, depth: int = 0) -> list[str]:
        out: list[str] = []
        pad = "  " * depth
        for e in effects or []:
            if not isinstance(e, dict):
                continue
            kind = e.get("type")
            gate = self.gates(e)
            suffix = f" *({gate})*" if gate else ""
            if kind == "narrate":
                style = e.get("style")
                tag = f" [{style}]" if style and style != "chat" else ""
                out.append(f"{pad}- {self.say(repr_text(e.get('text', '')))}{tag}{suffix}")
            elif kind == "give-item":
                what = e.get("name") or e.get("item")
                out.append(f"{pad}- the party receives **{what}** ×{e.get('count', 1)}{suffix}")
            elif kind == "set-flag":
                out.append(f"{pad}- remembers `{e.get('flag')}`{suffix}")
            elif kind in ("add-state", "set-state"):
                sign = "+" if kind == "add-state" and isinstance(e.get("amount"), int) and e["amount"] >= 0 else ""
                value = e.get("amount", e.get("value"))
                out.append(f"{pad}- `{e.get('state')}` {sign}{value}{suffix}")
            elif kind == "move-npc":
                out.append(f"{pad}- {self.name(e.get('npc'))} walks to `{e.get('to')}`{suffix}")
            elif kind == "spawn-npc":
                out.append(f"{pad}- {self.name(e.get('npc'))} appears{suffix}")
            elif kind == "despawn-npc":
                out.append(f"{pad}- {self.name(e.get('npc'))} leaves{suffix}")
            elif kind == "campaign-complete":
                out.append(f"{pad}- **THE END — `{e.get('ending', 'the ending')}`**{suffix}")
            elif kind == "set-time":
                out.append(f"{pad}- the hour changes to `{e.get('time')}`{suffix}")
            elif kind == "set-weather":
                out.append(f"{pad}- the weather changes to `{e.get('weather')}`{suffix}")
            elif kind in ("open-gate", "close-gate", "open-way"):
                target = e.get("gate") or e.get("way") or e.get("connection") or ""
                verb = "opens" if kind != "close-gate" else "shuts"
                out.append(f"{pad}- a way {verb}: `{target}`{suffix}")
            elif kind == "cutscene":
                shots = e.get("shots") or []
                out.append(f"{pad}- a cutscene plays ({len(shots)} shot{'s' if len(shots) != 1 else ''}){suffix}")
                out.extend(self.effect_lines(e.get("then"), depth + 1))
            elif kind == "sequence":
                for step in e.get("steps") or []:
                    out.extend(self.effect_lines(step.get("effects"), depth))
            elif kind in ("spawn-wave", "volley", "ambush", "begin-stealth"):
                out.append(f"{pad}- a fight: `{e.get('wave') or kind}`{suffix}")
            elif kind in ("firework", "particle", "lightning", "play-sound", "stop-sound"):
                out.append(f"{pad}- {kind.replace('-', ' ')}{suffix}")
            elif kind == "complete-objective":
                out.append(f"{pad}- completes `{e.get('objective')}`{suffix}")
            else:
                out.append(f"{pad}- {kind}{suffix}")
        return out

    # -- dialogue --------------------------------------------------------------

    def tree_lines(self, npc: str, root: str, depth_label: str) -> list[str]:
        out: list[str] = []
        tree = self.trees.get(npc)
        if tree is None:
            return [f"{depth_label}*{self.name(npc)} has no dialogue document.*"]
        by_id = {n["id"]: n for n in tree.get("nodes", [])}
        if root not in by_id:
            return [f"{depth_label}*`{root}` is not a node of {self.name(npc)}'s dialogue.*"]
        if (npc, root) in self.rendered_roots:
            return [f"{depth_label}*{self.name(npc)} speaks from `{root}` — as above.*"]
        self.rendered_roots.add((npc, root))
        seen_here: set[str] = set()
        stack = [root]
        while stack:
            nid = stack.pop(0)
            if nid in seen_here:
                continue
            seen_here.add(nid)
            node = by_id.get(nid)
            if node is None:
                out.append(f"{depth_label}`{nid}` — *not a node of this tree*")
                continue
            if nid in self.rendered_nodes:
                out.append(f"{depth_label}`{nid}` — *as above*")
                continue
            self.rendered_nodes.add(nid)
            out.append(f"{depth_label}`{nid}`")
            out.append(f"{depth_label}> {self.say(repr_text(node.get('text', '')))}")
            options = node.get("options") or []
            if not options:
                out.append(f"{depth_label}- *(the dialog closes)*")
            for opt in options:
                label = self.say(repr_text(opt.get("label", "")))
                bits = [f"**[{label}]**"]
                if opt.get("tooltip"):
                    bits.append(f"— *{self.say(repr_text(opt['tooltip']))}*")
                gate = self.gates(opt)
                if gate:
                    bits.append(f"({gate})")
                nxt = opt.get("next")
                if nxt:
                    bits.append(f"→ `{nxt}`")
                    if nxt not in seen_here:
                        stack.append(nxt)
                else:
                    bits.append("→ *closes*")
                out.append(f"{depth_label}- " + " ".join(bits))
                out.extend(self.effect_lines(opt.get("effects"), 1))
            out.append("")
        return out

    def cast_dialogue(self, quest: dict, npc: str) -> list[tuple[object, dict | None]]:
        """Each (dialogue, placement) the cast ledger shows for `npc` in `quest`.

        A placement is a dict with `at`; a list is one placement per branch; an
        absence, or no entry at all, shows the stage-6 root. Placements that show
        the same dialogue while doing the same thing are one entry, their gates
        joined, so a reader is not handed the same lines twice.
        """
        entry = (quest.get("cast") or {}).get(npc)
        if isinstance(entry, list):
            placements = [p for p in entry if isinstance(p, dict) and "at" in p]
        elif isinstance(entry, dict) and "at" in entry:
            placements = [entry]
        else:
            placements = []
        if not placements:
            tree = self.trees.get(npc)
            return [(tree["root"], None)] if tree else []
        merged: list[tuple[object, dict]] = []
        for p in placements:
            key = (json.dumps(p.get("dialogue"), sort_keys=True), p.get("doing"))
            for dialogue, kept in merged:
                if (json.dumps(dialogue, sort_keys=True), kept.get("doing")) == key:
                    kept.setdefault("_gates", []).append(self.gates(p))
                    break
            else:
                merged.append((p.get("dialogue"), {**p, "_gates": [self.gates(p)]}))
        return merged

    def speech_lines(self, quest: dict, npc: str, at_talk: bool = False) -> list[str]:
        """What `npc` says during `quest`; at a `talk-to` the tree is written out here."""
        out: list[str] = []
        for dialogue, placement in self.cast_dialogue(quest, npc):
            where = ""
            if placement is not None:
                doing = placement.get("doing")
                gates = [g for g in placement.get("_gates", []) if g]
                gate = " / ".join(gates) if gates and len(gates) == len(placement.get("_gates", [])) else ""
                where = " — " + "; ".join(x for x in (doing, f"*{gate}*" if gate else "") if x)
            if dialogue is None or dialogue == "none":
                out.append(f"{self.name(npc)}{where}: *will not speak.*")
            elif isinstance(dialogue, dict):
                barks = dialogue.get("barks") or []
                out.append(f"{self.name(npc)}{where}: says in passing —")
                out.extend(f"- {self.say(repr_text(b))}" for b in barks if isinstance(b, str))
            elif not at_talk and (npc, str(dialogue)) in self.talk_roots and (npc, str(dialogue)) not in self.rendered_roots:
                out.append(
                    f"{self.name(npc)}{where}: speaks from `{dialogue}` — written out under the objective that sends you to them."
                )
            else:
                out.append(f"{self.name(npc)}{where}:")
                out.extend(self.tree_lines(npc, str(dialogue), ""))
        return out

    def find_talk_roots(self) -> set[tuple[str, str]]:
        """Every (npc, root) a `talk-to` objective shows, so it is written out there."""
        roots: set[tuple[str, str]] = set()
        for quest in self.quests.get("quests", []):
            if not isinstance(quest, dict):
                continue
            for o in quest.get("objectives", []):
                if isinstance(o, dict) and o.get("type") == "talk-to" and o.get("npc"):
                    for dialogue, _ in self.cast_dialogue(quest, o["npc"]):
                        if isinstance(dialogue, str) and dialogue != "none":
                            roots.add((o["npc"], dialogue))
        return roots

    # -- quests ----------------------------------------------------------------

    def plan_order(self) -> list[dict]:
        plan = [q for q in self.plan.get("quests", []) if isinstance(q, dict)]
        index = {q["id"]: i for i, q in enumerate(plan)}
        depth: dict[str, int] = {}

        def d(qid: str, trail: tuple[str, ...] = ()) -> int:
            if qid in depth:
                return depth[qid]
            if qid in trail or qid not in index:
                return 0
            deps = plan[index[qid]].get("depends_on") or []
            depth[qid] = 1 + max((d(x, trail + (qid,)) for x in deps), default=-1)
            return depth[qid]

        return sorted(plan, key=lambda q: (d(q["id"]), index[q["id"]]))

    @staticmethod
    def objective_order(objectives: list[dict]) -> list[dict]:
        ids = [o["id"] for o in objectives]
        done: set[str] = set()
        out: list[dict] = []
        remaining = list(objectives)
        while remaining:
            ready = [o for o in remaining if all(a in done or a not in ids for a in (o.get("after") or []))]
            if not ready:
                ready = [remaining[0]]  # a cycle: file order, and `validate` refuses it
            o = ready[0]
            out.append(o)
            done.add(o["id"])
            remaining.remove(o)
        return out

    def objective_lines(self, quest: dict, o: dict, n: str, titles: dict[str, str]) -> list[str]:
        kind = o.get("type", "?")
        title = o.get("title")
        out = [f"### {n} {self.say(repr_text(title)) if title else '*(untitled)*'}"]
        after = o.get("after") or []
        opens = (
            "open from the line's start"
            if not after
            else "after " + ", ".join(f"*{titles.get(a, a)}*" for a in after)
        )
        target = ""
        if kind == "talk-to":
            target = f" · speak to {self.name(o.get('npc'))}"
        elif kind in ("reach-anchor", "interact", "collect"):
            target = f" · at `{o.get('anchor')}`"
        elif kind == "kill":
            target = f" · fight `{o.get('wave')}`"
        gate = self.gates(o)
        out.append(f"*{kind}{target} · {opens}{' · ' + gate if gate else ''}*")
        if o.get("hint"):
            out.append(f"- hint: {self.say(repr_text(o['hint']))}")
        else:
            out.append("- *no hint*")
        hidden = [k for k in ("compass", "announcement") if o.get(k) == "hidden"]
        if hidden:
            out.append(f"- {' and '.join(hidden)} hidden")
        if kind == "collect":
            out.append(f"- take **{o.get('item')}** ×{o.get('count', 1)}")
        if o.get("requires_item"):
            taken = " (it is taken)" if o.get("takes_item") else ""
            out.append(f"- needs **{o['requires_item']}** in hand{taken}")
        answers = o.get("answers") or {}
        for key, label in (("not_yet", "before it is time"), ("missing_item", "without the item"), ("done", "once done")):
            if answers.get(key):
                out.append(f"- it answers, {label}: {self.say(repr_text(answers[key]))}")
        if kind == "talk-to":
            out.append("")
            out.extend(self.speech_lines(quest, o.get("npc"), at_talk=True))
        done = (quest.get("on_objective_complete") or {}).get(o["id"])
        if done:
            out.append("- when done:")
            out.extend(self.effect_lines(done, 1))
        out.append("")
        return out

    def quest_lines(self, i: int, plan_entry: dict, quest: dict | None) -> list[str]:
        qid = plan_entry["id"]
        goal = plan_entry.get("goal", "")
        kind = "mandatory" if plan_entry.get("mandatory", True) else "optional"
        deps = plan_entry.get("depends_on") or []
        opens = "at the start" if not deps else "when " + " and ".join(f"`{d}`" for d in deps) + " finished"
        out = [f"## {i}. `{qid}` — {self.say(repr_text(goal)) if goal else '*(no goal)*'}", f"*{kind} · opens {opens}*", ""]
        if quest is None:
            out.append("*Planned, but `quests.json` writes nothing for it.*")
            out.append("")
            return out
        objectives = [o for o in quest.get("objectives", []) if isinstance(o, dict)]
        titles = {o["id"]: o.get("title") or o["id"] for o in objectives}
        ordered = self.objective_order(objectives)
        open_together = [o for o in objectives if not (o.get("after") or [])]
        if len(open_together) > 1:
            out.append(
                f"*{len(open_together)} objectives open together at the line's start; the party takes them in any order.*"
            )
            out.append("")
        for j, o in enumerate(ordered, 1):
            out.extend(self.objective_lines(quest, o, f"{i}.{j}", titles))
        spoken = {o.get("npc") for o in objectives if o.get("type") == "talk-to"}
        others = [npc for npc in (quest.get("cast") or {}) if npc not in spoken]
        if others:
            out.append(f"### Also about during this line")
            out.append("")
            for npc in others:
                out.extend(self.speech_lines(quest, npc))
                out.append("")
        if quest.get("on_complete"):
            out.append("**When the line is finished:**")
            out.extend(self.effect_lines(quest["on_complete"]))
            out.append("")
        return out

    # -- the world ---------------------------------------------------------------

    def world_lines(self) -> list[str]:
        q = self.quests
        out: list[str] = []
        triggers = [t for t in q.get("triggers", []) if isinstance(t, dict)]
        if triggers:
            out += ["## Pressed or passed in the town", ""]
            for t in triggers:
                on = t.get("on") or {}
                how = on.get("on", "?") if isinstance(on, dict) else str(on)
                if isinstance(on, dict) and on.get("range") is not None:
                    how += f" within {on['range']}"
                once = ", once" if t.get("once") else ""
                gate = self.gates(t)
                out.append(f"### `{t.get('id')}` — {how}{once} at `{t.get('at')}`{' · ' + gate if gate else ''}")
                answers = t.get("answers") or {}
                for key, label in (("not_yet", "before it is time"), ("missing_item", "without the item"), ("done", "once done")):
                    if answers.get(key):
                        out.append(f"- it answers, {label}: {self.say(repr_text(answers[key]))}")
                out.extend(self.effect_lines(t.get("effects")))
                out.append("")
        shops = [s for s in q.get("shops", []) if isinstance(s, dict)]
        for s in shops:
            out += [f"## Shop `{s.get('id')}` at `{s.get('anchor')}`", ""]
            for offer in s.get("offers") or []:
                label = self.say(repr_text(offer.get("label", "")))
                tip = f" — *{self.say(repr_text(offer['tooltip']))}*" if offer.get("tooltip") else ""
                out.append(f"- **[{label}]**{tip}")
                out.extend(self.effect_lines(offer.get("effects"), 1))
            out.append("")
        signs = [s for s in q.get("signs", []) if isinstance(s, dict)]
        if signs:
            out += ["## Signs", ""]
            for s in signs:
                faces = []
                for face in ("front", "back"):
                    text = " / ".join(x for x in (s.get(face) or []) if x)
                    if text:
                        faces.append(f"{face}: {self.say(text)}")
                out.append(f"- `{s.get('id')}` at `{s.get('anchor')}` — " + "; ".join(faces))
            out.append("")
        states = [s for s in q.get("state", []) if isinstance(s, dict) and s.get("display")]
        if states:
            out += ["## Counters the player sees", ""]
            for s in states:
                out.append(f"- **{self.say(s.get('name', s.get('id')))}** (`{s.get('id')}`, {s.get('display')}), starts at {s.get('initial', 0)}")
            out.append("")
        notice = (q.get("containers") or {}).get("decoration_notice") if isinstance(q.get("containers"), dict) else None
        if isinstance(notice, dict) and notice.get("text"):
            out += ["## On arrival, about the containers", "", f"> {self.say(repr_text(notice['text']))}", ""]
        pulses = [p for p in q.get("pulses", []) if isinstance(p, dict)]
        if pulses:
            out += ["## Sounds in the air", ""]
            for p in pulses:
                gate = self.gates(p)
                out.append(f"- `{p.get('sound')}` at `{p.get('place') or p.get('at')}` every {p.get('every')} ticks{' · ' + gate if gate else ''}")
            out.append("")
        return out

    # -- the whole -----------------------------------------------------------------

    def render(self) -> str:
        w = self.world
        out = [f"# {self.say(w.get('title', self.campaign.name))} — the script, in play order", ""]
        if w.get("premise"):
            out += [f"> {self.say(repr_text(w['premise']))}", ""]
        minutes = w.get("target_minutes")
        if minutes:
            out += [f"*Built for about {minutes} minutes.*", ""]
        classes = [c for c in self.classes.get("classes", []) if isinstance(c, dict)]
        if classes:
            out += ["## Before play: the parts on offer", ""]
            for c in classes:
                kit = ", ".join(str(k.get("name") or k.get("item")) for k in (c.get("kit") or []) if isinstance(k, dict))
                out.append(f"- **{self.say(c.get('name', c.get('id')))}** — {self.say(repr_text(c.get('blurb', '')))}" + (f" *(carries: {kit})*" if kit else ""))
            out.append("")
        out += [
            "## The lines of play",
            "",
            "*Laid out by what opens what: the first line, then every line it opens — listed in the documents' order, taken by the party in any order — then the finale. Inside a line, objectives stand in the order they open.*",
            "",
        ]
        by_id = {q["id"]: q for q in self.quests.get("quests", []) if isinstance(q, dict)}
        planned = self.plan_order()
        for i, entry in enumerate(planned, 1):
            out.extend(self.quest_lines(i, entry, by_id.get(entry["id"])))
        unplanned = [q for qid, q in by_id.items() if qid not in {p["id"] for p in planned}]
        for k, q in enumerate(unplanned, len(planned) + 1):
            out.extend(self.quest_lines(k, {"id": q["id"], "goal": "", "mandatory": True, "depends_on": []}, q))
        out.extend(self.world_lines())
        self.out = out
        return "\n".join(out).rstrip() + "\n"

    def counts(self) -> dict[str, int]:
        quests = [q for q in self.plan.get("quests", []) if isinstance(q, dict)]
        objectives = sum(len(q.get("objectives", [])) for q in self.quests.get("quests", []) if isinstance(q, dict))
        nodes = sum(len(t.get("nodes", [])) for t in self.trees.values())
        return {
            "quests": len(quests),
            "mandatory": sum(1 for q in quests if q.get("mandatory", True)),
            "objectives": objectives,
            "trees": len(self.trees),
            "nodes": nodes,
            "nodes_rendered": len(self.rendered_nodes),
            "triggers": len(self.quests.get("triggers", []) or []),
            "shops": len(self.quests.get("shops", []) or []),
            "signs": len(self.quests.get("signs", []) or []),
            "lines": self.lines,
        }


def repr_text(text: object) -> str:
    """A player-facing string as one markdown line."""
    return str(text).replace("\n", " ").strip()


# --- the handout ---------------------------------------------------------------


def handout(title: str, round_no: int, brief: str, questions: list[str], form: list[str], folder: pathlib.Path) -> str:
    script = folder / "script.md"
    review = folder / "review.md"
    qs = "\n".join(questions)
    steps = "\n".join(form)
    return f"""# Script review — {title}, round {round_no}

You are a reader who has never seen this delve's design, meeting its script
the way a player meets it. Read `{script}` from top to bottom, in order, as a
party playing it would: a line that opens several objectives together is taken
in any order, and you may imagine taking them in the worst one. Then write
your review to `{review}` in the form below, and report its path and its
verdict in one line. Then exit.

**Read nothing else.** Not the campaign's documents, not its design or
generation record, not the engine, not another review. What the author meant is
not evidence; what the player meets is. Change nothing anywhere: the review is
the only file you write.

## What the delve was asked to be

{brief.strip()}

A line that breaks what this pins is a finding under Q10, whatever else it does.

## The questions

Ask these of the script in play order, then of the whole. Each is answered with
findings — where, what the player meets, why it hurts — never with a yes.

{qs}

## The review

Write `{review}` in this order:

{steps}

A finding names its place by the ids the script shows (`quest/…`, `obj/…`,
`dlg/…`, a trigger, shop or sign id) and quotes the line the player meets. Rate
severity only after every finding is written. Finish with the verdict on its own
line: `verdict: build`, `verdict: revise` or `verdict: rethink`.
"""


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("campaign", type=pathlib.Path, help="the campaign directory")
    ap.add_argument("--brief", type=pathlib.Path, required=True, help="what the idea pinned, in the creator's words")
    ap.add_argument("--engine", type=pathlib.Path, required=True, help="the engine tree: its docs/reference/script-review.md is read")
    ap.add_argument("--out", type=pathlib.Path, default=None, help=f"where the rounds are written (default <campaign>/{OUT_DIR})")
    args = ap.parse_args(argv)

    campaign: pathlib.Path = args.campaign
    if not campaign.is_dir():
        die(f"{campaign} is not a directory", 1)
    missing = [name for name in REQUIRED if not (campaign / name).is_file()]
    if missing:
        die(
            f"{campaign} has no {', '.join(missing)}: "
            + ("step 5 has not been written, so there is no script to read" if "quests.json" in missing or "dialogue.json" in missing else "not a campaign this tool can read"),
            1,
        )
    if not args.brief.is_file() or not args.brief.read_text(encoding="utf-8").strip():
        die(f"{args.brief} is missing or empty: the review must know what the delve was asked to be", EXIT_UNUSABLE)
    if not args.engine.is_dir():
        die(f"{args.engine} is not a directory", EXIT_UNUSABLE)
    questions, form = read_page(args.engine)

    out_root: pathlib.Path = args.out if args.out is not None else campaign / OUT_DIR
    out_root.mkdir(parents=True, exist_ok=True)
    existing = [p for p in out_root.glob("round-*") if p.is_dir()]
    round_no = len(existing) + 1
    folder = out_root / f"round-{round_no}"
    folder.mkdir()

    script = Script(campaign)
    text = script.render()
    (folder / "script.md").write_text(text, encoding="utf-8")
    title = script.world.get("title", campaign.name)
    (folder / "handout.md").write_text(
        handout(title, round_no, args.brief.read_text(encoding="utf-8"), questions, form, folder.resolve()),
        encoding="utf-8",
    )

    c = script.counts()
    sys.stderr.write(
        f"script review round {round_no}: {c['quests']} quest(s) ({c['mandatory']} mandatory), "
        f"{c['objectives']} objective(s), {c['trees']} dialogue tree(s) with {c['nodes_rendered']} of {c['nodes']} node(s) rendered, "
        f"{c['triggers']} trigger(s), {c['shops']} shop(s), {c['signs']} sign(s); {c['lines']} player-facing line(s); "
        f"{len(questions)} question(s) and {len(form)} review step(s) read from {ENGINE_PAGE}\n"
    )
    if c["nodes_rendered"] < c["nodes"]:
        sys.stderr.write(
            f"script review: {c['nodes'] - c['nodes_rendered']} dialogue node(s) no cast root or stage-6 root reaches are not in the script\n"
        )
    print(folder.resolve() / "handout.md")
    print(f"Read and execute {folder.resolve() / 'handout.md'}. Report, then exit.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
