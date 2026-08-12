#!/usr/bin/env python3
"""Sandbox pipeline: build program (.js) -> MineBench voxel.exec -> .schem -> .nbt -> PNGs.

Everything is deterministic: the seed is baked into the tool-call envelope and
printed with the result. No repo file is written; the engine's `delve-schem` and
`delve-render` binaries are used read-only (built into a sandbox CARGO_TARGET_DIR).
"""
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
MB = os.path.join(ROOT, "minebench")
BIN = os.path.join(ROOT, "cargo-target", "release")
SHOT = os.path.join(ROOT, "cargo-target-shot", "release", "dw-shot")
JAR = os.path.expanduser("~/.chunky/resources/minecraft.jar")

SEED = 121111          # single recorded seed for every build in the experiment
GRID = 256
PALETTE = "advanced"

# (yaw, pitch, zoom, name) — 3/4 perspective, front elevation, opposite 3/4.
EXTRA_SHOTS = [(0.0, 0.0, 1.0, "front"), (90.0, 8.0, 1.0, "side")]


def run(name):
    js = os.path.join(ROOT, "programs", f"{name}.js")
    code = open(js).read()
    call = {
        "tool": "voxel.exec",
        "input": {"code": code, "gridSize": GRID, "palette": PALETTE, "seed": SEED},
    }
    call_path = os.path.join(ROOT, "builds", f"{name}.call.json")
    os.makedirs(os.path.join(ROOT, "builds"), exist_ok=True)
    with open(call_path, "w") as f:
        json.dump(call, f)

    env = dict(os.environ, MINEBENCH_TOOL_TIMEOUT_MS="60000")
    r = subprocess.run(
        ["npx", "tsx", "scripts/dw-run-build.ts", call_path,
         os.path.join(ROOT, "builds", name)],
        cwd=MB, capture_output=True, text=True, env=env,
    )
    if r.returncode != 0:
        print(r.stdout, r.stderr)
        sys.exit(f"harness failed for {name}")
    stats = json.loads(r.stdout)

    schem = os.path.join(ROOT, "builds", f"{name}.schem")
    nbt = os.path.join(ROOT, "builds", f"{name}.nbt")
    # --split 512: keep the whole monument in ONE structure file. The default is
    # 48 (the vanilla structure-block cap), which splits every build here into
    # dozens of parts — recorded as toolchain friction, not worked around in the
    # engine.
    r2 = subprocess.run([os.path.join(BIN, "delve-schem"), "convert", schem,
                         "--out", nbt, "--split", "512"],
                        capture_output=True, text=True)
    if r2.returncode != 0:
        print(r2.stderr)
        sys.exit(f"delve-schem failed for {name}")
    stats["schem_diagnostics"] = [l for l in r2.stderr.splitlines() if l.strip()]

    outdir = os.path.join(ROOT, "renders", name)
    os.makedirs(outdir, exist_ok=True)
    r3 = subprocess.run([os.path.join(BIN, "delve-render"), "piece", nbt,
                         "-o", outdir, "--size", "768", "--textures", JAR],
                        capture_output=True, text=True)
    if r3.returncode != 0:
        print(r3.stderr)
        sys.exit(f"delve-render failed for {name}")
    for yaw, pitch, zoom, shot in EXTRA_SHOTS:
        subprocess.run([SHOT, nbt, os.path.join(outdir, f"{name}-{shot}.png"),
                        str(yaw), str(pitch), str(zoom), "768", JAR],
                       capture_output=True, text=True, check=True)
    stats["renders"] = sorted(os.listdir(outdir))
    print(json.dumps(stats, indent=1))
    with open(os.path.join(ROOT, "builds", f"{name}.stats.json"), "w") as f:
        json.dump(stats, f, indent=1)


if __name__ == "__main__":
    for n in sys.argv[1:]:
        print(f"### {n}")
        run(n)
