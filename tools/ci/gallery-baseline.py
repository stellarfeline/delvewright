#!/usr/bin/env python3
"""The gallery's emission baseline, computed in the run (spec-0039 §5, ADR-0034).

## What one invocation does

It measures the gallery domain twice, from source, and publishes the delta:

- **head** — this checkout: the `delvec` handed by `--delvec`, the prefab
  directory handed by `--prefabs`, and this tree's `gallery/`;
- **comparison** — `merge-base(--base, HEAD)`, or `HEAD^1` when HEAD is itself on
  `--base` (a push to `main`). That commit is checked out into a worktree of its
  own and built there entirely from its own sources: its own `cargo build
  --locked -p delvec --bin delvec`, its own `tools/ci/gallery-prefabs.py`, its
  own `gallery/`, its own `tools/ci/gallery_domain.py` and `gallery_units.py`.
  Head's tools never run over the comparison's sources; what head contributes is
  the loop that asks each tree's own modules for its points and runs that tree's
  own `delvec` over them, identically on both sides.

A measurement is, per build of the domain, the compiler's `manifest.json` (its
SHA-256 index over every input and every output path) and the warning rows the
build printed, plus a header: the commit, `delvec --version`, the gallery source
hash, the generator-input hash and the coverage counts (spec-0039 §6).

Nothing of it is committed except `gallery/baseline/warnings.json`, the
expected-warnings ledger. The rest is written under `--work` (gitignored) and to
`$GITHUB_STEP_SUMMARY` when that is set; the head manifests are the document
`tools/ci/gallery-build.py` proves its tree against.

## Red, report, or green — said in the summary, every run

- **red**: either side fails to build (never a fallback); the head's warning set
  differs from the committed ledger (criterion 5); the delta is non-empty while
  the change reaches none of `EMISSION_INPUTS` — a determinism finding
  (ADR-0006); either side measured zero builds or zero emitted paths.
- **report**: the delta is non-empty and the change reaches an emission input.
  The reviewer reads it; it does not red.
- **green**: the delta is empty and the ledger matches.

The delta is two lists read together: every added/removed/changed emitted path
(by output class), and every recorded-but-not-emitted manifest value that moved
(`inputs`, versions, `campaign_id`, …). A manifest can differ while every
emitted byte is identical, so the second list is what keeps such a change from
being described as nothing.

## `--write`

Measures the head only and writes `gallery/baseline/warnings.json`. A write that
would not move a byte is refused as a noise commit, before anything is written.
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from types import ModuleType

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
import gallery_domain  # noqa: E402
import parallel  # noqa: E402
from delvec_bin import resolve as resolve_delvec  # noqa: E402
from gitbase import BaseUnresolved, resolve_base  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
LEDGER = REPO / "gallery" / "baseline" / "warnings.json"
DEFAULT_WORK = REPO / ".gallery-baseline"

# A warning line: code, stage, then the POINTER, which runs to the first `: `
# that ends it. A build-tier pointer names a phase of the build ("packtest watch
# coverage") and contains spaces. `ANY_WARNING_RE` is what keeps a line that
# announces itself as a warning and does not parse from being dropped: it reds.
WARNING_RE = re.compile(r"^(DW\d{4}) \[warning\] (\S+) (.+?): ")
ANY_WARNING_RE = re.compile(r"^(DW\d{4}) \[warning\] ")

# The paths whose content can reach a byte the gallery build emits or records.
# A non-empty delta on a change reaching none of them is a determinism finding.
# Membership is decided by that question and nothing else: the compiler with its
# manifest, lockfile and toolchain pin, the gallery source, the generator that
# writes its pieces, and the two tools that turn those into a campaign and a
# prefab directory. `versions.toml` is deliberately absent: the compiler reads
# nothing from it.
EMISSION_INPUTS = (
    "crates/",
    "gallery/",
    "prefabs/gallery-generator/",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "tools/ci/gallery_domain.py",
    "tools/ci/gallery-prefabs.py",
)

# Lines of path detail printed in the job summary; `delta.json` holds all of them.
SUMMARY_DETAIL = 400


class Red(Exception):
    """A verdict that fails the run, carrying the summary line that names it."""

    def __init__(self, verdict: str, detail: str):
        super().__init__(detail)
        self.verdict = verdict
        self.detail = detail


def write_canonical(path: Path, obj) -> None:
    """One document in `delvec fmt` canonical form (`ensure_ascii=False` is load-bearing)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(obj, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def tree_hash(root: Path, skip: set[str]) -> str:
    """A deterministic hash of a tree: relative path AND content, in path order."""
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


def under(path: str, prefix: str) -> bool:
    """`path` IS `prefix`, or lies inside it — never merely starts with its text."""
    return path == prefix.rstrip("/") or path.startswith(prefix.rstrip("/") + "/")


def git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)


