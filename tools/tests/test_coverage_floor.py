"""`tools/ci/coverage-floor.py`: each of its four refusals, by perturbation.

Every refusal is asserted twice: green on the unperturbed input, red on the one
edit only that refusal can catch (spec-0126 acceptance criterion 3).
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

import pytest

TOOL = pathlib.Path(__file__).resolve().parents[2] / "tools" / "ci" / "coverage-floor.py"

LIB = """pub fn one(x: u32) -> u32 {
    if x > 1 { x } else { 0 }
}

pub fn two() -> u32 {
    2
}

#[cfg(test)]
mod tests {
    #[test]
    fn t() {
        assert_eq!(super::two(), 2);
    }
}
"""

# Region entries on lines 1, 2 (two), 5 in production, 11 and 12 inside the
# cfg(test) item: `[line, col, count, has_count, is_region_entry, is_gap]`.
SEGMENTS = [
    [1, 28, 4, True, True, False],
    [2, 8, 4, True, True, False],
    [2, 16, 3, True, True, False],
    [2, 30, 1, True, True, False],
    [3, 2, 0, False, False, False],
    [5, 20, 0, True, True, False],
    [7, 2, 0, False, False, False],
    [12, 12, 1, True, True, False],
    [14, 6, 0, False, False, False],
]


def report(segments: list[list]) -> dict:
    return {
        "type": "llvm.coverage.json.export",
        "version": "3.0.1",
        "data": [
            {
                "files": [{"filename": "crates/x/src/lib.rs", "segments": segments}],
                "functions": [],
            }
        ],
    }


def run(*args: str, cwd: pathlib.Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(TOOL), *args], cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
    )


@pytest.fixture
def tree(tmp_path: pathlib.Path) -> pathlib.Path:
    root = tmp_path / "repo"
    (root / "crates/x/src").mkdir(parents=True)
    (root / "crates/x/src/lib.rs").write_text(LIB)
    (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.97.1"\n')
    (root / "versions.toml").write_text('[ci]\nnextest_version = "0.9.148"\nllvm_cov_version = "0.9.1"\n')
    subprocess.run(["git", "init", "-q", str(root)], check=True)
    subprocess.run(["git", "-C", str(root), "add", "-A"], check=True)
    subprocess.run(["git", "-C", str(root), "commit", "-q", "-m", "tree"], check=True)
    sha = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"], stdout=subprocess.PIPE, text=True, check=True
    ).stdout.strip()
    rep = tmp_path / "base.json"
    rep.write_text(json.dumps(report(SEGMENTS)))
    w = run(
        "--write", "--root", str(root), "--report", str(rep), "--second-report", str(rep),
        "--tree", sha, "--llvm-tools", "LLVM version 22.1.0", "--record", "record.json",
    )
    assert w.returncode == 0, w.stderr
    return root


def judge(root: pathlib.Path, tmp_path: pathlib.Path, segments: list[list]) -> subprocess.CompletedProcess:
    rep = tmp_path / "judged.json"
    rep.write_text(json.dumps(report(segments)))
    return run("--root", str(root), "--report", str(rep), "--record", "record.json")


def test_the_record_drops_cfg_test_regions_and_states_its_fields(tree: pathlib.Path):
    rec = json.loads((tree / "record.json").read_text())
    assert set(rec) == {
        "tree", "instrument", "command", "files", "excluded", "unstable_regions", "sha256", "rows_sha256",
    }
    assert rec["files"][0]["regions"] == {"covered": 4, "count": 5}
    assert rec["excluded"] == {"files": 1, "regions": 1}
    assert rec["unstable_regions"] == 0
    assert rec["instrument"]["nextest"] == "0.9.148"


def test_unperturbed_is_green_and_prints_the_binding_line(tree, tmp_path):
    r = judge(tree, tmp_path, SEGMENTS)
    assert r.returncode == 0, r.stderr
    assert "regions judged: 5 production regions in 1 files; excluded: 1 regions in 1 files" in r.stdout
    assert "covered: 4 (80.00 %); floor: 4/5 (80.00 %); headroom: +0.00 points" in r.stdout


def test_one_region_below_the_tolerated_floor_refuses(tree, tmp_path):
    below = [list(s) for s in SEGMENTS]
    below[3][2] = 0  # line 2's third region no longer runs
    r = judge(tree, tmp_path, below)
    assert r.returncode == 1
    assert "below the floor" in r.stderr


def test_zero_judged_regions_refuses(tree, tmp_path):
    r = judge(tree, tmp_path, [])
    assert r.returncode == 1
    assert "zero production regions judged" in r.stderr


def test_a_hand_edited_row_refuses(tree, tmp_path):
    path = tree / "record.json"
    rec = json.loads(path.read_text())
    rec["files"][0]["regions"]["covered"] = 3
    path.write_text(json.dumps(rec))
    r = judge(tree, tmp_path, SEGMENTS)
    assert r.returncode == 1
    assert "a hand-edited row" in r.stderr


def test_a_record_set_on_a_branch_refuses(tree, tmp_path):
    path = tree / "record.json"
    rec = json.loads(path.read_text())
    rec["tree"] = "0" * 40
    path.write_text(json.dumps(rec))
    r = judge(tree, tmp_path, SEGMENTS)
    assert r.returncode == 1
    assert "ancestor" in r.stderr


def test_an_opt_out_in_production_source_refuses(tree, tmp_path):
    lib = tree / "crates/x/src/lib.rs"
    lib.write_text(LIB.replace("pub fn two()", "#[mutants::skip]\npub fn two()", 1))
    r = judge(tree, tmp_path, SEGMENTS)
    assert r.returncode == 1
    assert "opt-out attribute" in r.stderr


def test_an_opt_out_inside_a_cfg_test_item_is_not_production(tree, tmp_path):
    lib = tree / "crates/x/src/lib.rs"
    lib.write_text(LIB.replace("    #[test]", "    #[mutants::skip]\n    #[test]", 1))
    r = judge(tree, tmp_path, SEGMENTS)
    assert "opt-outs in production source (mutants::skip, coverage(off)): 0" in r.stdout
