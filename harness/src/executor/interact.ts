// MineflayerExecutor: an interact step: pressing a block.

import type { InteractStep, Vec3Tuple } from "../critical-path.ts";
import { presentAndTrigger, presentItem } from "../held-item.ts";
import { parseBlockState, readBlockState } from "../block-state.ts";
import { FACE_NORMALS, pressFace } from "../press-face.ts";
import type { Item } from "prismarine-item";
import { delay, fmt } from "./connection.ts";
import { EFFECT_SETTLE_MS } from "./objective.ts";
import type { MineflayerExecutor } from "../executor.ts";

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
    await this.walkTo(step.pos, INTERACT_RANGE, `interact ${step.anchor}`, step.sneak, {
      objective: step.objective,
      transport: step.transport,
    });
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
