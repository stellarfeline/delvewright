"""Guards for `validation/world-save.sh`: the caller's working directory never matters.

Compose (v5, building through buildx bake) grants a build read access to the
files under the working directory it was started from, and asks for
`--allow=fs.read=<path>` for a Dockerfile anywhere else. `compose.yaml` takes the
server's Dockerfile from `validation/Dockerfile.delve`, beside the compose file
and outside the build tree, so a `compose build server` started from outside
the engine checkout is refused before it builds anything.

Docker is a stub that applies that rule: a `compose build` or `compose up
--build` started where `$DELVE_DOCKERFILE` is not under the working directory
answers with compose's refusal. The script is run from a directory outside the
copied engine tree, with and without `--seed` (the seeded boot builds first).
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parent.parent.parent

# The files `world-save.sh` runs or sources, copied into a scratch engine tree.
COPIED = (
    "validation/world-save.sh",
    "validation/fresh-volumes.sh",
    "validation/compose.yaml",
    "validation/Dockerfile.delve",
    "validation/lib/delve-image.sh",
    "validation/lib/ladder-images.sh",
    "tools/lib/rcon.sh",
)

DOCKER_STUB = r"""#!/usr/bin/env bash
# Records each compose build/up with its working directory; every other query
# answers "nothing held", so the script stops at "the server container did not
# start" once its boot was accepted.
case " $* " in
  *" compose "*" build "*|*" compose "*" up "*)
    echo "$PWD :: $*" >> "$DOCKER_LOG"
    case "$DELVE_DOCKERFILE" in
      "$PWD"/*) : ;;
      *) echo "additional privileges requested: pass \"--allow=fs.read=$DELVE_DOCKERFILE\" to grant requested privileges" >&2
         exit 1 ;;
    esac
    ;;
esac
exit 0
"""


@pytest.mark.parametrize("seeded", [False, True], ids=["unseeded", "seeded"])
def test_world_save_builds_from_a_directory_outside_the_checkout(tmp_path, seeded):
    engine = tmp_path / "engine"
    for rel in COPIED:
        dst = engine / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / rel, dst)

    tree = tmp_path / "content" / "out"
    (tree / "datapack").mkdir(parents=True)
    (tree / "server").mkdir()
    (tree / "server" / "server.properties").write_text("difficulty=peaceful\n")
    (tree / "manifest.json").write_text("{}\n")
    (tree / "critical-path.json").write_text("{}\n")
    elsewhere = tmp_path / "content"

    stubs = tmp_path / "bin"
    stubs.mkdir()
    (stubs / "docker").write_text(DOCKER_STUB)
    (stubs / "docker").chmod(0o755)
    log = tmp_path / "docker.log"
    env = {
        **os.environ,
        "PATH": f"{stubs}{os.pathsep}{os.environ['PATH']}",
        "EULA": "TRUE",
        "DOCKER_LOG": str(log),
    }
    for k in ("DELVE_DOCKERFILE", "DELVE_OUTPUT", "DELVE_IMAGE"):
        env.pop(k, None)

    args = ["bash", str(engine / "validation" / "world-save.sh"), "out", "--project", "dw-t"]
    if seeded:
        seed = tmp_path / "seed"
        seed.mkdir()
        (seed / "server.jar").write_text("x")
        args += ["--seed", str(seed)]
    r = subprocess.run(args, cwd=elsewhere, env=env, capture_output=True, text=True)

    out = r.stdout + r.stderr
    assert "additional privileges requested" not in out, out
    calls = log.read_text().splitlines()
    assert any(" up " in c for c in calls), out
    if seeded:
        assert any(" build server" in c for c in calls), out
    # It got past the boot and stopped where the stub stops it, nowhere earlier.
    assert "the server container did not start" in out, out
