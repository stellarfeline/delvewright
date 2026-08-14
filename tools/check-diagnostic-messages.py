#!/usr/bin/env python3
"""Every diagnostic message this engine can print is a whole sentence.

Nothing in this project tested a diagnostic's **message**. `tools/check-dw-codes.py`
proves a code exists, is documented, is unique and is asserted by some test; every
one of those gates is satisfied by a rule whose message reads

    ... so freezing it would put a  on disk whose metadata describes ...

which is what `delve-grammar expand` printed at the moment it refused an author's
export. The dropped noun and the doubled space it left behind survived review,
`cargo test`, `clippy` and twelve required status checks, because the only thing
anyone ever compared was `DW`+four digits.

An author meets a diagnostic exactly once, at the moment their work is refused.
The message is the entire product at that moment. So the message is checked here
the same way the code is checked next door.

## What is scanned (the enumeration; a site shape missing from this list is a
## hole in the gate, not a message that is fine)

Message text reaches a reader through three site shapes, and they are found by
walking `crates/*/src/**/*.rs` as source — which is also how `crates/render`
stays covered, it being a workspace of its own that no `cargo test` at the repo
root ever builds:

1. the last argument of `Diagnostic::error(...)` / `Diagnostic::warning(...)` —
   the compiler's 4-arg form and the schem/admit/render 2-arg form both end with
   the message;
2. a `message:` field initializer in a struct literal (`ParseError { message: … }`);
3. `write!(f, …)` / `writeln!(f, …)` / `f.write_str(…)` inside an
   `impl … Display for T` where `T` is an error type — one this crate gives an
   `impl std::error::Error`, or one whose name ends in `Error`. That shape is the
   reason the gate reaches the motivating instance at all: `ExportError` is an
   error enum, not a `Diagnostic`, and a rule keyed to `Diagnostic::` alone would
   have passed it;
4. the print family (`eprintln!` / `println!` / `eprint!` / `print!`). A CLI's
   refusal is the message an author reads at the moment their run stops, whether it
   travelled through a `Diagnostic` or straight to stderr, and it is the ONLY way
   `delve-orchestrator` refuses anything — key the gate to the `Diagnostic` type
   and that binary sits at a binding count of zero while the gate reports a pass.

Each site's message expression is reduced to its **format template(s)**: the
literal of a `format!` / `write!` / `writeln!`, or a bare literal argument. Two
hops are followed, because the longest diagnostics here are not written inline: a
named helper the expression calls, and a local the message was assembled into with
`let` / `write!` / `push_str`. An expression that still resolves to no template is
either a message value handed on from a site checked where it was BUILT (counted
as forwarded) or REPORTED BY NAME under "not renderable", never silently dropped.
A zero binding count is a failure.

## What is asserted

Each template is *rendered*: escapes decoded (including the `\\`-newline
continuation Rust uses to wrap a long message, which is how the motivating
instance's doubled space came to sit invisibly across two source lines) and every
`{…}` substituted with a non-empty sample. The rendered text must contain none of:

- a **doubled space** inside running prose — the shape a dropped word leaves;
- a **gap after an article** (`a` / `an` / `the` followed by two or more spaces) —
  the same shape where the next thing is a substitution rather than a word, which
  the rule above deliberately cannot judge (see "Alignment is not a hole");
- a **space before punctuation** — the shape a dropped word leaves at a clause end;
- a **dangling article** (`a` / `an` / `the` immediately before punctuation or the
  end of a line) — the shape a dropped noun leaves;
- an **empty quoted span** (`""`, ``` `` ```, `''`, `<>`) — the shape a dropped
  interpolation leaves;
- **empty brackets** (`()`, `[]`) — the shape a dropped parenthetical leaves.

Leading and trailing whitespace is deliberately NOT a rule. Measured over the whole
message set it fires once — on a sentence fragment written to be concatenated onto
the next one — and on no real defect; a rule whose only verdicts are false is worse
than no rule, because it teaches its reader to stop looking.

The one substitution the source cannot decide is an interpolation that is empty at
RUNTIME: `format!("put a {kind} on disk")` with an empty `kind` prints the same
defect this gate exists to catch, and from the source alone every template with a
`{}` in it looks the same. Rendering with a sample is the honest half of that:
prose around a substitution is checked, the substitution's own emptiness is not.
Assuming instead that every substitution may be empty would red several hundred
correct messages (`{id:?}` renders `""`, never nothing), which is a check nobody
could keep.

## Alignment is not a hole

Some messages are report blocks whose rows line a label up against a value with
several spaces. That is deliberate, and it is distinguished by SHAPE rather than
by an author's say-so: a run of spaces is alignment when it is *reproduced* — the
same message lines two or more rows up at the same column. A single doubled space
in one line of prose is never alignment, and no amount of writing it differently
makes it look like one. There is no allowlist and no marker an author can apply:
an escape hatch that a dropped word could itself satisfy would be exactly the
vacuous opt-out CLAUDE.md names.

Deterministic, offline, no dependencies (Python 3 stdlib). Run from the repo root:
    python3 tools/check-diagnostic-messages.py
Exit 0 = every message whole, 1 = holes found (see stderr), 2 = usage/IO error.
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
CRATES_DIR = REPO_ROOT / "crates"

# The sample an interpolation renders as. Deliberately a bare word: it must not
# itself introduce a space, a quote or a bracket, or the gate would report its own
# rendering as a hole.
SAMPLE = "X"


# --------------------------------------------------------------------------
# Rust source, masked
# --------------------------------------------------------------------------


class Source:
    """A Rust file split into a paren-safe mask and the literals it hides.

    `masked` has the same length and the same newlines as `text`, with comment
    bodies and string-literal bodies replaced by filler. Offsets therefore mean
    the same thing in both, so brace/paren matching can run on `masked` while the
    literal at an offset is still recoverable from `text`.
    """

    def __init__(self, path: pathlib.Path, text: str):
        self.path = path
        self.text = text
        self.masked, self.literals = _mask(text)
        self._line_starts = [0] + [m.end() for m in re.finditer(r"\n", text)]

    def line(self, offset: int) -> int:
        lo, hi = 0, len(self._line_starts) - 1
        while lo < hi:
            mid = (lo + hi + 1) // 2
            if self._line_starts[mid] <= offset:
                lo = mid
            else:
                hi = mid - 1
        return lo + 1


def _mask(text: str) -> tuple[str, list[tuple[int, int, bool]]]:
    """Return (masked, literals).

    `literals` is a list of `(start, end, is_raw)`, `start` at the opening quote
    (or at the `r` of a raw literal) and `end` one past the closing quote.
    """
    out: list[str] = []
    lits: list[tuple[int, int, bool]] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        # raw string literal: r"…", r#"…"#, r##"…"##, …
        if c == "r":
            j = i + 1
            while j < n and text[j] == "#":
                j += 1
            if j < n and text[j] == '"':
                close = '"' + "#" * (j - i - 1)
                end = text.find(close, j + 1)
                end = n if end == -1 else end + len(close)
                lits.append((i, end, True))
                out.append(_filler(text[i:end]))
                i = end
                continue
        if c == '"':
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    j += 1
                    break
                j += 1
            lits.append((i, j, False))
            out.append(_filler(text[i:j]))
            i = j
            continue
        if c == "'":
            # A char literal can hide a quote or a brace; a lifetime cannot.
            j = text.find("'", i + 1)
            if j != -1 and j - i <= 5 and "\n" not in text[i:j]:
                out.append(_filler(text[i : j + 1]))
                i = j + 1
                continue
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j == -1 else j
            out.append(_filler(text[i:j]))
            i = j
            continue
        if text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            out.append(_filler(text[i:j]))
            i = j
            continue
        out.append(c)
        i += 1
    return "".join(out), lits


def _filler(chunk: str) -> str:
    """Same length, same newlines, no character a scanner cares about."""
    return "".join("\n" if ch == "\n" else "\x00" for ch in chunk)


def match_delims(masked: str, open_at: int) -> int:
    """Offset one past the delimiter closing the one at `open_at`."""
    pairs = {"(": ")", "[": "]", "{": "}"}
    opener = masked[open_at]
    closer = pairs[opener]
    depth, i, n = 0, open_at, len(masked)
    while i < n:
        ch = masked[i]
        if ch in pairs:
            depth += 1
        elif ch in ")]}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return n


def top_level_args(masked: str, open_paren: int, close_paren: int) -> list[tuple[int, int]]:
    """Spans of the comma-separated arguments between `(` and `)`."""
    spans: list[tuple[int, int]] = []
    depth = 0
    start = open_paren + 1
    for i in range(open_paren + 1, close_paren - 1):
        ch = masked[i]
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        elif ch == "," and depth == 0:
            spans.append((start, i))
            start = i + 1
    spans.append((start, close_paren - 1))
    # Whitespace only — a masked string literal is all filler, and dropping it
    # here is what made `write!(f, "{e}")` look like a one-argument call.
    return [(a, b) for a, b in spans if masked[a:b].strip(" \t\n")]


# --------------------------------------------------------------------------
# Literals -> rendered messages
# --------------------------------------------------------------------------

ESCAPES = {"n": "\n", "t": "\t", "r": "\r", "0": "\0", '"': '"', "\\": "\\", "'": "'"}


def literal_value(src: Source, start: int, end: int, is_raw: bool) -> str:
    raw = src.text[start:end]
    if is_raw:
        body = raw[raw.index('"') + 1 :]
        return body[: body.rindex('"')] if '"' in body else body
    body = raw[1:-1] if len(raw) >= 2 else ""
    out: list[str] = []
    i, n = 0, len(body)
    while i < n:
        if body[i] == "\\" and i + 1 < n:
            nxt = body[i + 1]
            if nxt == "\n":
                # Rust's line continuation: the newline AND the indentation that
                # follows it are dropped. This is why a doubled space can sit
                # across two source lines and read as one space in the file.
                i += 2
                while i < n and body[i] in " \t":
                    i += 1
                continue
            if nxt == "u" and i + 2 < n and body[i + 2] == "{":
                close = body.find("}", i + 2)
                if close != -1:
                    try:
                        out.append(chr(int(body[i + 3 : close], 16)))
                    except ValueError:
                        out.append("?")
                    i = close + 1
                    continue
            out.append(ESCAPES.get(nxt, nxt))
            i += 2
            continue
        out.append(body[i])
        i += 1
    return "".join(out)


PLACEHOLDER_RE = re.compile(r"\{\{|\}\}|\{[^{}]*\}")


def render(template: str) -> str:
    """The template with every `{…}` filled by a non-empty sample."""
    return PLACEHOLDER_RE.sub(
        lambda m: {"{{": "{", "}}": "}"}.get(m.group(0), SAMPLE), template
    )


# --------------------------------------------------------------------------
# The rule: what a whole message may not contain
# --------------------------------------------------------------------------

DANGLING_ARTICLE_RE = re.compile(r"(?:^|(?<=[\s(\[`\"']))(an?|the)(?=[,.;:!?)\]]|\s*$)", re.I)
EMPTY_QUOTED_RE = re.compile(r"``|''|\"\"|<>")
# `actors[]` / `world.areas[].lighting` is how this DSL names an array field, so a
# bracket pair welded to an identifier is notation, not a gap. A gap has nothing
# on its left.
EMPTY_BRACKET_RE = re.compile(r"(?:^|(?<=[\s(\[`\"',]))(?:\(\)|\[\])")
SPACE_BEFORE_PUNCT_RE = re.compile(r"(?<=\S) [,.;:!?](?=\s|$)|(?<=\S) \)")
GAP_AFTER_ARTICLE_RE = re.compile(r"(?:^|(?<=[\s(\[`\"']))(?:an?|the) {2,}", re.I)
BACKTICK_SPAN_RE = re.compile(r"`[^`\n]*`")

# `! " ' ( ) , - . / : ; ?` inside backticks is an author being shown a character
# set, not prose with a gap in it. Prose detectors therefore read the message with
# its code spans blanked; the span's DELIMITERS stay, so an EMPTY one is still a
# finding, and doubled-space still reads the whole line — the defect this gate
# exists for does not hide inside backticks.
PROSE_ONLY = ("space-before-punctuation", "dangling-article")


def _blank_code_spans(line: str) -> str:
    return BACKTICK_SPAN_RE.sub(lambda m: "`" + "x" * (len(m.group(0)) - 2) + "`", line)


def alignment_ends(message: str) -> set[int]:
    """Columns at which this message ENDS a run of spaces more than once.

    A report block lines its values up at one column, so two or more of its rows
    end a space-run at that exact column; the runs themselves start wherever each
    label happens to end and are all different lengths. A dropped word makes a gap
    in exactly one line, at a column no other line reaches. Nothing an author can
    write in a single line earns this, and there is no marker to apply — an opt-out
    a dropped word could itself satisfy would not separate pass from fail
    (CLAUDE.md, sixth vacuity mode).
    """
    ends: dict[int, int] = {}
    for line in message.split("\n"):
        for m in re.finditer(r"(?<=\S) {2,}(?=\S)", line):
            ends[m.end()] = ends.get(m.end(), 0) + 1
    return {col for col, n in ends.items() if n >= 2}


def holes(message: str, also_aligned: set[int] | None = None) -> list[tuple[str, str]]:
    """Every (kind, excerpt) defect in a rendered message. Empty = whole.

    `also_aligned` carries the alignment columns of the sibling messages a report
    block is printed through — a table written as one `println!` per row is one
    table, and no single row can see the others.
    """
    found: list[tuple[str, str]] = []
    aligned = alignment_ends(message) | (also_aligned or set())

    for line in message.split("\n"):
        prose = _blank_code_spans(line)
        # A gap inside running prose: lowercase word on both sides. A report
        # column's gap always abuts a value — a substitution, a number, a padded
        # or capitalised field — so it does not have this shape, and the two are
        # told apart by what surrounds the gap rather than by anything an author
        # declares about it.
        for m in re.finditer(r"(?<=[a-z]) {2,}(?=[a-z])", line):
            if m.end() not in aligned:
                found.append(("doubled-space", _excerpt(line, m.start(), m.end())))
        # The one gap that IS ambiguous by shape — a dropped noun before a
        # substitution — stops being ambiguous after an article, because no report
        # has a column labelled `a`.
        for m in GAP_AFTER_ARTICLE_RE.finditer(line):
            found.append(("gap-after-article", _excerpt(line, m.start(), m.end())))
        for kind, rx in (
            ("space-before-punctuation", SPACE_BEFORE_PUNCT_RE),
            ("dangling-article", DANGLING_ARTICLE_RE),
            ("empty-quoted-span", EMPTY_QUOTED_RE),
            ("empty-brackets", EMPTY_BRACKET_RE),
        ):
            subject = prose if kind in PROSE_ONLY else line
            for m in rx.finditer(subject):
                found.append((kind, _excerpt(line, m.start(), m.end())))
    return found


def _excerpt(line: str, start: int, end: int) -> str:
    lo, hi = max(0, start - 34), min(len(line), end + 34)
    return ("…" if lo else "") + line[lo:hi].replace("\n", "\\n") + ("…" if hi < len(line) else "")


# --------------------------------------------------------------------------
# Site discovery
# --------------------------------------------------------------------------

DIAGNOSTIC_CALL_RE = re.compile(r"\bDiagnostic::(?:error|warning)\s*\(")
MESSAGE_FIELD_RE = re.compile(r"(?<![A-Za-z0-9_.])message\s*:\s*")
DISPLAY_IMPL_RE = re.compile(
    r"\bimpl(?:\s*<[^>]*>)?\s+(?:std::fmt::|fmt::|core::fmt::)?Display\s+for\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)"
)
ERROR_IMPL_RE = re.compile(
    r"\bimpl\s+(?:std::error::|core::error::)?Error\s+for\s+([A-Za-z_][A-Za-z0-9_]*)"
)
WRITE_MACRO_RE = re.compile(r"\b(?:write|writeln)\s*!\s*\(")
WRITE_STR_RE = re.compile(r"\.write_str\s*\(")
TEMPLATE_MACRO_RE = re.compile(r"\b(?:format|write|writeln|format_args)\s*!\s*\(")
PRINT_MACRO_RE = re.compile(r"\b(?:eprintln|println|eprint|print)\s*!\s*\(")
FN_RE = re.compile(r"\bfn\s+[A-Za-z_][A-Za-z0-9_]*")


def error_types(sources: list[Source]) -> set[str]:
    """Types whose `Display` IS a message to a reader.

    Two ways to qualify, because one alone leaves a gap. `impl std::error::Error`
    is the semantic answer and reaches types with any name. A name ending in
    `Error` catches the ones that never implement the trait — `dsl::fmt::ParseError`
    carries a `DW0770`/`DW0771` code and a message and implements nothing — where
    keying on the trait alone would quietly leave a whole error type unread.
    """
    names: set[str] = set()
    for src in sources:
        names.update(ERROR_IMPL_RE.findall(src.masked))
        names.update(m.group(1) for m in DISPLAY_IMPL_RE.finditer(src.masked)
                     if m.group(1).endswith("Error"))
    return names


def first_literal_in(src: Source, lo: int, hi: int) -> tuple[int, int, bool] | None:
    for start, end, is_raw in src.literals:
        if lo <= start < hi:
            return (start, end, is_raw)
    return None


def templates_in_span(src: Source, lo: int, hi: int) -> list[tuple[Source, int, int, bool]]:
    """Every format template the expression in [lo, hi) renders through.

    A `format!` / `write!` / `writeln!` contributes its own first literal; an
    expression that is nothing but a literal contributes itself. Anything else
    contributes nothing here, and the caller tries one hop through a helper before
    reporting the site as unreachable.
    """
    out: list[tuple[Source, int, int, bool]] = []
    for m in TEMPLATE_MACRO_RE.finditer(src.masked, lo, hi):
        open_paren = m.end() - 1
        close = match_delims(src.masked, open_paren)
        lit = first_literal_in(src, open_paren, min(close, hi))
        if lit:
            out.append((src, *lit))
    if not out:
        # A bare literal argument, with or without `.to_string()` / `.into()`.
        stripped = src.masked[lo:hi].strip(" \t\n")
        lit = first_literal_in(src, lo, hi)
        if lit and stripped.startswith(("\x00", '"')):
            out.append((src, *lit))
    return out


# A `message:` that introduces a FIELD rather than filling one. A type is a bare
# path; a message expression always has a call, a macro or a literal in it.
TYPE_ONLY_RE = re.compile(r"^(?:impl\s|dyn\s|&|[A-Z])[\w:<>,'&\s]*$")
CALL_HEAD_RE = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\s*\(")
# A message value handed on from somewhere else — a constructor's own parameter, or
# another diagnostic's `message`. There is nothing to render here; the text was
# written at the site that built it, and that site is checked.
FORWARDED_RE = re.compile(r"^(?:[A-Za-z_][A-Za-z0-9_]*\.)*message$")
CONVERSION_RE = re.compile(r"(?:\.clone\(\)|\.into\(\)|\.to_string\(\)|\.to_owned\(\))+$")


def is_forwarded(text: str) -> bool:
    return bool(FORWARDED_RE.match(CONVERSION_RE.sub("", text.strip())))


def fn_bodies(sources: list[Source]) -> dict[str, list[tuple[Source, int, int]]]:
    """`fn` name -> the bodies that define it, crate-wide.

    A message assembled by a named helper (`clearance_error(…)`, `burn_message(…)`)
    is still a message; without this hop its templates would be counted as
    unreachable and never read, which is how a whole family of the longest
    diagnostics in the compiler would have sat outside the gate.
    """
    out: dict[str, list[tuple[Source, int, int]]] = {}
    for src in sources:
        for m in re.finditer(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)", src.masked):
            paren = src.masked.find("(", m.end())
            if paren == -1:
                continue
            after_params = match_delims(src.masked, paren)
            brace = src.masked.find("{", after_params)
            # A `;` between the parameter list and the next brace means this `fn`
            # is a declaration (trait method, extern) and the brace belongs to
            # something else. The `;` must be looked for THERE and not inside the
            # parameters, where `cell: [i32; 3]` puts one in every signature that
            # takes a coordinate — which silently dropped the compiler's longest
            # diagnostics out of reach.
            if brace == -1 or ";" in src.masked[after_params:brace]:
                continue
            out.setdefault(m.group(1), []).append((src, brace, match_delims(src.masked, brace)))
    return out


BARE_NAME_RE = re.compile(r"^([a-z_][A-Za-z0-9_]*)$")


def local_templates(src: Source, use_at: int, name: str) -> list[tuple[Source, int, int, bool]]:
    """Templates of a message a local variable was assembled from.

    A long diagnostic is written as `let msg = format!(…)` then grown with
    `write!(msg, …)` / `msg.push_str(…)`; the reader still meets one sentence, so
    every piece of it is one message's worth of text.
    """
    body = enclosing_fn(src, use_at)
    if body < 0:
        return []
    end = match_delims(src.masked, body)
    out: list[tuple[Source, int, int, bool]] = []
    esc = re.escape(name)
    for pat in (
        rf"\blet\s+(?:mut\s+)?{esc}\s*(?::[^=;]*)?=",
        rf"\b(?:write|writeln)\s*!\s*\(\s*{esc}\s*,",
        rf"\b{esc}\s*\.\s*push_str\s*\(",
    ):
        for m in re.finditer(pat, src.masked[body:end]):
            at = body + m.end()
            stop = at
            depth = 0
            while stop < end:
                ch = src.masked[stop]
                if ch in "([{":
                    depth += 1
                elif ch in ")]}":
                    if depth == 0:
                        break
                    depth -= 1
                elif ch == ";" and depth == 0:
                    break
                stop += 1
            out.extend(templates_in_span(src, at, stop))
    return out


def enclosing_fn(src: Source, offset: int) -> int:
    """Start offset of the innermost `fn` body containing `offset`, or -1.

    A report table written as one `println!` per row is one table; the rows can
    only be recognised as lining up if they are looked at together, and the unit
    they share is the function that prints them.
    """
    best = -1
    for m in FN_RE.finditer(src.masked):
        brace = src.masked.find("{", m.end())
        if brace == -1 or brace > offset:
            continue
        end = match_delims(src.masked, brace)
        if brace < offset < end and brace > best:
            best = brace
    return best


def sites(
    src: Source, err_types: set[str], helpers: dict[str, list[tuple[Source, int, int]]]
) -> tuple[list[tuple[Source, int, int, bool, str]], list[tuple[int, str]], int]:
    """(rendered-template sites, unreachable sites, forwarded count) for one file."""
    found: list[tuple[Source, int, int, bool, str]] = []
    unreachable: list[tuple[int, str]] = []
    forwarded = 0

    def take(lo: int, hi: int, shape: str) -> None:
        nonlocal forwarded
        text = " ".join(src.text[lo:hi].split())
        templates = templates_in_span(src, lo, hi)
        if not templates:
            call = CALL_HEAD_RE.match(text)
            bare = BARE_NAME_RE.match(CONVERSION_RE.sub("", text))
            if call and call.group(1) in helpers:
                for hsrc, hlo, hhi in helpers[call.group(1)]:
                    templates.extend(templates_in_span(hsrc, hlo, hhi))
                shape = f"{shape} via {call.group(1)}()"
            elif bare:
                templates = local_templates(src, lo, bare.group(1))
                if templates:
                    shape = f"{shape} via let {bare.group(1)}"
        if templates:
            found.extend((s, a, b, r, shape) for s, a, b, r in templates)
        elif TYPE_ONLY_RE.match(text):
            pass  # a field declaration, not a message
        elif is_forwarded(text):
            forwarded += 1
        else:
            unreachable.append((src.line(lo), f"{shape}: {text[:100]}"))

    # 1. Diagnostic::error / Diagnostic::warning — the message is the last argument.
    for m in DIAGNOSTIC_CALL_RE.finditer(src.masked):
        open_paren = m.end() - 1
        close = match_delims(src.masked, open_paren)
        args = top_level_args(src.masked, open_paren, close)
        if args:
            take(args[-1][0], args[-1][1], "Diagnostic::…")

    # 2. `message:` struct-literal field.
    for m in MESSAGE_FIELD_RE.finditer(src.masked):
        lo = m.end()
        depth, i, n = 0, lo, len(src.masked)
        while i < n:
            ch = src.masked[i]
            if ch in "([{":
                depth += 1
            elif ch in ")]}":
                if depth == 0:
                    break
                depth -= 1
            elif ch == "," and depth == 0:
                break
            i += 1
        take(lo, i, "message: field")

    # 3. `impl Display for <error type>` bodies.
    for m in DISPLAY_IMPL_RE.finditer(src.masked):
        if m.group(1) not in err_types:
            continue
        brace = src.masked.find("{", m.end())
        if brace == -1:
            continue
        body_end = match_delims(src.masked, brace)
        for w in WRITE_MACRO_RE.finditer(src.masked, brace, body_end):
            open_paren = w.end() - 1
            close = match_delims(src.masked, open_paren)
            args = top_level_args(src.masked, open_paren, close)
            if len(args) >= 2:
                take(args[1][0], args[1][1], f"Display for {m.group(1)}")
        for w in WRITE_STR_RE.finditer(src.masked, brace, body_end):
            open_paren = w.end() - 1
            close = match_delims(src.masked, open_paren)
            take(open_paren + 1, close - 1, f"Display for {m.group(1)}")

    # 4. The print family. A CLI's refusal is the message an author reads at the
    #    moment their run stops, whether it travelled through a `Diagnostic` or
    #    straight to stderr — `delve-orchestrator` refuses ONLY this way, and
    #    keying the gate to the `Diagnostic` type would have left that binary at a
    #    binding count of zero while reporting a pass.
    for m in PRINT_MACRO_RE.finditer(src.masked):
        open_paren = m.end() - 1
        close = match_delims(src.masked, open_paren)
        args = top_level_args(src.masked, open_paren, close)
        if args:
            take(args[0][0], args[0][1], "print family")

    return found, unreachable, forwarded


# --------------------------------------------------------------------------


def crate_dirs() -> list[pathlib.Path]:
    return sorted(p for p in CRATES_DIR.iterdir() if (p / "src").is_dir())


def main() -> int:
    if not CRATES_DIR.is_dir():
        print(f"error: crates dir not found: {CRATES_DIR}", file=sys.stderr)
        return 2

    per_crate: dict[str, int] = {}
    failures: list[str] = []
    unreachable_all: list[str] = []
    rendered_total = 0
    forwarded_total = 0

    for crate in crate_dirs():
        sources = [
            Source(rs, rs.read_text(encoding="utf-8"))
            for rs in sorted((crate / "src").rglob("*.rs"))
        ]
        err_types = error_types(sources)
        helpers = fn_bodies(sources)
        count = 0
        # (Source, start, shape, rendered message), deduplicated: a helper reached
        # from three call sites is still one message.
        rendered: dict[tuple[str, int], tuple[Source, str, str]] = {}
        for src in sources:
            found, unreachable, fwd = sites(src, err_types, helpers)
            forwarded_total += fwd
            rel = src.path.relative_to(REPO_ROOT)
            for line, what in unreachable:
                unreachable_all.append(f"{rel}:{line}: {what}")
            for tsrc, start, end, is_raw, shape in found:
                rendered.setdefault(
                    (str(tsrc.path), start),
                    (tsrc, shape, render(literal_value(tsrc, start, end, is_raw))),
                )

        # Sibling messages of one function are one table when they line up.
        group_of = {
            key: (str(tsrc.path), enclosing_fn(tsrc, key[1]))
            for key, (tsrc, _, _) in rendered.items()
        }
        group_text: dict[tuple[str, int], list[str]] = {}
        for key, (_, _, message) in rendered.items():
            group_text.setdefault(group_of[key], []).append(message)
        group_aligned = {g: alignment_ends("\n".join(msgs)) for g, msgs in group_text.items()}

        for key, (tsrc, shape, message) in sorted(rendered.items()):
            count += 1
            rel = tsrc.path.relative_to(REPO_ROOT)
            for kind, excerpt in holes(message, group_aligned[group_of[key]]):
                failures.append(f"{rel}:{tsrc.line(key[1])} [{shape}] {kind}: {excerpt}")
        per_crate[crate.name] = count
        rendered_total += count

    binding = ", ".join(f"{k} {v}" for k, v in sorted(per_crate.items()))

    if rendered_total == 0:
        print(
            "diagnostic-message check FAILED: 0 messages rendered — the site "
            "patterns match nothing, so this gate binds to nothing (CLAUDE.md: a "
            "green gate that binds to nothing is vacuous, not a pass)",
            file=sys.stderr,
        )
        return 1

    if failures:
        print(
            f"diagnostic-message check FAILED: {len(failures)} hole(s) in "
            f"{rendered_total} rendered message(s) ({binding}):",
            file=sys.stderr,
        )
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        print(
            "\nA reader meets one of these at the moment their work is refused. "
            "Rewrite the message; do not exempt it.",
            file=sys.stderr,
        )
        return 1

    print(
        f"diagnostic messages OK: {rendered_total} rendered and whole "
        f"({binding}); {forwarded_total} forwarded from a site checked where it "
        f"was built; {len(unreachable_all)} not renderable from source:"
    )
    for u in unreachable_all:
        print(f"  ? {u}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
