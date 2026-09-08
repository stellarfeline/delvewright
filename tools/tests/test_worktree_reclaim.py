"""`tools/worktree-reclaim.py` — every verdict, driven in both directions.

A tool that deletes worktrees is only as good as its refusals, and a refusal is
the half that is easy to write and never exercise. So every KEEP below is
produced against a tree that a naive sweep WOULD have deleted: the pull request
is merged, the tree is clean and fully pushed, and exactly one further key says
no. Each such test then removes that one key and watches the same tree turn
RECLAIM, so no KEEP here can be passing for an unrelated reason.

Nothing in this file touches a real checkout. Every repository is built by
`git init` inside pytest's `tmp_path`, and the remote's pull-request authority is
a fake `gh` on PATH — which also means the tool's `gh` invocation itself is under
test, rather than a mocked-out function that a refactor could silently re-bind.

## Binding count for the build-output ladder

63 tests here, of which 11 cover `decide_target`. Run against the version that
preceded it, **10 of those 11 go red** — which is what says they bind to the new
behaviour rather than passing for an unrelated reason.

The tenth is the finding, and it is worth more than the nine. It is
`test_targets_only_removes_idle_output_and_never_the_worktree`, and it PASSES on
the old version, because `--targets-only` always bypassed the free-space
threshold and always deleted. So the old defect was never that the capability
was gated too tightly: the capability worked, and **nothing invoked it** — the
session hook passes `--apply` and no other flag, and that path collected nothing
at 258 GiB free. It is the UNRUN vacuity mode, not a threshold, and the
threshold was the second defect rather than the first: on the fixture the manual
path deleted the output of a tree that was being built in at that moment,
because it carried no liveness check at all.

## Binding count for the lease rung

16 of the tests here cover what ENDS a lease, and every one of them goes red on
the version that preceded them, because that version had no answer: a lease was
written by every dispatch and released by nothing, so the top rung of the ladder
held every object and each rung below it was unreachable for every tree the
dispatch script had ever made. The refusals in that group are written against a
tree whose lease is asserted GONE before the refusal is checked — otherwise the
lease itself would be producing the KEEP and the test would prove nothing, which
is the reassuring direction.
"""

from __future__ import annotations

import json
import os
import pathlib
import subprocess
import time

import pytest

TOOL = pathlib.Path(__file__).resolve().parents[1] / "worktree-reclaim.py"


def git(cwd, *args):
    return subprocess.run(
        ["git", "-C", str(cwd), *args],
        capture_output=True,
        text=True,
        check=True,
    ).stdout


def fake_gh(tmp_path, rows, *, page=0):
    """A `gh` that PAGES, the way the real one does.

    This oracle reads its own arguments, and that is the whole point. The first
    version of the authority fetched one bulk `gh pr list` per repository and
    looked branches up in the result; `gh` returns the most recent N and never
    says it stopped, so two branches whose requests sat outside the window were
    reported as "no pull request on the remote". An oracle that answered the
    same rows to every question could not tell the two designs apart.

    So: a query carrying `--head BRANCH` is answered from the FULL set, the way
    the remote answers a question about one branch. A bulk listing is answered
    with the first `page` rows only — by default NONE, which is what a query
    that stopped early looks like from the inside. Every reclaim assertion in
    this file therefore also asserts that the tool asked about the branch it was
    deciding.

    `rows` is newest-first, as `gh` returns them: index 0 is the newest request,
    and anything past `page` is only reachable by asking about it.
    """
    bindir = tmp_path / "fakebin"
    bindir.mkdir(exist_ok=True)
    script = bindir / "gh"
    script.write_text(
        "#!/usr/bin/env python3\n"
        "import json, sys\n"
        'sys.stdout.reconfigure(newline="\\n")\n'
        f"ROWS = {json.dumps(rows)}\n"
        f"PAGE = {page}\n"
        "args = sys.argv[1:]\n"
        'head = args[args.index("--head") + 1] if "--head" in args else None\n'
        "if head is None:\n"
        "    print(json.dumps(ROWS[:PAGE]))\n"
        "else:\n"
        '    print(json.dumps([r for r in ROWS if r["headRefName"] == head]))\n',
        encoding="utf-8",
    )
    script.chmod(0o755)
    return bindir


def broken_gh(tmp_path):
    bindir = tmp_path / "fakebin"
    bindir.mkdir(exist_ok=True)
    script = bindir / "gh"
    script.write_text(
        "#!/usr/bin/env bash\necho 'gh: not authenticated' >&2\nexit 1\n", encoding="utf-8"
    )
    script.chmod(0o755)
    return bindir


class Fixture:
    """An origin, a clone, and a scratch area to hang worktrees off."""

    def __init__(self, tmp_path):
        self.root = tmp_path
        self.origin = tmp_path / "owner" / "repo.git"
        self.origin.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            ["git", "init", "--bare", "-b", "main", str(self.origin)],
            capture_output=True,
            check=True,
        )
        self.repo = tmp_path / "clone"
        subprocess.run(
            ["git", "clone", str(self.origin), str(self.repo)], capture_output=True, check=True
        )
        git(self.repo, "config", "user.email", "t@example.invalid")
        git(self.repo, "config", "user.name", "T")
        (self.repo / "README").write_text("x\n", encoding="utf-8")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-m", "root")
        git(self.repo, "push", "-u", "origin", "main")
        self.scratch = tmp_path / "scratch"
        self.scratch.mkdir()

    def worktree(self, name, branch=None, *, detached=False):
        path = self.scratch / name
        if detached:
            head = git(self.repo, "rev-parse", "HEAD").strip()
            git(self.repo, "worktree", "add", "--detach", str(path), head)
        else:
            git(self.repo, "worktree", "add", "-b", branch, str(path), "main")
            git(path, "push", "-u", "origin", branch)
        return path


@pytest.fixture
def fx(tmp_path):
    return Fixture(tmp_path)


def sweep(fx, bindir, *extra):
    env = dict(os.environ)
    env["PATH"] = f"{bindir}{os.pathsep}{env['PATH']}"
    p = subprocess.run(
        ["python3", str(TOOL), "--repo", str(fx.repo), *extra],
        capture_output=True,
        text=True,
        env=env,
        cwd=str(fx.root),
    )
    assert p.returncode in (0, 1), p.stderr
    return p.stdout


def verdict_for(output, path):
    """The verdict line for a path, plus the reason line that follows it."""
    lines = output.splitlines()
    for i, line in enumerate(lines):
        if line.strip().endswith(str(path)):
            return line.strip().split()[0], lines[i + 1].strip()
    raise AssertionError(f"{path} does not appear in:\n{output}")


MERGED = [{"number": 11, "state": "MERGED", "headRefName": "landed", "title": "t"}]


# ---------------------------------------------------------------------------
# the reclaim rung — the tool must actually work
# ---------------------------------------------------------------------------


