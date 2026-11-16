// The party run's pure half (spec-0141 §6): its schedule over the path's step
// boundaries, the reading's wire format, and each verdict — driven green and
// driven red.

import { test } from "node:test";
import assert from "node:assert/strict";
import type { Seat } from "../src/death-loop.ts";
import type { Step } from "../src/critical-path.ts";
import {
  IDLER,
  MEMBERS,
  judgeArrival,
  judgeIdler,
  judgeOwed,
  judgeTellings,
  parseReadings,
  partyFromEnv,
  readingCommands,
  scheduleParty,
  type MemberReading,
} from "../src/party.ts";

const seats: Seat[] = [
  { cp: -1, label: "entry", cell: [0, 64, 0], inForceAfter: -1 },
  { cp: 0, label: "the bell fire", cell: [40, 70, 8], inForceAfter: 5 },
];

function steps(n: number): Step[] {
  const out: Step[] = [{ action: "select-class", class: "c", command: "/trigger dw.class set 1" } as Step];
  for (let i = 1; i < n; i++) out.push({ action: "reach" } as unknown as Step);
  return out;
}

function reading(over: Partial<MemberReading>): MemberReading {
  return {
    name: IDLER,
    online: true,
    pos: [0.5, 64, 0.5],
    mode: 2,
    observing: false,
    granted: 2,
    partyGranted: 2,
    deaths: 0,
    cp: 0,
    ...over,
  };
}

test("the switch is the environment's, off by default", () => {
  assert.equal(partyFromEnv({}), false);
  assert.equal(partyFromEnv({ DELVEWRIGHT_PARTY: "1" }), true);
  assert.equal(partyFromEnv({ DELVEWRIGHT_PARTY: "0" }), false);
});

test("the late member joins once a seat is armed and no earlier than halfway; the away member brackets beats", () => {
  const s = scheduleParty(steps(20), 18, seats)!;
  assert.equal(s.boundaries, 18);
  assert.equal(s.lateJoinAfter, 9, "halfway, past the first armed seat at 5");
  assert.equal(s.dropAfter, 4);
  assert.equal(s.rejoinAfter, 13);
  assert.equal(s.classCommand, "/trigger dw.class set 1");
  // A seat armed late pulls the join after it.
  const late = scheduleParty(steps(20), 18, [{ ...seats[1]!, inForceAfter: 15 }])!;
  assert.equal(late.lateJoinAfter, 15);
  // A path too short to separate the acts binds nothing.
  assert.equal(scheduleParty(steps(3), 2, seats), undefined);
  // No class step: the members take none.
  const plain = scheduleParty(steps(10).slice(1), 8, [])!;
  assert.equal(plain.classCommand, undefined);
  assert.ok(plain.dropAfter < plain.rejoinAfter && plain.rejoinAfter <= 7);
});

test("a reading round-trips through the server's rendering, an offline member included", () => {
  const cmds = readingCommands(7, MEMBERS);
  assert.equal(cmds.length, MEMBERS.length * 2);
  assert.ok(cmds[0]!.startsWith("/tellraw @s "));
  assert.ok(cmds[0]!.includes('"entity":"@a[name=party_idle,limit=1]"'));
  assert.ok(cmds[1]!.includes("tag=dw_cutscene"));
  const lines = [
    "[dw:party 7 party_idle|[0.5d, 64.0d, 0.5d]|2|3|3|0|1]",
    "[dw:party-obs 7 party_idle]",
    "[dw:party 7 party_late||||3||1]",
    "[dw:party 7 party_away|[1.0d,64.0d,2.0d]|3|1|3|1|1]",
    "[dw:party 6 party_away|[9.0d, 64.0d, 9.0d]|2|3|3|0|1]",
  ];
  const r = parseReadings(7, lines, MEMBERS);
  assert.deepEqual(r.get("party_idle"), {
    name: "party_idle",
    online: true,
    pos: [0.5, 64, 0.5],
    mode: 2,
    observing: true,
    granted: 3,
    partyGranted: 3,
    deaths: 0,
    cp: 1,
  });
  const late = r.get("party_late")!;
  assert.equal(late.online, false);
  assert.equal(late.granted, undefined);
  assert.equal(r.get("party_away")!.pos![0], 1, "the stale serial's line is not this reading");
  assert.throws(() => parseReadings(8, lines, MEMBERS), /stated nothing for party_idle/);
});

