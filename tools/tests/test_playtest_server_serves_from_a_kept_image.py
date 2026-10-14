"""`playtest-server.sh up` on a campaign serves from a kept image, and reopening is `docker run`.

The defect: closing a served campaign meant `down`, and reopening meant `up`,
which rebuilt the campaign, re-ran the staging gate and waited for the world to
be placed — more than ten minutes on The Stranding, while the owner waited.

Now the first `up` of a campaign ends in two local images keyed by the build's
manifest — the shipped delve image (validation/Dockerfile.delve) and over it the
playtest image holding the world that image placed (validation/Dockerfile.playtest)
— labelled with an INPUTS key computed before building. A later `up` whose key
matches a kept image goes straight to `docker run`, after proving the image's own
manifest and admission token agree with its labels and with each other.

These drive the script's seams through `DW_PLAYTEST_SERVER_TEST_HOOK=1`, and the
real `up`/`down`/`status` against a fake `docker` and a `delvec` that refuses to
build — so a reopen that built anything reds.
"""

import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "creator" / "playtest-server.sh"
VERIFIER = ROOT / "validation" / "staging-admission.sh"
BASE_PATH = f"{pathlib.Path(sys.executable).parent}:/usr/bin:/bin:/usr/sbin:/sbin"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run_hook(code: str, extra_env=None, cwd=ROOT):
    env = {"PATH": BASE_PATH, "HOME": "/nonexistent", **(extra_env or {})}
    return subprocess.run(
        ["bash", "-c", f'DW_PLAYTEST_SERVER_TEST_HOOK=1 . "{SCRIPT}"; {code}'],
        cwd=cwd, env=env, capture_output=True, text=True,
    )


def token_for(manifest: bytes, campaign: str = "demo", overridden: bool = False) -> bytes:
    """A token in the format `staging-gate.py write_admission` writes."""
    doc = {
        "_staging_gate_artifact": True,
        "build_fingerprint": sha256(manifest),
        "campaign": campaign,
        "findings_total": 1,
        "inapplicable": [],
        "inapplicable_count": 0,
        "ledger_digest": "0" * 64,
        "override": None,
        "overridden": overridden,
        "proven": None,
        "red_count": 0,
        "reds": [],
        "schema": 1,
    }
    return (json.dumps(doc, indent=2, sort_keys=True) + "\n").encode()


MANIFEST = b'{\n  "files": {},\n  "schema": 1\n}\n'


# ---------------------------------------------------------------------------
# The argv seams
# ---------------------------------------------------------------------------


def test_the_serving_run_publishes_25565_and_mounts_a_fresh_data_dir():
    result = run_hook('dw_playtest_serve_argv dw-x delvewright/playtest:abc playtest MAX_MEMORY=6G /stage ""')
    assert result.returncode == 0, result.stderr
    argv = result.stdout.splitlines()
    assert argv[:3] == ["docker", "run", "-d"], argv
    assert argv[argv.index("--name") + 1] == "dw-x", argv
    assert argv[argv.index("-p") + 1] == "25565:25565", argv
    assert "MAX_MEMORY=6G" in argv and "RCON_PASSWORD=playtest" in argv, argv
    assert argv[-3:] == ["-v", "/stage:/data", "delvewright/playtest:abc"], argv
    assert not any(a.startswith("DELVE_RESOURCE_PACK_URL=") for a in argv), argv


def test_a_pack_reaches_the_server_through_the_delve_entrypoint():
    result = run_hook('dw_playtest_serve_argv dw-x img playtest MEMORY=8G /stage http://127.0.0.1:25580/resourcepack.zip')
    argv = result.stdout.splitlines()
    assert "DELVE_RESOURCE_PACK_URL=http://127.0.0.1:25580/resourcepack.zip" in argv, argv


def test_the_placement_boot_publishes_no_port():
    result = run_hook('dw_playtest_place_argv dw-x delvewright/playtest-build:abc playtest MAX_MEMORY=6G /place ""')
    argv = result.stdout.splitlines()
    assert argv[argv.index("--name") + 1] == "dw-x-place", argv
    assert "-p" not in argv, "nobody may join a world that is still being built"
    assert not any(a.startswith("DATAPACKS=") for a in argv), argv
    assert argv[-1] == "delvewright/playtest-build:abc", argv


