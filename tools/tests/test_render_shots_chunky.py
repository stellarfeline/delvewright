"""Guards for the pinned Chunky core: `validation/render-shots.sh` says whether it
is installed, `validation/chunky.sh` renders with it or refuses by name, and
`validation/chunky-install.sh` refuses a build JDK the pinned revision does not
build under.

Every emitted scene is written for ONE Chunky core, `versions.toml [render]
chunky_core`: the camera basis, the water-surface offset and the night-vision
emulation were all read off that core's bytecode. The launcher's `--update
snapshot` installs the snapshot CHANNEL's newest build — on the third end-to-end
drill `…478.g527cb4a` against a pin of `…474.g156e2bb` — and the launcher renders
with whichever core it chooses. So the pin is installed from source at the
pinned revision, and a Chunky home holds it when the core jar answers the pinned
CONTENT digest (`tools/lib/chunky_core.py`) and every library its version record
names carries the recorded md5.

The perturbation is the drill's own shape: a Chunky home holding a core that is
not the pin, and a jar under the pin's name whose content is not the pin's.
Nothing else in the ladder can catch either — a scene set over the wrong core is
byte-identical to one over the right one, and only the pictures differ.

And WHERE it looks is under test too. The check used to read the shell's `$HOME`
while Chunky reads the JVM's `user.home`, which on macOS is the OS account's and
not the environment's. The proof is a core planted in each directory in turn
(`test_a_moved_shell_home_does_not_move_where_the_check_looks`).
"""

from __future__ import annotations

import hashlib
import json
import os
import zipfile
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


def core_jar(path: Path, marker: bytes, comment: bytes = b"#built\n") -> Path:
    """A jar shaped like a Chunky core: a manifest naming its main class, a class
    file whose bytes are the content, and a `Version.properties` whose comment
    line is the build's timestamp."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w") as z:
        z.writestr("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\nMain-Class: se.llbit.chunky.main.Chunky\n")
        z.writestr("se/llbit/chunky/main/Chunky.class", marker)
        z.writestr("se/llbit/chunky/main/Version.properties", comment + b"version=pinned\n")
    return path


def digest_of(jar: Path) -> str:
    r = subprocess.run(
        [sys.executable, str(REPO / "tools" / "lib" / "chunky_core.py"), "digest", str(jar)],
        capture_output=True,
        text=True,
    )
    assert r.returncode == 0, r.stderr
    return r.stdout.strip()


PIN_BYTES = b"the pinned core"
OTHER_BYTES = b"a newer snapshot"


def pins_toml(core: str, digest: str) -> str:
    return (
        f'[engine]\nversion = "{ENGINE_PIN}"\n\n[render]\nchunky_core = "{core}"\n'
        f'chunky_core_content_sha256 = "{digest}"\nchunky_source = "file:///nowhere"\n'
        f'chunky_revision = "the-pinned-revision"\nchunky_build_java = "17"\n'
    )


def tree(tmp_path: Path) -> Path:
    root = tmp_path / "repo"
    (root / "tools" / "lib").mkdir(parents=True)
    (root / "validation").mkdir(parents=True)
    for name in ("delvec-bin.sh", "versions.py", "chunky-home.sh", "chunky_core.py"):
        (root / "tools" / "lib" / name).write_bytes(
            (REPO / "tools" / "lib" / name).read_bytes()
        )
    for name in ("render-shots.sh", "chunky.sh", "chunky-install.sh"):
        (root / "validation" / name).write_bytes((REPO / "validation" / name).read_bytes())
    pin_digest = digest_of(core_jar(tmp_path / "reference" / "pin.jar", PIN_BYTES))
    (root / "versions.toml").write_text(pins_toml(CORE_PIN, pin_digest))
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


def md5(path: Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest().upper()


def chunky_home(
    tmp_path: Path, cores: list[str], *, forged: bool = False, record: bool = True
) -> Path:
    """A Chunky home in Chunky's own layout: each core in `lib/` beside a library,
    and a version record per core naming both with their md5. The pin's jar
    carries the pin's content; `forged` puts another core's content under the
    pin's name."""
    home = tmp_path / "chunky"
    (home / "lib").mkdir(parents=True)
    (home / "versions").mkdir(parents=True)
    dep = home / "lib" / "gson-2.9.0.jar"
    dep.write_bytes(b"a library")
    for core in cores:
        content = PIN_BYTES if core == CORE_PIN and not forged else OTHER_BYTES
        jar = core_jar(home / "lib" / f"{core}.jar", content, comment=b"#another day\n")
        if record:
            (home / "versions" / f"{core}.json").write_text(
                json.dumps(
                    {
                        "name": core,
                        "libraries": [
                            {"name": jar.name, "md5": md5(jar)},
                            {"name": dep.name, "md5": md5(dep)},
                        ],
                    }
                )
            )
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


