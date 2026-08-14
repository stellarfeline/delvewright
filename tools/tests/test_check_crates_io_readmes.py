"""The crates.io front-page gate (`tools/check-crates-io-readmes.py`).

The defect this pins, from the field: `crates/compiler/README.md` is rendered
VERBATIM as the front page of `crates.io/crates/delvec`, and it opened with
"The deterministic compiler (spec-0002, ADR-0001/0006/0011)" — a citation the
one reader who has never seen this repository cannot resolve and gains nothing
from. `crates/dsl/README.md` opened on `spec-0001`. Both were rewritten (#388)
with nothing to stop the regression, because CLAUDE.md's *Audience separation in
docs* was held by memory alone.

Two things these tests exist to keep true, and the second is the harder one:

* the six patterns fire on the shapes that actually leaked;
* `DW\\d{4}` does NOT fire on "a stable `DW####` code", which is exactly what
  these pages want to say about the diagnostic surface. A gate that reds honest
  prose teaches its readers to ignore it.

The file set is derived from the manifests, so a crate that becomes publishable
must inherit the gate with no edit — pinned below by deleting a `publish = false`
line and watching a previously-ignored README go red.

These drive the detector over synthetic trees so it keeps failing for the right
reason as the real pages grow. The live set is checked by the CI step.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-crates-io-readmes.py"

PUBLISHED_CARGO = """\
[package]
name = "published-crate"
version = "1.1.0"
readme = "README.md"
"""

PRIVATE_CARGO = """\
[package]
name = "private-crate"
version = "0.0.0"
publish = false
"""

# What an honest page looks like, including the two things it legitimately wants
# to say that are one character away from a forbidden pattern.
CLEAN_PAGE = """\
# published-crate

A deterministic compiler for Minecraft Java Edition adventure maps.

Every problem is reported as a stable `DW####` code with a severity and a path
into the document.