# ------------------------------------------------------------------ one tree


class Tree:
    """One checkout of the repository, read through ITS OWN modules.

    `gallery_domain` and `gallery_units` are loaded from the tree's own
    `tools/ci/`, so how a point is materialised, which overlays exist and which
    units the schema declares are that tree's answers — never head's applied to
    another tree's sources.
    """

    def __init__(self, root: Path, tag: str):
        self.root = Path(root).resolve()
        self.tag = tag
        self.domain = self._load("gallery_domain")
        self.units = self._load("gallery_units")

    def _load(self, name: str) -> ModuleType:
        path = self.root / "tools" / "ci" / f"{name}.py"
        if not path.is_file():
            raise Red(f"{self.tag} tree unreadable", f"`{path}` is missing")
        spec = importlib.util.spec_from_file_location(f"{name}__{self.tag}", path)
        mod = importlib.util.module_from_spec(spec)
        assert spec.loader is not None
        spec.loader.exec_module(mod)
        return mod

    @property
    def gallery(self) -> Path:
        return self.root / "gallery"

    def languages(self) -> list[str]:
        world = json.loads((self.gallery / "world.json").read_text(encoding="utf-8"))
        return list(world["content"].get("languages") or [])

    def points(self) -> list[tuple[str | None, str]]:
        """The builds of this tree's domain: its languages and overlays, by the one rule."""
        return gallery_domain.points(self.languages(), self.domain.overlays())

    def commit(self) -> str:
        r = git(self.root, "rev-parse", "HEAD")
        return r.stdout.strip() if r.returncode == 0 else ""

    def dirty(self) -> bool:
        r = git(self.root, "status", "--porcelain", "--untracked-files=no")
        return r.returncode == 0 and bool(r.stdout.strip())


def run_build(tree: Tree, delvec: Path, prefabs: Path, overlay: str | None, lang: str, work: Path):
    """Materialise one point with the tree's own module and compile it with the tree's `delvec`."""
    src = work / f"src-{overlay or 'primary'}-{lang}"
    out = work / f"out-{overlay or 'primary'}-{lang}"
    tree.domain.materialise(src, tree.gallery / "overlays" / overlay if overlay else None)
    r = subprocess.run(
        [str(delvec), "--lang", lang, "build", str(src), "-o", str(out), "--prefabs", str(prefabs)],
        capture_output=True,
        text=True,
    )
    return out, r


def read_build(side: str, overlay: str | None, lang: str, out: Path, r: subprocess.CompletedProcess):
    """`(manifest, warning rows)` of one finished build, or the red it earns."""
    key = gallery_domain.build_id(overlay, lang)
    if r.returncode != 0:
        raise Red(
            f"build failed ({side})",
            f"the {side} build `{key}` exited {r.returncode}. A delta is only "
            "meaningful between two green builds, and a failed side is never "
            "replaced by anything.\n" + r.stdout + r.stderr,
        )
    rows = []
    for line in (r.stdout + r.stderr).splitlines():
        stripped = line.strip()
        m = WARNING_RE.match(stripped)
        if m:
            rows.append({"code": m.group(1), "stage": m.group(2), "pointer": m.group(3)})
        elif ANY_WARNING_RE.match(stripped):
            raise Red(
                f"unparseable warning ({side})",
                "a warning line was emitted that the ledger cannot parse, so it "
                f"would have been dropped from the comparison in silence:\n  {stripped[:200]}",
            )
    return json.loads((out / "manifest.json").read_text(encoding="utf-8")), rows


