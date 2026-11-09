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
import os
import pathlib
import shutil
import subprocess

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
GATE = REPO / "tools" / "ci" / "check-skill-page.py"


@pytest.fixture(scope="module")
def mod():
    spec = importlib.util.spec_from_file_location("check_skill_page", GATE)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def engine(mod, tmp_path_factory):
    _repo, ref = mod.read_pin()
    rev, tag_exists = mod.resolve_ref(ref)
    into = tmp_path_factory.mktemp("engine")
    return mod.materialise(rev, into), rev, tag_exists


@pytest.fixture
def tree(mod, tmp_path, monkeypatch):
    """A working copy of the plugin, with the gate pointed at it.

    `REPO` stays the real repository: it is where the engine at `ref` is read
    from, and it is an instrument rather than a subject.
    """
    # ONE copy, laid out as the repository lays it out: the marketplace root is
    # `tmp_path`, and the entry's `path` resolves from there to the plugin root
    # this gate judges. A second copy under the marketplace would let the entry
    # point at a directory nothing here reads, which is the class ADR-0029
    # closes — so the fixture cannot contain the shape the rule forbids.
    plugin = tmp_path / ".claude" / "skills" / "delvewright"
    plugin.parent.mkdir(parents=True)
    shutil.copytree(mod.PLUGIN_ROOT, plugin)
    skill = plugin / "skills" / "new-delve"
    market = tmp_path / ".claude-plugin"
    market.mkdir(parents=True)
    shutil.copy2(mod.MARKETPLACE, market / "marketplace.json")

    monkeypatch.setattr(mod, "PLUGIN_ROOT", plugin)
    monkeypatch.setattr(mod, "SKILL_ROOT", skill)
    monkeypatch.setattr(mod, "SKILL", skill / "SKILL.md")
    monkeypatch.setattr(mod, "PIN", skill / "versions.toml")
    monkeypatch.setattr(mod, "PLUGIN_JSON", plugin / ".claude-plugin" / "plugin.json")
    monkeypatch.setattr(mod, "MARKETPLACE", market / "marketplace.json")
    return skill


def run(mod, engine):
    """Judge the tree the fixture set up, against the engine at the real pin.

    `rev` comes from the PERTURBED pin, so a test that breaks the pin's shape
    exercises the rule about it; the materialised engine stays the real one,
    because it is the instrument and no test perturbs it.
    """
    rep = mod.Report()
    engine_root, real_rev, real_tag_exists = engine
    _repo, ref = mod.read_pin()
    try:
        rev, tag_exists = mod.resolve_ref(ref)
    except mod.Unusable:
        rev, tag_exists = real_rev, real_tag_exists
    mod.check(rep, engine_root, rev, ref, tag_exists)
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
    """The tag is read out of the pin, never written down here.

    A gate's guard that pasted the name would be a second copy of it, in a file
    pin discovery reads — which is the very defect rule 2 exists for.
    """
    _repo, ref = mod.read_pin()
    edit(tree / "versions.toml", f'ref = "{ref}"', 'ref = "main"')
    assert has(run(mod, engine), "is not `<name>--v")


def test_a_tag_of_another_released_line_reds(mod, tree, engine):
    """The page installs the creator binary, so only that line may be pinned."""
    _repo, ref = mod.read_pin()
    edit(tree / "versions.toml", f'ref = "{ref}"', 'ref = "delvewright-dsl--v0.26.0"')
    assert has(run(mod, engine), "is a delvewright-dsl release tag")


def test_an_unborn_tag_that_is_not_this_trees_own_reds(mod, tree, engine):
    """The one unborn name a pin may carry is the tag this tree's own release
    would create (ADR-0029 §3); any other is a pin onto something nobody is
    about to publish."""
    _repo, ref = mod.read_pin()
    edit(tree / "versions.toml", f'ref = "{ref}"', 'ref = "delvec--v99.0.0"')
    assert has(run(mod, engine), "this repository has no tag for")


def test_a_pin_that_still_carries_release_is_unusable(mod, tree, engine):
    """Two keys naming one release is the restatement ADR-0029 §2 removes."""
    path = tree / "versions.toml"
    path.write_text(
        path.read_text(encoding="utf-8").replace("[engine]\n", '[engine]\nrelease = "v1.5.0"\n'),
        encoding="utf-8",
    )
    with pytest.raises(mod.Unusable, match="still carries"):
        mod.read_pin()


def test_the_tag_pasted_into_a_reference_reds(mod, tree, engine):
    _repo, ref = mod.read_pin()
    path = tree / "references" / "init.md"
    path.write_text(
        path.read_text(encoding="utf-8") + f"\nThe release is {ref}.\n",
        encoding="utf-8",
    )
    assert has(run(mod, engine), "carries the `ref` literal")


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


# ------------------------------------------------------ rule 23, one paragraph, one line --


def test_a_wrapped_paragraph_reds(mod, tree, engine):
    path = tree / "references" / "when-red.md"
    path.write_text(path.read_text(encoding="utf-8") + "\nA paragraph that\nis broken here.\n", encoding="utf-8")
    rep = run(mod, engine)
    assert has(rep, "when-red.md:") and has(rep, "continues the paragraph")


def test_a_wrapped_list_item_reds(mod, tree, engine):
    path = tree / "SKILL.md"
    path.write_text(path.read_text(encoding="utf-8") + "\n- an item that\n  carries on.\n", encoding="utf-8")
    assert has(run(mod, engine), "SKILL.md:")


def test_fences_tables_and_hard_breaks_are_not_wrapped_prose(mod):
    text = "# T\n\n```\na\nb\n```\n\n| a | b |\n| c | d |\n\nline  \nafter a hard break\n"
    assert mod.wrapped_lines(text) == []
    assert mod.wrapped_lines("one\ntwo\n") == [2]


