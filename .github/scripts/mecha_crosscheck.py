#!/usr/bin/env python3
"""mecha cross-check (ADR-0011).

Independently re-validate every emitted ``.mcfunction`` line against the pinned
1.21.11 command tree using mecha, as a CI-only check on our first-party
vendored-command-tree validator. Any disagreement (a line mecha rejects) fails
CI and is a bug in one of the two validators.

Usage: ``mecha_crosscheck.py <datapack-dir>`` (default ``out/datapack``).

A function file is read the way the server reads it. A plain line is a command.
A line opening with ``$`` is a **macro line**: the server substitutes every
``$(name)`` with the value the caller hands it and parses the result as a
command when the function is called, so the template is not a command (mecha,
parsing a file whole, keeps it as opaque text). Each macro line is therefore
judged after substitution, with the values the datapack itself hands it:

- a call ``function <id> {k: v, …}`` hands its literal compound;
- a call ``function <id> with storage <s> <path>`` hands what the datapack
  writes at ``<s> <path>.<name>``: every ``data modify storage <s> <p> set
  value <v>`` (the value as written) and every ``execute store result storage
  <s> <p> …`` (an integer).

Every candidate value of every name is substituted at least once. A name no
caller and no write hands a value is a failure: a macro line nothing binds is
judged by nothing. The run prints how many macro lines it judged and over how
many substitutions.

Note: pins are `mecha==0.104.1` + `beet` (see the workflow).
"""

import re
import sys
from collections import defaultdict
from pathlib import Path

VAR = re.compile(r"\$\(([A-Za-z0-9_]+)\)")
FUNCTION_ID = r"([a-z0-9_.-]+:[a-z0-9_./-]+)"
CALL_STORAGE = re.compile(r"\bfunction\s+" + FUNCTION_ID + r"\s+with\s+storage\s+(\S+)\s+(\S+)")
CALL_LITERAL = re.compile(r"\bfunction\s+" + FUNCTION_ID + r"\s+(\{.*\})\s*$")
SET_VALUE = re.compile(r"\bdata\s+modify\s+storage\s+(\S+)\s+(\S+)\s+set\s+value\s+(.+)$")
STORE_RESULT = re.compile(r"\bstore\s+(?:result|success)\s+storage\s+(\S+)\s+(\S+)\s")


def function_id(datapack: Path, path: Path) -> str:
    """`<ns>:<path>` of a function file under `<datapack>/data/<ns>/function/`."""
    ns, _kind, *rest = path.relative_to(datapack / "data").parts
    return f"{ns}:{'/'.join(rest)[: -len('.mcfunction')]}"


def split_compound(text: str) -> dict:
    """The top-level `key: value` pairs of an SNBT compound, values as written."""
    text = text.strip()
    if not (text.startswith("{") and text.endswith("}")):
        return {}
    body = text[1:-1]
    parts, depth, quote, start, i = [], 0, None, 0, 0
    while i < len(body):
        ch = body[i]
        if quote:
            if ch == "\\":
                i += 2
                continue
            if ch == quote:
                quote = None
        elif ch in "\"'":
            quote = ch
        elif ch in "{[":
            depth += 1
        elif ch in "}]":
            depth -= 1
        elif ch == "," and depth == 0:
            parts.append(body[start:i])
            start = i + 1
        i += 1
    parts.append(body[start:])
    out = {}
    for p in parts:
        if ":" in p:
            k, v = p.split(":", 1)
            out[k.strip().strip("\"'")] = v.strip()
    return out


def as_substituted(value: str) -> str:
    """A tag's text as macro substitution writes it: a string's own content,
    anything else as its SNBT."""
    v = value.strip()
    if len(v) >= 2 and v[0] == v[-1] and v[0] in "\"'":
        return re.sub(r"\\(.)", r"\1", v[1:-1])
    return v


def main(datapack_dir: str) -> int:
    datapack = Path(datapack_dir)
    functions = sorted(datapack.rglob("*.mcfunction"))
    if not functions:
        print(f"no .mcfunction files found under {datapack_dir}", file=sys.stderr)
        return 1

    texts = {p: p.read_text().splitlines() for p in functions}
    storage_calls: dict[str, set] = defaultdict(set)
    literal_calls: dict[str, list] = defaultdict(list)
    writes: dict[tuple, list] = defaultdict(list)
    for lines in texts.values():
        for raw in lines:
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            for m in CALL_STORAGE.finditer(line):
                storage_calls[m.group(1)].add((m.group(2), m.group(3)))
            m = CALL_LITERAL.search(line)
            if m:
                literal_calls[m.group(1)].append(split_compound(m.group(2)))
            m = SET_VALUE.search(line)
            if m:
                writes[(m.group(1), m.group(2))].append(as_substituted(m.group(3)))
            for m in STORE_RESULT.finditer(line):
                writes[(m.group(1), m.group(2))].append("0")

    def candidates(fid: str, name: str) -> list:
        out = []
        for compound in literal_calls.get(fid, []):
            if name in compound:
                out.append(as_substituted(compound[name]))
        for storage, path in sorted(storage_calls.get(fid, ())):
            out.extend(writes.get((storage, f"{path}.{name}"), []))
        return list(dict.fromkeys(out))

    from beet import run_beet
    from mecha import Mecha

    failures = 0
    checked = 0
    macro_lines = 0
    substitutions = 0
    # `minecraft` pins the command tree mecha validates against (ADR-0009).
    with run_beet({"require": ["mecha"], "minecraft": "1.21.11"}) as ctx:
        mc = ctx.inject(Mecha)

        def judge(path: Path, lineno: int, line: str, shown: str) -> None:
            nonlocal failures
            try:
                mc.parse(line, using="command")
            except Exception as exc:  # mecha.DiagnosticError and friends
                failures += 1
                print(f"FAIL {path}:{lineno}: {shown}\n  {exc}", file=sys.stderr)

        for path in functions:
            fid = function_id(datapack, path)
            for lineno, raw in enumerate(texts[path], start=1):
                line = raw.strip()
                if not line or line.startswith("#"):
                    continue
                checked += 1
                if not line.startswith("$"):
                    judge(path, lineno, line, line)
                    continue
                macro_lines += 1
                template = line[1:]
                names = sorted(set(VAR.findall(template)))
                values = {n: candidates(fid, n) for n in names}
                unbound = [n for n in names if not values[n]]
                if unbound:
                    failures += 1
                    print(
                        f"FAIL {path}:{lineno}: {line}\n  macro variable(s) "
                        f"{', '.join(unbound)} handed by no caller and written by no "
                        "storage write: the line is judged by nothing",
                        file=sys.stderr,
                    )
                    continue
                rounds = max((len(v) for v in values.values()), default=1)
                for i in range(rounds):
                    pick = {n: v[min(i, len(v) - 1)] for n, v in values.items()}
                    text = VAR.sub(lambda m: pick[m.group(1)], template)
                    substitutions += 1
                    judge(path, lineno, text, f"{line}\n  as called: {text}")

    print(f"mecha checked {checked} command line(s) across "
          f"{len(functions)} function(s), {macro_lines} of them macro line(s) judged "
          f"over {substitutions} substitution(s); {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    target = sys.argv[1] if len(sys.argv) > 1 else "out/datapack"
    sys.exit(main(target))
