"""`delvec`'s clap command surface, read out of the crates' sources.

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

The surface spans several crates (ADR-0023 §3): the binary's own `main.rs`
declares the compiler's verbs and MOUNTS the grammar, prefab, schematic,
harvest and render surfaces as tuple variants — `Grammar(GrammarArgs)` — whose
`Args` type carries a `#[command(subcommand)]` enum of its own. Callers hand
this parser the concatenation of `crates/delvec/src/main.rs` and every
`crates/*/src/**/*.rs`, `main.rs` first, so `Cli`'s own subcommand enum is the
first one found; a mounted group reads as ONE top-level subcommand with every
nested flag folded in, and `parse_groups` says which nested verbs it carries.

A parse that finds NOTHING is a failure for every caller, never a pass — the
callers own that refusal, because "zero subcommands" means something different
to each of them, but none of them may read it as agreement.
"""

from __future__ import annotations

import re

# A clap subcommand enum: `#[derive(Subcommand)] enum <Name> { ... }` — with any
# other derives beside it (`#[derive(Clone, Subcommand)]`). Variants sit at
# four spaces, their fields at eight — the shape rustfmt guarantees.
ENUM_RE = re.compile(
    r"(?ms)^#\[derive\([^)]*\b(?:clap::)?Subcommand\b[^)]*\)\]\s*\n(?:pub\s+)?enum\s+(\w+)\s*\{(.*?)\n\}"
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
# A mounted surface: a tuple variant whose payload is an `Args` struct, not
# preceded by `#[command(flatten)]` — `Grammar(delvewright_grammar::cli::GrammarArgs),`.
MOUNTED_VARIANT_RE = re.compile(r"^    (?P<name>[A-Z]\w*)\((?P<ty>[\w:]+)\),?\s*$")
# An `Args` struct: `#[derive(Clone, Args)]` / `#[derive(clap::Args)]`, any
# further attribute lines (`#[group(...)]`), then the struct. Its body carries
# ordinary `#[arg]` fields, `#[command(flatten)]` fields naming another `Args`
# struct, and at most one `#[command(subcommand)]` field naming an enum.
ARGS_STRUCT_RE = re.compile(
    r"(?ms)^#\[derive\([^)]*\b(?:clap::)?Args\b[^)]*\)\]\s*\n(?:#\[[^\n]*\]\s*\n)*"
    r"(?:pub\s+)?struct\s+(\w+)\s*\{(.*?)\n\}"
)
ARGS_FIELD_RE = re.compile(r"^    (?:pub\s+)?(?P<name>[a-z]\w*)\s*:\s*(?P<ty>[\w:<>]+)")
FLATTEN_FIELD_RE = re.compile(r"^        (?:pub\s+)?(?P<name>[a-z]\w*)\s*:\s*(?P<ty>[\w:]+)")
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
    mounted: dict[str, dict[str, str]] = {}
    args_structs = _parse_args_structs(source)

    for enum_name, body in ENUM_RE.findall(source):
        variants: dict[str, set[str]] = {}
        links: dict[str, str] = {}
        mounts: dict[str, str] = {}
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
                if pending_flatten:
                    # `#[command(flatten)] source: Source` inside a variant: the
                    # struct's own flags are this variant's flags.
                    flat_field = FLATTEN_FIELD_RE.match(line)
                    if flat_field is not None:
                        ty = flat_field.group("ty").split("::")[-1]
                        variants[variant].update(args_structs.get(ty, ({}, None, set()))[0])
                pending_long, pending_is_long = None, False
                pending_subcommand = False
                pending_flatten = False
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
                mount = MOUNTED_VARIANT_RE.match(line)
                if mount is not None:
                    mounts[variant] = mount.group("ty").split("::")[-1]
        enums[enum_name] = variants
        nested[enum_name] = links
        mounted[enum_name] = mounts

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
    # A mounted surface is ONE top-level subcommand carrying every flag of its
    # `Args` struct and of every verb under it, however deeply nested.
    for variant, ty in mounted.get(top_name, {}).items():
        subcommands.setdefault(variant, set()).update(
            _fold_args_struct(ty, args_structs, enums, nested, flattened, mounted)
        )

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


def _parse_args_structs(
    source: str,
) -> dict[str, tuple[set[str], str | None, set[str]]]:
    """`{struct name: (own long flags, subcommand enum or None, flattened structs)}`."""
    out: dict[str, tuple[set[str], str | None, set[str]]] = {}
    for name, body in ARGS_STRUCT_RE.findall(source):
        flags: set[str] = set()
        subcommand: str | None = None
        flattens: set[str] = set()
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
            if FLATTEN_ATTR_RE.match(line):
                pending_flatten = True
                continue
            field = ARGS_FIELD_RE.match(line)
            if field is not None:
                if pending_is_long:
                    flags.add(pending_long or kebab(field.group("name")))
                if pending_subcommand:
                    subcommand = field.group("ty").split("::")[-1].removeprefix("Option<").rstrip(">")
                if pending_flatten:
                    flattens.add(field.group("ty").split("::")[-1])
                pending_long, pending_is_long = None, False
                pending_subcommand = pending_flatten = False
        out[name] = (flags, subcommand, flattens)
    return out


def _fold_enum(
    enum_name: str,
    args_structs: dict[str, tuple[set[str], str | None, set[str]]],
    enums: dict[str, dict[str, set[str]]],
    nested: dict[str, dict[str, str]],
    flattened: dict[str, set[str]],
    mounted: dict[str, dict[str, str]],
    seen: set[str],
) -> set[str]:
    """Every long flag reachable under an enum's verbs, nested verbs included."""
    if enum_name in seen:
        return set()
    seen.add(enum_name)
    flags: set[str] = set()
    for variant, own in enums.get(enum_name, {}).items():
        flags.update(own)
        child = nested.get(enum_name, {}).get(variant)
        if child is not None:
            flags.update(_fold_enum(child, args_structs, enums, nested, flattened, mounted, seen))
        ty = mounted.get(enum_name, {}).get(variant)
        if ty is not None:
            flags.update(_fold_args_struct(ty, args_structs, enums, nested, flattened, mounted, seen))
    for child in flattened.get(enum_name, set()):
        flags.update(_fold_enum(child, args_structs, enums, nested, flattened, mounted, seen))
    return flags


def _fold_args_struct(
    ty: str,
    args_structs: dict[str, tuple[set[str], str | None, set[str]]],
    enums: dict[str, dict[str, set[str]]],
    nested: dict[str, dict[str, str]],
    flattened: dict[str, set[str]],
    mounted: dict[str, dict[str, str]],
    seen: set[str] | None = None,
) -> set[str]:
    """Every long flag of an `Args` struct: its own, its flattens', its enum's."""
    seen = set() if seen is None else seen
    own, subcommand, flattens = args_structs.get(ty, (set(), None, set()))
    flags = set(own)
    for flat in flattens:
        flags.update(_fold_args_struct(flat, args_structs, enums, nested, flattened, mounted, seen))
    if subcommand is not None:
        flags.update(_fold_enum(subcommand, args_structs, enums, nested, flattened, mounted, seen))
    return flags


def parse_groups(source: str) -> dict[str, set[str]]:
    """`{mounted top-level subcommand: {its own verbs}}` — `grammar` -> `{list, …}`.

    What a caller holding a built binary asserts against `delvec <group> --help`,
    so a mounted surface cannot lose a verb without the release gate noticing.
    An enum reached through a mounted struct's `#[command(subcommand)]` is that
    group's verb set; a group whose struct declares no subcommand (`harvest`)
    is absent here, and its flags are already folded by `parse_cli`.
    """
    args_structs = _parse_args_structs(source)
    top_name_match = TOP_ENUM_RE.search(source)
    top_name = top_name_match.group("name") if top_name_match else "Command"
    enum_bodies = dict(ENUM_RE.findall(source))
    groups: dict[str, set[str]] = {}
    for line in enum_bodies.get(top_name, "").splitlines():
        mount = MOUNTED_VARIANT_RE.match(line)
        if mount is None:
            continue
        ty = mount.group("ty").split("::")[-1]
        subcommand = args_structs.get(ty, (set(), None, set()))[1]
        if subcommand is None or subcommand not in enum_bodies:
            continue
        verbs = {
            kebab(m.group("name"))
            for m in (VARIANT_RE.match(l) for l in enum_bodies[subcommand].splitlines())
            if m is not None
        }
        groups[kebab(mount.group("name"))] = verbs
    return groups
