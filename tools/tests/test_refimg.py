"""Unit tests for tools/refimg.py (reference-image generation).

Three things are proven here, and the third is the one this file was added for.

1. **Config discipline.** Absent config exits 2 saying what to add; malformed
   config is a hard error, so a typo can never silently downgrade the creator to
   a different provider, a weaker anchor or a different frame.

2. **Capability refusals.** A flag the configured provider cannot honour is an
   ERROR, never a silently dropped parameter. That rule already covered the
   anchors; it now covers the FRAME too, and it has to, because the failure mode
   is invisible: a dropped anchor returns unrelated pictures, and a dropped
   frame returns a correctly-styled picture of the wrong SHAPE — which looks
   exactly like a picture that was asked for.

3. **A frame is per CALL and is recoverable from the sidecar.** A multi-view
   reference wants a different frame per view (a straight-down site plan is not
   16:9). Before the flags existed the only place to say so was a gitignored
   config file, so the round that needed it built an isolated tool root and
   varied the key between calls — careful, correct, and unreproducible from the
   repository afterwards. These tests hold both halves: the flag reaches the
   wire, and the RESOLVED value (not the configured one) reaches the sidecar.

No test may touch the network — `conftest.py` blocks `urlopen`, and every test
here either stops at `--dry-run` or injects a canned response.
"""

import importlib.util
import json
import sys
from pathlib import Path

import pytest

TOOL = Path(__file__).resolve().parents[1] / "refimg.py"


def _load():
    spec = importlib.util.spec_from_file_location("refimg", TOOL)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["refimg"] = mod
    spec.loader.exec_module(mod)
    return mod


t = _load()


# ---------------------------------------------------------------------------
# fixtures
# ---------------------------------------------------------------------------


GEMINI = """
[refimg]
provider = "gemini-native"
model = "gemini-3.1-flash-image"
api_key_env = "TEST_REFIMG_KEY"
"""

IDEOGRAM = """
[refimg]
provider = "ideogram-v3"
model = "ideogram-v3"
api_key_env = "TEST_REFIMG_KEY"
"""


@pytest.fixture
def root(tmp_path, monkeypatch):
    """A fake repo root, so no test reads or writes the real config file."""
    monkeypatch.setattr(t, "repo_root", lambda: tmp_path)
    return tmp_path


def config(root: Path, body: str, **extra) -> Path:
    text = body
    for key, value in extra.items():
        text += f'{key} = "{value}"\n'
    path = root / t.LOCAL_CONFIG_FILE
    path.write_text(text)
    return path


def run(argv):
    return t.main([str(a) for a in argv])


# ---------------------------------------------------------------------------
# config discipline
# ---------------------------------------------------------------------------


def test_absent_config_says_what_to_add_and_exits_2(root, capsys):
    """The exit CODE is pinned here, not only the message.

    The tool raised a bare `SystemExit` on absent config, so it left with 1 —
    while its own module docstring, its row in `docs/reference/tools.md` and the
    skill's Init step all said 2, and `tools/refscore.py`, which states the same
    convention in the same words, does exit 2. The old assertion covered the
    message alone, so nothing held the tool to the number a creator following
    the Init step is told to check for. Strengthened, not relaxed: the message
    is still asserted, and the code is asserted as well.
    """
    assert run(["--prompt", "x", "--dry-run"]) == 2
    assert t.LOCAL_CONFIG_FILE in capsys.readouterr().err


def test_a_config_with_no_refimg_section_leaves_the_same_way(root, capsys):
    """Nothing written and nothing written for THIS tool are one finding: the
    installation cannot draw an image, and it is told what to add."""
    (root / t.LOCAL_CONFIG_FILE).write_text('[refscore]\nbackend = "stub"\n')
    assert run(["--prompt", "x", "--dry-run"]) == 2
    assert t.SECTION in capsys.readouterr().err


def test_a_frame_typo_in_config_is_a_hard_error(root, capsys):
    config(root, GEMINI, image_size="0.5K")
    assert run(["--prompt", "x", "--dry-run"]) == 2
    err = capsys.readouterr().err
    assert "image_size" in err and "0.5K" in err


def test_config_frame_keys_are_validated_against_the_configured_provider(root, capsys):
    """The vocabulary comes from the provider's own `frame` declaration.

    Held explicitly because the check used to name `gemini-native` in an `if`,
    which is a rule keyed to the case its author met rather than to the object.
    """
    config(root, GEMINI, aspect_ratio="16x9")
    assert run(["--prompt", "x", "--dry-run"]) == 2
    assert "aspect_ratio" in capsys.readouterr().err


