"""`playtest-server.sh` checkpoints: a served campaign can go back to an earlier point.

The defect: a served session could only be reopened fresh from its kept image.
To play the other side of a late choice the owner had to replay the delve, or
have the choice reverted by hand over rcon, which left the world it touched
(time of day, weather, painted biomes) as the reverted session had it.

Now, while a campaign is served, a watcher saves a checkpoint of the running
world each time a quest's `#party dw.q_<quest>` score completes; `checkpoint`
saves one by hand; `restore` serves the kept image over the checkpoint's world;
`status`, `down`, `down --prune` and `rm-checkpoint` account for them.

These drive the file half (`tools/lib/playtest_checkpoint.py`) directly, and
the real verbs against a fake `docker` whose server answers rcon from a state
file the test writes.
"""

import importlib.util
import json
import os
import pathlib
import re
import subprocess
import sys
import time

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "creator" / "playtest-server.sh"
PY = ROOT / "tools" / "lib" / "playtest_checkpoint.py"
BASE_PATH = f"{pathlib.Path(sys.executable).parent}:/usr/bin:/bin:/usr/sbin:/sbin"

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from test_playtest_server_serves_from_a_kept_image import MANIFEST, sha256, token_for  # noqa: E402


def ckpt(*args):
    return subprocess.run([sys.executable, str(PY), *map(str, args)], capture_output=True, text=True)


def tellraw_complete(title):
    comp = {"color": "green", "fallback": "Objective complete: %s",
            "translate": "delve.demo.delvewright.ui.objective.complete",
            "with": [{"color": "white", "fallback": title, "translate": "delve.demo.obj.x.title"}]}
    return "tellraw @a " + json.dumps(comp)


def write_pack(root: pathlib.Path, extra_check=False):
    """A pack in the emitted shape: three quests, declared in setup; the first
    needs one titled objective, the second two (the last titled), the third
    one untitled."""
    fn = root / "data" / "demo" / "function"
    fn.mkdir(parents=True)
    (root / "pack.mcmeta").write_text("{}\n", encoding="utf-8")
    (fn / "setup.mcfunction").write_text(
        "scoreboard objectives add dw.q_gate dummy\n"
        "scoreboard objectives add dw.q_well dummy\n"
        "scoreboard objectives add dw.q_quiet dummy\n", encoding="utf-8")
    checks = {
        "gate": ["open_gate"],
        "well": ["find_rope", "draw_water"],
        "quiet": ["hush"],
    }
    for q, objs in checks.items():
        guard = " ".join(f"if score #party dw.o_{o} matches 1" for o in objs)
        (fn / f"check_q_{q}.mcfunction").write_text(
            f"execute {guard} unless score #party dw.q_{q} matches 1 run function demo:complete_q_{q}\n",
            encoding="utf-8")
    if extra_check:
        (fn / "check_q_gate_again.mcfunction").write_text(
            "execute if score #party dw.o_hush matches 1 unless score #party dw.q_gate matches 1 run function demo:x\n",
            encoding="utf-8")
    titles = {"open_gate": "Open the sea gate", "find_rope": "Find a rope", "draw_water": "Draw water", "hush": None}
    for o, title in titles.items():
        lines = [f"scoreboard players set #party dw.o_{o} 1"]
        if title:
            lines.append(tellraw_complete(title))
        (fn / f"complete_o_{o}.mcfunction").write_text("\n".join(lines) + "\n", encoding="utf-8")


# ---------------------------------------------------------------------------
# The quest table
# ---------------------------------------------------------------------------


def test_the_quest_table_names_each_quest_by_its_last_titled_objective_in_declaration_order(tmp_path):
    write_pack(tmp_path / "pack")
    r = ckpt("quests", "--datapack", tmp_path / "pack")
    assert r.returncode == 0, r.stderr
    assert r.stdout.splitlines() == [
        "dw.q_gate\tOpen the sea gate",
        "dw.q_well\tDraw water",
        "dw.q_quiet\tQuest 3 complete",
    ], r.stdout
    assert "3 quest completion score(s) declared by the pack" in r.stderr


