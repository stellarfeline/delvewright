r"""Guard: a `dsl_version` bumped past an unpublished one is a red, not a silence.

WHAT WENT WRONG, AND WHAT THIS PINS

`delvewright-dsl`'s package version is the `dsl_version` (ADR-0024), so the number
a document declares is only resolvable if the crate reached crates.io.
`.github/workflows/dsl-crate-publish.yml` is what puts it there, and it stopped:
run 34096612106 asked for its `crates-io` approval and never got one, the
workflow's shared concurrency group then held every later run in the single
pending slot GitHub keeps per group, and 21 successors were cancelled with zero
jobs. `0.21.2` was bumped on `main` inside that window and never published;
`0.22.0` replaced it. No check anywhere had an opinion.

`tools/check-dsl-version-published.py` is the opinion: when a change moves the
number from X to Y, X must be on crates.io. This suite is what proves it BINDS —
that the red arrives on the shape the incident had, and that the green is not the
checker failing to look.

OFFLINE. The registry is a local sparse index over `DW_CRATES_INDEX`
(`tools/lib/crates_index.py`'s override, the shape `test_publish_gate_order.py`
established), because whether a given `delvewright-dsl` version is on the real
crates.io changes with this project's own release history and a test must not
depend on that staying true. The trees are scratch git repositories holding
nothing but a `versions.toml`, which is the only file the gate reads.

THE PERTURBATION THE ROUND IS ABOUT is `test_red_when_the_retired_version_was_
never_published`: the index serves the crate but not the number being retired —
exactly `0.21.2`'s state on the day `0.22.0` landed — and the gate must exit 1.
Its control is `test_green_when_the_retired_version_is_published`, identical in
every respect but the one row in the index.
"""

from __future__ import annotations

import http.server
import json
import os
import subprocess
import threading
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-dsl-version-published.py"

# What the fake index serves for `delvewright-dsl`. The bind-test crate is always
# served: a run where it is missing is a different failure (exit 2), and
# `test_exit_2_when_the_lookup_itself_is_unbound` is where that is asserted.
BIND_ROW = {"name": "serde", "vers": "1.0.0", "cksum": "0" * 64}


def _versions_toml(dsl_version: str) -> str:
    """The one key the gate reads, in a file shaped like the real registry."""
    return (
        "[engine]\n"
        'version = "1.0.0"\n'
        f'dsl_crate_version = "{dsl_version}"\n'
    )


def _repo_with_bump(root: Path, base_version: str, tree_version: str) -> Path:
    """A git repo whose HEAD retires `base_version` in favour of `tree_version`.

    `origin/main` is a real remote-tracking ref rather than a branch name the gate
    happens to accept: `resolve_base` looks up exactly what CI gives it.
    """
    repo = root / "tree"
    repo.mkdir(parents=True)
    run = lambda *args: subprocess.run(  # noqa: E731
        ["git", "-C", str(repo), *args], check=True, capture_output=True
    )
    run("init", "-q", "-b", "main")
    run("config", "user.email", "t@example.com")
    run("config", "user.name", "t")
    (repo / "versions.toml").write_text(_versions_toml(base_version), encoding="utf-8")
    run("add", "versions.toml")
    run("commit", "-qm", "base")
    run("update-ref", "refs/remotes/origin/main", "HEAD")
    if tree_version != base_version:
        (repo / "versions.toml").write_text(_versions_toml(tree_version), encoding="utf-8")
        run("commit", "-qam", "bump")
    return repo


def _run(repo: Path, index: str, *extra: str) -> subprocess.CompletedProcess[str]:
    # `no_proxy` is not tidiness: a proxy configured in the developer's
    # environment would send the loopback fixture index somewhere else, and the
    # answer that comes back — "not published" — is the one this suite is trying
    # to tell apart from the real thing.
    env = {
        **os.environ,
        "DW_CRATES_INDEX": index,
        "no_proxy": "*",
        "NO_PROXY": "*",
    }
    return subprocess.run(
        ["python3", str(CHECKER), "--repo", str(repo), *extra],
        capture_output=True,
        text=True,
        env=env,
    )


def _handler(served: list[str]):
    """A sparse index serving `delvewright-dsl` at exactly `served`."""

    class _FakeIndex(http.server.BaseHTTPRequestHandler):
        def do_GET(self) -> None:  # noqa: N802 (stdlib handler signature)
            if self.path == "/se/rd/serde":
                rows = [BIND_ROW]
            elif self.path == "/de/lv/delvewright-dsl" and served:
                rows = [
                    {"name": "delvewright-dsl", "vers": v, "cksum": f"{i + 1:064d}"}
                    for i, v in enumerate(served)
                ]
            else:
                self.send_response(404)
                self.end_headers()
                return
            body = b"".join(json.dumps(r).encode() + b"\n" for r in rows)
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_args: object) -> None:
            pass

    return _FakeIndex


def _serve(served: list[str], monkeypatch: pytest.MonkeyPatch | None = None):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _handler(served))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, thread, f"http://127.0.0.1:{server.server_port}"


