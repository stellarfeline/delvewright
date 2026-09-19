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
  // The server's numbers are floats: an iron sword's -2.4 arrives as
  // -2.4000000953674316, which puts the period a hair over 12.5 ticks. Vanilla
  // compares the same floats, so a millionth of a tick is not a whole tick.
  return (Math.ceil(periodTicks - 0.5 - 1e-6) + 1) * TICK_MS;
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
  | { readonly kind: "pressed" }
  | { readonly kind: "none-carried" };

/**
 * Drink a healing draught when a bottle's whole heal fits under the missing
 * health — a player does not waste half a flask on a scratch, and does not
 * carry four of them into the grave. Of the bottles that fit, the strongest.
 *
 * Not while a melee attacker could reach the bot before the drink ends
 * ({@link DRINK_CLEAR_RANGE}): a drink holds the bottle for 32 ticks at a fifth
 * of walking speed, and the bot neither swings nor blocks while it lasts.
 * Measured on the pinned server (the vesperhold Porter, an assisted fight
 * opened at 13/20, N = 8): the six fights in which the bot only fenced took no
 * hit at all; the two in which it backed off and drank took every one of their
 * five hits while backing or drinking, the vindicator having closed from four
 * blocks to under one and a half inside a single drink. So `pressed` is not a
 * cue to retreat — the exchange simply goes on.
 *
 * Except at a third of max health or less ({@link DRINK_CRITICAL_FRACTION}),
 * and then only when the bot can live through one more blow from what is on it
 * (`health > nearestMeleeBlow`, the largest blow that attacker has landed this
 * run): the drink then nets its heal less one blow. When one more blow kills
 * the bot whether it drinks or not, it fights on — a drink it cannot finish
 * only gives away the swings (vesperhold: at 4.1/20 against a 13-damage
 * unassisted Porter the bot died mid-drink with no swing thrown, where the bot
 * that kept swinging had won that attempt three times in four).
 */
export function drinkDecision(opts: {
  readonly health: number;
  readonly maxHealth: number;
  /** The heal of every drinkable healing draught carried, one entry per kind. */
  readonly heals: readonly number[];
  /** Horizontal distance to the nearest melee attacker, `undefined` when none. */
  readonly nearestMeleeDistance: number | undefined;
  /** The largest blow the nearest melee attacker has landed on the bot, when known. */
  readonly nearestMeleeBlow?: number | undefined;
}): DrinkDecision {
  if (opts.heals.length === 0) {
    return opts.health < opts.maxHealth ? { kind: "none-carried" } : { kind: "healthy" };
  }
  const missing = opts.maxHealth - opts.health;
  const fitting = opts.heals.filter((h) => h <= missing);
  if (fitting.length === 0) return { kind: "healthy" };
  const heal = Math.max(...fitting);
  const pressed =
    opts.nearestMeleeDistance !== undefined && opts.nearestMeleeDistance < DRINK_CLEAR_RANGE;
  if (!pressed) return { kind: "drink", heal };
  const critical = opts.health <= opts.maxHealth * DRINK_CRITICAL_FRACTION;
  const survivesABlow = opts.nearestMeleeBlow === undefined || opts.health > opts.nearestMeleeBlow;
  return critical && survivesABlow ? { kind: "drink", heal } : { kind: "pressed" };
}

/**
 * How far (blocks, horizontal) the nearest melee attacker must be before a
 * draught is started: far enough that it cannot close to striking distance in
 * the 1.6 s the drink takes. Authored from the measurement above — a vindicator
 * crossed from four blocks to 1.3 within one drink — at eight blocks: its
 * crossing plus its reach, with the margin a faster walker needs.
 */
export const DRINK_CLEAR_RANGE = 8;

/** At or below this fraction of max health a draught is drunk even with a melee
 * attacker on the bot — see {@link drinkDecision}. */
export const DRINK_CRITICAL_FRACTION = 1 / 3;

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

/** A player's melee reach: `minecraft:entity_interaction_range`, eye to hitbox. */
export const PLAYER_REACH = 3.0;

/** A standing player's eye height. */
export const PLAYER_EYE_HEIGHT = 1.62;

/**
 * Horizontal distance (feet to feet) inside which a MELEE attacker is about to
 * swing, so the bot steps back while its own swing charges. A vindicator or a
 * zombie hits a body its attack box — its own box grown ~0.83 blocks sideways —
 * touches, about 1.4 blocks centre to centre; a player's sword reaches about 3.
 * The whole art of fighting one is standing in that gap.
 */