def test_a_quest_with_two_completion_checks_is_refused_not_guessed(tmp_path):
    write_pack(tmp_path / "pack", extra_check=True)
    r = ckpt("quests", "--datapack", tmp_path / "pack")
    assert r.returncode == 1
    assert "dw.q_gate: 2 distinct completion check(s)" in r.stderr, r.stderr


def test_a_pack_with_no_quest_states_zero(tmp_path):
    (tmp_path / "pack" / "data").mkdir(parents=True)
    (tmp_path / "pack" / "pack.mcmeta").write_text("{}\n", encoding="utf-8")
    r = ckpt("quests", "--datapack", tmp_path / "pack")
    assert r.returncode == 0 and r.stdout == "", r.stdout
    assert "0 quest completion score(s)" in r.stderr


# ---------------------------------------------------------------------------
# The copy
# ---------------------------------------------------------------------------


def make_world(w: pathlib.Path):
    (w / "region").mkdir(parents=True)
    (w / "data").mkdir()
    (w / "playerdata").mkdir()
    (w / "datapacks" / "pack").mkdir(parents=True)
    (w / "level.dat").write_bytes(b"level")
    (w / "region" / "r.0.0.mca").write_bytes(b"chunks")
    (w / "data" / "scoreboard.dat").write_bytes(b"scores")
    (w / "data" / "command_storage_dw.dat").write_bytes(b"storage")
    (w / "playerdata" / "p.dat").write_bytes(b"player")
    (w / "datapacks" / "pack" / "pack.mcmeta").write_bytes(b"{}")
    (w / "session.lock").write_bytes(b"lock")


def test_the_copy_is_the_world_without_its_packs_or_lock_and_its_digest_is_re_measurable(tmp_path):
    make_world(tmp_path / "world")
    r = ckpt("copy", "--src", tmp_path / "world", "--dst", tmp_path / "ck" / "world")
    assert r.returncode == 0, r.stderr
    digest, files, size = r.stdout.strip().split("\t")
    assert files == "5", r.stdout
    copy = tmp_path / "ck" / "world"
    for rel in ("level.dat", "region/r.0.0.mca", "data/scoreboard.dat", "data/command_storage_dw.dat", "playerdata/p.dat"):
        assert (copy / rel).read_bytes() == (tmp_path / "world" / rel).read_bytes(), rel
    assert not (copy / "datapacks").exists() and not (copy / "session.lock").exists()
    again = ckpt("digest", "--dir", copy)
    assert again.stdout.strip() == digest
    (copy / "data" / "scoreboard.dat").write_bytes(b"other scores")
    assert ckpt("digest", "--dir", copy).stdout.strip() != digest, "an edited checkpoint must digest differently"


def test_a_world_written_during_the_copy_is_refused(tmp_path, monkeypatch, capsys):
    spec = importlib.util.spec_from_file_location("playtest_checkpoint", PY)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    make_world(tmp_path / "world")
    real = mod.shutil.copytree

    def copy_then_write(src, dst, **kw):
        monkeypatch.setattr(mod.shutil, "copytree", real)
        out = real(src, dst, **kw)
        (pathlib.Path(src) / "region" / "r.0.0.mca").write_bytes(b"written mid-copy")
        return out

    monkeypatch.setattr(mod.shutil, "copytree", copy_then_write)
    args = type("A", (), {"src": str(tmp_path / "world"), "dst": str(tmp_path / "ck")})
    assert mod.cmd_copy(args) == 1
    assert "region/r.0.0.mca" in capsys.readouterr().err


# ---------------------------------------------------------------------------
# Resolving a checkpoint
# ---------------------------------------------------------------------------


