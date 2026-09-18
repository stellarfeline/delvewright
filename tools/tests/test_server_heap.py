"""Every Minecraft server this engine starts gets the heap its delve needs, from
one authority: `versions.toml` `[server].heap_max`.

itzg's own heap ceiling is 1G. A delve's structure templates live on that heap,
and at 1G vesperhold (84 tiles plus 170 horizon templates) threw 44
`java.lang.OutOfMemoryError`s under PackTest and 83 in the bot ladder — while
only `tools/creator/playtest-server.sh` set a memory figure. The lesson had
reached one tool. This suite binds it to every entry point:

- the delve entrypoint (`validation/world-settings-entrypoint.sh`, baked
  byte-identical into `validation/Dockerfile.delve` and mounted as the PackTest
  runner's entrypoint) is RUN, and must default the ceiling to the pin while
  obeying an operator's `MEMORY` / `INIT_MEMORY` / `MAX_MEMORY`;
- every compose service across `validation/*.yaml` that boots a server image
  must run that entrypoint, and none may set a heap of its own;
- every `docker run` in a tracked shell script or workflow must run the delve
  image (whose ENTRYPOINT is that script), take the heap from
  `tools/lib/server-heap.sh`, or not start the server at all (`--entrypoint`);
- no other shell file keeps a private copy of the OOM match.

Each enumeration prints what it bound, and a zero binding is a failure.
"""

import os
import pathlib
import re
import shutil
import subprocess
import sys

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "lib"))

import versions  # noqa: E402
import workflow_yaml  # noqa: E402

ENTRYPOINT = ROOT / "validation" / "world-settings-entrypoint.sh"
DOCKERFILE_DELVE = ROOT / "validation" / "Dockerfile.delve"
HEAP_LIB = ROOT / "tools" / "lib" / "server-heap.sh"

# The images a server boots from. The delve image is built from
# Dockerfile.delve; the rest are bare itzg images (the pinned base, its
# toolserver derivative, or itzg itself), whose own entrypoint runs the server
# at itzg's 1G unless told otherwise.
BARE_SERVER_IMAGE = re.compile(
    r"itzg/minecraft-server|delvewright-base|delvewright-toolserver"
)
DELVE_IMAGE = re.compile(r"\$\{?DELVE_IMAGE\b|delvewright/delve:")


def pin() -> str:
    return versions.server_heap_max()


def tracked(*patterns: str) -> list[pathlib.Path]:
    out = subprocess.run(
        ["git", "ls-files", "-z", "--", *patterns],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    return [ROOT / p for p in out.split("\0") if p]


# ---------------------------------------------------------------------------
# The authority
# ---------------------------------------------------------------------------


def test_the_pin_is_a_size_above_itzgs_own_1g():
    value = pin()
    m = re.fullmatch(r"(\d+)([MG])", value)
    assert m, f"[server].heap_max = {value!r} is not an itzg size like 4G"
    mib = int(m.group(1)) * (1024 if m.group(2) == "G" else 1)
    assert mib > 1024, f"{value} is not above itzg's own 1G, which OOM'd vesperhold"


# ---------------------------------------------------------------------------
# The delve entrypoint, run for real (only the itzg start it hands off to is
# replaced, by a stub that prints the heap variables it would have seen)
# ---------------------------------------------------------------------------


def run_entrypoint(tmp_path, env_extra):
    stub = tmp_path / "start"
    stub.write_text(
        "#!/usr/bin/env bash\n"
        'printf "MEMORY=%s\\nINIT_MEMORY=%s\\nMAX_MEMORY=%s\\n" '
        '"${MEMORY-<unset>}" "${INIT_MEMORY-<unset>}" "${MAX_MEMORY-<unset>}"\n'
    )
    stub.chmod(0o755)
    body = ENTRYPOINT.read_text()
    # Two hand-offs: the plain `exec` and the reset loop's child.
    assert body.count("/image/scripts/start") == 2, "the entrypoint's hand-off moved"
    script = tmp_path / "entrypoint.sh"
    script.write_text(body.replace("/image/scripts/start", str(stub)))
    props = tmp_path / "server.properties"
    props.write_text("level-name=world\nlevel-seed=1\n")
    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "DELVE_SERVER_PROPERTIES": str(props),
        **env_extra,
    }
    res = subprocess.run(["bash", str(script)], env=env, capture_output=True, text=True)
    assert res.returncode == 0, res.stderr
    lines = [line for line in res.stdout.splitlines() if "=" in line and not line.startswith("[")]
    return dict(line.split("=", 1) for line in lines), res.stdout


