"""The skill version-line gate (`tools/check-skill-version.py`).

The drift this pins (ADR-0016 line 3): the `/new-delve` skill declares its own
version and the `delvec` range it drives. Both are hand-typed, and a hand-typed
range nobody reads is the project's recurring failure class — the unbound
declaration that is green because it examined nothing.

Two declarations, two different bindings, and the tests keep them apart:
`requires.delvec` is a COMPATIBILITY window checked by MEMBERSHIP (the engine
must fall inside it), `verified_with` is EVIDENCE checked by EQUALITY (it must
be the engine in this tree). Collapsing them would make the frontmatter assert,
after every engine release, that older engines are unsupported — untested, and
probably false.

These tests drive the gate over a synthetic repo (a skill file, a compiler
`Cargo.toml`, a compiler `main.rs`) rather than the live tree, so they keep
failing for the right reason as the real skill and the real CLI grow.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-skill-version.py"

ENGINE = "1.0.0"

# A miniature `delvec` CLI in the exact clap shape the gate parses out of
# `crates/compiler/src/main.rs`.
MAIN_RS = '''
#[derive(Parser)]
#[command(name = "delvec")]
struct Cli {
    /// Print the version and exit.
    #[arg(long, global = true)]
    version: bool,
    #[arg(long, global = true, default_value = "en")]
    lang: String,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Validate.
    Validate {
        campaign_dir: PathBuf,
    },
    /// Schema.
    Schema {
        #[arg(long)]
        stage: String,
    },
    /// The l10n inventory.
    L10nInventory {
        campaign_dir: PathBuf,
    },
    /// Snapshot.
    Snapshot {
        campaign_dir: PathBuf,
        #[arg(long)]
        at: Option<String>,
        #[arg(long, requires = "at")]
        dist: Option<f64>,
        #[arg(long)]
        labels: bool,
    },
    /// The map editor.
    Edit {
        #[command(subcommand)]
        action: EditAction,
    },
}

#[derive(Subcommand)]
enum EditAction {
    /// Apply.
    Apply {
        campaign_dir: PathBuf,
        #[arg(long)]
        batch: Option<PathBuf>,
    },
}
'''

SKILL_BODY = """
## The loop

1. `delvec schema --stage <n>` — generate against the live schema.
2. `delvec validate <campaign-dir>` — fix by diagnostic code.
3. `delvec l10n-inventory <campaign-dir> --lang <code>` gives the key inventory.
4. `delvec snapshot` (`--at <anchor> --dist`, `--labels`) for visual review.

The storybook marker names an engine but no subcommand:

```
> **Requires delve engine 0.9.0 or newer** — last verified with delvec <version>.
```
"""


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at a synthetic repo."""
    spec = importlib.util.spec_from_file_location("check_skill_version", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)

    cargo = tmp_path / "crates" / "compiler" / "Cargo.toml"
    cargo.parent.mkdir(parents=True)
    cargo.write_text(
        f'[package]\nname = "delvewright-compiler"\nversion = "{ENGINE}"\n',
        encoding="utf-8",
    )
    main_rs = tmp_path / "crates" / "compiler" / "src" / "main.rs"
    main_rs.parent.mkdir(parents=True)
    main_rs.write_text(MAIN_RS, encoding="utf-8")

    skill = tmp_path / ".claude" / "skills" / "new-delve" / "SKILL.md"
    skill.parent.mkdir(parents=True)

    monkeypatch.setattr(module, "REPO", tmp_path)
    monkeypatch.setattr(module, "SKILL", skill)
    monkeypatch.setattr(module, "COMPILER_CARGO_TOML", cargo)
    monkeypatch.setattr(module, "COMPILER_MAIN_RS", main_rs)
    module.SKILL_PATH = skill
    module.CARGO_PATH = cargo
    return module


def write_skill(gate, frontmatter: str, body: str = SKILL_BODY) -> None:
    gate.SKILL_PATH.write_text(f"---\n{frontmatter}\n---\n{body}", encoding="utf-8")


