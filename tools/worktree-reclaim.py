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

That key is only worth what the QUESTION is worth, so the question is asked
about the branch being decided — one query per branch — rather than by fetching
a list and hoping the branch is on it. A bulk `gh pr list --limit N` returns the
N most recent requests and never says it stopped; two open requests on this
machine fell outside a 200-row window and were reported as "no pull request on
the remote for this branch", which is a fact about a fetch dressed as a fact
about the remote. See `Authority` for the full account.

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

## What ENDS a lease, and why it could not be a sweep

A claim nothing can release is not a lease; it is a permanent exemption. Every
tree `tools/worktree-new.sh` creates is claimed at dispatch, and the only
releaser was a `--release` flag nothing invoked — the script printed "release it
at the merge" and that sentence was the whole mechanism, which is the UNRUN
vacuity mode (`CLAUDE.md`: a doc line is not an invocation). Measured before the
repair: thirty-six leases, no releases, and eight of them over a branch the
remote had already merged. The top rung had swallowed every object, so nothing
below it — the reachability key, the pull-request authority, the whole argument
about what counts as permission — could be reached for any of those trees, or
for the forty-five `target/` directories they hold.

The event that ends a lease is the MERGE, so `--after-merge BRANCH` is where the
release is bound: the one entry point that already exists to be run in the same
breath as a merge. It is gated on the same un-forgeable key the reclaim rung
rests on — the remote's own MERGED or CLOSED verdict for that exact branch, not
the operator having named it, which only selects. A live dispatch cannot merge
its own pull request. Releasing the lease is not permission to delete: every
other rung is re-run afterwards and every one of them still outranks it.

The SWEEP still releases nothing, ever. It reports which leases are spent, states
how many it holds and how many of those the remote has landed, and prints the act
that ends each. A detached tree has no branch for the remote to hold a verdict
about, so the only thing that ends its lease is an operator's explicit
`--release`, which the narrowed run names rather than doing.

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

## Build output is a different question, and it used to be asked of the disk

A `target/` directory holds no work. It is a regenerable cache, and it says so
itself: `CACHEDIR.TAG`, which cargo writes and which this tool already requires
before touching anything, is the filesystem's standard marker for exactly that.
A directory holding work a deletion would destroy cannot produce one.

So the loss is bounded at "a worker pays for a rebuild", and the key that opens
it must be the matching question: is anything going to build here again? For a
long time the key was instead FREE DISK SPACE, which asks whether the machine is
about to fall over. It never opened — nineteen dead trees held a hundred
gibibytes with two hundred and fifty-eight free, and the accumulation was found
by a hand sweep. Lowering the threshold would only move the date.

`decide_target` replaces it. The un-forgeable key at the top is the kernel's:
cargo holds an exclusive `flock` on `<target>/<profile>/.cargo-lock` for the
whole of a build, and a running build cannot present its own lock as free. Below
that sit the same liveness keys the tree ladder uses, then the event that makes
output waste — the branch's pull request has LANDED — and last a stated idle
window for trees the remote holds no verdict on. Full argument, including which
arm of that disjunction is the weak one and why it is acceptable, is in
`decide_target`'s own docstring.

## Binding counts

Every run states how many repositories it enumerated, how many worktrees it
examined, how many symlinks the reverse-reference scan resolved, how many
`target/` directories it judged, how many trees it reclaimed and the reason for
each one it kept. A run that examined nothing is
a FINDING, not a pass: enumerating zero worktrees is what `git worktree list`
answers, confidently, when it is run against the wrong repository — which is why
nothing in this file ever changes directory, and every git call is `git -C`.

