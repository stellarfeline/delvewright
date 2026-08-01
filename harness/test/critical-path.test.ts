import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CriticalPathParseError,
  parseCriticalPath,
  parseCriticalPathJson,
  SUPPORTED_DSL_VERSIONS,
} from "../src/critical-path.ts";

// The canonical spec-0002 example (amended 2026-07-30), as a fresh object per call
// so tests can mutate.
function validRaw(): Record<string, unknown> {
  return {
    version: "0.2.0",
    campaign_id: "hello-world",
    steps: [
      {
        action: "select-class",
        class: "class/wanderer",
        command: "/trigger dw.class set 1",
      },
      {
        action: "talk-to",
        npc: "npc/keeper",
        pos: [8, 65, 12],
        command: "/trigger dw.dlg_keeper set 2",
      },
      { action: "reach", anchor: "anchor/exit", pos: [8, 65, 24], radius: 2 },
      {
        action: "assert-complete",
        scoreboard: { objective: "dw.campaign", value: 1 },
      },
    ],
  };
}

test("parses the canonical spec-0002 critical path", () => {
  const path = parseCriticalPath(validRaw());
  assert.ok((SUPPORTED_DSL_VERSIONS as readonly string[]).includes(path.version));
  assert.equal(path.campaignId, "hello-world");
  assert.equal(path.steps.length, 4);

  const [select, talk, reach, done] = path.steps;
  assert.deepEqual(select, {
    action: "select-class",
    class: "class/wanderer",
    command: "/trigger dw.class set 1",
  });
  assert.deepEqual(talk, {
    action: "talk-to",
    npc: "npc/keeper",
    pos: [8, 65, 12],
    command: "/trigger dw.dlg_keeper set 2",
  });
  assert.deepEqual(reach, {
    action: "reach",
    anchor: "anchor/exit",
    pos: [8, 65, 24],
    radius: 2,
  });
  assert.deepEqual(done, {
    action: "assert-complete",
    objective: "dw.campaign",
    value: 1,
  });
});

test("parseCriticalPathJson round-trips from text", () => {
  const path = parseCriticalPathJson(JSON.stringify(validRaw()));
  assert.equal(path.campaignId, "hello-world");
});

test("parses an optional transport marker on a step (gap 8)", () => {
  const raw = validRaw();
  (raw["steps"] as Record<string, unknown>[])[2] = {
    action: "reach",
    anchor: "anchor/exit",
    pos: [8, 65, 24],
    radius: 2,
    transport: [261, 65, 4],
  };
  const path = parseCriticalPath(raw);
  assert.deepEqual((path.steps[2] as { transport?: unknown }).transport, [261, 65, 4]);
});

test("a step without transport carries no transport key (byte-identical shape)", () => {
  const path = parseCriticalPath(validRaw());
  assert.ok(!("transport" in path.steps[2]!));
});

test("rejects a malformed transport marker with a precise pointer", () => {
  const raw = validRaw();
  (raw["steps"] as Record<string, unknown>[])[2] = {
    action: "reach",
    anchor: "anchor/exit",
    pos: [8, 65, 24],
    radius: 2,
    transport: [1, 2],
  };
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/steps/2/transport" &&
      /exactly 3 elements/.test(err.message),
  );
});

test("rejects invalid JSON text with a pointer at the root", () => {
  assert.throws(
    () => parseCriticalPathJson("{ not json"),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "" &&
      /is not valid JSON/.test(err.message),
  );
});

test("rejects a non-object root", () => {
  assert.throws(
    () => parseCriticalPath([]),
    (err: unknown) =>
      err instanceof CriticalPathParseError && /must be an object/.test(err.message),
  );
});

test("rejects an unsupported version", () => {
  const raw = validRaw();
  raw["version"] = "9.9.9";
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/version" &&
      /unsupported version/.test(err.message),
  );
});

test("rejects a missing campaign_id", () => {
  const raw = validRaw();
  delete raw["campaign_id"];
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError && err.pointer === "/campaign_id",
  );
});

test("rejects an empty steps array", () => {
  const raw = validRaw();
  raw["steps"] = [];
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/steps" &&
      /at least one step/.test(err.message),
  );
});

test("rejects an unknown action with the closed enum in the message", () => {
  const raw = validRaw();
  (raw["steps"] as unknown[])[1] = { action: "teleport" };
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/steps/1/action" &&
      /select-class, talk-to, reach, kill, collect, interact, assert-complete/.test(err.message),
  );
});

test("accepts a v0.3 path with kill / collect / interact steps", () => {
  const raw = validRaw();
  raw["version"] = "0.3.0";
  (raw["steps"] as unknown[]).splice(2, 0,
    { action: "kill", wave: "wave/guards", pos: [22, 65, 12], tag: "dw_wave_guards", count: 2 },
    { action: "collect", item: "minecraft:tripwire_hook", count: 1, pos: [44, 65, 20] },
    {
      action: "interact",
      anchor: "anchor/door",
      pos: [2, 65, 12],
      command: "/trigger dw.i_door set 1",
      requires_item: "minecraft:tripwire_hook",
    },
  );
  const path = parseCriticalPath(raw);
  assert.equal(path.version, "0.3.0");
  const kill = path.steps[2];
  assert.deepEqual(kill, {
    action: "kill",
    wave: "wave/guards",
    pos: [22, 65, 12],
    tag: "dw_wave_guards",
    count: 2,
  });
  const interact = path.steps[4];
  assert.deepEqual(interact, {
    action: "interact",
    anchor: "anchor/door",
    pos: [2, 65, 12],
    command: "/trigger dw.i_door set 1",
    requiresItem: "minecraft:tripwire_hook",
  });
});

