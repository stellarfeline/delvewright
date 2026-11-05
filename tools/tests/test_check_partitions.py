"""`tools/ci/check-partitions.py`: green when every listed test ran in exactly one partition, red on each way a sharding can drop or double one."""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

import pytest

TOOL = pathlib.Path(__file__).resolve().parents[2] / "tools" / "ci" / "check-partitions.py"

TESTS = [("delvec", "a::one"), ("delvec", "a::two"), ("delvec::it", "b::one"), ("dsl", "c::one")]


def write_list(path: pathlib.Path, tests=TESTS, ignored=()):
    suites: dict = {}
    for binary, name in tests:
        suites.setdefault(binary, {"testcases": {}})["testcases"][name] = {"ignored": False}
    for binary, name in ignored:
        suites.setdefault(binary, {"testcases": {}})["testcases"][name] = {"ignored": True}
    path.write_text(json.dumps({"rust-suites": suites}), encoding="utf-8")


def write_junit(path: pathlib.Path, cases, failing=()):
    path.parent.mkdir(parents=True, exist_ok=True)
    body = []
    for binary, name in cases:
        inner = '<failure message="no"/>' if (binary, name) in failing else ""
        body.append(f'<testcase name="{name}" classname="{binary}" time="1.5">{inner}</testcase>')
    xml = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        f'<testsuites name="nextest-run" tests="{len(cases)}" time="3.0">'
        f'<testsuite name="x" tests="{len(cases)}">{"".join(body)}</testsuite></testsuites>\n'
    )
    path.write_text(xml, encoding="utf-8")


def run(tmp: pathlib.Path, n: int) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(TOOL), "--list", str(tmp / "list.json"), "--junit-dir", str(tmp / "parts"), "--partitions", str(n)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


@pytest.fixture
def whole(tmp_path: pathlib.Path) -> pathlib.Path:
    write_list(tmp_path / "list.json", ignored=[("delvec", "a::ignored")])
    write_junit(tmp_path / "parts/p1/junit.xml", TESTS[:2])
    write_junit(tmp_path / "parts/p2/junit.xml", TESTS[2:])
    return tmp_path


def test_green_on_the_whole_with_its_counts(whole):
    r = run(whole, 2)
    assert r.returncode == 0, r.stderr
    assert "partitions: 2 of 2 declared; listed: 4 tests (1 ignored); ran: 4 distinct in 4 testcases" in r.stdout
    assert "p1: 2 tests" in r.stdout and "p2: 2 tests" in r.stdout


def test_a_dropped_partition_reds_on_count_and_on_the_tests_it_held(whole):
    (whole / "parts/p2/junit.xml").unlink()
    r = run(whole, 2)
    assert r.returncode == 1
    assert "1 partition report(s) found and 2 partition(s) were declared" in r.stderr
    assert "2 listed test(s) ran in no partition" in r.stderr


def test_an_empty_partition_reds(whole):
    write_junit(whole / "parts/p3/junit.xml", [])
    r = run(whole, 3)
    assert r.returncode == 1
    assert "partition p3 ran zero tests" in r.stderr


def test_zero_partitions_reds(whole):
    r = run(whole, 0)
    assert r.returncode == 1
    assert "partition count 0: a test run in no partition" in r.stderr


def test_a_test_in_two_partitions_reds(whole):
    write_junit(whole / "parts/p2/junit.xml", TESTS[1:])
    r = run(whole, 2)
    assert r.returncode == 1
    assert "1 test(s) ran in more than one partition: delvec::a::two (p1, p2)" in r.stderr


def test_an_unlisted_test_reds(whole):
    write_junit(whole / "parts/p2/junit.xml", TESTS[2:] + [("dsl", "c::stranger")])
    r = run(whole, 2)
    assert r.returncode == 1
    assert "1 test(s) ran that the archive does not list: dsl::c::stranger" in r.stderr


def test_a_failing_partition_reds(whole):
    write_junit(whole / "parts/p1/junit.xml", TESTS[:2], failing=[TESTS[0]])
    r = run(whole, 2)
    assert r.returncode == 1
    assert "partition p1 reports 1 failing test(s)" in r.stderr


def test_the_bounds_it_states_are_read_from_the_profile_the_partitions_ran_under(whole):
    """The census states the run bound and the per-test bound from `--root`'s
    `.config/nextest.toml`, through `tools/lib/suite_bounds.py`: moving the
    default in the profile moves the stated bound, with no literal in the gate."""
    root = whole / "tree"
    (root / ".config").mkdir(parents=True)
    profile = root / ".config/nextest.toml"
    for period, after, want in (("60s", 2, "per test 120 s by default"), ("120s", 3, "per test 360 s by default")):
        profile.write_text(
            f'[profile.ci]\nglobal-timeout = "78m"\nslow-timeout = {{ period = "{period}", terminate-after = {after} }}\n',
            encoding="utf-8",
        )
        r = subprocess.run(
            [sys.executable, str(TOOL), "--list", str(whole / "list.json"), "--junit-dir", str(whole / "parts"),
             "--partitions", "2", "--root", str(root)],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        assert r.returncode == 0, r.stderr
        assert "partition run 4680 s" in r.stdout and want in r.stdout, r.stdout
