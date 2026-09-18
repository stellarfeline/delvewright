// Melee: fighting the way a player with the class kit fights.
//
// The bot used to fight like nobody plays: it swung every 400 ms whatever the
// weapon, stood in the enemy's reach with the class's shield still in its bag,
// and carried its healing draughts through every fight untouched. Against the
// vesperhold Porter (a 90-health vindicator with an axe) that lost every
// unassisted attempt and half the assisted ones, a fight the owner beat with the
// same kit. Every rule below is a player's input — which hand is up, when the
// swing is released, when the bottle is drunk — decided from what the client is
// told. None of it is a server-side act.
//
// The rules live here, pure and clock-free, so they are unit-testable; the
// executor supplies the numbers and performs the inputs.

/** The key mineflayer files the player's `minecraft:attack_speed` under.
 *
 * mineflayer names an `update_attributes` entry through minecraft-data's
 * protocol mapper, and that mapper is WRONG for 1.21.11 from protocol id 20
 * onwards (it lacks `mining_efficiency` and `movement_efficiency`, so
 * `movement_speed` arrives filed as `generic.scale`). `attack_speed` is id 4 in
 * the pinned server's own registry report (`java -DbundlerMainClass=
 * net.minecraft.data.Main -jar minecraft_server.1.21.11.jar --reports`,
 * `minecraft:attribute`), below the break — `test/melee.test.ts` pins the
 * mapper's id 4 to this key, so a data update that moves it fails a test
 * instead of silently timing every swing off the wrong number. */
export const ATTACK_SPEED_KEY = "generic.attack_speed";

/** One attribute as the server sent it: the base value and its modifiers. */
export interface AttributeReading {
  readonly value: number;
  readonly modifiers: ReadonlyArray<{ readonly amount: number; readonly operation: number }>;
}

/**
 * An attribute's effective value, the way vanilla's `AttributeInstance`
 * computes it: every `add_value` (op 0) onto the base, then the base-scaled sum
 * of every `add_multiplied_base` (op 1), then each `add_multiplied_total` (op 2)
 * as a factor. The server sends the base and the modifiers, never the total.
 */
export function attributeValue(a: AttributeReading): number {
  let base = a.value;
  for (const m of a.modifiers) if (m.operation === 0) base += m.amount;
  let total = base;
  for (const m of a.modifiers) if (m.operation === 1) total += base * m.amount;
  for (const m of a.modifiers) if (m.operation === 2) total *= 1 + m.amount;
  return total;
}

/**
 * The player's attack speed (swings per second at full charge) from the
 * attributes the server last sent, or `undefined` when it sent none the client
 * can read — which the caller must SAY, never paper over with a number nobody
 * measured.
 */
export function attackSpeedFrom(
  attributes: Readonly<Record<string, AttributeReading>> | undefined,
): number | undefined {
  const a = attributes?.[ATTACK_SPEED_KEY];
  if (!a) return undefined;
  const v = attributeValue(a);
  return Number.isFinite(v) && v > 0 ? v : undefined;
}

/** Milliseconds per server tick. */
const TICK_MS = 50;

/**
 * How long after a swing the next one lands at FULL charge, in ms.
 *
 * Vanilla's attack strength is `clamp((ticksSinceSwing + 0.5) / (20 / attackSpeed))`,
 * and the damage a swing deals is scaled by `0.2 + 0.8 * strength²`; a swing
 * made early also restarts the clock. So a swing at 400 ms with an iron sword
 * (attack speed 1.6, a 12.5-tick period) is a 57% hit — and, landing inside
 * the target's 10-tick hurt immunity, every second one of them deals nothing.
 * Full charge is reached at `period - 0.5` ticks; this rounds that up to whole
 * ticks and adds one tick for the packet to cross.
 */
export function fullChargeMs(attackSpeed: number): number {
  const periodTicks = 20 / attackSpeed;
  return (Math.ceil(periodTicks - 0.5) + 1) * TICK_MS;
}

/**
 * The healing each drinkable potion restores, keyed by its protocol id in the
 * pinned 1.21.11 `minecraft:potion` registry (the server jar's own registry
 * report: `minecraft:healing` 24, `minecraft:strong_healing` 25).
 * `instant_health` heals `4 << amplifier`: 4 for Healing, 8 for Healing II.
 *
 * Only named potions are read. A kit may also declare `contents.effects` (a
 * custom `instant_health`); that bottle is not recognised here and the bot
 * does not drink it — the omission is stated, and a campaign that relies on it
 * would see the bot carry it unopened.
 */
export const HEALING_POTION_HEAL: ReadonlyMap<number, number> = new Map([
  [24, 4],
  [25, 8],
]);

/** The item names that are DRUNK. A splash or lingering bottle is thrown, and a
 * thrown heal is a different act this bot does not perform. */
const DRINKABLE_POTIONS = new Set(["potion"]);

