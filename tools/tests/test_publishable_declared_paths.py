"""The publishable-crate derivation's cross-check against the root manifest.

`tools/lib/publishable.py` globs `crates/*/Cargo.toml` and then asks the root
manifest whether it declares any path the glob did not reach. That question is
what stops a crate moving out from under `crates/` and silently leaving both
gates (`check-crates-io-readmes.py`, `check-reference-versions.py`) with a
smaller binding count and nothing to say about it.

The cross-check used to demand that EVERY declared path be under `crates/*/`,
which made `exclude = ["prefabs"]` — the line that keeps the prefab generators
out of `delvec`'s resolution — a derivation error. The repair is not an
exemption by name: what the accepted path must supply is a property the defect
cannot, namely that no `Cargo.toml` at or under it declares a package crates.io
could serve.

These tests drive the derivation over synthetic trees, so the rule keeps failing
for the right reason and keeps passing for the right one.
"""

import importlib.util
import pathlib
import sys

import pytest

MODULE = pathlib.Path(__file__).resolve().parents[1] / "lib" / "publishable.py"

PUBLISHED = """\
[package]
name = "published-crate"
version = "1.1.0"
"""

PRIVATE = """\
[package]
name = "{name}"
version = "0.0.0"
publish = false
"""

VIRTUAL = """\
[workspace]
members = ["gen"]
"""


@pytest.fixture
def publishable():
    spec = importlib.util.spec_from_file_location("publishable", MODULE)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    # Registered before it is executed: `@dataclass` resolves a field's
    # annotation through `sys.modules[cls.__module__]`, and a module that is
    # not there yet makes the decorator raise on an unrelated line.
    sys.modules[spec.name] = module
    try:
        spec.loader.exec_module(module)
        yield module
    finally:
        sys.modules.pop(spec.name, None)


def tree(root: pathlib.Path, root_manifest: str) -> pathlib.Path:
    """A minimal repo: one publishable crate under `crates/`, plus whatever the
    root manifest declares."""
    (root / "Cargo.toml").write_text(root_manifest, encoding="utf-8")
    crate = root / "crates" / "published"
    crate.mkdir(parents=True)
    (crate / "Cargo.toml").write_text(PUBLISHED, encoding="utf-8")
    (crate / "README.md").write_text("# published-crate\n", encoding="utf-8")
    return root


def side_workspace(root: pathlib.Path, manifest: str) -> None:
    """A nested workspace beside `crates/`, holding one package."""
    gen = root / "tools-ws" / "gen"
    gen.mkdir(parents=True)
    (root / "tools-ws" / "Cargo.toml").write_text(VIRTUAL, encoding="utf-8")
    (gen / "Cargo.toml").write_text(manifest, encoding="utf-8")


ROOT_EXCLUDING = """\
[workspace]
members = ["crates/published"]
exclude = ["tools-ws"]
"""


def test_an_excluded_workspace_of_unpublished_packages_is_accepted(
    tmp_path, publishable
):
    root = tree(tmp_path, ROOT_EXCLUDING)
    side_workspace(root, PRIVATE.format(name="gen"))
    names = [c.name for c in publishable.discover(root)]
    assert names == ["published-crate"]


def test_a_publishable_package_under_an_excluded_path_is_a_derivation_error(
    tmp_path, publishable
):
    """The shape the cross-check exists to refuse: a crate crates.io could serve,
    sitting where the glob cannot see it, kept out of both gates by one
    `exclude` line."""
    root = tree(tmp_path, ROOT_EXCLUDING)
    side_workspace(root, PUBLISHED)
    with pytest.raises(publishable.DerivationError) as exc:
        publishable.discover(root)
    assert "published-crate" in str(exc.value)


def test_a_declared_path_that_is_not_there_is_a_derivation_error(
    tmp_path, publishable
):
    root = tree(tmp_path, ROOT_EXCLUDING)
    with pytest.raises(publishable.DerivationError):
        publishable.discover(root)


def test_an_unparsable_manifest_under_an_excluded_path_is_not_absence(
    tmp_path, publishable
):
    """An unreadable declaration is not evidence that there is nothing to
    declare, so it counts as publishable and reds."""
    root = tree(tmp_path, ROOT_EXCLUDING)
    side_workspace(root, "this is not = [toml\n")
    with pytest.raises(publishable.DerivationError):
        publishable.discover(root)


def test_a_member_crate_outside_the_glob_still_reds(tmp_path, publishable):
    """The original rule, unweakened: a MEMBER that is not under `crates/*/` and
    is publishable is exactly the crate that escaped."""
    root = tree(
        tmp_path,
        '[workspace]\nmembers = ["crates/published", "tools-ws/gen"]\n',
    )
    side_workspace(root, PUBLISHED)
    with pytest.raises(publishable.DerivationError):
        publishable.discover(root)
