// mineflayer-backed StepExecutor. Connects a headless bot to a pinned 1.21.11
// server and drives the critical path against the amended bot-interaction
// contract (spec-0002, 2026-07-30).
//
// Interaction channel (settled): Minecraft 1.21.6+ routes NPC dialogue / class
// selection through the server-driven dialog system. mineflayer 4.37.x exposes no
// high-level dialog API and cannot reliably emit a dialog button click, so the
// compiler emits every dialog button as a `run_command` firing a `/trigger`, and
// the bot drives the same outcome by chatting that exact command (`bot.chat`).
// select-class / talk-to therefore just send `step.command`; talk-to walks to the
// NPC first (realism + reach mechanics that some dialogs gate on).

import { createBot, type Bot } from "mineflayer";
// mineflayer-pathfinder is CommonJS; import the default and destructure (the only
// harness dependency added for v0.3 — replaces the naive "face + hold forward"
// walk so turns/branches in jigsaw layouts are walkable).
import pathfinderPkg from "mineflayer-pathfinder";
const { pathfinder, Movements, goals } = pathfinderPkg;
import type {
  AssertCompleteStep,
  CollectStep,
  InteractStep,
  KillStep,
  ReachStep,
  SelectClassStep,
  TalkToStep,
} from "./critical-path.ts";
import type { StepExecutor } from "./sequencer.ts";

/** Connection + identity for the bot. Sourced from the environment (see below). */
export interface BotConfig {
  readonly host: string;
  readonly port: number;
  readonly username: string;
  /** Pinned per ADR-0009; mineflayer's max supported version is 1.21.11. */
  readonly version: string;
  /** `offline` for local/CI (offline-mode server); `microsoft` for real accounts. */
  readonly auth: "offline" | "microsoft";
}

/** The pinned Minecraft version (ADR-0009). Single source of truth for the harness. */
export const PINNED_MC_VERSION = "1.21.11";

/**
 * Build a {@link BotConfig} from environment variables, with local-testing
 * defaults:
 *   DELVEWRIGHT_MC_HOST      (default `127.0.0.1`)
 *   DELVEWRIGHT_MC_PORT      (default `25565`)
 *   DELVEWRIGHT_BOT_USERNAME (default `delve-bot`)
 *   DELVEWRIGHT_MC_VERSION   (default `1.21.11`, the ADR-0009 pin)
 *   DELVEWRIGHT_MC_AUTH      (`offline` | `microsoft`, default `offline`)
 */
export function botConfigFromEnv(
  env: Record<string, string | undefined> = process.env,
): BotConfig {
  const portRaw = env["DELVEWRIGHT_MC_PORT"] ?? "25565";
  const port = Number.parseInt(portRaw, 10);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(
      `DELVEWRIGHT_MC_PORT must be a valid TCP port, got ${JSON.stringify(portRaw)}`,
    );
  }
  const authRaw = env["DELVEWRIGHT_MC_AUTH"] ?? "offline";
  if (authRaw !== "offline" && authRaw !== "microsoft") {
    throw new Error(
      `DELVEWRIGHT_MC_AUTH must be 'offline' or 'microsoft', got ${JSON.stringify(authRaw)}`,
    );
  }
  return {
    host: env["DELVEWRIGHT_MC_HOST"] ?? "127.0.0.1",
    port,
    username: env["DELVEWRIGHT_BOT_USERNAME"] ?? "delve-bot",
    version: env["DELVEWRIGHT_MC_VERSION"] ?? PINNED_MC_VERSION,
    auth: authRaw,
  };
}

/** How long (ms) a movement step may run before it is declared failed. */
const REACH_TIMEOUT_MS = 60_000;
/** Polling interval (ms) while walking toward a target. */
const REACH_POLL_MS = 250;
/** How long (ms) a `kill` step may run before it is declared failed. */
const KILL_TIMEOUT_MS = 90_000;
/** Attack cadence (ms) — roughly the vanilla sword cooldown. */
const ATTACK_INTERVAL_MS = 400;
/**
 * How long (ms) to wait for a scoreboard value to reach its target after a chat
 * command. The datapack acts on the trigger on the next server tick(s); give it a
 * generous window so slow CI servers don't flake.
 */
