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

import base64
import importlib.util
import json
import sys
import urllib.error
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


def _image_step(n: int = 1) -> dict:
    """A canned `model_output` step carrying `n` inline JPEGs.

    Longer than the extractor's 256-char floor so the walk finds it, shorter than
    `REDACT_OVER_CHARS` so the redaction rule is not what these tests measure.
    """
    return {"type": "model_output",
            "content": [{"type": "image", "mime_type": "image/jpeg",
                         "data": base64.b64encode(b"\xff\xd8\xff" + bytes(400)).decode()}
                        for _ in range(n)]}


def canned(images: int = 1) -> dict:
    """A response with a known image count.

    One by default, and that default is load-bearing: a response carrying NO
    image now exits non-zero, so a fixture with none in it would be measuring the
    zero-return path in every test that only wanted a sidecar.
    """
    return {"id": "v1_abc", "object": "interaction", "steps": [_image_step(images)]}


CANNED = canned()


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
                        lambda cfg, p, f: {**canned(), "junk": payload})
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


# ---------------------------------------------------------------------------
# how many pictures came back, and what that cost
#
# A provider that has no count field decides for itself, and the decision is
# billed. Measured over 48 calls in one round: 42 returned one image, one
# returned 2, two returned 5, one returned ELEVEN, and two returned nothing at
# all. None of that was visible in `--help`, in `--dry-run`, in the sidecar, or
# in the exit code — which is what these tests are about.
# ---------------------------------------------------------------------------


def _canned_run(root, monkeypatch, response, argv=("--prompt", "x"), cfg_body=GEMINI):
    config(root, cfg_body)
    monkeypatch.setenv("TEST_REFIMG_KEY", "k")
    monkeypatch.setattr(t, "call", lambda cfg, p, f: response)
    out = root / "out" / "ref"
    return run([*argv, "--out", out]), out


def test_the_first_image_takes_the_name_that_was_asked_for(root, monkeypatch):
    """One image, and it is `<stem>.jpg` — not `<stem>-0.jpg`."""
    code, out = _canned_run(root, monkeypatch, canned(1))
    assert code == 0
    assert out.with_suffix(".jpg").exists()
    assert not out.with_name("ref-0.jpg").exists()


def test_a_multi_return_still_writes_the_requested_name(root, monkeypatch, capsys):
    """THE defect: five images used to be written `ref-0.jpg` … `ref-4.jpg` with
    no `ref.jpg` at all, so every existence check on the requested name reported
    the picture missing. How many a provider chose to draw is not a fact about
    what was asked for."""
    code, out = _canned_run(root, monkeypatch, canned(5))
    assert code == 0, "the requested image exists, so this is not a failure"
    assert out.with_suffix(".jpg").exists(), "the name that was asked for"
    for i in range(1, 5):
        assert out.with_name(f"ref-{i}.jpg").exists(), i
    assert not out.with_name("ref-0.jpg").exists(), "no image is named after index 0"


def test_a_multi_return_says_how_many_and_that_they_were_billed(root, monkeypatch, capsys):
    _, out = _canned_run(root, monkeypatch, canned(11))
    err = capsys.readouterr().err
    assert "11 images" in err
    assert "billed" in err
    assert "ref.jpg" in err and "ref-10.jpg" in err


def test_the_sidecar_records_the_count_and_the_names(root, monkeypatch):
    """A terminal line is gone by the next round; the sidecar travels with the
    image into the campaign, and is the only place a later reader can tell a
    one-image call from an eleven-image one."""
    _, out = _canned_run(root, monkeypatch, canned(3))
    doc = json.loads(out.with_suffix(".json").read_text())
    assert doc["images"]["returned"] == 3
    assert doc["images"]["written"] == ["ref.jpg", "ref-1.jpg", "ref-2.jpg"]


def test_a_one_image_call_records_the_count_too(root, monkeypatch):
    """Recorded on every call, not only the surprising ones — a field that is
    absent when the answer is boring cannot be read as an answer."""
    _, out = _canned_run(root, monkeypatch, canned(1))
    doc = json.loads(out.with_suffix(".json").read_text())
    assert doc["images"] == {"returned": 1, "written": ["ref.jpg"]}


def test_a_response_with_no_image_leaves_non_zero_and_names_out(root, monkeypatch, capsys):
    """It used to be a warning beside exit 0, so a script could not tell a drawn
    image from a response nobody could extract one out of."""
    code, out = _canned_run(root, monkeypatch, {"id": "v1_abc", "steps": []})
    assert code == 1
    err = capsys.readouterr().err
    assert str(out) in err
    assert "no image" in err
    assert str(out.with_suffix(".json")) in err, "the paid response is still kept"
    assert out.with_suffix(".json").exists()


def test_count_is_refused_by_a_provider_that_has_no_count_field(root):
    """The same rule the anchors and the frame follow. `--count 4` used to reach
    `gemini-native` and be dropped on the floor — a flag that reads as a bound on
    what you will be billed for and was not one."""
    config(root, GEMINI)
    with pytest.raises(SystemExit) as exc:
        run(["--prompt", "x", "--count", "4", "--dry-run"])
    message = str(exc.value)
    assert "--count" in message and "gemini-native" in message


