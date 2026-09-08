"""The release-publish gate (`tools/check-release-publish-gate.py`).

The defect it pins: `engine-release.yml` had two publishes and one approval in
front of one of them. The GitHub Release was created and filled by an ungated
job while the crates.io half waited for a reviewer, so a tag push made the
release public with nobody asked, and either half could land without the other.
The rule now is that one approval publishes both shelves or neither — the shelf
goes into a DRAFT release, which publishes nothing, and a single
environment-gated job uploads to crates.io and then undrafts.

That rule is a claim about which job holds which verb, and the arrangement it
replaces was described correctly by its own comments for as long as it existed.
So the tests below drive the checker over SYNTHETIC workflow trees assembled
from `tools/tests/fixtures/release-publish-gate/`, one per way of getting it
wrong, and never over `.github/workflows/` — the live tree is judged by the CI
step itself, and a test that read it would go red for reasons that have nothing
to do with this checker. Two tests do read the live tree, and they assert only
what is about to be true of it by definition: that the population is non-empty
(the vacuity question) and that the script population excludes the workflows it
is the counterpart to.

`tools/lib/workflow_yaml.py`, the shared parse rule this reads workflows
through, is cross-checked against Ruby's Psych in `test_check_approval_guard.py`
over both the fixtures and the real workflow files; nothing here re-does that.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import pathlib
import shutil
import subprocess

import pytest

TOOLS = pathlib.Path(__file__).resolve().parents[1]
REPO = TOOLS.parent
SCRIPT = TOOLS / "check-release-publish-gate.py"
FIXTURES = TOOLS / "tests" / "fixtures" / "release-publish-gate"
WORKFLOWS = REPO / ".github" / "workflows"


def load_gate():
    spec = importlib.util.spec_from_file_location("check_release_publish_gate", SCRIPT)
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


def run(gate, directory, repo=REPO) -> tuple[int, str]:
    """The gate's own `main`, with both streams captured."""
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = gate.main(["--workflows", str(directory), "--repo", str(repo)])
    return code, out.getvalue() + err.getvalue()


# --------------------------------------------------------------------------
# the shape that is right
# --------------------------------------------------------------------------
def test_one_gated_job_holding_both_publishes_is_clean(gate, tree):
    code, output = run(gate, tree("clean.yml"))
    assert code == 0, output
    assert "check-release-publish-gate: OK" in output
    # The binding count is stated, always: a proof that does not say what it
    # bound to is how a zero binding survives review.
    assert "publishing act(s)" in output


# --------------------------------------------------------------------------
# the shapes that are wrong — one per way of publishing without the approval
# --------------------------------------------------------------------------
@pytest.mark.parametrize(
    ("fixture", "kind", "job"),
    [
        ("ungated-undraft.yml", "release-undraft", "announce"),
        ("ungated-create.yml", "release-create", "publish-assets"),
        ("ungated-token.yml", "registry-token", "upload"),
    ],
)
def test_a_publishing_act_outside_the_gate_is_a_finding(gate, tree, fixture, kind, job):
    code, output = run(gate, tree("clean.yml", fixture))
    assert code == 1, output
    assert kind in output
    assert repr(job) in output
    assert "declares no `environment:`" in output


def test_the_ungated_create_finding_names_the_draft_remedy(gate, tree):
    _, output = run(gate, tree("clean.yml", "ungated-create.yml"))
    # A gate that names a remedy owes the remedy: the finding has to say what to
    # do, and what to do is a draft.
    assert "DRAFT release" in output
    assert "--draft" in output


def test_an_ungated_release_writer_must_read_the_draft_state_back(gate, tree):
    code, output = run(gate, tree("clean.yml", "no-readback.yml"))
    assert code == 1, output
    assert "never reads the release's draft state back" in output
    assert "'fill'" in output


def test_the_clean_fixtures_ungated_writer_is_counted_as_proving_it(gate, tree):
    _, output = run(gate, tree("clean.yml"))
    assert "1 ungated release-writer(s) proving the release is a draft" in output


# --------------------------------------------------------------------------
# vacuity: a scan that binds to nothing is not a pass
# --------------------------------------------------------------------------
def test_a_gate_with_no_publishing_act_is_refused(gate, tree):
    code, output = run(gate, tree("gated-no-acts.yml"))
    assert code == 2, output
    assert "no publishing act was found" in output


def test_no_environment_gated_job_at_all_is_refused(gate, tree):
    code, output = run(gate, tree("no-acts.yml"))
    assert code == 2, output
    assert "no job declares `environment:`" in output


def test_an_empty_workflow_directory_is_refused(gate, tmp_path):
    empty = tmp_path / "workflows"
    empty.mkdir()
    code, output = run(gate, empty)
    assert code == 2, output
    assert "no workflow file" in output


def test_a_missing_workflow_directory_is_refused(gate, tmp_path):
    code, output = run(gate, tmp_path / "nowhere")
    assert code == 2, output
    assert "bound to nothing" in output


