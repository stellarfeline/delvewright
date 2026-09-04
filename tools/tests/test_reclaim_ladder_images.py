r"""Guards for `validation/reclaim-ladder-images.sh` and the rule it shares with
`validation/fresh-volumes.sh` (`validation/lib/ladder-images.sh`).

The defect these exist for is not a red — it is 11.99 GB of images on the
creator's workstation, left by 67 compose projects that had not run for weeks,
because the teardown proved containers, volumes and networks and stopped there.
The runnable-locally guarantee decays with use unless what the toolchain leaves
behind is bounded.

What actually needs guarding is the SAFETY half, because it fails in the
reassuring direction: a sweep that deletes too much succeeds, prints a large
number, and is found out only when the owner's play session or a sibling round's
ladder rebuilds — or does not. So `docker` is replaced by a PATH shim answering a
fixture, exactly as `test_cargo_fingerprint_inputs.py` shims `rustc`: the shim
records every `rmi` it is asked to perform, and the assertions are about what is
in that log and what is NOT.

The three ways the sweep could be wrong, each with a case below:

  * `delvewright/delve:local` is the DEFAULT `DELVE_IMAGE`, the tag
    `owner-play.yaml` publishes on 25565 and the one the `playtest` profile
    builds — and it carries the compose project label of whichever project built
    it last (measured on the real daemon: `dw-round-n`). A label-only rule
    deletes the owner's play image while reclaiming a finished worker.
  * `dw-m5-final-bot:latest` starts with `dw-m5-`. A prefix match on the project
    name hands one project's image to another; only the exact
    `<project>-<service>:latest`, with the service read off the image's own
    label, tells them apart. The fixture mislabels that image on purpose, which
    is a perturbation nothing else in the chain can catch.
  * a project mid-run holds a container, a volume or a network. Age, quiet and
    "nobody is probably using it" are beliefs about who is running; those three
    are evidence.

Docker is never required: the shim is the whole daemon these tests see.
"""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
SWEEP = REPO / "validation" / "reclaim-ladder-images.sh"
LIB = REPO / "validation" / "lib" / "ladder-images.sh"
COMPOSE = REPO / "validation" / "compose.yaml"

SHIM = r"""#!/usr/bin/env python3
import json, os, sys

state = json.load(open(os.environ["DW_FIXTURE"]))
argv = sys.argv[1:]


def images_of(project):
    return [i for i in state["images"] if i.get("project") == project]


def emit(lines):
    for line in lines:
        print(line)


if argv[:1] == ["version"]:
    print("29.0.0")
elif argv[:2] == ["system", "df"]:
    print("Images %d 0B 0B" % len(state["images"]))
elif argv[:1] == ["images"]:
    label = next((a for a in argv if a.startswith("label=")), "")
    want = label.split("=", 2)[2] if label.count("=") >= 2 else None
    for i in state["images"]:
        if not i.get("project"):
            continue
        if want is not None and i["project"] != want:
            continue
        print(i["id"])
elif argv[:2] == ["image", "inspect"]:
    fmt = argv[argv.index("--format") + 1]
    ids = [a for a in argv[2:] if a.startswith("sha256:")]
    by_id = {i["id"]: i for i in state["images"]}
    for the_id in ids:
        i = by_id[the_id]
        if "com.docker.compose.project" in fmt and "{{.Id}}" not in fmt:
            print(i["project"])
        else:
            print("\t".join([i["id"], str(i["size"]), i.get("service", ""), ",".join(i.get("tags", []))]))
elif argv[:1] == ["ps"]:
    label = next((a for a in argv if a.startswith("label=")), "")
    want = label.split("=", 2)[2] if label.count("=") >= 2 else None
    for c in state.get("containers", []):
        if want is not None and c.get("project") != want:
            continue
        print(c["id"])
elif argv[:2] == ["container", "inspect"]:
    fmt = argv[argv.index("--format") + 1]
    ids = [a for a in argv[2:] if not a.startswith("-") and a != fmt]
    by_id = {c["id"]: c for c in state.get("containers", [])}
    for the_id in ids:
        c = by_id[the_id]
        print(c["image"] if "{{.Image}}" in fmt else "/%s (running)" % c["id"])
elif argv[:2] == ["volume", "ls"]:
    emit(state.get("volumes", []) if "--filter" not in argv else [])
elif argv[:2] == ["network", "ls"]:
    emit(state.get("networks", []) if "--filter" not in argv else [])
elif argv[:1] == ["rmi"]:
    with open(os.environ["DW_RMI_LOG"], "a") as fh:
        fh.write(argv[1] + "\n")
    if argv[1] in state.get("refuse", []):
        sys.stderr.write("Error response from daemon: conflict: %s\n" % argv[1])
        sys.exit(1)
    print("Untagged: %s" % argv[1])
else:
    sys.stderr.write("shim: unhandled docker %s\n" % " ".join(argv))
    sys.exit(2)
"""

