import { test } from "node:test";
import assert from "node:assert/strict";
import { componentText, lodestoneTarget, nbtPlain } from "../src/heard.ts";

test("an action-bar line decoded from network NBT reads as its text", () => {
  // `title @s actionbar [{translate, fallback}, " ", {text: "3"}, "/9"]`, resolved by the server.
  const tag = {
    type: "list",
    value: {
      type: "compound",
      value: [
        { translate: { type: "string", value: "race.x.passed" }, fallback: { type: "string", value: "Checkpoint" } },
        { text: { type: "string", value: " " } },
        { text: { type: "string", value: "3" } },
        { text: { type: "string", value: "/9" } },
      ],
    },
  };
  assert.equal(componentText(nbtPlain(tag)), "Checkpoint 3/9");
  assert.equal(componentText(nbtPlain({ type: "string", value: "Go!" })), "Go!");
});

test("a compass's lodestone target is read off its components, and none is undefined", () => {
  const aimed = [
    { type: "custom_data", data: {} },
    {
      type: "lodestone_tracker",
      data: { globalPosition: { dimensionName: "minecraft:overworld", location: { x: 241, y: 72, z: 378 } }, tracked: false },
    },
  ];
  assert.deepEqual(lodestoneTarget(aimed), [241, 72, 378]);
  assert.equal(lodestoneTarget([{ type: "lodestone_tracker", data: { tracked: false } }]), undefined);
  assert.equal(lodestoneTarget(undefined), undefined);
});
