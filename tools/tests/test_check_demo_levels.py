r"""Guards for `tools/check-demo-levels.py`.

The red this gate exists to prevent: a change lands author-facing command-line
surface and the demo queue never hears about it, so the capability has no demo
and therefore no way to be confirmed. It happened on the change that added an
author-aimable render camera (`--view`, `--biome`), which is the scenario
`test_new_flag_without_a_row_is_red` reproduces in miniature.

The tests build tiny real git repos: an `origin/main` commit constructed with
`hash-object`/`write-tree`/`commit-tree` (no clone, no network) standing in for
`--base`, plus on-disk files standing in for this branch's checkout. The
extraction tests are pure and take no repository at all — they are the half most
likely to rot silently, because a clap shape this parser stops recognising turns
the demand green forever rather than red.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-demo-levels.py"


@pytest.fixture
def checker():
    """The gate, loaded fresh so `ROOT` can be pointed at a fixture repo."""
    spec = importlib.util.spec_from_file_location("cdl", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


def run(args: list[str], cwd: Path, input: str = "") -> str:
    result = subprocess.run(args, cwd=cwd, capture_output=True, text=True, input=input)
    assert result.returncode == 0, f"{args}: {result.stderr}"
    return result.stdout


def init_repo(root: Path) -> None:
    root.mkdir(parents=True, exist_ok=True)
    run(["git", "init", "-q", "-b", "main"], cwd=root)
    run(["git", "config", "user.email", "test@example.com"], cwd=root)
    run(["git", "config", "user.name", "Test"], cwd=root)


def commit_base(root: Path, files: dict[str, str]) -> str:
    """Build the `--base` commit through a scratch index, never the real one."""
    idx = root / ".git" / "tmp-index"
    env = {**os.environ, "GIT_INDEX_FILE": str(idx)}
    subprocess.run(["git", "read-tree", "--empty"], cwd=root, env=env, check=True)
    for relpath, content in files.items():
        blob = run(
            ["git", "hash-object", "-w", "--stdin"], cwd=root, input=content
        ).strip()
        subprocess.run(
            ["git", "update-index", "--add", "--cacheinfo", f"100644,{blob},{relpath}"],
            cwd=root,
            env=env,
            check=True,
            capture_output=True,
        )
    tree = subprocess.run(
        ["git", "write-tree"],
        cwd=root,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    idx.unlink(missing_ok=True)
    return run(["git", "commit-tree", tree, "-m", "base fixture"], cwd=root).strip()


def set_base(root: Path, sha: str) -> None:
    run(["git", "update-ref", "refs/remotes/origin/main", sha], cwd=root)


def write_local(root: Path, files: dict[str, str]) -> None:
    for relpath, content in files.items():
        path = root / relpath
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


CARGO = '[package]\nname = "toolcrate"\n\n[[bin]]\nname = "delve-tool"\npath = "src/main.rs"\n'
LIB_CARGO = '[package]\nname = "libcrate"\n'

MAIN_ONE_FLAG = """\
use clap::Parser;

#[derive(Parser)]
struct Cli {
    /// The only flag this fixture starts with. DW0011 is emitted here.
    #[arg(long, global = true)]
    json: bool,
}
"""

MAIN_TWO_FLAGS = """\
use clap::Parser;

