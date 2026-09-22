import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DEATH_PHASES,
  DIE_RETRY_DEATHS,
  deathPhases,
  dieRetryBinding,
  dieRetryCoverageFailures,
  dieRetryFindings,
  scriptedDeathRefusal,
  openTrial,
  checkpointPrecondition,
  observationOf,
  CombatPlanParseError,
  parseCombatPlan,
  reseatFidelityFinding,
  returnAttritionFinding,
  returnHealthNote,
  dieRetryFidelityGaps,
  respawnedAtCheckpoint,
  retryOutcome,
  scriptedDeathCommand,
  trialVerdict,
  waveAttribution,
  type DeathTrial,
  type Encounter,
  dueRunBacks,
  respawnReseats,
  type RunBack,
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
    census: {
      census: "the-drowned-bell:wave_census_bellkeeper",
      brand: "the-drowned-bell:wave_brand_bellkeeper",
      unbrand: "the-drowned-bell:wave_unbrand_bellkeeper",
    },
    muster: {
      probe: "the-drowned-bell:wave_muster_bellkeeper",
      strike: "the-drowned-bell:wave_strike_bellkeeper",
      chip: "the-drowned-bell:wave_chip_bellkeeper",
      scale: 1000,
      unread: -1,
      bodies: 1,
      checked: 3,
      types: [
        {
          entity: "minecraft:drowned",
          facts: ["name=Bellkeeper"],
          droppedFacts: [],
          readsAttackDamage: false,
          readsFollowRange: false,
        },
      ],
      profiles: [
        {
          typeIndex: 0,
          count: 1,
          mask: 1,
          label: "1 × minecraft:drowned `Bellkeeper`",
          maxHealth: 30,
          armorAtLeast: 0,
          armorToughnessAtLeast: 0,
        },
      ],
    },
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
      census: {
        census: "the-drowned-bell:wave_census_gate_assault",
        brand: "the-drowned-bell:wave_brand_gate_assault",
        unbrand: "the-drowned-bell:wave_unbrand_gate_assault",
      },
      muster: {
        probe: "the-drowned-bell:wave_muster_gate_assault",
        strike: "the-drowned-bell:wave_strike_gate_assault",
        chip: "the-drowned-bell:wave_chip_gate_assault",
        scale: 1000,
        unread: -1,
        bodies: 2,
        checked: 4,
        types: [
          {
            entity: "minecraft:drowned",
            facts: ["name=Gate Drowned", "equipment.mainhand=minecraft:trident"],
            dropped_facts: [],
            reads_attack_damage: false,
            reads_follow_range: false,
          },
        ],
        profiles: [
          {
            type: 0,
            count: 2,
            mask: 3,
            label: "2 × minecraft:drowned `Gate Drowned`",
            max_health: 30,
            attack_damage: null,
            movement_speed: null,
            follow_range: null,
            armor_at_least: 0,
            armor_toughness_at_least: 0,
          },
        ],
      },
    },
  ],
  run_backs: [],
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

test("a plan that cannot name its census is refused, never silently silhouetted", () => {
  // The alternative is the failure mode of counting whatever the client tracks
  // and reporting ambush actors as wave mobs a re-seat left standing. A build too
  // old to state the probe cannot be measured by tag, and saying so beats guessing.
  const { census: _dropped, ...noCensus } = PLAN.encounters[0]!;
  assert.throws(() => parseCombatPlan({ ...PLAN, encounters: [noCensus] }), /census/);
});

test("an unknown tier is a parse failure, never a silent 'ordinary'", () => {
  // Silently downgrading would exempt the encounter from the floor gate — the one
  // place bot combat still has teeth.
  const raw = structuredClone(PLAN) as typeof PLAN & { encounters: Record<string, unknown>[] };
  raw.encounters[0]!["tier"] = "nightmare";
  assert.throws(() => parseCombatPlan(raw), /tier/);
});













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
    kitKept: true,
    returned: true,
    reEngaged: true,
    objectiveComplete: false,
    reseats: false,
    reseat: undefined,
    reengage: undefined,
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