test("accepts a null interact requires_item", () => {
  const raw = validRaw();
  raw["version"] = "0.3.0";
  (raw["steps"] as unknown[]).splice(2, 0, {
    action: "interact",
    anchor: "anchor/lever",
    pos: [1, 65, 1],
    command: "/trigger dw.i_lever set 1",
    requires_item: null,
  });
  const path = parseCriticalPath(raw);
  assert.deepEqual((path.steps[2] as { requiresItem: unknown }).requiresItem, null);
});

test("rejects a pos that is not a 3-tuple, pointing at /steps/i/pos", () => {
  const raw = validRaw();
  (raw["steps"] as Array<Record<string, unknown>>)[1]!["pos"] = [8, 65];
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/steps/1/pos" &&
      /exactly 3 elements/.test(err.message),
  );
});

test("rejects a non-finite coordinate at the exact element pointer", () => {
  const raw = validRaw();
  (raw["steps"] as Array<Record<string, unknown>>)[1]!["pos"] = [8, 65, "z"];
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError && err.pointer === "/steps/1/pos/2",
  );
});

test("rejects a select-class step missing its command", () => {
  const raw = validRaw();
  delete (raw["steps"] as Array<Record<string, unknown>>)[0]!["command"];
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/steps/0/command" &&
      /must be a string/.test(err.message),
  );
});

test("rejects a non-positive reach radius", () => {
  const raw = validRaw();
  (raw["steps"] as Array<Record<string, unknown>>)[2]!["radius"] = 0;
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/steps/2/radius" &&
      /must be positive/.test(err.message),
  );
});

test("rejects an unrecognized field on a step", () => {
  const raw = validRaw();
  (raw["steps"] as Array<Record<string, unknown>>)[0]!["extra"] = true;
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/steps/0/extra" &&
      /not a recognized field/.test(err.message),
  );
});

test("parses a sneak-marked walking step (gap 7)", () => {
  const raw = validRaw();
  raw["version"] = "0.4.0";
  (raw["steps"] as Record<string, unknown>[])[2] = {
    action: "reach",
    anchor: "anchor/vault",
    pos: [8, 65, 24],
    radius: 2,
    sneak: true,
  };
  const path = parseCriticalPath(raw);
  assert.equal((path.steps[2] as { sneak?: boolean }).sneak, true);
});

test("normalizes sneak:false to an absent key (default off)", () => {
  const raw = validRaw();
  raw["version"] = "0.4.0";
  (raw["steps"] as Record<string, unknown>[])[2] = {
    action: "reach",
    anchor: "anchor/exit",
    pos: [8, 65, 24],
    radius: 2,
    sneak: false,
  };
  const path = parseCriticalPath(raw);
  assert.ok(!("sneak" in path.steps[2]!));
});

test("rejects a non-boolean sneak with a precise pointer", () => {
  const raw = validRaw();
  raw["version"] = "0.4.0";
  (raw["steps"] as Record<string, unknown>[])[2] = {
    action: "reach",
    anchor: "anchor/exit",
    pos: [8, 65, 24],
    radius: 2,
    sneak: "yes",
  };
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/steps/2/sneak" &&
      /must be a boolean/.test(err.message),
  );
});

test("parses a cutscene_seconds marker on a step (gap 7)", () => {
  const raw = validRaw();
  raw["version"] = "0.4.0";
  (raw["steps"] as Record<string, unknown>[])[1] = {
    action: "talk-to",
    npc: "npc/keeper",
    pos: [8, 65, 12],
    command: "/trigger dw.dlg_keeper set 2",
    cutscene_seconds: 6,
  };
  const path = parseCriticalPath(raw);
  assert.equal((path.steps[1] as { cutsceneSeconds?: number }).cutsceneSeconds, 6);
});

test("rejects a non-positive cutscene_seconds with a precise pointer", () => {
  const raw = validRaw();
  raw["version"] = "0.4.0";
  (raw["steps"] as Record<string, unknown>[])[2] = {
    action: "reach",
    anchor: "anchor/exit",
    pos: [8, 65, 24],
    radius: 2,
    cutscene_seconds: 0,
  };
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/steps/2/cutscene_seconds" &&
      /must be a positive integer/.test(err.message),
  );
});

test("a plain step carries neither sneak nor cutsceneSeconds (byte-identical shape)", () => {
  const path = parseCriticalPath(validRaw());
  assert.ok(!("sneak" in path.steps[2]!));
  assert.ok(!("cutsceneSeconds" in path.steps[2]!));
});

test("accepts the 0.4.0 dsl version", () => {
  const raw = validRaw();
  raw["version"] = "0.4.0";
  assert.equal(parseCriticalPath(raw).version, "0.4.0");
});

test("accepts the 0.5.0 and 0.6.0 dsl versions (additive; same path contract)", () => {
  for (const v of ["0.5.0", "0.6.0"]) {
    const raw = validRaw();
    raw["version"] = v;
    assert.equal(parseCriticalPath(raw).version, v);
  }
});

test("rejects a non-integer assert-complete scoreboard value", () => {
  const raw = validRaw();
  (raw["steps"] as Array<Record<string, unknown>>)[3]!["scoreboard"] = {
    objective: "dw.campaign",
    value: 1.5,
  };
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/steps/3/scoreboard/value" &&
      /must be an integer/.test(err.message),
  );
});
