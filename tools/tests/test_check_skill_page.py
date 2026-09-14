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


def test_the_rule_reaches_the_reference_that_actually_runs_the_ladder(mod, tree, engine):
    """**`references/walk.md` is inside rule 16's population, by name.**

    The rule and the file have different authors: the rule was written where
    `SKILL.md` invokes Compose, and `walk.md` — the page that brings the server
    up for the walk — was rewritten afterwards. The test above would stay green
    if that rewrite had moved walk.md's `docker compose` line out of a fence or
    behind a variable, because SKILL.md alone still satisfies it, and the rule
    would then be saying nothing about the one step whose whole ladder is built
    on Compose.

    So this asserts the population rather than the refusal: the finding names
    walk.md, which it can only do if walk.md really invokes an acquired program
    in a fenced span the rule reads.
    """
    edit(tree / "SKILL.md", "\ndocker compose version", "\n# (nothing here)")
    init = tree / "references" / "init.md"
    text = init.read_text(encoding="utf-8")
    assert "docker compose version" in text
    init.write_text(text.replace("docker compose version", "docker info"), "utf-8")
    rep = run(mod, engine)
    assert has(rep, "references/walk.md invokes `docker compose`"), rep.findings


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


# ------------------------------------------- only the release moves the version --


def _plugin_repo(tmp_path: pathlib.Path, mod, version: str) -> pathlib.Path:
    """A repository of its own, holding a plugin root at the real layout.

    The rule's subject is a git HISTORY, so no perturbation of a copied tree can
    reach it — this is the smallest thing that can be a base and a head.
    """
    repo = tmp_path / "repo"
    plugin = repo / ".claude" / "skills" / "delvewright"
    (plugin / ".claude-plugin").mkdir(parents=True)
    (plugin / ".claude-plugin" / "plugin.json").write_text(
        json.dumps({"name": "delvewright", "version": version}) + "\n", encoding="utf-8"
    )
    (plugin / "page.md").write_text("the page, as it was\n", encoding="utf-8")
    for args in (
        ["init"],
        ["config", "user.email", "t@example.invalid"],
        ["config", "user.name", "t"],
        ["add", "-A"],
        ["commit", "-m", "base"],
    ):
        r = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)
        assert r.returncode == 0, r.stderr
    return repo


def _bump(plugin: pathlib.Path, version: str, **extra) -> None:
    (plugin / ".claude-plugin" / "plugin.json").write_text(
        json.dumps({"name": "delvewright", "version": version, **extra}) + "\n", encoding="utf-8"
    )


RELEASE = {"event": "workflow_dispatch", "ref": "refs/heads/release/plugin-1.2.0"}


def test_a_pull_request_that_bumps_the_version_reds(mod, tmp_path):
    """The perturbation: an ordinary change moving `plugin.json` `version`."""
    repo = _plugin_repo(tmp_path, mod, "1.1.0")
    plugin = repo / ".claude" / "skills" / "delvewright"
    _bump(plugin, "1.2.0")
    rep = mod.Report()
    mod.version_move_rule(rep, "HEAD", "1.2.0", repo=repo, plugin_root=plugin, event="pull_request", ref="refs/heads/topic")
    assert any("moves from '1.1.0' to '1.2.0'" in f and "pull_request" in f for f in rep.findings), rep.findings


def test_a_page_edit_with_no_bump_holds(mod, tmp_path):
    repo = _plugin_repo(tmp_path, mod, "1.2.0")
    plugin = repo / ".claude" / "skills" / "delvewright"
    (plugin / "page.md").write_text("the page, edited\n", encoding="utf-8")
    rep = mod.Report()
    mod.version_move_rule(rep, "HEAD", "1.2.0", repo=repo, plugin_root=plugin, event="pull_request", ref="refs/heads/topic")
    assert rep.findings == [], rep.findings


def test_the_release_commit_holds(mod, tmp_path):
    repo = _plugin_repo(tmp_path, mod, "1.1.0")
    plugin = repo / ".claude" / "skills" / "delvewright"
    _bump(plugin, "1.2.0")
    rep = mod.Report()
    mod.version_move_rule(rep, "HEAD", "1.2.0", repo=repo, plugin_root=plugin, **RELEASE)
    assert rep.findings == [], rep.findings


def test_a_release_branch_that_also_edits_the_page_reds(mod, tmp_path):
    repo = _plugin_repo(tmp_path, mod, "1.1.0")
    plugin = repo / ".claude" / "skills" / "delvewright"
    _bump(plugin, "1.2.0")
    (plugin / "page.md").write_text("the page, edited\n", encoding="utf-8")
    rep = mod.Report()
    mod.version_move_rule(rep, "HEAD", "1.2.0", repo=repo, plugin_root=plugin, **RELEASE)
    assert any("2 file(s) under the plugin root" in f for f in rep.findings), rep.findings


def test_a_release_commit_that_changes_another_manifest_field_reds(mod, tmp_path):
    repo = _plugin_repo(tmp_path, mod, "1.1.0")
    plugin = repo / ".claude" / "skills" / "delvewright"
    _bump(plugin, "1.2.0", description="changed")
    rep = mod.Report()
    mod.version_move_rule(rep, "HEAD", "1.2.0", repo=repo, plugin_root=plugin, **RELEASE)
    assert any("more than `version`" in f for f in rep.findings), rep.findings