def test_the_placement_boot_runs_without_the_vanilla_watchdog_and_the_serving_run_with_it():
    """A placement tick past 60 s on a loaded host made the watchdog halt the
    server (exit 1) mid-placement. Reproduced with `--cpus 0.15`: red with the
    shipped default, placed with `MAX_TICK_TIME=-1`."""
    place = run_hook('dw_playtest_place_argv dw-x img playtest MAX_MEMORY=6G /place ""').stdout.splitlines()
    assert "MAX_TICK_TIME=-1" in place, place
    assert place[place.index("MAX_TICK_TIME=-1") - 1] == "-e", place
    serve = run_hook('dw_playtest_serve_argv dw-x img playtest MAX_MEMORY=6G /stage ""').stdout.splitlines()
    assert not any(a.startswith("MAX_TICK_TIME=") for a in serve), "the serving run keeps the shipped watchdog"


def test_a_watchdog_halt_is_named_as_one():
    log = "[21:33:40] [Server Watchdog/ERROR]: A single server tick took 60.00 seconds (should be max 0.05)"
    assert run_hook(f'dw_server_log_shows_watchdog "{log}"').returncode == 0
    assert run_hook('dw_server_log_shows_watchdog "Done (3.0s)!"').returncode == 1
    text = SCRIPT.read_text(encoding="utf-8")
    body = text[text.index("die_or_oom() {"):]
    body = body[: body.index("\n}\n")]
    assert "dw_server_log_shows_watchdog" in body and "watchdog halted it" in body


def test_the_placement_boot_places_with_the_creator_overlay_it_is_served_with():
    result = run_hook('dw_playtest_place_argv dw-x img playtest MAX_MEMORY=6G /place /out/creator-datapack')
    argv = result.stdout.splitlines()
    assert "/out/creator-datapack:/delve/extras/creator-datapack:ro" in argv, argv
    assert "DATAPACKS=/delve/datapack,/delve/extras/creator-datapack" in argv, argv
    dockerfile = (ROOT / "validation" / "Dockerfile.playtest").read_text(encoding="utf-8")
    assert "COPY extras /delve/extras" in dockerfile


# ---------------------------------------------------------------------------
# The inputs key
# ---------------------------------------------------------------------------


@pytest.fixture
def inputs(tmp_path):
    """A fake engine root holding every input file, a campaign, prefabs, a delvec."""
    root = tmp_path / "engine"
    (root / "tools" / "ci").mkdir(parents=True)
    shutil.copy(ROOT / "tools" / "ci" / "tree-digest.py", root / "tools" / "ci" / "tree-digest.py")
    files = run_hook('printf "%s\\n" $DW_PLAYTEST_INPUT_FILES').stdout.split()
    assert len(files) >= 6, files
    for rel in files:
        (root / rel).parent.mkdir(parents=True, exist_ok=True)
        (root / rel).write_text(f"{rel}\n", encoding="utf-8")
    camp = tmp_path / "campaign"
    camp.mkdir()
    (camp / "world.json").write_text("{}\n", encoding="utf-8")
    pre = tmp_path / "prefabs"
    pre.mkdir()
    (pre / "a.nbt").write_bytes(b"nbt")
    delvec = tmp_path / "delvec"
    delvec.write_bytes(b"engine-1")

    def key(lang="en"):
        r = run_hook(f'dw_playtest_inputs_key "{delvec}" "{camp}" "{pre}" "{lang}" "{root}"',
                     extra_env={"TMPDIR": str(tmp_path)})
        assert r.returncode == 0, r.stderr
        k = r.stdout.strip()
        assert re.fullmatch(r"[0-9a-f]{64}", k), r.stdout
        return k, r.stderr

    key.root, key.camp, key.pre, key.delvec, key.files, key.tmp = root, camp, pre, delvec, files, tmp_path
    return key


