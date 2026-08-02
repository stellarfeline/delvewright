#!/usr/bin/env python3
"""Translate a campaign's l10n sidecar with an external OpenAI-compatible LLM API.

Generation-time tooling only. Shipped delves never call an LLM (CLAUDE.md
forbidden zones): this writes `l10n/<code>.json` into the campaign source, the
compiler bakes those strings at build time, and the running server talks to
nothing. See `docs/reference/i18n.md`.

Pipeline:

1. `delvec l10n-inventory <campaign-dir> --lang <code>` — the authoritative key
   inventory (exactly the key set `DW0180`/`DW0181` enforce), each row carrying its
   canonical English, the NPC whose dialogue tree it belongs to, and whatever the
   current sidecar already translates.
2. Batch the untranslated rows into persona-aware chat-completions requests
   (temperature low, JSON-object replies, a glossary of already-translated proper
   nouns for cross-batch consistency).
3. Merge, write the sidecar (keys sorted; exactly the inventory, so no orphans),
   then run `delvec validate` and report coverage.

Idempotent: a re-run translates only the keys the sidecar is missing (`--force`
retranslates everything). `--dry-run` prints the exact prompts and key lists and
makes no network call.

The API key is read from the environment variable *named* in the config
(`api_key_env`) at call time, and is never stored, echoed, or logged.

Stdlib only (python >= 3.11 for `tomllib`).
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
import shlex
import subprocess
import sys
import time
import tomllib
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Iterable, Sequence

REPO_ROOT = Path(__file__).resolve().parents[1]

CONFIG_FILE = "delvewright.toml"
LOCAL_CONFIG_FILE = "delvewright.local.toml"

#: The only provider protocol implemented: POST <base_url>/chat/completions with
#: an OpenAI-shaped body. DeepSeek, Moonshot/Kimi, OpenAI and most self-hosted
#: gateways all speak it.
PROVIDER = "openai-compatible"

DEFAULT_TEMPERATURE = 0.2
DEFAULT_BATCH_SIZE = 40
DEFAULT_TIMEOUT_S = 120
DEFAULT_MAX_RETRIES = 3

#: Keys whose translations are proper nouns worth pinning across batches: the
#: world title, NPC and area names, class names (never blurbs or prose).
GLOSSARY_PREFIXES = ("npc.", "area.")
GLOSSARY_LIMIT = 40


def is_glossary_key(key: str) -> bool:
    """Whether a key names a thing (rather than saying something about it)."""
    if key == "world.title" or key.startswith(GLOSSARY_PREFIXES):
        return True
    return key.startswith("class.") and key.endswith(".name")


class ConfigError(Exception):
    """`[i18n]` exists but is unusable — a mistake to surface, not to fall back on."""


class TranslateError(Exception):
    """A translation run failed (API error, unparseable reply, missing keys)."""


# --------------------------------------------------------------------- config --


@dataclass(frozen=True)
class I18nConfig:
    """Resolved `[i18n]` config. Holds the env var *name*, never the key itself."""

    base_url: str
    model: str
    api_key_env: str
    provider: str = PROVIDER
    temperature: float = DEFAULT_TEMPERATURE
    batch_size: int = DEFAULT_BATCH_SIZE
    timeout_seconds: int = DEFAULT_TIMEOUT_S
    max_retries: int = DEFAULT_MAX_RETRIES

    @property
    def endpoint(self) -> str:
        return self.base_url.rstrip("/") + "/chat/completions"

    def api_key(self, env: dict[str, str] | None = None) -> str | None:
        """The key from the named environment variable, or `None` when unset."""
        env = os.environ if env is None else env
        key = env.get(self.api_key_env, "").strip()
        return key or None


def _read_toml(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {}
    with path.open("rb") as fh:
        return tomllib.load(fh)


def load_config(
    root: Path = REPO_ROOT, explicit: Path | None = None
) -> I18nConfig | None:
    """Resolve `[i18n]` from `delvewright.toml`, overridden key-by-key by the
    gitignored `delvewright.local.toml`.

    Returns `None` when neither file declares an `[i18n]` section — the documented
    fallback (the `/new-delve` skill then translates in-agent). Raises
    [`ConfigError`] when a section *is* declared but malformed: a typo must not
    silently degrade into "not configured".
    """
    if explicit is not None:
        section = _read_toml(explicit).get("i18n")
        if not section:
            return None
        merged = dict(section)
    else:
        base = _read_toml(root / CONFIG_FILE).get("i18n") or {}
        local = _read_toml(root / LOCAL_CONFIG_FILE).get("i18n") or {}
        if not base and not local:
            return None
        merged = {**base, **local}

    provider = merged.get("provider", PROVIDER)
    if provider != PROVIDER:
        raise ConfigError(
            f"[i18n] provider = {provider!r} is not supported "
            f"(only {PROVIDER!r} — an OpenAI-shaped /chat/completions endpoint)"
        )
    missing = [k for k in ("base_url", "model", "api_key_env") if not merged.get(k)]
    if missing:
        raise ConfigError(
            f"[i18n] is missing required key(s): {', '.join(missing)} — see docs/reference/i18n.md"
        )
    if "api_key" in merged or "key" in merged:
        raise ConfigError(
            "[i18n] must never hold an API key — set `api_key_env` to the NAME of an "
            "environment variable holding it"
        )
    return I18nConfig(
        base_url=str(merged["base_url"]),
        model=str(merged["model"]),
        api_key_env=str(merged["api_key_env"]),
        provider=provider,
        temperature=float(merged.get("temperature", DEFAULT_TEMPERATURE)),
        batch_size=int(merged.get("batch_size", DEFAULT_BATCH_SIZE)),
        timeout_seconds=int(merged.get("timeout_seconds", DEFAULT_TIMEOUT_S)),
        max_retries=int(merged.get("max_retries", DEFAULT_MAX_RETRIES)),
    )


# ------------------------------------------------------------------ inventory --


@dataclass(frozen=True)
class Entry:
    """One inventory row from `delvec l10n-inventory`."""

    key: str
    en: str
    speaker: str | None = None
    existing: str | None = None


@dataclass(frozen=True)
class Inventory:
    """A parsed `delvec l10n-inventory` document."""

    campaign_id: str
    dsl_version: str
    lang: str
    declared: bool
    sidecar_present: bool
    world_title: str
    npcs: list[dict[str, Any]] = field(default_factory=list)
    entries: list[Entry] = field(default_factory=list)

    def pending(self, force: bool = False) -> list[Entry]:
        """Rows still needing a translation (all of them under `--force`)."""
        return [e for e in self.entries if force or e.existing is None]

    def glossary(self) -> dict[str, str]:
        """Already-translated proper nouns, to keep names stable across batches."""
        out: dict[str, str] = {}
        for e in self.entries:
            if e.existing and is_glossary_key(e.key):
                out[e.en] = e.existing
            if len(out) >= GLOSSARY_LIMIT:
                break
        return out


def parse_inventory(doc: dict[str, Any]) -> Inventory:
    """Parse the `delvec l10n-inventory` JSON document."""
    try:
        entries = [
            Entry(
                key=e["key"],
                en=e["en"],
                speaker=e.get("speaker"),
                existing=e.get("existing"),
            )
            for e in doc["entries"]
        ]
        return Inventory(
            campaign_id=doc["campaign_id"],
            dsl_version=doc["dsl_version"],
            lang=doc["lang"],
            declared=bool(doc["declared"]),
            sidecar_present=bool(doc["sidecar_present"]),
            world_title=doc.get("world_title", ""),
            npcs=list(doc.get("npcs", [])),
            entries=entries,
        )
    except (KeyError, TypeError) as exc:
        raise TranslateError(f"malformed l10n-inventory document: {exc}") from exc


def batches(entries: Sequence[Entry], size: int) -> list[list[Entry]]:
    """Split rows into request-sized batches, preserving inventory order (which
    groups an NPC's dialogue together, so a batch shares one voice)."""
    if size < 1:
        raise ValueError("batch size must be >= 1")
    return [list(entries[i : i + size]) for i in range(0, len(entries), size)]


# --------------------------------------------------------------------- prompt --

SYSTEM_PROMPT = """\
You are a professional video-game localizer working on a Minecraft adventure map.
You translate player-facing strings from English into {lang}.

Rules:
- Reply with ONE JSON object mapping every given key to its translated string.
  No prose, no explanation, no markdown fences, no extra or missing keys.
- Translate meaning and voice, not words. Keep the register of a hand-made
  fantasy adventure map; avoid machine-literal phrasing.
- Each item may name a `speaker`: that NPC's persona and speech style are given
  below — the translation must sound like that character.
- Keys ending in `.opt.<n>.label` are the PLAYER's reply inside that NPC's
  dialogue tree: keep them short, first-person, and selectable at a glance.
- Keys under `obj.` are objective titles/hints, `quest.` are goals, `class.` are
  class names/blurbs, `npc.`/`area.`/`world.` are proper nouns and headings.
- Reuse the glossary translations verbatim wherever those names appear.
- Preserve any placeholder, symbol, or formatting sequence exactly as given.
- Keep strings roughly as short as the English: they render in chat lines,
  item names, and title cards.
"""


def build_messages(inv: Inventory, batch: Sequence[Entry], lang: str) -> list[dict[str, str]]:
    """The chat messages for one batch: rules, campaign/persona context, keys."""
    speakers = {e.speaker for e in batch if e.speaker}
    personas = [n for n in inv.npcs if n.get("id") in speakers]
    glossary = inv.glossary()

    context: dict[str, Any] = {
        "campaign": inv.campaign_id,
        "world_title": inv.world_title,
        "target_language": lang,
    }
    if personas:
        context["speakers"] = personas
    if glossary:
        context["glossary_en_to_target"] = glossary

    items = [
        {k: v for k, v in (("key", e.key), ("en", e.en), ("speaker", e.speaker)) if v}
        for e in batch
    ]
    user = (
        "Context:\n"
        + json.dumps(context, ensure_ascii=False, indent=2, sort_keys=True)
        + "\n\nTranslate these strings into "
        + lang
        + " and reply with the JSON object of key -> translation:\n"
        + json.dumps(items, ensure_ascii=False, indent=2)
    )
    return [
        {"role": "system", "content": SYSTEM_PROMPT.format(lang=lang)},
        {"role": "user", "content": user},
    ]


def build_request(
    cfg: I18nConfig, messages: Sequence[dict[str, str]], api_key: str
) -> tuple[str, dict[str, Any], dict[str, str]]:
    """`(url, body, headers)` for one chat-completions call. The key only ever
    appears in the returned `Authorization` header — never in the body or a log."""
    body: dict[str, Any] = {
        "model": cfg.model,
        "messages": list(messages),
        "temperature": cfg.temperature,
        # Supported by OpenAI, DeepSeek and Moonshot/Kimi; harmless elsewhere, and
        # `parse_translations` still tolerates a fenced reply.
        "response_format": {"type": "json_object"},
        "stream": False,
    }
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {api_key}",
    }
    return cfg.endpoint, body, headers


