"""The ladder scripts that build the delve image run compose from the engine root.

Compose builds through buildx bake, and bake refuses to read a file outside the
working directory without an `fs.read` entitlement. The skill runs the ladder
from the content tree, where the engine's `validation/Dockerfile.delve` lies
outside the working directory, and the image build was refused before the
hatch stand ran ("additional privileges requested: pass
--allow=fs.read=../<engine>/validation/Dockerfile.delve"). Every script that
exports `DELVE_DOCKERFILE` and then drives compose changes into the engine root
first.
"""

from __future__ import annotations

import re
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
VALIDATION = REPO / "validation"


def _scripts_exporting_the_dockerfile() -> list[Path]:
    return sorted(p for p in VALIDATION.glob("*.sh") if "export DELVE_DOCKERFILE=" in p.read_text())


def test_every_script_that_builds_the_delve_image_cds_into_the_engine_root_before_compose():
    scripts = _scripts_exporting_the_dockerfile()
    assert len(scripts) >= 2, f"expected the bot ladder and the world save, found {scripts}"
    for p in scripts:
        lines = p.read_text().splitlines()
        cd = next(
            (i for i, l in enumerate(lines) if l.strip() in ('cd "$here/.."', 'cd "$(dirname "$0")/.."')),
            None,
        )
        compose = next(
            (i for i, l in enumerate(lines) if re.match(r'\s*COMPOSE=\(docker compose ', l)), None
        )
        assert compose is not None, f"{p.name}: no COMPOSE=(docker compose …) line"
        assert cd is not None and cd < compose, (
            f"{p.name}: compose runs from the caller's directory; bake then refuses the engine's "
            "Dockerfile.delve outside it"
        )