# ----------------------------------------------------------------- rule 7, the pointers --


def test_an_unpointed_bundled_file_reds(mod, tree, engine):
    (tree / "references" / "orphan.md").write_text("# Orphan\n", encoding="utf-8")
    assert has(run(mod, engine), "references/orphan.md is bundled")


def test_a_pointer_to_a_file_that_is_not_there_reds(mod, tree, engine):
    (tree / "references" / "when-red.md").unlink()
    rep = run(mod, engine)
    assert has(rep, "names `references/when-red.md`")


def test_a_pinned_url_into_another_repository_is_not_a_pointer(mod, tree, engine):
    path = tree / "references" / "when-red.md"
    url = "https://github.com/o/r/tree/0123abc/references/not-bundled"
    path.write_text(path.read_text(encoding="utf-8") + f"\nSee <{url}>.\n", encoding="utf-8")
    assert not has(run(mod, engine), "references/not-bundled")
    path.write_text(path.read_text(encoding="utf-8") + "\nSee `references/not-bundled`.\n", encoding="utf-8")
    assert has(run(mod, engine), "names `references/not-bundled`")


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


def test_a_markdown_link_to_a_sibling_reference_reds(mod, tree, engine):
    """A link resolved from the reference's own directory is the pointer a
    reader follows, whether or not the text spells `references/`."""
    path = tree / "references" / "when-red.md"
    path.write_text(
        path.read_text(encoding="utf-8") + "\nSee [the pitfalls](pitfalls.md#top).\n",
        encoding="utf-8",
    )
    assert has(run(mod, engine), "sends the reader on to `references/pitfalls.md`")


# ------------------------------------------------------------------- rule 9, --prefabs --


def test_a_bare_piece_reading_invocation_reds(mod, tree, engine):
    edit(
        tree / "references" / "build.md",
        'delvec --prefabs "$DELVEWRIGHT_PREFABS" analyze campaigns/<id>',
        "delvec analyze campaigns/<id>",
    )
    assert has(run(mod, engine), "is invoked without `--prefabs")


def test_a_build_into_a_shared_tree_reds(mod, tree, engine):
    """Rule 9's second arm: the build writes the session's own `$DELVE_OUTPUT`."""
    edit(
        tree / "references" / "build.md",
        'build campaigns/<id> -o "$DELVE_OUTPUT"',
        'build campaigns/<id> -o "$DELVEWRIGHT_ENGINE/validation/delve-output"',
    )
    rep = run(mod, engine)
    assert has(rep, 'writes somewhere other than `-o "$DELVE_OUTPUT"`')
    assert any(w.startswith("build invocation(s)") and b < o for w, b, o in rep.bindings), rep.bindings


# ------------------------------------------------------------ rule 22, the asks --


def test_an_ask_outside_every_listed_section_reds(mod, tree, engine):
    """spec-0122 §5's first perturbation: the provider question back in Init I7."""
    edit(
        tree / "references" / "init.md",
        "A variable whose name says what a key is *for* and nothing about *whose* it is names no provider and is treated as absent.",
        "A variable whose name says what a key is *for* and nothing about *whose* it is names no provider and is treated as absent. Ask the user which provider the key is for.",
    )
    rep = run(mod, engine)
    assert has(rep, "references/init.md:")
    assert has(rep, "asks the user to stop outside every section the ask table lists")
    assert any(w == "ask line(s) inside a listed section" and b < o for w, b, o in rep.bindings), rep.bindings


def test_a_row_deleted_while_its_ask_stands_reds(mod, tree, engine):
    """spec-0122 §5's second perturbation: A1 gone, the opening check still asking."""
    page = tree / "SKILL.md"
    text = page.read_text(encoding="utf-8")
    row = next(line for line in text.split("\n") if line.startswith("| **A1** |"))
    page.write_text(text.replace(row + "\n", ""), encoding="utf-8")
    rep = run(mod, engine)
    assert has(rep, "asks the user to stop outside every section the ask table lists")
    assert has(rep, "SKILL.md:")


def test_a_row_no_page_executes_is_unbound(mod, tree, engine):
    """The unrun shape: H's one stop removed, the row still listed."""
    edit(
        tree / "references" / "hand-over.md",
        "**Then report to the user and end your turn**",
        "**Then report to the user**",
    )
    rep = run(mod, engine)
    assert has(rep, "the ask table's row H is bound by no stop")
    assert any(w == "listed row(s) bound" and b < o for w, b, o in rep.bindings), rep.bindings


def test_an_offer_that_stops_reds(mod, tree, engine):
    edit(
        tree / "SKILL.md",
        "default, unless the user volunteers a number.",
        "default, unless the user volunteers a number; end your turn until they pick.",
    )
    rep = run(mod, engine)
    assert has(rep, "a paragraph that makes an offer also stops for an answer")


def test_no_ask_table_binds_nothing_and_reds(mod, tree, engine):
    edit(
        tree / "SKILL.md",
        "| id | where | what the user is asked, in player words | what the run waits for |",
        "| id | place | what the user is asked, in player words | what the run waits for |",
    )
    assert has(run(mod, engine), "carries no ask table")


def test_a_piece_free_subcommand_owes_nothing(mod, tree, engine):
    """The opt-out is the OBJECT's, so it cannot be widened by an author.

    `fmt` reads no piece and carries no flag in the committed tree; the
    baseline test above is what says that is a pass. This one says the set is
    the one the source names and not whatever a page happens to write.
    """
    assert mod.PIECE_FREE == {"fmt", "schema", "metrics"}
    assert mod.PIECE_FREE_GROUP_VERBS == {("grammar", "list")}


# ------------------------------------------------------- rule 10, retired --