export const KITE_DISTANCE = 2.5;

/**
 * How near (blocks) a body must be for the exchange to be fought with the
 * hands — stepping in, stepping back, holding the shield — rather than walked
 * to by the pathfinder.
 */
export const MELEE_ENGAGE_RANGE = 6;

/** Longest one exchange waits for its body to come into reach before handing the
 * approach back to the pathfinder. */
export const ENGAGE_BUDGET_MS = 2_000;

/** Weapons whose holder fights at range: it backs away rather than closing, so
 * stepping back from it is running from the fight. */
const RANGED_WEAPONS = new Set(["bow", "crossbow"]);

/** Whether a mob holding `heldItem` (its main hand, as the client sees it) fights at range. */
export function holdsRangedWeapon(heldItem: string | undefined): boolean {
  return heldItem !== undefined && RANGED_WEAPONS.has(heldItem);
}

/** An axis-aligned box: `[minX, minY, minZ, maxX, maxY, maxZ]`. */
export type Box3 = readonly [number, number, number, number, number, number];

/** Distance from a point to the nearest point of a box (0 inside it). */
export function distanceToBox(p: readonly [number, number, number], b: Box3): number {
  const dx = Math.max(b[0] - p[0], 0, p[0] - b[3]);
  const dy = Math.max(b[1] - p[1], 0, p[1] - b[4]);
  const dz = Math.max(b[2] - p[2], 0, p[2] - b[5]);
  return Math.hypot(dx, dy, dz);
}

/**
 * Whether a body standing at `feet` can hit a mob at `mobFeet` of `width` ×
 * `height`: vanilla measures reach from the eye to the target's hitbox.
 */
export function inReach(
  feet: readonly [number, number, number],
  mobFeet: readonly [number, number, number],
  width: number,
  height: number,
): boolean {
  const eye: [number, number, number] = [feet[0], feet[1] + PLAYER_EYE_HEIGHT, feet[2]];
  const h = width / 2;
  const box: Box3 = [
    mobFeet[0] - h,
    mobFeet[1],
    mobFeet[2] - h,
    mobFeet[0] + h,
    mobFeet[1] + height,
    mobFeet[2] + h,
  ];
  return distanceToBox(eye, box) <= PLAYER_REACH;
}

/** What the feet do this tick of an exchange. */
export type Footwork = "back" | "forward" | "hold";

/**
 * The footwork of one tick of an exchange, the way a player fences:
 *
 *  * with no usable shield, while the swing charges, a MELEE attacker inside
 *    {@link KITE_DISTANCE} is backed away from — the swing it is about to make
 *    lands on air (with one, the blow is taken on the shield: see `guardUp`);
 *  * a body out of reach is stepped toward (a ranged one at once, since it
 *    will not come; a melee one once the swing is charged, since it is coming);
 *  * otherwise the feet hold and the shield takes what comes.
 *
 * A step is taken only onto ground the caller has proven safe; a refused step
 * is a hold.
 */
export function footwork(opts: {
  readonly ranged: boolean;
  readonly horizontalDistance: number;
  readonly charged: boolean;
  readonly inReach: boolean;
  readonly canStepBack: boolean;
  readonly canStepIn: boolean;
  /** A usable shield is held, not run from: the bot backs away only without one. */
  readonly shieldUsable?: boolean;
}): Footwork {
  if (
    !opts.shieldUsable &&
    !opts.charged &&
    !opts.ranged &&
    opts.horizontalDistance < KITE_DISTANCE
  ) {
    return opts.canStepBack ? "back" : "hold";
  }
  if (!opts.inReach && (opts.ranged || opts.charged)) {
    return opts.canStepIn ? "forward" : "hold";
  }
  return "hold";
}

/**
 * How long a raised shield takes to start blocking. Measured on the pinned
 * server (probe: a NoAI husk's `mob_attack` applied by `/damage … by`, the
 * bot's off-hand shield raised with mineflayer's `activateItem(true)` for a
 * known number of ticks, N = 5 per row): 3 ticks 0/5 and 0/5 blocked, 4 ticks
 * 2/5 and 1/5, 5 ticks and longer 5/5 — vanilla's `block_delay_seconds` 0.25,
 * the 4-tick row being client timing slop.
 */
export const SHIELD_WARMUP_MS = 250;

