"""`playtest-server.sh up` judges a campaign's world only after the world exists.

A delve builds its world after the server logs `Done (`: `setup` starts the
build, the tick re-places templates, and `setup_finish` summons the NPCs, then
releases forceloads and sets `#placed dw.sys` to 1 as its last line. A probe
that runs at `Done (` judges a world that is not there yet, and `@e` answers
only for chunks that happen to be loaded. So
`up` waits for the shared placed reading (`tools/lib/rcon.sh`, the question the
harness puts) inside a stated budget, then counts the NPCs in the world save.

These tests drive the real verification seam (`dw_playtest_verify_campaign`,
reached with `DW_PLAYTEST_SERVER_TEST_HOOK=1`) against a fake `docker` that
plays a server: its placed reading answers `Test failed` until the Nth reading,
`@e` never sees an NPC (chunks not loaded), and `save-all flush` writes the
entities region file the world holds at that moment — NPCs only once placed.
Every rcon command is logged with the placed state it was answered in.
"""

import json
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "creator" / "playtest-server.sh"
PLACEMENT_TS = ROOT / "harness" / "src" / "placement.ts"
PLACED_QUESTION = "execute if score #placed dw.sys matches 1"

FAKE_DOCKER = r'''#!/usr/bin/env python3
import json, pathlib, sys
STATE = pathlib.Path(__STATE__)
sys.path.insert(0, __LIB__)
import anvil
st = json.loads(STATE.read_text())
args = sys.argv[1:]
def save():
    STATE.write_text(json.dumps(st))
if args[:1] == ["inspect"]:
    print(st["container_state"])
    sys.exit(0)
if args[:1] == ["logs"]:
    print("fake boot log")
    sys.exit(0)
if args[:1] == ["exec"]:
    cmd = args[-1]
    if cmd == __PLACED__:
        st["readings"] += 1
        if st["readings"] <= st["unloaded_readings"]:
            reply = "Unknown scoreboard objective 'dw.sys'...#placed dw.sys<--[HERE]"
        elif st["readings"] > st["placed_after"]:
            st["placed"] = True
            reply = "Test passed"
        else:
            reply = "Test failed"
    elif cmd == "scoreboard objectives list":
        reply = "There are 2 objective(s): [dw.sys], [dw.class]" if st["readings"] > st["unloaded_readings"] else "There are no objectives"
    elif cmd.startswith("execute if entity @e[tag=dw_npc]"):
        reply = "Test failed"
    elif cmd == "save-all flush":
        n = st["npcs_on_save"] if st["placed"] else 0
        world = pathlib.Path(st["world"])
        (world / "entities").mkdir(parents=True, exist_ok=True)
        T = anvil.Tag
        ents = [T(anvil.T_COMPOUND, {"id": T(anvil.T_STRING, "minecraft:mannequin"),
                                     "Tags": T(anvil.T_LIST, [T(anvil.T_STRING, "dw_npc"), T(anvil.T_STRING, "dw_npc_%d" % i)], anvil.T_STRING)})
                for i in range(n)]
        # A hitbox carries the NPC's id tag but not `dw_npc`: never counted.
        ents.append(T(anvil.T_COMPOUND, {"id": T(anvil.T_STRING, "minecraft:interaction"),
                                         "Tags": T(anvil.T_LIST, [T(anvil.T_STRING, "dw_npc_0")], anvil.T_STRING)}))
        root = T(anvil.T_COMPOUND, {"Entities": T(anvil.T_LIST, ents, anvil.T_COMPOUND)})
        anvil.region_write(world / "entities" / "r.0.0.mca", {(3, 4): root})
        reply = "Saving the game (this may take a moment!) Saved the game"
    else:
        reply = "Unknown or incomplete command, see below for error"
    st["log"].append({"cmd": cmd, "placed": st["placed"], "reply": reply})
    save()
    print(reply)
    sys.exit(0)
sys.exit(0)
'''


def _npcs_doc(path: pathlib.Path, standing: int, deferred: int) -> None:
    npcs = [{"id": f"npc/s{i}"} for i in range(standing)]
    npcs += [{"id": f"npc/d{i}", "deferred": True} for i in range(deferred)]
    path.mkdir(parents=True, exist_ok=True)
    (path / "npcs.json").write_text(json.dumps({"campaign_id": "c", "stage": "npcs", "content": {"npcs": npcs}}))