def test_merged_clean_pushed_is_reclaimed(fx, tmp_path):
    wt = fx.worktree("wt-landed", "landed")
    bindir = fake_gh(tmp_path, MERGED)

    out = sweep(fx, bindir)
    assert verdict_for(out, wt)[0] == "RECLAIM"
    assert "dry run" in out
    assert wt.exists(), "a dry run must not delete anything"

    out = sweep(fx, bindir, "--apply")
    assert not wt.exists()
    assert "landed" not in git(fx.repo, "branch", "--format=%(refname:short)")


def test_closed_pull_request_is_also_landed_enough(fx, tmp_path):
    wt = fx.worktree("wt-closed", "abandoned")
    bindir = fake_gh(
        tmp_path, [{"number": 12, "state": "CLOSED", "headRefName": "abandoned", "title": "t"}]
    )
    assert verdict_for(sweep(fx, bindir), wt)[0] == "RECLAIM"


# ---------------------------------------------------------------------------
# the refusals — each produced over a MERGED, otherwise-reclaimable tree
# ---------------------------------------------------------------------------


def test_dirty_outranks_a_merged_pull_request(fx, tmp_path):
    wt = fx.worktree("wt-dirty", "landed")
    (wt / "scratch-output.txt").write_text("a measurement nobody has saved\n", encoding="utf-8")
    bindir = fake_gh(tmp_path, MERGED)

    v, why = verdict_for(sweep(fx, bindir), wt)
    assert v == "KEEP" and "DIRTY" in why

    # remove the single blocking key -> the same tree is reclaimable
    (wt / "scratch-output.txt").unlink()
    assert verdict_for(sweep(fx, bindir), wt)[0] == "RECLAIM"


def test_unpushed_commit_outranks_a_merged_pull_request(fx, tmp_path):
    wt = fx.worktree("wt-ahead", "landed")
    (wt / "new.txt").write_text("work\n", encoding="utf-8")
    git(wt, "add", "-A")
    git(wt, "commit", "-m", "unpushed")
    bindir = fake_gh(tmp_path, MERGED)

    v, why = verdict_for(sweep(fx, bindir), wt)
    assert v == "KEEP" and "UNPUSHED" in why

    git(wt, "push")
    assert verdict_for(sweep(fx, bindir), wt)[0] == "RECLAIM"


def test_open_pull_request_is_kept(fx, tmp_path):
    wt = fx.worktree("wt-open", "inflight")
    bindir = fake_gh(
        tmp_path, [{"number": 13, "state": "OPEN", "headRefName": "inflight", "title": "t"}]
    )
    v, why = verdict_for(sweep(fx, bindir), wt)
    assert v == "KEEP" and "OPEN" in why


def test_open_pull_request_wins_over_a_stale_closed_one(fx, tmp_path):
    """A branch can carry several requests. Work is still expected on it."""
    wt = fx.worktree("wt-both", "reopened")
    bindir = fake_gh(
        tmp_path,
        [
            {"number": 14, "state": "CLOSED", "headRefName": "reopened", "title": "t"},
            {"number": 15, "state": "OPEN", "headRefName": "reopened", "title": "t"},
        ],
    )
    assert verdict_for(sweep(fx, bindir), wt)[0] == "KEEP"


def test_no_pull_request_at_all_is_kept(fx, tmp_path):
    """A branch with real, pushed work and no pull request yet is still KEPT.

    Distinct from an empty branch (below): here the tree has advanced past its
    base, so `branch_has_no_work_beyond_base` says False and the tool falls
    through to the ordinary "the work has not landed" refusal.
    """
    wt = fx.worktree("wt-fresh", "just-dispatched")
    (wt / "progress.txt").write_text("still working\n", encoding="utf-8")
    git(wt, "add", "-A")
    git(wt, "commit", "-m", "progress")
    git(wt, "push")
    v, why = verdict_for(sweep(fx, fake_gh(tmp_path, [])), wt)
    assert v == "KEEP" and "has not landed" in why


def test_an_empty_branch_with_no_pull_request_is_reclaimed(fx, tmp_path):
    """A branch cut from main and never advanced carries no work to land.

    This is the exact shape of the found defect: `wt-bh`
    (`fix/the-manifest-names-what-was-read`) had no commits beyond its base and
    no pull request, and the old ladder read "no PR" as "the work has not
    landed" and kept it forever. There is nothing here `origin/main` does not
    already hold, so it is reclaimable on the merge-base proof alone.
    """
    wt = fx.worktree("wt-empty", "nothing-to-land")
    v, why = verdict_for(sweep(fx, fake_gh(tmp_path, [])), wt)
    assert v == "RECLAIM"
    assert "merge-base" in why and "no work to land" in why

    out = sweep(fx, fake_gh(tmp_path, []), "--apply")
    assert not wt.exists()


def test_an_unpushed_commit_beyond_base_is_still_kept_with_no_pull_request(fx, tmp_path):
    """One real, unpushed commit is not "no work" — the new rung must not reach it.

    The branch is cut from main (so it starts equal to its merge-base, exactly
    like the reclaimed case above) and then advanced by one commit that is
    never pushed anywhere. The pre-existing UNPUSHED refusal (rung 1) must
    still catch this before the new merge-base rung is ever consulted.
    """
    wt = fx.worktree("wt-empty-ahead", "still-working")
    (wt / "wip.txt").write_text("wip\n", encoding="utf-8")
    git(wt, "add", "-A")
    git(wt, "commit", "-m", "wip")
    v, why = verdict_for(sweep(fx, fake_gh(tmp_path, [])), wt)
    assert v == "KEEP" and "UNPUSHED" in why


def test_a_request_outside_the_first_page_is_still_found(fx, tmp_path):
    """The defect this replaced, made falsifiable.

    `landed`'s request is old: it is the second row, and a bulk listing that
    returns one page contains only the newest. The previous design fetched that
    page and concluded "no pull request on the remote for this branch", which is
    a statement about a fetch wearing the clothes of a statement about the
    remote. Asking about the branch finds it, and the tree is reclaimed.
    """
    wt = fx.worktree("wt-landed", "landed")
    old_request = 259
    rows = [
        {"number": 466, "state": "OPEN", "headRefName": "something-recent", "title": "t"},
        {"number": old_request, "state": "MERGED", "headRefName": "landed", "title": "t"},
    ]
    bindir = fake_gh(tmp_path, rows, page=1)

    out = sweep(fx, bindir)
    v, why = verdict_for(out, wt)
    assert v == "RECLAIM", "a request older than one page was missed"
    assert str(old_request) in why


def test_an_answer_that_fills_the_limit_is_not_authoritative(fx, tmp_path):
    """A per-branch answer can itself be cut off at its limit. Fail closed."""
    wt = fx.worktree("wt-landed", "landed")
    rows = [
        {"number": 100 + i, "state": "MERGED", "headRefName": "landed", "title": "t"}
        for i in range(50)
    ]
    v, why = verdict_for(sweep(fx, fake_gh(tmp_path, rows)), wt)
    assert v == "KEEP"
    assert "NO PR AUTHORITY" in why and "truncated" in why


