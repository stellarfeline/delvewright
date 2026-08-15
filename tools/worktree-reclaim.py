#!/usr/bin/env python3
"""Reclaim the worktrees whose work has landed, and refuse to touch any other.

## The defect this exists to end

A worktree is created by a dispatch and is supposed to be destroyed by the merge
that lands its work. That obligation lived in a sentence in a document, which is
the UNRUN vacuity mode (`CLAUDE.md`): a gate nothing invokes is not a gate. It
held for roughly one dispatch in four. The disk filled twice — thirty-six trees,
each carrying a full `cargo target/`, until `df` itself was unrunnable — and a
later hand sweep still found nineteen stale trees, twelve of them belonging to
pull requests that had merged or closed, sixty-six gigabytes in seven of them.

So this is not a better sentence. It is bound to the two events a merge cannot
avoid: it is run at the merge, and it is run again from `tools/planner-state.sh`,
which `.claude/settings.json` fires on `SessionStart` and on `UserPromptSubmit`
once the page is stale. A merge done by hand, a pull request closed in a browser,
a stopped worker whose branch was pushed by someone else — none of those pass
through a script, and all of them are drained at the next prompt.

## The opposite defect, which is worse

Clearing disk space once deleted the scratch directories of a RUNNING worker and
its measurements had to be retaken; a later sweep deleted two detached content
checkouts that two live workers were reading through their `campaigns` symlink,
and left both workers with a dangling link mid-run. Keeping a dead tree costs
disk, which is recoverable. Deleting a live one costs work that was never pushed,
which is not. Every rule below is therefore fail-closed, and the tool's default
mode is a dry run.

## What counts as permission to delete (the sixth vacuity mode)

`CLAUDE.md`'s sixth vacuity mode asks two questions of any escape hatch: what
does this permission demand, and COULD THE THING IT IS MEANT TO EXCLUDE PRODUCE
IT? The thing to exclude is a live dispatch. So:

  * Quiet is not evidence. A live agent between two tool calls is quiet.
  * mtime is not evidence, for the same reason.
  * "no process has its cwd in there" is not evidence — an agent between tool
    calls has no process at all.
  * A commit being reachable from the remote is not evidence either. A worker
    reading pinned content at a detached commit has exactly that property, and
    so does a spent verification tree; the two are indistinguishable by any
    local signal.

The one key a live dispatch cannot forge is the REMOTE'S OWN PULL-REQUEST STATE.
`merged` or `closed` is asserted by an authority outside this machine, about work
that has already landed there. A worker still working cannot cause its pull
request to be merged, and a worker whose pull request HAS merged has, by
construction, nothing left in that tree that the remote does not hold.

That key is necessary and never sufficient. Every reclamation additionally
demands, and every one of these outranks it:

  1. the working tree is CLEAN (`git status --porcelain` empty — untracked files
     count, because a worker's unsaved scratch output is untracked);
  2. NO commit in the tree is absent from every remote
     (`git rev-list HEAD --not --remotes`), which is the only git state a
     deletion actually destroys;
  3. NOTHING LIVE POINTS INTO IT. Liveness can live entirely in a symlink
     somewhere else: `campaigns` inside another worker's worktree is how a
     worker reads content, and the tree it resolves to presents every signal a
     spent tree presents. So the sweep builds a REVERSE-REFERENCE index over the
     whole scratch area first, and a tree that anything outside itself links
     into is never a candidate, whatever its git state says;
  4. no LEASE is held (below);
  5. it is not the tree this program is running in, and it is not locked with
     `git worktree lock`.

`git worktree remove` is then called WITHOUT `--force`, so git re-checks
dirtiness itself. Two independent layers refuse the same mistake.

## Leases — how a live dispatch removes itself from the question

A dispatch may claim its tree (`--lease`), and a claimed tree is never evaluated.
The lease is honoured EVEN WHEN IT LOOKS STALE: an expiry that voided a lease
would be "quiet means dead" wearing a timestamp, and that is the belief this tool
exists to refuse. An expired lease is reported, and a lease sitting over a merged
branch is reported too — never silently resolved, because "this looks finished"
is exactly the judgement that deleted a running worker's directories.

The lease lives in the worktree's git admin directory
(`.git/worktrees/<name>/dw-lease.json`), not in the working tree: a file in the
working tree would make it dirty, which conflates a claim with a finding. Git
removes the admin directory as part of `git worktree remove`, so a lease cannot
outlive the tree it names.

## Broken links are loud

A dangling symlink is the shape of this repository's oldest silent failure — a
test that stayed green over a directory that had ceased to exist, because its
filter was "if it exists". A `campaigns` link into a deleted tree fails the same
way one layer out: the worker does not crash, it measures zero. Every dangling
link the reverse-reference scan meets is therefore reported as a finding in its
own right, at every run, whether or not this tool ever touched it.

## Binding counts

Every run states how many repositories it enumerated, how many worktrees it
examined, how many symlinks the reverse-reference scan resolved, how many trees
it reclaimed and the reason for each one it kept. A run that examined nothing is
a FINDING, not a pass: enumerating zero worktrees is what `git worktree list`
answers, confidently, when it is run against the wrong repository — which is why
nothing in this file ever changes directory, and every git call is `git -C`.

Deterministic, stdlib-only python3. Read-only unless `--apply` is given.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

SELF = Path(__file__).resolve()
REPO = SELF.parent.parent

# Directories the reverse-reference scan never descends into: rebuildable build
# output, git internals, package caches, browser profiles. They hold no dispatch
# symlink and walking them turns a two-second scan into a two-minute one.
PRUNE_DIRS = {
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    ".venv",
    "chrome-profile",
    ".cargo",
    ".gradle",
}

# How deep the reverse-reference scan walks below each scan root. A dispatch
# symlink sits at a worktree root (depth 2 from a scratch directory); the margin
# is for a worker that nests one.
SCAN_DEPTH = 5

# Free-space floor, in gibibytes, below which the run widens to `target/`
# directories. Chosen so the widening happens well before the failure it exists
# to prevent: at zero free space the shell cannot open its own output file.
DEFAULT_PRESSURE_GIB = 25

LEASE_FILE = "dw-lease.json"

# Links that are SUPPOSED to dangle. A running browser writes these four as
# sentinels whose "target" is a hostname, a process id or a version string —
# they were never paths and nothing resolves them. Reporting them as findings
# would put six lines of noise above the one real dead dispatch link, and a
# finding list that is mostly noise teaches its reader that the red means
# nothing. They are counted and the count is printed, so the filter itself is
# visible: a silently dropped class is how the next real one gets dropped too.
BENIGN_DANGLING = {
    "SingletonLock",
    "SingletonCookie",
    "SingletonSocket",
    "RunningChromeVersion",
    "lockfile",
}


# ---------------------------------------------------------------------------
# process helpers
#
# Every one of these takes the directory as an argument. Nothing here calls
# `cd`, and nothing composes a command whose first clause changes directory: a
# `cd` in the first clause of a compound command persists through the rest of
# it, which is how `gh` and `git worktree` have been made to answer confidently
# about the wrong repository.
# ---------------------------------------------------------------------------


def run(args: list[str], *, timeout: int = 60) -> tuple[int, str, str]:
    try:
        p = subprocess.run(
            args,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except FileNotFoundError:
        return 127, "", f"{args[0]}: not found"
    except subprocess.TimeoutExpired:
        return 124, "", f"{args[0]}: timed out after {timeout}s"
    return p.returncode, p.stdout, p.stderr


def git(path: Path | str, *args: str, timeout: int = 60) -> tuple[int, str, str]:
    return run(["git", "-C", str(path), *args], timeout=timeout)


def real(p: Path | str) -> Path:
    """Resolve a path for identity comparison.

    The scratch area is reached through `/tmp`, which is a symlink to
    `/private/tmp` on this platform, so two spellings of one directory are the
    normal case rather than the exotic one. Every path this tool compares,
    indexes or refuses goes through here first.
    """
    try:
        return Path(os.path.realpath(str(p)))
    except OSError:
        return Path(str(p))


# ---------------------------------------------------------------------------
# repositories and their worktrees
# ---------------------------------------------------------------------------


class Worktree:
    def __init__(self, path: Path, head: str, branch: str | None):
        self.path = path
        self.head = head
        self.branch = branch  # None => detached
        self.locked = False
        self.prunable = False
        self.exists = path.exists()
        self.dirty_files = 0
        self.unpushed = 0
        self.lease: dict | None = None
        self.inbound: list[Path] = []  # symlinks pointing INTO this tree
        self.pr: dict | None = None
        self.verdict = "KEEP"
        self.reason = "not evaluated"
        self.size_kib: int | None = None

    @property
    def label(self) -> str:
        return self.branch or f"(detached {self.head[:8]})"


def enumerate_worktrees(repo: Path) -> tuple[list[Worktree], Path | None, str | None]:
    """Every worktree of `repo` beyond the main checkout.

    Returns (worktrees, main checkout, error). The main checkout is the first
    record git prints and is never a candidate — and it is reported back so a
    run started inside a LINKED worktree names the checkout it swept rather than
    the directory it happened to be invoked from. The two share one object store
    and give one answer; only the label would have differed, and a label that
    names the wrong directory is how this repository has been misled before.
    """
    code, out, err = git(repo, "worktree", "list", "--porcelain")
    if code != 0:
        return [], None, (err.strip() or f"git worktree list exited {code}")

    records: list[dict] = []
    cur: dict = {}
    for line in out.splitlines():
        if not line.strip():
            if cur:
                records.append(cur)
                cur = {}
            continue
        key, _, value = line.partition(" ")
        cur.setdefault(key, value)
    if cur:
        records.append(cur)

    main = real(records[0]["worktree"]) if records else None
    trees: list[Worktree] = []
    for rec in records[1:]:  # [0] is the main checkout
        path = real(rec["worktree"])
        branch = rec.get("branch")
        if branch and branch.startswith("refs/heads/"):
            branch = branch[len("refs/heads/") :]
        elif "detached" in rec:
            branch = None
        wt = Worktree(path, rec.get("HEAD", ""), branch)
        wt.locked = "locked" in rec
        wt.prunable = "prunable" in rec
        trees.append(wt)
    return trees, main, None


def repo_slug(repo: Path) -> str | None:
    """`owner/name` from the origin remote, so `gh` is told which repository.

    `gh` otherwise infers the repository from the working directory, and this
    tool never has a working directory it trusts.
    """
    code, out, _ = git(repo, "remote", "get-url", "origin")
    if code != 0:
        return None
    url = out.strip()
    if url.endswith(".git"):
        url = url[:-4]
    if url.startswith("git@") and ":" in url:
        url = url.split(":", 1)[1]
    else:
        parts = url.split("/")
        if len(parts) >= 2:
            url = "/".join(parts[-2:])
    return url or None


def pull_requests(slug: str, *, timeout: int) -> tuple[dict[str, dict] | None, str]:
    """branch -> the most decisive pull request the REMOTE holds for it.

    One call per repository, not one per branch: a per-branch loop is slow
    enough that a hook would be tempted to skip it, and a skipped authority is
    an unauthorised deletion waiting to happen.

    When `gh` is unavailable, unauthenticated or slow, this returns None and
    NOTHING in that repository is reclaimable. The absence of the authority is
    never read as permission.
    """
    code, out, err = run(
        [
            "gh",
            "pr",
            "list",
            "-R",
            slug,
            "--state",
            "all",
            "--limit",
            "200",
            "--json",
            "number,state,headRefName,title",
        ],
        timeout=timeout,
    )
    if code != 0:
        return None, (err.strip().splitlines() or ["gh failed"])[0]
    try:
        rows = json.loads(out or "[]")
    except json.JSONDecodeError as exc:
        return None, f"gh returned unparseable JSON ({exc})"

    # A branch may carry several pull requests. OPEN dominates: an open request
    # over a branch means work is still expected on it, whatever a superseded
    # closed one says.
    rank = {"OPEN": 3, "MERGED": 2, "CLOSED": 1}
    best: dict[str, dict] = {}
    for row in rows:
        head = row.get("headRefName") or ""
        if not head:
            continue
        prev = best.get(head)
        if prev is None or rank.get(row.get("state", ""), 0) > rank.get(prev["state"], 0):
            best[head] = row
    return best, ""


# ---------------------------------------------------------------------------
# leases
# ---------------------------------------------------------------------------


def admin_dir(wt_path: Path) -> Path | None:
    """`.git/worktrees/<name>` for a linked worktree — where the lease lives."""
    code, out, _ = git(wt_path, "rev-parse", "--git-dir")
    if code != 0:
        return None
    d = Path(out.strip())
    if not d.is_absolute():
        d = wt_path / d
    return real(d)


def read_lease(wt_path: Path) -> dict | None:
    d = admin_dir(wt_path)
    if d is None:
        return None
    f = d / LEASE_FILE
    if not f.is_file():
        return None
    try:
        return json.loads(f.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        # An unreadable lease is still a lease. Fail closed: something wrote a
        # claim here, and this tool is not the judge of its syntax.
        return {"holder": "(unreadable lease file)", "created": 0, "hours": 0}


def write_lease(wt_path: Path, holder: str, hours: int, reason: str) -> str:
    d = admin_dir(wt_path)
    if d is None:
        raise SystemExit(f"not a git worktree: {wt_path}")
    if not d.exists():
        raise SystemExit(f"no git admin directory for {wt_path} (is it the main checkout?)")
    payload = {
        "holder": holder,
        "reason": reason,
        "created": int(time.time()),
        "hours": hours,
    }
    (d / LEASE_FILE).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    return str(d / LEASE_FILE)


def lease_is_expired(lease: dict) -> bool:
    hours = lease.get("hours") or 0
    created = lease.get("created") or 0
    if not hours or not created:
        return False
    return time.time() > created + hours * 3600


# ---------------------------------------------------------------------------
# reverse-reference index: what points INTO these trees
#
# This is the key the incident added. A detached checkout at a pinned content
# commit is how a worker READS content: no branch, no pull request, clean, fully
# pushed, nobody's cwd inside it, untouched for hours. Its liveness is not in the
# tree at all — it is in a `campaigns` symlink inside somebody else's worktree.
# Deleting it does not fail where the deletion happened; the live worker keeps
# running and measures zero over a directory that no longer exists.
#
# So liveness is established over the whole scratch area, never over a tree in
# isolation.
# ---------------------------------------------------------------------------


class LinkIndex:
    def __init__(self) -> None:
        self.links: list[tuple[Path, Path]] = []  # (link location, resolved target)
        self.broken: list[tuple[Path, str]] = []  # (link location, raw target)
        self.benign_broken = 0
        self.roots: list[Path] = []
        self.examined = 0

    def scan(self, roots: list[Path], depth: int = SCAN_DEPTH) -> None:
        seen: set[Path] = set()
        for root in roots:
            root = real(root)
            if root in seen or not root.is_dir():
                continue
            seen.add(root)
            self.roots.append(root)
            base_depth = len(root.parts)
            for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
                here = Path(dirpath)
                if len(here.parts) - base_depth >= depth:
                    dirnames[:] = []
                # A symlinked directory is reported by os.walk in `dirnames`,
                # not `filenames`, so both lists are inspected.
                for name in list(dirnames) + filenames:
                    p = here / name
                    if not p.is_symlink():
                        continue
                    self.examined += 1
                    raw = os.readlink(p)
                    target = raw if os.path.isabs(raw) else str(here / raw)
                    resolved = real(target)
                    if not os.path.exists(str(resolved)):
                        if p.name in BENIGN_DANGLING:
                            self.benign_broken += 1
                        else:
                            self.broken.append((p, raw))
                    else:
                        self.links.append((p, resolved))
                dirnames[:] = [d for d in dirnames if d not in PRUNE_DIRS and not (here / d).is_symlink()]

    def inbound(self, tree: Path) -> list[Path]:
        """Symlinks OUTSIDE `tree` that resolve to `tree` or into it.

        A link inside the tree pointing at itself proves nothing about
        liveness, and would make every tree permanently unreclaimable.
        """
        hits = []
        for link, target in self.links:
            if is_within(link, tree):
                continue
            if target == tree or is_within(target, tree):
                hits.append(link)
        return hits


def is_within(child: Path, parent: Path) -> bool:
    try:
        child.relative_to(parent)
        return True
    except ValueError:
        return False


# ---------------------------------------------------------------------------
# per-tree facts and the verdict ladder
# ---------------------------------------------------------------------------


def measure(wt: Worktree) -> None:
    if not wt.exists:
        return
    code, out, _ = git(wt.path, "status", "--porcelain")
    wt.dirty_files = len([ln for ln in out.splitlines() if ln.strip()]) if code == 0 else -1
    # Commits reachable from this checkout that NO remote ref holds. This is the
    # only git state a deletion destroys, and it answers for a detached HEAD
    # too, which `@{u}..HEAD` does not.
    code, out, _ = git(wt.path, "rev-list", "--count", "HEAD", "--not", "--remotes")
    wt.unpushed = int(out.strip() or 0) if code == 0 else -1
    wt.lease = read_lease(wt.path)


def decide(wt: Worktree, *, self_paths: set[Path], prs: dict[str, dict] | None, pr_error: str) -> None:
    """The verdict ladder. Order is the design: every KEEP above the reclaim
    rung outranks the pull-request authority, so no amount of "it merged" can
    reach a tree holding work or a tree something points into."""

    if wt.prunable or not wt.exists:
        wt.verdict = "PRUNE"
        wt.reason = "the directory is gone; only git's stale administrative record remains"
        return

    # 1. Unrecoverable state. Outranks everything, by any path, for any reason.
    if wt.dirty_files != 0:
        wt.verdict = "KEEP"
        wt.reason = (
            f"DIRTY ({wt.dirty_files} file(s)) — uncommitted or untracked work lives here"
            if wt.dirty_files > 0
            else "DIRTY state could not be computed"
        )
        return
    if wt.unpushed != 0:
        wt.verdict = "KEEP"
        wt.reason = (
            f"UNPUSHED ({wt.unpushed} commit(s) on no remote) — the one git state a deletion destroys"
            if wt.unpushed > 0
            else "unpushed count could not be computed"
        )
        return

    # 2. Something outside this tree points into it. The incident key: liveness
    #    can live entirely in a symlink elsewhere.
    if wt.inbound:
        names = ", ".join(str(p) for p in wt.inbound[:3])
        more = "" if len(wt.inbound) <= 3 else f" (+{len(wt.inbound) - 3} more)"
        wt.verdict = "KEEP"
        wt.reason = f"LINK TARGET — {len(wt.inbound)} live reference(s) point into it: {names}{more}"
        return

    # 3. A dispatch has claimed it. Honoured even when it looks stale.
    if wt.lease:
        holder = wt.lease.get("holder", "?")
        stale = " (lease window has elapsed — REPORTED, not resolved)" if lease_is_expired(wt.lease) else ""
        wt.verdict = "KEEP"
        wt.reason = f"LEASED by {holder}{stale}"
        return

    # 4. Self and git's own lock.
    if wt.path in self_paths:
        wt.verdict = "KEEP"
        wt.reason = "this program is running in it"
        return
    if wt.locked:
        wt.verdict = "KEEP"
        wt.reason = "locked with `git worktree lock`"
        return

    # 5. The external authority.
    if wt.branch is None:
        wt.verdict = "KEEP"
        wt.reason = (
            "DETACHED — no branch, so the remote holds no pull-request state about it. "
            "A pinned-content read tree and a spent verification tree are indistinguishable "
            "by every local signal. Nothing points into it and it holds nothing unpushed, so "
            "once its measurement has been reported it is reclaimed by naming it: "
            f"--tree {wt.path} --apply"
        )
        return
    if prs is None:
        wt.verdict = "KEEP"
        wt.reason = f"NO PR AUTHORITY — {pr_error}; absence of the authority is not permission"
        return
    pr = prs.get(wt.branch)
    wt.pr = pr
    if pr is None:
        wt.verdict = "KEEP"
        wt.reason = "no pull request on the remote for this branch — the work has not landed"
        return
    state = pr.get("state", "?")
    if state not in {"MERGED", "CLOSED"}:
        wt.verdict = "KEEP"
        wt.reason = f"pull request #{pr.get('number')} is {state}"
        return

    wt.verdict = "RECLAIM"
    wt.reason = (
        f"pull request #{pr.get('number')} is {state} on the remote; clean, fully pushed, "
        "unreferenced, unleased"
    )


# ---------------------------------------------------------------------------
# disk
# ---------------------------------------------------------------------------


def free_gib(path: Path) -> float:
    try:
        return shutil.disk_usage(str(path)).free / (1024**3)
    except OSError:
        return float("inf")


def dir_kib(path: Path) -> int:
    code, out, _ = run(["du", "-sk", str(path)], timeout=120)
    if code != 0:
        return 0
    first = out.split("\t", 1)[0].strip()
    return int(first) if first.isdigit() else 0


def is_cargo_target(path: Path) -> bool:
    """A directory named `target` that cargo demonstrably owns.

    `CACHEDIR.TAG` is written by cargo into every target directory and its first
    line is a fixed signature. Requiring it means this tool cannot mistake a
    source directory that happens to be called `target` for build output.
    """
    if path.name != "target" or not path.is_dir() or path.is_symlink():
        return False
    tag = path / "CACHEDIR.TAG"
    if not tag.is_file():
        return False
    try:
        return tag.read_text(encoding="utf-8", errors="replace").startswith(
            "Signature: 8a477f597d28d172789f06886806bc55"
        )
    except OSError:
        return False


def find_targets(roots: list[Path], depth: int = SCAN_DEPTH) -> list[Path]:
    found: list[Path] = []
    seen: set[Path] = set()
    for root in roots:
        root = real(root)
        if root in seen or not root.is_dir():
            continue
        seen.add(root)
        base = len(root.parts)
        for dirpath, dirnames, _ in os.walk(root, followlinks=False):
            here = Path(dirpath)
            if len(here.parts) - base >= depth:
                dirnames[:] = []
                continue
            for d in list(dirnames):
                p = here / d
                if is_cargo_target(p):
                    found.append(p)
            dirnames[:] = [
                d for d in dirnames if d not in PRUNE_DIRS and not (here / d).is_symlink()
            ]
    return found


# ---------------------------------------------------------------------------
# the sweep
# ---------------------------------------------------------------------------


def resolve_content(engine: Path) -> Path | None:
    link = engine / "campaigns"
    if not link.exists():
        return None
    target = real(link)
    code, out, _ = git(target, "rev-parse", "--show-toplevel")
    if code != 0:
        return None
    return real(out.strip())


def harness_branches(repo: Path, prefix: str = "worktree-agent-") -> list[str]:
    """The harness's own throwaway branches, already contained in origin/main.

    Not worktrees — local refs left behind by the agent harness. Listed with
    `--merged origin/main` so the containment is git's answer, not a guess, and
    filtered in python rather than with `git grep -E`, whose build here does not
    support `\\b`.
    """
    code, out, _ = git(repo, "branch", "--format=%(refname:short)", "--merged", "origin/main")
    if code != 0:
        return []
    checked_out: set[str] = set()
    code2, out2, _ = git(repo, "worktree", "list", "--porcelain")
    if code2 == 0:
        for line in out2.splitlines():
            if line.startswith("branch refs/heads/"):
                checked_out.add(line[len("branch refs/heads/") :])
    return [
        b.strip()
        for b in out.splitlines()
        if b.strip().startswith(prefix) and b.strip() not in checked_out
    ]


class RepoSweep:
    def __init__(self, name: str, path: Path):
        self.name = name
        self.path = path
        self.main: Path | None = None
        self.trees: list[Worktree] = []
        self.error: str | None = None
        self.pr_error = ""
        self.prs: dict[str, dict] | None = None
        self.stale_branches: list[str] = []


def sweep(
    repos: list[tuple[str, Path]],
    *,
    extra_scan: list[Path],
    gh_timeout: int,
) -> tuple[list[RepoSweep], LinkIndex]:
    sweeps = [RepoSweep(name, path) for name, path in repos]

    for rs in sweeps:
        rs.trees, rs.main, rs.error = enumerate_worktrees(rs.path)
        for wt in rs.trees:
            measure(wt)

    # The reverse-reference index spans every scratch area any worktree lives
    # in, plus the checkouts themselves — a link in one repository's worktree
    # routinely points into the other's.
    roots: list[Path] = [p for _, p in repos]
    for rs in sweeps:
        for wt in rs.trees:
            if wt.exists:
                roots.append(wt.path.parent)
    roots.extend(extra_scan)
    index = LinkIndex()
    index.scan(roots)

    for rs in sweeps:
        for wt in rs.trees:
            wt.inbound = index.inbound(wt.path)

    self_paths = {real(REPO), real(Path.cwd())}

    for rs in sweeps:
        slug = repo_slug(rs.path)
        if slug is None:
            rs.pr_error = "no origin remote, so no pull-request authority exists"
        else:
            rs.prs, rs.pr_error = pull_requests(slug, timeout=gh_timeout)
        for wt in rs.trees:
            decide(wt, self_paths=self_paths, prs=rs.prs, pr_error=rs.pr_error)
        rs.stale_branches = harness_branches(rs.path)

    return sweeps, index


# ---------------------------------------------------------------------------
# actions
# ---------------------------------------------------------------------------


def reclaim(rs: RepoSweep, wt: Worktree, out) -> bool:
    # `--force` is deliberately absent: git re-checks dirtiness itself, which
    # makes the refusal independent of every measurement above.
    code, _, err = git(rs.path, "worktree", "remove", str(wt.path))
    if code != 0:
        print(f"  FAILED  {wt.path} — git refused: {err.strip()}", file=out)
        return False
    print(f"  removed {wt.path}  [{wt.label}]", file=out)
    if wt.branch:
        code, _, err = git(rs.path, "branch", "-D", wt.branch)
        if code == 0:
            print(f"          local branch {wt.branch} deleted (every commit is on a remote)", file=out)
        else:
            print(f"          local branch {wt.branch} kept — {err.strip()}", file=out)
    return True


def render(
    sweeps: list[RepoSweep],
    index: LinkIndex,
    *,
    apply: bool,
    pressure: bool,
    free: float,
    threshold: float,
    targets: list[tuple[Path, int]],
    out,
) -> int:
    examined = sum(len(rs.trees) for rs in sweeps)
    reclaimed = 0
    kept = 0

    print("== worktree reclamation", file=out)
    print(
        f"  mode: {'APPLY (destructive)' if apply else 'dry run (default — nothing is deleted)'}"
        f"   free space: {free:.1f} GiB"
        f"{'  — BELOW ' + str(threshold) + ' GiB, disk-pressure mode' if pressure else ''}",
        file=out,
    )

    for rs in sweeps:
        print(f"\n  {rs.name}: {rs.main or rs.path}", file=out)
        if rs.error:
            print(f"    COULD NOT ENUMERATE — {rs.error}", file=out)
            continue
        if rs.pr_error:
            print(f"    pull-request authority: UNAVAILABLE — {rs.pr_error}", file=out)
        elif rs.prs is not None:
            print(f"    pull-request authority: {len(rs.prs)} branch(es) known to the remote", file=out)
        if not rs.trees:
            print("    no worktrees beyond the main checkout", file=out)
        for wt in rs.trees:
            print(f"    {wt.verdict:<7} {wt.path}", file=out)
            print(f"            {wt.label} — {wt.reason}", file=out)
            if wt.verdict in {"RECLAIM", "PRUNE"}:
                reclaimed += 1
            else:
                kept += 1
        if rs.stale_branches:
            print(
                f"    harness throwaway branches contained in origin/main: "
                f"{len(rs.stale_branches)} ({', '.join(rs.stale_branches[:5])})",
                file=out,
            )

    if index.benign_broken:
        print(
            f"\n  {index.benign_broken} dangling link(s) that are supposed to dangle "
            "(browser sentinels) — counted, not listed",
            file=out,
        )
    if index.broken:
        print(f"\n  FINDING — {len(index.broken)} DANGLING SYMLINK(S):", file=out)
        for link, raw in index.broken:
            print(f"    {link} -> {raw}  (target does not exist)", file=out)
        print(
            "    A worker reading through a dangling link does not crash; it measures\n"
            "    ZERO. Repoint or remove each one before trusting any measurement taken\n"
            "    through it.",
            file=out,
        )

    if targets:
        total = sum(k for _, k in targets) / (1024**2)
        print(f"\n  cargo target/ directories outside live trees: {len(targets)}, {total:.1f} GiB", file=out)
        for p, k in targets:
            print(f"    {k / (1024**2):.1f} GiB  {p}", file=out)
        if not pressure:
            print(
                f"    Rebuildable output. Not touched: free space is above {threshold} GiB.",
                file=out,
            )

    print(
        f"\n  binding: {len(sweeps)} repositor(y|ies) enumerated, {examined} worktree(s) examined, "
        f"{index.examined} symlink(s) resolved across {len(index.roots)} scan root(s), "
        f"{reclaimed} reclaimable, {kept} kept",
        file=out,
    )
    if examined == 0:
        print(
            "  BINDING ZERO — no worktree was examined, so this run proves nothing.\n"
            "  Either no dispatch has a tree, or the enumeration ran against the wrong\n"
            "  repository, which `git worktree list` answers confidently and wrongly.",
            file=out,
        )
    if index.examined == 0:
        print(
            "  BINDING ZERO — the reverse-reference scan resolved no symlink, so the\n"
            "  'nothing points into this tree' key bound to nothing on this run.",
            file=out,
        )
    return reclaimed


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description="Reclaim worktrees whose work has landed. Dry run unless --apply.",
    )
    ap.add_argument("--apply", action="store_true", help="actually delete (default: report only)")
    ap.add_argument(
        "--after-merge",
        metavar="BRANCH",
        help="the merge-time entry point: reclaim the tree holding BRANCH, "
        "under exactly the same proof as a sweep",
    )
    ap.add_argument(
        "--tree",
        metavar="PATH",
        help="reclaim only this tree, and accept the operator naming it as the authority "
        "a detached checkout has no pull request to supply. Every other key still applies.",
    )
    ap.add_argument("--lease", metavar="PATH", help="claim a tree so no sweep evaluates it")
    ap.add_argument("--release", metavar="PATH", help="drop the lease on a tree")
    ap.add_argument("--holder", default=os.environ.get("USER", "dispatch"), help="lease holder")
    ap.add_argument("--hours", type=int, default=24, help="lease window, reported when elapsed, never enforced")
    ap.add_argument("--reason", default="", help="why this tree is claimed")
    ap.add_argument("--scan-dir", action="append", default=[], metavar="DIR",
                    help="extra directory to include in the reverse-reference and target scans")
    ap.add_argument("--free-below", type=float, default=DEFAULT_PRESSURE_GIB, metavar="GIB",
                    help=f"disk-pressure threshold in GiB (default {DEFAULT_PRESSURE_GIB})")
    ap.add_argument("--targets-only", action="store_true",
                    help="do not touch worktrees; only rebuildable cargo target/ output")
    ap.add_argument("--repo", action="append", default=[], metavar="PATH",
                    help="repository to sweep (default: this engine checkout and the content checkout)")
    ap.add_argument("--gh-timeout", type=int, default=45)
    args = ap.parse_args(argv)

    out = sys.stdout

    if args.lease:
        p = real(args.lease)
        where = write_lease(p, args.holder, args.hours, args.reason)
        print(f"leased {p} for {args.holder} ({args.hours}h window) — {where}")
        print("A lease is honoured even after its window elapses; it is reported, never voided.")
        return 0
    if args.release:
        p = real(args.release)
        d = admin_dir(p)
        f = (d / LEASE_FILE) if d else None
        if f and f.exists():
            f.unlink()
            print(f"lease released on {p}")
        else:
            print(f"no lease on {p}")
        return 0

    repos: list[tuple[str, Path]] = []
    if args.repo:
        for r in args.repo:
            repos.append((Path(r).name, real(r)))
    else:
        repos.append(("engine ", real(REPO)))
        content = resolve_content(real(REPO))
        if content:
            repos.append(("content", content))

    sweeps, index = sweep(
        repos,
        extra_scan=[real(d) for d in args.scan_dir],
        gh_timeout=args.gh_timeout,
    )

    free = free_gib(repos[0][1] if repos else Path.cwd())
    pressure = free < args.free_below

    # `target/` directories are only ever considered inside trees this run is
    # not protecting: never the tree running this program, never a leased tree,
    # never one something links into. Rebuildable output is cheap to lose, but
    # not free — it costs a live worker a rebuild.
    protected: list[Path] = []
    for rs in sweeps:
        for wt in rs.trees:
            if wt.verdict != "RECLAIM" and (wt.lease or wt.inbound or wt.path in {real(REPO), real(Path.cwd())}):
                protected.append(wt.path)
    target_roots = [p for _, p in repos] + [
        wt.path.parent for rs in sweeps for wt in rs.trees if wt.exists
    ] + [real(d) for d in args.scan_dir]
    targets: list[tuple[Path, int]] = []
    if pressure or args.targets_only:
        for t in find_targets(target_roots):
            if any(is_within(t, p) for p in protected):
                continue
            targets.append((t, dir_kib(t)))
        targets.sort(key=lambda kv: -kv[1])

    # --tree / --after-merge narrow the sweep to one tree. The proof is
    # unchanged; --tree additionally accepts the operator's naming in place of
    # the pull-request state a detached checkout cannot have.
    selected: list[tuple[RepoSweep, Worktree]] = []
    named = None
    if args.tree:
        named = real(args.tree)
    for rs in sweeps:
        for wt in rs.trees:
            if args.after_merge and wt.branch == args.after_merge:
                selected.append((rs, wt))
            elif named is not None and wt.path == named:
                if wt.verdict == "KEEP" and wt.branch is None and wt.reason.startswith("DETACHED"):
                    wt.verdict = "RECLAIM"
                    wt.reason = (
                        "named explicitly by an operator, which is the authority a detached "
                        "checkout has no pull request to supply — OVERRIDE: clean, fully pushed, "
                        "unreferenced and unleased were all still required and all hold"
                    )
                selected.append((rs, wt))

    if args.after_merge or args.tree:
        if not selected:
            print(f"no worktree matches {args.after_merge or args.tree} in any swept repository")
            print(
                "Nothing was deleted, and this exits non-zero on purpose: a name that matched\n"
                "nothing is a finding when a tree was expected there. It is also the ordinary\n"
                "answer when the sweep bound to the session hook already reclaimed it — check\n"
                "the last page before treating it as a problem."
            )
            return 1
        for rs, wt in selected:
            print(f"{wt.verdict}  {wt.path}\n        {wt.label} — {wt.reason}")
        if args.apply and not args.targets_only:
            for rs, wt in selected:
                if wt.verdict in {"RECLAIM", "PRUNE"}:
                    reclaim(rs, wt, out)
                    git(rs.path, "worktree", "prune")
        elif not args.apply:
            print("dry run — nothing deleted. Re-run with --apply.")
        return 0

    render(
        sweeps,
        index,
        apply=args.apply,
        pressure=pressure,
        free=free,
        threshold=args.free_below,
        targets=targets,
        out=out,
    )

    if not args.apply:
        print("\n  dry run — nothing was deleted. Re-run with --apply to act on the RECLAIM rows.")
        return 0

    if not args.targets_only:
        print("\n  applying:", file=out)
        acted = 0
        for rs in sweeps:
            for wt in rs.trees:
                if wt.verdict in {"RECLAIM", "PRUNE"}:
                    if reclaim(rs, wt, out):
                        acted += 1
            git(rs.path, "worktree", "prune")
        if acted == 0:
            print("    nothing was reclaimable this run", file=out)

    if targets and (pressure or args.targets_only):
        print("\n  disk pressure — removing rebuildable cargo output:", file=out)
        for p, k in targets:
            try:
                shutil.rmtree(p)
                print(f"    freed {k / (1024**2):.1f} GiB  {p}", file=out)
            except OSError as exc:
                print(f"    FAILED {p} — {exc}", file=out)

    # A deletion that broke a link must be visible in the run that caused it,
    # not hours later as a worker measuring zero.
    after = LinkIndex()
    after.scan(index.roots)
    if after.broken:
        print(f"\n  FINDING — {len(after.broken)} dangling symlink(s) after this run:", file=out)
        for link, raw in after.broken:
            print(f"    {link} -> {raw}", file=out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