def test_a_step_heading_is_the_page_s_to_rename(mod, tree, engine):
    """A step that moves is renamed with it, and no census of an older page
    holds the old words in place: the steps are an ordering the page owns."""
    edit(
        tree / "SKILL.md",
        "## 12. Visual review",
        "## 12. Visual review — the POV sequence first",
    )
    rep = run(mod, engine)
    assert not has(rep, "Visual review"), rep.findings


# ---------------------------------------------------------------- rules 11 and 12 --


def test_a_relative_path_source_reds(mod, tree, engine):
    """A relative path is a string: it carries no field, so it cannot pin."""
    path = mod.MARKETPLACE
    data = json.loads(path.read_text(encoding="utf-8"))
    data["plugins"][0]["source"] = "./.claude/skills/delvewright"
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    assert has(run(mod, engine), "It is a `git-subdir` object")


def test_an_entry_ref_that_is_not_the_plugins_own_tag_reds(mod, tree, engine):
    """The entry's `ref` is derived from `plugin.json` `version`, held equal here
    or it is two authorities; an engine tag there is the shape ADR-0038 moved
    away from, and it reds like any other name."""
    _repo, ref = mod.read_pin()
    path = mod.MARKETPLACE
    data = json.loads(path.read_text(encoding="utf-8"))
    data["plugins"][0]["source"]["ref"] = ref
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    rep = run(mod, engine)
    assert has(rep, f"`source.ref` is {ref!r} and the plugin's own release tag"), rep.findings


def test_an_entry_url_that_is_not_the_pins_repo_reds(mod, tree, engine):
    path = mod.MARKETPLACE
    data = json.loads(path.read_text(encoding="utf-8"))
    data["plugins"][0]["source"]["url"] = "someone-else/delvewright"
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    assert has(run(mod, engine), "`source.url` is 'someone-else/delvewright'")


def test_an_entry_carrying_a_sha_reds(mod, tree, engine):
    """The documentation makes a `sha` the effective pin where both are set."""
    path = mod.MARKETPLACE
    data = json.loads(path.read_text(encoding="utf-8"))
    data["plugins"][0]["source"]["sha"] = "0" * 40
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    assert has(run(mod, engine), "source declares `sha`")


def test_an_entry_path_that_is_not_the_plugin_root_reds(mod, tree, engine):
    path = mod.MARKETPLACE
    data = json.loads(path.read_text(encoding="utf-8"))
    data["plugins"][0]["source"]["path"] = ".claude/skills/moved-away"
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    assert has(run(mod, engine), "not to the plugin root")


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
    path = tree / "scripts" / "debug-note.py"
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


def test_the_rule_reaches_the_reference_that_actually_serves_the_build(mod, tree, engine):
    """**`references/hand-over.md` is inside rule 16's population, by name.**

    The rule and the file have different authors: the rule was written where
    `SKILL.md` invokes Compose, and `hand-over.md` — the page that brings the
    play server up — was written afterwards. The test above would stay green
    if that page's `docker compose` line moved out of a fence or behind a
    variable, because SKILL.md alone still satisfies it, and the rule would then
    be saying nothing about the step that serves the build.

    So this asserts the population rather than the refusal: the finding names
    hand-over.md, which it can only do if hand-over.md really invokes an
    acquired program in a fenced span the rule reads.
    """
    edit(tree / "SKILL.md", "\ndocker compose version", "\n# (nothing here)")
    init = tree / "references" / "init.md"
    text = init.read_text(encoding="utf-8")
    assert "docker compose version" in text
    init.write_text(text.replace("docker compose version", "docker info"), "utf-8")
    rep = run(mod, engine)
    assert has(rep, "references/hand-over.md invokes `docker compose`"), rep.findings


# ------------------------------------------------ rule 19, the pin check every run --


def test_a_run_shape_that_builds_the_toolchain_once_per_machine_reds(mod, tree, engine):
    """The shape the page had while an updated plugin kept running its old engine."""
    page = tree / "SKILL.md"
    text = page.read_text(encoding="utf-8")
    start = text.index("\nInit            ") + 1
    end = text.index("\n", start)
    page.write_text(
        text[:start] + "Init            build the toolchain, once per machine" + text[end:],
        encoding="utf-8",
    )
    assert has(run(mod, engine), "the run shape's `Init` entry does not name I1b")


def test_an_i8_checklist_without_the_pin_check_reds(mod, tree, engine):
    page = tree / "SKILL.md"
    text = page.read_text(encoding="utf-8")
    fence_line = '"$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/check-toolchain.py" \\\n'
    assert text.count(fence_line) == 1
    page.write_text(text.replace(fence_line, "# "), encoding="utf-8")
    assert has(run(mod, engine), "the I8 checklist does not run")


def test_the_pin_check_named_only_in_prose_under_i1b_does_not_count(mod, tree, engine):
    """An inline mention is not an invocation: the fence has to carry it."""
    init = tree / "references" / "init.md"
    text = init.read_text(encoding="utf-8")
    section = text.split("\n## I1b ")[1].split("\n## ")[0]
    fence = section[section.index("```sh"): section.index("```", section.index("```sh") + 5) + 3]
    assert "check-toolchain.py" in fence
    init.write_text(text.replace(fence, "Run `scripts/check-toolchain.py`."), encoding="utf-8")
    assert has(run(mod, engine), "has no I1b section whose fence runs")


def test_a_command_named_only_in_inline_PROSE_inside_init_does_not_prove_it(
    mod, tree, engine
):
    """The opt-out a defect could otherwise supply. Init already carries
    `docker compose … --profile play` as an inline span in a sentence about
    output paths; that sentence must not stand in for a check."""
    proofs = mod.init_proof_set()
    assert not any("--profile play" in p for p in proofs), sorted(proofs)


# ------------------------------- rule 11, the version is the plugin's own line --


def _set_version(mod, version: str) -> None:
    path = mod.PLUGIN_JSON
    data = json.loads(path.read_text(encoding="utf-8"))
    data["version"] = version
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")


