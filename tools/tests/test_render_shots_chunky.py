"""Guards for `validation/render-shots.sh` — the renderer is named, or the run says so.

Every emitted scene is written for ONE Chunky core, `versions.toml [render]
chunky_core`: the camera basis, the water-surface offset and the night-vision
emulation were all read off that core's bytecode. The install line every page
prints — `java -jar ChunkyLauncher.jar --update snapshot` — installs whatever
the snapshot CHANNEL serves today, which on the third end-to-end drill was
`…478.g527cb4a` against a pin of `…474.g156e2bb`. Nothing compared them, so the
frames that judged a delve came off an unpinned renderer and no artifact said so.

The pinned core cannot be asked for by name — established against the launcher,
not recalled: `--update` takes a release channel, every channel resolves to
`snapshot.json`/`latest.json` which name only the newest build, and
`<updateSite>/lib/<name>.jar` serves today's jar whatever name it is handed
(asking for the pin returns `content-disposition: …478.g527cb4a.jar`). A refusal
would therefore be a wall with no remedy behind it, so the script REPORTS, and
what these tests hold is that it reports, in words, with both revisions in them.

The perturbation is the drill's own shape: a Chunky home holding a core that is
not the pin. Nothing else in the ladder can catch it — a scene set over the wrong
core is byte-identical to one over the right one, and only the pictures differ.

And WHERE it looks is under test too. The check used to read the shell's `$HOME`
while Chunky reads the JVM's `user.home`, which on macOS is the OS account's and
not the environment's — so on the drill it printed `NONE installed` with the pin
in the real directory the whole time, and the `MISMATCH` verdict it also names
could never be reached on any machine where the two differ. The proof is a core
planted in each directory in turn (`test_the_check_reads_the_directory_java_reads`).
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent

ENGINE_PIN = "9.9.9"
CORE_PIN = "chunky-core-2.5.0-SNAPSHOT.474.g156e2bb"
CORE_OTHER = "chunky-core-2.5.0-SNAPSHOT.478.g527cb4a"

# A `delvec` that answers `--version` with the pinned engine and writes the three
# artifacts `render-shots.sh` asks it for. The subject is the SCRIPT.
DELVEC_STUB = f"""#!/usr/bin/env bash
case "$1" in
  --version) echo "delvec {ENGINE_PIN}, dsl 0.0.0, mc 1.21.11"; exit 0;;
  scene|panorama)
    out=""; while [ $# -gt 0 ]; do case "$1" in -o) out="$2"; shift 2;; *) shift;; esac; done
    mkdir -p "$out"; echo '{{}}' > "$out/$1-stub.json"; exit 0;;
  index)
    out=""; while [ $# -gt 0 ]; do case "$1" in -o) out="$2"; shift 2;; *) shift;; esac; done
    mkdir -p "$(dirname "$out")"; echo '[]' > "$out"; exit 0;;
esac
exit 3
"""


def tree(tmp_path: Path) -> Path:
    root = tmp_path / "repo"
    (root / "tools" / "lib").mkdir(parents=True)
    (root / "validation").mkdir(parents=True)
    for name in ("delvec-bin.sh", "versions.py", "chunky-home.sh"):
        (root / "tools" / "lib" / name).write_bytes(
            (REPO / "tools" / "lib" / name).read_bytes()
        )
    (root / "validation" / "render-shots.sh").write_bytes(
        (REPO / "validation" / "render-shots.sh").read_bytes()
    )
    (root / "versions.toml").write_text(
        f'[engine]\nversion = "{ENGINE_PIN}"\n\n[render]\nchunky_core = "{CORE_PIN}"\n'
    )
    return root


def build_dir(tmp_path: Path) -> Path:
    """A `delvec build` output that satisfies the world gate — that gate is not
    what is under test here, and it refuses before anything else runs."""
    b = tmp_path / "build"
    (b / "world" / "region").mkdir(parents=True)
    (b / "render-plan.json").write_text(json.dumps({"shots": []}))
    (b / "world" / "level.dat").write_bytes(b"\x00")
    (b / "world" / "region" / "r.0.0.mca").write_bytes(b"\x00")
    return b


def chunky_home(tmp_path: Path, cores: list[str]) -> Path:
    home = tmp_path / "chunky"
    (home / "lib").mkdir(parents=True)
    for core in cores:
        (home / "lib" / f"{core}.jar").write_bytes(b"\x00")
    return home


def run(
    root: Path,
    build: Path,
    home: Path | None,
    *,
    shell_home: Path | None = None,
    chunky_home_var: bool = True,
) -> subprocess.CompletedProcess:
    bindir = root.parent / "bin"
    bindir.mkdir(exist_ok=True)
    delvec = bindir / "delvec"
    delvec.write_text(DELVEC_STUB)
    delvec.chmod(0o755)
    shim = root.parent / "shim"
    shim.mkdir(exist_ok=True)
    py = shim / "python3"
    if not py.exists():
        py.symlink_to(sys.executable)

    env = dict(os.environ)
    env["PATH"] = os.pathsep.join(
        [str(bindir), str(shim), "/usr/bin", "/bin", "/usr/sbin", "/sbin"]
    )
    if not chunky_home_var:
        # The unset case: what the script resolves on its own is the subject.
        env.pop("DELVEWRIGHT_CHUNKY_HOME", None)
    elif home is not None:
        env["DELVEWRIGHT_CHUNKY_HOME"] = str(home)
    else:
        # No Chunky at all: point the variable at a directory that does not exist,
        # rather than at the developer's real `~/.chunky`, which would decide the
        # answer instead of the fixture.
        env["DELVEWRIGHT_CHUNKY_HOME"] = str(root.parent / "no-chunky-here")
    if shell_home is not None:
        env["HOME"] = str(shell_home)
    return subprocess.run(
        ["bash", str(root / "validation" / "render-shots.sh"), str(build)],
        env=env,
        capture_output=True,
        text=True,
    )


def test_the_pinned_core_alone_is_reported_as_the_one_to_render_with(
    tmp_path: Path,
) -> None:
    root = tree(tmp_path)
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_PIN]))

    assert r.returncode == 0, r.stderr
    assert f"chunky core: pinned {CORE_PIN} is installed" in r.stdout
    assert "render with it" in r.stdout
    assert "MISMATCH" not in r.stdout + r.stderr


def test_the_pin_beside_another_core_does_not_claim_it_is_the_renderer(
    tmp_path: Path,
) -> None:
    """The drill machine's real state, which the three-verdict check could not
    describe: the pin IS installed and the launcher still selects the newer core
    beside it. `render with it` is a claim this check cannot keep there, so that
    lib gets its own verdict naming the other core."""
    root = tree(tmp_path)
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_PIN, CORE_OTHER]))

    assert r.returncode == 0, r.stderr
    said = r.stdout + r.stderr
    assert "NOT the only core there" in said
    assert CORE_PIN in said and CORE_OTHER in said
    assert "render with it" not in said
    assert "MISMATCH" not in said, "the pin is present; this is not a mismatch"


def resolve(env_extra: dict[str, str]) -> tuple[str, str]:
    """Run `tools/lib/chunky-home.sh`'s resolver and read back both answers."""
    env = dict(os.environ)
    env.pop("DELVEWRIGHT_CHUNKY_HOME", None)
    env.update(env_extra)
    script = (
        f'. "{REPO}/tools/lib/chunky-home.sh"; dw_resolve_chunky_home; '
        'printf "%s\\n%s\\n" "$DW_CHUNKY_HOME" "$DW_CHUNKY_HOME_SOURCE"'
    )
    r = subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
    home, source = r.stdout.splitlines()[:2]
    return home, source


def java_user_home() -> Path | None:
    """The JVM's `user.home` — the property Chunky reads — or None with no java."""
    try:
        out = subprocess.run(
            ["java", "-XshowSettings:properties", "-version"],
            capture_output=True,
            text=True,
        )
    except FileNotFoundError:
        return None
    for line in (out.stderr + out.stdout).splitlines():
        if line.strip().startswith("user.home = "):
            return Path(line.split(" = ", 1)[1].strip())
    return None


def test_a_moved_shell_home_does_not_move_where_the_check_looks(
    tmp_path: Path,
) -> None:
    """**The path defect, perturbed directly.** Chunky resolves its directory
    from the JVM's `user.home`; the check used to read the shell's `$HOME`. On
    macOS those disagree the moment `$HOME` is moved, which is how the drill got
    `NONE installed` with the pinned core present the whole time.

    Planted in each directory in turn: a `.chunky` under a moved `$HOME` must NOT
    be what the resolver names, and `user.home`'s must be.
    """
    real = java_user_home()
    if real is None:
        import pytest

        pytest.skip("no java on PATH: `user.home` cannot be read")
    moved = tmp_path / "moved-home"
    (moved / ".chunky" / "lib").mkdir(parents=True)

    home, source = resolve({"HOME": str(moved)})
    assert home == str(real / ".chunky"), f"resolved {home}, java reads {real}"
    assert home != str(moved / ".chunky"), "the resolver followed the shell's HOME"
    assert "user.home" in source

    # And the override still wins, because that is Chunky's own first rule.
    override = tmp_path / "elsewhere"
    home2, source2 = resolve(
        {"HOME": str(moved), "DELVEWRIGHT_CHUNKY_HOME": str(override)}
    )
    assert home2 == str(override)
    assert source2 == "DELVEWRIGHT_CHUNKY_HOME"


def test_an_unresolvable_home_says_so_instead_of_guessing_quietly(
    tmp_path: Path,
) -> None:
    """With no `java` there is no `user.home` to read, so the answer falls back
    to `$HOME` — the value that was wrong. An absence reported from a directory
    nobody confirmed the renderer reads is a statement about this script, so the
    fallback is carried in the source string rather than swallowed."""
    # A PATH that holds a shell (bash has to be findable to run the script at
    # all) and no `java`.
    binonly = tmp_path / "no-java-here"
    binonly.mkdir()
    for tool in ("bash", "awk"):
        found = shutil.which(tool)
        assert found, tool
        (binonly / tool).symlink_to(found)
    assert shutil.which("java", path=str(binonly)) is None
    moved = tmp_path / "moved-home"
    moved.mkdir()
    home, source = resolve({"HOME": str(moved), "PATH": str(binonly)})
    assert home == str(moved / ".chunky")
    assert "UNVERIFIED" in source


def test_a_core_that_is_not_the_pin_is_reported_with_both_revisions(
    tmp_path: Path,
) -> None:
    """The drill's own machine: the pin says 474, `--update snapshot` gave 478."""
    root = tree(tmp_path)
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_OTHER]))

    assert r.returncode == 0, r.stderr
    said = r.stdout + r.stderr
    assert "MISMATCH" in said
    # Both revisions, named. A report that says only "mismatch" is a report the
    # reader cannot act on or contradict.
    assert CORE_PIN in said
    assert CORE_OTHER in said
    assert "not refused" in said