def fake_checkpoint(root, image_hex, seq, name, ref="delvewright/playtest:m"):
    iroot = root / image_hex
    (iroot / seq / "world").mkdir(parents=True)
    (iroot / seq / "world" / "level.dat").write_bytes(b"x" * 2048)
    (iroot / seq / "checkpoint.json").write_text(json.dumps({"name": name, "completed": []}), encoding="utf-8")
    (iroot / "image.json").write_text(json.dumps({"image_id": f"sha256:{image_hex}", "image_ref": ref,
                                                 "manifest_sha256": "m" * 8, "campaign": "demo"}), encoding="utf-8")
    return iroot / seq


def test_a_checkpoint_resolves_by_number_by_label_and_by_a_name_only_it_carries(tmp_path):
    a = fake_checkpoint(tmp_path, "a" * 64, "01", "Open the sea gate")
    b = fake_checkpoint(tmp_path, "a" * 64, "02", "Draw water")
    for q, want in (("1", a), ("02", b), ("01 Open the sea gate", a), ("Draw water", b)):
        r = ckpt("resolve", "--root", tmp_path, "--query", q)
        assert r.returncode == 0 and r.stdout.strip() == str(want), (q, r.stdout, r.stderr)


def test_a_name_two_checkpoints_carry_is_a_candidate_list_not_a_match(tmp_path):
    fake_checkpoint(tmp_path, "a" * 64, "01", "Before the gate")
    fake_checkpoint(tmp_path, "a" * 64, "03", "Before the gate")
    r = ckpt("resolve", "--root", tmp_path, "--query", "Before the gate")
    assert r.returncode == 1
    assert "names 2 checkpoints" in r.stderr and "01 Before the gate" in r.stderr and "03 Before the gate" in r.stderr


def test_a_checkpoint_of_another_build_is_refused_with_both_identities(tmp_path):
    fake_checkpoint(tmp_path, "a" * 64, "01", "Open the sea gate", ref="delvewright/playtest:old")
    (tmp_path / ("b" * 64)).mkdir()
    (tmp_path / ("b" * 64) / "image.json").write_text(json.dumps({"image_id": "sha256:" + "b" * 64,
                                                                 "image_ref": "delvewright/playtest:new"}), encoding="utf-8")
    r = ckpt("resolve", "--root", tmp_path, "--query", "1", "--image-id", "sha256:" + "b" * 64)
    assert r.returncode == 1
    assert "REFUSING" in r.stderr
    assert "delvewright/playtest:old" in r.stderr and "a" * 64 in r.stderr, r.stderr
    assert "delvewright/playtest:new" in r.stderr and "b" * 64 in r.stderr, r.stderr


def test_the_list_carries_names_and_sizes_and_nothing_else(tmp_path):
    fake_checkpoint(tmp_path, "a" * 64, "01", "Open the sea gate")
    r = ckpt("list", "--root", tmp_path)
    assert r.returncode == 0, r.stderr
    assert re.search(r"^  01 Open the sea gate  \(\d+ KiB\)$", r.stdout, re.MULTILINE), r.stdout


# ---------------------------------------------------------------------------
# The verbs, against a fake docker and a fake server
# ---------------------------------------------------------------------------

