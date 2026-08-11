#!/usr/bin/env python3
"""The staging gate: refuse to stage a build for the owner while any past
finding's general form is not a live, binding check on THAT build.

## Why this exists, and why a green ladder does not discharge it

The owner's standing directive (2026-08-09): *her playtest is CONTENT QC only;
every bug a compiler or a test could catch must already be fixed before she
touches a build.* `nobodys-cave-island` cost her twenty-two rounds of finding
MECHANICAL defects. The reason the existing ladder cannot promise this is that
most of those findings were things **no check existed for at the time**. So
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
  check nobody maintains is a promise, not a proof.
- `UNBOUND` — the check exists and matched **zero objects** on this campaign.
  The bot's combat floor gate examined zero enemies for nineteen island rounds
  because `floor_gate.covered`, `.not_covered` and `actors[]` were all empty at
  once and nothing counted them.
- `UNFENCED` — the check exists and would bind, but the campaign's declared
  `dsl_version` never reached the surface it keys off, so the proof is inert.
  The island's four branch proofs were physically impossible before round 19
  declared `branch_points`, and were reported green throughout.

Plus one the ledger's own shape can produce:

- `NO-SOURCE` — the named campaign has no DSL stage files. A campaign that
  cannot be measured is **never** a pass; the drowned-bell remake is in exactly
  this state today (`REMAKE.md`, no stage JSON), and a gate that shrugged at it
  would green-light the very build this directive was written for.

## What is deliberately NOT a red

`docs/reference/playtest-methodology.md` rule 2 permits one escape and this
honours it exactly, no wider: a finding may close with *"a declared, justified
reason none is possible"*. Such a row carries `disposition` of `no-machine-form`
(prose quality, a judgement no compiler can make) or `owner-ruled` (she ruled it
not a defect), plus a `justification` this tool requires to be present and
substantive. Those rows do not fail the gate — they are printed in their own
section with their justification, and their COUNT is in the headline, because
rule 4 makes each one a standing risk item at every staging review. `--strict`
fails on them too, for a reviewer who wants the absolute floor.

That escape is the only one. There is no "skip", no "known-red", no threshold.
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
    "UNFENCED",
    "UNBOUND",
    "INAPPLICABLE",
)
EXEMPT_VERDICTS = ("DECLARED-UNCOVERABLE",)
PASS_VERDICTS = ("BOUND",)

VALID_DISPOSITIONS = ("no-machine-form", "owner-ruled")
MIN_JUSTIFICATION = 24  # chars — a justification has to say something


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
    )

    def __init__(self, campaign: pathlib.Path, build: pathlib.Path) -> None:
        self.campaign = campaign
        self.build = build
        self.name = campaign.name
        self.stages: dict[str, object] = {}
        self.stage_versions: dict[str, str] = {}
        self.parse_errors: list[str] = []
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


def parse_version(text: str) -> tuple[int, ...]:
    parts = re.findall(r"\d+", text or "0")
    return tuple(int(p) for p in parts[:3]) or (0,)


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
            return None, f"none of {files} present in {subj.name}"
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
        for p in sorted(root.rglob("*")):
            if not p.is_file():
                continue
            rel = p.relative_to(root).as_posix()
            if not fnmatch.fnmatch(rel, pattern):
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

    # Version fence BEFORE the binding count: an unfenced campaign's zero is
    # explained by the fence, and reporting it as UNBOUND would send a reader
    # hunting for objects that could not have been declared.
    req = row.get("requires")
    if req:
        f = req["file"]
        need = req["min_dsl_version"]
        have = subj.stage_versions.get(f)
        if have is None:
            out["verdict"] = "UNFENCED"
            out["detail"] = (
                f"`{f}` declares no dsl_version; the check keys off a surface "
                f"introduced at {need}"
            )
            return out
        if parse_version(have) < parse_version(need):
            out["verdict"] = "UNFENCED"
            out["detail"] = (
                f"`{f}` declares dsl_version {have}; the surface this check keys "
                f"off arrived at {need} — the proof is inert on this campaign"
            )
            return out

    count, detail = probe(row["binding"], subj)
    out["detail"] = detail
    if count is None:
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
        out["verdict"] = "INAPPLICABLE"
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
        if r["verdict"] == "INAPPLICABLE":
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
    if fail:
        print(
            f"staging-gate: REFUSED — {len(fail)} of {len(results)} findings have no "
            f"live, binding check on `{subj.name}`",
            file=sys.stderr,
        )
        for r in fail:
            print(f"  {r['id']:<14} {r['verdict']:<16} {r['detail']}", file=sys.stderr)
        return 1
    print(
        f"staging-gate: {subj.name} is stageable — all {len(results)} findings carry a "
        f"live, binding check or a justified exemption",
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
