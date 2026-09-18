import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import {
  ATTACK_SPEED_KEY,
  HEALING_POTION_HEAL,
  SHIELD_BLOCK_DELAY_MS,
  attackSpeedFrom,
  attributeValue,
  describeTally,
  drinkDecision,
  drinkHeal,
  emptyTally,
  fullChargeMs,
  planStrike,
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
  assert.deepEqual(drinkDecision({ health: 12, maxHealth: 20, heals: [8] }), {
    kind: "drink",
    heal: 8,
  });
  assert.deepEqual(drinkDecision({ health: 12.5, maxHealth: 20, heals: [8] }), {
    kind: "healthy",
  });
  assert.deepEqual(drinkDecision({ health: 14, maxHealth: 20, heals: [4, 8] }), {
    kind: "drink",
    heal: 4,
  });
  assert.deepEqual(drinkDecision({ health: 3, maxHealth: 20, heals: [4, 8] }), {
    kind: "drink",
    heal: 8,
  });
  assert.deepEqual(drinkDecision({ health: 3, maxHealth: 20, heals: [] }), {
    kind: "none-carried",
  });
  assert.deepEqual(drinkDecision({ health: 20, maxHealth: 20, heals: [] }), { kind: "healthy" });
});

test("the shield goes up only when there is time for it to block", () => {
  const base = { shieldInOffhand: true, onGround: true, headroom: true } as const;
  assert.equal(planStrike({ ...base, msUntilCharged: 600 }).guard, true);
  assert.equal(planStrike({ ...base, msUntilCharged: SHIELD_BLOCK_DELAY_MS }).guard, true);
  assert.equal(planStrike({ ...base, msUntilCharged: SHIELD_BLOCK_DELAY_MS - 1 }).guard, false);
  assert.equal(planStrike({ ...base, shieldInOffhand: false, msUntilCharged: 600 }).guard, false);
});

test("the crit jump is taken only from footing with headroom", () => {
  const base = { shieldInOffhand: true, msUntilCharged: 600 } as const;
  assert.equal(planStrike({ ...base, onGround: true, headroom: true }).jump, true);
  assert.equal(planStrike({ ...base, onGround: false, headroom: true }).jump, false);
  assert.equal(planStrike({ ...base, onGround: true, headroom: false }).jump, false);
});

test("the tally line names every count", () => {
  const t = emptyTally();
  t.swings = 15;
  t.crits = 9;
  t.blocked = 2;
  t.shieldDisabled = 1;
  t.draughts = 3;
  assert.equal(
    describeTally(t),
    "15 charged swing(s), 9 critical, 2 blow(s) taken on the shield, shield disabled 1×, " +
      "3 draught(s) drunk",
  );
});
