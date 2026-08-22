#!/usr/bin/env python3
"""Generate a REFERENCE IMAGE for the design-alignment gate.

A reference image is concept art produced BEFORE any prefab exists: the creator
describes a scene, a model draws it, and the owner confirms the *design* against
a picture rather than against prose. It is not a render — a render is a candidate
prefab imaged by `delve-render`, which happens later, at contact-sheet curation.
Two stages, two producers; do not conflate them.

Output lands in a gitignored working directory (`.refimg/` by default), which is
where a DRAFT belongs. An APPROVED reference goes somewhere else: it is copied
into the campaign it belongs to — `design/reference/` beside its sidecar — and
committed with the campaign in the content repo, because an approval that lives
only in a gitignored directory is bound to nothing, and a later round authoring
against it goes blind. Either way nothing here can move a delve's bytes
(ADR-0006): a reference image is drawn for a human to judge a design by, and no
part of the toolchain places, reads or compiles one. This tool is
human-in-the-loop: it is never called from a build.

Config lives in the gitignored `delvewright.local.toml` under `[refimg]`; see the
commented convention block in `delvewright.toml` and `docs/reference/tools.md`.
The API key NEVER enters a file: `api_key_env` names an environment variable,
read at call time, never stored, printed, or logged.

The FRAME — the shape and size of the picture — is per CALL, not per
installation: a series of views of one subject wants a different frame per view,
because a straight-down site plan is not 16:9. `--aspect-ratio` and
`--image-size` (and `--resolution` on the providers that frame that way)
override config for one call, and the resolved value is recorded in the sidecar,
so a view is reproducible from what the repository holds rather than from a
config file nobody kept.

Absent config falls back to nothing (the tool says what to add and exits 2);
MALFORMED config is a hard error. A typo must never silently downgrade the
creator to a different provider or a weaker anchor.

Stdlib only (python >= 3.11 for `tomllib`).

Usage:
    tools/refimg.py --prompt "a sea-gate barbican at dusk, ..." --out .refimg/z1
    tools/refimg.py --prompt-file zone2.txt --style-code A1B2C3D4 --seed 42
    tools/refimg.py --prompt "..." --dry-run     # show the request, call nothing

    # a multi-view reference: view 1 from the prompt alone, every later view
    # anchored on VIEW 1 and framed for what it shows
    tools/refimg.py --prompt-file v1-front.txt --aspect-ratio 16:9 --out .refimg/v1
    tools/refimg.py --prompt-file v3-plan.txt  --aspect-ratio 1:1 \
        --chain-from "$(python3 -c 'import json;print(json.load(open(".refimg/v1.json"))["id"])')" \
        --out .refimg/v3
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
        # This wire frames a picture by naming the pixels outright; it has no
        # aspect-ratio or size vocabulary at all. Declared rather than inferred
        # from the wire shape, for the same reason `anchors` is: what a provider
        # CAN honour is a capability, and a flag it cannot honour is refused.
        "frame": ("resolution",),
    },
    "gemini-native": {
        # The Interactions API, NOT the OpenAI-compatibility layer: that layer has
        # no image input and SILENTLY IGNORES unknown parameters, so a
        # style-anchored request through it would drop the anchor without error.
        #
        # Schema below is COPIED FROM THE OFFICIAL SDK's generated types
        # (googleapis/python-genai, `google/genai/_gaos/types/interactions/`),
        # not inferred from prose docs or from a paid probe. Reading the SDK
        # corrected three things the docs had left wrong or unsaid: a `seed`
        # exists, `previous_interaction_id` exists, and `image_size` has no
        # "0.5K".
        "endpoint": "https://generativelanguage.googleapis.com/v1beta/interactions",
        "auth_header": "x-goog-api-key",
        "wire": "json",
        # `ImageContent` is {data|uri, mime_type, resolution, type} — there is NO
        # role field, confirmed in the SDK and not merely in the docs. A reference
        # image cannot declare itself a STYLE reference rather than a subject one;
        # the role is carried by the prompt text. The structural anchor here is
        # `previous_interaction_id` (--chain-from), which pins a whole conversation
        # rather than re-describing a look.
        "anchors": ("images", "chain"),
        "seed": True,
        # `response_format` carries both, and neither has a pixel spelling — a
        # `--resolution` handed to this provider would have nowhere to go.
        "frame": ("aspect_ratio", "image_size"),
    },
}

USER_AGENT = "delvewright-refimg/1"

RENDERING_SPEEDS = ("TURBO", "DEFAULT", "QUALITY")

# Copied from the SDK's generated literals (ImageResponseFormat*). Validated
# locally so a typo is an error here rather than a silently different picture.
IMAGE_SIZES = ("512", "1K", "2K", "4K")
ASPECT_RATIOS = ("1:1", "2:3", "3:2", "3:4", "4:3", "4:5", "5:4",
                 "9:16", "16:9", "21:9", "1:8", "8:1", "1:4", "4:1")

# The frame keys, their defaults, and the closed value sets they have one.
# `resolution` is a free WxH string: no provider here publishes its full list,
# and inventing one would refuse frames the service accepts — so it is passed
# through, while the two that DO have published literals are validated.
FRAME_KEYS = ("aspect_ratio", "image_size", "resolution")
FRAME_DEFAULTS = {"aspect_ratio": "16:9", "image_size": "2K"}
FRAME_CHOICES = {"aspect_ratio": ASPECT_RATIOS, "image_size": IMAGE_SIZES}


def frame_flag(key: str) -> str:
    return "--" + key.replace("_", "-")


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

    # Frame keys are validated against the CONFIGURED provider's own frame
    # vocabulary rather than against a provider named here, so a provider added
    # to the table above is validated by declaring `frame` and nothing else.
    for key in PROVIDERS[cfg["provider"]]["frame"]:
        choices = FRAME_CHOICES.get(key)
        if choices is None or key not in cfg:
            continue
        if cfg[key] not in choices:
            hint = " (There is no \"0.5K\" — the SDK's own literals are the authority.)" \
                if key == "image_size" else ""
            raise ConfigError(
                f"[{SECTION}].{key} = {cfg[key]!r}; allowed: {', '.join(choices)}.{hint}"
            )
    return cfg


def resolve_frame(cfg: dict, args) -> dict[str, str]:
    """The frame this call actually asks for: flag first, config second, default last.

    ONE authority, because the wire and the sidecar must agree. A frame the
    sidecar reports and the request did not carry is worse than no record at all
    — the series looks reproducible and is not — and that is exactly what
    reading config in one place and the flag in another produces.

    Capability refusal is the same rule the anchors follow: a frame flag the
    configured provider has no vocabulary for is an ERROR. A dropped frame does
    not fail, it returns a correctly-styled picture of the wrong shape, and
    nothing downstream can tell that from the picture that was asked for.
    """
    provider = PROVIDERS[cfg["provider"]]
    taken = provider["frame"]
    frame: dict[str, str] = {}
    for key in FRAME_KEYS:
        given = getattr(args, key, None)
        if key not in taken:
            if given is not None:
                raise SystemExit(
                    f"{frame_flag(key)}: provider {cfg['provider']!r} has no {key} — it "
                    f"frames a picture with {', '.join(frame_flag(k) for k in taken)}. "
                    f"Refused rather than dropped: a dropped frame comes back as a "
                    f"picture of the wrong shape with no error anywhere."
                )
            continue
        value = given if given is not None else cfg.get(key)
        if value is None:
            value = FRAME_DEFAULTS.get(key)
        if value is None:
            continue
        choices = FRAME_CHOICES.get(key)
        if choices is not None and value not in choices:
            # Only a FLAG reaches this: a config value was already refused as a
            # hard error by `load_config`, which exits 2 rather than 1.
            raise SystemExit(
                f"{frame_flag(key)} = {value!r}; allowed: {', '.join(choices)}."
            )
        frame[key] = value
    return frame


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


def build_request(cfg: dict, args, frame: dict[str, str]) -> tuple[dict, list[tuple[str, Path]]]:
    """Return the provider-shaped fields plus the reference images to attach.

    Capability refusals live here rather than at the wire: a flag the configured
    provider cannot honour is an ERROR, never a silently dropped parameter. The
    whole reason this tool exists is that a dropped anchor produces N unrelated
    pictures with no error anywhere. `frame` arrives already resolved and
    already refused against this provider (`resolve_frame`), so every key in it
    has a place on this wire by construction.
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
    if args.chain_from and "chain" not in provider["anchors"]:
        raise SystemExit(
            f"--chain-from: provider {cfg['provider']!r} has no interaction chaining."
        )
    if args.style_note and provider["wire"] != "json":
        raise SystemExit(
            f"--style-note: provider {cfg['provider']!r} has no system-instruction channel; "
            f"put the style contract in the prompt itself."
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
        if frame.get("resolution"):
            fields["resolution"] = frame["resolution"]
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
            **frame,
            # NO `delivery`. The SDK's generated types carry it ("inline"|"uri"),
            # but the endpoint answers `400 Image delivery mode is not supported`
            # for this model — the SDK types are a SUPERSET of what the service
            # accepts. Authoritative for shape, not for availability. Omitting it
            # yields inline bytes, which is what we want anyway.
        },
        # Storing is what makes this interaction addressable as a later
        # `previous_interaction_id` — without it the series anchor cannot exist.
        "store": True,
    }
    if args.seed is not None:
        body["generation_config"] = {"seed": args.seed}
    if args.chain_from:
        body["previous_interaction_id"] = args.chain_from
    if args.style_note:
        body["system_instruction"] = args.style_note
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


