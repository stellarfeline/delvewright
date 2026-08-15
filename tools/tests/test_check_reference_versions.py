"""The compiler-reference version-header gate (`tools/check-reference-versions.py`).

The defect this pins, from the field: `docs/reference/compiler.md` is the
authoritative current-behavior record, and its header read `delvec 0.1.0`,
`dsl 0.8.0` and listed `dsl_version 0.2.0 … 0.8.0` while the build was at
`delvec 1.1.0` / `dsl 0.9.0` and accepted `0.9.0`. The BODY of that same file
documented the v0.9 surface correctly; only the header a reader consults first
to pick a stage envelope's `dsl_version` was wrong, and every gate was green,
because no gate related the two.

The second instance, found while fixing the first: the `DW0102` catalog row
restates the supported set by hand and had gone stale the same way.
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
  Supported campaign `dsl_version`: **{supported}** (additive supersets).

| Code | Meaning |
|---|---|
| `DW0102` | Unsupported `dsl_version` (not in `{{{dw0102}}}`). |
"""

CARGO_TEMPLATE = """\
[package]
name = "delvec"
version = "{version}"
"""

ENVELOPE_TEMPLATE = """\
//! doc
pub const SUPPORTED_DSL_VERSION: &str = "{latest}";
pub const SUPPORTED_DSL_VERSIONS: &[&str] = &[
    {list}
];
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
- **Campaign format**: `dsl_version` `{lo}` through `{hi}`.
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
    monkeypatch.setattr(module, "COMPILER_CARGO_TOML", tmp_path / "Cargo.toml")
    monkeypatch.setattr(module, "ENVELOPE_RS", tmp_path / "envelope.rs")
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
    doc_dsl="0.9.0",
    doc_mc="1.21.11",
    doc_supported=("0.8.0", "0.9.0"),
    doc_dw0102=None,
    real_delvec="1.1.0",
    real_dsl="0.9.0",
    real_mc="1.21.11",
    real_supported=("0.8.0", "0.9.0"),
    page_mc=None,
    page_prose_mc=None,
    page_lo=None,
    page_hi=None,
    page_rust="1.97.1",
    crate_rust="1.97.1",
    crate_version="1.1.0",
    page_text=None,
    crate_manifest=None,
) -> int:
    if doc_dw0102 is None:
        doc_dw0102 = doc_supported
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
            lo=real_supported[0] if page_lo is None else page_lo,
            hi=real_supported[-1] if page_hi is None else page_hi,
            rust=page_rust,
        )
        if page_text is None
        else page_text,
        encoding="utf-8",
    )
    gate.DOC.write_text(
        DOC_TEMPLATE.format(
            delvec=doc_delvec,
            dsl=doc_dsl,
            mc=doc_mc,
            supported="**" + ", ".join(f"`{v}`" for v in doc_supported) + "**",
            dw0102=",".join(doc_dw0102),
        ),
        encoding="utf-8",
    )
    gate.COMPILER_CARGO_TOML.write_text(
        CARGO_TEMPLATE.format(version=real_delvec), encoding="utf-8"
    )
    gate.ENVELOPE_RS.write_text(
        ENVELOPE_TEMPLATE.format(
            latest=real_dsl, list=", ".join(f'"{v}"' for v in real_supported)
        ),
        encoding="utf-8",
    )
    gate.VERSIONS_TOML.write_text(
        VERSIONS_TOML_TEMPLATE.format(mc=real_mc), encoding="utf-8"
    )
    return gate.main()


def test_header_matching_the_build_passes(gate):
    assert run(gate) == 0


# --- the stale-older direction: what actually happens ----------------------


def test_stale_delvec_version_is_red(gate):
    assert run(gate, doc_delvec="0.1.0", real_delvec="1.1.0") == 1


def test_stale_dsl_version_is_red(gate):
    assert run(gate, doc_dsl="0.8.0", real_dsl="0.9.0") == 1


def test_stale_supported_list_is_red(gate):
    """The exact motivating drift: the build accepts 0.9.0, the doc lists to 0.8.0."""
    assert run(gate, doc_supported=("0.8.0",), real_supported=("0.8.0", "0.9.0")) == 1


def test_stale_dw0102_row_alone_is_red(gate):
    """The second instance: header right, the DW0102 row restating it stale."""
    assert (
        run(
            gate,
            doc_supported=("0.8.0", "0.9.0"),
            doc_dw0102=("0.8.0",),
            real_supported=("0.8.0", "0.9.0"),
        )
        == 1
    )


# --- the ahead-of-the-build direction, which a one-sided gate would miss ----


def test_doc_ahead_of_the_build_is_red(gate):
    assert run(gate, doc_delvec="2.0.0", real_delvec="1.1.0") == 1


def test_supported_list_naming_a_version_the_build_rejects_is_red(gate):
    assert run(gate, doc_supported=("0.8.0", "0.9.0", "1.0.0")) == 1


# --- ordering, and the pinned Minecraft version ----------------------------


def test_same_members_wrong_order_is_red(gate):
    """The list doubles as the reading order for the additive-superset claim."""
    assert run(gate, doc_supported=("0.9.0", "0.8.0")) == 1


def test_stale_minecraft_pin_is_red(gate):
    assert run(gate, doc_mc="1.21.9", real_mc="1.21.11") == 1


def test_minecraft_version_is_read_from_the_minecraft_table(gate):
    """`versions.toml` has several `version =` keys; only [minecraft]'s counts."""
    assert run(gate, doc_mc="1.21.11", real_mc="1.21.11") == 0


