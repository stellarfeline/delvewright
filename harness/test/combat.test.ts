import { test } from "node:test";
import assert from "node:assert/strict";
import {
  ASSIST_AMPLIFIER,
  ASSIST_TICKS,
  AssistLedger,
  DEATH_PHASES,
  DIE_RETRY_DEATHS,
  assistClearCommand,
  assistCommand,
  assistPolicy,
  deathPhases,
  dieRetryFindings,
  floorFinding,
  parseCombatPlan,
  respawnedAtCheckpoint,
  scriptedDeathCommand,
  trialVerdict,
  type DeathTrial,
  type Encounter,
} from "../src/combat.ts";

function encounter(over: Partial<Encounter> = {}): Encounter {
  return {
    wave: "wave/bellkeeper",
    objective: "obj/the-keeper",
    step: 16,
    tier: "ordinary",
    pos: [101, 93, -99],
    count: 1,
    respawnsOnRest: false,
    checkpoint: [97, 71, -96],
    ...over,
  };
}

const PLAN = {
  version: "0.6.0",
  campaign_id: "the-drowned-bell",
  difficulty: "easy",
  encounters: [
    {
      wave: "wave/gate-assault",
      objective: "obj/hold-the-gate",
      step: 11,
      tier: "ordinary",
      pos: [12, 71, -85],
      count: 2,
      respawns_on_rest: true,
      checkpoint: [34, 71, -113],
    },
  ],
};

// --- the plan ---------------------------------------------------------------

test("a combat plan parses into encounters with their governing checkpoint", () => {
  const plan = parseCombatPlan(PLAN);
  assert.equal(plan.campaignId, "the-drowned-bell");
  assert.equal(plan.difficulty, "easy");
  assert.equal(plan.encounters.length, 1);
  const enc = plan.encounters[0]!;
  assert.equal(enc.wave, "wave/gate-assault");
  assert.equal(enc.tier, "ordinary");
  assert.equal(enc.respawnsOnRest, true);
  assert.deepEqual(enc.checkpoint, [34, 71, -113]);
});

test("an encounter with no checkpoint yet parses with an absent one", () => {
  const { checkpoint: _dropped, ...noCheckpoint } = PLAN.encounters[0]!;
  const raw = { ...PLAN, encounters: [noCheckpoint] };
  assert.equal(parseCombatPlan(raw).encounters[0]!.checkpoint, undefined);
});

test("an unknown tier is a parse failure, never a silent 'ordinary'", () => {
  // Silently downgrading would exempt the encounter from the floor gate — the one
  // place bot combat still has teeth.
  const raw = structuredClone(PLAN) as typeof PLAN & { encounters: Record<string, unknown>[] };
  raw.encounters[0]!["tier"] = "nightmare";
  assert.throws(() => parseCombatPlan(raw), /tier/);
});

// --- combat assist (spec-0023 §3) -------------------------------------------

test("the assist is Resistance III, not immunity", () => {
  // Amplifier 4 would make the bot invulnerable, and an invulnerable bot cannot
  // tell a wave that can hurt it from one that cannot.
  assert.equal(ASSIST_AMPLIFIER, 2);
  assert.match(assistCommand(), /^\/effect give @s minecraft:resistance 60 2 true$/);
  assert.equal(assistClearCommand(), "/effect clear @s minecraft:resistance");
});

test("an ordinary encounter is assisted from the start; a billed one is not", () => {
  assert.equal(assistPolicy(encounter()), "assisted");
  assert.equal(assistPolicy(encounter({ tier: "elite" })), "unassisted-first");
  assert.equal(assistPolicy(encounter({ tier: "boss" })), "unassisted-first");
});

test("every assist window is recorded with its encounter id and ticks", () => {
  // spec-0023 §3 acceptance: "the run artifact names every assist window
  // (encounter id, ticks)".
  const ledger = new AssistLedger();
  const w = ledger.open(encounter(), "policy: ordinary encounter", 1_000);
  ledger.close(w, 4_000);
  const [only] = ledger.windows();
  assert.equal(only!.encounter, "obj/the-keeper");
  assert.equal(only!.wave, "wave/bellkeeper");
  assert.equal(only!.ticks, ASSIST_TICKS);
  assert.equal(only!.closedAtMs, 4_000);
  assert.equal(ledger.leaked().length, 0);
});

