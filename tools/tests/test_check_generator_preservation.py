"""`tools/check-generator-preservation.py` — a generator deletes nothing it did
not write.

The gate itself runs the generators, which needs a release build of a second
workspace; that half lives in CI. What is bound HERE is everything the gate
decides with — the two halves that can be wrong without any generator running:

  * the population is DERIVED from `prefabs/Cargo.toml`'s own workspace members,
    so a generator added and forgotten in a list here cannot exist;
  * `leaf_paths` counts the same things the Rust writer counts, so a key
    "surviving" means the same to both sides of the pair;
  * `decorate` plants only keys the generator does NOT already write, because
    overwriting one would measure re-measurement rather than preservation — the
    defect this suite caught the first time the gate was run.
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools" / "check-generator-preservation.py"


def load():
    spec = importlib.util.spec_from_file_location("gen_preservation", TOOL)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_the_population_is_the_workspaces_own_member_list():
    module = load()
    members = module.workspace_members()
    declared = re.search(
        r"^members\s*=\s*\[(.*?)\]",
        (ROOT / "prefabs" / "Cargo.toml").read_text(encoding="utf-8"),
        re.S | re.M,
    ).group(1)
    assert members == re.findall(r'"([^"]+)"', declared)
    assert len(members) > 1, f"the sweep would examine {len(members)} member(s)"
    # Every member is a real directory: a stale name would make the gate skip in
    # silence rather than red.
    for m in members:
        assert (ROOT / "prefabs" / m / "Cargo.toml").is_file(), m


def test_leaf_paths_reach_into_arrays_and_objects():
    module = load()
    assert module.leaf_paths(
        {"a": {"b": 1}, "c": [{"d": 2}, 3], "e": [], "f": {}}
    ) == ["a.b", "c[0].d", "c[1]", "e", "f"]


def test_the_rust_writer_and_this_gate_agree_on_what_a_leaf_is():
    """The pair reads one rule. `prefab_invariants::document::leaf_paths` and the
    function above are two implementations of the same walk, so the Rust side's
    own unit test asserts the identical expectation over the identical value."""
    rust = (ROOT / "prefabs" / "invariants" / "src" / "document.rs").read_text(encoding="utf-8")
    assert 'vec!["a.b", "c[0].d", "c[1]", "e", "f"]' in rust


def test_decorate_leaves_a_key_the_generator_already_writes_alone(tmp_path):
    module = load()
    doc = tmp_path / "piece.json"
    doc.write_text(
        json.dumps(
            {
                "prefab_id": "prefab/x",
                "spatial_contract": {"entry": "hall", "spaces": {"hall": {"boxes": []}}},
                "anchors": {"anchor/a": {"pos": [1, 1, 1], "role": "entry"}},
            }
        ),
        encoding="utf-8",
    )
    planted = module.decorate(doc)
    after = json.loads(doc.read_text(encoding="utf-8"))
    # The generator's own contract is untouched, and no path of it was claimed.
    assert after["spatial_contract"]["spaces"] == {"hall": {"boxes": []}}
    assert not any(p.startswith("spatial_contract") for p in planted)
    # An existing `role` is likewise the generator's, and is not re-planted.
    assert "anchors.anchor/a.role" not in planted
    # What IS planted is what a later step really adds.
    assert "anchors.anchor/hand-added-by-a-library-round.pos[0]" in planted
    assert "shown_faces[0]" in planted
    assert "anchors.anchor/a.note" in planted
    assert after["anchors"]["anchor/a"]["role"] == "entry"


def test_a_pool_declaration_is_decorated_as_the_object_it_is(tmp_path):
    """`pools.json` is not a prefab document: it has no anchors and no faces, and
    the object decides that rather than a filename list."""
    module = load()
    doc = tmp_path / "pools.json"
    doc.write_text(json.dumps({"pools": {"pool/x": []}}), encoding="utf-8")
    planted = module.decorate(doc)
    after = json.loads(doc.read_text(encoding="utf-8"))
    assert "anchors" not in after
    assert planted == ["x_added_by_a_newer_producer.note"]
