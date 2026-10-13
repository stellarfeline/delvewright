// The sculk family's runtime assertions (spec-0100 §4.7): the bot hears what the
// compiler predicted the proven walk sets off, and nothing it must never see.
//
// The compiler exports, on each leg of `critical-path-waypoints.json`, the sculk
// sensors the leg's proven route sets off and the shriekers that answer each
// (`vibrations`, spec-0100 §4.6). For every predicted sensor the bot records a
// `blockUpdate` at the sensor's cell whose new state has
// `sculk_sensor_phase=active`; for every predicted shrieker a `world_event`
// packet with id 3007 (the shriek) at the shrieker's cell — each from the leg's
// start to {@link VIBRATION_TAIL_TICKS} game ticks after its end. Over the whole
// run it counts every `darkness` effect on itself and every `warden` that spawns.
//
// Every span here is a span of the server's game ticks, read from the server
// (`game-time.ts`), never stood for by wall time: a server that falls behind
// stretches each of them.
//
// Assertions only: the harness predicts nothing itself. A predicted event not
// heard fails the step; a darkness effect or a warden fails the run.

import type { Vec3Tuple } from "./critical-path.ts";

/** One vibration the compiler predicts a leg makes: the sensor, and the
 * shriekers that answer it. */
export interface Vibration {
  readonly sensor: Vec3Tuple;
  readonly shriekers: readonly Vec3Tuple[];
}

/** The `world_event` id the server sends when a shrieker shrieks
 * (`SculkShriekerBlockEntity.shriek` → `levelEvent(3007)`). */
export const SHRIEK_LEVEL_EVENT = 3007;

/**
 * How many game ticks after a leg's end its predicted events may still arrive:
 * a sensor clicks on the last footsteps of the leg after the vibration's travel
 * (`floor(distance)` ticks, at most 8 for a sensor, 16 for a calibrated one),
 * and a shrieker answers the click after its own travel (at most 8). Authored:
 * three seconds of a server at 20 ticks a second, over spec-0100 §2.2 and §2.4.
 */
export const VIBRATION_TAIL_TICKS = 60;

/**
 * A device's busy phases, which the leg's start reads off the server. A sensor
 * that clicked is deaf while its phase is `active` and then `cooldown`
 * (`SculkSensorBlock.ACTIVE_TICKS` 30 — `CalibratedSculkSensorBlock` 10 — then
 * `COOLDOWN_TICKS` 10; `canActivate` is phase `inactive`), and a shrieker that
 * shrieked hears nothing while `shrieking` (90 ticks) — spec-0100 §2.2 and
 * §2.4, the constants read off the pinned 1.21.11 server jar. A device found in
 * one of these states at the leg's start was set off by an earlier event inside
 * its busy span, counted by the game in its own ticks; that event is the one
 * this leg would have made, and no later one can come until it is over.
 */
export const SENSOR_BUSY_STATES: readonly string[] = [
  "minecraft:sculk_sensor[sculk_sensor_phase=active]",
  "minecraft:sculk_sensor[sculk_sensor_phase=cooldown]",
  "minecraft:calibrated_sculk_sensor[sculk_sensor_phase=active]",
  "minecraft:calibrated_sculk_sensor[sculk_sensor_phase=cooldown]",
];
export const SHRIEKER_BUSY_STATES: readonly string[] = ["minecraft:sculk_shrieker[shrieking=true]"];

/** A sensor's listener radius squared: the parse refuses a prediction the
 * game could not make (spec-0100 §2.6). */
export const LISTENER_RADIUS_SQ = 64;

interface Heard {
  readonly pos: Vec3Tuple;
  readonly at: number;
}

function same(a: Vec3Tuple, b: Vec3Tuple): boolean {
  return a[0] === b[0] && a[1] === b[1] && a[2] === b[2];
}

function cellKey(p: Vec3Tuple): string {
  return `${p[0]} ${p[1]} ${p[2]}`;
}

/** The line a busy device makes the server speak in leg-start reading `serial`. */
function busyLine(serial: number, p: Vec3Tuple): string {
  return `[dw:sculk ${serial} busy ${cellKey(p)}]`;
}

/**
 * The commands that ask, in the tick of game-time reading `serial`, which of the
 * leg's predicted devices is busy ({@link SENSOR_BUSY_STATES},
 * {@link SHRIEKER_BUSY_STATES}). A device in an unloaded chunk is not ticking,
 * so it cannot be busy, and is asked nothing rather than refused.
 */
export function legStartCommands(serial: number, vibrations: readonly Vibration[]): string[] {
  const out: string[] = [];
  const asked = new Set<string>();
  const ask = (p: Vec3Tuple, states: readonly string[]): void => {
    const key = cellKey(p);
    if (asked.has(key)) return;
    asked.add(key);
    for (const state of states) {
      out.push(
        `/execute if loaded ${key} if block ${key} ${state} run tellraw @s ` +
          JSON.stringify({ text: busyLine(serial, p) }),
      );
    }
  };
  for (const v of vibrations) {
    ask(v.sensor, SENSOR_BUSY_STATES);
    for (const s of v.shriekers) ask(s, SHRIEKER_BUSY_STATES);
  }
  return out;
}

/** The devices reading `serial` found busy, by cell, out of its reply lines. */
export function parseLegStartBusy(serial: number, lines: readonly string[]): Set<string> {
  const prefix = `[dw:sculk ${serial} busy `;
  const busy = new Set<string>();
  for (const line of lines) {
    const at = line.indexOf(prefix);
    if (at < 0) continue;
    const m = /^(-?\d+) (-?\d+) (-?\d+)\]/.exec(line.slice(at + prefix.length));
    if (m) busy.add(`${m[1]} ${m[2]} ${m[3]}`);
  }
  return busy;
}

