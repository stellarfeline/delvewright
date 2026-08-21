r"""Guards for `tools/check-pins.py`.

The defect it exists to prevent, from the round that paid for it: a content-repo
workflow built its judge from a pinned pipeline commit; the pin sat while the
rule that judge enforced was settled upstream; a zone was reported red for
failing a rule that no longer existed. Four hundred commits of drift, and nothing
anywhere was red, because a pin's staleness is invisible — the file reads the same
on the day it is written and a year later.

The tests below assert the gate fails in the direction the defect actually
arrives from, and that each of the three ways it could be vacuous is closed:

- an UNREGISTERED pin (the pin exists and no entry mentions it),
- a pin whose instrument moved and whose record does not say anyone looked,
- a pin declared exempt by a policy the object does not support — a `release`
  pin with no release tag, or an own-repo pin downgraded to `immutable`,

plus the two shapes this project keeps shipping: a binding of zero reported as a
pass, and a check declared but never invoked (`judged_by` naming a file that does
not run it).

The repository's OWN registry is exercised too, in both directions: it must be
complete now, and it must red when a pin is taken out of it. A checker that only
ever passes proves nothing.

A separate group is about the class where a registry entry's two obligations come
apart. A pin named by a VERSION STRING is discovered by nothing — the literal
carries no shape separating it from data — so the recorded decision is owed
exactly where discovery is impossible. Both halves of that gap are asserted: the
site that CAUSES the fetch is discovered by its key, so an unregistered one still
reds; the sites that RESTATE it are covered by `bound_by`, and a binder that does
not read the key, does not name a site, or is run by nothing is a red. Without
those the field would be prose, and a defect can write prose.

A group after that is about MEMBERSHIP, which is decided by discovery and never
by importance. Three schemas reach a value with no shape of its own — a Python
package manifest, an action's input contract, and an install command the repo
runs — and each is asserted in both directions, because a rule that only ever
finds more is not a rule about the object. So an exact requirement is discovered
and a RANGE is not (a range names no version, and demanding a decision about a
value that does not exist is how a correct gate teaches people to write
exemptions); an install naming no version is a finding in a workflow step and in
a shell script, and the identical words printed by a Python program are prose.
Two entries claiming one value is its own red, because discovery is keyed by the
value and a collision would merge their sites and erase one silently — an answer
rather than an error.

The last group is about the OTHER direction — a refusal the tree cannot satisfy.
The enumeration that keeps `FETCH_SITES` honest reads a verb in the language of
the file it is found in, because a COMMAND (`docker run`, `git clone`) is a
process any language can spawn while a DIRECTIVE (`uses:`, `FROM`, a Cargo
`git =`) is a statement in one configuration language and prose everywhere else.
Both halves are asserted, because a narrowing that cannot tell a Rust file which
really shells out to `docker run` from one whose diagnostic wraps the words
`FROM A SOFT-LOCK` onto a new line is not a narrowing, it is an exemption.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-pins.py"

# Assembled rather than written out, and the reason is the checker itself: it
# discovers pins by SHAPE across every tracked file that can execute, and this
# file is one. A 40-hex fixture spelled in full would be found here and reported
# as an unregistered pin in this repository — correctly, by its own rule. Keeping
# every literal below the threshold means the enumeration needs no exemption for
# test data, which is the kind of exemption that later covers a real pin.
DIGEST = "sha256:" + "ab12" * 16
ACTION = "example/fetch" + "er@v4"  # `uses: <this>` here would be a pin too
REV = "0123456789abcdef" * 2 + "01234567"


def run(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--root", str(root), *args],
        capture_output=True,
        text=True,
    )


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """A minimal git repo with one workflow holding one pin."""
    (tmp_path / ".github" / "workflows").mkdir(parents=True)
    (tmp_path / ".github" / "workflows" / "audit.yml").write_text(
        "name: audit\n"
        "jobs:\n"
        "  a:\n"
        "    steps:\n"
        "      - uses: " + ACTION + "\n"
        f"        with:\n          image: {DIGEST}\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "-C", str(tmp_path), "init", "-q"], check=True)
    subprocess.run(["git", "-C", str(tmp_path), "add", "-A"], check=True)
    return tmp_path


def write_registry(repo: Path, body: str) -> None:
    (repo / ".github" / "pins.toml").write_text(body, encoding="utf-8")
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)


def add_file(repo: Path, rel: str, body: str) -> None:
    """Track one more file, so the fetch-verb enumeration has to read it."""
    path = repo / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)


COMPLETE = f"""
[[pin]]
id = "fetcher"
value = "{ACTION}"
sites = [".github/workflows/audit.yml"]
policy = "floating"
why = "held at its major tag"

