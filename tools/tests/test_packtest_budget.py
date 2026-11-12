"""A PackTest run is bounded by the build's own budget, never by a constant.

`validation/packtest-run.sh` stopped every run at a fixed 900 s (exit 124),
measured once on another campaign. A campaign whose tests declare minutes of
game time each — the cheerful town's crate-expiry templates — met that guard
while its suite was still making progress. The budget is now computed from
what the build declares (`tools/lib/packtest-budget.sh`): the sum of its
tests' `@timeout` ticks at 20 a second, plus a start-up allowance. These tests
read the rule over built trees, and hold the runner to it.
"""

import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
LIB = ROOT / "tools" / "lib" / "packtest-budget.sh"
RUNNER = ROOT / "validation" / "packtest-run.sh"


def budget(build: pathlib.Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["bash", "-c", f'. "{LIB}"; dw_packtest_budget "{build}"; '
         'echo "start=$DW_PACKTEST_START_SECONDS default=$DW_PACKTEST_DEFAULT_TIMEOUT_TICKS"'],
        capture_output=True, text=True,
    )


def tree(tmp_path: pathlib.Path, tests: dict[str, str]) -> pathlib.Path:
    build = tmp_path / "out"
    for name, body in tests.items():
        f = build / "packtest-datapack" / "data" / "camp" / "test" / name
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text(body)
    fn = build / "packtest-datapack" / "data" / "camp" / "function" / "helper.mcfunction"
    fn.parent.mkdir(parents=True, exist_ok=True)
    fn.write_text("# @timeout 99999\nsay not a test\n")
    return build


def test_the_budget_is_the_declared_ticks_plus_the_start_up(tmp_path):
    build = tree(tmp_path, {
        "short.mcfunction": "# @dummy\n# @timeout 100\nassert block ~ ~ ~ air\n",
        "expires.mcfunction": "# @environment camp:solo/x\n# @timeout 12101\n\nawait delay 1s\n",
        "nested/plain.mcfunction": "say no directive: PackTest's default\n",
        # A directive after the first command is not a directive.
        "late.mcfunction": "# @timeout 40\nsay go\n# @timeout 99999\n",
    })
    res = budget(build)
    assert res.returncode == 0, res.stderr
    line, consts = res.stdout.splitlines()
    assert consts == "start=900 default=100"
    ticks = 100 + 12101 + 100 + 40
    secs = -(-ticks // 20)
    assert line == f"{900 + secs} 4 {ticks}", line
    print(f"budget binding: 4 of 4 tests counted, {ticks} ticks, {900 + secs} s")


def test_a_town_sized_suite_is_given_its_time(tmp_path):
    # The cheerful town's suite at the round that met the old guard: 186
    # tests of 100 ticks, 14 of 12101, 4 of 6101, 2 of 9701 and one of 7100.
    tests = {}
    for n, t in [(186, 100), (14, 12101), (4, 6101), (2, 9701), (1, 7100)]:
        for i in range(n):
            tests[f"t{t}_{i}.mcfunction"] = f"# @timeout {t}\nsay x\n"
    res = budget(tree(tmp_path, tests))
    s, n, ticks = map(int, res.stdout.splitlines()[0].split())
    assert (n, ticks) == (207, 238920)
    assert s == 900 + 11946
    assert s > 900, "the old constant stopped this suite while it was still running"


def test_a_build_with_no_test_has_no_budget(tmp_path):
    missing = budget(tmp_path / "nothing")
    assert missing.returncode != 0 and "no PackTest datapack" in missing.stderr
    empty = tmp_path / "empty"
    (empty / "packtest-datapack" / "data" / "camp" / "function").mkdir(parents=True)
    res = budget(empty)
    assert res.returncode != 0 and "no test under" in res.stderr


def test_the_runner_takes_the_builds_budget_and_prints_any_override():
    text = RUNNER.read_text()
    assert 'budget="$(dw_packtest_budget "$build_dir")" || exit 2' in text
    assert 'timeout=""' in text and "timeout=900" not in text
    assert 'timeout="$budget_s"' in text
    assert "OVERRIDE of the build's budget" in text
    assert 'dw_server_watch "$up_pid" "$runlog" "$timeout"' in text
