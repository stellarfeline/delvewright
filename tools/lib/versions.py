"""`versions.toml`, read the way TOML is read — one authority for every gate.

The pin registry is a TOML file, and a gate that needs a value out of it has two
ways to get one: parse the format with the standard library, or match a regex
against the bytes. The second is a private re-implementation of a format, and
this repository already ran two of them side by side for the same key. So the
reading lives here, it goes through `tomllib` — a real implementation of the
format, which is what a checker over a structured document owes its consumers —
and a gate that wants a pin calls this rather than growing a third copy.

Deliberately tiny: it answers for the keys gates actually ask about, and a new
key earns a function here rather than a regex at the call site.

Stdlib only (`tomllib`, Python 3.11+), no I/O beyond reading the file.
"""

import pathlib
import tomllib

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
VERSIONS_TOML = REPO_ROOT / "versions.toml"


class PinError(RuntimeError):
    """`versions.toml` does not hold the pin a gate asked for.

    Raised rather than defaulted: a gate that silently substitutes a value for a
    pin it could not read is a gate that passes on a number nobody wrote down.
    """


def load(path: pathlib.Path | None = None) -> dict:
    """The whole registry, parsed."""
    return tomllib.loads((path or VERSIONS_TOML).read_text(encoding="utf-8"))


def pin(section: str, key: str, path: pathlib.Path | None = None) -> str:
    """One string pin, by section and key. Raises [`PinError`] when absent."""
    doc = load(path)
    value = doc.get(section, {}).get(key)
    if not isinstance(value, str) or not value:
        where = path or VERSIONS_TOML
        raise PinError(
            f"{where} has no string `{key}` under `[{section}]` — the pin moved "
            "or changed shape; fix the reader, never drop the gate"
        )
    return value


def minecraft_version(path: pathlib.Path | None = None) -> str:
    """`[minecraft] version` — the Minecraft Java version every delve runs on
    (ADR-0009), and the version a player is told to install."""
    return pin("minecraft", "version", path)