test("the idler holds where it stood, and is red when moved, stranded or seated wrong", () => {
  const stand: [number, number, number] = [0.5, 64, 0.5];
  assert.equal(judgeIdler(stand, 0, reading({}), seats, "after step 3"), undefined);
  assert.match(
    judgeIdler(stand, 0, reading({ pos: [6.5, 64, 0.5] }), seats, "after step 3")!,
    /moved .* with no death of its own/,
  );
  assert.match(judgeIdler(stand, 0, reading({ mode: 3 }), seats, "after step 3")!, /stranded out of play/);
  assert.match(judgeIdler(stand, 0, reading({ observing: true }), seats, "x")!, /observation tag/);
  assert.match(judgeIdler(stand, 0, reading({ online: false, pos: undefined }), seats, "x")!, /not on the server/);
  // A death puts it on the seat in force, which is where the table says it stands.
  assert.equal(judgeIdler(stand, 0, reading({ deaths: 1, pos: [40.5, 70, 8.5] }), seats, "x"), undefined);
  assert.match(judgeIdler(stand, 0, reading({ deaths: 1, pos: [0.5, 64, 0.5] }), seats, "x")!, /seat in force/);
});

test("a member's grants are level with the party's, and a build with none is unbound", () => {
  assert.deepEqual(judgeOwed(reading({}), "m").ok, true);
  const behind = judgeOwed(reading({ granted: 1 }), "m");
  assert.equal(behind.ok, false);
  assert.match(behind.detail, /never reached this member/);
  assert.equal(judgeOwed(reading({ granted: undefined }), "m").ok, false);
  const none = judgeOwed(reading({ granted: undefined, partyGranted: undefined }), "m");
  assert.equal(none.bound, false);
});

test("a late member arrives on the seat in force, or the entry before any", () => {
  assert.equal(judgeArrival(reading({ pos: [40.5, 70, 8.5] }), seats).ok, true);
  const wrong = judgeArrival(reading({ pos: [0.5, 64, 0.5] }), seats);
  assert.equal(wrong.ok, false);
  assert.match(wrong.detail, /the bell fire/);
  assert.equal(judgeArrival(reading({ cp: -1, pos: [0.5, 64, 0.5] }), seats).ok, true);
  assert.equal(judgeArrival(reading({ cp: undefined, pos: [0.5, 64, 0.5] }), seats).ok, true);
  assert.equal(judgeArrival(reading({ cp: 4 }), seats).bound, false);
});

test("a member present is told every beat the walker is told, and never one it was away for", () => {
  const walker = [
    { token: "obj/a", at: 10_000 },
    { token: "obj/b", at: 20_000 },
    { token: "obj/c", at: 40_000 },
  ];
  // Present throughout, heard all three.
  const all = judgeTellings(walker, walker.map((h) => ({ ...h, at: h.at + 5 })), [{ from: 0 }]);
  assert.deepEqual([all.owed, all.heard, all.missing.length, all.replayed.length], [3, 3, 0, 0]);
  // Present throughout, missed one.
  const missed = judgeTellings(walker, [walker[0]!, walker[2]!], [{ from: 0 }]);
  assert.deepEqual(missed.missing, ["obj/b"]);
  // Away from 15s to 30s: owed a and c only; `b` replayed on return is red.
  const away = [{ from: 0, to: 15_000 }, { from: 30_000 }];
  const ok = judgeTellings(walker, [walker[0]!, walker[2]!], away);
  assert.deepEqual([ok.owed, ok.heard, ok.missing.length, ok.replayed.length], [2, 2, 0, 0]);
  const replay = judgeTellings(walker, [walker[0]!, { token: "obj/b", at: 30_100 }, walker[2]!], away);
  assert.deepEqual(replay.replayed, ["obj/b"]);
  // A beat at the moment of a join is neither owed nor a replay.
  const edge = judgeTellings([{ token: "obj/d", at: 30_500 }], [], away);
  assert.deepEqual([edge.owed, edge.edge, edge.missing.length], [0, 1, 0]);
});
