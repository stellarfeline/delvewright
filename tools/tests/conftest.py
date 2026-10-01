"""The tools test-suite's guarantees: no network, a committer, and a reach.

The i18n tool talks to a third-party LLM endpoint, so its tests must prove the
request they *would* send without ever sending one — including when a future
refactor accidentally re-binds the injected HTTP poster. Blocking `urlopen` at the
socket boundary makes that failure mode loud instead of silent (and keeps CI
offline-deterministic).
"""

import os
import pathlib
import subprocess
import sys
import urllib.request

import pytest


@pytest.fixture(autouse=True)
def no_network(monkeypatch):
    def blocked(*args, **kwargs):
        raise AssertionError(
            "a test attempted a real HTTP request — inject/monkeypatch the poster instead"
        )

    monkeypatch.setattr(urllib.request, "urlopen", blocked)


# --- the committer every scratch repository gets, without asking ------------
#
# Seven files in this suite run `git commit` in a repository they build, and
# before this they did it two ways: four set an identity in the environment of
# each call, three ran `git config user.email` per repository — and a file that
# began committing without knowing it had to do either passed on a developer's
# machine and failed on a runner with `empty ident name`. That is the same
# rule written per caller, with the failure falling on whoever writes the next
# caller.
#
# So the identity is here, once, autouse: every test in the suite runs with a
# committer, and a new test that builds a repository and commits gets it for
# free rather than discovering the rule from a red runner. The two config
# variables are pointed at nothing in the same breath and for the opposite
# reason — what the machine happens to hold must not decide what a scratch
# repository does, in either direction.
GIT_IDENTITY = {
    "GIT_AUTHOR_NAME": "delvewright-tests",
    "GIT_AUTHOR_EMAIL": "tests@example.invalid",
    "GIT_COMMITTER_NAME": "delvewright-tests",
    "GIT_COMMITTER_EMAIL": "tests@example.invalid",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_SYSTEM": os.devnull,
}


@pytest.fixture(autouse=True)
def git_identity(monkeypatch):
    for key, value in GIT_IDENTITY.items():
        monkeypatch.setenv(key, value)


# --- which CI job runs which file, and the reach the narrow job is held to ---
#
# Two required jobs run this suite (`.github/workflows/ci.yml`), each reading one
# group of `.github/ci-reach.toml` through `DW_CI_REACH_GROUP`:
#
# * `tool-tests-tree` runs the files in WHOLE_TREE: each reads the whole tracked
#   tree (an `ls-files` sweep, every Markdown file, every JSON document), the
#   creator plugin under `.claude/`, or a document outside the engine's own
#   reference, so its group's glob is `**`.
# * `tool-tests` runs every other file, and its group lists only the paths those
#   files read. The complement is taken here, so a new test file lands in the
#   narrow job by default and is held to that job's reach by the guard below.
#
# With no group set (a developer's `pytest tools/tests`), every file runs and no
# guard is installed.
REPO = pathlib.Path(__file__).resolve().parents[2]
TESTS = pathlib.Path(__file__).resolve().parent
REACH_ENV = "DW_CI_REACH_GROUP"
TREE_GROUP = "tool-tests-tree"
NARROW_GROUP = "tool-tests"
WHOLE_TREE = frozenset(
    {
        # every tracked file, through `git ls-files` or a sweep of the tree
        "test_check_ci_reach.py",
        "test_check_doc_dupes.py",
        "test_check_eol_attributes.py",
        "test_check_json_canonical.py",
        "test_check_pins.py",
        "test_check_python_shell_newlines.py",
        "test_check_release_publish_gate.py",
        "test_check_shell_bash32.py",
        "test_check_shell_redirect_dirs.py",
        "test_mdtable.py",
        "test_server_heap.py",
        "test_version_sites.py",
        # the creator plugin; `check-skill-page.py` also holds every engine path
        # the page names to the tracked tree, which can be any path
        "test_check_skill_page.py",
        "test_check_toolchain.py",
        "test_fetch_delvec.py",
        "test_release_tags.py",
        # documents outside `docs/reference/compiler.md`
        "test_check_demo_levels.py",
        "test_check_numbered_doc_index.py",
        "test_check_stated_counts.py",
    }
)