GOOD_FRONTMATTER = f"""name: new-delve
description: Generate a delve.
version: 1.0.0
requires:
  delvec: ">=1.0.0 <2.0.0"
verified_with: {ENGINE}"""


def test_true_declaration_passes(gate, capsys):
    write_skill(gate, GOOD_FRONTMATTER)
    assert gate.main() == 0
    out = capsys.readouterr().out
    assert "check-skill-version: OK" in out
    # The binding count is stated on every run, pass or fail.
    assert "4 distinct subcommand(s) (l10n-inventory, schema, snapshot, validate)" in out
    assert "5 long-flag reference(s)" in out


def test_a_patch_engine_bump_inside_the_window_stays_green(gate):
    """The whole point of the split: 1.0.0 -> 1.4.0 does not move `requires`.

    Only `verified_with` restamps. A window whose floor tracked the engine would
    red here and force the frontmatter to claim 1.0.0-1.3.x are unsupported.
    """
    gate.CARGO_PATH.write_text(
        '[package]\nname = "delvewright-compiler"\nversion = "1.4.0"\n',
        encoding="utf-8",
    )
    write_skill(gate, GOOD_FRONTMATTER.replace(f"verified_with: {ENGINE}", "verified_with: 1.4.0"))
    assert gate.main() == 0


def test_window_above_the_engine_is_red(gate, capsys):
    """Membership: an engine below the floor is outside the window."""
    write_skill(gate, GOOD_FRONTMATTER.replace(">=1.0.0 <2.0.0", ">=1.1.0 <2.0.0"))
    assert gate.main() == 1
    assert "is OUTSIDE the declared window" in capsys.readouterr().err


def test_window_below_the_engine_is_red(gate, capsys):
    """Membership, the other side: `>=0.9.0 <1.0.0` excludes a 1.0.0 engine."""
    write_skill(gate, GOOD_FRONTMATTER.replace(">=1.0.0 <2.0.0", ">=0.9.0 <1.0.0"))
    assert gate.main() == 1
    assert "is OUTSIDE the declared window" in capsys.readouterr().err


def test_major_engine_bump_leaves_the_window_behind(gate, capsys):
    """The bump the window exists to survive: delvec 2.0.0 under `<2.0.0`."""
    gate.CARGO_PATH.write_text(
        '[package]\nname = "delvewright-compiler"\nversion = "2.0.0"\n',
        encoding="utf-8",
    )
    write_skill(gate, GOOD_FRONTMATTER.replace(f"verified_with: {ENGINE}", "verified_with: 2.0.0"))
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "delvec 2.0.0 is OUTSIDE the declared window >=1.0.0 <2.0.0" in err


def test_ceiling_must_be_the_floors_next_major(gate, capsys):
    write_skill(gate, GOOD_FRONTMATTER.replace("<2.0.0", "<1.5.0"))
    assert gate.main() == 1
    assert "next major" in capsys.readouterr().err


def test_verified_with_above_the_engine_is_red(gate, capsys):
    """Evidence from a compiler that does not exist."""
    write_skill(gate, GOOD_FRONTMATTER.replace(f"verified_with: {ENGINE}", "verified_with: 1.1.0"))
    assert gate.main() == 1
    assert "ABOVE this repo's engine" in capsys.readouterr().err


def test_verified_with_below_the_engine_is_stale(gate, capsys):
    """The engine moved and nobody re-ran the skill against it."""
    gate.CARGO_PATH.write_text(
        '[package]\nname = "delvewright-compiler"\nversion = "1.4.0"\n',
        encoding="utf-8",
    )
    write_skill(gate, GOOD_FRONTMATTER)
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "is STALE" in err
    assert "verified_with: 1.4.0" in err
    # …and it must NOT ask for the compatibility window to move.
    assert "Leave `requires.delvec` alone" in err


