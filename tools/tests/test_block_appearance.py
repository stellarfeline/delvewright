r"""spec-0035: the screened shelf, its classification half, and the mix report.

**What binds where, stated so nobody has to guess.** This tool's happy path needs
the EULA-gated client jar, which CI does not have and must never have. So the
suite is split, and every jar-gated test is collected and reported rather than
quietly absent:

  * **CI-bound, no jar** — the vendored family/form table (spec-0035 AC2), the
    mix arithmetic over a committed fixture of measured numbers (AC4/AC5), the
    refusal paths, the screen's expression language, the swatch sheet's PNG
    encoder and seeded tiling over synthetic pixels (AC6's determinism), and the
    agreement between this tool's gravity set and the compiler's own.
  * **jar-gated** — the exact measured values for named blocks (AC1), the
    1146 → 409 → 58 → 16 → 14 cascade over the real shelf (AC3), and the real
    sheet (AC6's content). `test_jar_gated_inventory_is_named` fails if that list
    is silently emptied, because a skipped test that nobody can name is an UNRUN
    gate wearing a green tick.

**A finding recorded here rather than in a commit message**, because the fixture
is the place it can keep failing: spec-0035 §4.2 states the cascade as
1146 → 409 → **57** → 16 → 14. Measured against the pinned 1.21.11 jar under the
definitions that reproduce every other number in the spec (§3.3's five rows to
the digit, §3.2's five texture ranges, the chroma deciles, and all fourteen
survivor ids), step 2 yields **58**. The extra block is
`minecraft:cherry_leaves`, which is biome-tinted: excluding tinted blocks gives
57 at step 2 but also gives 1124 and 397 at steps 0 and 1, contradicting the
spec's own 1146 and 409. The spec's cascade mixes a tinted-included head with a
tinted-excluded step 2. Both cascades are asserted below, each self-consistent;
neither number was re-baselined away.
"""

from __future__ import annotations

import importlib.util
import json
import math
import os
import re
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
TOOL = REPO / "tools" / "block-appearance.py"
EXTRACTOR = REPO / "tools" / "extract-block-classification.py"
CLASSIFICATION = REPO / "crates" / "compiler" / "data" / "block-classification-1.21.11.json"
ASSEMBLED = REPO / "crates" / "compiler" / "src" / "assembled.rs"


