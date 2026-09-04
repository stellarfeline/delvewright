// Kill-objective accounting: which of the bot's kills count as wave progress, and
// when a wave is finished.
//
// Why this is its own module (the-drowned-bell, souls ladder run 13): self-defense
// let the bot kill mobs OUTSIDE the kill loop — a wave husk that attacked it on
// the approach leg died to the fight-or-flight path
// (`[defend] husk#43 is down; resuming wave wave/grave-echoes waypoint 11/12`) and was
// never counted, because only mobs the kill LOOP had targeted were credited. The step
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

// ---------------------------------------------------------------------------
// The wave's clear, as the SERVER states it
// ---------------------------------------------------------------------------

/**
 * How many consecutive census answers must report nothing of the wave standing
 * before the fight is called over.
 *
 * Deliberately much smaller than {@link WAVE_CLEAR_STREAK}, and the reason is
 * that the two guard different things. The client-side streak guards a one-FRAME
 * gap in entity tracking: the bot is looking at a world it is told about late and
 * incompletely, so a single empty poll means very little. A census is one atomic
 * server function walking the wave's own tag, and it counts a body still playing
 * its death animation as present — it errs toward saying the wave STANDS, which
 * is the safe direction. Two agreeing answers, each its own round trip, rule out
 * a torn read without paying eight of them.
 */
export const WAVE_CENSUS_CLEAR_STREAK = 2;

/**
 * What this step's censuses have said so far.
 *
 * The kill step's terminal condition is the server's answer, not the bot's tally
 * of what it watched die. Two failures made that necessary, and the second is the
 * one no client-side rule can reach:
 *
 *   * a body that dies in a way the proximity rule cannot attribute — off the
 *     anchor, in a lethal volume, down a fall — is never credited, so
 *     `killed >= count` is unreachable for the whole rest of the step;
 *   * a scripted die-retry death RE-SEATS a `respawns_on_rest` wave, and a
 *     re-seat resets what the server says while a private counter carries on
 *     counting a cohort that no longer exists.
 *
 * Measured on the gallery ladder: the bot confirmed one kill of a three-body
 * wave whose other two had already withered and fallen, and `1/3` was as far as
 * that counter could ever get.
 */
export interface WaveCensusWatch {
  /** Census answers received this step. */
  answers: number;
  /** Consecutive answers reporting zero standing, since the last non-zero one. */
  clearStreak: number;
  /** The most mobs any answer reported standing — the step's own binding count. */
  peakStanding: number;
  /** The last answer's standing count, or `undefined` before the first answer. */
  standing: number | undefined;
  /**
   * Whether any answer has been positive EVIDENCE THAT THE WAVE EXISTS — bodies
   * of it standing, or deaths of it credited to the party since its seating.
   *
   * This is what stops the terminal condition being vacuous. "Nothing of this
   * wave stands" is the same sentence whether the wave is beaten or has simply
   * not been seated yet, and a step that ended on the second reading would report
   * a fight nobody fought. Both facts here are positive: a body seen, or a body
   * felled. Neither can be produced by a wave that never spawned.
   */
  seen: boolean;
}

/** One census's answer, as the terminal condition reads it. */
export interface CensusAnswer {
  /** Bodies of the wave standing. */
  readonly present: number;
  /** Deaths of the wave the party was credited with, since its seating. */
  readonly credited: number;
}

/** A fresh watch, before any census has answered. */
export function beginCensusWatch(): WaveCensusWatch {
  return { answers: 0, clearStreak: 0, peakStanding: 0, standing: undefined, seen: false };
}

/**
 * Record one census answer. A census that did not come back is NOT an answer —
 * pass `undefined` and nothing moves, because a silent zero would read as "the
 * wave is gone" and end a fight on a broken probe.
 */
export function observeCensus(w: WaveCensusWatch, answer: CensusAnswer | undefined): void {
  if (answer === undefined) return;
  w.answers += 1;
  w.standing = answer.present;
  w.peakStanding = Math.max(w.peakStanding, answer.present);
  w.clearStreak = answer.present === 0 ? w.clearStreak + 1 : 0;
  if (answer.present > 0 || answer.credited > 0) w.seen = true;
}

/**
 * Whether the server has said, consistently, that nothing of this wave stands —
 * having first said the wave was there at all.
 *
 * Two conditions and both are load-bearing. {@link WaveCensusWatch.seen} is the
 * anti-vacuity half: without it, a census asked before a spawn or a re-seat lands
 * reports zero and would end a fight nobody fought. The streak is the torn-read
 * half. A watch nobody fed clears nothing, so a probe that never answers can
 * never end a step; the pre-census exits still cover that, saying so.
 */
export function censusCleared(
  w: WaveCensusWatch,
  streak: number = WAVE_CENSUS_CLEAR_STREAK,
): boolean {
  return w.seen && w.clearStreak >= streak;
}

/**
 * Whether the fight LOOKS over from the client — every mob the bot engaged is down
 * and nothing hostile is near enough to still be part of it.
 *
 * A driver, never a terminal condition: what ENDS the step is
 * {@link censusCleared}, the server's own answer. This is what prompts the step to
 * ask, and it is deliberately conservative — it cannot fire before the bot has
 * fought something, and it cannot fire while anything hostile is within
 * {@link WAVE_ENGAGE_NEAR}.
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
