import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import {
  ATTACK_SPEED_KEY,
  HEALING_POTION_HEAL,
  DRINK_CLEAR_RANGE,
  DRINK_CRITICAL_FRACTION,
  KITE_DISTANCE,
  PLAYER_REACH,
  attackSpeedFrom,
  attributeValue,
  describeTally,
  drinkDecision,
  drinkHeal,
  emptyTally,
  fullChargeMs,
  footwork,
  guardUp,
  holdsRangedWeapon,
  inReach,
  jumpForCrit,
  swingVerdict,
} from "../src/melee.ts";

const require = createRequire(import.meta.url);

test("the attack-speed key is the one the pinned protocol files attribute id 4 under", () => {
  // Protocol id 4 is `minecraft:attack_speed` in the 1.21.11 server's own registry
  // report. mineflayer names update_attributes entries through this mapper, which
  // is wrong from id 20 onwards on 1.21.11 — pin the one entry the bot reads.
  const protocol = require(
    "minecraft-data/minecraft-data/data/pc/1.21.11/protocol.json",
  ) as {
    play: { toClient: { types: Record<string, unknown> } };
  };
  const packet = JSON.stringify(protocol.play.toClient.types["packet_entity_update_attributes"]);
  const mappings = /"mappings":(\{[^}]*\})/.exec(packet);
  assert.ok(mappings, "the attributes packet carries a key mapper");
  const table = JSON.parse(mappings![1]!) as Record<string, string>;
  assert.equal(table["4"], ATTACK_SPEED_KEY);
});

test("an attribute's value is the server's base with its modifiers applied in vanilla order", () => {
  // The iron sword the Sellsword kit carries: base 4.0, the item's -2.4 add_value.
  assert.ok(
    Math.abs(
      attributeValue({ value: 4, modifiers: [{ amount: -2.4000000953674316, operation: 0 }] }) -
        1.6,
    ) < 1e-6,
  );
  // add_value, then add_multiplied_base on the new base, then add_multiplied_total.
  assert.equal(
    attributeValue({
      value: 2,
      modifiers: [
        { amount: 2, operation: 0 },
        { amount: 0.5, operation: 1 },
        { amount: 1, operation: 2 },
      ],
    }),
    12,
  );
});

test("no readable attack speed is undefined, never a guessed number", () => {
  assert.equal(attackSpeedFrom(undefined), undefined);
  assert.equal(attackSpeedFrom({}), undefined);
  assert.equal(
    attackSpeedFrom({ [ATTACK_SPEED_KEY]: { value: 4, modifiers: [{ amount: -4, operation: 0 }] } }),
    undefined,
  );
  assert.equal(attackSpeedFrom({ [ATTACK_SPEED_KEY]: { value: 4, modifiers: [] } }), 4);
});

test("a full-charge swing waits out the whole cooldown the weapon has", () => {
  // Iron sword, 1.6/s: a 12.5-tick period, full at 12 ticks, +1 tick in flight.
  assert.equal(fullChargeMs(1.6), 650);
  // …and as the server actually sends it: base 4 with a -2.4000000953674316 float.
  assert.equal(fullChargeMs(4 - 2.4000000953674316), 650);
  // Every sword swing is now slower than the old flat 400 ms cadence, which was a
  // 57% swing with a sword and every second one inside the target's hurt immunity.
  assert.ok(fullChargeMs(1.6) > 400);
  // Axe (0.9/s) and bare hand (4/s).
  assert.equal(fullChargeMs(0.9), 1150);
  assert.equal(fullChargeMs(4), 300);
});