FIXTURE = {
    "images": [
        # A finished project: one compose-named bot image and the untagged
        # predecessor its last rebuild left behind. Both are this project's.
        {"id": "sha256:aaa", "project": "dw-alpha", "service": "bot", "size": 1_000_000_000,
         "tags": ["dw-alpha-bot:latest"]},
        {"id": "sha256:bbb", "project": "dw-alpha", "service": "server", "size": 800_000_000,
         "tags": []},
        # The shared default tag, wearing a finished project's label.
        {"id": "sha256:ccc", "project": "dw-beta", "service": "server", "size": 800_000_000,
         "tags": ["delvewright/delve:local"]},
        # This project's own name, and finished — but a container of ANOTHER
        # project is holding the image, which is the rung's whole point.
        {"id": "sha256:ddd", "project": "dw-gamma", "service": "bot", "size": 900_000_000,
         "tags": ["dw-gamma-bot:latest"]},
        # Mid-run: a container of its own.
        {"id": "sha256:eee", "project": "dw-delta", "service": "bot", "size": 900_000_000,
         "tags": ["dw-delta-bot:latest"]},
        # The prefix trap: `dw-m5-final-bot:latest` is not `dw-m5`'s name for
        # anything, however much it looks like it.
        {"id": "sha256:fff", "project": "dw-m5", "service": "bot", "size": 900_000_000,
         "tags": ["dw-m5-final-bot:latest"]},
        # Compose's default project — where the owner's play session lands.
        {"id": "sha256:ggg", "project": "validation", "service": "bot", "size": 900_000_000,
         "tags": ["validation-bot:latest"]},
        # Pulled, so no compose label: must never be selected at all.
        {"id": "sha256:hhh", "project": "", "service": "", "size": 900_000_000,
         "tags": ["itzg/minecraft-server:java21"]},
    ],
    "containers": [
        {"id": "c1", "project": "dw-other", "image": "sha256:ddd"},
        {"id": "c2", "project": "dw-delta", "image": "sha256:eee"},
    ],
}


@pytest.fixture()
def sweep(tmp_path):
    shim_dir = tmp_path / "bin"
    shim_dir.mkdir()
    docker = shim_dir / "docker"
    docker.write_text(SHIM)
    docker.chmod(0o755)
    fixture = tmp_path / "fixture.json"
    fixture.write_text(json.dumps(FIXTURE))
    log = tmp_path / "rmi.log"
    log.write_text("")

    def run(*args):
        # Truncated per invocation: a test that sweeps twice must read the second
        # sweep's removals, not the union — a shared log is a computed key over a
        # non-unique name.
        log.write_text("")
        env = dict(os.environ)
        env["PATH"] = f"{shim_dir}:{env['PATH']}"
        env["DW_FIXTURE"] = str(fixture)
        env["DW_RMI_LOG"] = str(log)
        proc = subprocess.run(
            ["bash", str(SWEEP), *args], capture_output=True, text=True, env=env
        )
        return proc, [line for line in log.read_text().splitlines() if line]

    return run


def test_dry_run_removes_nothing_and_says_what_it_would(sweep):
    proc, removed = sweep()
    assert proc.returncode == 0, proc.stderr
    assert removed == [], "a dry run asked the daemon to remove something"
    assert "DRY RUN" in proc.stdout
    assert "dw-alpha-bot:latest" in proc.stdout
    assert "would remove" in proc.stdout


