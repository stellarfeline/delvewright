"""`tools/ci/check-required-contexts.py`: a matrix job's contexts are its expanded names, and the manifest must list each one."""

from __future__ import annotations

import pathlib
import subprocess
import sys

TOOL = pathlib.Path(__file__).resolve().parents[2] / "tools" / "ci" / "check-required-contexts.py"

CI = """name: CI
on: [push]
jobs:
  changes:
    name: changes (which jobs a pull request reaches)
    runs-on: ubuntu-latest
    steps:
      - run: "true"
  part:
    name: rust partition (${{ matrix.k }} of 3)
    needs: changes
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        k: [1, 2, 3]
    steps:
      - run: "true"
  plain:
    name: rust (fmt, clippy, test)
    needs: [changes, part]
    runs-on: ubuntu-latest
    steps:
      - run: "true"
"""

MANIFEST = """# the contexts
changes (which jobs a pull request reaches)
rust partition (1 of 3)
rust partition (2 of 3)
rust partition (3 of 3)
rust (fmt, clippy, test)
"""


def run(tmp: pathlib.Path, ci: str = CI, manifest: str = MANIFEST) -> subprocess.CompletedProcess:
    (tmp / "ci.yml").write_text(ci, encoding="utf-8")
    (tmp / "required.txt").write_text(manifest, encoding="utf-8")
    return subprocess.run(
        [sys.executable, str(TOOL), "--ci", str(tmp / "ci.yml"), "--manifest", str(tmp / "required.txt")],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def test_expanded_matrix_names_resolve(tmp_path):
    r = run(tmp_path)
    assert r.returncode == 0, r.stderr
    assert "5 ci.yml jobs, all 5 required contexts resolve" in r.stdout


def test_the_template_name_is_not_a_context(tmp_path):
    manifest = MANIFEST.replace(
        "rust partition (1 of 3)\nrust partition (2 of 3)\nrust partition (3 of 3)\n",
        "rust partition (${{ matrix.k }} of 3)\n",
    )
    r = run(tmp_path, manifest=manifest)
    assert r.returncode == 1
    assert "required context 'rust partition (${{ matrix.k }} of 3)' matches no job" in r.stderr
    assert "job 'rust partition (1 of 3)' is not a required status check" in r.stderr


def test_one_partition_missing_from_the_manifest_reds(tmp_path):
    r = run(tmp_path, manifest=MANIFEST.replace("rust partition (2 of 3)\n", ""))
    assert r.returncode == 1
    assert "job 'rust partition (2 of 3)' is not a required status check" in r.stderr


def test_a_matrix_name_without_a_matrix_list_is_a_finding(tmp_path):
    ci = CI.replace("      matrix:\n        k: [1, 2, 3]\n", "      matrix:\n        k: ${{ fromJSON(needs.changes.outputs.ks) }}\n")
    r = run(tmp_path, ci=ci)
    assert r.returncode == 1
    assert "`strategy.matrix.k` is not a plain, non-empty list of scalars" in r.stderr


def test_the_real_tree_resolves():
    r = subprocess.run([sys.executable, str(TOOL)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    assert r.returncode == 0, r.stderr
    assert "required contexts resolve" in r.stdout
