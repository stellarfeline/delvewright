import { test } from "node:test";
import assert from "node:assert/strict";
import {
  configureLeg,
  type ControlBot,
  type LegMovements,
} from "../src/movement.ts";

// A fake bot that records the sneak control-state toggles it is asked to make.
class FakeControlBot implements ControlBot {
  readonly toggles: Array<[string, boolean]> = [];
  setControlState(control: "sneak", state: boolean): void {
    this.toggles.push([control, state]);
  }
}

function freshMovements(): LegMovements {
  // Defaults mirror mineflayer-pathfinder: digging/towers on, sprinting on.
  return { canDig: true, allow1by1towers: true, allowSprinting: true };
}

test("configureLeg locks adventure-mode movement and leaves sprint alone for a plain leg", () => {
  const bot = new FakeControlBot();
  const movements = freshMovements();
  const restore = configureLeg(bot, movements, false);
  assert.equal(movements.canDig, false);
  assert.equal(movements.allow1by1towers, false);
  assert.equal(movements.allowSprinting, true); // not a sneak leg → sprint untouched
  assert.deepEqual(bot.toggles, []); // no sneak control toggled
  // Restore is a no-op and must not toggle sneak on a leg that never sneaked.
  restore();
  assert.deepEqual(bot.toggles, []);
});

test("configureLeg disables sprint and turns sneak ON for a sneak leg", () => {
  const bot = new FakeControlBot();
  const movements = freshMovements();
  const restore = configureLeg(bot, movements, true);
  assert.equal(movements.canDig, false);
  assert.equal(movements.allow1by1towers, false);
  assert.equal(movements.allowSprinting, false); // sprinting bot is not sneaking
  assert.deepEqual(bot.toggles, [["sneak", true]]);
  // Restore clears the crouch so a later plain leg is not left sneaking.
  restore();
  assert.deepEqual(bot.toggles, [
    ["sneak", true],
    ["sneak", false],
  ]);
});
