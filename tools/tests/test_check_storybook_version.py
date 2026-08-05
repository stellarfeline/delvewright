"""The storybook engine-version gate (`tools/check-storybook-version.py`).

The drift this pins (owner directive, task #147): a campaign's storybook is what
a server host reads before running the delve, and the one internal fact it is
allowed to carry — which engine the delve needs — is a hand-typed number. Hand-
typed numbers go stale the moment a campaign adopts a new `dsl_version`, and a
stale one is worse than none: it tells a host on an old engine to go ahead.

These tests drive the gate over synthetic campaign trees rather than the live
content repo, so they keep failing for the right reason as real campaigns come
and go (and while the real ones are still allowlisted behind their open PRs).
"""

import importlib.util
import json
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-storybook-version.py"

ENGINE_DELVEC = "0.1.0"


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at a synthetic repo + content."""
    spec = importlib.util.spec_from_file_location("check_storybook_version", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)

    cargo_toml = tmp_path / "crates" / "compiler" / "Cargo.toml"
    cargo_toml.parent.mkdir(parents=True)
    cargo_toml.write_text(
        f'[package]\nname = "delvewright-compiler"\nversion = "{ENGINE_DELVEC}"\n'
        'edition = "2024"\n',
        encoding="utf-8",
    )
    root = tmp_path / "content" / "campaigns"
    root.mkdir(parents=True)

    monkeypatch.setattr(module, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(module, "COMPILER_CARGO_TOML", cargo_toml)
    monkeypatch.setattr(module, "DEFAULT_CAMPAIGNS_ROOT", root)
    monkeypatch.setattr(module, "ALLOWLIST", {})
    module.ROOT = root
    return module


def make_campaign(
    gate,
    name: str,
    dsl_versions: dict[str, str] | str = "0.9.0",
    languages: list[str] | None = None,
    readmes: dict[str, str] | None = None,
) -> pathlib.Path:
    """A synthetic campaign: stage docs + whatever storybooks were asked for."""
    campaign = gate.ROOT / name
    campaign.mkdir(parents=True, exist_ok=True)
    if isinstance(dsl_versions, str):
        dsl_versions = {stage: dsl_versions for stage in gate.STAGE_FILES}
    for stage, version in dsl_versions.items():
        doc = {"dsl_version": version, "campaign_id": name, "stage": stage.split(".")[0]}
        if stage == "world.json":
            doc["content"] = {"languages": languages or []}
        (campaign / stage).write_text(json.dumps(doc), encoding="utf-8")
    for filename, body in (readmes or {}).items():
        (campaign / filename).write_text(body, encoding="utf-8")
    return campaign


def storybook(marker: str | None, title: str = "# The Test Delve") -> str:
    body = [title, ""]
    if marker is not None:
        body += [marker, ""]
    body += ["A moor, a keep, a vow nobody remembers making.", ""]
    return "\n".join(body)


def run(gate) -> int:
    return gate.main(["--campaigns", str(gate.ROOT)])


# --- the gate's happy path --------------------------------------------------


def test_a_marker_matching_the_declared_versions_is_green(gate, capsys):
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    make_campaign(gate, "greenfield", "0.9.0", readmes={"README.md": storybook(marker)})
    assert run(gate) == 0
    assert "storybook version markers OK: 1 campaign(s)" in capsys.readouterr().out


def test_the_marker_states_the_MAX_per_stage_dsl_version(gate):
    """Stages adopt versions one at a time; the host needs the highest of them."""
    versions = dict.fromkeys(gate.STAGE_FILES, "0.6.0")
    versions["quests.json"] = "0.10.0"
    marker = gate.marker_line("0.10.0", ENGINE_DELVEC)
    make_campaign(gate, "mixed", versions, readmes={"README.md": storybook(marker)})
    assert run(gate) == 0


def test_a_marker_naming_an_older_delvec_is_green(gate):
    """`last verified with` is a fact about a past ladder run — an older compiler
    is a true statement, and staleness is version-adoption's job, not this gate's."""
    marker = gate.marker_line("0.9.0", "0.0.9")
    make_campaign(gate, "verified-earlier", readmes={"README.md": storybook(marker)})
    assert run(gate) == 0


# --- the drift this gate exists for -----------------------------------------


def test_a_marker_behind_the_declared_dsl_version_is_RED(gate, capsys):
    """The motivating scenario: the campaign moved to 0.9.0, the README says 0.3.0."""
    marker = gate.marker_line("0.3.0", ENGINE_DELVEC)
    make_campaign(gate, "drifted", "0.9.0", readmes={"README.md": storybook(marker)})
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "claims delve engine 0.3.0" in err
    assert "declare dsl_version 0.9.0" in err


def test_a_marker_ahead_of_the_declared_dsl_version_is_RED(gate, capsys):
    """Over-claiming is drift too: a host on 0.9.0 is turned away for nothing."""
    marker = gate.marker_line("0.11.0", ENGINE_DELVEC)
    make_campaign(gate, "overclaimed", "0.9.0", readmes={"README.md": storybook(marker)})
    assert run(gate) == 1
    assert "claims delve engine 0.11.0" in capsys.readouterr().err


def test_a_missing_marker_is_RED(gate, capsys):
    make_campaign(gate, "unstamped", readmes={"README.md": storybook(None)})
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "carries NO engine-version marker" in err
    assert gate.marker_line("0.9.0", ENGINE_DELVEC) in err


def test_a_missing_storybook_is_RED(gate, capsys):
    make_campaign(gate, "no-storybook")
    assert run(gate) == 1
    assert "README.md is MISSING" in capsys.readouterr().err


def test_a_malformed_marker_is_RED_and_not_read_as_missing(gate, capsys):
    make_campaign(
        gate,
        "sloppy",
        readmes={"README.md": storybook("Requires delve engine 0.9.0.")},
    )
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "the marker is MALFORMED" in err
    assert "carries NO engine-version marker" not in err


def test_a_marker_buried_below_the_fold_is_RED(gate, capsys):
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    body = "\n".join(["# The Test Delve", ""] + ["filler"] * 12 + [marker, ""])
    make_campaign(gate, "buried", readmes={"README.md": body})
    assert run(gate) == 1
    assert "buried below line" in capsys.readouterr().err


def test_two_markers_in_one_storybook_are_RED(gate, capsys):
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    body = "\n".join(["# The Test Delve", "", marker, "", marker, ""])
    make_campaign(gate, "doubled", readmes={"README.md": body})
    assert run(gate) == 1
    assert "one storybook, one stamp" in capsys.readouterr().err


def test_a_delvec_newer_than_the_engine_is_RED(gate, capsys):
    """`last verified with delvec 9.9.9` names a compiler that does not exist."""
    marker = gate.marker_line("0.9.0", "9.9.9")
    make_campaign(gate, "time-traveller", readmes={"README.md": storybook(marker)})
    assert run(gate) == 1
    assert "NEWER than this engine's own delvec" in capsys.readouterr().err


# --- localized editions -----------------------------------------------------


def test_every_declared_language_edition_needs_the_marker_too(gate, capsys):
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    make_campaign(
        gate,
        "localized",
        languages=["zh-cn"],
        readmes={"README.md": storybook(marker)},
    )
    assert run(gate) == 1
    assert "README.zh-cn.md is MISSING" in capsys.readouterr().err


def test_the_marker_is_byte_identical_across_editions(gate):
    """It is a version stamp, not prose: a translated gloss may follow it, but a
    translated version number is a wrong version number."""
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    make_campaign(
        gate,
        "localized-ok",
        languages=["zh-cn"],
        readmes={
            "README.md": storybook(marker),
            "README.zh-cn.md": storybook(marker, title="# 试炼之地"),
        },
    )
    assert run(gate) == 0


# --- the allowlist ----------------------------------------------------------


def test_an_allowlisted_campaign_is_skipped_and_ANNOUNCED(gate, capsys, monkeypatch):
    """A temporary exemption nobody can see is an exemption nobody removes."""
    monkeypatch.setattr(gate, "ALLOWLIST", {"blocked": "blocked by content PR #22"})
    make_campaign(gate, "blocked", readmes={"README.md": storybook(None)})
    make_campaign(
        gate,
        "fine",
        readmes={"README.md": storybook(gate.marker_line("0.9.0", ENGINE_DELVEC))},
    )
    assert run(gate) == 0
    out = capsys.readouterr().out
    assert "TEMPORARILY ALLOWLISTED (no marker required yet): blocked" in out
    assert "blocked by content PR #22" in out
    assert "1 campaign(s) checked" in out and "1 allowlisted" in out


def test_an_allowlisted_campaign_that_now_PASSES_is_RED(gate, capsys, monkeypatch):
    """The exemption's own expiry: once the marker is right, the entry must go."""
    monkeypatch.setattr(gate, "ALLOWLIST", {"fixed": "blocked by content PR #22"})
    make_campaign(
        gate,
        "fixed",
        readmes={"README.md": storybook(gate.marker_line("0.9.0", ENGINE_DELVEC))},
    )
    assert run(gate) == 1
    assert "delete its entry from ALLOWLIST" in capsys.readouterr().err