# No identifier is megabytes. A style code is 8 hex chars, a seed is an int, an
# interaction id is a short token — so a string this long is a PAYLOAD (image
# bytes, a thought signature), never an anchor a series could need. Redacting by
# size rather than by field name keeps the rule true for fields no provider has
# documented yet, which is the case this sidecar exists for.
REDACT_OVER_CHARS = 4096


def _redacted(node):
    """`node` with every oversized payload string replaced by its field size."""
    if isinstance(node, dict):
        return {k: _redacted(v) for k, v in node.items()}
    if isinstance(node, list):
        return [_redacted(v) for v in node]
    if isinstance(node, str) and len(node) > REDACT_OVER_CHARS:
        return f"<{len(node)} chars elided — payload, not an anchor>"
    return node


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


def save(result: dict, out: Path, request: dict | None = None) -> list[Path]:
    out.parent.mkdir(parents=True, exist_ok=True)
    # The FULL response is kept beside the images on purpose. It is the only place
    # an anchor the series depends on can be read back from, and no provider here
    # promises one comes back — Ideogram's generate response was MEASURED not to
    # return a style code. Record it and look, rather than assume.
    # Strip inline image payloads out of the SAVED response, never out of the one
    # we extract from: a 2K image is ~4M base64 chars, so keeping it would write an
    # 8 MB sidecar per generation — ~330 MB for one 8-zone round — duplicating bytes
    # already written as the image file. Everything else is kept verbatim, because
    # the sidecar's job is to preserve whatever the provider returned that a series
    # might need (an interaction id to chain from, a seed, a style code).
    # The REQUEST goes in beside the response, and this was a real gap: the
    # first eight-zone round shipped six images whose prompt and style note
    # existed only in the shell that launched them. A reference image is the
    # design-alignment gate's artifact — the owner looks at it and says yes or
    # no — so an image nobody can re-issue with one word changed is a dead end,
    # and a series whose shared style note is unrecoverable cannot be EXTENDED
    # at all, which is exactly what a two-zone follow-up needs to do.
    #
    # Anchors are recorded by provenance, never by payload: a reference image
    # is named by path, the chained interaction by id. Same reason the response
    # drops inline image bytes.
    meta = out.with_suffix(".json")
    # Merged, not nested: the response's own fields stay at the top level so
    # the six sidecars written before this change keep the same shape, and
    # reading an interaction id to chain from stays `d["id"]` everywhere.
    doc: dict = dict(_redacted(result))
    if request is not None:
        # NOT redacted, and the asymmetry is the point. `_redacted` is a rule
        # about the RESPONSE, where an oversized string is image bytes or a
        # thought signature — "no identifier is megabytes". The request record
        # holds only things WE wrote: the prompt, the style note, the frame, and
        # anchors named by provenance (a path, an id). A reference prompt runs to
        # thousands of words, so applying the response's size rule here elided
        # the one field the record exists for and left a series unreproducible
        # while looking complete.
        doc["request"] = request
    meta.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
    written = [meta]

    urls = [item.get("url") for item in result.get("data", []) if isinstance(item, dict) and item.get("url")]

    # Prefer the MODEL'S OWN OUTPUT steps. A chained interaction
    # (`previous_interaction_id`) can carry earlier images in the same document,
    # and a generic walk would save the conversation's input back out as if the
    # model had just drawn it. Fall back to the walk when no such step exists —
    # the response shape is undocumented and the call is already paid for.
    steps = result.get("steps")
    inline: list[tuple[str, str]] = []
    if not urls and isinstance(steps, list):
        for step in steps:
            if isinstance(step, dict) and step.get("type") == "model_output":
                inline.extend(_walk_images(step.get("content")))
    if not urls and not inline:
        inline = _walk_images(result)
    total = len(urls) + len(inline)

    def dest_for(i: int, mime: str = "image/png") -> Path:
        # The extension follows the bytes. Gemini returns JPEG for this model; a
        # file named .png that is actually a JPEG is the kind of small lie that
        # costs an hour when some later tool trusts the name.
        ext = {"image/jpeg": ".jpg", "image/jpg": ".jpg", "image/webp": ".webp"}.get(mime, ".png")
        return out.with_name(f"{out.name}-{i}{ext}") if total > 1 else out.with_suffix(ext)

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

    for i, (mime, b64) in enumerate(inline):
        try:
            dest_for(i, mime).write_bytes(base64.b64decode(b64))
        except (ValueError, TypeError) as exc:
            print(f"warning: inline image {i} did not decode ({exc}); response kept at {meta}",
                  file=sys.stderr)
            continue
        written.append(dest_for(i, mime))

    if total == 0:
        print(f"warning: no image found in the response; it is kept at {meta} — "
              f"inspect it and widen the extractor rather than re-paying.", file=sys.stderr)
    return written


