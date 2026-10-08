"""Unit tests for tools/creator/i18n-translate.py (external-LLM l10n translation).

No test may touch the network: the HTTP poster is always injected or monkeypatched
to explode. What is proven here is the request we *would* send, the config
resolution (including the fallback rule), inventory parsing, reply parsing and the
sidecar we write.
"""

import importlib.util
import io
import json
import os
import subprocess
import sys
import urllib.error
from pathlib import Path

import pytest

TOOL = Path(__file__).resolve().parents[1] / "creator" / "i18n-translate.py"


def _load():
    spec = importlib.util.spec_from_file_location("i18n_translate", TOOL)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["i18n_translate"] = mod
    spec.loader.exec_module(mod)
    return mod


t = _load()


@pytest.fixture(autouse=True)
def _named_delvec(monkeypatch):
    """`main` resolves a `delvec` before it fetches anything; tests that are not
    about resolution name one, as a creator would with `--delvec`."""
    monkeypatch.setenv("DELVEC", "delvec")


@pytest.fixture(autouse=True)
def _creator_prefabs(tmp_path_factory, monkeypatch):
    """The creator env `~/.delvewright/env.sh` exports; tests about the prefab
    root override or unset it."""
    monkeypatch.setenv("DELVEWRIGHT_PREFABS", str(tmp_path_factory.mktemp("prefabs")))

INVENTORY_DOC = {
    "campaign_id": "keep-trial",
    "dsl_version": "0.6.0",
    "lang": "zh-cn",
    "declared": True,
    "sidecar_present": True,
    "world_title": "The Stone Keep",
    "npcs": [
        {
            "id": "keeper",
            "name": "The Keeper",
            "archetype": "ruined captain",
            "speech_style": "clipped, soldierly",
            "motivation": "hold the gate",
        },
        {
            "id": "smith",
            "name": "The Smith",
            "archetype": "village smith",
            "speech_style": "warm, rambling",
            "motivation": "sell iron",
        },
    ],
    "entries": [
        {
            "key": "npc.keeper.name",
            "en": "The Keeper",
            "kind": "name",
            "speaker": "keeper",
            "existing": "守关人",
        },
        {"key": "world.title", "en": "The Stone Keep", "kind": "title", "existing": "石垒要塞"},
        {
            "key": "dlg.keeper.greet.text",
            "en": "You came. Good.",
            "kind": "dialogue",
            "speaker": "keeper",
            "situation": ["The player can answer: Who are you?"],
        },
        {
            "key": "dlg.keeper.greet.opt.0.label",
            "en": "Who are you?",
            "kind": "option-label",
            "speaker": "keeper",
            "situation": ["Answers the NPC line: You came. Good."],
        },
        {
            "key": "quest.greet.goal",
            "en": "Meet the Keeper.",
            "kind": "objective",
            "situation": ["What the quest does to the story: The party reaches the gate."],
        },
    ],
}


def inventory():
    return t.parse_inventory(json.loads(json.dumps(INVENTORY_DOC)))


def sent_rows(body):
    """The rows a transcreate request sends: the JSON list after its instruction."""
    return json.loads(body["messages"][1]["content"].split("key -> line:\n")[1])


#: A step whose reply is free text — the critique's shape, reduced to what the
#: transport tests need.
PROSE_STEP = t.Step(
    name="reflect",
    messages=[{"role": "user", "content": "Write your critique."}],
    json_object=False,
)


def write_config(path: Path, body: str) -> Path:
    path.write_text(body, "utf-8")
    return path


CONFIG = """
[i18n]
provider = "openai-compatible"
base_url = "https://api.example-provider.test/v1/"
model = "test-model"
api_key_env = "TEST_I18N_KEY"
"""


# ------------------------------------------------------------------- config --


def test_no_config_files_means_not_configured(tmp_path):
    assert t.load_config(root=tmp_path) is None


def test_config_without_i18n_section_means_not_configured(tmp_path):
    write_config(tmp_path / t.CONFIG_FILE, '[other]\nx = 1\n')
    assert t.load_config(root=tmp_path) is None


def test_local_config_overrides_committed_config(tmp_path):
    write_config(tmp_path / t.CONFIG_FILE, CONFIG)
    write_config(tmp_path / t.LOCAL_CONFIG_FILE, '[i18n]\nmodel = "kimi-k2"\nbatch_size = 5\n')
    cfg = t.load_config(root=tmp_path)
    assert cfg.model == "kimi-k2"
    assert cfg.batch_size == 5
    assert cfg.base_url == "https://api.example-provider.test/v1/"


def test_local_config_alone_is_enough(tmp_path):
    write_config(tmp_path / t.LOCAL_CONFIG_FILE, CONFIG)
    assert t.load_config(root=tmp_path).model == "test-model"


def test_endpoint_is_chat_completions(tmp_path):
    write_config(tmp_path / t.CONFIG_FILE, CONFIG)
    assert t.load_config(root=tmp_path).endpoint == (
        "https://api.example-provider.test/v1/chat/completions"
    )


def test_unsupported_provider_is_an_error_not_a_fallback(tmp_path):
    write_config(tmp_path / t.CONFIG_FILE, CONFIG.replace("openai-compatible", "anthropic"))
    with pytest.raises(t.ConfigError, match="provider"):
        t.load_config(root=tmp_path)


def test_missing_required_key_is_an_error(tmp_path):
    write_config(tmp_path / t.CONFIG_FILE, '[i18n]\nbase_url = "https://x.test/v1"\n')
    with pytest.raises(t.ConfigError, match="model"):
        t.load_config(root=tmp_path)


def test_inline_api_key_is_refused(tmp_path):
    write_config(tmp_path / t.CONFIG_FILE, CONFIG + 'api_key = "sk-whatever"\n')
    with pytest.raises(t.ConfigError, match="never hold an API key"):
        t.load_config(root=tmp_path)


def test_api_key_comes_from_the_named_env_var_only(tmp_path):
    write_config(tmp_path / t.CONFIG_FILE, CONFIG)
    cfg = t.load_config(root=tmp_path)
    assert cfg.api_key(env={}) is None
    assert cfg.api_key(env={"TEST_I18N_KEY": "  "}) is None
    assert cfg.api_key(env={"TEST_I18N_KEY": "secret-value"}) == "secret-value"
    assert "secret-value" not in json.dumps(cfg.__dict__)


# ---------------------------------------------------------------- inventory --