/**
 * Vanilla's melee cooldown between one mob's blows (`MeleeAttackGoal`, 20
 * ticks). After an attacker's blow the next is this far off.
 */
export const MOB_BLOW_INTERVAL_MS = 1_000;

/**
 * How long after an attacker's blow the bot may still drop its shield, swing,
 * and have the shield raised and warm again before that attacker's next blow:
 * the blow interval less the warm-up, less two ticks for the release, the swing
 * and the packets to cross.
 */
export const OPENING_MS = MOB_BLOW_INTERVAL_MS - SHIELD_WARMUP_MS - 2 * 50 - 150;

/** How near (blocks, horizontal) a melee attacker must be to count as able to
 * strike before a released swing and a re-raised shield are done. */
export const MOB_STRIKE_RANGE = 3;

/**
 * How long a charged swing waits for an opening before it is released anyway
 * — the answer to a crowd whose blows never all fall inside one window.
 */
export const OPENING_WAIT_MS = 1_200;

/** How long an attacker may stand in strike range without swinging before it
 * is taken as not attacking (it is then no reason to keep the shield up). */
export const IDLE_ATTACKER_MS = 1_500;

/**
 * Whether a mob's main hand holds a weapon whose blow disables a shield. In
 * 1.21.11 that is every axe (the item's `weapon` component,
 * `disable_blocking_for_seconds: 5`); measured on the pinned server, three of
 * three axe blows on a raised shield put it on a 100-tick cooldown. Against such
 * an attacker the shield buys one blow per five seconds, so the bot fences the
 * way it does without one — and a controlled repeat of the Porter fight with
 * the shield held against his axe lost 5 of 5 where kiting had won 8 of 8.
 */
export function disablesShields(heldItem: string | undefined): boolean {
  return heldItem !== undefined && heldItem.endsWith("_axe");
}

/** One melee attacker as the opening rule reads it. */
export interface StrikeThreat {
  /** Horizontal distance to the bot. */
  readonly distance: number;
  /** ms since the server last showed it swinging, `undefined` when never. */
  readonly swungAgoMs: number | undefined;
  /** ms it has stood within {@link MOB_STRIKE_RANGE} this exchange. */
  readonly inRangeForMs: number;
}

/**
 * Whether now is an OPENING: every melee attacker able to strike has just
 * struck (inside {@link OPENING_MS}) or has stood in range without striking for
 * {@link IDLE_ATTACKER_MS}. The way a player fights with a shield: take the blow
 * on it, answer in the gap before the next, raise again.
 */
export function isOpening(threats: readonly StrikeThreat[]): boolean {
  return threats
    .filter((t) => t.distance <= MOB_STRIKE_RANGE)
    .every(
      (t) =>
        (t.swungAgoMs !== undefined && t.swungAgoMs <= OPENING_MS) ||
        (t.inRangeForMs >= IDLE_ATTACKER_MS &&
          (t.swungAgoMs === undefined || t.swungAgoMs >= IDLE_ATTACKER_MS)),
    );
}

/**
 * Whether to release a charged swing now: with the shield usable, only in an
 * opening, or once {@link OPENING_WAIT_MS} of waiting has passed; without one
 * (none carried, or an axe has disabled it), at once.
 */
export function releaseSwing(opts: {
  readonly charged: boolean;
  readonly inReach: boolean;
  readonly shieldUsable: boolean;
  readonly opening: boolean;
  readonly chargedForMs: number;
}): boolean {
  if (!opts.charged || !opts.inReach) return false;
  if (!opts.shieldUsable) return true;
  return opts.opening || opts.chargedForMs >= OPENING_WAIT_MS;
}

/**
 * Whether to hold the shield up this tick: whenever it is usable and the feet
 * are still — including while the swing is charged and the bot waits for its
 * opening. A raised shield slows a walking player to a crawl, so a step lowers
 * it; the swing lowers it for the tick it is released.
 */
export function guardUp(opts: {
  readonly shieldUsable: boolean;
  readonly footwork: Footwork;
}): boolean {
  return opts.shieldUsable && opts.footwork === "hold";
}

/**
 * Whether to jump now so the charged swing lands falling (a critical hit): the
 * body is in reach, the feet are on the ground with headroom and not stepping,
 * and the charge completes within {@link CRIT_JUMP_LEAD_MS}.
 */
