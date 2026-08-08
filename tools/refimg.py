#!/usr/bin/env python3
"""Generate a REFERENCE IMAGE for the design-alignment gate.

A reference image is concept art produced BEFORE any prefab exists: the creator
describes a scene, a model draws it, and the owner confirms the *design* against
a picture rather than against prose. It is not a render — a render is a candidate
prefab imaged by `delve-render`, which happens later, at contact-sheet curation.
Two stages, two producers; do not conflate them.

Generation-time working material only. Ref images are local, gitignored, never
shipped, and never enter the content repo — so image-model output licensing never
touches a shipped asset (ADR-0013), and nothing here can move a delve's bytes
(ADR-0006). This tool is human-in-the-loop: it is never called from a build.

Config lives in the gitignored `delvewright.local.toml` under `[refimg]`; see the
commented convention block in `delvewright.toml` and `docs/reference/tools.md`.
The API key NEVER enters a file: `api_key_env` names an environment variable,
read at call time, never stored, printed, or logged.

Absent config falls back to nothing (the tool says what to add and exits 2);
MALFORMED config is a hard error. A typo must never silently downgrade the
creator to a different provider or a weaker anchor.

Stdlib only (python >= 3.11 for `tomllib`).

Usage:
    tools/refimg.py --prompt "a sea-gate barbican at dusk, ..." --out .refimg/z1
    tools/refimg.py --prompt-file zone2.txt --style-code A1B2C3D4 --seed 42
    tools/refimg.py --prompt "..." --dry-run     # show the request, call nothing
"""

from __future__ import annotations

import argparse
import base64
import json
import mimetypes
import os
import secrets
import sys
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

LOCAL_CONFIG_FILE = "delvewright.local.toml"
SECTION = "refimg"

# Only providers whose wire shape AND capability set have been verified belong
# here. The discriminant carries CAPABILITY, not just the wire format: Google's
# OpenAI-compatible endpoint accepts an images call but has no image input and
# SILENTLY IGNORES unknown parameters, so routing a style-anchored request
# through it would discard the anchor without an error and produce N unrelated
# pictures. A provider that cannot take the anchor must fail loudly instead.
PROVIDERS = {
    "ideogram-v3": {
        "endpoint": "https://api.ideogram.ai/v1/ideogram-v3/generate",
        "auth_header": "Api-Key",
        "wire": "multipart",
        # A style CODE is an identifier reapplied exactly, so it cannot drift
        # across a series. Measured caveat: the generate response does NOT return
        # one (fields observed: is_image_safe, prompt, resolution, seed,
        # style_type, upscaled_resolution, url), so a code has to come from the
        # web UI. Reference images are the fallback anchor here.
        "anchors": ("code", "images"),
        "seed": True,
    },
    "gemini-native": {
        # The Interactions API, NOT the OpenAI-compatibility layer: that layer has
        # no image input and SILENTLY IGNORES unknown parameters, so a
        # style-anchored request through it would drop the anchor without error.
        "endpoint": "https://generativelanguage.googleapis.com/v1beta/interactions",
        "auth_header": "x-goog-api-key",
        "wire": "json",
        # Images only — and weaker than advertised: every reference image is
        # submitted as an untyped `{"type":"image"}`, with NO field distinguishing
        # a style reference from a character or object one. The role is carried by
        # the PROMPT TEXT, so the anchor is prose-mediated, not structural.
        "anchors": ("images",),
        "seed": False,
    },
}

USER_AGENT = "delvewright-refimg/1"

RENDERING_SPEEDS = ("TURBO", "DEFAULT", "QUALITY")


class ConfigError(Exception):
    """Malformed configuration. Never recovered from — see the module docstring."""


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def load_config() -> dict:
    path = repo_root() / LOCAL_CONFIG_FILE
    if not path.exists():
        raise SystemExit(
            f"no {LOCAL_CONFIG_FILE} — create it with a [{SECTION}] section.\n"
            f"See the commented convention block in delvewright.toml."
        )
    with path.open("rb") as fh:
        data = tomllib.load(fh)
    cfg = data.get(SECTION)
    if not cfg:
        raise SystemExit(
            f"{LOCAL_CONFIG_FILE} has no [{SECTION}] section.\n"
            f"See the commented convention block in delvewright.toml."
        )

    provider = cfg.get("provider")
    if provider not in PROVIDERS:
        raise ConfigError(
            f"[{SECTION}].provider = {provider!r} is not supported. "
            f"Known: {', '.join(sorted(PROVIDERS))}."
        )
    if "api_key" in cfg:
        raise ConfigError(
            f"[{SECTION}].api_key is refused — a key must never live in a file. "
            f"Use api_key_env to name an environment variable."
        )
    if not cfg.get("api_key_env"):
        raise ConfigError(f"[{SECTION}].api_key_env is required (the NAME of an env var).")

    speed = cfg.get("rendering_speed", "TURBO")
    if speed not in RENDERING_SPEEDS:
        raise ConfigError(
            f"[{SECTION}].rendering_speed = {speed!r}; allowed: {', '.join(RENDERING_SPEEDS)}."
        )
    cfg["rendering_speed"] = speed
    return cfg