#[derive(Parser)]
struct Cli {
    /// The only flag this fixture starts with. DW0011 is emitted here.
    #[arg(long, global = true)]
    json: bool,
    /// A camera the author aims.
    #[arg(long)]
    view: Vec<String>,
}
"""


def queue(rows: list[str]) -> str:
    head = "# Demo levels\n\n## Mechanic demos\n\n| Mechanic (spec) | Demo concept | Status |\n|---|---|---|\n"
    return head + "".join(rows) + "\n## Later\n\ntail\n"


ROW_TRAPS = "| Traps (0011) | **The Toll Road** | pending |\n"
ROW_CAMERA = "| Author-aimed review camera (`--view`) | **The Gatehouse Elevations** | pending |\n"

SPEC = {"docs/specs/spec-0011-traps.md": "spec"}


def fixture(root: Path, main_rs: str, rows: list[str]) -> dict[str, str]:
    return {
        "crates/toolcrate/Cargo.toml": CARGO,
        "crates/toolcrate/src/main.rs": main_rs,
        "crates/libcrate/Cargo.toml": LIB_CARGO,
        "crates/libcrate/src/lib.rs": "// no binary here\n",
        "docs/demo-levels.md": queue(rows),
        **SPEC,
    }


def build(root: Path, base_main: str, base_rows: list[str], head_main: str, head_rows: list[str]):
    init_repo(root)
    set_base(root, commit_base(root, fixture(root, base_main, base_rows)))
    write_local(root, fixture(root, head_main, head_rows))


# ------------------------------------------------------------ extraction ----
def test_named_long_flag(checker):
    src = '#[arg(long = "no-validate")]\n    skip: bool,\n'
    assert checker.flags_in_source(src) == {"no-validate"}


def test_bare_long_takes_the_field_name_kebab_cased(checker):
    src = "#[arg(long, global = true)]\n    dark_threshold: f32,\n"
    assert checker.flags_in_source(src) == {"dark-threshold"}


def test_doc_comments_and_stacked_attributes_between_attr_and_field(checker):
    src = (
        "#[arg(long)]\n"
        "    #[serde(default)]\n"
        "    /// what it does\n"
        "    reachable_floor: bool,\n"
    )
    assert checker.flags_in_source(src) == {"reachable-floor"}


def test_a_positional_argument_is_not_a_flag(checker):
    assert checker.flags_in_source("#[arg(value_name = \"DIR\")]\n    dir: PathBuf,\n") == set()


def test_a_paren_inside_a_string_literal_does_not_end_the_attribute(checker):
    src = '#[arg(long, default_value = "a)b", help = "x")]\n    seed: u64,\n'
    assert checker.flags_in_source(src) == {"seed"}


def test_the_real_repository_yields_a_flag_surface(checker):
    """The extraction half is the one that rots silently: a clap shape this
    parser stops recognising makes the demand unfalsifiable rather than red.
    Bound to the live tree, not a fixture."""
    owners, crates, files = checker.flag_owners(None)
    assert crates >= 5, crates
    assert files >= 50, files
    assert len(owners) >= 40, sorted(owners)
    # Flags that only exist because a real binary declares them.
    assert owners["view"] == {"delvec render"}
    assert "delvec" in owners["lang"]


# ---------------------------------------------------------------- demand ----
def test_clean_tree_is_green_and_states_its_binding(checker, tmp_path, capsys, monkeypatch):
    build(tmp_path, MAIN_ONE_FLAG, [ROW_TRAPS], MAIN_ONE_FLAG, [ROW_TRAPS])
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 0
    out = capsys.readouterr().out
    assert "1 binary crate(s)" in out
    assert "1 long flag(s) here" in out
    assert "Queue: 1 row(s) here, 1 at origin/main" in out
    assert "1 spec number(s)" in out


def test_new_flag_without_a_row_is_red(checker, tmp_path, capsys, monkeypatch):
    """The aimable camera, in miniature."""
    build(tmp_path, MAIN_ONE_FLAG, [ROW_TRAPS], MAIN_TWO_FLAGS, [ROW_TRAPS])
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 1
    out = capsys.readouterr().out
    assert "--view  (delve-tool)" in out
    assert "adds no row to docs/demo-levels.md" in out


def test_the_same_change_with_its_row_is_green(checker, tmp_path, capsys, monkeypatch):
    build(tmp_path, MAIN_ONE_FLAG, [ROW_TRAPS], MAIN_TWO_FLAGS, [ROW_TRAPS, ROW_CAMERA])
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 0
    assert "OK" in capsys.readouterr().out


def test_a_flag_on_a_crate_with_no_binary_is_out_of_scope(checker, tmp_path, capsys, monkeypatch):
    """Scope is the binaries an authoring session runs. A library gaining a
    clap-shaped attribute is not new surface anyone can reach."""
    build(tmp_path, MAIN_ONE_FLAG, [ROW_TRAPS], MAIN_ONE_FLAG, [ROW_TRAPS])
    write_local(tmp_path, {"crates/libcrate/src/lib.rs": MAIN_TWO_FLAGS})
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 0
    assert "OK" in capsys.readouterr().out


# ------------------------------------------------- where the table ends ----
# The gate counts rows off a pipe table, so where that table ENDS decides what
# the obligation means. A blank line ends one — CommonMark's GFM extension runs
# a table to "the first empty line, or beginning of another block-level
# structure" — and the parse this replaced ignored blank lines entirely, so a
# detached row was an entry for the gate and a paragraph of literal pipes for
# every reader. These bind the parse to the renderer's rule in both directions:
# a detached row must be named, and it must not pay for the demand above.
def test_a_blank_line_ends_the_table_as_the_renderer_ends_it(checker, tmp_path):
    write_local(tmp_path, {"docs/demo-levels.md": queue([ROW_TRAPS, "\n", ROW_CAMERA])})
    checker.ROOT = tmp_path
    rows, orphans = checker.parse_queue(None)
    assert [cells[0] for cells in rows] == ["Traps (0011)"]
    assert len(orphans) == 1, orphans
    lineno, text = orphans[0]
    # 1 title, 2 blank, 3 heading, 4 blank, 5 header, 6 delimiter, 7 row,
    # 8 blank, 9 the detached row — the finding names a line a reader can open.
    assert lineno == 9, orphans
    assert text.startswith("| Author-aimed review camera")


def test_a_heading_ends_the_table_even_with_no_blank_line(checker, tmp_path):
    doc = (
        "# Demo levels\n\n## Mechanic demos\n\n"
        "| Mechanic (spec) | Demo concept | Status |\n|---|---|---|\n"
        + ROW_TRAPS
        + "## Later\n\ntail\n"
    )
    write_local(tmp_path, {"docs/demo-levels.md": doc})
    checker.ROOT = tmp_path
    rows, orphans = checker.parse_queue(None)
    assert [cells[0] for cells in rows] == ["Traps (0011)"]
    assert orphans == []


def test_a_detached_row_is_red_and_names_its_line(checker, tmp_path, capsys, monkeypatch):
    build(tmp_path, MAIN_ONE_FLAG, [ROW_TRAPS], MAIN_ONE_FLAG, [ROW_TRAPS, "\n", ROW_CAMERA])
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 1
    out = capsys.readouterr().out
    assert "docs/demo-levels.md:9 is a table row that no table contains" in out
    assert "Rows no table contains: 1 here, 0 at origin/main" in out


def test_a_detached_row_does_not_pay_for_a_new_flag(checker, tmp_path, capsys, monkeypatch):
    """The whole point. A row the renderer drops must not satisfy the demand,
    or the obligation is met in the letter and void in the reading — which is
    one-directional falsifiability, the mode this gate is supposed to be free
    of. Both findings must appear: the row is unreadable AND the flag is
    undocumented."""
    build(tmp_path, MAIN_ONE_FLAG, [ROW_TRAPS], MAIN_TWO_FLAGS, [ROW_TRAPS, "\n", ROW_CAMERA])
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 1
    out = capsys.readouterr().out
    assert "is a table row that no table contains" in out
    assert "--view  (delve-tool)" in out
    assert "adds no row to docs/demo-levels.md" in out
    assert "Queue: 1 row(s) here, 1 at origin/main" in out


def test_the_same_row_attached_is_green(checker, tmp_path, capsys, monkeypatch):
    """The other direction: the identical row, one blank line shorter, is an
    entry — so the red above is about the blank line and nothing else."""
    build(tmp_path, MAIN_ONE_FLAG, [ROW_TRAPS], MAIN_TWO_FLAGS, [ROW_TRAPS, ROW_CAMERA])
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 0
    assert "Rows no table contains: 0 here, 0 at origin/main" in capsys.readouterr().out


def test_a_delimiter_row_that_does_not_match_the_header_opens_no_table(checker, tmp_path):
    """GFM: a delimiter row whose cell count differs from the header's makes no
    table at all. The parse must agree loudly — every line becomes a finding —
    rather than quietly keeping the rows a renderer would not draw."""
    doc = (
        "# Demo levels\n\n## Mechanic demos\n\n"
        "| Mechanic (spec) | Demo concept | Status |\n|---|---|\n" + ROW_TRAPS + "\n"
    )
    write_local(tmp_path, {"docs/demo-levels.md": doc})
    checker.ROOT = tmp_path
    rows, orphans = checker.parse_queue(None)
    assert rows == []
    assert len(orphans) == 3, orphans


def test_the_live_queue_has_no_detached_rows(checker):
    """Bound to the real document, not a fixture: this is the assertion the
    external review's nit would have failed."""
    rows, orphans = checker.parse_queue(None)
    assert orphans == [], orphans
    assert len(rows) >= 30, len(rows)