def test_the_entrypoint_defaults_the_ceiling_to_the_pin(tmp_path):
    seen, out = run_entrypoint(tmp_path, {})
    assert seen["MAX_MEMORY"] == pin(), out
    # The initial heap is left to itzg (1G), so a small delve commits what it uses.
    assert seen["MEMORY"] == "<unset>" and seen["INIT_MEMORY"] == "<unset>", out
    assert f"MAX_MEMORY={pin()}" in out and "[init] Java heap:" in out, out


@pytest.mark.parametrize(
    "given",
    [{"MEMORY": "2G"}, {"MAX_MEMORY": "8G"}, {"INIT_MEMORY": "512M"}],
)
def test_an_operators_heap_is_obeyed_as_given(tmp_path, given):
    seen, out = run_entrypoint(tmp_path, given)
    for key in ("MEMORY", "INIT_MEMORY", "MAX_MEMORY"):
        assert seen[key] == given.get(key, "<unset>"), (key, out)


def test_the_delve_image_runs_the_entrypoint():
    text = DOCKERFILE_DELVE.read_text()
    assert re.search(r'^ENTRYPOINT \["/delve/entrypoint\.sh"\]$', text, re.M), (
        "Dockerfile.delve no longer runs /delve/entrypoint.sh — every compose "
        "service and docker run of the delve image loses its heap default"
    )
    # Byte-identity of the baked copy is validation/check-world-settings.sh's.
    assert "RUN cat > /delve/entrypoint.sh <<'EOS'" in text


# ---------------------------------------------------------------------------
# Compose: every service across validation/*.yaml, merged the way compose
# merges `-f a -f b` (later files add keys to the same service)
# ---------------------------------------------------------------------------

WORLD_ENTRYPOINT = ["/bin/bash", "/packs/world-settings-entrypoint.sh"]
# The one service that builds the delve image and does NOT boot a server: it
# runs the staging-admission verifier and exits (owner-play.yaml).
VERIFIER_ENTRYPOINT_SCRIPT = "/verify/staging-admission.sh"


def compose_services() -> dict[str, dict]:
    files = sorted((ROOT / "validation").glob("*.yaml"))
    assert files, "no compose files under validation/"
    merged: dict[str, dict] = {}
    for f in files:
        doc = workflow_yaml.load(f.read_text()) or {}
        for name, svc in (doc.get("services") or {}).items():
            merged.setdefault(name, {"_files": [], "_env_keys": set()})
            env = (svc or {}).get("environment") or {}
            merged[name]["_env_keys"] |= set(
                env.keys() if isinstance(env, dict) else [e.split("=", 1)[0] for e in env]
            )
            merged[name].update(svc or {})
            merged[name]["_files"].append(f.name)
    return merged


def classify_service(svc: dict) -> str:
    build = svc.get("build")
    dockerfile = build.get("dockerfile", "") if isinstance(build, dict) else ""
    image = svc.get("image") or ""
    entry = svc.get("entrypoint")
    delve = "Dockerfile.delve" in dockerfile
    bare = bool(BARE_SERVER_IMAGE.search(image))
    if not (delve or bare):
        return "not-a-server-image"
    env = svc.get("environment") or {}
    keys = set(env.keys() if isinstance(env, dict) else [e.split("=", 1)[0] for e in env])
    keys |= svc.get("_env_keys", set())  # every file's keys, not the last file's
    own = sorted({"MEMORY", "INIT_MEMORY", "MAX_MEMORY"} & keys)
    if own:
        return f"FAIL: sets its own heap ({', '.join(own)}) — a second authority"
    if entry == WORLD_ENTRYPOINT:
        return "server via world-settings-entrypoint"
    if entry is None and delve:
        return "server via the delve image's entrypoint"
    if isinstance(entry, list) and len(entry) >= 2 and entry[1] == VERIFIER_ENTRYPOINT_SCRIPT:
        return "verifier (runs staging-admission.sh and exits)"
    return f"FAIL: boots a server image through entrypoint {entry!r}, not the delve entrypoint"