def coverage_counts(tree: Tree, delvec: Path) -> dict:
    """Units total / bound / refusal-proven / unaccounted, by the tree's own enumerator."""
    r = subprocess.run([str(delvec), "schema", "--stage", "all"], capture_output=True, text=True)
    if r.returncode != 0:
        raise Red(f"schema export failed ({tree.tag})", f"`delvec schema --stage all` exited {r.returncode}")
    export = json.loads(r.stdout)
    e = tree.units.Enumerator(export)
    units = e.run()
    if not units:
        raise Red(f"vacuous ({tree.tag})", "the schema export enumerated ZERO units")

    def bind_dir(root: Path, label: str, into: set) -> None:
        b = tree.units.Binder(e)
        for stage, fn in tree.units.stage_files(export).items():
            f = root / fn
            if f.is_file():
                b.walk(export[stage], json.loads(f.read_text(encoding="utf-8")), label)
        into |= set(b.bound) & set(units)

    bound: set = set()
    bind_dir(tree.gallery, "primary", bound)
    work = Path(tempfile.mkdtemp(prefix="gallery-header-"))
    try:
        for name in tree.domain.overlays():
            dest = work / name
            tree.domain.materialise(dest, tree.gallery / "overlays" / name)
            bind_dir(dest, f"overlay:{name}", bound)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    proven: set = set()
    pdir = tree.gallery / "probes"
    if pdir.is_dir():
        for d in sorted(x for x in pdir.iterdir() if x.is_dir()):
            m = json.loads((d / "probe.json").read_text(encoding="utf-8"))
            proven |= {u for u in (m.get("units") or []) if u in units}
    return {
        "units_total": len(units),
        "units_bound": len(bound),
        "units_refusal_proven": len(proven - bound),
        "units_unaccounted": len(set(units) - bound - proven),
    }


def delvec_versions(delvec: Path) -> dict:
    r = subprocess.run([str(delvec), "--version"], capture_output=True, text=True)
    if r.returncode != 0:
        raise Red("delvec unrunnable", f"`{delvec} --version` exited {r.returncode}")
    # `delvec x.y.z, dsl a.b.c, mc x.y.z`
    parts = dict(
        (k.strip(), v.strip()) for k, v in (p.split(" ", 1) for p in r.stdout.strip().split(", "))
    )
    return {"delvec": parts.get("delvec", ""), "dsl": parts.get("dsl", "")}


def measure(tree: Tree, delvec: Path, prefabs: Path) -> dict:
    """One side: `{header, manifests, warnings}` over every point of the tree's domain."""
    points = tree.points()
    work = Path(tempfile.mkdtemp(prefix=f"gallery-baseline-{tree.tag}-"))
    manifests, warnings = {}, {}
    try:
        runs = parallel.ordered(lambda p: run_build(tree, delvec, prefabs, p[0], p[1], work), points)
        with contextlib.closing(runs):
            for (overlay, lang), outcome in zip(points, runs):
                m, rows = read_build(tree.tag, overlay, lang, *outcome.get())
                manifests[gallery_domain.build_id(overlay, lang)] = m
                warnings[gallery_domain.build_id(overlay, lang)] = rows
    finally:
        shutil.rmtree(work, ignore_errors=True)
    v = delvec_versions(delvec)
    header = {
        "commit": tree.commit(),
        "dirty": tree.dirty(),
        "coverage": coverage_counts(tree, delvec),
        "delvec_version": v["delvec"],
        "dsl_version": v["dsl"],
        # `baseline/` holds the committed ledger and `README.md` is prose; neither
        # can reach a byte of emission.
        "gallery_source_sha256": tree_hash(tree.gallery, {"baseline", "README.md"}),
        "generator_input_sha256": tree_hash(Path(prefabs), set()),
    }
    return {"header": header, "manifests": manifests, "warnings": warnings}


def paths_of(manifests: dict) -> int:
    return sum(len(m.get("outputs") or {}) for m in manifests.values())


# ------------------------------------------------------- the comparison tree


