"""`check-skill-page.py`, perturbed toward each shape it exists to catch.

A gate is tested by making the tree wrong in exactly the way that gate is the
only thing standing against, and checking it reds. Every test below does one
perturbation of a real copy of the plugin — never a fixture built to fail — so a
green here is evidence about the tool and not about a mock.

The engine is materialised once, at the pinned revision, and shared: it is the
instrument, and re-extracting it per test would be several seconds each for a
tree nothing perturbs.
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import shutil
import subprocess

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
GATE = REPO / "tools" / "check-skill-page.py"


@pytest.fixture(scope="module")
def mod():
    spec = importlib.util.spec_from_file_location("check_skill_page", GATE)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def engine(mod, tmp_path_factory):
    _repo, _release, rev = mod.read_pin()
    into = tmp_path_factory.mktemp("engine")
    return mod.materialise(rev, into), rev


@pytest.fixture
def tree(mod, tmp_path, monkeypatch):
    """A working copy of the plugin, with the gate pointed at it.

    `REPO` stays the real repository: it is where the engine at `ref` and the
    pre-split blob are read from, and both are instruments rather than subjects.
    """
    plugin = tmp_path / "plugin"
    shutil.copytree(mod.PLUGIN_ROOT, plugin)
    skill = plugin / "skills" / "new-delve"
    market = tmp_path / "marketplace" / ".claude-plugin"
    market.mkdir(parents=True)
    # The marketplace's `source` is resolved from ITS root, so the copy sits
    # where the real layout puts it.
    (tmp_path / "marketplace" / ".claude").mkdir(parents=True, exist_ok=True)
    (tmp_path / "marketplace" / ".claude" / "skills").mkdir(exist_ok=True)
    shutil.copytree(plugin, tmp_path / "marketplace" / ".claude" / "skills" / "delvewright")
    shutil.copy2(mod.MARKETPLACE, market / "marketplace.json")

    census = tmp_path / "skill-page-headings.json"
    shutil.copy2(mod.CENSUS, census)

    monkeypatch.setattr(mod, "PLUGIN_ROOT", plugin)
    monkeypatch.setattr(mod, "SKILL_ROOT", skill)
    monkeypatch.setattr(mod, "SKILL", skill / "SKILL.md")
    monkeypatch.setattr(mod, "PIN", skill / "versions.toml")
    monkeypatch.setattr(mod, "PLUGIN_JSON", plugin / ".claude-plugin" / "plugin.json")
    monkeypatch.setattr(mod, "MARKETPLACE", market / "marketplace.json")
    monkeypatch.setattr(mod, "CENSUS", census)
    return skill


def run(mod, engine, base=None):
    """Judge the tree the fixture set up, against the engine at the real pin.

    `rev` comes from the PERTURBED pin, so a test that breaks the pin's shape
    exercises the rule about it; the materialised engine stays the real one,
    because it is the instrument and no test perturbs it.
    """
    rep = mod.Report()
    engine_root, _real_rev = engine
    _repo, release, rev = mod.read_pin()
    mod.check(rep, engine_root, rev, release, base)
    return rep


def edit(path: pathlib.Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    assert text.count(old) == 1, f"{path.name}: {text.count(old)} matches for {old!r}"
    path.write_text(text.replace(old, new), encoding="utf-8")


def has(rep, needle: str) -> bool:
    return any(needle in f for f in rep.findings)


# ------------------------------------------------------- the tree as it stands --


def test_the_committed_plugin_holds(mod, tree, engine):
    """The baseline, without which every perturbation below proves nothing."""
    rep = run(mod, engine)
    assert rep.findings == [], rep.findings
    assert all(bound > 0 for _what, bound, _of in rep.bindings), rep.bindings


def test_every_binding_states_its_denominator(mod, tree, engine):
    rep = run(mod, engine)
    assert len(rep.bindings) >= 12
    for what, bound, of in rep.bindings:
        assert isinstance(of, int) and of >= 0, (what, of)


# ------------------------------------------------------------- rule 1, frontmatter --


def test_a_fourth_frontmatter_field_reds(mod, tree, engine):
    edit(tree / "SKILL.md", "metadata:\n", "argument-hint: <prompt>\nmetadata:\n")
    assert has(run(mod, engine), "frontmatter keys are")


def test_a_window_whose_ceiling_is_not_the_next_major_reds(mod, tree, engine):
    edit(tree / "SKILL.md", '">=1.0.0 <2.0.0"', '">=1.0.0 <1.5.0"')
    assert has(run(mod, engine), "is not the floor's next major")


def test_a_name_that_is_not_the_directory_reds(mod, tree, engine):
    edit(tree / "SKILL.md", "name: new-delve", "name: make-delve")
    assert has(run(mod, engine), "the directory is")


# ------------------------------------------------------------------ rule 2, the pin --


def test_a_branch_name_in_ref_reds(mod, tree, engine):
    """The revision is read out of the pin, never written down here.

    A gate's guard that pasted the revision would be a second copy of it, in a
    file pin discovery reads — which is the very defect rule 2 exists for.
    """
    _repo, _release, rev = mod.read_pin()
    edit(tree / "versions.toml", f'ref = "{rev}"', 'ref = "main"')
    assert has(run(mod, engine), "not a full 40-hex revision")


def test_the_release_pasted_into_a_reference_reds(mod, tree, engine):
    _repo, release, _ref = mod.read_pin()
    path = tree / "references" / "init.md"
    path.write_text(
        path.read_text(encoding="utf-8") + f"\nThe release is {release}.\n",
        encoding="utf-8",
    )
    assert has(run(mod, engine), "carries the `release` literal")


def test_a_page_that_stops_reading_the_pin_reds(mod, tree, engine):
    for path in [tree / "SKILL.md"] + sorted((tree / "references").glob("*.md")):
        text = path.read_text(encoding="utf-8")
        path.write_text(
            text.replace('["engine"]["ref"]', "[REDACTED]").replace(
                "[engine].ref", "[REDACTED]"
            ),
            encoding="utf-8",
        )
    for path in sorted((tree / "scripts").glob("*.py")):
        text = path.read_text(encoding="utf-8")
        path.write_text(text.replace("[engine].{key}", "[REDACTED]"), encoding="utf-8")
    assert has(run(mod, engine), "extracts `[engine].ref`")


# --------------------------------------------------------------- rule 4, the surface --


def test_a_subcommand_the_cli_does_not_have_reds(mod, tree, engine):
    edit(
        tree / "references" / "build.md",
        'delvec --prefabs "$DELVEWRIGHT_PREFABS" analyze campaigns/<id>',
        'delvec --prefabs "$DELVEWRIGHT_PREFABS" analyse campaigns/<id>',
    )
    assert has(run(mod, engine), "which the CLI does not have")


def test_a_flag_that_is_neither_the_subcommands_nor_a_global_reds(mod, tree, engine):
    edit(
        tree / "references" / "build.md",
        "delvec fmt campaigns/<id>",
        "delvec fmt --recursive campaigns/<id>",
    )
    assert has(run(mod, engine), "neither one of its own")


# -------------------------------------------------- rule 5, the engine's own surfaces --


def test_a_stage_document_the_page_never_names_reds(mod, tree, engine):
    for path in [tree / "SKILL.md"] + sorted((tree / "references").glob("*.md")):
        text = path.read_text(encoding="utf-8")
        path.write_text(text.replace("site-plan", "the-plan"), encoding="utf-8")
    assert has(run(mod, engine), "campaign stage document `site-plan.json`")


def test_a_world_field_the_page_names_in_no_code_span_reds(mod, tree, engine):
    """`outro` is the field the page names exactly once, so one edit unnames it.

    It is also the field the defect actually takes: an optional one whose
    absence changes the last sentence a player reads and refuses nothing.
    """
    for path in [tree / "SKILL.md"] + sorted((tree / "references").glob("*.md")):
        text = path.read_text(encoding="utf-8")
        path.write_text(text.replace("`outro`", "*the closing line*"), encoding="utf-8")
    assert has(run(mod, engine), "field `outro`")


# ------------------------------------------------------------------ rule 6, the budget --


def test_a_body_of_501_lines_reds(mod, tree, engine):
    path = tree / "SKILL.md"
    body = mod.body_lines(path.read_text(encoding="utf-8"))
    padding = "\n".join(["padding."] * (501 - len(body)))
    path.write_text(path.read_text(encoding="utf-8") + "\n" + padding, encoding="utf-8")
    assert has(run(mod, engine), "budget is 500")


# ----------------------------------------------------------------- rule 7, the pointers --


def test_an_unpointed_bundled_file_reds(mod, tree, engine):
    (tree / "references" / "orphan.md").write_text("# Orphan\n", encoding="utf-8")
    assert has(run(mod, engine), "references/orphan.md is bundled")


def test_a_pointer_to_a_file_that_is_not_there_reds(mod, tree, engine):
    (tree / "references" / "when-red.md").unlink()
    rep = run(mod, engine)
    assert has(rep, "names `references/when-red.md`")


# ------------------------------------------------------------------ rule 8, the shape --


def test_a_long_reference_with_no_contents_list_reds(mod, tree, engine):
    path = tree / "references" / "quest-capabilities.md"
    lines = path.read_text(encoding="utf-8").split("\n")
    keep = [line for line in lines if not line.startswith("- [")]
    path.write_text("\n".join(keep), encoding="utf-8")
    assert has(run(mod, engine), "opens with no contents list")


def test_a_contents_link_that_resolves_to_nothing_reds(mod, tree, engine):
    edit(
        tree / "references" / "quest-capabilities.md",
        "- [Objectives](#objectives)",
        "- [Objectives](#objective)",
    )
    assert has(run(mod, engine), "resolves to no heading of its own")


def test_a_reference_that_sends_the_reader_to_another_reds(mod, tree, engine):
    path = tree / "references" / "when-red.md"
    path.write_text(
        path.read_text(encoding="utf-8") + "\nSee `references/pitfalls.md`.\n",
        encoding="utf-8",
    )
    assert has(run(mod, engine), "sends the reader on to")


# ------------------------------------------------------------------- rule 9, --prefabs --


def test_a_bare_piece_reading_invocation_reds(mod, tree, engine):
    edit(
        tree / "references" / "build.md",
        'delvec --prefabs "$DELVEWRIGHT_PREFABS" analyze campaigns/<id>',
        "delvec analyze campaigns/<id>",
    )
    assert has(run(mod, engine), "is invoked without `--prefabs")


def test_a_piece_free_subcommand_owes_nothing(mod, tree, engine):
    """The opt-out is the OBJECT's, so it cannot be widened by an author.

    `fmt` reads no piece and carries no flag in the committed tree; the
    baseline test above is what says that is a pass. This one says the set is
    the one the source names and not whatever a page happens to write.
    """
    assert mod.PIECE_FREE == {"fmt", "schema", "metrics"}
    assert mod.PIECE_FREE_GROUP_VERBS == {("grammar", "list")}


# ------------------------------------------------------------ rule 10, the split --


def test_a_section_the_split_dropped_reds(mod, tree, engine):
    path = tree / "references" / "pitfalls.md"
    edit(path, "# Reference: authoring pitfalls", "# Pitfalls")
    rep = run(mod, engine)
    assert has(rep, "Reference: authoring pitfalls")
    assert has(rep, "is a heading of no file")


def test_a_section_the_split_doubled_reds(mod, tree, engine):
    path = tree / "references" / "when-red.md"
    path.write_text(
        path.read_text(encoding="utf-8") + "\n## Reference: authoring pitfalls\n",
        encoding="utf-8",
    )
    assert has(run(mod, engine), "One section, one home")


def test_a_row_claiming_restated_outside_init_reds(mod, tree, engine, monkeypatch):
    """The opt-out is secured by a property the defect cannot supply.

    `restated` is admissible only for a heading the census MEASURED as living
    under Init, because spec-0063 §6 restates Init's decomposition and nothing
    else's. A dropped step heading relabelled `restated` therefore cannot pass:
    its recorded section is the step, not Init.
    """
    census = json.loads(mod.CENSUS.read_text(encoding="utf-8"))
    for row in census["headings"]:
        if row["heading"] == "Reference: authoring pitfalls":
            row["destination"] = mod.RESTATED
            break
    else:  # pragma: no cover - the census would have to have lost the row
        pytest.fail("the census no longer carries the heading this test perturbs")
    mod.CENSUS.write_text(json.dumps(census, indent=2), encoding="utf-8")
    edit(
        tree / "references" / "pitfalls.md",
        "# Reference: authoring pitfalls",
        "# Pitfalls",
    )
    rep = run(mod, engine)
    assert has(rep, "Only a heading UNDER Init may be restated")


def test_the_census_is_re_derived_from_the_blob_it_names(mod, tree, engine):
    """A census that stopped describing the page it names is a red, not a claim."""
    census = json.loads(mod.CENSUS.read_text(encoding="utf-8"))
    census["headings"] = census["headings"][:-1]
    mod.CENSUS.write_text(json.dumps(census, indent=2), encoding="utf-8")
    assert has(run(mod, engine), "disagrees with the page it names")


def test_the_census_names_the_blob_this_repository_carries(mod):
    """The record is a measurement, so it names what was measured, by hash."""
    census = json.loads(mod.CENSUS.read_text(encoding="utf-8"))
    blob = census["source"]["blob"]
    proc = subprocess.run(
        ["git", "-C", str(REPO), "cat-file", "-t", blob], capture_output=True, text=True
    )
    assert proc.stdout.strip() == "blob", (
        "the pre-split page's blob is unreachable from this checkout, so rule 10 "
        "can only run against the frozen record"
    )


# ---------------------------------------------------------------- rules 11 and 12 --


def test_a_plugin_version_that_is_not_semver_reds(mod, tree, engine):
    path = mod.PLUGIN_JSON
    data = json.loads(path.read_text(encoding="utf-8"))
    data["version"] = "1.0"
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    assert has(run(mod, engine), "which is not semver")


def test_a_marketplace_source_resolving_to_no_plugin_reds(mod, tree, engine):
    path = mod.MARKETPLACE
    data = json.loads(path.read_text(encoding="utf-8"))
    data["plugins"][0]["source"] = "./.claude/skills/moved-away"
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    assert has(run(mod, engine), "carries no `.claude-plugin/plugin.json`")


def test_a_marketplace_entry_declaring_its_own_version_reds(mod, tree, engine):
    path = mod.MARKETPLACE
    data = json.loads(path.read_text(encoding="utf-8"))
    data["plugins"][0]["version"] = "9.9.9"
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    assert has(run(mod, engine), "authorities for one decision")


# ------------------------------------------- rule 14, an unsubstituted placeholder --


def test_a_template_placeholder_in_a_shipped_file_reds(mod, tree, engine):
    """`references/writing-craft.md` shipped to the marketplace at 1.1.0 with
    `@@TOC@@` as line 1. Nothing renders these pages, so nothing failed."""
    path = tree / "references" / "writing-craft.md"
    path.write_text("@@TOC@@\n\n" + path.read_text(encoding="utf-8"), encoding="utf-8")
    assert has(run(mod, engine), "carries the unsubstituted placeholder `@@TOC@@`")


def test_a_placeholder_in_a_script_reds_too(mod, tree, engine):
    """The rule binds to the whole shipped plugin, not to the two directories
    rule 7 enumerates: a creator reads what ships, whatever its suffix."""
    path = tree / "scripts" / "find-jdk.py"
    path.write_text(
        "# {{SUMMARY}}\n" + path.read_text(encoding="utf-8"), encoding="utf-8"
    )
    assert has(run(mod, engine), "unsubstituted placeholder `{{SUMMARY}}`")


# ------------------------------------ rule 15, a flag one supported provider refuses --


def test_a_refimg_flag_taught_without_the_provider_that_refuses_it_reds(mod, tree, engine):
    """The pre-repair shape: two pages taught `--chain-from` and `--style-note`
    as THE method for holding a series to one style, and `ideogram-v3` — one of
    the two providers the page's own request text offers — refuses both."""
    path = tree / "references" / "map-reference.md"
    path.write_text(
        path.read_text(encoding="utf-8").replace("ideogram-v3", "the other one"),
        encoding="utf-8",
    )
    rep = run(mod, engine)
    assert has(rep, "names `--chain-from`, which ideogram-v3 refuses")
    assert has(rep, "names `--style-note`, which ideogram-v3 refuses")


