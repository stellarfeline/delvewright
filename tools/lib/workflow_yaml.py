"""The one place a repository gate parses a GitHub Actions workflow file.

WHY THIS EXISTS

A gate over `.github/workflows/*.yml` has to answer a STRUCTURAL question — for
this job, in order, what are its steps — and the checkers already in this tree
answer structural questions with line regexes because there is no YAML parser
here to reach for: CI checks are Python, stdlib-only (ADR-0018 §2), and the
stdlib has no YAML. `check-required-contexts.py` keys off `^    name:` at
exactly four spaces; `check-shell-bash32.py` re-derives `run:` blocks from
indentation. Each is a private copy of a partial parse rule, and this project's
own rule is that a checker reads a document the way its consumer reads it, with
ONE shared parse rule and no private copy per gate.

So this is that rule, extracted rather than copied. It is a parser for the
BLOCK-YAML SUBSET GitHub Actions files are written in, and its defining property
is that it REFUSES what it does not understand instead of guessing:

- anchors (`&a`), aliases (`*a`), merge keys (`<<`), explicit tags (`!x`),
  explicit keys (`? `), multi-document streams, folded scalars (`>`), and flow
  collections that do not close on their own line all raise `WorkflowYamlError`;
- a tab in indentation raises;
- an unexpected indent raises.

A gate built on it therefore cannot silently misread a construct into a clean
pass — the unbound-gate shape. If a workflow ever grows one of the refused
constructs, the gate reds and names the line, and this file grows the support
deliberately.

WHAT IT DELIBERATELY DOES NOT DO: scalar type resolution. Every plain scalar
comes back as `str` (an empty value comes back as `None`). YAML 1.1 resolves
`on`, `no`, `off`, `y` and `false` to booleans and `0` to an integer, and a
CI checker that needs `jobs` never needs those distinctions — while getting
them subtly wrong would be exactly the silent misread above. `tools/tests/
test_check_approval_guard.py` cross-checks this parser against a real
implementation of the format (Ruby's Psych) over a committed corpus, comparing
both sides after the same stringifying normalisation, so the structure is
verified against something that is not us.
"""

from __future__ import annotations

import re
from typing import Any

__all__ = ["WorkflowYamlError", "load", "normalize"]


class WorkflowYamlError(ValueError):
    """A construct this parser refuses to guess at, with the line that carries it."""


_BLOCK_SCALAR = re.compile(r"^(?P<style>[|>])(?P<chomp>[-+]?)(?P<indent>\d*)$")
_REFUSED_LEADERS = (
    ("&", "an anchor"),
    ("*", "an alias"),
    ("!", "an explicit tag"),
    ("? ", "an explicit key"),
)


def load(text: str) -> Any:
    """Parse one workflow document. Raises `WorkflowYamlError` on anything refused."""
    return _Parser(text).parse_document()


def normalize(node: Any) -> Any:
    """Stringify every scalar, recursively, leaving `None` alone.

    Both sides of the Psych cross-check go through this, so the comparison is
    about STRUCTURE — which keys, which order, which nesting — and never about
    whose YAML version resolves `on:` to a boolean.
    """
    if node is None:
        return None
    if isinstance(node, dict):
        return {normalize(k): normalize(v) for k, v in node.items()}
    if isinstance(node, list):
        return [normalize(v) for v in node]
    if isinstance(node, bool):
        return "true" if node else "false"
    return str(node)