def test_apply_removes_only_the_names_this_project_minted(sweep):
    proc, removed = sweep("--apply")
    assert proc.returncode == 0, proc.stderr
    assert sorted(removed) == ["dw-alpha-bot:latest", "sha256:bbb"], proc.stdout


def test_the_shared_default_tag_is_kept_and_named(sweep):
    proc, removed = sweep("--apply")
    assert "delvewright/delve:local" not in removed
    assert "delvewright/delve:local" in proc.stdout
    assert "not this project's name for it" in proc.stdout


def test_a_prefix_of_the_project_name_is_not_the_project_name(sweep):
    # `dw-m5-final-bot:latest` under project `dw-m5`. A prefix match would take it.
    proc, removed = sweep("--apply")
    assert "dw-m5-final-bot:latest" not in removed, "a prefix match claimed another project's image"
    assert "dw-m5-final-bot:latest" in proc.stdout


def test_a_project_holding_a_container_is_skipped_as_mid_run(sweep):
    proc, removed = sweep("--apply")
    assert "SKIP dw-delta" in proc.stdout
    assert "dw-delta-bot:latest" not in removed


def test_an_image_a_container_holds_is_exempt_by_id(sweep):
    proc, removed = sweep("--apply")
    assert "dw-gamma-bot:latest" not in removed
    assert "held by container" in proc.stdout


def test_the_default_project_is_swept_only_when_named(sweep):
    proc, removed = sweep("--apply")
    assert "validation-bot:latest" not in removed
    assert "validation-bot" not in proc.stdout

    proc, removed = sweep("--project", "validation", "--apply")
    assert removed == ["validation-bot:latest"], proc.stdout


def test_a_pulled_image_carrying_no_compose_label_is_never_selected(sweep):
    proc, removed = sweep("--apply")
    assert "itzg/minecraft-server:java21" not in removed
    assert "itzg" not in proc.stdout


def test_a_daemon_refusal_keeps_the_image_and_names_the_reason(tmp_path):
    shim_dir = tmp_path / "bin"
    shim_dir.mkdir()
    docker = shim_dir / "docker"
    docker.write_text(SHIM)
    docker.chmod(0o755)
    state = json.loads(json.dumps(FIXTURE))
    state["refuse"] = ["dw-alpha-bot:latest"]
    fixture = tmp_path / "fixture.json"
    fixture.write_text(json.dumps(state))
    log = tmp_path / "rmi.log"
    log.write_text("")
    env = dict(os.environ)
    env["PATH"] = f"{shim_dir}:{env['PATH']}"
    env["DW_FIXTURE"] = str(fixture)
    env["DW_RMI_LOG"] = str(log)
    proc = subprocess.run(
        ["bash", str(SWEEP), "--apply"], capture_output=True, text=True, env=env
    )
    assert proc.returncode == 0, proc.stderr
    assert "the daemon refused" in proc.stdout


def test_bad_arguments_refuse_before_the_daemon_is_touched():
    for args in (["--project", "../etc"], ["--nonsense"], ["--project"]):
        proc = subprocess.run(
            ["bash", str(SWEEP), *args], capture_output=True, text=True
        )
        assert proc.returncode == 2, (args, proc.stdout, proc.stderr)


def test_the_default_project_constant_is_the_compose_file_s_own_directory():
    """`validation` is not a name to remember — it is what compose derives."""
    declared = [
        line.split("=", 1)[1].strip().strip("'")
        for line in LIB.read_text().splitlines()
        if line.startswith("DW_IMG_DEFAULT_PROJECT=")
    ]
    assert declared == [COMPOSE.parent.name]


def test_the_shared_delve_tag_the_header_names_is_still_compose_s_default():
    """The header's safety argument rests on `delvewright/delve:local` being the
    default `DELVE_IMAGE`. If compose ever renames that default, the argument is
    about a tag nobody boots and this test is the only thing that would say so."""
    text = COMPOSE.read_text()
    assert "${DELVE_IMAGE:-delvewright/delve:local}" in text
    assert "delvewright/delve:local" in LIB.read_text()
