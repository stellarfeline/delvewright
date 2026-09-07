r"""Guard: the pair is judged in the WORKFLOW's order.

`engine-release.yml` runs `tools/check-publishable.sh` and THEN
`tools/crates-io-publish.sh --plan|--publish` in the same job, on the belief
that the second reads what the first packaged. Release re-run 34072383204
(`v1.2.0`, tree `d0505ef9`, workflow from `main` at `d5908698`) is where nothing
had ever run them in that exact order before the tag: `check-publishable.sh`
packaged all eight crates, printed OK, and its `cleanup()` trap removed the
whole verify tree on every exit — success included — so `crates-io-publish.sh
--plan` immediately failed with "no packaged tarball at
.../package-verify/package/delvewright-dsl-0.20.0.crate — run
tools/check-publishable.sh first", three steps after that exact script had just
run. See docs/notes/private/briefs/
BY-the-publish-plan-reads-the-tarballs-the-gate-packaged.md.

This test runs BOTH scripts, in that order, in a scratch clone — offline:
`cargo` is shimmed (a real `cargo package`/`cargo build` of all 8 crates is
already exercised by CI running the real scripts; this test is about the
SHELL-LEVEL lifecycle contract between the two processes, not about proving a
real package builds — that shim shape is `test_check_publishable_cleanup.py`'s
precedent), and the crates.io sparse index is a local, in-process HTTP server
reached through `crates-io-publish.sh`'s `DW_CRATES_INDEX` override — real
crates.io is never asked about our engine crates' names, because whether they
are actually published there changes over calendar time (this project's own
release history) and a test must not depend on that staying true forever.

RED ON d5908698's BEHAVIOUR: that revision's `crates-io-publish.sh` has no way
to reach a fake index at all (`DW_CRATES_INDEX` does not exist there — its bind
test hits the real `https://index.crates.io`, unconditionally), so running IT
offline is exactly the case the brief's fallback names: `test_red_on_the_
original_gate_leaves_no_tarball_for_the_plan` runs only the base revision's
`check-publishable.sh`, frozen as
`tools/tests/fixtures/check-publishable-d5908698.sh` (a `git show` at test time
does not survive CI's shallow checkout — the required-status job running this
suite hit exactly that, `git show d5908698:...` exiting 128 because the commit
is not in the clone — and reaching into git history for a red instrument is the
wrong shape regardless: a frozen measurement names its instrument by exact
revision and CARRIES it, never through an indirection that can vanish).
The test asserts the tarball-presence contract `crates-io-publish.sh`'s
`local_crate_path` checks — the exact `.crate` path is gone after a run that
printed OK, which is the release run's reported error one level down, without
needing the network the old plan would otherwise demand and without invoking
`git` at all.
GREEN AFTER: `test_the_plan_reads_what_the_gate_packaged` runs the CURRENT two
scripts, in sequence, through `DW_CRATES_INDEX`, and asserts `--plan` reports
every engine crate (`delvewright-dsl 0.20.0` and `delvec 1.2.0` included) as
`PUBLISH` (i.e. unpublished, read off the sha256 the gate wrote).
"""

from __future__ import annotations

import http.server
import json
import os
import shutil
import subprocess
import threading
import tomllib
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
LIB = REPO / "tools" / "lib"
# `d5908698:tools/check-publishable.sh`, frozen — see the fixture's own header
# comment for the revision and why it exists. Read as a committed file, never
# via `git show`: CI's shallow checkout does not carry that commit at all (the
# required-status job running this suite: `git show d5908698:...` exited 128,
# "invalid object name"), and a red instrument reached through git history is
# an indirection that can vanish out from under the test regardless of
# checkout depth.
FIXTURE_CHECK_PUBLISHABLE_D5908698 = (
    Path(__file__).resolve().parent / "fixtures" / "check-publishable-d5908698.sh"
)


def _engine_crates(versions_toml: Path) -> list[tuple[str, str]]:
    e = tomllib.loads(versions_toml.read_text(encoding="utf-8"))["engine"]
    out = [(e["dsl_crate"], e["dsl_crate_version"])]
    out += [(n, e["version"]) for n in e["crates"]]
    out.append((e["crate"], e["version"]))
    return out


