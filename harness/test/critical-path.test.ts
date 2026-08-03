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
    format_version: 2,
    campaign_id: "hello-world",
    steps: [
      {
        action: "select-class",
        class: "class/wanderer",
        command: "/trigger dw.class set 1",
      },
      {
        action: "talk-to",
        objective: "obj/greet",
        npc: "npc/keeper",
        pos: [8, 65, 12],
        command: "/trigger dw.dlg_keeper set 2",
      },
      {
        action: "reach",
        objective: "obj/exit",
        anchor: "anchor/exit",
        pos: [8, 65, 24],
        radius: 2,
      },
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
    objective: "obj/greet",
    npc: "npc/keeper",
    pos: [8, 65, 12],
    command: "/trigger dw.dlg_keeper set 2",
  });
  assert.deepEqual(reach, {
    action: "reach",
    objective: "obj/exit",
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
    objective: "obj/exit",
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
    objective: "obj/exit",
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
      /select-class, talk-to, reach, kill, collect, interact, rest, assert-complete/.test(err.message),
  );
});

test("accepts a v0.3 path with kill / collect / interact steps", () => {
  const raw = validRaw();
  raw["version"] = "0.3.0";
  (raw["steps"] as unknown[]).splice(2, 0,
    {
      action: "kill",
      objective: "obj/guards",
      wave: "wave/guards",
      pos: [22, 65, 12],
      tag: "dw_wave_guards",
      count: 2,
    },
    {
      action: "collect",
      objective: "obj/hook",
      item: "minecraft:tripwire_hook",
      count: 1,
      pos: [44, 65, 20],
    },
    {
      action: "interact",
      objective: "obj/door",
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
    objective: "obj/guards",
    wave: "wave/guards",
    pos: [22, 65, 12],
    tag: "dw_wave_guards",
    count: 2,
  });
  const interact = path.steps[4];
  assert.deepEqual(interact, {
    action: "interact",
    objective: "obj/door",
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
    objective: "obj/lever",
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
    objective: "obj/vault",
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
    objective: "obj/exit",
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
    objective: "obj/exit",
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
    objective: "obj/greet",
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
    objective: "obj/exit",
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

// --- contract format 2: the per-objective completion oracle (AUDIT-P0) ---------

test("parses format_version and every objective-bearing step's objective id", () => {
  const path = parseCriticalPath(validRaw());
  assert.equal(path.formatVersion, 2);
  assert.equal((path.steps[1] as { objective: string }).objective, "obj/greet");
  assert.equal((path.steps[2] as { objective: string }).objective, "obj/exit");
  // The framing steps prove no objective and carry no id.
  assert.ok(!("objective" in path.steps[0]!));
});

test("rejects a path with no format_version — it predates the oracle and is unverifiable", () => {
  const raw = validRaw();
  delete raw["format_version"];
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError &&
      err.pointer === "/format_version" &&
      /rebuild the delve/.test(err.message),
  );
});

test("rejects an unknown format_version rather than guessing the shape", () => {
  const raw = validRaw();
  raw["format_version"] = 3;
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError && err.pointer === "/format_version",
  );
});

test("rejects an objective-bearing step with no objective id", () => {
  for (const i of [1, 2]) {
    const raw = validRaw();
    delete (raw["steps"] as Array<Record<string, unknown>>)[i]!["objective"];
    assert.throws(
      () => parseCriticalPath(raw),
      (err: unknown) =>
        err instanceof CriticalPathParseError &&
        err.pointer === `/steps/${i}/objective`,
    );
  }
});

test("rejects a malformed objective id with a precise pointer", () => {
  for (const bad of ["greet", "quest/greet", "obj/Greet", "obj/", "obj/a--b"]) {
    const raw = validRaw();
    (raw["steps"] as Array<Record<string, unknown>>)[2]!["objective"] = bad;
    assert.throws(
      () => parseCriticalPath(raw),
      (err: unknown) =>
        err instanceof CriticalPathParseError &&
        err.pointer === "/steps/2/objective" &&
        /objective id/.test(err.message),
      bad,
    );
  }
});

// --- rest steps (compiler #220) ---------------------------------------------

test("a rest step parses with its bonfire index, anchor, pos and command", () => {
  const raw = validRaw();
  (raw["steps"] as unknown[]).splice(1, 0, {
    action: "rest",
    bonfire: 2,
    anchor: "anchor/beach-fire",
    pos: [12, 63, -8],
    command: "/trigger dw.rest set 2",
  });
  const path = parseCriticalPath(raw);
  const step = path.steps[1]!;
  assert.equal(step.action, "rest");
  assert.deepEqual(step, {
    action: "rest",
    bonfire: 2,
    anchor: "anchor/beach-fire",
    pos: [12, 63, -8],
    command: "/trigger dw.rest set 2",
  });
});

test("a rest step carries no objective — it proves none, it performs the loop", () => {
  const raw = validRaw();
  (raw["steps"] as unknown[]).splice(1, 0, {
    action: "rest",
    bonfire: 0,
    anchor: "anchor/fire",
    pos: [1, 2, 3],
    command: "/trigger dw.rest set 2",
    objective: "obj/nope",
  });
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) => err instanceof CriticalPathParseError && /objective/.test(err.message),
  );
});

test("a rest step with a negative bonfire index is rejected", () => {
  const raw = validRaw();
  (raw["steps"] as unknown[]).splice(1, 0, {
    action: "rest",
    bonfire: -1,
    anchor: "anchor/fire",
    pos: [1, 2, 3],
    command: "/trigger dw.rest set 2",
  });
  assert.throws(
    () => parseCriticalPath(raw),
    (err: unknown) =>
      err instanceof CriticalPathParseError && err.pointer === "/steps/1/bonfire",
  );
});
