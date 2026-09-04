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

import pytest

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
    monkeypatch.setattr(
        mod, "resolve_delvec", lambda *_a, **_k: pathlib.Path("/nonexistent/delvec")
    )
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


def test_every_committed_probe_satisfies_the_contract_the_checker_states():
    """The contract is not restated here — it is DRIVEN.

    This assertion used to be hand-written, and it drifted the moment the
    checker learned the demonstration kind: it went on demanding that every
    probe claim a unit, which the tool had stopped implementing, so the red it
    produced was about the test rather than about the gallery. Two hand-written
    authorities on one contract is what caused that, so there is now one —
    `probe_kind` / `probe_discharges` in the checker — and this walks the
    committed manifests through it.

    Scope, stated rather than assumed: the pytest job has no `delvec`, so the
    obligations that need one (*this document really is refused, with this
    code*, and *every claimed unit exists*) are the gallery job's, run against
    the real export. What is checkable here is that every committed manifest is
    well formed under the contract and classifies the way it reads.
    """
    probes = REPO / "gallery" / "probes"
    assert probes.is_dir(), "the gallery must carry its refusal probes"
    mod = _load_checker()

    entries = sorted(probes.iterdir())
    kinds: dict[str, str] = {}
    for pd in (p for p in entries if p.is_dir()):
        manifest = json.loads((pd / "probe.json").read_text())
        kind, code, claimed, why = mod.probe_kind(pd.name, manifest)
        kinds[pd.name] = kind
        assert (kind == mod.EXEMPTION) == bool(claimed), "the kind IS what is claimed"
        # A demonstration discharges nothing, so it must survive being offered an
        # EMPTY unit set: anything it tried to discharge would not be a unit and
        # the checker would refuse it by name.
        if kind == mod.DEMONSTRATION:
            assert mod.probe_discharges(pd.name, kind, code, claimed, why, {}, {}) == {}

    n = len(kinds)
    assert n > 0, "zero probes examined — this assertion bound to nothing"
    assert n == len(entries), (
        f"{n} probe(s) examined of {len(entries)} entries under `gallery/probes/` "
        "— a stray entry is a document the gate never runs"
    )


def test_the_probe_contract_refuses_every_way_of_breaking_it():
    """Each obligation, driven directly, in the direction that must red."""
    mod = _load_checker()
    ok = {"code": "DW0001", "units": ["A.b"], "why": "because"}

    for broken, what in (
        ({**ok, "code": None}, "no code at all"),
        ({**ok, "code": "0839"}, "a code that is not a DW code"),
        ({**ok, "why": ""}, "no reason a creator could read"),
    ):
        try:
            mod.probe_kind("p", broken)
        except SystemExit:
            continue
        raise AssertionError(f"a probe manifest with {what} must be refused")

    # A claim is held honest: the unit must exist, and must not already be bound.
    with pytest.raises(SystemExit):
        mod.probe_discharges("p", mod.EXEMPTION, "DW0001", ["A.b"], "w", {}, {})
    with pytest.raises(SystemExit):
        mod.probe_discharges("p", mod.EXEMPTION, "DW0001", ["A.b"], "w", {"A.b": 1}, {"A.b": []})

    # …and both kinds owe the refusal itself, with the code they name.
    for kind in (mod.EXEMPTION, mod.DEMONSTRATION):
        with pytest.raises(SystemExit):
            mod.assert_refused("p", kind, "DW0001", 0, [], "build")
        with pytest.raises(SystemExit):
            mod.assert_refused("p", kind, "DW0001", 1, ["DW0002"], "validate")
        mod.assert_refused("p", kind, "DW0001", 1, ["DW0002", "DW0001"], "validate")  # green

    # The green case discharges exactly what it claims.
    assert mod.probe_discharges(
        "p", mod.EXEMPTION, "DW0001", ["A.b"], "w", {"A.b": 1}, {}
    ) == {"A.b": {"probe": "p", "code": "DW0001", "why": "w"}}