def test_missing_verified_with_is_red(gate, capsys):
    write_skill(gate, GOOD_FRONTMATTER.replace(f"\nverified_with: {ENGINE}", ""))
    assert gate.main() == 1
    assert "`verified_with:` is missing" in capsys.readouterr().err


def test_missing_version_field_is_red(gate, capsys):
    write_skill(gate, GOOD_FRONTMATTER.replace("version: 1.0.0\n", ""))
    assert gate.main() == 1
    assert "`version:` is missing" in capsys.readouterr().err


def test_missing_requires_block_is_red(gate, capsys):
    write_skill(gate, "name: new-delve\ndescription: d.\nversion: 1.0.0")
    assert gate.main() == 1
    assert "`requires: delvec:` is missing" in capsys.readouterr().err


def test_subcommand_the_cli_does_not_have_is_red(gate, capsys):
    """The range is a claim about a CLI surface — so the surface is checked."""
    write_skill(gate, GOOD_FRONTMATTER, SKILL_BODY + "\n5. `delvec rehearse <dir>`\n")
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "`delvec rehearse`, which the CLI does not have" in err


def test_flag_the_subcommand_does_not_have_is_red(gate, capsys):
    write_skill(gate, GOOD_FRONTMATTER, "1. `delvec schema --stages <n>`\n")
    assert gate.main() == 1
    assert "`--stages`" in capsys.readouterr().err


def test_global_flag_is_accepted_on_any_subcommand(gate):
    write_skill(gate, GOOD_FRONTMATTER, "1. `delvec validate <dir> --lang zh-cn`\n")
    assert gate.main() == 0


def test_flags_in_the_parenthetical_after_a_span_bind(gate, capsys):
    """`delvec snapshot` documents its flags in a following paren, not in-span."""
    write_skill(gate, GOOD_FRONTMATTER, "- `delvec snapshot` (`--at`, `--elevation`)\n")
    assert gate.main() == 1
    assert "`--elevation`" in capsys.readouterr().err


def test_zero_binding_is_a_failure_not_a_pass(gate, capsys):
    """A gate that examined nothing is vacuous — the island's floor-gate lesson."""
    write_skill(gate, GOOD_FRONTMATTER, "No commands here at all.\n")
    assert gate.main() == 1
    assert "extracted 0 delvec subcommand references" in capsys.readouterr().err


def test_unparseable_cli_is_a_failure_not_a_pass(gate, capsys):
    """If the clap shape moves, the gate reds instead of silently binding to zero."""
    gate.COMPILER_MAIN_RS.write_text("fn main() {}\n", encoding="utf-8")
    write_skill(gate, GOOD_FRONTMATTER)
    assert gate.main() == 1
    assert "parsed 0 subcommands" in capsys.readouterr().err


def test_marker_template_contributes_no_subcommand(gate):
    """`last verified with delvec <version>.` must not read as `delvec <version>`."""
    spans = gate.code_spans(SKILL_BODY)
    subs = [sub for sub, _ in gate.invocations(spans) if sub is not None]
    assert "<version>" not in subs


def test_cli_parse_finds_subcommands_and_globals(gate):
    subcommands, globals_ = gate.parse_cli(MAIN_RS)
    assert set(subcommands) == {
        "validate",
        "schema",
        "l10n-inventory",
        "snapshot",
        "edit",
    }
    assert subcommands["snapshot"] == {"at", "dist", "labels"}
    assert {"version", "lang"} <= globals_


def test_nested_action_is_not_a_top_level_subcommand(gate, capsys):
    """`delvec edit apply` exists; `delvec apply` does not."""
    write_skill(gate, GOOD_FRONTMATTER, "1. `delvec apply <dir>`\n")
    assert gate.main() == 1
    assert "`delvec apply`, which the CLI does not have" in capsys.readouterr().err


def test_nested_action_flags_belong_to_their_parent(gate):
    """…and its flags read correctly through the parent: `delvec edit --batch`."""
    write_skill(gate, GOOD_FRONTMATTER, "1. `delvec edit apply <dir> --batch b.json`\n")
    assert gate.main() == 0