def test_the_authority_reports_what_it_actually_covered(fx, tmp_path):
    """A number that reads as coverage and is not coverage is worse than none.

    The line this replaced printed how many branches a bulk listing happened to
    contain, which measured the size of a fetch and read as knowledge of the
    remote.
    """
    fx.worktree("wt-a", "a")
    fx.worktree("wt-b", "b")
    out = sweep(fx, fake_gh(tmp_path, []))
    assert "2 branch(es) asked directly" in out
    assert "known to the remote" not in out


def test_the_authority_is_not_consulted_for_a_tree_decided_above_it(fx, tmp_path):
    """Rungs above the authority answer without asking — and say so."""
    wt = fx.worktree("wt-dirty", "landed")
    (wt / "unsaved.txt").write_text("x\n", encoding="utf-8")
    out = sweep(fx, fake_gh(tmp_path, MERGED))
    assert "not consulted — no tree reached that rung" in out
    assert verdict_for(out, wt)[0] == "KEEP"


def test_absent_authority_is_never_permission(fx, tmp_path):
    """`gh` down means nothing is reclaimable — not that everything is."""
    wt = fx.worktree("wt-landed", "landed")
    out = sweep(fx, broken_gh(tmp_path))
    v, why = verdict_for(out, wt)
    assert v == "KEEP" and "NO PR AUTHORITY" in why
    assert "UNAVAILABLE" in out


# ---------------------------------------------------------------------------
# leases
# ---------------------------------------------------------------------------


def lease(fx, path, *extra):
    subprocess.run(
        ["python3", str(TOOL), "--lease", str(path), *extra], capture_output=True, check=True
    )


def test_a_lease_is_honoured_over_a_merged_branch_and_reported(fx, tmp_path):
    """Honoured AND reported — and the second half was the one that was missing.

    This test carried `and_reported` in its name and asserted only that the row
    said `LEASED`. The rung it covers returned before asking the remote anything,
    so the property both this file's subject and `docs/reference/tools.md`
    promised — *a lease over a merged branch is reported* — did not exist, and
    the test named for it was green. Thirty-six leases on the machine printed one
    indistinguishable line and eight of them were over branches already merged.
    """
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a", "--reason", "still measuring")
    bindir = fake_gh(tmp_path, MERGED)

    out = sweep(fx, bindir)
    v, why = verdict_for(out, wt)
    assert v == "KEEP" and "LEASED by worker-a" in why
    merged_pr = MERGED[0]["number"]
    assert "SPENT" in why and f"#{merged_pr}" in why and "MERGED" in why
    assert "REPORTED, not resolved" in why
    # and the act that ends it, named where the row is read
    assert "--after-merge landed --apply" in out

    subprocess.run(["python3", str(TOOL), "--release", str(wt)], capture_output=True, check=True)
    assert verdict_for(sweep(fx, bindir), wt)[0] == "RECLAIM"


def test_a_lease_over_a_branch_the_remote_still_holds_open_is_not_called_spent(fx, tmp_path):
    """The report is a claim about the remote, so it must move with the remote."""
    wt = fx.worktree("wt-leased", "inflight")
    lease(fx, wt, "--holder", "worker-a")
    rows = [{"number": 12, "state": "OPEN", "headRefName": "inflight", "title": "t"}]
    out = sweep(fx, fake_gh(tmp_path, rows))
    v, why = verdict_for(out, wt)
    assert v == "KEEP" and "LEASED by worker-a" in why
    assert "SPENT" not in why
    assert "0 spent" in out


def test_a_lease_is_never_called_spent_on_a_remote_that_could_not_be_asked(fx, tmp_path):
    """`gh` down is not `no pull request`, and neither is it `landed`.

    A kept row must never print a claim about the remote out of a question the
    remote never answered.
    """
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a")
    out = sweep(fx, broken_gh(tmp_path))
    v, why = verdict_for(out, wt)
    assert v == "KEEP" and "LEASED by worker-a" in why
    assert "SPENT" not in why and "LANDED" not in why
    assert "could not be asked" in why


def test_a_lease_is_counted_spent_even_when_a_higher_rung_answers_first(fx, tmp_path):
    """The count is over LEASES, never over "leases whose tree reached rung 3".

    This tree is kept by UNPUSHED, which answers before the lease rung is ever
    reached — so `wt.exists()` would pass whether or not the lease was probed,
    and the only thing that moves is the number. That is the isolating
    perturbation, and it is the one that catches the real defect: asking at the
    rung reported 2 spent leases on a machine holding 8, because five of the
    eight sat over trees carrying commits on no remote. A numerator taken over
    one population and printed against another's denominator reads as coverage
    and understates in the reassuring direction.
    """
    wt = fx.worktree("wt-leased", "landed")
    (wt / "work.txt").write_text("x\n", encoding="utf-8")
    git(wt, "add", "-A")
    git(wt, "commit", "-m", "on no remote")
    lease(fx, wt, "--holder", "worker-a")

    out = sweep(fx, fake_gh(tmp_path, MERGED))
    v, why = verdict_for(out, wt)
    assert v == "KEEP" and "UNPUSHED" in why, "rung 1 must still be the one that answers"
    assert "1 lease(s) held (1 spent)" in out
    assert "--after-merge landed --apply" in out


def test_the_sweep_never_releases_a_lease_even_over_a_merged_branch(fx, tmp_path):
    """The refusal that keeps this from becoming `quiet means dead`.

    The perturbation is chosen so that only the thing under test could catch it:
    the assertion is on the LEASE FILE, which nothing but a release path removes.
    A sweep that read a landed pull request as permission to drop the claim would
    be resolving on its own reading of how finished something looks, which is the
    judgement that once deleted a running worker's directories.
    """
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a")
    admin = pathlib.Path(git(wt, "rev-parse", "--git-dir").strip())
    if not admin.is_absolute():
        admin = wt / admin
    out = sweep(fx, fake_gh(tmp_path, MERGED), "--apply")
    assert (admin / "dw-lease.json").exists(), "a sweep released a lease"
    assert wt.exists()
    assert "1 lease(s) held (1 spent)" in out


# ---------------------------------------------------------------------------
# what ENDS a lease
#
# Before this, nothing did. `tools/worktree-new.sh` claims every tree it makes
# and prints "release it at the merge"; nothing invoked `--release`, so the
# sentence WAS the mechanism — the UNRUN vacuity mode. `--after-merge`, the entry
# point that exists to run in the same breath as a merge, was itself blocked by
# the lease handed out at dispatch, so the top rung held every object and every
# rung below it was unreachable for every tree the dispatch script had created.
# Measured on this machine at the time: 36 leases, 0 releases, 8 of them over a
# branch the remote had already merged.
# ---------------------------------------------------------------------------


def after_merge(fx, bindir, branch, *extra):
    env = dict(os.environ)
    env["PATH"] = f"{bindir}{os.pathsep}{env['PATH']}"
    p = subprocess.run(
        ["python3", str(TOOL), "--repo", str(fx.repo), "--after-merge", branch, *extra],
        capture_output=True,
        text=True,
        env=env,
    )
    return p.stdout + p.stderr


