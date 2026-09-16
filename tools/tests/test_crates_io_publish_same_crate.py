r"""Guard: a publish run that finds its crate already served AS THE SAME CRATE ends green.

`tools/crates-io-publish.sh --publish --only delvewright-dsl` decides "same
crate, different bytes" when the registry's tarball differs from this tree's
only in provenance (`.cargo_vcs_info.json` names the commit it was packaged
at). That is the normal state of a re-run from a later `main` commit — which
`dsl-crate-publish.yml` now reaches whenever the `delvewright-dsl--v<version>`
Release is missing though the registry already serves the version (ADR-0028 §6).

MEASURED before the fix, against the real registry: a local run of that
workflow's publish step on a `main` commit later than the one the current
format version was uploaded from printed `skip … (already published, same crate)`, uploaded
nothing, then polled the post-condition — index sha256 == OUR sha256 — for 180
seconds and failed `only 0/1 visible`. The index keeps the tarball the earlier
commit uploaded, so that post-condition could never hold. The post-condition now
compares the index with the sha256 the plan decided: ours for an upload or a
byte-identical skip, the registry's for a same-crate skip.

OFFLINE: the scratch clone and fake `cargo` are `test_publish_gate_order.py`'s;
the sparse index and the download host are loopback servers reached through
`DW_CRATES_INDEX` and `DW_CRATES_STATIC`. The registry's tarball is built from
the one the fake `cargo package` wrote, plus a `.cargo_vcs_info.json`, so the two
differ exactly by provenance.
"""

from __future__ import annotations

import gzip
import hashlib
import http.server
import io
import json
import subprocess
import tarfile
import threading
from pathlib import Path

from test_publish_gate_order import REPO, _engine_crates, _install_fake_cargo, _scratch_clone


def _registry_crate(local: Path) -> bytes:
    """The local tarball's members plus a provenance file, deterministically packed."""
    raw = io.BytesIO()
    with tarfile.open(local, "r:gz") as src, tarfile.open(fileobj=raw, mode="w", format=tarfile.USTAR_FORMAT) as dst:
        top = None
        for member in src:
            top = top or member.name.split("/", 1)[0]
            data = src.extractfile(member).read() if member.isfile() else None
            info = tarfile.TarInfo(member.name)
            info.type = member.type
            info.mode = member.mode
            info.mtime = 0
            if data is not None:
                info.size = len(data)
                dst.addfile(info, io.BytesIO(data))
            else:
                dst.addfile(info)
        vcs = json.dumps({"git": {"sha1": "0" * 40}}).encode()
        info = tarfile.TarInfo(f"{top}/.cargo_vcs_info.json")
        info.size = len(vcs)
        info.mtime = 0
        dst.addfile(info, io.BytesIO(vcs))
    out = io.BytesIO()
    with gzip.GzipFile(fileobj=out, mode="wb", mtime=0) as gz:
        gz.write(raw.getvalue())
    return out.getvalue()


def _serve(tree: Path, name: str, version: str):
    local = tree / "package-verify" / "package" / f"{name}-{version}.crate"

    class Handler(http.server.BaseHTTPRequestHandler):
        def _send(self, body: bytes) -> None:
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self) -> None:  # noqa: N802
            if self.path == "/se/rd/serde":
                return self._send(json.dumps({"name": "serde", "vers": "1.0.0", "cksum": "0" * 64}).encode() + b"\n")
            if not local.exists():
                self.send_response(404)
                self.end_headers()
                return
            crate = _registry_crate(local)
            if self.path == f"/de/lv/{name}":
                row = {"name": name, "vers": version, "cksum": hashlib.sha256(crate).hexdigest()}
                return self._send(json.dumps(row).encode() + b"\n")
            if self.path == f"/static/{name}/{name}-{version}.crate":
                return self._send(crate)
            self.send_response(404)
            self.end_headers()

        def log_message(self, *_args) -> None:
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, thread, f"http://127.0.0.1:{server.server_port}"


def test_a_same_crate_skip_passes_its_post_condition(tmp_path: Path) -> None:
    crates = _engine_crates(REPO / "versions.toml")
    name, version = crates[0]
    assert name == "delvewright-dsl", crates
    tree = _scratch_clone(
        tmp_path,
        (REPO / "tools" / "check-publishable.sh").read_text(encoding="utf-8"),
        (REPO / "tools" / "crates-io-publish.sh").read_text(encoding="utf-8"),
    )
    env = _install_fake_cargo(tree, crates)
    server, thread, base = _serve(tree, name, version)
    try:
        run_env = {
            **env,
            "DW_CRATES_INDEX": base,
            "DW_CRATES_STATIC": f"{base}/static",
            "no_proxy": "*",
            "NO_PROXY": "*",
        }
        run_env.pop("CARGO_REGISTRY_TOKEN", None)
        result = subprocess.run(
            ["bash", str(tree / "tools" / "crates-io-publish.sh"), "--publish", "--only", name, "--allow-dirty"],
            cwd=tree, capture_output=True, text=True, env=run_env, timeout=120,
        )
    finally:
        server.shutdown()
        thread.join(timeout=5)
    out = result.stdout + result.stderr
    assert result.returncode == 0, out
    assert f"skip    {name} {version} (already published, same crate)" in out
    assert "TO_PUBLISH=\n" in result.stdout
    assert "1/1 visible with the expected sha256" in out