Deterministic, stdlib-only python3. Read-only unless `--apply` is given.
"""

from __future__ import annotations

import argparse
import fcntl
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

# Free-space floor, in gibibytes. This is an ALARM, not a gate: below it the run
# additionally lists the build output it is protecting, so an operator in real
# trouble can see what is being held and by which rung. It does not decide
# whether anything is reclaimed — see `decide_target` for why it never should
# have.
DEFAULT_PRESSURE_GIB = 25

# How long build output must have gone untouched before it is reclaimed on the
# weak arm of `decide_target`. Only reached by a tree whose work has NOT landed,
# and only after the kernel has said no build holds the directory's lock.
DEFAULT_TARGET_IDLE_HOURS = 72

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
        # The pull request, if the remote holds a TERMINAL one for this tree's
        # branch while a lease is still held over it. Set only by the lease rung,
        # and only from the remote's own answer: it is the fact that the event
        # this lease was taken against has already happened.
        self.lease_spent: dict | None = None
        # Why the remote could not be asked about this tree's branch, if it
        # could not. Kept apart from `lease_spent`: "not landed" and "not asked"
        # are different facts and a row must never print one as the other.
        self.lease_probe_error: str = ""
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


# The most requests one branch can plausibly have carried. An answer that
# reaches this is treated as possibly truncated and therefore as NO authority —
# see `Authority.for_branch`.
PER_BRANCH_LIMIT = 50

# A branch may carry several pull requests. OPEN dominates: an open request over
# a branch means work is still expected on it, whatever a superseded closed one
# says.
STATE_RANK = {"OPEN": 3, "MERGED": 2, "CLOSED": 1}


class Authority:
    """The remote's pull-request state, ASKED ABOUT THE BRANCH BEING DECIDED.

    One query per branch, never a bulk listing filtered locally. That is not a
    style preference — it is the difference between an answer about the remote
    and an answer about a fetch.

    The first version of this asked once per repository with
    `gh pr list --state all --limit 200` and looked branches up in the result.
    `gh` returns the most recent N and says nothing about having stopped, so two
    branches whose requests sat outside that window were reported as "no pull
    request on the remote for this branch" — a fact about a truncated fetch,
    stated as a fact about the remote, with a confident count printed beside it
    saying how many branches were "known". That is the UNTRAVERSED vacuity mode
    (CLAUDE.md): coverage that stops partway and reports as though it had
    covered everything. It was fail-safe only by accident of which way the
    missing rows happened to fall, and it made the tool structurally unable to
    reclaim anything older than one page — which is exactly the accumulation it
    exists to drain.

    THE PROPERTY THIS RESTORES: a branch's absence from an answer means the
    remote says there is no request for it, never that the query stopped early.
    Each per-branch answer also proves it is not itself truncated — an answer
    that fills `PER_BRANCH_LIMIT` is treated as no authority at all rather than
    as its first rows.

    Consulted lazily, only for a tree that has already passed every rung above
    it, so the cost is a query per branch actually being decided rather than
    one per worktree, and the answer is cached.
    """

    def __init__(self, slug: str | None, *, timeout: int):
        self.slug = slug
        self.timeout = timeout
        self.cache: dict[str, tuple[dict | None, str]] = {}
        self.queries = 0
        self.failures = 0

    def for_branch(self, branch: str) -> tuple[dict | None, str]:
        """(the decisive request for `branch`, error).

        `(None, "")` is the remote's own "there is no request for this branch".
        A non-empty error means the authority could not be established, and
        nothing is reclaimable on it — absence of the authority is never read
        as permission.
        """
        if self.slug is None:
            return None, "no origin remote, so no pull-request authority exists"
        if branch in self.cache:
            return self.cache[branch]

        self.queries += 1
        code, out, err = run(
            [
                "gh",
                "pr",
                "list",
                "-R",
                self.slug,
                "--head",
                branch,
                "--state",
                "all",
                "--limit",
                str(PER_BRANCH_LIMIT),
                "--json",
                "number,state,headRefName,title",
            ],
            timeout=self.timeout,
        )
        answer: tuple[dict | None, str]
        if code != 0:
            self.failures += 1
            answer = (None, (err.strip().splitlines() or ["gh failed"])[0])
        else:
            try:
                rows = json.loads(out or "[]")
            except json.JSONDecodeError as exc:
                self.failures += 1
                answer = (None, f"gh returned unparseable JSON ({exc})")
            else:
                if len(rows) >= PER_BRANCH_LIMIT:
                    # The answer reached the limit, so it may have been cut off
                    # at it. Fail closed rather than decide on its first rows.
                    self.failures += 1
                    answer = (
                        None,
                        f"the answer for this branch filled the {PER_BRANCH_LIMIT}-row limit, "
                        "so it may be truncated and is not treated as authoritative",
                    )
                else:
                    # `--head` is matched by the remote, but a stray row would
                    # decide the wrong branch, so it is re-checked here.
                    mine = [r for r in rows if (r.get("headRefName") or "") == branch]
                    best = None
                    for row in mine:
                        if best is None or STATE_RANK.get(row.get("state", ""), 0) > STATE_RANK.get(
                            best.get("state", ""), 0
                        ):
                            best = row
                    answer = (best, "")
        self.cache[branch] = answer
        return answer


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


def drop_lease(wt_path: Path) -> bool:
    """Delete the lease file over `wt_path`. True if one was there."""
    d = admin_dir(wt_path)
    f = (d / LEASE_FILE) if d else None
    if f is None or not f.exists():
        return False
    f.unlink()
    return True


def probe_lease(wt: "Worktree", authority: Authority) -> None:
    """Has the event this lease was taken against already happened?

    Asked of EVERY lease, and that is the whole point of the function existing
    separately from the verdict ladder. "Is this lease spent" is a fact about the
    LEASE — about whether the remote has landed the branch it was taken for — and
    not a fact about which rung of the ladder happens to answer about the tree
    first. The first version of this asked at the lease rung, which is reached
    only by a tree that is clean, fully pushed and unreferenced; on this machine
    that reported **2** spent leases where an independent census of the same
    thirty-six counted **8**, because five of the eight sit over trees carrying
    commits on no remote and rung 1 answers about those before rung 3 is reached.

    That is a numerator computed over one population and printed against the
    denominator of another, which is worse than no count at all: it reads as
    coverage, it is honest about the trees it examined, and it understates in the
    direction that makes the backlog look smaller than it is. It was caught only
    because the figure disagreed with an independent observer — which is the one
    reason every count here states what it is a count OF.

    A tree with no branch is not asked about and is not an error: the remote holds
    no pull-request state about a detached checkout, so no merge can end its lease
    and only an operator's `--release` can.
    """
    if not wt.lease or wt.branch is None:
        return
    pr, pr_error = authority.for_branch(wt.branch)
    if pr_error:
        wt.lease_probe_error = pr_error
    elif pr is not None and pr.get("state") in {"MERGED", "CLOSED"}:
        wt.lease_spent = pr


def lease_spent_note(wt: "Worktree") -> str:
    """The clause that says a lease is being honoured over work that has landed.

    One authority for the wording, reached from three places — the tree row, the
    `target/` row, and the protected-path reason `main` builds before the target
    ladder runs. The target ladder is consulted through TWO paths (a protected
    path wins before `decide_target` ever looks at the tree), and a note added to
    only one of them is invisible in exactly the rows that hold the disk.
    """
    if not wt.lease_spent:
        return ""
    return (
        f" — SPENT (#{wt.lease_spent.get('number')} {wt.lease_spent.get('state')});"
        f" ends at --after-merge {wt.branch} --apply"
    )


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


def branch_has_no_work_beyond_base(
    path: Path, head: str, base_ref: str = "origin/main"
) -> tuple[bool, str]:
    """Is `head` already contained in `base_ref` — i.e. is there nothing to land?

    True exactly when the merge-base of the tree's own HEAD and `base_ref`
    IS that HEAD: every commit this tree carries is already on `base_ref`, so
    a branch cut from it and never advanced is not "work not yet proposed",
    it is no work at all. A branch that has advanced past its base can still
    lack a pull request — that is the ordinary in-flight case, and this
    function says False for it, leaving the existing "the work has not
    landed" refusal to hold it.

    Reached only after the tree has already passed dirty, unpushed, inbound,
    lease and self/lock — so a HEAD equal to this merge-base is, by
    construction, also on some remote (`git rev-list HEAD --not --remotes`
    already read 0), whether or not this exact branch's own remote ref still
    exists. "Fully pushed" and "unpushed-but-empty" are the same fact from
    this rung's point of view.
    """
    code, out, err = git(path, "merge-base", head, base_ref)
    if code != 0:
        return False, f"merge-base against {base_ref} could not be computed ({err.strip() or 'git failed'})"
    merge_base = out.strip()
    if merge_base == head:
        return True, f"merge-base(HEAD, {base_ref}) == {head[:8]} == the tree's own HEAD"
    return (
        False,
        f"merge-base(HEAD, {base_ref}) == {merge_base[:8]}, not HEAD ({head[:8]}) — "
        "commits beyond base",
    )


def decide(wt: Worktree, *, self_paths: set[Path], authority: Authority) -> None:
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

    # 3. A dispatch has claimed it. Honoured even when it looks stale — but
    #    honouring it is not the same as saying nothing about it.
    #
    #    This rung used to return here without ever asking anything, and that is
    #    what made the top of the ladder opaque as well as absorbing. Both this
    #    file's own header and `docs/reference/tools.md` promised that "a lease
    #    over a merged branch is reported, never silently resolved"; the code
    #    asked no such question, and the test named for the property asserted
    #    only that the row said LEASED. So every one of thirty-six leases on this
    #    machine printed one indistinguishable line, and an operator could not
    #    tell the live claim from the spent one. A rung that swallows every
    #    object and then declines to say which of them it is protecting from
    #    what is a gate reporting a binding count it has not established.
    #
    #    So the remote IS asked, about this tree's own branch, and the answer is
    #    REPORTED. The verdict does not move: a lease is honoured, and the act
    #    that ends one is the merge (`--after-merge`), never a sweep's reading of
    #    how finished something looks. What changes is that the report now names
    #    which leases are spent and what ends them.
    #
    #    `probe_lease` has already asked, for EVERY lease rather than for the
    #    ones that happen to arrive here — see its docstring for why the
    #    difference is the whole value of the count.
    if wt.lease:
        holder = wt.lease.get("holder", "?")
        notes: list[str] = []
        if lease_is_expired(wt.lease):
            notes.append("lease window has elapsed")
        if wt.lease_probe_error:
            # Never printed as "not landed": that would be a claim about the
            # remote made out of a question the remote never answered.
            notes.append(f"the remote could not be asked about its branch ({wt.lease_probe_error})")
        elif wt.lease_spent:
            pr = wt.lease_spent
            wt.pr = pr
            notes.append(
                f"SPENT — the remote says pull request #{pr.get('number')} is "
                f"{pr.get('state')}, so the event this lease was taken against has "
                f"happened; end it at the merge: --after-merge {wt.branch} --apply"
            )
        wt.verdict = "KEEP"
        wt.reason = f"LEASED by {holder}"
        if notes:
            wt.reason += " (" + "; ".join(notes) + " — REPORTED, not resolved)"
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
    pr, pr_error = authority.for_branch(wt.branch)
    wt.pr = pr
    if pr_error:
        wt.verdict = "KEEP"
        wt.reason = f"NO PR AUTHORITY — {pr_error}; absence of the authority is not permission"
        return
    if pr is None:
        no_work, proof = branch_has_no_work_beyond_base(wt.path, wt.head)
        if no_work:
            wt.verdict = "RECLAIM"
            wt.reason = (
                "no pull request on the remote for this branch, and there is no work to "
                f"land — {proof}; clean, fully pushed or unpushed-but-empty, unreferenced, "
                "unleased"
            )
            return
        wt.verdict = "KEEP"
        wt.reason = (
            "the remote, asked about this branch, holds no pull request for it — "
            "the work has not landed"
        )
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


def build_in_flight(target: Path) -> tuple[bool, str]:
    """Is a cargo build running in this target directory RIGHT NOW?

    Cargo holds an exclusive `flock` on `<target>/<profile>/.cargo-lock` for the
    duration of a build. That lock is held by the KERNEL on behalf of a live
    process, which is what makes it the right key here and what distinguishes
    this from the beliefs the rest of this file refuses: quiet can be faked by a
    live agent between two tool calls, and an mtime can be faked by anything,
    but a running build cannot present its own lock as free.

    A lock that cannot be opened or tested is reported as HELD. Absence of the
    answer is never permission — the same rule the pull-request authority obeys
    one layer up.
    """
    candidates = sorted(target.glob("*/.cargo-lock"))
    root_lock = target / ".cargo-lock"
    if root_lock.is_file():
        candidates.append(root_lock)
    for lock in candidates:
        try:
            fd = os.open(str(lock), os.O_RDWR)
        except OSError as exc:
            return True, f"the build lock {lock} could not be opened ({exc}) — treated as HELD"
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            fcntl.flock(fd, fcntl.LOCK_UN)
        except OSError:
            return True, f"a cargo build holds {lock}"
        finally:
            os.close(fd)
    return False, ""


def build_idle_hours(path: Path, window_hours: float) -> float | None:
    """Hours since anything under `path` was last modified. `None` if unknowable.

    The walk EXITS at the first entry newer than the window's cutoff, so a live
    target answers in milliseconds and only a genuinely cold one pays for the
    full traversal. The consequence, stated because it would otherwise be a
    silently wrong number: when the answer is below the window it is the age of
    the first recent entry found, not of the newest one. That is enough to
    decide the verdict and is not enough to print as "last built"; the caller
    words it accordingly.
    """
    now = time.time()
    cutoff = now - window_hours * 3600
    newest = 0.0
    saw_anything = False
    try:
        for dirpath, dirnames, filenames in os.walk(path, followlinks=False):
            for name in dirnames + filenames:
                try:
                    st = os.lstat(os.path.join(dirpath, name))
                except OSError:
                    continue
                saw_anything = True
                if st.st_mtime > cutoff:
                    return max(0.0, (now - st.st_mtime) / 3600)
                newest = max(newest, st.st_mtime)
    except OSError:
        return None
    if not saw_anything:
        return None
    return max(0.0, (now - newest) / 3600)


class TargetDir:
    """One cargo `target/` directory and the verdict on its build output."""

    def __init__(self, path: Path, tree: Worktree | None):
        self.path = path
        self.tree = tree
        self.size_kib = 0
        self.idle_hours: float | None = None
        self.verdict = "KEEP"
        self.reason = "not evaluated"


def decide_target(
    td: TargetDir,
    *,
    idle_window: float,
    protected: list[tuple[Path, str]],
    authority: Authority | None,
) -> None:
    """When may rebuildable build output be deleted?

    ## Why this is not the worktree ladder, and not the disk either

    The worktree ladder above protects work that a deletion would destroy
    forever. `target/` holds none: it is, by the definition of the marker this
    tool already requires before touching anything, a REGENERABLE CACHE.
    `CACHEDIR.TAG` is written by cargo and says exactly that, and a directory
    holding real work cannot produce one. So the loss here is bounded at "a
    worker pays for a rebuild", never "a worker loses what it wrote".

    Because the loss is different in kind, the key must be different in kind
    too — and the key it USED to have was free disk space. That is the defect.
    A capacity threshold answers "is the machine about to fall over", which is
    not a fact about whether this output is waste; nineteen dead trees sat on
    a hundred gibibytes with two hundred and fifty-eight free, so the valve
    never opened, and the accumulation was found by hand. Lowering the number
    would only move the day it happens again.

    The event that makes build output waste is that NOTHING IS GOING TO BUILD
    HERE AGAIN, so that is what each rung asks about.

    ## The rungs

      1. A build is in flight. The kernel says so, and a live build cannot say
         otherwise. Absolute.
      2. The tree is leased, something links into it, or it is this program's
         own — the same three liveness keys as the ladder above, unchanged.
      3. LANDED: the remote holds a MERGED or CLOSED pull request for the
         branch. No threshold, no timer: the work is on the remote and this
         output rebuilds from it. This is the rung the disk gate should always
         have been.
      4. IDLE: nothing under it has been touched for `idle_window` hours.

    ## The disjunction, named because CLAUDE.md requires it to be

    Rungs 3 and 4 are alternatives, so the effective obligation is their
    disjunction and is only as strong as rung 4, the weaker. Two things keep
    that honest. Which arm applies is decided BY THE OBJECT — whether the
    remote holds a terminal pull-request state for this branch — and never
    chosen by whoever runs the tool. And rung 4 is reached only after the
    kernel has already said no build holds the lock, so the case it can get
    wrong is "an agent that has not compiled for three days and never took a
    lease", whose whole cost is one rebuild that this tool names in its output.

    A tree that reaches neither rung is KEPT, with the reason said out loud.
    """
    # The reason travels with the path. A protected row that says only "this run
    # is protecting it" tells a reader nothing about WHICH key held it, and a
    # gate a creator cannot read has met half its obligation.
    for path, why in protected:
        if is_within(td.path, path):
            td.verdict = "KEEP"
            td.reason = why
            return

    held, why = build_in_flight(td.path)
    if held:
        td.verdict = "KEEP"
        td.reason = f"BUILD IN FLIGHT — {why}"
        return

    if td.tree is not None:
        if td.tree.lease:
            td.verdict = "KEEP"
            td.reason = f"LEASED by {td.tree.lease.get('holder', '?')}" + lease_spent_note(td.tree)
            return
        if td.tree.inbound:
            td.verdict = "KEEP"
            td.reason = f"LINK TARGET — {len(td.tree.inbound)} live reference(s) point into its tree"
            return

    td.idle_hours = build_idle_hours(td.path, idle_window)

    # Rung 3 is asked only when rung 4 has not already answered, so a cold tree
    # costs no network round trip. The reason printed still names the rung that
    # decided it.
    if td.idle_hours is not None and td.idle_hours >= idle_window:
        td.verdict = "RECLAIM"
        td.reason = f"IDLE — nothing under it modified for {td.idle_hours:.0f}h (window {idle_window:.0f}h), and no build holds its lock"
        return

    # What the remote said, kept apart from what it could not be asked. The two
    # are different facts and the row must not print one as the other: "the work
    # has not landed" is a claim about the remote, and it is unearned whenever
    # there was no branch to ask about or the query failed.
    landing = "the work has not landed"
    if td.tree is None or not td.tree.branch:
        landing = "no branch, so the remote holds no verdict about it"
    elif authority is None:
        landing = "no pull-request authority was available for its repository"
    else:
        pr, pr_error = authority.for_branch(td.tree.branch)
        if pr_error:
            landing = f"the remote could not be asked ({pr_error})"
        elif pr is not None and pr.get("state") in {"MERGED", "CLOSED"}:
            td.verdict = "RECLAIM"
            td.reason = (
                f"LANDED — pull request #{pr.get('number')} is {pr.get('state')} on the remote, "
                "so this output rebuilds from what the remote already holds"
            )
            return
        elif pr is not None:
            landing = f"pull request #{pr.get('number')} is {pr.get('state')}"

    if td.idle_hours is None:
        td.verdict = "KEEP"
        td.reason = "its idle time could not be measured, and absence of the answer is not permission"
        return
    td.verdict = "KEEP"
    td.reason = (
        f"still live — a file modified {td.idle_hours:.1f}h ago is inside the "
        f"{idle_window:.0f}h window, and {landing}"
    )


def find_targets(roots: list[Path], depth: int = SCAN_DEPTH) -> list[Path]:
    """Every cargo target directory under `roots`, each named ONCE.

    The roots OVERLAP by construction — the scan is given each repository plus
    the parent of every worktree, and a worktree's parent is routinely inside
    another root. Deduplicating the ROOTS is not enough, because two distinct
    roots legitimately reach the same directory. Measured on the run that found
    it: 52 directories reported where there were 36, sixteen rows duplicated,
    one listed three times, and the reclaimable count inflated from 11 to 13.

    A count that double-counts is not a smaller error than a count that misses.
    It is the same error — a number that reads as coverage and is not — and it
    inflates in the direction that makes a sweep look more thorough than it was.
    """
    found: list[Path] = []
    found_set: set[Path] = set()
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
                    rp = real(p)
                    if rp not in found_set:
                        found_set.add(rp)
                        found.append(rp)
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
        self.authority: Authority | None = None
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
        rs.authority = Authority(repo_slug(rs.path), timeout=gh_timeout)
        # Every lease is probed BEFORE the ladder runs, so the spent count is a
        # count over leases rather than over "leases whose tree happened to reach
        # rung 3". The answers are cached, so a branch the ladder asks about
        # later costs nothing twice.
        for wt in rs.trees:
            probe_lease(wt, rs.authority)
        for wt in rs.trees:
            decide(wt, self_paths=self_paths, authority=rs.authority)
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
    tdirs: list[TargetDir],
    out,
) -> int:
    examined = sum(len(rs.trees) for rs in sweeps)
    reclaimed = 0
    kept = 0

    print("== worktree reclamation", file=out)
    print(
        f"  mode: {'APPLY (destructive)' if apply else 'dry run (default — nothing is deleted)'}"
        f"   free space: {free:.1f} GiB"
        f"{'  — BELOW ' + str(threshold) + ' GiB, ALARM (not a gate)' if pressure else ''}",
        file=out,
    )

    for rs in sweeps:
        print(f"\n  {rs.name}: {rs.main or rs.path}", file=out)
        if rs.error:
            print(f"    COULD NOT ENUMERATE — {rs.error}", file=out)
            continue
        # What the authority actually covered, never a number that merely reads
        # as coverage. It is asked one branch at a time, about the branch being
        # decided, so the only honest figure is how many branches were asked.
        if rs.authority is not None:
            a = rs.authority
            if a.slug is None:
                print("    pull-request authority: UNAVAILABLE — no origin remote", file=out)
            elif a.queries == 0:
                print(
                    "    pull-request authority: not consulted — no tree reached that rung",
                    file=out,
                )
            elif a.failures == a.queries:
                print(
                    f"    pull-request authority: UNAVAILABLE — all {a.queries} query(ies) failed, "
                    "so nothing in this repository is reclaimable",
                    file=out,
                )
            else:
                failed = f", {a.failures} unanswered" if a.failures else ""
                print(
                    f"    pull-request authority: {a.queries} branch(es) asked directly{failed} "
                    f"(one query per branch — a branch's absence from an answer is the remote's "
                    f"answer, not a page boundary)",
                    file=out,
                )
        if not rs.trees:
            print("    no worktrees beyond the main checkout", file=out)
        for wt in rs.trees:
            print(f"    {wt.verdict:<7} {wt.path}", file=out)
            print(f"            {wt.label} — {wt.reason}", file=out)
            if wt.verdict in {"RECLAIM", "PRUNE"}:
                reclaimed += 1
            else:
                kept += 1
        # The top rung states its own binding, computed from the trees rather
        # than written down beside them. Without it a reader sees N identical
        # `LEASED by …` rows and cannot tell how much of the ladder below has
        # been reached at all — which is how a rung that absorbed every object
        # on this machine went unnoticed while the tool underneath it was
        # correct in every reviewable way.
        leased = [wt for wt in rs.trees if wt.lease]
        spent = [wt for wt in leased if wt.lease_spent]
        if leased:
            print(
                f"    leases: {len(leased)} of {len(rs.trees)} tree(s) claimed; "
                f"{len(spent)} over a branch the remote says has LANDED",
                file=out,
            )
        if spent:
            print(
                f"    FINDING — {len(spent)} SPENT LEASE(S). Each is honoured here and ends\n"
                "    at the merge, which is the event it was taken against:",
                file=out,
            )
            for wt in spent:
                print(
                    f"      --after-merge {wt.branch} --apply"
                    f"   (#{wt.lease_spent.get('number')} "
                    f"{wt.lease_spent.get('state')}, held by {wt.lease.get('holder', '?')})",
                    file=out,
                )
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

    t_reclaim = [t for t in tdirs if t.verdict == "RECLAIM"]
    if tdirs:
        freeable = sum(t.size_kib for t in t_reclaim) / (1024**2)
        print(
            f"\n  cargo build output: {len(tdirs)} target/ director(y|ies) examined, "
            f"{len(t_reclaim)} reclaimable (~{freeable:.1f} GiB by du; df is the instrument "
            "that will say what was actually freed)",
            file=out,
        )
        for td in tdirs:
            size = f"{td.size_kib / (1024**2):>5.1f} GiB" if td.size_kib else "    (unsized)"
            print(f"    {td.verdict:<7} {size}  {td.path}", file=out)
            print(f"            {td.reason}", file=out)
        if not t_reclaim:
            print(
                "    BINDING ZERO — every target/ examined was held by a rung above the\n"
                "    reclaim one. That is a pass only if the reasons above are liveness\n"
                "    reasons; if they are all 'could not be measured', this run proved nothing.",
                file=out,
            )
    if pressure:
        print(
            f"\n  FREE SPACE BELOW {threshold} GiB. The rows above are listed with their sizes so\n"
            "  a tree being protected can be released deliberately — by ending its lease, or\n"
            "  by naming it with --tree. Free space has never decided whether output is waste\n"
            "  and does not decide it here.",
            file=out,
        )

    all_leased = [wt for rs in sweeps for wt in rs.trees if wt.lease]
    all_spent = [wt for wt in all_leased if wt.lease_spent]
    print(
        f"\n  binding: {len(sweeps)} repositor(y|ies) enumerated, {examined} worktree(s) examined, "
        f"{index.examined} symlink(s) resolved across {len(index.roots)} scan root(s), "
        f"{len(tdirs)} target/ director(y|ies) judged, "
        f"{len(all_leased)} lease(s) held ({len(all_spent)} spent), "
        f"{reclaimed} tree(s) reclaimable, {kept} kept, {len(t_reclaim)} target(s) reclaimable",
        file=out,
    )
    if all_spent:
        print(
            f"  {len(all_spent)} of {len(all_leased)} lease(s) sit over a branch the remote has\n"
            "  already landed. A sweep never resolves one — the merge does. Until each is\n"
            "  ended, the rungs below the lease are unreachable for that tree AND its build\n"
            "  output, so a zero above is a fact about the top rung, not about the ladder.",
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
                    help=f"free-space ALARM in GiB (default {DEFAULT_PRESSURE_GIB}). Widens the "
                         "report; it does not decide whether anything is reclaimed")
    ap.add_argument("--target-idle-hours", type=float, default=DEFAULT_TARGET_IDLE_HOURS,
                    metavar="H",
                    help=f"how long build output must be untouched before the weak arm of the "
                         f"target ladder reclaims it (default {DEFAULT_TARGET_IDLE_HOURS}). The "
                         "strong arm — the branch's pull request has landed — needs no window")
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
        if drop_lease(p):
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

    # `target/` directories are judged on EVERY run, because whether build output
    # is waste is a fact about the tree that produced it and never a fact about
    # how full the disk is. `decide_target` holds that argument in full.
    protected: list[tuple[Path, str]] = []
    for rs in sweeps:
        for wt in rs.trees:
            if wt.verdict == "RECLAIM":
                continue
            if wt.lease:
                held = f"LEASED by {wt.lease.get('holder', '?')}" + lease_spent_note(wt)
                protected.append((wt.path, held))
                # A dispatch is given a tree AND a scratch directory beside it,
                # and a live worker's scratch is never touched — that rule is
                # older than this file. The target scan reaches a worker's whole
                # area (it walks each tree's PARENT), so a build output sitting
                # in scratch would otherwise be judged with no lease to find,
                # because it belongs to no worktree. The lease covers both.
                sibling = wt.path.parent / "scratch"
                if sibling.is_dir():
                    protected.append((sibling, f"{held} — the scratch directory beside a claimed tree"))
            elif wt.inbound:
                protected.append((
                    wt.path,
                    f"LINK TARGET — {len(wt.inbound)} live reference(s) point into its tree",
                ))
            elif wt.path in {real(REPO), real(Path.cwd())}:
                protected.append((wt.path, "this program is running in its tree"))
    # The main checkout of each repository is protected too, and for a reason
    # this tool did not previously have: it is the DONOR a new worktree clones
    # its build output from (`tools/worktree-new.sh`). Deleting it does not free
    # the blocks a clone shares with it, and it makes the next dispatch pay for a
    # cold compile — the exact cost this whole round exists to remove.
    donor = "the main checkout, which is the donor tools/worktree-new.sh clones from"
    protected.extend((real(p), donor) for _, p in repos)
    protected.extend((real(rs.main), donor) for rs in sweeps if rs.main)
    target_roots = [p for _, p in repos] + [
        wt.path.parent for rs in sweeps for wt in rs.trees if wt.exists
    ] + [real(d) for d in args.scan_dir]

    tree_of: list[tuple[Path, Worktree, RepoSweep]] = [
        (wt.path, wt, rs) for rs in sweeps for wt in rs.trees if wt.exists
    ]
    # `--after-merge` and `--tree` narrow to one tree and return before the
    # target ladder is ever consulted, so scanning for build output there is
    # work whose result is discarded — and it is not cheap work: it walks every
    # target directory on the machine and `du`s the reclaimable ones. A merge is
    # the one moment this tool runs while someone is waiting for it.
    narrowed = bool(args.after_merge or args.tree)
    tdirs: list[TargetDir] = []
    for t in ([] if narrowed else find_targets(target_roots)):
        owner = next(((wt, rs) for path, wt, rs in tree_of if is_within(t, path)), None)
        td = TargetDir(t, owner[0] if owner else None)
        decide_target(
            td,
            idle_window=args.target_idle_hours,
            protected=protected,
            authority=(owner[1].authority if owner else None),
        )
        tdirs.append(td)
    # `du` is slow and is only ever a REPORTING figure here — every space claim
    # this tool makes about what it freed comes from `df`. So it is spent on the
    # rows that will actually be acted on, plus, under the free-space alarm, on
    # the rows being protected, which is precisely what an operator in trouble
    # needs to see.
    for td in tdirs:
        if td.verdict == "RECLAIM" or pressure:
            td.size_kib = dir_kib(td.path)
    tdirs.sort(key=lambda t: (t.verdict != "RECLAIM", -t.size_kib, str(t.path)))

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
            # Naming which repositories were actually swept is the difference
            # between a refusal a reader can act on and one that only reports its
            # own confusion: this message cannot know the branch lives elsewhere,
            # but it can say exactly what it looked at, so a repository the sweep
            # never touched (a hand-rolled content clone that is not a worktree of
            # the shared checkout, e.g.) is visible as an omission rather than
            # read as "the branch does not exist anywhere".
            swept = ", ".join(f"{name.strip()} ({path})" for name, path in repos)
            print(f"swept: {swept}")
            print(
                "Nothing was deleted, and this exits non-zero on purpose: a name that matched\n"
                "nothing is a finding when a tree was expected there. It is also the ordinary\n"
                "answer when the sweep bound to the session hook already reclaimed it — check\n"
                "the last page before treating it as a problem.\n"
                "If the branch lives in a checkout not listed above — a hand-rolled clone, a\n"
                "second content worktree, anything this sweep would not discover on its own —\n"
                "point at it explicitly with --repo <path>. This sweep never guesses at a\n"
                "checkout you did not name."
            )
            return 1

        # ------------------------------------------------------------------
        # THE MERGE IS THE EVENT THAT ENDS A LEASE.
        #
        # `tools/worktree-new.sh` takes a lease on every tree it creates and
        # prints "release it at the merge". Nothing invoked `--release`, so that
        # instruction was a doc line, which is the UNRUN vacuity mode: the top
        # rung of the ladder claimed every tree the dispatch script had ever
        # made, and the rungs below it — the ones carrying the whole argument
        # about what counts as permission to delete — could never be reached.
        # `--after-merge`, the entry point that exists precisely to be run in the
        # same breath as a merge, was itself blocked by the lease it had handed
        # out at dispatch. Thirty-six leases, zero releases, no release path.
        #
        # The key is NOT the operator having typed the branch name — that only
        # selects. It is the same un-forgeable key the reclaim rung already
        # rests on: the remote's own TERMINAL pull-request state for this exact
        # branch. `CLAUDE.md`'s sixth vacuity mode asks what an escape hatch
        # demands and whether the thing it excludes could supply it; a live
        # dispatch cannot merge its own pull request, so it cannot.
        #
        # What this deliberately does NOT do: the SWEEP never releases anything.
        # A lease sitting over a landed branch is reported by the sweep and
        # resolved only here, at the merge, by the party performing it. An
        # expiry that voided a lease would be "quiet means dead" wearing a
        # timestamp, and a sweep that resolved one on its own reading would be
        # the judgement that once deleted a running worker's directories.
        #
        # Releasing the lease is not permission to delete. Every other rung is
        # re-run afterwards and every one of them still outranks: dirty and
        # unpushed above all, then anything pointing into the tree.
        # ------------------------------------------------------------------
        self_paths = {real(REPO), real(Path.cwd())}
        if args.after_merge:
            for rs, wt in selected:
                if not wt.lease:
                    continue
                pr, pr_error = (
                    rs.authority.for_branch(args.after_merge)
                    if rs.authority is not None
                    else (None, "no pull-request authority exists for this repository")
                )
                if pr_error:
                    print(
                        f"LEASE KEPT  {wt.path}\n"
                        f"        the remote could not be asked about {args.after_merge} "
                        f"({pr_error}); absence of the authority is not permission"
                    )
                    continue
                if pr is None or pr.get("state") not in {"MERGED", "CLOSED"}:
                    said = (
                        "holds no pull request for it"
                        if pr is None
                        else f"says pull request #{pr.get('number')} is {pr.get('state')}"
                    )
                    print(
                        f"LEASE KEPT  {wt.path}\n"
                        f"        the remote {said}, so the event that ends this lease "
                        f"has not happened"
                    )
                    continue
                verb = "released" if args.apply else "WOULD BE released (dry run)"
                print(
                    f"LEASE {verb}  {wt.path}\n"
                    f"        held by {wt.lease.get('holder', '?')}; pull request "
                    f"#{pr.get('number')} is {pr.get('state')} on the remote, which is the "
                    f"event this lease was taken against. Every other key is re-checked below."
                )
                if args.apply:
                    drop_lease(wt.path)
                # Set aside in memory either way, so a dry run shows the verdict
                # the ladder actually reaches once the lease is gone rather than
                # the one it is standing in front of.
                wt.lease = None
                wt.lease_spent = None
                decide(wt, self_paths=self_paths, authority=rs.authority)

        for rs, wt in selected:
            print(f"{wt.verdict}  {wt.path}\n        {wt.label} — {wt.reason}")
            if wt.verdict == "KEEP" and wt.lease and wt.reason.startswith("LEASED"):
                # Only when the LEASE is the rung that answered. A tree kept for
                # being dirty is held by rung 1, and telling its operator that
                # the lease is what stands in the way would send them to release
                # a claim that is not the obstacle — a hint that names the wrong
                # cause is worse than no hint, because it is actionable.
                #
                # A narrowed run that does nothing and says nothing about why is
                # the silent no-op this tool is otherwise careful to refuse. A
                # detached tree cannot reach `--after-merge` at all: it has no
                # branch for the remote to hold a verdict about, so the only act
                # that ends its lease is an operator's explicit one.
                print(
                    f"        the lease is what is holding it. `--after-merge` ends a lease "
                    f"only on the remote's terminal verdict for its branch; to end this one "
                    f"deliberately: --release {wt.path}"
                )
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
        tdirs=tdirs,
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

    # A target inside a tree that was just removed went with it; reporting it as
    # a failed deletion would be a red about work that succeeded.
    doomed = [td for td in tdirs if td.verdict == "RECLAIM" and td.path.exists()]
    if doomed:
        # df before and after, because `du` cannot say what a deletion actually
        # gives back: it counts a cloned block once per file that names it. The
        # two figures DISAGREEING is a finding worth printing, not noise — a du
        # far above the df recovery means the output was sharing blocks with a
        # tree that still holds them.
        before = free_gib(doomed[0].path.parent)
        print("\n  removing rebuildable cargo output whose tree will not build again:", file=out)
        for td in doomed:
            try:
                shutil.rmtree(td.path)
                print(f"    removed {td.path}\n            {td.reason}", file=out)
            except OSError as exc:
                print(f"    FAILED {td.path} — {exc}", file=out)
        after = free_gib(doomed[0].path.parent)
        du_total = sum(td.size_kib for td in doomed) / (1024**2)
        print(
            f"    df recovered {after - before:.2f} GiB (du had said {du_total:.2f} GiB). "
            "df is the instrument;\n    a large gap means those blocks were shared with a tree that still holds them.",
            file=out,
        )

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
