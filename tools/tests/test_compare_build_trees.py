"""`tools/ci/compare-build-trees.py`: two build trees, a hash per file, and the
differences no allowlist names (spec-0106 §6).

Each perturbation is one way the comparison could pass over a real difference:
a changed datapack function, a file present on one side only, a resource-pack
entry, a second change inside an allowlisted JSON file, a stale admission, and
a tree with nothing in it.
"""

from __future__ import annotations

import io
import json
import subprocess
import sys
import zipfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "tools" / "ci" / "compare-build-trees.py"
LANG = "resourcepack.zip!/assets/delvewright/lang/en_us.json"


def tree(root: Path, lang: dict, fn: str = "say hi\n", extra: str | None = None) -> Path:
    (root / "datapack/data/x/function").mkdir(parents=True)
    (root / "datapack/data/x/function/a.mcfunction").write_text(fn)
    if extra:
        (root / "datapack/data/x/function/b.mcfunction").write_text(extra)
    (root / "server").mkdir()
    (root / "server/server.properties").write_text("motd=x\n")
    (root / "validation").mkdir()
    (root / "validation/blockout.json").write_text("{}")
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        z.writestr("assets/delvewright/lang/en_us.json", json.dumps(lang))
    (root / "resourcepack.zip").write_bytes(buf.getvalue())
    return root


def run(*args: object) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *map(str, args)], capture_output=True, text=True
    )


def base(tmp: Path) -> Path:
    return tree(tmp / "a", {"k": "v", "area.name": "Castle"})


def test_identical_trees_pass_and_state_their_binding(tmp_path: Path) -> None:
    a = base(tmp_path)
    b = tree(tmp_path / "b", {"k": "v", "area.name": "Castle"})
    r = run(a, b)
    assert r.returncode == 0, r.stdout
    assert "3 file(s) in A, 3 in B, 3 compared; 0 differing" in r.stdout


def test_the_named_key_alone_is_admitted(tmp_path: Path) -> None:
    a = base(tmp_path)
    b = tree(tmp_path / "b", {"k": "v"})
    r = run(a, b, "--allow", f"{LANG}#area.name")
    assert r.returncode == 0, r.stdout
    assert "by key: area.name" in r.stdout


def test_a_second_change_in_an_admitted_file_reds(tmp_path: Path) -> None:
    a = base(tmp_path)
    b = tree(tmp_path / "b", {"k": "w"})
    r = run(a, b, "--allow", f"{LANG}#area.name")
    assert r.returncode == 1
    assert "differs beyond its admitted key(s)" in r.stdout


def test_a_changed_function_reds(tmp_path: Path) -> None:
    a = base(tmp_path)
    b = tree(tmp_path / "b", {"k": "v", "area.name": "Castle"}, fn="say ho\n")
    r = run(a, b)
    assert r.returncode == 1
    assert "DIFFERS: datapack/data/x/function/a.mcfunction" in r.stdout


def test_a_file_on_one_side_only_reds(tmp_path: Path) -> None:
    a = base(tmp_path)
    b = tree(tmp_path / "b", {"k": "v", "area.name": "Castle"}, extra="x\n")
    r = run(a, b)
    assert r.returncode == 1
    assert "b.mcfunction" in r.stdout


def test_a_stale_admission_reds(tmp_path: Path) -> None:
    a = base(tmp_path)
    b = tree(tmp_path / "b", {"k": "v", "area.name": "Castle"})
    r = run(a, b, "--allow", f"{LANG}#area.name")
    assert r.returncode == 1
    assert "STALE" in r.stdout


def test_report_only_lists_without_judging(tmp_path: Path) -> None:
    a = base(tmp_path)
    b = tree(tmp_path / "b", {"k": "v", "area.name": "Castle"})
    (b / "validation/blockout.json").write_text('{"x": 1}')
    r = run(a, b, "--report-only", "validation")
    assert r.returncode == 0, r.stdout
    assert "report-only validation/: 1 file(s) in A, 1 in B, 1 differing" in r.stdout


def test_an_empty_tree_is_refused(tmp_path: Path) -> None:
    a = base(tmp_path)
    (tmp_path / "empty").mkdir()
    r = run(a, tmp_path / "empty")
    assert r.returncode == 1
    assert "a comparison over nothing is not a pass" in r.stderr