def test_every_compose_server_runs_the_delve_entrypoint():
    services = compose_services()
    verdicts = {n: classify_service(s) for n, s in services.items()}
    servers = [n for n, v in verdicts.items() if v.startswith("server")]
    print(f"compose: {len(services)} services examined, {len(servers)} boot a server")
    for n, v in sorted(verdicts.items()):
        print(f"  {n} ({', '.join(services[n]['_files'])}): {v}")
    fails = {n: v for n, v in verdicts.items() if v.startswith("FAIL")}
    assert not fails, fails
    assert servers, "bound nothing: no compose service boots a server"
    # The packtest runner mounts the very script it runs.
    pt = services["packtest"]
    assert any("world-settings-entrypoint.sh:/packs/world-settings-entrypoint.sh" in v
               for v in pt.get("volumes", [])), pt.get("volumes")


def test_a_toolserver_service_without_the_entrypoint_is_refused():
    svc = {"image": "ghcr.io/stellarfeline/delvewright-toolserver@sha256:x",
           "environment": {"EULA": "TRUE"}}
    assert classify_service(svc).startswith("FAIL")
    svc2 = {"build": {"dockerfile": "../Dockerfile.delve"}, "environment": {"MEMORY": "1G"}}
    assert classify_service(svc2).startswith("FAIL")


# ---------------------------------------------------------------------------
# docker run: every one in a tracked shell script or workflow
# ---------------------------------------------------------------------------

DOCKER_RUN = re.compile(r"\bdocker\s+(?:container\s+)?(?:run|create)\b")
HEAP_FROM_LIB = re.compile(r'-e\s+"\$\{?(?:HEAP_ENV|heap_env)\}?"')


def logical_lines(text: str):
    """Comment lines dropped, backslash continuations joined."""
    buf, start = "", None
    for i, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not buf and line.startswith("#"):
            continue
        if buf and line.startswith("#"):
            continue
        if start is None:
            start = i
        if line.endswith("\\"):
            buf += line[:-1] + " "
            continue
        yield start, buf + line
        buf, start = "", None
    if buf:
        yield start, buf


def classify_docker_run(cmd: str, file_text: str) -> str:
    if "--entrypoint" in cmd:
        return "runs a command, not the server (--entrypoint)"
    if DELVE_IMAGE.search(cmd):
        return "server via the delve image's entrypoint"
    if HEAP_FROM_LIB.search(cmd) and "dw_server_heap_env" in file_text:
        return "server with the heap from tools/lib/server-heap.sh"
    return "FAIL: starts a container with no heap from the shared default"


def docker_run_sites():
    files = tracked("*.sh", ".github/workflows/*.yml", ".github/workflows/*.yaml")
    # Excluded, each for its reason, and counted:
    #   tools/tests/ — fixtures quote `docker run` as data under test; they
    #     start nothing;
    #   docs/ — a docs/experiments script reproduces a recorded measurement
    #     under the conditions it recorded (its own image, its own heap);
    #     changing them would change the experiment the record describes.
    excluded = [f for f in files
                if f.relative_to(ROOT).as_posix().startswith(("tools/tests/", "docs/"))]
    print(f"docker run: {len(excluded)} tracked files excluded (tools/tests/ fixtures, docs/ records)")
    files = [f for f in files if f not in excluded]
    sites = []
    for f in files:
        text = f.read_text(encoding="utf-8", errors="replace")
        for lineno, cmd in logical_lines(text):
            for m in DOCKER_RUN.finditer(cmd):
                rest = cmd[m.start():]
                sites.append((f.relative_to(ROOT).as_posix(), lineno, rest,
                              classify_docker_run(rest, text)))
    return files, sites


def test_every_docker_run_takes_the_shared_heap_or_starts_no_server():
    files, sites = docker_run_sites()
    print(f"docker run: {len(files)} tracked shell/workflow files read, {len(sites)} sites")
    for path, line, _cmd, verdict in sites:
        print(f"  {path}:{line}: {verdict}")
    fails = [(p, n, c[:160], v) for p, n, c, v in sites if v.startswith("FAIL")]
    assert not fails, fails
    servers = [s for s in sites if s[3].startswith("server")]
    assert servers, "bound nothing: no docker run of a server found"


