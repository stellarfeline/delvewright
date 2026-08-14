# ADR-0014: Creator distribution — plugin install, content repo as workdir

- **Status**: Accepted (implementation deferred to M4)
- **Date**: 2026-07-31
- **Refines**: ADR-0012 (product form)

## Context

Today `/new-delve` is a project-level skill in the pipeline repo: using it
requires cloning `delvewright` and running Claude Code there. The owner's
product design separates audiences: ordinary creators should never need the
pipeline repo.

## Decision

- **Creators clone only `delvewright-campaigns`** — the content repo is their
  working directory (campaign sources + prefab library + catalog in one place).
- **The skill ships as a Claude Code plugin** distributed via a plugin
  marketplace under this GitHub account; the content repo's Claude Code
  settings recommend the plugin, so opening it prompts installation.
- **The toolchain arrives without the pipeline repo**: the skill bootstraps
  pinned, checksum-verified multi-platform `delvec`/`delve-render` binaries
  from GitHub Releases, pulls validation images from GHCR, and carries the
  compose rig inside the plugin.
- **The skill becomes dual-mode**: pipeline-repo checkout → `cargo run` (dev
  mode, today's behavior); content-repo workdir → plugin-managed binaries
  (creator mode).
- Cloning `delvewright` remains the path for DSL/compiler development only.

## Consequences

- M4 gains three work items: multi-platform binary releases, dual-mode skill
  path resolution, plugin + marketplace + content-repo recommendation config.
- Until M4, the documented flow (README) is dev mode; the owner's M2
  acceptance runs in dev mode by design.