# --- a reshaped source must be loud (exit 2), never quietly green ----------


def test_missing_version_header_exits_2(gate):
    run(gate)
    gate.DOC.write_text("# no version header here\n", encoding="utf-8")
    assert gate.main() == 2


def test_missing_supported_constant_exits_2(gate):
    run(gate)
    gate.ENVELOPE_RS.write_text(
        'pub const SUPPORTED_DSL_VERSION: &str = "0.9.0";\n', encoding="utf-8"
    )
    assert gate.main() == 2


def test_missing_dw0102_row_exits_2(gate):
    run(gate)
    gate.DOC.write_text(
        DOC_TEMPLATE.format(
            delvec="1.1.0",
            dsl="0.9.0",
            mc="1.21.11",
            supported="**`0.8.0`, `0.9.0`**",
            dw0102="0.8.0,0.9.0",
        ).replace("| `DW0102` |", "| `DW9999` |"),
        encoding="utf-8",
    )
    assert gate.main() == 2


def test_absent_file_exits_2(gate):
    run(gate)
    gate.VERSIONS_TOML.unlink()
    assert gate.main() == 2


# --- the same constants, on the pages a stranger reads ----------------------
#
# `crates/compiler/README.md` and `crates/dsl/README.md` are rendered VERBATIM
# as crates.io front pages. They state the Minecraft version, the `dsl_version`
# window and the minimum Rust — the facts that decide whether a visitor can use
# the crate — and those were bound to nothing before this gate. The file set is
# DERIVED from the manifests, never listed.


def test_page_matching_the_build_passes(gate):
    assert run(gate) == 0


def test_stale_minecraft_version_on_the_page_is_red(gate):
    """The direction that happens: the page was written once, `mc` moved."""
    assert run(gate, real_mc="1.21.11", page_mc="1.21.9", page_prose_mc="1.21.9") == 1


def test_page_ahead_of_the_build_is_red(gate):
    assert run(gate, real_mc="1.21.11", page_mc="1.22.0", page_prose_mc="1.22.0") == 1


def test_stale_dsl_version_window_on_the_page_is_red(gate):
    """The exact live risk: a `dsl_version` bump the front page never heard about."""
    assert (
        run(gate, real_supported=("0.8.0", "0.9.0"), page_hi="0.8.0", page_lo="0.8.0")
        == 1
    )


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
                "- **Campaign format**: `dsl_version` `0.8.0` through `0.9.0`.\n"
                "- **Rust**: 1.97.1 or newer.\n"
                "- Also needs libfoo 3.4.5.\n"
            ),
        )
        == 1
    )


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
        "- **Campaign format**: `dsl_version` `0.8.0` through `0.9.0`.\n"
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
    assert gate.main() == 2


def test_no_crate_manifests_at_all_exits_2(gate):
    run(gate)
    for manifest in (gate.REPO_ROOT / "crates").glob("*/Cargo.toml"):
        manifest.unlink()
    assert gate.main() == 2


def test_a_declared_readme_that_does_not_exist_exits_2(gate):
    run(gate)
    (gate.REPO_ROOT / "crates" / "published" / "README.md").unlink()
    assert gate.main() == 2


def test_a_stale_allowlist_entry_is_red(gate):
    """An entry that suppresses nothing must go, or it rots into a licence."""
    gate.UNBOUND_VERSION_LITERALS[("crates/gone/README.md", "9.9.9")] = "stale"
    try:
        assert run(gate) == 1
    finally:
        gate.UNBOUND_VERSION_LITERALS.clear()
