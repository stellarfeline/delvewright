"""The CI reach filter (`tools/lib/ci_reach.py`) and its gate (`tools/check-ci-reach.py`).

The filter decides which jobs a pull request skips, and a skip passes branch
protection, so each refusal of the gate is proven here by perturbing a COPY of the
live `ci.yml` or table toward the shape it refuses and watching it red. The live
tree itself is judged green once, which is the binding the CI step relies on.
"""

from __future__ import annotations

import importlib.util
import pathlib
import subprocess
import sys

import pytest

TOOLS = pathlib.Path(__file__).resolve().parents[1]
REPO = TOOLS.parent
WORKFLOW = REPO / ".github" / "workflows" / "ci.yml"
TABLE = REPO / ".github" / "ci-reach.toml"

sys.path.insert(0, str(TOOLS / "lib"))

import ci_reach  # noqa: E402

_spec = importlib.util.spec_from_file_location("check_ci_reach", TOOLS / "check-ci-reach.py")
gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate)


def run_gate(capsys, workflow: pathlib.Path = WORKFLOW, table: pathlib.Path = TABLE) -> tuple[int, str]:
    code = gate.main(["--workflow", str(workflow), "--table", str(table), "--repo", str(REPO)])
    out = capsys.readouterr()
    return code, out.out + out.err


def perturbed(tmp_path: pathlib.Path, old: str, new: str, source: pathlib.Path = WORKFLOW) -> pathlib.Path:
    text = source.read_text(encoding="utf-8")
    assert text.count(old) == 1, f"the perturbation's anchor is not unique in {source.name}: {old!r}"
    dest = tmp_path / source.name
    dest.write_text(text.replace(old, new), encoding="utf-8")
    return dest


HARNESS_IF = "    if: github.event_name != 'pull_request' || needs.changes.outputs.harness == 'true'\n"


# ------------------------------------------------------------------- globs


@pytest.mark.parametrize(
    "glob, path, hit",
    [
        ("crates/**", "crates/delvec/src/main.rs", True),
        ("crates/**", "crates", False),
        ("crates/**", "cratesx/a", False),
        ("**", "README.md", True),
        ("**/*.json", "a.json", True),
        ("**/*.json", "gallery/x/y.json", True),
        ("**/*.json", "gallery/x/y.jsonl", False),
        ("tools/lib/*", "tools/lib/a.py", True),
        ("tools/lib/*", "tools/lib/x/a.py", False),
        ("Cargo.toml", "prefabs/Cargo.toml", False),
        ("**/Cargo.toml", "prefabs/Cargo.toml", True),
        ("docs/a?.md", "docs/ab.md", True),
        ("docs/a?.md", "docs/a/.md", False),
    ],
)
def test_glob_semantics(glob, path, hit):
    assert bool(ci_reach.glob_regex(glob).match(path)) is hit


def test_a_glob_with_a_partial_double_star_is_refused():
    with pytest.raises(ci_reach.TableError):
        ci_reach.glob_regex("crates/a**")


# ------------------------------------------------------------------ filter


def on(verdict: dict[str, str | None]) -> set[str]:
    return {g for g, why in verdict.items() if why}


def test_a_readme_change_reaches_only_the_whole_tree_readers():
    table = ci_reach.load_table()
    assert on(ci_reach.decide(table, "pull_request", ["README.md"])) == {
        "manifest",
        "dsl-crate-version",
        "i18n-tool",
        "docs",
        "line-endings",
    }


def test_a_compiler_source_change_reaches_everything_that_builds_the_crates():
    table = ci_reach.load_table()
    got = on(ci_reach.decide(table, "pull_request", ["crates/delvec/src/main.rs"]))
    assert {"rust", "engine-shelf", "mecha-crosscheck", "content-pin", "gallery", "tier2-validation"} <= got
    assert got.isdisjoint({"harness", "skin-tool", "storybook-version", "prefab-generators"})


@pytest.mark.parametrize("path", [".github/workflows/ci.yml", ".github/ci-reach.toml", "versions.toml", "Cargo.lock", "tools/lib/ci_reach.py"])
def test_what_every_job_reads_runs_every_job(path):
    table = ci_reach.load_table()
    assert on(ci_reach.decide(table, "pull_request", [path])) == set(table.groups)


@pytest.mark.parametrize("event", ["push", "workflow_dispatch"])
def test_every_other_event_runs_every_group(event):
    table = ci_reach.load_table()
    assert on(ci_reach.decide(table, event, None)) == set(table.groups)


def test_an_empty_change_reaches_nothing():
    table = ci_reach.load_table()
    assert on(ci_reach.decide(table, "pull_request", [])) == set()


def test_a_move_reaches_the_readers_of_the_old_path(tmp_path):
    def git(*args):
        subprocess.run(["git", "-C", str(tmp_path), *args], check=True, capture_output=True)

    git("init", "-q")
    git("config", "user.email", "t@example.invalid")
    git("config", "user.name", "t")
    (tmp_path / "harness").mkdir()
    (tmp_path / "harness" / "a.ts").write_text("export const a = 1;\n")
    git("add", "-A")
    git("commit", "-qm", "a")
    (tmp_path / "docs").mkdir()
    git("mv", "harness/a.ts", "docs/a.ts")
    git("commit", "-qm", "b")
    assert ci_reach.changed_paths("HEAD~1", "HEAD", tmp_path) == ["docs/a.ts", "harness/a.ts"]


