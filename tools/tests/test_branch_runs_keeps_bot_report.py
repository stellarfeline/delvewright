"""Guards for `validation/branch-runs.sh`: it never touches the bot's run report.

The ladder page runs the critical-path bot (`validation/bot-run.sh --project P`),
then the branch runs under the same `--project P`, then the staging gate, which
reads `validation/run-out/P/run-report.json`. `bot-run.sh` writes that file;
`branch-runs.sh` must leave it as it found it, or the documented sequence cannot
be walked green.

Docker and node are stubs: the subject is which files the script deletes and
where its bot writes, and a real server would make this slower without making
it stronger. The stub `docker compose up` writes the bot's report where the
compose mount would put it — `validation/$DW_BOT_OUT/run-report.json`.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent

# The files `branch-runs.sh` runs or sources, copied into a scratch engine tree.
COPIED = (
    "validation/branch-runs.sh",
    "validation/fresh-volumes.sh",
    "validation/compose.yaml",
    "validation/lib/delve-image.sh",
    "validation/lib/ladder-images.sh",
    "tools/lib/server-heap.sh",
)

DOCKER_STUB = r"""#!/usr/bin/env bash
# Every query answers "nothing held"; `compose ... up` plays the bot.
case " $* " in
  *" up "*)
    out="validation/${DW_BOT_OUT#./}"
    mkdir -p "$out"
    printf '{"branches":{"tier":"all","outcomes":[{"branch":"%s","ran":true,"passed":true}]}}\n' \
      "$DELVEWRIGHT_BRANCH" > "$out/run-report.json"
    ;;
esac
exit 0
"""

NODE_STUB = "#!/usr/bin/env bash\necho world/a\n"


def _exe(path: Path, text: str) -> None:
    path.write_text(text)
    path.chmod(0o755)


def test_branch_runs_under_the_bots_project_keeps_its_run_report(tmp_path):
    engine = tmp_path / "engine"
    for rel in COPIED:
        dst = engine / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / rel, dst)

    out = tmp_path / "delve-output"
    (out / "validation").mkdir(parents=True)
    (out / "critical-path.json").write_text("{}\n")
    (out / "validation" / "branch-plan.json").write_text(
        json.dumps(
            {
                "campaign_id": "c",
                "branches": [{"id": "world/a", "chronicle": None, "endings": []}],
            }
        )
    )

    project = "dw-c-r1"
    # What `bot-run.sh --project dw-c-r1` left for the staging gate.
    bot_report = engine / "validation" / "run-out" / project / "run-report.json"
    bot_report.parent.mkdir(parents=True)
    bot_report.write_text('{"the":"critical path"}\n')

    stubs = tmp_path / "bin"
    stubs.mkdir()
    _exe(stubs / "docker", DOCKER_STUB)
    _exe(stubs / "node", NODE_STUB)
    env = {
        **os.environ,
        "PATH": f"{stubs}{os.pathsep}{os.environ['PATH']}",
        "EULA": "TRUE",
        "DELVE_OUTPUT": str(out),
    }
    for k in ("DW_RUN_OUT", "DW_BOT_OUT", "DW_COMPOSE_PROJECT", "DELVEWRIGHT_BRANCHES"):
        env.pop(k, None)

    r = subprocess.run(
        ["bash", str(engine / "validation" / "branch-runs.sh"), "--project", project],
        cwd=tmp_path,
        env=env,
        capture_output=True,
        text=True,
    )
    assert r.returncode == 0, r.stdout + r.stderr

    assert bot_report.is_file(), "branch-runs.sh deleted the bot's run-report.json"
    assert bot_report.read_text() == '{"the":"critical path"}\n'
    merged = json.loads(
        (engine / "validation" / "run-out" / project / "branch-runs.json").read_text()
    )
    assert [(b["branch"], b["ran"], b["passed"]) for b in merged["branches"]] == [
        ("world/a", True, True)
    ]