export function jumpForCrit(opts: {
  readonly msUntilCharged: number;
  readonly inReach: boolean;
  readonly onGround: boolean;
  readonly headroom: boolean;
}): boolean {
  return (
    opts.inReach && opts.onGround && opts.headroom && opts.msUntilCharged <= CRIT_JUMP_LEAD_MS
  );
}

/**
 * What a fight's hands did — so a log reader can see whether the bot fenced or
 * flailed. `swings` and `guards` (times the shield went up) are the bot's own
 * inputs. `landed` and `noDamage` are the server's verdict on each swing, read
 * off the sound vanilla plays at the attacker for it (`swingVerdict`). `crits` is the server's critical-hit animation
 * on the target;
 * `shieldDisabled` is the server putting the shield on cooldown (`set_cooldown`
 * for `minecraft:shield`, what an axe blow on a raised shield does);
 * `draughts` counts only drinks the server finished (entity event 9) and the
 * bag shows gone.
 *
 * A blow the shield TOOK is not counted, because nothing the client is sent
 * says so reliably: measured on the pinned server, a zombie's blow on a raised
 * shield arrived as no packet at all, and only the axe blow that disabled it
 * played `item.shield.block`.
 */
export interface MeleeTally {
  swings: number;
  /** Of the swings made with a usable shield, how many were released in an opening. */
  openings: number;
  landed: number;
  noDamage: number;
  crits: number;
  guards: number;
  shieldDisabled: number;
  draughts: number;
  /** Health losses while a fight was open, and their sum. */
  hitsTaken: number;
  damageTaken: number;
}

export function emptyTally(): MeleeTally {
  return { swings: 0, openings: 0, landed: 0, noDamage: 0, crits: 0, guards: 0, shieldDisabled: 0, draughts: 0, hitsTaken: 0, damageTaken: 0 };
}

/** One line for the log: what the hands did this fight. */
export function describeTally(t: MeleeTally): string {
  return (
    `${t.swings} charged swing(s) (${t.landed} hurt the target, ${t.noDamage} did nothing), ` +
    `${t.crits} critical, ${t.openings} in an opening, shield raised ${t.guards}×, ` +
    `shield disabled ${t.shieldDisabled}×, ${t.draughts} draught(s) drunk; ` +
    `took ${t.hitsTaken} hit(s), ${t.damageTaken.toFixed(1)} damage`
  );
}

/**
 * The server's verdict on a player's swing, keyed by the protocol id of the
 * sound it plays at the attacker, in the pinned 1.21.11 `minecraft:sound_event`
 * registry (the server jar's own registry report): `entity.player.attack.crit`
 * 1241, `.knockback` 1242, `.nodamage` 1243, `.strong` 1244, `.sweep` 1245,
 * `.weak` 1246. `nodamage` follows a swing that hurt nothing; every other one
 * follows a hurt target.
 *
 * By id, never by the name mineflayer attaches: minecraft-data files every
 * 1.21.11 sound one id late (it has `entity.player.attack.strong` at 1245), and
 * minecraft-protocol already takes the holder's +1 off, so mineflayer's
 * `soundEffectHeard` names each sound after the one registered before it — a
 * sweep arrives called `strong`, a strong hit called `nodamage`.
 * `test/melee.test.ts` pins that offset, so a data fix fails a test rather than
 * silently re-labelling the tally.
 */
export const ATTACK_SOUND_VERDICT: ReadonlyMap<number, "landed" | "nodamage"> = new Map([
  [1241, "landed"],
  [1242, "landed"],
  [1243, "nodamage"],
  [1244, "landed"],
  [1245, "landed"],
  [1246, "landed"],
]);

/** The verdict for a `sound_effect` packet's registry id, `undefined` for any
 * sound that is not a player's attack. */
export function swingVerdict(soundId: number): "landed" | "nodamage" | undefined {
  return ATTACK_SOUND_VERDICT.get(soundId);
}

/** How near the bot (blocks) an attack sound must play to be the bot's own swing —
 * vanilla plays it at the attacker, which is the bot's server-side position,
 * up to a jump and a step away from where the client last put it. */
export const OWN_SWING_RADIUS = 3;

/** The cooldown group vanilla puts a disabled shield in. */
export const SHIELD_COOLDOWN_GROUP = "minecraft:shield";

/** How long (ms) a drink may take before it is abandoned: a potion's 32-tick use
 * plus the round trip. */
export const DRINK_TIMEOUT_MS = 2_500;