# ------------------------------------------------------------------- rot ----
def test_a_row_citing_a_dead_dw_code_is_red(checker, tmp_path, capsys, monkeypatch):
    row = "| Traps (0011) | shows `DW0999` | pending |\n"
    build(tmp_path, MAIN_ONE_FLAG, [row], MAIN_ONE_FLAG, [row])
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 1
    assert "cites DW0999" in capsys.readouterr().out


def test_a_row_citing_a_spec_that_does_not_exist_is_red(checker, tmp_path, capsys, monkeypatch):
    row = "| Traps (0099) | **The Toll Road** | pending |\n"
    build(tmp_path, MAIN_ONE_FLAG, [row], MAIN_ONE_FLAG, [row])
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 1
    assert "cites spec 0099" in capsys.readouterr().out


def test_an_adr_number_is_not_read_as_a_spec_number(checker, tmp_path, capsys, monkeypatch):
    """`(0036, ADR-0020)` cites one spec and one ADR, never two specs."""
    row = "| Spatial contract (0011, ADR-0020) | **The Two Wards** | pending |\n"
    files = {"docs/adr/0020-map-design-pipeline.md": "adr"}
    build(tmp_path, MAIN_ONE_FLAG, [row], MAIN_ONE_FLAG, [row])
    write_local(tmp_path, files)
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 0
    out = capsys.readouterr().out
    assert "1 spec number(s), 1 ADR number(s)" in out