def _set_entry_ref(mod, ref: str) -> None:
    path = mod.MARKETPLACE
    data = json.loads(path.read_text(encoding="utf-8"))
    data["plugins"][0]["source"]["ref"] = ref
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")


def _newest_plugin_tag(mod) -> tuple[str, str]:
    """The newest `delvewright--v*` tag this repository carries, with its version."""
    tags = mod.plugin_tags_here()
    key = lambda t: mod.release_tags.version_key(mod.release_tags.parse(t)[1])  # noqa: E731
    tag = max(tags, key=key)
    return tag, mod.release_tags.parse(tag)[1]


def _bump(version: str, patch: int) -> str:
    major, minor, p = (int(x) for x in version.split("."))
    return f"{major}.{minor}.{p + patch}"


def test_the_newest_released_version_holds_with_its_drift_printed(mod, tree, engine, capsys):
    """A page edit merged between releases: the version stays the newest
    release's, the entry names that tag, and the edit is counted, not refused —
    it reaches creators with the next plugin release (ADR-0038 §1)."""
    tag, version = _newest_plugin_tag(mod)
    _set_version(mod, version)
    _set_entry_ref(mod, tag)
    (tree / "references" / "when-red.md").write_text(
        (tree / "references" / "when-red.md").read_text(encoding="utf-8") + "\nA line merged after the release.\n",
        encoding="utf-8",
    )
    rep = run(mod, engine)
    assert not has(rep, "`plugin.json` `version` is"), rep.findings
    assert not has(rep, "`source.ref`"), rep.findings
    out = capsys.readouterr().out
    assert f"is the newest release of its line ({tag})" in out
    assert "differs from it in" in out


def test_a_version_behind_the_newest_release_reds(mod, tree, engine):
    """The defect ADR-0033 fixed, kept fixed: a version a creator may already
    hold reaches nobody, so the line never goes back."""
    tag, version = _newest_plugin_tag(mod)
    older = min(mod.plugin_tags_here(), key=lambda t: mod.release_tags.version_key(mod.release_tags.parse(t)[1]))
    if older == tag:
        pytest.skip("one release on the line; nothing older to go back to")
    _set_version(mod, mod.release_tags.parse(older)[1])
    _set_entry_ref(mod, older)
    assert has(run(mod, engine), f"the newest release of the line is {tag}")


def test_an_unborn_version_above_the_newest_release_holds(mod, tree, engine, capsys):
    """The pull request that prepares a plugin release: the version is above
    every release, no tag carries it, and the entry names the tag the release
    dispatched on the merge commit will write."""
    tag, version = _newest_plugin_tag(mod)
    nxt = _bump(version, 1)
    _set_version(mod, nxt)
    _set_entry_ref(mod, f"delvewright--v{nxt}")
    rep = run(mod, engine)
    assert not has(rep, "`plugin.json` `version` is"), rep.findings
    assert not has(rep, "`source.ref`"), rep.findings
    assert f"is unborn and above the newest release of its line ({tag})" in capsys.readouterr().out


def test_an_unborn_version_below_the_newest_release_reds(mod, tree, engine):
    tag, version = _newest_plugin_tag(mod)
    major, minor, patch = (int(x) for x in version.split("."))
    below = f"{major}.{minor - 1}.99" if minor else f"{major - 1}.99.99"
    _set_version(mod, below)
    _set_entry_ref(mod, f"delvewright--v{below}")
    assert has(run(mod, engine), "which no release of the line carries, and the newest release is")


def test_a_version_outside_the_grammar_reds(mod, tree, engine):
    """`1.0` names no tag; the tag is derived from the version, strictly."""
    _set_version(mod, "1.0")
    assert has(run(mod, engine), "is not a strict `major.minor.patch`")


def test_a_leading_zero_in_the_version_reds(mod, tree, engine):
    _, version = _newest_plugin_tag(mod)
    _set_version(mod, "0" + _bump(version, 1))
    assert has(run(mod, engine), "is not a strict `major.minor.patch`")


def test_a_checkout_with_no_plugin_tag_cannot_judge(mod, tmp_path, monkeypatch):
    """A clone that has not fetched the line's tags would call every version
    unborn; the rule refuses and names the fetch instead."""
    bare = tmp_path / "bare"
    subprocess.run(["git", "init", "-q", str(bare)], check=True)
    monkeypatch.setattr(mod, "REPO", bare)
    with pytest.raises(mod.Unusable, match="carries no `delvewright--v\\*` tag"):
        mod.plugin_tags_here()


def test_own_version_rule_reads_the_tags_it_is_given(mod):
    """The rule's three states, over a list it is handed, so the judgement does
    not depend on which tags this checkout happens to carry."""
    tags = ["delvewright--v1.11.0", "delvewright--v1.15.0"]
    rep = mod.Report()
    assert mod.own_version_rule(rep, "1.15.1", tags) == "delvewright--v1.15.1"
    assert rep.findings == [], rep.findings
    rep = mod.Report()
    assert mod.own_version_rule(rep, "1.11.0", tags) == "delvewright--v1.11.0"
    assert any("the newest release of the line is delvewright--v1.15.0" in f for f in rep.findings), rep.findings
    rep = mod.Report()
    assert mod.own_version_rule(rep, "1.14.9", tags) == "delvewright--v1.14.9"
    assert any("which no release of the line carries" in f for f in rep.findings), rep.findings
    rep = mod.Report()
    assert mod.own_version_rule(rep, None, tags) is None
    assert rep.findings and rep.bindings == [("plugin release tag(s) of the line read", 2, 2)]


# ------------------------------------ rule 17, a DW code the pinned engine declares --