FAKE_DOCKER = r'''#!/usr/bin/env python3
import fcntl, json, os, pathlib, re, shutil, sys
STATE = pathlib.Path(__STATE__)
args = sys.argv[1:]
lock = open(str(STATE) + ".lock", "w")
fcntl.flock(lock, fcntl.LOCK_EX)
st = json.loads(STATE.read_text())
st["calls"].append(args)
def done(out="", code=0):
    tmp = pathlib.Path(str(STATE) + ".tmp")
    tmp.write_text(json.dumps(st))
    os.replace(tmp, STATE)
    if out:
        print(out)
    sys.exit(code)
def label_of(fmt):
    m = re.search(r'index \.Config\.Labels "([^"]+)"', fmt)
    return m.group(1) if m else None
def image(ref):
    if ref in st["images"]:
        return st["images"][ref]
    for img in st["images"].values():
        if img["id"] == ref:
            return img
    return None
if args[:1] == ["ps"]:
    done("")
if args[:1] == ["images"]:
    if "-q" in args:
        done("\n".join(sorted({i["id"] for i in st["images"].values()})))
    filt = [args[i + 1] for i, a in enumerate(args) if a == "--filter"]
    want = [f.split("=", 2) for f in filt if f.startswith("label=")]
    repos = [f.split("=", 1)[1] for f in filt if f.startswith("reference=")]
    hits = []
    for ref, img in st["images"].items():
        if repos and ref.rsplit(":", 1)[0] not in repos:
            continue
        if all(img["labels"].get(w[1]) == w[2] for w in want if len(w) == 3):
            hits.append(ref)
    done("\n".join(hits))
if args[:2] == ["image", "inspect"]:
    img = image(args[-1])
    if img is None:
        done(code=1)
    fmt = args[args.index("-f") + 1] if "-f" in args else ""
    if fmt == "{{.Id}}":
        done(img["id"])
    if fmt.startswith("{{if .RepoTags}}"):
        L = img["labels"]
        ref = [r for r, i in st["images"].items() if i is img][0]
        done("\t".join([ref, str(img["size"]), L.get("delvewright.playtest.manifest-sha256", ""),
                        L.get("delvewright.playtest.campaign", "")]))
    done(img["labels"].get(label_of(fmt), ""))
if args[:1] == ["rmi"]:
    ref = args[-1]
    if ref in st["images"]:
        del st["images"][ref]
        st["removed"].append(ref)
        done()
    done(code=1)
if args[:1] == ["create"]:
    done("cid-1")
if args[:1] == ["cp"]:
    src = args[1].split(":", 1)[1]
    f = pathlib.Path(st["image_files"]) / src.lstrip("/")
    if f.is_dir():
        shutil.copytree(f, args[2])
        done()
    if not f.is_file():
        done(code=1)
    shutil.copy(f, args[2])
    done()
if args[:1] == ["rm"]:
    for name in args[1:]:
        if name in st["running"]:
            st["running"].remove(name)
    done()
if args[:2] == ["container", "inspect"]:
    done(code=0 if args[2] in st["running"] else 1)
if args[:1] == ["run"]:
    name = args[args.index("--name") + 1]
    st["running"].append(name)
    mounts = [args[i + 1] for i, a in enumerate(args) if a == "-v"]
    for m in mounts:
        host, ctr = m.split(":")[:2]
        if ctr == "/data":
            world = pathlib.Path(host) / "world"
            st["served_worlds"].append(world.exists() and (world / "level.dat").read_text())
            if not world.exists():
                world.mkdir(parents=True)
                (world / "level.dat").write_text("fresh")
                (world / "session.lock").write_text("lock")
            st["stage"] = host
    done("container-id")
if args[:1] == ["logs"]:
    done("[init] fake\nDone (1.2s)! For help, type \"help\"")
if args[:1] == ["inspect"]:
    fmt = args[args.index("-f") + 1]
    name = args[-1]
    if fmt == "{{.State.Running}}":
        done("true" if name in st["running"] else "false")
    done("running" if name in st["running"] else "exited")
if args[:1] == ["exec"]:
    cmd = args[-1]
    m = re.fullmatch(r"execute if score #party (\S+) matches 1", cmd)
    if m:
        done("Test passed" if m.group(1) in st["done"] else "Test failed")
    if cmd.startswith("execute if score #placed"):
        done("Test passed")
    if cmd == "scoreboard objectives list":
        done("There are 2 objective(s): [dw.sys], [dw.class]")
    if cmd == "save-off":
        st["saving"] = False
        done("Automatic saving is now disabled")
    if cmd == "save-on":
        st["saving"] = True
        done("Automatic saving is now enabled")
    if cmd == "save-all flush":
        world = pathlib.Path(st["stage"]) / "world"
        (world / "level.dat").write_text(st["world_text"])
        done("Saving the game (this may take a moment!)\nSaved the game")
    done("Unknown or incomplete command")
done()
'''


