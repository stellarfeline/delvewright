// Scripted-teardown death classification (2026-08-06 island triage). The
// compiler's despawn-actor `style: vanish` idiom (crates/compiler/src/emit.rs,
// `emit_despawn_actor`) relocates an actor far below the floor and then kills
// it — `execute as … at @s run tp @s ~ -128 ~` followed by `kill` — so the
// server broadcasts the SAME "<name> died" line a real combat loss would. The
// message carries no signal that this was staging, not a fight: the island
// run's report surfaced five such named-entity deaths with no way to tell which
// were the two scripted vanishes and which were the three real losses. It cost
// a triage cycle.
//
// The distinguishing fact is depth: a real death happens somewhere in the
// playable map; a scripted vanish happens far below it, at a Y nothing in the
// delve's story ever visits. Classify on Y, not on cause text — the compiler
// deliberately reuses the ordinary damage path so the vanish reads as an
// ordinary death on purpose (that is the whole point of the idiom).
//
// This module is pure (no mineflayer import) so the classifier is unit-testable
// with fake positions; wiring it to a real death's observed Y lives in
// executor.ts.

/** How a named entity's death is classified in the run report. */
export type DeathKind = "scripted_teardown" | "combat";

/**
 * The Y depth at or below which a named entity's death is always a scripted
 * teardown, never a real loss.
 *
 * Derived from the delve's own floor when the run context carries one:
 * `worldMinY - 64` sits clear of the compiler's own relocation without
 * hardcoding it — that relocation is `emit_despawn_actor`'s implementation
 * detail (currently 128 blocks, and RELATIVE to each actor's own Y — `execute
 * as … at @s run tp @s ~ -128 ~`, not anchored to `worldMinY`), not a property
 * of the world this classifier should depend on.
 *
 * The harness has no wired source for a delve's `min_y` today — nothing in
 * `critical-path.json` or the run environment carries it (see
 * docs/reference/tools.md) — so absent one this falls back to a fixed
 * `y <= -100` heuristic. KNOWN LIMITATION: because the compiler's relocation is
 * relative to the actor's own Y rather than anchored to the world floor, an
 * actor staged above y≈28 vanishes to a depth SHALLOWER than -100 and this
 * heuristic under-classifies it as `combat`. The island's own vanishes (actors
 * at y≈-55) landed at -128, well past the cutoff — the heuristic is sound for
 * a below-ground box-garden delve, the case it was built for; a min_y-derived
 * threshold is the correct fix for a delve staged nearer the surface, which is
 * exactly why this function accepts one.
 */
export function scriptedTeardownThreshold(worldMinY?: number): number {
  return worldMinY !== undefined ? worldMinY - 64 : -100;
}

/** Classify a single named entity's death by where it happened. */
export function classifyDeathDepth(y: number, worldMinY?: number): DeathKind {
  return y <= scriptedTeardownThreshold(worldMinY) ? "scripted_teardown" : "combat";
}

/** One named entity's death, as observed by the bot's tracker. */
export interface NamedEntityDeath {
  /** The entity's server-assigned custom name (an actor's story name). */
  readonly name: string;
  /** The runtime entity id, for cross-referencing other diagnostics. */
  readonly entityId: number;
  /** Last known position before the entity was removed from the tracker. */
  readonly position: readonly [number, number, number];
}

/** One classified named-entity death, ready for the run report. */
export interface ClassifiedDeath extends NamedEntityDeath {
  readonly kind: DeathKind;
}

/** Classify a batch of observed named-entity deaths. */
export function classifyNamedEntityDeaths(
  deaths: readonly NamedEntityDeath[],
  worldMinY?: number,
): ClassifiedDeath[] {
  return deaths.map((d) => ({ ...d, kind: classifyDeathDepth(d.position[1], worldMinY) }));
}
