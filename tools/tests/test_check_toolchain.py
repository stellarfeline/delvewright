"""`check-toolchain.py`, the per-run pin check (Init I1b), perturbed per comparison.

Every run of `/new-delve` asks it before anything reads a document, and the page
binds to its exit codes. Each test builds a real machine in a scratch `HOME` —
a skill root holding copies of the two scripts and a pin, a git checkout for
the engine, a `delvec` that answers a version, and an `env.sh` — then moves
exactly one thing and checks the script names the step that repairs it.

The I4 command is taken out of `references/init.md`'s own fence and run, so the
repair the check prescribes is the one the page actually carries.
"""

from __future__ import annotations

import os
import pathlib
import re
import shutil
import subprocess
import sys

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
SKILL = REPO / ".claude" / "skills" / "delvewright" / "skills" / "new-delve"
SCRIPT = SKILL / "scripts" / "check-toolchain.py"
INIT = SKILL / "references" / "init.md"

# An invented engine: the numbers are the rig's own, so no pin this repository
# holds can move a test here.
RELEASE = "v9.8.7"
OTHER = "delvec 9.8.6, dsl 0.0.0, mc 1.21.11"
ANSWER = "delvec 9.8.7, dsl 0.0.0, mc 1.21.11"


def _git(*args: str, cwd: pathlib.Path) -> str:
    env = {
        **os.environ,
        "GIT_AUTHOR_NAME": "rig",
        "GIT_AUTHOR_EMAIL": "rig@example.invalid",
        "GIT_COMMITTER_NAME": "rig",
        "GIT_COMMITTER_EMAIL": "rig@example.invalid",
        "GIT_CONFIG_GLOBAL": os.devnull,
    }
    return subprocess.run(
        ["git", *args], cwd=cwd, env=env, check=True, capture_output=True, text=True
    ).stdout.strip()


def _commit(engine: pathlib.Path, text: str) -> str:
    (engine / "versions.toml").write_text(text, encoding="utf-8")
    _git("add", "versions.toml", cwd=engine)
    _git("commit", "-q", "-m", "rig", cwd=engine)
    return _git("rev-parse", "HEAD", cwd=engine)


def _delvec(bindir: pathlib.Path, answer: str) -> None:
    bindir.mkdir(parents=True, exist_ok=True)
    binary = bindir / "delvec"
    binary.write_text(f"#!/bin/sh\necho '{answer}'\n", encoding="utf-8")
    binary.chmod(0o755)


class Machine:
    def __init__(self, root: pathlib.Path) -> None:
        self.home = root / "home"
        self.skill = root / "plugin" / "skills" / "new-delve"
        self.engine = self.home / ".delvewright" / "engine"
        self.bindir = self.home / ".delvewright" / "bin"
        self.env = self.home / ".delvewright" / "env.sh"

        (self.skill / "scripts").mkdir(parents=True)
        for name in ("check-toolchain.py", "fetch-delvec.py"):
            shutil.copy2(SKILL / "scripts" / name, self.skill / "scripts" / name)

        self.engine.mkdir(parents=True)
        _git("init", "-q", cwd=self.engine)
        self.ref = _commit(self.engine, '[engine]\nversion = "9.8.7"\n')
        self.pin(RELEASE, self.ref)
        _delvec(self.bindir, ANSWER)
        self.write_env(
            mode="creator", engine=self.engine, skill=self.skill, bindir=self.bindir
        )

    def pin(self, release: str, ref: str) -> None:
        (self.skill / "versions.toml").write_text(
            f'[engine]\nrepo = "stellarfeline/delvewright"\n'
            f'release = "{release}"\nref = "{ref}"\n',
            encoding="utf-8",
        )

    def write_env(self, **values: object) -> None:
        self.env.write_text(
            f'export DELVEWRIGHT_MODE="{values["mode"]}"\n'
            f'export DELVEWRIGHT_ENGINE="{values["engine"]}"\n'
            f'export DELVEWRIGHT_SKILL="{values["skill"]}"\n'
            f'export PATH="{values["bindir"]}:$PATH"\n',
            encoding="utf-8",
        )

    def check(self, mode: str = "creator", engine: pathlib.Path | None = None, **extra: str):
        env = {**os.environ, "HOME": str(self.home), **extra}
        for key in ("DELVEWRIGHT_SKILL", "DELVEWRIGHT_MODE", "DELVEWRIGHT_ENGINE"):
            if key not in extra:
                env.pop(key, None)
        return subprocess.run(
            [
                sys.executable,
                str(self.skill / "scripts" / "check-toolchain.py"),
                "--mode",
                mode,
                "--engine",
                str(engine or self.engine),
            ],
            capture_output=True,
            text=True,
            env=env,
        )