[[pin]]
id = "image"
value = "{DIGEST}"
sites = [".github/workflows/audit.yml"]
policy = "immutable"
why = "third-party bytes"
"""


def test_complete_registry_passes(repo: Path) -> None:
    write_registry(repo, COMPLETE)
    r = run(repo)
    assert r.returncode == 0, r.stderr
    assert "binding: 2 pin(s)" in r.stdout


def test_unregistered_pin_is_a_finding(repo: Path) -> None:
    """The shape the incident had: the pin is right there and nothing names it."""
    write_registry(repo, COMPLETE.split("[[pin]]\nid = \"image\"")[0])
    r = run(repo)
    assert r.returncode == 1
    assert "unregistered pin" in r.stderr and DIGEST in r.stderr


def test_registry_that_drifted_from_the_file_is_a_finding(repo: Path) -> None:
    write_registry(repo, COMPLETE + f"""
[[pin]]
id = "ghost"
value = "{REV}"
sites = [".github/workflows/audit.yml"]
policy = "immutable"
why = "no longer there"
""")
    r = run(repo)
    assert r.returncode == 1
    assert "is not there any more" in r.stderr


def test_zero_binding_is_a_finding_not_a_pass(tmp_path: Path) -> None:
    """A gate that examined nothing has proved nothing."""
    (tmp_path / ".github").mkdir()
    (tmp_path / ".github" / "pins.toml").write_text("", encoding="utf-8")
    (tmp_path / "README.md").write_text("no pins here\n", encoding="utf-8")
    subprocess.run(["git", "-C", str(tmp_path), "init", "-q"], check=True)
    subprocess.run(["git", "-C", str(tmp_path), "add", "-A"], check=True)
    r = run(tmp_path)
    assert r.returncode == 1
    assert "binding of zero" in r.stderr


# A TRACKED FILE IS EXAMINED WHEREVER IT SITS, and this is the guard for the one
# defect that cost this tool 27 files. `BUILD_OUTPUT_DIRS` is a claim about a raw
# filesystem walk of an upstream checkout; applied to `tracked_files()`, whose
# population is `git ls-files`, it can only ever subtract authored content. It
# was so applied, and it was inert in the repository it was written in — every
# entry matched zero tracked files there — so nothing ever showed it was wrong.
# Copied to the repository where `campaigns/` IS the content, the same entry
# removed every campaign stage document from both pin discovery and the fetch-
# verb enumeration, with no count anywhere saying so.
#
# The names below are therefore of two kinds, and both belong here. `target`,
# `node_modules` and `dist` are STILL in `BUILD_OUTPUT_DIRS`, so they fail the
# moment anyone re-applies that constant to the tracked enumeration. `campaigns`
# and `content-repo` are the two the list used to carry, and are the historical
# instance. A skip list over tracked files is the defect either way.
@pytest.mark.parametrize(
    "directory",
    ["target", "node_modules", "dist", "campaigns", "content-repo"],
)
def test_a_tracked_file_is_examined_whatever_directory_it_sits_in(
    repo: Path, directory: str
) -> None:
    other = "sha256:" + "cd34" * 16
    write_registry(repo, COMPLETE)
    add_file(
        repo,
        f"{directory}/deploy.yml",
        f"jobs:\n  a:\n    steps:\n      - with:\n          image: {other}\n",
    )
    r = run(repo)
    assert r.returncode == 1, (
        f"a tracked file under {directory}/ was not examined — a skip list has "
        f"reached the tracked enumeration again"
    )
    assert "unregistered pin" in r.stderr and other in r.stderr


def test_own_repo_pin_may_not_be_called_immutable(repo: Path) -> None:
    """The escape hatch the defect would reach for, closed by the object.

    A commit id IS content-addressed, so `immutable` reads as defensible — and it
    would exempt exactly the pins that rot. The kind is decided by what the pin
    names, not by what the author calls it.
    """
    write_registry(repo, COMPLETE + f"""
[[pin]]
id = "engine"
value = "{REV}"
sites = [".github/workflows/audit.yml"]
repo = "stellarfeline/delvewright"
policy = "immutable"
why = "a commit names exact bytes"
""")
    (repo / ".github" / "workflows" / "audit.yml").write_text(
        f"name: audit\nenv:\n  E: {REV}\njobs:\n  a:\n    steps:\n"
        f"      - uses: {ACTION}\n"
        f"        with:\n          image: {DIGEST}\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)
    r = run(repo)
    assert r.returncode == 1
    assert "not exempt by being called immutable" in r.stderr


def test_track_pin_must_say_what_it_was_reviewed_against(repo: Path) -> None:
    write_registry(repo, COMPLETE + f"""
