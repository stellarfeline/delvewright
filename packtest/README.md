# packtest/

PackTest templates for the runtime validation layer (ADR-0005, spec-0003).
PackTest is a validation-only mod — it never runs on the player-facing server
(ADR-0003) — and is exercised in CI tier 2 via `-Dpacktest.auto` against a
headless pinned server (Minecraft Java 1.21.11, ADR-0009). Empty stub for now;
templates land with spec-0003.
