# Repo scaffolding plan — first PR (Milestone M0)

Scope guard: **no feature code**. This PR makes the repo exist, makes CI the arbiter,
and lands the founding documents. Anything beyond that list is out of scope.

## Contents of PR #1

1. **Founding docs** (written in the kickoff planning session):
   - `CLAUDE.md`
   - `docs/handoff-2026-07-29.md`
   - `docs/adr/` — ADR-0001..0010 + index
   - `docs/specs/` — four skeletons + index
   - `docs/ROADMAP.md`, `docs/open-questions.md`, this file
2. **Empty Rust workspace**: root `Cargo.toml` with workspace members
   `crates/dsl`, `crates/compiler`, `crates/orchestrator` — each a lib crate with a
   single trivial test, so `cargo test` exercises the workspace. No logic.
3. **Harness stub**: `harness/package.json` + `tsconfig.json` (strict) + one trivial
   test. No mineflayer usage yet.
4. **Placeholders with intent**: `prefabs/README.md` (license/provenance rules,
   git-lfs note), `packtest/README.md`, `validation/README.md` — one paragraph each
   pointing at the owning spec/ADR.
5. **CI skeleton** (spec-0004 tier 1 only): fmt, clippy `-D warnings`, cargo test,
   tsc/lint, markdown link check.
6. **Licensing**: root `LICENSE` (GPL — version pending owner confirmation, see open
   questions; use GPL-3.0-or-later if unconfirmed at PR time and note it),
   `prefabs/LICENSE-ASSETS.md` stating the CC0/CC BY-only ingestion rule.
7. **Repo config**: `.gitignore`, `.gitattributes` (git-lfs for `prefabs/**/*.nbt`),
   PR template asking "which spec does this implement? what does CI now prove?".

## Setup steps (around the PR)

1. `git init` locally; create public GitHub repo `delvewright` (name verified free
   2026-07-29); also register nothing yet on crates.io/npm — squatting our own name
   there only when first publishing.
2. Enable git-lfs, GitHub Actions.
3. Push scaffold branch, open PR #1, confirm CI green.
4. After merge: branch protection on `main` (PR + green tier 1 required, linear
   history) — done after merge so the first PR isn't blocked by its own workflow
   not existing on `main` yet.

## Exit criteria (= M0 exit)

- [ ] Public repo, PR #1 merged via green CI.
- [ ] `cargo test` + harness test run in CI on the empty workspace.
- [ ] Branch protection active; a red check demonstrably blocks merge.
