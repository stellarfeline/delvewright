// The death loop's pure half: the contract parser, the forfeit rule
// re-derived from spec-0032's text, the seat/row lookups, and the verdicts.
//
// Every assertion here is about a PROMISE the campaign made. Nothing consults the
// emitter, which is the point: an assertion written by reading the emitter cannot
// fail when the emitter is wrong, and the live run that motivated this module
// found `on_death` firing nothing at all on a player's first death.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DeathPlanParseError,
  SUPPORTED_DEATH_PLAN_FORMAT,
  boxCells,
  deathLoopBinding,
  deathLoopBindingFailures,
  entryCellOf,
  expectedForfeit,
  inBox,
  lethalTrialFailures,
  openLethalTrial,
  parseDeathPlan,
  seatAtRespawn,
  tableAnchor,
  type DeathPlan,
  type LethalTrial,
  type LethalVolume,
  type StakeRule,
} from "../src/death-loop.ts";

/** The economy fixture's plan, as `delvec` really emits it. */
function planDoc(): Record<string, unknown> {
  return {
    campaign_id: "economy",
    version: "0.2.0",
    format_version: SUPPORTED_DEATH_PLAN_FORMAT,
    lethal_volumes: [
      {
        id: "lethal/the-drop",
        region: { lo: [5, 65, 8], hi: [5, 65, 8] },
        message: "The stone floor gives way beneath you.",
        message_key: "lethal.the-drop.message",
        damage_type: "minecraft:fall",
      },
    ],
    on_death: { effects: 1, drops_stake: ["stake/embers"] },
    stakes: [
      {
        id: "stake/embers",
        currency: {
          state: "state/embers",
          objective: "dw.s_embers",
          initial: 5,
          scope: "player",
          name: "Embers",
          name_key: "state.embers.name",
        },
        forfeit: { kind: "all" },
        max_live: 1,
        on_full: "replace",
        collect_by: "owner",
        collected_message: "You take back what the drop took.",
        collected_message_key: "stake.embers.collected",
        marker_item: "minecraft:soul_lantern",
      },
    ],
    placement: {
      seats: [
        { cp: -1, label: "the campaign's entry spawn", cell: [5, 65, 2] },
        { cp: 0, label: "checkpoint anchor `anchor/keeper-stand`", cell: [5, 65, 4] },
      ],
      regions: [
        {
          label: "lethal volume `lethal/the-drop`",
          lethal: true,
          volume: "lethal/the-drop",
          region: { lo: [5, 65, 8], hi: [5, 65, 8] },
        },
        {
          label: "the runtime-mutable ground of gate anchor `anchor/door`",
          lethal: false,
          volume: null,
          region: { lo: [4, 65, 6], hi: [5, 67, 6] },
        },
      ],
      rows: [
        { seat: 0, region: 0, anchor: [4, 65, 8] },
        { seat: 0, region: 1, anchor: [4, 65, 5] },
        { seat: 1, region: 0, anchor: [4, 65, 8] },
        { seat: 1, region: 1, anchor: [4, 65, 5] },
      ],
    },
    binding: {
      lethal_volumes: 1,
      on_death_effects: 1,
      stakes: 1,
      respawn_seats: 2,
      placement_rows: 4,
      unbound: false,
      reason: null,
    },
  };
}

function plan(): DeathPlan {
  return parseDeathPlan(planDoc());
}

const VOLUME: LethalVolume = {
  id: "lethal/the-drop",
  region: { lo: [5, 65, 8], hi: [5, 65, 8] },
  message: "The stone floor gives way beneath you.",
  messageKey: "lethal.the-drop.message",
  damageType: "minecraft:fall",
};

function stakeRule(over: Partial<StakeRule> = {}): StakeRule {
  return {
    id: "stake/embers",
    currency: {
      state: "state/embers",
      objective: "dw.s_embers",
      initial: 5,
      scope: "player",
      name: "Embers",
      nameKey: "state.embers.name",
    },
    forfeit: { kind: "all" },
    maxLive: 1,
    onFull: "replace",
    collectBy: "owner",
    collectedMessage: "You take back what the drop took.",
    markerItem: "minecraft:soul_lantern",
    ...over,
  };
}

/** A trial in which everything the campaign promised actually happened. */
function goodTrial(): LethalTrial {
  const t = openLethalTrial(VOLUME, [5, 65, 8], stakeRule());
  t.died = true;
  t.deathPos = [5, 65, 8];
  t.wordingSeen = true;
  t.balanceBefore = 5;
  t.expectedForfeit = 5;
  t.balanceAfterDeath = 0;
  t.respawnPos = [5.5, 65, 4.5];
  t.respawnSeat = "checkpoint anchor `anchor/keeper-stand`";
  t.expectedAnchor = [4, 65, 8];
  t.markerPos = [4.5, 65, 8.5];
  t.walkedBack = true;
  t.collectClicks = 2;
  t.balanceAfterCollect = 5;
  t.markerRetired = true;
  return t;
}