def test_the_refused_set_comes_from_the_tool_and_is_not_empty(mod, tree, engine):
    """The verdict is `refimg.py`'s own, put to it at the pin. A rule whose
    refusal set is empty judges every page green and means nothing."""
    rep = run(mod, engine)
    refused = dict(
        (what, bound) for what, bound, _of in rep.bindings
    )["refimg flag(s) some provider refuses"]
    assert refused >= 4, rep.bindings


# ------------------------------------------- rule 16, Init proves what a step runs --


def test_a_program_a_later_step_invokes_and_init_never_proves_reds(mod, tree, engine):
    """The shape a full drill of the page found: `docker info` passes on a
    machine with no Compose plugin,
    and step 10 dies hours later with `unknown shorthand flag: 'p' in -p`."""
    edit(tree / "SKILL.md", "\ndocker compose version", "\n# (nothing here)")
    init = tree / "references" / "init.md"
    text = init.read_text(encoding="utf-8")
    assert "docker compose version" in text
    init.write_text(text.replace("docker compose version", "docker info"), "utf-8")
    assert has(run(mod, engine), "invokes `docker compose`, and Init proves it nowhere")


def test_a_command_named_only_in_inline_PROSE_inside_init_does_not_prove_it(
    mod, tree, engine
):
    """The opt-out a defect could otherwise supply. Init already carries
    `docker compose … --profile play` as an inline span in a sentence about
    output paths; that sentence must not stand in for a check."""
    proofs = mod.init_proof_set()
    assert not any("--profile play" in p for p in proofs), sorted(proofs)


# --------------------------------------------------------------- the gate refuses --


def test_an_unreachable_pinned_engine_refuses_rather_than_judging_the_working_tree(mod):
    """Exit 2, not a pass. A gate that cannot reach its instrument checked nothing."""
    with pytest.raises(mod.Unusable) as caught:
        mod.materialise("0" * 40, pathlib.Path("/tmp"))
    assert "cannot serve" in str(caught.value)


def test_the_cli_exits_zero_on_the_committed_tree():
    proc = subprocess.run(
        ["python3", str(GATE)], capture_output=True, text=True, cwd=str(REPO)
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "check-skill-page: ok" in proc.stdout