def test_a_dw_code_the_pinned_engine_does_not_declare_reds(mod, tree, engine):
    """The shape a page written against a newer engine than it pins has: it names
    a diagnostic the installed engine cannot print. The code is checked absent
    from the materialised engine first, so the red is about the pin."""
    engine_root, _rev, _tag_exists = engine
    source = "\n".join(
        rs.read_text(encoding="utf-8") for rs in (engine_root / "crates").rglob("*.rs")
    )
    code = next(f"DW{n:04d}" for n in range(9999, 0, -1) if f"DW{n:04d}" not in source)
    path = tree / "references" / "when-red.md"
    path.write_text(
        path.read_text(encoding="utf-8") + f"\nA refusal carries `{code}`.\n",
        encoding="utf-8",
    )
    assert has(run(mod, engine), f"`{code}`, and the engine at")


def test_a_dw_code_only_a_comment_mentions_is_not_declared(mod):
    """Declared means a diagnostic constant, read by `check-dw-codes.py`'s own
    rule over comment-stripped source — never a mention."""
    dw = mod.dw_codes_module()
    src = '// pub const OLD: DwCode = DwCode::new("DW0001", ExitTier::Build);\n'
    assert dw.CONST_RE.findall(dw.strip_comments(src)) == []
    src = 'pub const NEW: DwCode = DwCode::new("DW0002", ExitTier::Build);\n'
    assert dw.CONST_RE.findall(dw.strip_comments(src)) == [("NEW", "DW0002")]


# ------------------------------- rule 18, the names the page gives, asked of the release --
#
# The release binary is not reachable offline, so its answers are handed in by a
# runner. The SCHEMA below is a stand-in shaped like `delvec schema`'s output, and
# the subject is the real page: what these tests prove is the reading of the page
# and the resolution against a schema, not what any release exports — the online
# run asks the release itself.

RECORD_TWO = {
    "$defs": {
        "State": {
            "oneOf": [
                {"const": "open", "type": "string"},
                {"const": "closed", "type": "string"},
            ]
        }
    },
    "properties": {
        "state": {"$ref": "#/$defs/State"},
        "areas": {"type": "array"},
        "notes": {"type": "array"},
    },
}
HELP = (
    "      --stage <STAGE>      Which document. `stand-in` for the stand-in record; "
    "`<prefab-id>.json` is not a stage; or `all` for every stage document\n"
)


def fake_release(record: dict):
    def delvec(argv):
        if argv == ["schema", "--stage", "all"]:
            return 0, json.dumps({"world": {"properties": {"time": {"enum": ["dusk"]}}}})
        if argv == ["schema", "--help"]:
            return 0, HELP
        if argv == ["schema", "--stage", "stand-in"]:
            return 0, json.dumps(record)
        return 2, ""

    return delvec


def release_rep(mod, record):
    rep = mod.Report()
    mod.release_binary_rule(rep, fake_release(record), "v0.0.0")
    return rep


def teach(tree, fragment):
    """Append one document fragment to a real page file."""
    path = tree / "references" / "hand-over.md"
    path.write_text(path.read_text(encoding="utf-8") + f"\n`{fragment}`\n", encoding="utf-8")


def test_a_variant_the_release_does_not_admit_reds(mod, tree):
    """The measured case: the page teaches `state: "ajar"` and a record of two
    states refuses it as an unknown variant."""
    teach(tree, '{"state": "ajar", "areas": []}')
    rep = release_rep(mod, RECORD_TWO)
    assert has(rep, "gives `state` the value 'ajar'"), rep.findings


def test_the_same_page_holds_against_a_release_that_admits_it(mod, tree):
    teach(tree, '{"state": "ajar", "areas": []}')
    record = json.loads(json.dumps(RECORD_TWO))
    record["$defs"]["State"]["oneOf"].append({"const": "ajar", "type": "string"})
    rep = release_rep(mod, record)
    assert not has(rep, "gives `state`"), rep.findings
    bound, of = {what: (b, n) for what, b, n in rep.bindings}[
        "closed-set value(s) the release's schemas admit"
    ]
    assert of >= 1 and bound == of, rep.bindings


def test_a_field_the_release_does_not_carry_reds(mod, tree):
    """A document fragment whose keys are mostly the release's, naming one that
    is not. The stage is found through the binary's help, not a list here."""
    teach(tree, '{"state": "open", "areas": [], "opened_by": "a"}')
    assert has(release_rep(mod, RECORD_TWO), "names the field `opened_by`")


def test_a_fragment_of_mostly_unknown_keys_is_not_read_as_a_document(mod, tree):
    """A text component, a skin palette or a renderer option is not a document,
    and the object decides that: most of its keys are no field at all."""
    teach(tree, '{"translate": "k", "fallback": "x", "state": "open"}')
    rep = release_rep(mod, RECORD_TWO)
    assert not has(rep, "names the field `translate`"), rep.findings


def test_a_value_given_to_a_field_some_document_leaves_open_is_not_judged(mod, tree):
    """A name one document closes and another leaves open resolves to a
    candidate, not a match — so the value is not refused on the closed one."""
    teach(tree, '{"state": "ajar", "areas": []}')
    record = json.loads(json.dumps(RECORD_TWO))
    record["properties"]["nested"] = {"properties": {"state": {"type": "string"}}}
    rep = release_rep(mod, record)
    assert not has(rep, "gives `state`"), rep.findings


def codes_release(listed, exit_code=0):
    """A release answering `delvec codes` with one JSON line per listed code."""

    def delvec(argv):
        if argv == ["codes"]:
            lines = [json.dumps({"code": c, "tier": "Build", "name": "X"}) for c in listed]
            return exit_code, "\n".join(lines) + "\n"
        return 2, ""

    return delvec


def code_rep(mod, monkeypatch, delvec, verb=True):
    monkeypatch.setattr(mod, "engine_cli", lambda _engine: ({"codes": set()} if verb else {}, set()))
    rep = mod.Report()
    mod.release_code_rule(rep, delvec, pathlib.Path("engine"), "v0.0.0")
    return rep


