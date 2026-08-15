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


def fake_gh(tmp_path, rows):
    """A `gh` that answers with `rows` and nothing else.

    Named `gh` and placed first on PATH, so the real invocation — flags, `-R`,
    `--json` field list, JSON parsing — is the thing being exercised.
    """
    bindir = tmp_path / "fakebin"
    bindir.mkdir(exist_ok=True)
    payload = json.dumps(rows)
    script = bindir / "gh"
    script.write_text(
        "#!/usr/bin/env bash\n"
        "# every argument is ignored; this is an oracle, not a client\n"
        f"cat <<'JSON'\n{payload}\nJSON\n",
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
    wt = fx.worktree("wt-fresh", "just-dispatched")
    assert verdict_for(sweep(fx, fake_gh(tmp_path, [])), wt)[0] == "KEEP"


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
    wt = fx.worktree("wt-leased", "landed")
    lease(fx, wt, "--holder", "worker-a", "--reason", "still measuring")
    bindir = fake_gh(tmp_path, MERGED)

    v, why = verdict_for(sweep(fx, bindir), wt)
    assert v == "KEEP" and "LEASED by worker-a" in why

    subprocess.run(["python3", str(TOOL), "--release", str(wt)], capture_output=True, check=True)
    assert verdict_for(sweep(fx, bindir), wt)[0] == "RECLAIM"


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


def test_target_directories_are_left_alone_when_space_is_fine(fx, tmp_path):
    wt = fx.worktree("wt-open", "inflight")
    make_target(wt)
    out = sweep(
        fx,
        fake_gh(tmp_path, [{"number": 18, "state": "OPEN", "headRefName": "inflight", "title": "t"}]),
        "--free-below",
        "0.0001",
    )
    assert (wt / "target").exists()
    assert "target/" not in out or "Not touched" in out


def test_disk_pressure_removes_only_rebuildable_output(fx, tmp_path):
    wt = fx.worktree("wt-open", "inflight")
    t = make_target(wt)
    out = sweep(
        fx,
        fake_gh(tmp_path, [{"number": 19, "state": "OPEN", "headRefName": "inflight", "title": "t"}]),
        "--free-below",
        "999999",
        "--apply",
        "--targets-only",
    )
    assert "disk-pressure mode" in out
    assert not t.exists()
    assert wt.exists(), "--targets-only must never touch a worktree"
    assert (wt / "README").exists()


def test_disk_pressure_spares_a_leased_tree(fx, tmp_path):
    wt = fx.worktree("wt-open", "inflight")
    t = make_target(wt)
    lease(fx, wt, "--holder", "worker-b")
    sweep(
        fx,
        fake_gh(tmp_path, [{"number": 20, "state": "OPEN", "headRefName": "inflight", "title": "t"}]),
        "--free-below",
        "999999",
        "--apply",
        "--targets-only",
    )
    assert t.exists(), "a claimed tree keeps its build output; a rebuild is not free"


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
