"""The compose-isolation gate (`tools/check-compose-isolation.py`).

The defect class this pins, twice from the field before task #185 removed the
pins entirely: `validation/compose.yaml` names a container or publishes a host
port, two concurrent ladders collide on a Docker-GLOBAL name that `-p <project>`
does not isolate, and one teardown reaches into the other — `server` (#190), then
`bot` (the-drowned-bell round 2). The predecessor gate only demanded a matching
`!reset` in a worker override, i.e. the pin survived and every caller had to
remember an extra `-f`. Now the pin itself is the violation.

These tests drive the detector over synthetic `validation/` trees so they keep
failing for the right reason as the real profiles grow. The live tree is checked
by the CI step itself.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-compose-isolation.py"

CLEAN_COMPOSE = """\
services:
  server:
    image: x
    volumes:
      - server-data:/data
  bot:
    build: ../harness
  packtest:
    image: y
"""

OWNER = """\
services:
  server:
    container_name: delvewright-server
    ports:
      - "127.0.0.1:25565:25565"
"""

EPHEMERAL = """\
services:
  server:
    ports:
      - "127.0.0.1::25565"
"""


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at a synthetic validation/ tree."""
    spec = importlib.util.spec_from_file_location("check_compose_isolation", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    root = tmp_path / "validation"
    root.mkdir()
    monkeypatch.setattr(module, "ROOT", root)
    module.write = lambda name, text: (root / name).write_text(text, encoding="utf-8")
    module.write("compose.yaml", CLEAN_COMPOSE)
    module.write("owner-play.yaml", OWNER)
    return module


def test_a_global_free_stack_passes(gate):
    gate.write("ephemeral-port.yaml", EPHEMERAL)
    assert gate.main() == 0


def test_a_pinned_container_name_in_the_base_file_fails(gate, capsys):
    gate.write("compose.yaml", CLEAN_COMPOSE.replace(
        "  bot:\n", "  bot:\n    container_name: delvewright-bot\n"))
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "`bot`" in err and "container_name" in err


def test_a_published_port_in_the_base_file_fails(gate, capsys):
    gate.write("compose.yaml", CLEAN_COMPOSE.replace(
        "  server:\n    image: x\n",
        '  server:\n    image: x\n    ports:\n      - "127.0.0.1:25565:25565"\n'))
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "`server`" in err and "ports" in err


def test_an_inline_port_list_in_the_base_file_fails(gate, capsys):
    # The block-list spelling is not the only one; an inline list must not slip past.
    gate.write("compose.yaml", CLEAN_COMPOSE.replace(
        "  server:\n    image: x\n",
        '  server:\n    image: x\n    ports: ["127.0.0.1:25565:25565"]\n'))
    assert gate.main() == 1
    assert "`server`" in capsys.readouterr().err


def test_a_fixed_port_outside_the_owner_file_fails(gate, capsys):
    gate.write("some-override.yaml", 'services:\n  server:\n    ports:\n      - "25566:25565"\n')
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "25566:25565" in err and "ephemeral" in err


def test_an_ephemeral_port_outside_the_owner_file_passes(gate):
    gate.write("some-override.yaml", 'services:\n  server:\n    ports:\n      - "127.0.0.1::25565"\n')
    assert gate.main() == 0


def test_the_owner_file_may_bind_only_25565(gate, capsys):
    gate.write("owner-play.yaml", OWNER.replace("25565:25565", "25570:25565"))
    assert gate.main() == 1
    assert "25565" in capsys.readouterr().err


def test_a_container_name_outside_the_owner_file_fails(gate, capsys):
    gate.write("some-override.yaml", "services:\n  server:\n    container_name: dw-x\n")
    assert gate.main() == 1
    assert "only" in capsys.readouterr().err


def test_a_missing_owner_file_is_an_error(gate):
    (gate.ROOT / "owner-play.yaml").unlink()
    assert gate.main() == 2
