#!/usr/bin/env python3
r"""A redirect must not be the first thing that fails.

WHY THIS EXISTS (v1.0.0 release run, 2026-08-06 — Actions run 31205291235)

`crates.io preflight (no credential)` reported:

    tools/check-publishable.sh: line 79: .../target/package-log.txt: No such file
      FAIL cargo package failed:
    sed: can't read .../target/package-log.txt

`cargo package` had NOT failed. `cargo package` had never run. The runner restored
no build cache, so `target/` did not exist; `>"$ROOT/target/package-log.txt"` is
opened by the SHELL before the command it captures is executed, so the redirect
failed, the subshell died with status 1, the `if` took the else branch, and the
else branch then `sed`ed a file whose absence WAS the finding.

The general form is worth more than the instance:

    AN ERROR PATH MUST NOT DEPEND ON AN ARTIFACT THE ERROR MAY HAVE PREVENTED
    FROM EXISTING.

The report was not merely unhelpful — it was wrong about what happened, and it
named an innocent command. That is strictly worse than no report, because it
sends the next reader to `cargo`.

THE RULE CHECKED HERE

Every `>` / `>>` redirect in a repo shell script that writes INTO A DIRECTORY
must be preceded, in the same script, by something that guarantees the directory:
a `mkdir -p` covering it, a `mkdir` naming it exactly, a `mktemp -d`, or a
directory that always exists (`/tmp`, `/dev`). Variables are resolved through
their literal assignments, so hoisting the path into `PKG_LOG=` does not hide it.

The other half of the fix — an else-branch that says "the log does not exist, so
the command DID NOT RUN" rather than `sed`ing it — is a property of the message,
not of the syntax, so it is guarded by a test instead:
`tools/tests/test_check_shell_redirect_dirs.py::test_missing_log_is_reported_honestly`
runs `check-publishable.sh` against a `cargo` that fails and asserts the report
names the real situation.

SCOPE: shell scripts only. Workflow `run:` blocks are prose-adjacent YAML where a
bare `>` is as likely to be a folded-scalar marker as a redirect, and the release
workflow's own redirects go to `$GITHUB_OUTPUT` and `/tmp`.

Exit 0 clean, 1 with one finding per line.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Frozen record, not live tooling (see tools/check-shell-pipe-shortcircuit.py).
EXCLUDED_PREFIXES = ("docs/experiments/",)

# Directories no script has to create.
#
#   * the OS ones exist on every runner and in every image;
#   * `/data` is the itzg server image's own data directory. The one script that
#     writes there (`validation/world-settings-entrypoint.sh`) runs INSIDE that
#     image, where the directory is the image's contract, not the script's — and
#     that file is byte-locked to the heredoc in `validation/Dockerfile.delve`
#     (validation/check-world-settings.sh), so an edit there moves the shipped
#     delve image. A class rule, not a line-number allowlist: it cannot go stale.
ALWAYS_PRESENT = ("/tmp", "/var/tmp", "/dev", "/proc", "/data", ".", "..")

ASSIGN = re.compile(r"^\s*(?:local\s+|export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$")
MKTEMP_DIR = re.compile(r"mktemp\s+(?:[^\s]*\s+)*-d\b|mktemp\s+-d\b")
MKDIR = re.compile(r"\bmkdir\b(?P<p>\s+-p\b)?(?P<args>[^;&|]*)")
HEREDOC_START = re.compile(r"<<-?\s*(['\"]?)([A-Za-z_][A-Za-z0-9_]*)\1")


def git_ls(*patterns: str) -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", *patterns], cwd=REPO, capture_output=True, text=True, check=True
    ).stdout
    return [p for p in out.splitlines() if p]


def shell_files() -> list[str]:
    return [p for p in git_ls("*.sh") if not p.startswith(EXCLUDED_PREFIXES)]


def tracked_dirs() -> set[str]:
    """Directories that exist in any checkout, so no script has to create them."""
    dirs: set[str] = set()
    for path in git_ls():
        parts = path.split("/")[:-1]
        for i in range(1, len(parts) + 1):
            dirs.add("/".join(parts[:i]))
    return dirs


def redirect_targets(code: str) -> list[str]:
    """Every `>`/`>>` target on a line, ignoring `>` that sits inside a quote.

    Quote-awareness is the whole job: `want_in "mineflayer -> harness/package.json"`
    and an awk program `'/cat > \\/delve\\/entrypoint/'` both contain a `>` that is
    text, not a redirection, and a regex that cannot tell the difference reports
    four findings that are not bugs — which is how a gate stops being read.
    """
    targets: list[str] = []
    i, n = 0, len(code)
    single = double = False
    while i < n:
        c = code[i]
        if c == "\\" and not single:
            i += 2
            continue
        if c == "'" and not double:
            single = not single
        elif c == '"' and not single:
            double = not double
        elif c == ">" and not single and not double:
            if i and code[i - 1] in "0123456789&>":
                i += 1
                continue
            i += 2 if code[i : i + 2] == ">>" else 1
            while i < n and code[i] == " ":
                i += 1
            if i < n and code[i] in "\"'":
                q = code[i]
                j = code.find(q, i + 1)
                if j == -1:
                    break
                targets.append(code[i + 1 : j])
                i = j + 1
            else:
                j = i
                while j < n and code[j] not in " \t;&|)<>":
                    j += 1
                if j > i:
                    targets.append(code[i:j])
                i = j
            continue
        i += 1
    return targets


def split_args(text: str) -> list[str]:
    """Split a shell argument list, honouring quotes; drop option flags."""
    return [
        (m.group(1) or m.group(2) or m.group(3))
        for m in re.finditer(r"""(?:"([^"]*)"|'([^']*)'|([^\s]+))""", text.strip())
        if not (m.group(3) or "").startswith("-")
    ]


