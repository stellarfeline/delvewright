import { test } from "node:test";
import assert from "node:assert/strict";
import {
  allowNonCollidingEntities,
  configureLeg,
  describeStuckNeighbours,
  NON_COLLIDING_ENTITY_TYPES,
  partitionByPassability,
  type ControlBot,
  type LegMovements,
  type Neighbour,
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

// The muster hall's own crowd, taken verbatim from the `[stuck]` line of a gallery
// ladder run (`near [9.4, 65.0, 17.5]`), trimmed to the shapes that matter: one
// mannequin and one villager the pathfinder routes around, and the interaction /
// item_display / item hitboxes it routes through. The whole point of the report is
// that a reader can tell these two groups apart without re-deriving the rule.
const MUSTER_CROWD: readonly Neighbour[] = [
  { name: "mannequin", distance: 10.0 },
  { name: "interaction", distance: 10.0 },
  { name: "interaction", distance: 9.5 },
  { name: "item_display", distance: 10.8 },
  { name: "villager", distance: 6.4 },
  { name: "item", distance: 8.0 },
  { name: "spider", distance: 7.1 },
];

/** The passable set a leg actually walks with: pathfinder defaults + our additions. */
function legPassableSet(): Set<string> {
  // The subset of mineflayer-pathfinder 2.4.5's `passableEntities.json` that appears
  // in a delve: dropped items, arrows and orbs are passable to the search by default.
  const movements = { passableEntities: new Set<string>(["item", "arrow", "experience_orb"]) };
  allowNonCollidingEntities(movements);
  return movements.passableEntities;
}

test("partitionByPassability asks the pathfinder's own set, and keeps tracker order", () => {
  const { obstructing, passable } = partitionByPassability(MUSTER_CROWD, legPassableSet());
  // The bodies with a collision box the player cannot walk through.
  assert.deepEqual(
    obstructing.map((n) => n.name),
    ["mannequin", "villager", "spider"],
  );
  // Everything else is invisible to the search: interaction/display hitboxes are
  // marked passable by `allowNonCollidingEntities`, drops by the pathfinder itself.
  assert.deepEqual(
    passable.map((n) => n.name),
    ["interaction", "interaction", "item_display", "item"],
  );
  // Order within each half is the input's, so two reports are comparable.
  assert.deepEqual(
    passable.map((n) => n.distance),
    [10.0, 9.5, 10.8, 8.0],
  );
});

test("the stuck report separates the bodies routed AROUND from the ones routed THROUGH", () => {
  const line = describeStuckNeighbours(MUSTER_CROWD, legPassableSet());
  // The counts are the diagnosis: 3 of the 7 could obstruct anything at all.
  assert.match(line, /^7 entities within 12 blocks\./);
  assert.match(line, /Routed AROUND: 3 \(mannequin@10\.0, villager@6\.4, spider@7\.1\)/);
  assert.match(line, /Routed THROUGH: 4 \(/);
  // RED BEFORE THIS CHANGE: the report was an undifferentiated dump, so an
  // `interaction` hitbox the search never indexes appeared in the same breath as a
  // mannequin, and a round was commissioned against the resulting crowd. Nothing in
  // the old line could be matched for "this one cannot be the reason".
  assert.doesNotMatch(line, /Routed AROUND:[^.]*interaction/);
  assert.doesNotMatch(line, /Routed AROUND:[^.]*item_display/);
  // And it states what a body can and cannot do to this pathfinder, so the next
  // reader does not have to re-derive it from the dependency.
  assert.match(line, /never a wall/);
  assert.match(line, /No path to the goal!/);
});

test("the stuck report says so when nothing is near, and when nothing classified it", () => {
  // An empty neighbourhood is the strongest possible statement and must not read as
  // a missing measurement.
  assert.match(describeStuckNeighbours([], legPassableSet()), /nothing within 12 blocks/);
  // No Movements in force: report the list, and say it is UNCLASSIFIED rather than
  // implying a partition nobody computed.
  const line = describeStuckNeighbours(MUSTER_CROWD, undefined);
  assert.match(line, /7 entities within 12 blocks, unclassified/);
  assert.doesNotMatch(line, /Routed AROUND/);
});