def test_an_unset_count_is_not_a_refusal(root):
    """The default has to be distinguishable from an explicit 1, or the refusal
    above would refuse every ordinary call."""
    config(root, GEMINI)
    assert run(["--prompt", "x", "--dry-run"]) == 0


def test_an_unset_count_asks_the_wire_that_has_one_for_exactly_one(root, capsys):
    config(root, IDEOGRAM)
    assert run(["--prompt", "x", "--dry-run"]) == 0
    assert "num_images = 1" in capsys.readouterr().out


def test_dry_run_says_a_call_may_return_more_than_one(root, capsys):
    """`--dry-run` is the costless mode, so it is where a creator looks before
    spending anything — and it said nothing at all about how many pictures a
    call might bill for."""
    config(root, GEMINI)
    assert run(["--prompt", "x", "--out", root / "z", "--dry-run"]) == 0
    out = capsys.readouterr().out
    assert "ELEVEN" in out and "billed" in out
    assert "no count field" in out


def test_dry_run_on_a_counted_provider_says_the_number(root, capsys):
    config(root, IDEOGRAM)
    assert run(["--prompt", "x", "--count", "3", "--dry-run"]) == 0
    assert "images: exactly 3" in capsys.readouterr().out


# ---------------------------------------------------------------------------
# a timeout is retried once, then NAMED
# ---------------------------------------------------------------------------


def _timeout_call(root, monkeypatch, failures, cfg_body=GEMINI):
    """Drive the real `call` with a fake `urlopen` that times out `failures` times."""
    config(root, cfg_body)
    monkeypatch.setenv("TEST_REFIMG_KEY", "k")
    calls = {"n": 0}

    class _Resp:
        def __enter__(self):
            return self

        def __exit__(self, *a):
            return False

        def read(self):
            return json.dumps(canned(1)).encode()

    def fake_urlopen(req, timeout=None):
        calls["n"] += 1
        if calls["n"] <= failures:
            raise TimeoutError("The read operation timed out")
        return _Resp()

    monkeypatch.setattr(t.urllib.request, "urlopen", fake_urlopen)
    return calls


def test_one_timeout_is_retried_and_the_call_succeeds(root, monkeypatch, capsys):
    calls = _timeout_call(root, monkeypatch, failures=1)
    out = root / "out" / "ref"
    assert run(["--prompt", "x", "--out", out]) == 0
    assert calls["n"] == 2, "one retry, and it was used"
    assert out.with_suffix(".jpg").exists()
    err = capsys.readouterr().err
    assert "retrying once" in err
    assert "billed" in err, "a timed-out attempt may still have been charged"


def test_two_timeouts_fail_in_one_line_naming_out(root, monkeypatch, capsys):
    """The traceback this replaces was twenty lines of `urllib` frames ending
    `TimeoutError: The read operation timed out`, naming neither the prompt nor
    the output path — so a round that lost two calls out of forty-eight could not
    tell from the terminal which two."""
    calls = _timeout_call(root, monkeypatch, failures=2)
    out = root / "out" / "ref"
    assert run(["--prompt", "x", "--out", out]) == 1
    assert calls["n"] == t.CALL_ATTEMPTS == 2, "bounded: it does not retry forever"
    err = capsys.readouterr().err.strip()
    assert len(err.splitlines()) == 2, "the retry notice and one failure line"
    failure = err.splitlines()[-1]
    assert str(out) in failure
    assert "twice" in failure and "billed" in failure


def test_a_connect_timeout_wrapped_in_urlerror_is_the_same_finding(root, monkeypatch, capsys):
    """A read timeout arrives bare; a connect timeout arrives inside a
    `URLError`. Both are one finding to a creator."""
    config(root, GEMINI)
    monkeypatch.setenv("TEST_REFIMG_KEY", "k")

    def fake_urlopen(req, timeout=None):
        raise urllib.error.URLError(TimeoutError("timed out"))

    monkeypatch.setattr(t.urllib.request, "urlopen", fake_urlopen)
    out = root / "out" / "ref"
    assert run(["--prompt", "x", "--out", out]) == 1
    assert "twice" in capsys.readouterr().err


def test_an_unreachable_provider_is_named_rather_than_traced(root, monkeypatch, capsys):
    """Not retried — it is not transient — but still one line rather than a
    traceback, because it is a case the tool can name."""
    config(root, GEMINI)
    monkeypatch.setenv("TEST_REFIMG_KEY", "k")
    calls = {"n": 0}

    def fake_urlopen(req, timeout=None):
        calls["n"] += 1
        raise urllib.error.URLError("nodename nor servname provided")

    monkeypatch.setattr(t.urllib.request, "urlopen", fake_urlopen)
    out = root / "out" / "ref"
    assert run(["--prompt", "x", "--out", out]) == 1
    assert calls["n"] == 1, "a name-resolution failure is not retried"
    err = capsys.readouterr().err.strip()
    assert len(err.splitlines()) == 1
    assert str(out) in err and "could not be reached" in err


def test_dry_run_never_reaches_the_wire(root, monkeypatch):
    """The retry lives inside `call`; `--dry-run` must still cost nothing.
    `conftest.py` blocks `urlopen`, so a call here is an AssertionError."""
    config(root, GEMINI)
    assert run(["--prompt", "x", "--dry-run"]) == 0
