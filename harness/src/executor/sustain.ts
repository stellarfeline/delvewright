// MineflayerExecutor: the body's health: damage, health, eating, holding full health.

import type { Entity } from "prismarine-entity";
import type { Item } from "prismarine-item";
import { ATTRIBUTION_RANGE, attributeBotDamage, type ThreatCandidate } from "../threat.ts";
import {
  EAT_COOLDOWN_MS,
  EAT_SAFE_RANGE,
  eatDecision,
  isSafeFood,
  pickFood,
  restoreAmplifier,
} from "../sustain.ts";
import { isWaveMob } from "./wave.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** How long (ms) after a drink before another is considered, so the health the
 * first one restored has arrived before the decision is taken again. */
const DRINK_SETTLE_MS = 1_000;

/** Vanilla entity event 9: the entity finished using its item (the drink landed). */
const ENTITY_EVENT_USE_FINISHED = 9;

/**
 * How long (ms) after a damage packet with a named source the health-drop fallback
 * stays quiet. mineflayer emits `entityHurt` (from `damage_event`) and the health
 * update from the same server tick batch; this grace stops one hit being counted twice
 * — once attributed, once guessed.
 */
const HEALTH_ATTRIBUTION_GRACE_MS = 500;

/**
 * Vanilla player maxima on 1.21.11. A delve's class kit changes gear, never these
 * attributes, so they are constants rather than a per-run read.
 */
const PLAYER_MAX_HEALTH = 20;

