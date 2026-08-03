"""Unit tests for tools/i18n-translate.py (external-LLM l10n translation).

No test may touch the network: the HTTP poster is always injected or monkeypatched
to explode. What is proven here is the request we *would* send, the config
resolution (including the fallback rule), inventory parsing, reply parsing and the
sidecar we write.
"""

import importlib.util
import json
import sys
import urllib.error
from pathlib import Path

import pytest

TOOL = Path(__file__).resolve().parents[1] / "i18n-translate.py"


def _load():
    spec = importlib.util.spec_from_file_location("i18n_translate", TOOL)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["i18n_translate"] = mod
    spec.loader.exec_module(mod)
    return mod


t = _load()

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
        {"key": "npc.keeper.name", "en": "The Keeper", "speaker": "keeper", "existing": "守关人"},
        {"key": "world.title", "en": "The Stone Keep", "existing": "石垒要塞"},
        {"key": "dlg.keeper.greet.text", "en": "You came. Good.", "speaker": "keeper"},
        {"key": "dlg.keeper.greet.opt.0.label", "en": "Who are you?", "speaker": "keeper"},
        {"key": "quest.greet.goal", "en": "Meet the Keeper."},
    ],
}


def inventory():
    return t.parse_inventory(json.loads(json.dumps(INVENTORY_DOC)))


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
    assert inventory().glossary() == {"The Keeper": "守关人", "The Stone Keep": "石垒要塞"}
    assert t.is_glossary_key("class.warden.name")
    assert not t.is_glossary_key("class.warden.blurb"), "a blurb is prose, not a term"
    assert not t.is_glossary_key("dlg.keeper.greet.text")


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
    msgs = t.build_messages(inv, batch, "zh-cn")
    system, user = msgs[0]["content"], msgs[1]["content"]
    assert msgs[0]["role"] == "system" and msgs[1]["role"] == "user"
    assert "zh-cn" in system
    assert "clipped, soldierly" in user, "the speaker's speech style must reach the model"
    assert "warm, rambling" not in user, "only speakers in this batch are described"
    assert "守关人" in user, "glossary keeps proper nouns stable across batches"
    for e in batch:
        assert e.key in user and e.en in user


def test_system_prompt_states_the_player_reply_rule():
    msgs = t.build_messages(inventory(), inventory().entries[:1], "zh-cn")
    assert ".opt." in msgs[0]["content"]
    assert "JSON" in msgs[0]["content"]


# --------------------------------------------------- reflection prompt pass --


def test_translationese_guidance_is_language_scoped():
    assert "翻译腔" in t.translationese_guidance("zh-cn")
    assert t.translationese_guidance("zh-cn") == t.translationese_guidance("ZH-TW")
    assert t.translationese_guidance("zh") == t.translationese_guidance("zh-cn")
    assert t.translationese_guidance("ja") == "", "no checklist beats a wrong checklist"
    assert t.translationese_guidance("de") == ""


def test_zh_system_prompt_carries_the_translationese_checklist():
    system = t.build_messages(inventory(), inventory().entries[:1], "zh-cn")[0]["content"]
    for rule in ("的的不休", "名词化", "信达雅"):
        assert rule in system
    assert "的的不休" not in t.build_messages(inventory(), inventory().entries[:1], "ja")[0]["content"]


def test_reflection_prompt_names_all_four_critique_axes():
    inv = inventory()
    batch = inv.pending()
    msgs = t.build_reflection_messages(inv, batch, "zh-cn", {"dlg.keeper.greet.text": "你来了。"})
    system, user = msgs[0]["content"], msgs[1]["content"]
    for axis in ("ACCURACY", "FLUENCY", "STYLE / REGISTER", "TERMINOLOGY"):
        assert axis in system
    assert "翻译腔" in system, "the zh checklist replaces a generic fluency criterion"
    assert "no change" in system, "an unchanged line must be an expected verdict"
    assert "你来了。" in user, "the critique step sees the draft"
    assert "You came. Good." in user, "and the English beside it"
    assert "clipped, soldierly" in user, "and the persona it must sound like"


def test_option_label_button_budget_reaches_both_prompts():
    """Owner ruling 2026-08-03: an over-long option label scrolls on its
    fixed-width button. The budget must survive translation, so it is stated in
    the translate step AND checked in the critique step."""
    inv = inventory()
    batch = inv.pending()
    translate = t.build_messages(inv, batch, "zh-cn")[0]["content"]
    critique = t.build_reflection_messages(inv, batch, "zh-cn", {})[0]["content"]
    for prompt in (translate, critique):
        assert "12 Han" in prompt and "20 Latin" in prompt
        assert "scroll" in prompt.lower()