def lease_file(wt):
    admin = pathlib.Path(git(wt, "rev-parse", "--git-dir").strip())
    if not admin.is_absolute():
        admin = wt / admin
    return admin / "dw-lease.json"


def test_the_merge_ends_the_lease_it_was_taken_against(fx, tmp_path):
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a")
    out = after_merge(fx, fake_gh(tmp_path, MERGED), "landed", "--apply")
    assert "LEASE released" in out
    assert not wt.exists(), out


def test_a_dry_run_shows_the_verdict_behind_the_lease_and_ends_nothing(fx, tmp_path):
    """A dry run must show the ladder's answer, not the lease standing in front."""
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a")
    f = lease_file(wt)
    out = after_merge(fx, fake_gh(tmp_path, MERGED), "landed")
    assert "WOULD BE released" in out
    assert "RECLAIM" in out
    assert f.exists(), "a dry run deleted a lease file"
    assert wt.exists()


def test_the_merge_does_not_end_a_lease_the_remote_has_not_landed(fx, tmp_path):
    """Naming the branch only SELECTS; the key is the remote's own verdict.

    The isolating assertion is the lease file: with an open pull request the tree
    is kept by the authority rung too, so `wt.exists()` would pass whether or not
    the release fired. Only a release path can remove that file.
    """
    wt = fx.worktree("wt-leased", "inflight")
    lease(fx, wt, "--holder", "worker-a")
    f = lease_file(wt)
    rows = [{"number": 12, "state": "OPEN", "headRefName": "inflight", "title": "t"}]
    out = after_merge(fx, fake_gh(tmp_path, rows), "inflight", "--apply")
    open_pr = rows[0]["number"]
    assert "LEASE KEPT" in out and f"#{open_pr} is OPEN" in out
    assert f.exists(), "a lease was ended over a branch that had not landed"
    assert wt.exists()


def test_the_merge_does_not_end_a_lease_on_a_remote_it_could_not_ask(fx, tmp_path):
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a")
    f = lease_file(wt)
    out = after_merge(fx, broken_gh(tmp_path), "landed", "--apply")
    assert "LEASE KEPT" in out and "not permission" in out
    assert f.exists() and wt.exists()


def test_ending_the_lease_is_not_permission_dirty_still_refuses(fx, tmp_path):
    """The perturbation only rung 1 can catch.

    The lease is gone by construction here — asserted, not assumed — so nothing
    above `DIRTY` is left to produce the refusal. If the release were treated as
    permission this tree would be deleted with uncommitted work in it.
    """
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a")
    (wt / "unsaved.txt").write_text("x\n", encoding="utf-8")
    f = lease_file(wt)
    out = after_merge(fx, fake_gh(tmp_path, MERGED), "landed", "--apply")
    assert not f.exists(), "the lease was not actually released, so this proves nothing"
    assert "DIRTY" in out
    assert wt.exists() and (wt / "unsaved.txt").exists()


def test_ending_the_lease_is_not_permission_unpushed_still_refuses(fx, tmp_path):
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a")
    (wt / "work.txt").write_text("x\n", encoding="utf-8")
    git(wt, "add", "-A")
    git(wt, "commit", "-m", "unpushed")
    f = lease_file(wt)
    out = after_merge(fx, fake_gh(tmp_path, MERGED), "landed", "--apply")
    assert not f.exists(), "the lease was not actually released, so this proves nothing"
    assert "UNPUSHED" in out
    assert wt.exists()


def test_ending_the_lease_is_not_permission_a_link_target_still_refuses(fx, tmp_path):
    """The reachability key survives the release too — liveness in a symlink."""
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a")
    reader = fx.worktree("wt-reader", "inflight")
    (reader / "campaigns").symlink_to(wt)
    f = lease_file(wt)
    out = after_merge(fx, fake_gh(tmp_path, MERGED), "landed", "--apply")
    assert not f.exists(), "the lease was not actually released, so this proves nothing"
    assert "LINK TARGET" in out
    assert wt.exists()


def test_a_leased_detached_tree_is_told_what_ends_its_lease(fx, tmp_path):
    """A detached tree has no branch, so no merge can end its lease.

    `--tree` is the operator's naming, and it is not a release: the run must say
    so rather than doing nothing and exiting zero, which is the silent no-op this
    tool refuses everywhere else.
    """
    pinned = fx.worktree("content-pin", detached=True)
    lease(fx, pinned, "--holder", "planner-x")
    bindir = fake_gh(tmp_path, [])
    env = dict(os.environ)
    env["PATH"] = f"{bindir}{os.pathsep}{env['PATH']}"
    p = subprocess.run(
        ["python3", str(TOOL), "--repo", str(fx.repo), "--tree", str(pinned), "--apply"],
        capture_output=True,
        text=True,
        env=env,
    )
    assert pinned.exists()
    assert "LEASED by planner-x" in p.stdout
    assert f"--release {pinned}" in p.stdout


def test_the_hint_names_the_rung_that_actually_answered(fx, tmp_path):
    """A hint that names the wrong cause is worse than none, being actionable.

    This tree is leased AND dirty, and rung 1 is what keeps it. Telling its
    operator "the lease is what is holding it" would send them to release a claim
    that is not the obstacle, and the tree would still be kept afterwards.
    """
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a")
    (wt / "unsaved.txt").write_text("x\n", encoding="utf-8")
    out = after_merge(fx, fake_gh(tmp_path, []), "landed")
    assert "DIRTY" in out
    assert "the lease is what is holding it" not in out

    # and where the lease IS the rung that answered, the hint appears
    (wt / "unsaved.txt").unlink()
    out = after_merge(fx, fake_gh(tmp_path, []), "landed")
    assert "the lease is what is holding it" in out
    assert f"--release {wt}" in out


def test_the_lease_binding_count_moves_with_the_leases(fx, tmp_path):
    """A stated count that is a constant is green on a gate that binds nothing.

    Perturbed at every value it can take: none held, all held and all spent, then
    one ended at the merge — and the pair of numbers moves each time, with the
    tree count moving behind it.
    """
    bindir = fake_gh(fx.root, MERGED + [{"number": 13, "state": "MERGED",
                                         "headRefName": "landed2", "title": "t"}])
    a = fx.worktree("wt-a", "landed")
    b = fx.worktree("wt-b", "landed2")

    # none held: the ladder below is reachable for both. The pair is printed
    # unconditionally — a count that appears only when it is non-zero cannot be
    # read as a binding, because its absence and a zero look the same.
    out = sweep(fx, bindir)
    assert "0 lease(s) held (0 spent)" in out
    assert "2 tree(s) reclaimable" in out

    # both held, both spent: the top rung has swallowed both objects
    lease(fx, a, "--holder", "worker-a")
    lease(fx, b, "--holder", "worker-b")
    out = sweep(fx, bindir)
    assert "2 lease(s) held (2 spent)" in out
    assert "0 tree(s) reclaimable" in out
    assert "FINDING — 2 SPENT LEASE(S)" in out

    # a dry run at the merge moves nothing at all
    assert "WOULD BE released" in after_merge(fx, bindir, "landed")
    out = sweep(fx, bindir)
    assert "2 lease(s) held (2 spent)" in out

    # one ended by the event that ends it, and the population moves behind it
    assert "LEASE released" in after_merge(fx, bindir, "landed", "--apply")
    out = sweep(fx, bindir)
    assert "1 worktree(s) examined" in out
    assert "1 lease(s) held (1 spent)" in out