def test_the_pinned_core_is_reported_as_installed(tmp_path: Path) -> None:
    root = tree(tmp_path)
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_PIN]))

    assert r.returncode == 0, r.stderr
    assert f"chunky core: pinned {CORE_PIN} is installed" in r.stdout
    assert "content digest verified" in r.stdout
    assert "chunky-install.sh" not in r.stdout + r.stderr


def test_the_pin_beside_another_core_is_still_the_pin(tmp_path: Path) -> None:
    """The drill machine's real state: the pin and a newer core side by side.
    `chunky.sh` names its classpath, so the other core does not decide the
    renderer and the home holds the pin."""
    root = tree(tmp_path)
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_PIN, CORE_OTHER]))

    assert r.returncode == 0, r.stderr
    assert "content digest verified" in r.stdout


def test_a_core_that_is_not_the_pin_names_the_installer(tmp_path: Path) -> None:
    """The drill's own machine: the pin says 474, `--update snapshot` gave 478.
    Reported with both revisions and the command that installs the pin; the
    shot set itself is still written, because this step renders nothing."""
    root = tree(tmp_path)
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_OTHER]))

    assert r.returncode == 0, r.stderr
    said = r.stdout + r.stderr
    assert f"the pinned core {CORE_PIN} is not installed" in said
    assert CORE_OTHER in said
    assert "chunky-install.sh" in said
    assert "refuses to render" in said


def test_a_jar_under_the_pins_name_is_held_to_its_content(tmp_path: Path) -> None:
    """A name is not an identity: the update site serves today's jar under any
    name. The forged jar carries another core's classes under the pin's name."""
    root = tree(tmp_path)
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_PIN], forged=True))

    assert r.returncode == 0, r.stderr
    said = r.stdout + r.stderr
    assert "is named as the pin and is not it" in said
    assert "content digest verified" not in said


def test_no_core_installed_names_the_installer(tmp_path: Path) -> None:
    root = tree(tmp_path)
    r = run(root, build_dir(tmp_path), None)

    assert r.returncode == 0, r.stderr
    said = r.stdout + r.stderr
    assert f"the pinned core {CORE_PIN} is not installed" in said
    assert "chunky-install.sh" in said


def test_the_pin_is_read_and_never_restated(tmp_path: Path) -> None:
    """A pin has one home. The scripts name no revision of their own — proved by
    moving the registry's value and reading what the report then says."""
    root = tree(tmp_path)
    moved = "chunky-core-2.5.0-SNAPSHOT.999.gdeadbee"
    digest = (root / "versions.toml").read_text().split('chunky_core_content_sha256 = "')[1].split('"')[0]
    (root / "versions.toml").write_text(pins_toml(moved, digest))
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_PIN]))

    assert r.returncode == 0, r.stderr
    said = r.stdout + r.stderr
    assert f"the pinned core {moved} is not installed" in said
    for script in ("render-shots.sh", "chunky.sh", "chunky-install.sh"):
        assert CORE_PIN not in (REPO / "validation" / script).read_text(), script


def test_an_unreadable_pin_refuses_rather_than_reporting_nothing(tmp_path: Path) -> None:
    root = tree(tmp_path)
    (root / "versions.toml").write_text(f'[engine]\nversion = "{ENGINE_PIN}"\n')
    r = run(root, build_dir(tmp_path), chunky_home(tmp_path, [CORE_PIN]))

    assert r.returncode == 1
    assert "[render].chunky_core" in r.stderr


# ---------------------------------------------------------------------------
# The step that renders
# ---------------------------------------------------------------------------

JAVA_STUB = """#!/usr/bin/env bash
printf '%s\\n' "$@" > "$JAVA_ARGS_OUT"
"""


def render(root: Path, home: Path, *args: str) -> tuple[subprocess.CompletedProcess, Path]:
    bindir = root.parent / "javabin"
    bindir.mkdir(exist_ok=True)
    java = bindir / "java"
    java.write_text(JAVA_STUB)
    java.chmod(0o755)
    shim = root.parent / "shim"
    shim.mkdir(exist_ok=True)
    if not (shim / "python3").exists():
        (shim / "python3").symlink_to(sys.executable)
    out = root.parent / "java-args.txt"
    env = dict(os.environ)
    env["PATH"] = os.pathsep.join([str(bindir), str(shim), "/usr/bin", "/bin"])
    env["DELVEWRIGHT_CHUNKY_HOME"] = str(home)
    env["JAVA_ARGS_OUT"] = str(out)
    r = subprocess.run(
        ["bash", str(root / "validation" / "chunky.sh"), *args],
        env=env,
        capture_output=True,
        text=True,
    )
    return r, out


def test_chunky_sh_renders_with_the_pinned_classpath(tmp_path: Path) -> None:
    root = tree(tmp_path)
    home = chunky_home(tmp_path, [CORE_PIN, CORE_OTHER])
    r, out = render(root, home, "-scene-dir", "s", "-render", "x")

    assert r.returncode == 0, r.stderr
    argv = out.read_text().splitlines()
    assert argv[0] == f"-Dchunky.home={home}"
    classpath = argv[argv.index("-cp") + 1].split(os.pathsep)
    assert classpath == [str(home / "lib" / f"{CORE_PIN}.jar"), str(home / "lib" / "gson-2.9.0.jar")]
    assert CORE_OTHER not in argv[argv.index("-cp") + 1]
    assert argv[argv.index("-cp") + 2] == "se.llbit.chunky.main.Chunky"
    assert argv[-4:] == ["-scene-dir", "s", "-render", "x"]
    assert f"rendering with {CORE_PIN}" in r.stderr