def test_the_same_inputs_make_the_same_key_and_say_what_they_were(inputs):
    k1, shown = inputs()
    k2, _ = inputs()
    assert k1 == k2
    for name in ("delvec", "campaign", "prefabs", "lang", *inputs.files):
        assert f"  inputs: {name} " in shown, (name, shown)
    leftover = [p.name for p in inputs.tmp.iterdir() if p.name.startswith("dw-playtest-")]
    assert not leftover, f"scratch directories left behind: {leftover}"


def test_every_input_moves_the_key(inputs):
    base, _ = inputs()
    perturbations = {
        "campaign": lambda: (inputs.camp / "world.json").write_text('{"x":1}\n', encoding="utf-8"),
        "prefabs": lambda: (inputs.pre / "a.nbt").write_bytes(b"nbt2"),
        "delvec": lambda: inputs.delvec.write_bytes(b"engine-2"),
    }
    seen = {base}
    for name, perturb in perturbations.items():
        perturb()
        k, _ = inputs()
        assert k not in seen, f"changing the {name} did not move the key"
        seen.add(k)
    k, _ = inputs(lang="zh-CN")
    assert k not in seen, "changing --lang did not move the key"
    seen.add(k)
    for rel in inputs.files:
        (inputs.root / rel).write_text(f"{rel} edited\n", encoding="utf-8")
        k, _ = inputs(lang="zh-CN")
        assert k not in seen, f"editing {rel} did not move the key"
        seen.add(k)


def test_an_unreadable_input_fails_the_key_rather_than_dropping_out(inputs):
    (inputs.root / inputs.files[0]).unlink()
    r = run_hook(f'dw_playtest_inputs_key "{inputs.delvec}" "{inputs.camp}" "{inputs.pre}" en "{inputs.root}"',
                 extra_env={"TMPDIR": str(inputs.tmp)})
    assert r.returncode != 0
    assert inputs.files[0] in r.stderr, r.stderr


# ---------------------------------------------------------------------------
# The identity check
# ---------------------------------------------------------------------------


def identity(tmp_path, manifest=MANIFEST, token=None, tag=None, lmanifest=None, ladmit=None):
    d = tmp_path / "image-files"
    d.mkdir(exist_ok=True)
    if manifest is not None:
        (d / "manifest.json").write_bytes(manifest)
    tok = token_for(MANIFEST) if token is None else token
    if tok is not False:
        (d / "staging-admission.json").write_bytes(tok)
    m = sha256(MANIFEST)
    a = sha256(tok) if tok else ""
    r = run_hook(
        f'dw_playtest_identity_check "{d}" "{tag or m}" "{lmanifest or m}" "{ladmit or a}" '
        '|| { echo "FAILURE: $DW_IDENTITY_FAILURE"; exit 3; }'
    )
    return r


def test_an_image_whose_files_agree_with_its_labels_is_admitted(tmp_path):
    r = identity(tmp_path)
    assert r.returncode == 0, r.stdout + r.stderr
    assert "admitted" in r.stdout, r.stdout


def test_a_manifest_that_is_not_the_tagged_one_is_refused_named(tmp_path):
    r = identity(tmp_path, manifest=MANIFEST + b" ")
    assert r.returncode == 3, r.stdout
    assert "manifest mismatch" in r.stdout, r.stdout


def test_a_token_that_is_not_the_labelled_one_is_refused_named(tmp_path):
    r = identity(tmp_path, ladmit="f" * 64)
    assert r.returncode == 3, r.stdout
    assert "admission mismatch" in r.stdout, r.stdout


def test_a_token_minted_for_another_build_is_refused_even_with_matching_labels(tmp_path):
    r = identity(tmp_path, token=token_for(b"another build"))
    assert r.returncode == 3, r.stdout
    assert "does not admit its own manifest" in r.stdout, r.stdout


def test_an_image_with_no_token_is_refused(tmp_path):
    r = identity(tmp_path, token=False)
    assert r.returncode == 3, r.stdout
    assert "never admitted" in r.stdout, r.stdout


# ---------------------------------------------------------------------------
# The bake context
# ---------------------------------------------------------------------------


