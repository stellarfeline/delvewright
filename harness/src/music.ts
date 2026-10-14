// A scene plays what it declares (spec-0109 §5.3).
//
// The pinned client chooses music from the biome under the camera, and nothing
// outside a client observes its music manager: the fade, the loop seam and the
// now-playing toast are confirmed on a demo level, never here. What a bot CAN
// observe is the two server-side facts the music rests on:
//
//   * every `stop-sound` the forced route performs reaches the client as a
//     clientbound `stop_sound` packet carrying the ledger's category and sound,
//     within ten seconds of the completion marker of the bundle that fires it;
//   * at each listening station — the first route cell whose eye stands in a
//     declared biome — the biome the client holds under the eye is the one the
//     ledger names, read from the chunk data the server sent and the biome
//     registry it sent at login: a second instrument that shares no
//     configuration with the compiler's biome map.
//
// The compiler computes everything (`validation/music.json`): the stations from
// the forced route's proven legs, the marker token of each cut's bundle. The
// packet's category is decoded through the pinned protocol data's own
// `soundSource` table, never typed here. A cutscene's camera cells are not stood
// in by the bot (it does not watch cutscenes); their proof is PackTest's
// `music_hears_<n>`, and this reports each as `not_stood: cutscene`.
//
// Assertions and bookkeeping only — no game logic (CLAUDE.md).

import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";

import type { Vec3Tuple } from "./critical-path.ts";

/** Where the ledger sits, relative to the critical path's directory. */
export const MUSIC_LEDGER_SUBPATH = ["validation", "music.json"] as const;

/** How long after its bundle's marker a cut's packet may arrive. */
export const STOP_WINDOW_MS = 10_000;

/** A cell of the forced route the bot stands at. */
export interface MusicStation {
  readonly cell: Vec3Tuple;
  readonly step: number;
  /** The token (objective or trigger id) of that step: "before this step". */
  readonly before: string;
}

/** One biome a camera can stand in. */
export interface MusicBiome {
  readonly id: string;
  readonly kind: "atmosphere" | "ground" | "band";
  readonly station: MusicStation | undefined;
}

/** One `stop-sound`. */
export interface MusicStop {
  readonly effect: string;
  readonly category: string | undefined;
  /** Namespaced. */
  readonly sound: string | undefined;
  /** The marker token of the bundle that fires it, or undefined. */
  readonly after: string | undefined;
  /** Whether the forced route fires it. */
  readonly performed: boolean;
}

/** One cutscene, as the build judged it. */
export interface MusicCutscene {
  readonly effect: string;
  readonly verdict: string;
}

/** The parsed ledger. */
export interface MusicPlan {
  readonly biomes: readonly MusicBiome[];
  readonly stops: readonly MusicStop[];
  readonly cutscenes: readonly MusicCutscene[];
}

/** Raised when the ledger is malformed — a hard failure, never a skip. */
export class MusicPlanParseError extends Error {
  override readonly name = "MusicPlanParseError";
}

function bad(detail: string): never {
  throw new MusicPlanParseError(`music.json ${detail}`);
}

function optString(v: unknown, at: string): string | undefined {
  if (v === null || v === undefined) return undefined;
  if (typeof v !== "string") bad(`${at} is not a string or null`);
  return v;
}

function station(v: unknown, at: string): MusicStation | undefined {
  if (v === null) return undefined;
  if (typeof v !== "object" || v === undefined) bad(`${at} is not an object or null`);
  const s = v as Record<string, unknown>;
  const cell = s["cell"];
  if (!Array.isArray(cell) || cell.length !== 3 || !cell.every((n) => Number.isInteger(n))) {
    bad(`${at}/cell is not a block cell`);
  }
  if (typeof s["step"] !== "number") bad(`${at}/step is not a number`);
  if (typeof s["before"] !== "string") bad(`${at}/before is not a string`);
  return {
    cell: [cell[0] as number, cell[1] as number, cell[2] as number],
    step: s["step"] as number,
    before: s["before"] as string,
  };
}

function list(root: Record<string, unknown>, key: string): unknown[] {
  const v = root[key];
  if (!Array.isArray(v)) bad(`/${key} is not an array`);
  return v;
}

function row(entry: unknown, at: string): Record<string, unknown> {
  if (typeof entry !== "object" || entry === null || Array.isArray(entry)) bad(`${at} is not an object`);
  return entry as Record<string, unknown>;
}

