// Kill-objective accounting: which of the bot's kills count as wave progress, and
// when a wave is finished.
//
// Why this is its own module (the-drowned-bell, souls ladder run 13): self-defense
// (#173) let the bot kill mobs OUTSIDE the kill loop — a wave husk that attacked it on
// the approach leg died to the fight-or-flight path
// (`[defend] husk#43 is down; resuming wave wave/grave-echoes waypoint 11/12`) and was
// never counted, because #173 credited only mobs the kill LOOP had targeted. The step
// then could not finish: `killed` could never reach `step.count`, and the degraded
// "no eligible mob remains" exit never fired either, so the objective burned its whole
// 90s budget and failed a wave the bot had actually beaten.
//
// The accounting rules live here, pure and entity-free, so both call sites (the kill
// loop and the defense path) share one definition and it is unit-testable.

/** A position, reduced to what the accounting rules need. */
export interface Point {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

/**
 * How near the wave anchor (blocks) an entity's last-known position must be, when it
 * winks out, to count as a bot-inflicted wave kill rather than a chunk unload or a far
 * despawn. Wave mobs are fought at the anchor and die in melee beside the bot; a mob
 * vanishing well away from the anchor is not a confirmed kill. Comfortably larger than
 * the melee envelope, tighter than the cross-area gaps (~64+ blocks) that separate the
 * wave from Invulnerable actors elsewhere in the delve.
 */
export const WAVE_KILL_NEAR = 16;

/**
 * How near the BOT (blocks) a hostile must be to still be part of the fight when
 * deciding a wave is over. Wave mobs are aggroed and close on the bot, so anything
 * further than this is not something the bot is going to clear by walking at it — it
 * is another area's actor. Deliberately wider than {@link WAVE_KILL_NEAR} so a mob
 * circling the arena still counts as present.
 */
export const WAVE_ENGAGE_NEAR = 32;

/**
 * Consecutive polls a terminal condition must hold before the wave is called. Guards
 * against a one-frame gap (a mob mid-respawn, an entity not yet tracked client-side)
 * ending a fight early.
 */
export const WAVE_CLEAR_STREAK = 8;

/**
 * Accounting for the kill step currently in progress. Armed for the WHOLE step —
 * including the walk to the anchor — so a wave mob killed in self-defense on the way in
 * is credited exactly as one killed at the anchor.
 */
export interface WaveEngagement {
  /** The wave id, for logs. */
  readonly wave: string;
  /** The wave anchor (block centre), against which kills are judged. */
  readonly anchor: Point;
  /** Every entity the bot has attacked while this wave was the objective. */
  readonly engaged: Set<number>;
  /** Entities already counted, so a re-fired `entityGone` cannot double-count. */
  readonly credited: Set<number>;
  /** Confirmed bot-inflicted kills. */
  killed: number;
}

/** Start accounting for `wave`, anchored at the block centre of `pos`. */
export function beginWave(wave: string, pos: readonly [number, number, number]): WaveEngagement {
  return {
    wave,
    anchor: { x: pos[0] + 0.5, y: pos[1], z: pos[2] + 0.5 },
    engaged: new Set(),
    credited: new Set(),
    killed: 0,
  };
}

/**
 * Whether a vanished entity counts as a wave kill.
 *
 * The rule is the kill loop's own, unchanged: a mob the bot ATTACKED while this wave
 * was the objective, whose last known position was within {@link WAVE_KILL_NEAR} of the
 * anchor. What changed is only WHO may feed it — the defense path now does too, because
 * the mob it killed was, by that same standard, a mob of this wave.
 *
 * It is deliberately a proximity rule and not an identity one: mineflayer on 1.21.11
 * cannot read the entity tags that would name a wave's members (`KillStep.tag` is
 * informational). The residual is that a non-wave stalker killed in the middle of the
 * arena is credited — the standard the kill loop has always used. The alternative,
 * refusing to credit anything the loop did not personally target, is what deadlocked
 * run 13: it makes a wave the bot has beaten impossible to finish.
 */
export function creditsWaveKill(
  engagement: WaveEngagement,
  id: number,
  deathPos: Point | undefined,
  near: number = WAVE_KILL_NEAR,
): boolean {
  if (!engagement.engaged.has(id)) return false;
  if (engagement.credited.has(id)) return false;
  if (!deathPos) return false;
  const a = engagement.anchor;
  return Math.hypot(deathPos.x - a.x, deathPos.y - a.y, deathPos.z - a.z) <= near;
}

/**
 * Whether the fight is over judged by the LIVE mobs, not by the wave's declared count.
 *
 * True when the bot has engaged at least one mob, every mob it engaged is down (gone,
 * or written off as unkillable), and no hostile it could still fight is within
 * {@link WAVE_ENGAGE_NEAR}. That is the honest reading of "the wave is beaten" when
 * some member died in a way the confirmed-kill counter could not attribute — the case
 * `killed >= count` can never see, because `count` is the wave's ORIGINAL size.
 *
 * Conservative by construction: it cannot fire before the bot has fought something, and
 * it cannot fire while anything hostile is still near enough to be part of the fight.
 */
export function waveEngagementCleared(opts: {
  readonly engagedIds: readonly number[];
  /** Whether the engaged entity is gone or blacklisted as unkillable. */
  readonly isDown: (id: number) => boolean;
  /** Distance to the nearest hostile the bot could still target, if any. */
  readonly nearestEligibleDistance: number | undefined;
  readonly near?: number;
}): boolean {
  if (opts.engagedIds.length === 0) return false;
  if (!opts.engagedIds.every((id) => opts.isDown(id))) return false;
  const near = opts.near ?? WAVE_ENGAGE_NEAR;
  return opts.nearestEligibleDistance === undefined || opts.nearestEligibleDistance > near;
}
