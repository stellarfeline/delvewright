"""`delvec`'s clap command surface, read out of the crate's sources.

ONE PARSER, because three gates in two repositories ask the same question and a
mirror of a parse rule is the defect this project names rather than a saving:

  - the campaigns repository's `tools/check-skill-version.py` holds every
    `delvec` subcommand and long flag the `/new-delve` page names against this
    surface (it vendors this file byte-for-byte, under the `engine-authoring`
    pin, so "one parser" stays true across the repository boundary rather than
    being an intention);
  - `tools/build-release-binaries.sh` holds the BUILT binary's `--help` against
    the same surface, so a release artifact cannot promise a command its bytes
    do not carry;
  - and either could be extended without the other growing a second copy.

It is TEXTUAL, deliberately. Every caller is stdlib-only and must run on a
creator's clone with nothing installed and without building the compiler, so the
surface is parsed out of the clap derive macros rather than asked of a binary.
The shape it keys off is the one rustfmt guarantees: variants at four spaces,
their fields at eight.

A parse that finds NOTHING is a failure for every caller, never a pass — the
callers own that refusal, because "zero subcommands" means something different
to each of them, but none of them may read it as agreement.
"""

from __future__ import annotations

import re

# A clap subcommand enum: `#[derive(Subcommand)] enum <Name> { ... }`. Variants
# sit at four spaces, their fields at eight — the shape rustfmt guarantees.
ENUM_RE = re.compile(
    r"(?ms)^#\[derive\(Subcommand\)\]\s*\n(?:pub\s+)?enum\s+(\w+)\s*\{(.*?)\n\}"
)
# `#[command(flatten)] View(some::path::ViewCommand),` — the flattened enum's own
# variants ARE top-level subcommands, so a parser that stopped at the variant
# name would report a subcommand (`view`) the CLI does not have and miss the six
# it does. The enum may live in any module of the crate, so its declaration is
# looked up across the crate's sources rather than in `main.rs` alone.
FLATTEN_ATTR_RE = re.compile(r"^\s*#\[command\(flatten\)\]")
FLATTEN_VARIANT_RE = re.compile(r"^    (?P<name>[A-Z]\w*)\((?P<ty>[\w:]+)\)")
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
    flattened: dict[str, set[str]] = {}

    for enum_name, body in ENUM_RE.findall(source):
        variants: dict[str, set[str]] = {}
        links: dict[str, str] = {}
        variant: str | None = None
        pending_long: str | None = None
        pending_is_long = False
        pending_subcommand = False
        pending_flatten = False
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
            if FLATTEN_ATTR_RE.match(line):
                pending_flatten = True
                continue
            if pending_flatten:
                flat = FLATTEN_VARIANT_RE.match(line)
                if flat is not None:
                    flattened.setdefault(enum_name, set()).add(
                        flat.group("ty").split("::")[-1]
                    )
                    pending_flatten = False
                    variant = None
                    continue
                pending_flatten = False
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
    # A flattened enum contributes its OWN variants as top-level subcommands.
    for child in flattened.get(top_name, set()):
        for name, flags in enums.get(child, {}).items():
            subcommands.setdefault(name, set()).update(flags)

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