/** Parse the ledger strictly. */
export function parseMusicPlan(raw: unknown): MusicPlan {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) bad("is not an object");
  const root = raw as Record<string, unknown>;
  const biomes = list(root, "biomes").map((entry, i): MusicBiome => {
    const e = row(entry, `/biomes/${i}`);
    if (typeof e["id"] !== "string") bad(`/biomes/${i}/id is not a string`);
    const kind = e["kind"];
    if (kind !== "atmosphere" && kind !== "ground" && kind !== "band") {
      bad(`/biomes/${i}/kind is not atmosphere, ground or band`);
    }
    return { id: e["id"] as string, kind, station: station(e["station"], `/biomes/${i}/station`) };
  });
  if (!biomes.some((b) => b.kind === "ground")) bad("names no ground biome");
  const stops = list(root, "stop_sounds").map((entry, i): MusicStop => {
    const e = row(entry, `/stop_sounds/${i}`);
    if (typeof e["effect"] !== "string") bad(`/stop_sounds/${i}/effect is not a string`);
    if (typeof e["performed"] !== "boolean") bad(`/stop_sounds/${i}/performed is not a boolean`);
    return {
      effect: e["effect"] as string,
      category: optString(e["category"], `/stop_sounds/${i}/category`),
      sound: optString(e["sound"], `/stop_sounds/${i}/sound`),
      after: optString(e["after"], `/stop_sounds/${i}/after`),
      performed: e["performed"] as boolean,
    };
  });
  const cutscenes = list(root, "cutscenes").map((entry, i): MusicCutscene => {
    const e = row(entry, `/cutscenes/${i}`);
    if (typeof e["effect"] !== "string") bad(`/cutscenes/${i}/effect is not a string`);
    if (typeof e["verdict"] !== "string") bad(`/cutscenes/${i}/verdict is not a string`);
    return { effect: e["effect"] as string, verdict: e["verdict"] as string };
  });
  return { biomes, stops, cutscenes };
}

/** Load the ledger beside a critical path; absent means a build that writes none. */
export async function loadMusicPlanForCriticalPath(
  criticalPathFile: string,
): Promise<MusicPlan | undefined> {
  const file = path.join(path.dirname(criticalPathFile), ...MUSIC_LEDGER_SUBPATH);
  let text: string;
  try {
    text = await readFile(file, "utf8");
  } catch {
    return undefined;
  }
  return parseMusicPlan(JSON.parse(text) as unknown);
}

/**
 * The pinned protocol's `soundSource` table — the category literal for each
 * index a `stop_sound` packet carries — read from minecraft-data for the
 * pinned game version, never typed here.
 */
export function pinnedSoundSources(version = "1.21.11"): ReadonlyMap<number, string> {
  const require = createRequire(import.meta.url);
  const mcData = require("minecraft-data") as (v: string) => { protocol: Record<string, unknown> };
  const types = (mcData(version).protocol["types"] ?? {}) as Record<string, unknown>;
  const source = types["soundSource"];
  const mappings =
    Array.isArray(source) && typeof source[1] === "object" && source[1] !== null
      ? ((source[1] as Record<string, unknown>)["mappings"] as Record<string, string> | undefined)
      : undefined;
  if (!mappings) {
    throw new MusicPlanParseError(`the pinned protocol data (${version}) carries no soundSource table`);
  }
  return new Map(Object.entries(mappings).map(([k, v]) => [Number(k), v]));
}

function namespaced(s: string): string {
  return s.includes(":") ? s : `minecraft:${s}`;
}

/** One `stop_sound` packet the client received. */
interface StopEvent {
  readonly at: number;
  readonly category: string | undefined;
  readonly sound: string | undefined;
}

/** What one cut's check concluded. */
export interface StopVerdict {
  readonly effect: string;
  /** Whether its bundle was seen to fire. */
  readonly performed: boolean;
  readonly failure?: string;
}

/** What one listening station read. */
export interface MusicStationRecord {
  readonly biome: string;
  readonly cell: Vec3Tuple;
  /** The biome the client holds under the eye, or undefined when unread. */
  readonly read: string | undefined;
  /** `not_read: registry` when the client could not name the biome. */
  readonly notRead?: string;
  readonly failure: string | null;
}

/**
 * The music ledger's runtime half: every `stop_sound` packet, every completion
 * marker, and the biome registry the server sent at login.
 */
export class MusicWatch {
  private readonly stops: StopEvent[] = [];
  private readonly markers = new Map<string, number>();
  /** Index → name of the `minecraft:worldgen/biome` registry the client received. */
  private biomeRegistry: string[] | undefined;
  readonly stations: MusicStationRecord[] = [];

  readonly plan: MusicPlan;
  private readonly sources: ReadonlyMap<number, string>;

