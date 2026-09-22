import { test } from "node:test";
import assert from "node:assert/strict";
import {
  parseMusterBody,
  parseMusterSummary,
  verifyMuster,
  type MusterBody,
  type MusterSummary,
} from "../src/muster.ts";
import type { MusterPlan } from "../src/combat.ts";

/**
 * `wave/unremembered-guard` as vesperhold declares it: five iron-armoured
 * zombies, 34 health, `attack_damage` 6, `follow_range` 24, named.
 *
 * The identity facts are the compiler's, in the compiler's order — bit 0 is the
 * name, bits 1..3 the three equipment slots — so a body carrying all four reads
 * mask 15.
 */
const GUARD: MusterPlan = {
  probe: "vesperhold:wave_muster_unremembered_guard",
  strike: "vesperhold:wave_strike_unremembered_guard",
  chip: "vesperhold:wave_chip_unremembered_guard",
  scale: 1000,
  unread: -1,
  bodies: 5,
  checked: 9,
  types: [
    {
      entity: "minecraft:zombie",
      facts: [
        "name=#0",
        "equipment.mainhand=minecraft:iron_sword",
        "equipment.head=minecraft:iron_helmet",
        "equipment.chest=minecraft:iron_chestplate",
      ],
      droppedFacts: [],
      readsAttackDamage: true,
      readsFollowRange: true,
    },
  ],
  profiles: [
    {
      typeIndex: 0,
      count: 5,
      mask: 15,
      label: "5 × minecraft:zombie (stack 0)",
      maxHealth: 34,
      attackDamage: 6,
      followRange: 24,
      armorAtLeast: 8,
      armorToughnessAtLeast: 0,
    },
  ],
};

/** One body exactly as the declaration says it should arrive. */
function guardBody(over: Partial<MusterBody> = {}): MusterBody {
  return {
    campaignId: "vesperhold",
    wave: "wave/unremembered-guard",
    seq: 1,
    typeIndex: 0,
    mask: 15,
    maxHealth: 34_000,
    armor: 10_000,
    armorToughness: 0,
    movementSpeed: 230,
    attackDamage: 6_000,
    followRange: 24_000,
    attackDamageEffective: 11_000,
    ...over,
  };
}

function summary(over: Partial<MusterSummary> = {}): MusterSummary {
  return {
    campaignId: "vesperhold",
    wave: "wave/unremembered-guard",
    seq: 1,
    counted: 5,
    tagged: 5,
    ...over,
  };
}

// --- the wire ---------------------------------------------------------------

test("a muster line parses whole, or not at all", () => {
  const line = "[dw:musterbody vesperhold wave/unremembered-guard 3 0 15 34000 10000 0 230 6000 24000 11000]";
  const body = parseMusterBody(line);
  assert.ok(body);
  assert.equal(body.seq, 3);
  assert.equal(body.mask, 15);
  assert.equal(body.maxHealth, 34_000);
  assert.equal(body.attackDamageEffective, 11_000);
  // Prefixed, suffixed or short lines are not the channel.
  assert.equal(parseMusterBody(`<player> ${line}`), undefined);
  assert.equal(parseMusterBody(line.replace(" 11000]", "]")), undefined);
});

test("a muster summary carries both counts, so an undeclared kind cannot hide", () => {
  const s = parseMusterSummary("[dw:muster vesperhold wave/unremembered-guard 2 5 6]");
  assert.ok(s);
  assert.equal(s.counted, 5);
  assert.equal(s.tagged, 6);
});

// --- the comparison ---------------------------------------------------------

test("a wave that arrived as declared produces no finding", () => {
  const v = verifyMuster(GUARD, summary(), Array.from({ length: 5 }, () => guardBody()));
  assert.deepEqual(v.failures, []);
  assert.deepEqual(v.findings, []);
  assert.equal(v.matched, 5);
  assert.equal(v.checked, 9);
});

