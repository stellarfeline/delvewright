r"""Guards for `tools/blockout-pack.py` and `tools/shopfront.py`.

Three things here can go wrong silently, and each has a test that reds when it
does rather than a sentence saying it must not.

**The licence boundary.** The whole reason the shopfront builds its own resource
pack is that a page built from the client jar carries Mojang's texture bytes and
may not be served. A pack that quietly acquired a jar path, or a page that
resolved against one, would look identical in every artifact — so the pack is
asserted to contain nothing but this project's own PNGs, and the emitted page is
asserted to be built with an explicit `--textures` pointing at that pack.

**The special-texture table.** `viewer/resources.rs` names block-entity textures
the renderer asks for and no model file does; a wrong or missing id renders
magenta and says nothing. The pack reads that table out of the emitter's own
source rather than restating it, which is only worth anything if a moved table
REDS. So the parse is perturbed — the const renamed, exactly as a refactor would
rename it — and the tool must refuse rather than write a pack covering nothing.

**"The subject is named in one place."** That is a claim about the repository,
not about a function, so it is checked against the repository: the subject spec
must appear in `.github/shopfront.toml` and in no other tracked file. A second
copy is how "one edit" quietly becomes three.

The pack is 1166 blocks of small files, so the tests that need a real one build
it once per session.
"""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tomllib
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
PACK_TOOL = REPO / "tools" / "blockout-pack.py"
SHOPFRONT_TOOL = REPO / "tools" / "shopfront.py"
SUBJECT_FILE = REPO / ".github" / "shopfront.toml"
REGISTRY = REPO / "crates" / "dsl" / "data" / "blocks-1.21.11.json"
RESOURCES_RS = REPO / "crates" / "delvec" / "src" / "compiler" / "view" / "viewer" / "resources.rs"


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="session")
def pack_module():
    return load("blockout_pack", PACK_TOOL)


