"""A campaign that declares no NPC is served, and one that declares NPCs is probed for them.

After boot, `up` on a campaign asks the server for a `dw_npc` entity. A campaign
whose `npcs.json` is empty (a one-room demo level) owes none, so the probe runs
only when the campaign declares at least one NPC. The count is read by a short
Python expression inside the script; these tests run that exact expression over
an empty and a populated `npcs.json`, and check the probe sits behind it.
"""

import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "creator" / "playtest-server.sh"


def _count_expression():
    text = SCRIPT.read_text(encoding="utf-8")
    found = re.findall(r"NPCS_DECLARED=\"\$\(python3 -c '([^']+)' \"\$CAMPAIGN/npcs\.json\"\)\"", text)
    assert len(found) == 1, f"expected one NPC-count expression in {SCRIPT}, found {len(found)}"
    return found[0]


def _npcs(tmp_path, npcs):
    doc = {"campaign_id": "c", "content": {"npcs": npcs}, "dsl_version": "0.35.0", "stage": "npcs"}
    path = tmp_path / "npcs.json"
    path.write_text(json.dumps(doc), encoding="utf-8")
    return path


def _run(expression, path):
    out = subprocess.run([sys.executable, "-c", expression, str(path)], capture_output=True, text=True)
    assert out.returncode == 0, out.stderr
    return out.stdout.strip()


def test_an_empty_npcs_document_counts_zero(tmp_path):
    assert _run(_count_expression(), _npcs(tmp_path, [])) == "0"


def test_a_populated_npcs_document_counts_its_npcs(tmp_path):
    assert _run(_count_expression(), _npcs(tmp_path, [{"id": "npc/a"}, {"id": "npc/b"}])) == "2"


def test_the_npc_probe_runs_only_behind_a_positive_count():
    code = [ln for ln in SCRIPT.read_text(encoding="utf-8").splitlines() if not ln.lstrip().startswith("#")]
    probe = [i for i, ln in enumerate(code) if "@e[tag=dw_npc]" in ln]
    guard = [i for i, ln in enumerate(code) if re.search(r'if \[ "\$NPCS_DECLARED" -gt 0 \]', ln)]
    assert len(probe) == 1, f"expected one dw_npc probe, found {len(probe)}"
    assert len(guard) == 1, f"expected one NPC-count guard, found {len(guard)}"
    assert guard[0] < probe[0] <= guard[0] + 3, "the dw_npc probe is not directly inside the NPC-count guard"
