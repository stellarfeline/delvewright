"""Guards for `tools/determinism-subject.sh` — the cross-OS gate's subject.

Both properties asserted here were learned from one red, and neither was
guessable from the script's own text.

**It must be self-contained.** The script claimed the metrics gym needs no
prefab library. That was true of what the gym PLACES and false of what the
compiler READS: `delvec build` opens `--prefabs` (default `campaigns/prefabs`)
whatever the campaign turns out to place, and refuses at exit 10 when it is not
there. The claim went unnoticed because both places it had ever run carried that
directory — a dev worktree through the `campaigns/` symlink, and the `rust` job
through `./.github/actions/checkout-content`. The macOS runner has neither, so it
was the first host on which the claim was actually tested, and it failed there.

**A failure must name itself.** The two verbs' output went to log files and
`set -e` ended the run, so the runner printed `Process completed with exit code
10` and nothing else — while the engine's one-line diagnostic, which named the
missing path exactly, sat in a file nobody read.

The subject here is the SCRIPT, so the engine is a stub: these are assertions
about what the script asks for and what it says when the answer is no, and a
real `delvec` would make them slower without making them stronger.
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO / "tools" / "determinism-subject.sh"

# A stub engine that refuses exactly the way the real one did: `build` without
# `--prefabs` is `internal error: cannot read prefabs dir campaigns/prefabs`,
# exit 10.
STUB = r"""#!/usr/bin/env bash
case "$1" in
  --version) echo "delvec 0.0.0-stub, dsl 0.0.0, mc 1.21.11"; exit 0;;
  metrics)   mkdir -p "$3"; echo '{"a":1}' > "$3/world.json"; exit 0;;
  build)
    out=""; prefabs=""
    while [ $# -gt 0 ]; do
      case "$1" in
        -o) out="$2"; shift 2;;
        --prefabs) prefabs="$2"; shift 2;;
        *) shift;;
      esac
    done
    if [ -z "$prefabs" ] || [ ! -d "$prefabs" ]; then
      echo "internal error: cannot read prefabs dir campaigns/prefabs: No such file or directory (os error 2)"
      exit 10
    fi
    mkdir -p "$out/datapack"
    echo '{"campaign_id":"metrics-gym"}' > "$out/manifest.json"
    exit 0;;
esac
exit 3
"""


def bare_tree(tmp_path: Path, stub: str = STUB) -> tuple[Path, Path]:
    """A checkout shaped like a fresh runner's: NO `campaigns/` anywhere."""
    root = tmp_path / "repo"
    (root / "tools").mkdir(parents=True)
    (root / "bin").mkdir()
    for name in ("determinism-subject.sh", "tree-digest.py"):
        (root / "tools" / name).write_bytes((REPO / "tools" / name).read_bytes())
        (root / "tools" / name).chmod(0o755)
    engine = root / "bin" / "delvec"
    engine.write_text(stub)
    engine.chmod(0o755)
    assert not (root / "campaigns").exists()
    return root, engine


def run(root: Path, engine: Path, out: Path) -> subprocess.CompletedProcess:
    # An empty HOME too: a fresh runner has no `~/.chunky`, no caches, nothing
    # a developer's machine quietly supplies.
    home = root.parent / "home"
    home.mkdir(exist_ok=True)
    return subprocess.run(
        [
            "bash",
            str(root / "tools" / "determinism-subject.sh"),
            "--delvec",
            str(engine),
            "--out",
            str(out),
        ],
        cwd=str(root),
        capture_output=True,
        text=True,
        env={"PATH": os.environ["PATH"], "HOME": str(home)},
    )


def test_the_subject_builds_on_a_tree_with_no_prefab_library(tmp_path):
    root, engine = bare_tree(tmp_path)
    out = tmp_path / "digest.txt"
    r = run(root, engine, out)
    assert r.returncode == 0, r.stdout + r.stderr
    assert out.is_file()
    assert "tree-digest v1" in out.read_text()


def test_the_build_is_handed_a_prefab_directory_that_exists(tmp_path):
    """The independence is STRUCTURAL — an empty library the script makes — not
    incidental on a directory the host happened to have."""
    root, engine = bare_tree(tmp_path)
    r = run(root, engine, tmp_path / "digest.txt")
    assert "--prefabs" in r.stdout
    prefabs = r.stdout.split("--prefabs ")[1].split()[0]
    assert Path(prefabs).is_dir()
    # And it is NOT inside the digested subject: an input is not the artifact.
    assert "/subject/" not in prefabs


def test_every_verb_says_what_it_runs_before_it_runs(tmp_path):
    root, engine = bare_tree(tmp_path)
    r = run(root, engine, tmp_path / "digest.txt")
    for expected in ("engine   —", "cwd      —", "work dir —", "metrics —", "build —"):
        assert expected in r.stdout, expected


def test_a_failing_verb_prints_what_the_engine_said(tmp_path):
    """`Process completed with exit code 10` and nothing else is what this
    replaces. The engine's own line named the missing path exactly."""
    root, engine = bare_tree(tmp_path)
    # Take the prefab directory away between the script making it and the build
    # reading it — the stub refuses on a `--prefabs` that is not a directory,
    # which is the real engine's failure reproduced.
    engine.write_text(STUB.replace('if [ -z "$prefabs" ]', 'if true || [ -z "$prefabs" ]'))
    engine.chmod(0o755)
    r = run(root, engine, tmp_path / "digest.txt")
    assert r.returncode == 10
    assert "`build` exited 10" in r.stderr
    assert "cannot read prefabs dir" in r.stderr


def test_the_digest_is_not_written_when_a_verb_failed(tmp_path):
    """A digest of a half-built subject would compare against the other host and
    disagree for a reason that has nothing to do with determinism."""
    root, engine = bare_tree(tmp_path)
    engine.write_text(STUB.replace('if [ -z "$prefabs" ]', 'if true || [ -z "$prefabs" ]'))
    engine.chmod(0o755)
    out = tmp_path / "digest.txt"
    assert run(root, engine, out).returncode == 10
    assert not out.exists()
