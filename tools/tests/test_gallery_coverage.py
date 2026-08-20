"""The gallery coverage gate: does it RED when it should? (spec-0039 §8.15)

A coverage gate that can only ever go green is the vacuity this spec exists to
close, so the gate owes a red→green demonstration of its own. Every test here
drives `gallery_units` over a synthetic schema rather than over the real export:
the question is whether the *machinery* fires, and pinning that to the live
surface would make these tests fail every time the DSL grows — which is the one
thing the gate is supposed to do quietly.

The reachability half — that the release pipeline and the staging surface cannot
name the gallery — lives in `test_gallery_not_shippable.py`.
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import sys

REPO = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools"))

from gallery_units import Binder, Enumerator  # noqa: E402


def _load_checker():
    spec = importlib.util.spec_from_file_location(
        "check_gallery_coverage", REPO / "tools" / "check-gallery-coverage.py"
    )
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _schema(extra_props: dict | None = None, extra_variants: list | None = None) -> dict:
    """A miniature stage export with one record type and one tagged union."""
    props = {"id": {"type": "string"}, "name": {"type": "string"}}
    props.update(extra_props or {})
    variants = [
        {
            "type": "object",
            "properties": {"kind": {"const": "open", "type": "string"}, "at": {"type": "string"}},
            "required": ["kind"],
        },
        {
            "type": "object",
            "properties": {"kind": {"const": "shut", "type": "string"}},
            "required": ["kind"],
        },
    ]
    variants += extra_variants or []
    return {
        "world": {
            "title": "Envelope_for_WorldContent",
            "type": "object",
            "properties": {"content": {"$ref": "#/$defs/WorldContent"}},
            "$defs": {
                "WorldContent": {"type": "object", "properties": props},
                "Verb": {"oneOf": variants},
            },
        }
    }


def _doc(content: dict) -> dict:
    return {"content": content}


def test_enumerates_properties_and_variants():
    units = Enumerator(_schema()).run()
    assert "WorldContent.id" in units
    assert "Verb::open" in units
    assert "Verb::open.at" in units
    # The tag is the variant, never also a property of it (edge semantics 2).
    assert "Verb::open.kind" not in units


def test_binding_is_schema_guided_not_grep():
    """A string that merely SPELLS a variant name binds nothing."""
    schema = _schema({"verb": {"$ref": "#/$defs/Verb"}})
    e = Enumerator(schema)
    e.run()
    b = Binder(e)
    # `name` happens to hold the text "open"; only the tagged position counts.
    b.walk(schema["world"], _doc({"id": "x", "name": "open"}), "t")
    assert "Verb::open" not in b.bound
    b2 = Binder(e)
    b2.walk(schema["world"], _doc({"verb": {"kind": "open", "at": "a"}}), "t")
    assert "Verb::open" in b2.bound
    assert "Verb::open.at" in b2.bound


def test_a_new_unit_with_no_binding_is_unaccounted():
    """The red→green pair: the schema gains a unit, the document does not."""
    grown = _schema({"horizon": {"type": "string"}})
    e = Enumerator(grown)
    units = e.run()
    assert "WorldContent.horizon" in units

    b = Binder(e)
    b.walk(grown["world"], _doc({"id": "x", "name": "n"}), "t")
    unaccounted = set(units) - set(b.bound)
    assert "WorldContent.horizon" in unaccounted, "a new unit nothing writes must be a finding"

    b2 = Binder(e)
    b2.walk(grown["world"], _doc({"id": "x", "name": "n", "horizon": "ocean"}), "t")
    assert "WorldContent.horizon" not in set(units) - set(b2.bound), (
        "writing the field must discharge it — otherwise the gate is unfixable"
    )


def test_untagged_union_still_binds():
    """`CastEntry` is one placement OR a list of them, with no discriminator.

    The real regression: descending an untagged union as if it were a record
    found no `properties` and bound NOTHING, silently, on a value the campaign
    really wrote.
    """
    schema = _schema({"cast": {"$ref": "#/$defs/Entry"}})
    schema["world"]["$defs"]["Entry"] = {
        "anyOf": [
            {"$ref": "#/$defs/Placement"},
            {"type": "array", "items": {"$ref": "#/$defs/Placement"}},
        ]
    }
    schema["world"]["$defs"]["Placement"] = {
        "type": "object",
        "properties": {"at": {"type": "string"}, "doing": {"type": "string"}},
    }
    e = Enumerator(schema)
    e.run()
    b = Binder(e)
    b.walk(schema["world"], _doc({"cast": {"at": "a", "doing": "d"}}), "t")
    assert "Placement.at" in b.bound
    assert "Placement.doing" in b.bound


def test_untagged_record_union_picks_the_branch_that_fits():
    """`MobDrop` is `SlotDrop{slot}` or `ItemDrop{item, name}` — two records.

    The JSON type discriminates nothing here, so picking the first record branch
    bound `SlotDrop.slot` for every drop in the campaign and left the whole
    `ItemDrop` surface reading as unbound — on a gallery that writes both.
    Silent, and wrong in the direction that hides work.
    """
    schema = _schema({"drops": {"type": "array", "items": {"$ref": "#/$defs/Drop"}}})
    defs = schema["world"]["$defs"]
    defs["Drop"] = {"anyOf": [{"$ref": "#/$defs/SlotDrop"}, {"$ref": "#/$defs/ItemDrop"}]}
    defs["SlotDrop"] = {
        "type": "object",
        "properties": {"slot": {"type": "string"}},
        "required": ["slot"],
    }
    defs["ItemDrop"] = {
        "type": "object",
        "properties": {"item": {"type": "string"}, "name": {"type": "string"}},
        "required": ["item"],
    }
    e = Enumerator(schema)
    units = e.run()
    b = Binder(e)
    doc = _doc({"drops": [{"slot": "head"}, {"item": "minecraft:bone", "name": "A Bone"}]})
    b.walk(schema["world"], doc, "t")
    assert "SlotDrop.slot" in b.bound
    assert "ItemDrop.item" in b.bound, "the second record branch must be reachable"
    assert "ItemDrop.name" in b.bound
    assert not (set(b.bound) - set(units)), "every recorded id must be a real unit"


def test_option_wrapped_union_keeps_the_declaring_type_name():
    """`Option<Horizon>` names the variant after Horizon, never after the site.

    Losing the name here is the worst kind of silent failure: the walk reports a
    hit at a name no unit has, and the real unit reads as unbound. Both halves
    stay green and the gate proves nothing.
    """
    schema = _schema({"horizon": {"anyOf": [{"$ref": "#/$defs/Horizon"}, {"type": "null"}]}})
    schema["world"]["$defs"]["Horizon"] = {
        "oneOf": [
            {"const": "void", "type": "string"},
            {"const": "ocean", "type": "string"},
        ]
    }
    e = Enumerator(schema)
    units = e.run()
    assert "Horizon::ocean" in units
    b = Binder(e)
    b.walk(schema["world"], _doc({"horizon": "ocean"}), "t")
    assert "Horizon::ocean" in b.bound
    assert not (set(b.bound) - set(units)), "every recorded id must be a real unit"


def test_anonymous_subschema_members_belong_to_their_site():
    """`TrapEffect.dispense.item`, never `TrapEffect.item`."""
    schema = _schema()
    schema["world"]["$defs"]["TrapEffect"] = {
        "type": "object",
        "properties": {
            "dispense": {
                "type": "object",
                "properties": {"item": {"type": "string"}, "count": {"type": "integer"}},
            }
        },
    }
    schema["world"]["$defs"]["WorldContent"]["properties"]["effect"] = {
        "$ref": "#/$defs/TrapEffect"
    }
    e = Enumerator(schema)
    units = e.run()
    assert "TrapEffect.dispense.item" in units
    assert "TrapEffect.item" not in units
    b = Binder(e)
    b.walk(schema["world"], _doc({"effect": {"dispense": {"item": "i", "count": 1}}}), "t")
    assert "TrapEffect.dispense.item" in b.bound
    assert not (set(b.bound) - set(units))


def test_zero_units_is_refused_not_passed(tmp_path, monkeypatch, capsys):
    """A gate that enumerated nothing must fail loudly, not pass universally."""
    mod = _load_checker()
    monkeypatch.setattr(mod, "schema_export", lambda _d: {})
    monkeypatch.setattr(mod, "find_delvec", lambda _e: pathlib.Path("/nonexistent/delvec"))
    prefabs = tmp_path / "prefabs"
    prefabs.mkdir()
    monkeypatch.setattr(sys, "argv", ["check", "--prefabs", str(prefabs)])
    try:
        mod.main()
    except SystemExit as e:
        assert e.code == 1
    else:
        raise AssertionError("an empty unit set must not be a pass")
    assert "ZERO surface units" in capsys.readouterr().err


def test_overlay_must_declare_a_non_empty_binds_set():
    """A redundant overlay is what stops the overlay set growing by reflex."""
    gallery = REPO / "gallery" / "overlays"
    assert gallery.is_dir(), "the gallery must carry its overlays"
    found = 0
    for od in sorted(p for p in gallery.iterdir() if p.is_dir()):
        manifest = json.loads((od / "overlay.json").read_text())
        assert manifest.get("binds"), f"overlay `{od.name}` declares nothing"
        found += 1
    assert found > 0, "zero overlays examined — this assertion bound to nothing"


def test_every_probe_names_a_code_and_a_unit():
    """An exemption is only as good as the diagnostic it names."""
    probes = REPO / "gallery" / "probes"
    assert probes.is_dir(), "the gallery must carry its refusal probes"
    found = 0
    for pd in sorted(p for p in probes.iterdir() if p.is_dir()):
        manifest = json.loads((pd / "probe.json").read_text())
        assert manifest.get("code", "").startswith("DW"), f"probe `{pd.name}` names no DW code"
        assert manifest.get("units"), f"probe `{pd.name}` claims no unit"
        assert manifest.get("why"), (
            f"probe `{pd.name}` carries no reason — a probe is what a creator reads "
            "to learn what the engine checks"
        )
        found += 1
    assert found > 0, "zero probes examined — this assertion bound to nothing"
