"""The one-authority gate for anchor providers (`tools/check-anchor-providers.py`).

The drift this pins: *what anchors does this campaign have* was answered by
eleven hand-rolled walks over `world.areas` in `dsl::validate`. When the site
plan became a second placement authority, one walk learned that a derived world
synthesizes its anchors and ten did not — so every stage-5 verb but the ones that
walk became unauthorable on a derived map, and **nothing was red**, because a
check resolving against a truncated world refuses CONTENT rather than itself.

These tests drive the gate over a synthetic crate rather than the live tree, so
they keep failing for the right reason as `dsl::validate` grows.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-anchor-providers.py"

REGISTRY_RS = """
pub trait AnchorRegistry {
    fn anchors_for(&self, prefab: &PrefabId) -> Option<&BTreeSet<String>>;
}

impl AnchorRegistry for VendoredAnchorRegistry {
    fn anchors_for(&self, prefab: &PrefabId) -> Option<&BTreeSet<String>> {
        self.by_id.get(prefab)
    }
}
"""

# The repaired shape: one broad answer, in the one place that may hold it.
GOOD_VALIDATE_RS = """
impl AnchorProviders {
    pub(crate) fn build(c: &Campaign, anchors: &dyn AnchorRegistry) -> Self {
        for a in &c.world.content.areas {
            if let Some(set) = anchors.anchors_for(prefab) {
                per_area.insert(a.id.to_string(), set.clone());
            }
        }
        Self { per_area }
    }
}

fn shortcut_checks(c: &Campaign, providers: &AnchorProviders) {
    let _ = providers.resolvable("anchor/x");
}
"""


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at a synthetic crate."""
    spec = importlib.util.spec_from_file_location("check_anchor_providers", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)

    src = tmp_path / "crates" / "dsl" / "src"
    src.mkdir(parents=True)
    (src / "registry.rs").write_text(REGISTRY_RS, encoding="utf-8")
    (src / "validate.rs").write_text(GOOD_VALIDATE_RS, encoding="utf-8")
    (src / "lib.rs").write_text("pub mod validate;\n", encoding="utf-8")

    monkeypatch.setattr(module, "REPO", tmp_path)
    monkeypatch.setattr(module, "DSL_SRC", src)
    module.SRC = src
    return module


def test_one_authority_passes(gate, capsys):
    assert gate.main() == 0
    out = capsys.readouterr().out
    assert "check-anchor-providers: OK" in out
    # The binding count states its denominator: call sites, and the population
    # of files they were looked for in.
    assert "3 `anchors_for` call site(s) across 2 of 3 file(s)" in out


def test_a_second_walk_in_validate_is_a_finding(gate, capsys):
    """The twelfth copy — the whole reason this gate exists."""
    (gate.SRC / "validate.rs").write_text(
        GOOD_VALIDATE_RS
        + """
fn loot_checks(c: &Campaign, anchors: &dyn AnchorRegistry) {
    for a in &c.world.content.areas {
        if let Some(set) = anchors.anchors_for(prefab) {
            known.extend(set);
        }
    }
}
""",
        encoding="utf-8",
    )
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "calls `anchors_for` 2 times" in err
    assert "AnchorProviders::build" in err
    # It says what to do instead, and what widening the authority is for.
    assert "WIDEN IT" in err


def test_a_walk_in_any_other_file_is_a_finding(gate, capsys):
    (gate.SRC / "siteplan.rs").write_text(
        "fn f(anchors: &dyn AnchorRegistry) { anchors.anchors_for(p); }\n",
        encoding="utf-8",
    )
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "siteplan.rs" in err
    assert "will then be quietly wrong about a whole class of campaign" in err


def test_the_authority_going_missing_is_a_finding(gate, capsys):
    """A call in `validate.rs` with no `AnchorProviders` is a walk by definition."""
    (gate.SRC / "validate.rs").write_text(
        "fn f(anchors: &dyn AnchorRegistry) { anchors.anchors_for(p); }\n",
        encoding="utf-8",
    )
    assert gate.main() == 1
    assert "the one authority this gate exists to protect is gone" in capsys.readouterr().err


def test_zero_call_sites_is_a_failure_not_a_pass(gate, capsys):
    """A renamed trait method would leave this gate guarding nothing, green."""
    (gate.SRC / "registry.rs").write_text("// the method moved\n", encoding="utf-8")
    (gate.SRC / "validate.rs").write_text("impl AnchorProviders {}\n", encoding="utf-8")
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "found 0 `anchors_for` call sites" in err
    assert "Fix the pattern, do not drop the gate" in err


def test_the_registry_may_hold_as_many_as_it_likes(gate):
    """`registry.rs` DECLARES the narrow question; a trait and its impls are not
    copies of a walk."""
    (gate.SRC / "registry.rs").write_text(REGISTRY_RS * 3, encoding="utf-8")
    assert gate.main() == 0
