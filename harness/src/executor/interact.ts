// MineflayerExecutor: an interact step: pressing a block.

import type { InteractStep, Vec3Tuple } from "../critical-path.ts";
import { presentAndTrigger, presentItem } from "../held-item.ts";
import { parseBlockState, readBlockState } from "../block-state.ts";
import { FACE_NORMALS, pressFace } from "../press-face.ts";
import type { Item } from "prismarine-item";
import { delay, fmt } from "./connection.ts";
import { EFFECT_SETTLE_MS } from "./objective.ts";
import type { MineflayerExecutor } from "../executor.ts";
import { INTERACTION_REACH, acquire, boxOf, type Hitbox } from "../crosshair.ts";

/** Vanilla standing eye height, 1.21.11: where the pick ray starts. */
const EYE_HEIGHT = 1.62;

/** How far a tracked box's corner may sit from an exported tile's and still be
 * that tile: the summon's `f` floats against the export's doubles. */
const TILE_SLACK = 0.02;

export const INTERACT_RANGE = 3;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Interact at the anchor: go there, take the required item in hand, then chat
   * the emitted `/trigger` command.
   *
   * The interaction advancement and that chat command both feed the same per-tick
   * handler, and the datapack applies the `requires_item` + flag guards there —
   * `requires_item` against the MAINHAND, which is why the hand
   * is loaded first. See {@link presentAndTrigger}.
   */
  async interact(this: MineflayerExecutor, step: InteractStep): Promise<void> {
    const bot = this.requireBot();
    if (step.area !== undefined && step.stand !== undefined) {
      // Format 16: an area is used from the stance the build proved clicks it,
      // the cell the route's leg ends at; the bot right-clicks a tile there.
      const label = `interact ${step.anchor}`;
      if (this.aboard === undefined) {
        await this.walkTo(step.stand, 0, `${label} — to its stand cell`, step.sneak, {
          objective: step.objective,
          transport: step.transport,
        });
        await this.atStance();
      }
      await presentItem<Item>(bot, step, step.anchor);
      await this.clickArea(step, label);
      await this.requireObjective(step.objective, label);
      await delay(EFFECT_SETTLE_MS);
      return;
    }
    if (this.aboard === undefined) {
      await this.walkTo(step.pos, INTERACT_RANGE, `interact ${step.anchor}`, step.sneak, {
        objective: step.objective,
        transport: step.transport,
      });
      await this.atStance();
    } else {
      // spec-0135 §5: done from the seat the ride before it stayed in.
      process.stderr.write(`[seat] interact ${step.anchor}: from ${this.aboard.vehicle}'s seat\n`);
    }
    if (step.block !== undefined) {
      // spec-0093 §6.5: the thing is a vanilla block a hand presses — a lever,
      // a button — and the datapack reads vanilla's own report of the press.
      // No hitbox exists and no chat command is sent: the bot right-clicks the
      // block exactly as a player does, with the required item in hand.
      await presentItem<Item>(bot, step, step.anchor);
      await this.pressBlock(step.pos, step.block, `interact ${step.anchor}`);
    } else {
      // Same proof as `talkTo`: the affordance the party has to right-click must be
      // what a crosshair actually reaches, not merely what is nearest the cell.
      this.requireCrosshair(step.pos, `interact ${step.anchor}`, INTERACT_RANGE);
      this.armTrigger(step.command);
      await presentAndTrigger<Item>(bot, step, step.anchor);
    }
    await this.requireObjective(step.objective, `interact ${step.anchor}`);
    await delay(EFFECT_SETTLE_MS);
  },

  /**
   * Right-click an area (format 16): find the tracked `interaction` boxes that
   * are the area's tiles, prove from the bot's own eye that a ray reaches one
   * of them before any block or other body (the tiles never occlude each
   * other: every one is the one press), look at the point the build aimed at,
   * and right-click that tile as a player does.
   */
  async clickArea(this: MineflayerExecutor, step: InteractStep, label: string): Promise<void> {
    const bot = this.requireBot();
    const area = step.area!;
    const widest = Math.max(...area.tiles.map((t) => Math.max(t.max[0] - t.min[0], t.max[2] - t.min[2])));
    const bodies = this.hitboxesNear(area.cell, INTERACTION_REACH + widest + 5);
    const isTile = (h: Hitbox) => {
      if (h.name !== "interaction") return false;
      const b = boxOf(h);
      return area.tiles.some(
        (t) =>
          Math.abs(b.min.x - t.min[0]) < TILE_SLACK &&
          Math.abs(b.min.y - t.min[1]) < TILE_SLACK &&
          Math.abs(b.min.z - t.min[2]) < TILE_SLACK &&
          Math.abs(b.max.x - t.max[0]) < TILE_SLACK &&
          Math.abs(b.max.y - t.max[1]) < TILE_SLACK &&
          Math.abs(b.max.z - t.max[2]) < TILE_SLACK,
      );
    };
    const tiles = bodies.filter(isTile);
    if (tiles.length === 0) {
      throw new Error(
        `${label}: none of the area's ${area.tiles.length} click box(es) is tracked near ` +
          `[${area.cell.join(", ")}] — the area has no body to right-click, so the use can never ` +
          `land (bot at ${fmt(bot.entity.position)})`,
      );
    }
    const others = bodies.filter((b) => !isTile(b));
    const world = this.blockShapes();
    const p = bot.entity.position;
    const eye = { x: p.x, y: p.y + EYE_HEIGHT, z: p.z };
    let hit: { tile: Hitbox; aim: { x: number; y: number; z: number } } | undefined;
    for (const tile of tiles) {
      const a = acquire(eye, tile, others, INTERACTION_REACH, world.shapes, world.names);
      if (a.ok) {
        hit = { tile, aim: a.aim };
        break;
      }
    }
    if (hit === undefined) {
      throw new Error(
        `${label}: from the stance [${(step.stand ?? []).join(", ")}] no ray reaches any of the ` +
          `area's ${tiles.length} tracked click box(es) within ${INTERACTION_REACH} blocks before ` +
          `a block or another body (bot at ${fmt(bot.entity.position)})`,
      );
    }
    const target = bot.entities[hit.tile.id];
    if (!target) {
      throw new Error(`${label}: the area's click box #${hit.tile.id} is no longer tracked`);
    }
    const [ax, ay, az] = area.aim;
    await bot.lookAt(p.offset(ax - p.x, ay - p.y, az - p.z), true);
    process.stderr.write(
      `[interact] ${label}: right-clicking the area at [${area.cell.join(", ")}] through its ` +
        `click box at ${fmt(target.position)} from ${fmt(bot.entity.position)}\n`,
    );
    await bot.activateEntity(target);
  },

  /**
   * Right-click the vanilla block at `pos` (spec-0093 §6.5) — a lever flipped, a
   * button pressed — the act a block-bound interact or `use` trigger is fired
   * by. The block is read back first, so a missing or different block is the
   * datapack's defect named here rather than a silent click into air.
   */
  async pressBlock(this: MineflayerExecutor, pos: Vec3Tuple, expected: string, label: string): Promise<void> {
    const bot = this.requireBot();
    const here = bot.entity.position;
    const block = bot.blockAt(here.offset(pos[0] - here.x, pos[1] - here.y, pos[2] - here.z));
    const want = parseBlockState(expected).name;
    if (!block || parseBlockState(block.name).name !== want) {
      throw new Error(
        `${label}: expected the block \`${expected}\` at [${pos.join(", ")}] to press, found ` +
          `${block ? `\`${block.name}\`` : "no block (chunk unloaded?)"} — the act is the block's ` +
          `own use, so nothing else here can be pressed (bot at ${fmt(bot.entity.position)})`,
      );
    }
    const eye = bot.entity.position.offset(0, bot.entity.height, 0);
    // The face a player would click: of the faces the block performs its use
    // on (a bell rings only on some sides, `pressFace`), the one turned toward
    // the bot — a horizontal face even from straight below — with the cursor at
    // that face's middle.
    const state = readBlockState(block.name, block.getProperties());
    const side = pressFace(state, eye.x - (pos[0] + 0.5), eye.z - (pos[2] + 0.5));
    const [nx, nz] = FACE_NORMALS[side];
    const face = here.offset(nx - here.x, -here.y, nz - here.z);
    const cursor = here.offset(0.5 + nx * 0.5 - here.x, 0.5 - here.y, 0.5 + nz * 0.5 - here.z);
    await bot.lookAt(block.position.offset(0.5 + nx * 0.5, 0.5, 0.5 + nz * 0.5), true);
    process.stderr.write(
      `[press] ${label}: right-clicking the ${state} at [${pos.join(", ")}] on its ${side} ` +
        `face from ${fmt(eye)}\n`,
    );
    await bot.activateBlock(block, face, cursor);
  },
};
