"""Guards for `tools/lib/delvec-bin.sh` — which `delvec` a creator step runs.

The rule under test is one sentence: **a step uses the instrument the creator
established, or it says out loud that it is using a different one.** A creator's
`Init` downloads the pinned release archive, verifies its checksum and puts
`delvec` on `PATH` (ADR-0023). What the third end-to-end drill found is that the
steps after `Init` each answered the question privately and silently, in opposite
directions — `playtest-server.sh` ignored `PATH` and compiled the workspace,
`render-shots.sh` took `PATH` unconditionally and never looked at its version.

So the perturbations here are the two defective shapes themselves:

1. a `PATH` binary of a DIFFERENT engine version must **not** be used, and the
   line the step prints must name both versions — the check nothing else can
   catch, because a wrong-engine build fails no downstream gate;
2. a `PATH` binary of the PINNED version must be used without a source build —
   the 93 seconds the drill paid, and the reason the first check has to be a
   version comparison rather than a refusal to look at `PATH` at all.

The subject is the SHELL LIBRARY, so every engine here is a stub: these are
assertions about which path is chosen and what is said about it, and a real
`delvec` would make them slower without making them stronger.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent

PIN = "9.9.9"


def engine_stub(version: str) -> str:
    """A `delvec` that answers `--version` in the real one's shape."""
    return (
        "#!/usr/bin/env bash\n"
        'case "$1" in\n'
        f'  --version) echo "delvec {version}, dsl 0.0.0, mc 1.21.11"; exit 0;;\n'
        "esac\n"
        "exit 0\n"
    )


MUTE_STUB = "#!/usr/bin/env bash\nexit 1\n"

# A `cargo` that builds nothing but records that it was asked to, and produces
# the binary at the path the library expects — so the source-build arm is
# exercised without a compiler.
CARGO_STUB = """#!/usr/bin/env bash
echo "cargo $*" >> "$CARGO_LOG"
mkdir -p target/release
cat > target/release/delvec <<'BIN'
#!/usr/bin/env bash
case "$1" in
  --version) echo "delvec {pin}, dsl 0.0.0, mc 1.21.11"; exit 0;;
esac
exit 0
BIN
chmod +x target/release/delvec
exit 0
"""


def tree(tmp_path: Path) -> Path:
    """A checkout carrying only what the library reads: the pin and the reader."""
    root = tmp_path / "repo"
    (root / "tools" / "lib").mkdir(parents=True)
    for name in ("delvec-bin.sh", "versions.py"):
        (root / "tools" / "lib" / name).write_bytes(
            (REPO / "tools" / "lib" / name).read_bytes()
        )
    (root / "versions.toml").write_text(f'[engine]\nversion = "{PIN}"\n')
    return root