def test_a_repository_with_no_scripts_is_refused(gate, tree, tmp_path):
    """Population B is the way AROUND population A; not scanning it is not a pass."""
    bare = tmp_path / "bare"
    bare.mkdir()
    subprocess.run(["git", "-C", str(bare), "init", "-q"], check=True)
    code, output = run(gate, tree("clean.yml"), repo=bare)
    assert code == 2, output
    assert "the way AROUND the workflow rule was never looked at" in output


# --------------------------------------------------------------------------
# population B — a release verb hidden behind a script
# --------------------------------------------------------------------------
def _scratch_repo(tmp_path: pathlib.Path, *, script: str) -> pathlib.Path:
    repo = tmp_path / "scratch"
    (repo / "tools").mkdir(parents=True)
    (repo / "tools" / "release-helper.sh").write_text(script, encoding="utf-8")
    subprocess.run(["git", "-C", str(repo), "init", "-q"], check=True)
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)
    return repo


def test_an_undraft_moved_into_a_script_is_still_a_finding(gate, tree, tmp_path):
    repo = _scratch_repo(
        tmp_path,
        script='#!/usr/bin/env bash\nset -euo pipefail\ngh release edit "$1" --draft=false\n',
    )
    code, output = run(gate, tree("clean.yml"), repo=repo)
    assert code == 1, output
    assert "tools/release-helper.sh" in output
    assert "release-undraft" in output
    assert "invisible to the rule" in output


def test_a_public_release_create_in_a_script_is_still_a_finding(gate, tree, tmp_path):
    repo = _scratch_repo(
        tmp_path,
        script='#!/usr/bin/env bash\ngh release create "$1" --generate-notes\n',
    )
    code, output = run(gate, tree("clean.yml"), repo=repo)
    assert code == 1, output
    assert "release-create" in output


def test_a_script_that_only_drafts_and_uploads_is_clean(gate, tree, tmp_path):
    repo = _scratch_repo(
        tmp_path,
        script=(
            "#!/usr/bin/env bash\n"
            'gh release create "$1" --draft --generate-notes\n'
            'gh release upload "$1" dist/*.tar.gz --clobber\n'
        ),
    )
    code, output = run(gate, tree("clean.yml"), repo=repo)
    assert code == 0, output


def test_the_registry_upload_script_itself_is_not_a_population_b_act(gate, tree, tmp_path):
    """`cargo publish` legitimately lives in a script; its INVOCATION SITE is
    what population A judges, and refusing the script would refuse the design."""
    repo = _scratch_repo(
        tmp_path,
        script='#!/usr/bin/env bash\ncargo publish -p delvec --no-verify\n',
    )
    code, output = run(gate, tree("clean.yml"), repo=repo)
    assert code == 0, output


# --------------------------------------------------------------------------
# the live tree: only the questions that are about it
# --------------------------------------------------------------------------
def test_the_live_population_is_not_empty(gate):
    """The vacuity question is about the real directory by definition."""
    findings, counts = gate.check_workflows(WORKFLOWS)
    assert counts["files"] > 0
    assert counts["jobs"] > 0
    assert counts["gated"] > 0
    assert counts["acts"] > 0
    assert counts["acts"] == counts["acts_gated"], findings


def test_population_b_excludes_the_workflows_and_is_not_empty(gate):
    files = gate.population_b(REPO)
    assert files, "no script or composite action was found in this repository"
    assert not [p for p in files if ".github/workflows/" in p.as_posix()]
    assert any(p.name == "crates-io-publish.sh" for p in files)


def test_the_shared_publish_script_is_reached_only_with_publish(gate):
    """`--plan` is not an act and `--publish` is: the distinction that lets the
    preflight job run the same script with no credential in the run."""
    assert gate.acts_on_line("bash tools/crates-io-publish.sh --plan") == []
    assert [k for k, _ in gate.acts_on_line("bash tools/crates-io-publish.sh --publish")] == [
        "cargo-publish"
    ]


def test_a_drafted_create_is_not_an_act_and_an_undrafted_one_is(gate):
    assert gate.acts_on_line('gh release create "$TAG" --draft --title "x"') == []
    assert [k for k, _ in gate.acts_on_line('gh release create "$TAG" --title "x"')] == [
        "release-create"
    ]
    assert [k for k, _ in gate.acts_on_line('gh release create "$TAG" --draft=false')] == [
        "release-create"
    ]


def test_setting_the_draft_flag_is_not_reading_it_back(gate):
    """The readback rule must not be satisfied by the act it exists to prevent."""
    assert not gate.DRAFT_READBACK.search('gh release edit "$TAG" --draft=false')
    assert not gate.DRAFT_READBACK.search('gh release create "$TAG" --draft')
    assert gate.DRAFT_READBACK.search('gh release view "$TAG" --json isDraft --jq .isDraft')


def test_a_command_split_across_continuations_is_read_as_one(gate):
    text = 'echo hello\ngh release create "$TAG" \\\n  --draft \\\n  --generate-notes\n'
    lines = [(n, line) for n, line in gate.numbered_logical_lines(text) if line.strip()]
    assert [n for n, _ in lines] == [1, 2], lines
    # The command is one line for the act rules, and it is reported at the
    # physical line it starts on, not at the line the continuation ended.
    assert gate.acts_on_line(lines[1][1]) == []
    assert "--draft" in lines[1][1] and "--generate-notes" in lines[1][1]