def test_parse_inventory_reads_every_row():
    inv = inventory()
    assert inv.campaign_id == "keep-trial"
    assert inv.declared and inv.sidecar_present
    assert [e.key for e in inv.entries] == [e["key"] for e in INVENTORY_DOC["entries"]]
    assert inv.entries[0].speaker == "keeper"
    assert inv.entries[1].speaker is None


def test_malformed_inventory_is_rejected():
    with pytest.raises(t.TranslateError):
        t.parse_inventory({"campaign_id": "x"})


def test_pending_skips_translated_keys_unless_forced():
    inv = inventory()
    assert [e.key for e in inv.pending()] == [
        "dlg.keeper.greet.text",
        "dlg.keeper.greet.opt.0.label",
        "quest.greet.goal",
    ]
    assert len(inv.pending(force=True)) == len(inv.entries)


def test_glossary_pins_names_only_not_prose():
    inv = inventory()
    assert inv.glossary() == {"The Keeper": "守关人", "The Stone Keep": "石垒要塞"}
    assert [e.key for e in inv.entries if e.is_name] == ["npc.keeper.name", "world.title"]
    # A name this run redoes is not settled, so it pins nothing.
    keeper = [e for e in inv.entries if e.key == "npc.keeper.name"]
    assert inv.glossary(redo=keeper) == {"The Stone Keep": "石垒要塞"}


def test_batches_preserve_order_and_size():
    inv = inventory()
    chunks = t.batches(inv.entries, 2)
    assert [len(c) for c in chunks] == [2, 2, 1]
    assert [e.key for c in chunks for e in c] == [e.key for e in inv.entries]
    with pytest.raises(ValueError):
        t.batches(inv.entries, 0)


# ------------------------------------------------------------------- prompt --


def test_messages_carry_persona_glossary_and_keys():
    inv = inventory()
    batch = [e for e in inv.entries if e.speaker == "keeper" and e.existing is None]
    msgs = t.translate_step(inv, batch, "zh-cn").messages
    system, user = msgs[0]["content"], msgs[1]["content"]
    assert msgs[0]["role"] == "system" and msgs[1]["role"] == "user"
    assert "zh-cn" in system
    assert "clipped, soldierly" in user, "the speaker's speech style must reach the model"
    assert "warm, rambling" not in user, "only speakers in this batch are described"
    assert "守关人" not in user, "the batch's English mentions no glossary name"
    for e in batch:
        assert e.key in user and e.en in user
    goal = [e for e in inv.entries if e.key == "quest.greet.goal"]
    assert "守关人" in t.translate_step(inv, goal, "zh-cn").messages[1]["content"], (
        "a batch is handed the rendering of every name its English mentions"
    )


def test_system_prompt_states_the_player_reply_rule():
    msgs = t.translate_step(inventory(), inventory().entries[:1], "zh-cn").messages
    assert "`option-label`: the PLAYER's own reply" in msgs[0]["content"]
    assert "JSON" in msgs[0]["content"]


# --------------------------------------------------- reflection prompt pass --


def test_translationese_guidance_is_language_scoped():
    assert "翻译腔" in t.translationese_guidance("zh-cn")
    assert t.translationese_guidance("zh-cn") == t.translationese_guidance("ZH-TW")
    assert t.translationese_guidance("zh") == t.translationese_guidance("zh-cn")
    assert t.translationese_guidance("ja") == "", "no checklist beats a wrong checklist"
    assert t.translationese_guidance("de") == ""


def test_zh_system_prompt_carries_the_translationese_checklist():
    system = t.translate_step(inventory(), inventory().entries[:1], "zh-cn").messages[0]["content"]
    for rule in ("的的不休", "名词化", "信达雅"):
        assert rule in system
    assert "的的不休" not in t.translate_step(inventory(), inventory().entries[:1], "ja").messages[0]["content"]


def test_reflection_prompt_names_all_four_critique_axes():
    inv = inventory()
    batch = inv.pending()
    msgs = t.critique_step(inv, batch, "zh-cn", {"dlg.keeper.greet.text": "你来了。"}).messages
    system, user = msgs[0]["content"], msgs[1]["content"]
    for axis in ("ACCURACY", "FLUENCY", "STYLE / REGISTER", "TERMINOLOGY"):
        assert axis in system
    assert "翻译腔" in system, "the zh checklist replaces a generic fluency criterion"
    assert "no change" in system, "an unchanged line must be an expected verdict"
    assert "你来了。" in user, "the critique step sees the draft"
    assert "You came. Good." in user, "and the English beside it"
    assert "clipped, soldierly" in user, "and the persona it must sound like"


def test_option_label_button_budget_reaches_both_prompts():
    """An over-long option label scrolls on its fixed-width button.
    The budget must survive translation, so it is stated in
    the translate step AND checked in the critique step."""
    inv = inventory()
    batch = inv.pending()
    translate = t.translate_step(inv, batch, "zh-cn").messages[0]["content"]
    critique = t.critique_step(inv, batch, "zh-cn", {}).messages[0]["content"]
    for prompt in (translate, critique):
        assert "12 Han" in prompt and "20 Latin" in prompt
        assert "scroll" in prompt.lower()


def test_reflection_step_does_not_ask_for_json():
    step = t.critique_step(inventory(), inventory().pending(), "zh-cn", {})
    system = step.messages[0]["content"]
    assert "only diagnoses" in system
    assert "corrected line" in system
    # The prompt asking for prose and the request asking for a JSON object is the
    # pairing a provider rejects, and asserting only the prompt text is what let
    # `--reflect` ship unrunnable: the reply shape is asserted here too.
    assert step.json_object is False
    assert not step.mentions_json(), "a prose step must not even mention json"


def test_improvement_prompt_carries_critique_draft_and_anti_churn_rule():
    inv = inventory()
    batch = inv.pending()
    draft = {e.key: "草稿:" + e.en for e in batch}
    msgs = t.revise_step(inv, batch, "zh-cn", draft, "  line 3 is too literal  ").messages
    system, user = msgs[0]["content"], msgs[1]["content"]
    assert "BYTE-IDENTICAL" in system, "a reflection pass must not churn good lines"
    assert "ONE JSON object" in system
    assert "line 3 is too literal" in user
    for e in batch:
        assert e.key in user and draft[e.key] in user


def test_require_keys_rejects_a_reply_with_holes():
    chunk = inventory().pending()
    full = {e.key: "x" for e in chunk}
    assert t.require_keys({**full, "bogus": "y"}, chunk, "batch 1") == full
    with pytest.raises(t.TranslateError, match="omitted"):
        t.require_keys({chunk[0].key: "x"}, chunk, "batch 1")


