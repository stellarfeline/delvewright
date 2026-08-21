r"""Guards for `tools/check-shell-bash32.py`.

The gate refuses bash-4-and-later syntax in repo shell, because Dev is macOS and
macOS ships bash 3.2.57. Its failure mode is not a wrong verdict — it is a
SILENT one, in two directions:

  * a member that stops matching (the gate goes green over a real `mapfile`);
  * an input that stops being read (the gate goes green over a file it skipped).

Both look identical from outside, so both are asserted here. `POSITIVE` is the
construct catalogue: every entry was run on this machine's `/bin/bash` 3.2.57 and
on `bash:5` in Docker, and the comment on each says what 3.2 did with it. The
truncation tests exist because the scanner shipped two such bugs during its own
development — a here-string read as a heredoc, and a heredoc inside `"$( … )"`
never seen at all — each of which silently swallowed the rest of a file.
"""

from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-shell-bash32.py"


def load():
    spec = importlib.util.spec_from_file_location("check_shell_bash32", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


MOD = load()


def findings(*lines: str) -> list[str]:
    return MOD.check("fixture.sh", list(lines))[0]


# Each line here fails on bash 3.2.57 and succeeds on bash 5, measured on both.
# The comment records what 3.2 actually does — half of them do not even abort,
# which is the reason a human reading the output never noticed.
POSITIVE = [
    ('mapfile -t a < f', "command not found; array left EMPTY; status 0"),
    ('readarray -t a < f', "command not found; array left EMPTY; status 0"),
    ('coproc cat', "command not found; status 0"),
    ('wait -n', "invalid option; waits for nothing"),
    ('declare -A m', "invalid option; m becomes an ordinary indexed array"),
    ('local -A m', "invalid option"),
    ('typeset -A m', "invalid option"),
    ('declare -n ref=v', "invalid option"),
    ('declare -l v=AB', "invalid option; no lowercasing happens"),
    ('declare -u v=ab', "invalid option"),
    ('declare -g g=1', "invalid option"),
    ('echo "${v,,}"', "bad substitution — aborts the script"),
    ('echo "${v^^}"', "bad substitution"),
    ('echo "${v^}"', "bad substitution"),
    ('echo "${arr[@]^^}"', "bad substitution"),
    ('echo "${v@Q}"', "bad substitution"),
    ('echo "${a[-1]}"', "bad array subscript; expands to EMPTY"),
    ('echo hi &>> log', "syntax error"),
    ('echo hi |& cat', "syntax error"),
    ('case x in x) : ;;& *) : ;; esac', "syntax error"),
    ('case x in x) : ;& *) : ;; esac', "syntax error"),
    ('[[ -v name ]]', "conditional binary operator expected — syntax error"),
    ('printf -v "a[0]" %s y', "not a valid identifier; prints to stdout instead"),
    ('printf -v t "%(%Y)T" -1', "invalid format character"),
    ("printf '%(%Y)T\\n' -1", "invalid format character — and the format is SINGLE-quoted,"
                              " which bash's own printf still interprets"),
    ('shopt -s globstar', "invalid shell option name; exits 1"),
    ('shopt -s lastpipe', "invalid shell option name"),
    ('shopt -u inherit_errexit', "invalid shell option name"),
    ('exec {fd}</dev/null', "tries to execute a file named {fd}"),
]

# Valid bash 3.2 that a careless pattern would flag. `set -e` is on in these
# scripts, so a false positive is not a nuisance — it is the reason a gate stops
# being read.
NEGATIVE = [
    '# Not `mapfile`: macOS ships bash 3.2, which has no such builtin',
    '  # `mapfile` is a bash 4 builtin, and using it made this script cover ZERO',
    'echo "no mapfile here"  # readarray is also out',
    'declare -a arr',
    'declare -r -i n=1',
    'export -n VAR',
    'echo "${v#prefix}"',
    'echo "${v%%suffix}"',
    'echo "${v/a/b}"',
    'echo "${!prefix@}"',
    'echo "${a[i-1]}"',
    'echo "${arr[@]+"${arr[@]}"}"',
    'cmd >>log 2>&1',
    'a=1; b=2',
    'shopt -s extglob',
    'shopt -s nullglob',
    'shopt -q login_shell',
    'case x in x) : ;; *) : ;; esac',
    'IFS=$\'\\t\' read -r a b <<< "$row"',
    "awk '{ print | \"cat\" }' f",
    'printf -v var %s val',
    'f() { local x=1; }',
    # A command word inside quotes is text, not a command: `bare` blanks both
    # quote kinds, which is what keeps the seven recorded comments and every
    # message string green.
    'echo "declare -A is a bash 4 thing"',
    "grep -F 'coproc' file",
    "echo '${v,,}'",
    'echo "the ${v} value"',
]


@pytest.mark.parametrize("line,behaviour", POSITIVE, ids=[p[0] for p in POSITIVE])
def test_bash4_construct_is_a_finding(line: str, behaviour: str) -> None:
    got = findings("#!/usr/bin/env bash", line)
    assert got, f"not flagged: {line!r} — on bash 3.2.57 this {behaviour}"
    assert "fixture.sh:2:" in got[0]


@pytest.mark.parametrize("line", NEGATIVE)
def test_valid_bash32_is_not_a_finding(line: str) -> None:
    assert findings("#!/usr/bin/env bash", line) == []


def test_the_seven_recorded_comments_stay_green() -> None:
    """Every recorded instance of this lesson is a comment SAYING NOT TO USE IT.

    A grep for the string reds exactly the lines that preserve the lesson, so the
    comment stripper is not a nicety — it is what lets the evidence survive.
    """
    recorded = subprocess.run(
        ["git", "grep", "-n", "-F", "-e", "mapfile", "-e", "bash 3.2", "--", "*.sh"],
        cwd=REPO,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    files = {row.split(":", 1)[0] for row in recorded}
    assert len(files) >= 7, f"expected the lesson in >=7 scripts, found {sorted(files)}"
    for rel in sorted(files):
        lines = (REPO / rel).read_text(encoding="utf-8").splitlines()
        assert MOD.check(rel, lines)[0] == [], f"{rel} reddened by its own comment"


def test_the_three_masks_disagree_where_they_must() -> None:
    """One line, three readings — and each catalogue needs a different one.

    A COMMAND inside quotes is text (`echo "no mapfile here"`). An EXPANSION
    inside double quotes is real (`"${v,,}"`) and inside single quotes is not.
    A printf FORMAT is interpreted by bash however it is quoted.
    """
    assert findings("python3 -c 'x = 1  # mapfile'") == []
    assert findings('echo "no mapfile here"') == []
    assert findings('echo "${v,,}"')
    assert findings("echo '${v,,}'") == []
    assert findings("printf '%(%s)T' -1")


def test_here_string_is_not_a_heredoc() -> None:
    """The regression that silently skipped 51 lines of `packtest-all.sh`.

    Read one `<` at a time, `<<< "$row"` becomes a heredoc whose delimiter is
    `$row` — which never arrives, so the rest of the file is its body.
    """
    got = findings(
        'read -r a b <<< "$row"',
        "mapfile -t x < f",
    )
    assert got and "fixture.sh:2:" in got[0]


def test_heredoc_body_is_not_read_and_is_counted() -> None:
    code, skipped, unread = MOD.scan(
        ["python3 - <<'PY'", "mapfile = 1  # python, not shell", "PY", "echo done"]
    )
    assert skipped == 1
    assert unread == []
    assert findings("python3 - <<'PY'", "mapfile = 1", "PY", "echo done") == []


def test_heredoc_inside_command_substitution_is_still_a_heredoc() -> None:
    """Command substitution restarts quoting; four repo scripts rely on it.

    Without the `$(` push the opener sits inside a double-quoted span, is never
    seen, and the python body is read as shell.
    """
    _, skipped, _ = MOD.scan(
        ['eval "$(python3 - "$M" <<\'PY\'', "mapfile = 1", "PY", ')"', "echo done"]
    )
    assert skipped == 1


def test_quoted_heredoc_marker_inside_a_string_is_not_a_heredoc() -> None:
    """`validation/check-world-settings.sh`'s failure message contains `<<EOS`."""
    _, skipped, unread = MOD.scan(
        ['fail "Dockerfile: no \'RUN cat > /delve/entrypoint.sh <<EOS\' found"', "mapfile -t x < f"]
    )
    assert skipped == 0 and unread == []
    assert findings(
        'fail "Dockerfile: no \'RUN cat > /delve/entrypoint.sh <<EOS\' found"',
        "mapfile -t x < f",
    )


def test_a_heredoc_that_writes_shell_is_a_finding() -> None:
    """The one case where fail-open on heredoc bodies would be wrong."""
    assert any("SHELL SCRIPT" in f for f in findings('cat >setup.sh <<EOF', "x", "EOF"))
    assert any("SHELL SCRIPT" in f for f in findings('bash <<EOF', "x", "EOF"))


def test_an_unterminated_heredoc_refuses_rather_than_passing() -> None:
    """Everything after it was skipped; a quiet pass over that is truncation."""
    assert any("NOT read" in f for f in findings("cat <<EOF", "body", "no delimiter here"))


def test_run_blocks_separate_shell_from_defaults_run_config() -> None:
    lines = [
        "    defaults:",
        "      run:",
        "        working-directory: harness",
        "    steps:",
        "      - run: npm ci",
        "      - name: two",
        "        run: |",
        "          set -euo pipefail",
        "          echo hi",
    ]
    blocks, config = MOD.run_blocks(lines)
    assert config == 1
    assert [b[1] for b in blocks] == [["npm ci"], ["          set -euo pipefail", "          echo hi"]]


def test_github_expressions_are_not_read_as_shell() -> None:
    assert findings("echo ${{ github.sha }}") == []


def test_shopt_is_a_closed_world_read_out_of_bash_32() -> None:
    """A member list would need extending for every future bash; this cannot."""
    assert len(MOD.BASH32_SHOPTS) == 34
    assert "extglob" in MOD.BASH32_SHOPTS  # 3.2 has it
    assert "globstar" not in MOD.BASH32_SHOPTS  # 4.0
    assert "lastpipe" not in MOD.BASH32_SHOPTS  # 4.2


def test_population_is_derived_and_covers_both_halves() -> None:
    shell, workflows, excluded, tracked = MOD.population()
    assert tracked > 500
    assert shell and workflows
    # Derivation, not a hand-list: name-based UNION shebang-based.
    assert "validation/packtest-all.sh" in shell
    assert ".github/workflows/engine-release.yml" in workflows
    assert ".github/actions/checkout-content/action.yml" in workflows
    # docs/experiments/ is excluded BY PREFIX and the exclusion is counted, so an
    # exclusion that has stopped matching cannot quietly shrink the population.
    assert excluded and all(e.startswith("docs/experiments/") for e in excluded)
    assert not any(s.startswith("docs/experiments/") for s in shell)


def test_the_repository_is_green() -> None:
    proc = subprocess.run(
        [sys.executable, str(CHECKER)], cwd=REPO, capture_output=True, text=True
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "OK" in proc.stdout