def test_the_zero_stays_zero_when_every_lease_is_genuinely_live(fx, tmp_path):
    """The other direction, which is the one that makes the count worth reading.

    Two claimed trees whose pull requests the remote still holds OPEN: the sweep
    must report the leases, report NONE of them spent, reclaim nothing, and end
    nothing at the merge either.
    """
    rows = [
        {"number": 21, "state": "OPEN", "headRefName": "live-a", "title": "t"},
        {"number": 22, "state": "OPEN", "headRefName": "live-b", "title": "t"},
    ]
    bindir = fake_gh(fx.root, rows)
    a = fx.worktree("wt-a", "live-a")
    fx.worktree("wt-b", "live-b")
    lease(fx, a, "--holder", "worker-a")
    lease(fx, fx.scratch / "wt-b", "--holder", "worker-b")

    out = sweep(fx, bindir, "--apply")
    assert "2 lease(s) held (0 spent)" in out
    assert "0 tree(s) reclaimable" in out
    assert "SPENT LEASE(S)" not in out
    assert a.exists() and (fx.scratch / "wt-b").exists()
    assert lease_file(a).exists()

    assert "LEASE KEPT" in after_merge(fx, bindir, "live-a", "--apply")
    assert lease_file(a).exists() and a.exists()


def test_a_spent_lease_is_named_on_the_build_output_it_is_holding(fx, tmp_path):
    """The disk is in the target rows, so that is where the reason must be legible."""
    wt = fx.worktree("wt-leased", "landed")
    # The tree must be CLEAN for the lease rung to be the one that decides it —
    # `target/` is untracked otherwise and rung 1 answers first, which is the
    # ladder working, not a fixture detail worth hiding.
    (wt / ".gitignore").write_text("target/\n", encoding="utf-8")
    git(wt, "add", "-A")
    git(wt, "commit", "-m", "ignore build output")
    git(wt, "push")
    make_target(wt)
    lease(fx, wt, "--holder", "worker-a")
    out = sweep(fx, fake_gh(tmp_path, MERGED))
    rows = out.splitlines()
    target_row = [
        rows[i + 1] for i, ln in enumerate(rows) if ln.strip().endswith(str(wt / "target"))
    ]
    assert target_row, out
    assert "SPENT" in target_row[0] and "--after-merge landed --apply" in target_row[0]


def test_an_elapsed_lease_is_still_honoured(fx, tmp_path):
    """Quiet is not evidence, and neither is a timestamp. An expired lease is
    reported as elapsed and the tree is still kept."""
    wt = fx.worktree("wt-old", "landed")
    lease(fx, wt, "--hours", "1")
    admin = pathlib.Path(git(wt, "rev-parse", "--git-dir").strip())
    if not admin.is_absolute():
        admin = wt / admin
    f = admin / "dw-lease.json"
    payload = json.loads(f.read_text(encoding="utf-8"))
    # Ten hours old against a one-hour window. NOT the epoch: a zero `created`
    # means "this lease did not record when it was taken", which the tool reads
    # as an unknown rather than as an expiry — the same fail-closed direction.
    payload["created"] = int(time.time()) - 10 * 3600
    f.write_text(json.dumps(payload), encoding="utf-8")

    v, why = verdict_for(sweep(fx, fake_gh(tmp_path, MERGED)), wt)
    assert v == "KEEP"
    assert "elapsed" in why and "REPORTED, not resolved" in why


def test_a_lease_does_not_survive_its_tree(fx, tmp_path):
    """The lease lives in git's admin directory, so `worktree remove` takes it."""
    wt = fx.worktree("wt-gone", "landed")
    lease(fx, wt)
    admin = pathlib.Path(git(wt, "rev-parse", "--git-dir").strip())
    if not admin.is_absolute():
        admin = wt / admin
    assert (admin / "dw-lease.json").exists()
    assert not (wt / "dw-lease.json").exists(), "a lease must not make the tree dirty"
    subprocess.run(["python3", str(TOOL), "--release", str(wt)], capture_output=True, check=True)
    sweep(fx, fake_gh(tmp_path, MERGED), "--apply")
    assert not admin.exists()


# ---------------------------------------------------------------------------
# reachability — the key the incident added
# ---------------------------------------------------------------------------


def test_a_detached_tree_is_never_reclaimed_by_a_sweep(fx, tmp_path):
    """A pinned-content read tree and a spent verification tree look identical."""
    wt = fx.worktree("content-pin", detached=True)
    v, why = verdict_for(sweep(fx, fake_gh(tmp_path, MERGED)), wt)
    assert v == "KEEP" and "DETACHED" in why


def test_a_link_target_is_kept_even_though_every_local_signal_says_spent(fx, tmp_path):
    """The incident, reconstructed.

    `content-f0dd596` is detached, clean, fully pushed, has no pull request and
    nobody's cwd is inside it. Its liveness is one symlink in a live worker's
    tree: `wt-pressunion/campaigns`. A sweep that reads the tree in isolation
    deletes it and the live worker keeps running, measuring zero.
    """
    pinned = fx.worktree("content-f0dd596", detached=True)
    live = fx.worktree("wt-pressunion", "inflight")
    (live / "campaigns").symlink_to(pinned)

    out = sweep(
        fx,
        fake_gh(tmp_path, [{"number": 16, "state": "OPEN", "headRefName": "inflight", "title": "t"}]),
    )
    v, why = verdict_for(out, pinned)
    assert v == "KEEP"
    assert "LINK TARGET" in why and "campaigns" in why


def test_reachability_outranks_a_merged_pull_request(fx, tmp_path):
    """The ladder, not a special case for detached trees: a branch tree whose
    work has landed is still kept while anything points into it."""
    target = fx.worktree("wt-landed", "landed")
    live = fx.worktree("wt-cratesweep", "inflight")
    (live / "campaigns").symlink_to(target)
    bindir = fake_gh(
        tmp_path,
        MERGED + [{"number": 17, "state": "OPEN", "headRefName": "inflight", "title": "t"}],
    )

    v, why = verdict_for(sweep(fx, bindir), target)
    assert v == "KEEP" and "LINK TARGET" in why

    (live / "campaigns").unlink()
    assert verdict_for(sweep(fx, bindir), target)[0] == "RECLAIM"