def test_reflect_config_key_defaults_off_and_is_settable(tmp_path):
    write_config(tmp_path / t.CONFIG_FILE, CONFIG)
    assert t.load_config(root=tmp_path).reflect is False
    write_config(tmp_path / t.LOCAL_CONFIG_FILE, "[i18n]\nreflect = true\n")
    assert t.load_config(root=tmp_path).reflect is True


def test_translate_batch_single_pass_makes_one_call(tmp_path, monkeypatch):
    cfg = config(tmp_path)
    inv, calls = inventory(), []

    def poster(url, body, headers, timeout):
        calls.append(body["messages"][0]["content"])
        return {"choices": [{"message": {"content": '{"quest.greet.goal": "去见守关人。"}'}}]}

    monkeypatch.setattr(t, "post_json", poster)
    chunk = [e for e in inv.entries if e.key == "quest.greet.goal"]
    result = t.translate_batch(cfg, inv, chunk, "zh-cn", "secret-value")
    assert result.accepted == {"quest.greet.goal": "去见守关人。"}
    assert result.refused == {}
    assert len(calls) == 1, "a line that passes the fact check costs no second call"


def test_translate_batch_reflect_runs_three_steps_and_keeps_the_revision(tmp_path, monkeypatch):
    cfg = config(tmp_path)
    inv, seen = inventory(), []
    replies = [
        '{"quest.greet.goal": "去和守关人进行对话。"}',  # draft: 弱动词 进行
        "quest.greet.goal: 进行对话 is a weak-verb construction; say 对话.",
        '{"quest.greet.goal": "去和守关人对话。"}',
    ]

    def poster(url, body, headers, timeout):
        seen.append(body["messages"][0]["content"])
        return {"choices": [{"message": {"content": replies[len(seen) - 1]}}]}

    monkeypatch.setattr(t, "post_json", poster)
    chunk = [e for e in inv.entries if e.key == "quest.greet.goal"]
    out = t.translate_batch(cfg, inv, chunk, "zh-cn", "secret-value", reflect=True)

    assert out.accepted == {"quest.greet.goal": "去和守关人对话。"}
    assert len(seen) == 3, "translate -> reflect -> improve"
    assert "You\ntranscreate" in seen[0] or "transcreate" in seen[0]
    assert "senior localization editor" in seen[1]
    assert "revising your own draft" in seen[2]


def test_reflect_run_that_drops_a_key_fails_instead_of_writing_a_hole(tmp_path, monkeypatch):
    cfg = config(tmp_path)
    inv = inventory()
    replies = ['{"quest.greet.goal": "去见守关人。"}', "no change", "{}"]
    seen = []

    def poster(url, body, headers, timeout):
        seen.append(1)
        return {"choices": [{"message": {"content": replies[len(seen) - 1]}}]}

    monkeypatch.setattr(t, "post_json", poster)
    chunk = [e for e in inv.entries if e.key == "quest.greet.goal"]
    with pytest.raises(t.TranslateError, match="improve"):
        t.translate_batch(cfg, inv, chunk, "zh-cn", "secret-value", reflect=True)


# ------------------------------------------------------------------ request --


def test_request_shape_and_key_placement(tmp_path):
    write_config(tmp_path / t.CONFIG_FILE, CONFIG)
    cfg = t.load_config(root=tmp_path)
    step = t.translate_step(inventory(), inventory().entries[:2], "zh-cn")
    url, body, headers = t.build_request(cfg, step, "secret-value")

    assert url == "https://api.example-provider.test/v1/chat/completions"
    assert body["model"] == "test-model"
    assert body["temperature"] == pytest.approx(0.2)
    assert body["stream"] is False
    assert body["messages"] == list(step.messages)
    assert headers["Authorization"] == "Bearer secret-value"
    assert "secret-value" not in json.dumps(body), "the key belongs in the header only"


def three_steps():
    """The three steps of one `--reflect` batch, in order."""
    inv = inventory()
    batch = inv.pending()
    draft = {e.key: "草稿" for e in batch}
    return [
        t.translate_step(inv, batch, "zh-cn"),
        t.critique_step(inv, batch, "zh-cn", draft),
        t.revise_step(inv, batch, "zh-cn", draft, "line 1 is too literal"),
    ]


def test_response_format_is_per_step_not_per_request(tmp_path):
    """`--reflect` could never run: `response_format: json_object` was set once
    for all three steps, and the critique prompt deliberately never says `json`.
    OpenAI and DeepSeek both answer that pairing with `HTTP 400 "Prompt must
    contain the word 'json' in some form"`, so the run died on batch 1 before it
    had written a translation. Two of the three steps want a JSON object back and
    one wants prose, so the shape belongs to the step."""
    cfg = config(tmp_path)
    translate, critique, revise = three_steps()
    assert [s.name for s in (translate, critique, revise)] == [
        "translate",
        "reflect",
        "improve",
    ]
    for step in (translate, revise):
        body = t.build_request(cfg, step, "secret-value")[1]
        assert body["response_format"] == {"type": "json_object"}, step.name
        assert step.mentions_json(), f"`{step.name}` asks for JSON and says so"
    body = t.build_request(cfg, critique, "secret-value")[1]
    assert "response_format" not in body, (
        "the critique asks for free text; sending json_object is the HTTP 400"
    )


def test_a_json_object_step_whose_prompt_never_says_json_is_refused(tmp_path):
    """The check that makes the flag and the prompt one thing rather than two
    settings: a mismatch is refused here, locally, instead of being discovered
    as a provider's 400 halfway through a paid run."""
    cfg = config(tmp_path)
    bad = t.Step(
        name="critique-as-json",
        messages=[{"role": "system", "content": "Write your critique."}],
        json_object=True,
    )
    with pytest.raises(t.TranslateError, match="json"):
        t.build_request(cfg, bad, "secret-value")


def test_a_provider_rejection_carries_the_providers_own_reason(tmp_path):
    """A bare `HTTP 400` is what kept this defect unread: the sentence naming the
    cause was in the response body and nothing printed it."""
    cfg = config(tmp_path)
    body = (
        b'{"error":{"message":"Prompt must contain the word \'json\' in some form to use '
        b'\'response_format\' of type \'json_object\'.","type":"invalid_request_error"}}'
    )

    def poster(url, req, headers, timeout):
        raise urllib.error.HTTPError(url, 400, "Bad Request", {}, io.BytesIO(body))

    with pytest.raises(t.TranslateError) as exc:
        t.chat_once(cfg, PROSE_STEP, "secret-value", poster=poster, sleep=lambda _: None)
    assert "HTTP 400" in str(exc.value)
    assert "must contain the word" in str(exc.value)
    assert "`reflect`" in str(exc.value), "which step was refused"
    assert "secret-value" not in str(exc.value)


