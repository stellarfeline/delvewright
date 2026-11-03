"""Where a checkout's gitignored `delvewright.local.toml` is.

The local config is gitignored, and a linked worktree checks out tracked files
only, so an engine run from a worktree never sees the file its main checkout
holds. This module answers the one question every tool that reads the file asks
(`tools/creator/i18n-translate.py`, `tools/creator/refimg.py`): the checkout's own
file when it has one, else the main checkout's of the same repository, else the
checkout's own path (absent). Outside a git checkout it is the root's own path.
"""

from __future__ import annotations

import subprocess
from pathlib import Path

LOCAL_CONFIG_FILE = "delvewright.local.toml"


def _main_checkout(root: Path) -> Path | None:
    """The main worktree of the repository `root` is in, or None."""
    try:
        out = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "--path-format=absolute", "--git-common-dir"],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        return None
    common = out.stdout.strip()
    if out.returncode != 0 or not common:
        return None
    common_dir = Path(common)
    return common_dir.parent if common_dir.name == ".git" else None


def local_config_path(root: Path) -> Path:
    """The `delvewright.local.toml` a tool run from the checkout `root` reads."""
    own = root / LOCAL_CONFIG_FILE
    if own.is_file():
        return own
    main = _main_checkout(root)
    if main is not None and (main / LOCAL_CONFIG_FILE).is_file():
        return main / LOCAL_CONFIG_FILE
    return own
