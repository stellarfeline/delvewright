// A repaint reaches the client (spec-0080 §5.2).
//
// A `set-atmosphere` is `/fillbiome` over a volume while the party stands in it.
// PackTest proves the server holds the new biome; this proves the CLIENT was
// told, the way the eldritch-visuals spike measured it on the pinned server: a
// `chunk_biomes` packet naming every chunk of the painted volume the client
// holds, and no `map_chunk` resending any of them. A repaint that only reached
// the client by a chunk reload (or not at all) is the defect this catches.
//
// The compiler exports what to expect beside the critical path
// (`validation/atmosphere-repaints.json`): per repaint, the chunks of its
// painted 4-cell box and the completion-marker token of the bundle that fires
// it (`obj/<id>` or `trigger/<id>`), or null when nothing the bot sees marks the
// moment it fires. The harness never derives a chunk or a biome itself.
//
// Assertions and bookkeeping only — no game logic (CLAUDE.md).

import { readFile } from "node:fs/promises";
import path from "node:path";

/** Where the export sits, relative to the critical path's directory — the
 * build tree's `validation/`, beside every other contract the bot reads. */
export const REPAINT_PLAN_SUBPATH = ["validation", "atmosphere-repaints.json"] as const;

/** How long after its bundle's marker a repaint's packets may arrive. */
export const REPAINT_WINDOW_MS = 10_000;

/** One repaint the build performs. */
export interface PlannedRepaint {
  /** JSON path of the effect in its stage document, for the message. */
  readonly effect: string;
  /** The biome it paints. */
  readonly biome: string;
  /** The marker token of the bundle that fires it, or null. */
  readonly after: string | null;
  /** Chunk coordinates `[cx, cz]` of its painted 4-cell box. */
  readonly chunks: readonly (readonly [number, number])[];
}

/** The parsed export. */
export interface RepaintPlan {
  readonly repaints: readonly PlannedRepaint[];
}

/** Raised when the export is malformed — a hard failure, never a skip. */
export class RepaintPlanParseError extends Error {
  override readonly name = "RepaintPlanParseError";
}

function bad(detail: string): never {
  throw new RepaintPlanParseError(`atmosphere-repaints.json ${detail}`);
}

/** Parse the export strictly. */
export function parseRepaintPlan(raw: unknown): RepaintPlan {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) bad("is not an object");
  const root = raw as Record<string, unknown>;
  const list = root["repaints"];
  if (!Array.isArray(list)) bad("/repaints is not an array");
  const repaints = list.map((entry, i): PlannedRepaint => {
    if (typeof entry !== "object" || entry === null) bad(`/repaints/${i} is not an object`);
    const e = entry as Record<string, unknown>;
    const effect = e["effect"];
    const biome = e["biome"];
    const after = e["after"];
    const chunks = e["chunks"];
    if (typeof effect !== "string") bad(`/repaints/${i}/effect is not a string`);
    if (typeof biome !== "string") bad(`/repaints/${i}/biome is not a string`);
    if (after !== null && typeof after !== "string") bad(`/repaints/${i}/after is not a string or null`);
    if (!Array.isArray(chunks) || chunks.length === 0) bad(`/repaints/${i}/chunks is not a non-empty array`);
    const parsed = chunks.map((c, j): [number, number] => {
      if (!Array.isArray(c) || c.length !== 2 || !c.every((n) => Number.isInteger(n))) {
        bad(`/repaints/${i}/chunks/${j} is not [cx, cz]`);
      }
      return [c[0] as number, c[1] as number];
    });
    return { effect, biome, after, chunks: parsed };
  });
  return { repaints };
}

/** Load the export beside a critical path; absent means the build repaints nothing. */
export async function loadRepaintPlanForCriticalPath(
  criticalPathFile: string,
): Promise<RepaintPlan | undefined> {
  const file = path.join(path.dirname(criticalPathFile), ...REPAINT_PLAN_SUBPATH);
  let text: string;
  try {
    text = await readFile(file, "utf8");
  } catch {
    return undefined;
  }
  return parseRepaintPlan(JSON.parse(text) as unknown);
}

/** One packet the ledger keeps: a chunk packet, or the client dropping every chunk. */
interface ChunkEvent {
  readonly at: number;
  /** `reset`: a `login` or `respawn` — the client discards every chunk it held,
   * and the server tracks the new body from nothing, with no `unload_chunk`. */
  readonly kind: "biomes" | "map" | "unload" | "reset";
  readonly chunks: readonly string[];
}

/** One completion marker: when it arrived and where the bot stood. */
interface MarkerSighting {
  readonly at: number;
  /** The bot's position when the marker arrived; `undefined` when unknown. */
  readonly pos: readonly [number, number, number] | undefined;
}

const key = (x: number, z: number): string => `${x},${z}`;

/** Blocks per chunk on the horizontal axes. */
const CHUNK_BLOCKS = 16;

