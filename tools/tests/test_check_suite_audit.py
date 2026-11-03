"""`tools/ci/check-suite-audit.py`: green on a whole table, red on each defect it names."""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

import pytest

TOOL = pathlib.Path(__file__).resolve().parents[2] / "tools" / "ci" / "check-suite-audit.py"
TABLE = "crates/x/tests/measured/suite-audit.json"

A = "#[test]\nfn a_one() {\n    assert_eq!(1, 1);\n    assert!(\"DW0101\".len() == 6);\n}\n"
B = "#[test]\nfn b_one() {\n    assert!(true);\n}\n"
NEXTEST = """[profile.ci]
slow-timeout = { period = "60s", terminate-after = 2 }

[[profile.ci.overrides]]
# measured: 300 s on the build machine; the whole gallery is its scenario.
filter = 'test(=a::a_one)'
slow-timeout = { period = "400s", terminate-after = 1 }
threads-required = "num-test-threads"
priority = 100
"""


def git(root: pathlib.Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(root), *args], stdout=subprocess.PIPE, text=True, check=True).stdout


@pytest.fixture
def repo(tmp_path: pathlib.Path) -> pathlib.Path:
    root = tmp_path / "repo"
    (root / "crates/x/tests/common").mkdir(parents=True)
    (root / "crates/x/tests/a.rs").write_text(A)
    (root / "crates/x/tests/b.rs").write_text(B)
    (root / "crates/x/tests/common/mod.rs").write_text("pub fn helper() {}\n")
    git(root.parent, "init", "-q", str(root))
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", "tree")
    tree = git(root, "rev-parse", "HEAD").strip()
    (root / ".config").mkdir()
    (root / ".config/nextest.toml").write_text(NEXTEST)
    write(root, tree, [row("crates/x/tests/a.rs"), row("crates/x/tests/b.rs")])
    return root


def row(file: str, verdict: str = "keep", state: str = "none") -> dict:
    return {"file": file, "tests": 1, "seconds": 0.1, "unique_regions": 3, "shared_state": state, "verdict": verdict, "proof": None}


def write(root: pathlib.Path, tree: str, rows: list[dict], overrides: int = 1) -> None:
    p = root / TABLE
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps({"tree": tree, "files": 2, "overrides": overrides, "rows": rows}))


def table(root: pathlib.Path) -> dict:
    return json.loads((root / TABLE).read_text())


def run(root: pathlib.Path, *extra: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(TOOL), "--root", str(root), "--table", TABLE, *extra],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
    )


def test_a_whole_table_is_green(repo):
    r = run(repo, "--merges", "--overrides")
    assert r.returncode == 0, r.stderr
    assert "suite audit: 2 rows for 2 test files" in r.stdout
    assert "overrides: 1 entries" in r.stdout


def test_a_test_file_without_a_row_is_red(repo):
    t = table(repo)
    write(repo, t["tree"], t["rows"][:1])
    r = run(repo)
    assert r.returncode == 1 and "have no row" in r.stderr


def test_a_verdict_outside_the_closed_set_is_red(repo):
    t = table(repo)
    write(repo, t["tree"], [row("crates/x/tests/a.rs", verdict="drop it"), t["rows"][1]])
    assert "not in the closed set" in run(repo).stderr


def test_a_holder_that_is_not_a_test_in_the_tree_is_red(repo):
    t = table(repo)
    write(repo, t["tree"], [t["rows"][0], row("crates/x/tests/b.rs", verdict="remove: held by crates/x/tests/a.rs::nope")])
    (repo / "crates/x/tests/b.rs").unlink()
    r = run(repo)
    assert r.returncode == 1 and "not a test function" in r.stderr


def test_a_disk_row_that_never_calls_built_is_red(repo):
    t = table(repo)
    write(repo, t["tree"], [row("crates/x/tests/a.rs", state="disk"), t["rows"][1]])
    assert "never calls common::built" in run(repo).stderr


def test_a_merge_that_drops_an_assertion_is_red(repo):
    t = table(repo)
    write(repo, t["tree"], [row("crates/x/tests/a.rs", verdict="merge into crates/x/tests/b.rs"), t["rows"][1]])
    (repo / "crates/x/tests/a.rs").unlink()
    (repo / "crates/x/tests/b.rs").write_text(B + A.replace('    assert!("DW0101".len() == 6);\n', ""))
    r = run(repo, "--merges")
    assert r.returncode == 1 and "dropped by the merge" in r.stderr


def test_a_merge_that_moves_every_assertion_is_green(repo):
    t = table(repo)
    write(repo, t["tree"], [row("crates/x/tests/a.rs", verdict="merge into crates/x/tests/b.rs"), t["rows"][1]])
    (repo / "crates/x/tests/a.rs").unlink()
    (repo / "crates/x/tests/b.rs").write_text(B + A)
    r = run(repo, "--merges")
    assert r.returncode == 0, r.stderr


def test_a_file_moved_into_the_one_test_crate_still_counts_as_kept(repo):
    (repo / "crates/x/tests/it").mkdir()
    (repo / "crates/x/tests/a.rs").rename(repo / "crates/x/tests/it/a.rs")
    assert run(repo).returncode == 0


def test_an_override_count_the_table_does_not_state_is_red(repo):
    t = table(repo)
    write(repo, t["tree"], t["rows"], overrides=0)
    assert "the table states 0" in run(repo, "--overrides").stderr


def test_an_override_without_a_priority_is_red(repo):
    cfg = repo / ".config/nextest.toml"
    cfg.write_text(cfg.read_text().replace("priority = 100\n", ""))
    assert "carries no priority above the default" in run(repo, "--overrides").stderr


def test_an_override_without_its_measured_time_is_red(repo):
    cfg = repo / ".config/nextest.toml"
    cfg.write_text(cfg.read_text().replace("# measured: 300 s", "# the gallery"))
    assert "states its measured time" in run(repo, "--overrides").stderr


def test_an_override_that_does_not_run_alone_is_red(repo):
    cfg = repo / ".config/nextest.toml"
    cfg.write_text(cfg.read_text().replace('threads-required = "num-test-threads"\n', ""))
    assert "does not run alone" in run(repo, "--overrides").stderr
