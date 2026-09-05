#!/usr/bin/env python3
"""Generate a REFERENCE IMAGE for the design-alignment gate.

A reference image is concept art produced BEFORE any prefab exists: the creator
describes a scene, a model draws it, and the owner confirms the *design* against
a picture rather than against prose. It is not a render — a render is a candidate
prefab imaged by `delvec render`, which happens later, at contact-sheet curation.
Two stages, two producers; do not conflate them.

Output lands in a gitignored working directory (`.refimg/` by default), which is
where a DRAFT belongs. An APPROVED reference goes somewhere else: it is copied
into the campaign it belongs to — `design/concept/` for one scene,
`design/reference/` for a whole map, beside its sidecar in either case — and
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

HOW MANY PICTURES A CALL DRAWS IS NOT ALWAYS YOURS TO SAY, and a call that draws
more than one has billed for more than one. `ideogram-v3` takes a count on the
wire (`num_images`); `gemini-native`'s Interactions request has no image-count
field in the SDK's generated types, so the model decides — measured over 48
unpriced calls: 42 returned one image, one returned 2, two returned 5, and one
returned ELEVEN. So the tool never names a file after a number it did not
choose: the FIRST image is always `<out>.<ext>` — the name that was asked for —
and any extra is `<out>-1.<ext>`, `<out>-2.<ext>`, with one line saying
how many came back, what was written, and that the call may have billed for all
of them. A call that returns NOTHING says so and leaves non-zero; a read timeout
is retried once and then reported in one line, never as a traceback. The count
and the names go into the sidecar too, so a later reader can tell a one-image
call from an eleven-image one.

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
from typing import NamedTuple

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
        # `num_images` is on this wire, so --count is honoured here.
        "count": True,
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
        # NO count. The SDK's generated types for an Interactions request carry
        # no image-count field anywhere — not on the request, not on
        # `response_format`, not on `generation_config` — so there is nothing to
        # send, and inventing a parameter is how an anchor gets silently dropped
        # (this table's whole reason for existing). The model decides how many
        # images it draws, which is why `save` names the first one after what was
        # asked for and reports the rest rather than pretending one came back.
        "count": False,
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


class MissingConfig(Exception):
    """No config at all, or no `[refimg]` section in it.

    A separate class from `ConfigError` because the two are different findings —
    nothing was written vs something was written wrong — but they leave by the
    SAME exit. Both are "this installation cannot draw an image", both print what
    to add, and both return 2 — which is what this module's docstring, this tool's
    row in `docs/reference/tools.md` and the skill's Init step each state, and
    what `tools/refscore.py` — which states the same convention in the same
    words — actually does. A bare `SystemExit` carrying a message leaves with 1,
    so a creator following the Init step and checking the documented code saw a
    number no document names, and could not tell an unconfigured installation
    from a failed call.
    """


class ConfigError(Exception):
    """Malformed configuration. Never recovered from — see the module docstring."""


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def load_config() -> dict:
    path = repo_root() / LOCAL_CONFIG_FILE
    if not path.exists():
        raise MissingConfig(
            f"no {LOCAL_CONFIG_FILE} — create it with a [{SECTION}] section.\n"
            f"See the commented convention block in delvewright.toml."
        )
    with path.open("rb") as fh:
        data = tomllib.load(fh)
    cfg = data.get(SECTION)
    if not cfg:
        raise MissingConfig(
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
    if args.count is not None and not provider["count"]:
        raise SystemExit(
            f"--count: provider {cfg['provider']!r} has no image-count field on its "
            f"request — the model decides how many it draws, and a call has been "
            f"measured returning eleven. Refused rather than dropped: a count that "
            f"goes nowhere reads as a bound on what you will be billed for. Drop the "
            f"flag; the first image is still written to --out and any extra is "
            f"reported by name."
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
            # An unset --count asks this wire for ONE, explicitly. Leaving the
            # field off would let the service pick its own default, which is the
            # gemini-native failure mode ported to the provider that can avoid it.
            "num_images": str(args.count if args.count is not None else 1),
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


class CallFailed(Exception):
    """The request could not be completed, and the tool can NAME why.

    Raised instead of letting `urllib` out, because a twenty-line traceback
    ending `TimeoutError: The read operation timed out` names neither the prompt
    nor the output path — measured on 2 of 48 calls in one round, where the two
    that failed were indistinguishable from each other in the terminal. `main`
    catches this and prints one line carrying `--out`, which is the only string
    that tells a creator WHICH picture they have to draw again.
    """


# One bounded retry, and no more. A read timeout is the one failure here that is
# plausibly transient — the request was accepted and the drawing simply took
# longer than the deadline — so a single retry converts a dead round into a slow
# one. It is bounded because a retry is not free: a timed-out call may have been
# billed already, which the retry line says out loud rather than hiding.
CALL_ATTEMPTS = 2


def _timeout_reason(exc: BaseException) -> BaseException | None:
    """The timeout inside `exc`, if that is what it is.

    A read timeout surfaces as `TimeoutError`; a connect timeout arrives wrapped
    in `urllib.error.URLError`. Both are the same finding to a creator.
    """
    if isinstance(exc, TimeoutError):
        return exc
    reason = getattr(exc, "reason", None)
    return reason if isinstance(reason, TimeoutError) else None


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
    seconds = cfg.get("timeout_seconds", 180)
    for attempt in range(1, CALL_ATTEMPTS + 1):
        try:
            with urllib.request.urlopen(req, timeout=seconds) as resp:
                return json.loads(resp.read())
        except urllib.error.HTTPError as exc:
            detail = exc.read().decode("utf-8", "replace")[:2000]
            # The key is in the request headers, never in this message.
            raise SystemExit(f"provider returned HTTP {exc.code}:\n{detail}")
        except (TimeoutError, urllib.error.URLError) as exc:
            timeout = _timeout_reason(exc)
            if timeout is None:
                # Not transient and not retried, but still named: a DNS or TLS
                # failure is as much "a case the tool can name" as a timeout is.
                raise CallFailed(f"the provider could not be reached ({exc})") from exc
            if attempt == CALL_ATTEMPTS:
                raise CallFailed(
                    f"the provider did not answer within {seconds}s, twice "
                    f"({timeout}). Nothing was written. A timed-out call may still "
                    f"have been billed — check the provider's console before "
                    f"re-running, and raise [{SECTION}].timeout_seconds if this "
                    f"repeats."
                ) from exc
            print(
                f"refimg: no answer within {seconds}s; retrying once "
                f"({attempt + 1} of {CALL_ATTEMPTS}). The timed-out attempt may "
                f"still have been billed.",
                file=sys.stderr,
            )
    raise AssertionError("unreachable: the loop returns or raises")


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


class Saved(NamedTuple):
    """What `save` actually wrote, so the caller can SAY it.

    `returned` is how many images the provider sent, which is not a number this
    tool chose on every provider and is not always one. `images` is what landed
    on disk, in the order the response carried them; `paths` is everything
    written including the sidecar.
    """

    paths: list[Path]
    images: list[Path]
    returned: int


def save(result: dict, out: Path, request: dict | None = None) -> Saved:
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
    # Written NOW, before a single image is decoded, because the response has
    # already been paid for and an exception between here and the end of this
    # function must not lose it. It is written a second time at the end, once
    # `images` can say what actually landed.
    meta.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
    written = [meta]
    images: list[Path] = []

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

    # The same stem the sidecar is named from, so `<stem>.json` and `<stem>.jpg`
    # cannot disagree when the stem itself contains a dot.
    stem = out.with_suffix("")

    def dest_for(i: int, mime: str = "image/png") -> Path:
        # The extension follows the bytes. Gemini returns JPEG for this model; a
        # file named .png that is actually a JPEG is the kind of small lie that
        # costs an hour when some later tool trusts the name.
        #
        # THE FIRST IMAGE ALWAYS TAKES THE NAME THAT WAS ASKED FOR. It used to
        # take it only when exactly one came back, so a call the provider chose
        # to answer with five wrote `<stem>-0.jpg` … `<stem>-4.jpg` and NO
        # `<stem>.jpg` — and every existence check downstream, the page's own
        # included, reported the requested picture missing. How many images a
        # provider draws is not a fact about what the creator asked for.
        ext = {"image/jpeg": ".jpg", "image/jpg": ".jpg", "image/webp": ".webp"}.get(mime, ".png")
        if i == 0:
            return stem.with_name(stem.name + ext)
        return stem.with_name(f"{stem.name}-{i}{ext}")

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
        images.append(dest_for(i))

    for i, (mime, b64) in enumerate(inline):
        try:
            dest_for(i, mime).write_bytes(base64.b64decode(b64))
        except (ValueError, TypeError) as exc:
            print(f"warning: inline image {i} did not decode ({exc}); response kept at {meta}",
                  file=sys.stderr)
            continue
        written.append(dest_for(i, mime))
        images.append(dest_for(i, mime))

    # The count and the names go INTO the sidecar, not only onto the terminal.
    # A terminal line is gone by the next round; the sidecar travels with the
    # image into the campaign, and it is the only place a later reader can tell
    # a one-image call from an eleven-image one — which is a billing fact, and
    # the whole reason the extras are kept rather than deleted.
    # Names, not paths: the sidecar is copied into `design/` beside its image,
    # where the working directory it was drawn in no longer exists.
    doc["images"] = {"returned": total, "written": [p.name for p in images]}
    meta.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")

    return Saved(paths=written, images=images, returned=total)


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
    # No default of 1, and the difference is not cosmetic: the default has to be
    # distinguishable from "the creator asked for one", so a provider with no
    # count vocabulary can refuse the FLAG without refusing every ordinary call.
    ap.add_argument("--count", type=int, default=None,
                    help="how many images to ask for (ideogram-v3 only; unset asks "
                         "for 1). gemini-native has no count field on its request "
                         "and refuses this flag — that provider decides how many it "
                         "draws, a call has been measured returning eleven, and every "
                         "image it returns is billed")
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
    except (MissingConfig, ConfigError) as exc:
        print(f"refimg: {exc}", file=sys.stderr)
        return 2

    frame = resolve_frame(cfg, args)
    fields, files = build_request(cfg, args, frame)

    stem, meta = args.out.with_suffix(""), args.out.with_suffix(".json")

    if args.dry_run:
        print(f"POST {cfg.get('endpoint') or PROVIDERS[cfg['provider']]['endpoint']}")
        print(f"auth: {PROVIDERS[cfg['provider']]['auth_header']}: <${cfg['api_key_env']}>")
        # What this call may COST, said by the costless mode, because the costless
        # mode is where a creator looks before spending anything. Printed BEFORE
        # the wire dump so the dump stays the last thing on stdout and remains
        # parseable as a whole.
        if PROVIDERS[cfg["provider"]]["count"]:
            print(f"images: exactly {fields.get('num_images', 1)} — this provider takes "
                  f"a count.")
        else:
            print(f"images: at least 1, and this provider has no count field — a call "
                  f"has been measured returning ELEVEN, and every image returned is "
                  f"billed. The first lands at {stem}.<ext>, any extra at "
                  f"{stem}-1.<ext>, and the number goes into {meta}.")
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

    try:
        result = call(cfg, fields, files)
    except CallFailed as exc:
        # ONE line, and it names `--out`. The traceback this replaces named
        # neither the prompt nor the output path, so a round that lost two calls
        # out of forty-eight could not tell from the terminal which two.
        print(f"refimg: {args.out}: {exc}", file=sys.stderr)
        return 1
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
    saved = save(result, args.out, request)
    for path in saved.paths:
        print(path)

    if saved.returned == 0:
        # Non-zero, because the picture that was asked for does not exist. This
        # used to be a warning beside a zero exit, so a script could not tell a
        # drawn image from a response nobody could extract one out of.
        print(
            f"refimg: {args.out}: the provider returned no image. The paid response "
            f"is kept at {meta} — inspect it and widen the "
            f"extractor rather than re-paying.",
            file=sys.stderr,
        )
        return 1

    if saved.returned > 1:
        # Exit 0: the requested image EXISTS, at the requested name. The line is
        # about money, not about failure.
        names = ", ".join(p.name for p in saved.images)
        print(
            f"refimg: the provider returned {saved.returned} images for one call and "
            f"may have billed for all {saved.returned}. Written: {names}. The first is "
            f"the one you asked for; the extras are kept so the charge is visible, and "
            f"the count is in {meta}.",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
