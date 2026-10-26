"""`scripts/references.py`, against a local HTTP server: an image, a page and a failure (spec-0123 §3.3)."""

from __future__ import annotations

import http.server
import json
import pathlib
import subprocess
import sys
import threading

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = REPO / ".claude" / "skills" / "delvewright" / "skills" / "new-delve" / "scripts" / "references.py"

PNG = b"\x89PNG\r\n\x1a\n" + b"\x00" * 24


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):  # noqa: N802 — the stdlib's name
        if self.path == "/smithy.png":
            self.send_response(200)
            self.send_header("Content-Type", "image/png")
            self.send_header("Content-Length", str(len(PNG)))
            self.end_headers()
            self.wfile.write(PNG)
        elif self.path == "/forge-page":
            body = b"<html><body>a page about forges</body></html>"
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_error(404)

    def log_message(self, *args):
        pass


@pytest.fixture
def server():
    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    t = threading.Thread(target=httpd.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{httpd.server_address[1]}"
    httpd.shutdown()


def campaign(root: pathlib.Path, base: str) -> pathlib.Path:
    c = root / "campaign"
    c.mkdir()
    (c / "world.json").write_text('{"campaign_id": "t", "stage": "world", "content": {}}\n', encoding="utf-8")
    rows = [
        {"url": f"{base}/smithy.png", "shows": "a smithy front", "terms": "CC0"},
        {"url": f"{base}/forge-page", "shows": "a page of forges", "terms": "none stated"},
        {"url": f"{base}/missing.png", "shows": "a picture that is gone", "terms": "none stated"},
    ]
    graph = {
        "campaign_id": "t",
        "stage": "layout-graph",
        "content": {"nodes": [{"id": "node/smithy", "intent": "smithy", "references": rows}, {"id": "node/lane", "intent": "lane"}]},
    }
    (c / "layout-graph.json").write_text(json.dumps(graph), encoding="utf-8")
    return c


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run([sys.executable, "-I", str(SCRIPT), *args], capture_output=True, text=True, check=False)


def test_an_image_is_fetched_a_page_recorded_and_a_failure_counted(tmp_path: pathlib.Path, server: str):
    c = campaign(tmp_path, server)
    out = run(str(c), "node/smithy")
    assert out.returncode == 0, out.stderr
    d = c / ".out" / "references" / "smithy"
    assert (d / "1.png").read_bytes() == PNG
    assert sorted(p.name for p in d.iterdir()) == ["1.png", "index.json"]
    index = json.loads((d / "index.json").read_text(encoding="utf-8"))
    assert index["place"] == "node/smithy"
    kinds = [(r["n"], r["kind"], r["file"]) for r in index["rows"]]
    assert kinds == [(1, "image", ".out/references/smithy/1.png"), (2, "page", None), (3, "failed", None)]
    assert index["rows"][2]["error"]
    assert out.stdout.startswith("references: 3 row(s); 1 image(s) fetched, 1 page(s), 1 failed")


def test_all_reads_every_place_that_carries_a_reference(tmp_path: pathlib.Path, server: str):
    c = campaign(tmp_path, server)
    out = run(str(c), "--all")
    assert out.returncode == 0, out.stderr
    assert "references: 3 row(s) over 1 of 2 place(s); 1 image(s) fetched, 1 page(s), 1 failed" in out.stdout
    assert not (c / ".out" / "references" / "lane").exists()


def test_a_place_the_graph_does_not_hold_is_refused(tmp_path: pathlib.Path, server: str):
    c = campaign(tmp_path, server)
    out = run(str(c), "node/nowhere")
    assert out.returncode == 1
    assert "`node/nowhere` is not a node of the graph" in out.stderr


def test_a_directory_that_is_not_a_campaign_is_refused(tmp_path: pathlib.Path):
    out = run(str(tmp_path), "--all")
    assert out.returncode == 2
    assert "not a campaign" in out.stderr
