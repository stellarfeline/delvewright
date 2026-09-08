#!/usr/bin/env python3
"""Score candidate RENDERS against a REFERENCE IMAGE, to order a contact sheet.

The score RANKS the contact sheet; it NEVER gates it (spec-0028 §3). Cross-domain calibration between a painterly reference and a voxel render
is unproven, so a number here may decide where a candidate sits on the page and
may never decide whether it is on the page. This tool therefore emits ONE SCORE
PER CANDIDATE and nothing else — no threshold, no `keep`, no `reject`. The
consumer (`delvec contact-sheet`) enforces the rest: whatever ordering it
computes must be a permutation of the candidate set, or it refuses with DW0725.
Promoting the score to a gate needs its own owner-approved amendment backed by
accumulated batch data. Do not add one here.

Two images, two stages, two producers. A **reference image** is concept art
drawn by an image model at the design-alignment gate (`tools/refimg.py`), before
any prefab exists. A **render** is a candidate prefab imaged by `delvec render`,
later, at contact-sheet curation. This tool measures the second against the
first; it draws neither.

## Backends

- `stub` — a deterministic, dependency-free, OFFLINE number derived from the
  file bytes. It is NOT a similarity measure and says so on every artifact it
  touches. Its whole job is to exercise the loop end to end (spec-0028 §5 AC2:
  "verified in a dry-run harness with a stub model, no live API in CI"), which
  is what CI runs.
- `open-clip` — cosine similarity between CLIP image embeddings of the
  reference and each render (`open_clip_torch`, MIT).
- `vqascore` — VQAScore (`t2v_metrics`, Apache-2.0), which is TEXT-conditioned:
  it asks a VLM how well each render answers a prompt. It scores against
  `--prompt`, not against the reference image, and refuses to run without one.

The real backends pull PyTorch and multi-GB model weights, so they do NOT sit in
CI and are not installed by anything in this repo. They live in a local opt-in
virtualenv the creator makes once (see `docs/reference/tools.md`). A missing
dependency is an ERROR that names the install line — this tool never silently
falls back to `stub`, because a stub number that looks like a measurement is
worse than no number at all.

## Config

`[refscore]` in the gitignored `delvewright.local.toml` (convention block in
`delvewright.toml`), same shape as `[refimg]` / `[i18n]`. Absent config with no
`--backend` exits 2 saying what to add; MALFORMED config is a hard error, so a
typo can never silently downgrade the metric. Today's backends run locally and
need no key; an inline `api_key` is refused outright regardless, and a future
remote backend names an environment variable (`api_key_env`) read at call time —
never stored, printed or logged.

Nothing here ships. Scores, sheets and reference images are generation-time
working material: local, gitignored, never committed to the content repo, never
relicensed (ADR-0013), and unable to move a delve's bytes (ADR-0006).

Stdlib only on the `stub` / `--dry-run` path (python >= 3.11 for `tomllib`).

Usage:
    # 1. an unranked sheet — it always writes the manifest naming every cell
    delvec contact-sheet .sheets/renders -o .sheets/zone2.png
    # 2. score those exact candidates against the zone's reference image
    tools/refscore.py --sheet .sheets/zone2.json --reference .refimg/zone2.png \\
        -o .sheets/zone2-scores.json
    # 3. the same page, now ordered by the score
    delvec contact-sheet .sheets/renders -o .sheets/zone2.png \\
        --scores .sheets/zone2-scores.json
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
import tomllib
from pathlib import Path

LOCAL_CONFIG_FILE = "delvewright.local.toml"
SECTION = "refscore"
SCHEMA = "delvewright.refscore/1"
SHEET_SCHEMA = "delvewright.contact-sheet/1"

STUB = "stub"

# Only backends whose license has been VERIFIED from upstream and recorded in
# docs/ACKNOWLEDGEMENTS.md belong here (CLAUDE.md attribution ledger).
BACKENDS = {
    STUB: {
        "needs_reference_image": True,
        "needs_prompt": False,
        "higher_is_better": True,
        "default_model": "sha256-pair",
        "install": None,
    },
    "open-clip": {
        # Image↔image: CLIP embeddings of the reference and of each render,
        # compared by cosine similarity in [-1, 1].
        "needs_reference_image": True,
        "needs_prompt": False,
        "higher_is_better": True,
        "default_model": "ViT-L-14",
        "default_pretrained": "laion2b_s32b_b82k",
        "install": "pip install open_clip_torch  # MIT",
    },
    "vqascore": {
        # Text↔image: "does this render answer the prompt?" There is no image
        # reference in the measurement at all, which is why --prompt is required
        # rather than optional.
        "needs_reference_image": False,
        "needs_prompt": True,
        "higher_is_better": True,
        "default_model": "clip-flant5-xl",
        "install": "pip install t2v-metrics  # Apache-2.0",
    },
}


class ConfigError(Exception):
    """Malformed configuration. Never recovered from — see the module docstring."""


class BackendUnavailable(Exception):
    """A real backend's dependency is not installed. Never falls back to stub."""


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def load_config(path: Path | None = None) -> dict:
    """Read `[refscore]`. Returns `{}` when the file or section is absent —
    absence is not an error here (a `--backend` flag alone is enough to run);
    MALFORMED is."""
    path = path or repo_root() / LOCAL_CONFIG_FILE
    if not path.exists():
        return {}
    with path.open("rb") as fh:
        data = tomllib.load(fh)
    cfg = data.get(SECTION)
    if not cfg:
        return {}
    if "api_key" in cfg:
        raise ConfigError(
            f"[{SECTION}].api_key is refused — a key must never live in a file. "
            f"Today's backends run locally and need no key at all; a future remote "
            f"one would name an environment variable via api_key_env."
        )
    backend = cfg.get("backend")
    if backend is not None and backend not in BACKENDS:
        raise ConfigError(
            f"[{SECTION}].backend = {backend!r} is not supported. "
            f"Known: {', '.join(sorted(BACKENDS))}."
        )
    return cfg


