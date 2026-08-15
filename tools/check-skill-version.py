#!/usr/bin/env python3
"""The `/new-delve` skill's version declarations, held to what they each claim.

WHY THIS EXISTS (ADR-0016, third version line)

ADR-0016 settles three independent version lines: `dsl_version` (format),
`delvec` (engine, semver from v1.0.0), and the `/new-delve` skill (product) —
"its own version, declared in the skill itself, together with the `delvec`
version range it drives". Lines 1 and 2 have machinery behind them
(`DW0141`'s per-stage fences; `crates/compiler/Cargo.toml` -> `DELVEC_VERSION`
-> `manifest.json` -> `versions.toml`). Line 3 had none: the skill's frontmatter
carried `name` and `description` and nothing else.

A `requires:` line nobody checks is the failure class this project keeps being
bitten by — the **unbound declaration** (CLAUDE.md; `playtest-methodology.md`
rule 1). The island's combat floor gate was green for nineteen rounds because it
examined zero enemies. A hand-typed engine range would be green forever for the
same reason: nothing would ever read it.

TWO FIELDS, BECAUSE THEY ARE TWO DIFFERENT CLAIMS

    requires:
      delvec: ">=1.0.0 <2.0.0"   # COMPATIBILITY — the major window it drives
    verified_with: 1.0.0          # EVIDENCE — the engine this tree proves it on

`requires.delvec` is what a creator reads as "older engines will not work". It is
ADR-0016's own shape (`e.g. delvec >= 1.0 < 2`): a **major window**, stable
across the whole 1.x line, because format compatibility is guaranteed by the
per-stage fences and an engine may release many times inside one window.

`verified_with` is the narrower, provable claim: the engine this repo actually
exercises the skill against. Collapsing the two — pinning the window's floor to
the current engine — would make the frontmatter assert, after every engine
release, that older engines are unsupported, which nobody tested and which is
probably false. It would also make ADR-0016's own example un-writable the moment
the engine reached 1.1.0.

WHAT IS CHECKED

1. **Shape.** The frontmatter carries `version:` (semver), `requires: delvec:`
   (a `>=X.Y.Z <A.B.C` range) and `verified_with:` (semver), alongside the
   loader's own `name`/`description`.

2. **`requires.delvec` binds by MEMBERSHIP.** The window is a well-formed semver
   major window — ceiling == floor's next major — and this repo's engine is
   INSIDE it: `floor <= engine < ceiling`. That is what catches a major bump:
   `delvec 2.0.0` shipping beside a skill that still says `<2.0.0` is a skill
   declaring it does not drive the engine it lives next to.

3. **`verified_with` binds by EQUALITY** to `crates/compiler/Cargo.toml`'s
   `[package] version` — the single source `DELVEC_VERSION` derives from
   (`env!("CARGO_PKG_VERSION")`), so this script never carries a second
   hand-typed copy. BOTH directions are red:

   - ABOVE it names a compiler that does not exist, so no run anywhere produced
     that evidence (the same falsification `check-storybook-version.py` applies
     to `last verified with delvec <Y>`);
   - BELOW it is stale: the engine moved and nobody re-ran the skill against it,
     so the field records evidence from a build that is no longer here.

   Restamping it is one line in the engine's own release commit, and it is NOT a
   product-version bump — ADR-0016 keeps `version:` independent precisely so
   engine fixes never touch it.

4. **Every `delvec` subcommand the skill names EXISTS**, and every long flag it
   names alongside one exists on that subcommand or is a global. This is what
   makes the window a claim about something real rather than a shrug: the range
   is a claim about a CLI surface, so the gate reads that surface out of
   `crates/compiler/src/main.rs` (the clap `Command`/`EditAction` subcommand
   enums and the global `Cli` args) and holds the skill's own command spans
   against it. `delvec calibrate` losing its `--layout`, or a subcommand renamed
   out from under step 9, is exactly the drift a version range is supposed to
   make impossible.

WHAT THIS GATE DOES *NOT* PROVE

A floor that has become **too low**. If the skill starts driving a subcommand
that only appeared in `delvec` 1.1.0 while the window still says `>=1.0.0`, check
4 passes — it tests the skill against the CURRENT CLI, which of course has that
subcommand — and nothing here notices that an engine at the declared floor would
choke. Catching it honestly needs older engines in the tree to test against, and
this repo has one engine.

That gap is the reason `verified_with` earns its place: the window states intent
and is checked for internal consistency, while the field a reader can actually
rely on states which single engine anybody has run. Do not read a green here as
"the whole 1.x line was tested" — nothing tested it.

BINDING COUNT

Every run prints what it examined: `delvec` mentions found in the skill's code
spans, subcommand references checked (and how many distinct), and long-flag
references checked. **Zero subcommand references is a FAILURE, not a pass** — it
means the extraction stopped matching the skill's prose and checks 2-3 are all
that is left standing. A gate that binds to nothing is vacuous.

Deterministic, offline, no dependencies (Python 3 stdlib). Run from anywhere:

    python3 tools/check-skill-version.py

Exit 0 = the declarations are true, 1 = a finding (see stderr), 2 = IO error.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SKILL = REPO / ".claude" / "skills" / "new-delve" / "SKILL.md"
COMPILER_CARGO_TOML = REPO / "crates" / "compiler" / "Cargo.toml"
COMPILER_MAIN_RS = REPO / "crates" / "compiler" / "src" / "main.rs"

SEMVER_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")

# `>=X.Y.Z <A.B.C` — the only range shape this project declares. Deliberately
# strict: the expected form is printed verbatim in every failure.
RANGE_RE = re.compile(r"^>=(?P<floor>\d+\.\d+\.\d+)\s+<(?P<ceiling>\d+\.\d+\.\d+)$")

CARGO_VERSION_RE = re.compile(r'(?m)^version\s*=\s*"([^"]+)"')

# --------------------------------------------------------------- frontmatter --

# YAML is not in the stdlib and the frontmatter is two levels deep at most, so
# it is parsed by hand rather than by pulling a dependency into a CI gate.
TOP_KEY_RE = re.compile(r"^(?P<key>[A-Za-z][\w-]*):\s*(?P<value>.*?)\s*$")
NESTED_KEY_RE = re.compile(r"^  (?P<key>[A-Za-z][\w-]*):\s*(?P<value>.*?)\s*$")


def read_frontmatter(path: Path) -> dict[str, object]:
    """`name`/`description`/`version` plus one level of nesting (`requires:`)."""
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0].strip() != "---":
        raise SystemExit(f"{path} does not open with a `---` frontmatter fence")
    try:
        end = lines.index("---", 1)
    except ValueError as exc:
        raise SystemExit(f"{path}'s frontmatter fence is never closed") from exc

    out: dict[str, object] = {}
    current: str | None = None
    for line in lines[1:end]:
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        nested = NESTED_KEY_RE.match(line)
        if nested and current is not None:
            sub = out.setdefault(current, {})
            if isinstance(sub, dict):
                sub[nested.group("key")] = unquote(nested.group("value"))
            continue
        top = TOP_KEY_RE.match(line)
        if top is None:
            continue
        value = top.group("value")
        if value == "":
            out[top.group("key")] = {}
            current = top.group("key")
        else:
            out[top.group("key")] = unquote(value)
            current = None
    return out


def unquote(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        return value[1:-1]
    return value


# ------------------------------------------------------------------- the CLI --

# A clap subcommand enum: `#[derive(Subcommand)] enum <Name> { ... }`. Variants
# sit at four spaces, their fields at eight — the shape rustfmt guarantees.
ENUM_RE = re.compile(r"(?ms)^#\[derive\(Subcommand\)\]\s*\nenum\s+(\w+)\s*\{(.*?)\n\}")
VARIANT_RE = re.compile(r"^    (?P<name>[A-Z]\w*)\s*(?P<open>\{)?")
FIELD_RE = re.compile(r"^        (?P<name>[a-z]\w*)\s*:")
ARG_ATTR_RE = re.compile(r"^\s*#\[(?:arg|clap)\((?P<body>.*)")
EXPLICIT_LONG_RE = re.compile(r'long\s*=\s*"(?P<name>[^"]+)"')
SUBCOMMAND_ATTR_RE = re.compile(r"^\s*#\[command\(subcommand\)\]")
NESTED_TYPE_RE = re.compile(r":\s*(?:Option<)?(?P<name>[A-Z]\w*)")
GLOBAL_STRUCT_RE = re.compile(r"(?ms)^struct\s+Cli\s*\{(.*?)\n\}")
GLOBAL_FIELD_RE = re.compile(r"^    (?P<name>[a-z]\w*)\s*:")
# The top-level subcommand enum is whatever `Cli`'s own `#[command(subcommand)]`
# field names — everything else is a nested action set.
TOP_ENUM_RE = re.compile(
    r"#\[command\(subcommand\)\]\s*\n\s*command:\s*(?:Option<)?(?P<name>\w+)"
)


def kebab(name: str) -> str:
    """clap's default rename (heck's kebab-case): `L10nInventory` -> `l10n-inventory`."""
    spaced = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "-", name)
    spaced = re.sub(r"(?<=[A-Z])(?=[A-Z][a-z])", "-", spaced)
    return spaced.replace("_", "-").lower()


def normalize(name: str) -> str:
    """Hyphen-insensitive key, so this gate never has to re-implement heck exactly.

    The failure it exists to catch — the skill naming a subcommand or flag that
    does NOT exist — is caught either way; re-deriving clap's word-boundary rules
    would only add a way for the gate itself to be wrong.
    """
    return name.replace("-", "").replace("_", "").lower()


def parse_cli(source: str) -> tuple[dict[str, set[str]], set[str]]:
    """`{top-level subcommand: {long flags}}` and the set of global long flags.

    A nested action set (`delvec edit apply|preview`) is NOT a top-level
    subcommand — `delvec apply` does not exist — but its flags are folded into
    its parent's allowed set, so `delvec edit apply --batch` reads correctly.
    """
    enums: dict[str, dict[str, set[str]]] = {}
    nested: dict[str, dict[str, str]] = {}

    for enum_name, body in ENUM_RE.findall(source):
        variants: dict[str, set[str]] = {}
        links: dict[str, str] = {}
        variant: str | None = None
        pending_long: str | None = None
        pending_is_long = False
        pending_subcommand = False
        for line in body.splitlines():
            attr = ARG_ATTR_RE.match(line)
            if attr is not None:
                attr_body = attr.group("body")
                explicit = EXPLICIT_LONG_RE.search(attr_body)
                pending_is_long = bool(re.search(r"\blong\b", attr_body))
                pending_long = explicit.group("name") if explicit else None
                continue
            if SUBCOMMAND_ATTR_RE.match(line):
                pending_subcommand = True
                continue
            field = FIELD_RE.match(line)
            if field is not None and variant is not None:
                if pending_is_long:
                    variants[variant].add(pending_long or kebab(field.group("name")))
                if pending_subcommand:
                    link = NESTED_TYPE_RE.search(line)
                    if link is not None:
                        links[variant] = link.group("name")
                pending_long, pending_is_long = None, False
                pending_subcommand = False
                continue
            var = VARIANT_RE.match(line)
            if var is not None:
                variant = kebab(var.group("name"))
                variants.setdefault(variant, set())
                pending_long, pending_is_long = None, False
                pending_subcommand = False
        enums[enum_name] = variants
        nested[enum_name] = links

    top_name_match = TOP_ENUM_RE.search(source)
    top_name = top_name_match.group("name") if top_name_match else "Command"
    subcommands = {name: set(flags) for name, flags in enums.get(top_name, {}).items()}
    for variant, child in nested.get(top_name, {}).items():
        for flags in enums.get(child, {}).values():
            subcommands.setdefault(variant, set()).update(flags)

    globals_: set[str] = set()
    struct = GLOBAL_STRUCT_RE.search(source)
    if struct is not None:
        pending_long = None
        pending_is_long = False
        for line in struct.group(1).splitlines():
            attr = ARG_ATTR_RE.match(line)
            if attr is not None:
                attr_body = attr.group("body")
                explicit = EXPLICIT_LONG_RE.search(attr_body)
                pending_is_long = bool(re.search(r"\blong\b", attr_body))
                pending_long = explicit.group("name") if explicit else None
                continue
            field = GLOBAL_FIELD_RE.match(line)
            if field is not None:
                if pending_is_long:
                    globals_.add(pending_long or kebab(field.group("name")))
                pending_long, pending_is_long = None, False
    # `--help` is clap's, not ours, and never appears in the enum.
    globals_.add("help")
    return subcommands, globals_


# ------------------------------------------------- what the skill claims to drive --

FENCE_RE = re.compile(r"^\s*```")
# A markdown code span may wrap across a line (it renders as a space) but never
# across a blank line — `delvec calibrate <report> --layout …` in this skill is
# split over two source lines, and a newline-free pattern silently misses it.
INLINE_CODE_RE = re.compile(r"`((?:[^`\n]|\n(?!\s*\n))+?)`")
SUBCOMMAND_TOKEN_RE = re.compile(r"^[a-z][a-z0-9-]*$")
LONG_FLAG_RE = re.compile(r"^--(?P<name>[a-z][a-z0-9-]*)(?:=.*)?$")

# `` `delvec <sub>` (`--flag …`, `--flag …`) `` — the paren must open IMMEDIATELY
# after the span, so this keys off syntax rather than paragraph proximity: it is
# how `delvec snapshot`'s flags are documented, and a looser "same bullet" rule
# would red on an innocent later edit.
PARENTHETICAL_RE = re.compile(r"`delvec ([a-z][a-z0-9-]*)[^`]*`\s*\(")


def code_spans(markdown: str) -> list[str]:
    """Every inline-code span and fenced-block line, in document order.

    Only code is read. A version gate that tried to parse commands out of prose
    would red on a sentence and pass on a broken invocation.
    """
    spans: list[str] = []
    fenced = False
    prose: list[str] = []
    for line in markdown.splitlines():
        if FENCE_RE.match(line):
            fenced = not fenced
            continue
        if fenced:
            spans.append(line)
        else:
            prose.append(line)
    spans.extend(INLINE_CODE_RE.findall("\n".join(prose)))
    return spans


def parenthetical_flags(markdown: str) -> list[tuple[str, list[str]]]:
    """Flags documented in the parenthesis that opens right after a subcommand span."""
    out: list[tuple[str, list[str]]] = []
    for match in PARENTHETICAL_RE.finditer(markdown):
        depth = 1
        i = match.end()
        while i < len(markdown) and depth:
            if markdown[i] == "(":
                depth += 1
            elif markdown[i] == ")":
                depth -= 1
            i += 1
        inner = markdown[match.end() : i - 1]
        flags = [
            m.group("name")
            for span in INLINE_CODE_RE.findall(inner)
            for token in span.split()
            if (m := LONG_FLAG_RE.match(token.strip(",.;:()[]'\"")))
        ]
        if flags:
            out.append((match.group(1), flags))
    return out


def invocations(spans: list[str]) -> list[tuple[str | None, list[str]]]:
    """`(subcommand | None, [long flags])` for every `delvec …` span occurrence.

    `None` is a bare mention (`` `delvec` ``, `` `delvec --version` ``): its
    flags are still held against the globals. A token that is not a lowercase
    word — a placeholder `<version>`, a version number, a flag — yields no
    subcommand, which is how the storybook marker template in this same file
    ("last verified with delvec <version>.") contributes nothing.

    Since ADR-0017 the CARGO PACKAGE is also called `delvec`, so `delvec` now
    appears in the skill as an argument to cargo (`-p delvec`, `--bin delvec`)
    as well as as a command. Those occurrences are not invocations and the
    tokens after them belong to cargo, not to this CLI — reading them as
    invocations made `cargo build -p delvec --bin delvec` report that `delvec`
    was given a `--bin` flag it does not have.

    The discriminator is the selector in front TOGETHER with the `--` behind:
    `cargo run … --bin delvec -- schema` really does hand `schema` to this CLI,
    so that occurrence is an invocation, while `-p delvec` and a `--bin delvec`
    that ends the command are pure cargo arguments. Dropping only on the
    selector loses the `-- schema` binding, which the test beside this asserts
    in both directions.
    """
    CARGO_SELECTORS = {"-p", "--package", "--bin", "--example"}
    found: list[tuple[str | None, list[str]]] = []
    for span in spans:
        tokens = span.replace("`", " ").split()
        for i, token in enumerate(tokens):
            if token != "delvec":
                continue
            after = tokens[i + 1] if i + 1 < len(tokens) else None
            if i > 0 and tokens[i - 1] in CARGO_SELECTORS and after != "--":
                continue
            rest = tokens[i + 1 :]
            if rest and rest[0] == "--":  # `cargo run … --bin delvec -- schema`
                rest = rest[1:]
            sub: str | None = None
            if rest and SUBCOMMAND_TOKEN_RE.match(rest[0]):
                sub = rest[0]
                rest = rest[1:]
            flags: list[str] = []
            for tok in rest:
                if tok == "delvec":
                    break
                m = LONG_FLAG_RE.match(tok.strip("`,.;:()[]'\""))
                if m is not None:
                    flags.append(m.group("name"))
            found.append((sub, flags))
    return found


# ------------------------------------------------------------------- the gate --


def version_key(version: str) -> tuple[int, int, int]:
    m = SEMVER_RE.match(version)
    if m is None:
        raise SystemExit(f"not a semver version: {version!r}")
    return (int(m.group(1)), int(m.group(2)), int(m.group(3)))


def engine_major_floor(version: str) -> str:
    """The start of `version`'s major window — what a suggested `requires:` opens at.

    The window floor is a COMPATIBILITY claim, so a suggestion never proposes the
    current engine as the floor: that would assert every earlier release in the
    same major is unsupported, which nothing tested.
    """
    return f"{version_key(version)[0]}.0.0"


def engine_version() -> str:
    text = COMPILER_CARGO_TOML.read_text(encoding="utf-8")
    match = CARGO_VERSION_RE.search(text)
    if match is None:
        raise SystemExit(
            f"could not read `version` from {COMPILER_CARGO_TOML} — the [package] "
            "version field moved or changed shape; fix this check, do not drop the gate"
        )
    return match.group(1)


def main() -> int:
    for path in (SKILL, COMPILER_CARGO_TOML, COMPILER_MAIN_RS):
        if not path.is_file():
            print(f"check-skill-version: FAIL — {path} is missing", file=sys.stderr)
            return 2

    findings: list[str] = []
    front = read_frontmatter(SKILL)
    engine = engine_version()

    # -- 1. shape ------------------------------------------------------------
    for key in ("name", "description"):
        if not isinstance(front.get(key), str):
            findings.append(
                f"frontmatter has no `{key}:` — the skill loader needs it; this gate "
                "adds fields to that block, it never replaces it"
            )

    skill_version = front.get("version")
    if not isinstance(skill_version, str) or not SEMVER_RE.match(skill_version):
        findings.append(
            f"frontmatter `version:` is missing or not semver (got {skill_version!r}). "
            "ADR-0016 line 3: the /new-delve skill carries its OWN version, on its own "
            "cadence — engine fixes never bump it, skill rewording never forces an "
            "engine release. Expected e.g. `version: 1.0.0`"
        )

    requires = front.get("requires")
    declared = requires.get("delvec") if isinstance(requires, dict) else None
    range_match = RANGE_RE.match(declared) if isinstance(declared, str) else None
    if range_match is None:
        findings.append(
            f"frontmatter `requires: delvec:` is missing or malformed (got "
            f"{declared!r}). ADR-0016 line 3 pairs the skill's version with the "
            f"delvec window it DRIVES — a MAJOR window, stable across the whole "
            f"line. Expected exactly:\n"
            f'    requires:\n      delvec: ">={engine_major_floor(engine)} '
            f'<{version_key(engine)[0] + 1}.0.0"'
        )

    verified = front.get("verified_with")
    if not isinstance(verified, str) or not SEMVER_RE.match(verified):
        findings.append(
            f"frontmatter `verified_with:` is missing or not semver (got "
            f"{verified!r}). `requires.delvec` states COMPATIBILITY (what a creator "
            f"reads as 'older engines will not work'); `verified_with` states "
            f"EVIDENCE — the one engine this tree actually proves the skill on. "
            f"Expected `verified_with: {engine}`"
        )

    # -- 2. the window is well formed, and this engine is INSIDE it ----------
    if range_match is not None:
        floor = range_match.group("floor")
        ceiling = range_match.group("ceiling")
        expected_ceiling = f"{version_key(floor)[0] + 1}.0.0"

        if ceiling != expected_ceiling:
            findings.append(
                f"declared ceiling {ceiling} is not the floor's next major "
                f"({expected_ceiling}). A major release may remove any subcommand the "
                f"skill drives, so the window closes at the next major and nowhere else"
            )
        elif not version_key(floor) <= version_key(engine) < version_key(ceiling):
            findings.append(
                f"this repo's delvec {engine} is OUTSIDE the declared window "
                f"{declared} — the skill ships beside an engine it says it does not "
                f"drive.\n"
                f"    crates/compiler/Cargo.toml [package] version = {engine} "
                f"(== DELVEC_VERSION). Widen or move the window:\n"
                f'      delvec: ">={engine_major_floor(engine)} '
                f'<{version_key(engine)[0] + 1}.0.0"\n'
                f"    That is the WINDOW moving, not the product version: leave "
                f"`version: {skill_version}` alone unless the skill's own workflow changed."
            )

    # -- 3. `verified_with` IS this repo's engine, both directions -----------
    if isinstance(verified, str) and SEMVER_RE.match(verified) and verified != engine:
        direction = (
            "ABOVE this repo's engine — it names a compiler that does not exist, so "
            "no run anywhere produced that evidence"
            if version_key(verified) > version_key(engine)
            else "STALE — the engine moved and nobody re-ran the skill against it, so "
            "the field records evidence from a build that is no longer in this tree. "
            "An unverifiable claim is an unbound declaration (CLAUDE.md)"
        )
        findings.append(
            f"`verified_with: {verified}` is {direction}.\n"
            f"    crates/compiler/Cargo.toml [package] version = {engine} "
            f"(== DELVEC_VERSION). Restamp it:\n"
            f"      verified_with: {engine}\n"
            f"    Leave `requires.delvec` alone unless the skill genuinely stopped "
            f"driving the older engines in its window — that is a compatibility "
            f"claim, and this one is only evidence."
        )

    # -- 4. every command the skill names exists -----------------------------
    subcommands, globals_ = parse_cli(COMPILER_MAIN_RS.read_text(encoding="utf-8"))
    if not subcommands:
        print(
            "check-skill-version: FAIL — parsed 0 subcommands from "
            f"{COMPILER_MAIN_RS}; the clap `#[derive(Subcommand)] enum` shape this "
            "gate keys off has changed. Fix the parser, do not drop the gate",
            file=sys.stderr,
        )
        return 1

    by_norm = {normalize(name): name for name in subcommands}
    flags_by_norm = {
        normalize(name): {normalize(f) for f in flags}
        for name, flags in subcommands.items()
    }
    global_norm = {normalize(f) for f in globals_}

    markdown = SKILL.read_text(encoding="utf-8")
    calls: list[tuple[str | None, list[str]]] = invocations(code_spans(markdown))
    calls.extend(parenthetical_flags(markdown))
    sub_refs = 0
    flag_refs = 0
    seen: set[str] = set()

    for sub, flags in calls:
        allowed = set(global_norm)
        if sub is not None:
            sub_refs += 1
            seen.add(sub)
            key = normalize(sub)
            if key not in by_norm:
                findings.append(
                    f"the skill drives `delvec {sub}`, which the CLI does not have.\n"
                    f"    {COMPILER_MAIN_RS.relative_to(REPO)} offers: "
                    f"{', '.join(sorted(subcommands))}"
                )
                continue
            allowed |= flags_by_norm[key]
        for flag in flags:
            flag_refs += 1
            if normalize(flag) not in allowed:
                where = f"`delvec {sub}`" if sub else "`delvec`"
                findings.append(
                    f"{where} is given `--{flag}`, which is neither one of its own "
                    f"args nor a global. Globals: "
                    f"{', '.join('--' + f for f in sorted(globals_))}"
                )

    # -- vacuity guard -------------------------------------------------------
    if sub_refs == 0:
        print(
            "check-skill-version: FAIL — extracted 0 delvec subcommand references "
            f"from {SKILL.relative_to(REPO)}. The range would then be a claim about a "
            "CLI surface nothing in this gate ever touched; a green that binds to "
            "nothing is vacuous, not a pass (CLAUDE.md).",
            file=sys.stderr,
        )
        return 1

    binding = (
        f"{len(calls)} delvec mention(s) in code spans, {sub_refs} subcommand "
        f"reference(s) over {len(seen)} distinct subcommand(s) "
        f"({', '.join(sorted(seen))}), {flag_refs} long-flag reference(s)"
    )

    if findings:
        print(
            f"check-skill-version: {len(findings)} finding(s) — bound to {binding}\n",
            file=sys.stderr,
        )
        for finding in findings:
            print(f"  - {finding}", file=sys.stderr)
        return 1

    print(
        f"check-skill-version: OK — new-delve {skill_version} drives {declared}, "
        f"verified_with {verified}, engine is {engine}. Bound to {binding}. "
        f"(Membership only: nothing here tested an engine other than {engine}.)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
