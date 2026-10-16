import { test } from "node:test";
import assert from "node:assert/strict";
import {
  GameClockWatch,
  GameTimeStoppedError,
  LIVE_REPLY_GAP_MS,
  STOPPED_CLOCK_MS,
} from "../src/game-time.ts";

/** Feed `watch` one reading per `(tick, answeredAt)`; the error it throws, or `undefined`. */
function feed(watch: GameClockWatch, readings: readonly (readonly [number, number])[]): unknown {
  try {
    for (const [tick, at] of readings) watch.observe(tick, at);
    return undefined;
  } catch (err) {
    return err;
  }
}

/** Readings `stepMs` apart from `t0` for `spanMs`, every one at `tick`. */
function steady(tick: number, t0: number, spanMs: number, stepMs: number): [number, number][] {
  const out: [number, number][] = [];
  for (let at = t0; at <= t0 + spanMs; at += stepMs) out.push([tick, at]);
  return out;
}

test("a frozen game clock, answered promptly, is stopped once the answers span the window", () => {
  const watch = new GameClockWatch("the watch line's tick", 100, 2);
  const err = feed(watch, steady(101, 0, STOPPED_CLOCK_MS + 60, 60));
  assert.ok(err instanceof GameTimeStoppedError, String(err));
  assert.match(
    (err as Error).message,
    /^game time: the watch line's tick: the game clock stopped at tick 101, 1 of 2 game tick\(s\) in: the server answered \d+ reading\(s\) over 10\d\d\dms, each within 500ms/,
  );
  // One answer short of the window is not yet a stop.
  assert.equal(feed(new GameClockWatch("p", 100, 2), steady(101, 0, STOPPED_CLOCK_MS - 60, 60)), undefined);
});

test("a host stall is one long gap: the watch-line red, a 12s stall after one tick, is not a stop", () => {
  // The shape that reded: the start at tick 100, one reading at 101, then the
  // host stalled and the next reply was taken 12s later, still at 101.
  const watch = new GameClockWatch("the watch line's tick", 100, 2);
  assert.equal(feed(watch, [[101, 50], [101, 12_050]]), undefined);
  assert.deepEqual(watch.seen, { replies: 0, ms: 0 }, "a gap past the live limit adds nothing");
  // Stalls forever, each reply far past the live limit: never a stop on its own.
  assert.equal(feed(watch, steady(101, 12_050, 600_000, LIVE_REPLY_GAP_MS + 1)), undefined);
});

test("the slowest live game, one tick a second, answered every 60ms, never stops", () => {
  const readings: [number, number][] = [];
  for (let at = 0; at < 120_000; at += 60) readings.push([1_000 + Math.floor(at / 1_000), at]);
  assert.equal(feed(new GameClockWatch("p", 1_000, 1_000), readings), undefined);
});

test("only the answers at the current tick count: a tick restarts the sum", () => {
  const watch = new GameClockWatch("p", 0, 10);
  const half = STOPPED_CLOCK_MS / 2 + 100;
  assert.equal(feed(watch, steady(5, 0, half, 50)), undefined);
  assert.equal(feed(watch, steady(6, half + 50, half, 50)), undefined);
  assert.ok(watch.seen.ms < STOPPED_CLOCK_MS);
});

test("prompt answers count across a stall: a frozen clock on a stalling host is still stopped", () => {
  const watch = new GameClockWatch("p", 0, 2);
  const first = steady(0, 0, STOPPED_CLOCK_MS / 2, 50);
  const second = steady(0, STOPPED_CLOCK_MS / 2 + 30_000, STOPPED_CLOCK_MS / 2 + 50, 50);
  assert.ok(feed(watch, [...first, ...second]) instanceof GameTimeStoppedError);
});