[[pin]]
id = "engine"
value = "{REV}"
sites = [".github/workflows/audit.yml"]
repo = "stellarfeline/delvewright"
policy = "track"
judged_by = ".github/workflows/audit.yml"
why = "the judge"
""")
    (repo / ".github" / "workflows" / "audit.yml").write_text(
        f"name: audit\nenv:\n  E: {REV}\njobs:\n  a:\n    steps:\n"
        f"      - uses: {ACTION}\n"
        f"        with:\n          image: {DIGEST}\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)
    r = run(repo)
    assert r.returncode == 1
    assert "must carry `reviewed`" in r.stderr


def test_judged_by_that_does_not_invoke_the_check_is_a_finding(repo: Path) -> None:
    """A gate nothing invokes is not a gate. `judged_by` is verified, not trusted."""
    write_registry(repo, COMPLETE + f"""
[[pin]]
id = "engine"
value = "{REV}"
sites = [".github/workflows/audit.yml"]
repo = "stellarfeline/delvewright"
policy = "track"
judged_by = ".github/workflows/audit.yml"
reviewed = "{REV}"
builds = []
why = "the judge"
""")
    (repo / ".github" / "workflows" / "audit.yml").write_text(
        f"name: audit\nenv:\n  E: {REV}\njobs:\n  a:\n    steps:\n"
        f"      - uses: {ACTION}\n"
        f"        with:\n          image: {DIGEST}\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)
    r = run(repo)
    assert r.returncode == 1
    assert "declared and never runs" in r.stderr


def test_builds_that_omits_what_the_site_builds_is_a_finding(repo: Path) -> None:
    """The watch set is derived from `builds`, so shrinking `builds` is the dodge."""
    write_registry(repo, COMPLETE + f"""
[[pin]]
id = "engine"
value = "{REV}"
sites = [".github/workflows/audit.yml"]
repo = "stellarfeline/delvewright"
policy = "track"
judged_by = ".github/workflows/audit.yml"
reviewed = "{REV}"
builds = []
why = "the judge"
""")
    (repo / ".github" / "workflows" / "audit.yml").write_text(
        f"name: audit\nenv:\n  E: {REV}\njobs:\n  a:\n    steps:\n"
        f"      - uses: {ACTION}\n"
        f"        with:\n          image: {DIGEST}\n"
        f"      - run: python3 tools/check-pins.py --online engine\n"
        f"      - run: cargo build -p delvewright-admit --release\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)
    r = run(repo)
    assert r.returncode == 1
    assert "`builds` does not name it" in r.stderr


def test_a_checkout_at_a_branch_is_a_pin_too(repo: Path) -> None:
    """A cross-repo checkout carries no hex and is still the loosest pin there is."""
    (repo / ".github" / "workflows" / "audit.yml").write_text(
        f"name: audit\njobs:\n  a:\n    steps:\n"
        f"      - uses: {ACTION}\n"
        f"        with:\n"
        f"          repository: stellarfeline/delvewright\n"
        f"          ref: main\n"
        f"          image: {DIGEST}\n",
        encoding="utf-8",
    )
    write_registry(repo, COMPLETE)
    r = run(repo)
    assert r.returncode == 1
    assert "stellarfeline/delvewright@main" in r.stderr


# ---------------------------------------------------------------------------
# A value with no shape: the class where "nothing escapes discovery" and "every
# held version has a recorded decision" stop being the same claim.
#
# Deliberately not a real toolchain number. A version literal spelled the way the
# repo's own is would be true of this tree as well as the fixture's, and a test
# that can pass for a reason other than the one it names is the calibration trap.
# ---------------------------------------------------------------------------
TOOLVER = "9.87.6"
BINDER = "tools/hold-versions.sh"
STATED_SITES = ("versions.toml", "crates/app/Cargo.toml")


def with_a_stated_pin(
    repo: Path,
    *,
    binder_body: str | None = None,
    key: str = "toolchain_version",
    sites: tuple[str, ...] = STATED_SITES,
    at: dict[str, str] | None = None,
    binder: str = BINDER,
    write_binder: bool = True,
    runs: bool = True,
) -> None:
    """A pin named by a version string, with the binder the class owes.

    `at` is what each site actually holds, so a caller can move one of them apart
    from the rest — which is the drift the registry exists to catch.
    """
    at = at or {s: TOOLVER for s in sites}
    for site, held in at.items():
        add_file(repo, site, f'# fixture consumer\nversion = "{held}"\n')
    if binder_body is None:
        binder_body = (
            "#!/usr/bin/env bash\n"
            f"# reads {key} out of the manifest and asserts every consumer holds it\n"
            + "".join(f'want_in "$ROOT/{s}"\n' for s in sites)
        )
    if write_binder:
        add_file(repo, binder, binder_body)
    if runs:
        add_file(
            repo,
            ".github/workflows/hold.yml",
            f"name: hold\njobs:\n  a:\n    steps:\n      - run: bash {binder}\n",
        )
    write_registry(
        repo,
        COMPLETE
        + f"""
