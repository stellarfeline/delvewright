# spec-0002: Compiler CLI contract

- **Status**: Skeleton
- **ADRs**: 0001, 0004, 0006, 0009, 0010

The deterministic compiler: staged DSL in, datapack + world assets out. Foundation
settled by ADR-0011: **Rust-native** (`crates/compiler`), vendored 1.21.11 command
tree for static syntax validation, mecha as an independent CI cross-check only.

## Proposed CLI shape (to refine in Draft)

```
delvec validate <campaign-dir>          # stages 1–5 schema + referential checks
delvec analyze  <campaign-dir>          # quest-graph reachability report (ADR-0005)
delvec build    <campaign-dir> -o out/  # emit datapack + world + critical-path trace
delvec --version                        # includes dsl_version + pinned MC version
```

- Input: a campaign directory of stage JSON files (spec-0001).
- Exit codes: 0 ok / 1 validation failure / 2 analysis failure (unreachable quest,
  deadlock) / >2 internal error. Machine-readable JSON diagnostics on `--json`.
- Output of `build`:
  - `datapack/` — advancements, scoreboards (init/objectives), mcfunctions,
    structure + template_pool JSON referencing `prefabs/` by content
  - `world-fragment/` or seed+config sufficient to produce the world (mechanism TBD:
    depends on jigsaw-at-server-gen vs pre-generated region files — resolve in M1)
  - `critical-path.json` — machine-readable walkthrough for the bot harness
    (spec-0003); this is a **contract output**, versioned with the DSL
- Determinism: `build` twice on same input → byte-identical output tree (ADR-0006
  rules: no timestamps, sorted iteration, seeded PRNG only).

## Acceptance criteria (to be made precise in Draft)

- [ ] Double-build hash-compare passes in CI on every fixture campaign.
- [ ] `validate`/`analyze` exit codes and JSON diagnostics match fixtures.
- [ ] Emitted datapack loads on the pinned headless server with zero log errors.
- [ ] All emitted commands pass syntax validation for the pinned version.
- [ ] `critical-path.json` schema is versioned and consumed successfully by the
      harness in M1.

## Open

- World emission mechanism (server-side jigsaw gen vs compiler-placed regions) —
  experiment in M1; outcome may trigger ADR-0004's fallback.