def test_an_unreadable_diff_is_an_error_not_an_empty_change(tmp_path):
    subprocess.run(["git", "-C", str(tmp_path), "init", "-q"], check=True)
    with pytest.raises(ci_reach.TableError):
        ci_reach.changed_paths("deadbeef", "HEAD", tmp_path)


def test_the_cli_writes_one_output_line_per_group(capsys):
    assert ci_reach.main(["groups", "--event", "push"]) == 0
    lines = capsys.readouterr().out.splitlines()
    assert lines == [f"{g}=true" for g in ci_reach.load_table().groups]


# -------------------------------------------------------------------- gate


def test_the_live_workflow_and_table_agree(capsys):
    code, out = run_gate(capsys)
    assert code == 0, out
    jobs = gate.load(WORKFLOW.read_text(encoding="utf-8"))["jobs"]
    assert f"{len(jobs) - 1} job(s) judged" in out


def test_an_if_that_drops_workflow_dispatch_is_refused(tmp_path, capsys):
    wf = perturbed(
        tmp_path,
        HARNESS_IF,
        "    if: github.event_name == 'push' || needs.changes.outputs.harness == 'true'\n",
    )
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "job `harness` does not run on `workflow_dispatch`" in out


def test_the_explicit_form_without_its_dispatch_clause_is_refused(tmp_path, capsys):
    explicit = perturbed(
        tmp_path,
        HARNESS_IF,
        "    if: github.event_name == 'push' || github.event_name == 'workflow_dispatch' || needs.changes.outputs.harness == 'true'\n",
    )
    code, out = run_gate(capsys, explicit)
    assert code == 0, out
    (tmp_path / "b").mkdir()
    dropped = perturbed(
        tmp_path / "b",
        " || github.event_name == 'workflow_dispatch'",
        "",
        source=explicit,
    )
    code, out = run_gate(capsys, dropped)
    assert code == 1
    assert "does not run on `workflow_dispatch`" in out


def test_an_if_that_drops_push_is_refused(tmp_path, capsys):
    wf = perturbed(
        tmp_path,
        HARNESS_IF,
        "    if: github.event_name == 'workflow_dispatch' || needs.changes.outputs.harness == 'true'\n",
    )
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "does not run on `push`" in out


def test_a_job_with_no_if_has_no_group(tmp_path, capsys):
    code, out = run_gate(capsys, perturbed(tmp_path, HARNESS_IF, ""))
    assert code == 1
    assert "job `harness` has no job-level `if`" in out
    assert "table group `harness` is read by no job" in out


def test_an_if_that_ignores_its_group_is_refused(tmp_path, capsys):
    wf = perturbed(tmp_path, HARNESS_IF, "    if: github.event_name != 'pull_request' || true\n")
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "job `harness` reads no filter group" in out


def test_an_if_that_runs_on_every_pull_request_is_refused(tmp_path, capsys):
    wf = perturbed(
        tmp_path,
        HARNESS_IF,
        "    if: github.event_name != 'pull_request' || needs.changes.outputs.harness == 'true' || github.event_name == 'pull_request'\n",
    )
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "runs on a pull request that reaches none of its groups" in out


def test_a_function_call_is_refused_by_name(tmp_path, capsys):
    wf = perturbed(
        tmp_path,
        HARNESS_IF,
        "    if: always() && (github.event_name != 'pull_request' || needs.changes.outputs.harness == 'true')\n",
    )
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "`always(…)` is not evaluated" in out


def test_a_group_no_job_reads_is_refused(tmp_path, capsys):
    text = TABLE.read_text(encoding="utf-8") + '\n[[group]]\nname = "orphan"\ninputs = [\n  { glob = "README.md", why = "t" },\n]\n'
    table = tmp_path / "ci-reach.toml"
    table.write_text(text, encoding="utf-8")
    code, out = run_gate(capsys, table=table)
    assert code == 1
    assert "table group `orphan` is read by no job" in out
    assert "table group `orphan` is not an output of `changes`" in out


def test_a_glob_that_binds_nothing_is_refused(tmp_path, capsys):
    table = perturbed(tmp_path, '{ glob = "tools/skin/**",', '{ glob = "tools/skn/**",', source=TABLE)
    code, out = run_gate(capsys, table=table)
    assert code == 1
    assert "glob `tools/skn/**` matches no tracked file" in out


def test_an_all_set_that_misses_the_filter_is_refused(tmp_path, capsys):
    table = perturbed(
        tmp_path,
        '  { glob = "tools/lib/ci_reach.py", why = "the filter that decides every other group" },\n',
        "",
        source=TABLE,
    )
    code, out = run_gate(capsys, table=table)
    assert code == 1
    assert "`[all]` does not match `tools/lib/ci_reach.py`" in out


def test_a_filter_with_an_if_is_refused(tmp_path, capsys):
    wf = perturbed(
        tmp_path,
        "    name: changes (which jobs a pull request reaches)\n",
        "    name: changes (which jobs a pull request reaches)\n    if: github.event_name == 'pull_request'\n",
    )
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "`changes` has an `if`" in out


def test_a_missing_filter_output_is_refused(tmp_path, capsys):
    wf = perturbed(tmp_path, "      harness: ${{ steps.reach.outputs.harness }}\n", "")
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "table group `harness` is not an output of `changes`" in out