def comparison_commit(base_ref: str, override: str | None) -> tuple[str, str]:
    """`(sha, how)` — the commit the head is compared with, and how it was chosen."""
    if override:
        r = git(REPO, "rev-parse", "--verify", f"{override}^{{commit}}")
        if r.returncode != 0:
            raise Red("comparison unresolved", f"`--compare {override}` names no commit here")
        return r.stdout.strip(), f"`--compare {override}`"
    try:
        base = resolve_base(REPO, base_ref, "gallery-baseline")
    except BaseUnresolved as e:
        raise Red("comparison unresolved", e.message) from None
    head = git(REPO, "rev-parse", "HEAD").stdout.strip()
    mb = git(REPO, "merge-base", base, "HEAD")
    if mb.returncode != 0 or not mb.stdout.strip():
        raise Red(
            "comparison unresolved",
            f"no merge base between `{base_ref}` and HEAD — a shallow checkout is the usual cause",
        )
    sha = mb.stdout.strip()
    if sha != head:
        return sha, f"merge-base(`{base_ref}`, HEAD)"
    parent = git(REPO, "rev-parse", "--verify", "HEAD^1")
    if parent.returncode != 0:
        raise Red("comparison unresolved", "HEAD is on the base branch and has no parent")
    return parent.stdout.strip(), f"HEAD^1 (HEAD is on `{base_ref}`)"


def prepare_comparison(sha: str, work: Path, target_dir: Path) -> tuple[Path, Path, Path]:
    """Check `sha` out into its own worktree and build its `delvec` and prefabs there.

    Returns `(root, delvec, prefabs)`. Every step runs that tree's own command.
    """
    root = work / "compare"
    if root.exists():
        listed = git(REPO, "worktree", "list", "--porcelain").stdout
        if f"worktree {root.resolve()}" not in listed.splitlines():
            raise Red(
                "comparison tree blocked",
                f"`{root}` exists and is not a worktree of this repository; refusing to delete it",
            )
        r = git(REPO, "worktree", "remove", "--force", str(root))
        if r.returncode != 0:
            raise Red("comparison tree blocked", r.stderr)
    git(REPO, "worktree", "prune")
    r = git(REPO, "worktree", "add", "--detach", str(root), sha)
    if r.returncode != 0:
        raise Red("comparison checkout failed", r.stderr)
    env = dict(os.environ, CARGO_TARGET_DIR=str(target_dir))
    print(f"gallery-baseline: building the comparison tree `{sha[:12]}` from source", flush=True)
    b = subprocess.run(
        ["cargo", "build", "--locked", "-p", "delvec", "--bin", "delvec"],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
    )
    if b.returncode != 0:
        raise Red(
            "build failed (comparison)",
            f"`cargo build --locked -p delvec --bin delvec` at `{sha[:12]}` exited {b.returncode}\n"
            + b.stderr[-4000:],
        )
    delvec = target_dir / "debug" / "delvec"
    prefabs = root / "gallery-prefabs"
    p = subprocess.run(
        [
            sys.executable,
            str(root / "tools" / "ci" / "gallery-prefabs.py"),
            "--delvec",
            str(delvec),
            "--out",
            str(prefabs),
        ],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
    )
    if p.returncode != 0:
        raise Red(
            "build failed (comparison)",
            f"`tools/ci/gallery-prefabs.py` at `{sha[:12]}` exited {p.returncode}\n"
            + (p.stdout + p.stderr)[-4000:],
        )
    return root, delvec, prefabs


def reclaim_comparison(root: Path) -> None:
    """The comparison worktree is removed and pruned; its cargo target dir is the cache and stays."""
    if root.exists():
        git(REPO, "worktree", "remove", "--force", str(root))
    git(REPO, "worktree", "prune")


def changed_paths(sha: str) -> list[str]:
    """What this change touches relative to the comparison: the working tree against `sha`."""
    r = git(REPO, "diff", "--name-only", sha)
    if r.returncode != 0:
        raise Red("diff unreadable", f"`git diff --name-only {sha}` exited {r.returncode}: {r.stderr}")
    return r.stdout.splitlines()


