r"""Guards for `tools/check-json-canonical.py` — the canonical-form sweep.

The defect this replaces was not a wrong check. It was a check that looked at
two directories somebody named by hand in `ci.yml` and was green about the other
ninety-five files, fifty of which were not canonical. So the property under test
here is **the derivation**, not the formatter: `delvec fmt` proves its own
correctness in `crates/dsl/src/fmt.rs`, and re-testing it here would be a second
measurement to get wrong.

The load-bearing test is `test_a_new_directory_is_swept_with_no_edit_here`. It
builds a throwaway git repository containing a directory this repository has
never heard of and asserts the file inside it reached the formatter — with the
checker's own source untouched. That is the whole claim of the change, and it is
demonstrated rather than asserted.

`delvec` is stubbed. A stub is the right instrument precisely because the
question is *which paths were handed over*, and a stub is the only thing that can
answer it exactly; using the real binary would test the formatter again and say
nothing about the set. The stub also lets the red direction be produced on
demand, which a tree that is currently green cannot do.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-json-canonical.py"

# Copied from the checker on purpose, not imported: if a rename moves the
# exemption, these fixtures should stop describing it and the tests should
# notice, rather than silently following it wherever it went.
GOLDEN_DIR = "crates/compiler/tests/golden"
BINDING_FILE = "crates/compiler/src/view/scene.rs"

STUB = """#!/usr/bin/env python3
import os, sys

argv = sys.argv[1:]
assert argv[:2] == ["fmt", "--check"], argv
with open(os.environ["STUB_LOG"], "a") as f:
    for p in argv[2:]:
        f.write(p + "\\n")
if int(os.environ.get("STUB_EXIT", "0")):
    print("DW0773 [error] fmt %s: not in canonical form" % argv[2], file=sys.stderr)
