#!/usr/bin/env python3
r"""Repo shell runs on the shell the creator actually has, which is bash 3.2.

WHY THIS EXISTS

`CLAUDE.md`: *every validation authoring needs must be runnable on the creator's
own machine, and this is not negotiable*, with the floor at "clone the repo and
build from source". Dev is a macOS workstation, and **macOS ships bash 3.2.57** —
frozen at the last GPLv2 release. `mapfile` and `readarray` are bash 4.0
builtins and are not there.

A missing builtin does not fail loudly. Measured on this machine's
`/bin/bash` (3.2.57), `mapfile -t a </dev/null; echo ok` prints

    bash: mapfile: command not found
    ok

and **exits 0** with `a` empty. That is what shipped: `validation/packtest-all.sh`
— the script whose whole job is to run every PackTest project — used `mapfile`,
covered ZERO projects, printed, and exited 0. A gate that ran and examined
nothing.

THE LESSON WAS RECORDED SEVEN TIMES, IN SEVEN COMMENTS

`tools/fmt-workspaces.sh`, `tools/build-release-binaries.sh`,
`tools/check-publishable.sh`, `tools/lib/rcon.sh`, `validation/packtest-all.sh`,
`validation/check-versions.sh`, `validation/branch-runs.sh` each carry a comment
saying "not this, macOS ships bash 3.2". `CLAUDE.md` ranks the forms a debugging
lesson may take — *compiler diagnostic > tooling default > generator invariant >
docs* — and a comment is the bottom rung, chosen seven times. This is the rung
above: an ordinary red on the `checks` job.

THE CLASS, AND HOW EACH MEMBER WAS ESTABLISHED

The class is **syntax and builtins bash 3.2 does not have**, not the one string
everybody remembers. Every member below was established by running it on this
machine's `/bin/bash` 3.2.57 *and* on `bash:5` in Docker — a different binary on
a different OS, so the two instruments share no calibration. A member that fails
on both would be a typo rather than a version boundary, and a member that passes
on 3.2 does not belong here. The transcript is in the pull request; the columns
that matter are reproduced per member below as `3.2: <what it does>`.

The dangerous half of the class is the members that **do not abort**: `mapfile`,
`readarray`, `coproc`, `declare -A|-n|-l|-u|-g`, `wait -n`, `printf -v a[0]` and
`${a[-1]}` all print a diagnostic to stderr and let the script carry on with a
wrong value. The rest (`${v,,}`, `&>>`, `|&`, `;&`, `;;&`, `[[ -v ]]`, `${v@Q}`,
`shopt -s globstar`, `exec {fd}<`) abort. Both halves are refused: the first
because it is silent, the second because it is a crash on the creator's machine
and green on every runner.

`shopt` is checked as a CLOSED WORLD rather than as a list of bash-4 names: the
34 option names bash 3.2.57 knows were read out of that shell (`shopt`), and
anything else is by construction newer. A member list would have to be extended
for every future bash; this cannot go stale.

THE POPULATION

Derived, never hand-listed — a hand-written list in this repository named 9 of 10
cargo workspaces and, in the content repository, excluded 27 tracked files:

  * every tracked file whose name ends `.sh`/`.bash`, UNION every tracked file
    whose first line is a `bash`/`sh` shebang. The union is the point: a script
    with a shebang and no extension is shell, and a `.sh` fragment meant to be
    sourced has no shebang.
  * every tracked `*.yml`/`*.yaml` under `.github/`, plus any tracked
    `action.yml`/`action.yaml` anywhere — read for `run:` blocks. This follows
    `tools/check-python-shell-newlines.py`, which covers workflow `run:` for the
    same reason: `.github/workflows/engine-release.yml` builds two
    `*-apple-darwin` targets on `macos-latest`, whose `/bin/bash` is 3.2, and
    which bash an Actions step resolves is a property of a runner image this
    repository neither pins nor controls. Relying on it is the unstated premise
    this project keeps paying for.

Counts are printed every run, against the tracked-file population they were drawn
from, so a truncated input cannot read as a clean pass.

WHAT IS NOT READ, AND WHY — QUOTES AND HEREDOCS

**Single-quoted spans are blanked.** bash performs no expansion inside them, so
`echo '${v,,}'` is text; and a command word inside them is a program for
something else — an `awk` script, a `python3 -c`, a `bash -c` sent into a
container. Double-quoted spans ARE read, because `"${v,,}"` is a real use. The
quoting is tracked across lines by a scanner rather than per line, which is also
what keeps `validation/check-world-settings.sh`'s message string — it contains a
literal `<<EOS` inside double quotes — from being mistaken for a heredoc.

**Comments are stripped, quote-aware, and `#` only opens one at a word start.**
This is load-bearing twice over: every one of the seven recorded instances is a
comment *saying not to use `mapfile`*, so a naive grep reds exactly the lines
that record the lesson; and `${v#prefix}` is not a comment.

**Heredoc bodies are not read — fail-open, deliberately.** A heredoc body is data
handed to another program, not shell the enclosing bash executes. Every heredoc
in this repository's shell (measured: 40 of them) feeds `python3`, writes JSON,
JS or a `config.toml`, prints usage text, or feeds a `while read` loop; none is
bash. Reading them would flag python and JS as shell, which is how a gate stops
being read. The bodies are COUNTED and printed, so the size of what is skipped is
never invisible.

The one case where fail-open would be wrong is closed rather than hoped away: a
heredoc that WRITES A SHELL SCRIPT (`cat >x.sh <<EOF`) or that IS PIPED INTO A
SHELL (`bash <<EOF`) is a finding on the spot, since this checker cannot read it.
It binds nothing today, which is stated rather than assumed. (`validation/
Dockerfile.delve` writes an entrypoint from a heredoc, but a Dockerfile is not in
the population and that script runs inside a pinned Linux image, not on the
creator's machine.)

Exit 0 clean, 1 with one finding per line.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple

REPO = Path(__file__).resolve().parent.parent


class Masks(NamedTuple):
    """One source line, three times, differing only in what quoting removed.

    Same length as the original line, so an offset into any of them is an offset
    into the source.
    """

    bare: str  # nothing quoted survives — for COMMANDS
    code: str  # double-quoted text survives — for EXPANSIONS
    text: str  # all quoted text survives — for LITERALS

# Frozen record, not live tooling: docs/experiments/ preserves an experiment
# exactly as it was run. Rewriting it would falsify the record, and nothing in CI
# or the authoring loop executes it. Same prefix and same words as
# tools/check-shell-pipe-shortcircuit.py, tools/check-shell-redirect-dirs.py and
# tools/fmt-workspaces.sh — and, like fmt-workspaces, it is COUNTED every run: an
# exclusion that matches zero tracked files is a refusal, because a stale
# exclusion is a blind spot wearing a rule's clothes.
EXCLUDED_PREFIXES = ("docs/experiments/",)

SHEBANG = re.compile(r"^#!\s*(?:\S*/)?(?:env\s+)?(?:bash|sh)\b")

# The 34 `shopt` option names bash 3.2.57 knows, read out of that shell itself
# (`/bin/bash -c shopt`). Closed world: `shopt -s <anything else>` is a bash-4+
# option, so this cannot go stale as bash grows. 3.2 exits 1 with "invalid shell
# option name" — under `set -e` that ends the script.
BASH32_SHOPTS = frozenset(
    """cdable_vars cdspell checkhash checkwinsize cmdhist compat31 dotglob
    execfail expand_aliases extdebug extglob extquote failglob force_fignore
    gnu_errfmt histappend histreedit histverify hostcomplete huponexit
    interactive_comments lithist login_shell mailwarn no_empty_cmd_completion
    nocaseglob nocasematch nullglob progcomp promptvars restricted_shell
    shift_verbose sourcepath xpg_echo""".split()
)

# `declare`/`typeset`/`local` options bash 3.2 does not have. 3.2's usage line is
# `declare [-afFirtx] [-p]`, and each of these prints "invalid option" and
# CONTINUES, leaving the variable with ordinary scalar/indexed semantics.
# `export -n` is real in 3.2, which is why `export` is not in the command set.
BAD_DECLARE_FLAGS = {
    "A": "associative arrays are bash 4.0",
    "n": "namerefs are bash 4.3",
    "l": "`declare -l` (lowercase attribute) is bash 4.0",
    "u": "`declare -u` (uppercase attribute) is bash 4.0",
    "g": "`declare -g` (force global) is bash 4.2",
}
DECLARE = re.compile(r"(?<![\w-])(declare|typeset|local)((?:\s+-[A-Za-z]+)+)")

SHOPT = re.compile(r"(?<![\w-])shopt\s+(?:-[qp]\s+)*-[su]\s+([A-Za-z_][A-Za-z0-9_]*)")

# Two catalogues, because a double-quoted span means opposite things to them.
#
# COMMANDS and OPERATORS are only themselves at a word position OUTSIDE quotes:
# `echo "no mapfile here"` is a string, and flagging it would red a sentence.
#
# EXPANSIONS are expanded INSIDE double quotes — `"${v,,}"` is the ordinary way
# to write the bug — so they are read from a mask that keeps double-quoted text.
#
# Single-quoted text is in neither: bash expands nothing there, and a command
# word inside it is a program for something else (`awk`, `python3 -c`, a
# `bash -c` sent into a container).
#
# Each entry records what bash 3.2 actually does with it, measured rather than
# recalled.
COMMANDS: tuple[tuple[re.Pattern[str], str], ...] = (
    (
        re.compile(r"(?<![\w./-])(?:mapfile|readarray)(?![\w./-])"),
        "`mapfile`/`readarray` are bash 4.0 builtins; 3.2 prints "
        "`command not found`, LEAVES THE ARRAY EMPTY and carries on at status 0 "
        "— use `while IFS= read -r line; do arr+=(\"$line\"); done < <(...)`",
    ),
    (
        re.compile(r"(?<![\w./-])coproc(?![\w./-])"),
        "`coproc` is bash 4.0; 3.2 prints `command not found` and continues",
    ),
    (
        re.compile(r"(?<![\w./-])wait\s+-n(?![\w-])"),
        "`wait -n` is bash 4.3; 3.2 prints `invalid option` and waits for nothing",
    ),
    (
        re.compile(r"&>>"),
        "`&>>` (append stdout+stderr) is bash 4.0; 3.2 is a syntax error — use "
        "`>>file 2>&1`",
    ),
    (
        re.compile(r"\|&"),
        "`|&` (pipe stderr too) is bash 4.0; 3.2 is a syntax error — use "
        "`2>&1 |`",
    ),
    (
        re.compile(r";;?&"),
        "`;&` / `;;&` case fall-through is bash 4.0; 3.2 is a syntax error",
    ),
    (
        re.compile(r"\[\[\s+-v\s"),
        "`[[ -v name ]]` is bash 4.2; 3.2 is a syntax error — use "
        "`[[ -n ${name+x} ]]`",
    ),
    (
        re.compile(r"(?:^|[\s;&|(])\{[A-Za-z_][A-Za-z0-9_]*\}[<>]"),
        "`{fd}<file` automatic descriptor allocation is bash 4.1; 3.2 tries to "
        "execute a file named `{fd}`",
    ),
)

# Read from the mask that KEEPS double-quoted text, because that is where these
# are written: `"${v,,}"`, `printf -v t "%(%Y)T"`.
EXPANSIONS: tuple[tuple[re.Pattern[str], str], ...] = (
    (
        # ${v,,} ${v^^} ${v,} ${v^} ${arr[@]^^} — a `,`/`^` immediately after the
        # parameter (and its optional subscript) is the case-modification
        # operator, which does not exist in 3.2 at all.
        re.compile(r"\$\{[#!]?[A-Za-z_][A-Za-z0-9_]*(?:\[[^]{}]*\])?[,^]"),
        "`${v,,}` / `${v^^}` case modification is bash 4.0; 3.2 aborts the whole "
        "script with `bad substitution` — use `tr '[:upper:]' '[:lower:]'`",
    ),
    (
        # ${v@Q} and friends. `${!prefix@}` IS valid in 3.2, so a letter after the
        # `@` is required.
        re.compile(r"\$\{[#!]?[A-Za-z_@*][A-Za-z0-9_]*(?:\[[^]{}]*\])?@[A-Za-z]\}"),
        "`${v@Q}` parameter transformation is bash 4.4; 3.2 aborts with "
        "`bad substitution`",
    ),
    (
        re.compile(r"\$\{[#!]?[A-Za-z_][A-Za-z0-9_]*\[\s*-\s*\d"),
        "a negative array subscript is bash 4.2; 3.2 prints "
        "`bad array subscript` and expands to EMPTY",
    ),
    (
        re.compile(r"printf\s+(?:[^|;&\n]*\s)?-v\s+[A-Za-z_][A-Za-z0-9_]*\s*\["),
        "`printf -v arr[i]` is bash 4.1; 3.2 refuses it as `not a valid "
        "identifier` and prints to stdout instead",
    ),
)

# Read from the mask that keeps ALL quoted text. bash's own `printf` interprets
# its format string whether it is quoted or not, so `printf '%(%Y)T'` is bash-4
# syntax living inside single quotes — the one construct here that the
# single-quote rule would otherwise put out of reach. The `printf` word itself
# must still be a real command word, which is what keeps this off a python
# `print("%(name)s" % d)` inside a heredoc… and heredocs are not read anyway.
LITERALS: tuple[tuple[re.Pattern[str], str], ...] = (
    (
        re.compile(r"(?:^|[\s;&|(])printf\s[^|;&\n]*%\([^)\n]*\)T"),
        "`printf '%(fmt)T'` is bash 4.2; 3.2 refuses `(` as a format character "
        "and prints the format instead — use `date +fmt`",
    ),
)

HEREDOC = re.compile(r"<<-?\s*(?:(['\"])([^'\"]+)\1|([A-Za-z_][A-Za-z0-9_]*))")
# A heredoc this checker must NOT skip: its body is a shell script.
WRITES_SHELL = re.compile(r">\s*[\"']?[^\s\"';|&]*\.(?:sh|bash)\b")
FEEDS_SHELL = re.compile(r"(?:^|[\s;&|(])(?:ba|z|k)?sh(?:\s+-[A-Za-z]+)*\s*<<")

# `${{ ... }}` is a GitHub Actions expression, evaluated before any shell sees
# the line. Blanked so its contents are never read as shell.
GH_EXPR = re.compile(r"\$\{\{[^}]*\}\}")


def git_ls(*patterns: str) -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", *patterns], cwd=REPO, capture_output=True, text=True, check=True
    ).stdout
    return [p for p in out.splitlines() if p]


def first_line(rel: str) -> str:
    try:
        with (REPO / rel).open("rb") as fh:
            return fh.readline(400).decode("utf-8", "replace")
    except OSError:
        return ""


def population() -> tuple[list[str], list[str], list[str], int]:
    """(shell scripts, workflow files, excluded, tracked total) — all derived."""
    tracked = git_ls()
    shell, workflows, excluded = [], [], []
    for rel in tracked:
        name = rel.rsplit("/", 1)[-1]
        is_shell = rel.endswith((".sh", ".bash")) or bool(SHEBANG.match(first_line(rel)))
        is_workflow = (
            rel.startswith(".github/") and rel.endswith((".yml", ".yaml"))
        ) or name in ("action.yml", "action.yaml")
        if not (is_shell or is_workflow):
            continue
        if rel.startswith(EXCLUDED_PREFIXES):
            excluded.append(rel)
            continue
        (shell if is_shell else workflows).append(rel)
    return shell, workflows, excluded, len(tracked)


RUN_KEY = re.compile(r"^(\s*)(?:-\s+)?run:\s*(?:([|>][-+]?\d*)\s*)?(.*)$")
YAML_KEY = re.compile(r"^\s*[A-Za-z_][A-Za-z0-9_-]*:(?:\s|$)")


def run_blocks(lines: list[str]) -> tuple[list[tuple[int, list[str]]], int]:
    """Every `run:` block in a workflow file, and the count that were not shell.

    Returns (1-based lineno of the first body line, body) per block. The second
    number exists because `run:` is TWO keys in the Actions schema: a step's shell,
    and `defaults.run`'s configuration mapping (`working-directory:`), of which
    this repository has two. Telling them apart by "is the indented block a
    mapping" rather than by looking for `defaults:` above keeps a real block from
    ever being silently dropped — the caller asserts key count == blocks + config,
    so the two can never drift without a red.
    """
    blocks: list[tuple[int, list[str]]] = []
    config = 0
    i = 0
    while i < len(lines):
        m = RUN_KEY.match(lines[i])
        if not m:
            i += 1
            continue
        indent, scalar, inline = len(m.group(1)), m.group(2), m.group(3)
        if scalar is None and inline:
            blocks.append((i + 1, [inline]))
            i += 1
            continue
        body, j = [], i + 1
        while j < len(lines):
            line = lines[j]
            if line.strip() and (len(line) - len(line.lstrip())) <= indent:
                break
            body.append(line)
            j += 1
        populated = [b for b in body if b.strip()]
        if scalar is None and populated and all(YAML_KEY.match(b) for b in populated):
            config += 1
        else:
            blocks.append((i + 2, body))
        i = j
    return blocks, config


def scan(lines: list[str]) -> tuple[list[tuple[int, Masks]], int, list[tuple[int, str]]]:
    """Strip comments and heredocs; emit the three masks the catalogues need.

    Returns (per-line masks, heredoc bodies skipped, unread-input findings).
    Quoting state is carried ACROSS lines: a multi-line single-quoted `awk`
    program is one span, not three unquoted ones.

    THREE MASKS, because "is this quoted" has three different answers depending
    on who consumes the characters:

      * `bare` — everything quoted is blanked. A COMMAND is only a command at a
        word position outside quotes, so `echo "no mapfile here"` is a sentence
        and reddening it is how a gate stops being read.
      * `code` — double-quoted text kept. An EXPANSION happens inside double
        quotes; `"${v,,}"` is the ordinary way to write the bug.
      * `text` — all quoted text kept. bash's own `printf` interprets its format
        whether or not it is quoted, so `printf '%(%Y)T'` is bash-4 syntax
        sitting inside single quotes.

    A heredoc START is recognised inside the lexer, at a point where the scanner
    KNOWS it is not inside quotes — never by a regex over the finished line. The
    difference is not theoretical: `validation/check-world-settings.sh` fails with
    the message `"Dockerfile.delve: no 'RUN cat > /delve/entrypoint.sh <<EOS'
    heredoc found"`, which a regex reads as a heredoc that writes a shell script,
    and which then swallows the remaining 200 lines of the file as its body.

    Command substitution RESTARTS quoting, so `$(` pushes the enclosing state and
    the body is lexed as fresh shell. Four scripts here open a heredoc inside
    `"$( … <<'PY' … )"`; without the push, the opener sits inside a double-quoted
    span, the scanner never sees it, and ~100 lines of python get read as shell.
    """
    masks: list[tuple[int, Masks]] = []
    heredocs_skipped = 0
    shell_heredocs: list[tuple[int, str]] = []
    state = ""  # "", "'", '"'
    stack: list[str] = []  # enclosing states, one per open `$(`
    pending: list[str] = []  # heredoc delimiters awaiting their bodies
    i = 0
    while i < len(lines):
        raw = lines[i]
        if pending:
            if raw.strip() == pending[0]:
                pending.pop(0)
            i += 1
            continue
        bare: list[str] = []
        out: list[str] = []
        text: list[str] = []
        starts: list[tuple[str, int]] = []  # (delimiter, offset of the `<<`)
        j, n = 0, len(raw)

        def emit(bare_c: str, out_c: str, text_c: str) -> None:
            bare.append(bare_c)
            out.append(out_c)
            text.append(text_c)

        while j < n:
            c = raw[j]
            if state == "'":
                if c == "'":
                    state = ""
                    emit(" ", " ", " ")
                else:
                    emit(" ", " ", c)
                j += 1
                continue
            if c == "\\" and j + 1 < n:
                emit("  ", "  ", "  ")
                j += 2
                continue
            if raw[j : j + 3] == "$((":
                # Arithmetic, not a subshell: copy it through so its `))` cannot be
                # mistaken for the close of a `$(`.
                end = raw.find("))", j)
                end = n if end < 0 else end + 2
                chunk = raw[j:end]
                emit(chunk, chunk, chunk)
                j = end
                continue
            if raw[j : j + 2] == "$(":
                stack.append(state)
                state = ""
                emit("$(", "$(", "$(")
                j += 2
                continue
            if c == ")" and state == "" and stack:
                state = stack.pop()
                emit(")", ")", ")")
                j += 1
                continue
            if state == '"':
                if c == '"':
                    state = ""
                    emit(" ", " ", " ")
                else:
                    emit(" ", c, c)
                j += 1
                continue
            if c in "'\"":
                state = c
                emit(" ", " ", " ")
                j += 1
                continue
            if c == "#" and (not bare or bare[-1] in " \t;|&(){}"):
                pad = " " * (n - j)
                emit(pad, pad, pad)
                break
            if raw[j : j + 3] == "<<<":
                # A here-STRING, not a heredoc, and the distinction is not a nicety:
                # left to fall through one `<` at a time, `read … <<< "$row"` is read
                # as a heredoc whose delimiter is `$row`, which never arrives — so
                # the remaining 51 lines of `validation/packtest-all.sh` were skipped
                # in silence. Truncation fakes coverage in the direction that reads
                # as a clean pass, which is the same defect this file exists to stop.
                emit("   ", "   ", "   ")
                j += 3
                continue
            if raw[j : j + 2] == "<<":
                m = HEREDOC.match(raw, j)
                if m:
                    starts.append((m.group(2) or m.group(3), j))
                    pad = " " * (m.end() - j)
                    emit(pad, pad, pad)
                    j = m.end()
                    continue
            emit(c, c, c)
            j += 1
        line = Masks("".join(bare)[:n], "".join(out)[:n], "".join(text)[:n])
        if state == "" and not raw.rstrip().endswith("\\"):
            for delim, at in starts:
                pending.append(delim)
                heredocs_skipped += 1
                if WRITES_SHELL.search(line.code[:at]) or FEEDS_SHELL.search(
                    line.code[:at] + "<<"
                ):
                    shell_heredocs.append(
                        (
                            i + 1,
                            f"a heredoc whose body is a SHELL SCRIPT — this checker "
                            f"does not read heredoc bodies, so nothing judges that "
                            f"shell. Put it in a `.sh` file.\n    {raw.strip()}",
                        )
                    )
        masks.append((i + 1, line))
        i += 1
    if pending:
        # An unterminated heredoc means every line after it was skipped. Whatever
        # the cause — a delimiter this scanner mis-read, or a genuinely broken
        # script — the honest report is "I did not read the rest of this file",
        # never a quiet pass over a truncated input.
        shell_heredocs.append(
            (
                len(lines),
                f"unterminated heredoc(s) {pending!r}: everything after them was "
                f"NOT read",
            )
        )
    return masks, heredocs_skipped, shell_heredocs


def constructs(line: Masks) -> list[str]:
    """Every bash-4+ construct on one line, each catalogue read from its own mask."""
    hits: list[str] = []
    for pattern, why in COMMANDS:
        if pattern.search(line.bare):
            hits.append(why)
    for pattern, why in EXPANSIONS:
        if pattern.search(line.code):
            hits.append(why)
    for pattern, why in LITERALS:
        if pattern.search(line.text):
            hits.append(why)
    for m in DECLARE.finditer(line.bare):
        for flag, why in BAD_DECLARE_FLAGS.items():
            if flag in m.group(2):
                hits.append(f"`{m.group(1)} -{flag}`: {why}")
    for m in SHOPT.finditer(line.bare):
        if m.group(1) not in BASH32_SHOPTS:
            hits.append(
                f"`shopt … {m.group(1)}` is not one of the 34 options bash 3.2 has; "
                f"3.2 exits 1 with `invalid shell option name`"
            )
    return hits


def check(rel: str, lines: list[str], offset: int = 0) -> tuple[list[str], int, int]:
    findings: list[str] = []
    masks, skipped, unread = scan(lines)
    for lineno, why in unread:
        findings.append(f"{rel}:{lineno + offset}: {why}")
    for lineno, line in masks:
        for why in constructs(line):
            findings.append(
                f"{rel}:{lineno + offset}: {why}\n    {lines[lineno - 1].strip()}"
            )
    return findings, skipped, len(masks)


def main() -> int:
    shell, workflows, excluded, tracked_total = population()

    findings: list[str] = []
    heredocs = 0
    lines_read = 0

    for rel in shell:
        lines = (REPO / rel).read_text(encoding="utf-8").splitlines()
        f, skipped, n = check(rel, lines)
        findings += f
        heredocs += skipped
        lines_read += n

    blocks = 0
    for rel in workflows:
        lines = (REPO / rel).read_text(encoding="utf-8").splitlines()
        found, config = run_blocks(lines)
        keys = sum(1 for line in lines if RUN_KEY.match(line))
        if keys != len(found) + config:
            # Truncation fakes coverage, and it fakes it in the direction that
            # reads as a clean pass. A `run:` this parser neither ran nor
            # classified is a hole in the sweep, not a quiet skip.
            findings.append(
                f"{rel}: {keys} `run:` key(s) but {len(found)} shell block(s) + "
                f"{config} config mapping(s) — this parser lost one, so the sweep "
                f"is smaller than it claims"
            )
        for start, body in found:
            blocks += 1
            cleaned = [GH_EXPR.sub(lambda m: " " * len(m.group(0)), b) for b in body]
            f, skipped, n = check(rel, cleaned, offset=start - 1)
            findings += f
            heredocs += skipped
            lines_read += n

    # Vacuity guards (CLAUDE.md: a green gate that binds to nothing is not a pass).
    # Four counts, because each can silently go to zero for a different reason: the
    # two halves of the population, the lines actually read after stripping, and
    # the exclusion — an exclusion that matches nothing is a stale constant, which
    # is how a sweep quietly starts covering a smaller world than it claims.
    if not shell or not workflows or not lines_read:
        print(
            f"check-shell-bash32: FAIL — vacuous: {len(shell)} shell script(s), "
            f"{len(workflows)} workflow file(s), {lines_read} line(s) of shell read",
            file=sys.stderr,
        )
        return 1
    if not excluded:
        print(
            f"check-shell-bash32: FAIL — EXCLUDED_PREFIXES={EXCLUDED_PREFIXES} matches "
            f"zero tracked files. Either the prefix moved (the sweep is now claiming "
            f"coverage it does not have) or it is gone (delete the constant).",
            file=sys.stderr,
        )
        return 1

    if findings:
        print(
            f"check-shell-bash32: {len(findings)} finding(s) across "
            f"{len(shell)} shell script(s) and {len(workflows)} workflow file(s)\n",
            file=sys.stderr,
        )
        for f in findings:
            print(f, file=sys.stderr)
        print(
            "\nDev is macOS and macOS ships bash 3.2.57, so this is what a creator\n"
            "runs. Half of these do not even fail: `mapfile` prints `command not\n"
            "found`, leaves the array EMPTY and exits 0 — which is how the script\n"
            "that runs every PackTest project came to run none of them.\n",
            file=sys.stderr,
        )
        return 1

    print(
        f"check-shell-bash32: OK — {lines_read} line(s) of shell across "
        f"{len(shell)} script(s) and {blocks} workflow `run:` block(s) in "
        f"{len(workflows)} file(s), drawn from {tracked_total} tracked files; "
        f"{len(excluded)} excluded by prefix, {heredocs} heredoc body(ies) not read; "
        f"no bash-4-only construct"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
