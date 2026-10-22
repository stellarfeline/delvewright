"""The horizon of a stage-1 document, read the way the compiler reads it.

`horizon` is written in the object form (`{"base": "ocean", ...params}`). The
bare base (`"ocean"`) names the same variant; the compiler reads it only to
refuse it (`DW1031`), and a ledger row's match may still spell it. Both resolve
to one base (`Horizon::resolved`, `horizon_base` in `crates/dsl`); every Python
reader goes through `base_of` so a second parse rule cannot exist.
"""

# The default base when `horizon` is not declared (`HorizonBase::default`).
DEFAULT_BASE = "void"


def base_of(value) -> str | None:
    """The base a `horizon` value names; `None` when it names none.

    Accepts the string shorthand and the object form. Anything else (absent,
    null, malformed) names no base: a caller that needs the default applies
    `DEFAULT_BASE` itself, since "undeclared" and "void" are different facts to
    a checker that counts declarations.
    """
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        base = value.get("base")
        return base if isinstance(base, str) else None
    return None


def declared_base(doc) -> str | None:
    """The base a stage-1 document (`world.json`, with or without its
    `content` wrapper) declares, or `None` if it declares none."""
    if not isinstance(doc, dict):
        return None
    body = doc.get("content", doc)
    if not isinstance(body, dict):
        return None
    return base_of(body.get("horizon"))
