#!/usr/bin/env python3
"""No repository artifact carries an identifier a reader cannot resolve.

The sanctioned identifiers are ADR numbers, spec numbers and DW codes
(CLAUDE.md *Privacy in repo artifacts*). A task id, a pull-request number and a
dated attribution are not among them: a stranger cannot resolve any of the
three, and the third additionally records who decided something and when they
said it. A citation is also how a present-tense page turns into a changelog —
the sentence stops saying what the software IS and starts saying where the
behaviour came from.

This is a RATCHET, not a one-off tidy. The count refilled twice in the week the
first sweep ran, because merged branches carry their own citations in and no
gate looked. `FLOOR` records, per file, exactly how many occurrences that file
is still allowed to hold, and the comparison is EQUALITY in both directions:

  * more than the floor  -> red. A new citation was introduced.
  * fewer than the floor -> red. The floor has fallen and the entry must be
    lowered in the same change, which is what makes it a ratchet rather than an
    amnesty. `--tighten` rewrites the entries, and REFUSES to raise one or to
    add one — so the only way to satisfy this gate is to remove the citation.
  * a file with matches and no entry -> red.
  * an entry on a file with no matches -> red. A stale exemption is how the
    next one gets waved through.

`FLOOR` lives in this file rather than in a data file on purpose: raising a
number is then a diff to a checker, which is never a mechanical change.

Deterministic, offline, stdlib-only python3. States its binding counts; zero
files scanned, or zero bytes examined, is a red rather than a pass.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

SELF = Path(__file__).resolve()
REPO = SELF.parent.parent

# --------------------------------------------------------------------------
# What an unresolvable identifier looks like.
#
# Each kind is measured and reported separately, because they arrive by
# different routes: task ids from dispatch briefs, pull-request numbers from
# commit-message habits, dated attributions from notes written next to the
# person who said the thing.
# --------------------------------------------------------------------------

# "task #41", "tasks #103", "task-#45", "Task #78".
TASK_ID = re.compile(r"(?i)\btasks?[ \-]#\s?\d+")

# "#388", "PR #361", "PRs #301/#302/#321", "PR 2", "pull/312".
# A bare "#N" of one digit is left alone: single digits are row numbers, list
# indices and numbered sections inside the document that writes them, and this
# repository has never numbered a pull request below 10.
PR_NUMBER = re.compile(
    r"(?i)(?<![\w#])#\d{2,}\b"
    r"|(?<![\w])PRs?\s+#?\d+\b"
    r"|(?<![\w])pull(?:\s+request)?[/\s]\s*#?\d+\b"
)

# A colour literal is not a citation. Only an ALL-DIGIT run can reach PR_NUMBER
# at all (`#1e1e1e` has no word boundary after the digits), so the collision is
# exactly `#RRGGBB` / `#RRGGBBAA` written without a hex letter.
COLOUR_RUN = re.compile(r"^#(?:\d{6}|\d{8})$")

ISO_DATE = re.compile(r"\b20\d\d-[01]\d-[0-3]\d\b")

# A date is an attribution when it sits beside a person or a decision. A date
# that is a plain fact — an upstream release, the day a licence was verified, an
# ADR's own `Date:` header — is not: ADRs are the one place history legitimately
# lives, and a licence date is evidence a reader may need to re-check.
# The cue list is deliberately short. Two words that read as attributions here
# are ordinary nouns elsewhere in this repository — a *route planner*, a
# *licence verdict* — and a gate that reds correct prose teaches the people who
# see it that this red means nothing. Both are still reached through the person
# or the decision word that accompanies a real attribution.
ATTRIBUTION_CUE = re.compile(
    r"(?i)(?<![\w])(?:owner|she|her|hers|his|reviewer"
    r"|ruling|rulings|ruled|decision|decided|directive)(?![\w])"
)
ATTRIBUTION_WINDOW = 120

KINDS: dict[str, re.Pattern[str]] = {
    "task-id": TASK_ID,
    "pr-number": PR_NUMBER,
    "dated-attribution": ISO_DATE,
}

# Extensions that are text but whose content is not prose an agent reads.
SKIP_SUFFIXES = {".png", ".jpg", ".jpeg", ".gif", ".webp", ".nbt", ".zip", ".jar", ".ico"}

# --------------------------------------------------------------------------
# The one exemption, and it is not an amnesty: a file whose SUBJECT is these
# patterns. `check-crates-io-readmes.py` refuses a task id and a pull-request
# number on a crates.io front page, so its tests must write one down to prove it
# — and a gate that forbade that would forbid testing the sibling gate.
#
# The property this opt-out demands is one the defect cannot supply: the file
# must be a test whose assertion FAILS if the string stops being a citation. A
# document that merely wants to keep a citation cannot produce that.
#
# An entry carrying no match is itself a finding: an exemption that has outlived
# its reason is how the next one gets waved through.
# --------------------------------------------------------------------------
ALLOWED: dict[str, str] = {
    "tools/tests/test_check_crates_io_readmes.py":
        "fixtures that prove check-crates-io-readmes.py rejects a task id and a "
        "pull-request number on a page a stranger lands on",
    "tools/tests/test_check_unsanctioned_identifiers.py":
        "this gate's own red demonstrations; each asserts a FINDING on the "
        "string, so the file goes green here only while it stays red there",
}

# --------------------------------------------------------------------------
# The floor. Every number here may only ever fall.
#
# `crates/` is the one area still carrying citations. It is being cleared on its
# own line; these entries are deleted as that lands, and `--tighten` is the
# supported way to do it.
# --------------------------------------------------------------------------
FLOOR: dict[str, int] = {
    "crates/admit/Cargo.toml": 1,
    "crates/admit/src/gallery.rs": 3,
    "crates/admit/src/main.rs": 1,
    "crates/admit/tests/gallery.rs": 2,
    "crates/compiler/Cargo.toml": 1,
    "crates/compiler/src/analyze.rs": 3,
    "crates/compiler/src/assembled.rs": 39,
    "crates/compiler/src/branch.rs": 1,
    "crates/compiler/src/camera.rs": 5,
    "crates/compiler/src/combat.rs": 12,
    "crates/compiler/src/commands.rs": 3,
    "crates/compiler/src/daylight.rs": 3,
    "crates/compiler/src/deathplan.rs": 3,
    "crates/compiler/src/eclipse.rs": 1,
    "crates/compiler/src/edit.rs": 8,
    "crates/compiler/src/emit.rs": 153,
    "crates/compiler/src/flow.rs": 5,
    "crates/compiler/src/gates.rs": 1,
    "crates/compiler/src/lib.rs": 1,
    "crates/compiler/src/light.rs": 4,
    "crates/compiler/src/loot.rs": 2,
    "crates/compiler/src/main.rs": 6,
    "crates/compiler/src/massing.rs": 2,
    "crates/compiler/src/nav.rs": 77,
    "crates/compiler/src/plan.rs": 32,
    "crates/compiler/src/pool.rs": 1,
    "crates/compiler/src/pressable.rs": 1,
    "crates/compiler/src/snapshot.rs": 1,
    "crates/compiler/src/solver.rs": 2,
    "crates/compiler/src/stake.rs": 4,
    "crates/compiler/src/teleport.rs": 2,
    "crates/compiler/src/textfit.rs": 2,
    "crates/compiler/src/timeline.rs": 2,
    "crates/compiler/src/waypoints.rs": 14,
    "crates/compiler/tests/anchor_seal.rs": 3,
    "crates/compiler/tests/boundary_assembled.rs": 1,
    "crates/compiler/tests/branch.rs": 1,
    "crates/compiler/tests/branch_transport.rs": 1,
    "crates/compiler/tests/cast_emit.rs": 2,
    "crates/compiler/tests/cli.rs": 3,
    "crates/compiler/tests/combat.rs": 8,
    "crates/compiler/tests/common/mod.rs": 2,
    "crates/compiler/tests/daylight.rs": 2,
    "crates/compiler/tests/die_retry_fixture.rs": 2,
    "crates/compiler/tests/edit.rs": 1,
    "crates/compiler/tests/effect_root_walkers.rs": 4,
    "crates/compiler/tests/emit.rs": 7,
    "crates/compiler/tests/fixtures/lift/quests.json": 1,
    "crates/compiler/tests/fixtures/v06-edits/world-edits.json": 1,
    "crates/compiler/tests/flag_objective_roots.rs": 4,
    "crates/compiler/tests/flow.rs": 1,
    "crates/compiler/tests/flow_effect_roots.rs": 1,
    "crates/compiler/tests/fmt.rs": 1,
    "crates/compiler/tests/gate_model_roots.rs": 1,
    "crates/compiler/tests/grammar_prefab.rs": 1,
    "crates/compiler/tests/i18n_v2.rs": 5,
    "crates/compiler/tests/massing.rs": 1,
    "crates/compiler/tests/move_supersede.rs": 2,
    "crates/compiler/tests/option_width.rs": 1,
    "crates/compiler/tests/packtest_batch.rs": 2,
    "crates/compiler/tests/packtest_campaign.rs": 1,
    "crates/compiler/tests/pool_double_draw.rs": 2,
    "crates/compiler/tests/relight.rs": 2,
    "crates/compiler/tests/score_seeding.rs": 2,
    "crates/compiler/tests/seal_hint.rs": 4,
    "crates/compiler/tests/shortcut_wrong_side.rs": 1,
    "crates/compiler/tests/solver.rs": 1,
    "crates/compiler/tests/souls_bonfire.rs": 5,
    "crates/compiler/tests/souls_reseat.rs": 1,
    "crates/compiler/tests/souls_td_lanes.rs": 1,
    "crates/compiler/tests/souls_timed_gate.rs": 4,
    "crates/compiler/tests/souls_timed_gate_disarm.rs": 1,
    "crates/compiler/tests/talkto_cast_pos.rs": 1,
    "crates/compiler/tests/tick_arming.rs": 1,
    "crates/compiler/tests/timeline_effect_roots.rs": 2,
    "crates/compiler/tests/v04.rs": 4,
    "crates/compiler/tests/v06.rs": 2,
    "crates/compiler/tests/v06_checkpoints.rs": 5,
    "crates/compiler/tests/v06_cutscene.rs": 1,
    "crates/compiler/tests/v06_cutscene_freeze.rs": 1,
    "crates/compiler/tests/v06_effect_flags.rs": 1,
    "crates/compiler/tests/v06_trap_payloads.rs": 2,
    "crates/compiler/tests/v06_wave_equipment.rs": 1,
    "crates/compiler/tests/v07_held_item.rs": 1,
    "crates/compiler/tests/v08_collect_container.rs": 1,
    "crates/compiler/tests/v08_option_tooltip.rs": 1,
    "crates/compiler/tests/v09_drops.rs": 2,
    "crates/compiler/tests/v10_lift.rs": 2,
    "crates/compiler/tests/v10_on_death.rs": 1,
    "crates/compiler/tests/v10_teleport.rs": 1,
    "crates/compiler/tests/world_difficulty.rs": 1,
    "crates/dsl/src/canonical.rs": 1,
    "crates/dsl/src/chrome.rs": 1,
    "crates/dsl/src/diagnostic.rs": 16,
    "crates/dsl/src/effects.rs": 3,
    "crates/dsl/src/envelope.rs": 4,
    "crates/dsl/src/fence.rs": 3,
    "crates/dsl/src/l10n.rs": 17,
    "crates/dsl/src/registry.rs": 1,
    "crates/dsl/src/stages.rs": 54,
    "crates/dsl/src/validate.rs": 40,
    "crates/dsl/tests/common/mod.rs": 1,
    "crates/dsl/tests/dw0499_boss_respawn.rs": 2,
    "crates/dsl/tests/gate_consumers.rs": 5,
    "crates/dsl/tests/l10n_effect_roots.rs": 1,
    "crates/dsl/tests/souls_ambush.rs": 1,
    "crates/dsl/tests/souls_bonfire.rs": 2,
    "crates/dsl/tests/souls_timed_gate.rs": 1,
    "crates/dsl/tests/v06.rs": 1,
    "crates/dsl/tests/v06_cutscene.rs": 1,
    "crates/dsl/tests/v06_nested_consumers.rs": 2,
    "crates/dsl/tests/v06_wave_equipment.rs": 1,
    "crates/dsl/tests/v07_held_item.rs": 1,
    "crates/dsl/tests/v08_actor_tier.rs": 1,
    "crates/dsl/tests/v08_collect_container.rs": 1,
    "crates/dsl/tests/v08_kit_potion.rs": 1,
    "crates/dsl/tests/v08_option_tooltip.rs": 1,
    "crates/dsl/tests/v08_sealed_hint.rs": 2,
    "crates/dsl/tests/v09_drops.rs": 2,
    "crates/dsl/tests/v10_state.rs": 4,
    "crates/dsl/tests/world_difficulty.rs": 1,
    "crates/grammar/src/block.rs": 1,
    "crates/grammar/src/export.rs": 3,
    "crates/grammar/src/ir.rs": 1,
    "crates/grammar/src/main.rs": 2,
    "crates/grammar/src/model.rs": 1,
    "crates/grammar/tests/arguments.rs": 2,
    "crates/grammar/tests/cli.rs": 1,
    "crates/grammar/tests/determinism.rs": 2,
    "crates/grammar/tests/failures.rs": 1,
    "crates/grammar/tests/frames_compose.rs": 2,
    "crates/grammar/tests/mirror_reachability.rs": 4,
    "crates/render/README.md": 3,
    "crates/render/src/panorama.rs": 1,
    "crates/render/src/viewer/page.css": 1,
    "crates/schem/src/blocks.rs": 2,
    "crates/schem/src/nav.rs": 1,
    "crates/schem/src/split.rs": 1,
}


def tracked_files() -> list[Path]:
    out = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "-z"],
        capture_output=True, text=True, check=True,
    ).stdout
    paths = [REPO / p for p in out.split("\0") if p]
    return [p for p in paths if p.suffix.lower() not in SKIP_SUFFIXES]


def read_text(path: Path) -> str | None:
    try:
        data = path.read_bytes()
    except OSError:
        return None
    if b"\0" in data[:8192]:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def is_attribution(text: str, start: int, end: int) -> bool:
    """Is the cue near enough to be attaching a person to this date?

    The window is measured in CHARACTERS and crosses line ends, because prose
    wraps: `(owner directive` / newline / `2026-08-07)` is one attribution split
    by a fill, and a window clipped to the date's own line goes quiet on it.

    It stops at a BLANK LINE, because an attribution binds inside a paragraph
    and nowhere further. Without that, an ADR's `- **Source**: kickoff handoff
    (2026-07-29)` pairs with the word "owner" in the first sentence of the
    section below it — two unrelated facts a hundred characters apart.
    """
    lo = max(0, start - ATTRIBUTION_WINDOW)
    hi = min(len(text), end + ATTRIBUTION_WINDOW)
    before, after = text[lo:start], text[end:hi]
    cut = before.rfind("\n\n")
    if cut >= 0:
        before = before[cut + 2:]
    cut = after.find("\n\n")
    if cut >= 0:
        after = after[:cut]
    return bool(ATTRIBUTION_CUE.search(before + text[start:end] + after))


def hits(text: str, kind: str) -> list[tuple[int, str]]:
    """(line number, matched text) for one kind.

    `task #41` is one citation, not two: the `#41` inside a task id is claimed
    by `task-id` and never counted again as a pull-request number.
    """
    claimed = [m.span() for m in TASK_ID.finditer(text)] if kind == "pr-number" else []
    found: list[tuple[int, str]] = []
    for m in KINDS[kind].finditer(text):
        token = m.group(0)
        if kind == "pr-number":
            if COLOUR_RUN.match(token):
                continue
            if any(a <= m.start() < b for a, b in claimed):
                continue
        if kind == "dated-attribution" and not is_attribution(text, m.start(), m.end()):
            continue
        found.append((text.count("\n", 0, m.start()) + 1, token))
    return found


def scan() -> tuple[dict[str, dict[str, list[tuple[int, str]]]], int, int]:
    """{relpath: {kind: hits}}, files examined, bytes examined."""
    per_file: dict[str, dict[str, list[tuple[int, str]]]] = {}
    files = 0
    size = 0
    for path in tracked_files():
        text = read_text(path)
        if text is None:
            continue
        files += 1
        size += len(text)
        rel = path.relative_to(REPO).as_posix()
        if path == SELF:
            continue  # this file spells the patterns out; it is not a citation
        found = {k: h for k in KINDS if (h := hits(text, k))}
        if found:
            per_file[rel] = found
    return per_file, files, size


def total(found: dict[str, list[tuple[int, str]]]) -> int:
    return sum(len(v) for v in found.values())


def tighten() -> int:
    per_file, files, _ = scan()
    src = SELF.read_text(encoding="utf-8")
    new: dict[str, int] = {}
    raised = []
    for rel, found in sorted(per_file.items()):
        if rel in ALLOWED:
            continue
        n = total(found)
        old = FLOOR.get(rel)
        if old is not None and n > old:
            raised.append((rel, old, n))
        elif old is None and n > 0:
            raised.append((rel, 0, n))
        new[rel] = n
    if raised:
        print("check-unsanctioned-identifiers: REFUSING to tighten — these went UP:")
        for rel, old, n in raised:
            print(f"  {rel}: {old} -> {n}")
        print("  The floor only ever falls. Remove the new citations instead.")
        return 1
    body = "FLOOR: dict[str, int] = {"
    if new:
        body += "\n" + "".join(f'    "{rel}": {n},\n' for rel, n in sorted(new.items()))
    body += "}"
    src = re.sub(r"FLOOR: dict\[str, int\] = \{.*?\n?\}", body, src, count=1, flags=re.S)
    Path(__file__).write_text(src, encoding="utf-8")
    print(f"check-unsanctioned-identifiers: floor tightened over {files} files, "
          f"{len(new)} entries, {sum(new.values())} occurrences")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--tighten", action="store_true",
                    help="rewrite FLOOR to today's counts; refuses to raise one")
    args = ap.parse_args(argv)
    if args.tighten:
        return tighten()

    per_file, files, size = scan()

    over: list[str] = []
    under: list[str] = []
    stale: list[str] = []

    for rel in sorted(ALLOWED):
        if rel not in per_file:
            stale.append(f"  {rel}: allowlisted for {ALLOWED[rel]!r}, but carries none "
                         f"— delete the entry")

    for rel, found in sorted(per_file.items()):
        if rel in ALLOWED:
            continue
        n = total(found)
        floor = FLOOR.get(rel, 0)
        if n > floor:
            lines = []
            for kind, hs in sorted(found.items()):
                for ln, tok in hs:
                    lines.append(f"      {rel}:{ln}  [{kind}]  {tok}")
            over.append(
                f"  {rel}: {n} occurrence(s), floor {floor}\n" + "\n".join(sorted(lines))
            )
        elif n < floor:
            under.append(f"  {rel}: {n} occurrence(s), floor {floor} — lower it to {n}")

    for rel, floor in sorted(FLOOR.items()):
        if rel not in per_file:
            stale.append(f"  {rel}: floor {floor}, but the file carries none — delete the entry")

    by_kind = {k: 0 for k in KINDS}
    for rel, found in per_file.items():
        if rel in ALLOWED:
            continue
        for kind, hs in found.items():
            by_kind[kind] += len(hs)

    print("check-unsanctioned-identifiers: binding —")
    print(f"  files examined ......... {files}")
    print(f"  bytes examined ......... {size}")
    print(f"  files with occurrences . {len(per_file.keys() - ALLOWED.keys())}")
    for kind in KINDS:
        print(f"  {kind:<22} {by_kind[kind]}")
    print(f"  floor entries .......... {len(FLOOR)} ({sum(FLOOR.values())} occurrences)")
    print(f"  allowlisted files ...... {len(ALLOWED)}")

    if files == 0 or size == 0:
        print("check-unsanctioned-identifiers: FAIL — examined nothing. A check that "
              "matched no files is not a check that passed.")
        return 2

    if over:
        print("\ncheck-unsanctioned-identifiers: FAIL — a citation a reader cannot resolve.")
        print("The sanctioned identifiers are ADR numbers, spec numbers and DW codes.")
        print("Keep the OBLIGATION as a plain present-tense fact, or delete the sentence;")
        print("do not replace the citation with a note about what it used to assert.\n")
        print("\n".join(over))
    if under:
        print("\ncheck-unsanctioned-identifiers: FAIL — the floor has fallen and did not "
              "move with it. Run `python3 tools/check-unsanctioned-identifiers.py --tighten`.\n")
        print("\n".join(under))
    if stale:
        print("\ncheck-unsanctioned-identifiers: FAIL — a stale exemption pre-excuses the "
              "next citation written into that file.\n")
        print("\n".join(stale))

    if over or under or stale:
        return 1
    print("check-unsanctioned-identifiers: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
