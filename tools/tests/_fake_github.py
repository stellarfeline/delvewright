"""A loopback stand-in for the part of the GitHub API `tools/lib/github_releases.py` reads.

The gates that ask whether a retired version has its Release run as subprocesses
pointed at this server through `DW_GITHUB_API`, the shape the crates.io gate's
suite established with `DW_CRATES_INDEX`: whether a real Release exists changes
with this project's own release history, and a test must not depend on it.

The world is three maps — releases by tag, tag → commit, and (path, commit) →
bytes — and the bind-test Release (`v1.5.0`, published, one asset) is always
served unless `bind=False`, so a run without it is the unbound case.
"""

from __future__ import annotations

import base64
import http.server
import json
import threading
import urllib.parse
from dataclasses import dataclass, field

REPO = "stellarfeline/delvewright"


@dataclass
class World:
    releases: dict[str, dict] = field(default_factory=dict)  # tag -> {"draft": bool, "assets": {name: bytes}}
    tags: dict[str, str] = field(default_factory=dict)  # tag -> commit sha
    files: dict[tuple[str, str], bytes] = field(default_factory=dict)  # (path, commit) -> bytes
    bind: bool = True


def serve(world: World):
    """Start a server for `world`; returns `(url, stop)`."""

    class Handler(http.server.BaseHTTPRequestHandler):
        def _json(self, code: int, body) -> None:
            raw = json.dumps(body).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)

        def _release(self, tag: str, rel: dict) -> dict:
            base = f"http://{self.headers['Host']}"
            return {
                "tag_name": tag,
                "draft": rel.get("draft", False),
                "assets": [
                    {"name": n, "browser_download_url": f"{base}/dl/{urllib.parse.quote(tag)}/{urllib.parse.quote(n)}"}
                    for n in rel.get("assets", {})
                ],
            }

        def _all(self) -> dict[str, dict]:
            out = dict(world.releases)
            if world.bind:
                out.setdefault("v1.5.0", {"draft": False, "assets": {"SHA256SUMS": b"x"}})
            return out

        def do_GET(self) -> None:  # noqa: N802
            parsed = urllib.parse.urlparse(self.path)
            path = urllib.parse.unquote(parsed.path)
            query = urllib.parse.parse_qs(parsed.query)
            prefix = f"/repos/{REPO}/"
            releases = self._all()
            if path.startswith("/dl/"):
                _, _, tag, name = path.split("/", 3)
                body = releases.get(tag, {}).get("assets", {}).get(name)
                if body is None:
                    return self._json(404, {})
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            if not path.startswith(prefix):
                return self._json(404, {})
            rest = path[len(prefix):]
            if rest.startswith("releases/tags/"):
                tag = rest[len("releases/tags/"):]
                rel = releases.get(tag)
                if rel is None or rel.get("draft"):
                    return self._json(404, {})
                return self._json(200, self._release(tag, rel))
            if rest == "releases":
                page = int(query.get("page", ["1"])[0])
                rows = [self._release(t, r) for t, r in releases.items()]
                return self._json(200, rows if page == 1 else [])
            if rest.startswith("git/ref/tags/"):
                tag = rest[len("git/ref/tags/"):]
                if tag not in world.tags:
                    return self._json(404, {})
                return self._json(200, {"object": {"type": "commit", "sha": world.tags[tag]}})
            if rest.startswith("contents/"):
                file_path = rest[len("contents/"):]
                ref = query.get("ref", [""])[0]
                body = world.files.get((file_path, ref))
                if body is None:
                    return self._json(404, {})
                return self._json(200, {"encoding": "base64", "content": base64.b64encode(body).decode()})
            return self._json(404, {})

        def log_message(self, *_args) -> None:
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    def stop() -> None:
        server.shutdown()
        thread.join(timeout=5)

    return f"http://127.0.0.1:{server.server_port}", stop