# ----------------------------------------------------------------- the delta


def compute_delta(old: dict, new: dict) -> dict:
    """Every added/removed/changed emitted path, `<build>:<path>`, grouped by output class."""

    def files(m: dict) -> dict:
        return {
            f"{bid}:{p}": sha
            for bid, man in m.items()
            for p, sha in (man.get("outputs") or {}).items()
        }

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


def manifest_field_delta(old: dict, new: dict) -> list[str]:
    """Every recorded-but-not-emitted manifest value that moved, named (`inputs` by file)."""
    lines: list[str] = []
    for bid in sorted(set(old) | set(new)):
        if bid not in new:
            lines.append(f"- build `{bid}`: in the comparison, not in the head")
            continue
        if bid not in old:
            lines.append(f"+ build `{bid}`: in the head, not in the comparison")
            continue
        a, b = old[bid], new[bid]
        for k in sorted((set(a) | set(b)) - {"outputs"}):
            av, bv = a.get(k), b.get(k)
            if av == bv:
                continue
            if isinstance(av, dict) and isinstance(bv, dict):
                for sub in sorted(set(av) | set(bv)):
                    if av.get(sub) != bv.get(sub):
                        lines.append(f"~ {bid}.{k}[{sub}]: `{av.get(sub)}` -> `{bv.get(sub)}`")
                continue
            lines.append(f"~ {bid}.{k}: `{av}` -> `{bv}`")
    return lines


def warning_delta(old: dict, new: dict) -> list[str]:
    """Every warning row whose COUNT moved, with both counts — counted, never set-compared.

    A duplicated pass emits the same row twice; as sets the two ledgers would be
    identical while the documents differ.
    """

    def counted(m: dict) -> dict:
        out: dict = {}
        for bid, rs in m.items():
            for r in rs:
                key = (bid, json.dumps(r, sort_keys=True, ensure_ascii=False))
                out[key] = out.get(key, 0) + 1
        return out

    before, now = counted(old), counted(new)
    lines = []
    for key in sorted(set(before) | set(now)):
        was, is_ = before.get(key, 0), now.get(key, 0)
        if was != is_:
            lines.append(f"{'+' if is_ > was else '-'} {key[0]}  x{was} -> x{is_}  {key[1]}")
    if not lines and old != new:
        lines.append(
            f"(every row and count identical; the build id set moved: {sorted(old)} -> {sorted(new)})"
        )
    return lines


def emission_inputs_touched(paths: list[str]) -> list[str]:
    return [p for p in paths if any(under(p, i) for i in EMISSION_INPUTS)]


def verdict(delta: dict, fields: list[str], touched: list[str], ledger_drift: list[str]) -> tuple[str, list[str]]:
    """`(kind, reds)` — kind is `green`, `report` or `red`; reds names every red condition met."""
    reds = []
    if ledger_drift:
        reds.append("warning ledger drift (spec-0039 criterion 5)")
    moved = bool(delta["added"] or delta["removed"] or delta["changed"] or fields)
    if moved and not touched:
        reds.append(
            "DETERMINISM FINDING (ADR-0006): the gallery's emission moved and the change "
            "reaches none of " + ", ".join(f"`{p}`" for p in EMISSION_INPUTS)
        )
    if reds:
        return "red", reds
    return ("report" if moved else "green"), []


