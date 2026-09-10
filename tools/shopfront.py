#!/usr/bin/env python3
"""Build the shopfront: the page a stranger lands on and can drive a camera in.

## What this is for

Someone arrives at this repository, sees a building at the top of the README,
and clicks. What they get is not a screenshot and not a video: it is the engine's
own emitted blocks, in a browser, with a camera they hold. `delvec viewer`
already produces exactly that artifact — one self-contained `.html`, no server,
no CDN, no fetch — and this wraps it in the two things a public page needs and a
review page does not: a landing that says what it is, and a resource pack that
may legally be served.

    tools/shopfront.py --out <dir>

The result is a directory GitHub Pages can publish as-is: `index.html`,
`view.html`, and the hero image both of them use.

## The subject is named in exactly one place

`.github/shopfront.toml`, and nowhere else. No path to the subject appears in
this file, in the workflow, or in the README — they all resolve it from there, so
pointing the shopfront at a different building is an edit to that file and to
nothing else. Three forms, and the third is the one a released campaign uses:

* `grammar:<program>@<XxYxZ>` — expand a program of this repository's own rule
  library over a region. Needs nothing but this checkout.
* `gallery:<id>` — a piece of the gallery campaign, built by
  `prefabs/gallery-generator`. Also needs nothing but this checkout.
* `content:<path>` — a `.nbt`, a tile-set manifest, or a directory of them,
  under the pinned content checkout (`versions.toml` `[content]`, resolved
  locally through the `campaigns/` symlink and in CI through
  `.github/actions/checkout-content`).

## Why the page is a blockout and not the game

`delvec viewer` inlines the asset source it is given, and the source a creator
uses is their own client jar. That jar is EULA-gated and this project's standing
rule is that it is never committed, cached or published (`docs/ACKNOWLEDGEMENTS.md`;
ADR-0010). A page built from it embeds Mojang's texture bytes and serving it from
Pages distributes them, so it cannot be the public artifact. There is no third
state: `delvec viewer` with no texture source at all refuses (`DW0723`) and
writes nothing.

So the public page is built from `tools/blockout-pack.py` — boxes and flat
colours this repository authored, zero game assets — and it says so on itself, in
one line, above the frame. The jar-built page is still what a creator makes
locally for review; it is simply not what a stranger is served.

## Binding counts

Every run states the subject it resolved, the prefabs and anchors the page holds,
the blockstates it resolved and failed to resolve, and the bytes of every file it
wrote. A page holding zero prefabs, or a site whose page is smaller than the
renderer it must contain, is refused rather than published.
"""

from __future__ import annotations

import argparse
import html
import json
import os
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SUBJECT_FILE = REPO / ".github" / "shopfront.toml"
HERO = REPO / "docs" / "images" / "shopfront-hero.png"

# The vendored renderer is present in every page exactly once, whatever the page
# holds, so a page below its weight cannot be a working page (docs/reference/
# tools.md §4). A floor, not a target.
MIN_PAGE_BYTES = 200_000


def die(msg: str) -> int:
    print(f"shopfront: {msg}", file=sys.stderr)
    return 2