def multipart(fields: dict[str, str], files: list[tuple[str, Path]]) -> tuple[bytes, str]:
    """Build a multipart/form-data body. Ideogram's v3 generate takes form data,
    not JSON, so this is not an optional nicety."""
    boundary = "----refimg" + secrets.token_hex(16)
    out = bytearray()
    for name, value in fields.items():
        out += f"--{boundary}\r\n".encode()
        out += f'Content-Disposition: form-data; name="{name}"\r\n\r\n'.encode()
        out += f"{value}\r\n".encode()
    for name, path in files:
        ctype = mimetypes.guess_type(path.name)[0] or "application/octet-stream"
        out += f"--{boundary}\r\n".encode()
        out += (
            f'Content-Disposition: form-data; name="{name}"; filename="{path.name}"\r\n'
            f"Content-Type: {ctype}\r\n\r\n"
        ).encode()
        out += path.read_bytes() + b"\r\n"
    out += f"--{boundary}--\r\n".encode()
    return bytes(out), f"multipart/form-data; boundary={boundary}"


def build_request(cfg: dict, args) -> tuple[dict, list[tuple[str, Path]]]:
    """Return the provider-shaped fields plus the reference images to attach.

    Capability refusals live here rather than at the wire: a flag the configured
    provider cannot honour is an ERROR, never a silently dropped parameter. The
    whole reason this tool exists is that a dropped anchor produces N unrelated
    pictures with no error anywhere.
    """
    provider = PROVIDERS[cfg["provider"]]

    if args.style_code and "code" not in provider["anchors"]:
        raise SystemExit(
            f"--style-code: provider {cfg['provider']!r} has no style-code anchor "
            f"(it anchors on reference images). Use --style-ref."
        )
    if args.style_code and args.style_ref:
        raise SystemExit(
            "--style-code and --style-ref are mutually exclusive (provider constraint). "
            "Pick ONE anchor and use it for every zone in the series."
        )
    if args.seed is not None and not provider["seed"]:
        raise SystemExit(
            f"--seed: provider {cfg['provider']!r} has no seed. Holding a seed is how "
            f"one changed word becomes one changed thing; without it every edit is a "
            f"full reroll. Drop the flag to accept that, or configure a provider with one."
        )

    files: list[tuple[str, Path]] = []
    for ref in args.style_ref:
        rp = Path(ref)
        if not rp.exists():
            raise SystemExit(f"style reference image not found: {rp}")
        files.append(("style_reference_images", rp))

    if provider["wire"] == "multipart":
        fields: dict[str, str] = {
            "prompt": args.prompt,
            "rendering_speed": args.rendering_speed or cfg["rendering_speed"],
            "num_images": str(args.count),
        }
        resolution = args.resolution or cfg.get("resolution")
        if resolution:
            fields["resolution"] = resolution
        if args.seed is not None:
            fields["seed"] = str(args.seed)
        if args.style_code:
            fields["style_codes"] = args.style_code
        return fields, files

    # JSON wire (gemini-native). Reference images are inline base64 in `input`.
    inputs: list[dict] = [{"type": "text", "text": args.prompt}]
    for _, rp in files:
        inputs.append({
            "type": "image",
            "mime_type": mimetypes.guess_type(rp.name)[0] or "image/png",
            "data": base64.b64encode(rp.read_bytes()).decode(),
        })
    body: dict = {
        "model": cfg.get("model") or "gemini-3.1-flash-image",
        "input": inputs,
        "response_format": {
            "type": "image",
            "aspect_ratio": cfg.get("aspect_ratio", "16:9"),
            "image_size": cfg.get("image_size", "2K"),
        },
    }
    return body, files


