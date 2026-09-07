"""The approval-guard gate (`tools/check-approval-guard.py`) and its parse rule.

The defect class this pins is recorded in ADR-0017: the `crates-io` environment
held `CARGO_REGISTRY_TOKEN`, its required-reviewer rule had never been saved, and
a tag push published two crates to crates.io through a job called "publish to
crates.io (owner approval)" with nobody reviewing anything. `tools/assert-run-
approved.sh` closed that by reading the run's own approval history — and was then
invoked by hand, as a first step, in one job. A second environment-gated job now
exists, which is the trigger ADR-0017 names for building this checker.

The tests drive the gate over SYNTHETIC workflow trees assembled from
`tools/tests/fixtures/approval-guard/`, never over `.github/workflows/`: the live
tree is judged by the CI step itself, and a test that reads it would go green or
red for reasons that have nothing to do with this checker. One test does read the
live tree, and it asserts only that the population is non-empty and parses — the
vacuity question, which is about the live directory by definition.

`tools/lib/workflow_yaml.py`, the shared parse rule, is cross-checked against
Ruby's Psych — a real implementation of the format that shares no line of code
with ours. The comparison is committed twice over:

* `fixtures/approval-guard/psych-parse.json` is Psych's own parse of every
  fixture, recorded, with the instrument named in the file. It is compared on
  every run, on any machine, with no Ruby present — so the cross-check is never
  the unrun vacuity mode.
* when Ruby IS present (ubuntu-24.04 runners ship 3.2.3; a dev machine usually
  has one), two further tests run it live: one over the fixtures, which also
  proves the recording above is not stale, and one over the six real workflow
  files, which drift and therefore cannot be frozen.

Regenerate the recording after editing a fixture:

    python3 tools/tests/test_check_approval_guard.py
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import random
import shutil
import signal
import subprocess
import sys

import pytest

TOOLS = pathlib.Path(__file__).resolve().parents[1]
REPO = TOOLS.parent
SCRIPT = TOOLS / "check-approval-guard.py"
FIXTURES = TOOLS / "tests" / "fixtures" / "approval-guard"
RECORDING = FIXTURES / "psych-parse.json"
WORKFLOWS = REPO / ".github" / "workflows"

sys.path.insert(0, str(TOOLS / "lib"))

import workflow_yaml as wy  # noqa: E402

# Psych resolves the top-level `on:` key to the boolean `true` (YAML 1.1); this
# parser resolves no plain scalar and keeps the string. That is the ONLY
# difference between the two readings of every file in this repository, it is
# documented in `workflow_yaml`'s docstring, and it is undone here rather than
# tolerated as slack — so any other divergence is a failure.
RESOLUTION_DIFFERENCES = {"true": "on"}

# Psych's output, normalised the same way ours is, as one JSON line per file.
PSYCH_PROGRAM = r"""
require "yaml"
require "json"
def n(x)
  case x
  when Hash then x.map { |k, v| [n(k), n(v)] }.to_h
  when Array then x.map { |v| n(v) }
  when NilClass then nil
  when TrueClass then "true"
  when FalseClass then "false"
  else x.to_s
  end
end
out = {}
ARGV.each { |p| out[File.basename(p)] = n(YAML.unsafe_load(File.read(p))) }
puts JSON.generate({"psych" => Psych::VERSION, "documents" => out})
"""


# --------------------------------------------------------------------------
# helpers
# --------------------------------------------------------------------------
def load_gate():
    spec = importlib.util.spec_from_file_location("check_approval_guard", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def gate():
    return load_gate()


@pytest.fixture
def tree(tmp_path):
    """Assemble a workflow directory out of named fixtures."""

    def build(*names: str) -> pathlib.Path:
        directory = tmp_path / "workflows"
        directory.mkdir(exist_ok=True)
        for name in names:
            shutil.copy(FIXTURES / name, directory / name)
        return directory

    return build


def run(gate, directory) -> tuple[int, str]:
    import io
    import contextlib

    err = io.StringIO()
    with contextlib.redirect_stderr(err):
        code = gate.main(["--workflows", str(directory)])
    return code, err.getvalue()


def psych(paths: list[pathlib.Path]) -> tuple[str, dict[str, object]]:
    out = subprocess.run(
        ["ruby", "-e", PSYCH_PROGRAM, "--", *[str(p) for p in paths]],
        capture_output=True,
        text=True,
        check=True,
    )
    parsed = json.loads(out.stdout)
    return parsed["psych"], parsed["documents"]


def ours(path: pathlib.Path) -> object:
    document = wy.normalize(wy.load(path.read_text(encoding="utf-8")))
    if isinstance(document, dict):
        document = {
            RESOLUTION_DIFFERENCES.get(k, k) if isinstance(k, str) else k: v
            for k, v in document.items()
        }
    return document


def undo_resolution(document: object) -> object:
    if isinstance(document, dict):
        return {
            RESOLUTION_DIFFERENCES.get(k, k) if isinstance(k, str) else k: v
            for k, v in document.items()
        }
    return document


class _Hang(Exception):
    """Raised by the fuzz test's alarm when one `load` has not terminated."""


