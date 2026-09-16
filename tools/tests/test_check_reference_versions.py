"""The compiler-reference version-header gate (`tools/check-reference-versions.py`).

The defect this pins, from the field: `docs/reference/compiler.md` is the
authoritative current-behavior record, and its header read `delvec 0.1.0`,
`dsl 0.8.0` while the build was at `delvec 1.1.0` / `dsl 0.9.0`. The BODY of
that same file documented the v0.9 surface correctly; only the header a reader
consults first to pick a stage envelope's `dsl_version` was wrong, and every gate
was green, because no gate related the two.

The second instance, found while fixing the first: the `DW0102` catalog row
restates the accepted number by hand and had gone stale the same way.
`check-dw-codes.py` was green on it and always would be — it proves a code
EXISTS in both source and doc and is asserted by a test, never that the BEHAVIOR
the doc ascribes to it is the behavior the code has.

The direction that matters is STALE-OLDER: docs are written once and the build
moves. A gate that only rejected "newer than the build" is exactly what let a
storybook ship a `v1.0` marker through the whole `v1.1` release green, so
these tests pin BOTH directions as red.

These drive the detector over synthetic sources so it keeps failing for the
right reason as the real files grow. The live set is checked by the CI step.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-reference-versions.py"

DOC_TEMPLATE = """\
# delvec compiler — behavior reference

- Versions (as of this doc): `delvec {delvec}`, `dsl {dsl}`, `mc {mc}`.

| Code | Meaning |
|---|---|
| `DW0102` | The document's `dsl_version` is not the one this engine accepts, `{dw0102}`; the message names it. |
"""

CARGO_TEMPLATE = """\
[package]
name = "delvec"
version = "{version}"
"""

# The constant DERIVES the number from the crate's own package version, so the
# source carries no literal at all and the manifest below is the one authority.
ENVELOPE_TEMPLATE = """\
//! doc
pub const DSL_VERSION: &str = env!("CARGO_PKG_VERSION");
"""

DSL_CARGO_TEMPLATE = """\
[package]
name = "delvewright-dsl"
version = "{dsl}"
publish = false
"""

VERSIONS_TOML_TEMPLATE = """\
[minecraft]
version = "{mc}"
server_jar_sha1 = "deadbeef"

[datapack]
version = "not-the-minecraft-one"
"""

# The crates.io front page. A stranger reads exactly these three compatibility
# facts to decide whether they can use the crate, and every one of them is a
# number this build owns.
README_TEMPLATE = """\
# published-crate

A compiler for **Minecraft Java Edition {mc}** adventure maps. It checks every
command it writes against the vendored {prose_mc} command tree.

## Compatibility