# ---------------------------------------------------------------------------
# Candidates
# ---------------------------------------------------------------------------


def candidates_from_sheet(path: Path) -> list[tuple[str, Path]]:
    """Read the candidate list out of a contact-sheet manifest.

    The manifest is the recommended input because it makes `delvec render` the
    SINGLE discoverer of candidates: the ids scored here are, by construction,
    the ids the sheet will bind against. A second directory walk written in
    Python would be a second source of truth for what a candidate is called, and
    the two would drift.
    """
    doc = json.loads(path.read_text())
    if doc.get("schema") != SHEET_SCHEMA:
        raise SystemExit(
            f"{path}: schema is {doc.get('schema')!r}, expected {SHEET_SCHEMA!r} — "
            f"pass the `.json` that `delvec contact-sheet` wrote beside its PNG."
        )
    base = path.parent
    out: list[tuple[str, Path]] = []
    for cell in doc.get("cells", []):
        image = Path(cell["image"])
        out.append((cell["id"], image if image.is_absolute() else base / image))
    if not out:
        raise SystemExit(f"{path}: no cells — nothing to score.")
    return out


def candidates_from_images(paths: list[Path]) -> list[tuple[str, Path]]:
    """Ad-hoc candidate list: one image per candidate, id = file stem.

    For a flat folder of picks. In `delvec render batch` output the stems carry
    the shot suffix (`temple-a-ext-se`), which will NOT bind to the sheet's
    candidate ids — use `--sheet` for that layout. A mismatch is not silent:
    the sheet reports its binding count and refuses a zero binding (DW0726).
    """
    return [(p.stem, p) for p in paths]


# ---------------------------------------------------------------------------
# Backends
# ---------------------------------------------------------------------------


def digest(path: Path) -> bytes:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.digest()