def _serve(tmp_path, *, standing=8, deferred=4, placed_after=3, unloaded_readings=1,
           npcs_on_save=None, container_state="running", budget=60, poll=0):
    campaign = tmp_path / "campaign"
    _npcs_doc(campaign, standing, deferred)
    world = tmp_path / "stage" / "world"
    world.mkdir(parents=True)
    (world / "level.dat").write_bytes(b"")
    state = tmp_path / "server.json"
    state.write_text(json.dumps({
        "readings": 0, "placed": False, "placed_after": placed_after,
        "unloaded_readings": unloaded_readings,
        "npcs_on_save": standing if npcs_on_save is None else npcs_on_save,
        "container_state": container_state, "world": str(world), "log": [],
    }))
    bindir = tmp_path / "fakebin"
    bindir.mkdir()
    docker = bindir / "docker"
    docker.write_text(
        FAKE_DOCKER.replace("__STATE__", repr(str(state)))
        .replace("__LIB__", repr(str(ROOT / "tools" / "lib")))
        .replace("__PLACED__", repr(PLACED_QUESTION))
        .replace("#!/usr/bin/env python3", f"#!{sys.executable}", 1)
    )
    docker.chmod(0o755)
    env = {"PATH": f"{bindir}{os.pathsep}{pathlib.Path(sys.executable).parent}:/usr/bin:/bin:/usr/sbin:/sbin",
           "HOME": str(tmp_path)}
    code = (f'DW_PLAYTEST_SERVER_TEST_HOOK=1 . "{SCRIPT}"; '
            f'if dw_playtest_verify_campaign dw-test "{campaign}" "{world}" {budget} {poll}; '
            f'then echo VERIFIED; else echo "FAILED: $DW_VERIFY_FAILURE"; fi')
    out = subprocess.run(["bash", "-c", code], cwd=ROOT, env=env, capture_output=True, text=True)
    return out, json.loads(state.read_text())["log"]


def test_nothing_about_the_world_is_asked_before_the_server_says_placed(tmp_path):
    out, log = _serve(tmp_path, placed_after=3)
    assert "VERIFIED" in out.stdout, out.stdout + out.stderr
    first_placed = next(i for i, c in enumerate(log) if c["cmd"] == PLACED_QUESTION and c["reply"] == "Test passed")
    early = [c["cmd"] for c in log[:first_placed] if c["cmd"] != PLACED_QUESTION]
    assert early == [], f"the world was judged before the server said it was placed: {early}"
    # The wait was real: the server said "not yet" (and "not loaded") first.
    replies = [c["reply"] for c in log[:first_placed]]
    assert any(r == "Test failed" for r in replies) and any("<--[HERE]" in r for r in replies), replies
    assert "placement: first reading after Done, at 0s: unanswered" in out.stdout, out.stdout


def test_the_npcs_are_counted_in_the_save_where_at_e_sees_none(tmp_path):
    out, log = _serve(tmp_path, standing=8, deferred=4)
    assert "VERIFIED" in out.stdout, out.stdout + out.stderr
    assert "npc probe: 8 of 8 NPC(s) standing at world init are in the saved world (12 declared, 4 deferred" in out.stdout
    assert any(c["cmd"] == "save-all flush" and c["placed"] for c in log), log


def test_a_save_short_of_an_npc_fails_with_both_counts(tmp_path):
    out, _ = _serve(tmp_path, standing=8, deferred=4, npcs_on_save=7)
    assert "FAILED: the saved world holds 7 dw_npc entit(ies) where npcs.json has 8 standing at world init (12 declared, 4 deferred)" in out.stdout, out.stdout


def test_placement_that_never_arrives_fails_by_name_within_the_budget(tmp_path):
    out, log = _serve(tmp_path, placed_after=10**6, budget=2, poll=1)
    assert "FAILED: the datapack did not report the world placed (#placed dw.sys matches 1) within the 2s budget; last reading: pending" in out.stdout, out.stdout
    assert [c["cmd"] for c in log if c["cmd"] != PLACED_QUESTION] == [], log


