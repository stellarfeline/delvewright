r"""Guards for `tools/check-stated-counts.py`.

THE RED IT EXISTS TO PREVENT, in the exact bytes it arrives in. Two open PRs
change the idiom index's intent. One adds a tenth technique (`idiom-arguments`)
and moves the count everywhere it is stated. The other, which never touches the
library, adds a section three lines above that table:

    ### The order of the splits — decided before any of the nine

    Not a tenth technique and not a row in the table: the nine below are how a
    shape is made, and this is the decision taken before any of them […]

`git merge-tree --write-tree` resolves the pair with exit 0 — no conflict, no
marker, different regions of one file — and the merged tree ships a page whose
own sentence says nine over its own table of ten. Measured on that tree:
`check-doc-dupes`, `check-dw-codes`, `check-reference-versions` and
`check-grammar-ir-compat` all exit 0. So does this gate on EITHER branch alone.
It is the merge that is wrong, and the merge is the thing nobody re-reads.

    A NUMBER STATED IN A REFERENCE DOCUMENT NAMES THE THING THAT DECIDES IT.

`test_the_merge_of_the_two_branches_is_red` is that scenario, byte for byte, and
the two `_alone_` tests around it are what make it a merge finding rather than a
branch finding. The rest hold the machinery to the properties CLAUDE.md asks of
a gate: it must bind to something, say how much, fail in the direction drift
actually arrives from, and not be silenceable.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-stated-counts.py"


@pytest.fixture
def checker():
    """The gate, loaded fresh so `ROOT`/`SITES` can be pointed at a fixture."""
    spec = importlib.util.spec_from_file_location("csc", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# --------------------------------------------------------------- the fixture --
# A miniature of the real tree: the library that decides the numbers, and the
# four pages that state them.

TECHNIQUES = [
    ("idiom-repetition", "Repetition"),
    ("idiom-priority", "Priority"),
    ("idiom-shape", "Shape"),
]


def library(ids: list[str]) -> str:
    entries = "\n".join(
        f'    entry("{i}", {i.replace("-", "_")}, [5, 5, 5], 1, PIECE),' for i in ids
    )
    return (
        "pub const PROGRAMS: &[LibraryProgram] = &[\n" + entries + "\n];\n"
    )


def grammar_md(
    *,
    techniques: list[tuple[str, str]] = TECHNIQUES,
    composition: bool = True,
    heading_count: str = "Three",
    below_count: str | None = None,
    library_count: str = "five",
) -> str:
    rows = "\n".join(
        f"| {n} | {name} | `{pid}` | 9 × 5 × 3, 1 | what it shows |"
        for n, (pid, name) in enumerate(techniques, 1)
    )
    if composition:
        rows += (
            "\n| — | A composition demonstration | `idiom-composition-arcade` "
            "| 3 × 14 × 20, 1 | all of them at once |"
        )
    body = "\n".join(
        [
            "# Grammar",
            "",
            "## 2b. `mark` — anchor declarations",
            "",
            "Nothing to count here.",
            "",
            "## 2c. The idiom index — how the constructs make shapes",
            "",
            f"{heading_count} techniques, one minimal program each:",
            "",
            "```sh",
            "delve-grammar expand --program idiom-shape --region 15x9x3 --seed 1",
            "```",
            "",
            "| # | Technique | Program | Region, seed | What it shows |",
            "|---|---|---|---|---|",
            rows,
            "",
        ]
    )
    if below_count is not None:
        body += "\n".join(
            [
                f"### The order of the splits — decided before any of the {below_count}",
                "",
                f"Not a fourth technique and not a row in the table: the "
                f"{below_count} below are how a shape is made.",
                "",
            ]
        )
    body += "\n".join(
        [
            "## 4c. Reachability",
            "",
            f"2 of the {library_count} library programs have no roofed floor.",
            "",
        ]
    )
    return body


def tools_md(programs: str = "four") -> str:
    return (
        "# Tools\n\n`list` names an **`idiom-*` block**: "
        f"{programs} teaching programs, one per technique plus one "
        "composition.\n"
    )


def procedure_md(techniques: str = "three") -> str:
    return (
        "# Prefab procedure\n\n## 3. Author the program as JSON\n\n"
        f"Read the idiom index first. It is {techniques} techniques with a "
        "runnable program each, and a scene that looks impossible is usually "
        f"one of the {techniques}.\n"
    )


def skill_md(techniques: str = "three") -> str:
    return (
        "# /new-delve\n\nRead the idiom index: "
        f"{techniques} techniques with a runnable program each. A scene that "
        f"looks impossible is usually one of the {techniques}.\n"
    )


def build_tree(root: Path, **kw) -> Path:
    """Write the miniature tree. Keyword arguments perturb one page each."""
    ids = [t[0] for t in kw.get("techniques", TECHNIQUES)]
    if kw.get("composition", True):
        ids.append("idiom-composition-arcade")
        ids += ["castle", "church"]  # non-idiom library programs
    files = {
        "crates/grammar/src/library/mod.rs": library(
            kw.get("library_ids", sorted(ids))
        ),
        "docs/reference/grammar.md": grammar_md(
            techniques=kw.get("techniques", TECHNIQUES),
            composition=kw.get("composition", True),
            heading_count=kw.get("heading_count", "Three"),
            below_count=kw.get("below_count"),
            library_count=kw.get("library_count", "six"),
        ),
        "docs/reference/tools.md": tools_md(kw.get("programs", "four")),
        "docs/reference/prefab-procedure.md": procedure_md(
            kw.get("procedure_techniques", "three")
        ),
        ".claude/skills/new-delve/SKILL.md": skill_md(
            kw.get("skill_techniques", "three")
        ),
    }
    for rel, text in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8")
    return root


# ------------------------------------------------------------------- the gate --


def test_a_consistent_tree_passes_and_says_how_much_it_bound(
    checker, tmp_path, capsys
):
    checker.ROOT = build_tree(tmp_path)
    assert checker.main() == 0
    out = capsys.readouterr().out
    assert "OK" in out
    # The binding count is the deliverable, not a footnote (CLAUDE.md).
    assert "5 prose sites" in out
    assert "stated counts examined" in out
    assert "idiom-techniques = 3" in out
    assert "idiom-programs = 4" in out


def test_a_stale_count_in_one_page_is_a_finding(checker, tmp_path, capsys):
    """The plain drift: the library grew, one page kept the old number."""
    checker.ROOT = build_tree(tmp_path, programs="three")
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "docs/reference/tools.md" in err
    assert "claims there are 3" in err
    assert "There are 4 teaching programs" in err


def test_the_count_is_checked_in_every_page_that_states_it(
    checker, tmp_path, capsys
):
    """One oracle, four pages. A gate bound only to the page that first stated
    the number leaves the second consumer with no surface."""
    checker.ROOT = build_tree(
        tmp_path, procedure_techniques="four", skill_techniques="two"
    )
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "docs/reference/prefab-procedure.md" in err
    assert ".claude/skills/new-delve/SKILL.md" in err


def test_digits_and_words_are_the_same_claim(checker, tmp_path, capsys):
    """A page that switches to numerals must not stop being checked."""
    checker.ROOT = build_tree(tmp_path, programs="9")
    assert checker.main() == 1
    assert "claims there are 9" in capsys.readouterr().err
    checker.ROOT = build_tree(tmp_path, programs="4")
    assert checker.main() == 0


def test_an_ordinal_claim_states_a_cardinal_fact(checker, tmp_path, capsys):
    """"Not a fourth technique" is true only while there are three."""
    checker.ROOT = build_tree(tmp_path, below_count="three")
    assert checker.main() == 0
    checker.ROOT = build_tree(
        tmp_path,
        techniques=TECHNIQUES + [("idiom-erosion", "Erosion")],
        heading_count="Four",
        programs="five",
        procedure_techniques="four",
        skill_techniques="four",
        library_count="seven",
        below_count="four",
    )
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "claims there is no number 4" in err


def test_a_site_that_states_no_count_is_a_finding_not_a_pass(
    checker, tmp_path, capsys
):
    """The unbound mode: the sentence was reworded and the gate went dark."""
    root = build_tree(tmp_path)
    (root / "docs/reference/tools.md").write_text(
        "# Tools\n\n`list` names an `idiom-*` block of teaching programs.\n",
        encoding="utf-8",
    )
    checker.ROOT = root
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "states no idiom-programs count at all" in err
    assert "binds to zero is a finding" in err


def test_a_missing_page_is_a_finding(checker, tmp_path, capsys):
    root = build_tree(tmp_path)
    (root / "docs/reference/prefab-procedure.md").unlink()
    checker.ROOT = root
    assert checker.main() == 1
    assert "is missing" in capsys.readouterr().err


def test_a_missing_section_anchor_is_a_finding(checker, tmp_path, capsys):
    """A section anchor that matches nothing would silently zero its site."""
    root = build_tree(tmp_path)
    p = root / "docs/reference/grammar.md"
    p.write_text(
        p.read_text(encoding="utf-8").replace("## 2c. ", "## 2d. "),
        encoding="utf-8",
    )
    checker.ROOT = root
    assert checker.main() == 1
    assert "no heading matches" in capsys.readouterr().err


def test_a_program_with_no_row_in_the_index_is_a_finding(
    checker, tmp_path, capsys
):
    """The set claim: counts agreeing does not mean the sets agree."""
    root = build_tree(tmp_path)
    ids = sorted(
        [t[0] for t in TECHNIQUES]
        + ["idiom-composition-arcade", "idiom-light", "castle"]
    )
    (root / "crates/grammar/src/library/mod.rs").write_text(
        library(ids), encoding="utf-8"
    )
    checker.ROOT = root
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "no row for idiom-light" in err


def test_a_row_naming_no_program_is_a_finding(checker, tmp_path, capsys):
    root = build_tree(tmp_path)
    ids = sorted([t[0] for t in TECHNIQUES] + ["castle", "church"])
    (root / "crates/grammar/src/library/mod.rs").write_text(
        library(ids), encoding="utf-8"
    )
    checker.ROOT = root
    assert checker.main() == 1
    assert "has a row for idiom-composition-arcade" in capsys.readouterr().err


def test_an_unparseable_index_table_is_a_finding_not_a_count_of_nought(
    checker, tmp_path, capsys
):
    """Zero rows would make every technique claim 'wrong by N' and the oracle
    meaningless; the honest answer is that the oracle could not be computed."""
    root = build_tree(tmp_path)
    p = root / "docs/reference/grammar.md"
    p.write_text(
        p.read_text(encoding="utf-8").replace("| `idiom-", "| idiom-"),
        encoding="utf-8",
    )
    checker.ROOT = root
    assert checker.main() == 1
    assert "did not parse" in capsys.readouterr().err


def test_a_programs_table_that_yields_no_entries_is_a_finding(
    checker, tmp_path, capsys
):
    """A `PROGRAMS` block whose entries are written in a shape the parser does
    not know reads as a library of nought, and a library of nought makes every
    stated count wrong by N rather than reporting that nothing was measured.

    This is the live shape: the table's entries changed from `("id", build)`
    to `entry("id", build, region, seed, gates)` when the registry took on the
    expansion each program is judged at. Without this guard that change was a
    green parser reporting 33 programs as 0.
    """
    root = build_tree(tmp_path)
    p = root / "crates/grammar/src/library/mod.rs"
    p.write_text(
        p.read_text(encoding="utf-8").replace("    entry(", "    ("),
        encoding="utf-8",
    )
    checker.ROOT = root
    assert checker.main() == 1
    assert "ZERO programs" in capsys.readouterr().err


def test_numbers_inside_a_code_fence_are_not_claims(checker, tmp_path, capsys):
    """`--region 15x9x3` names nothing enumerable."""
    root = build_tree(tmp_path)
    p = root / "docs/reference/grammar.md"
    p.write_text(
        p.read_text(encoding="utf-8").replace(
            "```sh\n", "```sh\n# nineteen techniques, twelve below\n"
        ),
        encoding="utf-8",
    )
    checker.ROOT = root
    assert checker.main() == 0


def test_an_empty_registry_is_vacuous_not_green(checker, tmp_path, capsys):
    checker.ROOT = build_tree(tmp_path)
    checker.SITES = []
    assert checker.main() == 1
    assert "binds to nothing" in capsys.readouterr().err


# ------------------------------------------- the live merge this was built for --


def _tree_from(root: Path, section_text: str, techniques: int) -> Path:
    """A miniature of one branch: `techniques` rows, and grammar.md's added
    section verbatim from `origin/work/skill`."""
    names = [
        ("idiom-repetition", "Repetition"),
        ("idiom-priority", "Priority"),
        ("idiom-shape", "Shape"),
        ("idiom-erosion", "Erosion"),
    ][:techniques]
    words = {3: ("Three", "three", "four", "six"), 4: ("Four", "four", "five", "seven")}
    heading, lower, programs, libc = words[techniques]
    build_tree(
        root,
        techniques=names,
        heading_count=heading,
        programs=programs,
        procedure_techniques=lower,
        skill_techniques=lower,
        library_count=libc,
    )
    if section_text:
        p = root / "docs/reference/grammar.md"
        text = p.read_text(encoding="utf-8")
        marker = "\n## 4c. Reachability"
        p.write_text(text.replace(marker, section_text + marker), encoding="utf-8")
    return root


#: The section `origin/work/skill` adds, with its two counts. Written against a
#: three-technique index, which is what that branch's tree has.
ADDED_SECTION = (
    "### The order of the splits — decided before any of the three\n"
    "\n"
    "Not a fourth technique and not a row in the table: the three below are how\n"
    "a shape is made, and this is the decision taken before any of them.\n"
    "\n"
)


def test_the_prose_branch_alone_is_green(checker, tmp_path, capsys):
    checker.ROOT = _tree_from(tmp_path, ADDED_SECTION, techniques=3)
    assert checker.main() == 0
    assert "OK" in capsys.readouterr().out


def test_the_library_branch_alone_is_green(checker, tmp_path, capsys):
    checker.ROOT = _tree_from(tmp_path, "", techniques=4)
    assert checker.main() == 0
    assert "OK" in capsys.readouterr().out


def test_the_merge_of_the_two_branches_is_red(checker, tmp_path, capsys):
    """Both branches green, the clean auto-merge red. The whole point.

    Neither branch's author can see this: one never touched the library, the
    other never touched that section, and git has no reason to stop either.
    """
    checker.ROOT = _tree_from(tmp_path, ADDED_SECTION, techniques=4)
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "the three below" in err
    assert "any of the three" in err
    assert "Not a fourth technique" in err
    assert "There are 4 techniques" in err


# ------------------------------------------------------------------ this repo --


def test_the_real_repo_is_consistent_and_bound(capsys):
    """The gate, unpatched, over this checkout — the state the PR leaves behind.

    The binding assertions are the ones that matter: a registry pointed at pages
    that no longer state their counts would pass this by examining nothing.
    """
    spec = importlib.util.spec_from_file_location("csc_real", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    assert mod.main() == 0
    out = capsys.readouterr().out
    assert "OK" in out
    for site in mod.SITES:
        assert f"{site['path']}" in out
    # Every oracle is derived from something, and every site states something.
    for oracle_id in mod.ORACLES:
        value, _evidence = mod.ORACLES[oracle_id]["compute"](REPO)
        assert value > 0, f"{oracle_id} counts nothing"
