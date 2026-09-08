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

Shell scripts read pins through the same door: `python3 tools/lib/versions.py
<section>.<key>` prints one pin and exits non-zero, with a named reason, when the
registry does not hold it.

Stdlib only (`tomllib`, Python 3.11+), no I/O beyond reading the file.
"""

import pathlib
import sys
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


def engine_version(path: pathlib.Path | None = None) -> str:
    """`[engine] version` — the `delvec` release line this tree IS.

    Read by `tools/lib/delvec-bin.sh` to decide whether a `delvec` already on
    `PATH` is this engine or a different one.
    """
    return pin("engine", "version", path)


def chunky_core(path: pathlib.Path | None = None) -> str:
    """`[render] chunky_core` — the Chunky snapshot core every emitted scene was
    verified against (`crates/delvec/src/compiler/view/scene.rs`).

    Read by `validation/render-shots.sh`, which names it beside whatever core is
    actually installed rather than assuming the two agree.
    """
    return pin("render", "chunky_core", path)


def _main(argv: list[str]) -> int:
    """`python3 tools/lib/versions.py <section>.<key>` — the shell's reader.

    A shell script that needs a pin has the same two choices a Python gate has,
    and the regex is the same private re-implementation there. This entry point
    is what makes "every reader reads it from `versions.toml`" true for `sh` as
    well, with no second parser.
    """
    if len(argv) != 1 or argv[0].count(".") != 1:
        print("usage: versions.py <section>.<key>", file=sys.stderr)
        return 2
    section, key = argv[0].split(".", 1)
    try:
        sys.stdout.write(pin(section, key) + "\n")
    except PinError as e:
        print(f"versions.py: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(_main(sys.argv[1:]))