def name_code(tree, code):
    path = tree / "references" / "when-red.md"
    path.write_text(
        path.read_text(encoding="utf-8") + f"\nA refusal carries `{code}`.\n",
        encoding="utf-8",
    )


def test_a_code_the_release_lists_is_green_whatever_its_bytes(mod, tree, monkeypatch):
    """The measured false negative: `delvec--v1.8.0` raises `DW0944` and its bytes
    never spell it — the literal is written by two immediate stores, `"DW09"` and
    `"44"`. Rule 18 asks the release's registry, so the page is green."""
    name_code(tree, "DW0944")
    named = mod.page_dw_codes()
    rep = code_rep(mod, monkeypatch, codes_release(sorted(named)))
    assert not rep.findings, rep.findings
    bound = {what: (b, n) for what, b, n in rep.bindings}
    assert bound["DW code(s) named that the release's `delvec codes` lists"] == (len(named), len(named))


def test_a_code_the_release_does_not_list_reds(mod, tree, monkeypatch):
    """The vacuous shape: a page naming a code the release does not declare."""
    name_code(tree, "DW0944")
    listed = sorted(set(mod.page_dw_codes()) - {"DW0944"})
    rep = code_rep(mod, monkeypatch, codes_release(listed))
    assert has(rep, "name `DW0944`, and `delvec codes` of the v0.0.0 release"), rep.findings


def test_a_release_whose_tag_declares_the_verb_and_refuses_it_reds(mod, tree, monkeypatch):
    rep = code_rep(mod, monkeypatch, codes_release([], exit_code=2))
    assert has(rep, "declares `delvec codes`, and the release binary answered it"), rep.findings


def test_a_release_older_than_the_verb_is_named_and_left_to_rule_17(mod, tree, monkeypatch, capsys):
    """Not asked, and said so: the tree at the tag has no `delvec codes`."""
    rep = code_rep(mod, monkeypatch, codes_release([]), verb=False)
    assert not rep.findings and not rep.bindings
    assert "rule 18 does NOT ask v0.0.0 for its DW codes" in capsys.readouterr().out


# --------------------------------------------------------------- the gate refuses --


def test_an_unreachable_pinned_engine_refuses_rather_than_judging_the_working_tree(mod):
    """Exit 2, not a pass. A gate that cannot reach its instrument checked nothing."""
    with pytest.raises(mod.Unusable) as caught:
        mod.materialise("0" * 40, pathlib.Path("/tmp"))
    assert "cannot serve" in str(caught.value)


def test_a_zero_binding_does_not_swallow_the_findings(mod, capsys, monkeypatch):
    """Both verdicts, always. A run that reported only `a binding of zero` and
    kept the findings it already held told the reader less than it knew — the
    same defect as a gate that refuses without saying what it examined."""

    def both(rep, *_args, **_kwargs):
        rep.find("a finding the reader has to see")
        rep.bind("thing(s) nothing bound to", 0, 3)

    monkeypatch.setattr(mod, "materialise", lambda _rev, into: into)
    monkeypatch.setattr(mod, "check", both)
    assert mod.main([]) == 1
    err = capsys.readouterr().err
    assert "a finding the reader has to see" in err
    assert "a binding of zero on: thing(s) nothing bound to" in err


# ------------------------------------------ rule 20, the names a playtest uses --


def test_a_trigger_the_overlay_does_not_register_reds(mod, tree, engine):
    edit(
        tree / "references" / "tools-by-symptom.md",
        "`/trigger dw.note`",
        "`/trigger dw.notes`",
    )
    assert has(run(mod, engine), "names trigger `dw.notes`")


def test_a_moved_layout_manifest_reds(mod, tree, engine):
    """One occurrence is enough, and the page names the path more than once.

    `edit` demands a unique match, which is the right rule for a perturbation
    that must land somewhere known — so the anchor here is the whole invocation
    that carries the path, not the path alone. Moving one occurrence is what the
    rule sees: it reads every `creator-datapack/` path a page file names.
    """
    edit(
        tree / "references" / "tools-by-symptom.md",
        "calibrate <report> --layout <out>/creator-datapack/layout.json",
        "calibrate <report> --layout <out>/creator-datapack/manifest.json",
    )
    assert has(run(mod, engine), "names path `creator-datapack/manifest.json`")


def test_a_profile_compose_does_not_declare_reds(mod, tree, engine):
    edit(
        tree / "references" / "hand-over.md",
        "--profile play up",
        "--profile plays up",
    )
    assert has(run(mod, engine), "names profile `plays`")


def test_a_log_read_from_a_container_nobody_names_reds(mod, tree, engine):
    edit(
        tree / "references" / "tools-by-symptom.md",
        "then `delvec harvest` → `playtest-report.json`",
        "then `docker logs dw-playtests > server.log` and `delvec harvest` → `playtest-report.json`",
    )
    assert has(run(mod, engine), "names container `dw-playtests`")


def test_the_overlay_triggers_are_read_the_way_the_emitter_resolves_them(mod):
    source = """
const NOTE: &str = "dw.note";
const CALIBRATION: [&str; 2] = ["dw.mark", "dw.done"];
const CAM: &str = "dw.cam";
const FREE: &str = "dw.free";
const SCRATCH: &str = "dw.rh";
fn init() {
    let a = vec![format!("scoreboard objectives add {NOTE} trigger")];
    let b: Vec<String> = CALIBRATION.iter().map(|t| format!("scoreboard objectives add {t} trigger")).collect();
    let c: Vec<String> = [CAM, FREE].iter().map(|t| format!("scoreboard objectives add {t} trigger")).collect();
    let d = format!("scoreboard objectives add dw.seen trigger");
    let e = format!("scoreboard objectives add {SCRATCH} dummy");
    // scoreboard objectives add dw.commented trigger
}
"""
    assert mod.overlay_triggers(source) == {
        "dw.note",
        "dw.mark",
        "dw.done",
        "dw.cam",
        "dw.free",
        "dw.seen",
    }
    with pytest.raises(mod.Unusable):
        mod.overlay_triggers('let x = format!("scoreboard objectives add {name} trigger");')