def main(argv: list[str] | None = None) -> int:
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
    ap.add_argument("--chain-from", metavar="INTERACTION_ID",
                    help="continue a previous interaction (its `id`, recorded in that "
                         "call's sidecar) — the series anchor: zones 2..N chain off zone 1")
    ap.add_argument("--style-note", metavar="TEXT",
                    help="system instruction carrying the style contract, held constant "
                         "across a series while the prompt varies per zone")
    ap.add_argument("--count", type=int, default=1)
    ap.add_argument("--rendering-speed", choices=RENDERING_SPEEDS)
    # The three frame flags. No argparse `choices=`: the capability refusal is
    # the more useful message and must come FIRST — telling someone their ratio
    # is misspelt when their provider has no ratio at all sends them to fix the
    # wrong thing. Validation therefore lives in `resolve_frame`, once.
    ap.add_argument("--aspect-ratio", metavar="W:H",
                    help="frame this call, overriding config — the per-view frame of a "
                         f"multi-view series. One of: {', '.join(ASPECT_RATIOS)}")
    ap.add_argument("--image-size", metavar="SIZE",
                    help=f"frame size, overriding config. One of: {', '.join(IMAGE_SIZES)}")
    ap.add_argument("--resolution", metavar="WxH",
                    help="frame in pixels, overriding config — the frame vocabulary of "
                         "the providers that have no aspect ratio")
    ap.add_argument("--dry-run", action="store_true",
                    help="print the request that would be sent; call nothing, need no key")
    args = ap.parse_args(argv)

    if args.prompt_file:
        args.prompt = args.prompt_file.read_text().strip()

    try:
        cfg = load_config()
    except ConfigError as exc:
        print(f"refimg: {exc}", file=sys.stderr)
        return 2

    frame = resolve_frame(cfg, args)
    fields, files = build_request(cfg, args, frame)

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
    request = {
        "provider": cfg["provider"],
        "model": cfg.get("model"),
        "prompt": args.prompt,
        "style_note": getattr(args, "style_note", None),
        "seed": args.seed,
        # The RESOLVED frame, not the configured one: what was asked for is what
        # is recorded. All three keys are always present, `null` where this
        # provider has no such vocabulary, so a sidecar reader never has to
        # decide whether an absent key means "not asked for" or "not written".
        **{key: frame.get(key) for key in FRAME_KEYS},
        "chain_from": getattr(args, "chain_from", None),
        "reference_images": [str(rp) for _, rp in files],
    }
    for path in save(result, args.out, request):
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
