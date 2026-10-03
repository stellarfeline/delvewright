"""The staging gate holds `resource_pack_overrides_vanilla` to the zip (spec-0084 §4.2).

Every script that serves a delve's pack reads the manifest's statement of
what the pack carries. A build that ships a pack and states nothing, or states
it falsely, is refused before any ledger row is judged; a build with no pack
is not this rule's concern.
"""

import importlib.util
import json
import pathlib
import subprocess
import sys
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
GATE = ROOT / "tools" / "creator" / "staging-gate.py"


def gate():
    spec = importlib.util.spec_from_file_location("staging_gate", GATE)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def build(tmp: pathlib.Path, entries: list[str] | None, statement) -> pathlib.Path:
    out = tmp / "out"
    out.mkdir(parents=True)
    manifest = {"campaign_id": "c"}
    if entries is not None:
        with zipfile.ZipFile(out / "resourcepack.zip", "w") as z:
            for e in entries:
                z.writestr(e, b"x")
        manifest["resource_pack_sha1"] = "0" * 40
    if statement is not None:
        manifest["resource_pack_overrides_vanilla"] = statement
    (out / "manifest.json").write_text(json.dumps(manifest))
    return out


def test_no_pack_is_not_this_rules_concern(tmp_path):
    assert gate().pack_statement_refusal(build(tmp_path, None, None)) is None


def test_a_true_statement_passes_in_both_directions(tmp_path):
    g = gate()
    a = build(tmp_path / "a", ["pack.mcmeta", "assets/minecraft/textures/block/stone.png"], True)
    b = build(tmp_path / "b", ["pack.mcmeta", "assets/delvewright/lang/en_us.json"], False)
    assert g.pack_statement_refusal(a) is None
    assert g.pack_statement_refusal(b) is None


def test_a_missing_or_false_statement_is_refused(tmp_path):
    g = gate()
    missing = build(tmp_path / "m", ["pack.mcmeta"], None)
    lying = build(tmp_path / "l", ["pack.mcmeta", "assets/minecraft/textures/block/stone.png"], False)
    assert "does not state" in g.pack_statement_refusal(missing)
    assert "false" in g.pack_statement_refusal(lying)


def test_the_cli_refuses_before_the_ledger(tmp_path):
    (tmp_path / "camp").mkdir()
    out = build(tmp_path, ["pack.mcmeta", "assets/minecraft/textures/block/stone.png"], False)
    r = subprocess.run(
        [sys.executable, str(GATE), "--campaign", str(tmp_path / "camp"), "--build", str(out)],
        capture_output=True,
        text=True,
    )
    assert r.returncode == 1, r.stderr
    assert "spec-0084" in r.stderr, r.stderr