- [Compiler reference](https://github.com/stellarfeline/delvewright/blob/main/docs/reference/compiler.md)
- [Sibling crate](https://crates.io/crates/delvewright-dsl)
- [In-page anchor](#install)
"""


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at a synthetic tree."""
    spec = importlib.util.spec_from_file_location("check_crates_io_readmes", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    monkeypatch.setattr(module, "REPO_ROOT", tmp_path)

    published = tmp_path / "crates" / "published"
    published.mkdir(parents=True)
    (published / "Cargo.toml").write_text(PUBLISHED_CARGO, encoding="utf-8")
    (published / "README.md").write_text(CLEAN_PAGE, encoding="utf-8")

    private = tmp_path / "crates" / "private"
    private.mkdir(parents=True)
    (private / "Cargo.toml").write_text(PRIVATE_CARGO, encoding="utf-8")

    module.PAGE = published / "README.md"
    module.PRIVATE = private
    return module


def page(gate, body: str) -> int:
    gate.PAGE.write_text(body, encoding="utf-8")
    return gate.main()


def test_an_honest_page_passes(gate):
    assert gate.main() == 0


# --- the shapes that actually leaked ---------------------------------------


@pytest.mark.parametrize(
    "line",
    [
        "The deterministic compiler (spec-0002, ADR-0001/0006/0011).",
        "The campaign format defined by spec-0001.",
        "See ADR-0011 for the reasoning.",
        "Unsupported `dsl_version` raises DW0102.",
        "Fixed in task #157.",
        "Rewritten in PR #388.",
    ],
)
def test_an_internal_reference_is_red(gate, line):
    assert page(gate, f"# published-crate\n\n{line}\n") == 1


def test_lowercase_spellings_are_caught_too(gate):
    assert page(gate, "# c\n\nSee adr-0011 and spec-0002.\n") == 1


# --- the deliberate subtlety: the page must be able to say `DW####` ---------


def test_the_placeholder_diagnostic_code_is_not_a_finding(gate):
    """`DW####` is the phrase these pages legitimately want. Four hashes are not
    four digits, which is why the pattern is digit-anchored."""
    assert (
        page(
            gate,
            "# c\n\nEvery problem is a stable `DW####` code, and the complete\n"
            "`DW####` catalogue lives in the reference.\n",
        )
        == 0
    )


def test_ordinary_prose_near_the_patterns_is_not_a_finding(gate):
    """A gate that reds correct prose is worse than no gate."""
    assert (
        page(
            gate,
            "# c\n\nThe schemas are generated from the Rust types, so a document "
            "and the parser\nthat reads it cannot disagree. Rooms come from a "
            "prefab library; point the\ncompiler at one with `--prefabs <dir>`. "
            "A PR is welcome, and task lists are\nnot special here.\n",
        )
        == 0
    )


# --- links: crates.io serves this markdown with no repository around it -----


@pytest.mark.parametrize(
    "target",
    ["docs/reference/compiler.md", "./CHANGELOG.md", "../dsl/README.md", "LICENSE"],
)
def test_a_repo_relative_link_is_red(gate, target):
    assert page(gate, f"# c\n\n[the reference]({target})\n") == 1


@pytest.mark.parametrize(
    "target",
    [
        "https://github.com/stellarfeline/delvewright",
        "http://example.invalid/x",
        "mailto:nobody@example.invalid",
        "#install",
        "//example.invalid/x",
    ],
)
def test_an_absolute_or_anchor_target_is_fine(gate, target):
    assert page(gate, f"# c\n\n[somewhere]({target})\n") == 0


def test_a_relative_image_is_red(gate):
    assert page(gate, "# c\n\n![diagram](docs/img/pipeline.svg)\n") == 1


def test_a_relative_reference_definition_is_red(gate):
    assert page(gate, "# c\n\nSee [the reference][ref].\n\n[ref]: docs/x.md\n") == 1


# --- a fenced block is not a way out ---------------------------------------


def test_a_citation_inside_a_code_fence_is_still_red(gate):
    """A gate a page can step out of with three backticks is not a gate."""
    assert page(gate, "# c\n\n```\ndelvec: error DW0102\n```\n") == 1


# --- the binding: derived from the manifests, and never empty --------------


def test_an_unpublished_crates_readme_is_not_examined(gate):
    """`publish = false` keeps a maintainer-facing README out of the set — its
    reader is standing in the directory, and the audience rule leaves it alone."""
    (gate.PRIVATE / "README.md").write_text(
        "# private-crate\n\nModule map; see spec-0027 and DW0489.\n"
        "[fixtures](fixtures/README.md)\n",
        encoding="utf-8",
    )
    assert gate.main() == 0


def test_a_crate_that_becomes_publishable_inherits_the_gate_with_no_edit(gate):
    """Deleting one `publish = false` line is all it should take."""
    (gate.PRIVATE / "README.md").write_text(
        "# private-crate\n\nSee spec-0027.\n", encoding="utf-8"
    )
    assert gate.main() == 0
    (gate.PRIVATE / "Cargo.toml").write_text(
        '[package]\nname = "private-crate"\nversion = "0.0.0"\nreadme = "README.md"\n',
        encoding="utf-8",
    )
    assert gate.main() == 1


def test_a_readme_key_pointing_elsewhere_is_followed(gate):
    (gate.PAGE.parent / "FRONT.md").write_text(
        "# c\n\nSee ADR-0001.\n", encoding="utf-8"
    )
    (gate.PAGE.parent / "Cargo.toml").write_text(
        '[package]\nname = "published-crate"\nversion = "1.1.0"\nreadme = "FRONT.md"\n',
        encoding="utf-8",
    )
    assert gate.main() == 1


def test_zero_publishable_readmes_exits_2(gate):
    """A green that binds to nothing is not a pass (CLAUDE.md)."""
    (gate.PAGE.parent / "Cargo.toml").write_text(PRIVATE_CARGO, encoding="utf-8")
    assert gate.main() == 2


def test_no_crate_manifests_at_all_exits_2(gate):
    for manifest in (gate.REPO_ROOT / "crates").glob("*/Cargo.toml"):
        manifest.unlink()
    assert gate.main() == 2


def test_a_declared_readme_that_does_not_exist_exits_2(gate):
    gate.PAGE.unlink()
    assert gate.main() == 2


def test_a_workspace_member_outside_the_globbed_directory_exits_2(gate):
    """The derivation globs `crates/*/`. A crate that moves out from under it
    must red loudly, not drop out of the binding count in silence."""
    (gate.REPO_ROOT / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["crates/published", "vendor/elsewhere"]\n',
        encoding="utf-8",
    )
    assert gate.main() == 2


# --- the report names the file and the pattern ------------------------------


def test_the_report_names_the_file_the_line_and_what_was_found(gate, capsys):
    assert page(gate, "# c\n\nfiller\nThe compiler (spec-0002).\n") == 1
    err = capsys.readouterr().err
    assert "crates/published/README.md:4" in err
    assert "spec-0002" in err
    assert "published-crate" in err


def test_the_ok_line_states_the_binding_count(gate, capsys):
    assert gate.main() == 0
    out = capsys.readouterr().out
    assert "1 published page(s) examined" in out
    assert "crates/published/README.md" in out


def test_a_publishable_crate_with_no_readme_at_all_is_named_not_dropped(gate, capsys):
    """An absent `readme` key auto-detects, and a crate that simply has none is
    not a broken declaration — but it IS outside this gate's reach, so the
    binding count has to say so instead of quietly shrinking."""
    (gate.PRIVATE / "Cargo.toml").write_text(
        '[package]\nname = "private-crate"\nversion = "0.0.0"\n', encoding="utf-8"
    )
    assert gate.main() == 0
    out = capsys.readouterr().out
    assert "1 publishable crate(s) serve NO page" in out
    assert "private-crate" in out