  constructor(plan: MusicPlan, sources: ReadonlyMap<number, string>) {
    this.plan = plan;
    this.sources = sources;
  }

  /** Feed one raw packet. */
  packet(name: string, data: unknown, at: number): void {
    if (typeof data !== "object" || data === null) return;
    const d = data as Record<string, unknown>;
    if (name === "stop_sound") {
      const source = d["source"];
      const sound = d["sound"];
      this.stops.push({
        at,
        category: typeof source === "number" ? this.sources.get(source) : undefined,
        sound: typeof sound === "string" ? namespaced(sound) : undefined,
      });
    } else if (name === "registry_data") {
      const id = d["id"] ?? d["registryId"];
      const entries = d["entries"];
      if (id === "minecraft:worldgen/biome" && Array.isArray(entries)) {
        this.biomeRegistry = entries.map((e) => {
          const k = (e as Record<string, unknown>)["key"] ?? (e as Record<string, unknown>)["id"];
          return typeof k === "string" ? namespaced(k) : "";
        });
      }
    }
  }

  /** Record a completion marker; first arrival wins. */
  marker(token: string, at: number): void {
    if (!this.markers.has(token)) this.markers.set(token, at);
  }

  /** The name the login registry gives biome index `id`, when it gave one. */
  biomeName(id: number): string | undefined {
    const name = this.biomeRegistry?.[id];
    return name === undefined || name === "" ? undefined : name;
  }

  /** Judge one listening station's reading and record it. */
  recordStation(biome: MusicBiome, read: string | undefined): MusicStationRecord {
    const cell = biome.station?.cell ?? [0, 0, 0];
    const r: MusicStationRecord =
      read === undefined
        ? {
            biome: biome.id,
            cell,
            read,
            notRead: "registry",
            failure: null,
          }
        : {
            biome: biome.id,
            cell,
            read,
            failure:
              read === biome.id
                ? null
                : `music station ${JSON.stringify(cell)}: the client holds ${read} under the eye, and ` +
                  `the ledger says ${biome.id} — the biome map the compiler proved is not the one ` +
                  `the server sent`,
          };
    this.stations.push(r);
    return r;
  }

  /** Judge every cut whose bundle was seen to fire. */
  stopVerdicts(windowMs = STOP_WINDOW_MS): StopVerdict[] {
    return this.plan.stops.map((s): StopVerdict => {
      const fired = s.after === undefined ? undefined : this.markers.get(s.after);
      if (!s.performed || fired === undefined) return { effect: s.effect, performed: false };
      const hit = this.stops.some(
        (e) =>
          e.at >= fired - 1_000 &&
          e.at <= fired + windowMs &&
          e.category === s.category &&
          e.sound === s.sound,
      );
      return hit
        ? { effect: s.effect, performed: true }
        : {
            effect: s.effect,
            performed: true,
            failure:
              `stop-sound at ${s.effect}: no stop_sound packet with category ` +
              `${s.category ?? "(any)"} and sound ${s.sound ?? "(any)"} arrived within ${windowMs} ms of ` +
              `the marker ${s.after}`,
          };
    });
  }
}

/** The stations due before the step whose token is `token`, not yet read. */
export function musicStationsBefore(
  plan: MusicPlan,
  token: string,
  done: ReadonlySet<string>,
): MusicBiome[] {
  return plan.biomes.filter(
    (b) => b.station !== undefined && b.station.before === token && !done.has(b.id),
  );
}

/** The binding line, with its denominators. */
export function musicBindingLine(
  plan: MusicPlan,
  verdicts: readonly StopVerdict[],
  stations: readonly MusicStationRecord[],
): string {
  const withStation = plan.biomes.filter((b) => b.station !== undefined).length;
  const read = stations.filter((s) => s.read !== undefined);
  const agreed = read.filter((s) => s.failure === null).length;
  const observable = plan.stops.filter((s) => s.performed && s.after !== undefined).length;
  const performed = verdicts.filter((v) => v.performed);
  const heard = performed.filter((v) => v.failure === undefined).length;
  return (
    `music binding: ${stations.length} of ${withStation} listening station(s) stood at ` +
    `(${plan.biomes.length} biome(s)), ${agreed} of ${read.length} read agree with the ledger, ` +
    `${stations.length - read.length} not_read: registry; ${heard} of ${performed.length} ` +
    `performed stop-sound(s) heard as a stop_sound packet (${observable} observable of ` +
    `${plan.stops.length}); ${plan.cutscenes.length} cutscene(s) not_stood: cutscene`
  );
}