def test_an_ideogram_config_is_not_judged_by_geminis_vocabulary(root):
    """`resolution` has no published literal set, so it is passed through."""
    config(root, IDEOGRAM, resolution="1280x800")
    assert run(["--prompt", "x", "--dry-run"]) == 0


# ---------------------------------------------------------------------------
# capability refusals — the frame follows the same rule as the anchors
# ---------------------------------------------------------------------------


def test_aspect_ratio_is_refused_by_a_provider_that_frames_in_pixels(root):
    config(root, IDEOGRAM)
    with pytest.raises(SystemExit) as exc:
        run(["--prompt", "x", "--aspect-ratio", "1:1", "--dry-run"])
    message = str(exc.value)
    assert "--aspect-ratio" in message
    assert "ideogram-v3" in message
    assert "--resolution" in message, "a refusal names the frame this provider DOES take"


def test_image_size_is_refused_by_a_provider_that_frames_in_pixels(root):
    config(root, IDEOGRAM)
    with pytest.raises(SystemExit) as exc:
        run(["--prompt", "x", "--image-size", "2K", "--dry-run"])
    assert "--image-size" in str(exc.value)


def test_resolution_is_refused_by_a_provider_that_frames_in_ratios(root):
    """The refusal runs in BOTH directions, which is the half that was missing.

    `--resolution` reached the multipart wire and was read nowhere on the JSON
    one, so passing it to `gemini-native` was a silently dropped frame — the
    exact defect the flag rule exists to prevent, sitting inside the tool that
    states the rule.
    """
    config(root, GEMINI)
    with pytest.raises(SystemExit) as exc:
        run(["--prompt", "x", "--resolution", "1280x800", "--dry-run"])
    message = str(exc.value)
    assert "--resolution" in message
    assert "--aspect-ratio" in message and "--image-size" in message


def test_a_misspelt_frame_value_on_the_flag_is_refused(root):
    config(root, GEMINI)
    with pytest.raises(SystemExit) as exc:
        run(["--prompt", "x", "--aspect-ratio", "16x9", "--dry-run"])
    assert "16x9" in str(exc.value)


def test_the_capability_refusal_comes_before_the_value_check(root):
    """Telling someone their ratio is misspelt when their provider has no ratio
    at all sends them to fix the wrong thing."""
    config(root, IDEOGRAM)
    with pytest.raises(SystemExit) as exc:
        run(["--prompt", "x", "--aspect-ratio", "nonsense", "--dry-run"])
    assert "has no aspect_ratio" in str(exc.value)


# ---------------------------------------------------------------------------
# the frame is per call, and it reaches the wire
# ---------------------------------------------------------------------------


def test_the_flag_overrides_config_on_the_wire(root, capsys):
    config(root, GEMINI, aspect_ratio="16:9", image_size="2K")
    assert run(["--prompt", "a plan", "--aspect-ratio", "1:1", "--dry-run"]) == 0
    body = json.loads(capsys.readouterr().out.split("json body:", 1)[1])
    assert body["response_format"]["aspect_ratio"] == "1:1"
    assert body["response_format"]["image_size"] == "2K", "config still supplies the rest"


def test_config_supplies_the_frame_when_no_flag_is_given(root, capsys):
    config(root, GEMINI, aspect_ratio="21:9")
    assert run(["--prompt", "x", "--dry-run"]) == 0
    body = json.loads(capsys.readouterr().out.split("json body:", 1)[1])
    assert body["response_format"]["aspect_ratio"] == "21:9"


def test_the_default_frame_applies_when_neither_says_anything(root, capsys):
    config(root, GEMINI)
    assert run(["--prompt", "x", "--dry-run"]) == 0
    body = json.loads(capsys.readouterr().out.split("json body:", 1)[1])
    assert body["response_format"] == {"type": "image", "aspect_ratio": "16:9",
                                       "image_size": "2K"}


def test_resolution_reaches_the_multipart_wire(root, capsys):
    config(root, IDEOGRAM)
    assert run(["--prompt", "x", "--resolution", "1280x800", "--dry-run"]) == 0
    assert "resolution = 1280x800" in capsys.readouterr().out


# ---------------------------------------------------------------------------
# the frame is recoverable from the sidecar
# ---------------------------------------------------------------------------


CANNED = {"id": "v1_abc", "object": "interaction", "steps": []}