def test_chunky_sh_refuses_a_home_without_the_pin(tmp_path: Path) -> None:
    root = tree(tmp_path)
    r, out = render(root, chunky_home(tmp_path, [CORE_OTHER]), "-render", "x")

    assert r.returncode == 2
    assert not out.exists(), "java was started"
    assert f"the pinned core {CORE_PIN} is not installed" in r.stderr
    assert "chunky-install.sh" in r.stderr
    assert "refusing to render" in r.stderr


def test_chunky_sh_refuses_a_forged_pin(tmp_path: Path) -> None:
    root = tree(tmp_path)
    r, out = render(root, chunky_home(tmp_path, [CORE_PIN], forged=True), "-render", "x")

    assert r.returncode == 2
    assert not out.exists(), "java was started"
    assert "is named as the pin and is not it" in r.stderr


def test_chunky_sh_refuses_a_library_that_is_not_the_recorded_one(tmp_path: Path) -> None:
    root = tree(tmp_path)
    home = chunky_home(tmp_path, [CORE_PIN])
    (home / "lib" / "gson-2.9.0.jar").write_bytes(b"something else")
    r, out = render(root, home, "-render", "x")

    assert r.returncode == 2
    assert not out.exists(), "java was started"
    assert "does not carry the md5" in r.stderr


def test_chunky_sh_refuses_a_pin_with_no_version_record(tmp_path: Path) -> None:
    root = tree(tmp_path)
    r, out = render(root, chunky_home(tmp_path, [CORE_PIN], record=False), "-render", "x")

    assert r.returncode == 2
    assert not out.exists(), "java was started"
    assert "names the libraries it runs with" in r.stderr


# ---------------------------------------------------------------------------
# The content digest and the installer's own refusals
# ---------------------------------------------------------------------------


def test_the_digest_is_over_content_not_over_the_zip(tmp_path: Path) -> None:
    """Two builds of one revision differ in the zip's timestamps and in the
    `Version.properties` comment; a different revision differs in a class."""
    a = core_jar(tmp_path / "a.jar", PIN_BYTES, comment=b"#Sun Jul 26\n")
    b = core_jar(tmp_path / "b.jar", PIN_BYTES, comment=b"#Mon Sep 14\n")
    c = core_jar(tmp_path / "c.jar", OTHER_BYTES, comment=b"#Sun Jul 26\n")
    assert a.read_bytes() != b.read_bytes()
    assert digest_of(a) == digest_of(b)
    assert digest_of(a) != digest_of(c)


def install(root: Path, home: Path, java_home: Path | None) -> subprocess.CompletedProcess:
    shim = root.parent / "shim"
    shim.mkdir(exist_ok=True)
    if not (shim / "python3").exists():
        (shim / "python3").symlink_to(sys.executable)
    env = dict(os.environ)
    env["PATH"] = os.pathsep.join([str(shim), "/usr/bin", "/bin"])
    env["DELVEWRIGHT_CHUNKY_HOME"] = str(home)
    env.pop("JAVA_HOME", None)
    args = ["bash", str(root / "validation" / "chunky-install.sh"), "--work", str(root.parent / "work")]
    if java_home is not None:
        args += ["--java-home", str(java_home)]
    return subprocess.run(args, env=env, capture_output=True, text=True)


def fake_jdk(tmp_path: Path, major: str) -> Path:
    home = tmp_path / f"jdk-{major}"
    (home / "bin").mkdir(parents=True)
    java = home / "bin" / "java"
    java.write_text(f'#!/usr/bin/env bash\necho \'openjdk version "{major}.0.1" 2026-01-01\' >&2\n')
    java.chmod(0o755)
    return home


def test_the_installer_leaves_a_home_that_holds_the_pin_alone(tmp_path: Path) -> None:
    root = tree(tmp_path)
    r = install(root, chunky_home(tmp_path, [CORE_PIN]), None)

    assert r.returncode == 0, r.stderr
    assert "nothing to do" in r.stdout
    assert not (root.parent / "work").exists()


def test_the_installer_refuses_a_jdk_the_revision_does_not_build_under(tmp_path: Path) -> None:
    root = tree(tmp_path)
    home = chunky_home(tmp_path, [CORE_OTHER])
    for java_home in (fake_jdk(tmp_path, "21"), None):
        r = install(root, home, java_home)
        assert r.returncode == 2, r.stderr
        assert "builds under JDK 17" in r.stderr
        assert "--java-home" in r.stderr
        assert not (root.parent / "work").exists(), "a build started"
    assert not (home / "lib" / f"{CORE_PIN}.jar").exists()