/**
 * Whether chunk `(cx, cz)` is inside the disc the server is sure to send a body
 * standing in chunk `(bx, bz)` at a served view distance of `served` chunks: the
 * chunk-offset distance strictly under `served`. The pinned server sends one more
 * ring than its `view-distance` (spec-0091's measurement), so this disc is inside
 * what it sends by at least that ring; a chunk outside it is one the client holds
 * or not by where in its chunk the body stands and how far streaming has got.
 */
export function insideServedDisc(
  bx: number,
  bz: number,
  cx: number,
  cz: number,
  served: number,
): boolean {
  const dx = cx - bx;
  const dz = cz - bz;
  return dx * dx + dz * dz < served * served;
}

/** What one repaint's check concluded. */
export interface RepaintVerdict {
  readonly effect: string;
  /** Whether the bundle that fires it was seen to fire. */
  readonly performed: boolean;
  /** Chunks of the painted volume — the denominator of every count below. */
  readonly chunks: number;
  /** Of them, inside the served disc around the bot when it fired — what any
   * client standing there is sure to be sent. */
  readonly owed: number;
  /** Of them, held by the client when it fired, wherever they lie. */
  readonly held: number;
  /** Held AND owed: the binding. A chunk held only at the server's margin is
   * judged when held but binds nothing, because whether it is held is luck. */
  readonly bound: number;
  /** Of the held chunks, how many a `chunk_biomes` named inside the window. */
  readonly told: number;
  /** Of the held chunks, how many a `map_chunk` resent inside the window. */
  readonly resent: number;
  /** Why it failed, when it did. */
  readonly failure?: string;
}

/**
 * The client-reach ledger: every `chunk_biomes`, `map_chunk`, `unload_chunk`,
 * `login` and `respawn` packet, the view distance the server serves, and every
 * completion marker with the time it arrived and where the bot stood.
 */
export class RepaintWatch {
  private readonly events: ChunkEvent[] = [];
  private readonly markers = new Map<string, MarkerSighting>();
  /** The server's `view-distance`, from `login` and `update_view_distance`. */
  private serverViewDistance: number | undefined;

  private readonly plan: RepaintPlan;
  /** The view distance the client asks for; the server serves the lesser. */
  private readonly clientViewDistance: number;

  constructor(plan: RepaintPlan, clientViewDistance: number) {
    this.plan = plan;
    this.clientViewDistance = clientViewDistance;
  }

  /** Feed one raw packet (`bot._client.on("packet", (data, meta) => …)`). */
  packet(name: string, data: unknown, at: number): void {
    if (typeof data !== "object" || data === null) return;
    const d = data as Record<string, unknown>;
    if (name === "chunk_biomes") {
      const list = Array.isArray(d["biomes"]) ? (d["biomes"] as unknown[]) : [];
      const chunks = list.flatMap((b) => {
        const p = (b as { position?: { x?: unknown; z?: unknown } }).position;
        return typeof p?.x === "number" && typeof p?.z === "number" ? [key(p.x, p.z)] : [];
      });
      this.events.push({ at, kind: "biomes", chunks });
    } else if (name === "map_chunk" || name === "unload_chunk") {
      const x = d["x"] ?? d["chunkX"];
      const z = d["z"] ?? d["chunkZ"];
      if (typeof x === "number" && typeof z === "number") {
        this.events.push({ at, kind: name === "map_chunk" ? "map" : "unload", chunks: [key(x, z)] });
      }
    } else if (name === "login" || name === "respawn") {
      this.events.push({ at, kind: "reset", chunks: [] });
      if (name === "login" && typeof d["viewDistance"] === "number") {
        this.serverViewDistance = d["viewDistance"];
      }
    } else if (name === "update_view_distance" && typeof d["viewDistance"] === "number") {
      this.serverViewDistance = d["viewDistance"];
    }
  }

  /** The view distance the client is served: the lesser of what it asked for and
   * what the server serves; `undefined` until the server has said. */
  served(): number | undefined {
    if (this.serverViewDistance === undefined) return undefined;
    return Math.min(this.clientViewDistance, this.serverViewDistance);
  }

  /** Record a completion marker's token and where the bot stood; first arrival wins. */
  marker(token: string, at: number, pos?: readonly [number, number, number]): void {
    if (!this.markers.has(token)) this.markers.set(token, { at, pos });
  }

  /** Which chunks the client held at `at`: loaded, not since unloaded, and not
   * dropped by a `login`/`respawn` since. */
  private heldAt(at: number): Set<string> {
    const held = new Set<string>();
    for (const e of this.events) {
      if (e.at > at) break;
      if (e.kind === "reset") held.clear();
      for (const c of e.chunks) {
        if (e.kind === "map") held.add(c);
        else if (e.kind === "unload") held.delete(c);
      }
    }
    return held;
  }

