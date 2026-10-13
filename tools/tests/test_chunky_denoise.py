"""The denoise kit and step (`tools/lib/chunky_denoise.py`): a platform with no
pinned Open Image Denoise archive is refused by name, a download whose sha256 is
not the pin installs nothing, the passes JVM is ended once its job is done and
the wait is bounded, and the write-back is the core's dump with the raw header.
"""

from __future__ import annotations

import hashlib
import importlib.util
import struct
import subprocess
import sys
import tarfile
from pathlib import Path

import pytest

LIB = Path(__file__).resolve().parents[1] / "lib" / "chunky_denoise.py"


def load():
    spec = importlib.util.spec_from_file_location("chunky_denoise_t", LIB)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


def kit_pins(tmp_path: Path, triple: str, *, wrong: bool = False) -> dict:
    d = tmp_path / "src"
    (d / "o" / "bin").mkdir(parents=True)
    jar = d / "plugin.jar"
    jar.write_bytes(b"plugin")
    (d / "o" / "bin" / "oidnDenoise").write_text("#!/bin/sh\n")
    arc = d / "o.tar.gz"
    with tarfile.open(arc, "w:gz") as t:
        t.add(d / "o", arcname="o")
    key = triple.replace("-", "_")
    sha = hashlib.sha256(arc.read_bytes()).hexdigest()
    return {
        "denoiser_plugin_url": jar.as_uri(),
        "denoiser_plugin_sha256": hashlib.sha256(jar.read_bytes()).hexdigest(),
        "oidn_version": "0.0.0",
        "oidn_url_base": d.as_uri() + "/",
        f"oidn_archive_{key}": arc.name,
        f"oidn_sha256_{key}": ("0" * 64) if wrong else sha,
    }


def home(tmp_path: Path) -> Path:
    h = tmp_path / "home"
    for sub in ("lib", "versions", "resources"):
        (h / sub).mkdir(parents=True)
    return h


def test_a_platform_without_an_archive_is_refused_by_name(tmp_path, monkeypatch):
    m = load()
    pins = kit_pins(tmp_path, "aarch64-apple-darwin")
    monkeypatch.setattr(m, "render_pins", lambda: pins)
    monkeypatch.setattr(m, "host_triple", lambda: "aarch64-unknown-linux-gnu")
    with pytest.raises(m.KitError) as e:
        m.install(home(tmp_path), tmp_path / "work")
    assert e.value.code == 4
    assert "ships no archive for aarch64-unknown-linux-gnu" in str(e.value)
    assert "aarch64-apple-darwin" in str(e.value)


def test_the_real_pins_name_every_shipped_platform_and_refuse_aarch64_linux(monkeypatch):
    m = load()
    pins = m.render_pins()
    for t in ("aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"):
        name, sha = m.oidn_archive(pins, t)
        assert name.startswith("oidn-2.5.1.") and len(sha) == 64
    with pytest.raises(m.KitError) as e:
        m.oidn_archive(pins, "aarch64-unknown-linux-gnu")
    assert e.value.code == 4


def test_a_download_that_is_not_the_pin_installs_nothing(tmp_path, monkeypatch):
    m = load()
    pins = kit_pins(tmp_path, "aarch64-apple-darwin", wrong=True)
    monkeypatch.setattr(m, "render_pins", lambda: pins)
    monkeypatch.setattr(m, "host_triple", lambda: "aarch64-apple-darwin")
    h = home(tmp_path)
    with pytest.raises(m.KitError) as e:
        m.install(h, tmp_path / "work")
    assert "nothing was installed" in str(e.value)
    assert not (h / "denoise" / "oidn").exists() and not (h / "denoise" / "home").exists()
    assert not (tmp_path / "work").exists(), "the staging directory was reclaimed"


def test_an_install_verifies_and_a_tampered_archive_is_refused(tmp_path, monkeypatch):
    m = load()
    pins = kit_pins(tmp_path, "aarch64-apple-darwin")
    monkeypatch.setattr(m, "render_pins", lambda: pins)
    monkeypatch.setattr(m, "host_triple", lambda: "aarch64-apple-darwin")
    h = home(tmp_path)
    k = m.install(h, tmp_path / "work")
    assert Path(k["oidn"]).name == "oidnDenoise"
    (h / "denoise" / "archives" / "o.tar.gz").write_bytes(b"changed")
    with pytest.raises(m.KitError) as e:
        m.verify(h)
    assert "oidn_sha256" in str(e.value)


def test_the_passes_jvm_is_ended_once_its_job_is_done(tmp_path):
    m = load()
    d = tmp_path / "aux"
    d.mkdir()
    log = tmp_path / "aux.log"
    code = (
        "import time,sys\n"
        f"open({str(d / 'x.albedo.pfm')!r},'wb').write(b'a')\n"
        f"open({str(d / 'x.normal.pfm')!r},'wb').write(b'n')\n"
        "print('Render job finished.', flush=True)\n"
        "time.sleep(600)\n"
    )
    with open(log, "w") as f:
        p = subprocess.Popen([sys.executable, "-c", code], stdout=f)
    m.await_maps(d, "x", log, p.pid, timeout=30)
    assert p.wait(timeout=15) != 0, "the hung JVM was ended"


def test_the_wait_is_bounded_and_says_what_is_missing(tmp_path):
    m = load()
    d = tmp_path / "aux"
    d.mkdir()
    log = tmp_path / "aux.log"
    log.write_text("")
    p = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(600)"])
    with pytest.raises(m.KitError) as e:
        m.await_maps(d, "x", log, p.pid, timeout=2)
    assert "did not finish within 2 s" in str(e.value) and "x.albedo.pfm" in str(e.value)
    assert p.wait(timeout=15) != 0


def test_the_writeback_is_the_core_dump_with_the_raw_header(tmp_path):
    m = load()
    s = tmp_path / "s"
    s.mkdir()
    (s / "x.dump").write_bytes(b"DUMP" + struct.pack(">iiiiq", 1, 2, 2, 520, 99) + b"compressed")
    pfm = tmp_path / "d.pfm"
    vals = [float(i) for i in range(12)]  # rows bottom to top: (0..5) is the bottom row
    pfm.write_bytes(b"PF\n2 2\n-1.0\n" + struct.pack("<12f", *vals))
    m.writeback(pfm, s, "x")
    out = (s / "x.dump").read_bytes()
    assert out[:28] == b"DUMP" + struct.pack(">iiiiq", 2, 2, 2, 520, 99)
    assert list(struct.unpack(">12d", out[28:])) == vals[6:] + vals[:6]
    assert (s / "x.dump.raw").read_bytes().endswith(b"compressed")
    assert m.restore(s, "x") and (s / "x.dump").read_bytes().endswith(b"compressed")