def literal_assignments(lines: list[str]) -> tuple[dict[str, str], set[str]]:
    """name -> literal value, plus the set of names holding a `mktemp -d` path."""
    values: dict[str, str] = {}
    tempdirs: set[str] = set()
    for line in lines:
        m = ASSIGN.match(line.split("#", 1)[0] if not line.lstrip().startswith("#") else "")
        if not m:
            continue
        name, rhs = m.group(1), m.group(2).strip()
        if MKTEMP_DIR.search(rhs):
            tempdirs.add(name)
            continue
        if "$(" in rhs or "`" in rhs:
            continue  # opaque: keep the `$NAME` token itself as the identity
        values[name] = rhs.strip('"').strip("'")
    return values, tempdirs


def expand(path: str, values: dict[str, str]) -> str:
    for _ in range(4):
        before = path
        path = re.sub(
            r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)",
            lambda m: values.get(m.group(1) or m.group(2), m.group(0)),
            path,
        )
        if path == before:
            return path
    return path


def guaranteed(
    dirname: str,
    mkdirs: list[tuple[bool, str]],
    tempdirs: set[str],
    repo_dirs: set[str],
) -> bool:
    if dirname in ALWAYS_PRESENT or dirname.startswith(("/tmp/", "/var/tmp/", "/dev/", "/data/")):
        return True
    # A directory that is tracked in this repo exists in every checkout.
    if dirname.lstrip("./") in repo_dirs:
        return True
    head = re.match(r"\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?", dirname)
    if head and head.group(1) in tempdirs:
        return True
    for recursive, arg in mkdirs:
        if arg == dirname:
            return True
        # `mkdir -p a/b/c` also creates `a` and `a/b`.
        if recursive and arg.startswith(dirname.rstrip("/") + "/"):
            return True
    return False


def check_file(rel: str, text: str, repo_dirs: set[str]) -> tuple[list[str], int]:
    lines = text.splitlines()
    values, tempdirs = literal_assignments(lines)

    mkdirs: list[tuple[bool, str]] = []
    heredoc: str | None = None
    body: list[tuple[int, str]] = []
    for lineno, raw in enumerate(lines, start=1):
        if heredoc is not None:
            if raw.strip() == heredoc:
                heredoc = None
            continue
        code = "" if raw.lstrip().startswith("#") else raw.split(" #", 1)[0]
        for m in MKDIR.finditer(code):
            for arg in split_args(m.group("args")):
                mkdirs.append((bool(m.group("p")), expand(arg, values)))
        if code.strip():
            body.append((lineno, code))
        start = HEREDOC_START.search(code)
        if start:
            heredoc = start.group(2)

    findings, examined = [], 0
    for lineno, code in body:
        for target in redirect_targets(code):
            if target.startswith("&") or target == "/dev/null":
                continue
            resolved = expand(target, values)
            if "/" not in resolved:
                continue  # writes into the cwd, which exists by definition
            examined += 1
            dirname = resolved.rsplit("/", 1)[0] or "/"
            if not guaranteed(dirname, mkdirs, tempdirs, repo_dirs):
                findings.append(
                    f"{rel}:{lineno}: redirect into `{dirname}`, which nothing in this "
                    f"script guarantees exists\n    {code.strip()}"
                )
    return findings, examined


def main() -> int:
    findings: list[str] = []
    scanned = examined = 0
    repo_dirs = tracked_dirs()
    for rel in shell_files():
        scanned += 1
        f, n = check_file(rel, (REPO / rel).read_text(encoding="utf-8"), repo_dirs)
        findings += f
        examined += n

    # Vacuity guard (CLAUDE.md): state what the gate actually bound to.
    if scanned == 0 or examined == 0:
        print(
            f"check-shell-redirect-dirs: FAIL — vacuous: {scanned} script(s), "
            f"{examined} directory-writing redirect(s)",
            file=sys.stderr,
        )
        return 1

    if findings:
        print(
            f"check-shell-redirect-dirs: {len(findings)} finding(s) across "
            f"{scanned} shell script(s)\n",
            file=sys.stderr,
        )
        for f in findings:
            print(f, file=sys.stderr)
        print(
            "\nThe shell opens a redirect BEFORE running the command it captures. If\n"
            "the directory is missing the redirect fails, the command never runs, and\n"
            "a failure branch that reads the log then reports the wrong thing about\n"
            "the wrong command. `mkdir -p` the directory first — and make the failure\n"
            "branch say so when the log is missing or empty.\n",
            file=sys.stderr,
        )
        return 1

    print(
        f"check-shell-redirect-dirs: OK — {examined} directory-writing redirect(s) "
        f"across {scanned} shell script(s), every target directory guaranteed"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