sys.exit(int(os.environ.get("STUB_EXIT", "0")))
"""


def write(root: Path, rel: str, text: str) -> Path:
    p = root / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
    return p


@pytest.fixture
def fake_repo(tmp_path: Path) -> Path:
    """A throwaway repository with the shape the checker reasons about."""
    root = tmp_path / "repo"
    root.mkdir()
    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    # The pin that admits the one exemption, and a golden under it.
    write(
        root,
        BINDING_FILE,
        "fn golden_scene_matches() {}\nfn every_golden_is_emitter_output() {}\n",
    )
    write(root, f"{GOLDEN_DIR}/view/spawn.json", '{"b":1,"a":2}\n')
    write(root, "crates/dsl/fixtures/valid/world.json", '{"a": 1}\n')
    subprocess.run(["git", "add", "-A"], cwd=root, check=True)
    return root


def run(repo: Path, tmp_path: Path, exit_code: int = 0) -> tuple[int, list[str], str]:
    """Run the checker against `repo` with a stubbed delvec; return the paths it swept."""
    stub = tmp_path / "delvec-stub.py"
    stub.write_text(STUB, encoding="utf-8")
    stub.chmod(0o755)
    log = tmp_path / "swept.txt"
    log.write_text("", encoding="utf-8")
    r = subprocess.run(
        [
            sys.executable,
            str(CHECKER),
            "--repo",
            str(repo),
            "--delvec",
            str(stub),
        ],
        capture_output=True,
        text=True,
        env={
            **os.environ,
            "STUB_LOG": str(log),
            "STUB_EXIT": str(exit_code),
        },
    )
    return r.returncode, log.read_text(encoding="utf-8").split(), r.stdout + r.stderr


def test_a_new_directory_is_swept_with_no_edit_here(fake_repo: Path, tmp_path: Path):
    """THE claim. A directory nobody named is swept the moment it is committed."""
    before = CHECKER.read_bytes()

    code, swept, out = run(fake_repo, tmp_path)
    assert code == 0, out
    assert "brand-new-area/thing.json" not in swept

    # A month later, someone adds a whole area of authored JSON. Nothing about
    # the checker, the workflow, or any list is touched.
    write(fake_repo, "brand-new-area/thing.json", '{"z":1,"a":2}\n')
    write(fake_repo, "brand-new-area/nested/deep/other.json", "{}\n")
    subprocess.run(["git", "add", "-A"], cwd=fake_repo, check=True)

    code, swept, out = run(fake_repo, tmp_path)
    assert code == 0, out
    assert "brand-new-area/thing.json" in swept
    assert "brand-new-area/nested/deep/other.json" in swept
    assert CHECKER.read_bytes() == before, "the check was edited to see the new files"


def test_an_untracked_file_is_not_swept(fake_repo: Path, tmp_path: Path):
    """The exclusions are properties, not names: `target/` is absent because git
    does not track it, so nothing here has to know what a build directory is
    called."""
    write(fake_repo, "target/debug/build/whatever.json", "{}\n")
    write(fake_repo, "harness/node_modules/pkg/package.json", "{}\n")
    code, swept, out = run(fake_repo, tmp_path)
    assert code == 0, out
    assert not [p for p in swept if p.startswith(("target/", "harness/node_modules/"))]


def test_the_golden_directory_is_the_only_exemption(fake_repo: Path, tmp_path: Path):
    code, swept, out = run(fake_repo, tmp_path)
    assert code == 0, out
    assert f"{GOLDEN_DIR}/view/spawn.json" not in swept
    assert "crates/dsl/fixtures/valid/world.json" in swept
    assert "1 of 2 tracked JSON document(s) swept; 1 exempt" in out


def test_a_document_out_of_canonical_form_reds(fake_repo: Path, tmp_path: Path):
    """The red direction, produced rather than described."""
    code, swept, out = run(fake_repo, tmp_path, exit_code=1)
    assert code == 1
    assert "not in canonical form" in out
    assert swept, "reporting red while having swept nothing would be a worse pass"


def test_a_stale_exemption_is_a_red(fake_repo: Path, tmp_path: Path):
    """An exclusion whose directory has gone is measuring nothing — the shape an
    exclusion rots into after a rename, green forever."""
    (fake_repo / GOLDEN_DIR / "view" / "spawn.json").unlink()
    subprocess.run(["git", "add", "-A"], cwd=fake_repo, check=True)
    code, _, out = run(fake_repo, tmp_path)
    assert code == 1
    assert "matches ZERO tracked files" in out


def test_deleting_the_pin_reds_instead_of_unbinding_the_exemption(
    fake_repo: Path, tmp_path: Path
):
    """The exemption is a POINTER. Delete what it points at and it must red here,
    not quietly become an ordinary name in a list."""
    write(fake_repo, BINDING_FILE, "fn golden_scene_matches() {}\n")
    subprocess.run(["git", "add", "-A"], cwd=fake_repo, check=True)
    code, _, out = run(fake_repo, tmp_path)
    assert code == 1
    assert "every_golden_is_emitter_output" in out
    assert "bound to nothing" in out


def test_a_repository_with_no_json_is_a_red_not_a_pass(tmp_path: Path):
    root = tmp_path / "empty"
    root.mkdir()
    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    code, _, out = run(root, tmp_path)
    assert code == 1
    assert "vacuous pass" in out


def test_the_real_repository_is_swept_against_its_whole_population():
    """The denominator is git's answer, and the sweep is nearly all of it. Guards
    the direction that actually happens — a future exclusion quietly shrinking
    what is examined — without pinning a count that legitimately grows."""
    tracked = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "-z", "--", "*.json"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.split("\0")
    tracked = [p for p in tracked if p]
    exempt = [p for p in tracked if p.startswith(GOLDEN_DIR + "/")]
    assert tracked, "git tracks no JSON at all — the population is the denominator"
    assert exempt, "the one exemption matches nothing"
    assert len(exempt) <= 5, (
        "more than a handful of this repository's JSON is outside the sweep; the "
        "hand-named-roots defect has come back"
    )
    # And the fixture roots the old hand-written CI step named are still in it.
    assert any(p.startswith("crates/dsl/fixtures/") for p in tracked)
    assert any(p.startswith("gallery/") for p in tracked)


def test_every_tracked_json_parses_as_json():
    """Cheap, and it is the half `--check` cannot state: a `DW0770` would name one
    file, while this says the whole population is JSON at all."""
    out = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "-z", "--", "*.json"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    bad = []
    for rel in (p for p in out.split("\0") if p):
        try:
            json.loads((REPO / rel).read_text(encoding="utf-8"))
        except Exception as e:  # noqa: BLE001 — the point is to name every one
            bad.append(f"{rel}: {e}")
    assert not bad, "\n".join(bad)
