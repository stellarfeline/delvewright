// The sculk family's runtime assertions (spec-0100 §4.7): the bot hears what the
// compiler predicted the proven walk sets off, and nothing it must never see.
//
// The compiler exports, on each leg of `critical-path-waypoints.json`, the sculk
// sensors the leg's proven route sets off and the shriekers that answer each
// (`vibrations`, spec-0100 §4.6). For every predicted sensor the bot records a
// `blockUpdate` at the sensor's cell whose new state has
// `sculk_sensor_phase=active`; for every predicted shrieker a `world_event`
// packet with id 3007 (the shriek) at the shrieker's cell — each from the leg's
// start to {@link VIBRATION_TAIL_MS} after its end. Over the whole run it counts
// every `darkness` effect on itself and every `warden` that spawns.
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

/** How long after a leg's end its predicted events may still arrive: a
 * shrieker answers its sensor after the vibration's travel time, and a sensor
 * clicks on the last footsteps of the leg. */
export const VIBRATION_TAIL_MS = 3_000;

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

/** Integer-cell squared distance. */
export function distSq(a: Vec3Tuple, b: Vec3Tuple): number {
  return (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2;
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

/** The `[sculk]` line a leg prints (spec-0100 §4.7). */
export function legLine(label: string, h: LegHearing): string {
  return (
    `[sculk] ${label}: ${h.sensorsPredicted} sensor(s) predicted, ${h.sensorsHeard} heard; ` +
    `${h.shriekersPredicted} shrieker(s) predicted, ${h.shriekersHeard} heard`
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

  /** Whether every predicted event has been heard in `[from, to]`. */
  hear(vibrations: readonly Vibration[], from: number, to: number): LegHearing {
    const inWindow = (list: readonly Heard[], pos: Vec3Tuple): boolean =>
      list.some((h) => same(h.pos, pos) && h.at >= from && h.at <= to);
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
