# packtest/

PackTest templates for the runtime validation layer (ADR-0005, spec-0003).
PackTest is a validation-only mod — it never runs on the player-facing server
(ADR-0003) — and is exercised in CI tier 2 via `-Dpacktest.auto` against a
headless pinned server (Minecraft Java 1.21.11, ADR-0009).

**Templates are generated, not authored here.** The compiler emits the whole
suite into `packtest-datapack/` per campaign (`compiler::emit`, catalogued in
`docs/reference/compiler.md`), and `validation/packtest-run.sh` is the ladder
entry. This directory holds hand-written templates, of which there are currently
none — a hand-written template would be raw mcfunction, which CLAUDE.md forbids
an LLM to author, so anything landing here is a deliberate human exception.