def test_a_key_echoed_back_by_a_provider_is_redacted(tmp_path):
    cfg = config(tmp_path)

    def poster(url, req, headers, timeout):
        raise urllib.error.HTTPError(
            url, 401, "Unauthorized", {}, io.BytesIO(b'{"error":"key secret-value is revoked"}')
        )

    with pytest.raises(t.TranslateError) as exc:
        t.chat_once(cfg, PROSE_STEP, "secret-value", poster=poster, sleep=lambda _: None)
    assert "secret-value" not in str(exc.value)
    assert "<redacted>" in str(exc.value)


# -------------------------------------------------------------------- reply --


@pytest.mark.parametrize(
    "reply",
    [
        '{"a": "甲", "b": "乙"}',
        '```json\n{"a": "甲", "b": "乙"}\n```',
        'Sure! Here you go:\n{"a": "甲", "b": "乙"}\nHope that helps.',
    ],
)
def test_translations_parse_through_fences_and_prose(reply):
    assert t.parse_translations(reply) == {"a": "甲", "b": "乙"}


@pytest.mark.parametrize("reply", ["not json at all", '["a"]', '{"a": 3}'])
def test_unusable_replies_are_errors(reply):
    with pytest.raises(t.TranslateError):
        t.parse_translations(reply)


# ---------------------------------------------------------------- transport --


def config(tmp_path):
    write_config(tmp_path / t.CONFIG_FILE, CONFIG)
    return t.load_config(root=tmp_path)


def test_chat_retries_transient_failures(tmp_path):
    cfg = config(tmp_path)
    calls = []

    def poster(url, body, headers, timeout):
        calls.append(url)
        if len(calls) == 1:
            raise urllib.error.URLError("connection reset")
        return {"choices": [{"message": {"content": '{"k": "v"}'}}]}

    out = t.chat_once(cfg, PROSE_STEP, "secret-value", poster=poster, sleep=lambda _: None)
    assert out == '{"k": "v"}'
    assert len(calls) == 2


def test_auth_failure_fails_fast_without_leaking_the_key(tmp_path):
    cfg = config(tmp_path)
    calls = []

    def poster(url, body, headers, timeout):
        calls.append(url)
        raise urllib.error.HTTPError(url, 401, "Unauthorized", {}, None)

    with pytest.raises(t.TranslateError) as exc:
        t.chat_once(cfg, PROSE_STEP, "secret-value", poster=poster, sleep=lambda _: None)
    assert len(calls) == 1, "a bad key must not be retried"
    assert "secret-value" not in str(exc.value)


# ------------------------------------------------------------------ sidecar --


def test_merge_keeps_existing_applies_new_and_cannot_produce_orphans():
    inv = inventory()
    merged = t.merge_content(inv, {"dlg.keeper.greet.text": "你来了。", "bogus.key": "x"})
    assert merged["npc.keeper.name"] == "守关人"
    assert merged["dlg.keeper.greet.text"] == "你来了。"
    assert "bogus.key" not in merged
    assert set(merged) <= {e.key for e in inv.entries}


#: A `delvec` that exits 0 and touches nothing. Canonical form is the compiler's
#: to decide and is proven against the real binary over a real written file in
#: `crates/delvec/tests/i18n_sidecar.rs`; what these tests own is the document
#: the writer hands it and the fact that it hands it over at all.
INERT_DELVEC = ["true"]


def test_sidecar_envelope_carries_the_campaign_and_the_inventory_version(tmp_path):
    inv = inventory()
    path = t.sidecar_path(tmp_path, "zh-cn")
    path.parent.mkdir()
    path.write_text(json.dumps({"dsl_version": "0.3.0", "content": {}}), "utf-8")

    t.write_sidecar(path, inv, {"b.key": "乙", "a.key": "甲"}, INERT_DELVEC)
    raw = path.read_text("utf-8")
    doc = json.loads(raw)
    assert doc["dsl_version"] == "0.6.0", (
        "an old version claim is not carried forward: `delvec fmt` stamps the version "
        "this engine implements (ADR-0024), so preserving one wrote a document the "
        "formatter refuses"
    )
    assert doc["campaign_id"] == "keep-trial"
    assert doc["kind"] == "l10n"
    assert doc["lang"] == "zh-cn"
    assert list(doc["content"]) == ["a.key", "b.key"]
    assert "甲" in raw, "translations stay human-readable, not \\u escapes"
    assert raw.endswith("\n")


def test_fresh_sidecar_takes_the_campaign_dsl_version(tmp_path):
    path = t.sidecar_path(tmp_path, "zh-cn")
    t.write_sidecar(path, inventory(), {"a.key": "甲"}, INERT_DELVEC)
    assert json.loads(path.read_text("utf-8"))["dsl_version"] == "0.6.0"


def test_the_written_sidecar_is_handed_to_the_formatter(tmp_path, monkeypatch):
    """One authority for canonical form: the file the tool writes goes through
    `delvec fmt`, never through a second layout implementation living here."""
    path = t.sidecar_path(tmp_path, "zh-cn")
    seen = []

    def fake_run(args, delvec):
        seen.append(list(delvec) + list(args))
        return subprocess.CompletedProcess(list(args), 0, "", "")

    monkeypatch.setattr(t, "run_delvec", fake_run)
    t.write_sidecar(path, inventory(), {"a.key": "甲"}, ["delvec"])
    assert seen == [["delvec", "fmt", str(path)]]


def test_a_formatter_refusal_fails_the_write(tmp_path, monkeypatch):
    """A sidecar the formatter will not accept fails the run rather than being
    written and forgotten: `DW0773` is an error tier, and non-canonical sidecars
    shipped for exactly as long as nobody was told."""
    path = t.sidecar_path(tmp_path, "zh-cn")

    def fake_run(args, delvec):
        return subprocess.CompletedProcess(
            list(args), 1, "DW0773 [error] fmt: not in canonical form\n", ""
        )

    monkeypatch.setattr(t, "run_delvec", fake_run)
    with pytest.raises(t.TranslateError, match="DW0773"):
        t.write_sidecar(path, inventory(), {"a.key": "甲"}, ["delvec"])


# --------------------------------------------------------------------- main --


def _no_network(monkeypatch):
    def boom(*a, **k):
        raise AssertionError("no HTTP request may be made")

    monkeypatch.setattr(t, "post_json", boom)


