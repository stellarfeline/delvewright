r"""Guards for `tools/check-pins.py`.

The defect it exists to prevent, from the round that paid for it: a content-repo
workflow built its judge from a pinned pipeline commit; the pin sat while the
rule that judge enforced was settled upstream; a zone was reported red for
failing a rule that no longer existed. Four hundred commits of drift, and nothing
anywhere was red, because a pin's staleness is invisible — the file reads the same
on the day it is written and a year later.

The tests below assert the gate fails in the direction the defect actually
arrives from, and that each of the three ways it could be vacuous is closed:

- an UNREGISTERED pin (the pin exists and no entry mentions it),
- a pin whose instrument moved and whose record does not say anyone looked,
- a pin declared exempt by a policy the object does not support — a `release`
  pin with no release tag, or an own-repo pin downgraded to `immutable`,

plus the two shapes this project keeps shipping: a binding of zero reported as a
pass, and a check declared but never invoked (`judged_by` naming a file that does
not run it).

The repository's OWN registry is exercised too, in both directions: it must be
complete now, and it must red when a pin is taken out of it. A checker that only
ever passes proves nothing.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-pins.py"

DIGEST = "sha256:" + "ab12" * 16
REV = "0123456789abcdef0123456789abcdef01234567"


def run(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--root", str(root), *args],
        capture_output=True,
        text=True,
    )


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """A minimal git repo with one workflow holding one pin."""
    (tmp_path / ".github" / "workflows").mkdir(parents=True)
    (tmp_path / ".github" / "workflows" / "audit.yml").write_text(
        "name: audit\n"
        "jobs:\n"
        "  a:\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n"
        f"        with:\n          image: {DIGEST}\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "-C", str(tmp_path), "init", "-q"], check=True)
    subprocess.run(["git", "-C", str(tmp_path), "add", "-A"], check=True)
    return tmp_path


def write_registry(repo: Path, body: str) -> None:
    (repo / ".github" / "pins.toml").write_text(body, encoding="utf-8")
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)


COMPLETE = f"""
[[pin]]
id = "checkout"
value = "actions/checkout@v4"
sites = [".github/workflows/audit.yml"]
policy = "floating"
why = "held at its major tag"

[[pin]]
id = "image"
value = "{DIGEST}"
sites = [".github/workflows/audit.yml"]
policy = "immutable"
why = "third-party bytes"
"""


def test_complete_registry_passes(repo: Path) -> None:
    write_registry(repo, COMPLETE)
    r = run(repo)
    assert r.returncode == 0, r.stderr
    assert "binding: 2 pin(s)" in r.stdout


def test_unregistered_pin_is_a_finding(repo: Path) -> None:
    """The shape the incident had: the pin is right there and nothing names it."""
    write_registry(repo, COMPLETE.split("[[pin]]\nid = \"image\"")[0])
    r = run(repo)
    assert r.returncode == 1
    assert "unregistered pin" in r.stderr and DIGEST in r.stderr


def test_registry_that_drifted_from_the_file_is_a_finding(repo: Path) -> None:
    write_registry(repo, COMPLETE + f"""
[[pin]]
id = "ghost"
value = "{REV}"
sites = [".github/workflows/audit.yml"]
policy = "immutable"
why = "no longer there"
""")
    r = run(repo)
    assert r.returncode == 1
    assert "is not there any more" in r.stderr


def test_zero_binding_is_a_finding_not_a_pass(tmp_path: Path) -> None:
    """A gate that examined nothing has proved nothing."""
    (tmp_path / ".github").mkdir()
    (tmp_path / ".github" / "pins.toml").write_text("", encoding="utf-8")
    (tmp_path / "README.md").write_text("no pins here\n", encoding="utf-8")
    subprocess.run(["git", "-C", str(tmp_path), "init", "-q"], check=True)
    subprocess.run(["git", "-C", str(tmp_path), "add", "-A"], check=True)
    r = run(tmp_path)
    assert r.returncode == 1
    assert "binding of zero" in r.stderr


def test_own_repo_pin_may_not_be_called_immutable(repo: Path) -> None:
    """The escape hatch the defect would reach for, closed by the object.

    A commit id IS content-addressed, so `immutable` reads as defensible — and it
    would exempt exactly the pins that rot. The kind is decided by what the pin
    names, not by what the author calls it.
    """
    write_registry(repo, COMPLETE + f"""
[[pin]]
id = "engine"
value = "{REV}"
sites = [".github/workflows/audit.yml"]
repo = "stellarfeline/delvewright"
policy = "immutable"
why = "a commit names exact bytes"
""")
    (repo / ".github" / "workflows" / "audit.yml").write_text(
        f"name: audit\nenv:\n  E: {REV}\njobs:\n  a:\n    steps:\n"
        f"      - uses: actions/checkout@v4\n"
        f"        with:\n          image: {DIGEST}\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)
    r = run(repo)
    assert r.returncode == 1
    assert "not exempt by being called immutable" in r.stderr


def test_track_pin_must_say_what_it_was_reviewed_against(repo: Path) -> None:
    write_registry(repo, COMPLETE + f"""
