// Scripted-teardown death classification. Every removal the story makes that
// is not a death on screen (crates/delvec/src/compiler/emit.rs,
// `removal_lines` with `Exit::Unseen`: a `despawn-npc`, a `despawn-actor`
// `vanish`, the puppet an `unleash` replaces, a bonfire re-seat) moves the body
// to the ABSOLUTE Y -128 down its own column — `execute as … at @s run tp @s
// ~ -128 ~` — and kills it there a few ticks later, so the server broadcasts the
// SAME "<name> died" line a real combat loss would. The message carries no
// signal that this was staging, not a fight.
//
// The distinguishing fact is depth: a real death happens somewhere in the
// playable map; a scripted removal happens below the world's floor, at a Y
// nothing in the delve's story ever visits. Classify on Y, not on cause text.
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
 * `worldMinY - 64` is the compiler's own relocation Y on the overworld's floor
 * (`-64 - 64 = -128`), and a removed body dies there.
 *
 * The harness has no wired source for a delve's `min_y` today — nothing in
 * `critical-path.json` or the run environment carries it (see
 * docs/reference/tools.md) — so absent one this falls back to a fixed
 * `y <= -100`, which every unseen removal passes: the relocation Y is absolute
 * (`~ -128 ~` keeps only X and Z relative), whatever height the body stood at.
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
