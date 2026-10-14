// spec-0108 §6.3 — the party stands in the world the ending left.
//
// The compiler exports, on the terminal `assert-complete` step, every aftermath
// effect of the scene of the ending the path fires: what changed in the world
// (a block, a body, the clock, the weather, a score) and how it can be seen.
// After the campaign marker and the tail, the bot reads what a player can read —
// a block at a cell, its own position, its health, the world time and weather,
// an effect on itself — and reports `not_observable` with the verb for the rest.
//
// Assertions + navigation only (the harness contract): every cell, block, clock
// value and destination is read off the compiler's artifact. What the harness
// owns is the reading.

/** What one aftermath effect left to look at, as the compiler exported it. */
export type AftermathObservable =
  | { readonly kind: "block"; readonly first: Vec3; readonly last: Vec3; readonly block: string }
  | {
      readonly kind: "position";
      readonly from: { readonly min: Vec3; readonly max: Vec3 };
      readonly to: Vec3;
    }
  | { readonly kind: "time"; readonly daytime: number }
  | { readonly kind: "weather"; readonly weather: string }
  | { readonly kind: "health"; readonly amount: number }
  | { readonly kind: "effect"; readonly effect: string }
  | { readonly kind: "no-effect"; readonly effect: string | null }
  /** A kind only the server can read (a score, a tag, a destination). */
  | { readonly kind: string };

type Vec3 = readonly [number, number, number];

/** One aftermath row of the `assert-complete` step. */
export interface AftermathEntry {
  /** The effect's JSON pointer in the campaign. */
  readonly path: string;
  /** Its verb's tag. */
  readonly verb: string;
  /** What it left to look at, or `emission-only`. */
  readonly observable: AftermathObservable | "emission-only";
}

/** What the bot can read of the world after the ending. */
export interface WorldReading {
  /** The block name at a cell (`minecraft:stone`), or `undefined` when unloaded. */
  blockAt(pos: Vec3): string | undefined;
  /** The bot's own position. */
  readonly position: Vec3;
  /** Where the bot stood when the path's last objective completed. */
  readonly stoodAt: Vec3 | undefined;
  /** `time of day` in ticks, `0..24000`. */
  readonly timeOfDay: number;
  readonly raining: boolean;
  readonly thundering: boolean;
  readonly health: number;
  /** The ids of the effects on the bot (`minecraft:speed`). */
  readonly effects: ReadonlySet<string>;
}

/** One aftermath effect, read. */
export interface AftermathResult {
  readonly path: string;
  readonly verb: string;
  readonly status: "observed" | "failed" | "not_observable";
  readonly detail: string;
}

const inBox = (p: Vec3, min: Vec3, max: Vec3): boolean =>
  [0, 1, 2].every((i) => Math.floor(p[i]!) >= min[i]! && Math.floor(p[i]!) <= max[i]!);

const fmt = (p: Vec3): string => `[${p.map((n) => Math.round(n * 100) / 100).join(", ")}]`;

const strip = (id: string): string => id.replace(/^minecraft:/, "");

/** Read every aftermath effect against the world. Pure: the bot supplies the reading. */
export function observeAftermath(
  entries: readonly AftermathEntry[],
  world: WorldReading,
): AftermathResult[] {
  return entries.map((e): AftermathResult => {
    const row = (status: AftermathResult["status"], detail: string): AftermathResult => ({
      path: e.path,
      verb: e.verb,
      status,
      detail,
    });
    const o = e.observable;
    if (o === "emission-only") {
      return row("not_observable", `${e.verb}: emission-only (client state the bot cannot read)`);
    }
    switch (o.kind) {
      case "block": {
        const b = o as Extract<AftermathObservable, { kind: "block" }>;
        const misses: string[] = [];
        for (const at of [b.first, b.last]) {
          const got = world.blockAt(at);
          if (got === undefined) {
            return row("not_observable", `${e.verb}: the cell ${fmt(at)} is not loaded`);
          }
          if (strip(got) !== strip(b.block)) misses.push(`${fmt(at)} is ${got}`);
        }
        return misses.length === 0
          ? row("observed", `${b.block} at ${fmt(b.first)} and ${fmt(b.last)}`)
          : row("failed", `expected ${b.block}: ${misses.join("; ")}`);
      }
      case "position": {
        const p = o as Extract<AftermathObservable, { kind: "position" }>;
        if (world.stoodAt === undefined || !inBox(world.stoodAt, p.from.min, p.from.max)) {
          return row(
            "not_observable",
            `${e.verb}: the bot did not stand inside ${fmt(p.from.min)}..${fmt(p.from.max)} ` +
              `when the scene played (stood at ${world.stoodAt ? fmt(world.stoodAt) : "unknown"})`,
          );
        }
        const target: Vec3 = [p.to[0] + 0.5, p.to[1], p.to[2] + 0.5];
        const d = Math.hypot(
          world.position[0] - target[0],
          world.position[1] - target[1],
          world.position[2] - target[2],
        );
        return d <= 1.5
          ? row("observed", `carried from the volume to ${fmt(p.to)} (${d.toFixed(2)} away)`)
          : row("failed", `expected to stand within 1.5 of ${fmt(p.to)}, stood at ${fmt(world.position)}`);
      }
      case "time": {
        const t = o as Extract<AftermathObservable, { kind: "time" }>;
        const day = ((world.timeOfDay % 24000) + 24000) % 24000;
        // The clock runs on after the cut only if the daylight cycle does; a
        // sealed world holds it, so the reading is the cut give or take the
        // ticks the bot took to read it.
        const drift = (((day - t.daytime) % 24000) + 24000) % 24000;
        return drift <= 200
          ? row("observed", `time of day ${day} (cut to ${t.daytime})`)
          : row("failed", `expected time of day ${t.daytime}, read ${day}`);
      }
      case "weather": {
        const w = o as Extract<AftermathObservable, { kind: "weather" }>;
        const read = world.thundering ? "thunder" : world.raining ? "rain" : "clear";
        return read === w.weather
          ? row("observed", `weather ${read}`)
          : row("failed", `expected weather ${w.weather}, read ${read}`);
      }
      case "health": {
        const h = o as Extract<AftermathObservable, { kind: "health" }>;
        return world.health < 20
          ? row("observed", `health ${world.health} after ${h.amount} damage`)
          : row("failed", `expected health below 20 after ${h.amount} damage, read ${world.health}`);
      }
      case "effect": {
        const f = o as Extract<AftermathObservable, { kind: "effect" }>;
        return world.effects.has(strip(f.effect))
          ? row("observed", `${f.effect} on the bot`)
          : row("failed", `expected ${f.effect} on the bot, carries ${[...world.effects].join(", ") || "none"}`);
      }
      case "no-effect": {
        const f = o as Extract<AftermathObservable, { kind: "no-effect" }>;
        const still =
          f.effect === null ? world.effects.size > 0 : world.effects.has(strip(f.effect));
        return still
          ? row("failed", `expected ${f.effect ?? "every effect"} cleared, carries ${[...world.effects].join(", ")}`)
          : row("observed", `${f.effect ?? "every effect"} cleared`);
      }
      default:
        return row("not_observable", `${e.verb}: a ${o.kind} only the server can read`);
    }
  });
}

/** The failures among `results`, as one message each. */
export function aftermathFailures(results: readonly AftermathResult[]): string[] {
  return results
    .filter((r) => r.status === "failed")
    .map((r) => `aftermath ${r.verb} (${r.path}): ${r.detail}`);
}
