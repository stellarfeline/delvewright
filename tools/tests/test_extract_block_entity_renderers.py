"""`extract-block-entity-renderers.py` checks the jar against the pin
`versions.toml` declares: `[minecraft] client_jar_sha256`, the key its sibling
extractors read."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TOOL = REPO / "tools" / "maintenance" / "extract-block-entity-renderers.py"
sys.path.insert(0, str(REPO / "tools" / "lib"))
import versions  # noqa: E402


def test_a_jar_that_is_not_the_pinned_one_is_refused_against_the_declared_pin(tmp_path):
    pinned = versions.load()["minecraft"]["client_jar_sha256"]
    jar = tmp_path / "not-the-client.jar"
    jar.write_bytes(b"not a jar")
    proc = subprocess.run(
        [sys.executable, str(TOOL), str(jar), "--check"], capture_output=True, text=True
    )
    assert proc.returncode == 1, proc
    assert "Traceback" not in proc.stderr, proc.stderr
    assert f"versions.toml [minecraft] client_jar_sha256 is {pinned}" in proc.stderr, proc.stderr