def _group(config) -> str | None:
    group = os.environ.get(REACH_ENV) or None
    if group not in (None, TREE_GROUP, NARROW_GROUP):
        raise pytest.UsageError(
            f"{REACH_ENV}={group!r}: this suite is split between {TREE_GROUP!r} and "
            f"{NARROW_GROUP!r}, and runs whole with the variable unset"
        )
    return group


def pytest_ignore_collect(collection_path, config):
    group = _group(config)
    path = pathlib.Path(collection_path)
    if group is None or path.parent != TESTS or not path.name.startswith("test_"):
        return None
    listed = path.name in WHOLE_TREE
    return not listed if group == TREE_GROUP else listed


def pytest_collection_modifyitems(config, items):
    # A file named on the command line is collected without asking
    # `pytest_ignore_collect`; the split still decides whether it runs.
    group = _group(config)
    if group is None:
        return
    keep, drop = [], []
    for item in items:
        path = pathlib.Path(str(item.path))
        listed = path.name in WHOLE_TREE
        mine = listed if group == TREE_GROUP else not listed
        (keep if path.parent != TESTS or mine else drop).append(item)
    if drop:
        config.hook.pytest_deselected(items=drop)
        items[:] = keep


_GUARD: dict = {"active": False}
_WATCHED = frozenset({"open", "os.listdir", "os.scandir", "subprocess.Popen"})
_TREE_WIDE_GIT = frozenset({"ls-files", "ls-tree", "grep", "archive"})


def _audit(event, args):
    if event not in _WATCHED or not _GUARD["active"]:
        return
    g = _GUARD
    if event == "subprocess.Popen":
        g["children"] += 1
        argv = args[1]
        if isinstance(argv, (str, bytes)) or not argv:
            return
        argv = [os.fsdecode(a) for a in argv]
        if os.path.basename(argv[0]) != "git":
            return
        rest, cwd = argv[1:], args[2]
        while len(rest) >= 2 and rest[0] == "-C":
            cwd, rest = rest[1], rest[2:]
        if not rest or rest[0] not in _TREE_WIDE_GIT:
            return
        if cwd is None or os.path.realpath(os.fsdecode(cwd)) != g["root"]:
            return
        specs = rest[rest.index("--") + 1 :] if "--" in rest else []
        literal = [s for s in specs if not any(c in s for c in "*?[:")]
        if not specs or len(literal) != len(specs):
            g["judge"](f"`git {' '.join(rest[:3])}` (the whole tracked tree)", None)
        else:
            for s in literal:
                g["judge"](f"`git {rest[0]}` over {s}", s.rstrip("/"))
        return
    target = args[0] if args else None
    if target is None or isinstance(target, int):
        return
    target = os.fsdecode(target)
    if event == "open" and args[1] is None and not os.path.isabs(target):
        # `os.open` relative to a directory descriptor (`shutil.rmtree` walks this
        # way): the event does not carry the descriptor, so the path cannot be
        # resolved, and is counted rather than guessed.
        g["unresolved"] += 1
        return
    if event != "open" and g["test"] == "collection":
        return  # pytest walking to its arguments, not a test reading the tree
    path = os.path.abspath(target)
    for root in g["roots"]:
        if path == root or path.startswith(root + os.sep):
            rel = path[len(root) + 1 :] if path != root else ""
            break
    else:
        return
    g["judge"](f"{event} {rel or '.'}", rel)


