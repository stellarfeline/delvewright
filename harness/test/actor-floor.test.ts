// The floor gate on the OTHER shape an elite takes (#222 emission, #114 harness):
// parsing the plan's `actors[]` and `floor_gate` ledger, deciding which actor
// fights THIS run can honestly reach, and the advisory it produces.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CombatPlanParseError,
  actorExercise,
  actorFloorFinding,
  parseCombatPlan,
  type ActorEncounter,
  type ActorTrial,
} from "../src/combat.ts";

/** The reference shape from #222: an elite actor, unleashed by a strike-npc trigger. */
function planJson(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: "0.8.0",
    campaign_id: "souls-bonfire",
    difficulty: "normal",
    encounters: [],
    actors: [
      {
        actor: "actor/barrow-warden",
        entity: "minecraft:wither_skeleton",
        name: "Barrow Warden",
        tier: "elite",
        anchor: "anchor/wave",
        pos: [10, 65, -4],
        tag: "dw_actor_barrow_warden",
        vulnerable: false,
        attributes: { max_health: 60.0 },
        spawned_by: [
          {
            site: "trigger",
            owner: "trigger/warden-answers",
            path: "/content/triggers/0/effects/0",
            on: "strike-npc",
            npc: "npc/keeper",
          },
        ],
        unleashed_by: [
          {
            site: "trigger",
            owner: "trigger/warden-answers",
            path: "/content/triggers/0/effects/1",
            on: "strike-npc",
            npc: "npc/keeper",
          },
        ],
        floor_gate: { covered: true },
      },
    ],
    floor_gate: {
      covered: [{ kind: "actor", id: "actor/barrow-warden", tier: "elite" }],
      not_covered: [],
    },
    ...over,
  };
}

/** The same actor, unleashed by an objective the path completes. */
function objectiveUnleashed(): Record<string, unknown> {
  const json = planJson();
  const actor = (json["actors"] as Record<string, unknown>[])[0]!;
  actor["unleashed_by"] = [
    {
      site: "objective",
      owner: "quest/the-barrow",
      objective: "obj/open-the-door",
      path: "/content/quests/0/on_objective_complete/obj~1open-the-door/1",
    },
  ];
  return json;
}

function actorOf(json: Record<string, unknown>): ActorEncounter {
  return parseCombatPlan(json).actors[0]!;
}

test("the plan's tiered actors parse with the beats that stage and unleash them", () => {
  const plan = parseCombatPlan(planJson());
  assert.equal(plan.actors.length, 1);
  const a = plan.actors[0]!;
  assert.equal(a.actor, "actor/barrow-warden");
  assert.equal(a.entity, "minecraft:wither_skeleton");
  assert.equal(a.tier, "elite");
  assert.deepEqual(a.pos, [10, 65, -4]);
  assert.equal(a.maxHealth, 60);
  assert.equal(a.vulnerable, false);
  assert.equal(a.floorGate.covered, true);
  assert.equal(a.unleashedBy[0]!.site, "trigger");
  assert.equal(a.unleashedBy[0]!.on, "strike-npc");
  assert.equal(a.unleashedBy[0]!.npc, "npc/keeper");
});

test("the floor-gate ledger parses both sides, each not-covered entry with its reason", () => {
  const json = planJson({
    floor_gate: {
      covered: [{ kind: "wave", id: "wave/bellkeeper", tier: "boss" }],
      not_covered: [
        {
          kind: "actor",
          id: "actor/barrow-warden",
          tier: "elite",
          reason: "no `spawn-actor` effect anywhere in the campaign summons it",
        },
      ],
    },
  });
  const ledger = parseCombatPlan(json).floorGate;
  assert.equal(ledger.present, true);
  assert.equal(ledger.covered[0]!.id, "wave/bellkeeper");
  assert.equal(ledger.notCovered[0]!.id, "actor/barrow-warden");
  assert.match(ledger.notCovered[0]!.reason!, /no `spawn-actor`/);
});

test("a not-covered entry with no reason is rejected — that silence is the bug", () => {
  const json = planJson({
    floor_gate: {
      covered: [],
      not_covered: [{ kind: "actor", id: "actor/x", tier: "boss" }],
    },
  });
  assert.throws(
    () => parseCombatPlan(json),
    (err: unknown) =>
      err instanceof CombatPlanParseError && err.pointer === "/floor_gate/not_covered/0/reason",
  );
});

test("a plan from before the ledger existed reads as ABSENT, never as empty", () => {
  // The two claims are different: "this campaign bills nothing hard" is
  // reassuring, "this build cannot tell you" is not, and a reader must be able to
  // tell them apart.
  const json = planJson();
  delete json["floor_gate"];
  delete json["actors"];
  const plan = parseCombatPlan(json);
  assert.equal(plan.floorGate.present, false);
  assert.deepEqual([...plan.actors], []);
});

