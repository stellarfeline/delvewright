import { test } from "node:test";
import assert from "node:assert/strict";
import {
  ATTRIBUTION_RANGE,
  RETALIATION_RANGE,
  STALKER_HITS,
  STALKER_RANGE,
  THREAT_WINDOW_MS,
  ThreatTracker,
  attributeBotDamage,
  pickRetaliationTarget,
  pickStalker,
  type ThreatCandidate,
} from "../src/threat.ts";

/** A tracker on a hand-cranked clock, so windows are exercised without sleeping. */
function trackerAt(clock: { now: number }): ThreatTracker {
  return new ThreatTracker(() => clock.now);
}

test("hits age out of the threat window", () => {
  const clock = { now: 1_000 };
  const tracker = trackerAt(clock);
  tracker.record(7);
  assert.equal(tracker.hitsWithin(7), 1);
  clock.now += THREAT_WINDOW_MS - 1;
  assert.equal(tracker.hitsWithin(7), 1, "still inside the window");
  clock.now += 2;
  assert.equal(tracker.hitsWithin(7), 0, "aged out — the bot has escaped it");
  assert.equal(tracker.lastHitAt(7), undefined);
  assert.deepEqual(tracker.active(), []);
});

test("active() reports hit counts, most recent first, deterministically", () => {
  const clock = { now: 0 };
  const tracker = trackerAt(clock);
  tracker.record(1);
  tracker.record(2);
  clock.now += 100;
  tracker.record(2);
  const active = tracker.active();
  assert.deepEqual(
    active.map((a) => [a.id, a.hits]),
    [
      [2, 2],
      [1, 1],
    ],
  );
  // Ties are broken by id, so the ordering never varies run to run.
  const tie = trackerAt({ now: 0 });
  tie.record(9, 0);
  tie.record(4, 0);
  assert.deepEqual(
    tie.active().map((a) => a.id),
    [4, 9],
  );
});

test("forget/clear drop grudges (a kill, and a respawn)", () => {
  const tracker = new ThreatTracker();
  tracker.record(3);
  tracker.record(4);
  tracker.forget(3);
  assert.equal(tracker.hitsWithin(3), 0);
  assert.equal(tracker.hitsWithin(4), 1);
  tracker.clear();
  assert.equal(tracker.hitsWithin(4), 0);
});

test("retaliation picks the closest recent attacker in melee, ignoring the rest", () => {
  const clock = { now: 0 };
  const tracker = trackerAt(clock);
  tracker.record(10); // the stalker husk, right on the bot
  tracker.record(11); // something that hit the bot but has drifted away
  const candidates: ThreatCandidate[] = [
    { id: 10, distance: 2.0 },
    { id: 11, distance: 12.0 }, // out of melee — not worth breaking off for
    { id: 12, distance: 1.0 }, // closer, but has never hit the bot: not retaliation
  ];
  assert.equal(pickRetaliationTarget(candidates, tracker), 10);
  // Out of range entirely → nothing to retaliate against; the wave rule takes over.
  assert.equal(
    pickRetaliationTarget([{ id: 10, distance: RETALIATION_RANGE + 0.1 }], tracker),
    undefined,
  );
  // Once the hit ages out, the attacker stops being a retaliation target.
  clock.now += THREAT_WINDOW_MS + 1;
  assert.equal(pickRetaliationTarget(candidates, tracker), undefined);
});

test("a stalker needs repeated hits AND proximity to interrupt a walking leg", () => {
  const clock = { now: 0 };
  const tracker = trackerAt(clock);
  const near: ThreatCandidate[] = [{ id: 5, distance: STALKER_RANGE - 0.5 }];
  tracker.record(5);
  assert.equal(
    pickStalker(near, tracker),
    undefined,
    "one graze in passing is shrugged off — the leg keeps walking",
  );
  tracker.record(5);
  assert.equal(tracker.hitsWithin(5), STALKER_HITS);
  assert.equal(pickStalker(near, tracker), 5, "latched on: stop and deal with it");
  // Same mob, now trailing behind: not worth stopping for.
  assert.equal(
    pickStalker([{ id: 5, distance: STALKER_RANGE + 0.5 }], tracker),
    undefined,
  );
  // Closest qualifying candidate wins; ties by id (deterministic).
  tracker.record(6);
  tracker.record(6);
  assert.equal(
    pickStalker(
      [
        { id: 5, distance: 2 },
        { id: 6, distance: 1 },
      ],
      tracker,
    ),
    6,
  );
});

test("two attackers landing one hit each is a fight, not two grazes", () => {
  // The-drowned-bell run 1: one hit from EACH of two Hollow Gate-Warders on the
  // approach lane (~7 damage apiece) left the bot at 6/20 at the wave anchor, and a
  // per-attacker hit count never tripped. Two hits is two hits, whoever landed them.
  const tracker = new ThreatTracker();
  tracker.record(30);
  tracker.record(31);
  const lane: ThreatCandidate[] = [
    { id: 30, distance: 2.5 },
    { id: 31, distance: 1.5 },
  ];
  assert.equal(pickStalker(lane, tracker), 31, "closest of the two attackers");
  // A candidate that has never hit the bot is never the target, however close.
  assert.equal(
    pickStalker([...lane, { id: 32, distance: 0.5 }], tracker),
    31,
  );
  // Hits from mobs OUT of range do not add up to a fight the bot must stop for.
  const far = new ThreatTracker();
  far.record(40);
  far.record(41);
  assert.equal(
    pickStalker(
      [
        { id: 40, distance: STALKER_RANGE + 1 },
        { id: 41, distance: STALKER_RANGE + 2 },
      ],
      far,
    ),
    undefined,
  );
});

test("damage is attributed to the server-named source when it is visible", () => {
  const candidates: ThreatCandidate[] = [
    { id: 20, distance: 9 }, // the shooter, far away
    { id: 21, distance: 1 }, // an innocent bystander in melee range
  ];
  assert.equal(attributeBotDamage(20, candidates), 20);
});

test("damage with no named source falls back to the nearest hostile in reach", () => {
  const candidates: ThreatCandidate[] = [
    { id: 30, distance: 3.5 },
    { id: 31, distance: 1.2 },
  ];
  assert.equal(attributeBotDamage(undefined, candidates), 31);
  // A source id the client cannot see is not usable; the fallback still applies.
  assert.equal(attributeBotDamage(999, candidates), 31);
});

test("a hazard with no hostile in reach blames nobody", () => {
  // Fall damage / a trap / drowning: the nearest mob is well out of melee reach, so
  // NOTHING is recorded — the bot must never swing at a bystander for a hazard.
  const candidates: ThreatCandidate[] = [{ id: 40, distance: ATTRIBUTION_RANGE + 0.5 }];
  assert.equal(attributeBotDamage(undefined, candidates), undefined);
  assert.equal(attributeBotDamage(undefined, []), undefined);
});