class _FakeIndex(http.server.BaseHTTPRequestHandler):
    """`/se/rd/serde` (the bind test's `serde 1.0.0`) resolves; everything else
    404s, so every one of our engine crates reads as ABSENT — the answer
    `--plan` needs to select PUBLISH rather than skip or hard-fail.
    """

    def do_GET(self) -> None:  # noqa: N802 (stdlib handler signature)
        if self.path == "/se/rd/serde":
            body = json.dumps({"name": "serde", "vers": "1.0.0", "cksum": "0" * 64}).encode() + b"\n"
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, *_args: object) -> None:  # silence stderr noise
        pass


@pytest.fixture
def fake_index() -> str:
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _FakeIndex)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}"
    finally:
        server.shutdown()
        thread.join(timeout=5)


_CARGO_SHIM = r"""#!/bin/sh
# A fake `cargo` for the two subcommands `check-publishable.sh` invokes: it
# never compiles anything, it only produces the SHAPES both scripts read —
# `package/<name>-<version>/Cargo.toml` (+ `.crate`) and a `delvec` stub binary
# that answers `--version` and `<group> --help`.
set -e
if [ "$1" = "package" ]; then
  shift
  pkg="$CARGO_TARGET_DIR/package"
  mkdir -p "$pkg"
  while [ "$#" -gt 0 ]; do
    if [ "$1" = "-p" ]; then
      name="$2"
      version="$(awk -F'\t' -v n="$name" '$1==n{print $2}' "@MAPFILE@")"
      d="$pkg/$name-$version"
      mkdir -p "$d"
      {
        echo '[package]'
        echo "name = \"$name\""
        echo "version = \"$version\""
        echo 'description = "t"'
        echo 'license = "MIT"'
        echo 'repository = "https://example.com"'
        echo 'readme = "README.md"'
        echo '[dependencies]'
        if [ "$name" != "@DSLNAME@" ]; then
          echo 'delvewright-dsl = "@DSLREQ@"'
        fi
      } > "$d/Cargo.toml"
      ( cd "$pkg" && tar -czf "$name-$version.crate" "$name-$version" )
      shift 2
    else
      shift
    fi
  done
  exit 0
fi
if [ "$1" = "build" ]; then
  mkdir -p target/release
  cat > target/release/delvec <<STUB
#!/bin/sh
if [ "\$1" = "--version" ]; then echo "delvec @ENGINEVERSION@, dsl @DSLVERSION@, mc 1.21.11"; exit 0; fi
if [ "\$2" = "--help" ]; then exit 0; fi
exit 1
STUB
  chmod +x target/release/delvec
  exit 0
fi
echo "fake cargo: unhandled: $*" >&2
exit 90
"""


def _scratch_clone(tmp_path: Path, check_src: str, publish_src: str) -> Path:
    """A tree with only what the two scripts (`ROOT`-relative) need: the two
    scripts themselves (given as SOURCE TEXT, so the red case can hand in
    `d5908698`'s frozen content without ever checking it out in the working
    tree), the shared libs the fixed scripts source, and `versions.toml`.
    """
    tree = tmp_path / "clone"
    (tree / "tools" / "lib").mkdir(parents=True)
    (tree / "tools" / "check-publishable.sh").write_text(check_src, encoding="utf-8")
    (tree / "tools" / "crates-io-publish.sh").write_text(publish_src, encoding="utf-8")
    shutil.copy(LIB / "checksum.sh", tree / "tools" / "lib" / "checksum.sh")
    shutil.copy(LIB / "package-verify.sh", tree / "tools" / "lib" / "package-verify.sh")
    shutil.copy(REPO / "versions.toml", tree / "versions.toml")
    return tree


def _install_fake_cargo(tree: Path, crates: list[tuple[str, str]]) -> dict[str, str]:
    binpath = tree / "bin"
    binpath.mkdir()
    mapfile = binpath / "crate-map.tsv"
    mapfile.write_text("".join(f"{n}\t{v}\n" for n, v in crates), encoding="utf-8")

    dsl_name = crates[0][0]
    dsl_version = crates[0][1]
    engine_version = crates[-1][1]
    e = tomllib.loads((tree / "versions.toml").read_text(encoding="utf-8"))["engine"]
    dsl_req = e["dsl_crate_req"]

    script = (
        _CARGO_SHIM.replace("@MAPFILE@", str(mapfile))
        .replace("@DSLNAME@", dsl_name)
        .replace("@DSLREQ@", dsl_req)
        .replace("@ENGINEVERSION@", engine_version)
        .replace("@DSLVERSION@", dsl_version)
    )
    cargo = binpath / "cargo"
    cargo.write_text(script, encoding="utf-8")
    cargo.chmod(0o755)

    env = {**os.environ, "PATH": f"{binpath}:{os.environ['PATH']}"}
    return env