@pytest.fixture
def served(tmp_path):
    bindir = tmp_path / "bin"
    bindir.mkdir()
    state_file = tmp_path / "docker-state.json"
    image_files = tmp_path / "image-files"
    (image_files / "delve" / "server").mkdir(parents=True)
    (image_files / "delve" / "manifest.json").write_bytes(MANIFEST)
    (image_files / "delve" / "staging-admission.json").write_bytes(token_for(MANIFEST))
    (image_files / "delve" / "server" / "server.properties").write_text("level-name=world\n", encoding="utf-8")
    (image_files / "delve" / "server" / "resources.properties").write_text("heap-max=6G\n", encoding="utf-8")
    write_pack(image_files / "delve" / "datapack")
    m = sha256(MANIFEST)
    state = {"calls": [], "running": [], "removed": [], "image_files": str(image_files), "images": {},
             "done": [], "saving": True, "stage": "", "world_text": "start", "served_worlds": []}
    docker = bindir / "docker"
    docker.write_text(FAKE_DOCKER.replace("__STATE__", repr(str(state_file))), encoding="utf-8")
    docker.chmod(0o755)
    delvec = tmp_path / "delvec"
    delvec.write_text("#!/bin/sh\nexit 9\n", encoding="utf-8")
    delvec.chmod(0o755)
    camp = tmp_path / "demo"
    camp.mkdir()
    (camp / "world.json").write_text("{}\n", encoding="utf-8")
    (camp / "npcs.json").write_text('{"content": {"npcs": []}}\n', encoding="utf-8")
    tmpdir = tmp_path / "tmp"
    tmpdir.mkdir()
    env = {
        "PATH": f"{bindir}{os.pathsep}{BASE_PATH}",
        "HOME": str(tmp_path),
        "TMPDIR": str(tmpdir),
        "DW_MUTEX_DIR": str(tmp_path / "mutex.lock.d"),
        "DW_PLAYTEST_RUNTIME_CACHE": str(tmp_path / "runtime"),
        "DW_PLAYTEST_CHECKPOINTS": str(tmp_path / "checkpoints"),
        "DW_CKPT_POLL_S": "1",
    }

    def read_state():
        return json.loads(state_file.read_text(encoding="utf-8"))

    def write_state(**kw):
        st = read_state()
        st.update(kw)
        state_file.write_text(json.dumps(st), encoding="utf-8")

    def run(*args, name="dw-t"):
        return subprocess.run(["bash", str(SCRIPT), *args, "--name", name],
                              cwd=tmp_path, env=env, capture_output=True, text=True, timeout=120)

    def keep_image(image_id="sha256:" + "a" * 64):
        r = subprocess.run(["bash", "-c", f'DW_PLAYTEST_SERVER_TEST_HOOK=1 . "{SCRIPT}"; '
                            f'dw_playtest_inputs_key "{delvec}" "{camp}" campaigns/prefabs "" "{ROOT}"'],
                           cwd=tmp_path, env={**env, "PATH": BASE_PATH}, capture_output=True, text=True)
        assert r.returncode == 0, r.stderr
        labels = {
            "delvewright.playtest.inputs": r.stdout.strip(),
            "delvewright.playtest.manifest-sha256": m,
            "delvewright.playtest.admission-sha256": sha256(token_for(MANIFEST)),
            "delvewright.playtest.pack-sha1": "",
            "delvewright.playtest.campaign": "demo",
        }
        st = read_state()
        st["images"][f"delvewright/playtest:{m}"] = {"labels": labels, "size": 912 * 1024 * 1024, "id": image_id}
        state_file.write_text(json.dumps(st), encoding="utf-8")

    state_file.write_text(json.dumps(state), encoding="utf-8")
    run.read_state, run.write_state, run.keep_image = read_state, write_state, keep_image
    run.camp, run.delvec, run.ckroot, run.tmpdir, run.manifest = camp, delvec, tmp_path / "checkpoints", tmpdir, m
    yield run
    # Leave nothing running: a watcher a failed test left behind is stopped.
    run("down")