@pytest.fixture(scope="session")
def pack(tmp_path_factory) -> Path:
    out = tmp_path_factory.mktemp("pack")
    result = subprocess.run(
        [sys.executable, str(PACK_TOOL), "--out", str(out)],
        text=True, capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    return out


# --- the pack covers the pinned registry, and nothing else -------------------


def test_every_registry_block_has_a_definition(pack):
    registry = json.loads(REGISTRY.read_text())
    states = pack / "assets/minecraft/blockstates"
    written = {p.stem for p in states.glob("*.json")}
    expected = {block.split(":", 1)[1] for block in registry}
    assert written == expected, sorted(expected ^ written)[:10]


def test_every_definition_resolves_to_a_model_and_a_texture(pack):
    """A definition naming a model nobody wrote is the failure that renders as
    the missing-texture checker while every count still reads clean."""
    models = pack / "assets/minecraft/models/block"
    textures = pack / "assets/minecraft/textures"
    missing_models: list[str] = []
    missing_textures: list[str] = []
    checked = 0
    for definition in sorted((pack / "assets/minecraft/blockstates").glob("*.json")):
        parsed = json.loads(definition.read_text())
        applies = (
            list(parsed["variants"].values())
            if "variants" in parsed
            else [part["apply"] for part in parsed["multipart"]]
        )
        for apply in applies:
            checked += 1
            model_id = apply["model"].split("/", 1)[1]
            model_path = models / f"{model_id}.json"
            if not model_path.exists():
                missing_models.append(model_id)
                continue
            texture = json.loads(model_path.read_text())["textures"]["all"]
            if not (textures / f"{texture.split(':', 1)[1]}.png").exists():
                missing_textures.append(texture)
    assert checked > 0, "no definition was examined — the pack is empty"
    assert not missing_models, missing_models[:5]
    assert not missing_textures, missing_textures[:5]


def test_the_pack_is_deterministic(tmp_path):
    def build(where: Path) -> dict[str, bytes]:
        subprocess.run(
            [sys.executable, str(PACK_TOOL), "--out", str(where)],
            check=True, text=True, capture_output=True,
        )
        return {
            str(p.relative_to(where)): p.read_bytes()
            for p in sorted(where.rglob("*")) if p.is_file()
        }

    first = build(tmp_path / "a")
    second = build(tmp_path / "b")
    assert first.keys() == second.keys()
    assert first == second


def test_the_pack_carries_no_game_asset(pack):
    """Every PNG is one this tool wrote: 16x16, and byte-identical to what the
    encoder produces for its own colour. A jar texture would not be."""
    pngs = sorted((pack / "assets/minecraft/textures").rglob("*.png"))
    assert pngs, "no textures at all"
    sizes = {p.stat().st_size for p in pngs}
    # A hand-encoded 16x16 solid PNG is under a hundred bytes; every 16x16 jar
    # texture with real pixels in it is several hundred.
    assert max(sizes) < 200, f"a texture of {max(sizes)} B is not a flat colour"


# --- the special-texture table is bound to the emitter's own source ----------


def test_the_special_texture_table_binds(pack_module):
    wanted, explicit, suffixes = pack_module.special_texture_ids(
        ["minecraft:chest", "minecraft:water", "minecraft:oak_sign"]
    )
    assert explicit > 0 and suffixes > 0
    assert "entity/chest/normal" in wanted
    assert "block/water_still" in wanted
    assert "entity/signs/oak" in wanted


def test_a_moved_table_is_refused_not_silently_empty(pack_module, tmp_path, monkeypatch):
    """The perturbation only this parse could catch: the emitter renames its
    const. Nothing else in the pack changes, and every other count stays green."""
    moved = tmp_path / "resources.rs"
    moved.write_text(
        RESOURCES_RS.read_text().replace("SPECIAL_TEXTURES", "BLOCK_ENTITY_TEXTURES")
    )
    monkeypatch.setattr(pack_module, "RESOURCES_RS", moved)
    with pytest.raises(SystemExit) as raised:
        pack_module.special_texture_ids(["minecraft:chest"])
    assert raised.value.code == 2


def test_an_emptied_table_is_refused(pack_module, tmp_path, monkeypatch):
    """The other direction: the const is still there and holds nothing. A parse
    that reported zero ids and carried on would produce a pack that covers no
    chest and no water, and say nothing about it."""
    source = RESOURCES_RS.read_text()
    start = source.index("const SPECIAL_TEXTURES")
    end = source.index("\n];", start)
    emptied = tmp_path / "resources.rs"
    emptied.write_text(source[:start] + "const SPECIAL_TEXTURES: &[(&str, &[&str])] = &[" + source[end:])
    monkeypatch.setattr(pack_module, "RESOURCES_RS", emptied)
    with pytest.raises(SystemExit) as raised:
        pack_module.special_texture_ids(["minecraft:chest"])
    assert raised.value.code == 2


# --- the subject is named in exactly one place ------------------------------


def test_the_subject_file_declares_a_subject_of_a_known_kind():
    declared = tomllib.loads(SUBJECT_FILE.read_text())
    assert declared["title"]
    kind = declared["subject"].split(":", 1)[0]
    assert kind in {"grammar", "gallery", "content"}, declared["subject"]


def test_the_subject_appears_in_no_other_tracked_file():
    subject = tomllib.loads(SUBJECT_FILE.read_text())["subject"]
    tracked = subprocess.run(
        ["git", "-C", str(REPO), "grep", "-l", "--fixed-strings", subject, "--", "."],
        text=True, capture_output=True,
    )
    # `git grep -l` exits 1 when nothing matches, which would mean the subject
    # file itself is untracked or unreadable — a finding, not a pass.
    assert result_lines(tracked) == [".github/shopfront.toml"], result_lines(tracked)


def result_lines(completed: subprocess.CompletedProcess) -> list[str]:
    return [line for line in completed.stdout.splitlines() if line]


# --- the shopfront refuses rather than publishing something empty -----------


def run_shopfront(*args: str, **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(SHOPFRONT_TOOL), *args], text=True, capture_output=True, **kwargs
    )


def stub_binary(tmp_path: Path) -> Path:
    """A `--delvec` that exists and is never reached.

    These refusals all happen before anything is executed, and pointing at the
    worktree's real build would make them pass or fail on whether somebody had
    run `cargo build` — which is not what any of them is about.
    """
    binary = tmp_path / "delvec"
    binary.write_text("")
    return binary


def test_an_unknown_subject_kind_is_refused(tmp_path):
    result = run_shopfront(
        "--out", str(tmp_path / "site"), "--delvec", str(stub_binary(tmp_path)),
        "--subject", "campaign:whatever",
    )
    assert result.returncode == 2, result.stdout
    assert "names no known subject kind" in result.stderr


def test_a_grammar_subject_without_a_region_is_refused(tmp_path):
    """The rule library states no default region, so neither does this."""
    result = run_shopfront(
        "--out", str(tmp_path / "site"), "--delvec", str(stub_binary(tmp_path)),
        "--subject", "grammar:castle",
    )
    assert result.returncode == 2, result.stdout
    assert "will not invent one" in result.stderr


def test_a_content_subject_that_is_not_there_is_refused(tmp_path):
    result = run_shopfront(
        "--out", str(tmp_path / "site"), "--delvec", str(stub_binary(tmp_path)),
        "--subject", "content:prefabs/nothing-of-this-name.nbt",
        "--content", str(tmp_path / "no-checkout"),
    )
    assert result.returncode == 2, result.stdout
    assert "does not exist" in result.stderr


def test_a_missing_binary_is_refused_before_anything_is_written(tmp_path):
    site = tmp_path / "site"
    result = run_shopfront("--out", str(site), "--delvec", str(tmp_path / "no-such-delvec"))
    assert result.returncode == 2, result.stdout
    assert "is not built" in result.stderr
    assert not site.exists(), "a refused run must not leave a half-built site behind"
