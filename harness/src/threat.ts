// Threat tracking: who has been hitting the bot, and how recently.
//
// Why this exists (the-drowned-bell, souls ladder rungs 3–5): the critical-path bot
// only ever attacked mobs belonging to the CURRENT kill objective's wave. A souls
// `ambush` desugars to spawn + unleash with NO kill objective, so an ambusher the bot
// legitimately walked past survives, stalks the bot across the map, and lands free
// hits during the next fight — the bot never once swung back at it, and died with a
// husk from a previous area chewing on it (`nearby(8): … husk(hostile), vindicator,
// vindicator` → `slain by Hollow Gate-Warder`).
//
// A human player hits back. This module is the memory that lets the bot do the same:
// a bounded, clock-injectable record of "entity N damaged me at time T", plus the two
// pure selection rules the executor uses (retaliate in a fight, fight-or-flight on a
// navigation leg). Deliberately free of mineflayer types — candidates are reduced to
// `{id, distance}` — so every rule here is unit-testable without a live server.
//
// This tracker WEAKENS NOTHING. It never suppresses a death, never shortens a wave and
// never satisfies an objective; it only widens the set of entities the bot is willing
// to swing at, and only toward entities that have already drawn its blood.

/**
 * How long (ms) a hit keeps an entity "hostile to the bot". Five seconds is a little
 * over two mob attack cooldowns (vanilla melee mobs strike about every 1s, ~2s for the
 * slower ones), so a mob still actively engaged always stays inside the window, while
 * one the bot has genuinely escaped ages out and is forgotten rather than hunted.
 */
export const THREAT_WINDOW_MS = 5_000;

/**
 * How near (blocks) a recent attacker must be to be worth hitting back at during a
 * kill objective. Vanilla mob melee reach is ~2–3 blocks and the bot's own attack
 * reach is 3; 4 covers a mob that is stepping in and out of reach without letting the
 * bot break off a wave to chase something across the room.
 */
export const RETALIATION_RANGE = 4;

/**
 * Fight-or-flight on a NAVIGATION leg: this many hits inside {@link THREAT_WINDOW_MS},
 * with an attacker still in range, means the bot is not walking away from anything — it
 * is being beaten on, and walking on just donates hits. One hit is a graze taken while
 * passing through; two inside the window is a fight.
 *
 * Counted per-attacker OR across attackers (see {@link pickStalker}): the first
 * the-drowned-bell run of this feature died having taken exactly one hit from EACH of
 * two Hollow Gate-Warders on the approach lane — ~7 damage apiece — and reached the
 * wave anchor at 6/20 without ever swinging back, because no single mob had hit it
 * twice. Two hits is two hits, whoever landed them.
 */
export const STALKER_HITS = 2;
/**
 * How near (blocks) an attacker must stay to be worth stopping the leg for. Matched to
 * {@link RETALIATION_RANGE}: a mob that just hit the bot is by definition inside its
 * own reach, and the bot's swing reaches it back — a tighter radius mostly measures
 * where the chasing mob happened to be at the poll instant, not whether it is on the bot.
 */
export const STALKER_RANGE = RETALIATION_RANGE;

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
 * A bounded, time-windowed record of which entities have damaged the bot.
 *
 * Entries are pruned on every read, so memory is bounded by the number of entities
 * that hit the bot within one window. The clock is injectable so the selection rules
 * are testable without sleeping.
 */
export class ThreatTracker {
  private readonly hits = new Map<number, number[]>();
  private readonly now: () => number;

  constructor(now: () => number = Date.now) {
    this.now = now;
  }

  /** Record that entity `id` damaged the bot (at `at`, default now). */
  record(id: number, at: number = this.now()): void {
    const times = this.hits.get(id);
    if (times) {
      times.push(at);
    } else {
      this.hits.set(id, [at]);
    }
  }

  /** How many hits `id` has landed inside `windowMs`. */
  hitsWithin(id: number, windowMs: number = THREAT_WINDOW_MS): number {
    return this.recent(id, windowMs).length;
  }

  /** Timestamp of `id`'s most recent hit inside `windowMs`, or `undefined`. */
  lastHitAt(id: number, windowMs: number = THREAT_WINDOW_MS): number | undefined {
    const times = this.recent(id, windowMs);
    return times.length > 0 ? times[times.length - 1] : undefined;
  }