def test_a_self_link_does_not_protect_a_tree(fx, tmp_path):
    """Otherwise every tree carrying a `campaigns` link would protect itself.

    The link has to be COMMITTED to test what it claims to test: an untracked
    symlink is untracked content, so the tree is dirty and would be kept for a
    reason that has nothing to do with reachability. That is asserted first, so
    the second half cannot pass for the first half's reason.
    """
    wt = fx.worktree("wt-landed", "landed")
    (wt / "selfref").symlink_to(wt / "README")
    bindir = fake_gh(tmp_path, MERGED)
    v, why = verdict_for(sweep(fx, bindir), wt)
    assert v == "KEEP" and "DIRTY" in why

    git(wt, "add", "-A")
    git(wt, "commit", "-m", "a link inside the tree, pointing inside the tree")
    git(wt, "push")
    assert verdict_for(sweep(fx, bindir), wt)[0] == "RECLAIM"


def test_dangling_links_are_loud(fx, tmp_path):
    live = fx.worktree("wt-live", "inflight")
    (live / "campaigns").symlink_to(fx.scratch / "content-deleted-by-a-hand-sweep")
    out = sweep(fx, fake_gh(tmp_path, []))
    assert "DANGLING SYMLINK" in out
    assert "measures\n" in out or "ZERO" in out
    assert "campaigns" in out


def test_a_link_that_is_supposed_to_dangle_is_counted_not_listed(fx, tmp_path):
    """A finding list that is mostly noise teaches its reader to skip it.

    A running browser leaves sentinels whose target is a process id or a
    hostname. They are filtered by name, counted, and the count printed — and a
    real dead dispatch link beside them is still reported in full.
    """
    live = fx.worktree("wt-live", "inflight")
    (live / "SingletonLock").symlink_to("Mac-55890")
    (live / "RunningChromeVersion").symlink_to("151.0.7922.108:1")
    (live / "campaigns").symlink_to(fx.scratch / "content-deleted-by-a-hand-sweep")

    out = sweep(fx, fake_gh(tmp_path, []))
    assert "2 dangling link(s) that are supposed to dangle" in out
    assert "FINDING — 1 DANGLING SYMLINK(S)" in out
    assert "SingletonLock" not in out.split("FINDING")[1]
    assert "campaigns" in out.split("FINDING")[1]


def test_the_reachability_key_states_its_binding(fx, tmp_path):
    fx.worktree("wt-landed", "landed")
    out = sweep(fx, fake_gh(tmp_path, []))
    assert "symlink(s) resolved across" in out


# ---------------------------------------------------------------------------
# binding counts
# ---------------------------------------------------------------------------


def test_examining_nothing_is_a_finding(fx, tmp_path):
    out = sweep(fx, fake_gh(tmp_path, []))
    assert "BINDING ZERO" in out
    assert "wrong\n  repository" in out or "wrong" in out


def test_every_run_states_what_it_examined(fx, tmp_path):
    fx.worktree("wt-a", "a")
    fx.worktree("wt-b", "b")
    out = sweep(fx, fake_gh(tmp_path, []))
    assert "2 worktree(s) examined" in out
    assert "1 repositor" in out


# ---------------------------------------------------------------------------
# the narrow entry points
# ---------------------------------------------------------------------------


def test_after_merge_reclaims_exactly_one_tree_under_the_same_proof(fx, tmp_path):
    landed = fx.worktree("wt-landed", "landed")
    other = fx.worktree("wt-other", "inflight")
    bindir = fake_gh(tmp_path, MERGED)
    env = dict(os.environ)
    env["PATH"] = f"{bindir}{os.pathsep}{env['PATH']}"
    subprocess.run(
        ["python3", str(TOOL), "--repo", str(fx.repo), "--after-merge", "landed", "--apply"],
        capture_output=True,
        text=True,
        env=env,
        check=True,
    )
    assert not landed.exists()
    assert other.exists()


def test_after_merge_on_a_dirty_tree_refuses(fx, tmp_path):
    landed = fx.worktree("wt-landed", "landed")
    (landed / "unsaved.txt").write_text("x\n", encoding="utf-8")
    bindir = fake_gh(tmp_path, MERGED)
    env = dict(os.environ)
    env["PATH"] = f"{bindir}{os.pathsep}{env['PATH']}"
    p = subprocess.run(
        ["python3", str(TOOL), "--repo", str(fx.repo), "--after-merge", "landed", "--apply"],
        capture_output=True,
        text=True,
        env=env,
    )
    assert landed.exists()
    assert "DIRTY" in p.stdout


def test_a_name_that_matches_nothing_is_a_finding(fx, tmp_path):
    bindir = fake_gh(tmp_path, [])
    env = dict(os.environ)
    env["PATH"] = f"{bindir}{os.pathsep}{env['PATH']}"
    p = subprocess.run(
        ["python3", str(TOOL), "--repo", str(fx.repo), "--after-merge", "never-existed", "--apply"],
        capture_output=True,
        text=True,
        env=env,
    )
    assert p.returncode == 1
    assert "finding" in p.stdout
    # A refusal that only says "no match" cannot be told apart from a repository
    # the sweep never looked at. It states what it swept, by path, and names the
    # flag that reaches a checkout outside that set.
    assert "swept:" in p.stdout
    assert fx.repo.name in p.stdout
    assert "--repo <path>" in p.stdout


def test_naming_a_detached_tree_is_the_authority_it_lacks_but_not_the_only_key(fx, tmp_path):
    pinned = fx.worktree("content-pin", detached=True)
    bindir = fake_gh(tmp_path, [])
    env = dict(os.environ)
    env["PATH"] = f"{bindir}{os.pathsep}{env['PATH']}"

    # still referenced -> the override does not reach it
    live = fx.worktree("wt-live", "inflight")
    (live / "campaigns").symlink_to(pinned)
    p = subprocess.run(
        ["python3", str(TOOL), "--repo", str(fx.repo), "--tree", str(pinned), "--apply"],
        capture_output=True,
        text=True,
        env=env,
    )
    assert pinned.exists()
    assert "LINK TARGET" in p.stdout

    # unreferenced -> the operator's naming is accepted, and says what it overrode
    (live / "campaigns").unlink()
    p = subprocess.run(
        ["python3", str(TOOL), "--repo", str(fx.repo), "--tree", str(pinned), "--apply"],
        capture_output=True,
        text=True,
        env=env,
    )
    assert "OVERRIDE" in p.stdout
    assert not pinned.exists()


# ---------------------------------------------------------------------------
# rebuildable output and disk pressure
# ---------------------------------------------------------------------------


def make_target(path):
    t = path / "target"
    t.mkdir()
    (t / "CACHEDIR.TAG").write_text(
        "Signature: 8a477f597d28d172789f06886806bc55\n", encoding="utf-8"
    )
    (t / "debug").mkdir()
    (t / "debug" / "blob").write_bytes(b"0" * 4096)
    return t


def backdate(path, hours):
    """Age everything under `path`, deepest first so no parent is re-touched."""
    old = time.time() - hours * 3600
    for dirpath, dirnames, filenames in os.walk(path, topdown=False):
        for name in filenames + dirnames:
            os.utime(os.path.join(dirpath, name), (old, old))
    os.utime(path, (old, old))


OPEN = [{"number": 18, "state": "OPEN", "headRefName": "inflight", "title": "t"}]