def test_the_cli_exits_zero_on_the_committed_tree():
    proc = subprocess.run(
        ["python3", str(GATE)], capture_output=True, text=True, cwd=str(REPO)
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "check-skill-page: ok" in proc.stdout


# ------------------------------------ rule 21, the tree the page ships from --


def test_a_page_naming_a_tool_that_is_not_in_the_tree_reds(mod, tree, engine):
    """The shape a `tools/` reorganisation walks into: the tool moved, the page did not."""
    edit(
        tree / "references" / "new-pieces.md",
        "$DELVEWRIGHT_ENGINE/tools/creator/block-appearance.py",
        "$DELVEWRIGHT_ENGINE/tools/render/block-appearance.py",
    )
    rep = run(mod, engine)
    assert has(rep, "`$DELVEWRIGHT_ENGINE/tools/render/block-appearance.py` is named by")
    assert has(rep, "does not carry it")


def test_the_finding_names_the_file_and_line_that_named_it(mod, tree, engine):
    page = tree / "references" / "new-pieces.md"
    edit(
        page,
        "$DELVEWRIGHT_ENGINE/tools/creator/block-appearance.py",
        "$DELVEWRIGHT_ENGINE/tools/gone.py",
    )
    line = next(
        i
        for i, text in enumerate(page.read_text(encoding="utf-8").split("\n"), 1)
        if "$DELVEWRIGHT_ENGINE/tools/gone.py" in text
    )
    rep = run(mod, engine)
    assert any(f"new-pieces.md:{line}" in f for f in rep.findings), rep.findings


def test_a_directory_only_ignore_pattern_is_read_as_git_reads_it(mod):
    """`validation/delve-output*/` matches a directory, and git must be asked so."""
    assert mod.produced(["validation/delve-output"]) == {"validation/delve-output"}
    assert mod.produced(["tools/creator/refimg.py"]) == set()
    assert mod.produced(["tools/gone.py"]) == set()
    assert mod.produced([]) == set()


def test_the_tracked_set_is_the_index_and_carries_directories(mod):
    tracked = mod.shipping_tree()
    assert "tools/creator/refimg.py" in tracked
    assert "tools" in tracked and "tools/lib" in tracked
    assert "validation/delve-output" not in tracked


def test_a_placeholder_segment_is_not_read_as_a_path(mod, tree, engine):
    named = mod.engine_paths(mod.shipped())
    assert "validation/run-out/<id>/run-report.json" in named
    assert "…" in named
    rep = run(mod, engine)
    assert not any("run-out/<id>" in f for f in rep.findings), rep.findings


def test_the_terminators_read_the_page_s_own_spellings(mod):
    got = mod.ENGINE_PATH_RE.search('"$DELVEWRIGHT_ENGINE/target/release:$PATH"')
    assert got.group("path") == "/target/release"
    got = mod.ENGINE_PATH_RE.search("`$DELVEWRIGHT_ENGINE/versions.toml`")
    assert got.group("path") == "/versions.toml"
    got = mod.ENGINE_PATH_RE.search("read $DELVEWRIGHT_ENGINE/CLAUDE.md.")
    assert got.group("path") == "/CLAUDE.md."
    got = mod.ENGINE_PATH_RE.search('"${DELVEWRIGHT_ENGINE}/tools/creator/refimg.py"')
    assert got.group("path") == "/tools/creator/refimg.py"


def test_both_arms_bind_to_every_path_the_page_names(mod, tree, engine):
    rep = run(mod, engine)
    bound = [b for b in rep.bindings if "engine path(s) held to" in b[0]]
    assert len(bound) == 2, rep.bindings
    assert any("ships from" in what for what, _b, _o in bound), bound
    assert any("the pin names" in what for what, _b, _o in bound), bound
    for _what, judged, of in bound:
        assert of == len(mod.engine_paths(mod.shipped()))
        assert judged > 0 and judged <= of


def test_the_pin_arm_reds_when_the_pinned_tree_lacks_a_named_script(
    mod, tree, engine, tmp_path
):
    """The property rule 21's second arm exists to hold, tested by removing it.

        `validation/world-save.sh` is a path the page names and this tree
    carries. The perturbation is a real git tree — built through a private index so
    this repository's own is untouched — with that one path removed, handed in
    as the object the pin resolves to. Only the PIN arm may red: the shipping
    tree still carries the file, which is exactly the state that used to be
    green.
    """
    removed = "validation/world-save.sh"
    assert removed in mod.shipping_tree()
    index = tmp_path / "perturbed.index"
    env = {**os.environ, "GIT_INDEX_FILE": str(index)}

    def git(*args: str) -> str:
        out = subprocess.run(
            ["git", "-C", str(mod.REPO), *args],
            capture_output=True,
            text=True,
            env=env,
            check=True,
        )
        return out.stdout.strip()

    git("read-tree", "HEAD")
    git("update-index", "--force-remove", removed)
    perturbed = git("write-tree")
    assert removed not in mod.pinned_tree(perturbed)
    assert removed in mod.shipping_tree(), "the real index must be untouched"

    rep = mod.Report()
    mod.engine_paths_rule(rep, perturbed, "delvec--v0.0.0", True)
    named = [f for f in rep.findings if removed in f]
    assert len(named) == 1, rep.findings
    assert "the tree the pin names" in named[0], named[0]
    assert "the tree the page ships from does not carry it" not in named[0]


# ------------------------------- rule 13, the plugin's own tag asked of the remote --


def _tag_commit(mod, tag: str) -> str:
    return subprocess.run(
        ["git", "-C", str(mod.REPO), "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}"],
        capture_output=True, text=True, check=True,
    ).stdout.strip()


def _plugin_online(mod, tag: str, exists: bool, answers: dict):
    def fetch(path: str):
        if path in answers:
            return answers[path]
        raise mod.NotFound(path)
    rep = mod.Report()
    mod.plugin_online(rep, "o/r", tag, exists, fetch=fetch)
    return rep


def test_an_unborn_plugin_tag_absent_on_the_remote_holds(mod):
    rep = _plugin_online(mod, "delvewright--v99.0.0", False, {})
    assert rep.findings == [], rep.findings
    assert rep.bindings == [("unborn plugin release tag(s) confirmed absent on o/r", 1, 1)]


def test_a_plugin_tag_only_this_machine_has_reds(mod):
    tag, _version = _newest_plugin_tag(mod)
    rep = _plugin_online(mod, tag, True, {})
    assert any("only this machine has" in f for f in rep.findings), rep.findings


def test_a_plugin_tag_that_appeared_on_the_remote_reds(mod):
    rep = _plugin_online(
        mod, "delvewright--v99.0.0", False,
        {"repos/o/r/git/ref/tags/delvewright--v99.0.0": {"object": {"type": "commit", "sha": "0" * 40}}},
    )
    assert any("now carries the plugin tag" in f for f in rep.findings), rep.findings


def test_a_released_plugin_tag_with_a_published_release_holds(mod):
    tag, _version = _newest_plugin_tag(mod)
    sha = _tag_commit(mod, tag)
    rep = _plugin_online(
        mod, tag, True,
        {
            f"repos/o/r/git/ref/tags/{tag}": {"object": {"type": "commit", "sha": sha}},
            f"repos/o/r/releases/tags/{tag}": {"draft": False, "assets": []},
        },
    )
    assert rep.findings == [], rep.findings
    assert rep.bindings == [("plugin release tag(s) of o/r held to the tree they name", 1, 1)]


def test_a_released_plugin_tag_at_another_commit_reds(mod):
    tag, _version = _newest_plugin_tag(mod)
    rep = _plugin_online(
        mod, tag, True,
        {
            f"repos/o/r/git/ref/tags/{tag}": {"object": {"type": "commit", "sha": "f" * 40}},
            f"repos/o/r/releases/tags/{tag}": {"draft": False, "assets": []},
        },
    )
    assert any("The entry delivers the remote's tree" in f for f in rep.findings), rep.findings


@pytest.mark.parametrize("release", [None, {"draft": True, "assets": []}])
def test_a_released_plugin_tag_with_no_published_release_reds(mod, release):
    """A tag standing alone is a release run that stopped after its tag write."""
    tag, _version = _newest_plugin_tag(mod)
    sha = _tag_commit(mod, tag)
    answers = {f"repos/o/r/git/ref/tags/{tag}": {"object": {"type": "commit", "sha": sha}}}
    if release is not None:
        answers[f"repos/o/r/releases/tags/{tag}"] = release
    rep = _plugin_online(mod, tag, True, answers)
    assert any("no published Release at it" in f for f in rep.findings), rep.findings


def _tagged_pin(mod, engine, tmp_path, monkeypatch):
    """`(engine tree, pin tag, the tag's commit)` in the state the shelf half judges.

    The shelf half runs only where the pinned tag EXISTS. Between a version
    move and its release the pin names this tree's own unborn tag (ADR-0029
    §3), a legitimate state in which the half has nothing to ask — so there the
    tag is made to exist: a scratch repository standing in for this checkout
    carries it, at a commit of its own, and the remote answers below name the
    same commit. The engine judged is the one the pin resolves to either way.
    """
    engine_root, _rev, tag_exists = engine
    _repo, ref = mod.read_pin()
    if tag_exists:
        return engine_root, ref, _tag_commit(mod, ref)
    scratch = tmp_path / "checkout"
    scratch.mkdir()
    git = ["git", "-C", str(scratch), "-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"]
    subprocess.run([*git, "init", "-q"], check=True)
    subprocess.run([*git, "commit", "-q", "--allow-empty", "-m", "the release commit"], check=True)
    subprocess.run([*git, "tag", ref], check=True)
    monkeypatch.setattr(mod, "REPO", scratch)
    return engine_root, ref, _tag_commit(mod, ref)


def test_a_draft_engine_release_reds(mod, engine, tmp_path, monkeypatch):
    """Rule 13's shelf half: a draft is served to nobody, so a shelf behind it is
    no shelf."""
    engine_root, ref, sha = _tagged_pin(mod, engine, tmp_path, monkeypatch)
    targets = mod.tomllib.loads((engine_root / "versions.toml").read_text(encoding="utf-8"))["engine"]["targets"]
    version = mod.engine_version(engine_root / "Cargo.toml")
    assets = [{"name": mod.ARCHIVE.format(version=version, target=t)} for t in targets] + [{"name": mod.CHECKSUMS}]
    answers = {
        f"repos/o/r/git/ref/tags/{ref}": {"object": {"type": "commit", "sha": sha}},
        f"repos/o/r/commits/{sha}/branches-where-head": [],
        f"repos/o/r/releases/tags/{ref}": {"draft": True, "assets": assets},
    }
    def fetch(path: str):
        if path in answers:
            return answers[path]
        raise mod.NotFound(path)
    rep = mod.Report()
    mod.online(rep, engine_root, "o/r", ref, version, True, fetch=fetch)
    assert any("is a DRAFT" in f for f in rep.findings), rep.findings
    answers[f"repos/o/r/releases/tags/{ref}"]["draft"] = False
    rep = mod.Report()
    mod.online(rep, engine_root, "o/r", ref, version, True, fetch=fetch)
    assert rep.findings == [], rep.findings