def session(served):
    rec = {}
    p = served.tmpdir / "dw-playtest-session.dw-t"
    if p.is_file():
        for line in p.read_text(encoding="utf-8").splitlines():
            k, v = line.split("\t", 1)
            rec[k] = v
    return rec


def wait_for(pred, timeout=20):
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        if pred():
            return True
        time.sleep(0.2)
    return False


def checkpoints_of(served):
    return sorted(p for p in served.ckroot.glob("*/[0-9]*") if (p / "checkpoint.json").is_file())


def alive(pid):
    try:
        os.kill(int(pid), 0)
        return True
    except (OSError, ValueError):
        return False


def test_up_starts_a_watcher_that_checkpoints_each_completed_quest_by_name(served):
    served.keep_image()
    r = served("up", str(served.camp), "--delvec", str(served.delvec))
    assert r.returncode == 0, r.stdout + r.stderr
    assert "checkpoints: " in r.stdout and "(3 quest completion score(s) in the pack)" in r.stdout, r.stdout
    assert "automatic checkpoints: on" in r.stdout, r.stdout
    pid = session(served)["watch-pid"]
    assert alive(pid)
    assert wait_for(lambda: "baseline" in (pathlib.Path(session(served)["watch-dir"]) / "watch.log").read_text())
    served.write_state(done=["dw.q_gate"], world_text="gate open")
    assert wait_for(lambda: len(checkpoints_of(served)) == 1), (pathlib.Path(session(served)["watch-dir"]) / "watch.log").read_text()
    ck = checkpoints_of(served)[0]
    meta = json.loads((ck / "checkpoint.json").read_text())
    assert ck.name == "01" and meta["name"] == "Open the sea gate" and meta["kind"] == "auto", meta
    assert meta["completed"] == ["dw.q_gate"], meta
    assert (ck / "world" / "level.dat").read_text() == "gate open", "the copy is the world as flushed"
    assert not (ck / "world" / "session.lock").exists()
    st = served.read_state()
    assert st["saving"] is True, "save-on is owed after every save"
    cmds = [c[-1] for c in st["calls"] if c[:1] == ["exec"]]
    i = cmds.index("save-off")
    assert cmds[i:].index("save-all flush") < cmds[i:].index("save-on"), cmds[i:]
    served.write_state(done=["dw.q_gate", "dw.q_well", "dw.q_quiet"], world_text="all done")
    assert wait_for(lambda: len(checkpoints_of(served)) == 2)
    meta = json.loads((checkpoints_of(served)[1] / "checkpoint.json").read_text())
    assert meta["name"] == "Draw water + Quest 3 complete", meta
    r = served("down")
    assert r.returncode == 0, r.stderr
    assert f"checkpoint watcher stopped (pid {pid})" in r.stdout, r.stdout
    assert not alive(pid), "down left the watcher running"
    assert "checkpoints KEPT" in r.stdout and "01 Open the sea gate" in r.stdout, r.stdout
    assert len(checkpoints_of(served)) == 2, "down removed checkpoints"


def test_checkpoint_saves_one_now_and_status_lists_it_with_its_size(served):
    served.keep_image()
    assert served("up", str(served.camp), "--delvec", str(served.delvec)).returncode == 0
    r = served("checkpoint", "Before the gate")
    assert r.returncode == 0, r.stdout + r.stderr
    assert re.search(r"^checkpoint saved: 01 Before the gate \(\d+ KiB\)", r.stdout, re.MULTILINE), r.stdout
    r = served("status")
    assert re.search(r"^  01 Before the gate  \(\d+ KiB\)$", r.stdout, re.MULTILINE), r.stdout
    assert "checkpoint watcher  running" in r.stdout, r.stdout
    for bad in ("3", "a/b"):
        r = served("checkpoint", bad)
        assert r.returncode != 0, bad