def summary(doc: dict) -> list[str]:
    """The job summary, as markdown lines: the verdict first, then the binding."""
    b = doc["binding"]
    d = doc["delta"]
    kind = doc["verdict"]["kind"]
    head = {
        "green": "GREEN — no emitted byte or recorded value moved, and the warning ledger matches.",
        "report": "REPORT — emission moved, and the change reaches an emission input. Read the delta; it does not red.",
        "red": "RED — " + "; ".join(doc["verdict"]["reds"]) + ".",
    }[kind]
    lines = [
        "## Gallery baseline",
        "",
        f"**{head}**",
        "",
        f"- head `{doc['head']['commit'][:12]}`{' (dirty)' if doc['head']['dirty'] else ''} "
        f"vs comparison `{doc['compare']['commit'][:12]}` — {doc['compare_how']}",
        f"- builds compared: head {b['builds_head']}, comparison {b['builds_compare']}, in both {b['builds_both']}",
        f"- emitted paths: head {b['paths_head']}, comparison {b['paths_compare']}; weighed {b['paths_union']}",
        f"- delta: {len(d['added'])} added, {len(d['removed'])} removed, {len(d['changed'])} changed "
        f"path(s) {json.dumps(d['classes'], sort_keys=True)}; {len(doc['fields'])} recorded value(s) moved",
        f"- warning rows: head {b['warnings_head']} against the committed ledger's {b['warnings_ledger']}; "
        f"{len(doc['ledger_drift'])} row(s) at a different count",
        f"- the change reaches {len(doc['touched'])} emission-input path(s) of {b['changed_paths']} changed",
    ]
    for side in ("compare", "head"):
        h = doc[side]
        lines.append(
            f"- {side} header: delvec {h['delvec_version']}, dsl {h['dsl_version']}, "
            f"coverage {json.dumps(h['coverage'], sort_keys=True)}, gallery `{h['gallery_source_sha256'][:12]}`, "
            f"generator `{h['generator_input_sha256'][:12]}`"
        )
    detail: list[str] = []
    if doc["ledger_drift"]:
        detail += ["warning ledger drift (head vs `gallery/baseline/warnings.json`):", *doc["ledger_drift"]]
    if d["added"] or d["removed"] or d["changed"]:
        detail += ["emitted paths:"]
        detail += [f"+ {p}" for p in d["added"]] + [f"- {p}" for p in d["removed"]]
        detail += [f"~ {p}" for p in d["changed"]]
    if doc["fields"]:
        detail += ["recorded values:", *doc["fields"]]
    if doc["warnings_moved"]:
        detail += ["warning rows, comparison -> head (information):", *doc["warnings_moved"]]
    if detail:
        shown = detail[:SUMMARY_DETAIL]
        if len(detail) > SUMMARY_DETAIL:
            shown.append(f"… {len(detail) - SUMMARY_DETAIL} more line(s) in delta.json")
        lines += ["", "```", *shown, "```"]
    return lines


def emit_summary(lines: list[str]) -> None:
    text = "\n".join(lines) + "\n"
    print(text)
    target = os.environ.get("GITHUB_STEP_SUMMARY")
    if target:
        with open(target, "a", encoding="utf-8") as f:
            f.write(text)


def run_compare(args, head_delvec: Path, head_prefabs: Path, work: Path) -> int:
    report = work / "report"
    shutil.rmtree(report, ignore_errors=True)
    head_tree = Tree(REPO, "head")
    sha, how = comparison_commit(args.base, args.compare)
    target = Path(args.compare_target_dir) if args.compare_target_dir else work / "target"
    root, cmp_delvec, cmp_prefabs = prepare_comparison(sha, work, target.resolve())
    try:
        cmp = measure(Tree(root, "compare"), cmp_delvec, cmp_prefabs)
    finally:
        reclaim_comparison(root)
    head = measure(head_tree, head_delvec, head_prefabs)

    for side, m in (("head", head), ("comparison", cmp)):
        if not m["manifests"] or paths_of(m["manifests"]) == 0:
            raise Red(f"vacuous ({side})", f"the {side} measured ZERO builds or ZERO emitted paths")

    if not LEDGER.is_file():
        raise Red("ledger missing", f"`{LEDGER.relative_to(REPO)}` is missing; write it with `--write`")
    ledger = json.loads(LEDGER.read_text(encoding="utf-8"))
    ledger_drift = warning_delta(ledger, head["warnings"]) if ledger != head["warnings"] else []
    delta = compute_delta(cmp["manifests"], head["manifests"])
    fields = manifest_field_delta(cmp["manifests"], head["manifests"])
    changed = changed_paths(sha)
    touched = emission_inputs_touched(changed)
    kind, reds = verdict(delta, fields, touched, ledger_drift)

    union = {
        f"{bid}:{p}"
        for m in (cmp["manifests"], head["manifests"])
        for bid, man in m.items()
        for p in (man.get("outputs") or {})
    }
    doc = {
        "compare": cmp["header"],
        "compare_how": how,
        "head": head["header"],
        "delta": delta,
        "fields": fields,
        "touched": touched,
        "ledger_drift": ledger_drift,
        "warnings_moved": warning_delta(cmp["warnings"], head["warnings"]),
        "verdict": {"kind": kind, "reds": reds},
        "binding": {
            "builds_head": len(head["manifests"]),
            "builds_compare": len(cmp["manifests"]),
            "builds_both": len(set(head["manifests"]) & set(cmp["manifests"])),
            "paths_head": paths_of(head["manifests"]),
            "paths_compare": paths_of(cmp["manifests"]),
            "paths_union": len(union),
            "warnings_head": sum(len(v) for v in head["warnings"].values()),
            "warnings_ledger": sum(len(v) for v in ledger.values()),
            "changed_paths": len(changed),
        },
    }
    write_canonical(report / "delta.json", doc)
    write_canonical(report / "header.json", {"compare": cmp["header"], "head": head["header"]})
    write_canonical(report / "manifests.json", {"tree": head["header"]["commit"], "manifests": head["manifests"]})
    write_canonical(report / "manifests-compare.json", {"tree": sha, "manifests": cmp["manifests"]})
    emit_summary(summary(doc))
    print(f"gallery-baseline: report written to `{report}`")
    if kind == "red":
        print(f"error: {'; '.join(reds)}", file=sys.stderr)
        return 1
    return 0