def test_dry_run_prints_the_prompt_and_calls_nothing(tmp_path, monkeypatch, capsys):
    _no_network(monkeypatch)
    monkeypatch.setattr(t, "fetch_inventory", lambda *a, **k: inventory())
    monkeypatch.delenv("TEST_I18N_KEY", raising=False)
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)

    rc = t.main([str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path), "--dry-run"])
    out = capsys.readouterr().out
    assert rc == 0
    assert "3 to transcreate" in out
    assert "dlg.keeper.greet.text" in out
    assert "clipped, soldierly" in out
    assert "TEST_I18N_KEY" in out, "dry run names the env var it would read"


def test_missing_env_var_is_a_clean_refusal(tmp_path, monkeypatch, capsys):
    _no_network(monkeypatch)
    monkeypatch.setattr(t, "fetch_inventory", lambda *a, **k: inventory())
    monkeypatch.delenv("TEST_I18N_KEY", raising=False)
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)

    rc = t.main([str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path)])
    assert rc == 2
    assert "TEST_I18N_KEY" in capsys.readouterr().err


def test_unconfigured_run_exits_without_translating(tmp_path, monkeypatch, capsys):
    _no_network(monkeypatch)
    cfg_path = write_config(tmp_path / "cfg.toml", "[other]\nx = 1\n")
    rc = t.main([str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path)])
    assert rc == 2
    assert "not configured" in capsys.readouterr().err


def test_undeclared_language_is_refused(tmp_path, monkeypatch, capsys):
    _no_network(monkeypatch)
    doc = json.loads(json.dumps(INVENTORY_DOC))
    doc["declared"] = False
    monkeypatch.setattr(t, "fetch_inventory", lambda *a, **k: t.parse_inventory(doc))
    monkeypatch.setenv("TEST_I18N_KEY", "secret-value")
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)

    rc = t.main([str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path)])
    assert rc == 1
    assert "world.json" in capsys.readouterr().err


def test_full_run_writes_only_missing_keys_then_validates(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(t, "fetch_inventory", lambda *a, **k: inventory())
    monkeypatch.setenv("TEST_I18N_KEY", "secret-value")
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)
    seen = []

    def poster(url, body, headers, timeout):
        sent = sent_rows(body)
        seen.append([i["key"] for i in sent])
        reply = {i["key"]: "译:" + i["en"].replace("the Keeper", "守关人") for i in sent}
        return {"choices": [{"message": {"content": json.dumps(reply, ensure_ascii=False)}}]}

    monkeypatch.setattr(t, "post_json", poster)
    validated = []

    def fake_delvec(args, delvec):
        validated.append(list(args))
        return subprocess.CompletedProcess(list(args), 0, "", "")

    monkeypatch.setattr(t, "run_delvec", fake_delvec)

    rc = t.main(
        [str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path), "--batch-size", "2"]
    )
    assert rc == 0
    assert seen == [
        ["dlg.keeper.greet.text", "dlg.keeper.greet.opt.0.label"],
        ["quest.greet.goal"],
    ], "only untranslated keys are sent, in inventory order, batched"
    assert [a[0] for a in validated] == ["fmt", "validate"], (
        "a run formats what it wrote before it validates it — a sidecar that is not "
        "canonical is a `DW0773` waiting in CI"
    )

    content = json.loads(t.sidecar_path(tmp_path, "zh-cn").read_text("utf-8"))["content"]
    assert content["npc.keeper.name"] == "守关人", "existing translation untouched"
    assert content["dlg.keeper.greet.text"] == "译:You came. Good."
    assert set(content) == {e.key for e in inventory().entries}
    assert "coverage: `delvec validate` passed" in capsys.readouterr().out


def test_dry_run_shows_all_three_steps_only_when_reflecting(tmp_path, monkeypatch, capsys):
    _no_network(monkeypatch)
    monkeypatch.setattr(t, "fetch_inventory", lambda *a, **k: inventory())
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)
    argv = [str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path), "--dry-run"]

    assert t.main(argv) == 0
    plain = capsys.readouterr().out
    assert "reflect=False" in plain
    assert "step: reflect" not in plain

    assert t.main([*argv, "--reflect"]) == 0
    out = capsys.readouterr().out
    assert "reflect=True" in out
    assert "step: reflect" in out and "step: improve" in out
    assert "senior localization editor" in out, "the critique prompt is reviewable dry"
    assert "<step-1 draft, filled at call time>" in out


def test_reflect_and_no_reflect_together_are_refused(tmp_path, monkeypatch, capsys):
    _no_network(monkeypatch)
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)
    rc = t.main(
        [str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path), "--reflect", "--no-reflect"]
    )
    assert rc == 2
    assert "mutually exclusive" in capsys.readouterr().err


def test_no_reflect_overrides_the_config(tmp_path, monkeypatch, capsys):
    _no_network(monkeypatch)
    monkeypatch.setattr(t, "fetch_inventory", lambda *a, **k: inventory())
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG + "reflect = true\n")
    rc = t.main(
        [str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path), "--dry-run", "--no-reflect"]
    )
    assert rc == 0
    assert "reflect=False" in capsys.readouterr().out


def test_full_reflect_run_writes_the_revised_text(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(t, "fetch_inventory", lambda *a, **k: inventory())
    monkeypatch.setenv("TEST_I18N_KEY", "secret-value")
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)
    steps = []

    def poster(url, body, headers, timeout):
        system = body["messages"][0]["content"]
        if "senior localization editor" in system:
            steps.append("reflect")
            return {"choices": [{"message": {"content": "tighten dlg.keeper.greet.text"}}]}
        keys = [e.key for e in inventory().pending()]
        stage = "improve" if "revising your own draft" in system else "translate"
        steps.append(stage)
        prefix = "终:" if stage == "improve" else "初:"
        # Every line names the Keeper's rendering, so the fact check passes it.
        reply = {k: prefix + k + " 守关人" for k in keys}
        return {"choices": [{"message": {"content": json.dumps(reply, ensure_ascii=False)}}]}

    monkeypatch.setattr(t, "post_json", poster)
    monkeypatch.setattr(
        t,
        "run_delvec",
        lambda args, delvec: __import__("subprocess").CompletedProcess(args, 0, "", ""),
    )

    rc = t.main([str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path), "--reflect"])
    assert rc == 0
    assert steps == ["translate", "reflect", "improve"]

    content = json.loads(t.sidecar_path(tmp_path, "zh-cn").read_text("utf-8"))["content"]
    assert content["dlg.keeper.greet.text"] == "终:dlg.keeper.greet.text 守关人", "the revision ships"
    assert content["npc.keeper.name"] == "守关人", "existing translations are still untouched"
    assert "translate -> reflect -> improve" in capsys.readouterr().out


