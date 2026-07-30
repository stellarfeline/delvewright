# validation/

Docker-compose profile running a headless pinned server plus the mineflayer bot
harness — the same image used by CI and prod (ADR-0005, ADR-0008, spec-0003).
"Works on my machine" is defined as "this compose profile passes". It drives the
tier-2 datapack load check and the tier-3 critical-path playthrough (spec-0004).
Empty stub for now; the compose profile lands with spec-0003.
