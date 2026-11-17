// A wait on the server's answer, counted on the server's own clock.
//
// A command the bot sends is run by the server at the start of a tick, and its
// answer comes back in that tick's packets. A wait of N ms on a server that
// ticks 20 times a second is a wait of N/50 ticks; a server whose tick stands
// still (its host swapping, its CPUs taken by another tenant) answers nothing
// until it runs again, however long the bot waits. A wall-clock deadline then
// judges the host, never the server: the read that failed `ride read: no
// answer in 5000ms` was sent at the start of a 21 s stall and answered in the
// first tick after it.
//
// So a deadline is over only when its span has passed on BOTH clocks: the wall
// clock, and the world age the server sends from its own tick loop
// (`update_time`, every 20 ticks; mineflayer's `bot.time.age`). A server whose
// clock stands still holds the wait open; a server that ticks and does not
// answer fails it a packet late at most. Bounded on the wall by
// {@link SERVER_STALL_CEILING_MS} past the span: vanilla's watchdog halts a
// server one tick of which runs that long (`max-tick-time`, 60 000 ms by
// default), so a clock that stood still for longer belongs to a server that is
// gone, and the connection's end says so.
//
// With no world age in hand (no clock packet yet, or a test double without
// one) the deadline is the wall clock alone. This file is pure: times are the
// caller's monotonic milliseconds, ages the caller's readings.

/** Milliseconds per server tick at vanilla's 20 ticks a second. */
export const MS_PER_TICK = 50;

/**
 * How long (ms) past its span a deadline waits on a server whose clock stands
 * still: vanilla's default `max-tick-time`, the longest one tick runs before
 * the server's own watchdog halts it.
 */
export const SERVER_STALL_CEILING_MS = 60_000;

/** A deadline of `ms` started at wall time `now`, world age `age`. */
export class ServerDeadline {
  readonly ms: number;
  private readonly startedAt: number;
  private readonly startAge: number | undefined;

  constructor(ms: number, now: number, age: number | undefined) {
    this.ms = ms;
    this.startedAt = now;
    this.startAge = age;
  }

  /** Game ticks the server's clock advanced since the start, or `undefined` with no age in hand. */
  ticks(age: number | undefined): number | undefined {
    return this.startAge === undefined || age === undefined ? undefined : age - this.startAge;
  }

  /** Whether the wait is over at wall time `now` and world age `age`. */
  over(now: number, age: number | undefined): boolean {
    const wall = now - this.startedAt;
    if (wall < this.ms) return false;
    const ticks = this.ticks(age);
    if (ticks === undefined) return true;
    return ticks * MS_PER_TICK >= this.ms || wall >= this.ms + SERVER_STALL_CEILING_MS;
  }

  /** The span a wait that is over ran, in words: wall time, and the server's ticks when known. */
  spent(now: number, age: number | undefined): string {
    const wall = Math.round(now - this.startedAt);
    const ticks = this.ticks(age);
    if (ticks === undefined) return `${this.ms}ms`;
    if (ticks * MS_PER_TICK >= this.ms) return `${this.ms}ms (${ticks} server tick(s) over ${wall}ms)`;
    return (
      `${wall}ms, over which the server's clock advanced only ${ticks} tick(s) of the ` +
      `${Math.ceil(this.ms / MS_PER_TICK)} the wait counts — its tick stood still past the ` +
      `${SERVER_STALL_CEILING_MS}ms vanilla's watchdog allows`
    );
  }
}