def test_incomplete_reply_fails_loudly(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(t, "fetch_inventory", lambda *a, **k: inventory())
    monkeypatch.setenv("TEST_I18N_KEY", "secret-value")
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)
    monkeypatch.setattr(
        t,
        "post_json",
        lambda *a, **k: {"choices": [{"message": {"content": '{"quest.greet.goal": "去"}'}}]},
    )
    rc = t.main([str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path)])
    assert rc == 1
    assert "omitted" in capsys.readouterr().err
    assert not t.sidecar_path(tmp_path, "zh-cn").exists(), "no partial sidecar is written"


def test_write_sidecar_records_what_each_row_was_translated_from(tmp_path):
    """`source` is what lets the compiler DETECT a stale translation (DW0187):
    coverage compares key sets, and rewriting an authored line moves no key."""
    inv = t.Inventory(
        campaign_id="demo",
        dsl_version="0.9.0",
        lang="zh-cn",
        declared=True,
        sidecar_present=False,
        world_title="Trial of the Keep",
        entries=[
            t.Entry(key="world.title", en="Trial of the Keep"),
            t.Entry(key="npc.keeper.name", en="The Keeper"),
        ],
    )
    path = tmp_path / "l10n" / "zh-cn.json"
    t.write_sidecar(
        path, inv, {"world.title": "\u8981\u585e\u7684\u8bd5\u70bc"}, INERT_DELVEC
    )
    doc = json.loads(path.read_text("utf-8"))

    # Exactly the rows `content` carries — a row with no translation records no
    # provenance, so the two maps cannot disagree about what was translated.
    assert doc["source"] == {"world.title": "Trial of the Keep"}
    assert set(doc["source"]) == set(doc["content"])


def test_rerunning_over_an_old_sidecar_adopts_provenance_without_retranslating(tmp_path):
    """Adoption is a re-run: every row already present is recorded against the
    English the inventory holds, and no translation changes."""
    inv = t.Inventory(
        campaign_id="demo",
        dsl_version="0.9.0",
        lang="zh-cn",
        declared=True,
        sidecar_present=True,
        world_title="Trial of the Keep",
        entries=[t.Entry(key="world.title", en="Trial of the Keep", existing="\u8981\u585e")],
    )
    path = tmp_path / "l10n" / "zh-cn.json"
    path.parent.mkdir(parents=True)
    path.write_text(
        json.dumps(
            {
                "dsl_version": "0.9.0",
                "campaign_id": "demo",
                "kind": "l10n",
                "lang": "zh-cn",
                "content": {"world.title": "\u8981\u585e"},
            }
        ),
        "utf-8",
    )
    t.write_sidecar(path, inv, t.merge_content(inv, {}), INERT_DELVEC)
    doc = json.loads(path.read_text("utf-8"))
    assert doc["content"] == {"world.title": "\u8981\u585e"}, "no retranslation"
    assert doc["source"] == {"world.title": "Trial of the Keep"}, "provenance adopted"


# ----------------------------------------------------------- delvec resolution


def _fake_delvec(tmp_path, version):
    path = tmp_path / "delvec"
    path.write_text(f"#!/bin/sh\necho 'delvec {version}, dsl 0.0.0, mc 1.21.11'\n")
    path.chmod(0o755)
    return path