const SCORE_SETTLE_MS = 15_000;
const SCORE_POLL_MS = 250;
/** Settle time (ms) after class selection (teleport + kit give) before moving on. */
const CLASS_SETTLE_MS = 3_000;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Reject with a labelled error if `promise` does not settle within `ms`. */
function withTimeout<T>(promise: Promise<T>, ms: number, what: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const guard = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${what}: timed out after ${ms}ms`)), ms);
  });
  return Promise.race([promise, guard]).finally(() => clearTimeout(timer));
}

/**
 * A mineflayer-backed executor. Construct, `await connect()`, then hand it to
 * `runSequence`. `close()` disconnects the bot. Not reusable across servers.
 */
/**
 * Stable completion marker the compiler broadcasts on campaign completion:
 * `[Delvewright] complete <objective> <value>`. The bot observes THIS (chat is
 * reliably parsed by mineflayer) rather than the sidebar score, because mineflayer
 * 4.37.x cannot decode 1.21.11 scoreboard score packets. The datapack still
 * displays the objective in the sidebar (amended contract, spec-0002) — this is
 * only the harness's observation channel, and it can switch to the live sidebar
 * read once mineflayer gains 1.21.11 score support.
 */
const COMPLETION_MARKER = /\[Delvewright\] complete (\S+) (-?\d+)/;

export class MineflayerExecutor implements StepExecutor {
  private readonly config: BotConfig;
  private bot: Bot | undefined;
  /** Latest value seen per objective from broadcast completion markers. */
  private readonly markerScores = new Map<string, number>();

  constructor(config: BotConfig) {
    this.config = config;
  }

  /** Connect and resolve once the bot has spawned into the world. */
  async connect(): Promise<void> {
    const bot = createBot({
      host: this.config.host,
      port: this.config.port,
      username: this.config.username,
      version: this.config.version,
      auth: this.config.auth,
    });
    this.bot = bot;
    bot.loadPlugin(pathfinder);

    // Capture completion markers from the moment we connect: the marker is
    // broadcast when the campaign completes, which happens DURING the final reach
    // step (before assertComplete runs), so it must be buffered as it arrives.
    bot.on("messagestr", (message: string) => {
      const match = COMPLETION_MARKER.exec(message);
      if (match) {
        this.markerScores.set(match[1]!, Number.parseInt(match[2]!, 10));
      }
    });

    await new Promise<void>((resolve, reject) => {
      const onSpawn = (): void => {
        cleanup();
        resolve();
      };
      const onError = (err: Error): void => {
        cleanup();
        reject(err);
      };
      const onEnd = (reason: string): void => {
        cleanup();
        reject(new Error(`bot disconnected before spawn: ${reason}`));
      };
      const onKicked = (reason: string): void => {
        cleanup();
        reject(new Error(`bot kicked before spawn: ${reason}`));
      };
      const cleanup = (): void => {
        bot.removeListener("spawn", onSpawn);
        bot.removeListener("error", onError);
        bot.removeListener("end", onEnd);
        bot.removeListener("kicked", onKicked);
      };
      bot.once("spawn", onSpawn);
      bot.once("error", onError);
      bot.once("end", onEnd);
      bot.once("kicked", onKicked);
    });
  }

  private requireBot(): Bot {
    if (!this.bot) {
      throw new Error("executor is not connected; call connect() first");
    }
    return this.bot;
  }

  /** Disconnect the bot, if connected. Safe to call more than once. */
  close(): void {
    if (this.bot) {
      this.bot.end();
      this.bot = undefined;
    }
  }

  async selectClass(step: SelectClassStep): Promise<void> {
    const bot = this.requireBot();
    // The class-selection dialog button runs `step.command` (a `/trigger`); the
    // bot fires the same command directly. The per-tick handler then applies the
    // kit and teleports the player to the campaign spawn.
    bot.chat(step.command);
    // Give the datapack a tick to reset the trigger, give the kit and teleport.
    await delay(CLASS_SETTLE_MS);
    // Equip the kit (sword + armor) so the bot can fight v0.3 combat waves. A
    // no-op for kits without those items.
    await this.equipLoadout();
  }

  /**
   * Equip the best weapon and each armour piece from the current inventory. Item
   * names are matched by substring (`sword`, `helmet`, …). Best-effort per slot.
   */
  private async equipLoadout(): Promise<void> {
    const bot = this.requireBot();
    const slots: ReadonlyArray<[string, "hand" | "head" | "torso" | "legs" | "feet"]> = [
      ["sword", "hand"],
      ["helmet", "head"],
      ["chestplate", "torso"],
      ["leggings", "legs"],
      ["boots", "feet"],
    ];
    for (const [key, dest] of slots) {
      const item = bot.inventory.items().find((i) => i.name.includes(key));
      if (item) {
        try {
          await bot.equip(item, dest);
        } catch {
          // best effort — a missing slot is not a failure
        }
      }
    }
  }

  async talkTo(step: TalkToStep): Promise<void> {
    const bot = this.requireBot();
    // Walk to the NPC first (realism; some dialog effects are reach-gated), then
    // chat the dialog-option `/trigger` command the button would have run.
    await this.walkTo(step.pos, 3, `npc ${step.npc}`);
    bot.chat(step.command);
    // Give the datapack time to apply the dialog effect (e.g. open the gate).
    await delay(2_000);
  }

  /**
   * Walk to the anchor using the pathfinder. The datapack completes the objective
   * with a `distance=..radius` check on the exact anchor point, so the bot targets
   * a goal one block *tighter* than `radius` — landing well inside the check rather
   * than on its boundary (where the pathfinder's block-granular goal and the
   * server's precise-position check can disagree).
   */
  async reach(step: ReachStep): Promise<void> {
    await this.walkTo(step.pos, Math.max(1, step.radius - 1), `anchor ${step.anchor}`);
  }

  /**
   * Pathfind to within `range` blocks of the absolute target (mineflayer-pathfinder
   * `GoalNear`). Replaces the pre-v0.3 "face + hold forward" walk, so turns and
   * branches in jigsaw layouts are walkable. Digging is disabled (adventure mode).
   */
  private async walkTo(
    pos: readonly [number, number, number],
    range: number,
    label: string,
  ): Promise<void> {
    const bot = this.requireBot();
    const [x, y, z] = pos;
    const r = Math.max(1, Math.floor(range));
    const movements = new Movements(bot);
    movements.canDig = false; // adventure mode: never break blocks
    movements.allow1by1towers = false;
    bot.pathfinder.setMovements(movements);
    try {
      await withTimeout(
        bot.pathfinder.goto(new goals.GoalNear(x, y, z, r)),
        REACH_TIMEOUT_MS,
        `reaching ${label}`,
      );
    } catch (err) {
      try {
        bot.pathfinder.stop();
      } catch {
        // ignore
      }
      const detail = err instanceof Error ? err.message : String(err);
      throw new Error(
        `failed ${label} at [${x}, ${y}, ${z}] (range ${r}); bot at ` +
          `${fmt(bot.entity.position)}: ${detail}`,
      );
    }
  }

  /**
   * Slay a wave: go to the wave anchor, then attack the nearest hostile mob in a
   * loop until none remain (the world is sealed, so the only mobs are the wave)
   * or the budget runs out. The datapack's kill advancement + countdown complete
   * the objective when the last mob dies.
   */
  async kill(step: KillStep): Promise<void> {
    const bot = this.requireBot();
    await this.equipLoadout();
    await this.walkTo(step.pos, 3, `wave ${step.wave}`);
    // Give AI-enabled mobs a moment to path toward the bot after we arrive.
    await delay(1_000);
    // Diagnostic: what does the bot see near the wave anchor?
    const near = Object.values(bot.entities)
      .filter((e) => e && e !== bot.entity && bot.entity.position.distanceTo(e.position) < 48)
      .map((e) => `${e.name ?? "?"}(t=${e.type},k=${(e as { kind?: string }).kind ?? "?"},h=${e.height ?? "?"})`);
    process.stderr.write(`[kill ${step.wave}] nearby(${near.length}): ${near.join(", ") || "none"}\n`);

    const deadline = Date.now() + KILL_TIMEOUT_MS;
    let emptyStreak = 0;
    while (Date.now() < deadline) {
      const mob = bot.nearestEntity((e) => isWaveMob(e, bot.entity));
      if (!mob) {
        // A sustained absence of wave mobs (world is sealed) → wave cleared.
        if (++emptyStreak >= 8) return;
        await delay(REACH_POLL_MS);
        continue;
      }
      emptyStreak = 0;
      const dist = bot.entity.position.distanceTo(mob.position);
      if (dist > 3) {
        await this.walkTo(
          [Math.floor(mob.position.x), Math.floor(mob.position.y), Math.floor(mob.position.z)],
          2,
          `mob ${mob.name ?? "?"}`,
        );
      } else {
        await bot.lookAt(mob.position.offset(0, (mob.height ?? 1) * 0.5, 0), true);
        bot.attack(mob);
        await delay(ATTACK_INTERVAL_MS);
      }
    }
    throw new Error(
      `kill timed out after ${KILL_TIMEOUT_MS}ms: wave ${step.wave} ` +
        `(${step.count} mobs) not cleared`,
    );
  }

  /** Collect items from the chest at the anchor: go there, open it, withdraw all. */
  async collect(step: CollectStep): Promise<void> {
    const bot = this.requireBot();
    await this.walkTo(step.pos, 2, `chest ${step.item}`);
    const here = bot.entity.position;
    const target = here.offset(
      step.pos[0] + 0.5 - here.x,
      step.pos[1] + 0.5 - here.y,
      step.pos[2] + 0.5 - here.z,
    );
    const block = bot.blockAt(target);
    if (!block) {
      throw new Error(`no block at collect anchor [${step.pos.join(", ")}]`);
    }
    const chest = await bot.openContainer(block);
    try {
      for (const item of chest.containerItems()) {
        await chest.withdraw(item.type, null, item.count);
      }
    } finally {
      chest.close();
    }
    // Let the inventory_changed advancement fire the completion.
    await delay(SCORE_POLL_MS * 4);
  }

  /** Interact at the anchor: go there, then chat the emitted `/trigger` command. */
  async interact(step: InteractStep): Promise<void> {
    const bot = this.requireBot();
    await this.walkTo(step.pos, 3, `interact ${step.anchor}`);
    // The interaction advancement and this chat command both feed the same
    // per-tick handler; the datapack applies the requires_item + flag guards.
    bot.chat(step.command);
    await delay(2_000);
  }

  async assertComplete(step: AssertCompleteStep): Promise<void> {
    const bot = this.requireBot();
    // Completion is observed two ways, whichever surfaces first:
    //   1. The broadcast completion marker (the working path on 1.21.11 — see
    //      COMPLETION_MARKER), buffered since connect.
    //   2. The sidebar score via mineflayer (future-proof: works if/when mineflayer
    //      gains 1.21.11 score-packet support; currently always unset).
    // The score is set on a server tick after the final reach, so poll until it
    // settles or the budget runs out.
    const deadline = Date.now() + SCORE_SETTLE_MS;
    while (Date.now() < deadline) {
      if (this.markerScores.get(step.objective) === step.value) {
        return;
      }
      const board = bot.scoreboards[step.objective];
      if (board?.itemsMap[bot.username]?.value === step.value) {
        return;
      }
      await delay(SCORE_POLL_MS);
    }
    const marker = this.markerScores.has(step.objective)
      ? `${this.markerScores.get(step.objective)}`
      : "no marker received";
    const board = bot.scoreboards[step.objective];
    const sidebar = board?.itemsMap[bot.username]?.value ?? "unset";
    throw new Error(
      `campaign not complete after ${SCORE_SETTLE_MS}ms: objective ${step.objective} ` +
        `expected ${step.value}; completion marker: ${marker}; sidebar: ${sidebar}`,
    );
  }
}

function fmt(p: { x: number; y: number; z: number }): string {
  return `[${p.x.toFixed(1)}, ${p.y.toFixed(1)}, ${p.z.toFixed(1)}]`;
}

/**
 * Entity names that are never a combat target. The delve world is sealed
 * (`spawn_mobs false`), so the only living mobs are compiler-summoned wave mobs;
 * everything else near the bot is an NPC, a display, or a dropped object.
 */
const NON_WAVE_ENTITIES = new Set<string>([
  "player",
  "villager",
  "interaction",
  "item",
  "experience_orb",
  "arrow",
  "spectral_arrow",
  "armor_stand",
  "marker",
  "text_display",
  "block_display",
  "item_display",
  "area_effect_cloud",
  "item_frame",
  "glow_item_frame",
  "painting",
  "leash_knot",
  "fishing_bobber",
]);

/**
 * True if `e` is a slayable wave mob: not the bot, not a player/NPC/display/
 * dropped object, and tall enough to be a living mob (excludes small dropped
 * entities). Classified by name (reliable across mineflayer versions) rather than
 * `type`/`kind`, which vary.
 */
function isWaveMob(e: unknown, self: unknown): boolean {
  if (!e || e === self) return false;
  const ent = e as { name?: string; height?: number };
  const name = ent.name ?? "";
  if (name === "" || NON_WAVE_ENTITIES.has(name)) return false;
  return (ent.height ?? 0) >= 0.5;
}
