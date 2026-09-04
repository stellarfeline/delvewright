"""Guards for `tools/check-advisories.py`.

The property that matters is not "it reports advisories" — it is that it cannot
report a clean tree when the audit never ran. That is the exact failure measured
on this repository: `npm audit` timed out against the registry's bulk advisory
endpoint, printed `undefined`, and exited 1, so the exit code alone could not
tell a vulnerability from an outage.

Every test here drives the real script against a FAKE audit binary on `PATH`, so
none of them touches the network. `tools/tests/conftest.py` blocks `urlopen`; a
subprocess is outside that, which is why the fake is the mechanism rather than a
monkeypatch.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
TOOL = REPO / "tools" / "check-advisories.py"

CLEAN_CARGO = {
    "database": {"advisory-count": 1},
    "lockfile": {"dependency-count": 7},
    "vulnerabilities": {"found": False, "count": 0, "list": []},
    "warnings": {},
}

DIRTY_CARGO = {
    "database": {"advisory-count": 1},
    "lockfile": {"dependency-count": 7},
    "vulnerabilities": {
        "found": True,
        "count": 1,
        "list": [
            {
                "advisory": {"id": "RUSTSEC-2099-0001", "title": "a hole"},
                "package": {"name": "leaky", "version": "0.1.0"},
            }
        ],
    },
    "warnings": {},
}


def fake_binary(tmp_path: Path, name: str, stdout: str, code: int = 0) -> Path:
    p = tmp_path / name
    p.write_text(
        "#!/bin/sh\ncat <<'JSONEOF'\n" + stdout + "\nJSONEOF\nexit " + str(code) + "\n"
    )
    p.chmod(0o755)
    return p


def run(*args: str, env_path: Path | None = None) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    if env_path is not None:
        env["PATH"] = f"{env_path}{os.pathsep}{env['PATH']}"
    # Attempts cost real seconds; the retry itself is asserted separately.
    return subprocess.run(
        [sys.executable, str(TOOL), *args], capture_output=True, text=True, env=env
    )


def repo_with_lockfiles(tmp_path: Path, n: int) -> Path:
    repo = tmp_path / "repo"
    repo.mkdir()
    env = {
        **os.environ,
        "GIT_AUTHOR_NAME": "t",
        "GIT_AUTHOR_EMAIL": "t@t",
        "GIT_COMMITTER_NAME": "t",
        "GIT_COMMITTER_EMAIL": "t@t",
    }
    subprocess.run(["git", "init", "-q", str(repo)], check=True, env=env)
    # One file that is never a lockfile, so a repository with zero lockfiles is
    # still a repository with a commit — the population under test is the
    # lockfiles, not whether git has anything to say.
    (repo / "README").write_text("t\n")
    for i in range(n):
        d = repo / f"w{i}"
        d.mkdir()
        (d / "Cargo.lock").write_text("version = 4\n")
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True, env=env)
    subprocess.run(["git", "-C", str(repo), "commit", "-qm", "t"], check=True, env=env)
    return repo


def test_a_clean_audit_passes_and_states_what_it_examined(tmp_path):
    repo = repo_with_lockfiles(tmp_path, 3)
    fake = fake_binary(tmp_path, "cargo-audit", json.dumps(CLEAN_CARGO))
    r = run("--cargo", "--repo", str(repo), "--cargo-audit", str(fake))
    assert r.returncode == 0, r.stderr
    assert "3 lockfile(s) audited over 21 resolved crate dependency(ies)" in r.stdout


def test_a_vulnerability_names_the_advisory_and_the_lockfile(tmp_path):
    repo = repo_with_lockfiles(tmp_path, 1)
    fake = fake_binary(tmp_path, "cargo-audit", json.dumps(DIRTY_CARGO), code=1)
    r = run("--cargo", "--repo", str(repo), "--cargo-audit", str(fake))
    assert r.returncode == 1
    assert "RUSTSEC-2099-0001" in r.stderr
    assert "w0/Cargo.lock" in r.stderr


def test_an_audit_that_did_not_answer_is_not_an_audit_that_found_nothing(tmp_path):
    """The measured failure: `undefined` on stdout, exit 1, from a registry
    timeout. A reader of the exit code alone cannot tell that from a finding."""
    repo = repo_with_lockfiles(tmp_path, 1)
    fake = fake_binary(tmp_path, "cargo-audit", "undefined", code=1)
    r = run("--cargo", "--repo", str(repo), "--cargo-audit", str(fake))
    assert r.returncode == 1
    assert "did not answer" in r.stderr
    assert "attempt(s)" in r.stderr


def test_the_transport_is_retried_before_the_refusal(tmp_path):
    """A retry of the FETCH, never of the verdict — so it is asserted that the
    attempts happen, and separately that the verdict they produce is used."""
    repo = repo_with_lockfiles(tmp_path, 1)
    counter = tmp_path / "attempts"
    fake = tmp_path / "cargo-audit"
    fake.write_text(
        "#!/bin/sh\n"
        f"echo x >> {counter}\n"
        f"n=$(wc -l < {counter})\n"
        'if [ "$n" -lt 2 ]; then echo undefined; exit 1; fi\n'
        "cat <<'JSONEOF'\n" + json.dumps(CLEAN_CARGO) + "\nJSONEOF\n"
    )
    fake.chmod(0o755)
    r = run("--cargo", "--repo", str(repo), "--cargo-audit", str(fake))
    assert r.returncode == 0, r.stderr
    assert counter.read_text().count("x") == 2


def test_auditing_zero_lockfiles_is_a_red(tmp_path):
    repo = repo_with_lockfiles(tmp_path, 0)
    fake = fake_binary(tmp_path, "cargo-audit", json.dumps(CLEAN_CARGO))
    r = run("--cargo", "--repo", str(repo), "--cargo-audit", str(fake))
    assert r.returncode == 1
    assert "ZERO cargo lockfiles" in r.stderr


def npm_report(**counts: int) -> str:
    full = {s: 0 for s in ("info", "low", "moderate", "high", "critical")}
    full.update(counts)
    return json.dumps(
        {"metadata": {"vulnerabilities": full, "dependencies": {"total": 93}}}
    )


def npm_project(tmp_path: Path) -> Path:
    d = tmp_path / "proj"
    d.mkdir()
    (d / "package-lock.json").write_text("{}\n")
    return d


def test_npm_fails_at_or_above_the_floor(tmp_path):
    proj = npm_project(tmp_path)
    fake_binary(tmp_path, "npm", npm_report(high=2), code=1)
    r = run("--npm", "--dir", str(proj), env_path=tmp_path)
    assert r.returncode == 1
    assert "2 high" in r.stderr


def test_what_sits_below_the_floor_is_printed_rather_than_forgiven(tmp_path):
    """A stated threshold that does not say what is under it is an ignore list
    with better manners."""
    proj = npm_project(tmp_path)
    fake_binary(tmp_path, "npm", npm_report(moderate=6), code=1)
    r = run("--npm", "--dir", str(proj), env_path=tmp_path)
    assert r.returncode == 0, r.stderr
    assert "moderate=6" in r.stdout
    assert "6 moderate" in r.stdout
    assert "not a thing this gate has forgiven" in r.stdout


def test_an_npm_report_this_reader_cannot_read_is_refused(tmp_path):
    proj = npm_project(tmp_path)
    fake_binary(tmp_path, "npm", '{"metadata": {}}')
    r = run("--npm", "--dir", str(proj), env_path=tmp_path)
    assert r.returncode == 1
    assert "no per-severity counts" in r.stderr


def test_both_arms_are_invoked_by_ci():
    """UNRUN is the vacuity mode this file cannot see from inside: a correct gate
    nothing calls. Binding: the two `run:` sites in `ci.yml`, one per arm."""
    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    assert "tools/check-advisories.py --cargo" in ci
    assert "tools/check-advisories.py --npm" in ci