def score_stub(reference: Path, images: list[Path], _cfg: dict) -> list[float]:
    """A deterministic number in [0, 1) derived from the file bytes.

    NOT a similarity measure, and never presented as one: the emitted document
    carries `backend: "stub"`, and the contact sheet paints "STUB SCORES — NOT A
    SIMILARITY MEASURE" across its header. What it IS: an offline, keyless,
    dependency-free way to exercise the whole score→rank→page loop, including in
    CI, where no model may run.
    """
    ref = digest(reference)
    out = []
    for path in images:
        h = hashlib.sha256(ref + digest(path)).digest()
        out.append(struct.unpack(">Q", h[:8])[0] / 2.0**64)
    return out


def score_open_clip(reference: Path, images: list[Path], cfg: dict) -> list[float]:
    """Cosine similarity between CLIP image embeddings (open_clip_torch, MIT)."""
    try:
        import open_clip  # noqa: PLC0415 — lazy: multi-GB dependency, never in CI
        import torch  # noqa: PLC0415
        from PIL import Image  # noqa: PLC0415
    except ImportError as exc:  # pragma: no cover - needs the real dependency
        raise BackendUnavailable(str(exc)) from exc

    model_name = cfg.get("model") or BACKENDS["open-clip"]["default_model"]
    pretrained = cfg.get("pretrained") or BACKENDS["open-clip"]["default_pretrained"]
    device = cfg.get("device") or "cpu"
    model, _, preprocess = open_clip.create_model_and_transforms(
        model_name, pretrained=pretrained, device=device
    )
    model.eval()

    def embed(path: Path):
        tensor = preprocess(Image.open(path).convert("RGB")).unsqueeze(0).to(device)
        with torch.no_grad():
            v = model.encode_image(tensor)
        return v / v.norm(dim=-1, keepdim=True)

    ref = embed(reference)
    return [float((ref @ embed(p).T).item()) for p in images]


def score_vqascore(reference: Path | None, images: list[Path], cfg: dict) -> list[float]:
    """VQAScore (t2v_metrics, Apache-2.0) — text-conditioned, not image-to-image."""
    try:
        import t2v_metrics  # noqa: PLC0415 — lazy: multi-GB dependency, never in CI
    except ImportError as exc:  # pragma: no cover - needs the real dependency
        raise BackendUnavailable(str(exc)) from exc

    model_name = cfg.get("model") or BACKENDS["vqascore"]["default_model"]
    metric = t2v_metrics.VQAScore(model=model_name, device=cfg.get("device") or "cpu")
    prompt = cfg["prompt"]
    return [float(metric(images=[str(p)], texts=[prompt]).item()) for p in images]


SCORERS = {
    STUB: score_stub,
    "open-clip": score_open_clip,
    "vqascore": score_vqascore,
}


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def resolve_backend(args, cfg: dict) -> str | None:
    """`--backend` wins, then `[refscore].backend`. `None` = neither, which the
    caller reports as "say what to add" rather than guessing a metric."""
    return args.backend or cfg.get("backend")


