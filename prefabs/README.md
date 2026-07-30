# prefabs/

The `.nbt` prefab library and its metadata, assembled into maps via vanilla
jigsaw / `template_pool` with compiler-controlled seeds (ADR-0004). No
block-by-block generation. **License/provenance ingestion rule (ADR-0007,
CLAUDE.md forbidden zones):** in-repo prefab assets must be original, CC0, or
CC BY only — **CC BY-NC or unknown-license material is never ingested** — and
the source/license of every asset is recorded in its prefab metadata. See
`LICENSE-ASSETS.md` for the full statement. **git-lfs note:** binary `.nbt`
files are tracked via git-lfs (see `.gitattributes`); clone with git-lfs
installed, and CI checks out this directory only in the tiers that need it.

**Authoring rule (owner QA, 2026-07-30): darkness is a declared decision, never a
default.** Every prefab declares a lighting profile in metadata — `lit` (floor
light ≥ 7, the default requirement), `dim` (3–6, with an atmosphere rationale), or
`dark` (< 3, only usable where the campaign provably supplies a mitigation such as
night vision). Levels are machine-measured once at library admission. See
spec-0001 "Lighting contract".