test("an assist window the harness never closed is reported, not swallowed", () => {
  const ledger = new AssistLedger();
  ledger.open(encounter(), "policy", 0);
  assert.equal(ledger.leaked().length, 1);
});

// --- the inverted floor gate ------------------------------------------------

test("an elite the unassisted bot beats first try is a floor finding", () => {
  const finding = floorFinding(encounter({ tier: "elite" }), { attempted: true, won: true });
  assert.ok(finding);
  assert.match(finding, /billed `elite`/);
  assert.match(finding, /Advisory/);
});

test("an elite the unassisted bot LOSES to says nothing", () => {
  assert.equal(
    floorFinding(encounter({ tier: "boss" }), { attempted: true, won: false }),
    undefined,
  );
});

test("an ordinary encounter carries no floor expectation however easily it falls", () => {
  assert.equal(floorFinding(encounter(), { attempted: true, won: true }), undefined);
});

// --- die-retry (spec-0023 §1) -----------------------------------------------

test("the default is two scripted deaths, one per phase", () => {
  assert.equal(DIE_RETRY_DEATHS, 2);
  assert.deepEqual(deathPhases(), ["first-contact", "mid-fight"]);
  assert.deepEqual(deathPhases(3), ["first-contact", "mid-fight", "first-contact"]);
  assert.equal(DEATH_PHASES.length, 2);
});

test("the scripted death runs the damage path, not /kill", () => {
  // `/kill` bypasses damage handling entirely, so the loop it opens is one no
  // player could ever take — it would prove the wrong thing.
  assert.equal(scriptedDeathCommand(), "/damage @s 1000 minecraft:generic");
});

test("a respawn within the radius counts; one across the map does not", () => {
  assert.equal(respawnedAtCheckpoint([97, 71, -96], [97, 71, -96]), true);
  assert.equal(respawnedAtCheckpoint([100, 71, -99], [97, 71, -96]), true);
  assert.equal(respawnedAtCheckpoint([12, 71, -85], [97, 71, -96]), false);
});

test("with no checkpoint declared yet there is nothing to contradict", () => {
  assert.equal(respawnedAtCheckpoint([0, 0, 0], undefined), true);
});

function trial(over: Partial<DeathTrial> = {}): DeathTrial {
  return {
    encounter: "obj/the-keeper",
    wave: "wave/bellkeeper",
    attempt: 1,
    phase: "first-contact",
    respawnPos: [97, 71, -96],
    atCheckpoint: true,
    returned: true,
    reEngaged: true,
    objectivesIntact: true,
    lostObjectives: [],
    ...over,
  };
}

test("a clean trial is silent", () => {
  assert.equal(trialVerdict(trial()), undefined);
  assert.deepEqual(dieRetryFindings([trial(), trial({ attempt: 2 })]), []);
});

test("a respawn away from the checkpoint fails the trial", () => {
  const v = trialVerdict(trial({ atCheckpoint: false, respawnPos: [0, 64, 0] }));
  assert.match(String(v), /not the checkpoint/);
});

test("an unwalkable route back fails the trial", () => {
  assert.match(String(trialVerdict(trial({ returned: false }))), /not walkable/);
});

test("losing a completed objective to a death is state corruption, not difficulty", () => {
  const v = trialVerdict(
    trial({ objectivesIntact: false, lostObjectives: ["obj/hold-the-gate"] }),
  );
  assert.match(String(v), /LOST completed progress/);
  assert.match(String(v), /obj\/hold-the-gate/);
});

test("an encounter that does not re-engage is a one-shot fight", () => {
  assert.match(String(trialVerdict(trial({ reEngaged: false }))), /only be attempted once/);
});

test("the corruption check outranks the re-engage check", () => {
  // Both broken at once: report the one that means the delve ate progress, which
  // is the more serious and the more confusing to debug from the other's message.
  const v = trialVerdict(
    trial({ reEngaged: false, objectivesIntact: false, lostObjectives: ["obj/x"] }),
  );
  assert.match(String(v), /LOST completed progress/);
});