[[pin]]
id = "toolchain"
value = "{TOOLVER}"
sites = {list(sites)!r}
policy = "immutable"
bound_by = "{binder}"
bound_key = "{key}"
why = "the compiler every artifact is built with"
""",
    )


def test_a_stated_pin_with_a_verified_binder_passes(repo: Path) -> None:
    with_a_stated_pin(repo)
    r = run(repo)
    assert r.returncode == 0, r.stdout + r.stderr


def test_a_version_string_with_no_binder_is_a_finding(repo: Path) -> None:
    """`sites` alone, for a value nothing discovers, is an untestable claim."""
    add_file(repo, "versions.toml", f'version = "{TOOLVER}"\n')
    write_registry(
        repo,
        COMPLETE
        + f"""
[[pin]]
id = "toolchain"
value = "{TOOLVER}"
sites = ["versions.toml"]
policy = "immutable"
why = "the compiler every artifact is built with"
""",
    )
    r = run(repo)
    assert r.returncode == 1
    assert "carries no pin shape" in r.stderr


def test_a_binder_that_never_reads_the_key_is_a_finding(repo: Path) -> None:
    """Required guard: a binder that does not bind is a red, or this is prose.

    The named file exists, is run by a workflow, and names every site — and it
    never reads the key, so the sites it lists agree only by intention.
    """
    with_a_stated_pin(
        repo,
        binder_body="#!/usr/bin/env bash\n"
        + "".join(f'want_in "$ROOT/{s}"\n' for s in STATED_SITES),
    )
    r = run(repo)
    assert r.returncode == 1
    assert "never reads `toolchain_version`" in r.stderr


def test_a_binder_that_does_not_name_a_site_is_a_finding(repo: Path) -> None:
    """A site no binder names is held by nobody, which is the state the entry denies."""
    with_a_stated_pin(
        repo,
        binder_body="#!/usr/bin/env bash\n"
        "# reads toolchain_version out of the manifest\n"
        f'want_in "$ROOT/{STATED_SITES[0]}"\n',
    )
    r = run(repo)
    assert r.returncode == 1
    assert f"never names {STATED_SITES[1]}" in r.stderr


def test_a_binder_no_workflow_runs_is_a_finding(repo: Path) -> None:
    """A gate nothing invokes is not a gate — the same demand `judged_by` makes."""
    with_a_stated_pin(repo, runs=False)
    r = run(repo)
    assert r.returncode == 1
    assert "declared and never runs" in r.stderr


def test_a_binder_that_is_not_a_file_is_a_finding(repo: Path) -> None:
    with_a_stated_pin(repo, binder="tools/no-such-checker.sh", write_binder=False)
    r = run(repo)
    assert r.returncode == 1
    assert "not a readable file" in r.stderr


def test_a_stated_site_that_lost_the_value_is_a_finding(repo: Path) -> None:
    """The drift the whole registry exists to catch: one of the sites moves.

    Nothing discovers this value, so the site claim has to be asserted directly.
    Losing this would be the weakening, not the repair.
    """
    with_a_stated_pin(
        repo, at={STATED_SITES[0]: TOOLVER, STATED_SITES[1]: "9.88.0"}
    )
    r = run(repo)
    assert r.returncode == 1
    assert f"declares site {STATED_SITES[1]}" in r.stderr
    assert "is not there any more" in r.stderr


def test_a_site_holding_a_longer_version_does_not_count_as_agreement(
    repo: Path,
) -> None:
    """`9.87.6` sits inside `9.87.65`, and a substring test would call that equal."""
    with_a_stated_pin(
        repo, at={STATED_SITES[0]: TOOLVER, STATED_SITES[1]: TOOLVER + "5"}
    )
    r = run(repo)
    assert r.returncode == 1
    assert f"declares site {STATED_SITES[1]}" in r.stderr


def test_a_shaped_value_may_not_declare_a_binder(repo: Path) -> None:
    """The arm is decided by the object. A digest is found wherever it sits.

    Letting an entry choose would make the effective obligation the disjunction
    of the two arms, and only as strong as the weaker one.
    """
    add_file(repo, BINDER, "#!/usr/bin/env bash\n# image, .github/workflows/audit.yml\n")
    add_file(
        repo,
        ".github/workflows/hold.yml",
        f"name: hold\njobs:\n  a:\n    steps:\n      - run: bash {BINDER}\n",
    )
    write_registry(
        repo,
        f"""
