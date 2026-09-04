"""The compose-isolation gate (`tools/check-compose-isolation.py`).

The defect class this pins, twice from the field before the pins were removed
entirely: `validation/compose.yaml` names a container or publishes a host
port, two concurrent ladders collide on a Docker-GLOBAL name that `-p <project>`
does not isolate, and one teardown reaches into the other — `server`, then
`bot` (the-drowned-bell round 2). The predecessor gate only demanded a matching
`!reset` in a worker override, i.e. the pin survived and every caller had to
remember an extra `-f`. Now the pin itself is the violation.

The IMAGE TAG is the third name of that class and the one nothing checked: a tag
is a key in the daemon's single image store, so `branch-runs.sh` and the two
`playtest`-profile flows each ran `up --build` in a unique compose project while
tagging the result `delvewright/delve:local` — three trees into one name, and the
loser boots the other ladder's delve with nothing to say so. Both halves are
pinned below: a compose service that cannot be scoped, and a caller that does not
scope it.

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


# ---- the image tag, the third Docker-global name ----------------------------

BUILDER_COMPOSE = """\
services:
  server:
    build:
      context: ./delve-output
    image: ${DELVE_IMAGE:-delvewright/delve:local}
  bot:
    build: ../harness
  packtest:
    image: ghcr.io/x/toolserver@sha256:abc
"""

LADDER = """\
#!/usr/bin/env bash
# A comment that names delvewright/delve:local and up --build, and must not count.
. validation/lib/delve-image.sh
dw_export_delve_image "$PROJECT"
docker compose -p "$PROJECT" -f validation/compose.yaml up --build
"""


@pytest.fixture
def builders(gate):
    """`gate`, plus a builder script and the lib the gate binds it to."""
    gate.write("compose.yaml", BUILDER_COMPOSE)
    (gate.ROOT / "lib").mkdir()
    (gate.ROOT / "lib" / "delve-image.sh").write_text(
        "dw_export_delve_image() { export DELVE_IMAGE=x; }\n", encoding="utf-8")
    gate.write("ladder.sh", LADDER)
    return gate


def test_a_scoped_builder_passes(builders, capsys):
    assert builders.main() == 0
    # Non-vacuity: the rule examined the script, rather than finding none.
    assert "1 script(s) that build scope the image tag" in capsys.readouterr().out


def test_a_built_service_with_a_literal_tag_fails(builders, capsys):
    builders.write("compose.yaml", BUILDER_COMPOSE.replace(
        "${DELVE_IMAGE:-delvewright/delve:local}", "delvewright/delve:local"))
    assert builders.main() == 1
    err = capsys.readouterr().err
    assert "`server`" in err and "BUILDS into the literal image tag" in err


def test_a_pulled_image_may_be_a_literal_tag(builders):
    # `packtest` names a pinned digest and BUILDS nothing — it is not this
    # ladder's image to overwrite, so the rule must not reach it.
    assert builders.main() == 0


def test_a_builder_that_never_scopes_the_tag_fails(builders, capsys):
    builders.write("ladder.sh", LADDER.replace('dw_export_delve_image "$PROJECT"', ":"))
    assert builders.main() == 1
    err = capsys.readouterr().err
    assert "ladder.sh" in err and "dw_export_delve_image" in err


def test_the_call_must_be_code_and_not_a_comment(builders, capsys):
    # The leak is documented in a comment beside every site it leaked at, so a
    # scan that reads comments finds the words instead of the code — and passes.
    builders.write("ladder.sh", LADDER.replace(
        'dw_export_delve_image "$PROJECT"', '# dw_export_delve_image "$PROJECT"'))
    assert builders.main() == 1
    assert "ladder.sh" in capsys.readouterr().err


def test_a_script_that_does_not_build_is_not_asked_to_scope(builders):
    # `up -d` with no `--build` boots an image somebody else tagged; it has no
    # tree of its own to name.
    builders.write("boot-only.sh", LADDER.replace(
        'dw_export_delve_image "$PROJECT"', ":").replace("up --build", "up -d"))
    assert builders.main() == 0


def test_the_dash_d_spelling_is_still_a_build(builders, capsys):
    builders.write("ladder.sh", LADDER.replace(
        'dw_export_delve_image "$PROJECT"', ":").replace(
        "up --build", "up -d --build server"))
    assert builders.main() == 1
    assert "ladder.sh" in capsys.readouterr().err


def test_a_missing_lib_is_a_violation_while_anything_builds(builders, capsys):
    (builders.ROOT / "lib" / "delve-image.sh").unlink()
    assert builders.main() == 1
    assert "is the ONE place the delve image tag is named" in capsys.readouterr().err
