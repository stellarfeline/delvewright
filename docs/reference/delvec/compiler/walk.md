# `delvec::compiler::walk`

The reference page for `crates/delvec/src/compiler/walk.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0974 — a walk record that does not describe this build (`compiler::walk`; error; exit 1)

| Code | Meaning |
|------|---------|
| `DW0974` | **A walk record that does not describe this build.** Validation funnel (exit 1), `compiler::walk::check` from `validate_loaded`, after the detail plan's checks: a PRESENT `walk-record.json` that does not parse, names no build (no site plan or no layout graph), or whose `site_plan_sha256` (grid), `layout_graph_sha256` (ways) or `detail_sha256` (detail) half differs from this build's — the first half that moved is named with both hashes; a detail half equal to the blockout's beside a build that binds a place is named a walk of the BLOCKOUT. An absent record and a `findings`/`unwalked` verdict of this build are not refused. **Drift advisory** (warning, same code): every key half equal and `blockout_sha256` differs from this engine's. Prescription: walk the build that ships and write its record from the hashes printed at validation. |

The owner walks the detailed world, after detail, and `walk-record.json` names
the build that was walked (§2, *The walk record*). Run in `validate_loaded`, the
one funnel every `delvec` subcommand's validation goes through — `build`
included — after the detail plan's checks; the hashes are printed just before it,
refused runs included.

**What it refuses.** A record that is PRESENT and does not describe this build:
it does not parse (its form is fixed, and the refusal names every field and
every verdict the type admits, read off the type), it names no build because the
campaign has no site plan or no layout graph, or one of its three key halves —
`site_plan_sha256` (the grid), `layout_graph_sha256` (the ways), `detail_sha256`
(what stands in the whole) — differs from this build's. The half that moved is
named and both of its hashes printed; the halves are compared in that order and
the first that moved is the one refused, so the binding line says how many of
the three were compared. A record whose detail half is the blockout's beside a
build that binds a place is named as **a walk of the BLOCKOUT** — a walk of
something other than the world that ships.

**What it does not refuse.** An absent record — the campaign nobody has walked
yet, whose build is the one the walk needs. A verdict: a `findings` or
`unwalked` record of this build is a true statement about it. Detail work: no
verb asks for a walk record before detail.

**Binding**, on every site-plan run and every run with a record: `walk binding:
<r> walk record(s) read, <c> of 3 key hash(es) compared (grid, ways, detail)`,
followed by the verdict of a record that describes this build, or what the zero
means.

**The drift advisory** (warning, same code): every key half equal, and the
record's `blockout_sha256` — the massing with nothing bound — differs from this
engine's. The massing is a pure function of the site plan, the layout graph, the
metrics table and the engine, and everything it reads out of the two documents is
in the grid and ways halves, so what is left to have moved is the toolchain; the
warning names both blockout hashes and both engine revisions and refuses nothing.
A record `DW0974` refuses never reaches it, and an absent layout graph is never
reported as unchanged. The record's `engine_revision` is not in the key: a key
that moved with every engine commit would demand a walk for a change no body can
feel.

**Remedy.** Walk this build and re-record from its output, or remove the record.
After a walk with findings, the edit that answers them makes the record a record
of another build, so it is removed with that edit and the build is walked again.
