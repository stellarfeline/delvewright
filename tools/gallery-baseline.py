#!/usr/bin/env python3
"""The gallery's emission baseline and its expected-warnings ledger (spec-0039 §5).

## What is committed, and what is not

Never the output tree — only, per build in the domain, a copy of that build's
`manifest.json` (the compiler's SHA-256 index over its inputs and every output
file) under `gallery/baseline/`, plus one `warnings.json` ledger and one
`delta.json` review artifact.

Every manifest copy carries a **header**: the delvec version, the `dsl_version`,
the gallery source-tree hash and the generator-input hash. The comparison
asserts the header FIRST and refuses with its own message when it disagrees,
instead of diffing noise — a baseline taken by a different delvec is not a
regression, it is a measurement of two different things, and reporting it as a
file-by-file diff buries the one fact the reader needs.

## Three verdicts from one mismatch, because they mean different things

- **no emitted path moved at all**, and the manifest differs anyway — a
  **declared input moved**. A manifest is not its outputs: the pinned versions,
  `campaign_id`, `resource_pack_sha1` and the whole `inputs` index are SIBLINGS
  of `outputs`, so a manifest can differ while every emitted byte is identical.
  Reported first because it is the one the other two cannot describe: the delta
  walks `outputs`, so it lists zero differing paths and the finding would come
  out as a determinism violation over nothing at all.
- the change touches an input that can reach emission (`EMISSION_INPUTS`) — an
  **emission change**: regenerate the baseline in this change, or explain the
  drift;
- it touches none of them — a **determinism finding** (ADR-0006), named as such.
  The baseline is thereby a standing cross-machine determinism probe, for free.

## The two arms are ONE rule, and the implication runs ONE WAY

`--write` refuses a noise commit by asking exactly the question the verify arm
asks — `baseline_matches`: *does `gallery/baseline/` already hold, byte for byte,
every document a write would produce?* — and nothing else. The pair property
holds, and it is the one that matters: **no tree can be refused by both arms.**

The BICONDITIONAL this file used to claim — *`--write` refuses if and only if
verify passes* — is false, and it is false on a pristine checkout of `main`.
Both arms ask one question, but they hand it a different
`produced["delta.json"]`, because a delta is a claim about a transition and the
arms name the far end of that transition differently: **verify is HANDED the
base by the artifact; `--write` re-derives `merge-base(--base, HEAD)`.** The
moment `--base` has advanced past the commit the artifact names — which is the
state of every branch a merge train has touched, and of `main` itself after any
merge — `--write` produces a document differing from the committed one, and
lands on a tree verify has just passed.

Only the forward direction holds, and it holds for a reason rather than by
luck: if `--write` refuses then `on_disk == produced`, so the committed
`delta.json` names `--write`'s own base, so verify resolves the SAME base and
passes. Hence `--write` refuses ⇒ verify passes, and no tree is refused by
both. The extra state is a WRITABLE one, never a deadlock, so this is not the
unsatisfiable-pair defect CLAUDE.md names.

What it is instead is a **report** defect, and that is the expensive half —
see `write_effect` and `write_report`.

That is a repair to the pair rather than to either half. The previous shape
ENUMERATED what to compare — emission, then the header, then, one fix later, the
warning ledger — and was unsatisfiable for anything the enumeration had not met.
It met one: a change moving only a recorded manifest value — one that is in no
header, no warning row and no output — left verify refusing the tree and
`--write` refusing the regeneration it had just prescribed. Enumerating is the
defect; comparing the writer's own output cannot go stale, and a fifth produced
document joins both arms by being produced.

The guard also runs BEFORE the write, so a refusal leaves the tree untouched and
the exit status and the effect agree.

## The review delta, and why it is measured from a COMMIT

`delta.json` answers *what emission does this change move, relative to the point
it branched from* — the question its reader has, and the one spec-0039 §5 says
CI must be able to recompute. Recomputing it needs the OTHER side of the
comparison, and no document under `gallery/baseline/` holds it: a delta is a
statement about a transition, and a directory records a state. So the artifact
**names the commit it was measured from** (`base_commit`) and the other side is
read out of git at that commit. That is the whole reconciliation between two
facts that had never been read together: `delta.json` is not a record of the
tree — it is a record of the tree *relative to a named point* — which is exactly
why it is not in `RECORDED` and exactly why it can still be checked.

Two consequences fall out, and both were live defects:

- The base used to be *whatever happened to be on disk*, so the artifact was a
  function of the write rather than of the tree. Running `--write` twice before
  committing silently re-based it onto its own first output, and a write that
  moved only the header rewrote it to empty — destroying the record of the last
  real emission change while every gate stayed green. Measured from a commit,
  two writes of one tree agree **while `--base` sits still**, which is all the
  idempotence there is: `--base` is a moving ref, so a write is still a function
  of WHEN it ran, and a write after the base advanced can still empty the delta.
  That is honest for the new base and is a record discarded all the same, so
  `write_report` says so in those words rather than leaving it to `git diff`.
- It becomes checkable at all. `base_commit` is a commit on the base branch, so
  it survives the squash that discards the branch, and the claim stays true on
  `main` for as long as the manifests do not move.

What this establishes and what it does not: the delta is proved truthful about
the transition it DECLARES. The declaration itself is `--write`'s policy
(`--base`, default `origin/main`), so a base deliberately mis-declared by hand is
not what this catches — staleness and hand-editing are, and those are what
spec-0039 asks for. The base is asserted to be an ancestor of `HEAD`, so it can
never name a commit this history does not contain.

## The warnings ledger

Judgement-tier warnings are legitimate; *drifting* warnings are not. The emitted
warning set must equal the committed ledger exactly, so "still green" can never
quietly absorb "warns differently now".

## Binding count

Every run states builds compared, output paths compared, warning rows checked,
recorded manifest values compared, and emitted paths weighed against the review
base — the last two being denominators for comparisons that had no stated
binding at all while they were invisible. Comparing zero of any of them is a
red: a baseline that matched nothing is vacuous, not a pass. The delta's own
length is NOT that denominator: an honest delta is empty whenever a change moves
a recorded input without moving an emitted byte, so counting it would make the
commonest legitimate update look vacuous.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
import gallery_domain  # noqa: E402
from gallery_domain import build_id, overlays  # noqa: E402
from delvec_bin import resolve as resolve_delvec  # noqa: E402
from gitbase import BaseUnresolved, resolve_base  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
GALLERY = REPO / "gallery"
BASELINE = GALLERY / "baseline"

# The builds that make up the domain: the primary in every declared language,
# plus each overlay in `en`. Derived, never listed — a listed set goes stale the
# first time a language or an overlay is added, and goes stale silently.
# A warning line: code, stage, then the POINTER, which runs to the first
# `: ` that ends it. The pointer is not a single token — a build-tier
# diagnostic points at a phase of the build ("packtest watch coverage"),
# not at a JSON pointer — and a `(\S+)` here silently dropped every one of
# them, so the ledger that claims the emitted warning set must equal it
# exactly was blind to a whole class of warning. `ANY_WARNING_RE` is what
# stops that being silent again: a line that announces itself as a warning
# and does not parse is a red, never a skip.
WARNING_RE = re.compile(r"^(DW\d{4}) \[warning\] (\S+) (.+?): ")
ANY_WARNING_RE = re.compile(r"^(DW\d{4}) \[warning\] ")

# What this baseline RECORDS about the tree, in the order a reader meets it.
# This set decides how a mismatch is CLASSIFIED — header, then emission, then the
# warning ledger — and nothing else.
#
# `delta.json` is deliberately absent, and the reason is unchanged: it is not a
# record of the tree. It is a record of the tree RELATIVE TO A NAMED COMMIT, so
# it cannot be compared against a measurement of the tree alone, and treating it
# as a record would make it a second vote on facts `manifests.json` already
# holds.
RECORDED = ("header.json", "manifests.json", "warnings.json")

# The review artifact: derived from `RECORDED` plus the commit it names.
DERIVED = ("delta.json",)

# Every document a write produces, which is what BOTH arms compare — see the
# module docstring. Derived from the two sets above rather than written out, so
# a fifth produced document joins the verify comparison AND the noise-commit
# guard by being produced, with nothing to remember.
#
# This is what replaced `RECORDED` at the guard. Comparing `RECORDED` there was
# itself an enumeration, one level up from the one it had just retired: a tree
# whose recorded documents were all correct and whose review delta was stale or
# hand-edited was refused by nothing, and — once anything DID refuse it — could
# not be repaired, because `--write` would have called the repair a noise commit.
# The pair has no satisfiable state unless both arms weigh every produced byte.
PRODUCED = RECORDED + DERIVED

# The paths whose content can reach a byte the compiler emits OR records, so a
# manifest mismatch alongside a change to one of them is an ordinary consequence
# rather than a violation of ADR-0006. Membership is decided by that question and
# by nothing else.
#
# `versions.toml` is deliberately NOT here, and it is the member a reader most
# expects: the compiler reads nothing from it. It is handed a campaign directory,
# a prefab directory and its flags, and every byte it emits or records is a
# function of those (`docs/reference/compiler.md`, Determinism). A pin is a fact
# about a checkout; the manifest's `dsl_version` comes from the campaign and its
# `mc_version` from a compiler constant. So a gallery mismatch alongside a re-pin
# is exactly as unexplained as one alongside no change at all, and naming it an
# emission change would be the reassuring direction.
#
# Widening this set makes nothing easier to ship: BOTH verdicts refuse, so it
# decides only what the reader is told the finding IS.
EMISSION_INPUTS = ("gallery/", "crates/")


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def write_canonical(path: Path, obj) -> None:
    """Write one baseline file in `delvec fmt` canonical form.

    `ensure_ascii=False` is the load-bearing argument and it is why this is a
    function rather than four call sites. Python's default escapes every
    non-ASCII character, so an em-dash inside a warning pointer landed in
    `warnings.json` as `\\u2014` — and that single habit was the ONLY reason any
    of `gallery/baseline/` was outside canonical form, which in turn was the only
    reason anyone could argue that a generated artifact needs an exemption from
    `tools/check-json-canonical.py`. A generated file that is already canonical
    needs no exemption, and cannot be used as cover by a file that merely never
    was (CLAUDE.md: an opt-out must be secured by a property the defect cannot
    supply — so the better move is to leave no opt-out to secure).

    The header does not hash `baseline/` (see `header`), so making these bytes
    canonical moves no header field and is not a determinism finding to anyone
    reading the diff afterwards. The three sibling files were already canonical;
    only `warnings.json` carried non-ASCII at all.
    """
    path.write_text(
        json.dumps(obj, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def tree_hash(root: Path, skip: set[str]) -> str:
    """A deterministic hash of a source tree: relative path AND content, in path order.

    Both halves, and the pairing is the point. Hashing only the file bytes makes
    a rename invisible; hashing the paths alongside a `shasum` listing — the
    shape this project has been bitten by — hashes the PATHS as content and
    reports two identical trees under different names as different.
    """
    h = hashlib.sha256()
    for p in sorted(root.rglob("*")):
        if not p.is_file():
            continue
        rel = p.relative_to(root).as_posix()
        if any(rel == s or rel.startswith(s + "/") for s in skip):
            continue
        h.update(rel.encode())
        h.update(b"\0")
        h.update(p.read_bytes())
        h.update(b"\0")
    return h.hexdigest()


def delvec_versions(delvec: Path) -> dict:
    r = subprocess.run([str(delvec), "--version"], capture_output=True, text=True)
    if r.returncode != 0:
        die(f"`delvec --version` exited {r.returncode}")
    # `delvec x.y.z, dsl a.b.c, mc x.y.z`
    parts = dict(
        (k.strip(), v.strip())
        for k, v in (p.split(" ", 1) for p in r.stdout.strip().split(", "))
    )
    return {"delvec": parts.get("delvec", ""), "dsl": parts.get("dsl", "")}


def declared_languages() -> list[str]:
    world = json.loads((GALLERY / "world.json").read_text())
    return list(world["content"].get("languages") or [])


def materialise(overlay: str | None, dest: Path) -> None:
    """One point of the domain as a campaign directory — `gallery_domain` decides what that means."""
    gallery_domain.materialise(dest, GALLERY / "overlays" / overlay if overlay else None)


def build_one(delvec: Path, prefabs: Path, overlay: str | None, lang: str, work: Path):
    """One build of the domain: `(manifest, warning rows)`."""
    src = work / f"src-{overlay or 'primary'}-{lang}"
    out = work / f"out-{overlay or 'primary'}-{lang}"
    materialise(overlay, src)
    r = subprocess.run(
        [str(delvec), "--lang", lang, "build", str(src), "-o", str(out), "--prefabs", str(prefabs)],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        die(
            f"build `{overlay or 'primary'}` in `{lang}` exited {r.returncode}. "
            "A baseline is only meaningful over a green build.\n" + r.stdout + r.stderr
        )
    manifest = json.loads((out / "manifest.json").read_text())
    rows = []
    for line in (r.stdout + r.stderr).splitlines():
        stripped = line.strip()
        m = WARNING_RE.match(stripped)
        if m:
            rows.append({"code": m.group(1), "stage": m.group(2), "pointer": m.group(3)})
        elif ANY_WARNING_RE.match(stripped):
            # Never `continue`. A warning the ledger cannot read is a warning the
            # ledger cannot compare, and dropping it makes the set-equality claim
            # false in exactly the direction that reads as a clean pass.
            die(
                "a warning line was emitted that this ledger cannot parse, so it "
                "would have been dropped from the comparison in silence — the set "
                "equality this file claims would be false and green:\n"
                f"  {stripped[:200]}"
            )
    return manifest, rows


def coverage_counts(delvec: Path) -> dict:
    """Units total / bound / refusal-proven / unaccounted, for the header.

    spec-0039 §6 asks for the deterministic counts in the baseline header so
    growth is a diffable number rather than a complaint. It also makes the
    unaccounted count a **committed fact**, which is what lets
    `check-required-contexts.py` EVALUATE the gallery job's advisory entry
    rather than recite its expiry condition — a hatch whose end state only a
    comment describes is a hatch that never ends.

    Derived from the same enumeration the coverage gate uses, never a second one.
    """
    sys.path.insert(0, str(REPO / "tools"))
    from gallery_units import Binder, Enumerator, stage_files

    r = subprocess.run(
        [str(delvec), "schema", "--stage", "all"], capture_output=True, text=True
    )
    if r.returncode != 0:
        die(f"`delvec schema --stage all` exited {r.returncode}")
    export = json.loads(r.stdout)
    e = Enumerator(export)
    units = e.run()
    if not units:
        die("the schema export enumerated ZERO units; the header would record a lie")

    def bind_dir(root: Path, label: str, into: set) -> None:
        b = Binder(e)
        for stage, fn in stage_files(export).items():
            f = root / fn
            if f.is_file():
                b.walk(export[stage], json.loads(f.read_text()), label)
        into |= set(b.bound) & set(units)

    bound: set = set()
    bind_dir(GALLERY, "primary", bound)
    work = Path(tempfile.mkdtemp(prefix="gallery-header-"))
    try:
        for name in overlays():
            dest = work / name
            materialise(name, dest)
            bind_dir(dest, f"overlay:{name}", bound)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    proven: set = set()
    pdir = GALLERY / "probes"
    if pdir.is_dir():
        for d in sorted(x for x in pdir.iterdir() if x.is_dir()):
            m = json.loads((d / "probe.json").read_text())
            proven |= {u for u in (m.get("units") or []) if u in units}

    return {
        "units_total": len(units),
        "units_bound": len(bound),
        "units_refusal_proven": len(proven - bound),
        "units_unaccounted": len(set(units) - bound - proven),
    }


def header(delvec: Path, prefabs: Path) -> dict:
    v = delvec_versions(delvec)
    return {
        "coverage": coverage_counts(delvec),
        "delvec_version": v["delvec"],
        "dsl_version": v["dsl"],
        # `baseline/` is this file's own output, and `README.md` is prose that
        # cannot reach a byte of emission — hashing either would make an ordinary
        # documentation edit demand a baseline regeneration, and a gate that
        # fires on changes it cannot possibly be about is a gate people learn to
        # discharge without reading.
        "gallery_source_sha256": tree_hash(GALLERY, {"baseline", "README.md"}),
        "generator_input_sha256": tree_hash(prefabs, set()),
    }


def classify() -> str:
    """Emission change, or determinism finding — decided by what the diff touches.

    Took the differing paths as an argument and never read them; the argument
    said the verdict depended on what moved, and it depends only on what the
    change touched.
    """
    r = subprocess.run(
        ["git", "-C", str(REPO), "diff", "--name-only", "origin/main...HEAD"],
        capture_output=True,
        text=True,
    )
    changed = r.stdout.splitlines() if r.returncode == 0 else []
    touched = [c for c in changed if any(under(c, p) for p in EMISSION_INPUTS)]
    if touched:
        return "emission-change"
    return "determinism-finding"


def under(path: str, prefix: str) -> bool:
    """`path` IS `prefix`, or lies inside it — never merely starts with its text.

    A bare `startswith("crates")` also claims `crates.bak`, and a prefix rule
    that quietly widens is how a determinism finding would get renamed by a file
    nobody meant to name.
    """
    return path == prefix.rstrip("/") or path.startswith(prefix.rstrip("/") + "/")


# --------------------------------------------------------------- the review base


def git(*args: str) -> tuple[int, str]:
    r = subprocess.run(
        ["git", "-C", str(REPO), *args], capture_output=True, text=True
    )
    return r.returncode, r.stdout


def review_base(base_ref: str) -> str:
    """The commit a fresh `delta.json` is measured from: `merge-base(base_ref, HEAD)`.

    The point this tree branched from, so the artifact answers *what emission
    does this change move* rather than *what did the last invocation of this tool
    happen to move*. On the base branch itself it degenerates to `HEAD`, which is
    honest: nothing is being proposed, so nothing is being moved.

    Resolved only by `--write`. Verify never re-derives it — it reads the commit
    the artifact NAMES, which is what makes the recomputation independent of
    whether `origin/main` has moved since.
    """
    try:
        sha = resolve_base(REPO, base_ref, "gallery-baseline")
    except BaseUnresolved as e:
        die(e.message)
    code, out = git("merge-base", sha, "HEAD")
    if code != 0 or not out.strip():
        die(
            f"no merge base between {base_ref!r} and HEAD, so there is no point "
            "to measure the review delta from. `delta.json` is a statement about "
            "a transition and cannot be written without both of its ends."
        )
    return out.strip()


def manifests_at(sha: str) -> dict:
    """`gallery/baseline/manifests.json` as of `sha` — `{}` only when it was not there.

    The two failure modes are separated deliberately. A commit this checkout does
    not have and a commit that predates the baseline both make `git show` exit
    non-zero, and collapsing them would answer *the baseline was empty then* to a
    question that was really *I cannot see that commit* — a plausible wrong number
    of exactly the shape this project keeps paying for. A shallow clone reaches
    the first case, and it dies.
    """
    if git("rev-parse", "--verify", "--quiet", f"{sha}^{{commit}}")[0] != 0:
        die(
            f"the review delta names commit `{sha}`, which is not in this "
            "checkout, so it cannot be recomputed. Fetch the history it came "
            "from — a shallow checkout is the usual cause — rather than "
            "regenerating, which would only record a base the next reader "
            "cannot see either."
        )
    code, out = git("show", f"{sha}:gallery/baseline/manifests.json")
    return json.loads(out) if code == 0 else {}


def warnings_at(sha: str) -> dict:
    code, out = git("show", f"{sha}:gallery/baseline/warnings.json")
    return json.loads(out) if code == 0 else {}


def base_of(delta: dict | None) -> str | None:
    """The commit a committed `delta.json` claims to be measured from, if it says."""
    if not isinstance(delta, dict):
        return None
    sha = delta.get("base_commit")
    return sha if isinstance(sha, str) and re.fullmatch(r"[0-9a-f]{40}", sha) else None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--delvec", help="the `delvec` this tool runs. Default: `target/release/delvec` then `target/debug/delvec` in this tree — resolved, NAMED on stderr, and refused when it is older than the compiler sources it was built from (`tools/lib/delvec_bin.py`).")
    ap.add_argument("--prefabs", required=True)
    ap.add_argument("--write", action="store_true", help="regenerate the baseline")
    ap.add_argument(
        "--base",
        default="origin/main",
        help=(
            "the branch a fresh review delta is measured from; its merge base "
            "with HEAD is recorded in `delta.json`. Read only by `--write` — "
            "verify uses the commit the artifact already names."
        ),
    )
    args = ap.parse_args()

    delvec = resolve_delvec(args.delvec, repo=REPO, caller="gallery-baseline")
    prefabs = Path(args.prefabs)

    builds = [(None, "en")] + [(None, l) for l in declared_languages()]
    builds += [(o, "en") for o in overlays()]

    work = Path(tempfile.mkdtemp(prefix="gallery-baseline-"))
    manifests, warnings = {}, {}
    try:
        for overlay, lang in builds:
            m, rows = build_one(delvec, prefabs, overlay, lang, work)
            manifests[build_id(overlay, lang)] = m
            warnings[build_id(overlay, lang)] = rows
    finally:
        shutil.rmtree(work, ignore_errors=True)

    hdr = header(delvec, prefabs)
    on_disk = read_baseline()

    # The review delta's other end. `--write` chooses it (the merge base with
    # `--base`); verify is HANDED it by the artifact, so a recomputation asks
    # about the transition the file claims rather than about wherever the base
    # branch has since moved to — the difference between recomputing a claim and
    # re-deciding it.
    if args.write:
        base_sha = review_base(args.base)
    else:
        if on_disk is None:
            missing = ", ".join(
                f"`gallery/baseline/{n}`" for n in PRODUCED if not (BASELINE / n).is_file()
            )
            die(f"the baseline is incomplete — {missing} missing. Run this with `--write`.")
        base_sha = base_of(on_disk["delta.json"])
        if base_sha is None:
            die(
                "`gallery/baseline/delta.json` does not name the commit it was "
                "measured from, so nothing can recompute it and its claim about "
                "what this change moves is unfalsifiable (spec-0039 §5 asks for "
                "exactly that recomputation). Regenerate it with `--write`."
            )
        if base_sha is not None and git("merge-base", "--is-ancestor", base_sha, "HEAD")[0] != 0:
            die(
                f"the review delta names commit `{base_sha}` as its base, and "
                "that commit is not an ancestor of HEAD. A delta measured from "
                "somewhere outside this history describes a transition this tree "
                "never made. Regenerate it with `--write`."
            )

    base_manifests = manifests_at(base_sha) if base_sha else {}
    delta = review_delta(base_sha, base_manifests, manifests) if base_sha else None

    measured = {"header": hdr, "manifests": manifests, "warnings": warnings}
    produced = {
        "header.json": hdr,
        "manifests.json": manifests,
        "warnings.json": warnings,
        "delta.json": delta,
    }
    assert set(produced) == set(PRODUCED), "the writer's output and `PRODUCED` disagree"

    n_paths = sum(len(m.get("outputs") or m.get("files") or {}) for m in manifests.values())
    n_warn = sum(len(v) for v in warnings.values())
    n_fields = field_count(manifests)
    n_delta = delta_binding(base_manifests, manifests) if base_sha else 0
    print(
        f"gallery baseline: {len(manifests)} build(s), {n_paths} emitted path(s), "
        f"{n_warn} warning row(s), {n_fields} recorded manifest value(s); review "
        f"delta measured over {n_delta} emitted path(s) against `{(base_sha or '-')[:12]}`."
    )
    if not manifests or n_paths == 0 or n_fields == 0 or n_delta == 0:
        die(
            "the baseline compared ZERO builds, ZERO emitted paths, ZERO "
            "recorded manifest values, or weighed ZERO emitted paths for the "
            "review delta. A baseline that matched nothing is vacuous, not a "
            "pass."
        )

    committed = recorded_triple(on_disk) if on_disk else None

    if args.write:
        # The noise-commit guard, and it asks ONE question: does
        # `gallery/baseline/` already hold every document this write would
        # produce? That is exactly the question the verify arm asks
        # (`baseline_matches`) — of the document THIS arm produces, which is
        # where the two part company: a refusal here means the committed
        # `delta.json` names this arm's own base, so verify resolves the same
        # base and passes. Hence `--write` refuses ⇒ verify passes, and no tree
        # is refused by both. The converse does NOT hold, and the module
        # docstring says why.
        #
        # It replaces an ENUMERATION — empty output delta, unchanged header,
        # unchanged warning ledger — which was correct about the three things its
        # authors had met and silent about the fourth. A recorded manifest value
        # is a sibling of `outputs`, in no header and no warning row, so a change
        # moving one moved something this baseline records while all three
        # qualifiers held: verify refused the tree and prescribed `--write`, and
        # `--write` refused the regeneration as noise. CLAUDE.md: when one gate's
        # prescription is
        # another gate's refusal, the defect belongs to the PAIR — so the repair
        # is one shared question, not a fourth qualifier.
        #
        # The question weighs the WRITER'S OUTPUT rather than the recorded
        # documents, which is the same repair applied one level up: comparing
        # `RECORDED` here left the review delta guarded by nothing, and a tree
        # whose delta alone was wrong could not be repaired at all — verify would
        # have prescribed `--write` and `--write` would have called the repair
        # noise. Measured from a commit rather than from disk, the delta is
        # idempotent under a repeated write, so including it cannot make an
        # ordinary rerun look like it moved something.
        #
        # It runs BEFORE the write. The old order wrote all four files and then
        # refused, so the exit status and the effect on disk disagreed and a
        # caller under `set -e` could not tell; worse, a genuinely-noise rewrite
        # blanked `delta.json`, destroying the review artifact of the last real
        # change, and the operator's undo is the `git checkout` this project has
        # already been bitten by. Guarding first makes a refusal a no-op.
        if baseline_matches(on_disk, produced):
            die(
                "nothing under `gallery/baseline/` would move: every document "
                "this write produces — header, manifests, warning ledger and the "
                "review delta — already equals what this run measured, so this "
                "rewrite is a noise commit. A baseline update is never split "
                "from the change that caused it (§5). Nothing was written."
            )
        # `on_disk` is read BEFORE the write, so it is the only chance this run
        # has to know what its own write moved. Take the answer now.
        effect = write_effect(on_disk, produced, base_sha)
        BASELINE.mkdir(parents=True, exist_ok=True)
        for name in PRODUCED:
            write_canonical(BASELINE / name, produced[name])
        old_warnings = warnings_at(base_sha)
        moved = warning_delta(old_warnings, warnings) if old_warnings != warnings else []
        fields = (
            manifest_field_delta(base_manifests, manifests, left=f"`{base_sha[:12]}`")
            if base_manifests
            else []
        )
        for line in write_report(effect, delta, moved, fields):
            print(line)
        return 0

    if not baseline_matches(on_disk, produced):
        # Classify against what the baseline RECORDS first: a drift in the tree
        # is a bigger fact than a stale review note, and `report_mismatch` is the
        # one authority on naming it. Only when every recorded document already
        # agrees is the review delta the sole remaining difference, and it gets
        # its own verdict rather than being folded into a message about emission.
        if not baseline_matches(committed, measured):
            report_mismatch(committed, measured)  # never returns
        report_delta_mismatch(on_disk["delta.json"], delta)  # never returns
    print(
        "baseline: header, manifests and warning ledger all match, and the "
        f"review delta recomputes identically from `{base_sha[:12]}`."
    )
    return 0


def baseline_matches(committed: dict | None, measured: dict) -> bool:
    """The ONE question both arms ask: does what is on disk already equal what a
    write would produce?

    Verify passes exactly when this is true; `--write` refuses exactly when it is
    true. That is the whole of the pair's agreement, and it lives in one function
    so it cannot become two enumerations that drift apart — which is what it was,
    and what made a pure content re-pin unsatisfiable in both directions.

    Deliberately a plain equality over whatever it is handed, and it is handed the
    writer's whole output. It has no list of documents inside it, so it cannot
    have a hole for a document nobody thought to add to a list.

    A missing baseline is not a match: there is nothing to have recorded
    anything, so verify reds (asking for `--write`) and `--write` lands.
    """
    return committed is not None and committed == measured


def read_baseline() -> dict | None:
    """The documents `gallery/baseline/` holds, or `None` if ANY is absent.

    All of `PRODUCED` or none of it. A partial baseline cannot answer whether
    anything moved, and answering anyway would be a guard reporting a fact about
    a smaller world than the one it claims to cover.
    """
    if not all((BASELINE / n).is_file() for n in PRODUCED):
        return None
    return {n: json.loads((BASELINE / n).read_text()) for n in PRODUCED}


def recorded_triple(files: dict) -> dict:
    """The `RECORDED` documents of a baseline file map, keyed as `report_mismatch` reads them."""
    return {n.removesuffix(".json"): files[n] for n in RECORDED}


def report_mismatch(committed: dict, measured: dict) -> None:
    """Name the mismatch and refuse. NEVER RETURNS.

    Reached only when `baseline_matches` is false, and it must die on every path
    through it: a fall-through here is a verify that reds nothing while `--write`
    refuses to regenerate — the pair's unsatisfiable state re-entering through
    the reporting rather than through the rule.
    """
    hdr, manifests = measured["header"], measured["manifests"]
    warnings = measured["warnings"]
    if committed["header"] != hdr:
        diffs = [
            f"  {k}: baseline `{committed['header'].get(k)}` vs this tree `{hdr.get(k)}`"
            for k in sorted(set(committed["header"]) | set(hdr))
            if committed["header"].get(k) != hdr.get(k)
        ]
        die(
            "the committed baseline was taken over DIFFERENT INPUTS, so a "
            "file-by-file diff would report noise rather than a finding:\n"
            + "\n".join(diffs)
            + "\nRegenerate it with `--write` in the same change that moved them."
        )
    if committed["manifests"] != manifests:
        delta = compute_delta(committed["manifests"], manifests)
        differing = delta["added"] + delta["removed"] + delta["changed"]
        fields = manifest_field_delta(committed["manifests"], manifests)
        if not differing:
            # The third verdict. Every emitted path is present and byte-identical
            # and the manifest still differs, so this is neither an emission
            # change nor a determinism finding — a DECLARED INPUT moved, and the
            # manifest records it beside the outputs rather than inside them.
            # Said first, and said in its own words, because the two verdicts
            # below cannot describe it: reported as a determinism finding, this
            # printed the gravest message this file has over a list of zero paths.
            die(
                "A DECLARED INPUT MOVED: every emitted path is present and "
                "byte-identical, and the manifest differs anyway — a manifest "
                "records more than its outputs. This is NOT a determinism "
                "finding and NOT an emission change; the baseline has gone stale "
                "against an input this change declares. Regenerate it with "
                "`--write` in this same change.\n" + "\n".join(fields)
            )
        extra = ("\nAnd these recorded values moved with it:\n" + "\n".join(fields)) if fields else ""
        if classify() == "determinism-finding":
            die(
                "DETERMINISM FINDING (ADR-0006): this change touches none of "
                f"{', '.join('`' + p + '`' for p in EMISSION_INPUTS)}, and the "
                "gallery's emission moved anyway. Same DSL + same seed must give "
                "byte-identical output.\n"
                + "\n".join(f"  {p}" for p in differing)
                + extra
            )
        die(
            "EMISSION CHANGE: the gallery's emitted bytes moved. Regenerate the "
            "baseline in this same change (`--write`) and confirm every path "
            "class below is a consequence this change claims to have.\n"
            + "\n".join(f"  {p}" for p in differing)
            + extra
        )
    if committed["warnings"] != warnings:
        die(
            "the emitted warning set no longer equals the committed ledger. A new "
            "or vanished warning is a red: 'still green' must never quietly "
            "absorb 'warns differently now' (§4.3). Regenerate with `--write` "
            "only once every row below is a consequence this change claims to "
            "have.\n" + "\n".join(warning_delta(committed["warnings"], warnings))
        )
    die(
        "the committed baseline differs from what this run measured, and no "
        "component comparison could say how. That is this file failing to "
        "REPORT, never a pass — and it is the one state in which `--write` is "
        "still the right next act."
    )


def report_delta_mismatch(committed: dict, recomputed: dict) -> None:
    """The review artifact does not tell the truth about this tree. NEVER RETURNS.

    Reached only when every RECORDED document already matches, so nothing about
    the tree is in question and exactly one thing is: `delta.json` claims a set
    of moved paths that recomputing from the commit it names does not produce.
    Two ways to get here and the message names both, because the remedy is the
    same and the finding is not: the file was hand-edited, or `manifests.json`
    moved in a later commit that did not regenerate it — which is the state
    spec-0039 §5 calls stale, and the reason it asks for this check at all.
    """
    base = committed.get("base_commit")
    lines = []
    for key in ("added", "removed", "changed"):
        was, now = set(committed.get(key) or []), set(recomputed.get(key) or [])
        for p in sorted(now - was):
            lines.append(f"  + {key}: `{p}` — moved, and the delta does not say so")
        for p in sorted(was - now):
            lines.append(f"  - {key}: `{p}` — claimed by the delta, and did not move")
    if committed.get("classes") != recomputed.get("classes"):
        lines.append(
            f"  ~ classes: delta says {committed.get('classes')}, "
            f"recomputed {recomputed.get('classes')}"
        )
    if not lines:
        lines.append(
            f"  (every path agrees; the documents differ elsewhere: "
            f"{sorted(set(committed) | set(recomputed))})"
        )
    die(
        "THE REVIEW DELTA IS NOT TRUE OF THIS TREE. Every document the baseline "
        "records matches, so this is not a drift in emission — it is "
        f"`gallery/baseline/delta.json` disagreeing with what recomputing it from "
        f"`{base}`, the commit it names, produces. Either it was hand-edited, or "
        "`manifests.json` moved afterwards without it. A reviewer reads this file "
        "to ask whether every path class listed is a consequence this change "
        "claims to have, and an untrue answer is worse than none. Regenerate it "
        "with `--write`.\n" + "\n".join(lines)
    )


# ------------------------------------------------- what a write actually moved


def write_effect(on_disk: dict | None, produced: dict, base_sha: str) -> dict:
    """What THIS INVOCATION moves on disk — measured before the write, never narrated.

    The write arm had no such measurement, and could not have had one after the
    fact: `on_disk` is read once, before the files are overwritten, so this is the
    only moment at which the run can know its own effect. Everything the arm
    printed instead was `<this tree> vs <the review base>` — a difference between
    two COMMITS, which is not what the invocation changed and can be arbitrarily
    large while the invocation changes one JSON field.

    The two ends of the report are therefore separate facts, and only one of them
    is about this run:

    - `changed` / `unchanged`: the documents whose bytes this write moves;
    - `recorded_moved`: those of them that RECORD the tree — an empty list means
      emission, the warning set and every recorded manifest value are already
      exactly what this run measured, so nothing about the tree drifted;
    - `rebased`: `delta.json` names a different base than it did, because
      `--base` advanced since the artifact was written. This is a re-base of a
      claim about a transition, not a movement of the tree, and it is the whole
      of the divergence between the two arms (see the module docstring).
    """
    changed = [n for n in PRODUCED if on_disk is None or on_disk.get(n) != produced[n]]
    old_delta = on_disk["delta.json"] if on_disk else None
    old_base = base_of(old_delta)

    def paths(d) -> dict | None:
        if not isinstance(d, dict):
            return None
        return {k: list(d.get(k) or []) for k in ("added", "removed", "changed")}

    was, now = paths(old_delta), paths(produced["delta.json"])
    n_was = sum(len(v) for v in was.values()) if was else 0
    n_now = sum(len(v) for v in now.values()) if now else 0
    return {
        "created": on_disk is None,
        "changed": changed,
        "unchanged": [n for n in PRODUCED if n not in changed],
        "recorded_moved": [n for n in changed if n in RECORDED],
        "old_base": old_base,
        "new_base": base_sha,
        "rebased": old_base is not None and old_base != base_sha,
        "delta_lists_moved": was != now,
        # A record this write DISCARDS: the delta it replaced listed paths and the
        # one it wrote lists none. Honest for the new base and still a loss, so it
        # is said rather than left to `git diff`.
        "discarded": n_was if (n_was and not n_now) else 0,
    }


def write_report(effect: dict, delta: dict, moved: list[str], fields: list[str]) -> list[str]:
    """The write arm's whole output, as two blocks that cannot be read as one.

    The defect this replaces: one sentence carried three figures — changed paths,
    warning rows, recorded manifest values — every one of them measured between
    THIS TREE and THE REVIEW BASE, and none of them measured against what was on
    disk. On a branch whose review base had advanced, that sentence announced
    `129 changed path(s); 24 warning row(s) at a new count; 17 recorded manifest
    value(s) moved` over a write that changed exactly one JSON field,
    `delta.json`'s `base_commit`. It reads as an emission drift; the words
    `moved` and `at a new count` are the tree's vocabulary, and the reader has
    nothing in the output to weigh them against. A false drift signal is the
    expensive mirror of a reassuring gloss, because a false drift gets acted on:
    a round told only "run `--write`" commits it as a regeneration, and the
    commit carries a measurement of nothing.

    So the effect is stated FIRST, in documents rather than in path counts, and
    the difference between two commits is stated second with both of its ends
    named. Neither block can stand in for the other: the first cannot be large
    (there are four documents), and the second never claims to be an effect.

    Explaining the ambiguity would not have been enough — the drift reading is
    removed by there being a measurement of the write's own effect at all, which
    is what did not exist. The prose only names what that measurement found.
    """
    n = len(effect["changed"])
    lines = [
        f"wrote {BASELINE} — this invocation changed {n} of {len(PRODUCED)} document(s):"
    ]
    for name in effect["changed"]:
        if name != "delta.json":
            lines.append(f"  ~ {name} — {'written' if effect['created'] else 'rewritten'}")
            continue
        if effect["rebased"]:
            tail = (
                "its path lists are unchanged"
                if not effect["delta_lists_moved"]
                else "its path lists moved with it"
            )
            lines.append(
                f"  ~ delta.json — RE-BASED `{effect['old_base'][:12]}` -> "
                f"`{effect['new_base'][:12]}`; {tail}"
            )
        elif effect["created"] or effect["old_base"] is None:
            lines.append(f"  ~ delta.json — written against `{effect['new_base'][:12]}`")
        else:
            lines.append(
                f"  ~ delta.json — rewritten against the same base "
                f"`{effect['new_base'][:12]}`"
            )
    if effect["unchanged"]:
        lines.append(f"  = {', '.join(effect['unchanged'])} — unchanged")
    if not effect["created"] and not effect["recorded_moved"]:
        lines.append(
            "  NOT AN EMISSION DRIFT: every document that RECORDS this tree — "
            f"{', '.join(RECORDED)} — already held exactly what this run measured, "
            "so emission, the warning set and the recorded manifest values did not "
            "move. `--base` has advanced since the review delta was written and the "
            "delta followed it. The figures below are a difference between two "
            "commits; this write did not produce them."
        )
    if effect["discarded"]:
        lines.append(
            f"  AND IT DISCARDED A REVIEW RECORD: the delta it replaced listed "
            f"{effect['discarded']} path(s) against `{effect['old_base'][:12]}`, and the "
            f"one written here is empty — this tree moves no emission relative to "
            f"`{effect['new_base'][:12]}`. That record now survives only in git at "
            f"`{effect['old_base'][:12]}`."
        )
    lines.append(
        f"review delta now recorded — THIS TREE against `{effect['new_base'][:12]}`, "
        "which is a difference between two commits and not a list of what this "
        "invocation changed:"
    )
    lines.append(
        f"  {len(delta['added'])} added, {len(delta['removed'])} removed, "
        f"{len(delta['changed'])} changed path(s); {len(moved)} warning row(s) at a "
        f"different count; {len(fields)} recorded manifest value(s) differ from the base."
    )
    lines.extend(moved + fields)
    return lines


def warning_delta(old: dict, new: dict) -> list[str]:
    """Every warning row whose COUNT moved, named, with both counts.

    Its sibling `compute_delta` lists every differing emitted path and the
    header branch lists every differing input; this branch used to list nothing,
    so a red here meant rebuilding the gallery to find out what it was about.

    Counted, never set-compared, and that distinction is the whole point rather
    than a detail: the first live difference this helper met was a warning the
    engine had been emitting TWICE and now emits once — six rows across six
    builds, one per build. As sets the two ledgers are identical, so a
    set-shaped delta printed "no row added or removed" beside a message saying
    they differ. A set is a lossy reading of a list, and the loss is exactly the
    class of change a duplicated pass produces.
    """
    def counted(m: dict) -> dict:
        out: dict = {}
        for bid, rs in m.items():
            for r in rs:
                key = (bid, json.dumps(r, sort_keys=True))
                out[key] = out.get(key, 0) + 1
        return out

    before, now = counted(old), counted(new)
    lines = []
    for key in sorted(set(before) | set(now)):
        was, is_ = before.get(key, 0), now.get(key, 0)
        if was == is_:
            continue
        bid, row = key
        mark = "+" if is_ > was else "-"
        lines.append(f"  {mark} {bid}  x{was} -> x{is_}  {row}")
    if not lines:
        # Same rows at the same counts: what moved is the shape of the
        # containing document — a build id gained or lost an empty list. Say so,
        # because an empty delta beside a message asserting a difference reads as
        # the check being wrong.
        lines.append(
            f"  (every row and count identical; the build id set moved: "
            f"{sorted(set(old))} -> {sorted(set(new))})"
        )
    return lines


def field_count(manifests: dict) -> int:
    """How many recorded-but-not-emitted manifest values this run compares.

    The denominator for the third verdict, stated for the same reason the other
    two are: a comparison that examined nothing is vacuous, not a pass — and this
    one examined nothing at all for as long as it did not exist. Counted at the
    leaf, so each `inputs` entry is its own recorded value, because that is the
    granularity `manifest_field_delta` names them at.
    """
    n = 0
    for man in manifests.values():
        for k, v in man.items():
            if k == "outputs":
                continue
            n += len(v) if isinstance(v, dict) else 1
    return n


def manifest_field_delta(old: dict, new: dict, left: str = "baseline") -> list[str]:
    """Every recorded-but-not-emitted manifest value whose content moved, named.

    `left` names what `old` IS, and it is a parameter because the two callers hand
    it different things. The verify arm's `old` is the committed baseline, so
    `baseline` is right there. The write arm's `old` is the manifest set at the
    REVIEW BASE — a commit, not the file on disk — and calling that "baseline"
    told the reader the committed file had moved, which is the same misreading
    `write_report` exists to end, one line further down the page.

    A manifest is not its outputs. The pinned `delvec_version` / `dsl_version` /
    `mc_version`, `campaign_id`, `resource_pack_sha1` and the whole `inputs`
    index are SIBLINGS of `outputs` — so a manifest can differ while every
    emitted byte is identical, and `compute_delta`, which walks `outputs`, then
    correctly reports nothing at all. That empty list beside a real mismatch is
    how such a change is announced as a determinism finding over zero differing
    paths unless something names the field that moved.

    Named rather than dumped: two seven-build manifests printed whole is not a
    reading anyone takes a finding from. `inputs` is descended one level so an
    entry is named by the file it indexes, which is what the compiler recorded.
    """
    lines: list[str] = []
    for bid in sorted(set(old) | set(new)):
        if bid not in new:
            lines.append(f"  - build `{bid}`: in the baseline, not built by this run")
            continue
        if bid not in old:
            lines.append(f"  + build `{bid}`: built by this run, not in the baseline")
            continue
        a, b = old[bid], new[bid]
        for k in sorted((set(a) | set(b)) - {"outputs"}):
            av, bv = a.get(k), b.get(k)
            if av == bv:
                continue
            if isinstance(av, dict) and isinstance(bv, dict):
                for sub in sorted(set(av) | set(bv)):
                    if av.get(sub) != bv.get(sub):
                        lines.append(
                            f"  ~ {bid}.{k}[{sub}]: {left} `{av.get(sub)}` "
                            f"vs this tree `{bv.get(sub)}`"
                        )
                continue
            lines.append(f"  ~ {bid}.{k}: {left} `{av}` vs this tree `{bv}`")
    return lines


def compute_delta(old: dict, new: dict) -> dict:
    """Every added/removed/changed emitted path, grouped by output class.

    The review artifact, and it opens with the question its reader answers: *is
    every path class listed here a consequence this change claims to have?*

    It walks `outputs` and NOTHING else, deliberately — it is about emitted
    bytes. Every other thing a manifest records is `manifest_field_delta`'s, and
    the two are read together: this one empty while the manifests differ is not
    "no difference", it is "the difference is not in emission".
    """
    def files(m: dict) -> dict:
        out = {}
        for bid, man in m.items():
            for path, sha in (man.get("outputs") or man.get("files") or {}).items():
                out[f"{bid}:{path}"] = sha
        return out

    a, b = files(old), files(new)
    added = sorted(set(b) - set(a))
    removed = sorted(set(a) - set(b))
    changed = sorted(k for k in set(a) & set(b) if a[k] != b[k])

    def klass(p: str) -> str:
        tail = p.split(":", 1)[1]
        for pre, name in (
            ("datapack/data/", "datapack function"),
            ("packtest-datapack/", "PackTest"),
            ("creator-datapack/", "creator overlay"),
            ("validation/", "validation ledger"),
            ("server/", "server config"),
            ("structures/", "structure"),
        ):
            if tail.startswith(pre):
                return name
        return "other"

    classes: dict[str, int] = {}
    for p in added + removed + changed:
        classes[klass(p)] = classes.get(klass(p), 0) + 1
    return {"added": added, "removed": removed, "changed": changed, "classes": classes}


def review_delta(base_sha: str, old: dict, new: dict) -> dict:
    """`compute_delta` plus the commit `old` was read from — the committed artifact.

    Naming the base is what turns a review note into a checkable claim: a reader
    of `gallery/baseline/delta.json` can say which two states it compares, and so
    can a gate. Without it the file asserts a difference between one state that
    is present and one that is nowhere.
    """
    return {"base_commit": base_sha, **compute_delta(old, new)}


def delta_binding(old: dict, new: dict) -> int:
    """Emitted paths weighed to produce the review delta — its denominator.

    The union of both sides, never the delta's own length. An honest delta is
    empty whenever a change moves a recorded input without moving an emitted
    byte, so a zero there is the commonest legitimate result; a zero HERE means
    the comparison had nothing on either side and the artifact is vacuous.
    """
    def files(m: dict) -> set[str]:
        return {
            f"{bid}:{p}"
            for bid, man in m.items()
            for p in (man.get("outputs") or man.get("files") or {})
        }

    return len(files(old) | files(new))


if __name__ == "__main__":
    raise SystemExit(main())