def _sidecar(root, monkeypatch, argv, cfg_body=GEMINI, **extra):
    config(root, cfg_body, **extra)
    monkeypatch.setenv("TEST_REFIMG_KEY", "not-a-real-key")
    monkeypatch.setattr(t, "call", lambda cfg, payload, files: dict(CANNED))
    out = root / "out" / "ref"
    assert run([*argv, "--out", out]) == 0
    return json.loads(out.with_suffix(".json").read_text())


def test_the_sidecar_records_the_frame_that_was_ASKED_FOR(root, monkeypatch):
    """Not the configured one. A sidecar that reports a frame the request did
    not carry is worse than no record: the series looks reproducible and is not.
    """
    doc = _sidecar(root, monkeypatch,
                   ["--prompt", "a plan", "--aspect-ratio", "1:1"],
                   aspect_ratio="16:9")
    assert doc["request"]["aspect_ratio"] == "1:1"


def test_the_sidecar_records_a_defaulted_frame_too(root, monkeypatch):
    """The default is what went on the wire, so leaving it null would make an
    unconfigured installation's images unreproducible for no reason."""
    doc = _sidecar(root, monkeypatch, ["--prompt", "x"])
    assert doc["request"]["aspect_ratio"] == "16:9"
    assert doc["request"]["image_size"] == "2K"


def test_every_frame_key_is_present_in_the_sidecar(root, monkeypatch):
    """`null` where the provider has no such vocabulary, so a reader never has
    to guess whether an absent key means "not asked for" or "not written"."""
    doc = _sidecar(root, monkeypatch, ["--prompt", "x"])
    for key in t.FRAME_KEYS:
        assert key in doc["request"], key
    assert doc["request"]["resolution"] is None


def test_the_sidecar_keeps_the_prompt_that_produced_the_image(root, monkeypatch):
    """A long prompt is the anchor a series needs, not a payload to elide.

    The request record exists because a round once shipped images whose prompt
    lived only in the shell that launched them. A reference prompt runs to
    thousands of words — the four whole-map views were 8–10 KB each — so a size
    rule written for image bytes elides exactly the thing this record is for.
    """
    prompt = "a sea-gate barbican at dusk. " * 500
    assert len(prompt) > t.REDACT_OVER_CHARS
    doc = _sidecar(root, monkeypatch, ["--prompt", prompt])
    assert doc["request"]["prompt"] == prompt


def test_the_sidecar_names_reference_images_by_path_never_by_payload(root, monkeypatch):
    ref = root / "anchor.png"
    ref.write_bytes(b"\x89PNG" + b"\x00" * 4096)
    doc = _sidecar(root, monkeypatch, ["--prompt", "x", "--style-ref", ref])
    assert doc["request"]["reference_images"] == [str(ref)]
    assert "iVBOR" not in json.dumps(doc)


def test_the_response_still_drops_oversized_payloads(root, monkeypatch):
    """The redaction rule is kept where it was written for — the RESPONSE, whose
    oversized strings are image bytes and thought signatures."""
    config(root, GEMINI)
    monkeypatch.setenv("TEST_REFIMG_KEY", "k")
    payload = "A" * (t.REDACT_OVER_CHARS + 1)
    monkeypatch.setattr(t, "call",
                        lambda cfg, p, f: {"id": "x", "junk": payload, "steps": []})
    out = root / "out" / "ref"
    assert run(["--prompt", "x", "--out", out]) == 0
    doc = json.loads(out.with_suffix(".json").read_text())
    assert doc["junk"] == f"<{len(payload)} chars elided — payload, not an anchor>"


# ---------------------------------------------------------------------------
# the anchors, unchanged — held so the frame work cannot quietly cost them
# ---------------------------------------------------------------------------


def test_chain_from_is_refused_by_a_provider_without_chaining(root):
    config(root, IDEOGRAM)
    with pytest.raises(SystemExit) as exc:
        run(["--prompt", "x", "--chain-from", "v1_abc", "--dry-run"])
    assert "--chain-from" in str(exc.value)


def test_style_code_is_refused_by_a_provider_without_one(root):
    config(root, GEMINI)
    with pytest.raises(SystemExit) as exc:
        run(["--prompt", "x", "--style-code", "A1B2C3D4", "--dry-run"])
    assert "--style-code" in str(exc.value)


def test_chain_from_reaches_the_wire(root, capsys):
    config(root, GEMINI)
    assert run(["--prompt", "x", "--chain-from", "v1_abc", "--dry-run"]) == 0
    body = json.loads(capsys.readouterr().out.split("json body:", 1)[1])
    assert body["previous_interaction_id"] == "v1_abc"
