#!/usr/bin/env python3
"""Transcreate a campaign's l10n sidecar with an external OpenAI-compatible LLM API.

Generation-time tooling only. Shipped delves never call an LLM (CLAUDE.md
forbidden zones): this writes `l10n/<code>.json` into the campaign source, the
compiler bakes those strings at build time, and the running server talks to
nothing. See `docs/reference/i18n.md`.

Another language is transcreated from the English, never translated line by
line (CLAUDE.md, Conventions): each line is rewritten from its intent, with the
facts, names and keys taken from the English. So every row the model is sent
carries that intent beside its English — derived by `delvec`, never typed.

Pipeline:

1. `delvec l10n-inventory <campaign-dir> --lang <code>` — the authoritative key
   inventory (exactly the key set `DW0180`/`DW0181` enforce), each row carrying its
   canonical English, its `kind` of text, the NPC whose voice it is, the
   `situation` it is said in (quest, objective, beat, what the speaker is doing,
   the line an option answers), whatever the current sidecar already holds, and
   whether that is `stale` (translated from English the line no longer reads).
2. Names first: rows of kind `name`/`title` are sent before everything else, so
   every later batch is handed the rendering of each name its English mentions.
3. Batch the pending rows into chat-completions requests: the system prompt
   carries the target language's writing rules, read from
   `docs/reference/game-writing.md` (one source; never a copy here), plus the
   translationese checklist. Each request is a `Step` carrying the reply shape
   its own prompt asks for — JSON object, or free text.
4. With `--reflect`, run each batch as three steps — transcreate, criticise the
   draft, revise with the critique in hand, returning already-good lines
   byte-identical.
5. Fact-check every answer mechanically: placeholders, formatting codes and
   styled-span markers (`[[<styles>|`, `]]`, spec-0096) kept, numbers kept, every declared name rendered the one way the campaign renders it
   and no two names sharing a rendering. A failing row is sent back once with
   its failures named; a row that still fails is refused — left out of the
   sidecar and named in the report, so the closing `delvec validate` says it is
   missing rather than a wrong line shipping.
6. Merge, write the sidecar (exactly the inventory, so no orphans) and hand it to
   `delvec fmt` for canonical form, then run `delvec validate` and report
   coverage.

Idempotent: a re-run sends only the keys the sidecar is missing or holds stale
(`--force` redoes everything). `--dry-run` prints the exact prompts and key lists
and makes no network call.

The API key is read from the environment variable *named* in the config
(`api_key_env`) at call time, and is never stored, echoed, or logged.

Stdlib only (python >= 3.11 for `tomllib`).
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import time
import tomllib
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Iterable, Sequence

REPO_ROOT = Path(__file__).resolve().parents[2]

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

#: Whether to run the three-step translate -> reflect -> improve pass. Off by
#: default because it triples the request count; the `/new-delve` localization
#: stage turns it on (`--reflect`), and `[i18n] reflect = true` makes that the
#: standing behaviour for a repo. See `docs/reference/i18n.md`.
DEFAULT_REFLECT = False

#: The kinds of row (`delvec l10n-inventory`'s `kind`, `dsl::key_kind`) that name
#: a thing rather than say something about it. They are sent first, and their
#: renderings are the campaign's name map: the glossary every later batch is
#: handed and the fact check holds every row to.
NAME_KINDS = ("name", "title")

#: Where the writing rules live. The system prompt reads them from here at call
#: time — one source, never a copy in this file.
GAME_WRITING = REPO_ROOT / "docs" / "reference" / "game-writing.md"

#: The `game-writing.md` section holding a target language's writing rules, by
#: primary subtag. A language with no section gets the general rules only.
WRITING_SECTION_BY_LANG = {"zh": "Chinese"}


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
    reflect: bool = DEFAULT_REFLECT

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
        reflect=bool(merged.get("reflect", DEFAULT_REFLECT)),
    )


# ------------------------------------------------------------------ inventory --


@dataclass(frozen=True)
class Entry:
    """One inventory row from `delvec l10n-inventory`."""

    key: str
    en: str
    #: The class of text (`dsl::key_kind`): `dialogue`, `option-label`, …
    kind: str | None = None
    speaker: str | None = None
    #: The campaign context the line is said in (`dsl::key_situations`).
    situation: tuple[str, ...] = ()
    existing: str | None = None
    #: `existing` was made from English the line no longer reads (DW0187).
    stale: bool = False

    @property
    def is_name(self) -> bool:
        return self.kind in NAME_KINDS


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
        """Rows to send (all of them under `--force`): untranslated or stale,
        names and titles first so every later batch has their renderings."""
        rows = [e for e in self.entries if force or e.existing is None or e.stale]
        return sorted(rows, key=lambda e: not e.is_name)

    def glossary(self, redo: Iterable[Entry] = ()) -> dict[str, str]:
        """The names the sidecar already renders, English to rendering — the
        first rendering of a name wins. Stale rows and the rows `redo` names (this
        run's pending rows) are excluded: a name being rewritten is not settled."""
        skip = {e.key for e in redo}
        return name_map((e for e in self.entries if e.key not in skip), {})


def name_map(entries: Iterable[Entry], translated: dict[str, str]) -> dict[str, str]:
    """English name -> its one rendering, from every `name`/`title` row: this
    run's rendering where it made one, else the sidecar's (when not stale). The
    first row to render a name fixes it; later rows are held to it."""
    out: dict[str, str] = {}
    for e in entries:
        if not e.is_name:
            continue
        value = translated.get(e.key)
        if value is None and not e.stale:
            value = e.existing
        if value is not None:
            out.setdefault(e.en, value)
    return out


def parse_inventory(doc: dict[str, Any]) -> Inventory:
    """Parse the `delvec l10n-inventory` JSON document."""
    try:
        entries = [
            Entry(
                key=e["key"],
                en=e["en"],
                kind=e.get("kind"),
                speaker=e.get("speaker"),
                situation=tuple(e.get("situation", ())),
                existing=e.get("existing"),
                stale=bool(e.get("stale", False)),
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
You are a native {lang} game writer localizing a Minecraft adventure map. You
transcreate: you do not translate sentence by sentence. For every line you are
given its English, the kind of text it is, who says it, and the situation it is
said in. Write the line a native {lang} writer would write for that moment, for
that speaker, doing that job. The English is the fact source: keep every fact,
name, number and direction it states, and add none.

Rules:
- Reply with ONE JSON object mapping every given key to its string. No prose, no
  explanation, no markdown fences, no extra or missing keys.
- `kind` says what the line is for:
  - `objective`, `refusal`, `prompt`: functional text. Say the action, the object
    and the place plainly; a refusal says what is wrong and what fixes it.
  - `dialogue`, `bark`: an NPC speaking. `speaker` names them; their persona and
    speech style are in the context. Make it sound like that person talking now.
  - `narration`: what happened, said plainly.
  - `option-label`: the PLAYER's own reply, drawn on a FIXED-WIDTH BUTTON. A label
    too wide for the button scrolls, which is a broken-looking UI — so a label is
    a caption, never a sentence. Budget: about 146 font pixels, i.e. roughly
    20 Latin characters or 12 Han characters. Being shorter than the English here
    is correct, not a loss.
  - `button-tooltip`: the full line or consequence that button stands for, shown
    in a hover box; it reads as the longer form of its sibling label.
  - `item-name`, `item-tooltip`: an item's name, and what the item does first.
  - `name`, `title`, `description`: proper names, headings, a class's role.
- `situation` is context, never text to render: use it to understand what the
  line must do, then write only the line.
- Every name in the glossary is written exactly as the glossary renders it, every
  time it appears. One name, one rendering; two names never share one.
- Preserve every placeholder (`%s`, `%1$s`), formatting code and digit exactly.
- A styled span is written `[[<styles>|<text>]]`. Keep every span of the English:
  copy its opener (`[[` up to and including `|`) and its closing `]]` byte for
  byte, transcreate only the text between them, and place the span where the
  {lang} line puts that phrase. Never add, drop or merge a span.
- Keep strings roughly as short as the English: they render in chat lines,
  item names, and title cards.
{writing_rules}{translationese}"""

#: Target-language-specific translationese guidance, appended to every prompt in
#: the three-step pass. Re-derived in our own words from the standard Chinese
#: translation-criticism tradition (the 余光中 / 思果 lineage of 翻译腔 critique) —
#: those texts are copyrighted and the prompt collections that circulate them are
#: unlicensed, so nothing here is quoted from either. See `docs/ACKNOWLEDGEMENTS.md`.
ZH_TRANSLATIONESE = """\
- Guard against 翻译腔 (English grammar wearing Chinese words). The governing
  rule: exhaust the constructions Chinese already has before importing one.
  Specifically:
  - 名词化: restore the verb — 作出决定 → 决定, 进行讨论 → 讨论.
  - 弱动词: 进行 / 加以 / 作出 / 予以 + noun is almost always one plain verb.
  - 的的不休: strings of 的. Chinese carries most modification by word order.
  - 被: Chinese marks the passive rarely, and mostly for things done TO someone
    against their interest. Prefer an active subject, or no marker at all.
  - Front-loaded modifiers: an English relative clause transplanted whole in
    front of the noun. Split it into a second clause.
  - 地 on every adverb, 们 on every plural, 当……的时候 for every temporal clause,
    a possessive pronoun before every body part — all English habits.
- 信达雅, read as a precedence: 信 first (say what the English says), then 达
  (a Chinese reader takes it in at one pass), then 雅 — and 雅 here means the
  register the scene actually calls for, not ornament. A soldier's clipped line
  stays clipped."""

TRANSLATIONESE_BY_LANG = {"zh": ZH_TRANSLATIONESE}


def writing_rules(lang: str, path: Path | None = None) -> str:
    """The target language's writing rules, read from `game-writing.md`.

    The section is the one `WRITING_SECTION_BY_LANG` names (`## 4. Chinese` for
    `zh-*`); each rule is taken up to its `**Cited**`/`**Authored**` tail, which
    is provenance for a human reader, not instruction. A language with no
    section gets `""`. A language that HAS one whose section cannot be found is
    an error: the rules moved, and a prompt silently without them would be the
    old line-by-line translation again.
    """
    title = WRITING_SECTION_BY_LANG.get(lang.split("-")[0].lower())
    if title is None:
        return ""
    path = GAME_WRITING if path is None else path
    try:
        text = path.read_text("utf-8")
    except OSError as exc:
        raise TranslateError(f"cannot read the writing rules at {path}: {exc}") from exc
    match = re.search(
        rf"^## (?:\d+\. )?{re.escape(title)}\s*$(.*?)(?=^## |\Z)", text, re.M | re.S
    )
    if match is None:
        raise TranslateError(f"{path} has no `## {title}` section — the {lang} writing rules moved")
    rules: list[str] = []
    for block in re.split(r"\n(?=- \*\*)", match.group(1).strip()):
        if not block.startswith("- **"):
            continue
        body = " ".join(block.split())
        body = re.split(r"\s\*\*(?:Cited|Authored)\b", body, maxsplit=1)[0]
        rules.append(body.rstrip())
    if not rules:
        raise TranslateError(f"{path} `## {title}` holds no rules")
    return f"\nWriting rules for {lang} (from the project's writing guide):\n" + "\n".join(rules) + "\n"


def translationese_guidance(lang: str) -> str:
    """Language-specific translationese guidance, or `""` when we have none.

    Matched on the primary subtag, so `zh`, `zh-cn` and `zh-tw` share a block.
    An unknown language simply gets the general rules — never a wrong checklist.
    """
    return TRANSLATIONESE_BY_LANG.get(lang.split("-")[0].lower(), "")


#: Step 2 of the three-step pass: criticise the draft before rewriting it. The
#: model must write the defect down, which is what makes step 3 more than a
#: re-roll. Structure and the four critique axes follow `andrewyng/translation-agent`
#: (MIT, (c) 2024 Andrew Ng); the axes are extended with our own domain criteria
#: (persona, key-kind conventions, render width) and the translationese block.
REFLECTION_PROMPT = """\
You are a senior localization editor reviewing a draft of a Minecraft adventure
map's player-facing strings, transcreated from English into {lang}.

Read each row's English, kind, speaker and situation beside the draft and write
specific, constructive criticism. One suggestion per problem, each naming the
key it applies to. Do NOT write a corrected line — this step only diagnoses.

Judge on four axes:
1. ACCURACY — every fact, name, number and direction of the English kept; nothing
   added; placeholders, symbols and formatting sequences unaltered.
2. FLUENCY — would a native {lang} writer have written this line for this moment?
   Grammar, punctuation and idiom; anything a native reader would have to re-read;
   English sentence structure carried across.
3. STYLE / REGISTER — does the line do its kind's job and sound like the speaking
   NPC's persona and speech style? Does the string still fit a chat line, item
   name or title card? `option-label` lines are drawn on a fixed-width button and
   SCROLL if they overrun — flag any that exceed roughly 20 Latin or 12 Han
   characters, and say how to cut it to a caption.
4. TERMINOLOGY — glossary names reproduced exactly; one English term rendered the
   same way at every key.

Say so plainly when a line is already accurate and natural — "no change" is a
valid and expected verdict, and most lines should get it. Do not invent
improvements to lines that do not need any.
{writing_rules}{translationese}"""

#: Step 3: apply the critique. Same output contract as step 1, plus an explicit
#: anti-churn rule — an unconditional rewrite pass degrades text that was fine.
IMPROVEMENT_PROMPT = """\
You are the writer again, revising your own draft with an editor's critique in
hand. Target language: {lang}.

- Reply with ONE JSON object mapping EVERY given key to its final string. No
  prose, no explanation, no markdown fences, no extra or missing keys.
- Apply the critique where it is right. Where you judge it wrong, keep your draft
  — you are the writer, not a patch applier.
- Where a draft line is already accurate and natural, return it BYTE-IDENTICAL.
  Rewriting a good line for the sake of motion is a defect, not an improvement.
- Every rule from the original brief still binds: the kind's job, persona voice,
  glossary names exactly, placeholders, span markers and digits preserved, lengths close to the
  English, and `option-label` rows are the player's own reply on a narrow button.
{writing_rules}{translationese}"""

#: The corrective step: rows the fact check refused, sent back once with each
#: failure named. Same output contract as step 1.
FIX_PROMPT = """\
You are the writer again. A mechanical check refused some of your lines in
{lang}; each row names what failed. Rewrite only those lines so the check
passes, changing nothing else about them that does not need to change.

- Reply with ONE JSON object mapping EVERY given key to its corrected string. No
  prose, no explanation, no markdown fences, no extra or missing keys.
- A glossary name must appear exactly as the glossary renders it. A placeholder,
  formatting code, styled-span marker (`[[<styles>|` and `]]`) or number must
  appear exactly as in the English.
{writing_rules}"""


def batch_glossary(batch: Sequence[Entry], names: dict[str, str]) -> dict[str, str]:
    """The slice of the name map a batch needs: every name its English mentions,
    and every name the batch itself renders (so a second rendering is visible)."""
    texts = [e.en for e in batch]
    return {
        en: target
        for en, target in sorted(names.items())
        if any(e.en == en for e in batch) or any(mentions(t, en) for t in texts)
    }


def _context(
    inv: Inventory, batch: Sequence[Entry], lang: str, names: dict[str, str] | None = None
) -> dict[str, Any]:
    """Campaign, speaking personas and glossary for one batch."""
    speakers = {e.speaker for e in batch if e.speaker}
    personas = [n for n in inv.npcs if n.get("id") in speakers]
    glossary = batch_glossary(batch, inv.glossary() if names is None else names)

    context: dict[str, Any] = {
        "campaign": inv.campaign_id,
        "world_title": inv.world_title,
        "target_language": lang,
    }
    if personas:
        context["speakers"] = personas
    if glossary:
        context["glossary_en_to_target"] = glossary
    return context


def _intent(e: Entry) -> dict[str, Any]:
    """One row as the model sees it: the English and its intent — kind, speaker,
    situation — each present only when the inventory carries it."""
    row: dict[str, Any] = {"key": e.key, "en": e.en}
    if e.kind:
        row["kind"] = e.kind
    if e.speaker:
        row["speaker"] = e.speaker
    if e.situation:
        row["situation"] = list(e.situation)
    return row


def _draft_rows(batch: Sequence[Entry], draft: dict[str, str]) -> list[dict[str, Any]]:
    """Source, intent and draft side by side — what steps 2 and 3 reason over."""
    return [{**_intent(e), "draft": draft.get(e.key, "")} for e in batch]


def _items(batch: Sequence[Entry]) -> list[dict[str, Any]]:
    return [_intent(e) for e in batch]


def _system(prompt: str, lang: str) -> dict[str, str]:
    return {
        "role": "system",
        "content": prompt.format(
            lang=lang,
            writing_rules=writing_rules(lang),
            translationese=translationese_guidance(lang),
        ),
    }


#: The token a provider looks for before it will honour `json_object`.
JSON_TOKEN = "json"


@dataclass(frozen=True)
class Step:
    """One chat-completions call: its messages **and the reply shape its own
    prompt asks for**.

    The reply shape travels with the prompt because it is a property of the
    prompt, not of the transport. Setting `response_format` once in the request
    builder made it one rule for three steps, and the three steps do not share
    one: translate and revise ask for a JSON object, the critique asks for prose.
    A provider enforces the agreement — OpenAI and DeepSeek both refuse
    `response_format: json_object` with `HTTP 400 "Prompt must contain the word
    'json' in some form"` when the prompt never asks for JSON — so the critique
    step could never be sent at all, and `--reflect` died on batch 1 before it
    had written a line.
    """

    #: Printed in progress and error lines: `translate`, `reflect`, `improve`.
    name: str
    messages: list[dict[str, str]]
    #: Whether to send `response_format: {"type": "json_object"}`.
    json_object: bool

    def mentions_json(self) -> bool:
        """Whether any message asks for JSON, which is what a provider checks."""
        return any(JSON_TOKEN in m.get("content", "").lower() for m in self.messages)


def _ctx_json(inv: Inventory, batch: Sequence[Entry], lang: str, names: dict[str, str] | None) -> str:
    return json.dumps(_context(inv, batch, lang, names), ensure_ascii=False, indent=2, sort_keys=True)


def translate_step(
    inv: Inventory, batch: Sequence[Entry], lang: str, names: dict[str, str] | None = None
) -> Step:
    """Step 1 — transcreate. Rules, campaign/persona/glossary context, and each
    row's English with its intent. JSON object."""
    user = (
        "Context:\n"
        + _ctx_json(inv, batch, lang, names)
        + "\n\nWrite these lines in "
        + lang
        + " from their intent and reply with the JSON object of key -> line:\n"
        + json.dumps(_items(batch), ensure_ascii=False, indent=2)
    )
    return Step(
        name="translate",
        messages=[_system(SYSTEM_PROMPT, lang), {"role": "user", "content": user}],
        json_object=True,
    )


def critique_step(
    inv: Inventory,
    batch: Sequence[Entry],
    lang: str,
    draft: dict[str, str],
    names: dict[str, str] | None = None,
) -> Step:
    """Step 2 — critique the draft. Free text, deliberately: making the model
    *write the defect down* is what stops step 3 being a second roll of the dice.
    So this step asks for no JSON and must not send `response_format`."""
    user = (
        "Context:\n"
        + _ctx_json(inv, batch, lang, names)
        + "\n\nSource, intent and draft:\n"
        + json.dumps(_draft_rows(batch, draft), ensure_ascii=False, indent=2)
        + "\n\nWrite your critique."
    )
    return Step(
        name="reflect",
        messages=[_system(REFLECTION_PROMPT, lang), {"role": "user", "content": user}],
        json_object=False,
    )


def revise_step(
    inv: Inventory,
    batch: Sequence[Entry],
    lang: str,
    draft: dict[str, str],
    critique: str,
    names: dict[str, str] | None = None,
) -> Step:
    """Step 3 — apply the critique and emit the final JSON object."""
    rows = _draft_rows(batch, draft)
    user = (
        "Context:\n"
        + _ctx_json(inv, batch, lang, names)
        + "\n\nSource, intent and draft:\n"
        + json.dumps(rows, ensure_ascii=False, indent=2)
        + "\n\nEditor's critique:\n"
        + critique.strip()
        + "\n\nReply with the JSON object of key -> final line:\n"
        + json.dumps([r["key"] for r in rows], ensure_ascii=False)
    )
    return Step(
        name="improve",
        messages=[_system(IMPROVEMENT_PROMPT, lang), {"role": "user", "content": user}],
        json_object=True,
    )


def fix_step(
    inv: Inventory,
    batch: Sequence[Entry],
    lang: str,
    draft: dict[str, str],
    failures: dict[str, list[str]],
    names: dict[str, str] | None = None,
) -> Step:
    """The corrective step: the refused rows, each with its draft and the
    failures the fact check named. JSON object."""
    rows = [
        {**_intent(e), "draft": draft.get(e.key, ""), "failed": failures[e.key]}
        for e in batch
        if e.key in failures
    ]
    user = (
        "Context:\n"
        + _ctx_json(inv, batch, lang, names)
        + "\n\nRefused lines:\n"
        + json.dumps(rows, ensure_ascii=False, indent=2)
        + "\n\nReply with the JSON object of key -> corrected line."
    )
    return Step(
        name="fix",
        messages=[_system(FIX_PROMPT, lang), {"role": "user", "content": user}],
        json_object=True,
    )


def build_request(
    cfg: I18nConfig, step: Step, api_key: str
) -> tuple[str, dict[str, Any], dict[str, str]]:
    """`(url, body, headers)` for one chat-completions call. The key only ever
    appears in the returned `Authorization` header — never in the body or a log.

    Refuses before the network when a step asks for `json_object` and its own
    prompt never says `json`: that pairing is the provider's documented `HTTP
    400`, and a tool that can tell locally should not spend a request finding
    out. The check is the one that makes [`Step.json_object`] and the prompt text
    a pair rather than two independent settings.
    """
    body: dict[str, Any] = {
        "model": cfg.model,
        "messages": list(step.messages),
        "temperature": cfg.temperature,
        "stream": False,
    }
    if step.json_object:
        if not step.mentions_json():
            raise TranslateError(
                f"step `{step.name}` asks for response_format=json_object but no message "
                f"says `{JSON_TOKEN}` — OpenAI and DeepSeek both reject that pairing with "
                "HTTP 400. Either say so in the prompt, or set json_object=False for a "
                "prose step."
            )
        # Supported by OpenAI, DeepSeek and Moonshot/Kimi; `parse_translations`
        # still tolerates a fenced reply from a provider that ignores it.
        body["response_format"] = {"type": "json_object"}
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


#: How much of a provider's error body reaches the diagnostic. Enough for the
#: sentence a provider writes ("Prompt must contain the word 'json' …"), short
#: enough that a stack of retries stays readable.
ERROR_BODY_CHARS = 400


def provider_reason(exc: urllib.error.HTTPError, api_key: str) -> str:
    """The provider's own explanation of a rejection, key-redacted.

    A bare `HTTP 400` says a request was refused and nothing about why, which is
    how `--reflect` came to fail for months with the reason sitting unread in the
    response body. The key can only be in the body if the provider echoed it,
    which no provider here does — it is redacted anyway, because the rule is that
    the key never reaches a log, not that it probably will not.
    """
    try:
        raw = exc.read().decode("utf-8", "replace")
    except Exception:  # noqa: BLE001 — a body we cannot read is simply absent
        return ""
    text = " ".join(raw.split())
    if api_key:
        text = text.replace(api_key, "<redacted>")
    if not text:
        return ""
    if len(text) > ERROR_BODY_CHARS:
        text = text[:ERROR_BODY_CHARS] + "…"
    return f": {text}"


def chat_once(
    cfg: I18nConfig,
    step: Step,
    api_key: str,
    poster: Poster | None = None,
    sleep: Callable[[float], None] = time.sleep,
) -> str:
    """One chat completion, retried on transport errors. Returns the reply text.

    Errors are re-raised with the endpoint, the step, the status and the
    provider's own key-redacted explanation — never the request headers, so the
    key cannot reach a log through an exception.

    `poster` is resolved at call time (not bound as a default) so a test that
    replaces the module's `post_json` truly intercepts every request — a default
    argument would have captured the real one at import and gone to the network.
    """
    poster = poster or post_json
    url, body, headers = build_request(cfg, step, api_key)
    last: Exception | None = None
    for attempt in range(cfg.max_retries):
        try:
            resp = poster(url, body, headers, cfg.timeout_seconds)
            return resp["choices"][0]["message"]["content"]
        except urllib.error.HTTPError as exc:
            last = TranslateError(
                f"{cfg.endpoint} returned HTTP {exc.code} on step `{step.name}`"
                f"{provider_reason(exc, api_key)}"
            )
            if exc.code in (400, 401, 403, 404, 422):
                raise last from None  # not transient: bad key/model/url
        except (urllib.error.URLError, TimeoutError, OSError) as exc:
            last = TranslateError(f"{cfg.endpoint} unreachable: {exc}")
        except (KeyError, IndexError, TypeError) as exc:
            last = TranslateError(f"unexpected response shape from {cfg.endpoint}: {exc}")
        if attempt + 1 < cfg.max_retries:
            sleep(2.0 * (attempt + 1))
    raise last or TranslateError("translation request failed")


# ----------------------------------------------------------------- fact check --
#
# Transcreation licenses the model to rewrite a line, and freer writing loses
# facts (docs/reference/game-writing.md §7). These checks are the mechanical
# half of "the facts, names and keys taken from the English": what a machine can
# decide without reading for meaning. A row that fails is sent back once with
# its failures named, and refused if it still fails.

#: A placeholder or formatting code the client substitutes or interprets, or a
#: styled-span marker (spec-0096): a span's opener `[[<styles>|` and its `]]`.
#: Compared as tokens, like a placeholder; the grammar itself is the compiler's
#: (`dsl::textstyle`), whose `DW0975`/`DW0976` the closing `delvec validate` runs.
PLACEHOLDER_RE = re.compile(r"%(?:\d+\$)?[sd]|%%|§[0-9a-fk-or]|\[\[[^\[\]|]*\||\]\]")
#: A number written in digits, thousands separators included (`10,811`); its
#: value is compared with the separators removed, so `10811` keeps it.
DIGITS_RE = re.compile(r"\d{1,3}(?:,\d{3})+|\d+")


def digit_numbers(text: str) -> list[str]:
    """Every number `text` writes in digits, separators removed."""
    return [m.replace(",", "") for m in DIGITS_RE.findall(text)]

#: English number words the check reads. `one` is excluded: it is a pronoun far
#: more often than a count ("no one", "the one true thing").
_UNITS = {
    "two": 2, "three": 3, "four": 4, "five": 5, "six": 6, "seven": 7, "eight": 8,
    "nine": 9, "ten": 10, "eleven": 11, "twelve": 12, "thirteen": 13,
    "fourteen": 14, "fifteen": 15, "sixteen": 16, "seventeen": 17,
    "eighteen": 18, "nineteen": 19,
}
_TENS = {
    "twenty": 20, "thirty": 30, "forty": 40, "fifty": 50, "sixty": 60,
    "seventy": 70, "eighty": 80, "ninety": 90,
}
_UNIT_DIGITS = {"one": 1, **{k: v for k, v in _UNITS.items() if v < 10}}
_NUMBER_WORD_RE = re.compile(
    r"\b(?:(?P<tens>" + "|".join(_TENS) + r")(?:-(?P<unit>" + "|".join(_UNIT_DIGITS) + r"))?"
    r"|(?P<small>" + "|".join(_UNITS) + r"))"
    r"(?:\s+(?P<scale>hundred|thousand))?\b",
    re.I,
)


def english_numbers(text: str) -> list[int]:
    """Every count the English states in words (`two` … `ninety-nine`, optionally
    `hundred`/`thousand`). Digits are checked separately, as digits."""
    out = []
    for m in _NUMBER_WORD_RE.finditer(text):
        if m.group("tens"):
            n = _TENS[m.group("tens").lower()] + _UNIT_DIGITS.get((m.group("unit") or "").lower(), 0)
        else:
            n = _UNITS[m.group("small").lower()]
        scale = (m.group("scale") or "").lower()
        n *= {"hundred": 100, "thousand": 1000}.get(scale, 1)
        out.append(n)
    return out


_ZH_DIGITS = "零一二三四五六七八九"


def zh_numerals(n: int) -> set[str]:
    """The ways Chinese writes a count below ten thousand: 十六, 四十, 一百零五, and
    the 两/俩/双 forms of two."""
    def render(n: int) -> str:
        if n < 10:
            return _ZH_DIGITS[n]
        if n < 20:
            return "十" + (_ZH_DIGITS[n % 10] if n % 10 else "")
        parts, zero = [], False
        for value, unit in ((1000, "千"), (100, "百"), (10, "十"), (1, "")):
            d = n // value % 10
            if d:
                if zero and parts:
                    parts.append("零")
                parts.append(_ZH_DIGITS[d] + unit)
                zero = False
            elif parts:
                zero = True
        return "".join(parts)

    forms = {render(n), str(n)}
    if n == 2:
        forms |= {"两", "俩", "双"}
    if n in (200, 2000):
        forms.add("两" + render(n)[1:])
    if 100 <= n < 200 or 1000 <= n < 2000:
        forms.add(render(n)[1:])  # 百, 千: the leading 一 is optional
    return forms


def _name_core(name: str) -> str:
    """The part of a name a sentence repeats: `The Keeper` is mentioned as `the
    Keeper`, so the article is not part of the match."""
    return re.sub(r"^(?:the|a|an)\s+", "", name, flags=re.I)


def mentions(text: str, name: str) -> bool:
    """Whether English `text` mentions the declared `name` (case-sensitive, on word
    boundaries, article-insensitive)."""
    core = _name_core(name)
    return bool(core) and re.search(rf"(?<!\w){re.escape(core)}(?!\w)", text) is not None


def mentioned_names(text: str, names: Iterable[str]) -> list[str]:
    """The declared names `text` mentions, dropping a name whose every mention
    sits inside a longer name's (`Warden` inside `Warden's Door`)."""
    spans: dict[str, list[tuple[int, int]]] = {}
    for name in names:
        core = _name_core(name)
        if not core:
            continue
        found = [m.span() for m in re.finditer(rf"(?<!\w){re.escape(core)}(?!\w)", text)]
        if found:
            spans[name] = found
    out = []
    for name, own in spans.items():
        covered = all(
            any(s <= a and b <= e and (e - s) > (b - a) for other, os_ in spans.items() if other != name for s, e in os_)
            for a, b in own
        )
        if not covered:
            out.append(name)
    return sorted(out)


def check_row(e: Entry, text: str, names: dict[str, str], lang: str) -> list[str]:
    """Every mechanical fact `text` (the target-language line for `e`) loses."""
    failures: list[str] = []
    if not text.strip():
        return ["the line is empty"]
    want, got = sorted(PLACEHOLDER_RE.findall(e.en)), sorted(PLACEHOLDER_RE.findall(text))
    if want != got:
        failures.append(f"placeholders/formatting codes/span markers {want} became {got}")
    have = digit_numbers(text)
    for d in digit_numbers(e.en):
        if d in have:
            have.remove(d)
        else:
            failures.append(f"the number {d} is missing")
    if lang.split("-")[0].lower() == "zh":
        for n in english_numbers(e.en):
            if not any(form in text for form in zh_numerals(n)):
                failures.append(f"the number {n} (written in words in the English) is missing")
    if not e.is_name:
        for name in mentioned_names(e.en, names):
            if names[name] not in text:
                failures.append(f"the name `{name}` must be written `{names[name]}`")
    return failures


def check_names(
    batch: Sequence[Entry], got: dict[str, str], names: dict[str, str]
) -> dict[str, list[str]]:
    """One name, one rendering; one rendering, one name — for the `name`/`title`
    rows of this batch against the campaign's name map (which holds every name
    already settled, the sidecar's and this run's)."""
    failures: dict[str, list[str]] = {}
    seen = dict(names)
    owner = {v: k for k, v in reversed(list(seen.items()))}
    for e in batch:
        if not e.is_name or e.key not in got:
            continue
        rendering = got[e.key]
        settled = seen.get(e.en)
        if settled is not None and settled != rendering:
            failures.setdefault(e.key, []).append(
                f"`{e.en}` is already written `{settled}` elsewhere in this campaign"
            )
            continue
        other = owner.get(rendering)
        if other is not None and other != e.en:
            failures.setdefault(e.key, []).append(
                f"`{rendering}` already names `{other}`; two names may not share one rendering"
            )
            continue
        seen.setdefault(e.en, rendering)
        owner.setdefault(rendering, e.en)
    return failures


def fact_check(
    batch: Sequence[Entry], got: dict[str, str], names: dict[str, str], lang: str
) -> dict[str, list[str]]:
    """Every row of `batch` whose line in `got` fails a mechanical check, with
    the failures named. `names` is the name map as settled before this batch;
    the batch's own accepted names join it for the non-name rows."""
    failures = check_names(batch, got, names)
    merged = dict(names)
    for e in batch:
        if e.is_name and e.key in got and e.key not in failures:
            merged.setdefault(e.en, got[e.key])
    for e in batch:
        if e.key not in got:
            continue
        found = check_row(e, got[e.key], merged, lang)
        if found:
            failures.setdefault(e.key, []).extend(found)
    return failures


# --------------------------------------------------------------------- sidecar --


def merge_content(inv: Inventory, translated: dict[str, str]) -> dict[str, str]:
    """The sidecar `content`: exactly the inventory keys, existing translations
    kept unless replaced or stale. Orphans cannot survive — the map is built from the
    inventory, so `DW0181` is unreachable by construction."""
    out: dict[str, str] = {}
    for e in inv.entries:
        # A stale row this run did not redo (it was refused) is dropped, never
        # kept: its translation says a line the English no longer says, and
        # recording today's English as its `source` would hide that (DW0187).
        value = translated.get(e.key, None if e.stale else e.existing)
        if value is not None:
            out[e.key] = value
    return out


def sidecar_path(campaign_dir: Path, lang: str) -> Path:
    return campaign_dir / "l10n" / f"{lang}.json"


def write_sidecar(
    path: Path, inv: Inventory, content: dict[str, str], delvec: Sequence[str]
) -> None:
    """Write `l10n/<code>.json` **in canonical form**, by handing the finished
    file to `delvec fmt`.

    Canonical form is defined by one authority — `crates/dsl/src/fmt.rs` — and
    this tool is not a second one. It used to write the envelope in the order the
    dict was built and to carry an existing sidecar's `dsl_version` forward, and
    both disagree with the formatter: canonical order sorts every object's keys,
    and `delvec fmt` stamps the `dsl_version` this engine implements (ADR-0024).
    Every sidecar this tool had ever written was therefore refused by
    `delvec fmt --check` with `DW0773`, an error tier, until somebody noticed and
    ran the formatter by hand. Running it here is what closes that: the bytes on
    disk are the formatter's own output, so the writer cannot drift from the
    check again — including when the canonical form changes.

    `source` records the canonical English each translated row was made from, which
    is what lets the compiler DETECT a stale translation (`DW0187`) instead of
    trusting an audit: coverage checks compare key sets, and rewriting an authored
    line moves no key. It is written for exactly the rows `content` carries, from
    the same inventory those rows were keyed by, so the two cannot drift.

    Re-running the tool over a sidecar that predates `source` therefore adopts the
    guard with no retranslation: every row it already had is recorded against the
    English the inventory holds today (`DW0188` counts the rows still unguarded)."""
    doc = {
        "dsl_version": inv.dsl_version,
        "campaign_id": inv.campaign_id,
        "kind": "l10n",
        "lang": inv.lang,
        "content": dict(sorted(content.items())),
        "source": {e.key: e.en for e in sorted(inv.entries, key=lambda x: x.key) if e.key in content},
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", "utf-8")
    proc = run_delvec(["fmt", str(path)], delvec)
    if proc.returncode != 0:
        raise TranslateError(
            f"`delvec fmt {path}` failed (exit {proc.returncode}) — the sidecar was written "
            f"but is not in canonical form:\n{proc.stdout}{proc.stderr}"
        )


# ------------------------------------------------------------------------ cli --


class NoDelvec(RuntimeError):
    """No `delvec` of the pinned engine is available to run."""


def _delvec_version(binary: str) -> str | None:
    """The engine version a `delvec` answers (`delvec <engine>, dsl …`), or None."""
    try:
        out = subprocess.run(
            [binary, "--version"], capture_output=True, text=True, check=False
        ).stdout.split()
    except OSError:
        return None
    if len(out) >= 2 and out[0] == "delvec":
        return out[1].rstrip(",")
    return None


def delvec_command(explicit: str | None) -> list[str]:
    """How to invoke `delvec`: `--delvec`, then `$DELVEC`, then the engine the
    creator has — the `delvec` on `PATH`, then the tree's `target/release/delvec`
    — each taken only when its `--version` is the engine `versions.toml` pins
    (the rule `tools/lib/delvec-bin.sh` applies). Never a source build: a creator
    has the release binary and no toolchain. Raises `NoDelvec` when none answers.
    """
    cmd = explicit or os.environ.get("DELVEC")
    if cmd:
        return shlex.split(cmd)
    sys.path.insert(0, str(REPO_ROOT / "tools" / "lib"))
    try:
        import versions  # type: ignore[import-not-found]

        pin = versions.engine_version()
    finally:
        sys.path.pop(0)
    seen: list[str] = []
    for found in (shutil.which("delvec"), str(REPO_ROOT / "target" / "release" / "delvec")):
        if not found or not Path(found).is_file():
            continue
        version = _delvec_version(found)
        if version == pin:
            return [found]
        seen.append(f"{found} answers {version or '<no version line>'}")
    raise NoDelvec(
        f"no delvec of engine {pin} is available ({'; '.join(seen) or 'none on PATH'}). "
        "Install the release archive for that version (the skill's Init does this) "
        "or pass --delvec <binary>."
    )


class NoPrefabs(RuntimeError):
    """No prefab library was named, or the one named is not a directory."""


def prefabs_root(explicit: Path | None) -> Path:
    """The prefab library every `delvec` call of the run reads: `--prefabs`, then
    `$DELVEWRIGHT_PREFABS`. Never a guessed path — `delvec`'s own default,
    `campaigns/prefabs`, resolves only where the engine tree carries the `campaigns/`
    symlink, which a creator's content clone does not."""
    named = explicit if explicit is not None else (os.environ.get("DELVEWRIGHT_PREFABS") or None)
    if named is None:
        raise NoPrefabs(
            "no prefab library named — pass --prefabs <dir> or source ~/.delvewright/env.sh "
            "(it exports DELVEWRIGHT_PREFABS)"
        )
    path = Path(named)
    if not path.is_dir():
        raise NoPrefabs(
            f"the prefab library {path} is not a directory — fix --prefabs, or "
            "DELVEWRIGHT_PREFABS in ~/.delvewright/env.sh"
        )
    return path


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


def plan_batches(pending: Sequence[Entry], size: int) -> list[list[Entry]]:
    """Request-sized batches: every `name`/`title` row first, in batches of
    their own, so every later batch is handed the rendering of each name its
    English mentions; then the rest in inventory order."""
    names = [e for e in pending if e.is_name]
    rest = [e for e in pending if not e.is_name]
    return batches(names, size) + batches(rest, size)


def require_keys(got: dict[str, str], chunk: Sequence[Entry], label: str) -> dict[str, str]:
    """The reply restricted to the batch's keys — a reply that dropped any of
    them fails the run rather than writing a hole into the sidecar."""
    wanted = {e.key for e in chunk}
    missing = sorted(wanted - got.keys())
    if missing:
        raise TranslateError(
            f"{label} reply omitted {len(missing)} key(s): {missing[:5]}"
        )
    return {k: v for k, v in got.items() if k in wanted}


@dataclass
class BatchResult:
    """One batch's answer, after the fact check: the lines accepted, and the
    rows refused with what failed."""

    accepted: dict[str, str]
    refused: dict[str, list[str]]
    #: The refused rows' last answer, for the report.
    rejected_text: dict[str, str] = field(default_factory=dict)
    #: Rows that failed the first check and passed after the corrective step.
    fixed: list[str] = field(default_factory=list)


def translate_batch(
    cfg: I18nConfig,
    inv: Inventory,
    chunk: Sequence[Entry],
    lang: str,
    api_key: str,
    reflect: bool = False,
    label: str = "batch",
    names: dict[str, str] | None = None,
) -> BatchResult:
    """One batch, transcreated and fact-checked. With `reflect`, the three-step
    pass:

    1. transcreate,
    2. criticise the draft against accuracy / fluency / style / terminology
       (plus the language's writing rules and translationese checklist),
    3. revise with the critique in hand — returning unrevised lines unchanged.

    Step 2's reply is free text on purpose: forcing the model to *write the
    defect down* is what makes step 3 more than a second roll of the dice. That
    is why the reply shape is per step — see [`Step`].

    Then the fact check ([`fact_check`]); rows that fail go back once through
    [`fix_step`] with their failures named, and rows that still fail are refused.
    """
    names = inv.glossary() if names is None else names
    got = require_keys(
        parse_translations(chat_once(cfg, translate_step(inv, chunk, lang, names), api_key)),
        chunk,
        label,
    )
    if reflect:
        critique = chat_once(cfg, critique_step(inv, chunk, lang, got, names), api_key)
        got = require_keys(
            parse_translations(
                chat_once(cfg, revise_step(inv, chunk, lang, got, critique, names), api_key)
            ),
            chunk,
            f"{label} (improve)",
        )
    failures = fact_check(chunk, got, names, lang)
    fixed: list[str] = []
    if failures:
        retry = [e for e in chunk if e.key in failures]
        answer = require_keys(
            parse_translations(
                chat_once(cfg, fix_step(inv, chunk, lang, got, failures, names), api_key)
            ),
            retry,
            f"{label} (fix)",
        )
        got.update(answer)
        again = fact_check(chunk, got, names, lang)
        fixed = sorted(k for k in failures if k not in again)
        failures = again
    return BatchResult(
        accepted={k: v for k, v in got.items() if k not in failures},
        refused=failures,
        rejected_text={k: got[k] for k in failures},
        fixed=fixed,
    )


def settle_names(names: dict[str, str], chunk: Sequence[Entry], accepted: dict[str, str]) -> None:
    """Add a batch's accepted `name`/`title` rows to the running name map."""
    for e in chunk:
        if e.is_name and e.key in accepted:
            names.setdefault(e.en, accepted[e.key])


def report_refusals(
    refused: dict[str, list[str]], rejected: dict[str, str], inv: Inventory, out=None
) -> None:
    """Name every refused row: its English, the line refused, and what failed.
    `out` resolves at call time, so a redirected `sys.stdout` is honoured."""
    out = sys.stdout if out is None else out
    en = {e.key: e.en for e in inv.entries}
    for key in sorted(refused):
        print(f"  REFUSED {key}", file=out)
        print(f"    en:   {en.get(key, '')}", file=out)
        print(f"    got:  {rejected.get(key, '')}", file=out)
        for why in refused[key]:
            print(f"    fail: {why}", file=out)


def main(argv: Sequence[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        prog="i18n-translate.py",
        description="Transcreate a campaign's l10n sidecar with an external OpenAI-compatible LLM API "
        "(generation-time only; shipped delves never call an LLM).",
    )
    p.add_argument("campaign_dir", type=Path, help="campaign directory (holds world.json)")
    p.add_argument("--lang", required=True, help="target language code, e.g. zh-cn")
    p.add_argument("--config", type=Path, default=None, help="config file (default: delvewright.toml + .local)")
    p.add_argument("--delvec", default=None, help="delvec invocation (default: $DELVEC, then the pinned-version delvec on PATH)")
    p.add_argument("--prefabs", type=Path, default=None, help="the prefab library the campaign builds from (default: $DELVEWRIGHT_PREFABS, which ~/.delvewright/env.sh exports)")
    p.add_argument("--batch-size", type=int, default=None, help="override [i18n] batch_size")
    p.add_argument("--dry-run", action="store_true", help="print prompts and keys; make no API call")
    p.add_argument(
        "--reflect",
        action="store_true",
        help="three-step pass: transcreate, critique the draft, revise (3x the requests)",
    )
    p.add_argument(
        "--no-reflect", action="store_true", help="single pass, overriding [i18n] reflect"
    )
    p.add_argument("--force", action="store_true", help="redo keys that already have a current translation")
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
    if args.reflect and args.no_reflect:
        print("error: --reflect and --no-reflect are mutually exclusive", file=sys.stderr)
        return 2
    reflect = cfg.reflect
    if args.reflect:
        reflect = True
    if args.no_reflect:
        reflect = False

    api_key = cfg.api_key()
    if api_key is None and not args.dry_run:
        print(
            f"error: ${cfg.api_key_env} is unset — export the key for {cfg.base_url} "
            "(it is read at call time and never stored)",
            file=sys.stderr,
        )
        return 2

    try:
        delvec = delvec_command(args.delvec)
        delvec = [*delvec, "--prefabs", str(prefabs_root(args.prefabs))]
    except (NoDelvec, NoPrefabs) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
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
    stale = sum(1 for e in pending if e.stale)
    print(
        f"{inv.campaign_id} -> {args.lang}: {len(inv.entries)} inventory keys, "
        f"{len(pending)} to transcreate ({stale} stale), "
        f"{len(inv.entries) - len(pending)} already present"
    )
    try:
        writing_rules(args.lang)
    except TranslateError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    chunks = plan_batches(pending, cfg.batch_size)
    if args.dry_run:
        names = inv.glossary(redo=pending)
        for i, chunk in enumerate(chunks, start=1):
            print(f"\n===== batch {i} ({len(chunk)} keys) -> {cfg.endpoint} model={cfg.model} "
                  f"temperature={cfg.temperature} reflect={reflect} =====")
            steps = [translate_step(inv, chunk, args.lang, names)]
            if reflect:
                sample = {e.key: "<step-1 draft, filled at call time>" for e in chunk}
                steps.append(critique_step(inv, chunk, args.lang, sample, names))
                steps.append(
                    revise_step(
                        inv, chunk, args.lang, sample, "<step-2 critique, filled at call time>", names
                    )
                )
            for step in steps:
                shape = "json_object" if step.json_object else "free text"
                print(f"\n----- batch {i} step: {step.name} (reply: {shape}) -----")
                for m in step.messages:
                    print(f"--- {m['role']} ---\n{m['content']}")
        print(
            "\n(names settled by earlier batches join the glossary of later ones at call "
            "time; a row the fact check refuses is sent back once through a `fix` step)"
        )
        print(f"\ndry run: no request sent (key would come from ${cfg.api_key_env})")
        return 0

    translated: dict[str, str] = {}
    refused: dict[str, list[str]] = {}
    rejected: dict[str, str] = {}
    fixed: list[str] = []
    names = inv.glossary(redo=pending)
    assert api_key is not None
    steps = "translate -> reflect -> improve" if reflect else "translate"
    for i, chunk in enumerate(chunks, start=1):
        print(f"batch {i}: {len(chunk)} keys -> {cfg.model} ({steps}) ...", flush=True)
        try:
            result = translate_batch(
                cfg, inv, chunk, args.lang, api_key, reflect=reflect, label=f"batch {i}",
                names=names,
            )
        except TranslateError as exc:
            print(f"error: {exc}", file=sys.stderr)
            return 1
        translated.update(result.accepted)
        refused.update(result.refused)
        rejected.update(result.rejected_text)
        fixed.extend(result.fixed)
        settle_names(names, chunk, result.accepted)

    print(
        f"fact check: {len(pending)} rows checked against {len(names)} names; "
        f"{len(pending) - len(refused)} accepted ({len(fixed)} after one correction), "
        f"{len(refused)} refused"
    )
    for key in fixed:
        print(f"  fixed {key}")
    report_refusals(refused, rejected, inv)

    path = sidecar_path(args.campaign_dir, args.lang)
    try:
        write_sidecar(path, inv, merge_content(inv, translated), delvec)
    except TranslateError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    print(f"wrote {path} ({len(inv.entries)} keys, {len(translated)} newly written)")

    if args.no_validate:
        return 1 if refused else 0
    proc = run_delvec(["validate", str(args.campaign_dir)], delvec)
    sys.stdout.write(proc.stdout)
    sys.stderr.write(proc.stderr)
    print(
        "coverage: `delvec validate` "
        + ("passed — sidecar covers the inventory exactly" if proc.returncode == 0 else "FAILED")
    )
    if refused:
        print(
            f"{len(refused)} row(s) refused by the fact check are missing from the sidecar — "
            "write them by hand (or re-run) and validate again",
            file=sys.stderr,
        )
        return proc.returncode or 1
    return proc.returncode


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