def test_the_bake_context_carries_a_readable_world_without_its_packs_or_lock(tmp_path):
    world = tmp_path / "data" / "world"
    (world / "datapacks" / "datapack").mkdir(parents=True)
    (world / "region").mkdir()
    (world / "level.dat").write_bytes(b"level")
    (world / "level.dat").chmod(0o600)
    (world / "region" / "r.0.0.mca").write_bytes(b"r")
    (world / "region" / "r.0.0.mca").chmod(0o600)
    (world / "session.lock").write_bytes(b"lock")
    token = tmp_path / "staging-admission.json"
    token.write_bytes(token_for(MANIFEST))
    creator = tmp_path / "creator-datapack"
    creator.mkdir()
    (creator / "pack.mcmeta").write_text("{}", encoding="utf-8")
    pack = tmp_path / "resourcepack.zip"
    pack.write_bytes(b"zip")
    bake = tmp_path / "bake"
    r = run_hook(f'dw_playtest_bake_context "{world}" "{bake}" "{token}" "{creator}" "{pack}"')
    assert r.returncode == 0, r.stderr
    assert r.stdout.strip() == "/delve/datapack,/delve/extras/creator-datapack", r.stdout
    placed = bake / "placed-world"
    assert not world.exists(), "the world is moved, not copied"
    assert (placed / "level.dat").is_file() and (placed / "region" / "r.0.0.mca").is_file()
    assert not (placed / "datapacks").exists() and not (placed / "session.lock").exists()
    for f in (placed / "level.dat", placed / "region" / "r.0.0.mca"):
        assert f.stat().st_mode & 0o004, f"{f} is not readable by the image's server user"
    assert (bake / "staging-admission.json").read_bytes() == token.read_bytes()
    assert (bake / "extras" / "creator-datapack" / "pack.mcmeta").is_file()
    assert (bake / "extras" / "resourcepack.zip").read_bytes() == b"zip"
    world2 = tmp_path / "data2" / "world"
    world2.mkdir(parents=True)
    r = run_hook(f'dw_playtest_bake_context "{world2}" "{tmp_path / "bake2"}" "{token}" "" ""')
    assert r.stdout.strip() == "/delve/datapack", r.stdout
    assert not any((tmp_path / "bake2" / "extras").iterdir())


# ---------------------------------------------------------------------------
# The server runtime cache
# ---------------------------------------------------------------------------


def test_the_runtime_is_kept_and_seeded_only_when_the_jar_is_the_pinned_one(tmp_path):
    data = tmp_path / "data"
    (data / "libraries" / "x").mkdir(parents=True)
    (data / "libraries" / "x" / "lib.jar").write_bytes(b"lib")
    (data / "minecraft_server.1.21.11.jar").write_bytes(b"jar")
    (data / ".vanilla-manifest.json").write_text("{}", encoding="utf-8")
    (data / "world").mkdir()
    cache = tmp_path / "cache"
    good = sha256(b"jar")
    r = run_hook(f'dw_playtest_runtime_keep "{data}" "{cache}" 1.21.11 {"0" * 64}')
    assert "NOT kept" in r.stdout and not cache.exists(), r.stdout
    r = run_hook(f'dw_playtest_runtime_keep "{data}" "{cache}" 1.21.11 {good}')
    assert r.returncode == 0 and "kept" in r.stdout, r.stdout + r.stderr
    assert (cache / "libraries" / "x" / "lib.jar").is_file()
    assert not (cache / "world").exists(), "a world is never part of the runtime"
    fresh = tmp_path / "fresh"
    fresh.mkdir()
    r = run_hook(f'dw_playtest_runtime_seed "{cache}" "{fresh}" 1.21.11 {"0" * 64}')
    assert "not used" in r.stdout and not any(fresh.iterdir()), r.stdout
    r = run_hook(f'dw_playtest_runtime_seed "{cache}" "{fresh}" 1.21.11 {good}')
    assert (fresh / "minecraft_server.1.21.11.jar").read_bytes() == b"jar", r.stdout
    assert (fresh / ".vanilla-manifest.json").is_file()


# ---------------------------------------------------------------------------
# The real `up` / `down` / `status`, against a fake docker
# ---------------------------------------------------------------------------