const PLAYER_MAX_FOOD = 20;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Every hostile the bot can currently see, as {@link ThreatCandidate}s plus a lookup
   * back to the live entities. "Hostile" is exactly {@link isWaveMob} — the same
   * classifier the kill loop uses, so NPC mannequins, displays and dropped items can
   * never be recorded as attackers or become defense targets.
   */
  visibleHostiles(this: MineflayerExecutor): {
    candidates: ThreatCandidate[];
    byId: Map<number, Entity>;
  } {
    const bot = this.bot;
    const candidates: ThreatCandidate[] = [];
    const byId = new Map<number, Entity>();
    if (!bot?.entity) return { candidates, byId };
    const here = bot.entity.position;
    for (const entity of Object.values(bot.entities)) {
      if (!entity?.position || !isWaveMob(entity, bot.entity, this.requireNonCombatants()))
        continue;
      candidates.push({ id: entity.id, distance: here.distanceTo(entity.position) });
      byId.set(entity.id, entity);
    }
    return { candidates, byId };
  },

  /**
   * The bot took a hit — take whatever dealt it out of the run.
   *
   * **Staging, not self-defence.** The ladder does not fight, so there is no
   * exchange to win and nothing here judges one: a body that draws the bot's blood
   * is removed by one attributed command and named in the run artifact as a staged
   * removal. What the ladder is for at a combat step — the wave spawned as
   * declared, the kill wiring fired, the re-seat re-seated, dying is safe — is not
   * touched by it, and none of it depends on the bot surviving an exchange.
   *
   * `sourceId` is the entity the server named as responsible
   * (`damage_event.sourceCauseId`, resolved by mineflayer), or `undefined` when it
   * named none — see {@link attributeBotDamage} for what happens then.
   */
  onBotDamaged(this: MineflayerExecutor, sourceId: number | undefined): void {
    const { candidates, byId } = this.visibleHostiles();
    const attacker = attributeBotDamage(sourceId, candidates);
    if (attacker === undefined) return;
    const named = sourceId !== undefined && attacker === sourceId;
    if (named) {
      this.lastAttributionAt = Date.now();
      this.lastNamedHit = { id: attacker, at: this.lastAttributionAt };
    }
    // ONLY a body the server itself named. The nearest-hostile fallback is a
    // guess, and a guess that removes a body is the harness deleting part of the
    // delve on suspicion: measured on the gallery, where a burn tick with no
    // source entity was attributed to a villager standing beside the bot and
    // staged it away. A guess is good enough to explain a health drop in the log
    // and nowhere near good enough to act on.
    if (!named) {
      process.stderr.write(
        `[staged] the bot was hit with no source the server named; the nearest body is ` +
          `${byId.get(attacker)?.name ?? "?"}#${attacker}, which is a guess and so is left ` +
          `standing\n`,
      );
      return;
    }
    // The stages are over: a body that hits the bot now is outside every stage,
    // and a reading started now would be of a world no stage observed.
    if (this.stagingClosed) return;
    const task: Promise<void> = this.stageAway(
      attacker,
      "it hit the bot (the server named it)",
      byId.get(attacker)?.name,
    ).finally(() => this.stagingTasks.delete(task));
    this.stagingTasks.add(task);
  },

  /**
   * Health-drop fallback attribution: a drop with no packet-named source in the last
   * {@link HEALTH_ATTRIBUTION_GRACE_MS} is blamed on the nearest hostile within
   * {@link ATTRIBUTION_RANGE}. If nothing is that close, nothing is staged away — a
   * fall, a trap or drowning must never take a bystander out of the delve.
   */
  onHealthUpdate(this: MineflayerExecutor): void {
    const bot = this.bot;
    if (!bot) return;
    const previous = this.lastHealth;
    this.lastHealth = bot.health;
    if (previous === undefined || bot.health >= previous) return;
    if (this.walkLegs > 0) void this.holdFullHealth(`a drop to ${bot.health.toFixed(1)} on it`);
    // A drop inside the grace of a NAMED hit is that body's blow — what a staged
    // removal of it refunds (see `refundBlows`).
    const hit = this.lastNamedHit;
    if (hit !== undefined && Date.now() - hit.at < HEALTH_ATTRIBUTION_GRACE_MS) {
      this.damageBy.set(hit.id, (this.damageBy.get(hit.id) ?? 0) + (previous - bot.health));
    }
    if (Date.now() - this.lastAttributionAt < HEALTH_ATTRIBUTION_GRACE_MS) return;
    const { candidates, byId } = this.visibleHostiles();
    const attacker = attributeBotDamage(undefined, candidates, ATTRIBUTION_RANGE);
    if (attacker === undefined) return;
    // Logged, never acted on: a drop with no source is a fall, a trap, drowning or
    // fire as readily as a blow, and nothing here can tell them apart. Removing a
    // body on this evidence would be the harness answering a lethal volume by
    // deleting whoever was standing nearby.
    process.stderr.write(
      `[staged] the bot lost ${(previous - bot.health).toFixed(1)} health with no named source; ` +
        `the nearest body is ${byId.get(attacker)?.name ?? "?"}#${attacker}, which is a guess and ` +
        `so is left standing\n`,
    );
  },

  /**
   * Every edible item currently in the inventory that is safe to eat, with its hunger
   * value. The registry says what is edible; {@link isSafeFood} says what a player
   * would actually swallow — rotten flesh is food to minecraft-data and poison to the
   * run (round 2: 7.3 → 3.4 health from the bot's own "eat when hurt" behavior).
   */
  foodInInventory(this: MineflayerExecutor): Array<{ item: Item; name: string; foodPoints: number }> {
    const bot = this.requireBot();
    // The pinned minecraft-data registry is the single source of truth for what counts
    // as food — no hardcoded item list to drift from the class kits.
    const foods = (bot.registry as unknown as { foods?: Record<number, { foodPoints?: number }> })
      .foods;
    if (!foods) return [];
    const out: Array<{ item: Item; name: string; foodPoints: number }> = [];
    for (const item of bot.inventory.items()) {
      const food = foods[item.type];
      if (!food) continue;
      if (!isSafeFood(item.name)) continue;
      out.push({ item, name: item.name, foodPoints: food.foodPoints ?? 0 });
    }
    return out;
  },

  /**
   * Eat, the way a player does, when the bot is hurt and nothing is in its face.
   *
   * The class kits hand every class food (the-drowned-bell gives each one rabbit stew);
   * before this the bot carried it through the whole delve untouched, so damage taken
   * in one fight was still missing at the start of the next. Bounded and throttled by
   * {@link EAT_COOLDOWN_MS}; every outcome — including the reasons NOT to eat — is
   * logged, so a run's log says whether the bot was healing or just hurt.
   *
   * Asserts nothing and hides nothing: a bot that dies still fails the run.
   */
  async maybeEat(this: MineflayerExecutor, label: string): Promise<void> {
    const bot = this.bot;
    if (!bot?.entity) return;
    if (Date.now() - this.lastEatAt < EAT_COOLDOWN_MS) return;
    const foods = this.foodInInventory();
    const decision = eatDecision({
      health: bot.health,
      maxHealth: PLAYER_MAX_HEALTH,
      food: bot.food,
      maxFood: PLAYER_MAX_FOOD,
      nearestHostileDistance: this.nearestHostile(),
      hasFood: foods.length > 0,
    });
    if (decision === "healthy") return;
    this.lastEatAt = Date.now();
    const state =
      `health ${bot.health.toFixed(1)}/${PLAYER_MAX_HEALTH}, hunger ${bot.food}/${PLAYER_MAX_FOOD}`;
    if (decision !== "eat") {
      const why = {
        "no-food": "no safe edible item in the kit (harmful food is never eaten)",
        "hostile-near": `a hostile is within ${EAT_SAFE_RANGE} blocks — eating would donate free hits`,
        "hunger-full": "hunger is full, so vanilla forbids eating; natural regeneration is running",
      }[decision];
      process.stderr.write(`[eat] ${label}: not eating (${state}) — ${why}\n`);
      return;
    }
    const choice = pickFood(foods);
    if (!choice) return; // unreachable (hasFood was true) — defensive
    const before = bot.health;
    try {
      await bot.equip(choice.item, "hand");
      await bot.consume();
      process.stderr.write(
        `[eat] ${label}: ate ${choice.name} at ${state} → health ` +
          `${bot.health.toFixed(1)}/${PLAYER_MAX_HEALTH}, hunger ${bot.food}/${PLAYER_MAX_FOOD} ` +
          `(was ${before.toFixed(1)})\n`,
      );
    } catch (err) {
      // Eating is opportunistic: a failed bite is reported and the run carries on.
      process.stderr.write(
        `[eat] ${label}: could not eat ${choice.name} (${state}): ` +
          `${err instanceof Error ? err.message : String(err)}\n`,
      );
    } finally {
      // Back to the sword — never leave the bot walking into a fight holding a bowl.
      await this.equipLoadout();
    }
  },

  /**
   * How far the nearest hostile body is, or `undefined` when none is tracked.
   * Read by the eat rule, which will not stand still to eat with one in reach.
   */
  nearestHostile(this: MineflayerExecutor): number | undefined {
    const { candidates } = this.visibleHostiles();
    let best: number | undefined;
    for (const c of candidates) if (best === undefined || c.distance < best) best = c.distance;
    return best;
  },

  /**
   * **Hold the bot at full health for the walk leg in progress.**
   *
   * The ladder verifies mechanism; whether the bot survives a walk is not part of
   * what any walk leg is for. Yet a hostile's FIRST blow lands before the body can
   * be staged away, and it lands on whatever health the bot has: a bot that
   * arrives at 4.4 is killed by a blow a player at full health shrugs off, and
   * that death is the harness's, not the delve's. Two were measured on vesperhold
   * — an Unremembered Guard on the die-retry return leg, which had begun at the
   * respawn's full 20 and bled to 14.8 on blows the per-body refund never
   * attributed, and a mob on the death-loop approach, which began where the
   * critical path left the bot. So the rule is not "refund what each body took"
   * but "full health, at the start of every walk leg and after every drop on
   * one": {@link walkTo} calls this on entry, and {@link onHealthUpdate} on every
   * drop while a leg is in progress.
   *
   * One instant-health effect of the amplifier that covers the whole deficit
   * ({@link restoreAmplifier}), read by the shared rejection rule and named in
   * `staged_removals` as staging, like every other act of the harness on the
   * world. A lethal volume, a crush gate or any single blow of 20 or more still
   * kills: nothing here can undo a death, and the delve's own hazards keep their
   * reach.
   */
  holdFullHealth(this: MineflayerExecutor, when: string): Promise<void> {
    if (this.restoringHealth) return this.restoringHealth;
    // Read now: the effect's reply is awaited, and the leg can end meanwhile.
    const leg = this.walkLabel;
    const run = async (): Promise<void> => {
      // Bounded: a drop that lands while one effect is in flight is closed by
      // the next round, and a server that keeps refusing is recorded, not retried.
      for (let round = 0; round < 3; round++) {
        const bot = this.bot;
        if (!bot?.entity || this.death || !(bot.health > 0)) return;
        const before = bot.health;
        const deficit = PLAYER_MAX_HEALTH - before;
        if (deficit <= 0) return;
        const amp = restoreAmplifier(deficit);
        const refusal = await this.refusalOf(`/effect give @s minecraft:instant_health 1 ${amp} true`);
        this.stagedRemovals.push({
          kind: "player",
          why:
            `full health for walk leg '${leg}' (${when}): ${before.toFixed(1)} of ` +
            `${PLAYER_MAX_HEALTH}, restored with instant health ${amp + 1}`,
          performed: refusal === undefined,
          detail: refusal,
        });
        process.stderr.write(
          `[staged] walk leg '${leg}' (${when}): health ${before.toFixed(1)} → ` +
            `${refusal === undefined ? bot.health.toFixed(1) : `unchanged, refused — ${refusal}`}\n`,
        );
        if (refusal !== undefined) return;
      }
    };
    this.restoringHealth = run().finally(() => {
      this.restoringHealth = undefined;
    });
    return this.restoringHealth;
  },
};