def test_reflection_step_does_not_ask_for_json():
    system = t.build_reflection_messages(inventory(), inventory().pending(), "zh-cn", {})[0][
        "content"
    ]
    assert "only diagnoses" in system
    assert "corrected translation" in system


def test_improvement_prompt_carries_critique_draft_and_anti_churn_rule():
    inv = inventory()
    batch = inv.pending()
    draft = {e.key: "草稿:" + e.en for e in batch}
    msgs = t.build_improvement_messages(inv, batch, "zh-cn", draft, "  line 3 is too literal  ")
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
    assert t.translate_batch(cfg, inv, chunk, "zh-cn", "secret-value") == {
        "quest.greet.goal": "去见守关人。"
    }
    assert len(calls) == 1


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

    assert out == {"quest.greet.goal": "去和守关人对话。"}
    assert len(seen) == 3, "translate -> reflect -> improve"
    assert "professional video-game localizer" in seen[0]
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
    msgs = t.build_messages(inventory(), inventory().entries[:2], "zh-cn")
    url, body, headers = t.build_request(cfg, msgs, "secret-value")

    assert url == "https://api.example-provider.test/v1/chat/completions"
    assert body["model"] == "test-model"
    assert body["temperature"] == pytest.approx(0.2)
    assert body["stream"] is False
    assert body["messages"] == list(msgs)
    assert headers["Authorization"] == "Bearer secret-value"
    assert "secret-value" not in json.dumps(body), "the key belongs in the header only"


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

    out = t.chat_once(cfg, [], "secret-value", poster=poster, sleep=lambda _: None)
    assert out == '{"k": "v"}'
    assert len(calls) == 2


def test_auth_failure_fails_fast_without_leaking_the_key(tmp_path):
    cfg = config(tmp_path)
    calls = []

    def poster(url, body, headers, timeout):
        calls.append(url)
        raise urllib.error.HTTPError(url, 401, "Unauthorized", {}, None)

    with pytest.raises(t.TranslateError) as exc:
        t.chat_once(cfg, [], "secret-value", poster=poster, sleep=lambda _: None)
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


def test_sidecar_envelope_is_sorted_and_preserves_dsl_version(tmp_path):
    inv = inventory()
    path = t.sidecar_path(tmp_path, "zh-cn")
    path.parent.mkdir()
    path.write_text(json.dumps({"dsl_version": "0.3.0", "content": {}}), "utf-8")

    t.write_sidecar(path, inv, {"b.key": "乙", "a.key": "甲"})
    raw = path.read_text("utf-8")
    doc = json.loads(raw)
    assert doc["dsl_version"] == "0.3.0", "an existing sidecar keeps its version claim"
    assert doc["campaign_id"] == "keep-trial"
    assert doc["kind"] == "l10n"
    assert doc["lang"] == "zh-cn"
    assert list(doc["content"]) == ["a.key", "b.key"]
    assert "甲" in raw, "translations stay human-readable, not \\u escapes"
    assert raw.endswith("\n")


def test_fresh_sidecar_takes_the_campaign_dsl_version(tmp_path):
    path = t.sidecar_path(tmp_path, "zh-cn")
    t.write_sidecar(path, inventory(), {"a.key": "甲"})
    assert json.loads(path.read_text("utf-8"))["dsl_version"] == "0.6.0"


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
    assert "3 to translate" in out
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
        sent = json.loads(body["messages"][1]["content"].split("key -> translation:\n")[1])
        seen.append([i["key"] for i in sent])
        return {
            "choices": [
                {"message": {"content": json.dumps({i["key"]: "译:" + i["en"] for i in sent})}}
            ]
        }

    monkeypatch.setattr(t, "post_json", poster)
    validated = []

    def fake_validate(args, delvec):
        validated.append(list(args))
        return __import__("subprocess").CompletedProcess(args, 0, "", "")

    monkeypatch.setattr(t, "run_delvec", fake_validate)

    rc = t.main(
        [str(tmp_path), "--lang", "zh-cn", "--config", str(cfg_path), "--batch-size", "2"]
    )
    assert rc == 0
    assert seen == [
        ["dlg.keeper.greet.text", "dlg.keeper.greet.opt.0.label"],
        ["quest.greet.goal"],
    ], "only untranslated keys are sent, in inventory order, batched"
    assert validated and validated[0][0] == "validate"

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
        return {"choices": [{"message": {"content": json.dumps({k: prefix + k for k in keys})}}]}

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
    assert content["dlg.keeper.greet.text"] == "终:dlg.keeper.greet.text", "the revision ships"
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