test("an actor unleashed by an on-path objective is the one shape the run can fight", () => {
  const a = actorOf(objectiveUnleashed());
  const decision = actorExercise(a, new Set(["obj/open-the-door", "obj/exit"]));
  assert.deepEqual(decision, { kind: "exercise", afterObjective: "obj/open-the-door" });
});

test("an ambient trigger is skipped, naming the beat — the bot may not invent a moment", () => {
  // The reference shape: the warden stands up when a player strikes the keeper.
  // The campaign does not schedule that, so neither may the bot; fabricating the
  // moment would fabricate the fight the telemetry then reported on.
  const a = actorOf(planJson());
  const decision = actorExercise(a, new Set(["obj/open-the-door"]));
  assert.equal(decision.kind, "skip");
  assert.match(decision.kind === "skip" ? decision.reason : "", /strike-npc/);
  assert.match(decision.kind === "skip" ? decision.reason : "", /trigger\/warden-answers/);
  assert.match(decision.kind === "skip" ? decision.reason : "", /no position in the quest DAG/);
});

test("an off-path objective is skipped, naming the objective the path never completes", () => {
  const a = actorOf(objectiveUnleashed());
  const decision = actorExercise(a, new Set(["obj/somewhere-else"]));
  assert.equal(decision.kind, "skip");
  assert.match(decision.kind === "skip" ? decision.reason : "", /obj\/open-the-door/);
  assert.match(decision.kind === "skip" ? decision.reason : "", /never completes/);
});

test("a quest-site unleash is skipped, because the path names objectives, not quests", () => {
  const json = objectiveUnleashed();
  const actor = (json["actors"] as Record<string, unknown>[])[0]!;
  actor["unleashed_by"] = [
    { site: "quest", owner: "quest/the-barrow", path: "/content/quests/0/on_complete/0" },
  ];
  const decision = actorExercise(actorOf(json), new Set(["obj/open-the-door"]));
  assert.equal(decision.kind, "skip");
  assert.match(decision.kind === "skip" ? decision.reason : "", /quest\/the-barrow/);
});

test("the compiler's own not-covered reason is carried through, never reworded", () => {
  const json = objectiveUnleashed();
  const actor = (json["actors"] as Record<string, unknown>[])[0]!;
  actor["floor_gate"] = { covered: false, reason: "it is staged but never unleashed" };
  const decision = actorExercise(actorOf(json), new Set(["obj/open-the-door"]));
  assert.equal(decision.kind, "skip");
  assert.match(decision.kind === "skip" ? decision.reason : "", /staged but never unleashed/);
});

test("an ordinary actor is skipped: the gate measures only what the content bills hard", () => {
  const json = objectiveUnleashed();
  (json["actors"] as Record<string, unknown>[])[0]!["tier"] = "ordinary";
  const decision = actorExercise(actorOf(json), new Set(["obj/open-the-door"]));
  assert.equal(decision.kind, "skip");
  assert.match(decision.kind === "skip" ? decision.reason : "", /ordinary/);
});

test("an actor with no resolved cell is skipped — there is nowhere to walk", () => {
  const json = objectiveUnleashed();
  delete (json["actors"] as Record<string, unknown>[])[0]!["pos"];
  const decision = actorExercise(actorOf(json), new Set(["obj/open-the-door"]));
  assert.equal(decision.kind, "skip");
  assert.match(decision.kind === "skip" ? decision.reason : "", /nowhere to walk/);
});

const TRIAL: ActorTrial = {
  actor: "actor/barrow-warden",
  tier: "elite",
  afterObjective: "obj/open-the-door",
  outcome: "won-first-try",
  swings: 11,
  elapsedMs: 8_400,
};

test("beating a billed actor cold is the advisory — the inverted gate, on actors", () => {
  const finding = actorFloorFinding(TRIAL);
  assert.match(finding!, /billed `elite`/);
  assert.match(finding!, /UNASSISTED bot beat it on its first attempt/);
  assert.match(finding!, /11 swing/);
});

test("losing, timing out, or never finding the body says nothing — and never a pass", () => {
  // A bot that loses is the DESIGN (spec-0023 downgraded bot melee competence to
  // telemetry). What must not happen is any of these reading as a measured win.
  for (const outcome of ["lost", "timed-out", "body-not-found"] as const) {
    assert.equal(actorFloorFinding({ ...TRIAL, outcome }), undefined, outcome);
  }
  assert.equal(actorFloorFinding({ ...TRIAL, tier: "ordinary" }), undefined);
});
