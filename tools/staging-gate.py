#!/usr/bin/env python3
"""The staging gate: refuse to stage a build for the owner while any past
finding's general form is not a live, binding check on THAT build.

## Why this exists, and why a green ladder does not discharge it

The standing obligation: a staged build is for CONTENT QC only, so every bug a
compiler or a test could catch is already fixed before anyone plays it.
`nobodys-cave-island` cost twenty-two rounds of finding MECHANICAL defects.
The reason the existing ladder cannot promise this is that most of those
findings were things **no check existed for at the time**. So
"everything is green" and "she will not find a mechanical bug" are different
claims, and only the first was measurable.

CLAUDE.md already carries the two rules that close the gap — *a finding is not
closed until its general form is a diagnostic*, and *audit the findings ledger
from round 1, never from the last round, before staging any build*. Neither was
enforced by anything. This is that enforcement.

**The question this tool asks is about COVERAGE, not about correctness.** It
re-runs nothing. For every finding ever reported, it asks: does a general-form
check exist, and does it BIND — non-zero — on the campaign about to be staged?

## The three ways a green has lied here, each with a real instance

Every verdict below names one of them, because a gate that folded them together
would be the fourth way.

- `NO-GENERAL-FORM` — the instance was fixed and the class left open. Island
  round 7's misplaced click was fixed in round 10 by moving one anchor; the
  general rule (`DW0489`, "the crosshair is a ray") landed **eleven rounds
  later** and on its first run found a second live instance the owner had by
  then hit herself.
- `MISSING-CHECK` — a general form is declared in this ledger but does not exist
  in the engine that built this tree: the code is gone from source, or has no
  test asserting it, or its artifact was never emitted. A ledger that names a
  check nobody maintains is a promise, not a proof. A probe over a stage
  document is MISSING-CHECK when the COMPILER read that document and this gate
  holds no parsed copy of it — format rot. It is **not** MISSING-CHECK merely
  because the document is absent: five of the eleven stage documents are
  optional, and a campaign that declares no edit script and no map-pipeline
  stage is not a campaign missing a document. Which of the two it is, is read
  off `manifest.json` `inputs` — the compiler's own record of what it read —
  and never off the filesystem, because a directory or an unreadable file
  standing where a document belongs is exactly what a broken campaign looks
  like. Such a campaign does not build, so it has no manifest to present.
- `UNBOUND` — the check exists and matched **zero objects** on this campaign.
  The bot's combat floor gate examined zero enemies for nineteen island rounds
  because `floor_gate.covered`, `.not_covered` and `actors[]` were all empty at
  once and nothing counted them.

Plus one the ledger's own shape can produce:

- `NO-SOURCE` — the named campaign has no DSL stage files. A campaign that
  cannot be measured is **never** a pass; the drowned-bell remake is in exactly
  this state today (`REMAKE.md`, no stage JSON), and a gate that shrugged at it
  would green-light the very build this directive was written for.

## What is deliberately NOT a red

`docs/reference/playtest-methodology.md` rule 2 permits one escape and this
honours it exactly, no wider: a finding may close with *"a declared, justified
reason none is possible"*. Such a row carries `disposition` of `no-machine-form`
(prose quality, a judgement no compiler can make) or `not-a-defect` (judged not
to be a defect at all), plus a `justification` this tool requires to be present
and substantive. Those rows do not fail the gate — they are printed in their own
section with their justification, and their COUNT is in the headline, because
rule 4 makes each one a standing risk item at every staging review. `--strict`
fails on them too, for a reviewer who wants the absolute floor.

The second non-red is not an escape at all — it is a different subject. A
**pre-detail blockout** (a site-plan campaign whose only geometry is the
derived massing; spec-0049) is staged for a walk that judges scale, pacing,
route legibility and silhouette — a build that does not claim to be finished,
and whose own artifact chain says so: the campaign's placement authority is
the site plan, the build's manifest was compiled from it, and no detail-plan
document exists in either. On such a subject a row whose class **measures
zero everywhere it could be declared** — zero binding AND zero precondition,
both counted, never asserted — is `OUT-OF-STAGE`: the walk cannot exercise
the class because this build contains none of its objects, and the build does
not pretend to be the build that could. Those rows are printed in their own
section, their count is in the headline, the admission token carries their
ids, and the boot banner names them — the owner is told what this session is
not protected from, per class, exactly as rule 4 demands. `--strict` fails on
them too. The moment the campaign leaves the blockout stage (a detail-plan
document exists), every one of these rows is adjudicated as red again: the
verdict is a statement about one stage, re-derived at every staging, never a
standing exemption.

The precondition may be a declared `applies_when`, or the binding probe's own
shape where that probe COUNTS THE OBJECT CLASS ITSELF: an identity-shaped
`dsl` predicate, or a campaign-source file glob with no `contains`, where the
file is the object and no stage document declares that a campaign has one. See
`probe_is_self_measuring` — the second clause is a bounded loosening and states
its bound there.

What the mechanism demands, and why the defect it exists to catch cannot
supply it: "this build has no combat" is proven by two measured zeros over
the campaign's own declared design plus the compiler-written record that the
world is derived massing. A build whose combat *went missing* fails at least
one of the three — declared objects make the binding non-zero (BOUND or
UNBOUND), a declared precondition surface makes the precondition non-zero
(UNBOUND), a declared-but-unemitted combat artifact is MISSING-CHECK, and a
detailed or areas-placed campaign cannot present the blockout record at all.
No operator flag, row field or disposition reaches this verdict.

That is all. There is no "skip", no "known-red", no threshold.
A finding whose general form was never built is a red, and an honest red list is
this tool's deliverable — never a reason to backfill a weak diagnostic so a row
goes green.

## Which direction it fails in

Both, and that is the point — a gate that can only fail in the direction that
never happens is decoration. Every verdict below is driven red and then green
again in `tools/tests/test_staging_gate.py`.

- The direction that **actually drifts** is a NEW finding arriving with no
  general form — a row appended after a playtest, `carrier: null`, no
  justification. That is `NO-GENERAL-FORM` and it reds the gate immediately.
  The gate is red **by default** for every new row: coverage must be argued for,
  never assumed.
- The direction that drifts **slowly** is an existing check rotting — deleted
  from source, losing its test, or its object class disappearing from the
  campaign. Those are `MISSING-CHECK` and `UNBOUND`.

## Usage

    python3 tools/staging-gate.py --campaign <campaign-dir> --build <delvec-out-dir>
        [--ledger docs/playtest-findings.json] [--report out.md] [--json out.json]
        [--strict]

`--build` is the tree `delvec build` wrote. It is required: "binds on this
build" is a question about emitted artifacts, and a ledger checked against
source alone would be exactly the compile-time-only green rule 1 warns about.

Exit 0 = every finding carries a live, binding check (or a justified
exemption). Exit 1 = at least one does not — the build is NOT stageable.
Exit 2 = usage/IO error.

Deterministic, offline, Python 3 stdlib only.
"""

