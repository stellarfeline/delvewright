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

/** One chunk packet the client received. */
interface ChunkEvent {
  readonly at: number;
  readonly kind: "biomes" | "map" | "unload";
  readonly chunks: readonly string[];
}

const key = (x: number, z: number): string => `${x},${z}`;

/** What one repaint's check concluded. */
export interface RepaintVerdict {
  readonly effect: string;
  /** Whether the bundle that fires it was seen to fire. */
  readonly performed: boolean;
  /** Chunks of the volume the client held when it fired — the denominator. */
  readonly held: number;
  /** Of those, how many a `chunk_biomes` named inside the window. */
  readonly told: number;
  /** Of those, how many a `map_chunk` resent inside the window. */
  readonly resent: number;
  /** Why it failed, when it did. */
  readonly failure?: string;
}

/**
 * The client-reach ledger: every `chunk_biomes`, `map_chunk` and `unload_chunk`
 * packet, and every completion marker, with the time it arrived.
 */
export class RepaintWatch {
  private readonly events: ChunkEvent[] = [];
  private readonly markers = new Map<string, number>();

  private readonly plan: RepaintPlan;

  constructor(plan: RepaintPlan) {
    this.plan = plan;
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
    }
  }

  /** Record a completion marker's token; first arrival wins. */
  marker(token: string, at: number): void {
    if (!this.markers.has(token)) this.markers.set(token, at);
  }

  /** Which chunks the client held at `at`: loaded and not since unloaded. */
  private heldAt(at: number): Set<string> {
    const held = new Set<string>();
    for (const e of this.events) {
      if (e.at > at) break;
      for (const c of e.chunks) {
        if (e.kind === "map") held.add(c);
        else if (e.kind === "unload") held.delete(c);
      }
    }
    return held;
  }

  /** Judge every repaint whose bundle was seen to fire. */
  verdicts(windowMs = REPAINT_WINDOW_MS): RepaintVerdict[] {
    return this.plan.repaints.map((r): RepaintVerdict => {
      const fired = r.after === null ? undefined : this.markers.get(r.after);
      if (fired === undefined) {
        return { effect: r.effect, performed: false, held: 0, told: 0, resent: 0 };
      }
      const held = this.heldAt(fired);
      const volume = r.chunks.map(([x, z]) => key(x, z)).filter((c) => held.has(c));
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
      const missing = volume.filter((c) => !told.has(c));
      const reloaded = volume.filter((c) => resent.has(c));
      const failures: string[] = [];
      if (volume.length === 0) {
        failures.push(
          `the client held none of the ${r.chunks.length} chunk(s) of the painted volume when ` +
            `its bundle fired, so nothing was measured — a zero binding is not a pass`,
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
        held: volume.length,
        told: volume.length - missing.length,
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
  const held = performed.reduce((n, v) => n + v.held, 0);
  const told = performed.reduce((n, v) => n + v.told, 0);
  const resent = performed.reduce((n, v) => n + v.resent, 0);
  return (
    `atmosphere repaints: ${plan.repaints.length} in the build, ${observable} with a marker, ` +
    `${performed.length} performed; ${told} of ${held} held chunk(s) told by chunk_biomes, ` +
    `${resent} resent by map_chunk`
  );
}