test("a healing draught is recognised by its potion contents, not its name", () => {
  const vigil = [
    { type: "custom_name", data: { value: "Vigil Draught" } },
    { type: "potion_contents", data: { potionId: 25, customEffects: [] } },
  ];
  assert.equal(drinkHeal("potion", vigil), 8);
  assert.equal(drinkHeal("potion", [{ type: "potion_contents", data: { potionId: 24 } }]), 4);
  // Harming (26) in the same bottle is never drunk.
  assert.equal(drinkHeal("potion", [{ type: "potion_contents", data: { potionId: 26 } }]), undefined);
  // A thrown bottle is a different act.
  assert.equal(drinkHeal("splash_potion", vigil), undefined);
  assert.equal(drinkHeal("potion", undefined), undefined);
  assert.equal(drinkHeal("bread", []), undefined);
  assert.deepEqual([...HEALING_POTION_HEAL.keys()].sort(), [24, 25]);
});

test("a draught is drunk when its whole heal fits, the strongest that fits", () => {
  const clear = { maxHealth: 20, nearestMeleeDistance: undefined } as const;
  assert.deepEqual(drinkDecision({ ...clear, health: 12, heals: [8] }), { kind: "drink", heal: 8 });
  assert.deepEqual(drinkDecision({ ...clear, health: 12.5, heals: [8] }), { kind: "healthy" });
  assert.deepEqual(drinkDecision({ ...clear, health: 14, heals: [4, 8] }), { kind: "drink", heal: 4 });
  assert.deepEqual(drinkDecision({ ...clear, health: 3, heals: [4, 8] }), { kind: "drink", heal: 8 });
  assert.deepEqual(drinkDecision({ ...clear, health: 3, heals: [] }), { kind: "none-carried" });
  assert.deepEqual(drinkDecision({ ...clear, health: 20, heals: [] }), { kind: "healthy" });
});

test("a draught waits for a melee attacker to be out of its reach", () => {
  const hurt = { maxHealth: 20, health: 9, heals: [8] } as const;
  assert.deepEqual(
    drinkDecision({ ...hurt, nearestMeleeDistance: DRINK_CLEAR_RANGE - 0.1 }),
    { kind: "pressed" },
  );
  assert.deepEqual(drinkDecision({ ...hurt, nearestMeleeDistance: DRINK_CLEAR_RANGE }), {
    kind: "drink",
    heal: 8,
  });
});

test("reach is the eye to the hitbox, three blocks", () => {
  // A vindicator (0.6 × 1.95) three blocks away on the level: the eye is 1.62 up,
  // inside the box's height, so the reach is the horizontal gap to its face.
  assert.equal(inReach([0, 64, 0], [3.2, 64, 0], 0.6, 1.95), true);
  assert.equal(inReach([0, 64, 0], [PLAYER_REACH + 0.31, 64, 0], 0.6, 1.95), false);
  // Standing on a ledge two blocks above a zombie: the reach bends down to its head.
  assert.equal(inReach([0, 66, 0], [2, 64, 0], 0.6, 1.95), true);
});

test("the footwork backs away from a melee attacker while the swing charges", () => {
  const base = { ranged: false, charged: false, inReach: true, canStepBack: true, canStepIn: true };
  assert.equal(footwork({ ...base, horizontalDistance: KITE_DISTANCE - 0.1 }), "back");
  // A ledge behind: stand and take it on the shield.
  assert.equal(
    footwork({ ...base, horizontalDistance: KITE_DISTANCE - 0.1, canStepBack: false }),
    "hold",
  );
  // Charged: the swing is the answer, not the retreat.
  assert.equal(footwork({ ...base, horizontalDistance: 1, charged: true }), "hold");
  // An archer is never backed away from.
  assert.equal(footwork({ ...base, ranged: true, horizontalDistance: 1 }), "hold");
});

test("the footwork steps in on a body out of reach", () => {
  const base = { horizontalDistance: 4, inReach: false, canStepBack: true, canStepIn: true };
  // An archer backs off, so it is chased at once.
  assert.equal(footwork({ ...base, ranged: true, charged: false }), "forward");
  // A melee attacker is coming: wait for it until the swing is ready, then meet it.
  assert.equal(footwork({ ...base, ranged: false, charged: false }), "hold");
  assert.equal(footwork({ ...base, ranged: false, charged: true }), "forward");
  assert.equal(footwork({ ...base, ranged: false, charged: true, canStepIn: false }), "hold");
});