/**
 * Where a leg's hearing starts: when it opened (wall stamp — an event that
 * arrives after it is the leg's), the game tick the server read at that start,
 * the wall stamp at which that reading's reply had arrived, and which of the
 * leg's devices the server found busy in that tick.
 */
export interface LegStart {
  readonly at: number;
  readonly tick: number;
  readonly readAt: number;
  readonly busy: ReadonlySet<string>;
}

/** What a leg heard of what it was predicted to make. */
export interface LegHearing {
  readonly sensorsPredicted: number;
  readonly sensorsHeard: number;
  readonly shriekersPredicted: number;
  readonly shriekersHeard: number;
  /** Each predicted event not heard, in words. */
  readonly missing: readonly string[];
}

/**
 * The `[sculk]` line a leg prints (spec-0100 §4.7), with the game ticks it was
 * judged over: the start's tick and the devices busy in it, and the tick of the
 * last reading the hearing waited for (`undefined` when nothing was missing at
 * the leg's end, so no tail was waited).
 */
export function legLine(label: string, h: LegHearing, start: LegStart, heardThrough: number | undefined): string {
  return (
    `[sculk] ${label}: ${h.sensorsPredicted} sensor(s) predicted, ${h.sensorsHeard} heard; ` +
    `${h.shriekersPredicted} shrieker(s) predicted, ${h.shriekersHeard} heard ` +
    `(leg start at game tick ${start.tick}, ${start.busy.size} predicted device(s) busy then` +
    (heardThrough === undefined ? "" : `; heard through game tick ${heardThrough}`) +
    `)`
  );
}

/**
 * The run's ear for the sculk family: every sensor click, every shriek, every
 * darkness effect on the bot and every warden spawn, timestamped as they
 * arrive. Fed by the executor's listeners; judged by {@link hear} and
 * {@link runFailure}.
 */
export class SculkEar {
  private readonly clicks: Heard[] = [];
  private readonly shrieks: Heard[] = [];
  /** Darkness effects applied to the bot. */
  darkness = 0;
  /** Wardens that spawned. */
  wardens = 0;

  /** A block changed: a sensor whose new state is `active` clicked. */
  onBlockUpdate(pos: Vec3Tuple, props: Record<string, unknown> | undefined, at: number): void {
    if (props?.["sculk_sensor_phase"] === "active") this.clicks.push({ pos, at });
  }

  /** A `world_event` packet: id 3007 is a shriek. */
  onWorldEvent(id: number, pos: Vec3Tuple, at: number): void {
    if (id === SHRIEK_LEVEL_EVENT) this.shrieks.push({ pos, at });
  }

  /** An effect applied to the bot itself, by registry name. */
  onSelfEffect(name: string | undefined): void {
    if ((name ?? "").toLowerCase() === "darkness") this.darkness += 1;
  }

  /** An entity spawned, by registry name. */
  onSpawn(name: string | undefined): void {
    if (name === "warden") this.wardens += 1;
  }

  /**
   * Whether every predicted event has been heard between the leg's start and
   * `to`. An event before the start answers for the leg only when the server
   * found its device busy in the start's tick ({@link LegStart.busy}): it was
   * then set off inside its busy span, counted in game ticks by the game itself,
   * and the event the ear heard for it arrived before that reading's reply.
   */
  hear(vibrations: readonly Vibration[], start: LegStart, to: number): LegHearing {
    const inWindow = (list: readonly Heard[], pos: Vec3Tuple): boolean =>
      list.some(
        (h) =>
          same(h.pos, pos) &&
          h.at <= to &&
          (h.at >= start.at || (start.busy.has(cellKey(pos)) && h.at <= start.readAt)),
      );
    const missing: string[] = [];
    let sensorsHeard = 0;
    let shriekersPredicted = 0;
    let shriekersHeard = 0;
    for (const v of vibrations) {
      if (inWindow(this.clicks, v.sensor)) {
        sensorsHeard += 1;
      } else {
        missing.push(`the sensor at [${v.sensor.join(", ")}] never turned active`);
      }
      for (const s of v.shriekers) {
        shriekersPredicted += 1;
        if (inWindow(this.shrieks, s)) {
          shriekersHeard += 1;
        } else {
          missing.push(
            `the shrieker at [${s.join(", ")}] never shrieked (no world_event ${SHRIEK_LEVEL_EVENT} ` +
              `at its cell) for the sensor at [${v.sensor.join(", ")}]`,
          );
        }
      }
    }
    return {
      sensorsPredicted: vibrations.length,
      sensorsHeard,
      shriekersPredicted,
      shriekersHeard,
      missing,
    };
  }

  /** The run's closing `[sculk]` line. */
  runLine(): string {
    return `[sculk] darkness ${this.darkness}, warden ${this.wardens}`;
  }

  /** Why the run fails, or `undefined`: a darkness effect or a warden. */
  runFailure(): string | undefined {
    if (this.darkness === 0 && this.wardens === 0) return undefined;
    return (
      `${this.runLine().slice("[sculk] ".length)} — a shrieker with can_summon=false applies no ` +
      `darkness and summons nothing (spec-0100 §2.4), so the shipped world holds a shrieker the ` +
      `engine did not prove`
    );
  }
}
