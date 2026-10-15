// spec-0108 §6.3 — the party stands in the world the ending left: the
// `assert-complete.aftermath[]` contract is parsed, each row is read against
// what a player can read, and a reading that disagrees fails the run.

import { test } from "node:test";
import assert from "node:assert/strict";
import { CriticalPathParseError, parseCriticalPath } from "../src/critical-path.ts";
import {
  aftermathFailures,
  observeAftermath,
  type AftermathEntry,
  type WorldReading,
} from "../src/aftermath.ts";

/** The gallery's terminal step, as `delvec build` exports it. */
const GALLERY_AFTERMATH = [
  {
    path: "/content/quests/1/on_complete/0",
    verb: "set-flag",
    observable: { kind: "score", holder: "#party", objective: "dw.f_hall_sealed", value: 1 },
  },
  {
    path: "/content/quests/1/on_complete/7/then/0",
    verb: "teleport",
    observable: { kind: "position", from: { min: [10, 66, 24], max: [12, 68, 26] }, to: [5, 67, 9] },
  },
  {
    path: "/content/quests/1/on_complete/7/then/1",
    verb: "fill-region",
    observable: { kind: "block", first: [10, 67, 24], last: [12, 67, 26], block: "minecraft:moss_carpet" },
  },
  {
    path: "/content/quests/1/on_complete/7/then/2",
    verb: "set-time",
    observable: { kind: "time", daytime: 12782 },
  },
  { path: "/content/quests/1/on_complete/9", verb: "give-item", observable: "emission-only" },
];

function pathWith(aftermath: unknown): Record<string, unknown> {
  return {
    version: "0.2.0",
    format_version: 5,
    campaign_id: "gallery",
    non_combatants: { kinds: ["villager"], ambiguous: [], examined: 1, unbound: false },
    steps: [
      { action: "select-class", class: "class/wanderer", command: "/trigger dw.class set 1" },
      {
        action: "assert-complete",
        scoreboard: { objective: "dw.campaign", value: 1 },
        ending_tail_ticks: 42,
        aftermath,
      },
    ],
  };
}

/** A world in which the gallery's ending did exactly what it says. */
function keptWorld(over: Partial<WorldReading> = {}): WorldReading {
  return {
    blockAt: (p) =>
      p[1] === 67 && p[0] >= 10 && p[0] <= 12 && p[2] >= 24 && p[2] <= 26
        ? "minecraft:moss_carpet"
        : "minecraft:stone",
    position: [5.5, 67, 9.5],
    stoodAt: [11.4, 67, 25.6],
    timeOfDay: 12790,
    raining: false,
    thundering: false,
    health: 20,
    effects: new Set<string>(),
    ...over,
  };
}

test("the terminal step's aftermath parses with every kind the bot reads", () => {
  const path = parseCriticalPath(pathWith(GALLERY_AFTERMATH));
  const done = path.steps[path.steps.length - 1]!;
  assert.equal(done.action, "assert-complete");
  if (done.action !== "assert-complete") return;
  assert.equal(done.endingTailTicks, 42);
  assert.equal(done.aftermath?.length, 5);
  assert.deepEqual(done.aftermath?.[2]?.observable, {
    kind: "block",
    first: [10, 67, 24],
    last: [12, 67, 26],
    block: "minecraft:moss_carpet",
  });
  assert.equal(done.aftermath?.[4]?.observable, "emission-only");
});

test("a malformed aftermath row is refused at its pointer", () => {
  const bad = structuredClone(GALLERY_AFTERMATH) as Record<string, unknown>[];
  (bad[2]!["observable"] as Record<string, unknown>)["first"] = [10, 67];
  assert.throws(
    () => parseCriticalPath(pathWith(bad)),
    (err: unknown) =>
      err instanceof CriticalPathParseError && /aftermath\/2\/observable\/first/.test(err.message),
  );
});

test("the world the ending left is read row by row, and the server-only rows say so", () => {
  const path = parseCriticalPath(pathWith(GALLERY_AFTERMATH));
  const done = path.steps[1]!;
  if (done.action !== "assert-complete") throw new Error("not the terminal step");
  const results = observeAftermath(done.aftermath as AftermathEntry[], keptWorld());
  assert.deepEqual(
    results.map((r) => [r.verb, r.status]),
    [
      ["set-flag", "not_observable"],
      ["teleport", "observed"],
      ["fill-region", "observed"],
      ["set-time", "observed"],
      ["give-item", "not_observable"],
    ],
  );
  assert.deepEqual(aftermathFailures(results), []);
});

test("an ending that left a different world fails, naming what it read", () => {
  const entries = parseCriticalPath(pathWith(GALLERY_AFTERMATH)).steps[1]!;
  if (entries.action !== "assert-complete") throw new Error("not the terminal step");
  const results = observeAftermath(
    entries.aftermath as AftermathEntry[],
    keptWorld({
      blockAt: () => "minecraft:stone",
      position: [11.5, 67, 25.5],
      timeOfDay: 6000,
    }),
  );
  const failures = aftermathFailures(results);
  assert.equal(failures.length, 3, failures.join("\n"));
  assert.match(failures.join("\n"), /expected minecraft:moss_carpet/);
  assert.match(failures.join("\n"), /expected to stand within 1.5 of \[5, 67, 9\]/);
  assert.match(failures.join("\n"), /expected time of day 12782, read 6000/);
});

test("a carry the bot was never inside is not observable rather than a pass", () => {
  const results = observeAftermath(
    [GALLERY_AFTERMATH[1] as AftermathEntry],
    keptWorld({ stoodAt: [0, 67, 0] }),
  );
  assert.equal(results[0]!.status, "not_observable");
});

test("the carry is judged by the selector's rule: a hitbox meeting the box was inside it", () => {
  // The box is [10, 66, 24]..[12, 68, 26]. A body at x 9.75 has its feet cell at
  // 9, outside the box, and its 0.6-wide hitbox reaching 10.05, inside the span
  // the teleport's `x=10,dx=2` selector matches; at 9.25 it reaches only 9.55.
  const carried = observeAftermath(
    [GALLERY_AFTERMATH[1] as AftermathEntry],
    keptWorld({ stoodAt: [9.75, 67, 25.5], position: [5.5, 67, 9.5] }),
  );
  assert.equal(carried[0]!.status, "observed", carried[0]!.detail);
  const rim = observeAftermath(
    [GALLERY_AFTERMATH[1] as AftermathEntry],
    keptWorld({ stoodAt: [9.25, 67, 25.5] }),
  );
  assert.equal(rim[0]!.status, "not_observable", rim[0]!.detail);
});

test("weather, health and effects are read off the bot's own state", () => {
  const rows: AftermathEntry[] = [
    { path: "/a", verb: "set-weather", observable: { kind: "weather", weather: "rain" } },
    { path: "/b", verb: "damage-players", observable: { kind: "health", amount: 4 } },
    { path: "/c", verb: "give-effect", observable: { kind: "effect", effect: "minecraft:slowness" } },
    { path: "/d", verb: "clear-effect", observable: { kind: "no-effect", effect: null } },
  ];
  const ok = observeAftermath(
    rows,
    keptWorld({ raining: true, health: 16, effects: new Set(["slowness"]) }),
  );
  assert.deepEqual(
    ok.map((r) => r.status),
    ["observed", "observed", "observed", "failed"],
  );
});