  /** Judge every repaint whose bundle was seen to fire. */
  verdicts(windowMs = REPAINT_WINDOW_MS): RepaintVerdict[] {
    const served = this.served();
    return this.plan.repaints.map((r): RepaintVerdict => {
      const chunks = r.chunks.length;
      const sighting = r.after === null ? undefined : this.markers.get(r.after);
      if (sighting === undefined) {
        return { effect: r.effect, performed: false, chunks, owed: 0, held: 0, bound: 0, told: 0, resent: 0 };
      }
      const fired = sighting.at;
      const heldNow = this.heldAt(fired);
      const volume = r.chunks.map(([x, z]) => key(x, z));
      const held = volume.filter((c) => heldNow.has(c));
      const pos = sighting.pos;
      const owedSet = new Set<string>();
      let nearest = Number.POSITIVE_INFINITY;
      if (pos !== undefined && served !== undefined) {
        const bx = Math.floor(pos[0] / CHUNK_BLOCKS);
        const bz = Math.floor(pos[2] / CHUNK_BLOCKS);
        for (const [x, z] of r.chunks) {
          nearest = Math.min(nearest, Math.hypot(x - bx, z - bz));
          if (insideServedDisc(bx, bz, x, z, served)) owedSet.add(key(x, z));
        }
      }
      const bound = held.filter((c) => owedSet.has(c));
      // The marker and the paint leave the server in one tick, marker first,
      // so a biome packet a moment before it is still this repaint's; a chunk
      // the client LOADED before the marker is what it held, never a resend.
      const told = new Set(
        this.events
          .filter((e) => e.kind === "biomes" && e.at >= fired - 1_000 && e.at <= fired + windowMs)
          .flatMap((e) => e.chunks),
      );
      const resent = new Set(
        this.events
          .filter((e) => e.kind === "map" && e.at > fired && e.at <= fired + windowMs)
          .flatMap((e) => e.chunks),
      );
      const missing = held.filter((c) => !told.has(c));
      const reloaded = held.filter((c) => resent.has(c));
      const failures: string[] = [];
      if (pos === undefined || served === undefined) {
        failures.push(
          `${pos === undefined ? "where the bot stood when its bundle fired" : "the view distance the server serves"} ` +
            `is unknown, so which of the ${chunks} chunk(s) of the painted volume the client was ` +
            `owed cannot be said and nothing binds — a zero binding is not a pass`,
        );
      } else if (owedSet.size === 0) {
        failures.push(
          `every one of the ${chunks} chunk(s) of the painted volume lies beyond the ${served}-chunk ` +
            `view distance the client is served, from the chunk the bot stood in when its bundle ` +
            `fired (the nearest ${nearest.toFixed(1)} chunks away), so no client standing there is ` +
            `sure to be sent any of it and nothing binds — a zero binding is not a pass` +
            (held.length > 0
              ? ` (${held.length} were held at the server's margin, which is luck, not a binding)`
              : ""),
        );
      } else if (bound.length === 0) {
        failures.push(
          `the client held none of the ${owedSet.size} chunk(s) of the painted volume inside its ` +
            `${served}-chunk served view distance when its bundle fired, so nothing was measured — ` +
            `a zero binding is not a pass`,
        );
      }
      if (missing.length > 0) {
        failures.push(`no chunk_biomes named chunk(s) ${missing.join(" ")} within ${windowMs} ms`);
      }
      if (reloaded.length > 0) {
        failures.push(`map_chunk resent chunk(s) ${reloaded.join(" ")} — the client was reloaded, not told`);
      }
      return {
        effect: r.effect,
        performed: true,
        chunks,
        owed: owedSet.size,
        held: held.length,
        bound: bound.length,
        told: held.length - missing.length,
        resent: reloaded.length,
        ...(failures.length > 0
          ? { failure: `set-atmosphere at ${r.effect} (${r.biome}): ${failures.join("; ")}` }
          : {}),
      };
    });
  }
}

/** The binding line, with its denominators. */
export function repaintBindingLine(plan: RepaintPlan, verdicts: readonly RepaintVerdict[]): string {
  const observable = plan.repaints.filter((r) => r.after !== null).length;
  const performed = verdicts.filter((v) => v.performed);
  const sum = (f: (v: RepaintVerdict) => number): number => performed.reduce((n, v) => n + f(v), 0);
  return (
    `atmosphere repaints: ${plan.repaints.length} in the build, ${observable} with a marker, ` +
    `${performed.length} performed; ${sum((v) => v.bound)} bound chunk(s) (held inside the ` +
    `served view distance) of ${sum((v) => v.owed)} owed of ${sum((v) => v.chunks)} painted; ` +
    `${sum((v) => v.told)} of ${sum((v) => v.held)} held chunk(s) told by chunk_biomes, ` +
    `${sum((v) => v.resent)} resent by map_chunk`
  );
}