import argparse
import fnmatch
import hashlib
import importlib.util
import json
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_LEDGER = REPO_ROOT / "docs" / "playtest-findings.json"

# Verdicts, in report order. The first four are reds; they are listed
# separately rather than as one "FAIL" because the remedy differs completely
# per class — build the check / fix the check / give the campaign the object /
# bump the campaign's dsl_version.
RED_VERDICTS = (
    "NO-SOURCE",
    "NO-GENERAL-FORM",
    "MISSING-CHECK",
    "UNBOUND",
    "INAPPLICABLE",
)
EXEMPT_VERDICTS = ("DECLARED-UNCOVERABLE", "OUT-OF-STAGE")
PASS_VERDICTS = ("BOUND",)

VALID_DISPOSITIONS = ("no-machine-form", "not-a-defect")
MIN_JUSTIFICATION = 24  # chars — a justification has to say something

# Sentinel for "this cache has not been filled yet", distinct from a cached
# `None` (which means "the build tree cannot answer").
_UNREAD = object()


# ---------------------------------------------------------------------------
# Reuse, never re-derive: the DW catalogue/source/test facts have one owner.
# ---------------------------------------------------------------------------


def _load_dw_checker():
    """Import `tools/check-dw-codes.py` as a module (its name has a dash).

    That tool is the single authority on which DW codes exist in source, which
    are documented, and which are asserted by a test. Re-implementing any of
    those three here would be a second answer to a settled question — the
    private-copy defect CLAUDE.md names, one layer up.
    """
    path = REPO_ROOT / "tools" / "check-dw-codes.py"
    spec = importlib.util.spec_from_file_location("dw_codes", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class Engine:
    """The engine facts a carrier's existence is decided against."""

    def __init__(self) -> None:
        dw = _load_dw_checker()
        self.dw_in_source = dw.source_codes()
        self.dw_tested = dw.tested_codes()
        self.dw_documented = set(dw.catalog_row_counts())
        self.dw_allowlisted = set(dw.ALLOWLIST)
        self._rust_text: str | None = None
        self._harness_text: str | None = None

    def _read_all(self, root: pathlib.Path, pattern: str) -> str:
        if not root.is_dir():
            return ""
        parts = []
        for p in sorted(root.rglob(pattern)):
            try:
                parts.append(p.read_text(encoding="utf-8", errors="replace"))
            except OSError:
                continue
        return "\n".join(parts)

    @property
    def rust_text(self) -> str:
        # `prefabs/` is in scope alongside `crates/`: the strongest general form
        # of several findings is a GENERATOR invariant (a sheared canopy, debris
        # stacked on a tread, an unsupported gravity floor), and those live with
        # the generators, not with the compiler.
        if self._rust_text is None:
            self._rust_text = self._read_all(
                REPO_ROOT / "crates", "*.rs"
            ) + self._read_all(REPO_ROOT / "prefabs", "*.rs")
        return self._rust_text

    @property
    def harness_text(self) -> str:
        if self._harness_text is None:
            root = REPO_ROOT / "harness"
            self._harness_text = self._read_all(root, "*.ts") + self._read_all(
                root, "*.js"
            )
        return self._harness_text

    def dw_exists(self, code: str) -> tuple[bool, str]:
        """A DW carrier exists iff it is live in source, documented, AND
        asserted by a test. Documented-but-absent is a stale promise; live but
        untested is a rule nothing exercises — either way the row's proof is
        not one this build carries."""
        if code not in self.dw_in_source:
            return False, f"{code} is not declared anywhere in crates/**/*.rs"
        if code not in self.dw_documented:
            return False, f"{code} has no diagnostics-catalog row in compiler.md"
        if code not in self.dw_tested and code not in self.dw_allowlisted:
            return False, f"{code} is asserted by no test (check-dw-codes coverage gate)"
        return True, ""


# ---------------------------------------------------------------------------
# The build + campaign under test
# ---------------------------------------------------------------------------


class Subject:
    """The campaign source and the build tree the gate is asked about."""

    STAGE_FILES = (
        "world.json",
        "npcs.json",
        "classes.json",
        "quest-plan.json",
        "quests.json",
        "dialogue.json",
        "world-edits.json",
        # The map-pipeline stage documents (spec-0049/spec-0050). Probes may
        # quantify over them, and the pre-detail determination reads them.
        "geometry-brief.json",
        "layout-graph.json",
        "site-plan.json",
        "detail-plan.json",
    )

    def __init__(self, campaign: pathlib.Path, build: pathlib.Path) -> None:
        self.campaign = campaign
        self.build = build
        self.name = campaign.name
        self.stages: dict[str, object] = {}
        self.stage_versions: dict[str, str] = {}
        self.parse_errors: list[str] = []
        self._manifest_inputs: object = _UNREAD
        for fname in self.STAGE_FILES:
            p = campaign / fname
            if not p.is_file():
                continue
            try:
                doc = json.loads(p.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError) as e:
                self.parse_errors.append(f"{fname}: {e}")
                continue
            self.stages[fname] = doc
            if isinstance(doc, dict) and isinstance(doc.get("dsl_version"), str):
                self.stage_versions[fname] = doc["dsl_version"]

    @property
    def has_source(self) -> bool:
        return bool(self.stages)

    @property
    def manifest_inputs(self) -> dict | None:
        """The stage documents the COMPILER read to build this tree, by name.

        `manifest.json` `inputs` is written by `emit::emit_manifest` from the
        byte map `load::load_campaign_dir` filled, hashed per document — so it
        is the compiler's own testimony about which documents this world was
        compiled from, not a restatement of what is on disk now. It is the one
        authority for that question; `pre_detail` and the `dsl` probe both ask
        it, and neither re-derives it.

        `None` = the build tree cannot answer (no manifest, unreadable, or no
        `inputs` object). Every caller must fail closed on `None`: "the
        compiler did not read such a document" and "I cannot tell what the
        compiler read" are the two facts this whole tool exists to keep apart.
        """
        if self._manifest_inputs is _UNREAD:
            self._manifest_inputs = None
            m = self.build / "manifest.json"
            if m.is_file():
                try:
                    inputs = json.loads(m.read_text(encoding="utf-8")).get("inputs")
                except (OSError, json.JSONDecodeError):
                    inputs = None
                if isinstance(inputs, dict):
                    self._manifest_inputs = inputs
        return self._manifest_inputs

    @property
    def pre_detail(self) -> bool:
        """Is this subject a pre-detail blockout — a site-plan campaign whose
        only geometry is the derived massing (spec-0049), staged for the walk
        that is that campaign's first gate?

        Measured from the object twice, by instruments with unrelated failure
        modes, and any disagreement is NOT a blockout (fail closed):

        - the campaign SOURCE places by site plan (`site-plan.json` is a stage
          document; DW0839 makes the two placement authorities exclusive) and
          carries no detail-stage document;
        - the BUILD's own manifest — written by the compiler, not by whoever
          runs this gate — lists `site-plan.json` among the inputs the world
          was compiled from, and no `detail-plan.json`.

        The determination is re-derived at every staging. The day a campaign
        gains a detail-plan document (spec-0050), this returns False and every
        OUT-OF-STAGE row on it reverts to red — the verdict is about a stage,
        never about a campaign.
        """
        if "site-plan.json" not in self.stages:
            return False
        if (self.campaign / "detail-plan.json").is_file():
            return False
        inputs = self.manifest_inputs
        if inputs is None:
            return False
        return "site-plan.json" in inputs and "detail-plan.json" not in inputs

    def artifact(self, rel: str) -> object | None:
        p = self.build / "validation" / rel
        if not p.is_file():
            return None
        try:
            return json.loads(p.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return None

    def artifact_exists(self, rel: str) -> bool:
        return (self.build / "validation" / rel).is_file()


# ---------------------------------------------------------------------------
# Binding probes — "how many objects of the class this check quantifies over
# does THIS campaign actually contain"
# ---------------------------------------------------------------------------


def _iter_nodes(node):
    """Every dict anywhere in a JSON document, in document order."""
    if isinstance(node, dict):
        yield node
        for v in node.values():
            yield from _iter_nodes(v)
    elif isinstance(node, list):
        for v in node:
            yield from _iter_nodes(v)


def _matches(node: dict, pred: dict) -> bool:
    for field, want in (pred.get("eq") or {}).items():
        if node.get(field) != want:
            return False
    for field, wants in (pred.get("in") or {}).items():
        if node.get(field) not in wants:
            return False
    for field, want in (pred.get("prefix") or {}).items():
        val = node.get(field)
        if not isinstance(val, str) or not val.startswith(want):
            return False
    for field in pred.get("has") or []:
        if field not in node:
            return False
    any_of = pred.get("has_any") or []
    if any_of and not any(f in node for f in any_of):
        return False
    return True


def describe(pred: dict) -> str:
    """Name the object class a predicate picks out, so a binding count reads as
    a fact about the campaign rather than as "0 nodes"."""
    bits = []
    for field, want in (pred.get("eq") or {}).items():
        bits.append(f"{field}={want}")
    for field, wants in (pred.get("in") or {}).items():
        bits.append(f"{field}∈{{{','.join(map(str, wants))}}}")
    for field, want in (pred.get("prefix") or {}).items():
        bits.append(f"{field}~{want}*")
    for field in pred.get("has") or []:
        bits.append(f"{field}:declared")
    if pred.get("has_any"):
        bits.append("any of " + "/".join(pred["has_any"]))
    return "[" + ", ".join(bits) + "]" if bits else "[any node]"


def _dig(doc, dotted: str):
    cur = doc
    for seg in dotted.split("."):
        if isinstance(cur, dict) and seg in cur:
            cur = cur[seg]
        else:
            return None
    return cur


def _absent_stage_docs(files: list, pred: dict, subj: Subject) -> tuple[int | None, str]:
    """Every stage document a `dsl` probe names is absent from this campaign.
    Is that "I could not look", or is it "this campaign declares none"?

    Both readings are live, and telling them apart is this tool's whole job.
    Five of the eleven stage documents are OPTIONAL (`compiler::load`:
    `world-edits.json`, and the four map-pipeline documents), so a campaign
    that ships none of them is not a campaign missing a document — it is a
    campaign that declares no such stage. A probe over `world-edits.json` on
    such a campaign was reading `MISSING-CHECK`: the ledger names a check the
    engine has, the campaign simply has nothing for it to quantify over, and
    the gate could not say so because it had no way to distinguish that from a
    document it failed to read.

    **The compiler answers it, and nothing else may.** `manifest.json` `inputs`
    is written by the compiler from the bytes it actually read, hashed per
    document. So:

    - a document the compiler read is IN `inputs`. If it is nonetheless not in
      `subj.stages`, this gate could not parse what the compiler could — that
      is format rot and stays `None` (MISSING-CHECK), a refusal this probe did
      not previously have at all;
    - a document in NO build's `inputs` was never read, and the campaign
      compiled without it. That is a measured zero.

    **What the defect cannot supply.** The defect this gate exists to catch is
    *nobody measured*, and it cannot present this fact:

    - a campaign that DOES declare the document has it hashed into `inputs`
      (pinned by `crates/compiler/tests/edit.rs`: "the stage-7 script is a
      hashed build input"), so it can never take this branch;
    - a campaign whose document is present but broken — a directory in its
      place, unreadable, non-UTF-8, malformed — does not build at all:
      `load::optional` treats ONLY `NotFound` as absent, by a rule that file
      states was written for exactly this class of silent wrong build. No
      build tree, no manifest, and `--build` is mandatory;
    - a build tree that cannot say what the compiler read keeps `None`.

    And the zero this returns is **not an exemption**. It is handed to the same
    unchanged adjudication as any other zero: INAPPLICABLE (red) on a finished
    campaign, and OUT-OF-STAGE only where the pre-detail blockout
    determination — itself twice-measured — already grants it. No verdict, flag,
    row field or disposition is added anywhere.
    """
    inputs = subj.manifest_inputs
    if inputs is None:
        return None, (
            f"none of {files} present in {subj.name}, and this build's "
            "manifest.json cannot say which documents the compiler read"
        )
    read_by_compiler = [f for f in files if f in inputs]
    if read_by_compiler:
        return None, (
            f"the compiler compiled this world from {read_by_compiler}, and "
            f"this gate holds no parsed copy — {'; '.join(subj.parse_errors) or 'unreadable'}"
        )
    return 0, (
        f"0 × {describe(pred)} — the compiler compiled this world from "
        f"{len(inputs)} stage document(s) and none of {files} was among them "
        "(manifest.json inputs), so this campaign declares no such stage"
    )


def glob_paths(root: pathlib.Path, pattern: str):
    """Every path under `root` whose root-relative posix name matches `pattern`.

    ONE matcher, shared by the counting probe and by the self-measuring test
    below, so the two can never disagree about which paths a glob picks out —
    a second private copy of the rule is how "zero files" and "zero objects of
    the class" quietly become answers to different questions.
    """
    if not root.is_dir():
        return
    for p in sorted(root.rglob("*")):
        if fnmatch.fnmatch(p.relative_to(root).as_posix(), pattern):
            yield p


def probe(binding: dict, subj: Subject) -> tuple[int | None, str]:
    """Return (count, detail). `None` means the probe could not run at all —
    reported as MISSING-CHECK, never silently as zero: "I could not look" and
    "I looked and found nothing" are different facts (rule 1's whole lesson)."""
    kind = binding.get("kind")

    if kind == "dsl":
        files = binding.get("files") or list(subj.stages)
        pred = binding.get("match") or {}
        n = 0
        seen_any_file = False
        for f in files:
            doc = subj.stages.get(f)
            if doc is None:
                continue
            seen_any_file = True
            n += sum(1 for node in _iter_nodes(doc) if _matches(node, pred))
        if not seen_any_file:
            return _absent_stage_docs(files, pred, subj)
        return n, f"{n} × {describe(pred)} in {', '.join(files)}"

    if kind == "artifact":
        rel = binding["file"]
        doc = subj.artifact(rel)
        if doc is None:
            return None, f"validation/{rel} was not emitted by this build"
        val = _dig(doc, binding["path"])
        if val is None:
            return None, f"validation/{rel}: no key `{binding['path']}`"
        if isinstance(val, bool):
            return None, f"validation/{rel}:{binding['path']} is a bool, not a count"
        if isinstance(val, list):
            return len(val), f"validation/{rel}:{binding['path']} = {len(val)} entries"
        if isinstance(val, (int, float)):
            return int(val), f"validation/{rel}:{binding['path']} = {int(val)}"
        return None, f"validation/{rel}:{binding['path']} is not countable"

    if kind in ("out", "campaign"):
        # `out` counts in the BUILD tree, `campaign` in the campaign source
        # tree — some checks (the storybook marker) act on files the compiler
        # reads rather than files it writes.
        root = subj.build if kind == "out" else subj.campaign
        pattern = binding["glob"]
        contains = binding.get("contains")
        rx = re.compile(contains) if contains else None
        hits = 0
        matched = 0
        for p in glob_paths(root, pattern):
            if not p.is_file():
                continue
            matched += 1
            if rx is None:
                hits += 1
                continue
            try:
                if rx.search(p.read_text(encoding="utf-8", errors="replace")):
                    hits += 1
            except OSError:
                continue
        detail = f"{hits} file(s) under {pattern}"
        if rx is not None:
            detail += f" matching /{contains}/ (of {matched} candidates)"
        return hits, detail

    return None, f"unknown binding kind `{kind}`"


def probe_is_self_measuring(binding: dict, subj: Subject) -> bool:
    """Does a zero from this probe answer the precondition question by itself?

    The question is about **what the probe counts**, never about which kind of
    probe it is. The two kinds of zero (see `adjudicate`) differ in one thing:
    whether an object exists ONE STEP BEHIND the probe's population that a
    precondition probe could still have found.

    - A `dsl` predicate selecting objects purely by IDENTITY — `eq`/`in`/
      `prefix`, nothing else — counts the object class itself, so its zero is
      the class measuring zero across the declared design. Nothing is left
      behind it to find.
    - A `campaign` glob with no `contains` counts files in the campaign
      SOURCE — the tree the author writes — and for such a class the file IS
      the object. No stage document declares that a campaign has a storybook,
      so there is no declaration standing behind `README*.md` for an
      `applies_when` to count; a row asked for one could only name its own
      binding, which `load_ledger` refuses outright as an exemption a row
      grants itself. This branch is that refusal's other half: the probe was
      already self-measuring and the recogniser was keyed to `dsl`.
    - A `contains` glob is the opposite shape and stays ambiguous: it counts a
      DECLARATION inside carriers that exist, so zero hits over N candidates is
      exactly the island's floor gate (the carriers were there; `tier` was
      not). Same for `has`/`has_any` predicates, and for `artifact`/`out`
      probes, which count derived output one step from a declaration. For
      those, which kind of zero it is must be measured by `applies_when`,
      never inferred.

    **The `campaign` branch is a LOOSENING and this is its bound.** Its only
    caller is inside the pre-detail blockout branch of `adjudicate`, so nothing
    outside a twice-measured blockout changes: the same zero on an assembled
    campaign is still `UNBOUND`. What it stops catching is a future row bound
    to a campaign-source file class that ought to exist BEFORE the walk — a
    design-approval image set, say — which would go quiet on a blockout instead
    of redding. What it still refuses is in `tools/tests/test_staging_gate.py`,
    driven in both directions per clause.

    Fail closed on the `is_file()` trap: a directory, or a broken symlink,
    standing where the file class belongs answers an honest `False` to
    `is_file()`, and the probe would then report a zero that is a wrong
    measurement rather than a measured zero. Any non-file path matching the
    glob withdraws the self-measuring claim, and the row goes back to
    `UNBOUND`.
    """
    kind = binding.get("kind")
    if kind == "dsl":
        m = binding.get("match") or {}
        if not m:
            return False
        return not (m.get("has") or m.get("has_any"))
    if kind == "campaign" and not binding.get("contains"):
        return all(p.is_file() for p in glob_paths(subj.campaign, binding["glob"]))
    return False


# ---------------------------------------------------------------------------
# Carrier existence
# ---------------------------------------------------------------------------


def carrier_exists(carrier: dict, eng: Engine, subj: Subject) -> tuple[bool, str]:
    kind = carrier.get("kind")
    if kind == "dw":
        return eng.dw_exists(carrier["code"])
    if kind == "packtest":
        name = carrier["template"]
        if name not in eng.rust_text:
            return False, f"no emitter in crates/ mentions PackTest template `{name}`"
        return True, ""
    if kind == "harness":
        needle = carrier["symbol"]
        if needle not in eng.harness_text:
            return False, f"no harness source mentions `{needle}`"
        return True, ""
    if kind == "invariant":
        # A named Rust test. The strongest general form is often not a
        # diagnostic at all but an emission invariant that makes the defect
        # unrepresentable (CLAUDE.md debug doctrine ranks a tooling default
        # ABOVE a docs line for exactly this reason). A test that no longer
        # exists is a promise nobody keeps, so the name is checked, not assumed.
        name = carrier["test"]
        if not re.search(rf"\bfn\s+{re.escape(name)}\s*\(", eng.rust_text):
            return False, f"no `fn {name}(` anywhere in crates/ or prefabs/"
        return True, ""
    if kind == "tool":
        # A CI check script. Same rule as every other carrier: named, not
        # assumed. A ledger row pointing at a tool nobody kept is a promise.
        p = REPO_ROOT / "tools" / carrier["script"]
        if not p.is_file():
            return False, f"tools/{carrier['script']} does not exist"
        return True, ""
    if kind == "artifact":
        rel = carrier["file"]
        if not subj.artifact_exists(rel):
            return False, f"this build emitted no validation/{rel}"
        return True, ""
    return False, f"unknown carrier kind `{kind}`"


def carrier_label(carrier: dict | None) -> str:
    if carrier is None:
        return "—"
    kind = carrier.get("kind")
    if kind == "dw":
        return carrier["code"]
    if kind == "packtest":
        return f"PackTest `{carrier['template']}`"
    if kind == "harness":
        return f"harness `{carrier['symbol']}`"
    if kind == "invariant":
        return f"invariant `{carrier['test']}`"
    if kind == "tool":
        return f"`tools/{carrier['script']}`"
    if kind == "artifact":
        return f"`validation/{carrier['file']}`"
    return str(carrier)


# ---------------------------------------------------------------------------
# Adjudication
# ---------------------------------------------------------------------------


def adjudicate(row: dict, eng: Engine, subj: Subject) -> dict:
    out = {
        "id": row["id"],
        "campaign": row.get("campaign", ""),
        "round": row.get("round"),
        "finding": row["finding"],
        "triage": row.get("triage", ""),
        "general_form": row.get("general_form") or "",
        "carrier": carrier_label(row.get("carrier")),
        "binding": None,
        "precondition": None,
        "verdict": "",
        "detail": "",
    }

    # A campaign with no DSL source cannot be measured, and an unmeasurable
    # build is never stageable. Reported per row so the artifact still shows
    # the full ledger the reviewer has to read.
    if not subj.has_source:
        out["verdict"] = "NO-SOURCE"
        out["detail"] = (
            f"campaign `{subj.name}` has no DSL stage files — nothing to bind against"
        )
        return out

    carrier = row.get("carrier")
    if carrier is None:
        disp = row.get("disposition")
        just = (row.get("justification") or "").strip()
        if disp in VALID_DISPOSITIONS and len(just) >= MIN_JUSTIFICATION:
            out["verdict"] = "DECLARED-UNCOVERABLE"
            out["detail"] = f"{disp}: {just}"
            return out
        out["verdict"] = "NO-GENERAL-FORM"
        if disp in VALID_DISPOSITIONS:
            out["detail"] = (
                f"disposition `{disp}` carries no substantive justification "
                f"(need ≥{MIN_JUSTIFICATION} chars); a bare label is not a reason"
            )
        else:
            out["detail"] = (
                "the instance was fixed; no general-form check was ever built, "
                "and no justified exemption is declared"
            )
        return out

    ok, why = carrier_exists(carrier, eng, subj)
    if not ok:
        out["verdict"] = "MISSING-CHECK"
        out["detail"] = why
        return out

    binding = row["binding"]
    count, detail = probe(binding, subj)
    out["detail"] = detail
    if count is None:
        # An artifact this build did not emit is "I could not look" — except
        # when the row also declares a precondition probe, which CAN look: a
        # zero precondition explains the absence (the compiler emits these
        # ledgers only over objects that exist), while a non-zero one is the
        # loud version of the same verdict — the campaign declares the class
        # and the build lost its ledger. Any other None stays MISSING-CHECK:
        # a present-but-unreadable artifact is format rot, not absence.
        aw = row.get("applies_when")
        if (
            binding.get("kind") == "artifact"
            and not subj.artifact_exists(binding["file"])
            and aw is not None
        ):
            pre, pre_detail = probe(aw, subj)
            if pre is None:
                out["verdict"] = "MISSING-CHECK"
                out["detail"] = f"`applies_when` probe could not run: {pre_detail}"
                return out
            out["precondition"] = pre
            if pre > 0:
                out["verdict"] = "MISSING-CHECK"
                out["detail"] = (
                    f"{detail}, yet {pre_detail} exist that the artifact should "
                    "cover — the campaign declares the class and the build "
                    "emitted no ledger for it"
                )
                return out
            out["binding"] = 0
            out["verdict"] = "OUT-OF-STAGE" if subj.pre_detail else "INAPPLICABLE"
            out["detail"] = (
                f"{detail}; the defect class needs {pre_detail}, and this "
                "campaign declares none — nothing here can exercise the class"
            )
            return out
        out["verdict"] = "MISSING-CHECK"
        return out
    out["binding"] = count
    if count > 0:
        out["verdict"] = "BOUND"
        return out

    # A zero binding has two causes and they are NOT the same fact.
    #
    #   - The object class is simply absent from this campaign: a delve with no
    #     timed gate cannot have a timed-gate defect. INAPPLICABLE.
    #   - Objects that COULD carry the defect exist, but the declaration the
    #     check keys off is missing from them. UNBOUND — and this is the exact
    #     shape of the island's nineteen-round vacuous green: hostile actors
    #     existed the whole time, `tier` did not, so the floor gate examined
    #     zero enemies and reported nothing.
    #
    # Both are REDS. `applies_when` names WHICH zero this is, never excuses it:
    # an exemption a row can grant itself by declaring its own binding class as
    # its own precondition is not a gate, and "the class cannot occur here" is
    # precisely what the owner needs told rather than folded away. A finished
    # campaign that cannot exercise a past defect class is a build her session
    # is not protected on — that is a fact for the round summary, not a pass.
    aw = row.get("applies_when")
    if aw is None:
        # No declared precondition probe. Where the probe COUNTS THE OBJECT
        # CLASS ITSELF — an identity-shaped predicate over the declared
        # design, or a campaign-source file class where the file is the
        # object — it measures its own precondition, and on a pre-detail
        # blockout that measured double zero is OUT-OF-STAGE. Everywhere else,
        # and for every declaration- or derivation-shaped probe, the gate keeps
        # refusing to guess.
        if subj.pre_detail and probe_is_self_measuring(row["binding"], subj):
            why = (
                "the probe counts a campaign-source file class, where the file "
                "IS the object and no declaration stands behind it"
                if row["binding"].get("kind") == "campaign"
                else "the probe selects the object class by identity"
            )
            out["precondition"] = 0
            out["verdict"] = "OUT-OF-STAGE"
            out["detail"] = (
                f"{detail}; {why}, so its zero is the class measuring zero "
                "across the declared design of a pre-detail blockout — this "
                "walk cannot exercise the class, and this build does not claim "
                "to be the build that could"
            )
            return out
        out["verdict"] = "UNBOUND"
        out["detail"] = (
            f"{detail} — and the row declares no `applies_when` probe, so which "
            "kind of zero this is was never measured"
        )
        return out
    pre, pre_detail = probe(aw, subj)
    if pre is None:
        out["verdict"] = "MISSING-CHECK"
        out["detail"] = f"`applies_when` probe could not run: {pre_detail}"
        return out
    out["precondition"] = pre
    if pre == 0:
        out["verdict"] = "OUT-OF-STAGE" if subj.pre_detail else "INAPPLICABLE"
        out["detail"] = (
            f"{detail}; the defect class needs {pre_detail}, and this campaign "
            "declares none — nothing here can exercise the class"
        )
        return out
    out["verdict"] = "UNBOUND"
    out["detail"] = (
        f"{detail}, but {pre_detail} exist that could carry this defect — "
        "the check is inert over objects it should have something to say about"
    )
    return out


# ---------------------------------------------------------------------------
# Ledger loading + self-validation
# ---------------------------------------------------------------------------


def load_ledger(path: pathlib.Path) -> dict:
    doc = json.loads(path.read_text(encoding="utf-8"))
    rows = doc.get("findings")
    if not isinstance(rows, list) or not rows:
        raise ValueError(f"{path}: `findings` must be a non-empty list")
    seen = set()
    for r in rows:
        for k in ("id", "finding"):
            if not r.get(k):
                raise ValueError(f"{path}: a row is missing `{k}`: {r!r}")
        if r["id"] in seen:
            raise ValueError(f"{path}: duplicate finding id `{r['id']}`")
        seen.add(r["id"])
        if r.get("carrier") is not None and not r.get("binding"):
            raise ValueError(
                f"{path}: row `{r['id']}` names a carrier but no binding probe — "
                "a carrier with no binding count is the vacuity this gate exists "
                "to expose, so it may not be declared"
            )
        # A `dsl` probe may only name a document this gate knows how to read.
        # Without this, the measured-zero branch in `_absent_stage_docs` turns a
        # MISTYPED filename into a silent zero: no build's manifest lists
        # `worldedits.json`, so the row would measure zero everywhere and go
        # quiet on every campaign forever. The widening and this guard are one
        # act — a name outside the vocabulary is a ledger defect, and it fails
        # loudly at load, for every campaign, before any verdict is reached.
        for key in ("binding", "applies_when"):
            b = r.get(key)
            if not isinstance(b, dict) or b.get("kind") != "dsl":
                continue
            for f in b.get("files") or []:
                if f not in Subject.STAGE_FILES:
                    raise ValueError(
                        f"{path}: row `{r['id']}` `{key}` names `{f}`, which is "
                        "not a stage document this gate reads — a document no "
                        "campaign can declare measures zero on every campaign, "
                        "which is a check that has gone quiet, not a check"
                    )
        if r.get("applies_when") is not None and r.get("applies_when") == r.get("binding"):
            raise ValueError(
                f"{path}: row `{r['id']}` declares its own binding probe as its "
                "own precondition — an exemption a row can grant itself is not "
                "a gate, so it may not be declared"
            )
    return doc


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------


def render_report(doc: dict, subj: Subject, results: list[dict], strict: bool) -> str:
    by = {v: [r for r in results if r["verdict"] == v] for v in
          RED_VERDICTS + EXEMPT_VERDICTS + PASS_VERDICTS}
    reds = [r for r in results if r["verdict"] in RED_VERDICTS]
    exempt = [r for r in results if r["verdict"] in EXEMPT_VERDICTS]
    if strict:
        reds = reds + exempt

    L = []
    L.append(f"# Staging gate — `{subj.name}`")
    L.append("")
    L.append(f"- Campaign source: `{subj.campaign}`")
    L.append(f"- Build tree: `{subj.build}`")
    L.append(f"- Ledger: {len(results)} finding(s), {doc.get('ledger_version', '?')}")
    versions = ", ".join(f"{k} {v}" for k, v in sorted(subj.stage_versions.items()))
    L.append(f"- Declared dsl_version: {versions or '(none — no source)'}")
    stage = (
        "pre-detail blockout (site-plan placement authority; the only geometry "
        "is the derived massing — spec-0049)"
        if subj.pre_detail
        else "assembled (this build claims its content is complete)"
    )
    L.append(f"- Subject stage: {stage}")
    L.append("")
    L.append("## Verdict")
    L.append("")
    verdict = "REFUSED — do not stage" if reds else "STAGEABLE"
    L.append(f"**{verdict}**")
    L.append("")
    for v in RED_VERDICTS:
        L.append(f"- `{v}`: {len(by[v])}")
    n_unc = len(by["DECLARED-UNCOVERABLE"])
    L.append(
        f"- `DECLARED-UNCOVERABLE`: {n_unc} "
        "(justified; each is a standing risk item at this staging review)"
    )
    n_oos = len(by["OUT-OF-STAGE"])
    L.append(
        f"- `OUT-OF-STAGE`: {n_oos} "
        "(measured double zero on a pre-detail blockout; each is a class this "
        "walk cannot exercise, re-adjudicated at every staging)"
    )
    L.append(f"- `BOUND`: {len(by['BOUND'])}")
    L.append("")

    L.append("## Per-finding coverage")
    L.append("")
    L.append(
        "| # | Rd | Finding | General form | Check | Binds | Verdict |"
    )
    L.append("|---|----|---------|--------------|-------|-------|---------|")
    for r in results:
        binds = "—" if r["binding"] is None else str(r["binding"])
        if r["verdict"] in ("INAPPLICABLE", "OUT-OF-STAGE"):
            binds = f"0 / pre {r['precondition']}"
        rnd = "" if r["round"] is None else f"r{r['round']}"
        L.append(
            f"| {r['id']} | {rnd} | {esc(r['finding'])} | {esc(r['general_form']) or '—'} "
            f"| {esc(r['carrier'])} | {binds} | **{r['verdict']}** |"
        )
    L.append("")

    if reds:
        L.append("## Red list — every row that blocks staging")
        L.append("")
        for r in reds:
            L.append(f"### {r['id']} — `{r['verdict']}`")
            L.append("")
            L.append(f"- Finding: {r['finding']}")
            if r["general_form"]:
                L.append(f"- General form: {r['general_form']}")
            L.append(f"- Check: {r['carrier']}")
            L.append(f"- Why: {r['detail']}")
            L.append("")

    inap = by["INAPPLICABLE"]
    if inap:
        L.append("## Inapplicable — the precondition measures zero on this campaign")
        L.append("")
        L.append(
            "These are REDS, split out because their remedy differs: the campaign "
            "declares none of the objects the class needs, so no check on this "
            "build can exercise it. Read them as the list of past defects the "
            "owner's session on this campaign is NOT protected from."
        )
        L.append("")
        for r in inap:
            L.append(f"- **{r['id']}** — {r['finding']} — _{r['detail']}_")
        L.append("")

    oos = by["OUT-OF-STAGE"]
    if oos:
        L.append("## Out of stage — classes this blockout walk cannot exercise")
        L.append("")
        L.append(
            "This build is a pre-detail blockout: its placement authority is "
            "the site plan and its only geometry is the derived massing, so "
            "the walk it is staged for judges scale, pacing, routes and "
            "silhouette. Each row below measured ZERO objects of its class "
            "across the whole declared design (binding and precondition both "
            "counted). The owner's walk is not protected from these classes "
            "and cannot meet them; every one is re-adjudicated — as a red — "
            "the moment this campaign leaves the blockout stage."
        )
        L.append("")
        for r in oos:
            L.append(f"- **{r['id']}** — {r['finding']} — _{r['detail']}_")
        L.append("")

    unc = by["DECLARED-UNCOVERABLE"]
    if unc:
        L.append("## Declared uncoverable — no machine form is possible")
        L.append("")
        L.append(
            "These do not fail the gate (playtest-methodology.md rule 2 permits "
            "*a declared, justified reason none is possible*). Rule 4 makes each "
            "one a risk item to read aloud at this staging review."
        )
        L.append("")
        for r in unc:
            L.append(f"- **{r['id']}** — {r['finding']} — _{r['detail']}_")
        L.append("")

    return "\n".join(L) + "\n"


def esc(s: str) -> str:
    return str(s).replace("|", "\\|").replace("\n", " ")


def build_fingerprint(build: pathlib.Path) -> str | None:
    """This build tree's identity, for an admission token to bind to.

    `manifest.json` is the compiler's reproducibility index over the WHOLE
    output tree, so hashing it identifies the build without walking it — and a
    rebuilt or edited tree cannot reuse an older token. A tree with no manifest
    has no identity and therefore cannot be admitted at all.
    """
    m = build / "manifest.json"
    if not m.is_file():
        return None
    return hashlib.sha256(m.read_bytes()).hexdigest()


def write_admission(
    path: pathlib.Path,
    subj: Subject,
    results: list[dict],
    fingerprint: str,
    ledger_digest: str,
    override: dict | None,
) -> None:
    """Mint the token the owner-facing paths require.

    The token is not a receipt that the gate ran — it is a statement about ONE
    build tree. It carries the fingerprint so a verifier can refuse a token
    minted for a different build, which is what stops the obvious bypass: run
    the gate green on some tree, then serve another.
    """
    reds = [r for r in results if r["verdict"] in RED_VERDICTS]
    oos = [r for r in results if r["verdict"] == "OUT-OF-STAGE"]
    doc = {
        "schema": 1,
        "campaign": subj.name,
        "build_fingerprint": fingerprint,
        "ledger_digest": ledger_digest,
        "findings_total": len(results),
        "red_count": len(reds),
        "reds": [{"id": r["id"], "verdict": r["verdict"]} for r in reds],
        # A pre-detail blockout's admission names, per class, what the walk
        # cannot exercise — the boot banner reads these, so the session's
        # scope is announced rather than remembered.
        "pre_detail": subj.pre_detail,
        "out_of_stage_count": len(oos),
        "out_of_stage": [r["id"] for r in oos],
        "overridden": override is not None,
        "override": override,
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--campaign", required=True, type=pathlib.Path)
    ap.add_argument("--build", required=True, type=pathlib.Path)
    ap.add_argument("--ledger", type=pathlib.Path, default=DEFAULT_LEDGER)
    ap.add_argument("--report", type=pathlib.Path)
    ap.add_argument("--json", dest="json_out", type=pathlib.Path)
    ap.add_argument(
        "--strict",
        action="store_true",
        help="also fail on DECLARED-UNCOVERABLE rows (the absolute floor)",
    )
    ap.add_argument(
        "--admit",
        type=pathlib.Path,
        help=(
            "write the admission token here on a pass (default: "
            "<build>/staging-admission.json). The owner-facing paths refuse to "
            "serve a build whose token is absent, stale or for another tree."
        ),
    )
    ap.add_argument(
        "--stage-anyway",
        metavar="REASON",
        help=(
            "DELIBERATE OVERRIDE: admit a red build, recording REASON in the "
            "token. Requires --acknowledge-red with the exact current red count."
        ),
    )
    ap.add_argument(
        "--acknowledge-red",
        type=int,
        metavar="N",
        help=(
            "the number of red findings you are overriding. Must equal the real "
            "count exactly — it changes as the ledger does, so it cannot become "
            "a flag you type from memory."
        ),
    )
    args = ap.parse_args()

    if not args.campaign.is_dir():
        print(f"staging-gate: no campaign dir {args.campaign}", file=sys.stderr)
        return 2
    if not args.build.is_dir():
        print(f"staging-gate: no build tree {args.build}", file=sys.stderr)
        return 2
    try:
        doc = load_ledger(args.ledger)
    except (OSError, ValueError, json.JSONDecodeError) as e:
        print(f"staging-gate: {e}", file=sys.stderr)
        return 2

    eng = Engine()
    subj = Subject(args.campaign, args.build)
    for err in subj.parse_errors:
        print(f"staging-gate: unreadable stage file — {err}", file=sys.stderr)

    rows = [r for r in doc["findings"] if _applies(r, subj)]
    results = [adjudicate(r, eng, subj) for r in rows]

    report = render_report(doc, subj, results, args.strict)
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(report, encoding="utf-8")
    else:
        print(report)
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(
            json.dumps(
                {"campaign": subj.name, "findings": results}, indent=2, sort_keys=True
            )
            + "\n",
            encoding="utf-8",
        )

    fail = [r for r in results if r["verdict"] in RED_VERDICTS]
    if args.strict:
        fail += [r for r in results if r["verdict"] in EXEMPT_VERDICTS]

    admit_path = args.admit or (args.build / "staging-admission.json")
    fingerprint = build_fingerprint(args.build)
    ledger_digest = hashlib.sha256(args.ledger.read_bytes()).hexdigest()

    # A stale token must never survive a refusal: if this build was admitted
    # once and has since gone red, the old token is the bypass.
    if admit_path.is_file():
        admit_path.unlink()

    if fail:
        print(
            f"staging-gate: REFUSED — {len(fail)} of {len(results)} findings have no "
            f"live, binding check on `{subj.name}`",
            file=sys.stderr,
        )
        for r in fail:
            print(f"  {r['id']:<14} {r['verdict']:<16} {r['detail']}", file=sys.stderr)

        if args.stage_anyway is None:
            print(
                "staging-gate: this build is NOT stageable. Fix the red list, or "
                "override deliberately:\n"
                f'  --stage-anyway "<why this session needs a red build>" '
                f"--acknowledge-red {len(fail)}",
                file=sys.stderr,
            )
            return 1

        # ---- the deliberate override -----------------------------------
        # Explicit, loud, and re-typed against a number that moves. The failure
        # mode being designed against is not "someone overrides once", it is
        # "the override becomes the way the tool is run".
        reason = args.stage_anyway.strip()
        if len(reason) < MIN_JUSTIFICATION:
            print(
                f"staging-gate: --stage-anyway needs a real reason "
                f"(≥{MIN_JUSTIFICATION} chars); got {len(reason)}",
                file=sys.stderr,
            )
            return 2
        if args.acknowledge_red is None:
            print(
                "staging-gate: --stage-anyway requires --acknowledge-red "
                f"{len(fail)} — the count is the acknowledgement",
                file=sys.stderr,
            )
            return 2
        if args.acknowledge_red != len(fail):
            print(
                f"staging-gate: --acknowledge-red {args.acknowledge_red} does not "
                f"match the {len(fail)} red finding(s) above. The number moved "
                "since you last looked; read the list and try again.",
                file=sys.stderr,
            )
            return 2

        by_verdict: dict[str, list[str]] = {}
        for r in fail:
            by_verdict.setdefault(r["verdict"], []).append(r["id"])
        print("", file=sys.stderr)
        print("=" * 72, file=sys.stderr)
        print(
            f"OVERRIDE — staging `{subj.name}` with {len(fail)} UNCOVERED "
            "finding class(es).",
            file=sys.stderr,
        )
        print(f"Reason given: {reason}", file=sys.stderr)
        print("", file=sys.stderr)
        print("These are the defect classes this session is NOT protected from:", file=sys.stderr)
        for verdict, ids in sorted(by_verdict.items()):
            print(f"  {verdict:<16} {', '.join(ids)}", file=sys.stderr)
        print("", file=sys.stderr)
        print(
            "Every one of them is a class the owner has already hit once. If she "
            "hits one again in this session it is not a finding — it is this "
            "override.",
            file=sys.stderr,
        )
        print("=" * 72, file=sys.stderr)

        if fingerprint is None:
            print(
                "staging-gate: build tree has no manifest.json — it has no "
                "identity to admit, override or not",
                file=sys.stderr,
            )
            return 2
        write_admission(
            admit_path,
            subj,
            results,
            fingerprint,
            ledger_digest,
            {"reason": reason, "acknowledged_red": len(fail)},
        )
        print(f"staging-gate: admitted UNDER OVERRIDE -> {admit_path}", file=sys.stderr)
        return 0

    if args.stage_anyway is not None:
        print(
            "staging-gate: --stage-anyway given but nothing is red; refusing to "
            "record an override that overrode nothing",
            file=sys.stderr,
        )
        return 2
    if fingerprint is None:
        print(
            f"staging-gate: {subj.name} is clean, but the build tree has no "
            "manifest.json — nothing to bind an admission token to",
            file=sys.stderr,
        )
        return 2
    write_admission(admit_path, subj, results, fingerprint, ledger_digest, None)
    n_oos = sum(1 for r in results if r["verdict"] == "OUT-OF-STAGE")
    oos_note = (
        f" ({n_oos} class(es) OUT-OF-STAGE on this pre-detail blockout — named "
        "in the token and announced at boot)"
        if n_oos
        else ""
    )
    print(
        f"staging-gate: {subj.name} is stageable — all {len(results)} findings carry a "
        f"live, binding check or a justified exemption{oos_note}; admitted -> {admit_path}",
        file=sys.stderr,
    )
    return 0


def _applies(row: dict, subj: Subject) -> bool:
    """A finding is asked of every campaign by default.

    `scope: "campaign"` narrows a row to the campaign it was found on — used
    only where the defect genuinely cannot exist elsewhere (a content fact
    about one map). The default is deliberately the wide one: the whole lesson
    of the island is that a defect found on one campaign is a CLASS, and a
    ledger that quietly scoped every row to its birthplace would prove nothing
    about the next campaign — which is the only campaign that matters.
    """
    if row.get("scope") == "campaign":
        return row.get("campaign") == subj.name
    return True


if __name__ == "__main__":
    sys.exit(main())