  /** Every entity with a hit inside `windowMs`, most recently seen first. */
  active(windowMs: number = THREAT_WINDOW_MS): ThreatSighting[] {
    const out: ThreatSighting[] = [];
    for (const id of [...this.hits.keys()].sort((a, b) => a - b)) {
      const times = this.recent(id, windowMs);
      if (times.length === 0) {
        this.hits.delete(id);
        continue;
      }
      out.push({ id, hits: times.length, lastHitAt: times[times.length - 1]! });
    }
    // Most recent first; ties broken by id so the ordering is deterministic.
    out.sort((a, b) => b.lastHitAt - a.lastHitAt || a.id - b.id);
    return out;
  }

  /** Drop everything remembered about `id` (it died, or was written off). */
  forget(id: number): void {
    this.hits.delete(id);
  }

  /** Drop every record (a new leg / a respawn — old grudges are not carried). */
  clear(): void {
    this.hits.clear();
  }

  private recent(id: number, windowMs: number): number[] {
    const times = this.hits.get(id);
    if (!times) return [];
    const cutoff = this.now() - windowMs;
    const kept = times.filter((t) => t > cutoff);
    if (kept.length === 0) {
      this.hits.delete(id);
    } else if (kept.length !== times.length) {
      this.hits.set(id, kept);
    }
    return kept;
  }
}

/**
 * Which entity the bot should swing at first during a kill objective: the CLOSEST
 * recent attacker inside `range`, ties broken by most-recent hit then id (so the pick
 * is deterministic). `undefined` when nothing that hit the bot lately is in reach — the
 * caller then falls back to ordinary wave targeting.
 *
 * This is the fix for the stalker case: the wave's own mobs are picked by the wave
 * rule, and anything else currently drawing blood — an ambusher from an area the bot
 * already left — is picked here instead of being ignored while it free-hits the bot.
 */
export function pickRetaliationTarget(
  candidates: readonly ThreatCandidate[],
  tracker: ThreatTracker,
  range: number = RETALIATION_RANGE,
  windowMs: number = THREAT_WINDOW_MS,
): number | undefined {
  let best: { id: number; distance: number; lastHitAt: number } | undefined;
  for (const c of candidates) {
    if (c.distance > range) continue;
    const lastHitAt = tracker.lastHitAt(c.id, windowMs);
    if (lastHitAt === undefined) continue;
    if (
      !best ||
      c.distance < best.distance ||
      (c.distance === best.distance &&
        (lastHitAt > best.lastHitAt ||
          (lastHitAt === best.lastHitAt && c.id < best.id)))
    ) {
      best = { id: c.id, distance: c.distance, lastHitAt };
    }
  }
  return best?.id;
}

/**
 * Which entity, if any, is worth STOPPING a navigation leg to deal with.
 *
 * The bot is "in a fight" when `minHits` hits have landed inside the window — either
 * from one mob that latched on, or spread across the mobs currently in range (a lane
 * of two axemen taking one swing each is a fight, not a graze). The target is then the
 * CLOSEST in-range candidate that has actually hit the bot, ties broken by id so the
 * pick is deterministic.
 *
 * Still deliberately stricter than {@link pickRetaliationTarget}: mid-walk a single
 * graze is shrugged off and the leg keeps going, and the caller bounds how long the bot
 * may spend on whatever this returns.
 */
export function pickStalker(
  candidates: readonly ThreatCandidate[],
  tracker: ThreatTracker,
  range: number = STALKER_RANGE,
  minHits: number = STALKER_HITS,
  windowMs: number = THREAT_WINDOW_MS,
): number | undefined {
  const inRange = candidates.filter((c) => c.distance <= range);
  let totalHits = 0;
  let best: { id: number; distance: number } | undefined;
  for (const c of inRange) {
    const hits = tracker.hitsWithin(c.id, windowMs);
    if (hits === 0) continue;
    totalHits += hits;
    if (!best || c.distance < best.distance || (c.distance === best.distance && c.id < best.id)) {
      best = { id: c.id, distance: c.distance };
    }
  }
  return totalHits >= minHits ? best?.id : undefined;
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