def test_delvec_on_path_is_used_when_it_answers_the_pinned_version(tmp_path, monkeypatch):
    sys.path.insert(0, str(TOOL.parents[1] / "lib"))
    import versions

    fake = _fake_delvec(tmp_path, versions.engine_version())
    monkeypatch.delenv("DELVEC", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    assert t.delvec_command(None) == [str(fake)]


def test_no_delvec_of_the_pinned_version_is_a_named_failure_never_a_cargo_run(
    tmp_path, monkeypatch
):
    _fake_delvec(tmp_path, "0.0.0-not-the-pin")
    monkeypatch.delenv("DELVEC", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    monkeypatch.setattr(t, "REPO_ROOT", tmp_path / "no-tree")
    with pytest.raises(t.NoDelvec, match="0.0.0-not-the-pin"):
        t.delvec_command(None)


# ------------------------------------------------------------ transcreation --


def test_every_row_sent_carries_its_intent():
    """Transcreation writes from intent: each row the model sees carries the kind
    of text, the speaker and the situation the inventory derived — not just the
    English."""
    inv = inventory()
    rows = sent_rows({"messages": t.translate_step(inv, inv.pending(), "zh-cn").messages})
    by_key = {r["key"]: r for r in rows}
    assert by_key["dlg.keeper.greet.text"]["kind"] == "dialogue"
    assert by_key["dlg.keeper.greet.text"]["speaker"] == "keeper"
    assert by_key["dlg.keeper.greet.opt.0.label"]["kind"] == "option-label"
    assert by_key["dlg.keeper.greet.opt.0.label"]["situation"] == [
        "Answers the NPC line: You came. Good."
    ]
    assert by_key["quest.greet.goal"]["kind"] == "objective"
    assert "speaker" not in by_key["quest.greet.goal"], "absent intent is omitted, not null"
    assert by_key["quest.greet.goal"]["situation"][0].startswith("What the quest does")


def test_the_critique_and_revision_see_the_intent_too():
    inv = inventory()
    batch = inv.pending()
    for step in (
        t.critique_step(inv, batch, "zh-cn", {}),
        t.revise_step(inv, batch, "zh-cn", {}, "fine"),
    ):
        user = step.messages[1]["content"]
        assert '"kind": "option-label"' in user
        assert "Answers the NPC line: You came. Good." in user


def test_writing_rules_come_from_game_writing_md():
    """One source: the zh rules in the prompt are the `## 4. Chinese` section of
    the writing guide, read at call time, each rule cut before its provenance."""
    rules = t.writing_rules("zh-cn")
    doc = t.GAME_WRITING.read_text("utf-8")
    for n in range(1, 6):
        assert f"**C{n} — " in rules and f"**C{n} — " in doc
    assert "Cited" not in rules and "Authored" not in rules
    assert "N4 — " not in rules, "only the target language's section"
    system = t.translate_step(inventory(), inventory().pending(), "zh-cn").messages[0]["content"]
    assert rules in system
    assert t.writing_rules("ja") == "", "a language with no section gets the general rules"


def test_writing_rules_follow_the_file_and_refuse_when_the_section_moves(tmp_path):
    guide = tmp_path / "game-writing.md"
    guide.write_text(
        "# x\n\n## 4. Chinese\n\n- **C1 — Say it plainly.** Body text.\n  **Cited:** a source.\n"
        "- **C2 — Second rule.** More. **Authored.**\n\n## 5. Next\n\n- **N — not this.**\n",
        "utf-8",
    )
    rules = t.writing_rules("zh-cn", guide)
    assert "**C1 — Say it plainly.** Body text." in rules
    assert "**C2 — Second rule.** More." in rules
    assert "a source" not in rules and "not this" not in rules
    guide.write_text("# x\n\n## 4. Japanese\n\n- **J1 — x.**\n", "utf-8")
    with pytest.raises(t.TranslateError, match="Chinese"):
        t.writing_rules("zh-cn", guide)


def test_names_are_sent_first_in_batches_of_their_own():
    inv = inventory()
    chunks = t.plan_batches(inv.pending(force=True), 3)
    assert [[e.key for e in c] for c in chunks] == [
        ["npc.keeper.name", "world.title"],
        ["dlg.keeper.greet.text", "dlg.keeper.greet.opt.0.label", "quest.greet.goal"],
    ]


def _stale_inventory():
    doc = json.loads(json.dumps(INVENTORY_DOC))
    doc["entries"][2]["existing"] = "旧的译文"
    doc["entries"][2]["stale"] = True
    return t.parse_inventory(doc)


def test_a_stale_row_is_sent_again_and_dropped_if_not_redone():
    """A translation made from English the line no longer reads is wrong: it is
    redone, and if this run does not redo it, it is dropped rather than kept
    under a `source` that would claim it matches (DW0187)."""
    inv = _stale_inventory()
    assert "dlg.keeper.greet.text" in [e.key for e in inv.pending()]
    content = t.merge_content(inv, {})
    assert "dlg.keeper.greet.text" not in content
    assert content["npc.keeper.name"] == "守关人"
    assert t.merge_content(inv, {"dlg.keeper.greet.text": "新"})["dlg.keeper.greet.text"] == "新"


# ---------------------------------------------------------------- fact check --

NAMES = {"The Keeper": "守关人", "The Stone Keep": "石垒要塞"}


def _row(key, en, kind="dialogue"):
    return t.Entry(key=key, en=en, kind=kind)


def test_fact_check_passes_a_faithful_line():
    row = _row("dlg.a.b.text", "The Keeper waited 40 days at the Stone Keep, %s.")
    assert t.check_row(row, "守关人在石垒要塞等了40天，%s。", NAMES, "zh-cn") == []


def test_fact_check_refuses_a_name_rendered_another_way():
    row = _row("dlg.a.b.text", "Ask the Keeper.")
    failures = t.check_row(row, "去问守门人。", NAMES, "zh-cn")
    assert failures == ["the name `The Keeper` must be written `守关人`"]


def test_fact_check_refuses_a_lost_placeholder():
    row = _row("obj.q.o.hint", "%s has the key.", kind="objective")
    failures = t.check_row(row, "钥匙在他手上。", NAMES, "zh-cn")
    assert any("placeholders" in f for f in failures), failures


SPANNED = "The ledger is kept by [[obfuscated|someone else]] at [[italic,color=dark_purple|night]]."


def test_fact_check_passes_a_translation_that_moves_a_span():
    row = _row("cast.q.n.0.bark.2", SPANNED, kind="bark")
    zh = "[[italic,color=dark_purple|入夜]]以后，账本由[[obfuscated|别的什么人]]记。"
    assert t.check_row(row, zh, NAMES, "zh-cn") == []


@pytest.mark.parametrize(
    "zh",
    [
        "[[italic,color=dark_purple|入夜]]以后，账本由别的什么人记。",  # opener and closer lost
        "[[italic,color=dark_purple|入夜]]以后，账本由[[bold|别的什么人]]记。",  # restyled
        "[[italic,color=dark_purple|入夜]]以后，账本由[[obfuscated|别的什么人记。",  # unclosed
    ],
)
def test_fact_check_refuses_a_translation_that_loses_a_span(zh):
    row = _row("cast.q.n.0.bark.2", SPANNED, kind="bark")
    failures = t.check_row(row, zh, NAMES, "zh-cn")
    assert any("span markers" in f for f in failures), failures


def test_every_prompt_tells_the_writer_to_keep_spans():
    for prompt in (t.SYSTEM_PROMPT, t.FIX_PROMPT, t.IMPROVEMENT_PROMPT):
        assert "span" in prompt


@pytest.mark.parametrize(
    ("en", "zh", "lost"),
    [
        ("Montrose quartered here in 1645.", "蒙特罗斯1646年在这里驻扎。", "1645"),
        ("Sixteen arrows.", "十五支箭。", "16"),
        ("Forty strokes cut into the lectern.", "讲台上刻着五十道。", "40"),
    ],
)
def test_fact_check_refuses_a_changed_number(en, zh, lost):
    failures = t.check_row(_row("fx.q.done.0.narrate", en, "narration"), zh, NAMES, "zh-cn")
    assert any(lost in f for f in failures), failures


@pytest.mark.parametrize(
    ("en", "zh"),
    [
        ("Day 10,811. Rang the Vesper.", "第10811天。敲响了晚祷钟。"),
        ("Sixteen arrows.", "十六支箭。"),
        ("Forty-two steps.", "四十二级台阶。"),
        ("Two of you, then.", "那就你们俩。"),
        ("A hundred years.", "一百年。"),
        ("No one came back.", "没人回来。"),
    ],
)
def test_number_words_accept_the_chinese_forms(en, zh):
    assert t.check_row(_row("fx.q.done.0.narrate", en, "narration"), zh, NAMES, "zh-cn") == []


def test_a_name_inside_a_longer_name_is_held_to_the_longer_one():
    names = {"The Warden": "守钟人", "The Warden's Door": "北门"}
    row = _row("obj.q.o.title", "Open the Warden's Door", "objective")
    assert t.check_row(row, "打开北门", names, "zh-cn") == []
    assert t.check_row(_row("x", "The Warden is gone."), "守钟人走了。", names, "zh-cn") == []


def test_one_name_one_rendering_and_no_two_names_share_one():
    batch = [
        t.Entry(key="npc.porter.name", en="The Porter", kind="name"),
        t.Entry(key="actor.gw.name", en="Gate-warden", kind="name"),
        t.Entry(key="npc.keeper.name", en="The Keeper", kind="name"),
    ]
    got = {"npc.porter.name": "门卫", "actor.gw.name": "门卫", "npc.keeper.name": "看门人"}
    failures = t.check_names(batch, got, {"The Keeper": "守关人"})
    assert set(failures) == {"actor.gw.name", "npc.keeper.name"}
    assert "already names `The Porter`" in failures["actor.gw.name"][0]
    assert "already written `守关人`" in failures["npc.keeper.name"][0]


def test_a_refused_row_is_sent_back_once_then_refused_by_name(tmp_path, monkeypatch):
    cfg = config(tmp_path)
    inv = inventory()
    steps = []

    def poster(url, body, headers, timeout):
        system = body["messages"][0]["content"]
        steps.append("fix" if "mechanical check refused" in system else "translate")
        if steps[-1] == "fix":
            assert "must be written `守关人`" in body["messages"][1]["content"], (
                "the corrective step names the failure"
            )
        return {"choices": [{"message": {"content": '{"quest.greet.goal": "去见守门人。"}'}}]}

    monkeypatch.setattr(t, "post_json", poster)
    chunk = [e for e in inv.entries if e.key == "quest.greet.goal"]
    result = t.translate_batch(cfg, inv, chunk, "zh-cn", "secret-value")
    assert steps == ["translate", "fix"]
    assert result.accepted == {}
    assert list(result.refused) == ["quest.greet.goal"]


def test_a_corrected_row_is_accepted(tmp_path, monkeypatch):
    cfg = config(tmp_path)
    inv = inventory()
    replies = ['{"quest.greet.goal": "去见守门人。"}', '{"quest.greet.goal": "去见守关人。"}']

    def poster(url, body, headers, timeout):
        return {"choices": [{"message": {"content": replies.pop(0)}}]}

    monkeypatch.setattr(t, "post_json", poster)
    chunk = [e for e in inv.entries if e.key == "quest.greet.goal"]
    result = t.translate_batch(cfg, inv, chunk, "zh-cn", "secret-value")
    assert result.accepted == {"quest.greet.goal": "去见守关人。"}
    assert result.fixed == ["quest.greet.goal"] and result.refused == {}


def test_a_run_with_a_refused_row_leaves_it_out_and_fails(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(t, "fetch_inventory", lambda *a, **k: inventory())
    monkeypatch.setenv("TEST_I18N_KEY", "secret-value")
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)

    def poster(url, body, headers, timeout):
        if "mechanical check refused" in body["messages"][0]["content"]:
            return {"choices": [{"message": {"content": '{"quest.greet.goal": "去见守门人。"}'}}]}
        reply = {i["key"]: "译" for i in sent_rows(body)}
        reply["quest.greet.goal"] = "去见守门人。"
        return {"choices": [{"message": {"content": json.dumps(reply, ensure_ascii=False)}}]}

    monkeypatch.setattr(t, "post_json", poster)
    monkeypatch.setattr(
        t, "run_delvec", lambda args, delvec: subprocess.CompletedProcess(args, 0, "", "")
    )
    rc = t.main([str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path)])
    out = capsys.readouterr().out
    assert rc == 1, "a refused row fails the run even when validate is satisfied"
    assert "REFUSED quest.greet.goal" in out
    assert "the name `The Keeper` must be written `守关人`" in out
    assert "1 refused" in out
    content = json.loads(t.sidecar_path(tmp_path, "zh-cn").read_text("utf-8"))["content"]
    assert "quest.greet.goal" not in content, "a refused line never lands"
    assert content["dlg.keeper.greet.text"] == "译"


def _content_clone_run(tmp_path, monkeypatch, extra=()):
    """A creator's content clone: the engine's `campaigns/` symlink is not there, so
    `delvec`'s own default prefab root resolves to nothing. The fake `delvec` fails
    any call that names no existing prefab library, as the real one does."""
    monkeypatch.setattr(t, "fetch_inventory", lambda *a, **k: inventory())
    monkeypatch.setenv("TEST_I18N_KEY", "secret-value")
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)

    def poster(url, body, headers, timeout):
        reply = {i["key"]: "译:" + i["en"].replace("the Keeper", "守关人") for i in sent_rows(body)}
        return {"choices": [{"message": {"content": json.dumps(reply, ensure_ascii=False)}}]}

    monkeypatch.setattr(t, "post_json", poster)
    calls = []

    def fake_run(cmd, *a, **k):
        calls.append(list(cmd))
        argv = list(cmd)
        root = argv[argv.index("--prefabs") + 1] if "--prefabs" in argv else "campaigns/prefabs"
        ok = os.path.isdir(root) and root != "campaigns/prefabs"
        return subprocess.CompletedProcess(argv, 0 if ok else 1, "", "" if ok else "no prefabs")

    monkeypatch.setattr(t.subprocess, "run", fake_run)
    argv = [str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path), "--batch-size", "2", *extra]
    return t.main(argv), calls