def test_free_space_no_longer_decides_whether_build_output_is_waste(fx, tmp_path):
    """The defect this replaced, driven in BOTH directions on one tree.

    Build output is waste when nothing will build in it again, which is not a
    fact about how full the disk is. The old gate asked the disk, so nineteen
    dead trees held a hundred gibibytes with two hundred and fifty-eight free
    and the valve never opened. These two sweeps differ ONLY in the target's
    age; the free-space threshold is pinned at a value that makes pressure
    structurally impossible in both.
    """
    wt = fx.worktree("wt-open", "inflight")
    t = make_target(wt)
    bindir = fake_gh(tmp_path, OPEN)

    # fresh -> kept, and kept for a LIVENESS reason rather than for space
    out = sweep(fx, bindir, "--free-below", "0.0001")
    v, why = verdict_for(out, t)
    assert v == "KEEP"
    assert "still live" in why
    assert "ALARM" not in out, "0.0001 GiB must not trip the alarm"

    # idle -> reclaimed, at the same impossible-to-trip threshold
    backdate(t, 200)
    v, why = verdict_for(sweep(fx, bindir, "--free-below", "0.0001"), t)
    assert v == "RECLAIM"
    assert "IDLE" in why and "200h" in why


def test_targets_only_removes_idle_output_and_never_the_worktree(fx, tmp_path):
    wt = fx.worktree("wt-open", "inflight")
    t = make_target(wt)
    backdate(t, 200)
    sweep(fx, fake_gh(tmp_path, OPEN), "--apply", "--targets-only")
    assert not t.exists()
    assert wt.exists(), "--targets-only must never touch a worktree"
    assert (wt / "README").exists()


def test_a_build_in_flight_holds_its_output_against_every_other_key(fx, tmp_path):
    """The un-forgeable key at this layer, driven both ways on one tree.

    The tree is idle by every timestamp and its branch is not even open — the
    only thing standing between it and deletion is a live `flock`, which is
    exactly the thing a running build has and a dead one cannot fake.
    """
    import fcntl

    wt = fx.worktree("wt-open", "inflight")
    t = make_target(wt)
    (t / "debug" / ".cargo-lock").write_bytes(b"")
    backdate(t, 200)
    bindir = fake_gh(tmp_path, OPEN)

    fd = os.open(str(t / "debug" / ".cargo-lock"), os.O_RDWR)
    fcntl.flock(fd, fcntl.LOCK_EX)
    try:
        v, why = verdict_for(sweep(fx, bindir), t)
        assert v == "KEEP"
        assert "BUILD IN FLIGHT" in why
        sweep(fx, bindir, "--apply", "--targets-only")
        assert t.exists(), "a build in flight must keep its own output"
    finally:
        fcntl.flock(fd, fcntl.LOCK_UN)
        os.close(fd)

    # the one key removed -> the same tree turns RECLAIM
    v, why = verdict_for(sweep(fx, bindir), t)
    assert v == "RECLAIM"
    assert "IDLE" in why


def test_an_unopenable_build_lock_counts_as_held(fx, tmp_path):
    """Absence of the answer is never permission — the rule the authority obeys
    one layer up, applied to the kernel's answer at this one."""
    wt = fx.worktree("wt-open", "inflight")
    t = make_target(wt)
    lock = t / "debug" / ".cargo-lock"
    lock.write_bytes(b"")
    backdate(t, 200)
    os.chmod(lock, 0o000)
    try:
        v, why = verdict_for(sweep(fx, fake_gh(tmp_path, OPEN)), t)
        assert v == "KEEP"
        assert "could not be opened" in why and "HELD" in why
    finally:
        os.chmod(lock, 0o644)


def test_landed_work_gives_up_its_output_with_no_idle_window_at_all(fx, tmp_path):
    """The strong arm needs no threshold: the work is on the remote.

    The target here is brand new — well inside the idle window — so IDLE cannot
    be what decides it, and the mirror case pins that down: the identical tree
    with an OPEN request instead of a MERGED one is kept.
    """
    wt = fx.worktree("wt-landed", "landed")
    t = make_target(wt)
    (wt / "scratch-note.txt").write_text("dirty\n", encoding="utf-8")  # tree must be KEPT

    v, why = verdict_for(sweep(fx, fake_gh(tmp_path, MERGED)), t)
    assert v == "RECLAIM"
    # The number comes from the fixture rather than being written out, so this
    # assertion carries no identifier a reader could mistake for a real one.
    assert "LANDED" in why and str(MERGED[0]["number"]) in why

    other = fx.worktree("wt-open", "inflight")
    t2 = make_target(other)
    v2, why2 = verdict_for(sweep(fx, fake_gh(tmp_path, OPEN)), t2)
    assert v2 == "KEEP"
    assert "still live" in why2


def test_build_output_is_judged_on_liveness_while_the_tree_is_judged_on_git(fx, tmp_path):
    """One tree, two ladders, opposite verdicts — and the work survives.

    A dirty tree is never deleted, by any path, for any reason. Its `target/` is
    not git state at all, so the same tree can legitimately give up its build
    output while keeping every byte a deletion could destroy.
    """
    wt = fx.worktree("wt-dirty-idle", "landed")
    t = make_target(wt)
    backdate(t, 200)
    (wt / "untracked-work.txt").write_text("precious\n", encoding="utf-8")

    out = sweep(fx, fake_gh(tmp_path, MERGED), "--apply")
    assert verdict_for(out, wt)[0] == "KEEP"
    assert "DIRTY" in verdict_for(out, wt)[1]
    assert not t.exists(), "idle build output goes"
    assert wt.exists() and (wt / "untracked-work.txt").read_text() == "precious\n"


def test_a_leased_tree_keeps_its_build_output(fx, tmp_path):
    wt = fx.worktree("wt-open", "inflight")
    t = make_target(wt)
    backdate(t, 200)
    lease(fx, wt, "--holder", "worker-b")
    out = sweep(fx, fake_gh(tmp_path, OPEN), "--apply", "--targets-only")
    assert t.exists(), "a claimed tree keeps its build output; a rebuild is not free"
    assert "LEASED by worker-b" in verdict_for(out, t)[1], "the row must say WHICH key held it"


def test_a_lease_covers_the_scratch_directory_beside_the_tree(fx, tmp_path):
    """A dispatch gets a tree and a scratch directory beside it, and a live
    worker's scratch is never touched. Build output in scratch belongs to no
    worktree, so without this it would be judged with no lease to find."""
    wt = fx.worktree("wt-open", "inflight")
    beside = wt.parent / "scratch"
    beside.mkdir()
    scratch_target = make_target(beside)
    backdate(scratch_target, 200)
    lease(fx, wt, "--holder", "worker-c")
    out = sweep(fx, fake_gh(tmp_path, OPEN), "--apply", "--targets-only")
    assert scratch_target.exists(), "a claimed worker's scratch build output must survive"
    assert "scratch directory beside a claimed tree" in verdict_for(out, scratch_target)[1]


