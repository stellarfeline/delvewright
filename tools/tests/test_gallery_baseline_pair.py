"""The baseline computed in the run (ADR-0034), and the pair that reads it.

Every red condition of `tools/ci/gallery-baseline.py` is perturbed toward and
must red; the report and green outcomes must not. The pair is
`gallery-build.py`, which proves its tree against the manifests the run
measured: handed manifests that are not the run's, it must refuse.
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import subprocess
import sys

import pytest

TOOLS = pathlib.Path(__file__).resolve().parents[1]
REPO = TOOLS.parent


def _load(name: str, rel: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS / rel)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


GB = _load("gallery_baseline", "ci/gallery-baseline.py")
BUILD = _load("gallery_build", "ci/gallery-build.py")


def _manifest(outputs: dict, **fields) -> dict:
    base = {"campaign_id": "gallery", "dsl_version": "0.1.0", "inputs": {"world.json": "w"}, "outputs": outputs}
    return {**base, **fields}


BASE = {"primary.en": _manifest({"datapack/data/a.mcfunction": "1", "structures/x.nbt": "2"})}
EMPTY = {"added": [], "removed": [], "changed": [], "classes": {}}
MOVED = {"primary.en": _manifest({"datapack/data/a.mcfunction": "9", "structures/x.nbt": "2"})}


# ------------------------------------------------------------ the red conditions


def test_a_moved_emission_on_a_change_reaching_no_input_is_a_determinism_red():
    delta = GB.compute_delta(BASE, MOVED)
    assert delta["changed"] == ["primary.en:datapack/data/a.mcfunction"]
    touched = GB.emission_inputs_touched(["docs/reference/tools.md", "versions.toml"])
    kind, reds = GB.verdict(delta, [], touched, [])
    assert kind == "red"
    assert any("ADR-0006" in r for r in reds)


@pytest.mark.parametrize(
    "path",
    [
        "crates/delvec/src/main.rs",
        "gallery/world.json",
        "gallery/prefab-generator/src/main.rs",
        "Cargo.lock",
        "rust-toolchain.toml",
        "tools/ci/gallery_domain.py",
        "tools/ci/gallery-prefabs.py",
    ],
)
def test_the_same_delta_on_a_change_reaching_an_input_is_a_report(path):
    delta = GB.compute_delta(BASE, MOVED)
    assert GB.verdict(delta, [], GB.emission_inputs_touched([path]), []) == ("report", [])


def test_a_moved_recorded_value_alone_is_a_delta():
    """A manifest is not its outputs: an `inputs` entry moving is a moved delta."""
    head = {"primary.en": _manifest(BASE["primary.en"]["outputs"], inputs={"world.json": "v"})}
    delta = GB.compute_delta(BASE, head)
    fields = GB.manifest_field_delta(BASE, head)
    assert delta == EMPTY and fields == ["~ primary.en.inputs[world.json]: `w` -> `v`"]
    assert GB.verdict(delta, fields, [], [])[0] == "red"
    assert GB.verdict(delta, fields, ["gallery/world.json"], [])[0] == "report"


def test_a_build_on_one_side_only_is_a_delta():
    head = {**BASE, "easy.en": _manifest({"datapack/data/b.mcfunction": "3"})}
    assert GB.compute_delta(BASE, head)["added"] == ["easy.en:datapack/data/b.mcfunction"]
    assert GB.manifest_field_delta(BASE, head) == ["+ build `easy.en`: in the head, not in the comparison"]


def test_warning_ledger_drift_is_a_red_even_on_an_emission_change():
    ledger = {"primary.en": [{"code": "DW0001", "pointer": "/a", "stage": "world"}]}
    drift = GB.warning_delta(ledger, {"primary.en": []})
    assert drift and drift[0].startswith("- primary.en  x1 -> x0")
    kind, reds = GB.verdict(EMPTY, [], ["crates/x.rs"], drift)
    assert kind == "red" and any("criterion 5" in r for r in reds)


def test_a_duplicated_warning_is_drift_though_the_sets_agree():
    row = {"code": "DW0001", "pointer": "/a", "stage": "world"}
    assert GB.warning_delta({"p.en": [row]}, {"p.en": [row, row]}) == [
        f"+ p.en  x1 -> x2  {json.dumps(row, sort_keys=True)}"
    ]


def test_nothing_moved_is_green():
    assert GB.verdict(EMPTY, [], [], []) == ("green", [])


@pytest.mark.parametrize("side", ["head", "compare"])
def test_a_failed_build_on_either_side_is_a_red_never_a_fallback(tmp_path, side):
    failed = subprocess.CompletedProcess(args=[], returncode=10, stdout="", stderr="DW0001 [error] boom")
    with pytest.raises(GB.Red) as e:
        GB.read_build(side, None, "en", tmp_path, failed)
    assert e.value.verdict == f"build failed ({side})"


def test_an_unparseable_warning_is_a_red(tmp_path):
    (tmp_path / "manifest.json").write_text("{}")
    r = subprocess.CompletedProcess(args=[], returncode=0, stdout="DW9999 [warning] no-colon-anywhere\n", stderr="")
    with pytest.raises(GB.Red):
        GB.read_build("head", None, "en", tmp_path, r)


def test_a_vacuous_side_is_a_red(monkeypatch, tmp_path):
    """A side measuring zero emitted paths reds before anything is compared."""

    class Args:
        base = "origin/main"
        compare = None
        compare_target_dir = None

    monkeypatch.setattr(GB, "Tree", lambda *a: None)
    monkeypatch.setattr(GB, "comparison_commit", lambda *a: ("c" * 40, "test"))
    monkeypatch.setattr(GB, "prepare_comparison", lambda *a: (tmp_path / "compare", "d", "p"))
    monkeypatch.setattr(GB, "reclaim_comparison", lambda *a: None)
    sides = iter([
        {"header": {}, "manifests": BASE, "warnings": {}},
        {"header": {}, "manifests": {"primary.en": _manifest({})}, "warnings": {}},
    ])
    monkeypatch.setattr(GB, "measure", lambda *a: next(sides))
    with pytest.raises(GB.Red) as e:
        GB.run_compare(Args(), pathlib.Path("d"), pathlib.Path("p"), tmp_path)
    assert e.value.verdict == "vacuous (head)"


def test_the_emission_inputs_are_prefixes_not_text_matches():
    assert GB.emission_inputs_touched(["crates.bak/x", "gallery-prefabs/x.nbt", "Cargo.toml.orig"]) == []
    assert "versions.toml" not in GB.EMISSION_INPUTS, "the compiler reads nothing from it"


# --------------------------------------------------------- the comparison commit


def test_the_comparison_is_the_merge_base_and_on_main_the_parent(tmp_path, monkeypatch):
    repo = tmp_path / "r"
    repo.mkdir()

    def g(*a):
        r = subprocess.run(["git", "-C", str(repo), *a], check=True, capture_output=True, text=True)
        return r.stdout.strip()

    g("init", "-q", "-b", "main")
    g("config", "user.email", "t@example.invalid")
    g("config", "user.name", "t")
    for n in ("one", "two"):
        (repo / n).write_text(n)
        g("add", n)
        g("commit", "-q", "-m", n)
    two = g("rev-parse", "HEAD")
    one = g("rev-parse", "HEAD^1")
    g("update-ref", "refs/remotes/origin/main", two)
    monkeypatch.setattr(GB, "REPO", repo)
    assert GB.comparison_commit("origin/main", None) == (one, "HEAD^1 (HEAD is on `origin/main`)")
    g("checkout", "-q", "-b", "feature")
    (repo / "three").write_text("3")
    g("add", "three")
    g("commit", "-q", "-m", "three")
    assert GB.comparison_commit("origin/main", None) == (two, "merge-base(`origin/main`, HEAD)")
    assert GB.changed_paths(two) == ["three"]


def test_each_tree_is_read_through_its_own_modules(tmp_path):
    """A tree's `gallery_domain` is loaded from that tree, never from head's."""
    fake = tmp_path / "t"
    (fake / "tools" / "ci").mkdir(parents=True)
    (fake / "gallery" / "overlays" / "only-here").mkdir(parents=True)
    (fake / "gallery" / "world.json").write_text(json.dumps({"content": {"languages": ["fr"]}}))
    for name in ("gallery_domain.py", "gallery_units.py"):
        (fake / "tools" / "ci" / name).write_text((TOOLS / "ci" / name).read_text())
    tree = GB.Tree(fake, "compare")
    assert pathlib.Path(tree.domain.__file__).resolve() == (fake / "tools" / "ci" / "gallery_domain.py").resolve()
    assert tree.points() == [(None, "en"), (None, "fr"), ("only-here", "en")]


# ------------------------------------------------------------- what is committed


def test_write_writes_only_the_ledger_and_refuses_a_noise_write(monkeypatch, tmp_path):
    ledger = tmp_path / "warnings.json"
    rows = {"primary.en": [{"code": "DW0001", "pointer": "/a", "stage": "world"}]}
    GB.write_canonical(ledger, rows)
    before = ledger.read_bytes()
    monkeypatch.setattr(GB, "LEDGER", ledger)
    monkeypatch.setattr(GB, "REPO", tmp_path)
    monkeypatch.setattr(GB, "Tree", lambda *a: None)
    monkeypatch.setattr(GB, "measure", lambda *a: {"header": {}, "manifests": BASE, "warnings": rows})
    with pytest.raises(GB.Red) as e:
        GB.run_write(pathlib.Path("delvec"), pathlib.Path("prefabs"))
    assert e.value.verdict == "noise write refused"
    assert ledger.read_bytes() == before
    moved = {"primary.en": []}
    monkeypatch.setattr(GB, "measure", lambda *a: {"header": {}, "manifests": BASE, "warnings": moved})
    assert GB.run_write(pathlib.Path("delvec"), pathlib.Path("prefabs")) == 0
    assert json.loads(ledger.read_text()) == moved
    assert sorted(p.name for p in tmp_path.iterdir()) == ["warnings.json"]


# --------------------------------------------- the pair: gallery-build.py proves


def _doc(tmp_path, tree: str) -> pathlib.Path:
    p = tmp_path / "manifests.json"
    p.write_text(json.dumps({"tree": tree, "manifests": BASE}))
    return p


def test_the_rebuild_reads_the_runs_manifests(tmp_path):
    assert BUILD.baseline_row(_doc(tmp_path, "abc"), "primary", "en", head="abc") == BASE["primary.en"]


def test_the_rebuild_refuses_manifests_that_are_not_the_runs(tmp_path, capsys):
    with pytest.raises(SystemExit):
        BUILD.baseline_row(_doc(tmp_path, "abc"), "primary", "en", head="def")
    assert "not this run's" in capsys.readouterr().err


def test_the_rebuild_refuses_the_old_committed_shape(tmp_path, capsys):
    """A bare build-id map names no commit, so nothing says whose measurement it is."""
    p = tmp_path / "manifests.json"
    p.write_text(json.dumps(BASE))
    with pytest.raises(SystemExit):
        BUILD.baseline_row(p, "primary", "en", head="abc")
    assert "`tree`, `manifests`" in capsys.readouterr().err


def test_the_rebuild_refuses_a_point_the_run_did_not_measure(tmp_path, capsys):
    with pytest.raises(SystemExit):
        BUILD.baseline_row(_doc(tmp_path, "abc"), "site-plan", "en", head="abc")
    assert "hold no row `site-plan.en`" in capsys.readouterr().err


def test_the_report_names_the_commit_it_measured():
    src = (TOOLS / "ci" / "gallery-baseline.py").read_text()
    assert '{"tree": head["header"]["commit"], "manifests": head["manifests"]}' in src


PAGES = (
    "world store pages: 0 byte(s) resident over 0 section(s) of every store; 1200 array(s) given "
    "up, 1100 read back, 9 of them within one turnover of the resident arrays; the need raised 2 "
    "time(s) for a walk found thrashing, to 884736 byte(s); 65536 byte(s) in the scratch file in "
    "regions of 8192 byte(s), at most 8 of them holding a live array at once; at most 9 array(s) "
    "alive at once over every store (ADR-0037 §1)"
)
LEDGER = {"computed_never_measured": True, "scratch_bytes": {"stores": 81920}}


def test_a_scratch_file_within_the_stated_cost_is_green():
    assert BUILD.scratch_within_stated("other: line\n" + PAGES + "\n", LEDGER) == []


def test_a_scratch_file_past_the_stated_cost_is_red():
    reds = BUILD.scratch_within_stated(PAGES, {"computed_never_measured": True, "scratch_bytes": {"stores": 65535}})
    assert len(reds) == 1 and "65536 byte(s) against the 65535" in reds[0]


def test_a_build_without_the_pages_line_binds_nothing_and_is_red():
    assert BUILD.scratch_within_stated("reach budget: nothing\n", LEDGER) == [
        "0 `world store pages:` line(s) on stderr; one is read"
    ]
    assert BUILD.scratch_within_stated(PAGES + "\n" + PAGES, LEDGER)[0].startswith("2 `world store pages:`")


def test_a_ledger_without_the_figure_or_measured_is_red():
    assert BUILD.scratch_within_stated(PAGES, {"computed_never_measured": True}) == [
        "`validation/resource-cost.json` carries no `scratch_bytes.stores`"
    ]
    reds = BUILD.scratch_within_stated(PAGES, {"computed_never_measured": False, "scratch_bytes": {"stores": 0}})
    assert len(reds) == 2 and "computed_never_measured" in reds[0] and "not a positive" in reds[1]


SCRATCH = (
    "reach scratch: 515486071 byte(s) written to the scratch files, at most 438364597 byte(s) on disk "
    "(the sum of each file's longest), 438294121 kept at the end; the lists' and tables' 410355900 written, "
    "at most 334115289 long, 334112479 kept at the end; the bank's 99264699 written, at most 98383836 long, "
    "98316170 kept at the end; the snapshot's 5865472 written, at most 5865472 long, 5865472 kept at the end "
    "(ADR-0037 §2)"
)
REACH_USED = 334115289 + 98383836 + 5865472
REACH_LEDGER = {"computed_never_measured": True, "scratch_bytes": {"reach_graph": REACH_USED}}


def test_the_three_reach_files_within_the_stated_cost_are_green():
    reds, binding = BUILD.reach_scratch_within_stated("other: line\n" + SCRATCH + "\n", REACH_LEDGER)
    assert reds == []
    assert f"{REACH_USED} byte(s) at most on disk" in binding and "the bank's 98383836" in binding


def test_the_reach_files_past_the_stated_cost_are_red_by_one_byte():
    ledger = {"computed_never_measured": True, "scratch_bytes": {"reach_graph": REACH_USED - 1}}
    reds, _ = BUILD.reach_scratch_within_stated(SCRATCH, ledger)
    assert len(reds) == 1 and reds[0].startswith(f"the reach graph's scratch files took {REACH_USED} byte(s)")


def test_the_bank_is_held_to_the_reach_figure():
    big_bank = SCRATCH.replace("at most 98383836 long", "at most 98383837 long")
    reds, _ = BUILD.reach_scratch_within_stated(big_bank, REACH_LEDGER)
    assert len(reds) == 1 and "took " + str(REACH_USED + 1) in reds[0]


def test_a_build_without_the_reach_scratch_line_or_a_file_binds_nothing_and_is_red():
    assert BUILD.reach_scratch_within_stated("reach budget: nothing\n", REACH_LEDGER)[0] == [
        "0 `reach scratch:` line(s) on stderr; one is read"
    ]
    assert BUILD.reach_scratch_within_stated(SCRATCH + "\n" + SCRATCH, REACH_LEDGER)[0][0].startswith("2 `reach scratch:`")
    no_snapshot = SCRATCH.replace("the snapshot's", "the snapshots")
    assert BUILD.reach_scratch_within_stated(no_snapshot, REACH_LEDGER)[0][0].startswith(
        "the reach scratch line names no longest for the snapshot's"
    )


def test_a_ledger_without_the_reach_figure_or_measured_is_red():
    assert BUILD.reach_scratch_within_stated(SCRATCH, {"computed_never_measured": True, "scratch_bytes": {}})[0] == [
        "`validation/resource-cost.json` carries no `scratch_bytes.reach_graph`"
    ]
    reds, _ = BUILD.reach_scratch_within_stated(
        SCRATCH, {"computed_never_measured": False, "scratch_bytes": {"reach_graph": 0}}
    )
    assert len(reds) == 2 and "computed_never_measured" in reds[0] and "not a positive" in reds[1]