def write_exe(path: Path, text: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    path.chmod(0o755)
    return path


def resolve(
    root: Path,
    *,
    explicit: str = "",
    path_dirs: list[Path] | None = None,
    cargo_log: Path | None = None,
) -> subprocess.CompletedProcess:
    """Run `dw_resolve_delvec` exactly as a caller does, and hand back both streams."""
    script = (
        f'. "{root}/tools/lib/delvec-bin.sh"\n'
        f'dw_resolve_delvec "{explicit}" "{root}" "test-caller"\n'
    )
    env = dict(os.environ)
    # A PATH with nothing on it but what the case puts there: the machine's own
    # `delvec`, if the developer running the suite has one, would decide the
    # answer instead of the fixture. The one thing carried across is a `python3`
    # that can read TOML — macOS's `/usr/bin/python3` is 3.9 and has no
    # `tomllib`, so a bare system PATH would make every case fail on the reader
    # rather than on what it is asserting.
    shim = root.parent / "shim"
    shim.mkdir(exist_ok=True)
    py = shim / "python3"
    if not py.exists():
        py.symlink_to(sys.executable)
    env["PATH"] = os.pathsep.join(
        [
            *(str(p) for p in (path_dirs or [])),
            str(shim),
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin",
        ]
    )
    env["CARGO_LOG"] = str(cargo_log or (root.parent / "cargo.log"))
    return subprocess.run(
        ["bash", "-c", script],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
    )


def test_path_binary_at_the_pinned_version_is_used(tmp_path: Path) -> None:
    """The 93 seconds: what Init put on PATH is this engine, so it is the engine."""
    root = tree(tmp_path)
    bindir = tmp_path / "bin"
    on_path = write_exe(bindir / "delvec", engine_stub(PIN))

    r = resolve(root, path_dirs=[bindir])

    assert r.returncode == 0, r.stderr
    assert r.stdout.strip() == str(on_path)
    assert "on PATH" in r.stderr
    assert PIN in r.stderr
    assert "cargo" not in r.stderr


def test_path_binary_of_another_version_is_refused_and_both_versions_named(
    tmp_path: Path,
) -> None:
    """The perturbation only this check catches: a DIFFERENT engine on PATH.

    Nothing downstream would notice — the wrong engine builds a delve, every
    gate passes over it, and the artifact is about a compiler nobody chose.
    """
    root = tree(tmp_path)
    bindir = tmp_path / "bin"
    stranger = write_exe(bindir / "delvec", engine_stub("1.0.0"))
    built = write_exe(root / "target" / "release" / "delvec", engine_stub(PIN))

    r = resolve(root, path_dirs=[bindir])

    assert r.returncode == 0, r.stderr
    assert r.stdout.strip() == str(built)
    assert r.stdout.strip() != str(stranger)
    # Both revisions, in words, on the line that says what was chosen.
    assert "1.0.0" in r.stderr and PIN in r.stderr
    assert str(stranger) in r.stderr


def test_a_delvec_that_answers_no_version_is_not_used(tmp_path: Path) -> None:
    """`--version` failing is not the same as agreeing with the pin."""
    root = tree(tmp_path)
    bindir = tmp_path / "bin"
    write_exe(bindir / "delvec", MUTE_STUB)
    built = write_exe(root / "target" / "release" / "delvec", engine_stub(PIN))

    r = resolve(root, path_dirs=[bindir])

    assert r.returncode == 0, r.stderr
    assert r.stdout.strip() == str(built)
    assert "<no version line>" in r.stderr


def test_nothing_usable_builds_from_source_and_says_why(tmp_path: Path) -> None:
    root = tree(tmp_path)
    bindir = tmp_path / "bin"
    write_exe(bindir / "cargo", CARGO_STUB.format(pin=PIN))
    log = tmp_path / "cargo.log"

    r = resolve(root, path_dirs=[bindir], cargo_log=log)

    assert r.returncode == 0, r.stderr
    assert r.stdout.strip() == str(root / "target" / "release" / "delvec")
    assert "building" in r.stderr and "no delvec on PATH" in r.stderr
    assert "build --release -p delvec" in log.read_text()


def test_a_stale_built_binary_is_rebuilt_rather_than_run(tmp_path: Path) -> None:
    """`target/release/delvec` is checked against the pin like anything else.

    A binary left behind by the tree as it was before a version bump answers
    about an engine this tree no longer is.
    """
    root = tree(tmp_path)
    bindir = tmp_path / "bin"
    write_exe(bindir / "cargo", CARGO_STUB.format(pin=PIN))
    write_exe(root / "target" / "release" / "delvec", engine_stub("0.1.0"))
    log = tmp_path / "cargo.log"

    r = resolve(root, path_dirs=[bindir], cargo_log=log)

    assert r.returncode == 0, r.stderr
    assert "0.1.0" in r.stderr and "building" in r.stderr
    assert "build --release -p delvec" in log.read_text()


def test_explicit_delvec_is_used_and_named(tmp_path: Path) -> None:
    root = tree(tmp_path)
    named = write_exe(tmp_path / "elsewhere" / "delvec", engine_stub("0.0.1"))
    bindir = tmp_path / "bin"
    write_exe(bindir / "delvec", engine_stub(PIN))

    r = resolve(root, explicit=str(named), path_dirs=[bindir])

    assert r.returncode == 0, r.stderr
    assert r.stdout.strip() == str(named)
    assert "named by --delvec" in r.stderr


def test_an_unreadable_pin_refuses_rather_than_choosing(tmp_path: Path) -> None:
    """Choosing an engine without knowing which one is pinned is the removed state."""
    root = tree(tmp_path)
    (root / "versions.toml").write_text("[engine]\n")
    bindir = tmp_path / "bin"
    write_exe(bindir / "delvec", engine_stub(PIN))

    r = resolve(root, path_dirs=[bindir])

    assert r.returncode == 1
    assert "[engine].version" in r.stderr
    assert r.stdout.strip() == ""


def test_both_creator_steps_go_through_the_shared_rule(tmp_path: Path) -> None:
    """Not one script: the rule applied to the one script a drill walked is half a gate.

    Both creator-facing steps that reach for `delvec` source the library and
    neither keeps a private default. Read off the scripts, so a third private
    resolver added later reds here.
    """
    for rel in ("tools/playtest-server.sh", "validation/render-shots.sh"):
        src = (REPO / rel).read_text()
        assert "tools/lib/delvec-bin.sh" in src, rel
        assert "dw_resolve_delvec" in src, rel
        assert 'command -v delvec >/dev/null' not in src, rel
        assert 'DELVEC="$REPO_ROOT/target/release/delvec"' not in src, rel
