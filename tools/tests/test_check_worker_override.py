"""The worker-isolation coverage gate (`tools/check-worker-override.py`).

The defect this pins, twice from the field: `validation/compose.yaml` pins a
`container_name`, `validation/worker-override.yaml` forgets to reset it, and two
concurrent workers collide on a Docker-GLOBAL name that `-p <project>` does not
isolate — `server` (#190), then `bot` (the-drowned-bell round 2). Nothing else in
the repo relates the two files, so every other gate stays green while it happens.

These tests drive the detector over synthetic compose pairs so they keep failing
for the right reason as the real profiles grow. The live pair is checked by the
CI step itself.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-worker-override.py"


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at a synthetic compose pair."""
    spec = importlib.util.spec_from_file_location("check_worker_override", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    monkeypatch.setattr(module, "COMPOSE", tmp_path / "compose.yaml")
    monkeypatch.setattr(module, "OVERRIDE", tmp_path / "override.yaml")
    return module


def pair(gate, compose: str, override: str) -> int:
    gate.COMPOSE.write_text(compose, encoding="utf-8")
    gate.OVERRIDE.write_text(override, encoding="utf-8")
    return gate.main()


COMPOSE = """\
services:
  server:
    image: x
    container_name: delvewright-server
    ports:
      - "127.0.0.1:25565:25565"
  bot:
    build: ../harness
    container_name: delvewright-bot
  helper:
    image: y
"""


def test_a_fully_reset_override_passes(gate):
    assert (
        pair(
            gate,
            COMPOSE,
            "services:\n"
            "  server:\n    container_name: !reset null\n    ports: !reset []\n"
            "  bot:\n    container_name: !reset null\n",
        )
        == 0
    )


def test_a_missing_service_fails(gate, capsys):
    # The `bot` omission verbatim: the service is pinned but absent from the override.
    assert (
        pair(
            gate,
            COMPOSE,
            "services:\n  server:\n    container_name: !reset null\n    ports: !reset []\n",
        )
        == 1
    )
    assert "`bot`" in capsys.readouterr().err


def test_a_plain_null_does_not_count_as_a_reset(gate, capsys):
    # `container_name: null` is silently ignored by compose — the container comes
    # up with its pinned name anyway (validation/README.md).
    assert (
        pair(
            gate,
            COMPOSE,
            "services:\n"
            "  server:\n    container_name: null\n    ports: !reset []\n"
            "  bot:\n    container_name: !reset null\n",
        )
        == 1
    )
    assert "!reset null" in capsys.readouterr().err


def test_an_unpublished_service_needs_no_entry(gate):
    # `helper` pins nothing global, so the override must not have to mention it.
    assert (
        pair(
            gate,
            COMPOSE,
            "services:\n"
            "  server:\n    container_name: !reset null\n    ports: !reset []\n"
            "  bot:\n    container_name: !reset null\n",
        )
        == 0
    )


def test_a_stale_override_entry_fails(gate, capsys):
    assert (
        pair(
            gate,
            COMPOSE,
            "services:\n"
            "  server:\n    container_name: !reset null\n    ports: !reset []\n"
            "  bot:\n    container_name: !reset null\n"
            "  gone:\n    container_name: !reset null\n",
        )
        == 1
    )
    assert "`gone`" in capsys.readouterr().err
