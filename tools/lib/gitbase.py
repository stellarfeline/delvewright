"""Resolving a `--base` ref, and the ONE remedy printed when it is not there.

## The defect this exists to end

A gate that diffs the checkout against `origin/main`
(`check-numbered-doc-uniqueness`) does not fetch — that is CI's job — so it has to
tell an operator how to get the ref, and it printed the line CI runs:

    git fetch --no-tags --depth=1 origin main:refs/remotes/origin/main

In CI that is right and cheap: `actions/checkout` already leaves a shallow
checkout, the ref is not there at all, and `--depth=1` fetches one commit instead
of the branch's whole history. **On a developer's full clone the same line
converts the working repository into a shallow one**, and the damage is not
confined to the directory it was run in: worktrees share one object store, so the
`shallow` boundary applies to the main checkout and every linked worktree at once.

What that costs, measured on throwaway clones of a 405-commit history where a
feature branch was 1 commit ahead of `origin/main` and 5 behind:

| question                        | truth | after the `--depth=1` line |
| ------------------------------- | ----- | -------------------------- |
| `git merge origin/main`         | merges | `refusing to merge unrelated histories` |
| `merge-base HEAD origin/main`   | a sha | *empty* |
| `rev-list --count origin/main..HEAD` | 1 | **401** |
| `rev-list --count HEAD..origin/main` | 5 | **1** |

The refusal is loud and costs minutes. The counts are the dangerous half: they
are confidently wrong, nothing downstream re-checks them, and "401 ahead" is the
kind of number someone resets or force-pushes on.

## The rule

The remedy is **computed from the repository it will be run in**, never quoted
from CI:

- a **full** clone gets a plain fetch, which cannot shallow anything;
- an **already-shallow** checkout (CI, or a repo someone shallowed earlier) gets
  the `--depth=1` form, because there is no full history left to truncate and
  deepening it would be a cost nobody asked for.

Both install the same ref, so the gate runs either way.

## Why this is a library and not two copies

The unsafe line existed twice because the first gate to need it wrote it inline
and the second copied it. A rule that lives inside one caller is a rule the next
caller cannot reuse — the shape `tools/lib/rcon.{sh,mjs}` was extracted for after
an unchecked command reached a shipped world. A third gate that needs a base ref
imports `resolve_base` and inherits the correct remedy with no decision to make.
"""

from __future__ import annotations

import subprocess
from pathlib import Path


class BaseUnresolved(Exception):
    """`--base` does not name a commit in this checkout. Carries the remedy."""

    def __init__(self, message: str) -> None:
        super().__init__(message)
        self.message = message


def is_shallow(root: Path) -> bool:
    """True when `root`'s repository has a shallow boundary.

    Shallowness belongs to the object store, so this answers for the main
    checkout and every worktree linked to it, whichever one `root` is.
    """
    result = subprocess.run(
        ["git", "rev-parse", "--is-shallow-repository"],
        cwd=root,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip() == "true"


def _split_base(base: str) -> tuple[str, str] | None:
    """`origin/main` -> `("origin", "main")`; anything else -> None.

    Only the `<remote>/<branch>` shape can be turned into a fetch command that is
    certain to install the ref the caller asked for. A base that is a sha, a tag,
    or a local branch gets prose instead of a wrong command.
    """
    if base.startswith("refs/") or "/" not in base:
        return None
    remote, _, branch = base.partition("/")
    if not remote or not branch or "/" in branch:
        return None
    return remote, branch


def fetch_remedy(root: Path, base: str) -> str:
    """The indented remedy block to print when `base` is missing.

    Computed from this repository's own shallowness — see the module docstring.
    """
    parts = _split_base(base)
    if parts is None:
        return (
            f"    Fetch it first. {base!r} is not a `<remote>/<branch>` ref, so\n"
            f"    there is no single command to print here: fetch whatever ref you\n"
            f"    meant, WITHOUT `--depth`, which would shallow this checkout."
        )
    remote, branch = parts
    refspec = f"{remote} {branch}:refs/remotes/{remote}/{branch}"
    if is_shallow(root):
        return (
            f"    This gate diffs the checkout against that ref and cannot run\n"
            f"    without it having been fetched first. This repository is ALREADY\n"
            f"    SHALLOW, so a one-commit fetch is enough and costs nothing:\n"
            f"      git fetch --no-tags --depth=1 {refspec}"
        )
    return (
        f"    This gate diffs the checkout against that ref and cannot run\n"
        f"    without it having been fetched first:\n"
        f"      git fetch --no-tags {refspec}\n"
        f"    Do NOT add `--depth=1`. This is a full clone, and that flag would\n"
        f"    convert it — and every worktree sharing its object store — into a\n"
        f"    shallow one, after which merge-base, ahead/behind counts and every\n"
        f"    other ancestry answer are silently wrong rather than merely absent."
    )


def resolve_base(root: Path, base: str, tool: str) -> str:
    """The sha `base` names, or `BaseUnresolved` carrying the printable failure.

    `tool` is the name the failure is attributed to, so one library serves several
    gates without any of them re-deriving what to say.
    """
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"{base}^{{commit}}"],
        cwd=root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise BaseUnresolved(
            f"{tool}: FAIL — {base!r} does not resolve to a commit in this "
            f"checkout.\n{fetch_remedy(root, base)}"
        )
    return result.stdout.strip()