def test_a_dispatch_on_another_branch_reds(mod, tmp_path):
    repo = _plugin_repo(tmp_path, mod, "1.1.0")
    plugin = repo / ".claude" / "skills" / "delvewright"
    _bump(plugin, "1.2.0")
    rep = mod.Report()
    mod.version_move_rule(rep, "HEAD", "1.2.0", repo=repo, plugin_root=plugin, event="workflow_dispatch", ref="refs/heads/main")
    assert any("not refs/heads/release/plugin-1.2.0" in f for f in rep.findings), rep.findings


# ------------------------------------ rule 17, a DW code the pinned engine declares --


def test_a_dw_code_the_pinned_engine_does_not_declare_reds(mod, tree, engine):
    """The shape a page written against a newer engine than it pins has: it names
    a diagnostic the installed engine cannot print. The code is checked absent
    from the materialised engine first, so the red is about the pin."""
    engine_root, _rev = engine
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

WALK_TWO = {
    "$defs": {
        "Verdict": {
            "oneOf": [
                {"const": "passed", "type": "string"},
                {"const": "findings", "type": "string"},
            ]
        }
    },
    "properties": {
        "verdict": {"$ref": "#/$defs/Verdict"},
        "areas": {"type": "array"},
        "findings": {"type": "array"},
    },
}
HELP = (
    "      --stage <STAGE>      Which document. `walk-record` for the walk record; "
    "`<prefab-id>.json` is not a stage; or `all` for every stage document\n"
)


def fake_release(walk_record: dict):
    def delvec(argv):
        if argv == ["schema", "--stage", "all"]:
            return 0, json.dumps({"world": {"properties": {"time": {"enum": ["dusk"]}}}})
        if argv == ["schema", "--help"]:
            return 0, HELP
        if argv == ["schema", "--stage", "walk-record"]:
            return 0, json.dumps(walk_record)
        return 2, ""

    return delvec


def release_rep(mod, walk_record, binary=None):
    rep = mod.Report()
    if binary is None:
        binary = b" ".join(c.encode() for c in mod.page_dw_codes())
    mod.release_binary_rule(rep, fake_release(walk_record), binary, "v0.0.0")
    return rep


def test_a_variant_the_release_does_not_admit_reds(mod, tree):
    """The measured case: the page teaches `verdict: "unwalked"` and a walk record
    of two verdicts refuses it as an unknown variant."""
    rep = release_rep(mod, WALK_TWO)
    assert has(rep, "gives `verdict` the value 'unwalked'"), rep.findings


def test_the_same_page_holds_against_a_release_that_admits_it(mod, tree):
    walk = json.loads(json.dumps(WALK_TWO))
    walk["$defs"]["Verdict"]["oneOf"].append({"const": "unwalked", "type": "string"})
    rep = release_rep(mod, walk)
    assert not has(rep, "gives `verdict`"), rep.findings
    bound, of = {what: (b, n) for what, b, n in rep.bindings}[
        "closed-set value(s) the release's schemas admit"
    ]
    assert of >= 1 and bound == of, rep.bindings


def test_a_field_the_release_does_not_carry_reds(mod, tree):
    """A document fragment whose keys are mostly the release's, naming one that
    is not. The stage is found through the binary's help, not a list here."""
    path = tree / "references" / "walk.md"
    path.write_text(
        path.read_text(encoding="utf-8")
        + '\n`{"verdict": "passed", "areas": [], "walked_by": "a"}`\n',
        encoding="utf-8",
    )
    assert has(release_rep(mod, WALK_TWO), "names the field `walked_by`")


def test_a_fragment_of_mostly_unknown_keys_is_not_read_as_a_document(mod, tree):
    """A text component, a skin palette or a renderer option is not a document,
    and the object decides that: most of its keys are no field at all."""
    path = tree / "references" / "walk.md"
    path.write_text(
        path.read_text(encoding="utf-8")
        + '\n`{"translate": "k", "fallback": "x", "verdict": "passed"}`\n',
        encoding="utf-8",
    )
    rep = release_rep(mod, WALK_TWO)
    assert not has(rep, "names the field `translate`"), rep.findings


def test_a_value_given_to_a_field_some_document_leaves_open_is_not_judged(mod, tree):
    """A name one document closes and another leaves open resolves to a
    candidate, not a match — so the value is not refused on the closed one."""
    walk = json.loads(json.dumps(WALK_TWO))
    walk["properties"]["nested"] = {"properties": {"verdict": {"type": "string"}}}
    rep = release_rep(mod, walk)
    assert not has(rep, "gives `verdict`"), rep.findings


def test_a_dw_code_the_release_binary_does_not_spell_reds(mod, tree):
    """The second method for rule 17, sharing nothing with it: the bytes of the
    checksum-verified binary rather than the source at the tag."""
    codes = sorted(mod.page_dw_codes())
    assert codes, "the page names no DW code, so this rule binds to nothing"
    binary = b" ".join(c.encode() for c in codes[1:])
    rep = release_rep(mod, WALK_TWO, binary)
    assert has(rep, f"name `{codes[0]}`, and the v0.0.0 binary"), rep.findings


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


def test_the_cli_exits_zero_on_the_committed_tree():
    proc = subprocess.run(
        ["python3", str(GATE)], capture_output=True, text=True, cwd=str(REPO)
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "check-skill-page: ok" in proc.stdout
