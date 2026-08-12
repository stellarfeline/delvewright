#!/usr/bin/env python3
"""Nothing in this repo may speak to a Minecraft server without being able to hear it.

Task #70. A command whose response nobody reads cannot fail — and three sites
proved it. `crates/admit/src/gallery.rs` emitted four legacy camelCase gamerules
and an out-of-range `text_opacity:255b`; 1.21.11 refused `admit:load` and
`admit:finish` in their entirety, so the gallery world booted with no objectives,
nothing forceloaded and nothing placed, and every test stayed green.
`tools/spike-jump-arc/measure.mjs` set `gamerule fallDamage false` one line above
a comment asserting the bot took no fall damage. `validation/warden-probe.sh`
built its pad with `doMobSpawning` and `randomTickSpeed`. None of the three read
a reply.

Two checks, each driven by a PINNED artifact rather than a list somebody typed:

1. **A live command site imports the repo's one rejection rule.** Any shell/Node
   file that invokes `rcon-cli` must reach it through `tools/lib/rcon.sh` or
   `tools/lib/rcon.mjs`, whose business is knowing what a refusal looks like. The
   rule existed, correct, inside ONE spike; the two sites that needed it next had
   nothing to reuse, which is what this check makes impossible to repeat.

2. **A `gamerule` line names a rule the pinned server actually has.** The
   accepted identifiers are read out of the vendored 1.21.11 Brigadier tree
   (`crates/compiler/data/commands-1.21.11.json`) — the same artifact the
   compiler validates every emitted line against — so this cannot drift from the
   pin, and it needs no maintained list of "the bad old names".

Exit 0 clean, 1 with findings. Both checks print their binding count: a check
that matched nothing is a finding, not a pass (CLAUDE.md).
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMMAND_TREE = ROOT / "crates/compiler/data/commands-1.21.11.json"

# The only two files allowed to name `rcon-cli`: they ARE the shared rule.
CHANNELS = {"tools/lib/rcon.sh", "tools/lib/rcon.mjs"}
# How a file declares it is using that rule.
CHANNEL_MARKERS = ("tools/lib/rcon.sh", "lib/rcon.mjs")

# Frozen lab records. `docs/experiments/` holds the scripts EXACTLY as they were
# run, beside the evidence they produced; rewriting one would make the recorded
# result irreproducible from the recorded method, which is worse than the stale
# identifier it contains. The M2 jigsaw harness raises `maxCommandChainLength`,
# a name 1.21.11 rejects — noted in that experiment's README as a limitation of
# the record, not silently corrected here.
ALLOWLIST = {
    "docs/experiments/": "frozen experiment records — the script as it was run, beside its evidence",
}

# The one legitimate reason to write a rejected command down: a NEGATIVE fixture,
# the red half of a red->green proof. It must say so on the line, with a reason,
# and every honoured exemption is printed — an override nobody can see is how a
# convenient override becomes a habit.
EXEMPT = re.compile(r"check-live-commands: allow \(([^)]+)\)")

SCAN_SUFFIXES = (".sh", ".mjs", ".js", ".ts", ".rs", ".mcfunction", ".bash")
LINE_COMMENT = {
    ".sh": "#",
    ".bash": "#",
    ".mcfunction": "#",
    ".mjs": "//",
    ".js": "//",
    ".ts": "//",
    ".rs": "//",
}


def tracked_files() -> list[str]:
    out = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return [p for p in out.splitlines() if p]


def allowed(path: str) -> str | None:
    for prefix, reason in ALLOWLIST.items():
        if path.startswith(prefix):
            return reason
    return None


def strip_comment(line: str, suffix: str) -> str:
    """Drop a line comment. Conservative on purpose: it can only ever hide a
    violation, never invent one."""
    marker = LINE_COMMENT.get(suffix)
    if not marker:
        return line
    i = line.find(marker)
    return line if i < 0 else line[:i]


def gamerule_registry() -> set[str]:
    """Every `/gamerule` identifier the pinned server accepts, from its own tree."""
    tree = json.loads(COMMAND_TREE.read_text())
    children = tree["children"]["gamerule"]["children"]
    return {name.removeprefix("minecraft:") for name in children}


def check_channels(files: list[str]) -> tuple[list[str], int]:
    """Every live-command site reaches the server through the shared rule."""
    findings: list[str] = []
    bound = 0
    for path in files:
        if path in CHANNELS or Path(path).suffix not in (".sh", ".mjs", ".js", ".ts", ".bash"):
            continue
        text = (ROOT / path).read_text(errors="replace")
        suffix = Path(path).suffix
        hits = [
            (n, line)
            for n, line in enumerate(text.splitlines(), 1)
            if "rcon-cli" in strip_comment(line, suffix)
        ]
        if not hits:
            continue
        bound += 1
        reason = allowed(path)
        if reason:
            continue
        if any(marker in text for marker in CHANNEL_MARKERS):
            continue
        n, line = hits[0]
        findings.append(
            f"{path}:{n}: invokes `rcon-cli` without the shared rejection rule\n"
            f"    {line.strip()}\n"
            f"    A reply nobody reads cannot fail. Source `tools/lib/rcon.sh` "
            f"(shell) or import `tools/lib/rcon.mjs` (node) and send through it: "
            f"`dw_rcon`/`run` asserts the server accepted the command, "
            f"`dw_rcon_probe`/`probe` is the deliberate unjudged form."
        )
    return findings, bound


def check_gamerules(
    files: list[str], registry: set[str]
) -> tuple[list[str], int, list[str]]:
    """Every `gamerule <name>` names a rule the pinned 1.21.11 server has."""
    findings: list[str] = []
    exemptions: list[str] = []
    bound = 0
    # A `gamerule` COMMAND, not the English word: the identifier must be followed
    # by a value the rule could take (`true`/`false`/an integer). Prose — "the
    # gamerule registry", "`setup.mcfunction`'s gamerule block" — never is, and a
    # dynamic name (`gamerule ${g}`) is a probe this check cannot and should not
    # judge. The cost of the narrowing is a bare `gamerule <name>` QUERY with a
    # literal legacy name, which no site in this repo writes.
    pattern = re.compile(
        r"gamerule\s+((?:minecraft:)?[A-Za-z][A-Za-z0-9_]*)\s+(?:true|false|-?\d+)\b"
    )
    for path in files:
        suffix = Path(path).suffix
        if suffix not in SCAN_SUFFIXES:
            continue
        text = (ROOT / path).read_text(errors="replace")
        if "gamerule" not in text:
            continue
        for n, line in enumerate(text.splitlines(), 1):
            for name in pattern.findall(strip_comment(line, suffix)):
                bound += 1
                bare = name.removeprefix("minecraft:")
                if bare in registry:
                    continue
                if allowed(path):
                    continue
                exempt = EXEMPT.search(line)
                if exempt:
                    exemptions.append(f"{path}:{n}: `gamerule {name}` — {exempt.group(1)}")
                    continue
                findings.append(
                    f"{path}:{n}: `gamerule {name}` is not a rule on Minecraft 1.21.11\n"
                    f"    {line.strip()}\n"
                    f"    The pin renamed the whole registry to snake_case and reworded "
                    f"several rules; the old spelling answers \"Incorrect argument for "
                    f"command\" and changes nothing. The accepted names are the literal "
                    f"children of `gamerule` in crates/compiler/data/commands-1.21.11.json."
                )
    return findings, bound, exemptions


# The rejection rule has to exist twice — shell tools source `rcon.sh`, Node
# tools import `rcon.mjs` — and two copies of one truth is exactly the shape the
# rest of this file exists to prevent. So the two are compared, not trusted:
# every reply shape one knows, the other must know.
SHAPE = re.compile(r'"(?:\^|\*)?([A-Z][^"*|^]{4,})"|\|\^?([A-Z][^"|^]{4,})')


def rejection_shapes(text: str) -> set[str]:
    """The reply prefixes a rejection rule recognises, however it spells them."""
    shapes: set[str] = set()
    for a, b in SHAPE.findall(text):
        s = (a or b).strip()
        # Drop the shell's trailing glob and the regex's closing paren.
        s = s.removesuffix("*").removesuffix(")").strip()
        if s and not s.startswith(("shellcheck", "http")):
            shapes.add(s)
    return shapes


def check_rule_parity() -> tuple[list[str], int]:
    """`rcon.sh`'s list and `rcon.mjs`'s list recognise the same refusals."""
    sh = (ROOT / "tools/lib/rcon.sh").read_text()
    mjs = (ROOT / "tools/lib/rcon.mjs").read_text()
    # Only the rule bodies, never the surrounding prose.
    sh_body = sh.split("dw_rcon_rejected()", 1)[-1].split("\n}", 1)[0]
    mjs_body = mjs.split("export const REJECTION", 1)[-1].split(");", 1)[0]
    a, b = rejection_shapes(sh_body), rejection_shapes(mjs_body)
    bound = len(a | b)
    if a == b:
        return [], bound
    only_sh, only_mjs = sorted(a - b), sorted(b - a)
    lines = [
        "tools/lib/rcon.sh and tools/lib/rcon.mjs disagree about what a refusal is\n"
        f"    only rcon.sh:  {only_sh or '(none)'}\n"
        f"    only rcon.mjs: {only_mjs or '(none)'}\n"
        "    Every private copy of this rule that has ever been found was silent "
        "on exactly the refusals its own run never provoked — the shell half and "
        "the Node half are two copies of one truth and drift the same way. Widen "
        "whichever is short; a shape is removed only when the pinned server stops "
        "producing it."
    ]
    return lines, bound


def main() -> int:
    files = tracked_files()
    registry = gamerule_registry()
    if len(registry) < 50:
        print(
            f"check-live-commands: only {len(registry)} gamerule identifiers read from "
            f"{COMMAND_TREE.relative_to(ROOT)} — the tree is not what this check thinks it is",
            file=sys.stderr,
        )
        return 1

    channel_findings, channel_bound = check_channels(files)
    gamerule_findings, gamerule_bound, exemptions = check_gamerules(files, registry)
    parity_findings, parity_bound = check_rule_parity()

    print(
        f"check-live-commands: {channel_bound} file(s) invoke rcon-cli; "
        f"{gamerule_bound} `gamerule` line(s) checked against "
        f"{len(registry)} pinned identifiers; "
        f"{parity_bound} refusal shape(s) compared across the two rule halves"
    )
    for e in exemptions:
        print(f"check-live-commands: exempt — {e}")
    if channel_bound == 0 or gamerule_bound == 0 or parity_bound == 0:
        print(
            "check-live-commands: a check that binds to nothing is vacuous — "
            "expected live command sites, gamerule lines and refusal shapes to exist",
            file=sys.stderr,
        )
        return 1

    findings = channel_findings + gamerule_findings + parity_findings
    for f in findings:
        print(f"check-live-commands: {f}", file=sys.stderr)
    if findings:
        print(f"check-live-commands: {len(findings)} finding(s)", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