test("the shield is up only while standing and charging", () => {
  assert.equal(guardUp({ shieldInOffhand: true, footwork: "hold", charged: false }), true);
  assert.equal(guardUp({ shieldInOffhand: true, footwork: "back", charged: false }), false);
  assert.equal(guardUp({ shieldInOffhand: true, footwork: "hold", charged: true }), false);
  assert.equal(guardUp({ shieldInOffhand: false, footwork: "hold", charged: false }), false);
});

test("the crit jump is taken in reach, from footing with headroom, just before the charge", () => {
  const base = { msUntilCharged: 300, inReach: true, onGround: true, headroom: true };
  assert.equal(jumpForCrit(base), true);
  assert.equal(jumpForCrit({ ...base, msUntilCharged: 600 }), false);
  assert.equal(jumpForCrit({ ...base, inReach: false }), false);
  assert.equal(jumpForCrit({ ...base, onGround: false }), false);
  assert.equal(jumpForCrit({ ...base, headroom: false }), false);
});

test("a bow or crossbow in the main hand is a ranged fighter", () => {
  assert.equal(holdsRangedWeapon("bow"), true);
  assert.equal(holdsRangedWeapon("crossbow"), true);
  assert.equal(holdsRangedWeapon("iron_axe"), false);
  assert.equal(holdsRangedWeapon(undefined), false);
});

test("the tally line names every count", () => {
  const t = emptyTally();
  t.swings = 15;
  t.landed = 13;
  t.noDamage = 2;
  t.crits = 9;
  t.guards = 12;
  t.shieldDisabled = 1;
  t.draughts = 3;
  assert.equal(
    describeTally(t),
    "15 charged swing(s) (13 hurt the target, 2 did nothing), 9 critical, shield raised 12×, " +
      "shield disabled 1×, " +
      "3 draught(s) drunk",
  );
});

test("the server's attack sound is its verdict on the swing, by registry id", () => {
  assert.equal(swingVerdict(1244), "landed"); // entity.player.attack.strong
  assert.equal(swingVerdict(1241), "landed"); // .crit
  assert.equal(swingVerdict(1245), "landed"); // .sweep
  assert.equal(swingVerdict(1243), "nodamage");
  assert.equal(swingVerdict(1247), undefined);
  assert.equal(swingVerdict(1394), undefined); // item.shield.block
});

test("minecraft-data files 1.21.11 sounds one id after the server's registry", () => {
  // The pinned server's registry report puts entity.player.attack.crit at 1241 and
  // .weak at 1246; minecraft-data has each one id later. mineflayer's
  // `soundEffectHeard` looks the (already un-offset) packet id up in this table, so
  // every name it reports is the previous sound's. If this starts failing, the data
  // was fixed: re-derive ATTACK_SOUND_VERDICT rather than trusting either side.
  const sounds = require("minecraft-data/minecraft-data/data/pc/1.21.11/sounds.json") as Array<{
    id: number;
    name: string;
  }>;
  const byId = new Map(sounds.map((s) => [s.id, s.name]));
  assert.equal(byId.get(1241 + 1), "entity.player.attack.crit");
  assert.equal(byId.get(1244 + 1), "entity.player.attack.strong");
  assert.equal(byId.get(1246 + 1), "entity.player.attack.weak");
});

test("at a third of max health a draught is drunk with the attacker on the bot", () => {
  const near = { maxHealth: 20, heals: [8], nearestMeleeDistance: 1 } as const;
  assert.deepEqual(drinkDecision({ ...near, health: 20 * DRINK_CRITICAL_FRACTION }), {
    kind: "drink",
    heal: 8,
  });
  // The vesperhold grooms: 5.6/20, a spear beside the bot, four draughts in the bag.
  assert.deepEqual(drinkDecision({ ...near, health: 5.6 }), { kind: "drink", heal: 8 });
  assert.deepEqual(drinkDecision({ ...near, health: 20 * DRINK_CRITICAL_FRACTION + 0.1 }), {
    kind: "pressed",
  });
});