def test_no_core_installed_says_the_update_line_does_not_install_the_pin(
    tmp_path: Path,
) -> None:
    root = tree(tmp_path)
    r = run(root, build_dir(tmp_path), None)

    assert r.returncode == 0, r.stderr
    said = r.stdout + r.stderr
    assert "chunky core: NONE installed" in said
    assert CORE_PIN in said
    assert "--update snapshot" in said


def test_the_pin_is_read_and_never_restated(tmp_path: Path) -> None:
    """A pin has one home. The script names no revision of its own — proved by
    moving the registry's value and reading what the script then says."""
    root = tree(tmp_path)
    moved = "chunky-core-2.5.0-SNAPSHOT.999.gdeadbee"
    (root / "versions.toml").write_text(
        f'[engine]\nversion = "{ENGINE_PIN}"\n\n[render]\nchunky_core = "{moved}"\n'
    )
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_PIN]))

    assert r.returncode == 0, r.stderr
    said = r.stdout + r.stderr
    assert moved in said, "the script did not read the pin it was given"
    assert "MISMATCH" in said
    # And no revision literal of its own survives in the source.
    assert CORE_PIN not in (REPO / "validation" / "render-shots.sh").read_text()


def test_an_unreadable_pin_refuses_rather_than_reporting_nothing(tmp_path: Path) -> None:
    root = tree(tmp_path)
    (root / "versions.toml").write_text(f'[engine]\nversion = "{ENGINE_PIN}"\n')
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_PIN]))

    assert r.returncode == 1
    assert "[render].chunky_core" in r.stderr