@pytest.fixture
def machine(tmp_path):
    return Machine(tmp_path)


def _refused_steps(stdout: str) -> list[str]:
    m = re.search(r"REFUSED — run (.+?), then this check again", stdout)
    assert m, stdout
    return m.group(1).split(", ")


# ------------------------------------------------------------- the baseline --


def test_a_machine_at_the_pin_agrees_on_every_comparison(machine):
    """The baseline, without which every perturbation below proves nothing."""
    proc = machine.check()
    assert proc.returncode == 0, proc.stdout + proc.stderr
    agreeing = [line for line in proc.stdout.splitlines() if line.endswith("— agrees")]
    # the tree, the binary, and three lines of env.sh: five comparisons, all bound
    assert len(agreeing) == 5, proc.stdout
    assert "DISAGREES" not in proc.stdout


# ------------------------------------------------------- one move per comparison --


def test_a_moved_pin_release_is_refused_and_names_the_download(machine):
    """The plugin update: the page pins a newer engine than the binary on disk."""
    machine.pin("v9.8.8", machine.ref)  # the release moves, the tree does not
    proc = machine.check()
    assert proc.returncode == 3, proc.stdout
    assert "found delvec 9.8.7, dsl 0.0.0, mc 1.21.11, want delvec 9.8.8" in proc.stdout
    assert _refused_steps(proc.stdout) == ["I3a", "I4"]


def test_a_binary_answering_another_number_is_refused(machine):
    _delvec(machine.bindir, OTHER)
    proc = machine.check()
    assert proc.returncode == 3
    assert _refused_steps(proc.stdout) == ["I3a", "I4"]


def test_a_tree_at_another_revision_is_refused_and_names_I2(machine):
    old = machine.ref
    new = _commit(machine.engine, '[engine]\nversion = "9.8.7"\n# moved\n')
    machine.pin(RELEASE, new)
    _git("checkout", "-q", "--detach", old, cwd=machine.engine)
    proc = machine.check()
    assert proc.returncode == 3
    assert f"found {old}, want {new}" in proc.stdout
    assert _refused_steps(proc.stdout) == ["I2"]


def test_an_env_sh_written_from_another_skill_root_is_refused_and_names_I4(machine, tmp_path):
    machine.write_env(
        mode="creator", engine=machine.engine, skill=tmp_path / "old-root", bindir=machine.bindir
    )
    proc = machine.check()
    assert proc.returncode == 3
    assert "env.sh DELVEWRIGHT_SKILL" in proc.stdout
    assert _refused_steps(proc.stdout) == ["I4"]


def test_an_env_sh_of_the_other_mode_is_refused(machine):
    machine.write_env(
        mode="dev", engine=machine.engine, skill=machine.skill, bindir=machine.bindir
    )
    proc = machine.check()
    assert proc.returncode == 3
    assert _refused_steps(proc.stdout) == ["I4"]


def test_a_value_inherited_from_the_callers_shell_does_not_stand_in_for_env_sh(machine):
    """An `export` left over in the caller cannot supply a line the file lacks."""
    machine.env.write_text(
        f'export DELVEWRIGHT_MODE="creator"\n'
        f'export DELVEWRIGHT_ENGINE="{machine.engine}"\n'
        f'export PATH="{machine.bindir}:$PATH"\n',
        encoding="utf-8",
    )
    proc = machine.check(DELVEWRIGHT_SKILL=str(machine.skill))
    assert proc.returncode == 3, proc.stdout
    assert "env.sh DELVEWRIGHT_SKILL: found nothing" in proc.stdout


def test_a_delvec_only_the_callers_path_carries_is_not_the_one_env_sh_runs(machine, tmp_path):
    """The binary is the one `env.sh` resolves — the caller's PATH is the tail of it."""
    machine.write_env(
        mode="creator", engine=machine.engine, skill=machine.skill, bindir=tmp_path / "empty"
    )
    elsewhere = tmp_path / "elsewhere"
    _delvec(elsewhere, OTHER)
    proc = machine.check(PATH=f"{elsewhere}:{os.environ['PATH']}")
    assert proc.returncode == 3
    assert f"at {elsewhere / 'delvec'}" in proc.stdout


