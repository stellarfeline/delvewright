"""The harness/compiler `dsl_version` sync gate (`tools/check-harness-dsl-version.py`).

The defect this pins, from the field (task #157): the compiler's
`SUPPORTED_DSL_VERSION` moved to `0.9.0` (spec-0026) while
`harness/src/critical-path.ts`'s `SUPPORTED_DSL_VERSIONS` allowlist still ended
at `0.8.0`. Every other CI job was green — nothing else in the repo relates the
two files — and the bot tier refused every 0.9.0 campaign at the version gate,
after the server booted and the bot connected.

These tests drive the detector over synthetic source pairs so it keeps failing
for the right reason as the real files grow. The live pair is checked by the
CI step itself.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-harness-dsl-version.py"

ENVELOPE_TEMPLATE = """\
//! doc
pub const SUPPORTED_DSL_VERSION: &str = "{version}";
pub const SUPPORTED_DSL_VERSIONS: &[&str] = &["0.2.0", "{version}"];
"""

CRITICAL_PATH_TEMPLATE = """\
// doc
export const SUPPORTED_DSL_VERSIONS = [
{entries}
] as const;
"""


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at synthetic source files."""
    spec = importlib.util.spec_from_file_location("check_harness_dsl_version", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    monkeypatch.setattr(module, "ENVELOPE_RS", tmp_path / "envelope.rs")
    monkeypatch.setattr(module, "CRITICAL_PATH_TS", tmp_path / "critical-path.ts")
    return module


def pair(gate, compiler_version: str, harness_versions: list[str]) -> int:
    gate.ENVELOPE_RS.write_text(
        ENVELOPE_TEMPLATE.format(version=compiler_version), encoding="utf-8"
    )
    entries = "\n".join(f'  "{v}",' for v in harness_versions)
    gate.CRITICAL_PATH_TS.write_text(
        CRITICAL_PATH_TEMPLATE.format(entries=entries), encoding="utf-8"
    )
    return gate.main()


def test_harness_covering_the_compiler_ceiling_passes(gate):
    assert pair(gate, "0.9.0", ["0.2.0", "0.8.0", "0.9.0"]) == 0


def test_harness_lagging_the_compiler_ceiling_fails(gate, capsys):
    # The task #157 reproduction: compiler moved to 0.9.0, harness allowlist
    # still ends at 0.8.0.
    assert pair(gate, "0.9.0", ["0.2.0", "0.7.0", "0.8.0"]) == 1
    err = capsys.readouterr().err
    assert "0.9.0" in err
    assert "0.8.0" in err


def test_harness_may_carry_older_versions_the_compiler_dropped(gate):
    # Membership of the ceiling, not set equality — the harness is allowed to
    # keep accepting versions the compiler still emits/documents even if this
    # synthetic pair only names the current one.
    assert pair(gate, "0.5.0", ["0.2.0", "0.3.0", "0.4.0", "0.5.0"]) == 0


def test_missing_compiler_constant_is_a_usage_error(gate, capsys):
    gate.ENVELOPE_RS.write_text("// no constant here\n", encoding="utf-8")
    gate.CRITICAL_PATH_TS.write_text(
        CRITICAL_PATH_TEMPLATE.format(entries='  "0.2.0",'), encoding="utf-8"
    )
    assert gate.main() == 2
    assert "SUPPORTED_DSL_VERSION" in capsys.readouterr().err


def test_missing_harness_declaration_is_a_usage_error(gate, capsys):
    gate.ENVELOPE_RS.write_text(
        ENVELOPE_TEMPLATE.format(version="0.9.0"), encoding="utf-8"
    )
    gate.CRITICAL_PATH_TS.write_text("// no allowlist here\n", encoding="utf-8")
    assert gate.main() == 2
    assert "SUPPORTED_DSL_VERSIONS" in capsys.readouterr().err