def _run_sequence(tree: Path, env: dict[str, str], index_base: str) -> tuple[subprocess.CompletedProcess, subprocess.CompletedProcess]:
    check = subprocess.run(
        ["bash", str(tree / "tools" / "check-publishable.sh"), "--allow-dirty"],
        cwd=tree,
        capture_output=True,
        text=True,
        env=env,
    )
    plan_env = {**env, "DW_CRATES_INDEX": index_base}
    plan = subprocess.run(
        ["bash", str(tree / "tools" / "crates-io-publish.sh"), "--plan"],
        cwd=tree,
        capture_output=True,
        text=True,
        env=plan_env,
    )
    return check, plan


def test_the_plan_reads_what_the_gate_packaged(tmp_path: Path, fake_index: str) -> None:
    crates = _engine_crates(REPO / "versions.toml")
    tree = _scratch_clone(
        tmp_path,
        (REPO / "tools" / "check-publishable.sh").read_text(encoding="utf-8"),
        (REPO / "tools" / "crates-io-publish.sh").read_text(encoding="utf-8"),
    )
    env = _install_fake_cargo(tree, crates)

    check, plan = _run_sequence(tree, env, fake_index)

    assert check.returncode == 0, check.stdout + check.stderr
    assert "check-publishable: OK" in check.stdout, check.stdout

    assert plan.returncode == 0, plan.stdout + plan.stderr
    for name, version in crates:
        assert f"PUBLISH {name} {version}" in plan.stdout, (
            f"{name} {version} was not read as unpublished — the plan's own "
            f"output:\n{plan.stdout}"
        )
    assert "crates-io-publish: --plan, nothing was uploaded" in plan.stdout, plan.stdout
    # The DW_CRATES_INDEX override must announce itself; a silent override
    # could survive into a real run unnoticed.
    assert "DW_CRATES_INDEX=" in plan.stderr, plan.stderr


def test_red_on_the_original_gate_leaves_no_tarball_for_the_plan(tmp_path: Path) -> None:
    check_src = FIXTURE_CHECK_PUBLISHABLE_D5908698.read_text(encoding="utf-8")
    # `versions.toml` at HEAD (dsl_crate_version 0.20.0) still names the
    # derivation the base script's own inline python reads, so this is the
    # crate/version set that revision would have decided about — not a frozen
    # copy of an older `versions.toml`.
    crates = _engine_crates(REPO / "versions.toml")
    tree = tmp_path / "clone"
    tree.mkdir()
    (tree / "tools").mkdir()
    (tree / "tools" / "check-publishable.sh").write_text(check_src, encoding="utf-8")
    shutil.copy(REPO / "versions.toml", tree / "versions.toml")
    env = _install_fake_cargo(tree, crates)

    check = subprocess.run(
        ["bash", str(tree / "tools" / "check-publishable.sh"), "--allow-dirty"],
        cwd=tree,
        capture_output=True,
        text=True,
        env=env,
    )
    assert check.returncode == 0, check.stdout + check.stderr
    assert "check-publishable: OK" in check.stdout, check.stdout

    # The exact path `tools/crates-io-publish.sh`'s `local_crate_path` reads —
    # reproduced here, not imported, because at d5908698 that function computed
    # this same formula under `target/package-verify`, and the point is that
    # the FILE is gone, independent of which script's copy of the formula asks.
    missing = [
        f"{name}-{version}.crate"
        for name, version in crates
        if not (tree / "target" / "package-verify" / "package" / f"{name}-{version}.crate").exists()
    ]
    assert missing == [f"{n}-{v}.crate" for n, v in crates], (
        f"expected every tarball gone after a PASSING run (the bug: cleanup() "
        f"ran on every exit, success included) — still present: "
        f"{[f'{n}-{v}.crate' for n, v in crates if f'{n}-{v}.crate' not in missing]}"
    )
