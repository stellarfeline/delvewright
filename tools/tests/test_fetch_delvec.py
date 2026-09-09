"""`fetch-delvec.py`, driven against the shelf it was written for.

The script is the one acquisition on the `/new-delve` page that is not left to
the agent (spec-0063 §3), so every refusal it can return is exercised here by
its exit code — the page's Init table binds to those numbers rather than to
prose, and a code nothing tests is a table row nobody has ever taken.

The `SHA256SUMS` fixture is the file the engine's own release workflow
published, downloaded verbatim; `fixtures/skill-page/README.md` says where from
and why it is the one artifact worth driving the parser with.
"""

from __future__ import annotations

import hashlib
import importlib.util
import io
import pathlib
import subprocess
import sys
import tarfile

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = (
    REPO
    / ".claude"
    / "skills"
    / "delvewright"
    / "skills"
    / "new-delve"
    / "scripts"
    / "fetch-delvec.py"
)
FIXTURE = pathlib.Path(__file__).parent / "fixtures" / "skill-page" / "SHA256SUMS-v1.4.0"

# The five targets `[engine].targets` declares, with the `(system, machine)`
# pair a real interpreter reports on each. Every spelling here is one
# `platform.machine()` actually returns somewhere: macOS on Apple silicon says
# `arm64`, Linux on the same silicon says `aarch64`, and Windows says `AMD64`.
HOSTS = {
    ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
    ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
    ("Darwin", "x86_64"): "x86_64-apple-darwin",
    ("Darwin", "arm64"): "aarch64-apple-darwin",
    ("Windows", "AMD64"): "x86_64-pc-windows-msvc",
}