[[pin]]
id = "fetcher"
value = "{ACTION}"
sites = [".github/workflows/audit.yml"]
policy = "floating"
why = "held at its major tag"

[[pin]]
id = "image"
value = "{DIGEST}"
sites = [".github/workflows/audit.yml"]
policy = "immutable"
bound_by = "{BINDER}"
bound_key = "image"
why = "third-party bytes"
""",
    )
    r = run(repo)
    assert r.returncode == 1
    assert "does not get to pick the weaker one" in r.stderr


def test_a_keyed_manifest_field_is_discovered_though_its_value_has_no_shape(
    repo: Path,
) -> None:
    """The other half of the gap: an unregistered version pin must still red.

    rustup fetches the channel this file names, so the SITE carries the shape the
    value does not. Without this, deleting the entry would restore green — which
    is the weakening the registry exists to prevent.
    """
    write_registry(repo, COMPLETE)
    add_file(repo, "rust-toolchain.toml", f'[toolchain]\nchannel = "{TOOLVER}"\n')
    r = run(repo)
    assert r.returncode == 1
    assert f"unregistered pin {TOOLVER} in rust-toolchain.toml" in r.stderr


def test_a_keyed_manifest_this_tool_cannot_read_is_a_finding(repo: Path) -> None:
    """Discovering nothing is how an unregistered pin passes, so it is not a pass."""
    write_registry(repo, COMPLETE)
    add_file(repo, "rust-toolchain.toml", "# the channel moved somewhere else\n")
    r = run(repo)
    assert r.returncode == 1
    assert "has no `toolchain.channel`" in r.stderr


# ---------------------------------------------------------------------------
# A verb is read in the language of the file it is found in.
#
# The false positives first. Each of these is a DIRECTIVE of some configuration
# language sitting in a Rust source file, where it is prose. The first is the
# live one: a compiler diagnostic whose all-caps emphasis wraps onto a new line,
# read as an unregistered Dockerfile stage by a uniformly-applied pattern.
# ---------------------------------------------------------------------------
def test_a_dockerfile_directive_wrapped_into_rust_prose_is_not_a_fetch_site(
    repo: Path,
) -> None:
    write_registry(repo, COMPLETE)
    add_file(
        repo,
        "crates/compiler/src/nav.rs",
        'pub const SOFT_LOCK: &str = "\\\n'
        "             What this red claims is what declarations can carry: \\\n"
        "             NOTHING THIS CAMPAIGN DECLARES SEPARATES THIS RETRY \\\n"
        "             FROM A SOFT-LOCK — whether the loop is winnable is a \\\n"
        '             combat question this compiler refuses to simulate.";\n',
    )
    r = run(repo)
    assert r.returncode == 0, r.stdout + r.stderr


def test_a_workflow_directive_quoted_in_rust_is_not_a_fetch_site(repo: Path) -> None:
    write_registry(repo, COMPLETE)
    add_file(
        repo,
        "crates/compiler/src/docs.rs",
        "/// The shape a workflow step takes:\n"
        'pub const STEP: &str = r#"\n'
        "    - uses: " + ACTION + "\n"
        '"#;\n',
    )
    r = run(repo)
    assert r.returncode == 0, r.stdout + r.stderr


def test_a_cargo_dependency_line_quoted_in_rust_is_not_a_fetch_site(
    repo: Path,
) -> None:
    write_registry(repo, COMPLETE)
    add_file(
        repo,
        "crates/compiler/src/manifest.rs",
        'pub const EXAMPLE: &str = r#"\n'
        '    delvewright-grammar = { git = "https://example.invalid/g" }\n'
        '"#;\n',
    )
    r = run(repo)
    assert r.returncode == 0, r.stdout + r.stderr


# ---------------------------------------------------------------------------
# The true positives, which are what make the narrowing a repair rather than an
# exemption. A `.rs` file is not safe by being Rust: it can spawn a process.
# ---------------------------------------------------------------------------
def test_a_rust_file_that_really_runs_docker_is_a_finding(repo: Path) -> None:
    write_registry(repo, COMPLETE)
    add_file(
        repo,
        "crates/orchestrator/src/stage.rs",
        "pub fn stage() {\n"
        '    Command::new("sh").arg("-c").arg("docker run --rm base:latest");\n'
        "}\n",
    )
    r = run(repo)
    assert r.returncode == 1
    assert "crates/orchestrator/src/stage.rs" in r.stderr
    assert "no FETCH_SITES pattern covers it" in r.stderr


def test_a_rust_file_that_clones_a_repository_is_a_finding(repo: Path) -> None:
    write_registry(repo, COMPLETE)
    add_file(
        repo,
        "crates/orchestrator/src/fetch.rs",
        "pub fn fetch() {\n"
        '    Command::new("sh").arg("-c").arg("git clone https://example.invalid/r");\n'
        "}\n",
    )
    r = run(repo)
    assert r.returncode == 1
    assert "crates/orchestrator/src/fetch.rs" in r.stderr


def test_a_dockerfile_the_site_list_does_not_name_is_a_finding(repo: Path) -> None:
    """The directive keeps its meaning in a file that IS a Dockerfile.

    `build/base.dockerfile` is a Dockerfile by name and no `FETCH_SITES` pattern
    reaches it, which is precisely the escape the enumeration exists to close.
    """
    write_registry(repo, COMPLETE)
    add_file(repo, "build/base.dockerfile", "FROM alpine:3.20\nRUN true\n")
    r = run(repo)
    assert r.returncode == 1
    assert "build/base.dockerfile" in r.stderr


def test_a_file_of_unrecognised_kind_is_read_with_every_verb(repo: Path) -> None:
    """Fail-closed: an unknown kind keeps the directives, so it cannot escape.

    This is the property that separates keying a verb to a language from listing
    an exception. A Dockerfile written under a name nobody anticipated still
    reds, because the map says which kinds ARE a language and never which kinds
    are safe.
    """
    write_registry(repo, COMPLETE)
    add_file(repo, "build/image-recipe", "FROM alpine:3.20\nRUN true\n")
    r = run(repo)
    assert r.returncode == 1
    assert "build/image-recipe" in r.stderr


def test_the_fetch_verb_enumeration_states_what_it_examined(repo: Path) -> None:
    """The enumeration's own binding count, so a narrowing shows up as a number.

    Spelled out rather than recomputed from `FETCH_VERBS`: a second copy of the
    implementation's own arithmetic would agree with it however wrong it was.
    Adding or re-keying a verb moves these numbers, and that is the point — the
    change is meant to be looked at rather than absorbed.
    """
    write_registry(repo, COMPLETE)
    add_file(repo, "crates/compiler/src/nav.rs", "pub fn nav() {}\n")
    r = run(repo)
    assert r.returncode == 0, r.stdout + r.stderr
    line = next(
        ln for ln in r.stdout.splitlines() if ln.startswith("-- fetch-verb enumeration")
    )
    applications = int(line.split()[3])
    files = int(line.split("over ")[1].split()[0])
    # Two files are uncovered: the registry and `nav.rs` (the workflow is a
    # site). `pins.toml` is TOML, so it keeps both commands and the Cargo
    # directive — three. `nav.rs` is Rust, which is none of the three directive
    # languages, so it keeps the two commands only.
    assert files == 2, line
    assert applications == 5, line


def test_an_enumeration_that_applies_no_verb_is_a_finding(tmp_path: Path) -> None:
    """A zero here is the gate going dark, and the surrounding pins still bind.

    Every tracked file is either a fetch site or prose, so nothing is left for
    the enumeration to read — the state in which a new kind of fetch site would
    escape unseen. The registry is held outside the tree so that it is not itself
    the one file being examined.
    """
    (tmp_path / "repo" / ".github" / "workflows").mkdir(parents=True)
    (tmp_path / "repo" / ".github" / "workflows" / "audit.yml").write_text(
        "name: audit\njobs:\n  a:\n    steps:\n      - uses: " + ACTION + "\n",
        encoding="utf-8",
    )
    (tmp_path / "repo" / "README.md").write_text("prose\n", encoding="utf-8")
    subprocess.run(["git", "-C", str(tmp_path / "repo"), "init", "-q"], check=True)
    subprocess.run(["git", "-C", str(tmp_path / "repo"), "add", "-A"], check=True)
    registry = tmp_path / "pins.toml"
    registry.write_text(
        f'[[pin]]\nid = "fetcher"\nvalue = "{ACTION}"\n'
        'sites = [".github/workflows/audit.yml"]\n'
        'policy = "floating"\nwhy = "held at its major tag"\n',
        encoding="utf-8",
    )
    r = run(tmp_path / "repo", "--registry", str(registry))
    assert "binding: 1 pin(s)" in r.stdout
    assert r.returncode == 1
    assert "applied no verb to any file" in r.stderr


def test_this_repos_own_registry_is_complete() -> None:
    r = run(REPO)
    assert r.returncode == 0, r.stdout + r.stderr


def test_this_repos_own_registry_reds_when_a_pin_leaves_it(tmp_path: Path) -> None:
    """The other direction. A registry that cannot fail is not a record.

    The real tree, judged against a registry with one entry taken out. Which
    entry is read from the registry itself rather than named here, so this stays
    true when the inventory changes — a test that hardcodes today's pins is a
    second copy of the record, and a second copy drifts.
    """
    import tomllib

    registry = REPO / ".github" / "pins.toml"
    with registry.open("rb") as fh:
        pins = tomllib.load(fh)["pin"]
    assert pins, "the repo's own registry is empty"
    victim = pins[0]

    text = registry.read_text(encoding="utf-8")
    cut = text.index(f'id = "{victim["id"]}"')
    start = text.rindex("[[pin]]", 0, cut)
    end = text.index("[[pin]]", cut)
    doctored = tmp_path / "pins.toml"
    doctored.write_text(text[:start] + text[end:], encoding="utf-8")

    r = run(REPO, "--registry", str(doctored))
    assert r.returncode == 1
    assert f"unregistered pin {victim['value']}" in r.stderr


# ---------------------------------------------------------------------------
# Discovery by schema, for the three kinds of value that have no shape at all.
#
# `rust-toolchain.toml` closed one such gap by reading a manifest's KEY. These
# three are the same move where the thing that fixes the meaning is a Python
# package manifest, an ACTION'S INPUT CONTRACT, or an install command the repo
# runs. Each is a positive claim — about a file kind, about what an action is
# for, about what installing IS — so the set can only be incomplete in the
# direction of discovering less, and no author escapes an obligation by it
# growing.
#
# The strings below are assembled rather than spelled out for the same reason
# the fixtures at the top of this file are: a `uses:` line written in full HERE
# would be found by discovery in this repository and reported as a site of a real
# pin, correctly, by the tool's own rule.
# ---------------------------------------------------------------------------
SETUP_NODE = "uses: actions/setup-" + "node@v4"
TOOLVER_ALT = "7.7.7"


def _job_selecting_node(version: str) -> str:
    return (
        "name: probe\njobs:\n  a:\n    steps:\n"
        f"      - {SETUP_NODE}\n"
        f'        with:\n          node-version: "{version}"\n'
    )


def test_an_action_input_is_discovered_though_its_value_has_no_shape(
    repo: Path,
) -> None:
    """The setup action exists in order to fetch what its input names.

    `24` is two characters that could be anything; what makes it a fetched
    version is the contract of the action being handed it. Without this, a
    toolchain selection was a pin no scan could see and no entry had to record.
    """
    write_registry(repo, COMPLETE)
    add_file(repo, ".github/workflows/probe.yml", _job_selecting_node("24"))
    r = run(repo)
    assert r.returncode == 1
    assert "unregistered pin 24 in .github/workflows/probe.yml" in r.stderr


def test_an_action_input_is_not_read_out_of_a_file_of_another_language(
    repo: Path,
) -> None:
    """The step body is a YAML directive, and prose in a Rust file."""
    write_registry(repo, COMPLETE)
    add_file(
        repo,
        "crates/probe/src/lib.rs",
        "/// Documentation quoting a workflow step:\n"
        f"/// {SETUP_NODE}\n///   with:\n///     node-version: \"{TOOLVER_ALT}\"\n"
        "pub const N: u8 = 0;\n",
    )
    r = run(repo)
    assert r.returncode == 0, r.stdout + r.stderr


def test_an_exact_requirement_in_a_python_manifest_is_discovered(
    repo: Path,
) -> None:
    """A requirements file states what pip fetches, like any other manifest."""
    write_registry(repo, COMPLETE)
    add_file(repo, "tools/probe/requirements.txt", f"somepkg=={TOOLVER_ALT}\n")
    r = run(repo)
    assert r.returncode == 1
    assert f"unregistered pin {TOOLVER_ALT}" in r.stderr


def test_an_exact_dependency_in_a_pyproject_is_discovered(repo: Path) -> None:
    write_registry(repo, COMPLETE)
    add_file(
        repo,
        "tools/probe/pyproject.toml",
        "[project]\nname = \"probe\"\n"
        f'dependencies = ["somepkg=={TOOLVER_ALT}"]\n',
    )
    r = run(repo)
    assert r.returncode == 1
    assert f"unregistered pin {TOOLVER_ALT}" in r.stderr


@pytest.mark.parametrize("operator", [">=", "~=", ">", "!="])
def test_a_range_in_a_manifest_is_not_a_version(repo: Path, operator: str) -> None:
    """The half that keeps this a rule about the object rather than an exemption.

    A manifest states a REQUIREMENT, and a range is a legitimate way to state
    one — a `build-system` floor of `setuptools>=68` names no version, so there
    is nothing to discover and nothing an entry could record. Reading it as a pin
    would demand a decision about a value that does not exist, which is how a
    correct gate teaches people to write exemptions.
    """
    write_registry(repo, COMPLETE)
    add_file(
        repo, "tools/probe/requirements.txt", f"somepkg{operator}{TOOLVER_ALT}\n"
    )
    r = run(repo)
    assert r.returncode == 0, r.stdout + r.stderr


def test_a_pyproject_this_tool_cannot_read_is_a_finding(repo: Path) -> None:
    """Discovering nothing is how an unregistered pin passes, so it is not a pass."""
    write_registry(repo, COMPLETE)
    add_file(repo, "tools/probe/pyproject.toml", "[project\nname = broken\n")
    r = run(repo)
    assert r.returncode == 1
    assert "is a package manifest, and it is not parseable TOML" in r.stderr


# ---------------------------------------------------------------------------
# An install is an ACT, and an act that names no version is a fetch nobody
# pinned. This is the general form of the live defect: `beet`, which
# re-validates every emitted mcfunction inside a REQUIRED status check, was
# installed unpinned on the same command line as a pinned `mecha` — a frozen
# measurement standing beside an instrument that was not frozen.
# ---------------------------------------------------------------------------
@pytest.mark.parametrize(
    ("rel", "body"),
    [
        (
            ".github/workflows/probe.yml",
            "name: probe\njobs:\n  a:\n    steps:\n"
            '      - run: pip install "pinned==1.2.3" somepkg\n',
        ),
        (
            "tools/probe-install.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\n"
            'pip install "pinned==1.2.3" somepkg\n',
        ),
    ],
    ids=["workflow-run-step", "shell-script"],
)
def test_an_install_naming_no_version_is_a_finding(
    repo: Path, rel: str, body: str
) -> None:
    write_registry(repo, COMPLETE)
    add_file(repo, rel, body)
    r = run(repo)
    assert r.returncode == 1
    assert "installs `somepkg` without naming a version" in r.stderr
    # ...and the pinned package on the SAME line is discovered as an ordinary
    # pin, so the finding is about the argument and not about the command.
    assert "unregistered pin 1.2.3" in r.stderr


def test_an_install_line_printed_by_a_program_is_prose(repo: Path) -> None:
    """The third language case, and the one that would have made this a nuisance.

    `pip install` is a shell command line, so it is an invocation in a shell
    script and in a workflow's `run:` block — and in a Python file the identical
    characters are what a program PRINTS to tell a creator what to install, for
    an optional backend the repository installs nowhere and CI never uses. Read
    uniformly, the rule would demand a pin for a package this project does not
    depend on, which is exactly the pressure that produces an exception list.
    """
    write_registry(repo, COMPLETE)
    add_file(
        repo,
        "tools/probe_advice.py",
        '"""Advice printed for a creator."""\n'
        'INSTALL = "pip install somepkg  # MIT"\n',
    )
    r = run(repo)
    assert r.returncode == 0, r.stdout + r.stderr


def test_a_continued_install_command_is_read_whole(repo: Path) -> None:
    """A command split across lines is one command.

    Reading only its first line would find fewer packages than are installed,
    which is truncation faking coverage — and it fakes it in the direction that
    reads as a clean pass.
    """
    write_registry(repo, COMPLETE)
    add_file(
        repo,
        "tools/probe-install.sh",
        "#!/usr/bin/env bash\npip install \\\n    somepkg\n",
    )
    r = run(repo)
    assert r.returncode == 1
    assert "installs `somepkg` without naming a version" in r.stderr


def test_two_entries_may_not_claim_one_value(repo: Path) -> None:
    """Discovery is keyed by the value, so a collision would erase an entry.

    Not an error but a plausible wrong answer: the site sets merge, one entry
    stops being checked at all, and the registry reads complete. Two unrelated
    things at the same version is an ordinary state of the world, so it reds here
    and is repaired by making the value distinguishable — never by dropping one.
    """
    write_registry(
        repo,
        COMPLETE
        + f"""
[[pin]]
id = "twin"
value = "{DIGEST}"
sites = [".github/workflows/audit.yml"]
policy = "immutable"
why = "the same bytes, claimed twice"
""",
    )
    r = run(repo)
    assert r.returncode == 1
    assert "is also declared by image" in r.stderr
