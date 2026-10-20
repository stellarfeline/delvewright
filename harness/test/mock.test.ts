import { test } from "node:test";
import assert from "node:assert/strict";

import type { MockStep } from "../src/critical-path.ts";
import { mockCommand, mockVerdict } from "../src/mock.ts";
import { runSequence } from "../src/sequencer.ts";

const step: MockStep = {
  action: "mock",
  act: "shop/stall/offer-0",
  kind: "shop-offer",
  function: "hello-world:shop_pick_0_0",
  before: [{ objective: "dw.f_candle", holder: "#party", min: 1, max: 1, negate: true }],
  after: [{ objective: "dw.f_candle", holder: "#party", min: 1, max: 1, negate: false }],
};
const answered = { answered: true, lines: [] as string[] };

test("a mock runs the act's own function as the bot, at the bot", () => {
  assert.equal(mockCommand(step), "/execute as @s at @s run function hello-world:shop_pick_0_0");
});

test("a mock passes only when the change it promises is observed on both sides", () => {
  assert.equal(mockVerdict(step, [true], answered, [true]).passed, true);
  // Already set before: no change is shown.
  assert.match(mockVerdict(step, [false], answered, [true]).detail, /before the act/);
  // The function ran and nothing changed.
  assert.match(mockVerdict(step, [true], answered, [false]).detail, /after the act/);
  // An unreadable term establishes nothing.
  assert.match(mockVerdict(step, [true], answered, [undefined]).detail, /unread/);
});

test("a mock the server refuses or never answers fails, naming the command", () => {
  const refused = mockVerdict(step, [true], { answered: true, lines: ["Unknown function hello-world:shop_pick_0_0"] }, [true]);
  assert.equal(refused.passed, false);
  assert.match(refused.detail, /refused/);
  const silent = mockVerdict(step, [true], { answered: false, lines: [] }, [true]);
  assert.equal(silent.passed, false);
  assert.match(silent.detail, /never closed/);
});

test("a path with a mock step against an executor that cannot mock fails, never skips", async () => {
  const noop = async (): Promise<void> => {};
  const executor = {
    selectClass: noop,
    talkTo: noop,
    reach: noop,
    kill: noop,
    collect: noop,
    interact: noop,
    fireTrigger: noop,
    assertComplete: noop,
  };
  await assert.rejects(
    runSequence(
      {
        version: "0.0.0",
        formatVersion: 5,
        campaignId: "hello-world",
        nonCombatants: { kinds: [], examined: 0, ambiguous: [], unbound: false },
        steps: [
          { action: "select-class", class: "class/a", command: "/trigger dw.class set 1" },
          step,
          { action: "assert-complete", scoreboard: { objective: "dw.campaign", value: 1 } },
        ],
      } as never,
      executor,
    ),
    (e: unknown) => /cannot run a mock/.test(String((e as { cause?: unknown }).cause ?? e)),
  );
});