def _alarm(*_args) -> None:
    raise _Hang


ruby_present = pytest.mark.skipif(
    shutil.which("ruby") is None,
    reason="no ruby on this machine; the frozen Psych recording covers the same "
    "comparison and runs unconditionally",
)


# --------------------------------------------------------------------------
# the gate's verdicts
# --------------------------------------------------------------------------
def test_a_guarded_environment_job_passes(gate, tree):
    code, err = run(gate, tree("clean.yml"))
    assert code == 0, err


def test_the_environment_may_be_a_mapping_and_the_run_may_have_a_prelude(gate, tree):
    code, err = run(gate, tree("environment-mapping.yml"))
    assert code == 0, err


def test_a_missing_guard_step_is_a_red_naming_the_job_and_the_file(gate, tree):
    code, err = run(gate, tree("clean.yml", "missing-guard.yml"))
    assert code == 1
    assert "missing-guard.yml" in err and "'publish'" in err
    assert "tools/assert-run-approved.sh" in err
    # The clean file in the same directory must not be named.
    assert "clean.yml" not in err


def test_a_guard_below_another_run_step_is_a_red(gate, tree):
    code, err = run(gate, tree("guard-not-first.yml"))
    assert code == 1
    assert "guard-not-first.yml" in err
    assert "does not call tools/assert-run-approved.sh" in err
    # It names the step that got there first, so the repair is obvious.
    assert "package first, ask afterwards" in err


def test_a_guard_asserting_a_different_environment_is_a_red(gate, tree):
    code, err = run(gate, tree("wrong-environment.yml"))
    assert code == 1
    assert "'crates-io'" in err and "'staging'" in err


def test_a_gated_job_that_runs_no_command_is_a_red(gate, tree):
    code, err = run(gate, tree("no-run-step.yml"))
    assert code == 1
    assert "runs no command at all" in err


def test_a_gated_reusable_workflow_call_is_a_red(gate, tree):
    # It holds the environment — and therefore the secret — while having no step
    # of its own in which anything could assert the approval.
    code, err = run(gate, tree("no-steps.yml"))
    assert code == 1
    assert "no steps this checker can read" in err


def test_an_environment_with_no_readable_name_is_a_red(gate, tree):
    code, err = run(gate, tree("environment-unreadable.yml"))
    assert code == 1
    assert "cannot read a name out of" in err


def test_every_finding_is_reported_not_just_the_first(gate, tree):
    code, err = run(
        gate, tree("missing-guard.yml", "wrong-environment.yml", "guard-not-first.yml")
    )
    assert code == 1
    assert "3 finding(s)" in err


# --------------------------------------------------------------------------
# vacuity
# --------------------------------------------------------------------------
def test_a_directory_with_no_environment_gated_job_is_refused(gate, tree):
    code, err = run(gate, tree("ungated.yml"))
    assert code == 2
    assert "0 environment-gated job(s)" in err


def test_an_empty_directory_is_refused(gate, tmp_path):
    empty = tmp_path / "nothing"
    empty.mkdir()
    code, err = run(gate, empty)
    assert code == 2
    assert "0 workflow(s) scanned" in err


def test_a_missing_directory_is_refused(gate, tmp_path):
    code, err = run(gate, tmp_path / "absent")
    assert code == 2
    assert "not a directory" in err


def test_the_binding_count_is_stated_on_a_clean_run(gate, tree, capsys):
    code, _ = run(gate, tree("clean.yml", "ungated.yml"))
    assert code == 0
    out = capsys.readouterr().out
    assert "2 workflow(s) scanned, 1 environment-gated job(s)" in out


def test_the_live_workflow_population_is_not_empty_and_parses():
    """The gate's real population — the question the fixtures cannot answer."""
    files = sorted(p for p in WORKFLOWS.iterdir() if p.suffix in (".yml", ".yaml"))
    assert files, f"{WORKFLOWS} holds no workflow file"
    gated = 0
    for path in files:
        document = wy.load(path.read_text(encoding="utf-8"))
        jobs = document["jobs"]
        gated += sum(1 for job in jobs.values() if "environment" in job)
    assert gated > 0, "no live job declares `environment:` — the discovery rule broke"


# --------------------------------------------------------------------------
# the parse rule, against a real implementation of the format
# --------------------------------------------------------------------------
def test_our_parser_reproduces_the_recorded_psych_parse_of_every_fixture():
    recording = json.loads(RECORDING.read_text(encoding="utf-8"))
    documents = recording["documents"]
    fixtures = sorted(p.name for p in FIXTURES.glob("*.yml"))
    assert fixtures, "no fixture workflows to compare"
    assert sorted(documents) == fixtures, (
        "the recording and the fixture directory disagree about which files exist; "
        "regenerate with `python3 tools/tests/test_check_approval_guard.py`"
    )
    for name in fixtures:
        assert ours(FIXTURES / name) == undo_resolution(documents[name]), name


