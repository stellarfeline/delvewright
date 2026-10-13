#!/usr/bin/env bash
# district-hook.sh — install the write-set refusal in a district worker's
# worktree (spec-0103 §7.6): a pre-commit hook that hands the staged paths to
# `delvec district writes`, so a write outside the district is refused at the
# commit where it is entered.
#
#   tools/creator/district-hook.sh <worktree> <campaign-dir> <district> <prefabs-dir> [delvec]
#
# <campaign-dir> and <prefabs-dir> are the worktree's own, and are resolved to
# absolute paths here so the hook does not depend on where git runs it. The
# hook lives in the worktree's private git directory and is bound with a
# per-worktree `core.hooksPath`, so it guards this worktree and no other — the
# planner's checkout and the siblings' worktrees are untouched. Binding a
# per-worktree key needs `extensions.worktreeConfig` in the repository's shared
# config (git's own mechanism for it); that one shared line is set, and changes
# nothing for a worktree that has no `config.worktree` of its own.
#
# The rule itself is the subcommand's; this file only calls it. Exit 0 when the
# hook is installed, 2 on a usage error.
set -euo pipefail

if [[ $# -lt 4 || $# -gt 5 ]]; then
  echo "usage: $0 <worktree> <campaign-dir> <district> <prefabs-dir> [delvec]" >&2
  exit 2
fi
wt="$(cd "$1" && pwd)"
campaign="$(cd "$2" && pwd)"
district="$3"
prefabs="$(cd "$4" && pwd)"
delvec="${5:-delvec}"
if [[ $delvec == */* ]]; then
  delvec="$(cd "$(dirname "$delvec")" && pwd)/$(basename "$delvec")"
fi

gitdir="$(git -C "$wt" rev-parse --absolute-git-dir)"
hooks="$gitdir/district-hooks"
mkdir -p "$hooks"
cat >"$hooks/pre-commit" <<HOOK
#!/usr/bin/env bash
# Installed by tools/creator/district-hook.sh for $district (spec-0103 §7.6).
set -euo pipefail
staged="\$(git diff --cached --name-only)"
printf '%s\n' "\$staged" | "$delvec" --prefabs "$prefabs" district writes "$campaign" "$district" --paths -
HOOK
chmod +x "$hooks/pre-commit"
git -C "$wt" config extensions.worktreeConfig true
git -C "$wt" config --worktree core.hooksPath "$hooks"
echo "district hook: $district guarded in $wt ($hooks/pre-commit)"