def test_a_bare_docker_run_of_itzg_is_refused():
    cmd = 'docker run -d --name x -e EULA=TRUE itzg/minecraft-server:latest'
    assert classify_docker_run(cmd, "").startswith("FAIL")
    # Carrying the variable without the file ever calling the shared rule is
    # not taking the shared value.
    cmd2 = 'docker run -d -e "${HEAP_ENV}" itzg/minecraft-server:latest'
    assert classify_docker_run(cmd2, "HEAP_ENV=4G").startswith("FAIL")


def test_no_server_start_hides_in_another_language():
    """This suite reads shell `docker run`. A server started through a
    subprocess list, `spawn`, or `Command::new` would not be read, so any such
    form outside the test fixtures is a finding here rather than a blind spot."""
    pattern = re.compile(
        r"""["']docker["']\s*,\s*\[?\s*["'](?:run|create)["']|Command::new\(\s*"docker"\s*\)"""
    )
    hits = []
    for f in tracked("*.py", "*.mjs", "*.js", "*.ts", "*.rs"):
        rel = f.relative_to(ROOT).as_posix()
        if rel.startswith("tools/tests/") or "/node_modules/" in rel:
            continue
        for n, line in enumerate(f.read_text(errors="replace").splitlines(), 1):
            if pattern.search(line):
                hits.append(f"{rel}:{n}: {line.strip()}")
    assert not hits, hits


# ---------------------------------------------------------------------------
# One OOM rule
# ---------------------------------------------------------------------------

OOM_MATCH = re.compile(r"OutOfMemoryError")
MATCHING_CONTEXT = re.compile(r"grep|=~|\*[\"']|\bcase\b")


def test_the_oom_match_lives_in_one_place():
    files = tracked("*.sh", ".github/workflows/*.yml")
    copies = []
    for f in files:
        rel = f.relative_to(ROOT).as_posix()
        if f == HEAP_LIB:
            continue
        for n, cmd in logical_lines(f.read_text(errors="replace")):
            if OOM_MATCH.search(cmd) and MATCHING_CONTEXT.search(cmd):
                copies.append(f"{rel}:{n}: {cmd[:160]}")
    print(f"OOM rule: {len(files)} files read; the one rule is {HEAP_LIB.relative_to(ROOT)}")
    assert not copies, (
        "a private copy of the OOM match — call dw_server_log_shows_oom / "
        f"dw_server_log_file_shows_oom from tools/lib/server-heap.sh: {copies}"
    )


def run_lib(code: str, tmp_path=None):
    return subprocess.run(
        ["bash", "-c", f'. "{HEAP_LIB}"; {code}'],
        cwd=tmp_path or ROOT,
        env={"PATH": f"{pathlib.Path(sys.executable).parent}:/usr/bin:/bin"},
        capture_output=True, text=True,
    )


def test_the_watch_ends_on_an_oom_line_without_waiting_for_exit(tmp_path):
    log = tmp_path / "run.log"
    log.write_text("")
    res = run_lib(
        f'( sleep 1; echo "java.lang.OutOfMemoryError: Java heap space" >> "{log}"; exec sleep 60 ) >/dev/null 2>&1 & '
        f'p=$!; s=$(date +%s); dw_server_watch "$p" "{log}" 50 1; rc=$?; '
        'e=$(( $(date +%s) - s )); kill "$p" 2>/dev/null; echo "$rc $e"'
    )
    rc, elapsed = res.stdout.split()
    assert rc == "10", res
    assert int(elapsed) < 10, f"took {elapsed}s to notice an OOM line"


def test_the_watch_times_out_rather_than_hanging(tmp_path):
    log = tmp_path / "run.log"
    log.write_text("quiet server\n")
    res = run_lib(
        f'sleep 60 >/dev/null 2>&1 & p=$!; dw_server_watch "$p" "{log}" 2 1; rc=$?; kill "$p"; echo "$rc"'
    )
    assert res.stdout.strip() == "11", res


def test_the_watch_returns_when_the_run_ends_by_itself(tmp_path):
    log = tmp_path / "run.log"
    log.write_text("Done (1.0s)!\n")
    res = run_lib(f'sleep 1 & p=$!; dw_server_watch "$p" "{log}" 30 1; echo "$?"')
    assert res.stdout.strip() == "0", res


def test_the_lib_reads_the_pin_it_names():
    res = run_lib("dw_server_heap_env ''")
    assert res.stdout.strip() == f"MAX_MEMORY={pin()}", res
    assert shutil.which("bash")