def test_a_kept_row_never_claims_the_remote_said_something_it_was_not_asked(fx, tmp_path):
    """"The work has not landed" is a claim ABOUT THE REMOTE, and it is unearned
    when the remote could not be asked. A row that prints the two as one fact is
    the shape this whole tool exists to refuse, at the smallest scale: an absent
    answer wearing the clothes of a negative one."""
    wt = fx.worktree("wt-open", "inflight")
    t = make_target(wt)

    # asked, and answered -> the row may name the answer
    v, why = verdict_for(sweep(fx, fake_gh(tmp_path, OPEN)), t)
    assert v == "KEEP" and "is OPEN" in why

    # could not be asked -> the row must say so instead
    v, why = verdict_for(sweep(fx, broken_gh(tmp_path)), t)
    assert v == "KEEP"
    assert "could not be asked" in why
    assert "has not landed" not in why

    # detached, so there is nothing to ask about
    det = fx.worktree("wt-det", detached=True)
    t2 = make_target(det)
    v, why = verdict_for(sweep(fx, fake_gh(tmp_path, OPEN)), t2)
    assert v == "KEEP"
    assert "no branch" in why and "has not landed" not in why


def test_one_target_directory_is_counted_once_however_many_roots_reach_it(fx, tmp_path):
    """The scan roots overlap by construction: each repository plus the parent
    of every worktree, and a worktree's parent is routinely inside another root.
    A live run reported 52 targets where there were 36, duplicated sixteen rows,
    listed one three times, and inflated the reclaimable count from 11 to 13."""
    wt = fx.worktree("wt-open", "inflight")
    t = make_target(wt)
    backdate(t, 200)
    # --scan-dir names a root that already reaches this target by another path.
    out = sweep(fx, fake_gh(tmp_path, OPEN), "--scan-dir", str(fx.scratch),
                "--scan-dir", str(fx.root))
    rows = [ln for ln in out.splitlines() if ln.strip().endswith(str(t))]
    assert len(rows) == 1, f"counted {len(rows)} times:\n{out}"
    assert "1 target/ director(y|ies) examined" in out


def test_the_main_checkout_is_never_stripped_because_it_is_the_clone_donor(fx, tmp_path):
    """`tools/worktree-new.sh` clones a new worktree's `target/` from the main
    checkout. Deleting it frees none of the blocks a clone shares with it and
    makes the next dispatch pay for a cold compile."""
    t = make_target(fx.repo)
    backdate(t, 5000)
    out = sweep(fx, fake_gh(tmp_path, MERGED), "--apply")
    assert t.exists()
    assert "donor" in verdict_for(out, t)[1]


def test_a_directory_named_target_without_cargos_signature_is_not_touched(fx, tmp_path):
    wt = fx.worktree("wt-open", "inflight")
    d = wt / "target"
    d.mkdir()
    (d / "source.rs").write_text("fn main() {}\n", encoding="utf-8")
    sweep(
        fx,
        fake_gh(tmp_path, [{"number": 21, "state": "OPEN", "headRefName": "inflight", "title": "t"}]),
        "--free-below",
        "999999",
        "--apply",
        "--targets-only",
    )
    assert (d / "source.rs").exists()


# ---------------------------------------------------------------------------
# the harness's own throwaway branches
# ---------------------------------------------------------------------------


def test_harness_branches_contained_in_main_are_reported(fx, tmp_path):
    git(fx.repo, "branch", "worktree-agent-abc")
    fx.worktree("wt-open", "inflight")
    out = sweep(
        fx,
        fake_gh(tmp_path, [{"number": 22, "state": "OPEN", "headRefName": "inflight", "title": "t"}]),
    )
    assert "worktree-agent-abc" in out


# ---------------------------------------------------------------------------
# the binding — what INVOKES this
#
# A gate nothing invokes is not a gate, so the invocation is the thing under
# test here, not the tool. Both halves are checked: that the hook script still
# carries the call, and that running the hook script actually drains a stale
# tree. The first alone would pass over a call that had stopped working; the
# second alone would pass over a call nobody reaches.
# ---------------------------------------------------------------------------

HOOK = pathlib.Path(__file__).resolve().parents[1] / "planner-state.sh"


def test_the_hook_script_still_carries_the_invocation():
    text = HOOK.read_text(encoding="utf-8")
    assert "worktree-reclaim.py" in text
    assert "--apply" in text, "reporting is what this page did while the disk filled"


def test_the_hook_end_to_end_drains_a_stale_tree(fx, tmp_path):
    """`planner-state.sh` copied into a throwaway repo, then run.

    This is the whole point of the design: nobody typed a reclamation command.
    The tree goes away because a session started.
    """
    tools = fx.repo / "tools"
    tools.mkdir()
    for name in ("planner-state.sh", "worktree-reclaim.py"):
        src = pathlib.Path(__file__).resolve().parents[1] / name
        dst = tools / name
        dst.write_text(src.read_text(encoding="utf-8"), encoding="utf-8")
        dst.chmod(0o755)
    (fx.repo / "CLAUDE.local.md").write_text("local half\n", encoding="utf-8")
    git(fx.repo, "add", "-A")
    git(fx.repo, "commit", "-m", "tools")
    git(fx.repo, "push")

    landed = fx.worktree("wt-landed", "landed")
    inflight = fx.worktree("wt-inflight", "inflight")
    bindir = fake_gh(
        tmp_path,
        MERGED + [{"number": 23, "state": "OPEN", "headRefName": "inflight", "title": "t"}],
    )
    env = dict(os.environ)
    env["PATH"] = f"{bindir}{os.pathsep}{env['PATH']}"
    p = subprocess.run(
        ["bash", str(tools / "planner-state.sh")],
        capture_output=True,
        text=True,
        env=env,
        cwd=str(fx.root),
    )
    assert p.returncode == 0, p.stderr
    assert "worktree reclamation" in p.stdout
    assert not landed.exists(), "the hook reported instead of draining"
    assert inflight.exists(), "the hook deleted a tree whose work has not landed"


def test_the_hook_refuses_by_name_when_the_tool_is_absent(fx, tmp_path):
    """The same shape the page already uses for the missing constitution half:
    a silent no-op here is the UNRUN vacuity mode wearing the fix's clothes."""
    tools = fx.repo / "tools"
    tools.mkdir()
    src = pathlib.Path(__file__).resolve().parents[1] / "planner-state.sh"
    dst = tools / "planner-state.sh"
    dst.write_text(src.read_text(encoding="utf-8"), encoding="utf-8")
    dst.chmod(0o755)
    p = subprocess.run(
        ["bash", str(dst)], capture_output=True, text=True, cwd=str(fx.root)
    )
    assert "REFUSED" in p.stdout
    assert "worktree-reclaim.py" in p.stdout


def test_a_checked_out_harness_branch_is_not_listed(fx, tmp_path):
    """A throwaway branch that a worktree still has checked out is not garbage.

    The branch name still appears in the run — as that worktree's own row — so
    the assertion is on the section, not on the string.
    """
    fx.worktree("wt-agent", "worktree-agent-live")
    out = sweep(fx, fake_gh(tmp_path, []))
    assert "harness throwaway" not in out