def run(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    printable = " ".join(str(c) for c in cmd)
    print(f"  $ {printable}")
    result = subprocess.run(cmd, cwd=cwd, text=True, capture_output=True)
    for line in (result.stdout + result.stderr).splitlines():
        print(f"    {line}")
    if result.returncode != 0:
        raise SystemExit(die(f"`{printable}` exited {result.returncode}"))
    return result


def materialise(subject: str, work: Path, delvec: Path, content: Path) -> Path:
    """Resolve the subject to a path `delvec viewer` accepts."""
    kind, _, rest = subject.partition(":")

    if kind == "grammar":
        program, _, region = rest.partition("@")
        if not program or not region:
            raise SystemExit(
                die(f"`{subject}` must read `grammar:<program>@<XxYxZ>` — the rule library "
                    f"has no default region and this tool will not invent one")
            )
        out = work / "grammar"
        out.mkdir(parents=True, exist_ok=True)
        run([str(delvec), "grammar", "expand", "--program", program,
             "--region", region, "--out", str(out / f"{program}.nbt")])
        # `expand` writes the piece, its metadata and its judgement report into a
        # directory named after `--out`. The viewer is handed the `.nbt` itself,
        # never that directory: the report is not prefab metadata and `viewer`
        # refuses it (`DW0721`) rather than skipping a file it cannot read.
        produced = out / f"{program}.nbt"
        piece = produced / f"{program}.nbt" if produced.is_dir() else produced
        if not piece.exists():
            raise SystemExit(die(f"`grammar expand` wrote no {piece}"))
        return piece

    if kind == "gallery":
        out = work / "gallery"
        out.mkdir(parents=True, exist_ok=True)
        run(["cargo", "run", "--release", "--manifest-path",
             str(REPO / "prefabs" / "gallery-generator" / "Cargo.toml"), "--", str(out)],
            cwd=REPO)
        piece = out / f"{rest}.nbt"
        if rest and not piece.exists():
            raise SystemExit(die(f"the gallery generator wrote no `{rest}.nbt` in {out}"))
        return piece if rest else out

    if kind == "content":
        target = content / rest
        if not target.exists():
            raise SystemExit(
                die(f"`{subject}` resolves to {target}, which does not exist. Locally that "
                    f"is the `campaigns/` symlink (spec-0007 Step 0); in CI it is the "
                    f"checkout `.github/actions/checkout-content` makes at the "
                    f"`versions.toml` `[content]` pin")
            )
        return target

    raise SystemExit(die(f"`{subject}` names no known subject kind — expected "
                         f"`grammar:`, `gallery:` or `content:`"))


INDEX_CSS = """
:root { color-scheme: dark; --ink:#e8e6e1; --dim:#a8a49c; --bg:#16171a; --edge:#2c2e33;
        --accent:#c9a227; }
* { box-sizing: border-box; }
body { margin:0; background:var(--bg); color:var(--ink);
       font:16px/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif; }
main { max-width: 62rem; margin: 0 auto; padding: 3rem 1.25rem 4rem; }
h1 { font-size: clamp(1.9rem, 5vw, 2.9rem); line-height:1.15; margin:0 0 .5rem; letter-spacing:-.02em; }
.lede { color:var(--dim); font-size:1.05rem; margin:0 0 2rem; max-width:44rem; }
figure { margin:0 0 1.25rem; border:1px solid var(--edge); border-radius:10px; overflow:hidden;
         background:#1d1f23; }
/* The hero is committed exactly as `delvec render piece` wrote it — a square
   frame — so the landing crops it here rather than keeping a hand-edited second
   copy of the same picture on disk. */
figure img { display:block; width:100%; aspect-ratio:16/7; object-fit:cover;
             object-position:center 58%; }
figcaption { padding:.7rem .95rem; font-size:.85rem; color:var(--dim); border-top:1px solid var(--edge); }
.cta { display:inline-block; margin:.5rem 0 1.75rem; padding:.8rem 1.4rem; border-radius:8px;
       background:var(--accent); color:#1a1400; font-weight:650; text-decoration:none; }
.cta:hover { filter:brightness(1.08); }
.note { border-left:3px solid var(--accent); padding:.1rem 0 .1rem 1rem; color:var(--dim);
        font-size:.92rem; margin:0 0 2rem; max-width:46rem; }
dl { display:grid; grid-template-columns:auto 1fr; gap:.35rem 1.25rem; font-size:.92rem;
     color:var(--dim); margin:0 0 2rem; }
dt { color:var(--ink); }
dd { margin:0; }
footer { border-top:1px solid var(--edge); padding-top:1.25rem; color:var(--dim); font-size:.85rem; }
a { color:var(--accent); }
@media (max-width: 520px) { dl { grid-template-columns:1fr; gap:0 0; } dd { margin:0 0 .6rem; } }
"""


def index_html(title: str, subject: str, facts: dict[str, str], hero: bool) -> str:
    rows = "\n".join(
        f"    <dt>{html.escape(k)}</dt><dd>{html.escape(v)}</dd>" for k, v in facts.items()
    )
    figure = (
        '  <figure>\n'
        '    <img src="hero.png" alt="An oblique view of the building this page shows." />\n'
        f'    <figcaption>{html.escape(title)}</figcaption>\n'
        '  </figure>\n'
        if hero
        else ""
    )
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{html.escape(title)} — Delvewright</title>
<style>{INDEX_CSS}</style>
</head>
<body>
<main>
  <h1>{html.escape(title)}</h1>
  <p class="lede">Delvewright builds self-contained Minecraft adventure maps from a written
  brief. This is one building it emitted, and you can walk around inside it.</p>
{figure}  <a class="cta" href="view.html">Walk it &rarr;</a>
  <p class="note">This is a view of the blocks the engine emitted, not a picture of
  Minecraft. Shapes are the pinned version's own; the colours are Delvewright's
  own legend, because the game's textures belong to Mojang and are not ours to
  serve. Nothing here is a screenshot of anything running.</p>
  <dl>
{rows}
  </dl>
  <footer>
    <a href="https://github.com/stellarfeline/delvewright">stellarfeline/delvewright</a>
    &middot; subject <code>{html.escape(subject)}</code>
  </footer>
</main>
</body>
</html>
"""


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", required=True, help="the site directory to write")
    ap.add_argument("--work", help="scratch directory (default <out>/../shopfront-work)")
    ap.add_argument("--delvec", default=str(REPO / "target" / "release" / "delvec"))
    ap.add_argument("--content", default=str(REPO / "campaigns"),
                    help="the pinned content checkout a `content:` subject resolves under")
    ap.add_argument("--subject", help="override the subject file, for a one-off build")
    args = ap.parse_args()

    if not SUBJECT_FILE.exists():
        return die(f"{SUBJECT_FILE.relative_to(REPO)} is missing — it is the one place the "
                   f"subject is named and nothing else names it")
    declared = tomllib.loads(SUBJECT_FILE.read_text())
    subject = args.subject or declared.get("subject")
    title = declared.get("title")
    if not subject or not title:
        return die(f"{SUBJECT_FILE.relative_to(REPO)} must declare both `subject` and `title`")

    delvec = Path(args.delvec)
    if not delvec.exists():
        return die(f"{delvec} is not built — `cargo build --release --bin delvec` first")

    out = Path(args.out)
    work = Path(args.work) if args.work else out.parent / "shopfront-work"
    for directory in (out, work):
        directory.mkdir(parents=True, exist_ok=True)

    print(f"shopfront: subject `{subject}` from {SUBJECT_FILE.relative_to(REPO)}")
    target = materialise(subject, work, delvec, Path(args.content))

    pack = work / "pack"
    if pack.exists():
        shutil.rmtree(pack)
    run([sys.executable, str(REPO / "tools" / "blockout-pack.py"), "--out", str(pack)])

    page = out / "view.html"
    result = run([str(delvec), "viewer", "--json", "--textures", str(pack),
                  str(target), "-o", str(page), "--title", title])

    summary: dict = {}
    for line in result.stdout.splitlines():
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        if "prefabs" in parsed:
            summary = parsed
    if not summary:
        return die("`delvec viewer` printed no summary object — nothing to bind a count to")
    if not summary.get("prefabs"):
        return die("the page holds zero prefabs; a shopfront showing nothing is not a page")
    page_bytes = page.stat().st_size
    if page_bytes < MIN_PAGE_BYTES:
        return die(f"the page is {page_bytes} B, below the {MIN_PAGE_BYTES} B the vendored "
                   f"renderer alone weighs — it cannot be a working page")

    hero_present = HERO.exists()
    if hero_present:
        shutil.copyfile(HERO, out / "hero.png")

    facts = {
        "Subject": subject,
        "Pieces on the page": str(summary["prefabs"]),
        "Named anchors": str(summary["anchors"]),
        "Distinct block states": str(summary["states"]),
        "States the page could not draw as the file states them": str(summary["unresolved"]),
        "Minecraft version the blocks are from": "1.21.11",
    }
    (out / "index.html").write_text(index_html(title, subject, facts, hero_present))

    print("shopfront: wrote", out)
    total = 0
    for path in sorted(out.rglob("*")):
        if path.is_file():
            size = path.stat().st_size
            total += size
            print(f"  {path.relative_to(out)}  {size:,} B")
    print(f"  total  {total:,} B ({total / 1024:.0f} KiB)")
    print(
        f"  binding  {summary['prefabs']} prefab(s), {summary['anchors']} anchor(s), "
        f"{summary['states']} blockstate(s), {summary['unresolved']} unresolved"
    )
    if not hero_present:
        print(f"  note  {HERO.relative_to(REPO)} is absent, so the landing carries no hero image")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