FAKE_DOCKER = r'''#!/usr/bin/env python3
import json, pathlib, shutil, sys
STATE = pathlib.Path(__STATE__)
st = json.loads(STATE.read_text())
args = sys.argv[1:]
st["calls"].append(args)
def done(out="", code=0):
    STATE.write_text(json.dumps(st))
    if out:
        print(out)
    sys.exit(code)
def label_of(fmt):
    import re
    m = re.search(r'index \.Config\.Labels "([^"]+)"', fmt)
    return m.group(1) if m else None
if args[:1] == ["ps"]:
    done("")
if args[:1] == ["images"]:
    if "-q" in args:
        done("\n".join(st["images"].keys()))
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
    ref = args[-1]
    img = st["images"].get(ref)
    if img is None:
        done(code=1)
    fmt = args[args.index("-f") + 1] if "-f" in args else ""
    if fmt.startswith("{{if .RepoTags}}"):
        L = img["labels"]
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
    files = pathlib.Path(st["image_files"])
    f = files / src.lstrip("/")
    if not f.is_file():
        done(code=1)
    shutil.copy(f, args[2])
    done()
if args[:1] == ["rm"]:
    done()
if args[:2] == ["container", "inspect"]:
    done(code=0 if args[2] in st["running"] else 1)
if args[:1] == ["run"]:
    name = args[args.index("--name") + 1]
    st["running"].append(name)
    done("container-id")
if args[:1] == ["logs"]:
    done("[init] fake\nDone (1.2s)! For help, type \"help\"")
if args[:1] == ["inspect"]:
    done("running")
if args[:1] == ["exec"]:
    cmd = args[-1]
    if cmd.startswith("execute if score #placed"):
        done("Test passed")
    if cmd == "scoreboard objectives list":
        done("There are 2 objective(s): [dw.sys], [dw.class]")
    done("Unknown or incomplete command")
done()
'''


@pytest.fixture
def served(tmp_path):
    """A campaign with no NPCs, a delvec that refuses to build, and a fake docker
    whose image store holds whatever the test puts in `state["images"]`."""
    bindir = tmp_path / "bin"
    bindir.mkdir()
    state_file = tmp_path / "docker-state.json"
    image_files = tmp_path / "image-files"
    (image_files / "delve" / "server").mkdir(parents=True)
    (image_files / "delve" / "manifest.json").write_bytes(MANIFEST)
    (image_files / "delve" / "staging-admission.json").write_bytes(token_for(MANIFEST))
    (image_files / "delve" / "server" / "server.properties").write_text("level-name=world\n", encoding="utf-8")
    (image_files / "delve" / "server" / "resources.properties").write_text("heap-max=6G\n", encoding="utf-8")
    m = sha256(MANIFEST)
    state = {"calls": [], "running": [], "removed": [], "image_files": str(image_files), "images": {}}

    def keep_image(inputs_key, admission=None):
        labels = {
            "delvewright.playtest.inputs": inputs_key,
            "delvewright.playtest.manifest-sha256": m,
            "delvewright.playtest.admission-sha256": admission or sha256(token_for(MANIFEST)),
            "delvewright.playtest.pack-sha1": "",
            "delvewright.playtest.campaign": "demo",
        }
        state["images"][f"delvewright/playtest:{m}"] = {"labels": labels, "size": 912 * 1024 * 1024}
        state["images"][f"delvewright/playtest-build:{m}"] = {"labels": labels, "size": 870 * 1024 * 1024}

    docker = bindir / "docker"
    docker.write_text(FAKE_DOCKER.replace("__STATE__", repr(str(state_file))), encoding="utf-8")
    docker.chmod(0o755)
    delvec = tmp_path / "delvec"
    delvec.write_text("#!/bin/sh\necho \"delvec $* — a reopen must not build\" >&2\nexit 9\n", encoding="utf-8")
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
    }

    def run(*args):
        state_file.write_text(json.dumps(state), encoding="utf-8")
        r = subprocess.run(["bash", str(SCRIPT), *args, "--name", "dw-t"],
                           cwd=tmp_path, env=env, capture_output=True, text=True)
        state.update(json.loads(state_file.read_text(encoding="utf-8")))
        return r

    def key():
        r = run_hook(f'dw_playtest_inputs_key "{delvec}" "{camp}" campaigns/prefabs "" "{ROOT}"',
                     extra_env={"TMPDIR": str(tmpdir)}, cwd=tmp_path)
        assert r.returncode == 0, r.stderr
        return r.stdout.strip()

    run.state, run.keep_image, run.key, run.camp, run.delvec = state, keep_image, key, camp, delvec
    run.image_files, run.mutex, run.manifest = image_files, tmp_path / "mutex.lock.d", m
    return run