def pytest_configure(config):
    group = _group(config)
    missing = sorted(n for n in WHOLE_TREE if not (TESTS / n).is_file())
    if missing:
        raise pytest.UsageError(
            f"conftest.WHOLE_TREE names {', '.join(missing)}, which tools/tests does "
            f"not carry: a stale entry is a file the job split no longer accounts for"
        )
    if group != NARROW_GROUP:
        return
    sys.path.insert(0, str(REPO / "tools" / "lib"))
    import ci_reach  # noqa: E402  — the one interpreter of the table

    table = ci_reach.load_table()
    inputs = table.all + table.groups[NARROW_GROUP]
    regexes = [ci_reach.glob_regex(i.glob) for i in inputs]
    listed = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "-z"], capture_output=True, text=True, check=True
    ).stdout
    tracked = sorted(p for p in listed.split("\0") if p)
    if not tracked:
        raise pytest.UsageError("git ls-files listed nothing: the reach guard would judge nothing")
    reached = {p: any(r.match(p) for r in regexes) for p in tracked}
    outside_under: dict[str, bool] = {}

    def judge(what: str, rel: str | None) -> None:
        _GUARD["checked"] += 1
        if rel is None:
            bad = not all(reached.values())
        elif rel in reached:
            bad = not reached[rel]
        else:
            # A directory, or a path git does not track (build output, a scratch
            # file): a change in a pull request can only reach the tracked part.
            if rel not in outside_under:
                prefix = rel + "/" if rel else ""
                outside_under[rel] = any(
                    not ok for p, ok in reached.items() if p.startswith(prefix)
                )
            bad = outside_under[rel]
        if bad:
            _GUARD["violations"].setdefault(_GUARD["test"], []).append(what)

    _GUARD.update(
        roots=sorted({str(REPO), os.path.realpath(REPO)}),
        root=os.path.realpath(REPO),
        judge=judge,
        globs=len(inputs),
        tracked=len(tracked),
        checked=0,
        children=0,
        unresolved=0,
        violations={},
        test="collection",
        active=True,
    )
    if not _GUARD.get("hooked"):
        sys.addaudithook(_audit)
        _GUARD["hooked"] = True


@pytest.hookimpl(tryfirst=True)
def pytest_runtest_setup(item):
    # Before any fixture: a module-scoped fixture built for this test is this
    # test's read.
    if _GUARD["active"]:
        _GUARD["test"] = item.nodeid


@pytest.fixture(autouse=True)
def reach_guard(request):
    """In the narrow job, a test that reads a tracked path outside its reach fails."""
    if not _GUARD["active"]:
        yield
        return
    yield
    _GUARD["test"] = "between tests"
    found = _GUARD["violations"].pop(request.node.nodeid, [])
    if found:
        shown = "\n  ".join(sorted(set(found))[:20])
        pytest.fail(
            f"this test read tracked path(s) outside the `{NARROW_GROUP}` group of "
            f".github/ci-reach.toml, so a pull request touching them would not run "
            f"it:\n  {shown}\nAdd the path to that group with the reader in its "
            f"`why`, or list this file in conftest.WHOLE_TREE.",
            pytrace=False,
        )


def pytest_terminal_summary(terminalreporter, exitstatus, config):
    if not _GUARD["active"]:
        return
    stray = {k: v for k, v in _GUARD["violations"].items() if v}
    terminalreporter.write_line(
        f"reach guard ({NARROW_GROUP}): {_GUARD['checked']} read(s) of the repository "
        f"judged against {_GUARD['globs']} glob(s) over {_GUARD['tracked']} tracked "
        f"file(s); not seen: reads inside {_GUARD['children']} child process(es), "
        f"{_GUARD['unresolved']} descriptor-relative open(s), and existence checks"
    )
    for where, found in sorted(stray.items()):
        terminalreporter.write_line(f"reach guard: outside the reach during {where}: {sorted(set(found))[:20]}")


def pytest_sessionfinish(session, exitstatus):
    if _GUARD["active"] and any(_GUARD["violations"].values()) and session.exitstatus == 0:
        session.exitstatus = 1