def test_a_server_that_stopped_fails_by_name(tmp_path):
    out, _ = _serve(tmp_path, container_state="exited")
    assert "FAILED: the server stopped (exited) before the datapack reported the world placed" in out.stdout, out.stdout


def test_a_campaign_that_declares_no_npc_owes_none_and_saves_nothing(tmp_path):
    out, log = _serve(tmp_path, standing=0, deferred=0)
    assert "VERIFIED" in out.stdout, out.stdout + out.stderr
    assert "npc probe: 0 NPC(s) declared in npcs.json, so no dw_npc entity is owed" in out.stdout
    assert all(c["cmd"] != "save-all flush" for c in log)


def test_the_placed_reading_is_the_harness_reading():
    """One rule, two languages: the shell constants equal the harness's."""
    ts = PLACEMENT_TS.read_text(encoding="utf-8")
    sh = (ROOT / "tools" / "lib" / "rcon.sh").read_text(encoding="utf-8")

    def ts_const(name):
        m = re.findall(rf'export const {name} = ("?)([^";]+)\1;', ts)
        assert len(m) == 1, f"{name}: {m}"
        return m[0][1]

    def sh_const(name):
        m = re.findall(rf'^{name}="?([^"\n]+)"?$', sh, re.M)
        assert len(m) == 1, f"{name}: {m}"
        return m[0]

    for ts_name, sh_name in [("PLACED_HOLDER", "DW_PLACED_HOLDER"),
                             ("PLACED_OBJECTIVE", "DW_PLACED_OBJECTIVE"),
                             ("PLACED_DONE", "DW_PLACED_DONE")]:
        assert ts_const(ts_name) == sh_const(sh_name), ts_name
    assert PLACED_QUESTION == f"execute if score {ts_const('PLACED_HOLDER')} {ts_const('PLACED_OBJECTIVE')} matches {ts_const('PLACED_DONE')}"


def test_up_reaches_the_world_only_through_the_verification_seam():
    """The `up` flow asks nothing of a campaign's world between `Done (` and
    the seam: every campaign rcon question lives inside it."""
    text = SCRIPT.read_text(encoding="utf-8")
    flow = text[text.index('echo "waiting for world generation'):]
    campaign_arm = flow[flow.index('if [ "$SUBJECT_KIND" = "campaign" ]'):flow.index("\nelse\n")]
    code = [ln for ln in campaign_arm.splitlines() if not ln.lstrip().startswith("#")]
    assert any("dw_playtest_verify_campaign" in ln for ln in code), campaign_arm
    assert not any(re.search(r"\brcon(_raw)?\b|dw_rcon", ln) for ln in code), campaign_arm
    assert not any(re.search(r"\brcon(_raw)? \"", ln) for ln in flow[:flow.index('if [ "$SUBJECT_KIND" = "campaign" ]')].splitlines())


def test_a_reply_carrying_another_clients_text_is_not_an_answer(tmp_path):
    """Measured on the pinned server: rcon clients share one output buffer, so a
    reply can arrive as `Test passed` glued to someone else's answer."""
    bindir = tmp_path / "bin"
    bindir.mkdir()
    docker = bindir / "docker"
    docker.write_text("#!/bin/sh\nprintf '%s\\n' \"$DW_FAKE_REPLY\"\n")
    docker.chmod(0o755)
    lib = ROOT / "tools" / "lib" / "rcon.sh"
    for reply, expected in [("Test passed", "placed"), ("Test failed", "pending"),
                            ("Test passed#placed has 2 [dw.sys]", "unanswered Test passed#placed has 2 [dw.sys]"),
                            ("Test failed#placed has 2 [dw.sys]", "unanswered Test failed#placed has 2 [dw.sys]")]:
        out = subprocess.run(["bash", "-c", f'. "{lib}"; dw_placed_reading c'], capture_output=True, text=True,
                             env={"PATH": f"{bindir}:/usr/bin:/bin", "DW_FAKE_REPLY": reply})
        assert out.stdout.strip() == expected, (reply, out.stdout, out.stderr)
