"""The gallery pieces are handed down whole: everything generation wrote, caches excepted.

`tools/ci/gallery-pieces.py` packs by what is on disk rather than by a typed list
of roots, so these plant the shapes that would make the hand-down carry less than
the generator wrote — or carry nothing — and check each is refused or packed.
"""

from __future__ import annotations

import importlib.util
import pathlib
import subprocess
import tarfile

import pytest

TOOLS = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("gallery_pieces", TOOLS / "ci" / "gallery-pieces.py")
mod = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(mod)


def _git(repo: pathlib.Path, *args: str) -> None:
    subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True)


@pytest.fixture
def repo(tmp_path: pathlib.Path) -> pathlib.Path:
    r = tmp_path / "repo"
    (r / "gallery" / "design").mkdir(parents=True)
    (r / "gallery" / "design" / "cameras.json").write_text("{}\n")
    (r / ".gitignore").write_text("gallery-prefabs/\ngallery/skins/\ngallery/design/*\n!gallery/design/cameras.json\ntarget/\n__pycache__/\n")
    _git(r, "init", "-q")
    _git(r, "add", "-A")
    _git(r, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "base")
    return r


def _write(repo: pathlib.Path, rel: str, body: str = "x") -> None:
    p = repo / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(body)


def test_everything_generation_wrote_is_packed_and_caches_are_not(repo, tmp_path, capsys):
    for rel in ["gallery-prefabs/hall.nbt", "gallery/skins/a.png", "gallery/design/ref.png", "gallery/textures/t.png"]:
        _write(repo, rel)
    for rel in ["target/debug/delvec", "prefabs/gallery-generator/target/release/g", "tools/ci/__pycache__/x.pyc"]:
        _write(repo, rel)
    out = tmp_path / "pieces.tar"
    assert mod.main(["--out", str(out), "--repo", str(repo)]) == 0
    with tarfile.open(out) as tar:
        names = sorted(tar.getnames())
    assert names == ["gallery-prefabs/hall.nbt", "gallery/design/ref.png", "gallery/skins/a.png", "gallery/textures/t.png"]
    assert "skipped 3 build-cache file(s)" in capsys.readouterr().out


def test_a_tracked_file_the_generation_modified_is_refused(repo, tmp_path, capsys):
    _write(repo, "gallery-prefabs/hall.nbt")
    _write(repo, "gallery/design/cameras.json", "{\"moved\": true}\n")
    assert mod.main(["--out", str(tmp_path / "p.tar"), "--repo", str(repo)]) == 1
    assert "modified 1 tracked file(s)" in capsys.readouterr().err


def test_a_hand_down_of_nothing_is_refused(repo, tmp_path, capsys):
    _write(repo, "target/debug/delvec")
    assert mod.main(["--out", str(tmp_path / "p.tar"), "--repo", str(repo)]) == 1
    assert "ZERO untracked files outside the build caches (1 cache file(s) skipped)" in capsys.readouterr().err
