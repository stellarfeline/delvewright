#!/usr/bin/env python3
"""Hold `.github/ci-reach.toml` to the jobs in `ci.yml` that read it.

WHY THIS EXISTS

On a pull request, `ci.yml` runs a job only when the change touches a path the
job reads (the `changes` job computes that through `tools/lib/ci_reach.py`). A
job skipped on a pull request never ran on that tree, and branch protection
accepts the skip as a passing required check. So a job whose `if` forgot its
group, a group whose job was renamed away, a glob that no longer names anything,
or an `if` that quietly stopped running on `push` or on the plugin release's
`workflow_dispatch` each lets a change merge past a gate that never looked. This
refuses each of them.

WHAT IT ASSERTS

1. The filter job `changes` exists, has no `if` and no `needs`, runs
   `tools/lib/ci_reach.py groups`, and declares exactly one output per table
   group, each `${{ steps.<id>.outputs.<group> }}`.
2. Every other job needs `changes` and has a job-level `if` that reads at least
   one table group as `needs.changes.outputs.<group>`.
3. Evaluated the way Actions evaluates it, every such `if` is true on every event
   in `on:` other than `pull_request` with every group off; on `pull_request` it
   is false with every group off and true with any one of its groups on.
4. Every table group is read by at least one job.
5. Every glob in the table matches at least one tracked file, and the `[all]`
   set matches the workflow, the table and the filter itself.

The `if` grammar understood is the subset the table needs: `github.event_name`,
`needs.changes.outputs.<group>`, string literals, `true`/`false`, `==`, `!=`,
`!`, `&&`, `||` and parentheses. Anything else — a function call such as
`always()`, another context — is refused by name, never guessed at.

The workflow is read through `tools/lib/workflow_yaml.py`, the one shared parse
rule. States its binding counts; a run that judges zero jobs exits 2.
Deterministic, offline, stdlib-only python3.

Usage:
  python3 tools/check-ci-reach.py [--workflow PATH] [--table PATH] [--repo DIR]

Exit 0 clean, 1 with findings, 2 when the inputs cannot be read or bind nothing.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys
from typing import Any

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / "lib"))

import ci_reach  # noqa: E402
from workflow_yaml import WorkflowYamlError, load  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
WORKFLOW = REPO / ".github" / "workflows" / "ci.yml"
FILTER = "changes"
FILTER_INVOCATION = "tools/lib/ci_reach.py groups"
# What the `[all]` set must reach: the workflow, the table and the filter. A
# change to any of them changes what runs, so it runs everything.
ALL_MUST_REACH = (".github/workflows/ci.yml", ".github/ci-reach.toml", "tools/lib/ci_reach.py")
OUTPUT_VALUE = re.compile(r"^\$\{\{\s*steps\.[A-Za-z_][\w-]*\.outputs\.(?P<group>[\w-]+)\s*\}\}$")


class ExprError(ValueError):
    """An `if` construct this gate does not evaluate."""


# ---------------------------------------------------------------- expressions
_TOKEN = re.compile(
    r"\s*(?:(?P<op>==|!=|&&|\|\||!|\(|\))|'(?P<str>(?:[^']|'')*)'|(?P<ident>[A-Za-z_][\w-]*(?:\.[A-Za-z_][\w-]*)*)(?P<call>\s*\()?)"
)


def tokenize(text: str) -> list[tuple[str, str]]:
    body = text.strip()
    if body.startswith("${{") and body.endswith("}}"):
        body = body[3:-2]
    out: list[tuple[str, str]] = []
    pos = 0
    while pos < len(body):
        if body[pos:].strip() == "":
            break
        m = _TOKEN.match(body, pos)
        if not m or m.end() == pos:
            raise ExprError(f"cannot read the expression at {body[pos:]!r}")
        if m.group("op"):
            out.append(("op", m.group("op")))
        elif m.group("str") is not None:
            out.append(("str", m.group("str").replace("''", "'")))
        else:
            if m.group("call"):
                raise ExprError(f"a function call `{m.group('ident')}(…)` is not evaluated by this gate")
            out.append(("ident", m.group("ident")))
        pos = m.end()
    return out


class _Eval:
    """Recursive descent over the token list, evaluating as it parses."""

    def __init__(self, tokens: list[tuple[str, str]], env: dict[str, Any]) -> None:
        self.t = tokens
        self.i = 0
        self.env = env

    def peek(self) -> tuple[str, str] | None:
        return self.t[self.i] if self.i < len(self.t) else None

    def take(self) -> tuple[str, str]:
        tok = self.peek()
        if tok is None:
            raise ExprError("the expression ends early")
        self.i += 1
        return tok

    def run(self) -> Any:
        v = self.or_()
        if self.peek() is not None:
            raise ExprError(f"unexpected {self.peek()[1]!r}")
        return v

    def or_(self) -> Any:
        v = self.and_()
        while self.peek() == ("op", "||"):
            self.take()
            rhs = self.and_()
            v = v if truthy(v) else rhs
        return v

    def and_(self) -> Any:
        v = self.cmp()
        while self.peek() == ("op", "&&"):
            self.take()
            rhs = self.cmp()
            v = rhs if truthy(v) else v
        return v

    def cmp(self) -> Any:
        v = self.unary()
        while self.peek() in (("op", "=="), ("op", "!=")):
            op = self.take()[1]
            rhs = self.unary()
            same = equal(v, rhs)
            v = same if op == "==" else not same
        return v

    def unary(self) -> Any:
        if self.peek() == ("op", "!"):
            self.take()
            return not truthy(self.unary())
        return self.atom()

    def atom(self) -> Any:
        kind, val = self.take()
        if kind == "op" and val == "(":
            v = self.or_()
            if self.take() != ("op", ")"):
                raise ExprError("an unclosed parenthesis")
            return v
        if kind == "str":
            return val
        if kind == "ident":
            if val == "true":
                return True
            if val == "false":
                return False
            if val == "github.event_name" or val.startswith("needs.changes.outputs."):
                return self.env.get(val, "")
            raise ExprError(f"`{val}` is not a context this gate evaluates")
        raise ExprError(f"unexpected {val!r}")


def truthy(v: Any) -> bool:
    return bool(v) if not isinstance(v, str) else v != ""


def equal(a: Any, b: Any) -> bool:
    # Actions compares strings case-insensitively, and a boolean against a
    # string by coercing the boolean to its name.
    a = ("true" if a else "false") if isinstance(a, bool) else a
    b = ("true" if b else "false") if isinstance(b, bool) else b
    return str(a).lower() == str(b).lower()


def groups_read(expr: str) -> list[str]:
    return sorted(
        {val.split(".", 3)[3] for kind, val in tokenize(expr) if kind == "ident" and val.startswith("needs.changes.outputs.")}
    )


def evaluate(expr: str, event: str, on_groups: set[str], all_groups: list[str]) -> bool:
    env: dict[str, Any] = {"github.event_name": event}
    for g in all_groups:
        env[f"needs.changes.outputs.{g}"] = "true" if g in on_groups else "false"
    return truthy(_Eval(tokenize(expr), env).run())


# ----------------------------------------------------------------------- gate
def tracked(repo: pathlib.Path) -> list[str]:
    r = subprocess.run(["git", "-C", str(repo), "ls-files", "-z"], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"check-ci-reach: `git ls-files` exited {r.returncode}: {r.stderr.strip()}")
    return [p for p in r.stdout.split("\0") if p]


def needs_of(job: dict[str, Any]) -> list[str]:
    n = job.get("needs")
    if n is None:
        return []
    if isinstance(n, str):
        return [n]
    return [x for x in n if isinstance(x, str)]


def check(workflow: dict[str, Any], table: ci_reach.Table, files: list[str]) -> tuple[list[str], dict[str, int]]:
    findings: list[str] = []
    groups = list(table.groups)
    on = workflow.get("on")
    events = list(on) if isinstance(on, dict) else [on] if isinstance(on, str) else list(on or [])
    if "pull_request" not in events:
        findings.append("`on:` has no `pull_request`, so nothing here is filtered; the table and the `changes` job are dead weight")
    other_events = [e for e in events if e != "pull_request"]
    jobs = workflow.get("jobs")
    if not isinstance(jobs, dict):
        raise SystemExit("check-ci-reach: the workflow has no `jobs` mapping")

    # 1. the filter
    f = jobs.get(FILTER)
    if not isinstance(f, dict):
        findings.append(f"no `{FILTER}` job: nothing computes the groups every other job reads")
    else:
        if "if" in f:
            findings.append(f"`{FILTER}` has an `if`; every job needs it on every event, so a skip of it skips everything")
        if needs_of(f):
            findings.append(f"`{FILTER}` needs {needs_of(f)}; the filter must depend on nothing")
        steps = f.get("steps") or []
        if not any(isinstance(s, dict) and FILTER_INVOCATION in str(s.get("run") or "") for s in steps):
            findings.append(f"`{FILTER}` has no step running `{FILTER_INVOCATION}`")
        outputs = f.get("outputs") or {}
        if not isinstance(outputs, dict):
            outputs = {}
        for key, value in outputs.items():
            m = OUTPUT_VALUE.match(str(value or "").strip())
            if key not in table.groups:
                findings.append(f"`{FILTER}` output `{key}` is not a group in the table")
            elif not m or m.group("group") != key:
                findings.append(f"`{FILTER}` output `{key}` is {value!r}, not `${{{{ steps.<id>.outputs.{key} }}}}`")
        for g in groups:
            if g not in outputs:
                findings.append(f"table group `{g}` is not an output of `{FILTER}`, so every job reading it reads an empty string")

    # 2 + 3. every other job
    read_by: dict[str, list[str]] = {g: [] for g in groups}
    judged = 0
    for jid, job in jobs.items():
        if jid == FILTER:
            continue
        judged += 1
        if not isinstance(job, dict):
            findings.append(f"job `{jid}` is not a mapping")
            continue
        if FILTER not in needs_of(job):
            findings.append(f"job `{jid}` does not need `{FILTER}`, so it cannot read a group")
        expr = job.get("if")
        if not isinstance(expr, str) or not expr.strip():
            findings.append(f"job `{jid}` has no job-level `if`: it has no filter group and runs on every pull request")
            continue
        try:
            read = groups_read(expr)
        except ExprError as exc:
            findings.append(f"job `{jid}`: `if: {expr}` — {exc}")
            continue
        if not read:
            findings.append(f"job `{jid}` reads no filter group: its `if` names no `needs.{FILTER}.outputs.<group>`")
            continue
        unknown = [g for g in read if g not in table.groups]
        if unknown:
            findings.append(f"job `{jid}` reads group(s) {unknown} the table does not declare")
            continue
        for g in read:
            read_by[g].append(jid)
        try:
            for event in other_events:
                if not evaluate(expr, event, set(), groups):
                    findings.append(f"job `{jid}` does not run on `{event}`: `if: {expr}`")
            if "pull_request" in events:
                if evaluate(expr, "pull_request", set(), groups):
                    findings.append(f"job `{jid}` runs on a pull request that reaches none of its groups: `if: {expr}`")
                for g in read:
                    if not evaluate(expr, "pull_request", {g}, groups):
                        findings.append(f"job `{jid}` does not run on a pull request that reaches its group `{g}`: `if: {expr}`")
        except ExprError as exc:
            findings.append(f"job `{jid}`: `if: {expr}` — {exc}")

    # 4. every group has a reader
    for g, readers in read_by.items():
        if not readers:
            findings.append(f"table group `{g}` is read by no job")

    # 5. every glob binds
    globs = 0
    for where, inputs in [("[all]", table.all)] + [(f"group `{g}`", i) for g, i in table.groups.items()]:
        for inp in inputs:
            globs += 1
            rx = ci_reach.glob_regex(inp.glob)
            if not any(rx.match(p) for p in files):
                findings.append(f"{where}: glob `{inp.glob}` matches no tracked file")
    all_rx = [ci_reach.glob_regex(i.glob) for i in table.all]
    for must in ALL_MUST_REACH:
        if not any(r.match(must) for r in all_rx):
            findings.append(f"`[all]` does not match `{must}`: a change to it would not run every job")

    counts = {"jobs": judged, "groups": len(groups), "globs": globs, "files": len(files), "events": len(events)}
    return findings, counts


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--workflow", type=pathlib.Path, default=WORKFLOW)
    ap.add_argument("--table", type=pathlib.Path, default=ci_reach.TABLE)
    ap.add_argument("--repo", type=pathlib.Path, default=REPO)
    args = ap.parse_args(argv)

    try:
        workflow = load(args.workflow.read_text(encoding="utf-8"))
        table = ci_reach.load_table(args.table)
    except (OSError, WorkflowYamlError, ci_reach.TableError) as exc:
        print(f"check-ci-reach: FATAL — {exc}", file=sys.stderr)
        return 2
    if not isinstance(workflow, dict):
        print("check-ci-reach: FATAL — the workflow is not a mapping", file=sys.stderr)
        return 2
    files = tracked(args.repo)
    findings, counts = check(workflow, table, files)
    binding = (
        f"{counts['jobs']} job(s) judged against {counts['groups']} group(s); "
        f"{counts['globs']} glob(s) over {counts['files']} tracked file(s); {counts['events']} event(s)"
    )
    if counts["jobs"] == 0 or counts["groups"] == 0 or counts["files"] == 0:
        print(f"check-ci-reach: FAIL — a binding of zero ({binding}); this examined nothing", file=sys.stderr)
        return 2
    if findings:
        print(f"check-ci-reach: FAIL — {len(findings)} finding(s); {binding}", file=sys.stderr)
        for line in findings:
            print(f"  - {line}", file=sys.stderr)
        return 1
    print(f"check-ci-reach: OK — {binding}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
