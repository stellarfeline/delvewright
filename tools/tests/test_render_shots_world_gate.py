r"""`validation/render-shots.sh` refuses a build tree that has no world save.

WHY THIS GATE EXISTS. A Chunky scene names the world Chunky loads, and
`delvec build` writes no world: a delve's geometry is stamped by the datapack's
`place_all` over the first ticks of a server boot, so a fresh build tree has no
`level.dat` and no `region/` anywhere in it. Chunky's answer to a missing world
is an EMPTY SKY at exit 0, with `Could not load chunks (no world found for
scene)` buried inside a Java stack trace — measured: 279 scenes, ten launcher
invocations, ten exit-0 PNGs, every one of them the same empty frame. That is
the visual review's primary evidence, and nothing anywhere fails.

WHAT IS ASSERTED, and why it needs no Docker. The gate decides on the presence of
two files, so the whole of it is decidable on a directory: the trees here are
built by hand and `delvec` is a shim on PATH, so the script runs end to end
without a compiler and without a server. Booting a server to prove a refusal
about a missing world would be measuring something else.

The half-world cases are the point of the parametrisation: a `level.dat` with no
region files renders exactly the empty frame the gate exists to prevent, so
"the directory exists" and "level.dat exists" are each only a proxy, and the gate
is keyed on the pair.

The last test is the anti-vacuity half in both directions — that the world path
handed to `delvec scene` is ABSOLUTE (Chunky resolves it against the rendering
process's working directory, so a relative one is silently wrong from anywhere
but one place), and that removing the gate stops the refusal, which is what
separates "this gate refused" from "something else happened to refuse".
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "validation" / "render-shots.sh"

# A render plan the script's own pre-check reads before the world gate. Its
# contents never reach a real `delvec`, which is a shim in every test here.
RENDER_PLAN = {"campaign_id": "gate-fixture", "layout_aabb": {}, "shots": []}

SHIM = """#!/usr/bin/env bash
# Stand-in for `delvec`: record the argv of every arm and succeed.
printf '%s\\n' "$*" >> "$DW_SHIM_LOG"
exit 0
"""


def build_tree(tmp_path: Path, *, level_dat: bool, regions: int) -> Path:
    build = tmp_path / "build"
    build.mkdir()
    (build / "render-plan.json").write_text(json.dumps(RENDER_PLAN))
    world = build / "world"
    if level_dat:
        world.mkdir(parents=True, exist_ok=True)
        (world / "level.dat").write_bytes(b"\x1f\x8b")  # gzip magic; never parsed here
    for i in range(regions):
        (world / "region").mkdir(parents=True, exist_ok=True)
        (world / "region" / f"r.0.{i}.mca").write_bytes(b"\0" * 8)
    return build


def shim_env(tmp_path: Path) -> tuple[dict, Path]:
    """`delvec` on PATH, so the script's own arms run without a compiler."""
    binv = tmp_path / "bin"
    binv.mkdir(exist_ok=True)
    shim = binv / "delvec"
    shim.write_text(SHIM)
    shim.chmod(0o755)
    log = tmp_path / "delvec-argv.log"
    env = dict(os.environ)
    env["PATH"] = f"{binv}{os.pathsep}{env.get('PATH', '')}"
    env["DW_SHIM_LOG"] = str(log)
    return env, log


def run(script: Path, build: Path, out: Path, env: dict) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["bash", str(script), str(build), str(out)],
        capture_output=True,
        text=True,
        env=env,
    )


@pytest.mark.parametrize(
    "level_dat,regions,missing",
    [
        (False, 0, "no world at all"),
        (True, 0, "level.dat but no region files"),
        (False, 1, "region files but no level.dat"),
    ],
)
def test_refuses_without_a_world(tmp_path, level_dat, regions, missing):
    build = build_tree(tmp_path, level_dat=level_dat, regions=regions)
    env, log = shim_env(tmp_path)
    out = tmp_path / "shots"
    r = run(SCRIPT, build, out, env)
    assert r.returncode == 2, f"{missing}: expected a refusal, got {r.returncode}\n{r.stderr}"
    assert "has no world save" in r.stderr, r.stderr
    # The remedy is NAMED, and it is the only producer of that directory: a gate
    # that names no remedy sends the reader back to the empty frames.
    assert "validation/world-save.sh" in r.stderr, r.stderr
    # Nothing is written, and nothing is emitted. A scene set over a world that
    # does not exist must not exist either, or somebody renders it.
    assert not out.exists(), f"{missing}: the refusal still wrote {out}"
    assert not log.exists(), f"{missing}: the refusal still ran an emission arm"


def test_a_world_passes_and_the_scenes_name_it_absolutely(tmp_path):
    build = build_tree(tmp_path, level_dat=True, regions=2)
    env, log = shim_env(tmp_path)
    out = tmp_path / "shots"
    r = run(SCRIPT, build, out, env)
    assert r.returncode == 0, r.stdout + r.stderr
    assert "has no world save" not in r.stderr, r.stderr
    assert "2 region file(s)" in r.stdout, r.stdout

    argv = log.read_text().splitlines()
    world_args = [line.split("--world ", 1)[1].strip() for line in argv if "--world " in line]
    assert len(world_args) == 2, f"scene + panorama should both name a world: {argv}"
    for w in world_args:
        assert w == str(build / "world"), (
            "the world path must be ABSOLUTE — Chunky resolves it against the "
            f"rendering process's working directory, not the scene dir: {w}"
        )


def test_the_gate_is_the_only_thing_that_refuses_a_missing_world(tmp_path):
    """Perturbation, toward the vacuous shape.

    Strip the world check out of a scratch copy of the script and the same tree
    is no longer refused — so the refusal above is this gate's doing and not some
    other pre-check happening to fire on the same input.
    """
    build = build_tree(tmp_path, level_dat=False, regions=0)
    env, _ = shim_env(tmp_path)
    lines = SCRIPT.read_text().splitlines(keepends=True)
    start = next(i for i, l in enumerate(lines) if l.startswith("# The world gate."))
    end = next(i for i, l in enumerate(lines) if l.startswith("# Every arm this script runs"))
    assert start < end, "the gate no longer sits between its comment and the `delvec` runner"
    perturbed = tmp_path / "render-shots-no-gate.sh"
    # The removed block also defines `$world_dir` and `$world_regions`, which the
    # emission and the binding line read — substitute them so the perturbed copy
    # fails for no reason other than the missing gate.
    body = "".join(lines[:start] + lines[end:])
    body = body.replace('"$world_dir"', '"$build_dir/world"').replace("$world_dir", "$build_dir/world")
    body = body.replace("$world_regions", "unchecked")
    assert "world_dir" not in body and "world_regions" not in body, body
    perturbed.write_text(body)
    shutil.copymode(SCRIPT, perturbed)
    r = run(perturbed, build, tmp_path / "shots", env)
    assert "has no world save" not in r.stderr, (
        "the refusal survived the gate's removal — it is coming from somewhere else, "
        "and this suite is measuring the wrong thing\n" + r.stderr
    )
    assert r.returncode == 0, (
        "with the gate removed a worldless tree must sail through — that is the "
        "defect this gate exists to stop\n" + r.stdout + r.stderr
    )