@pytest.fixture
def index_with():
    """`index_with([...])` -> the base URL of an index serving those versions."""
    started: list[tuple[http.server.ThreadingHTTPServer, threading.Thread]] = []

    def make(served: list[str]) -> str:
        server, thread, url = _serve(served)
        started.append((server, thread))
        return url

    try:
        yield make
    finally:
        for server, thread in started:
            server.shutdown()
            thread.join(timeout=5)


# --------------------------------------------------------------------------
# the perturbation this round is about, and its control
# --------------------------------------------------------------------------
def test_red_when_the_retired_version_was_never_published(tmp_path, index_with):
    """0.21.2 -> 0.22.0 with 0.21.2 absent: the shape the incident had."""
    repo = _repo_with_bump(tmp_path, "0.21.2", "0.22.0")
    result = _run(repo, index_with(["0.21.0", "0.21.1"]))
    assert result.returncode == 1, result.stdout + result.stderr
    assert "0 of 1 retired version(s) on crates.io" in result.stderr
    assert "0.21.2" in result.stderr
    # The remedy is named, and it is the act that actually publishes it.
    assert ".github/workflows/dsl-crate-publish.yml" in result.stderr


def test_green_when_the_retired_version_is_published(tmp_path, index_with):
    """The same bump, differing only in the one index row: 0.21.2 is served."""
    repo = _repo_with_bump(tmp_path, "0.21.2", "0.22.0")
    result = _run(repo, index_with(["0.21.0", "0.21.1", "0.21.2"]))
    assert result.returncode == 0, result.stdout + result.stderr
    assert "1 of 1 retired version(s) on crates.io" in result.stdout


# --------------------------------------------------------------------------
# the states that are not a bump — a zero binding must say what it read
# --------------------------------------------------------------------------
def test_a_change_that_does_not_move_the_number_retires_nothing(tmp_path, index_with):
    repo = _repo_with_bump(tmp_path, "0.22.0", "0.22.0")
    result = _run(repo, index_with([]))
    assert result.returncode == 0, result.stdout + result.stderr
    assert "0 version(s) retired by this change" in result.stdout
    # A zero that names both numbers it compared cannot be a scan that bound to
    # nothing: the base was read, and it agreed.
    assert "states 0.22.0" in result.stdout


def test_a_branch_behind_the_base_retires_nothing(tmp_path, index_with):
    """The tree's number is OLDER than the base's — a stale branch, not a bump.

    Judging the base's own number here would red every branch cut before a bump
    landed on `main`, for a publish that belongs to `main` and is judged where it
    landed.
    """
    repo = _repo_with_bump(tmp_path, "0.21.2", "0.22.0")
    # `origin/main` now states 0.21.2 and HEAD states 0.22.0; put the working
    # tree back behind BOTH, which is what a branch cut before the bump looks
    # like once `origin/main` has moved past it.
    (repo / "versions.toml").write_text(_versions_toml("0.21.0"), encoding="utf-8")
    subprocess.run(
        ["git", "-C", str(repo), "update-ref", "refs/remotes/origin/main", "HEAD"],
        check=True,
        capture_output=True,
    )
    result = _run(repo, index_with([]))
    assert result.returncode == 0, result.stdout + result.stderr
    assert "BEHIND" in result.stdout


# --------------------------------------------------------------------------
# the ways the comparison can fail to be made at all — exit 2, never a pass
# --------------------------------------------------------------------------
def test_exit_2_when_the_base_does_not_resolve(tmp_path, index_with):
    repo = _repo_with_bump(tmp_path, "0.21.2", "0.22.0")
    result = _run(repo, index_with([]), "--base", "origin/nonexistent")
    assert result.returncode == 2, result.stdout + result.stderr
    assert "does not resolve to a commit" in result.stderr


def test_exit_2_when_the_lookup_itself_is_unbound(tmp_path):
    """An index that cannot answer for a version everyone knows exists.

    Without this the gate would read every registry outage as "the retired
    version was never published" and red a bump for a reason that is not true —
    the unbound-gate class, pointing the other way.
    """
    class _Nothing(http.server.BaseHTTPRequestHandler):
        """An index that 404s everything, `serde` included."""

        def do_GET(self) -> None:  # noqa: N802
            self.send_response(404)
            self.end_headers()

        def log_message(self, *_args: object) -> None:
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _Nothing)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        repo = _repo_with_bump(tmp_path, "0.21.2", "0.22.0")
        result = _run(repo, f"http://127.0.0.1:{server.server_port}")
        assert result.returncode == 2, result.stdout + result.stderr
        assert "serde 1.0.0" in result.stderr
    finally:
        server.shutdown()
        thread.join(timeout=5)


def test_exit_2_when_the_number_is_not_where_it_is_read_from(tmp_path, index_with):
    repo = _repo_with_bump(tmp_path, "0.21.2", "0.22.0")
    (repo / "versions.toml").write_text('[engine]\nversion = "1.0.0"\n', encoding="utf-8")
    result = _run(repo, index_with([]))
    assert result.returncode == 2, result.stdout + result.stderr
    assert "dsl_crate_version" in result.stderr
