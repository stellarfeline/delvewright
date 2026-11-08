"""`playtest-server.sh` ops the creator on every world it serves.

`validation/compose.yaml` ops `CREATOR_NAME`; `up` and `restore` take the same
variable, write an offline-mode `ops.json` into the served `/data`, and refuse
before any build when the name is missing or is not a Minecraft name. Driven
through `DW_PLAYTEST_SERVER_TEST_HOOK=1`, as the pack suite is.
"""

import hashlib
import json
import pathlib
import re
import subprocess
import sys
import uuid

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "creator" / "playtest-server.sh"
BASE_PATH = f"{pathlib.Path(sys.executable).parent}:/usr/bin:/bin:/usr/sbin:/sbin"


def run_hook(code: str, extra_env=None):
    return subprocess.run(
        ["bash", "-c", f'DW_PLAYTEST_SERVER_TEST_HOOK=1 . "{SCRIPT}"; {code}'],
        cwd=ROOT,
        env={"PATH": BASE_PATH, **(extra_env or {})},
        capture_output=True,
        text=True,
    )


def test_ops_json_is_the_offline_uuid_of_the_name():
    r = run_hook("dw_playtest_ops_json Stella_1")
    assert r.returncode == 0, r.stderr
    [entry] = json.loads(r.stdout)
    want = uuid.UUID(bytes=hashlib.md5(b"OfflinePlayer:Stella_1").digest(), version=3)
    assert entry == {"uuid": str(want), "name": "Stella_1", "level": 4, "bypassesPlayerLimit": False}


def test_a_missing_or_malformed_creator_name_is_refused():
    assert run_hook("dw_playtest_creator_name").returncode != 0
    assert run_hook("dw_playtest_creator_name", {"CREATOR_NAME": ""}).returncode != 0
    for bad in ("ab", "x" * 17, 'a"b[]', "a b c"):
        assert run_hook("dw_playtest_creator_name", {"CREATOR_NAME": bad}).returncode != 0, bad
    ok = run_hook("dw_playtest_creator_name", {"CREATOR_NAME": "Stella_1"})
    assert ok.returncode == 0 and ok.stdout == "Stella_1"


def test_up_and_restore_refuse_before_any_work_without_the_name():
    for args in (["up", "/nonexistent"], ["restore", "1"]):
        r = subprocess.run(
            ["bash", str(SCRIPT), *args], cwd=ROOT, env={"PATH": BASE_PATH}, capture_output=True, text=True
        )
        assert r.returncode != 0
        assert "CREATOR_NAME is not set" in r.stderr, r.stderr


def test_both_serving_paths_write_ops_json_into_the_stage():
    code = "\n".join(
        ln for ln in SCRIPT.read_text(encoding="utf-8").splitlines() if not ln.lstrip().startswith("#")
    )
    assert len(re.findall(r'dw_playtest_ops_json "\$CREATOR" > "\$STAGE/ops.json"', code)) == 2