def test_closing_validate_reads_the_creators_prefab_library(tmp_path, monkeypatch):
    rc, calls = _content_clone_run(tmp_path, monkeypatch)
    root = os.environ["DELVEWRIGHT_PREFABS"]
    validate = [c for c in calls if "validate" in c]
    assert rc == 0
    assert validate and validate[0][validate[0].index("--prefabs") + 1] == root


def test_prefabs_flag_outranks_the_env(tmp_path, monkeypatch):
    other = tmp_path / "other-prefabs"
    other.mkdir()
    rc, calls = _content_clone_run(tmp_path, monkeypatch, ["--prefabs", str(other)])
    assert rc == 0
    assert all(c[c.index("--prefabs") + 1] == str(other) for c in calls)


def test_missing_prefab_root_is_refused_naming_the_fix(tmp_path, monkeypatch, capsys):
    _no_network(monkeypatch)
    monkeypatch.delenv("DELVEWRIGHT_PREFABS")
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)
    monkeypatch.setenv("TEST_I18N_KEY", "secret-value")
    rc = t.main([str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path)])
    err = capsys.readouterr().err
    assert rc == 2
    assert "--prefabs" in err and "DELVEWRIGHT_PREFABS" in err and "env.sh" in err


def test_prefab_root_that_is_not_a_directory_is_refused(tmp_path, monkeypatch, capsys):
    _no_network(monkeypatch)
    cfg_path = write_config(tmp_path / "cfg.toml", CONFIG)
    monkeypatch.setenv("TEST_I18N_KEY", "secret-value")
    rc = t.main(
        [str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path), "--prefabs", str(tmp_path / "nope")]
    )
    assert rc == 2
    assert "not a directory" in capsys.readouterr().err