@ruby_present
def test_the_recorded_psych_parse_is_current():
    recording = json.loads(RECORDING.read_text(encoding="utf-8"))
    version, documents = psych(sorted(FIXTURES.glob("*.yml")))
    assert documents == recording["documents"], (
        "Psych now reads the fixtures differently from what is recorded; "
        f"regenerate with `python3 tools/tests/test_check_approval_guard.py` "
        f"(recorded psych {recording['instrument']['psych']}, this one {version})"
    )


@ruby_present
def test_our_parser_agrees_with_psych_on_every_live_workflow():
    paths = sorted(p for p in WORKFLOWS.iterdir() if p.suffix in (".yml", ".yaml"))
    assert paths
    _, documents = psych(paths)
    assert sorted(documents) == sorted(p.name for p in paths)
    for path in paths:
        assert ours(path) == undo_resolution(documents[path.name]), path.name


# --------------------------------------------------------------------------
# the parse rule refuses what it cannot read, rather than guessing past it
# --------------------------------------------------------------------------
@pytest.mark.parametrize(
    "text,why",
    [
        ("jobs:\n  a: &anchor\n    runs-on: x\n", "anchor"),
        ("jobs:\n  a:\n    runs-on: *alias\n", "alias"),
        ("jobs:\n  a:\n    runs-on: !!str x\n", "explicit tag"),
        ("a: 1\n---\nb: 2\n", "multi-document"),
        ("jobs:\n  a:\n    run: >\n      folded\n", "folded"),
        ("jobs:\n\ta: 1\n", "tab"),
        ("jobs:\n  a: 1\n     b: 2\n", "unexpected indent"),
        ("jobs:\n  a: { unterminated: 1\n", "flow collection"),
        # `[x::y]` left the flow scanner standing on a `:` neither branch
        # consumes, and the loop appended an empty item forever. A parser that
        # HANGS is worse than one that refuses: the CI step never reports at all,
        # and a job that never finishes reads as a slow runner. Found by fuzzing
        # this parser with random punctuation, not by reading it.
        ("a: [x::y]\n", "a flow entry with no separator after it"),
    ],
)
def test_the_parser_refuses_constructs_it_does_not_implement(text, why):
    with pytest.raises(wy.WorkflowYamlError):
        wy.load(text)


def test_the_parser_ends_in_a_verdict_on_random_punctuation():
    """Every input reaches an answer or a refusal — never a crash, never a loop.

    Both bugs this pins were found by fuzzing and neither by reading. `a: [x::y]`
    left the flow scanner standing on a `:` that neither branch consumed, and the
    loop appended an empty item FOREVER; `-` followed by trailing spaces indexed
    an empty string. A parser that hangs is worse than one that refuses: the CI
    step never reports at all, and a job that never finishes reads as a slow
    runner rather than as a defect. Seeded, so the corpus is the same every run.
    """
    random.seed(20260907)
    alphabet = list("abc: -[]{}\"'#|>&*!?\n\t01,.")
    deadline = hasattr(signal, "SIGALRM")
    if deadline:
        signal.signal(signal.SIGALRM, _alarm)
    try:
        for _ in range(4000):
            text = "".join(random.choice(alphabet) for _ in range(random.randint(1, 60)))
            if deadline:
                signal.setitimer(signal.ITIMER_REAL, 2.0)
            try:
                wy.load(text)
            except (wy.WorkflowYamlError, RecursionError):
                pass
            except _Hang:
                pytest.fail(f"the parser did not terminate on {text!r}")
            finally:
                if deadline:
                    signal.setitimer(signal.ITIMER_REAL, 0)
    finally:
        if deadline:
            signal.signal(signal.SIGALRM, signal.SIG_DFL)


def test_a_refused_workflow_file_is_a_finding_not_a_skip(gate, tmp_path):
    directory = tmp_path / "workflows"
    directory.mkdir()
    shutil.copy(FIXTURES / "clean.yml", directory / "clean.yml")
    (directory / "exotic.yml").write_text(
        "jobs:\n  publish:\n    environment: &env crates-io\n", encoding="utf-8"
    )
    code, err = run(gate, directory)
    assert code == 1
    assert "exotic.yml" in err and "refused this file" in err


# --------------------------------------------------------------------------
# regeneration
# --------------------------------------------------------------------------
def _regenerate() -> None:
    version, documents = psych(sorted(FIXTURES.glob("*.yml")))
    ruby = subprocess.run(
        ["ruby", "-e", "print RUBY_VERSION"], capture_output=True, text=True, check=True
    ).stdout
    payload = {
        "documents": documents,
        "instrument": {"psych": version, "ruby": ruby},
        "what": (
            "Psych's parse of every fixture workflow in this directory, with every "
            "scalar stringified (tools/lib/workflow_yaml.normalize does the same to "
            "ours). Regenerate: python3 tools/tests/test_check_approval_guard.py"
        ),
    }
    RECORDING.write_text(
        json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(f"wrote {RECORDING} — psych {version}, ruby {ruby}, {len(documents)} document(s)")


if __name__ == "__main__":
    _regenerate()