@pytest.fixture(scope="module")
def mod():
    spec = importlib.util.spec_from_file_location("fetch_delvec", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# --------------------------------------------------------------- the host map --


def test_every_published_target_is_reachable_from_a_real_host(mod):
    """Five pairs, five targets — and the map is the engine's, not this file's.

    The denominator is asserted, not assumed: if the shelf grows a sixth target
    this test still passes on its five and the assertion below is what says the
    census is short.
    """
    got = {pair: mod.host_target(*pair) for pair in HOSTS}
    assert got == HOSTS
    assert len(set(HOSTS.values())) == 5


def test_a_host_outside_the_map_is_a_refusal_and_not_a_guess(mod):
    """The sixth pair, which is none of the five: no target, so the floor."""
    assert mod.host_target("FreeBSD", "riscv64") is None
    assert mod.host_target("Linux", "riscv64") is None
    assert mod.host_target("Haiku", "x86_64") is None


def test_the_declared_targets_are_the_ones_this_map_produces():
    """The map's range is exactly `[engine].targets`, read from the manifest.

    A map that had drifted from the shelf would send one platform at an archive
    that is not published, which is `EXIT_NO_TARGET`'s job — but silently, at
    the wrong moment, for a machine the shelf does serve.
    """
    import tomllib

    targets = set(
        tomllib.loads((REPO / "versions.toml").read_text(encoding="utf-8"))["engine"][
            "targets"
        ]
    )
    assert set(HOSTS.values()) == targets


# ------------------------------------------------------- the SHA256SUMS parser --


def test_both_coreutils_row_forms_are_read_from_the_published_file(mod):
    """The measurement the shell form failed, against the file that failed it."""
    text = FIXTURE.read_text(encoding="utf-8")
    rows = [line for line in text.splitlines() if line.strip()]
    assert len(rows) == 5

    binary_form = [line for line in rows if " *" in line]
    text_form = [line for line in rows if " *" not in line]
    assert len(binary_form) == 1 and len(text_form) == 4
    assert "windows" in binary_form[0]

    for target in HOSTS.values():
        name = f"delvec-v1.4.0-{target}.tar.gz"
        digest = mod.digest_for(text, name)
        assert digest is not None, f"{name} unreadable — the parse missed a row"
        assert len(digest) == 64

    # The pattern the page carried before this script: it matches four of five,
    # and the one it misses is the platform nobody in this project tests on.
    import re

    matched = [
        name
        for target in HOSTS.values()
        if re.search(
            rf" {re.escape(f'delvec-v1.4.0-{target}.tar.gz')}$", text, re.M
        )
        for name in [target]
    ]
    assert len(matched) == 4
    assert "x86_64-pc-windows-msvc" not in matched


def test_a_row_for_another_name_is_not_this_archive(mod):
    text = FIXTURE.read_text(encoding="utf-8")
    assert mod.digest_for(text, "delvec-v1.4.0-sparc-unknown-linux-gnu.tar.gz") is None


# ------------------------------------------------------------- the refusals --


def _archive(version_line: str) -> bytes:
    """A `.tar.gz` carrying one executable `delvec` that prints `version_line`."""
    script = f"#!/bin/sh\necho '{version_line}'\n".encode()
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        info = tarfile.TarInfo("delvec")
        info.size = len(script)
        info.mode = 0o755
        tar.addfile(info, io.BytesIO(script))
    return buf.getvalue()


class _Shelf:
    """A recorded shelf, counting every fetch so a retry cannot go unseen."""

    def __init__(self, archive: bytes, digest: str | None = None) -> None:
        self.archive = archive
        self.digest = digest or hashlib.sha256(archive).hexdigest()
        self.fetched: list[str] = []

    def __call__(self, url: str) -> bytes:
        self.fetched.append(url)
        if url.endswith("SHA256SUMS"):
            name = "delvec-v1.4.0-x86_64-unknown-linux-gnu.tar.gz"
            return f"{self.digest}  {name}\n".encode()
        return self.archive


@pytest.fixture
def rig(mod, tmp_path, monkeypatch):
    """The script pointed at a fake shelf and a fake engine, with no network."""
    pin = tmp_path / "versions.toml"
    pin.write_text(
        '[engine]\nrepo = "stellarfeline/delvewright"\n'
        'release = "v1.4.0"\n'
        f'ref = "{"0" * 40}"\n',  # a placeholder, not a revision: nothing fetches it
        encoding="utf-8",
    )
    monkeypatch.setattr(
        mod, "engine_targets", lambda engine, ref: list(HOSTS.values())
    )
    return pin


def _run(mod, pin, tmp_path, shelf, monkeypatch, system="Linux", machine="x86_64"):
    monkeypatch.setattr(mod, "fetch", shelf)
    return mod.run(pin, tmp_path / "engine", tmp_path / "bin", system, machine)


def test_a_matching_digest_installs_and_reports_what_it_bound(mod, rig, tmp_path, monkeypatch):
    shelf = _Shelf(_archive("delvec 1.4.0, dsl 0.23.0, mc 1.21.11"))
    assert _run(mod, rig, tmp_path, shelf, monkeypatch) == 0
    assert (tmp_path / "bin" / "delvec").is_file()
    assert len(shelf.fetched) == 2  # the sums, then the archive. Never more.


def test_one_perturbed_byte_is_the_checksum_refusal_and_never_a_second_download(
    mod, rig, tmp_path, monkeypatch
):
    """The refusal that may not fall to the source build, and may not retry.

    The perturbation is one byte of the DIGEST — the smallest edit that makes
    the published row disagree with the bytes — because that is the shape a
    corrupted transfer and a substituted archive both take.
    """
    archive = _archive("delvec 1.4.0, dsl 0.23.0, mc 1.21.11")
    real = hashlib.sha256(archive).hexdigest()
    perturbed = ("0" if real[0] != "0" else "1") + real[1:]
    assert perturbed != real and len(perturbed) == len(real)

    shelf = _Shelf(archive, digest=perturbed)
    monkeypatch.setattr(mod, "fetch", shelf)
    with pytest.raises(mod.Refusal) as caught:
        mod.run(rig, tmp_path / "engine", tmp_path / "bin", "Linux", "x86_64")
    assert caught.value.code == mod.EXIT_CHECKSUM
    assert len(shelf.fetched) == 2, "a refused digest must not fetch again"
    assert not (tmp_path / "bin").exists(), "a refused archive must not be unpacked"


def test_a_binary_answering_another_version_is_the_version_refusal(
    mod, rig, tmp_path, monkeypatch
):
    shelf = _Shelf(_archive("delvec 1.3.0, dsl 0.22.0, mc 1.21.11"))
    monkeypatch.setattr(mod, "fetch", shelf)
    with pytest.raises(mod.Refusal) as caught:
        mod.run(rig, tmp_path / "engine", tmp_path / "bin", "Linux", "x86_64")
    assert caught.value.code == mod.EXIT_VERSION
    assert "1.3.0" in str(caught.value) and "v1.4.0" in str(caught.value)


def test_a_host_the_shelf_does_not_serve_takes_the_floor(mod, rig, tmp_path, monkeypatch):
    shelf = _Shelf(b"")
    monkeypatch.setattr(mod, "fetch", shelf)
    with pytest.raises(mod.Refusal) as caught:
        mod.run(rig, tmp_path / "engine", tmp_path / "bin", "FreeBSD", "riscv64")
    assert caught.value.code == mod.EXIT_NO_TARGET
    assert shelf.fetched == [], "no target means nothing is downloaded at all"


def test_a_failed_transfer_is_the_download_code_and_not_the_checksum_one(
    mod, rig, tmp_path, monkeypatch
):
    """The two codes the page may fall through on are not interchangeable."""

    def broken(url, *args, **kwargs):
        raise OSError("connection reset")

    monkeypatch.setattr(mod, "fetch", mod.fetch)  # keep the real wrapper
    monkeypatch.setattr("urllib.request.urlopen", broken)
    with pytest.raises(mod.Refusal) as caught:
        mod.run(rig, tmp_path / "engine", tmp_path / "bin", "Linux", "x86_64")
    assert caught.value.code == mod.EXIT_DOWNLOAD


def test_a_row_the_manifest_does_not_carry_is_a_refusal_not_the_floor(
    mod, rig, tmp_path, monkeypatch
):
    """A shelf whose manifest disagrees with it is nobody's toolchain."""

    def sums_without_the_row(url: str) -> bytes:
        if url.endswith("SHA256SUMS"):
            return b"0" * 64 + b"  delvec-v1.4.0-x86_64-apple-darwin.tar.gz\n"
        return b"x"

    monkeypatch.setattr(mod, "fetch", sums_without_the_row)
    with pytest.raises(mod.Refusal) as caught:
        mod.run(rig, tmp_path / "engine", tmp_path / "bin", "Linux", "x86_64")
    assert caught.value.code == mod.EXIT_CHECKSUM


def test_the_script_is_stdlib_only():
    """No third-party import, because Init runs it before anything is installed."""
    source = SCRIPT.read_text(encoding="utf-8")
    imported = {
        line.split()[1].split(".")[0]
        for line in source.splitlines()
        if line.startswith(("import ", "from ")) and "__future__" not in line
    }
    assert imported <= set(sys.stdlib_module_names), sorted(
        imported - set(sys.stdlib_module_names)
    )


def test_the_exit_codes_the_page_binds_to_are_the_ones_the_cli_returns(mod, tmp_path):
    """The CLI arm, so the numbers Init's table names are the process's own."""
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--pin", str(tmp_path / "absent.toml")],
        capture_output=True,
        text=True,
    )
    assert proc.returncode == mod.EXIT_UNUSABLE
    assert "fetch-delvec:" in proc.stderr