# --------------------------------------------------------------- vacuity ----
def test_no_binary_crates_is_a_finding_not_a_pass(checker, tmp_path, capsys, monkeypatch):
    init_repo(tmp_path)
    files = {"docs/demo-levels.md": queue([ROW_TRAPS]), **SPEC}
    set_base(tmp_path, commit_base(tmp_path, files))
    write_local(tmp_path, files)
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 1
    out = capsys.readouterr().out
    assert "examined 0 binary crates" in out
    assert "found 0 long command-line flags" in out


def test_a_moved_table_heading_is_a_finding_not_a_pass(checker, tmp_path, capsys, monkeypatch):
    build(tmp_path, MAIN_ONE_FLAG, [ROW_TRAPS], MAIN_ONE_FLAG, [ROW_TRAPS])
    write_local(
        tmp_path,
        {"docs/demo-levels.md": queue([ROW_TRAPS]).replace("## Mechanic demos", "## Showcases")},
    )
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 1
    out = capsys.readouterr().out
    assert "parsed 0 rows" in out
    assert "cite 0 resolvable identifiers" in out


# ------------------------------------------------------- the invocation ----
def test_ci_runs_this_gate():
    """A gate nothing invokes is not a gate (CLAUDE.md). The obligation this
    script carries used to live in a doc line; moving it into a script it is
    still nobody's job to run would be the same defect wearing the fix's
    clothes. This binds the invocation to a job that already has to pass."""
    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    assert "python3 tools/check-demo-levels.py" in ci, (
        "ci.yml no longer runs the demo-queue gate"
    )
    # The gate needs `origin/main` present; the docs job fetches it once for the
    # numbering gates and this rides that fetch. If the fetch goes, the gate
    # refuses instead of comparing against nothing — but say so here too.
    assert "origin main:refs/remotes/origin/main" in ci, (
        "ci.yml no longer fetches the base ref this gate diffs against"
    )


# ------------------------------------------------------------------ base ----
def test_an_unfetched_base_refuses_rather_than_comparing_with_nothing(
    checker, tmp_path, capsys, monkeypatch
):
    build(tmp_path, MAIN_ONE_FLAG, [ROW_TRAPS], MAIN_ONE_FLAG, [ROW_TRAPS])
    run(["git", "update-ref", "-d", "refs/remotes/origin/main"], cwd=tmp_path)
    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-demo-levels.py"])
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "origin/main" in err
    assert "git fetch" in err
