// Who hit the bot — attribution, and nothing else.
//
// The ladder does not fight, so there is no target to pick and no exchange to
// win: what remains of this module is the one question the damage handlers ask,
// which is WHICH body drew the bot's blood. The executor answers it by staging
// that body out of the delve, and the run artifact names every such removal.
//
// Deliberately free of mineflayer types — candidates are reduced to
// `{id, distance}` — so the rule is unit-testable without a live server.

/**
 * Fallback attribution radius (blocks). When the server's damage packet carries no
 * source entity (see `attributeBotDamage`), a hostile inside this radius is the honest
 * culprit for a melee hit — just outside vanilla mob reach, so a bystander across the
 * room is never blamed.
 */
export const ATTRIBUTION_RANGE = 4.5;

/** A hostile the bot can currently see, reduced to what the threat rules need. */
export interface ThreatCandidate {
  readonly id: number;
  /** Distance (blocks) from the bot. */
  readonly distance: number;
}

/** What the tracker knows about one entity. */
export interface ThreatSighting {
  readonly id: number;
  /** Hits recorded inside the query window. */
  readonly hits: number;
  /** Timestamp (ms) of the most recent recorded hit. */
  readonly lastHitAt: number;
}

/**
 * Decide WHO dealt a hit the bot just took.
 *
 * Primary channel (what mineflayer 4.37 reliably gives on 1.21.11): the 1.20+
 * `damage_event` packet carries `sourceCauseId` — "the id + 1 of the entity
 * responsible for the damage, if present, else 0" — which mineflayer resolves and
 * re-emits as `entityHurt(entity, source)` (lib/plugins/entities.js). When the hurt
 * entity is the bot and `source` names a live hostile, that IS the attacker: no
 * guessing.
 *
 * Fallback: `sourceCauseId` is 0 for damage with no responsible entity, and the
 * lookup also yields nothing if the source entity is not tracked client-side. Then the
 * nearest hostile inside {@link ATTRIBUTION_RANGE} is blamed — a mob that close is
 * within vanilla melee reach, and if nothing is that close (fall damage, drowning, a
 * trap) NOTHING is blamed, which is the point: the bot must never "retaliate" against
 * a bystander for a hazard.
 *
 * Pure: the caller supplies the resolved source id and the visible hostiles.
 */
export function attributeBotDamage(
  sourceId: number | undefined,
  candidates: readonly ThreatCandidate[],
  range: number = ATTRIBUTION_RANGE,
): number | undefined {
  if (sourceId !== undefined && candidates.some((c) => c.id === sourceId)) {
    return sourceId;
  }
  let best: { id: number; distance: number } | undefined;
  for (const c of candidates) {
    if (c.distance > range) continue;
    if (!best || c.distance < best.distance || (c.distance === best.distance && c.id < best.id)) {
      best = { id: c.id, distance: c.distance };
    }
  }
  return best?.id;
}