def test_a_kept_image_is_served_by_docker_run_with_no_build_gate_or_placement(served):
    served.keep_image(served.key())
    r = served("up", str(served.camp), "--delvec", str(served.delvec))
    assert r.returncode == 0, r.stdout + r.stderr
    assert "a reopen must not build" not in r.stderr, "the reopen ran delvec"
    assert f"kept image: delvewright/playtest:{served.manifest}" in r.stdout, r.stdout
    assert "staging gate:" not in r.stdout, "the reopen ran the staging gate"
    runs = [c for c in served.state["calls"] if c[:1] == ["run"]]
    assert len(runs) == 1, runs
    assert runs[0][-1] == f"delvewright/playtest:{served.manifest}", runs[0]
    assert not any(c[:1] == ["build"] for c in served.state["calls"]), "a reopen built an image"
    assert "MAX_MEMORY=6G" in runs[0], "the heap is the build's own statement, read from the image"
    assert "READY — Multiplayer -> Direct Connect: localhost:25565" in r.stdout, r.stdout
    assert re.search(r"^joinable \d+s after the command", r.stdout, re.MULTILINE), r.stdout
    assert f"identity: delvewright/playtest:{served.manifest} serves manifest {served.manifest}" in r.stdout
    assert (served.mutex / "HOLDER").read_text().startswith("owner-play-session")


def test_a_kept_image_whose_token_disagrees_with_its_label_is_refused_before_it_runs(served):
    served.keep_image(served.key(), admission="e" * 64)
    r = served("up", str(served.camp), "--delvec", str(served.delvec))
    assert r.returncode != 0
    assert "REFUSING to serve" in r.stderr and "admission mismatch" in r.stderr, r.stderr
    assert not any(c[:1] == ["run"] for c in served.state["calls"]), "a refused image was run"
    assert not served.mutex.exists(), "a refused reopen kept the 25565 mutex"


def test_a_kept_image_for_other_inputs_is_not_served(served):
    served.keep_image("0" * 64)
    r = served("up", str(served.camp), "--delvec", str(served.delvec))
    assert r.returncode != 0
    assert "kept image: none for these inputs" in r.stdout, r.stdout
    assert "a reopen must not build" in r.stderr, "with no matching image the build must run"


def test_down_keeps_the_images_and_prints_their_sizes(served):
    served.keep_image("k")
    r = served("down")
    assert r.returncode == 0, r.stderr
    assert "playtest images KEPT" in r.stdout, r.stdout
    assert f"delvewright/playtest:{served.manifest}  campaign demo  manifest {served.manifest}  (912.0 MiB)" in r.stdout
    assert not served.state["removed"]


def test_down_prune_removes_every_kept_image_child_first_with_sizes(served):
    served.keep_image("k")
    r = served("down", "--prune")
    assert r.returncode == 0, r.stderr
    assert served.state["removed"] == [
        f"delvewright/playtest:{served.manifest}",
        f"delvewright/playtest-build:{served.manifest}",
    ], served.state["removed"]
    assert f"image removed: delvewright/playtest:{served.manifest}  campaign demo  (912.0 MiB)" in r.stdout
    assert f"image removed: delvewright/playtest-build:{served.manifest}  campaign demo  (870.0 MiB)" in r.stdout


def test_status_lists_the_kept_images_with_manifest_and_size(served):
    served.keep_image("k")
    r = served("status")
    assert r.returncode == 0, r.stderr
    assert "kept playtest images" in r.stdout
    assert f"delvewright/playtest:{served.manifest}  campaign demo  manifest {served.manifest}  (912.0 MiB)" in r.stdout


def test_prune_belongs_to_down(served):
    r = served("status", "--prune")
    assert r.returncode != 0 and "--prune belongs to down" in r.stderr
