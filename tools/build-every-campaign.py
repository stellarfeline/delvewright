#!/usr/bin/env python3
"""Build EVERY campaign in the pinned content repo. A break in one is a red here.

WHY THIS EXISTS

Nothing in CI ever built a real campaign. Every compiler gate ran against
fixtures under `crates/**/tests/fixtures` and `crates/dsl/fixtures/valid` —
small, hand-shaped trees that exercise one verb each. The shipped campaigns, the
only artifacts a player ever sees, were built by hand on somebody's laptop when
somebody remembered.

The cost of that gap was found three separate times in one day; the most
expensive instance is a change that reached 10/10 green while stopping the
flagship released campaign `nobodys-cave-island` from building at all — 26
`DW0364` errors on cells at the ocean line. Ten required status checks, all
green, and the product did not compile.

This closes it: on every push, every campaign the pinned content repo carries is
built, in every language it declares, and a campaign that stops building reds a
required status check.

WHAT IT IS NOT

It is not a sampler and it is not a skip list. Campaigns are DISCOVERED (any
directory under `<content>/campaigns/` holding a `world.json`), never enumerated
here, so a campaign added to content `main` is gated by the next content re-pin
without anyone remembering to add a line. A campaign that legitimately cannot
build today goes in `.github/campaign-build-exclusions.toml`, which inverts the
assertion rather than removing it: the campaign is still built, must still fail,
and must fail with exactly the codes recorded there — a different code, or a
success, is a finding. See that file's header for why.

BINDING (CLAUDE.md: a green gate that binds to nothing is VACUOUS, not a pass).
Every run states how many campaigns it discovered, built, and held known-red,
each by name. Discovering zero campaigns is a red; building zero campaigns is a
red. Both are the shapes in which this gate could go green having proven
nothing — a content checkout that silently landed empty, or an exclusion file
that grew to cover everything.

Deterministic and offline apart from the compiler itself; stdlib-only python3.
Exit 0 clean, 1 with one finding per line.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from delvec_bin import resolve as resolve_delvec  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
EXCLUSIONS = REPO / ".github" / "campaign-build-exclusions.toml"
VERSIONS = REPO / "versions.toml"


class Exclusion:
    def __init__(self, raw: dict, index: int) -> None:
        missing = [k for k in ("id", "task", "reason", "expect_codes") if k not in raw]
        if missing:
            raise SystemExit(
                f"build-every-campaign: FAIL — exclusion #{index} in {EXCLUSIONS.name} "
                f"is missing {', '.join(missing)}. An exclusion without a reason and an "
                f"expected failure is a skip, and a skip is a vacuous green."
            )
        self.id: str = raw["id"]
        self.task: str = raw["task"]
        self.reason: str = " ".join(raw["reason"].split())
        self.expect_codes: set[str] = set(raw["expect_codes"])
        if not self.expect_codes:
            raise SystemExit(
                f"build-every-campaign: FAIL — exclusion {self.id!r} names no "
                f"expect_codes, so any failure at all would satisfy it."
            )


def read_exclusions() -> dict[str, Exclusion]:
    if not EXCLUSIONS.is_file():
        return {}
    raw = tomllib.load(EXCLUSIONS.open("rb"))
    out: dict[str, Exclusion] = {}
    for i, entry in enumerate(raw.get("exclusion", [])):
        ex = Exclusion(entry, i)
        out[ex.id] = ex
    return out


def content_pin() -> str:
    try:
        c = tomllib.load(VERSIONS.open("rb"))["content"]
        return f"{c['repo']}@{c['sha']}"
    except Exception:
        return "unpinned"


def declared_languages(world: Path) -> list[str]:
    """`en` (the canonical build) plus every language `world.json` declares.

    A campaign ships its localized output too (`out-zh/`), so a translation
    sidecar that stops satisfying the compiler's coverage checks breaks a
    released artifact exactly the way an English break does.
    """
    doc = json.loads(world.read_text(encoding="utf-8"))
    langs = doc.get("languages") or doc.get("content", {}).get("languages") or []
    return ["en"] + [str(l) for l in langs]


def build(delvec: Path, campaign: Path, prefabs: Path, lang: str) -> tuple[int, list[dict]]:
    """Run one `delvec build`. Returns (exit code, parsed JSONL diagnostics)."""
    out = Path(tempfile.mkdtemp(prefix=f"dw-{campaign.name}-{lang}-"))
    try:
        proc = subprocess.run(
            [
                str(delvec), "build", str(campaign),
                "-o", str(out),
                "--prefabs", str(prefabs),
                "--lang", lang,
                "--json",
            ],
            capture_output=True,
            text=True,
        )
    finally:
        shutil.rmtree(out, ignore_errors=True)

    diags: list[dict] = []
    for line in (proc.stdout + "\n" + proc.stderr).splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            d = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(d, dict) and "code" in d and "severity" in d:
            diags.append(d)
    # A build that dies without emitting a single parsable diagnostic (a panic,
    # a missing prefab library, an OOM) must not read as "no errors": record the
    # raw tail so the finding says something a reader can act on.
    if proc.returncode != 0 and not any(d["severity"] == "error" for d in diags):
        diags.append(
            {
                "code": "«no diagnostic»",
                "severity": "error",
                "stage": "process",
                "path": "",
                "message": (
                    f"delvec exited {proc.returncode} without emitting an error "
                    f"diagnostic. Last output:\n"
                    + "\n".join((proc.stderr or proc.stdout).splitlines()[-15:])
                ),
            }
        )
    return proc.returncode, diags


def errors(diags: list[dict]) -> list[dict]:
    return [d for d in diags if d.get("severity") == "error"]


def show(diags: list[dict], limit: int = 40) -> None:
    for d in diags[:limit]:
        print(f"      {d['code']} {d.get('stage', '')} {d.get('path', '')}")
        head = (d.get("message") or "").splitlines()
        print(f"        {(head[0] if head else '(no message)')[:500]}")
    if len(diags) > limit:
        print(f"      … and {len(diags) - limit} more")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--content",
        type=Path,
        default=REPO / "campaigns",
        help="content-repo checkout (holds `campaigns/` and `prefabs/`). "
        "Default `campaigns`, the CI checkout path and the local dev symlink.",
    )
    ap.add_argument(
        "--delvec",
        type=Path,
        help="the `delvec` binary to build with. Required and never inferred: "
        "the whole point of this gate is WHICH engine built the campaign, so "
        "the caller names it. The refusal, and the staleness refusal beside it, "
        "belong to `tools/lib/delvec_bin.py` — one authority for every tool in "
        "this directory that runs an engine.",
    )
    args = ap.parse_args()

    delvec = resolve_delvec(
        args.delvec, repo=REPO, caller="build-every-campaign", required=True
    ).resolve()

    sources = args.content / "campaigns"
    prefabs = args.content / "prefabs"
    for p, what in ((sources, "campaign sources"), (prefabs, "prefab library")):
        if not p.is_dir():
            print(
                f"build-every-campaign: FAIL — {what} not found at {p}. The content "
                f"checkout did not land; every campaign below would be 'skipped'.",
                file=sys.stderr,
            )
            return 1

    exclusions = read_exclusions()
    discovered = sorted(d for d in sources.iterdir() if (d / "world.json").is_file())

    version = subprocess.run(
        [str(delvec), "--version"], capture_output=True, text=True
    ).stdout.strip()
    print(f"engine   : {version or delvec}")
    # The pin is what CI checks out; an explicit --content is a LOCAL override
    # (a control run against another branch's pin), and saying "content: <pin>"
    # over a tree that is not that pin would be the gate lying in its own output.
    overridden = args.content.resolve() != (REPO / "campaigns").resolve()
    print(f"content  : {content_pin()}   (versions.toml [content])")
    print(f"checkout : {args.content}" + ("   ← --content OVERRIDE, not the pin above" if overridden else ""))
    print(f"discovered {len(discovered)} campaign(s): "
          f"{', '.join(d.name for d in discovered) or '(none)'}")
    print()

    findings: list[str] = []

    # Vacuity guard 1: a content checkout that landed but carried no campaign.
    if not discovered:
        print(
            f"build-every-campaign: FAIL — discovered 0 campaigns under {sources}. "
            f"A gate over nothing is not a pass.",
            file=sys.stderr,
        )
        return 1

    # Vacuity guard 2: an exclusion for a campaign that is not there any more.
    # Left alone it would silently pre-excuse a campaign of that name added later.
    known = {d.name for d in discovered}
    for eid, ex in sorted(exclusions.items()):
        if eid not in known:
            findings.append(
                f"exclusion {eid!r} ({ex.task}) names no campaign in the pinned "
                f"content repo. Delete the entry — a stale exclusion pre-excuses "
                f"any future campaign that takes the name."
            )

    built: list[str] = []
    known_red: list[str] = []

    for campaign in discovered:
        name = campaign.name
        ex = exclusions.get(name)
        langs = declared_languages(campaign / "world.json")
        head = f"::group::{name}  ({'+'.join(langs)})"
        print(head + ("   [EXCLUDED — expected red]" if ex else ""))

        lang_ok = True
        for lang in langs:
            code, diags = build(delvec, campaign, prefabs, lang)
            errs = errors(diags)
            warns = [d for d in diags if d.get("severity") == "warning"]
            status = "ok" if code == 0 else f"exit {code}"
            print(f"    --lang {lang}: {status}, {len(errs)} error(s), {len(warns)} warning(s)")

            if ex is None:
                if code != 0:
                    lang_ok = False
                    print(f"    ::error::{name} (--lang {lang}) NO LONGER BUILDS")
                    show(errs)
                    seen = sorted({d["code"] for d in errs})
                    findings.append(
                        f"{name} (--lang {lang}) no longer builds on this engine: "
                        f"{len(errs)} error(s), codes {', '.join(seen)}. This is a "
                        f"released campaign; the engine change that caused it is the "
                        f"thing to fix, not this gate."
                    )
                continue

            # Excluded: the assertion is INVERTED, not removed.
            seen = {d["code"] for d in errs}
            if code == 0:
                lang_ok = False
                findings.append(
                    f"{name} (--lang {lang}) BUILDS now, but is still listed in "
                    f"{EXCLUSIONS.name} ({ex.task}). Delete the exclusion in the PR "
                    f"that fixed it — until then the next regression it suffers is "
                    f"excused by a stale line."
                )
            elif not seen <= ex.expect_codes:
                lang_ok = False
                unexpected = sorted(seen - ex.expect_codes)
                print(f"    ::error::{name} (--lang {lang}) failed for a NEW reason")
                show([d for d in errs if d["code"] in unexpected])
                findings.append(
                    f"{name} (--lang {lang}) is excluded for {sorted(ex.expect_codes)} "
                    f"({ex.task}) but also failed with {unexpected}. A new break was "
                    f"hiding behind the exclusion."
                )
            else:
                print(f"    known-red as recorded: {sorted(seen)} — {ex.task}")

        print("::endgroup::")
        if ex is None:
            if lang_ok:
                built.append(name)
        else:
            known_red.append(name)

    print()
    print("---- binding -------------------------------------------------------")
    print(f"discovered : {len(discovered)}  ({', '.join(sorted(known))})")
    print(f"built green: {len(built)}  ({', '.join(built) or 'NONE'})")
    print(f"known-red  : {len(known_red)}  ({', '.join(known_red) or 'none'})")
    for name in known_red:
        ex = exclusions[name]
        print(f"    - {name}: {ex.task} — {ex.reason}")
    print("--------------------------------------------------------------------")

    # Vacuity guard 3: everything excluded. The gate would be green having built
    # nothing at all, which is the state this whole job exists to make impossible.
    if not built and not findings:
        findings.append(
            f"0 of {len(discovered)} campaigns were built — every one is excluded. "
            f"This job would be green having compiled no product at all."
        )

    if findings:
        print(f"\nbuild-every-campaign: {len(findings)} finding(s)\n", file=sys.stderr)
        for f in findings:
            print(f"  {f}\n", file=sys.stderr)
        return 1

    print(f"\nbuild-every-campaign: OK — {len(built)} campaign(s) build on this engine.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
