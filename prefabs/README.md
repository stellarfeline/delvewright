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

**Authoring rule (owner QA, 2026-07-30): light your interiors.** Playable interior
areas must reach floor light level ≥ 8 with embedded light sources — the validation
bot navigates by protocol data and cannot see darkness; humans can. See spec-0001
"Lighting requirement".