def build_document(backend: str, cfg: dict, reference: Path | None,
                   candidates: list[tuple[str, Path]], values: list[float],
                   prompt: str | None) -> dict:
    spec = BACKENDS[backend]
    doc = {
        "schema": SCHEMA,
        "backend": backend,
        "model": cfg.get("model") or spec["default_model"],
        "reference": str(reference) if reference else None,
        "prompt": prompt,
        # Carried explicitly so a future distance metric cannot silently invert
        # the page. The consumer reads it; it is never inferred.
        "higher_is_better": spec["higher_is_better"],
        # The ruling, recorded on the artifact itself. An archived score file
        # states what it was and was not allowed to do.
        "rank_only_never_gates": True,
        "note": (
            "STUB — deterministic file-byte hash, NOT a similarity measure; "
            "loop exercise only."
            if backend == STUB
            else "Similarity score. RANKS the contact sheet; never gates it "
                 "(spec-0028 s3)."
        ),
        "scores": [
            {"id": cid, "image": str(path), "score": round(v, 6)}
            for (cid, path), v in zip(candidates, values, strict=True)
        ],
    }
    return doc


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--sheet", type=Path,
                     help="contact-sheet manifest (the .json delvec render wrote beside "
                          "its PNG) — the recommended input: same discoverer, same ids")
    src.add_argument("--images", type=Path, nargs="+",
                     help="explicit candidate images; id = file stem")
    ap.add_argument("--reference", type=Path,
                    help="the reference image the renders are measured against")
    ap.add_argument("--prompt", help="reference prompt (required by --backend vqascore, "
                                     "which is text-conditioned)")
    ap.add_argument("--backend", choices=sorted(BACKENDS),
                    help=f"overrides [{SECTION}].backend")
    ap.add_argument("--model", help="backend model name (overrides config)")
    ap.add_argument("--device", help="cpu | cuda | mps (overrides config)")
    ap.add_argument("-o", "--out", type=Path, default=Path(".sheets/scores.json"))
    ap.add_argument("--dry-run", action="store_true",
                    help="print what would be measured; load no model, write nothing")
    args = ap.parse_args(argv)

    try:
        cfg = dict(load_config())
    except ConfigError as exc:
        print(f"refscore: {exc}", file=sys.stderr)
        return 2

    backend = resolve_backend(args, cfg)
    if not backend:
        print(f"refscore: no backend: pass --backend ({'|'.join(sorted(BACKENDS))}) or "
              f"add a [{SECTION}] section to {LOCAL_CONFIG_FILE}.\n"
              f"See the commented convention block in delvewright.toml.", file=sys.stderr)
        return 2

    spec = BACKENDS[backend]
    for key in ("model", "device"):
        if getattr(args, key):
            cfg[key] = getattr(args, key)

    # Capability refusals, at the flag boundary — never a silently dropped
    # input. Same rule as refimg: a backend that cannot honour what was asked
    # says so instead of measuring something else.
    if spec["needs_reference_image"] and not args.reference:
        print(f"refscore: --reference is required by backend {backend!r} "
              f"(it compares each render against the reference image).", file=sys.stderr)
        return 1
    if spec["needs_prompt"] and not args.prompt:
        print(f"refscore: --prompt is required by backend {backend!r} — VQAScore is "
              f"TEXT-conditioned: it asks how well a render answers a prompt, and never "
              f"looks at the reference image. Scoring without one measures nothing.",
              file=sys.stderr)
        return 1
    if args.reference and not spec["needs_reference_image"]:
        print(f"refscore: --reference: backend {backend!r} is text-conditioned and would "
              f"IGNORE the reference image. Refusing rather than reporting a number the "
              f"reference had no part in; use --prompt.", file=sys.stderr)
        return 1
    if args.reference and not args.reference.exists():
        print(f"refscore: reference image not found: {args.reference}", file=sys.stderr)
        return 1
    cfg["prompt"] = args.prompt

    candidates = (
        candidates_from_sheet(args.sheet) if args.sheet
        else candidates_from_images(args.images)
    )
    missing = [str(p) for _, p in candidates if not p.exists()]
    if missing:
        print(f"refscore: {len(missing)} candidate image(s) not found: "
              f"{', '.join(missing[:6])}", file=sys.stderr)
        return 1

    if args.dry_run:
        print(f"backend    : {backend}"
              f"{'  (NOT a similarity measure)' if backend == STUB else ''}")
        print(f"model      : {cfg.get('model') or spec['default_model']}")
        print(f"reference  : {args.reference}")
        print(f"prompt     : {args.prompt}")
        print(f"candidates : {len(candidates)}")
        for cid, path in candidates:
            print(f"  {cid}  <-  {path}")
        print(f"out        : {args.out}")
        print("the score RANKS the contact sheet; it never gates it (spec-0028 s3)")
        return 0

    try:
        values = SCORERS[backend](args.reference, [p for _, p in candidates], cfg)
    except BackendUnavailable as exc:
        print(f"refscore: backend {backend!r} is not installed ({exc}).\n"
              f"  {spec['install']}\n"
              f"Install it in a local virtualenv — these dependencies are multi-GB and "
              f"deliberately absent from CI. Not falling back to the stub: a stub number "
              f"that looks like a measurement is worse than no number.", file=sys.stderr)
        return 4

    doc = build_document(backend, cfg, args.reference, candidates, values, args.prompt)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
    print(args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