/** A component as prismarine-item exposes it on 1.21.x. */
export interface ItemComponent {
  readonly type: string;
  readonly data?: unknown;
}

/**
 * How much a carried item heals when drunk, or `undefined` when it is not a
 * drinkable healing potion. Read off the item's own `potion_contents`
 * component — the one fact that tells a healing draught from poison in the
 * same bottle.
 */
export function drinkHeal(
  name: string,
  components: readonly ItemComponent[] | undefined,
): number | undefined {
  if (!DRINKABLE_POTIONS.has(name)) return undefined;
  const pc = components?.find((c) => c.type === "potion_contents");
  const id = (pc?.data as { potionId?: unknown } | undefined)?.potionId;
  if (typeof id !== "number") return undefined;
  return HEALING_POTION_HEAL.get(id);
}

/** Whether to drink now, and which heal to drink. */
export type DrinkDecision =
  | { readonly kind: "drink"; readonly heal: number }
  | { readonly kind: "healthy" }
  | { readonly kind: "none-carried" };

/**
 * Drink a healing draught when a bottle's whole heal fits under the missing
 * health — a player does not waste half a flask on a scratch, and does not
 * carry four of them into the grave. Of the bottles that fit, the strongest.
 * Unlike food, a hostile in reach does not forbid it: a potion is the thing a
 * player drinks MID-fight, and the heal lands whole when the drink ends.
 */
export function drinkDecision(opts: {
  readonly health: number;
  readonly maxHealth: number;
  /** The heal of every drinkable healing draught carried, one entry per kind. */
  readonly heals: readonly number[];
}): DrinkDecision {
  if (opts.heals.length === 0) {
    return opts.health < opts.maxHealth ? { kind: "none-carried" } : { kind: "healthy" };
  }
  const missing = opts.maxHealth - opts.health;
  const fitting = opts.heals.filter((h) => h <= missing);
  if (fitting.length === 0) return { kind: "healthy" };
  return { kind: "drink", heal: Math.max(...fitting) };
}

/**
 * How long before its swing is charged the bot jumps, so the swing is released
 * on the way DOWN: vanilla crits a fully charged, non-sprinting swing made while
 * falling (1.5× damage). A jump peaks after ~6 ticks and lands after ~12, so a
 * jump 350 ms before the charge completes puts the charged swing just past the
 * apex.
 */
export const CRIT_JUMP_LEAD_MS = 350;

/** Longest the bot waits in the air for the fall to start before swinging
 * anyway — a jump under a low ceiling never rises, and the swing must not
 * wait on a fall that is not coming. */
export const CRIT_FALL_WAIT_MS = 400;

/** One strike's worth of what a player's hands do, in order. */
export interface StrikePlan {
  /** Raise the shield while the swing charges. */
  readonly guard: boolean;
  /** Jump so the charged swing lands falling. */
  readonly jump: boolean;
}

/**
 * What the bot does with the charge time before its next swing.
 *
 * The shield goes up whenever the off hand holds one and there is time for it
 * to count: vanilla's shield blocks only after `block_delay_seconds` (0.25 s)
 * raised, so a raise with less than that before the swing is a gesture, and it
 * is skipped. The jump is taken only from solid footing with headroom.
 */
export function planStrike(opts: {
  readonly msUntilCharged: number;
  readonly shieldInOffhand: boolean;
  readonly onGround: boolean;
  readonly headroom: boolean;
}): StrikePlan {
  return {
    guard: opts.shieldInOffhand && opts.msUntilCharged >= SHIELD_BLOCK_DELAY_MS,
    jump: opts.onGround && opts.headroom,
  };
}

/** Vanilla shield `blocks_attacks.block_delay_seconds`, in ms. */
export const SHIELD_BLOCK_DELAY_MS = 250;

/**
 * What a fight's hands did, counted from what the server reported back — so a
 * log reader can see whether the bot fenced or flailed. Every count except
 * `swings` and `draughts` is the server's own broadcast: a critical hit on the
 * target (animation 4), a blow taken on the shield (entity event 29), the
 * shield knocked out of use by an axe (entity event 30).
 */
export interface MeleeTally {
  swings: number;
  crits: number;
  blocked: number;
  shieldDisabled: number;
  draughts: number;
}

export function emptyTally(): MeleeTally {
  return { swings: 0, crits: 0, blocked: 0, shieldDisabled: 0, draughts: 0 };
}

/** One line for the log: what the hands did this fight. */
export function describeTally(t: MeleeTally): string {
  return (
    `${t.swings} charged swing(s), ${t.crits} critical, ${t.blocked} blow(s) taken on the ` +
    `shield, shield disabled ${t.shieldDisabled}×, ${t.draughts} draught(s) drunk`
  );
}

/** Vanilla entity events the tally counts off the bot's own entity. */
export const ENTITY_EVENT_SHIELD_BLOCK = 29;
export const ENTITY_EVENT_SHIELD_DISABLED = 30;