def test_an_allowlist_entry_for_an_absent_campaign_is_RED(gate, capsys, monkeypatch):
    monkeypatch.setattr(gate, "ALLOWLIST", {"ghost": "blocked by content PR #22"})
    make_campaign(
        gate,
        "fine",
        readmes={"README.md": storybook(gate.marker_line("0.9.0", ENGINE_DELVEC))},
    )
    assert run(gate) == 1
    assert "names a campaign that is not under" in capsys.readouterr().err


# --- the gate may never pass vacuously --------------------------------------


def test_an_empty_campaigns_root_is_RED_not_a_silent_pass(gate, capsys):
    assert run(gate) == 1
    assert "would pass vacuously" in capsys.readouterr().err


def test_a_missing_campaigns_root_is_a_usage_error(gate, capsys):
    assert gate.main(["--campaigns", str(gate.ROOT / "nope")]) == 2
    assert "campaign sources not found" in capsys.readouterr().err


def test_a_directory_without_stage_documents_is_not_a_campaign(gate):
    (gate.ROOT / "media").mkdir()
    (gate.ROOT / "media" / "cover.md").write_text("not a campaign", encoding="utf-8")
    make_campaign(
        gate,
        "real",
        readmes={"README.md": storybook(gate.marker_line("0.9.0", ENGINE_DELVEC))},
    )
    assert run(gate) == 0


def test_the_engine_delvec_version_comes_from_the_compiler_crate(gate):
    """One source for the number — never a second copy in this script."""
    assert gate.delvec_version() == ENGINE_DELVEC
