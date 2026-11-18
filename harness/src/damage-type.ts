// The damage type of a hit, as the server names it.
//
// A 1.20+ `damage_event` packet carries `sourceTypeId`: an index into the
// `minecraft:damage_type` registry the server sent in its configuration phase,
// as one `registry_data` packet whose `entries` are in registry order. A hit
// with no responsible entity (`sourceCauseId` 0) is still named by its type —
// `minecraft:starve`, `minecraft:fall`, `minecraft:in_fire` — so the harness
// never has to guess what hurt the bot from the bodies standing near it.

/** The damage type vanilla deals a body whose hunger is empty. */
export const STARVE = "minecraft:starve";

/** The damage-type registry one connection was sent, in registry order. */
export class DamageTypes {
  private names: readonly string[] = [];

  /** Read one `registry_data` packet; every registry but damage types is ignored. */
  observe(packet: unknown): void {
    const p = packet as { id?: unknown; entries?: unknown } | undefined;
    if (p?.id !== "minecraft:damage_type" || !Array.isArray(p.entries)) return;
    this.names = p.entries.map((e: unknown) => String((e as { key?: unknown })?.key ?? ""));
  }

  /** How many types the registry holds: 0 before the server sent it. */
  get size(): number {
    return this.names.length;
  }

  /** The type at `index`, or `undefined` for an index the registry does not hold. */
  name(index: unknown): string | undefined {
    if (typeof index !== "number" || !Number.isInteger(index)) return undefined;
    const n = this.names[index];
    return n === undefined || n === "" ? undefined : n;
  }
}
