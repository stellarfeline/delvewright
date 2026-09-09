# ADR-0027: The content repository is an optional clone, and the plugin reaches the engine tree instead of carrying pieces of it

- **Status**: Proposed
- **Date**: 2026-09-09
- **Source**: two constraints on the creator-facing front end, settled while
  spec-0063 was designed against ADR-0014: the shipped prefab library is of
  little use to a creator without a community around it, so the content
  repository is cloned only by a creator who wants that library; and the page
  needs the engine tree at its pinned revision, which no copy inside a plugin
  can stand in for.
- **Refines**: ADR-0014 (creator distribution). Its first Decision bullet and
  the compose-rig clause of its third are superseded below; its form — a
  Claude Code plugin from a marketplace under the project's account, pinned
  checksum-verified binaries from GitHub Releases, one page in two modes —
  stands unchanged, and spec-0063 executes it.
- **Refines**: ADR-0023 §10, whose sentence "creators work in the content
  repository" reads as "creators may".

## Context

ADR-0014 separated audiences: an ordinary creator never needs the engine
repository. It expressed that separation as a working directory — "creators
clone only `delvewright-campaigns`; the content repo is their working
directory (campaign sources + prefab library + catalog in one place)" — and it
had the plugin "carry the compose rig inside the plugin" so that the toolchain
could arrive "without the pipeline repo".

Two things have moved since. The shipped library is small (thirty-six pieces)
and, without a community contributing to it, a creator's own campaign is more
often built from grammar programs and converted schematics than from it —
`delvec grammar` expands a corpus that is Rust source inside the binary, and
`delvec schem` converts an outside schematic, so both original-content paths
reach a prefab without the library. A repository a creator must clone in order
to use none of its contents is a prerequisite with no consumer. And the page
as it stands names thirty-seven distinct paths under the engine tree — the
compose files and their entry scripts, the staging gate, the reference-image
and skin tools, the version manifest, the reference documents. ADR-0023 §2 had
already made the source build the floor that is always present, which means an
engine checkout at the pinned revision is already part of every creator's
toolchain; copying pieces of that tree into a plugin would create a second
authority for each piece while the first sits on the same disk.

Both are departures from ADR-0014's wording. Neither is a departure from its
decision: the creator still never needs the engine repository as a place to
work, and the toolchain still arrives through the plugin's own Init.

## Decision

1. **The content repository is an optional clone.** A creator clones
   `delvewright-campaigns` for one of two reasons — to use the shipped prefab
   library, at the revision the engine's own `versions.toml [content].sha`
   names for the pinned engine, or to publish a campaign there through a pull
   request — and for no other. The page works in an empty directory; a
   campaign lives under the creator's working directory, whatever it is; the
   library is named in Init and taken at the step that first needs it, by a
   stated postcondition. It is not a plugin dependency (the standard has no
   optional dependency) and it is not a versioned download of its own.
2. **The plugin carries the page and reaches the engine tree; it carries no
   copy of anything the tree already holds.** Init clones the engine
   repository at the revision the page pins and reads the compose rig, the
   Python tools, the version manifest and the reference documents from that
   checkout, in both of ADR-0014's modes. The plugin's own contents are the
   page, its bundled references and scripts, and its pin.

## Consequences

- ADR-0014's first Decision bullet is superseded by §1; the compose-rig clause
  of its third bullet is superseded by §2. Its remaining decisions stand.
- ADR-0023's revisit trigger "the skill ships from the content repository"
  does not fire: the skill ships from the engine repository. Its §10 reads
  with "may" in place of "do"; the floor it describes is now the toolchain's
  ordinary shape rather than a fallback.
- `docs/reference/skill-workflow.md`, which cited ADR-0014 as the reason the
  page lives in the content repository, is corrected in the pull request that
  moves the page; spec-0063 §10 names it.
- The content repository's page, its two page gates and its authoring pins
  leave it; its Claude Code settings recommend the marketplace and the plugin.
  spec-0063 §9 enumerates the change.

## Revisit triggers

- A community forms around the library and a campaign is more often assembled
  from shipped pieces than from grammar programs: §1's default is then
  re-examined, though the mechanism — an optional clone at a stated revision —
  need not change.
- The page stops needing the engine tree for anything the binary does not
  carry (every tool it names becomes a `delvec` subcommand, ADR-0023 §3): §2's
  clone becomes the floor alone again, taken only when the archive cannot be.
