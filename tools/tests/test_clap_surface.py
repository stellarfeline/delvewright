"""`tools/lib/clap_surface.py`: a declaration is read at every visibility.

The surface's nested action sets (`delvec edit apply|preview`, `delvec rig
describe`) live in modules of the binary at their narrowest visibility,
`pub(crate)`. A parser that recognised an enum or an `Args` struct only when it
was private or `pub` dropped every flag under them while still finding the
subcommand, so `delvec edit apply --batch` read as a flag the CLI does not have.
"""

from __future__ import annotations

import importlib.util
import pathlib

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "clap_surface", REPO / "tools" / "lib" / "clap_surface.py"
)
clap_surface = importlib.util.module_from_spec(spec)
spec.loader.exec_module(clap_surface)

ROOT = """#[derive(Parser)]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Edit {
        #[command(subcommand)]
        action: EditAction,
    },
    Mounted(some::cli::MountedArgs),
}
"""

NESTED = """#[derive(Subcommand)]
{vis}enum EditAction {{
    Apply {{
        campaign_dir: PathBuf,
        #[arg(long)]
        batch: Option<PathBuf>,
    }},
}}

#[derive(Clone, Args)]
{vis}struct MountedArgs {{
    #[arg(long)]
    {vis}gym: Option<PathBuf>,
}}
"""

VISIBILITIES = ["", "pub ", "pub(crate) ", "pub(super) ", "pub(in crate::cli) "]


@pytest.mark.parametrize("vis", VISIBILITIES)
def test_a_nested_action_set_is_read_at_every_visibility(vis):
    subcommands, globals_ = clap_surface.parse_cli(ROOT + "\n" + NESTED.format(vis=vis))
    assert subcommands == {"edit": {"batch"}, "mounted": {"gym"}}, vis
    assert globals_ == {"json", "help"}


def test_the_tree_surface_reads_the_binary_modules():
    """The concatenation every caller hands the parser — `main.rs` first, so
    `Cli`'s own subcommand enum is the first one found — carries the flags of
    the nested action sets that live in `crates/delvec/src/cli/`."""
    main_rs = REPO / "crates" / "delvec" / "src" / "main.rs"
    sources = [main_rs.read_text(encoding="utf-8")] + [
        f.read_text(encoding="utf-8")
        for f in sorted((REPO / "crates").glob("*/src/**/*.rs"))
        if f != main_rs
    ]
    subcommands, _ = clap_surface.parse_cli("\n".join(sources))
    assert {"batch", "out"} <= subcommands["edit"]
    assert "facing" in subcommands["rig"]
