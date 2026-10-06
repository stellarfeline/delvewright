"""`playtest-server.sh` serves the delve's resource pack and never installs it.

spec-0084 §11 (serving is a precondition): a build that ships
`resourcepack.zip` gets a busybox httpd sidecar, `<name>-pack`, over the build
output on the host loopback, and the staged `server.properties` carries the
three properties that point a client at it. Nothing is ever written into the
player's own `resourcepacks/` (`DELVEWRIGHT_RESOURCEPACKS_DIR`).

The end-to-end path needs a real build, the staging gate and a boot, so this
drives the script's seams through `DW_PLAYTEST_SERVER_TEST_HOOK=1`, exactly as
the heap and cleanup suite does, and holds the source to the rule where a seam
cannot reach.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "creator" / "playtest-server.sh"
BASE_PATH = f"{pathlib.Path(sys.executable).parent}:/usr/bin:/bin:/usr/sbin:/sbin"


def run_hook(code: str, path: str = BASE_PATH, extra_env=None):
    env = {"PATH": path, **(extra_env or {})}
    return subprocess.run(
        ["bash", "-c", f'DW_PLAYTEST_SERVER_TEST_HOOK=1 . "{SCRIPT}"; {code}'],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )


def pinned_pack_image() -> str:
    import tomllib

    pin = tomllib.loads((ROOT / "versions.toml").read_text(encoding="utf-8"))["images"]["pack_server"]
    return f"{pin['repo']}:{pin['tag']}@{pin['digest']}"


def test_the_sidecar_serves_the_build_output_read_only_on_the_loopback():
    result = run_hook('dw_playtest_pack_run_argv dw-x "$PACK_IMAGE" "$PACK_PORT" /builds/out')
    assert result.returncode == 0, result.stderr
    argv = result.stdout.splitlines()
    assert argv[:3] == ["docker", "run", "-d"], argv
    assert argv[argv.index("--name") + 1] == "dw-x-pack", argv
    assert argv[argv.index("-p") + 1] == "127.0.0.1:25580:8000", argv
    assert argv[argv.index("-v") + 1] == "/builds/out:/srv:ro", argv
    assert pinned_pack_image() in argv, (argv, pinned_pack_image())
    assert argv[argv.index("--entrypoint") + 1] == "httpd", argv
    assert argv[-5:] == ["-f", "-p", "8000", "-h", "/srv"], argv


def test_the_three_properties_point_a_client_at_the_pack():
    result = run_hook('dw_playtest_pack_properties http://127.0.0.1:25580/resourcepack.zip ' + "a" * 40)
    assert result.returncode == 0, result.stderr
    lines = result.stdout.splitlines()
    assert lines[0] == "resource-pack=http://127.0.0.1:25580/resourcepack.zip", lines
    assert lines[1] == "resource-pack-sha1=" + "a" * 40, lines
    key, _, prompt = lines[2].partition("=")
    assert key == "resource-pack-prompt", lines
    import json

    assert isinstance(json.loads(prompt), dict), "the prompt is a JSON text component (spec-0009)"


def test_the_resourcepacks_dir_is_never_written():
    code = "\n".join(
        ln for ln in SCRIPT.read_text(encoding="utf-8").splitlines() if not ln.lstrip().startswith("#")
    )
    # Every mention of the variable in executable code; none may be a write target.
    uses = [ln for ln in code.splitlines() if "DELVEWRIGHT_RESOURCEPACKS_DIR" in ln]
    assert uses, "the script no longer mentions DELVEWRIGHT_RESOURCEPACKS_DIR at all"
    for ln in uses:
        assert not re.search(r"\b(cp|mv|ln|install|tee|rsync)\b", ln), ln
        assert ">" not in ln.split("DELVEWRIGHT_RESOURCEPACKS_DIR", 1)[0], ln


def test_a_failed_up_removes_the_sidecar_too(tmp_path):
    fake = tmp_path / "bin"
    fake.mkdir()
    calls = tmp_path / "calls"
    (fake / "docker").write_text(
        "#!/bin/bash\n"
        f'echo "$@" >> "{calls}"\n'
        'if [ "$1" = container ] && [ "$2" = inspect ]; then exit 0; fi\n'
        "exit 0\n"
    )
    (fake / "docker").chmod(0o755)
    result = run_hook(
        'NAME=dw-x; STAGE=""; SESSION_FILE=""; UP_OK=0; dw_mutex_release() { :; }; up_failed',
        path=f"{fake}:{BASE_PATH}",
    )
    assert result.returncode == 0, result.stderr
    log = calls.read_text().splitlines()
    assert "rm -f dw-x-pack" in log, log
    assert "rm -f dw-x" in log, log
