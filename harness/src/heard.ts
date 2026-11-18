// What the bot is shown, written into the run's log: the action-bar line and
// where the engine's compass points. Observation only — nothing here asserts or
// acts — so a run's log shows a race's credits ("Checkpoint 3/9") and the
// compass turning to the next checkpoint as they happen.

/** A network NBT tag as node-minecraft-protocol decodes it (`anonymousNbt`). */
interface Tag {
  readonly type: string;
  readonly value: unknown;
}

function isTag(v: unknown): v is Tag {
  return typeof v === "object" && v !== null && "type" in v && "value" in v;
}

/** A network NBT tag as the plain value it stands for. */
export function nbtPlain(v: unknown): unknown {
  if (!isTag(v)) return v;
  switch (v.type) {
    case "compound": {
      const out: Record<string, unknown> = {};
      for (const [k, x] of Object.entries(v.value as Record<string, unknown>)) out[k] = nbtPlain(x);
      return out;
    }
    case "list": {
      const inner = v.value as { type: string; value: unknown[] };
      return (inner.value ?? []).map((x) => nbtPlain({ type: inner.type, value: x }));
    }
    default:
      return v.value;
  }
}

/** The text a server-resolved text component reads as, its fallback for a translation. */
export function componentText(c: unknown): string {
  if (typeof c === "string") return c;
  if (typeof c === "number") return String(c);
  if (Array.isArray(c)) return c.map(componentText).join("");
  if (typeof c !== "object" || c === null) return "";
  const o = c as Record<string, unknown>;
  const own =
    typeof o["text"] === "string"
      ? (o["text"] as string)
      : typeof o["fallback"] === "string"
        ? (o["fallback"] as string)
        : typeof o["translate"] === "string"
          ? (o["translate"] as string)
          : "";
  const extra = Array.isArray(o["extra"]) ? (o["extra"] as unknown[]).map(componentText).join("") : "";
  return own + extra;
}

/** An item's lodestone target, `[x, y, z]`, from its 1.20.5+ component list; `undefined` when it has none. */
export function lodestoneTarget(components: unknown): readonly [number, number, number] | undefined {
  if (!Array.isArray(components)) return undefined;
  const c = components.find((x) => (x as { type?: unknown })?.type === "lodestone_tracker") as
    | { data?: { globalPosition?: { location?: { x: number; y: number; z: number } } } }
    | undefined;
  const at = c?.data?.globalPosition?.location;
  return at ? [at.x, at.y, at.z] : undefined;
}