def load(path: Path):
    spec = importlib.util.spec_from_file_location(path.stem.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ba = load(TOOL)


def client_jar() -> Path | None:
    for candidate in (
        os.environ.get("DELVEWRIGHT_CLIENT_JAR"),
        str(Path.home() / ".chunky" / "resources" / "minecraft.jar"),
    ):
        if candidate and Path(candidate).exists():
            return Path(candidate)
    return None


JAR = client_jar()
needs_jar = pytest.mark.skipif(
    JAR is None,
    reason="EULA-gated 1.21.11 client jar absent (versions.toml [render]); this "
    "assertion is jar-gated and does NOT run in CI",
)

# Every jar-gated test in this file, named. A test removed from the suite without
# leaving this list is caught by `test_jar_gated_inventory_is_named`.
JAR_GATED = {
    "test_measured_values_match_committed_expectations",
    "test_fixture_rows_match_the_jar",
    "test_the_single_block_that_reconciles_the_two_cascades",
    "test_screen_reproduces_the_spec_cascade",
    "test_screen_cascade_excluding_tinted",
    "test_swatch_sheet_is_byte_identical_at_one_seed",
    "test_id_near_list_still_work",
}


# --------------------------------------------------------------------------
# AC2 — the family/form derivation, no jar
# --------------------------------------------------------------------------


@pytest.fixture(scope="module")
def table():
    assert CLASSIFICATION.exists(), (
        f"{CLASSIFICATION} is missing — regenerate with tools/extract-block-classification.py"
    )
    return json.loads(CLASSIFICATION.read_text())


def family_of(table, block: str) -> str:
    return table["blocks"][block]["family"]


def test_family_groups_a_material_and_separates_two(table):
    """spec-0035 AC2's three named memberships."""
    assert family_of(table, "minecraft:smooth_sandstone") == family_of(
        table, "minecraft:sandstone"
    )
    assert family_of(table, "minecraft:cracked_stone_bricks") == family_of(
        table, "minecraft:stone_bricks"
    )
    assert family_of(table, "minecraft:calcite") != family_of(table, "minecraft:sandstone")


def test_no_family_runs_away(table):
    """AC2's runaway-merge guard. The measured largest is deepslate at 20."""
    sizes: dict[str, int] = {}
    for row in table["blocks"].values():
        sizes[row["family"]] = sizes.get(row["family"], 0) + 1
    largest = max(sizes.items(), key=lambda kv: (kv[1], kv[0]))
    assert largest[1] <= 45, f"{largest[0]} has {largest[1]} members"
    assert largest[1] == 20, "the measured largest family is deepslate's 20"
    assert table["stats"]["largest_family"] == 20


def test_spec_family_probes(table):
    """§3.1's five probes, which are what pinned the derivation rule.

    `diorite` is the one that decides the rule: `granite` is diorite + quartz and
    `andesite` is diorite + cobblestone, so counting "one BLOCK-valued ingredient
    among any others" welds the whole stone group into a 41-member component and
    diorite's family reads 41 instead of 7.
    """
    sizes: dict[str, int] = {}
    for row in table["blocks"].values():
        sizes[row["family"]] = sizes.get(row["family"], 0) + 1
    expected = {
        "minecraft:sandstone": 11,
        "minecraft:diorite": 7,
        "minecraft:deepslate": 20,
        "minecraft:calcite": 1,
        "minecraft:dried_kelp_block": 1,
    }
    got = {block: sizes[family_of(table, block)] for block in expected}
    assert got == expected


def test_forms_come_from_vanilla_tags_not_from_names(table):
    blocks = table["blocks"]
    assert blocks["minecraft:oak_slab"]["form"] == "slab"
    assert blocks["minecraft:stone_brick_stairs"]["form"] == "stair"
    assert blocks["minecraft:cobblestone_wall"]["form"] == "wall"
    assert blocks["minecraft:oak_fence"]["form"] == "fence"
    assert blocks["minecraft:iron_door"]["form"] == "door"
    assert blocks["minecraft:oak_trapdoor"]["form"] == "trapdoor"
    assert blocks["minecraft:stone_button"]["form"] == "button"
    assert blocks["minecraft:stone_pressure_plate"]["form"] == "pressure_plate"
    assert blocks["minecraft:oak_sign"]["form"] == "sign"
    assert blocks["minecraft:sandstone"]["form"] == "block"
    # `nether_brick_fence` is a fence with no wood in its name and
    # `petrified_oak_slab` is a slab with no stone in its name: a suffix rule gets
    # both right by luck and `end_stone` wrong on purpose, which is why the form
    # axis reads tags.
    assert blocks["minecraft:nether_brick_fence"]["form"] == "fence"
    assert blocks["minecraft:petrified_oak_slab"]["form"] == "slab"


def test_pane_form_is_the_connection_model_and_reaches_bars(table):
    """Vanilla has no `#panes` tag, so the pane form is read off the blockstate.

    It must catch iron and copper BARS too — they share the model exactly — and
    must not catch anything else.
    """
    panes = {b for b, row in table["blocks"].items() if row["form"] == "pane"}
    assert "minecraft:glass_pane" in panes
    assert "minecraft:iron_bars" in panes
    assert "minecraft:copper_bars" in panes
    assert "minecraft:white_stained_glass_pane" in panes
    assert len(panes) == 26, sorted(panes)
    assert "minecraft:glass" not in panes
    assert "minecraft:oak_fence" not in panes


def test_every_block_is_classified(table):
    registry = json.loads((REPO / "crates" / "compiler" / "data" / "blocks-1.21.11.json").read_text())
    assert set(table["blocks"]) == set(registry)
    assert table["stats"]["blocks"] == 1166


def test_family_derivation_is_deterministic(tmp_path, table):
    """AC2's byte-identity. Re-derives from the committed table's own inputs is
    impossible without the mcmeta files, so this re-serialises instead: the union
    root is chosen by lexicographic order, so the mapping cannot depend on edge
    order, and re-emitting must reproduce the file byte for byte."""
    again = json.dumps(table, indent=2, sort_keys=True) + "\n"
    assert again == CLASSIFICATION.read_text()


# --------------------------------------------------------------------------
# AC4 / AC5 — the mix report, over a committed fixture of measured numbers
#
# The five rows below are spec-0035 §3.3's own table plus `andesite`, measured
# from the pinned jar. Committing them here (and NOT as a shipped data file) is
# what lets the arithmetic that carries the whole spec run in CI with no jar.
# `test_fixture_rows_match_the_jar` re-measures them when a jar is present, so
# the fixture cannot quietly go stale.
# --------------------------------------------------------------------------

FIXTURE = {
    "minecraft:sandstone": dict(
        rgb=[219, 207, 160], L=0.8513, C_mean=0.0629, C_p90=0.0688, hue=95.0
    ),
    "minecraft:smooth_sandstone": dict(
        rgb=[224, 214, 170], L=0.8735, C_mean=0.0588, C_p90=0.0605, hue=97.1
    ),
    "minecraft:calcite": dict(
        rgb=[223, 224, 221], L=0.9058, C_mean=0.0058, C_p90=0.0081, hue=118.3
    ),
    "minecraft:polished_diorite": dict(
        rgb=[193, 193, 195], L=0.8096, C_mean=0.0045, C_p90=0.0071, hue=281.4
    ),
    "minecraft:andesite": dict(
        rgb=[136, 136, 137], L=0.6271, C_mean=0.0033, C_p90=0.0059, hue=276.6
    ),
}

# §3.3's pair: A is 60% sandstone-family over grey stone; B swaps half of that
# for calcite and polished diorite. The spec publishes the four resulting numbers
# but not the member list, so this is the composition that reproduces all of them
# — mean #bbb59a vs the spec's #bcb69a, #b7b4a7 vs #b8b5a7, chroma mass
# 0.0378/0.0211 vs 0.0373/0.0205, ratio 1.79 vs 1.82.
MIX_A = "sandstone=30,smooth_sandstone=30,andesite=40"
MIX_B = "sandstone=15,smooth_sandstone=15,calcite=15,polished_diorite=15,andesite=40"


@pytest.fixture()
def by_id():
    rows = {}
    for block, values in FIXTURE.items():
        row = dict(values)
        row["id"] = block
        rows[block] = row
    return rows


def report(spec: str, by_id: dict, name: str = "mix") -> dict:
    return ba.mix_report(name, ba.parse_mix(spec), by_id)


def test_mix_report_emits_the_four_numbers(by_id):
    """AC4: the report's shape."""
    out = report(MIX_A, by_id)
    assert set(out) >= {
        "chroma_mass",
        "chromatic_area",
        "loudest_member",
        "dominant_hue",
    }
    assert out["loudest_member"]["id"] == "minecraft:sandstone"
    assert out["loudest_member"]["area_share"] == pytest.approx(0.30)
    assert out["dominant_hue"] == pytest.approx(96.0, abs=0.5)


def test_the_mean_does_not_separate_the_two_mixes(by_id):
    """AC4's teeth. If this ever passes for the wrong reason — because the two
    means DID separate — the fixture is no longer demonstrating the defect."""
    a, b = report(MIX_A, by_id, "A"), report(MIX_B, by_id, "B")
    distance = math.dist(a["mean_rgb_not_a_verdict"], b["mean_rgb_not_a_verdict"])
    assert distance < 15, distance
    assert distance == pytest.approx(13.5, abs=0.5)


def test_chromatic_area_does_separate_them(by_id):
    """AC4's fixture: 0.60 against 0.30, where the mean read 13.5 units."""
    a, b = report(MIX_A, by_id, "A"), report(MIX_B, by_id, "B")
    assert a["chromatic_area"] == pytest.approx(0.60)
    assert b["chromatic_area"] == pytest.approx(0.30)
    assert a["chroma_mass"] / b["chroma_mass"] == pytest.approx(1.79, abs=0.02)
    assert a["loudest_member"]["area_share"] == pytest.approx(0.30)
    assert b["loudest_member"]["area_share"] == pytest.approx(0.15)


def test_the_chromatic_threshold_is_the_derived_one(by_id):
    """AC's §7.3: the 0.03 threshold is the shelf's own 30th percentile. It is
    re-derived from the distribution if it mis-binds — never loosened so that a
    mix passes, which is what a test pinning it prevents."""
    assert ba.CHROMATIC_THRESHOLD == 0.03
    assert report(MIX_A, by_id)["chromatic_threshold"] == 0.03


def test_a_mean_is_never_the_verdict(by_id):
    """AC4: the mean may be printed, never alone and never as the answer."""
    out = report(MIX_A, by_id)
    assert "mean_rgb" not in out
    assert "mean_rgb_not_a_verdict" in out


def test_air_is_a_member_and_is_reported_as_void_not_dropped(by_id):
    """`minecraft:air` is a paint member like any other — it is the whole of decay
    in the grammar, and the first REAL program this report was pointed at
    (`idiom-erosion-graded`) is 45% air in one role.

    Dropping it would renormalise the survivors and report a solid wall's
    numbers for a wall that is nearly half holes; counting it as a colour would
    report the holes as a dark grey. It has area and no colour, so it dilutes
    chroma and is named separately.
    """
    out = report("sandstone=1,air=1", by_id, "half-gone")
    assert out["void_area"] == pytest.approx(0.5)
    assert [m["id"] for m in out["members"]] == ["minecraft:sandstone"]
    # Chroma is diluted by the void: half the area is not coloured.
    assert out["chroma_mass"] == pytest.approx(FIXTURE["minecraft:sandstone"]["C_mean"] / 2)
    assert out["chromatic_area"] == pytest.approx(0.5)
    # The mean is of the SOLID share, so it still reads as sandstone.
    assert out["mean_rgb_not_a_verdict"] == FIXTURE["minecraft:sandstone"]["rgb"]
    assert out["loudest_member"]["id"] == "minecraft:sandstone"
    # And it BINDS as a two-member mix. Counting only the measured members would
    # report every eroded role in the corpus as a solid, which is the zero-binding
    # lie one level down.
    assert out["member_count"] == 2


def test_a_paint_that_is_entirely_air_still_reports(by_id):
    out = report("air", by_id, "nothing")
    assert out["void_area"] == pytest.approx(1.0)
    assert out["loudest_member"] is None
    assert out["chromatic_area"] == 0
    assert out["member_count"] == 1


def test_report_refuses_a_member_it_cannot_measure(by_id):
    with pytest.raises(SystemExit) as excinfo:
        ba.mix_report("m", ba.parse_mix("sandstone=1,gold_block=1"), by_id)
    assert "refusing" in str(excinfo.value)


def test_zero_binding_is_reported_as_a_finding(by_id, capsys):
    """AC5. A single-member paint is not a mix, and a report over none of them
    must say so rather than print a clean page."""
    out = report("sandstone", by_id, "solid")
    ba.print_mix_report([out], 1)
    printed = capsys.readouterr().out
    assert "binding: 1 paint(s) examined, 0 mix(es) with >= 2 members" in printed
    assert "FINDING: zero binding" in printed
    # ...and a stone+air paint is NOT zero binding.
    ba.print_mix_report([report("sandstone=1,air=1", by_id, "eroded")], 1)
    assert "1 mix(es) with >= 2 members" in capsys.readouterr().out


def test_binding_count_is_stated_on_every_artifact(by_id, capsys):
    """AC5: on EVERY artifact it writes — the human table and the JSON both."""
    ba.print_mix_report([report(MIX_A, by_id, "A"), report(MIX_B, by_id, "B")], 2)
    assert "binding: 2 paint(s) examined, 2 mix(es) with >= 2 members" in capsys.readouterr().out


def test_program_mixes_reach_inline_fills_not_only_named_roles():
    """A paint is a role OR an inline `fill` material. Reading only `palette`
    leaves every inline mix unmeasured — the enumerate-three-of-five shape."""
    program = {
        "palette": {
            "wall": "minecraft:stone_bricks",
            "ruin": [
                {"weight": 9, "block": "minecraft:stone_bricks"},
                {"weight": 3, "block": "minecraft:mossy_stone_bricks"},
            ],
        },
        "rules": {
            "room": [
                {
                    "weight": 1,
                    "body": {
                        "op": "split",
                        "children": [
                            {
                                "op": "fill",
                                "material": [
                                    {"weight": 1, "block": "minecraft:sandstone"},
                                    {"weight": 1, "block": "minecraft:andesite"},
                                ],
                            }
                        ],
                    },
                }
            ]
        },
    }
    found = dict(ba.program_mixes(program))
    assert "palette.wall" in found and "palette.ruin" in found
    inline = [name for name in found if name.startswith("fill@")]
    assert inline, f"no inline fill found in {sorted(found)}"
    assert sorted(b for b, _ in found[inline[0]]) == [
        "minecraft:andesite",
        "minecraft:sandstone",
    ]


def test_program_mixes_read_a_paint_written_in_the_scopes_own_frame():
    """A `{"local": ...}` paint is the same states in the scope's axis frame. The
    frame decides which world direction a property names; it moves no block and
    changes no colour, so it is measured exactly like a bare paint.

    Skipping the wrapper is not a gap in one role's numbers — a program whose
    whole palette is local binds to ZERO paints, and a zero binding that nothing
    prints is the shelf listing arriving in place of a measurement.
    """
    program = {
        "palette": {
            "grille": {"local": "minecraft:stone_bricks"},
            "crag": {
                "local": [
                    {"weight": 3, "block": "minecraft:cobbled_deepslate"},
                    {"weight": 1, "block": "minecraft:blackstone"},
                ]
            },
        },
        "rules": {
            "wall": [
                {
                    "weight": 1,
                    "body": {
                        "op": "fill",
                        "material": {
                            "local": [
                                {"weight": 1, "block": "minecraft:sandstone"},
                                {"weight": 1, "block": "minecraft:andesite"},
                            ]
                        },
                    },
                }
            ]
        },
    }
    found = dict(ba.program_mixes(program))
    assert found["palette.grille"] == [("minecraft:stone_bricks", 1.0)]
    assert sorted(b for b, _ in found["palette.crag"]) == [
        "minecraft:blackstone",
        "minecraft:cobbled_deepslate",
    ]
    inline = [name for name in found if name.startswith("fill@")]
    assert inline, f"a local inline fill went unread: {sorted(found)}"
    assert sorted(b for b, _ in found[inline[0]]) == [
        "minecraft:andesite",
        "minecraft:sandstone",
    ]
    # A `{"role": ...}` material is a REFERENCE to a named paint, already reported
    # from `palette`, and must not be read a second time as an inline one.
    role_ref = {"rules": {"r": [{"body": {"op": "fill", "material": {"role": "crag"}}}]}}
    assert ba.program_mixes(role_ref) == []


def test_a_program_that_binds_to_nothing_prints_the_finding_not_the_shelf(tmp_path):
    """The zero-binding FINDING was written, correct, and unreachable: the report
    was gated on there being something to say, so a program this reader did not
    understand fell through to the whole-shelf listing and exited 0.

    A mix report is owed by the REQUEST, so `--program` over a paintless program
    states its binding of zero.
    """
    program = tmp_path / "empty.json"
    program.write_text(json.dumps({"version": "1.4.0", "palette": {}, "rules": {}}))
    proc = subprocess.run(
        [sys.executable, str(TOOL), "--program", str(program)],
        capture_output=True,
        text=True,
    )
    assert "binding: 0 paint(s) examined, 0 mix(es) with >= 2 members" in proc.stdout
    assert "FINDING: zero binding" in proc.stdout
    # The tell of the old behaviour: the full shelf arriving instead of a report.
    assert "minecraft:acacia_button" not in proc.stdout


def test_block_state_strings_reduce_to_their_block():
    assert ba.base_block("minecraft:oak_stairs[facing=east,half=top]") == "minecraft:oak_stairs"
    assert ba.base_block("stone") == "minecraft:stone"


# --------------------------------------------------------------------------
# The screen's expression language — no jar
# --------------------------------------------------------------------------


def rows_for_screen():
    return [
        {"id": "a", "full_cube": True, "L": 0.80, "C_mean": 0.01, "form": "block", "tinted": False},
        {"id": "b", "full_cube": True, "L": 0.90, "C_mean": 0.09, "form": "block", "tinted": True},
        {"id": "c", "full_cube": False, "L": 0.80, "C_mean": 0.01, "form": "slab", "tinted": False},
    ]


def test_constraints_eliminate_in_the_order_given():
    survivors, cascade = ba.run_screen(
        rows_for_screen(), ["full_cube", "L>=0.85", "C_mean<0.02"]
    )
    assert [n for _, n in cascade] == [3, 2, 1, 0]
    assert survivors == []


def test_boolean_negation_and_text_facets():
    survivors, _ = ba.run_screen(rows_for_screen(), ["not tinted", "form=slab"])
    assert [r["id"] for r in survivors] == ["c"]


def test_an_unknown_facet_is_refused_not_ignored():
    """A silently-ignored constraint is a screen that binds to less than it says."""
    with pytest.raises(SystemExit) as excinfo:
        ba.parse_where("shininess>0.5")
    assert "unknown field" in str(excinfo.value)


def test_a_malformed_constraint_is_refused():
    with pytest.raises(SystemExit):
        ba.parse_where("L>=lots")
    with pytest.raises(SystemExit):
        ba.parse_where("form<slab")


# --------------------------------------------------------------------------
# Refusals — no jar
# --------------------------------------------------------------------------


def run_tool(*args, env=None):
    environment = dict(os.environ)
    environment.pop("DELVEWRIGHT_CLIENT_JAR", None)
    environment["HOME"] = "/nonexistent-home-for-this-test"
    if env:
        environment.update(env)
    return subprocess.run(
        [sys.executable, str(TOOL), *args],
        capture_output=True,
        text=True,
        env=environment,
        cwd=REPO,
    )


def test_without_a_jar_the_tool_refuses_rather_than_answering():
    """A palette answer given without the textures is the recollection this tool
    exists to remove, so a missing jar must be a refusal and not an empty table."""
    done = run_tool("--id", "sandstone")
    assert done.returncode != 0
    assert "refusing" in (done.stderr + done.stdout)


def test_the_sheet_refuses_a_path_inside_the_repo():
    """AC6: a sheet carries Mojang texture pixels, so it may only land in the
    gitignored working dir — checked BEFORE any measurement, so the refusal is
    not buried under a page of output."""
    done = run_tool("--screen", "--where", "full_cube", "--sheet", "docs/palette.png")
    assert done.returncode == 2
    assert "refusing to write docs/palette.png" in done.stderr
    assert ".sheets/palette" in done.stderr


def test_sheet_path_predicate():
    assert ba.sheet_path_ok(REPO / ".sheets" / "palette" / "x.png")
    assert not ba.sheet_path_ok(REPO / "docs" / "x.png")
    assert not ba.sheet_path_ok(REPO / "x.png")
    assert ba.sheet_path_ok(Path("/tmp/x.png"))


def test_missing_classification_is_a_refusal(monkeypatch, tmp_path):
    monkeypatch.setattr(ba, "CLASSIFICATION", tmp_path / "absent.json")
    with pytest.raises(SystemExit) as excinfo:
        ba.load_classification()
    assert "refusing" in str(excinfo.value)


def test_the_extractor_refuses_an_unpinned_source(tmp_path):
    """The classification table is derived from a pinned mcmeta summary. A file
    with a different checksum is a different game version, and deriving from it
    silently is how a 1.20 family table ships against a 1.21.11 registry."""
    tags = tmp_path / "tags.json"
    tags.write_text("{}")
    recipes = tmp_path / "recipes.json"
    recipes.write_text("{}")
    done = subprocess.run(
        [sys.executable, str(EXTRACTOR), str(tags), str(recipes), str(tmp_path / "out.json")],
        capture_output=True,
        text=True,
    )
    assert done.returncode == 2
    assert "sha256" in done.stderr


# --------------------------------------------------------------------------
# The gravity set does not fork — no jar
# --------------------------------------------------------------------------


def test_gravity_set_agrees_with_the_compilers_own():
    """spec-0035 §3.1: reuse `DW0313`'s set, do NOT re-derive it.

    "Reuse" across a Rust/Python boundary cannot be a call, so it is a binding: this
    reads `is_falling_block`'s own source and fails if the two ever disagree. Without
    it the palette layer owns a second, private gravity model that drifts silently —
    the shape this repo names as a general mechanism privately re-implemented.
    """
    source = ASSEMBLED.read_text()
    start = source.index("pub fn is_falling_block")
    body = source[start : source.index("\n}", start)]
    matches = re.search(r"matches!\(\s*id,(.*?)\)", body, re.S)
    assert matches, "is_falling_block no longer uses matches!(id, ...) — re-read it"
    exact = {f"minecraft:{name}" for name in re.findall(r'"([a-z_]+)"', matches.group(1))}
    suffixes = tuple(re.findall(r'ends_with\("([a-z_]+)"\)', body))
    assert exact == ba.GRAVITY_EXACT, f"rust={sorted(exact)} python={sorted(ba.GRAVITY_EXACT)}"
    assert suffixes == ba.GRAVITY_SUFFIXES
    assert ba.is_gravity("minecraft:white_concrete_powder")
    assert ba.is_gravity("minecraft:sand")
    assert not ba.is_gravity("minecraft:suspicious_sand")


# --------------------------------------------------------------------------
# The swatch sheet's machinery, over synthetic pixels — no jar
# --------------------------------------------------------------------------


def test_png_encoder_round_trips_through_the_tools_own_decoder():
    canvas = ba.Canvas(7, 5, (10, 20, 30))
    canvas.set(3, 2, (200, 100, 50))
    width, height, pixels = ba.decode_png(canvas.to_png())
    assert (width, height) == (7, 5)
    assert pixels[2 * 7 + 3][:3] == (200, 100, 50)
    assert pixels[0][:3] == (10, 20, 30)


def test_png_encoding_is_byte_stable():
    first = ba.Canvas(9, 9, (1, 2, 3)).to_png()
    second = ba.Canvas(9, 9, (1, 2, 3)).to_png()
    assert first == second


def test_the_seeded_stream_is_a_function_of_the_seed_alone():
    a = [ba.Splitmix(7).next() for _ in range(4)]
    b = [ba.Splitmix(7).next() for _ in range(4)]
    c = [ba.Splitmix(8).next() for _ in range(4)]
    assert a == b
    assert a != c


def test_oklab_matches_the_published_transform():
    """Pure white is L=1 with no chroma; pure black is L=0. If the matrices are
    ever mistyped these two move, and every downstream number moves with them."""
    l_white, a_white, b_white = ba.oklab(255, 255, 255)
    assert l_white == pytest.approx(1.0, abs=1e-4)
    assert math.hypot(a_white, b_white) == pytest.approx(0.0, abs=1e-4)
    assert ba.oklab(0, 0, 0)[0] == pytest.approx(0.0, abs=1e-9)
    # Sanity on the axis that carries this spec: a saturated blue is far more
    # chromatic than mid grey.
    _, a_blue, b_blue = ba.oklab(0, 0, 255)
    _, a_grey, b_grey = ba.oklab(128, 128, 128)
    assert math.hypot(a_blue, b_blue) > 0.3 > math.hypot(a_grey, b_grey)


def test_percentile_interpolates():
    values = [0.0, 1.0, 2.0, 3.0, 4.0]
    assert ba.percentile(values, 0.0) == 0.0
    assert ba.percentile(values, 1.0) == 4.0
    assert ba.percentile(values, 0.5) == 2.0
    assert ba.percentile(values, 0.05) == pytest.approx(0.2)


# --------------------------------------------------------------------------
# jar-gated
# --------------------------------------------------------------------------


@pytest.fixture(scope="module")
def shelf():
    if JAR is None:
        pytest.skip("no client jar")
    jar = ba.Jar(JAR)
    classification = ba.load_classification()
    registry = json.loads((REPO / "crates" / "compiler" / "data" / "blocks-1.21.11.json").read_text())
    rows = []
    for block in sorted(registry):
        if block in ba.TECHNICAL:
            continue
        row = ba.appearance(jar, block, classification)
        if row is not None:
            rows.append(row)
    return rows


@needs_jar
def test_measured_values_match_committed_expectations(shelf):
    """AC1: the exact values for the named blocks."""
    by_id = {row["id"]: row for row in shelf}
    for block, want in FIXTURE.items():
        got = by_id[block]
        assert got["rgb"] == want["rgb"], block
        assert got["L"] == pytest.approx(want["L"], abs=5e-4), block
        assert got["C_mean"] == pytest.approx(want["C_mean"], abs=5e-4), block
        assert got["C_p90"] == pytest.approx(want["C_p90"], abs=5e-4), block
        assert got["hue"] == pytest.approx(want["hue"], abs=0.2), block
        for field in ("L_p05", "L_p95", "L_sd", "texture_range", "C_max", "family", "form"):
            assert field in got, (block, field)
    # §3.2's texture ranges, the axis a mean cannot see.
    assert by_id["minecraft:white_concrete"]["texture_range"] == pytest.approx(0.006, abs=5e-4)
    assert by_id["minecraft:stone_bricks"]["texture_range"] == pytest.approx(0.221, abs=5e-4)
    assert by_id["minecraft:dried_kelp_block"]["texture_range"] == pytest.approx(0.419, abs=5e-4)
    # AC1's other two named blocks.
    assert by_id["minecraft:packed_mud"]["rgb"] == [142, 107, 80]
    assert by_id["minecraft:packed_mud"]["family"] != by_id["minecraft:sandstone"]["family"]
    assert by_id["minecraft:stone_bricks"]["L"] == pytest.approx(0.5788, abs=5e-4)


@needs_jar
def test_fixture_rows_match_the_jar(shelf):
    """The CI-bound mix fixture is only honest if it still equals the measurement."""
    by_id = {row["id"]: row for row in shelf}
    for block, want in FIXTURE.items():
        assert by_id[block]["C_mean"] == pytest.approx(want["C_mean"], abs=5e-4), block


@needs_jar
def test_screen_reproduces_the_spec_cascade(shelf):
    """AC3, with the module docstring's step-2 finding asserted rather than hidden."""
    survivors, cascade = ba.run_screen(
        shelf,
        ["full_cube", "L>=0.75", "L<=0.95", "C_mean<0.02", "texture_range<=0.30"],
    )
    counts = [n for _, n in cascade]
    assert counts[0] == 1146
    assert counts[1] == 409
    assert counts[3] == 58, "spec-0035 §4.2 says 57; see this module's docstring"
    assert counts[4] == 16
    assert counts[5] == 14
    assert sorted(row["id"] for row in survivors) == sorted(
        f"minecraft:{name}"
        for name in (
            "calcite",
            "quartz_block",
            "smooth_quartz",
            "quartz_bricks",
            "quartz_pillar",
            "chiseled_quartz_block",
            "diorite",
            "white_concrete",
            "white_concrete_powder",
            "iron_block",
            "pale_oak_planks",
            "stripped_pale_oak_log",
            "pearlescent_froglight",
            "white_wool",
        )
    )
    # §4.2's own reading of the residue: the screen excludes sandstone outright —
    # which is what would have prevented the motivating defect — and still returns
    # a light source, a gravity block, wool and a metal.
    ids = {row["id"] for row in survivors}
    assert "minecraft:sandstone" not in ids
    assert "minecraft:pearlescent_froglight" in ids
    assert "minecraft:white_concrete_powder" in ids
    assert any(row["gravity"] for row in survivors)


@needs_jar
def test_screen_cascade_excluding_tinted(shelf):
    """The other self-consistent reading of §4.2: drop biome-tinted blocks and
    step 2 is the spec's 57 — but the head is then 1124 and 397, not 1146/409."""
    untinted = [row for row in shelf if not row["tinted"]]
    survivors, cascade = ba.run_screen(
        untinted,
        ["full_cube", "L>=0.75", "L<=0.95", "C_mean<0.02", "texture_range<=0.30"],
    )
    counts = [n for _, n in cascade]
    assert counts[0] == 1124
    assert counts[1] == 397
    assert counts[3] == 57
    assert counts[4] == 16
    assert len(survivors) == 14


@needs_jar
def test_the_single_block_that_reconciles_the_two_cascades(shelf):
    """Named, so the discrepancy can never be re-discovered as a mystery."""
    survivors, _ = ba.run_screen(shelf, ["full_cube", "L>=0.75", "L<=0.95"])
    tinted = [row["id"] for row in survivors if row["tinted"]]
    assert tinted == ["minecraft:cherry_leaves"]


@needs_jar
def test_swatch_sheet_is_byte_identical_at_one_seed(shelf, tmp_path):
    """AC6: no GPU, no Chunky, no world, and the same bytes twice at one seed."""
    jar = ba.Jar(JAR)
    shortlist, _ = ba.run_screen(
        shelf, ["full_cube", "L>=0.75", "L<=0.95", "C_mean<0.02", "texture_range<=0.30"]
    )
    mixes = [("A", ba.parse_mix(MIX_A)), ("B", ba.parse_mix(MIX_B))]
    first = ba.swatch_sheet(jar, shortlist, mixes, seed=7)
    second = ba.swatch_sheet(jar, shortlist, mixes, seed=7)
    assert first == second
    assert first[:8] == b"\x89PNG\r\n\x1a\n"
    other = ba.swatch_sheet(jar, shortlist, mixes, seed=8)
    assert other != first, "the seed must actually reach the tiling"
    width, height, _ = ba.decode_png(first)
    assert width > 0 and height > 0


@needs_jar
def test_id_near_list_still_work():
    """AC9: the behaviour authors and the skill already depend on is preserved."""
    one = run_tool("--id", "minecraft:packed_mud", env={"DELVEWRIGHT_CLIENT_JAR": str(JAR)})
    assert one.returncode == 0
    assert "minecraft:packed_mud" in one.stdout and "#8e6b50" in one.stdout

    near = run_tool("--near", "#6b6b6b", "-n", "5", env={"DELVEWRIGHT_CLIENT_JAR": str(JAR)})
    assert near.returncode == 0
    assert len(near.stdout.strip().splitlines()) >= 6

    listing = run_tool(
        "--list", "--full-cube-only", "--json", env={"DELVEWRIGHT_CLIENT_JAR": str(JAR)}
    )
    assert listing.returncode == 0
    doc = json.loads(listing.stdout)
    assert len(doc["blocks"]) == 409
    assert all(row["full_cube"] for row in doc["blocks"])
    # AC1: the JSON gained the new fields and kept the old ones.
    row = next(r for r in doc["blocks"] if r["id"] == "minecraft:calcite")
    for field in (
        "rgb", "coverage", "full_cube", "textures", "tinted",
        "L", "L_p05", "L_p95", "L_sd", "texture_range",
        "C_mean", "C_p90", "C_max", "hue", "family", "form", "gravity", "technical",
    ):
        assert field in row, field

    bad = run_tool("--id", "minecraft:not_a_block", env={"DELVEWRIGHT_CLIENT_JAR": str(JAR)})
    assert bad.returncode == 2


def test_jar_gated_inventory_is_named():
    """A skipped test is invisible; a skipped test nobody can name is an UNRUN
    gate. This fails if the jar-gated set changes without the docstring's split
    being updated with it."""
    defined = {
        name
        for name, value in globals().items()
        if name.startswith("test_") and any(
            mark.name == "skipif" for mark in getattr(value, "pytestmark", [])
        )
    }
    assert defined == JAR_GATED, f"jar-gated set drifted: {sorted(defined)}"
    if JAR is None:
        print(f"\nJAR-GATED AND NOT RUN HERE: {sorted(JAR_GATED)}")


# --------------------------------------------------------------------------
# The registry-absence refusal, and why this test exists at all
#
# `load_registry()` arrived in one PR and a second PR rewrote this whole tool
# from an older base, dropping it. Both branches were green, the conflict was in
# one hunk, and resolving it as "take the rewrite, it is the superset" would have
# deleted a merged fix with nothing anywhere going red — the rewrite carries a
# refusal for the CLASSIFICATION table, which is what makes the omission easy to
# read past. The behaviour is now asserted rather than remembered.
#
# It is deliberately not jar-gated: the registry path is the one input this tool
# needs that CI does have, so the refusal is provable in CI.
# --------------------------------------------------------------------------


def test_a_missing_registry_is_a_named_refusal_not_a_traceback(monkeypatch, tmp_path):
    monkeypatch.setattr(ba, "REGISTRY", tmp_path / "absent" / "blocks-1.21.11.json")
    with pytest.raises(SystemExit) as excinfo:
        ba.load_registry()
    message = str(excinfo.value)
    assert "no block registry at" in message
    # The refusal must name the fallback, or it turns a mandatory step optional.
    assert "delve-grammar" in message
    assert "not optional" in message


def test_the_registry_refusal_is_reachable_from_the_entry_point(monkeypatch, tmp_path):
    """The refusal is worth nothing if `main` reads the file some other way."""
    source = TOOL.read_text()
    assert "json.loads(load_registry())" in source, (
        "main() no longer routes the registry read through load_registry(); a bare "
        "REGISTRY.read_text() gives back the traceback this refusal replaced"
    )
    assert "REGISTRY.read_text()" not in source.replace(
        "return REGISTRY.read_text()", ""
    ), "a second, unrefused read of the registry has appeared"