test("a scripted death that did not land names the gamemode, never the op seed", () => {
  // Spectator: the command could not have worked, and the refusal says so plus the
  // server's own answer. The old sentence guessed — "was refused (is the bot
  // opped?)" — at the one cause it had never checked.
  const spec = scriptedDeathRefusal("/damage @s 1000 minecraft:generic", 15000, "spectator", [
    "This entity cannot be damaged",
  ]);
  assert.match(spec, /`spectator`/);
  assert.match(spec, /invulnerable/);
  assert.match(spec, /This entity cannot be damaged/);
  assert.doesNotMatch(spec, /opped/);

  // Adventure: the mode a death IS takeable in, so the reader is sent somewhere
  // else entirely — a different finding, and the two must not read alike.
  const adv = scriptedDeathRefusal("/damage @s 1000 minecraft:generic", 15000, "adventure", []);
  assert.match(adv, /the mode a death is takeable in/);
  assert.match(adv, /the server answered nothing/);
  assert.doesNotMatch(adv, /invulnerable/);

  // And a client that could not read its own gamemode says THAT rather than
  // asserting one.
  assert.match(
    scriptedDeathRefusal("/damage @s 1000 minecraft:generic", 15000, undefined, []),
    /a gamemode the client could not read/,
  );
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

// --- what was waiting at the end of the loop ----------------------------------

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

// --- report integrity: a death that happened is never silent -----

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

// --- re-seat fidelity, pure ---------------------------------------

const ANCHOR = [0, 0, 0] as const;

/** One census mob line, as the compiler would have printed it. */
function mob(over: Partial<{ distance: number; health: number; maxHealth: number }> = {}) {
  return {
    campaignId: "c",
    wave: "wave/x",
    seq: 1,
    pos: [over.distance ?? 1, 0, 0] as readonly [number, number, number],
    health: over.health ?? 20,
    maxHealth: over.maxHealth ?? 20,
  };
}

/** A settled census: the server's totals plus the mob lines that closed it. */
function census(
  mobs: ReturnType<typeof mob>[],
  over: Partial<{ present: number; branded: number; damaged: number; credited: number }> = {},
) {
  return {
    summary: {
      campaignId: "c",
      wave: "wave/x",
      seq: 1,
      present: over.present ?? mobs.length,
      branded: over.branded ?? 0,
      damaged: over.damaged ?? mobs.filter((m) => m.health < m.maxHealth).length,
      credited: over.credited ?? 0,
    },
    mobs,
  };
}

test("observationOf counts what came back, what carried over, and how far it strayed", () => {
  const obs = observationOf(
    census([mob({ distance: 60 }), mob({ health: 6 }), mob()], { branded: 1 }),
    3,
    [...ANCHOR],
    900,
  );
  assert.equal(obs.present, 3);
  assert.equal(obs.declared, 3);
  assert.equal(obs.carriedOver, 1);
  assert.equal(obs.damaged, 1);
  assert.equal(obs.healthReadable, 3);
  assert.equal(obs.nearest, 1);
  assert.equal(obs.farthest, 60);
  assert.equal(obs.settleMs, 900);
});

test("the census counts the WAVE — a bystander standing beside it cannot enter the tally", () => {
  // Two ambush husks and a neighbouring wave's mob are standing
  // where the bot is, and were standing there before it died. The server counted
  // by tag, so the observation is of the wave alone and nothing carried over.
  const obs = observationOf(census([mob(), mob()]), 2, [...ANCHOR], 40);
  assert.equal(obs.present, 2, "not 4 — the bystanders never had the wave tag");
  assert.equal(obs.carriedOver, 0);
  assert.equal(reseatFidelityFinding("wave/x", 1, "first-contact", obs), undefined);
});

test("a faithful re-seat is silent", () => {
  const obs = observationOf(census([mob(), mob(), mob()]), 3, [...ANCHOR], 40);
  assert.equal(reseatFidelityFinding("wave/x", 1, "first-contact", obs), undefined);
});

test("a survivor carried across a life outranks every other fidelity fault", () => {
  // Both wrong at once: report the carried-over mob, because that is the grind
  // the ruling forbids and it explains the missing health too.
  const obs = observationOf(census([mob({ health: 3 }), mob()], { branded: 1 }), 3, [...ANCHOR], 40);
  const v = reseatFidelityFinding("wave/x", 2, "mid-fight", obs);
  assert.match(String(v), /already/);
  assert.match(String(v), /never topped up around its survivors/);
});

test("a short re-seat names the observed and declared counts", () => {
  const obs = observationOf(census([mob(), mob()]), 3, [...ANCHOR], 6_000);
  assert.match(String(reseatFidelityFinding("wave/x", 1, "first-contact", obs)), /2 mob\(s\) standing, 3 declared/);
});

test("a whole but wounded re-seat is red on health alone", () => {
  const obs = observationOf(census([mob({ health: 11 }), mob()]), 2, [...ANCHOR], 40);
  assert.match(String(reseatFidelityFinding("wave/x", 1, "first-contact", obs)), /BELOW full/);
});

// The gallery's own die-retry death 1, reproduced: the return leg is assisted and
// the bot DEFENDS itself on it — `[defend] die-retry return wave/muster …:
// zombie#63 is down` — so the settle that follows counted 2 of a cohort of 3 that
// had come back whole, and the stage reported the bot's own kill as a short
// re-seat. `credited` is the server's count of this wave's deaths since its
// seating, which is exactly the correction, and is a thing the defect this check
// exists to catch cannot produce.
test("a body the party felled since the re-seat is not a body the re-seat withheld", () => {
  const obs = observationOf(census([mob(), mob()], { credited: 1 }), 3, [...ANCHOR], 6_000);
  assert.equal(obs.credited, 1);
  assert.equal(reseatFidelityFinding("wave/x", 1, "first-contact", obs), undefined);
});

test("a genuinely short re-seat still reds when nothing was credited", () => {
  const obs = observationOf(census([mob(), mob()]), 3, [...ANCHOR], 6_000);
  assert.match(String(reseatFidelityFinding("wave/x", 1, "first-contact", obs)), /came back SHORT/);
});

test("credit corrects the count, it does not cover a shortfall it cannot reach", () => {
  // One felled on the way back, and the cohort is STILL one short of three.
  const obs = observationOf(census([mob()], { credited: 1 }), 3, [...ANCHOR], 6_000);
  assert.match(String(reseatFidelityFinding("wave/x", 1, "first-contact", obs)), /came back SHORT/);
});

test("a wound read at the re-seat is red — nothing has touched the cohort yet", () => {
  const obs = observationOf(census([mob({ health: 11 }), mob()]), 2, [...ANCHOR], 40);
  assert.match(String(reseatFidelityFinding("wave/x", 1, "first-contact", obs)), /BELOW full/);
});

// Vesperhold's choir, reproduced: the re-seat summoned four Drowned Choristers
// whole, and two of them swam into the lethal well beside their seat ("Drowned
// Chorister drowned") while the bot walked back. The census at the encounter read
// 2 of 4 with nothing credited, and the stage said the RE-SEAT "came back SHORT".
test("bodies lost between the re-seat and the return are the encounter's, not the re-seat's", () => {
  const reseat = observationOf(census([mob(), mob(), mob(), mob()]), 4, [...ANCHOR], 150);
  const back = observationOf(census([mob(), mob()]), 4, [...ANCHOR], 6_000);
  assert.equal(reseatFidelityFinding("wave/x", 1, "first-contact", reseat), undefined);
  const v = String(returnAttritionFinding("wave/x", 1, "first-contact", reseat, back));
  assert.match(v, /re-seat brought back 4 of 4/);
  assert.match(v, /2 died to something that is not the party/);
  assert.doesNotMatch(v, /came back SHORT/);
  const verdict = String(
    trialVerdict(trial({ reseats: true, reseat, reengage: back, outcome: "re-engaged" })),
  );
  assert.match(verdict, /kills its own wave/);
});

test("a body the party felled on the way back is not attrition", () => {
  const reseat = observationOf(census([mob(), mob(), mob()]), 3, [...ANCHOR], 150);
  const back = observationOf(census([mob(), mob()], { credited: 1 }), 3, [...ANCHOR], 6_000);
  assert.equal(returnAttritionFinding("wave/x", 1, "first-contact", reseat, back), undefined);
});

// Reproduced from the live fight: vesperhold's skeletons shot one another and its spear
// zombies stabbed one another while the bot walked back, and the census read ~30 s
// after the re-seat found them hurt with no swing by the bot.
test("wounds found at the return are stated, never judged, once the re-seat was read whole", () => {
  const reseat = observationOf(census([mob(), mob(), mob()]), 3, [...ANCHOR], 150);
  const back = observationOf(census([mob({ health: 11 }), mob({ health: 14 }), mob()]), 3, [...ANCHOR], 250);
  const t = trial({ reseats: true, reseat, reengage: back, outcome: "re-engaged" });
  assert.equal(trialVerdict(t), undefined);
  const notes = dieRetryFidelityGaps([t]);
  assert.equal(notes.length, 1);
  assert.match(notes[0]!, /2 of 3 readable wave mob\(s\) stood below full health/);
  assert.equal(returnHealthNote("wave/x", 1, "first-contact", reseat), undefined);
});

test("a re-seating wave whose re-seat was never read is unproven, not passed", () => {
  const back = observationOf(census([mob(), mob()]), 2, [...ANCHOR], 250);
  assert.match(
    String(trialVerdict(trial({ reseats: true, reengage: back, outcome: "re-engaged" }))),
    /never read at the re-seat/,
  );
});

test("only a re-seating wave owes fidelity — a persisting wave is judged by outcome alone", () => {
  const wounded = observationOf(census([mob({ health: 4 })], { branded: 1 }), 2, [...ANCHOR], 40);
  assert.equal(
    trialVerdict(trial({ reseats: false, reengage: wounded, outcome: "re-engaged" })),
    undefined,
    "survivors ARE the design when the wave does not re-seat",
  );
  assert.match(
    String(
      trialVerdict(trial({ reseats: true, reseat: wounded, reengage: wounded, outcome: "re-engaged" })),
    ),
    /previous life/,
  );
});

// --- die-retry precondition: an armed checkpoint -----------------------------

const FIRE = { bonfire: 1, anchor: "anchor/beach-fire", pos: [97, 71, -96] as const, step: 4 };

test("a governing checkpoint on a bonfire the bot rested at is armed", () => {
  assert.equal(checkpointPrecondition(encounter(), [FIRE], new Set([1]), 16), undefined);
});

test("a bonfire the route walked past leaves the checkpoint unarmed", () => {
  // Bell round 3: a fire only ARMS on arrival; the respawn point moves when the
  // party RESTS. Every fire walked past, so both trials respawned at world spawn.
  const v = checkpointPrecondition(encounter(), [FIRE], new Set(), 16);
  assert.equal(v?.kind, "unarmed");
  // The RUN's own gap, so it reds the stage: every measurement after it would
  // describe the harness's skipped rest rather than the delve.
  assert.equal(v?.reds, true);
  assert.match(String(v?.finding), /no checkpoint armed/);
  assert.match(String(v?.finding), /passed bonfire 1 \(anchor\/beach-fire\) without resting/);
  assert.match(String(v?.finding), /No death was taken/);
});

test("a checkpoint that is no bonfire arms itself — nothing to contradict", () => {
  // An ordinary `set-checkpoint` fires with its beat. Only a bonfire needs a rest.
  assert.equal(checkpointPrecondition(encounter(), [], new Set(), 16), undefined);
});

test("an encounter with NO governing checkpoint is named, skipped, and not graded", () => {
  // This is the truthful reading of souls-bonfire's encounter: with
  // `fire_step < i`, a checkpoint armed by the encounter's OWN kill step is
  // correctly not its governing one, so the plan names none. A death there
  // respawns at world spawn — the retry loop is a full restart of the delve.
  //
  // That is a CONTENT fact about where the campaign puts its rest points, and in
  // a souls campaign a design smell; but the compiler's retry-cost and checkpoint
  // rules own that judgement, so the harness states it and declines to grade it.
  // What it must never do is take the death anyway (it would measure the delve
  // against world spawn) or say nothing (the loop went unproven).
  const v = checkpointPrecondition(encounter({ checkpoint: undefined }), [FIRE], new Set(), 16);
  assert.equal(v?.kind, "no-checkpoint");
  assert.equal(v?.reds, false, "advisory — the compiler owns this judgement, not the stage");
  assert.match(String(v?.finding), /no governing checkpoint/);
  assert.match(String(v?.finding), /die-retry cannot prove safe death here/);
  assert.match(String(v?.finding), /full restart/);
  assert.match(String(v?.finding), /No death was taken/);
  assert.match(String(v?.finding), /DW0379/);
});

test("a governing bonfire the path only rests at LATER is still unarmed now", () => {
  // The souls-bonfire fixture's real shape: the fire is armed by the very beat the
  // encounter completes, so the plan hands the encounter a checkpoint whose rest
  // step sits AFTER it. The respawn point has not moved when the death would be
  // scripted, whatever the path does afterwards.
  const v = checkpointPrecondition(encounter(), [{ ...FIRE, step: 40 }], new Set(), 16);
  assert.equal(v?.reds, true);
  assert.match(String(v?.finding), /does not rest at bonfire 1 \(anchor\/beach-fire\) until AFTER/);
  assert.match(String(v?.finding), /path step 40/);
});

// ---------------------------------------------------------------------------
// The die-retry stage's binding count (playtest-methodology.md rule 1)
// ---------------------------------------------------------------------------

test("a die-retry stage that scripted no death reports UNBOUND, whatever else it says", () => {
  // The exact state measured across every campaign and fixture in both repos on
  // 2026-08-11: encounters exist, every one is excluded for want of a governing
  // checkpoint, the coverage arithmetic runs over an emptied list and finds
  // nothing wrong, and the stage reports a pass having proven nothing.
  const b = dieRetryBinding(true, 1, new Set(), [], 1, 0);
  assert.equal(b.unbound, true);
  assert.equal(b.deathsScripted, 0);
  assert.match(b.reason ?? "", /fires NO\s+checkpoint before them/);
});

test("a build with no mandatory encounter says THAT, not something vaguer", () => {
  const b = dieRetryBinding(true, 0, new Set(), [], 0, 0);
  assert.equal(b.unbound, true);
  assert.match(b.reason ?? "", /declares NO mandatory encounter/);
});

test("a stage that did not run is distinguished from one that ran and found nothing", () => {
  const b = dieRetryBinding(false, 2, new Set(), [], 0, 0);
  assert.equal(b.unbound, true);
  assert.match(b.reason ?? "", /did not run/);
});

test("an unarmed governing checkpoint is the RUN's own gap, and is named as such", () => {
  const b = dieRetryBinding(true, 2, new Set(), [], 0, 2);
  assert.match(b.reason ?? "", /never armed — the run's own gap/);
});

test("a stage that took deaths is bound, and owes no reason", () => {
  const t1 = openTrial(encounter(), 1, "first-contact");
  t1.completed = true;
  const t2 = openTrial(encounter(), 2, "mid-fight");
  const b = dieRetryBinding(true, 1, new Set(["wave/bellkeeper"]), [t1, t2], 0, 0);
  assert.equal(b.unbound, false);
  assert.equal(b.reason, undefined);
  assert.equal(b.deathsScripted, 2);
  assert.equal(b.trialsCompleted, 1, "taken and completed are different counts");
  assert.equal(b.engaged, 1);
});

// --- the muster: what the wave DECLARES ---------------------------------------

test("a plan whose encounter states no muster is refused, never silently unread", () => {
  const raw = JSON.parse(JSON.stringify(PLAN)) as Record<string, unknown>;
  delete (raw["encounters"] as Record<string, unknown>[])[0]!["muster"];
  assert.throws(
    () => parseCombatPlan(raw),
    (err: unknown) => err instanceof CombatPlanParseError && /muster/.test(err.pointer),
  );
});

test("a muster profile pointing outside `types` is refused", () => {
  const raw = JSON.parse(JSON.stringify(PLAN)) as Record<string, unknown>;
  const m = (raw["encounters"] as Record<string, unknown>[])[0]!["muster"] as Record<
    string,
    unknown
  >;
  (m["profiles"] as Record<string, unknown>[])[0]!["type"] = 7;
  assert.throws(
    () => parseCombatPlan(raw),
    (err: unknown) => err instanceof CombatPlanParseError && /profiles\/0\/type/.test(err.pointer),
  );
});

test("the muster's declared facts survive the parse in the compiler's own order", () => {
  const plan = parseCombatPlan(PLAN);
  const m = plan.encounters[0]!.muster;
  assert.equal(m.probe, "the-drowned-bell:wave_muster_gate_assault");
  assert.equal(m.strike, "the-drowned-bell:wave_strike_gate_assault");
  assert.equal(m.chip, "the-drowned-bell:wave_chip_gate_assault");
  assert.equal(m.checked, 4);
  assert.deepEqual(m.types[0]!.facts, [
    "name=Gate Drowned",
    "equipment.mainhand=minecraft:trident",
  ]);
  assert.equal(m.profiles[0]!.mask, 3);
});

// --- run-backs ----------------------------------------------------------------

const RUN_BACK_RAW = {
  wave: "wave/walk-ambush",
  objective: "obj/survive-the-knives",
  bonfire: 3,
  before: "obj/enter-the-great-hall",
  tier: "ordinary",
  pos: [120, 92, 14],
  count: 3,
  radius: 16,
  crossing: [118, 92, 14],
  distance: 2.24,
  paths: ["critical-path", "halvard-released+ring"],
};

test("a plan's run-backs parse, and a plan that states none is refused", () => {
  const plan = parseCombatPlan({ ...PLAN, run_backs: [RUN_BACK_RAW] });
  assert.deepEqual(plan.runBacks[0], {
    wave: "wave/walk-ambush",
    objective: "obj/survive-the-knives",
    bonfire: 3,
    before: "obj/enter-the-great-hall",
    tier: "ordinary",
    pos: [120, 92, 14],
    count: 3,
    radius: 16,
    crossing: [118, 92, 14],
    distance: 2.24,
    paths: ["critical-path", "halvard-released+ring"],
  });
  // No default: a plan that cannot say whether a rest puts a fight back beside
  // the path would have the ladder walk the leg as empty.
  const { run_backs: _gone, ...without } = { ...PLAN };
  assert.throws(() => parseCombatPlan(without), /run_backs/);
  assert.throws(
    () => parseCombatPlan({ ...PLAN, run_backs: [{ ...RUN_BACK_RAW, paths: [] }] }),
    /paths/,
  );
});

test("a run-back is due only after this walk cleared the wave and then rested", () => {
  const rb = parseCombatPlan({ ...PLAN, run_backs: [RUN_BACK_RAW] }).runBacks as RunBack[];
  const before = "obj/enter-the-great-hall";
  assert.deepEqual(dueRunBacks(rb, before, new Map(), new Map([[3, 30]])), [], "never cleared");
  assert.deepEqual(dueRunBacks(rb, before, new Map([["wave/walk-ambush", 25]]), new Map()), [], "never rested");
  assert.deepEqual(
    dueRunBacks(rb, before, new Map([["wave/walk-ambush", 25]]), new Map([[3, 20]])),
    [],
    "the rest came before the clearance: the wave is down",
  );
  assert.equal(
    dueRunBacks(rb, before, new Map([["wave/walk-ambush", 25]]), new Map([[3, 30]])).length,
    1,
  );
  assert.deepEqual(
    dueRunBacks(rb, "obj/elsewhere", new Map([["wave/walk-ambush", 25]]), new Map([[3, 30]])),
    [],
    "only before the step whose leg re-crosses it",
  );
});

test("a death-respawn re-seats as the last rest did", () => {
  const rested = new Map([
    [1, 10],
    [3, 30],
  ]);
  respawnReseats(rested, 41);
  assert.deepEqual([...rested], [
    [1, 10],
    [3, 41],
  ]);
  const none = new Map<number, number>();
  respawnReseats(none, 41);
  assert.equal(none.size, 0, "no rest yet: the respawn is at world spawn");
});
