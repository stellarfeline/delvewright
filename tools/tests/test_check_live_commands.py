"""Rule (1) of `tools/ci/check-live-commands.py` binds to what it judges.

The field failure: the rule reported "2 file(s) invoke rcon-cli" while both
files were allowlisted harness scripts, so its count could never reach zero and
its vacuity check could never turn red. A raw `rcon-cli` site is refused, so the
intended count of them is zero; the binding that can go vacuous is the number of
calls through the shared rule that the refusal protects.

These tests drive the rule over synthetic trees.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "ci" / "check-live-commands.py"


@pytest.fixture
def gate(tmp_path, monkeypatch):
    spec = importlib.util.spec_from_file_location("check_live_commands", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    monkeypatch.setattr(module, "ROOT", tmp_path)
    return module


def _put(root, rel: str, text: str) -> str:
    p = root / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
    return rel


EXEMPT_FILE = "docs/experiments/m2/harness.sh"
CHANNEL_USER = "tools/rig/run.sh"


def test_exempt_files_are_reported_apart_and_never_counted_as_judged(gate, tmp_path):
    _put(tmp_path, EXEMPT_FILE, "docker exec c rcon-cli list\n")
    findings, judged, exempt, sites = gate.check_channels([EXEMPT_FILE])
    assert (findings, judged, exempt, sites) == ([], 0, 1, 0)


def test_a_raw_site_is_judged_and_refused(gate, tmp_path):
    f = _put(tmp_path, "tools/rig/raw.sh", "docker exec c rcon-cli list\n")
    findings, judged, exempt, sites = gate.check_channels([f])
    assert judged == 1 and exempt == 0 and sites == 0
    assert len(findings) == 1 and "without the shared rejection rule" in findings[0]


def test_calls_through_the_shared_rule_are_the_sites_it_protects(gate, tmp_path):
    f = _put(
        tmp_path,
        CHANNEL_USER,
        'source tools/lib/rcon.sh\ndw_rcon c "say hi"\ndw_rcon_ready c\n# dw_rcon c "ignored"\n',
    )
    m = _put(tmp_path, "tools/rig/m.mjs", 'import { rconChannel } from "../lib/rcon.mjs";\nconst r = rconChannel(c);\n')
    findings, judged, exempt, sites = gate.check_channels([f, m])
    assert findings == [] and judged == 0 and exempt == 0 and sites == 3


def _run_main(gate, monkeypatch, files, capsys):
    monkeypatch.setattr(gate, "tracked_files", lambda: files)
    monkeypatch.setattr(gate, "gamerule_registry", lambda: {f"r{i}" for i in range(60)})
    monkeypatch.setattr(gate, "check_gamerules", lambda *_: ([], 1, []))
    monkeypatch.setattr(gate, "check_rule_parity", lambda: ([], 1))
    monkeypatch.setattr(gate, "check_readiness", lambda *_: ([], 1))
    code = gate.main()
    return code, capsys.readouterr()


def test_the_gate_reds_with_only_exempt_files_present(gate, tmp_path, monkeypatch, capsys):
    _put(tmp_path, EXEMPT_FILE, "docker exec c rcon-cli list\n")
    code, out = _run_main(gate, monkeypatch, [EXEMPT_FILE], capsys)
    assert code == 1
    assert "binds to nothing" in out.err
    assert "0 file(s) judged" in out.out and "1 more exempt" in out.out


def test_the_gate_is_green_when_the_shared_rule_has_call_sites(gate, tmp_path, monkeypatch, capsys):
    _put(tmp_path, EXEMPT_FILE, "docker exec c rcon-cli list\n")
    _put(tmp_path, CHANNEL_USER, 'source tools/lib/rcon.sh\ndw_rcon c "say hi"\n')
    code, out = _run_main(gate, monkeypatch, [EXEMPT_FILE, CHANNEL_USER], capsys)
    assert code == 0, out.err
    assert "1 call site(s) on the shared rule" in out.out