def parse_translations(content: str) -> dict[str, str]:
    """Extract the key -> translation object from a model reply, tolerating a
    ```json fence or surrounding prose."""
    text = content.strip()
    if text.startswith("```"):
        text = text.split("```")[1]
        if text.lstrip().lower().startswith("json"):
            text = text.lstrip()[4:]
    text = text.strip()
    if not text.startswith("{"):
        start, end = text.find("{"), text.rfind("}")
        if start < 0 or end <= start:
            raise TranslateError(f"model reply contains no JSON object: {content[:200]!r}")
        text = text[start : end + 1]
    try:
        data = json.loads(text)
    except json.JSONDecodeError as exc:
        raise TranslateError(f"model reply is not valid JSON: {exc}") from exc
    if not isinstance(data, dict):
        raise TranslateError("model reply is not a JSON object")
    out: dict[str, str] = {}
    for k, v in data.items():
        if not isinstance(v, str):
            raise TranslateError(f"translation for `{k}` is not a string")
        out[str(k)] = v
    return out


# ------------------------------------------------------------------ transport --

Poster = Callable[[str, dict[str, Any], dict[str, str], int], dict[str, Any]]


def post_json(
    url: str, body: dict[str, Any], headers: dict[str, str], timeout: int
) -> dict[str, Any]:
    """POST a JSON body and return the parsed JSON response."""
    data = json.dumps(body, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    with urllib.request.urlopen(req, timeout=timeout) as resp:  # noqa: S310 (configured URL)
        return json.loads(resp.read().decode("utf-8"))


def chat_once(
    cfg: I18nConfig,
    messages: Sequence[dict[str, str]],
    api_key: str,
    poster: Poster | None = None,
    sleep: Callable[[float], None] = time.sleep,
) -> str:
    """One chat completion, retried on transport errors. Returns the reply text.

    Errors are re-raised with the endpoint and status only — never the request
    headers, so the key cannot reach a log through an exception.

    `poster` is resolved at call time (not bound as a default) so a test that
    replaces the module's `post_json` truly intercepts every request — a default
    argument would have captured the real one at import and gone to the network.
    """
    poster = poster or post_json
    url, body, headers = build_request(cfg, messages, api_key)
    last: Exception | None = None
    for attempt in range(cfg.max_retries):
        try:
            resp = poster(url, body, headers, cfg.timeout_seconds)
            return resp["choices"][0]["message"]["content"]
        except urllib.error.HTTPError as exc:
            last = TranslateError(f"{cfg.endpoint} returned HTTP {exc.code}")
            if exc.code in (400, 401, 403, 404, 422):
                raise last from None  # not transient: bad key/model/url
        except (urllib.error.URLError, TimeoutError, OSError) as exc:
            last = TranslateError(f"{cfg.endpoint} unreachable: {exc}")
        except (KeyError, IndexError, TypeError) as exc:
            last = TranslateError(f"unexpected response shape from {cfg.endpoint}: {exc}")
        if attempt + 1 < cfg.max_retries:
            sleep(2.0 * (attempt + 1))
    raise last or TranslateError("translation request failed")


# --------------------------------------------------------------------- sidecar --


def merge_content(inv: Inventory, translated: dict[str, str]) -> dict[str, str]:
    """The sidecar `content`: exactly the inventory keys, existing translations
    kept unless replaced. Orphans cannot survive — the map is built from the
    inventory, so `DW0181` is unreachable by construction."""
    out: dict[str, str] = {}
    for e in inv.entries:
        value = translated.get(e.key, e.existing)
        if value is not None:
            out[e.key] = value
    return out


def sidecar_path(campaign_dir: Path, lang: str) -> Path:
    return campaign_dir / "l10n" / f"{lang}.json"


def write_sidecar(path: Path, inv: Inventory, content: dict[str, str]) -> None:
    """Write `l10n/<code>.json`. An existing sidecar's `dsl_version` is preserved
    (it is a supported-version claim about the sidecar, not about this run)."""
    dsl_version = inv.dsl_version
    if path.is_file():
        try:
            dsl_version = json.loads(path.read_text("utf-8")).get("dsl_version", dsl_version)
        except (json.JSONDecodeError, OSError):
            pass
    doc = {
        "dsl_version": dsl_version,
        "campaign_id": inv.campaign_id,
        "kind": "l10n",
        "lang": inv.lang,
        "content": dict(sorted(content.items())),
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", "utf-8")


# ------------------------------------------------------------------------ cli --


def delvec_command(explicit: str | None) -> list[str]:
    """How to invoke `delvec`: `--delvec`, then `$DELVEC`, else a cargo run."""
    cmd = explicit or os.environ.get("DELVEC")
    if cmd:
        return shlex.split(cmd)
    return ["cargo", "run", "-q", "-p", "delvewright-compiler", "--bin", "delvec", "--"]


def run_delvec(args: Sequence[str], delvec: Sequence[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [*delvec, *args], cwd=REPO_ROOT, capture_output=True, text=True, check=False
    )


def fetch_inventory(campaign_dir: Path, lang: str, delvec: Sequence[str]) -> Inventory:
    proc = run_delvec(["l10n-inventory", str(campaign_dir), "--lang", lang], delvec)
    if proc.returncode != 0:
        raise TranslateError(
            f"`delvec l10n-inventory` failed (exit {proc.returncode}):\n{proc.stdout}{proc.stderr}"
        )
    return parse_inventory(json.loads(proc.stdout))


def _iter_batches(
    inv: Inventory, pending: Sequence[Entry], cfg: I18nConfig, lang: str
) -> Iterable[tuple[int, list[Entry], list[dict[str, str]]]]:
    chunks = batches(pending, cfg.batch_size)
    for i, chunk in enumerate(chunks, start=1):
        yield i, chunk, build_messages(inv, chunk, lang)


def main(argv: Sequence[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        prog="i18n-translate.py",
        description="Translate a campaign's l10n sidecar with an external OpenAI-compatible LLM API "
        "(generation-time only; shipped delves never call an LLM).",
    )
    p.add_argument("campaign_dir", type=Path, help="campaign directory (holds world.json)")
    p.add_argument("--lang", required=True, help="target language code, e.g. zh-cn")
    p.add_argument("--config", type=Path, default=None, help="config file (default: delvewright.toml + .local)")
    p.add_argument("--delvec", default=None, help="delvec invocation (default: $DELVEC or cargo run)")
    p.add_argument("--batch-size", type=int, default=None, help="override [i18n] batch_size")
    p.add_argument("--dry-run", action="store_true", help="print prompts and keys; make no API call")
    p.add_argument("--force", action="store_true", help="retranslate keys that already have a translation")
    p.add_argument("--no-validate", action="store_true", help="skip the closing `delvec validate`")
    args = p.parse_args(argv)

    try:
        cfg = load_config(explicit=args.config)
    except ConfigError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    if cfg is None:
        print(
            f"error: no [i18n] section in {CONFIG_FILE}/{LOCAL_CONFIG_FILE} — external "
            "translation is not configured (see docs/reference/i18n.md)",
            file=sys.stderr,
        )
        return 2
    if args.batch_size:
        cfg = dataclasses.replace(cfg, batch_size=args.batch_size)

    api_key = cfg.api_key()
    if api_key is None and not args.dry_run:
        print(
            f"error: ${cfg.api_key_env} is unset — export the key for {cfg.base_url} "
            "(it is read at call time and never stored)",
            file=sys.stderr,
        )
        return 2

    delvec = delvec_command(args.delvec)
    try:
        inv = fetch_inventory(args.campaign_dir, args.lang, delvec)
    except (TranslateError, json.JSONDecodeError, OSError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    if not inv.declared:
        print(
            f"error: `{args.lang}` is not declared in world.json `languages` — declare it "
            "first (the compiler validates coverage per declared language)",
            file=sys.stderr,
        )
        return 1

    pending = inv.pending(force=args.force)
    print(
        f"{inv.campaign_id} -> {args.lang}: {len(inv.entries)} inventory keys, "
        f"{len(pending)} to translate ({len(inv.entries) - len(pending)} already present)"
    )

    if args.dry_run:
        for i, chunk, messages in _iter_batches(inv, pending, cfg, args.lang):
            print(f"\n===== batch {i} ({len(chunk)} keys) -> {cfg.endpoint} model={cfg.model} "
                  f"temperature={cfg.temperature} =====")
            for m in messages:
                print(f"--- {m['role']} ---\n{m['content']}")
        print(f"\ndry run: no request sent (key would come from ${cfg.api_key_env})")
        return 0

    translated: dict[str, str] = {}
    assert api_key is not None
    for i, chunk, messages in _iter_batches(inv, pending, cfg, args.lang):
        print(f"batch {i}: {len(chunk)} keys -> {cfg.model} ...", flush=True)
        try:
            reply = chat_once(cfg, messages, api_key)
            got = parse_translations(reply)
        except TranslateError as exc:
            print(f"error: {exc}", file=sys.stderr)
            return 1
        wanted = {e.key for e in chunk}
        missing = sorted(wanted - got.keys())
        if missing:
            print(
                f"error: batch {i} reply omitted {len(missing)} key(s): {missing[:5]}",
                file=sys.stderr,
            )
            return 1
        translated.update({k: v for k, v in got.items() if k in wanted})

    path = sidecar_path(args.campaign_dir, args.lang)
    write_sidecar(path, inv, merge_content(inv, translated))
    print(f"wrote {path} ({len(inv.entries)} keys, {len(translated)} newly translated)")

    if args.no_validate:
        return 0
    proc = run_delvec(["validate", str(args.campaign_dir)], delvec)
    sys.stdout.write(proc.stdout)
    sys.stderr.write(proc.stderr)
    print(
        "coverage: `delvec validate` "
        + ("passed — sidecar covers the inventory exactly" if proc.returncode == 0 else "FAILED")
    )
    return proc.returncode


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
