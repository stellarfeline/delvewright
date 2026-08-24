"""The storybook version gate (`tools/check-storybook-version.py`).

The drift this pins: a campaign's storybook is what a server host reads before running the delve, and the one internal fact it is
allowed to carry — which engine the delve needs — is a hand-typed number. Hand-
typed numbers go stale the moment a campaign adopts a new `dsl_version`, and a
stale one is worse than none: it tells a host on an old engine to go ahead.

The second half is the general form of that, found the hard way: the v1.1.0
island release shipped a storybook whose marker was correct and whose OTHER
three version literals were not — a campaign-version stamp, a `docker run` line
naming the tag v1.1.0 had just replaced, and a localized gloss carrying its own
translated copy of the delvec number. Checking the marker harder would never
have caught any of them. So a storybook may carry NO version literal but the
marker, and these tests hold each of those three shapes red.

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
        f'[package]\nname = "delvec"\nversion = "{ENGINE_DELVEC}"\n'
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


# --- no OTHER version literal (the v1.1.0 island release) -------------------
#
# The marker being true was never enough: the v1.1.0 island storybook shipped
# three version literals and only the marker was bound to anything. It told a
# host to run `:v1.0.0` — the version it had just replaced.


def test_a_campaign_version_stamp_beside_the_marker_is_RED(gate, capsys):
    """Literal 2: `**v1.0.0** (exact engine pin: …)` — read by nothing, and a lie
    by construction between releases, since `main` is not a released version."""
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    body = "\n".join(
        [
            "# The Test Delve",
            "",
            "**v1.0.0** (exact engine pin: `versions.toml`)",
            "",
            marker,
            "",
        ]
    )
    make_campaign(gate, "stamped", readmes={"README.md": body})
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "README.md:3 carries the version literal `v1.0.0`" in err
    assert "which nothing binds" in err


def test_a_pinned_image_tag_in_the_host_command_is_RED(gate, capsys):
    """Literal 3, the one that actually hurt: a host copy-pastes this line."""
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    body = "\n".join(
        [
            "# The Test Delve",
            "",
            marker,
            "",
            "```sh",
            "docker run -d --name delve -p 25565:25565 -v delve-data:/data \\",
            "  -e EULA=TRUE ghcr.io/stellarfeline/delve-the-test-delve:v1.0.0",
            "```",
            "",
        ]
    )
    make_campaign(gate, "pinned-tag", readmes={"README.md": body})
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "README.md:7 pins an image tag:" in err
    assert "ghcr.io/stellarfeline/delve-the-test-delve:v1.0.0" in err
    assert "Name `:latest` here" in err


def test_a_pinned_image_tag_is_reported_ONCE_not_also_as_a_bare_literal(gate, capsys):
    """One line, one finding: the tag's own version is inside the image span, so
    the actionable message ("name `:latest`") is not buried under a duplicate."""
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    body = "\n".join(
        ["# The Test Delve", "", marker, "", "ghcr.io/x/delve-y:v1.0.0", ""]
    )
    make_campaign(gate, "once", readmes={"README.md": body})
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert err.count("README.md:5") == 1
    assert "carries the version literal" not in err


def test_the_host_command_naming_latest_is_green(gate, capsys):
    """`:latest` IS the storybook's claim — this is the current delve. The port
    mapping, the volume and `localhost:25565` on the same lines are not tags."""
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    body = "\n".join(
        [
            "# The Test Delve",
            "",
            marker,
            "",
            "Then Multiplayer → Direct Connect to `localhost:25565`:",
            "",
            "```sh",
            "docker run -d --name delve -p 25565:25565 -v delve-data:/data \\",
            "  -e EULA=TRUE ghcr.io/stellarfeline/delve-the-test-delve:latest",
            "```",
            "",
            "To hold one exact version, take the `:vX.Y.Z` tag off a release page.",
            "",
            "| **Licence** | CC BY-SA 4.0 |",
            "",
        ]
    )
    make_campaign(gate, "unpinned", readmes={"README.md": body})
    assert run(gate) == 0
    out = capsys.readouterr().out
    assert "1 storybook file(s) scanned for unbound version literals" in out


def test_a_localized_gloss_restating_the_markers_numbers_is_RED(gate, capsys):
    """Literal 4: the zh gloss carried a TRANSLATED copy of the delvec number and
    drifted to 1.0.0 while the untranslated stamp one line above said 1.1.0."""
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    gloss = (
        "> 需要 delve 引擎 0.9.0 或更高版本 — "
        f"最近一次通过验证的 delvec 版本为 {ENGINE_DELVEC}。"
    )
    make_campaign(
        gate,
        "glossed",
        languages=["zh-cn"],
        readmes={
            "README.md": storybook(marker),
            "README.zh-cn.md": "\n".join(["# 试炼之地", "", marker, gloss, ""]),
        },
    )
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "README.zh-cn.md:4 carries the version literal `0.9.0`" in err
    assert "the stamp is not translated" in err


def test_a_localized_gloss_carrying_NO_number_is_green(gate):
    """A gloss may say what the untranslated line above it means — just not in
    numbers, which is what the content round shipped."""
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    gloss = "> (上一行是版本印记,不翻译:它声明本战役需要的引擎版本。)"
    make_campaign(
        gate,
        "glossed-ok",
        languages=["zh-cn"],
        readmes={
            "README.md": storybook(marker),
            "README.zh-cn.md": "\n".join(["# 试炼之地", "", marker, gloss, ""]),
        },
    )
    assert run(gate) == 0


def test_a_two_component_number_is_NOT_a_version_literal(gate):
    """`CC BY-SA 4.0` and `GPL-3.0` are licences a storybook names legitimately."""
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    body = "\n".join(
        [
            "# The Test Delve",
            "",
            marker,
            "",
            "| **Licence** | CC BY-SA 4.0 |",
            "",
            "Code is GPL-3.0; the prose is CC BY 4.0.",
            "",
        ]
    )
    make_campaign(gate, "licensed", readmes={"README.md": body})
    assert run(gate) == 0


def test_an_unbound_literal_is_RED_even_with_no_marker_at_all(gate, capsys):
    """The two clauses are independent: a storybook nobody stamped can still be
    handing hosts a dead image tag, and both findings must arrive together."""
    body = "\n".join(
        ["# The Test Delve", "", "ghcr.io/stellarfeline/delve-x:v1.0.0", ""]
    )
    make_campaign(gate, "unstamped-and-pinned", readmes={"README.md": body})
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "pins an image tag" in err
    assert "carries NO engine-version marker" in err


def test_the_marker_line_itself_is_never_read_as_an_unbound_literal(gate):
    """The marker holds two version numbers by design — it is the bound one."""
    marker = gate.marker_line("0.9.0", ENGINE_DELVEC)
    make_campaign(gate, "just-the-marker", readmes={"README.md": storybook(marker)})
    assert run(gate) == 0


def test_a_malformed_marker_attempt_is_not_ALSO_an_unbound_literal(gate, capsys):
    """A broken marker is one finding, reported by the marker clause that owns
    it — not a second one about the numbers inside it."""
    make_campaign(
        gate,
        "sloppy-numbers",
        readmes={"README.md": storybook("Requires delve engine 0.9.0.")},
    )
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "the marker is MALFORMED" in err
    assert "carries the version literal" not in err


# --- the allowlist ----------------------------------------------------------


def test_an_allowlisted_campaign_is_skipped_and_ANNOUNCED(gate, capsys, monkeypatch):
    """A temporary exemption nobody can see is an exemption nobody removes."""
    monkeypatch.setattr(gate, "ALLOWLIST", {"blocked": "blocked by an open content round"})
    make_campaign(gate, "blocked", readmes={"README.md": storybook(None)})
    make_campaign(
        gate,
        "fine",
        readmes={"README.md": storybook(gate.marker_line("0.9.0", ENGINE_DELVEC))},
    )
    assert run(gate) == 0
    out = capsys.readouterr().out
    assert "TEMPORARILY ALLOWLISTED (no marker required yet): blocked" in out
    assert "blocked by an open content round" in out
    assert "1 campaign(s) checked" in out and "1 allowlisted" in out


def test_an_allowlisted_campaign_that_now_PASSES_is_RED(gate, capsys, monkeypatch):
    """The exemption's own expiry: once the marker is right, the entry must go."""
    monkeypatch.setattr(gate, "ALLOWLIST", {"fixed": "blocked by an open content round"})
    make_campaign(
        gate,
        "fixed",
        readmes={"README.md": storybook(gate.marker_line("0.9.0", ENGINE_DELVEC))},
    )
    assert run(gate) == 1
    assert "delete its entry from ALLOWLIST" in capsys.readouterr().err


def test_an_allowlist_entry_for_an_absent_campaign_is_RED(gate, capsys, monkeypatch):
    monkeypatch.setattr(gate, "ALLOWLIST", {"ghost": "blocked by an open content round"})
    make_campaign(
        gate,
        "fine",
        readmes={"README.md": storybook(gate.marker_line("0.9.0", ENGINE_DELVEC))},
    )
    assert run(gate) == 1
    assert "names a campaign that is not under" in capsys.readouterr().err


# --- the gate may never pass vacuously --------------------------------------


def test_zero_storybook_files_scanned_is_RED_even_with_campaigns_present(
    gate, capsys, monkeypatch
):
    """The literal clauses' own binding count. Allowlist the only campaign that
    ships a storybook and they examine nothing — green, and proving nothing."""
    monkeypatch.setattr(gate, "ALLOWLIST", {"only": "blocked by an open content round"})
    make_campaign(gate, "only", readmes={"README.md": storybook(None)})
    assert run(gate) == 1
    assert "ZERO storybook files were read" in capsys.readouterr().err


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
