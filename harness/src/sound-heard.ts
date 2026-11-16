// Where a sound packet the bot heard landed, judged against where the bot
// stands: the one rule every sound judgement in the harness reads (the quest
// cues, the containers' arrival notice).

type Vec3 = readonly [number, number, number];

/**
 * How far (blocks, per axis) a sound may land from the bot and still be heard
 * where the bot is. The packet carries its position in eighths of a block, and
 * the body may move a step, or settle onto the floor after a teleport, between
 * the server playing the sound and the client reading it; a sound played at the
 * command source instead of the player misses by the distance between them.
 */
export const SOUND_POS_TOLERANCE = 1.5;

/** A sound id without its `minecraft:` namespace, the form two ids compare in. */
export const bareSound = (s: string): string => s.replace(/^minecraft:/, "");

/** The packet landed at the bot: every axis within [`SOUND_POS_TOLERANCE`]. */
export function heardAt(heard: Vec3, bot: Vec3 | null): boolean {
  return bot !== null && heard.every((v, i) => Math.abs(v - bot[i]!) <= SOUND_POS_TOLERANCE);
}

/**
 * **Where the server may have held the bot when it ran the tick a packet came from.**
 *
 * A sound played at a player plays at the position the server holds for that
 * player in the tick the command runs: the last position the bot told it. The
 * bot's own position when the packet arrives is later than that by the time the
 * server took to run the tick and send it, and a walking bot is that far on
 * along its path. On an idle server the gap is a tick; on one that has fallen
 * behind the clock it is as long as the stall.
 *
 * The bound comes from the server's own clock packets, which it sends from its
 * tick loop and which carry the world age. Two readings of different ages were
 * sent from two different ticks, the older at least one tick before any tick
 * that has not yet sent its packets; by the time the server runs a later tick it
 * has handled what the bot told it after the older reading arrived. So the held
 * position is one the bot told at or after the second-latest reading arrived,
 * or the last one it told before (the one standing at that moment). Before two
 * readings have arrived, nothing bounds it, and the only position given is the
 * bot's own at arrival.
 *
 * The positions are the ones the bot sent (mineflayer's `move`, one per physics
 * tick that changed it) and the ones the server set (`forcedMove`). Times are the
 * caller's clock in milliseconds; the class reads none.
 */
export class ServerView {
  private told: { at: number; pos: Vec3 }[] = [];
  /** The two latest clock readings of distinct world ages, oldest first. */
  private clock: { at: number; age: number }[] = [];

  /** The bot told the server it stands at `pos` (or the server put it there). */
  tell(at: number, pos: Vec3): void {
    this.told.push({ at, pos });
  }

  /** A clock packet of world age `age` arrived. */
  clockRead(at: number, age: number): void {
    const last = this.clock[this.clock.length - 1];
    if (last && age <= last.age) return;
    this.clock.push({ at, age });
    if (this.clock.length > 2) this.clock.shift();
    const from = this.windowStart();
    if (from === undefined) return;
    // Keep the last position told before the window: it stood at its start.
    let keep = 0;
    while (keep + 1 < this.told.length && this.told[keep + 1]!.at < from) keep += 1;
    this.told = this.told.slice(keep);
  }

  /** When the window opens, or `undefined` before two readings have arrived. */
  windowStart(): number | undefined {
    return this.clock.length < 2 ? undefined : this.clock[0]!.at;
  }

  /**
   * The positions the server may have held for the bot in the tick whose packet
   * arrives now, oldest first, `now` (the bot's own position at arrival) last.
   */
  heldAt(now: Vec3): Vec3[] {
    const from = this.windowStart();
    if (from === undefined) return [now];
    const out: Vec3[] = [];
    let before: Vec3 | undefined;
    for (const t of this.told) {
      if (t.at < from) before = t.pos;
      else out.push(t.pos);
    }
    return [...(before ? [before] : []), ...out, now];
  }
}