def test_a_demonstration_discharges_nothing_and_the_verdict_says_so(tmp_path, monkeypatch):
    """The property a future change could silently break, asserted end to end.

    Not on the contract functions but on `main`'s own verdict, because that is
    where a discharge would have to show up. One unit is left unwritten and one
    probe is pointed at it, twice: claiming it (an exemption — discharged, exit
    0) and claiming nothing (a demonstration — still unaccounted, exit 1).

    The pair is what makes the author's free choice of kind safe. If a
    demonstration ever began discharging, the first half would go green and this
    test would fail — which is the whole of the hatch question: the weaker kind
    grants nothing, so picking it can only ever move a unit back into
    `unaccounted`, never out of it.
    """
    mod = _load_checker()
    # Deliberately smaller than `_schema()`: the only unit the gallery below
    # leaves unwritten is `horizon`, so `unaccounted` measures the claim alone.
    export = {
        "world": {
            "title": "Envelope_for_WorldContent",
            "type": "object",
            "properties": {"content": {"$ref": "#/$defs/WorldContent"}},
            "$defs": {
                "WorldContent": {
                    "type": "object",
                    "properties": {"id": {"type": "string"}, "horizon": {"type": "string"}},
                }
            },
        }
    }

    def verdict(claimed: list[str]) -> tuple[int, list[str]]:
        root = tmp_path / ("claims" if claimed else "silent")
        gallery = root / "gallery"
        (gallery / "probes" / "p").mkdir(parents=True)
        (gallery / "world.json").write_text(json.dumps(_doc({"id": "x"})))
        # A probe is the primary plus a declared edit, so this one declares one.
        # Perturbing `id` reaches no unit either way, which keeps `unaccounted`
        # measuring the claim and nothing else.
        (gallery / "probes" / "p" / "probe.json").write_text(
            json.dumps(
                {
                    "code": "DW9999",
                    "units": claimed,
                    "why": "w",
                    "patch": [
                        {"doc": "world.json", "op": "replace", "path": "/content/id", "value": "y"}
                    ],
                }
            )
        )
        prefabs = root / "prefabs"
        prefabs.mkdir()
        report = root / "report.json"
        monkeypatch.setattr(mod, "GALLERY", gallery)
        # …and the DOMAIN's source too, or the miniature materialises the real
        # gallery: `gallery_domain` reads its own `GALLERY`, and a fixture that
        # silently depends on the repository's own documents is a test about
        # something other than what it says.
        monkeypatch.setattr(mod.gallery_domain, "GALLERY", gallery)
        monkeypatch.setattr(mod, "schema_export", lambda _d: export)
        monkeypatch.setattr(
            mod,
            "resolve_delvec",
            lambda *_a, **_k: pathlib.Path("/nonexistent/delvec"),
        )
        monkeypatch.setattr(mod, "run_probe", lambda *_a: (1, ["DW9999"], "validate"))
        monkeypatch.setattr(
            sys, "argv", ["check", "--prefabs", str(prefabs), "--report", str(report)]
        )
        rc = mod.main()
        return rc, json.loads(report.read_text())["unaccounted"]

    rc, unaccounted = verdict([])
    assert (rc, unaccounted) == (1, ["WorldContent.horizon"]), (
        "a probe that claims nothing must discharge nothing — a demonstration "
        f"that let the gate pass would be the escape hatch: {unaccounted}"
    )

    rc, unaccounted = verdict(["WorldContent.horizon"])
    assert rc == 0 and unaccounted == [], (
        "…and the SAME probe claiming the unit does discharge it, so the "
        f"difference measured above is the claim and nothing else: {unaccounted}"
    )


def test_a_zero_compiler_binding_reds_rather_than_being_printed(tmp_path):
    """A verdict nothing acts on is not a gate.

    `read_build_ledgers` has always documented itself as *the caller reds on the
    ones the gallery writes*, and `--build-out`'s own help text says a zero
    binding is a red. Neither was true: the list was computed, printed in the
    summary line and written into the report, and `main` returned 0 regardless.
    That is worse than an absent check, because the printed count reads as a
    check that ran — a build whose watch ledger had stopped binding to anything
    would have said so on stdout and merged green.
    """
    mod = _load_checker()
    out = tmp_path / "build"
    (out / "validation").mkdir(parents=True)
    (out / "validation" / "bound.json").write_text(json.dumps({"examined": 4}))
    (out / "validation" / "stalled.json").write_text(json.dumps({"examined": 0}))

    ledgers, zeroes = mod.read_build_ledgers(out)
    assert len(ledgers) == 2, "both ledgers were read"
    assert zeroes == ["stalled.json: `examined` is 0"], (
        "the zero is named rather than counted, so the reader knows WHICH proof "
        f"stopped binding: {zeroes}"
    )

    # …and the gate is wired to that list. The source is read rather than
    # `main()` driven end to end, because reaching this branch needs a real
    # schema export, a real gallery walk and a real build tree — and a test that
    # needs all three to prove one `return 1` is a test nobody keeps green.
    src = (REPO / "tools" / "check-gallery-coverage.py").read_text()
    assert "if zero_bindings:" in src, "the list must gate, not merely print"
    body = src.split("if zero_bindings:", 1)[1].split("\n    return 0", 1)[0]
    assert "return 1" in body, "a zero binding must fail the run"


