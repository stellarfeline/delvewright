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
  dieRetryCoverageFailures,
  dieRetryFindings,
  floorFinding,
  openTrial,
  parseCombatPlan,
  respawnedAtCheckpoint,
  retryOutcome,
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
    outcome: "re-engaged",
    cause: undefined,
    respawnPos: [97, 71, -96],
    atCheckpoint: true,
    returned: true,
    reEngaged: true,
    objectiveComplete: false,
    objectivesIntact: true,
    lostObjectives: [],
    completed: true,
    abortedWith: undefined,
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

// --- what was waiting at the end of the loop (planner ruling 2026-08-03) ------

test("a re-engaged encounter is the ordinary pass", () => {
  assert.equal(trialVerdict(trial({ outcome: "re-engaged", reEngaged: true })), undefined);
});

test("a fight already won before the death is a PASS, not a broken retry loop", () => {
  // The sacred property is that dying is safe for PROGRESSION, not that the fight
  // must still be standing. A player who dies to the last mob's parting hit has
  // won; so has the bot. Reading this as red made the verdict depend on whether
  // the bot's timed melee happened to finish the wave — the keep-trial fixture
  // went red then green on consecutive live runs.
  const v = trialVerdict(
    trial({ outcome: "cleared-before-retry", reEngaged: false, objectiveComplete: true }),
  );
  assert.equal(v, undefined);
});

test("nothing to fight AND an unfinished objective is a soft lock, loudly", () => {
  const v = trialVerdict(
    trial({ outcome: "stranded", reEngaged: false, objectiveComplete: false }),
  );
  assert.match(String(v), /STRANDED/);
  assert.match(String(v), /soft lock/);
  assert.match(String(v), /obj\/the-keeper/, "the unfinished objective is named");
});

test("a loop that never established either is unproven, never a pass", () => {
  assert.match(String(trialVerdict(trial({ outcome: "unproven" }))), /Nothing was proved/);
});

test("retryOutcome maps the two observations onto the three outcomes", () => {
  assert.equal(retryOutcome(true, false), "re-engaged");
  assert.equal(retryOutcome(true, true), "re-engaged", "a live mob outranks a done objective");
  assert.equal(retryOutcome(false, true), "cleared-before-retry");
  assert.equal(retryOutcome(false, false), "stranded");
});

test("the corruption check outranks the stranded check", () => {
  // Both broken at once: report the one that means the delve ate progress, which
  // is the more serious and the more confusing to debug from the other's message.
  const v = trialVerdict(
    trial({ outcome: "stranded", reEngaged: false, objectivesIntact: false, lostObjectives: ["obj/x"] }),
  );
  assert.match(String(v), /LOST completed progress/);
});

// --- report integrity: a death that happened is never silent (task #102) -----

test("an opened trial starts at its FAILING values, so an abandoned one reads red", () => {
  // The record exists from the moment the harness commits to dying. If the run
  // ends there, every verdict field must still say "not proved" — a half-filled
  // record that defaulted to `true` would be worse than no record at all.
  const t = openTrial(encounter(), 1, "first-contact");
  assert.equal(t.completed, false);
  assert.equal(t.atCheckpoint, false);
  assert.equal(t.returned, false);
  assert.equal(t.reEngaged, false);
  assert.match(String(trialVerdict(t)), /ABANDONED/);
});

test("an abandoned trial outranks every other verdict and names why", () => {
  const v = trialVerdict(trial({ completed: false, abortedWith: "the bot never respawned" }));
  assert.match(String(v), /ABANDONED/);
  assert.match(String(v), /the bot never respawned/);
  // …even though the rest of the record looks clean: nothing downstream of an
  // abandoned loop was actually observed.
});

test("a stage that engaged an encounter and proved nothing cannot read as passed", () => {
  // The-drowned-bell round 3: `die_retry: []` next to a log line naming the death
  // it had just taken, and the stage reported `passed: true` because an empty
  // trial list yields an empty finding list.
  const plan = parseCombatPlan(PLAN).encounters;
  const failures = dieRetryCoverageFailures(plan, new Set(["wave/gate-assault"]), []);
  assert.equal(failures.length, 1);
  assert.match(failures[0]!, /ENGAGED this encounter but proved only 0\/2/);
});

test("an encounter the run never reached is reported unproven, not passed", () => {
  const plan = parseCombatPlan(PLAN).encounters;
  const failures = dieRetryCoverageFailures(plan, new Set(), []);
  assert.equal(failures.length, 1);
  assert.match(failures[0]!, /never reached this encounter/);
});

test("an incomplete trial does not count toward coverage", () => {
  const plan = parseCombatPlan(PLAN).encounters;
  const trials = [
    trial({ wave: "wave/gate-assault", attempt: 1 }),
    trial({ wave: "wave/gate-assault", attempt: 2, completed: false, abortedWith: "boom" }),
  ];
  const failures = dieRetryCoverageFailures(plan, new Set(["wave/gate-assault"]), trials);
  assert.equal(failures.length, 1);
  assert.match(failures[0]!, /proved only 1\/2 scripted death\(s\) \(2 recorded\)/);
});

test("two completed trials per encounter is full coverage and says nothing", () => {
  const plan = parseCombatPlan(PLAN).encounters;
  const trials = [
    trial({ wave: "wave/gate-assault", attempt: 1 }),
    trial({ wave: "wave/gate-assault", attempt: 2 }),
  ];
  assert.deepEqual(dieRetryCoverageFailures(plan, new Set(["wave/gate-assault"]), trials), []);
});
