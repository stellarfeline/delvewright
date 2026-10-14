import { test } from "node:test";
import assert from "node:assert/strict";
import {
  MusicPlanParseError,
  MusicWatch,
  musicBindingLine,
  musicStationsBefore,
  parseMusicPlan,
  pinnedSoundSources,
} from "../src/music.ts";

// spec-0109 §5.3: a cut reaches the client as a `stop_sound` packet with the
// ledger's category and sound; a station reads the ledger's biome under the
// eye. Each failure mode is planted here and must red.

const ledger = {
  documents: { world: "x" },
  biomes: [
    {
      id: "demo:atmosphere/hall",
      kind: "atmosphere",
      station: { cell: [1, 64, 1], step: 1, before: "obj/talk" },
    },
    { id: "demo:void", kind: "ground", station: null },
  ],
  cutscenes: [{ effect: "/content/quests/0/x/1", verdict: "confirmed" }],
  stop_sounds: [
    {
      effect: "/content/quests/0/x/0",
      category: "music",
      sound: null,
      after: "obj/talk",
      performed: true,
    },
    {
      effect: "/content/triggers/0/effects/0",
      category: null,
      sound: "minecraft:entity.warden.heartbeat",
      after: null,
      performed: false,
    },
  ],
  categories: [],
};

const plan = parseMusicPlan(ledger);
const sources = pinnedSoundSources();

test("the pinned protocol names the eleven categories by index", () => {
  assert.deepEqual(
    [...sources.entries()].sort((a, b) => a[0] - b[0]).map(([, v]) => v),
    ["master", "music", "record", "weather", "block", "hostile", "neutral", "player", "ambient", "voice", "ui"],
  );
});

test("a cut heard as a stop_sound with its category inside the window passes", () => {
  const w = new MusicWatch(plan, sources);
  w.marker("obj/talk", 1_000);
  w.packet("stop_sound", { flags: 1, source: 1 }, 1_050);
  const [v, unmarked] = w.stopVerdicts();
  assert.equal(v?.performed, true);
  assert.equal(v?.failure, undefined);
  assert.equal(unmarked?.performed, false, "a cut no marker announces is not judged");
});

test("a cut whose packet never comes, or names another category, reds", () => {
  const silent = new MusicWatch(plan, sources);
  silent.marker("obj/talk", 1_000);
  assert.match(silent.stopVerdicts()[0]?.failure ?? "", /no stop_sound packet/);
  const wrong = new MusicWatch(plan, sources);
  wrong.marker("obj/talk", 1_000);
  wrong.packet("stop_sound", { flags: 1, source: 8 }, 1_050);
  assert.match(wrong.stopVerdicts()[0]?.failure ?? "", /category music/);
  const late = new MusicWatch(plan, sources);
  late.marker("obj/talk", 1_000);
  late.packet("stop_sound", { flags: 1, source: 1 }, 20_000);
  assert.match(late.stopVerdicts()[0]?.failure ?? "", /within/);
});

test("a station reads the ledger's biome through the login registry", () => {
  const w = new MusicWatch(plan, sources);
  w.packet(
    "registry_data",
    { id: "minecraft:worldgen/biome", entries: [{ key: "minecraft:plains" }, { key: "demo:atmosphere/hall" }] },
    0,
  );
  const [due] = musicStationsBefore(plan, "obj/talk", new Set());
  assert.ok(due);
  assert.equal(w.biomeName(1), "demo:atmosphere/hall");
  assert.equal(w.recordStation(due, w.biomeName(1)).failure, null);
  assert.match(w.recordStation(due, w.biomeName(0)).failure ?? "", /ledger says demo:atmosphere\/hall/);
  const unread = w.recordStation(due, undefined);
  assert.equal(unread.notRead, "registry");
  assert.equal(unread.failure, null, "an unread station is stated, never a pass or a red");
  assert.match(
    musicBindingLine(plan, w.stopVerdicts(), w.stations),
    /3 of 1 listening station\(s\) stood at .* 1 of 2 read agree .* 1 not_read: registry; .* 1 cutscene\(s\) not_stood: cutscene/,
  );
});

test("a ledger with no ground is malformed", () => {
  assert.throws(
    () => parseMusicPlan({ ...ledger, biomes: [ledger.biomes[0]] }),
    MusicPlanParseError,
  );
});