def test_no_env_sh_is_the_first_run_code(machine):
    machine.env.unlink()
    proc = machine.check()
    assert proc.returncode == 4
    assert "I2 through I8" in proc.stdout


def test_an_env_sh_that_does_not_source_is_unusable(machine):
    machine.env.write_text("if then fi\n", encoding="utf-8")
    proc = machine.check()
    assert proc.returncode == 2, proc.stdout + proc.stderr


# -------------------------------------------------------------------- dev mode --


def test_dev_mode_holds_the_binary_to_the_checkouts_own_number(machine):
    """I3b's rule, unchanged: the checkout's `[engine].version`, never the pin's release."""
    dev = machine.engine
    machine.pin("v1.0.0", "0" * 40)  # a pin that disagrees with everything in dev
    skill_in_checkout = machine.skill
    machine.write_env(mode="dev", engine=dev, skill=skill_in_checkout, bindir=machine.bindir)
    proc = machine.check(mode="dev", engine=dev)
    assert proc.returncode == 0, proc.stdout
    assert "dev mode, recorded and not compared" in proc.stdout

    _commit(dev, '[engine]\nversion = "9.9.0"\n')
    proc = machine.check(mode="dev", engine=dev)
    assert proc.returncode == 3
    assert _refused_steps(proc.stdout) == ["I3b", "I4"]


# -------------------------------------------------- the repair the page carries --


def _i4_fence() -> str:
    text = INIT.read_text(encoding="utf-8")
    section = text.split("\n## I4 ")[1].split("\n## ")[0]
    fences = re.findall(r"```sh\n(.*?)```", section, re.S)
    writers = [f for f in fences if "env.sh" in f and "EOF" in f]
    assert len(writers) == 1, f"{len(writers)} env.sh-writing fences under I4"
    return writers[0]


def test_the_i4_the_page_carries_repairs_env_sh_and_keeps_every_other_line(machine):
    """The check prescribes I4; this runs the page's I4 and asks the check again."""
    machine.write_env(
        mode="creator", engine=machine.engine, skill="/an/older/root", bindir=machine.bindir
    )
    with machine.env.open("a", encoding="utf-8") as fh:
        fh.write('export DELVEWRIGHT_PREFABS="/the/library"\nexport SOME_PROVIDER_KEY="k"\n')
    assert machine.check().returncode == 3

    command = _i4_fence().replace("<the bin or target/release directory>", str(machine.bindir))
    run_env = {
        **os.environ,
        "HOME": str(machine.home),
        "JAVA_HOME": "/nowhere/jdk",
        "DELVEWRIGHT_MODE": "creator",
        "DELVEWRIGHT_ENGINE": str(machine.engine),
        "DELVEWRIGHT_PYTHON": sys.executable,
        "DELVEWRIGHT_SKILL": str(machine.skill),
    }
    for _ in range(2):  # safe to run any number of times
        subprocess.run(["sh", "-c", command], env=run_env, check=True)

    text = machine.env.read_text(encoding="utf-8")
    assert 'export DELVEWRIGHT_PREFABS="/the/library"' in text
    assert 'export SOME_PROVIDER_KEY="k"' in text
    for key in ("JAVA_HOME", "DELVEWRIGHT_MODE", "DELVEWRIGHT_SKILL", "DELVEWRIGHT_PREFABS", "PATH"):
        assert len(re.findall(rf"^export {key}=", text, re.M)) == 1, (key, text)
    proc = machine.check()
    assert proc.returncode == 0, proc.stdout


# ------------------------------------------------------------------ the script --


def test_the_script_is_stdlib_only():
    source = SCRIPT.read_text(encoding="utf-8")
    imported = {
        line.split()[1].split(".")[0]
        for line in source.splitlines()
        if line.startswith(("import ", "from ")) and "__future__" not in line
    }
    assert imported <= set(sys.stdlib_module_names), sorted(
        imported - set(sys.stdlib_module_names)
    )


def test_the_exit_codes_the_page_names_are_the_scripts(machine):
    """Init I1b's table binds to 0, 2, 3 and 4; the constants are those numbers."""
    import importlib.util

    spec = importlib.util.spec_from_file_location("check_toolchain", SCRIPT)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    assert (mod.EXIT_UNUSABLE, mod.EXIT_MISMATCH, mod.EXIT_NO_TOOLCHAIN) == (2, 3, 4)
    table = INIT.read_text(encoding="utf-8").split("\n## I1b ")[1].split("\n## ")[0]
    for code in ("`0`", "`2`", "`3`", "`4`"):
        assert f"| {code}" in table, code