def run_write(head_delvec: Path, head_prefabs: Path) -> int:
    head = measure(Tree(REPO, "head"), head_delvec, head_prefabs)
    if not head["manifests"] or paths_of(head["manifests"]) == 0:
        raise Red("vacuous (head)", "the head measured ZERO builds or ZERO emitted paths")
    on_disk = json.loads(LEDGER.read_text(encoding="utf-8")) if LEDGER.is_file() else None
    if on_disk == head["warnings"]:
        raise Red(
            "noise write refused",
            "`gallery/baseline/warnings.json` already equals the emitted warning set, so this "
            "rewrite is a noise commit. Nothing was written.",
        )
    moved = warning_delta(on_disk or {}, head["warnings"])
    write_canonical(LEDGER, head["warnings"])
    n = sum(len(v) for v in head["warnings"].values())
    print(
        f"wrote `{LEDGER.relative_to(REPO)}`: {n} row(s) over {len(head['warnings'])} build(s); "
        f"{len(moved)} row(s) at a new count:"
    )
    for line in moved:
        print(f"  {line}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument(
        "--delvec",
        help="the head's `delvec`. Default: this tree's build, resolved and NAMED (`tools/lib/delvec_bin.py`).",
    )
    ap.add_argument(
        "--prefabs", required=True, help="the head's gallery prefab directory (`tools/ci/gallery-prefabs.py --out`)"
    )
    ap.add_argument(
        "--write", action="store_true", help="measure the head only and write `gallery/baseline/warnings.json`"
    )
    ap.add_argument(
        "--base", default="origin/main", help="the branch the comparison is the merge base with (default `origin/main`)"
    )
    ap.add_argument("--compare", help="compare with this commit instead; named in the summary")
    ap.add_argument(
        "--work",
        default=str(DEFAULT_WORK),
        help="where the comparison worktree, its cargo target and the report go (default `.gallery-baseline/`, gitignored)",
    )
    ap.add_argument("--compare-target-dir", help="the comparison build's `CARGO_TARGET_DIR` (default `<work>/target`)")
    args = ap.parse_args()

    delvec = resolve_delvec(args.delvec, repo=REPO, caller="gallery-baseline")
    prefabs = Path(args.prefabs).resolve()
    work = Path(args.work).resolve()
    try:
        if args.write:
            return run_write(delvec, prefabs)
        return run_compare(args, delvec, prefabs, work)
    except Red as red:
        emit_summary(["## Gallery baseline", "", f"**RED — {red.verdict}.**", "", "```", red.detail[-6000:], "```"])
        print(f"error: {red.verdict}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