- **Minecraft**: Java Edition {mc}.
- **Campaign format**: `dsl_version` `{dsl}`.
- **Rust**: {rust} or newer.
"""

CRATE_CARGO_TEMPLATE = """\
[package]
name = "published-crate"
version = "{version}"
rust-version = "{rust}"
readme = "README.md"
"""

UNPUBLISHED_CARGO = """\
[package]
name = "private-crate"
version = "0.0.0"
publish = false
"""


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at synthetic sources."""
    spec = importlib.util.spec_from_file_location("check_reference_versions", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    monkeypatch.setattr(module, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(module, "DOC", tmp_path / "compiler.md")
    monkeypatch.setattr(module, "ROOT_CARGO_TOML", tmp_path / "Cargo.toml")
    monkeypatch.setattr(module, "ENVELOPE_RS", tmp_path / "envelope.rs")
    monkeypatch.setattr(module, "DSL_CARGO_TOML", tmp_path / "dsl-Cargo.toml")
    monkeypatch.setattr(module, "VERSIONS_TOML", tmp_path / "versions.toml")
    (tmp_path / "crates" / "published").mkdir(parents=True)
    (tmp_path / "crates" / "private").mkdir(parents=True)
    (tmp_path / "crates" / "private" / "Cargo.toml").write_text(
        UNPUBLISHED_CARGO, encoding="utf-8"
    )
    return module


def run(
    gate,
    *,
    doc_delvec="1.1.0",
    doc_dsl="0.19.0",
    doc_mc="1.21.11",
    doc_dw0102=None,
    real_delvec="1.1.0",
    real_dsl="0.19.0",
    real_mc="1.21.11",
    page_mc=None,
    page_prose_mc=None,
    page_dsl=None,
    page_rust="1.97.1",
    crate_rust="1.97.1",
    crate_version="1.1.0",
    page_text=None,
    crate_manifest=None,
) -> int:
    if doc_dw0102 is None:
        doc_dw0102 = doc_dsl
    crate_dir = gate.REPO_ROOT / "crates" / "published"
    crate_dir.mkdir(parents=True, exist_ok=True)
    (crate_dir / "Cargo.toml").write_text(
        CRATE_CARGO_TEMPLATE.format(version=crate_version, rust=crate_rust)
        if crate_manifest is None
        else crate_manifest,
        encoding="utf-8",
    )
    (crate_dir / "README.md").write_text(
        README_TEMPLATE.format(
            mc=real_mc if page_mc is None else page_mc,
            prose_mc=real_mc if page_prose_mc is None else page_prose_mc,
            dsl=real_dsl if page_dsl is None else page_dsl,
            rust=page_rust,
        )
        if page_text is None
        else page_text,
        encoding="utf-8",
    )
    gate.DOC.write_text(
        DOC_TEMPLATE.format(delvec=doc_delvec, dsl=doc_dsl, mc=doc_mc, dw0102=doc_dw0102),
        encoding="utf-8",
    )
    gate.ROOT_CARGO_TOML.write_text(
        CARGO_TEMPLATE.format(version=real_delvec), encoding="utf-8"
    )
    gate.ENVELOPE_RS.write_text(ENVELOPE_TEMPLATE, encoding="utf-8")
    gate.DSL_CARGO_TOML.write_text(DSL_CARGO_TEMPLATE.format(dsl=real_dsl), encoding="utf-8")
    gate.VERSIONS_TOML.write_text(
        VERSIONS_TOML_TEMPLATE.format(mc=real_mc), encoding="utf-8"
    )
    return gate.main([])


def test_header_matching_the_build_passes(gate):
    assert run(gate) == 0


# --- the stale-older direction: what actually happens ----------------------


def test_stale_delvec_version_is_red(gate):
    assert run(gate, doc_delvec="0.1.0", real_delvec="1.1.0") == 1


def test_stale_dsl_version_is_red(gate):
    """The exact motivating drift: the build moved to 0.19.0, the doc says 0.18.0."""
    assert run(gate, doc_dsl="0.18.0", doc_dw0102="0.18.0", real_dsl="0.19.0") == 1


def test_stale_dw0102_row_alone_is_red(gate, capsys):
    """The second instance: header right, the DW0102 row restating it stale."""
    assert run(gate, doc_dsl="0.19.0", doc_dw0102="0.18.0", real_dsl="0.19.0") == 1
    assert "the `DW0102` catalog row restates the accepted dsl_version" in capsys.readouterr().err


# --- the ahead-of-the-build direction, which a one-sided gate would miss ----


def test_doc_ahead_of_the_build_is_red(gate):
    assert run(gate, doc_delvec="2.0.0", real_delvec="1.1.0") == 1


def test_doc_dsl_ahead_of_the_build_is_red(gate):
    assert run(gate, doc_dsl="0.20.0", doc_dw0102="0.20.0", real_dsl="0.19.0") == 1


# --- the pinned Minecraft version ------------------------------------------


def test_stale_minecraft_pin_is_red(gate):
    assert run(gate, doc_mc="1.21.9", real_mc="1.21.11") == 1


def test_minecraft_version_is_read_from_the_minecraft_table(gate):
    """`versions.toml` has several `version =` keys; only [minecraft]'s counts."""
    assert run(gate, doc_mc="1.21.11", real_mc="1.21.11") == 0


# --- a reshaped source must be loud (exit 2), never quietly green ----------


def test_missing_version_header_exits_2(gate):
    run(gate)
    gate.DOC.write_text("# no version header here\n", encoding="utf-8")
    assert gate.main([]) == 2


def test_a_literal_put_back_in_place_of_the_derivation_exits_2(gate):
    """A literal there would be a SECOND authority, and this gate would then be
    comparing the doc against whichever of the two it happened to read."""
    run(gate)
    gate.ENVELOPE_RS.write_text('pub const DSL_VERSION: &str = "0.19.0";\n', encoding="utf-8")
    assert gate.main([]) == 2


def test_missing_dsl_version_constant_exits_2(gate):
    run(gate)
    gate.ENVELOPE_RS.write_text('pub const SOMETHING_ELSE: &str = "0.19.0";\n', encoding="utf-8")
    assert gate.main([]) == 2


def test_write_moves_the_pages_own_dependency_requirement(gate):
    """The dry run of the next bump found this one: everything else on the page
    moved and the `[dependencies]` snippet did not, which made a published page
    the fourth file a bump edits by hand. Rule 2 admits exactly one value there
    — the crate's current major.minor — so it is a bound claim, not a guess."""
    page = README_TEMPLATE.format(mc="1.21.11", prose_mc="1.21.11", dsl="0.19.0", rust="1.97.1")
    page += '\n```toml\n[dependencies]\npublished-crate = "1.0"\n```\n'
    assert run(gate, page_text=page) == 1, "a stale requirement for the page's own crate is red"
    assert gate.main(["--write"]) == 0, "…and --write moves it, because rule 2 admits one value"
    moved = (gate.REPO_ROOT / "crates" / "published" / "README.md").read_text(encoding="utf-8")
    assert 'published-crate = "1.1"' in moved, moved


def test_write_leaves_another_crates_number_alone(gate):
    """A third-party requirement is not this build's number to move. It still
    reds under rule 2 — nothing here knows what it should say — and `--write`
    leaves it exactly as it found it."""
    page = README_TEMPLATE.format(mc="1.21.11", prose_mc="1.21.11", dsl="0.19.0", rust="1.97.1")
    page += '\n```toml\n[dependencies]\nsomeone-else = "0.3"\n```\n'
    assert run(gate, page_text=page) == 1
    assert gate.main(["--write"]) == 1
    after = (gate.REPO_ROOT / "crates" / "published" / "README.md").read_text(encoding="utf-8")
    assert 'someone-else = "0.3"' in after, after


def test_write_moves_every_bound_claim_and_then_passes(gate):
    """`--write` is what makes these three documents SHAPE 2 rather than three
    more places a person retypes the number: a bump runs it, and the checking
    mode is what refuses a document that was not regenerated."""
    assert run(gate, doc_delvec="0.0.1", doc_dsl="0.0.2", doc_mc="0.0.3") == 1
    assert gate.main(["--write"]) == 0
    assert gate.main([]) == 0
    text = gate.DOC.read_text(encoding="utf-8")
    assert "1.1.0" in text and "0.19.0" in text and "1.21.11" in text
    assert "0.0.1" not in text and "0.0.2" not in text and "0.0.3" not in text


def test_missing_dw0102_row_exits_2(gate):
    run(gate)
    gate.DOC.write_text(
        DOC_TEMPLATE.format(delvec="1.1.0", dsl="0.19.0", mc="1.21.11", dw0102="0.19.0").replace(
            "| `DW0102` |", "| `DW9999` |"
        ),
        encoding="utf-8",
    )
    assert gate.main([]) == 2


def test_a_detached_dw0102_row_is_red(gate):
    """A blank line above the row ends the table: the reader sees a paragraph of
    pipes and the accepted number is stated to nobody."""
    run(gate)
    gate.DOC.write_text(
        DOC_TEMPLATE.format(delvec="1.1.0", dsl="0.19.0", mc="1.21.11", dw0102="0.19.0").replace(
            "|---|---|\n| `DW0102` |", "|---|---|\n\n| `DW0102` |"
        ),
        encoding="utf-8",
    )
    assert gate.main([]) == 1


def test_absent_file_exits_2(gate):
    run(gate)
    gate.VERSIONS_TOML.unlink()
    assert gate.main([]) == 2


# --- the same constants, on the pages a stranger reads ----------------------
#
# `crates/delvec/README.md` and `crates/dsl/README.md` are rendered VERBATIM
# as crates.io front pages. They state the Minecraft version, the `dsl_version`
# and the minimum Rust — the facts that decide whether a visitor can use the
# crate — and those were bound to nothing before this gate. The file set is
# DERIVED from the manifests, never listed.


def test_page_matching_the_build_passes(gate):
    assert run(gate) == 0


def test_stale_minecraft_version_on_the_page_is_red(gate):
    """The direction that happens: the page was written once, `mc` moved."""
    assert run(gate, real_mc="1.21.11", page_mc="1.21.9", page_prose_mc="1.21.9") == 1


def test_page_ahead_of_the_build_is_red(gate):
    assert run(gate, real_mc="1.21.11", page_mc="1.22.0", page_prose_mc="1.22.0") == 1


def test_stale_dsl_version_on_the_page_is_red(gate):
    """The exact live risk: a `dsl_version` bump the front page never heard about."""
    assert run(gate, real_dsl="0.19.0", page_dsl="0.18.0") == 1


def test_page_rust_version_disagreeing_with_the_manifest_is_red(gate):
    assert run(gate, page_rust="1.90.0", crate_rust="1.97.1") == 1


def test_a_page_claiming_a_minimum_rust_the_manifest_never_declares_is_red(gate):
    """A claim with no source is not a claim — `cargo install` is where a
    stranger would otherwise discover it."""
    assert (
        run(
            gate,
            crate_manifest='[package]\nname = "published-crate"\n'
            'version = "1.1.0"\nreadme = "README.md"\n',
        )
        == 1
    )


def test_a_stale_version_in_prose_is_red_where_the_labelled_claims_are_right(gate):
    """Rule 2 is the one that reaches prose. The `delvec` page states the
    Minecraft version three times; only one of them is a compatibility bullet."""
    assert run(gate, real_mc="1.21.11", page_prose_mc="1.21.9") == 1


def test_a_version_literal_the_build_does_not_own_is_red(gate):
    assert (
        run(
            gate,
            page_text=(
                "# published-crate\n\n"
                "## Compatibility\n\n"
                "- **Minecraft**: Java Edition 1.21.11.\n"
                "- **Campaign format**: `dsl_version` `0.19.0`.\n"
                "- **Rust**: 1.97.1 or newer.\n"
                "- Also needs libfoo 3.4.5.\n"
            ),
        )
        == 1
    )


# --- a two-part literal, bare or in a cargo requirement form — BZ ----------
#
# The live defect: `delvewright-dsl` published 0.20.0 while its own front page
# still told a visitor `delvewright-dsl = "0.19"`. `VERSION_LITERAL_RE` already
# matched that `0.19` fine; a `literal.count(".") != 2` filter right after it
# discarded every two-part match before it could be judged, on the theory that
# the only two-part shape on a page was `GPL-3.0-only`. It was not: a cargo
# dependency requirement is two-part BY CONVENTION.


def _readme_with_dependency_line(requirement: str) -> str:
    """A front page with a `[dependencies]` snippet, shaped like the real
    `crates/dsl/README.md` — plus the three labelled claims rule 1 needs."""
    return (
        "# published-crate\n\n"
        "## Use\n\n"
        "```toml\n"
        "[dependencies]\n"
        f'published-crate = "{requirement}"\n'
        "```\n\n"
        "## Compatibility\n\n"
        "- **Minecraft**: Java Edition 1.21.11.\n"
        "- **Campaign format**: `dsl_version` `0.19.0`.\n"
        "- **Rust**: 1.97.1 or newer.\n"
    )


def test_a_stale_two_part_dependency_requirement_is_red(gate, capsys):
    """The exact motivating drift, reproduced: the page's own crate is 0.20.0,
    its `[dependencies]` snippet still says `"0.19"` — named by line."""
    assert (
        run(gate, crate_version="0.20.0", page_text=_readme_with_dependency_line("0.19"))
        == 1
    )
    assert "README.md:7: version literal `0.19`" in capsys.readouterr().err


def test_a_dependency_requirement_at_the_crates_own_major_minor_is_green(gate):
    assert (
        run(gate, crate_version="0.20.0", page_text=_readme_with_dependency_line("0.20"))
        == 0
    )


def test_a_caret_dependency_requirement_at_the_crates_own_major_minor_is_green(gate):
    """An operator prefix is not part of the digit run — `^0.20` yields the
    same literal, `0.20`, as the bare form."""
    assert (
        run(gate, crate_version="0.20.0", page_text=_readme_with_dependency_line("^0.20"))
        == 0
    )


def test_a_prefix_of_the_crates_own_major_minor_is_still_red(gate):
    """`0.2` is a STRING PREFIX of `0.20`, not the same major.minor — a prefix
    is not a match."""
    assert (
        run(gate, crate_version="0.20.0", page_text=_readme_with_dependency_line("0.2"))
        == 1
    )


def test_an_spdx_license_id_is_not_a_version_literal(gate):
    """`GPL-3.0-only` embeds a two-part number, `3.0`, inside a hyphenated
    identifier — a real false positive on all eight published pages today,
    once a two-part literal is no longer discarded by dot-count alone."""
    page = README_TEMPLATE.format(mc="1.21.11", prose_mc="1.21.11", dsl="0.19.0", rust="1.97.1")
    page += "\nLicensed GPL-3.0-only.\n"
    assert run(gate, page_text=page) == 0


# --- the binding: derived, and never allowed to be empty --------------------


def test_an_unpublished_crate_is_not_examined(gate):
    """`publish = false` keeps a maintainer-facing README out of the set — it
    reaches no stranger, and by the audience rule it is correct as it stands."""
    private = gate.REPO_ROOT / "crates" / "private"
    (private / "README.md").write_text(
        "# private-crate\n\nSee spec-0002. Needs 9.9.9.\n", encoding="utf-8"
    )
    assert run(gate) == 0


def test_a_crate_that_becomes_publishable_is_picked_up_with_no_edit(gate):
    """The whole point of deriving the set: deleting one `publish = false` line
    puts a new page under the gate."""
    private = gate.REPO_ROOT / "crates" / "private"
    (private / "Cargo.toml").write_text(
        '[package]\nname = "private-crate"\nversion = "0.0.0"\n'
        'rust-version = "1.97.1"\nreadme = "README.md"\n',
        encoding="utf-8",
    )
    (private / "README.md").write_text(
        "# private-crate\n\n"
        "- **Minecraft**: Java Edition 1.21.11.\n"
        "- **Campaign format**: `dsl_version` `0.19.0`.\n"
        "- **Rust**: 1.97.1 or newer.\n"
        "- Stale: 1.21.9.\n",
        encoding="utf-8",
    )
    assert run(gate) == 1


def test_a_page_that_dropped_its_compatibility_section_exits_2(gate):
    """Not a silent pass: a page that stops stating the facts stops telling a
    stranger the one thing they need."""
    assert run(gate, page_text="# published-crate\n\nA compiler.\n") == 2


def test_zero_publishable_readmes_exits_2(gate):
    """A green that binds to nothing is not a pass (CLAUDE.md)."""
    run(gate)
    (gate.REPO_ROOT / "crates" / "published" / "Cargo.toml").write_text(
        '[package]\nname = "published-crate"\nversion = "1.1.0"\npublish = false\n',
        encoding="utf-8",
    )
    assert gate.main([]) == 2


def test_no_crate_manifests_at_all_exits_2(gate):
    run(gate)
    for manifest in (gate.REPO_ROOT / "crates").glob("*/Cargo.toml"):
        manifest.unlink()
    assert gate.main([]) == 2


def test_a_declared_readme_that_does_not_exist_exits_2(gate):
    run(gate)
    (gate.REPO_ROOT / "crates" / "published" / "README.md").unlink()
    assert gate.main([]) == 2


def test_a_stale_allowlist_entry_is_red(gate):
    """An entry that suppresses nothing must go, or it rots into a licence."""
    gate.UNBOUND_VERSION_LITERALS[("crates/gone/README.md", "9.9.9")] = "stale"
    try:
        assert run(gate) == 1
    finally:
        gate.UNBOUND_VERSION_LITERALS.clear()
