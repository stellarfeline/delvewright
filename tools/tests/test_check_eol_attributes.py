r"""Guards for `tools/check-eol-attributes.py`.

The red this gate exists to prevent: a Windows checkout writes a tracked script
to disk with CRLF, and the kernel reads `#!/usr/bin/env bash\r` as a request for
an interpreter named `bash\r`. `.gitattributes` repairs it per pattern, and a
pattern list is only ever as wide as the extension somebody remembered — so the
gate derives the population from the tree and refuses a member no pattern
covers.

Each test builds a tiny real git repository (an index, no commit — `git
ls-files -s` and `git check-attr` both read the index) and runs the gate against
it through `--repo`, so what is under test is the shipped command line rather
than an importable fragment of it.

The perturbation the vacuity doctrine asks for is `test_uncovered_shebang_reds`:
a tracked file with a shebang that no pattern reaches. It is a perturbation only
this gate can catch — the tree still compiles, every other check is green, and
the file runs correctly on the machine that wrote it.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-eol-attributes.py"

COVERS_SHELL_ONLY = "*.sh text eol=lf\n"


def git(repo: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True)


def fixture_repo(root: Path, attributes: str) -> Path:
    repo = root / "tree"
    repo.mkdir()
    git(repo, "init", "-q")
    (repo / ".gitattributes").write_text(attributes, encoding="utf-8")
    write(repo, "run.sh", "#!/usr/bin/env bash\necho hello\n", executable=True)
    return repo


def write(repo: Path, rel: str, body: str, *, executable: bool = False) -> Path:
    path = repo / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")
    if executable:
        path.chmod(0o755)
    return path


def run(repo: Path, *flags: str) -> subprocess.CompletedProcess[str]:
    git(repo, "add", "-A")
    return subprocess.run(
        ["python3", str(CHECKER), "--repo", str(repo), *flags],
        capture_output=True,
        text=True,
    )


# ------------------------------------------------------------------ the tree


def test_this_repository_is_clean_and_binds() -> None:
    """The live tree passes, and says how many files it judged out of how many."""
    proc = subprocess.run(
        ["python3", str(CHECKER)], capture_output=True, text=True, cwd=REPO
    )
    assert proc.returncode == 0, proc.stderr
    m = re.search(r"(\d+) of (\d+) tracked file\(s\)", proc.stdout)
    assert m, proc.stdout
    bound, denominator = int(m.group(1)), int(m.group(2))
    assert bound > 0
    assert denominator > bound


# ---------------------------------------------------------- the perturbation


def test_uncovered_shebang_reds(tmp_path: Path) -> None:
    """A tracked shebang file no pattern covers is named, and the gate reds."""
    repo = fixture_repo(tmp_path, COVERS_SHELL_ONLY)
    write(repo, "tools/report.rb", "#!/usr/bin/env ruby\nputs 1\n")
    proc = run(repo)
    assert proc.returncode == 1
    assert "tools/report.rb" in proc.stderr
    assert "eol is `unspecified`" in proc.stderr


def test_executable_bit_without_a_shebang_is_in_the_population(tmp_path: Path) -> None:
    """The population is a union: the executable bit alone puts a file in it."""
    repo = fixture_repo(tmp_path, COVERS_SHELL_ONLY)
    write(repo, "tools/launch", "exec /usr/bin/true\n", executable=True)
    proc = run(repo)
    assert proc.returncode == 1
    assert "tools/launch" in proc.stderr


def test_covering_the_new_extension_greens_it(tmp_path: Path) -> None:
    """The remedy the gate prints is the remedy that works."""
    repo = fixture_repo(tmp_path, COVERS_SHELL_ONLY + "*.rb text eol=lf\n")
    write(repo, "tools/report.rb", "#!/usr/bin/env ruby\nputs 1\n")
    proc = run(repo)
    assert proc.returncode == 0, proc.stderr
    assert "every one pinned `eol=lf`" in proc.stdout


# ------------------------------------------------------- what is NOT a shebang


def test_rust_inner_attribute_is_not_a_shebang(tmp_path: Path) -> None:
    """`#![allow(…)]` opens with `#!` and is never handed to the kernel."""
    repo = fixture_repo(tmp_path, COVERS_SHELL_ONLY)
    write(repo, "src/lib.rs", "#![allow(dead_code)]\npub fn f() {}\n")
    proc = run(repo)
    assert proc.returncode == 0, proc.stderr
    assert "src/lib.rs" not in proc.stderr


# ------------------------------------------------- what only Windows can show


def test_working_tree_crlf_reds_though_the_attribute_is_declared(tmp_path: Path) -> None:
    """The Windows assertion: declared is not the same as materialised."""
    repo = fixture_repo(tmp_path, COVERS_SHELL_ONLY)
    git(repo, "add", "-A")
    (repo / "run.sh").write_bytes(b"#!/usr/bin/env bash\r\necho hello\r\n")

    declared = subprocess.run(
        ["python3", str(CHECKER), "--repo", str(repo)], capture_output=True, text=True
    )
    assert declared.returncode == 0, declared.stderr

    materialised = subprocess.run(
        ["python3", str(CHECKER), "--repo", str(repo), "--working-tree"],
        capture_output=True,
        text=True,
    )
    assert materialised.returncode == 1
    assert "run.sh: materialised with CRLF" in materialised.stderr


# ------------------------------------------------------------------- vacuity


def test_an_empty_population_reds(tmp_path: Path) -> None:
    """A run that judges nothing is a failure, not a pass."""
    repo = tmp_path / "tree"
    repo.mkdir()
    git(repo, "init", "-q")
    write(repo, "README.md", "nothing here runs\n")
    proc = run(repo)
    assert proc.returncode == 1
    assert "vacuous" in proc.stderr
