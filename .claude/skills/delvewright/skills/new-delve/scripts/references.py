#!/usr/bin/env python3
"""Fetch the human-built references a place is built from, so its author can look at them (spec-0123 §3.3).

A place's design row in `layout-graph.json` carries `references[]`, each row
`{url, shows, terms}`. This tool fetches every row's URL once, before the
first handout, so the agent that authors the place reads a picture with its
file tool rather than describing one it never saw:

    "$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/references.py" <campaign-dir> <place>
    "$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/references.py" <campaign-dir> --all

For each place it writes into `<campaign>/.out/references/<place stem>/`:

- `<n>.<ext>` for a row whose response is an image (`Content-Type: image/…`),
  `n` the row's number from 1 in the order the node lists them;
- nothing for a row whose response is a page — the row is recorded as a page,
  and the author opens it by its URL;
- nothing for a row the fetch failed on — recorded as failed, with the error;
- `index.json`: `{"place", "rows": [{"n", "url", "kind", "file", "content_type",
  "error"}]}`, `kind` one of `image`, `page`, `failed`, `file` the image's
  path relative to the campaign (or null). `scripts/handout.py` reads it and
  names the fetched file in the place's section.

Nothing fetched is committed: `.out/` is build output, and a reference image is
not the campaign's to redistribute. A reference is looked at, never copied
(ADR-0013).

Prints, per place, `references: R row(s); F image(s) fetched, P page(s), E failed`
and, under `--all`, one more line over every place. Exits 1 when the place is
not a node of the graph, 2 when the directory is not a campaign. A row that
fails to fetch is recorded and counted, never an exit: the author still has its
URL and its sentence. Standard library only.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
import urllib.error
import urllib.request

REFERENCES_DIR = pathlib.Path(".out") / "references"
TIMEOUT_S = 20
MAX_BYTES = 32 * 1024 * 1024
USER_AGENT = "delvewright-references/1 (+https://github.com/stellarfeline/delvewright)"
EXTENSIONS = {
    "image/png": "png",
    "image/jpeg": "jpg",
    "image/jpg": "jpg",
    "image/gif": "gif",
    "image/webp": "webp",
    "image/bmp": "bmp",
    "image/svg+xml": "svg",
}


class Refusal(Exception):
    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code


def fetch(url: str) -> tuple[str, str, bytes | None, str | None]:
    """`(kind, content type, image bytes or None, error or None)` for one URL."""
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT_S) as resp:
            ctype = (resp.headers.get("Content-Type") or "").split(";")[0].strip().lower()
            if not ctype.startswith("image/"):
                return "page", ctype, None, None
            body = resp.read(MAX_BYTES + 1)
            if len(body) > MAX_BYTES:
                return "failed", ctype, None, f"the image is larger than {MAX_BYTES} bytes"
            return "image", ctype, body, None
    except (urllib.error.URLError, OSError, ValueError) as e:
        return "failed", "", None, str(e)


def extension(ctype: str, url: str) -> str:
    if ctype in EXTENSIONS:
        return EXTENSIONS[ctype]
    tail = url.rsplit("/", 1)[-1].rsplit(".", 1)
    return tail[1].lower() if len(tail) == 2 and tail[1].isalnum() else "img"


def one_place(campaign: pathlib.Path, node: dict) -> tuple[int, int, int, int]:
    place = node["id"]
    stem = place.removeprefix("node/")
    out = campaign / REFERENCES_DIR / stem
    out.mkdir(parents=True, exist_ok=True)
    for old in out.iterdir():
        if old.is_file():
            old.unlink()
    rows = []
    images = pages = failed = 0
    for n, ref in enumerate(node.get("references", []) or [], 1):
        url = ref.get("url", "")
        kind, ctype, body, error = fetch(url)
        rel = None
        if kind == "image" and body is not None:
            rel = REFERENCES_DIR / stem / f"{n}.{extension(ctype, url)}"
            (campaign / rel).write_bytes(body)
            images += 1
        elif kind == "page":
            pages += 1
        else:
            failed += 1
        rows.append(
            {
                "n": n,
                "url": url,
                "kind": kind,
                "file": str(rel) if rel else None,
                "content_type": ctype or None,
                "error": error,
            }
        )
    (out / "index.json").write_text(
        json.dumps({"place": place, "rows": rows}, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(
        f"references: {len(rows)} row(s); {images} image(s) fetched, {pages} page(s), {failed} failed "
        f"— `{place}`, index `{out / 'index.json'}`"
    )
    return len(rows), images, pages, failed


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("campaign_dir", type=pathlib.Path, help="the campaign directory")
    ap.add_argument("place", nargs="?", help="a layout-graph node id (`node/<kebab>`)")
    ap.add_argument("--all", action="store_true", help="every node that carries a reference")
    args = ap.parse_args(argv)
    try:
        return go(args)
    except Refusal as r:
        print(f"references: {r}", file=sys.stderr)
        return r.code


def go(args: argparse.Namespace) -> int:
    campaign = args.campaign_dir.expanduser().resolve()
    graph_path = campaign / "layout-graph.json"
    if not (campaign / "world.json").is_file() or not graph_path.is_file():
        raise Refusal(2, f"`{campaign}` carries no `world.json` and `layout-graph.json`; it is not a campaign with places")
    if bool(args.place) == bool(args.all):
        raise Refusal(2, "name one place (`node/<kebab>`) or pass `--all`, not both and not neither")
    try:
        nodes = json.loads(graph_path.read_text(encoding="utf-8"))["content"]["nodes"]
    except (OSError, ValueError, KeyError) as e:
        raise Refusal(2, f"`{graph_path}` is not a layout graph: {e}") from e
    if args.place:
        node = next((n for n in nodes if n.get("id") == args.place), None)
        if node is None:
            raise Refusal(1, f"`{args.place}` is not a node of the graph; its nodes: " + ", ".join(f"`{n.get('id')}`" for n in nodes))
        one_place(campaign, node)
        return 0
    totals = [0, 0, 0, 0]
    chosen = [n for n in nodes if n.get("references")]
    for node in chosen:
        for i, v in enumerate(one_place(campaign, node)):
            totals[i] += v
    print(
        f"references: {totals[0]} row(s) over {len(chosen)} of {len(nodes)} place(s); "
        f"{totals[1]} image(s) fetched, {totals[2]} page(s), {totals[3]} failed"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