// --- the contract ----------------------------------------------------------

test("the emitted plan parses, and every declaration survives the round trip", () => {
  const p = plan();
  assert.equal(p.campaignId, "economy");
  assert.equal(p.volumes.length, 1);
  assert.equal(p.volumes[0]!.message, "The stone floor gives way beneath you.");
  assert.deepEqual(p.dropsStake, ["stake/embers"]);
  assert.deepEqual(p.stakes[0]!.forfeit, { kind: "all" });
  assert.equal(p.stakes[0]!.currency.objective, "dw.s_embers");
  assert.equal(p.binding.unbound, false);
});

test("a plan from a newer contract is REFUSED, never half-read", () => {
  const doc = planDoc();
  doc["format_version"] = SUPPORTED_DEATH_PLAN_FORMAT + 1;
  assert.throws(() => parseDeathPlan(doc), (e: unknown) => {
    assert.ok(e instanceof DeathPlanParseError);
    assert.match(e.message, /never made/);
    return true;
  });
});

test("a forfeit rule this harness cannot compute is refused rather than skipped", () => {
  const doc = planDoc();
  (doc["stakes"] as Record<string, unknown>[])[0]!["forfeit"] = { kind: "half-on-tuesdays" };
  assert.throws(() => parseDeathPlan(doc), DeathPlanParseError);
});

test("a binding that disagrees with its own counts is refused", () => {
  const doc = planDoc();
  (doc["binding"] as Record<string, unknown>)["unbound"] = true;
  assert.throws(() => parseDeathPlan(doc), (e: unknown) => {
    assert.ok(e instanceof DeathPlanParseError);
    assert.match(e.message, /must be exactly/);
    return true;
  });
});

test("an unbound plan must say why — a zero binding is a finding, not a silence", () => {
  const doc = planDoc();
  doc["lethal_volumes"] = [];
  doc["binding"] = { ...(doc["binding"] as object), lethal_volumes: 0, unbound: true, reason: null };
  assert.throws(() => parseDeathPlan(doc), /must state why/);
});

// --- the forfeit rule, re-derived from spec-0032's text ---------------------

test("`all` takes the whole purse and `none` takes nothing", () => {
  assert.equal(expectedForfeit({ kind: "all" }, 7), 7);
  assert.equal(expectedForfeit({ kind: "none" }, 7), 0);
});

test("a proportion rounds TOWARD ZERO, as integer arithmetic (ADR-0006)", () => {
  assert.equal(expectedForfeit({ kind: "proportion", percent: 30 }, 7), 2, "2.1 → 2");
  assert.equal(expectedForfeit({ kind: "proportion", percent: 99 }, 1), 0, "0.99 → 0");
  assert.equal(expectedForfeit({ kind: "proportion", percent: 100 }, 7), 7);
});

test("a fixed forfeit is CAPPED at the balance, so a purse can never go negative", () => {
  assert.equal(expectedForfeit({ kind: "fixed", amount: 3 }, 7), 3);
  assert.equal(expectedForfeit({ kind: "fixed", amount: 30 }, 7), 7);
});

test("a negative balance forfeits nothing — a death must never HAND a player money", () => {
  for (const rule of [
    { kind: "all" },
    { kind: "proportion", percent: 50 },
    { kind: "fixed", amount: 4 },
  ] as const) {
    assert.equal(expectedForfeit(rule, -5), 0, JSON.stringify(rule));
  }
});

// --- geometry and the table ------------------------------------------------

test("box membership and enumeration agree", () => {
  const box = { lo: [0, 0, 0] as const, hi: [1, 0, 1] as const };
  assert.equal(boxCells(box).length, 4);
  assert.ok(inBox([1, 0, 1], box));
  assert.ok(!inBox([2, 0, 1], box));
});

test("the entry cell is the nearest cell of the box, ties broken lexicographically", () => {
  const box = { lo: [0, 0, 0] as const, hi: [2, 0, 0] as const };
  assert.deepEqual(entryCellOf(box, [5, 0, 0]), [2, 0, 0]);
  assert.deepEqual(entryCellOf(box, [1, 0, 5]), [1, 0, 0]);
  // Equidistant from [0,0,0] and [2,0,0] → the lexicographically first wins.
  assert.deepEqual(entryCellOf(box, [1, 0, 0]), [1, 0, 0]);
});