test("a declared health that never reached the body is named with both numbers", () => {
  // The perturbation this check exists for: the summon's `attributes` component
  // is dropped and the body arrives on vanilla zombie health.
  const bodies = [guardBody({ maxHealth: 20_000 }), ...Array.from({ length: 4 }, () => guardBody())];
  const v = verifyMuster(GUARD, summary(), bodies);
  assert.equal(v.matched, 4);
  assert.equal(v.failures.length, 1);
  assert.match(v.failures[0]!, /max_health` is declared 34 and the body's own attribute reads 20/);
});

test("a declared attack_damage that never reached the body is named", () => {
  const bodies = [guardBody({ attackDamage: 3_000 }), ...Array.from({ length: 4 }, () => guardBody())];
  const v = verifyMuster(GUARD, summary(), bodies);
  assert.match(v.failures[0]!, /attack_damage` is declared 6 and the body's own attribute reads 3/);
});

test("a missing piece of declared gear is named by slot and item", () => {
  // Bit 3 is the chestplate; the body arrives without it and its armour drops.
  const bodies = [
    guardBody({ mask: 0b0111, armor: 4_000 }),
    ...Array.from({ length: 4 }, () => guardBody()),
  ];
  const v = verifyMuster(GUARD, summary(), bodies);
  assert.equal(v.matched, 4);
  assert.match(v.failures[0]!, /equipment\.chest=minecraft:iron_chestplate/);
  assert.match(v.failures[0]!, /the gear did not reach it/);
});

test("a name that rendered as something else is a finding, not a silence", () => {
  const bodies = [guardBody({ mask: 0b1110 }), ...Array.from({ length: 4 }, () => guardBody())];
  const v = verifyMuster(GUARD, summary(), bodies);
  assert.match(v.failures[0]!, /name=#0/);
});

test("a body of an undeclared kind standing in the wave is a finding", () => {
  const v = verifyMuster(
    GUARD,
    summary({ counted: 5, tagged: 6 }),
    Array.from({ length: 5 }, () => guardBody()),
  );
  assert.match(v.failures[0]!, /1 body\/bodies of an undeclared entity kind/);
});

test("an empty anchor says the probe had nothing to read, not that five stacks are missing", () => {
  // A wave the world felled, or one a run-back has already cleared. Reporting it
  // as five absent declarations would be a defect claim about content that is
  // exactly as declared.
  const v = verifyMuster(GUARD, summary({ counted: 0, tagged: 0 }), []);
  assert.equal(v.findings.length, 1);
  assert.match(v.findings[0]!, /nothing of this wave was standing/);
  assert.match(v.findings[0]!, /9 declared fact\(s\) could be checked/);
});

test("the probe's own lines and its total must agree, or neither is evidence", () => {
  const v = verifyMuster(GUARD, summary({ counted: 5 }), [guardBody()]);
  assert.ok(v.failures.some((f) => /its total\s+disagree|and its total/.test(f)));
});

test("an attribute the probe did not read is a finding, never a silent pass", () => {
  // `unread` is the sentinel the compiler writes where it deliberately did not
  // ask. A declaration sitting over one is unverified, and says so.
  const bodies = [guardBody({ followRange: -1 }), ...Array.from({ length: 4 }, () => guardBody())];
  const v = verifyMuster(GUARD, summary(), bodies);
  assert.match(v.failures[0]!, /follow_range` is declared 24 but the probe did not read it/);
});

test("two stacks of one kind are matched as a multiset, not by position", () => {
  // `wave/drowned-choir`: three Choristers and one Precentor, identical but for
  // the name. Nothing on a live body says which stack it came from.
  const choir: MusterPlan = {
    ...GUARD,
    bodies: 4,
    checked: 8,
    types: [
      {
        entity: "minecraft:drowned",
        facts: ["name=#0", "name=#1"],
        droppedFacts: [],
        readsAttackDamage: false,
        readsFollowRange: false,
      },
    ],
    profiles: [
      {
        typeIndex: 0,
        count: 3,
        mask: 0b01,
        label: "3 × minecraft:drowned (stack 0)",
        maxHealth: 34,
        armorAtLeast: 0,
        armorToughnessAtLeast: 0,
      },
      {
        typeIndex: 0,
        count: 1,
        mask: 0b10,
        label: "1 × minecraft:drowned (stack 1)",
        maxHealth: 34,
        armorAtLeast: 0,
        armorToughnessAtLeast: 0,
      },
    ],
  };
  const body = (mask: number): MusterBody =>
    guardBody({ mask, armor: 0, attackDamage: -1, followRange: -1, attackDamageEffective: -1 });
  // Arrival order deliberately puts the Precentor first.
  const ok = verifyMuster(choir, summary({ counted: 4, tagged: 4 }), [
    body(0b10),
    body(0b01),
    body(0b01),
    body(0b01),
  ]);
  assert.deepEqual(ok.findings, []);
  // …and a cohort of four Choristers is short a Precentor, however it is ordered.
  const short = verifyMuster(choir, summary({ counted: 4, tagged: 4 }), [
    body(0b01),
    body(0b01),
    body(0b01),
    body(0b01),
  ]);
  assert.ok(short.failures.some((f) => /stack 1/.test(f)));
});