class _Parser:
    def __init__(self, text: str) -> None:
        # A mutable copy: a `- key: value` line is rewritten in place into a plain
        # mapping line so the mapping parser can handle it without a second code
        # path (which is how a parser grows two rules for one construct).
        self.lines = text.replace("\r\n", "\n").split("\n")
        self.i = 0

    # -- errors -------------------------------------------------------------
    def _die(self, why: str, at: int | None = None) -> None:
        n = self.i if at is None else at
        line = self.lines[n] if 0 <= n < len(self.lines) else ""
        raise WorkflowYamlError(f"line {n + 1}: {why}: {line.rstrip()!r}")

    # -- line helpers -------------------------------------------------------
    @staticmethod
    def _indent(line: str) -> int:
        return len(line) - len(line.lstrip(" "))

    def _skip_ignorable(self) -> None:
        """Advance past blank lines and whole-line comments."""
        while self.i < len(self.lines):
            stripped = self.lines[self.i].strip()
            if stripped == "" or stripped.startswith("#"):
                self.i += 1
            else:
                return

    def _check_line(self) -> str:
        line = self.lines[self.i]
        if "\t" in line[: self._indent(line) + 1]:
            self._die("a tab in indentation")
        body = line.lstrip(" ")
        for leader, what in _REFUSED_LEADERS:
            if body.startswith(leader):
                self._die(f"{what} is not supported by this parser")
        if body.startswith("- "):
            after = body[2:].lstrip(" ")
            for leader, what in _REFUSED_LEADERS:
                if after.startswith(leader):
                    self._die(f"{what} is not supported by this parser")
        return line

    # -- document -----------------------------------------------------------
    def parse_document(self) -> Any:
        # A leading `---` is a document start and is fine; a second one is a
        # multi-document stream, which has no single answer.
        self._skip_ignorable()
        if self.i < len(self.lines) and self.lines[self.i].rstrip() == "---":
            self.i += 1
        for n, line in enumerate(self.lines[self.i :], start=self.i):
            if line.rstrip() in ("---", "..."):
                self._die("a multi-document stream is not supported", at=n)
        self._skip_ignorable()
        if self.i >= len(self.lines):
            return None
        return self._parse_node(self._indent(self.lines[self.i]))

    def _parse_node(self, indent: int) -> Any:
        self._skip_ignorable()
        if self.i >= len(self.lines):
            return None
        body = self._check_line().lstrip(" ")
        if body == "-" or body.startswith("- "):
            return self._parse_sequence(indent)
        return self._parse_mapping(indent)

    # -- mappings -----------------------------------------------------------
    def _parse_mapping(self, indent: int) -> dict[str, Any]:
        out: dict[str, Any] = {}
        while True:
            self._skip_ignorable()
            if self.i >= len(self.lines):
                return out
            line = self._check_line()
            here = self._indent(line)
            if here < indent:
                return out
            if here > indent:
                self._die(f"unexpected indent (expected {indent}, saw {here})")
            body = line.lstrip(" ")
            if body == "-" or body.startswith("- "):
                self._die("a sequence entry where a mapping key was expected")
            key, rest = self._split_key(body)
            self.i += 1
            out[key] = self._parse_value(rest, indent)
        # unreachable

    def _split_key(self, body: str) -> tuple[str, str]:
        if body[0] in "'\"":
            key, offset = self._scan_quoted(body)
            tail = body[offset:]
            if not tail.startswith(":"):
                self._die("a quoted scalar that is not a mapping key")
            return key, tail[1:].strip()
        # A plain key ends at the first `:` that is followed by a space or the
        # end of the line — the same rule a YAML reader applies.
        for pos, ch in enumerate(body):
            if ch == "#" and pos > 0 and body[pos - 1] == " ":
                break
            if ch == ":" and (pos + 1 == len(body) or body[pos + 1] == " "):
                return body[:pos].strip(), body[pos + 1 :].strip()
        self._die("neither a mapping key nor a sequence entry")
        raise AssertionError  # pragma: no cover — _die always raises

    # -- sequences ----------------------------------------------------------
    def _parse_sequence(self, indent: int) -> list[Any]:
        out: list[Any] = []
        while True:
            self._skip_ignorable()
            if self.i >= len(self.lines):
                return out
            line = self._check_line()
            here = self._indent(line)
            if here < indent:
                return out
            if here > indent:
                self._die(f"unexpected indent (expected {indent}, saw {here})")
            body = line.lstrip(" ")
            if not (body == "-" or body.startswith("- ")):
                return out
            if body == "-":
                self.i += 1
                out.append(self._parse_block_child(indent))
                continue
            after = body[2:]
            content_col = here + 2 + (len(after) - len(after.lstrip(" ")))
            content = after.lstrip(" ")
            if self._looks_like_key(content):
                # Rewrite `  - uses: x` into `    uses: x` and let the mapping
                # parser own it; the entry's keys continue at `content_col`.
                self.lines[self.i] = " " * content_col + content
                out.append(self._parse_mapping(content_col))
            else:
                self.i += 1
                out.append(self._parse_scalar(content))
        # unreachable

    def _looks_like_key(self, content: str) -> bool:
        if content[0] in "'\"":
            try:
                _, offset = self._scan_quoted(content)
            except WorkflowYamlError:
                return False
            return content[offset:].startswith(":")
        if content[0] in "{[":
            return False
        for pos, ch in enumerate(content):
            if ch == "#" and pos > 0 and content[pos - 1] == " ":
                return False
            if ch == ":" and (pos + 1 == len(content) or content[pos + 1] == " "):
                return True
        return False

    # -- values -------------------------------------------------------------
    def _parse_value(self, rest: str, key_indent: int) -> Any:
        m = _BLOCK_SCALAR.match(rest)
        if m:
            if m.group("style") == ">":
                self._die("a folded block scalar (`>`) is not supported", at=self.i - 1)
            if m.group("indent"):
                self._die(
                    "an explicit block-scalar indentation indicator is not supported",
                    at=self.i - 1,
                )
            return self._parse_literal_block(key_indent, m.group("chomp"))
        if rest == "":
            return self._parse_block_child(key_indent)
        return self._parse_scalar(rest)

    def _parse_block_child(self, parent_indent: int) -> Any:
        """The nested node under a key with no inline value (or a bare `-`)."""
        save = self.i
        self._skip_ignorable()
        if self.i >= len(self.lines):
            self.i = save
            return None
        child_indent = self._indent(self.lines[self.i])
        body = self.lines[self.i].lstrip(" ")
        # A sequence may sit at the SAME indent as its parent key — the one place
        # YAML lets a child not be more indented than its parent.
        if child_indent == parent_indent and (body == "-" or body.startswith("- ")):
            return self._parse_sequence(child_indent)
        if child_indent <= parent_indent:
            self.i = save
            return None
        return self._parse_node(child_indent)

    def _parse_literal_block(self, key_indent: int, chomp: str) -> str:
        block: list[str] = []
        block_indent: int | None = None
        while self.i < len(self.lines):
            line = self.lines[self.i]
            if line.strip() == "":
                block.append("")
                self.i += 1
                continue
            here = self._indent(line)
            if here <= key_indent:
                break
            if block_indent is None:
                block_indent = here
            if here < block_indent:
                break
            block.append(line[block_indent:])
            self.i += 1
        while block and block[-1] == "":
            block.pop()
        if not block:
            return ""
        text = "\n".join(block)
        if chomp == "-":
            return text
        return text + "\n"

    # -- scalars ------------------------------------------------------------
    def _parse_scalar(self, s: str) -> Any:
        s = s.strip()
        if s == "":
            return None
        if s[0] in "'\"":
            value, offset = self._scan_quoted(s)
            tail = s[offset:].strip()
            if tail and not tail.startswith("#"):
                self._die(f"trailing content after a quoted scalar: {tail!r}")
            return value
        if s[0] in "{[":
            value, offset = self._scan_flow(s, 0)
            tail = s[offset:].strip()
            if tail and not tail.startswith("#"):
                self._die(f"trailing content after a flow collection: {tail!r}")
            return value
        self._refuse_node_property(s)
        plain = self._strip_plain_comment(s)
        if plain in ("~", "null"):
            return None
        return plain

    def _refuse_node_property(self, s: str) -> None:
        """An anchor, alias or tag in VALUE position — `runs-on: *alias`.

        The line-leader check catches these on their own line; a node property
        attached to a value reaches the scalar parser instead, where reading it
        as the plain string `*alias` would be a silent misread of a reference.
        """
        for leader, what in _REFUSED_LEADERS:
            if s.startswith(leader):
                self._die(f"{what} is not supported by this parser", at=self.i - 1)

    @staticmethod
    def _strip_plain_comment(s: str) -> str:
        for pos in range(1, len(s)):
            if s[pos] == "#" and s[pos - 1] == " ":
                return s[:pos].rstrip()
        return s.rstrip()

    def _scan_quoted(self, s: str) -> tuple[str, int]:
        quote = s[0]
        out: list[str] = []
        pos = 1
        while pos < len(s):
            ch = s[pos]
            if quote == "'":
                if ch == "'":
                    if pos + 1 < len(s) and s[pos + 1] == "'":
                        out.append("'")
                        pos += 2
                        continue
                    return "".join(out), pos + 1
                out.append(ch)
                pos += 1
                continue
            if ch == "\\":
                if pos + 1 >= len(s):
                    break
                nxt = s[pos + 1]
                out.append({"n": "\n", "t": "\t", "r": "\r"}.get(nxt, nxt))
                pos += 2
                continue
            if ch == '"':
                return "".join(out), pos + 1
            out.append(ch)
            pos += 1
        self._die("an unterminated quoted scalar (multi-line scalars are not supported)")
        raise AssertionError  # pragma: no cover — _die always raises

    def _scan_flow(self, s: str, pos: int) -> tuple[Any, int]:
        opener = s[pos]
        closer = "}" if opener == "{" else "]"
        pos += 1
        items: list[Any] = []
        pairs: dict[str, Any] = {}
        while True:
            while pos < len(s) and s[pos] in " ,":
                pos += 1
            if pos >= len(s):
                self._die("a flow collection that does not close on its own line")
            if s[pos] == closer:
                return (pairs if opener == "{" else items), pos + 1
            value, pos = self._scan_flow_scalar(s, pos)
            while pos < len(s) and s[pos] == " ":
                pos += 1
            if opener == "{":
                if pos >= len(s) or s[pos] != ":":
                    self._die("a flow mapping entry with no value")
                pos += 1
                while pos < len(s) and s[pos] == " ":
                    pos += 1
                inner, pos = self._scan_flow_scalar(s, pos)
                pairs[str(value)] = inner
            else:
                items.append(value)

    def _scan_flow_scalar(self, s: str, pos: int) -> tuple[Any, int]:
        if s[pos] in "{[":
            return self._scan_flow(s, pos)
        if s[pos] in "'\"":
            value, offset = self._scan_quoted(s[pos:])
            return value, pos + offset
        start = pos
        while pos < len(s) and s[pos] not in ",:}]":
            pos += 1
        raw = s[start:pos].strip()
        self._refuse_node_property(raw)
        if raw in ("~", "null", ""):
            return None, pos
        return raw, pos