def call(cfg: dict, payload, files: list[tuple[str, Path]]) -> dict:
    provider = PROVIDERS[cfg["provider"]]
    key = os.environ.get(cfg["api_key_env"])
    if not key:
        raise SystemExit(
            f"environment variable {cfg['api_key_env']} is not set.\n"
            f"Export it in your shell; it is never read from a file."
        )
    if provider["wire"] == "multipart":
        body, content_type = multipart(payload, files)
    else:
        body, content_type = json.dumps(payload).encode(), "application/json"
    req = urllib.request.Request(
        cfg.get("endpoint") or provider["endpoint"],
        data=body,
        method="POST",
        headers={
            provider["auth_header"]: key,
            "Content-Type": content_type,
            "User-Agent": USER_AGENT,
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=cfg.get("timeout_seconds", 180)) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", "replace")[:2000]
        # The key is in the request headers, never in this message.
        raise SystemExit(f"provider returned HTTP {exc.code}:\n{detail}")


def _walk_images(node) -> list[tuple[str, str]]:
    """Every (mime, base64) image found anywhere in a response, in document order.

    Deliberately structural rather than schema-bound: the provider's response
    shape for the image bytes is NOT documented (the reference shows only an SDK
    convenience property), and the call has already been paid for by the time this
    runs. A walk that finds the bytes wherever they are cannot be broken by a
    field being renamed or nested one level deeper.
    """
    found: list[tuple[str, str]] = []
    if isinstance(node, dict):
        data = node.get("data")
        mime = node.get("mime_type") or node.get("mimeType") or ""
        if isinstance(data, str) and len(data) > 256 and (mime.startswith("image/") or not mime):
            found.append((mime or "image/png", data))
        else:
            for v in node.values():
                found.extend(_walk_images(v))
    elif isinstance(node, list):
        for v in node:
            found.extend(_walk_images(v))
    return found


def save(result: dict, out: Path) -> list[Path]:
    out.parent.mkdir(parents=True, exist_ok=True)
    # The FULL response is kept beside the images on purpose. It is the only place
    # an anchor the series depends on can be read back from, and no provider here
    # promises one comes back — Ideogram's generate response was MEASURED not to
    # return a style code. Record it and look, rather than assume.
    meta = out.with_suffix(".json")
    meta.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    written = [meta]

    urls = [item.get("url") for item in result.get("data", []) if isinstance(item, dict) and item.get("url")]
    inline = [] if urls else _walk_images(result)
    total = len(urls) + len(inline)

    def dest_for(i: int) -> Path:
        return out.with_name(f"{out.name}-{i}.png") if total > 1 else out.with_suffix(".png")

    for i, url in enumerate(urls):
        # The image host rejects urllib's default `Python-urllib/x.y` agent with a
        # 403 — the generate call has already been PAID FOR at this point, so a
        # download failure must never be an unhandled traceback that loses it. The
        # URL is ephemeral (a signed `exp=` ~24h out), so the saved response is not
        # something this can be replayed from indefinitely either.
        req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        try:
            with urllib.request.urlopen(req, timeout=120) as resp:
                dest_for(i).write_bytes(resp.read())
        except urllib.error.HTTPError as exc:
            print(
                f"warning: image {i} could not be downloaded (HTTP {exc.code}); "
                f"the paid response is kept at {meta} and its `url` stays valid "
                f"until the `exp` it carries.",
                file=sys.stderr,
            )
            continue
        written.append(dest_for(i))

    for i, (_mime, b64) in enumerate(inline):
        try:
            dest_for(i).write_bytes(base64.b64decode(b64))
        except (ValueError, TypeError) as exc:
            print(f"warning: inline image {i} did not decode ({exc}); response kept at {meta}",
                  file=sys.stderr)
            continue
        written.append(dest_for(i))

    if total == 0:
        print(f"warning: no image found in the response; it is kept at {meta} — "
              f"inspect it and widen the extractor rather than re-paying.", file=sys.stderr)
    return written


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--prompt")
    src.add_argument("--prompt-file", type=Path)
    ap.add_argument("--out", type=Path, default=Path(".refimg/ref"),
                    help="output path stem (default .refimg/ref)")
    ap.add_argument("--style-code", help="8-char hex style code — the series anchor")
    ap.add_argument("--style-ref", action="append", default=[],
                    help="style reference image (repeatable, max 3); excludes --style-code")
    ap.add_argument("--seed", type=int, help="hold it to change one word and see one change")
    ap.add_argument("--count", type=int, default=1)
    ap.add_argument("--rendering-speed", choices=RENDERING_SPEEDS)
    ap.add_argument("--resolution")
    ap.add_argument("--dry-run", action="store_true",
                    help="print the request that would be sent; call nothing, need no key")
    args = ap.parse_args()

    if args.prompt_file:
        args.prompt = args.prompt_file.read_text().strip()

    try:
        cfg = load_config()
    except ConfigError as exc:
        print(f"refimg: {exc}", file=sys.stderr)
        return 2

    fields, files = build_request(cfg, args)

    if args.dry_run:
        print(f"POST {cfg.get('endpoint') or PROVIDERS[cfg['provider']]['endpoint']}")
        print(f"auth: {PROVIDERS[cfg['provider']]['auth_header']}: <${cfg['api_key_env']}>")
        if PROVIDERS[cfg["provider"]]["wire"] == "multipart":
            print("multipart fields:")
            for k, v in fields.items():
                shown = v if k != "prompt" else v[:120] + ("…" if len(v) > 120 else "")
                print(f"  {k} = {shown}")
            for name, rp in files:
                print(f"  {name} = @{rp}")
        else:
            # Reference images are inline base64 — print their provenance, not
            # a megabyte of payload.
            redacted = json.loads(json.dumps(fields))
            for item in redacted.get("input", []):
                if item.get("type") == "image":
                    item["data"] = f"<{len(item['data'])} base64 chars>"
                elif item.get("type") == "text":
                    item["text"] = item["text"][:120] + ("…" if len(item["text"]) > 120 else "")
            print("json body:")
            print(json.dumps(redacted, indent=2, ensure_ascii=False))
            for name, rp in files:
                print(f"  (attached as inline image) {rp}")
        return 0

    result = call(cfg, fields, files)
    for path in save(result, args.out):
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
