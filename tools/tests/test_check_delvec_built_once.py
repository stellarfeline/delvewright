"""`tools/ci/check-delvec-built-once.py`: one `delvec` per CI run.

Each refusal is proven by perturbing a COPY of the live `ci.yml` toward the shape
it refuses and watching it red; the live tree is judged green once, which is the
binding the CI step relies on.
"""

from __future__ import annotations

import importlib.util
import pathlib

TOOLS = pathlib.Path(__file__).resolve().parents[1]
REPO = TOOLS.parent
WORKFLOW = REPO / ".github" / "workflows" / "ci.yml"

_spec = importlib.util.spec_from_file_location("check_delvec_built_once", TOOLS / "ci" / "check-delvec-built-once.py")
gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate)

MECHA_TAKE = """      - uses: ./.github/actions/checkout-content
      - uses: ./.github/actions/delvec-binary
        with: { sha256: "${{ needs.delvec-binary.outputs.sha256 }}" }
      # A DIFFERENT interpreter line"""
MECHA_BUILD = """          target/debug/delvec \\
            build crates/dsl/fixtures/valid/hello-world \\
            -o out --prefabs campaigns/prefabs"""


# The download action as the live workflow pins it (`.github/pins.toml` holds the
# version; this file does not restate it).
DOWNLOAD = next(
    line.split("uses:", 1)[1].strip()
    for line in WORKFLOW.read_text(encoding="utf-8").splitlines()
    if "uses: actions/download-artifact@" in line
)


def run_gate(capsys, workflow: pathlib.Path = WORKFLOW) -> tuple[int, str]:
    code = gate.main(["--workflow", str(workflow), "--repo", str(REPO)])
    out = capsys.readouterr()
    return code, out.out + out.err


def perturbed(tmp_path: pathlib.Path, old: str, new: str) -> pathlib.Path:
    text = WORKFLOW.read_text(encoding="utf-8")
    assert text.count(old) == 1, f"the perturbation's anchor is not unique in ci.yml: {old!r}"
    dest = tmp_path / "ci.yml"
    dest.write_text(text.replace(old, new), encoding="utf-8")
    return dest


def test_the_live_workflow_builds_delvec_once(capsys):
    code, out = run_gate(capsys)
    assert code == 0, out
    jobs = gate.load(WORKFLOW.read_text(encoding="utf-8"))["jobs"]
    consumers = [j for j, job in jobs.items() if j not in gate.COMPILES and gate.BINARY in str(job)]
    assert len(consumers) >= 4, consumers
    assert f"{len(consumers)} of them running `{gate.BINARY}`" in out


def test_a_consumer_that_compiles_the_engine_again_is_refused(tmp_path, capsys):
    wf = perturbed(tmp_path, MECHA_BUILD, MECHA_BUILD.replace("target/debug/delvec", "cargo run --locked -p delvec --bin delvec --"))
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "job `mecha-crosscheck` step" in out and "compiles the root workspace: cargo run --locked -p delvec" in out


def test_a_script_that_can_build_its_own_engine_is_refused(tmp_path, capsys):
    """`render-shots.sh` sources `tools/lib/delvec-bin.sh`, whose fallback compiles."""
    wf = perturbed(tmp_path, MECHA_TAKE, MECHA_TAKE.replace(
        "      # A DIFFERENT interpreter line",
        "      - run: bash validation/render-shots.sh --help\n      # A DIFFERENT interpreter line",
    ))
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "`validation/render-shots.sh` → `tools/lib/delvec-bin.sh` compiles the root workspace" in out


def test_another_workspace_may_be_compiled(tmp_path, capsys):
    wf = perturbed(tmp_path, MECHA_TAKE, MECHA_TAKE.replace(
        "      # A DIFFERENT interpreter line",
        "      - run: cargo build --locked --manifest-path prefabs/Cargo.toml\n      # A DIFFERENT interpreter line",
    ))
    code, out = run_gate(capsys, wf)
    assert code == 0, out


def test_a_consumer_that_does_not_take_the_build_is_refused(tmp_path, capsys):
    wf = perturbed(tmp_path, MECHA_TAKE, MECHA_TAKE.replace(
        """      - uses: ./.github/actions/delvec-binary
        with: { sha256: "${{ needs.delvec-binary.outputs.sha256 }}" }
""", ""))
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "job `mecha-crosscheck` runs `target/debug/delvec` without taking it through" in out


def test_a_consumer_that_skips_the_hash_is_refused(tmp_path, capsys):
    wf = perturbed(tmp_path, MECHA_TAKE, MECHA_TAKE.replace(
        """        with: { sha256: "${{ needs.delvec-binary.outputs.sha256 }}" }
""", """        with: { sha256: "" }
"""))
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "job `mecha-crosscheck` runs `target/debug/delvec` without taking it through" in out


def test_a_consumer_that_downloads_the_artifact_itself_is_refused(tmp_path, capsys):
    wf = perturbed(tmp_path, MECHA_TAKE, MECHA_TAKE.replace(
        "      # A DIFFERENT interpreter line",
        f"      - uses: {DOWNLOAD}\n        with: {{ name: delvec-binary }}\n      # A DIFFERENT interpreter line",
    ))
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "job `mecha-crosscheck` step" in out and "downloads `delvec-binary` itself" in out


def test_a_build_job_with_no_hash_output_is_refused(tmp_path, capsys):
    wf = perturbed(tmp_path, "    outputs:\n      sha256: ${{ steps.hash.outputs.sha256 }}\n", "")
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "`delvec-binary` declares no `sha256` output" in out


def test_a_build_job_that_builds_another_profile_is_refused(tmp_path, capsys):
    wf = perturbed(
        tmp_path,
        "        run: cargo build --locked -p delvec --bin delvec\n      - name: the binary's sha256\n",
        "        run: cargo build --locked --release -p delvec --bin delvec\n      - name: the binary's sha256\n",
    )
    code, out = run_gate(capsys, wf)
    assert code == 1
    assert "`delvec-binary` has no step running exactly `cargo build --locked -p delvec --bin delvec`" in out