def test_restore_serves_the_checkpoint_world_and_holds_the_server_to_its_scores(served):
    served.keep_image()
    assert served("up", str(served.camp), "--delvec", str(served.delvec)).returncode == 0
    served.write_state(world_text="before the gate")
    assert served("checkpoint", "Before the gate").returncode == 0
    served.write_state(done=["dw.q_gate"], world_text="gate open")
    old_stage = session(served)["stage-dir"]
    old_pid = session(served)["watch-pid"]
    r = served("restore", "1")
    # The fake server still reads dw.q_gate complete: the checkpoint recorded none.
    assert r.returncode != 0
    assert "the restored server reads complete: dw.q_gate" in r.stderr, r.stderr
    assert not alive(old_pid), "restore left the replaced session's watcher running"
    assert not pathlib.Path(old_stage).exists(), "restore left the replaced world behind"
    served.write_state(done=[])
    r = served("restore", "01 Before the gate")
    assert r.returncode == 0, r.stdout + r.stderr
    assert "RESTORED 01 Before the gate" in r.stdout, r.stdout
    assert "restored: all 3 quest score(s) read back as the checkpoint recorded them (0 complete)" in r.stdout
    assert re.search(r"^joinable \d+s after the command", r.stdout, re.MULTILINE), r.stdout
    st = served.read_state()
    assert st["served_worlds"][-1] == "before the gate", "the server must boot the checkpoint's world, not a fresh one"
    assert "automatic checkpoints: on" in r.stdout
    assert alive(session(served)["watch-pid"])


def test_restore_refuses_when_the_image_id_moved_with_both_identities(served):
    served.keep_image()
    assert served("up", str(served.camp), "--delvec", str(served.delvec)).returncode == 0
    assert served("checkpoint", "Before the gate").returncode == 0
    assert served("down").returncode == 0
    served.keep_image(image_id="sha256:" + "b" * 64)  # rebaked: same tag, another image
    r = served("restore", "1")
    assert r.returncode != 0
    assert "REFUSING to restore 01 Before the gate onto another build" in r.stderr, r.stderr
    assert "a" * 64 in r.stderr and "b" * 64 in r.stderr, r.stderr
    assert not any(c[:1] == ["run"] for c in served.read_state()["calls"][-20:])


def test_restore_refuses_a_checkpoint_of_another_build_than_the_session_serves(served):
    served.keep_image()
    assert served("up", str(served.camp), "--delvec", str(served.delvec)).returncode == 0
    assert served("checkpoint", "Before the gate").returncode == 0
    assert served("down").returncode == 0
    served.keep_image(image_id="sha256:" + "c" * 64)  # the next build the session serves
    assert served("up", str(served.camp), "--delvec", str(served.delvec)).returncode == 0
    r = served("restore", "1")
    assert r.returncode != 0
    assert "checkpoint of another build" in r.stderr, r.stderr
    assert "a" * 64 in r.stderr and "c" * 64 in r.stderr, r.stderr


def test_rm_checkpoint_removes_one_and_down_prune_removes_them_with_their_image(served):
    served.keep_image()
    assert served("up", str(served.camp), "--delvec", str(served.delvec)).returncode == 0
    assert served("checkpoint", "One").returncode == 0
    assert served("checkpoint", "Two").returncode == 0
    r = served("rm-checkpoint", "One")
    assert r.returncode == 0, r.stderr
    assert re.search(r"^checkpoint removed: 01 One \(\d+ KiB\)", r.stdout, re.MULTILINE), r.stdout
    assert [p.name for p in checkpoints_of(served)] == ["02"]
    r = served("down", "--prune")
    assert r.returncode == 0, r.stderr
    assert "checkpoints removed with their image" in r.stdout, r.stdout
    assert not checkpoints_of(served)