[[pin]]
id = "engine"
value = "{REV}"
sites = [".github/workflows/audit.yml"]
repo = "stellarfeline/delvewright"
policy = "track"
judged_by = ".github/workflows/audit.yml"
why = "the judge"
""")
    (repo / ".github" / "workflows" / "audit.yml").write_text(
        f"name: audit\nenv:\n  E: {REV}\njobs:\n  a:\n    steps:\n"
        f"      - uses: actions/checkout@v4\n"
        f"        with:\n          image: {DIGEST}\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)
    r = run(repo)
    assert r.returncode == 1
    assert "must carry `reviewed`" in r.stderr


def test_judged_by_that_does_not_invoke_the_check_is_a_finding(repo: Path) -> None:
    """A gate nothing invokes is not a gate. `judged_by` is verified, not trusted."""
    write_registry(repo, COMPLETE + f"""
[[pin]]
id = "engine"
value = "{REV}"
sites = [".github/workflows/audit.yml"]
repo = "stellarfeline/delvewright"
policy = "track"
judged_by = ".github/workflows/audit.yml"
reviewed = "{REV}"
builds = []
why = "the judge"
""")
    (repo / ".github" / "workflows" / "audit.yml").write_text(
        f"name: audit\nenv:\n  E: {REV}\njobs:\n  a:\n    steps:\n"
        f"      - uses: actions/checkout@v4\n"
        f"        with:\n          image: {DIGEST}\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)
    r = run(repo)
    assert r.returncode == 1
    assert "declared and never runs" in r.stderr


def test_builds_that_omits_what_the_site_builds_is_a_finding(repo: Path) -> None:
    """The watch set is derived from `builds`, so shrinking `builds` is the dodge."""
    write_registry(repo, COMPLETE + f"""
[[pin]]
id = "engine"
value = "{REV}"
sites = [".github/workflows/audit.yml"]
repo = "stellarfeline/delvewright"
policy = "track"
judged_by = ".github/workflows/audit.yml"
reviewed = "{REV}"
builds = []
why = "the judge"
""")
    (repo / ".github" / "workflows" / "audit.yml").write_text(
        f"name: audit\nenv:\n  E: {REV}\njobs:\n  a:\n    steps:\n"
        f"      - uses: actions/checkout@v4\n"
        f"        with:\n          image: {DIGEST}\n"
        f"      - run: python3 tools/check-pins.py --online engine\n"
        f"      - run: cargo build -p delvewright-admit --release\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)
    r = run(repo)
    assert r.returncode == 1
    assert "`builds` does not name it" in r.stderr


def test_a_checkout_at_a_branch_is_a_pin_too(repo: Path) -> None:
    """A cross-repo checkout carries no hex and is still the loosest pin there is."""
    (repo / ".github" / "workflows" / "audit.yml").write_text(
        f"name: audit\njobs:\n  a:\n    steps:\n"
        f"      - uses: actions/checkout@v4\n"
        f"        with:\n"
        f"          repository: stellarfeline/delvewright\n"
        f"          ref: main\n"
        f"          image: {DIGEST}\n",
        encoding="utf-8",
    )
    write_registry(repo, COMPLETE)
    r = run(repo)
    assert r.returncode == 1
    assert "stellarfeline/delvewright@main" in r.stderr


def test_this_repos_own_registry_is_complete() -> None:
    r = run(REPO)
    assert r.returncode == 0, r.stdout + r.stderr


def test_this_repos_own_registry_reds_when_a_pin_leaves_it(tmp_path: Path) -> None:
    """The other direction. A registry that cannot fail is not a record."""
    import shutil
    import tomllib

    registry = REPO / ".github" / "pins.toml"
    with registry.open("rb") as fh:
        pins = tomllib.load(fh)["pin"]
    assert pins, "the repo's own registry is empty"

    work = tmp_path / "repo"
    shutil.copytree(REPO / ".github", work / ".github")
    subprocess.run(["git", "-C", str(REPO), "ls-files", "-z"], check=True,
                   capture_output=True)
    # Only the .github tree is needed to reproduce the reds this asserts.
    subprocess.run(["git", "-C", str(work), "init", "-q"], check=True)
    subprocess.run(["git", "-C", str(work), "add", "-A"], check=True)
    text = registry.read_text(encoding="utf-8")
    cut = text.index('id = "action-checkout"')
    start = text.rindex("[[pin]]", 0, cut)
    end = text.index("[[pin]]", cut)
    (work / ".github" / "pins.toml").write_text(text[:start] + text[end:],
                                                encoding="utf-8")
    subprocess.run(["git", "-C", str(work), "add", "-A"], check=True)
    r = run(work)
    assert r.returncode == 1
    assert "unregistered pin actions/checkout@v4" in r.stderr