test("the respawn seat is identified from the OBSERVED position, not from engine state", () => {
  const p = plan();
  // Vanilla lands a respawning player at cell + (0.5, 0.1, 0.5).
  assert.equal(seatAtRespawn(p.seats, [5.5, 65.1, 4.5]), 1);
  assert.equal(seatAtRespawn(p.seats, [5.5, 65.1, 2.5]), 0);
  assert.equal(
    seatAtRespawn(p.seats, [40, 65, 40]),
    undefined,
    "a player who came back somewhere the campaign never declared matches NO seat — " +
      "which is itself the finding",
  );
});

test("the placement table answers per (seat, volume), and says nothing it was not asked", () => {
  const p = plan();
  assert.deepEqual(tableAnchor(p, 1, "lethal/the-drop"), [4, 65, 8]);
  assert.equal(tableAnchor(p, 1, "lethal/nowhere"), undefined);
});

// --- the verdicts ----------------------------------------------------------

test("a loop in which every promise was kept produces no failures", () => {
  assert.deepEqual(lethalTrialFailures(goodTrial()), []);
});

test("standing in a lethal volume and surviving is the first and loudest failure", () => {
  const t = goodTrial();
  t.died = false;
  const out = lethalTrialFailures(t);
  assert.equal(out.length, 1, "nothing downstream of the death edge is even reported");
  assert.match(out[0]!, /did NOT die/);
});

test("a death with the volume's own wording withheld is a failure", () => {
  const t = goodTrial();
  t.wordingSeen = false;
  assert.match(lethalTrialFailures(t).join("\n"), /never reached them/);
});

test("the forfeit is judged against the DECLARED rule, and names all three numbers", () => {
  const t = goodTrial();
  t.balanceAfterDeath = 5; // the engine took nothing — the live first-death defect
  const out = lethalTrialFailures(t).join("\n");
  assert.match(out, /was 5 before and 5 after/);
  assert.match(out, /should be 0/);
});

test("a respawn at no declared seat is a failure, and says why it matters", () => {
  const t = goodTrial();
  t.respawnSeat = undefined;
  t.respawnPos = [0.5, 65, 0.5];
  assert.match(lethalTrialFailures(t).join("\n"), /not at any respawn seat/);
});

test("a missing stake at the table's anchor is a failure", () => {
  const t = goodTrial();
  t.markerPos = undefined;
  assert.match(lethalTrialFailures(t).join("\n"), /no recovery stake stands at \[4, 65, 8\]/);
});

test("a stake standing somewhere other than the proven anchor is a failure", () => {
  const t = goodTrial();
  t.markerPos = [9.5, 65, 9.5];
  assert.match(lethalTrialFailures(t).join("\n"), /blocks from \[4, 65, 8\]/);
});

test("a double click that credits the purse twice is caught by the amount, not by a race", () => {
  const t = goodTrial();
  t.balanceAfterCollect = 10; // both clicks paid out
  const out = lethalTrialFailures(t).join("\n");
  assert.match(out, /not idempotent/);
});

test("a stake that short-changes the player is caught by the same clause", () => {
  const t = goodTrial();
  t.balanceAfterCollect = 3;
  assert.match(lethalTrialFailures(t).join("\n"), /short-changed/);
});

test("a collected stake whose hardware still stands is a failure", () => {
  const t = goodTrial();
  t.markerRetired = false;
  assert.match(lethalTrialFailures(t).join("\n"), /still standing/);
});

test("a trial that could not be exercised is a failure, never a quiet pass", () => {
  const t = openLethalTrial(VOLUME, [5, 65, 8], stakeRule());
  t.abandoned = "the near lip could not be reached";
  const out = lethalTrialFailures(t);
  assert.equal(out.length, 1);
  assert.match(out[0]!, /could not be exercised/);
});

// --- binding ---------------------------------------------------------------

test("a stage that entered no volume reports VACUOUS, not pass", () => {
  const p = plan();
  const b = deathLoopBinding(p, []);
  assert.equal(b.volumesEntered, 0);
  const out = deathLoopBindingFailures(b);
  assert.equal(out.length, 1);
  assert.match(out[0]!, /examined nothing/);
});

test("a stage that entered a volume and saw no death is unbound downstream", () => {
  const p = plan();
  const t = openLethalTrial(VOLUME, [5, 65, 8], stakeRule());
  const b = deathLoopBinding(p, [t]);
  assert.equal(b.deathsObserved, 0);
  assert.match(deathLoopBindingFailures(b).join("\n"), /ZERO\s+player deaths/);
});

test("the binding counts what was really examined", () => {
  const p = plan();
  const b = deathLoopBinding(p, [goodTrial()]);
  assert.deepEqual(b, {
    declaredVolumes: 1,
    volumesEntered: 1,
    deathsObserved: 1,
    stakesExamined: 1,
    seatsMatched: 1,
    walksBack: 1,
  });
  assert.deepEqual(deathLoopBindingFailures(b), []);
});