def test_the_patch_declaration_is_refused_every_way_of_being_unreadable():
    """Shape, driven directly. Whether an edit APPLIES is `gallery_domain`'s half.

    The two are separate on purpose and the reader is told which happened: a
    malformed `op` is somebody typing, a pointer the primary no longer holds is
    the gallery having moved under a probe that was right when it was written —
    and only the second one is the drift this mechanism exists to make loud.
    """
    mod = _load_checker()
    ok = {"code": "DW0001", "units": [], "why": "because"}
    good = {"doc": "world.json", "op": "replace", "path": "/content/difficulty", "value": "peaceful"}
    assert mod.probe_patch("p", {**ok, "patch": [good]}) == [good]
    assert mod.probe_patch("p", ok) == [], "a probe with no patch declares none"

    for broken, what in (
        ({**good, "op": "set"}, "a verb that is not one of the three"),
        ({**good, "op": None}, "no verb at all"),
        ({k: v for k, v in good.items() if k != "doc"}, "no document"),
        ({**good, "doc": ""}, "an empty document name"),
        ({**good, "path": "content/difficulty"}, "a path that is not a JSON pointer"),
        ({**good, "path": None}, "no path"),
        ({k: v for k, v in good.items() if k != "value"}, "a replace with no value"),
    ):
        try:
            mod.probe_patch("p", {**ok, "patch": [broken]})
        except SystemExit:
            continue
        raise AssertionError(f"a patch edit with {what} must be refused")

    for shape in ("not a list", {"doc": "x"}):
        with pytest.raises(SystemExit):
            mod.probe_patch("p", {**ok, "patch": shape if isinstance(shape, str) else [shape, 3]})


def test_a_probe_that_perturbs_nothing_is_refused_by_name(tmp_path, monkeypatch):
    """A probe that is the primary demonstrates nothing, and says so before delvec runs.

    Reachable by an ordinary migration going one step too far — drop the copies,
    forget the patch — and NOT caught by `assert_refused`, which asks only
    whether the document was refused with the named code. A no-op probe IS the
    primary, so the day the primary itself is refused every one of them passes
    that check for a reason none of them names, which is the vacuity in its
    purest form. The refusal fires ahead of the compiler for the same reason it
    is worth having: *this probe perturbs nothing* is a different sentence from
    *this probe was accepted*, and only the first says what to do.
    """
    mod = _load_checker()
    gallery = tmp_path / "gallery"
    (gallery / "probes" / "p").mkdir(parents=True)
    (gallery / "world.json").write_text(json.dumps(_doc({"id": "x"})))
    (gallery / "probes" / "p" / "probe.json").write_text(
        json.dumps({"code": "DW9999", "units": [], "why": "w"})
    )
    prefabs = tmp_path / "prefabs"
    prefabs.mkdir()
    monkeypatch.setattr(mod, "GALLERY", gallery)
    monkeypatch.setattr(mod.gallery_domain, "GALLERY", gallery)
    monkeypatch.setattr(mod, "schema_export", lambda _d: _schema())
    monkeypatch.setattr(
        mod, "resolve_delvec", lambda *_a, **_k: pathlib.Path("/nonexistent/delvec")
    )
    monkeypatch.setattr(mod, "run_probe", lambda *_a: (1, ["DW9999"], "validate"))
    monkeypatch.setattr(sys, "argv", ["check", "--prefabs", str(prefabs)])
    with pytest.raises(SystemExit):
        mod.main()

    # …and the SAME probe with one declared edit gets past this refusal, so what
    # was measured is the perturbation and not some other objection.
    (gallery / "probes" / "p" / "probe.json").write_text(
        json.dumps(
            {
                "code": "DW9999",
                "units": [],
                "why": "w",
                "patch": [{"doc": "world.json", "op": "replace", "path": "/content/id", "value": "y"}],
            }
        )
    )
    mod.main()


def test_the_gate_names_the_probe_in_a_materialisation_refusal():
    """`gallery_domain` raises; a refusal that does not say WHICH probe is a search."""
    mod = _load_checker()
    with pytest.raises(SystemExit):
        mod.materialise_point("probe", REPO / "gallery" / "probes" / "peaceful-difficulty", REPO)


def test_the_binding_line_states_the_patch_figures():
    """A count nobody prints is a count nobody can contradict (CLAUDE.md)."""
    src = (REPO / "tools" / "check-gallery-coverage.py").read_text()
    for phrase in ("probe patches:", "probe(s) examined", "JSON path(s) touched"):
        assert phrase in src, f"the binding line no longer states `{phrase}`"
