import { test } from "node:test";
import assert from "node:assert/strict";
import {
  allowNonCollidingEntities,
  configureLeg,
  NON_COLLIDING_ENTITY_TYPES,
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
  // Defaults mirror mineflayer-pathfinder: digging/towers on, doors off, sprinting on.
  return { canDig: true, allow1by1towers: true, canOpenDoors: false, allowSprinting: true };
}

test("configureLeg locks adventure-mode movement and leaves sprint alone for a plain leg", () => {
  const bot = new FakeControlBot();
  const movements = freshMovements();
  const restore = configureLeg(bot, movements, false);
  assert.equal(movements.canDig, false);
  assert.equal(movements.allow1by1towers, false);
  // Doors/fence-gates are opened (a use interaction adventure mode allows) so the bot
  // can enter an area whose only opening is a closed gate (the ram pen).
  assert.equal(movements.canOpenDoors, true);
  assert.equal(movements.allowSprinting, true); // not a sneak leg → sprint untouched
  assert.deepEqual(bot.toggles, []); // no sneak control toggled
  // Restore is a no-op and must not toggle sneak on a leg that never sneaked.
  restore();
  assert.deepEqual(bot.toggles, []);
});

test("allowNonCollidingEntities marks display/interaction/marker entities passable", () => {
  // A fresh Movements starts with the pathfinder's default passable set (mobs and
  // the mannequin NPC are NOT in it and stay avoided). Seed one solid entity to
  // prove it is left untouched.
  const movements = { passableEntities: new Set<string>(["arrow"]) };
  allowNonCollidingEntities(movements);
  // Every non-colliding type is now passable.
  for (const name of NON_COLLIDING_ENTITY_TYPES) {
    assert.ok(
      movements.passableEntities.has(name),
      `${name} must be passable (it has no player-blocking collision box)`,
    );
  }
  // The interaction hitbox that congested the terminal NPC approach is passable.
  assert.ok(movements.passableEntities.has("interaction"));
  // Solid/pushing entities are NOT added — a mob or mannequin stays an obstacle.
  assert.ok(!movements.passableEntities.has("zombie"));
  assert.ok(!movements.passableEntities.has("mannequin"));
  assert.ok(!movements.passableEntities.has("armor_stand"));
  // Pre-existing entries are preserved (additive, never removes).
  assert.ok(movements.passableEntities.has("arrow"));
});

test("configureLeg disables sprint and turns sneak ON for a sneak leg", () => {
  const bot = new FakeControlBot();
  const movements = freshMovements();
  const restore = configureLeg(bot, movements, true);
  assert.equal(movements.canDig, false);
  assert.equal(movements.allow1by1towers, false);
  assert.equal(movements.canOpenDoors, true); // gate-opening applies to a sneak leg too
  assert.equal(movements.allowSprinting, false); // sprinting bot is not sneaking
  assert.deepEqual(bot.toggles, [["sneak", true]]);
  // Restore clears the crouch so a later plain leg is not left sneaking.
  restore();
  assert.deepEqual(bot.toggles, [
    ["sneak", true],
    ["sneak", false],
  ]);
});
