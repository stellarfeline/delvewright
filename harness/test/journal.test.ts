import { test } from "node:test";
import assert from "node:assert/strict";
import {
  JournalLedgerParseError,
  journalBinding,
  journalBindingLine,
  journalFailures,
  pagesShow,
  parseJournalLedger,
  scoreIsOne,
  scoreIsOneCommand,
  JOURNAL_PAGES_PATH,
  journalPageCommand,
  readJournalPages,
  type JournalVerdict,
  heldCommands,
  isHeld,
  journalStartDue,
} from "../src/journal.ts";

// The journal names the next act at every quest start. Each reading the bot
// makes off the server is planted here, the refusal shapes with it.

const raw = {
  party: "#party",
  mode: "dw.mode",
  quests: [
    {
      quest: "quest/the-house",
      active: "dw.qa_the_house",
      told: [
        {
          objective: "obj/hear",
          done: "dw.o_hear",
          hint_key: "delve.x.obj.the-house.hear.hint",
        },
      ],
      gated: [],
      held: [],
    },
    {
      quest: "quest/the-boat",
      active: "dw.qa_the_boat",
      told: [
        {
          objective: "obj/bolt",
          done: "dw.o_bolt",
          hint_key: "delve.x.obj.the-boat.bolt.hint",
        },
      ],
      gated: ["obj/branch"],
      held: ["1", "3..4"],
    },
  ],
};

test("the ledger parses strictly", () => {
  const l = parseJournalLedger(raw);
  assert.equal(l.quests.length, 2);
  assert.equal(l.quests[1]!.told[0]!.hintKey, "delve.x.obj.the-boat.bolt.hint");
  assert.throws(() => parseJournalLedger({ ...raw, quests: {} }), JournalLedgerParseError);
  // A bake's page carries no key: `null`, never an empty string.
  const baked = parseJournalLedger({
    party: "#party",
    mode: null,
    quests: [{ ...raw.quests[0], told: [{ objective: "obj/a", done: "dw.o_a", hint_key: null }] }],
  });
  assert.equal(baked.quests[0]!.told[0]!.hintKey, null);
  assert.throws(
    () =>
      parseJournalLedger({
        party: "#party",
        mode: null,
        quests: [{ ...raw.quests[0], told: [{ objective: "obj/a", done: "dw.o_a", hint_key: "" }] }],
      }),
    JournalLedgerParseError,
  );
  assert.throws(
    () =>
      parseJournalLedger({ party: "#party", mode: null, quests: [{ ...raw.quests[0], told: [{ objective: "obj/a" }] }] }),
    JournalLedgerParseError,
  );
});

test("a score reads 1 by its own marker and by nothing else", () => {
  const cmd = scoreIsOneCommand("#party", "dw.qa_the_boat");
  assert.equal(
    cmd,
    '/execute if score #party dw.qa_the_boat matches 1 run tellraw @s {"text":"[dw:journal dw.qa_the_boat 1]"}',
  );
  assert.equal(scoreIsOne(["[dw:journal dw.qa_the_boat 1]"], "dw.qa_the_boat"), true);
  assert.equal(scoreIsOne([], "dw.qa_the_boat"), false);
  // Another score's marker is not this one's.
  assert.equal(scoreIsOne(["[dw:journal dw.qa_the_house 1]"], "dw.qa_the_boat"), false);
  assert.match(journalPageCommand(3), /^\/tellraw @s \{"nbt":"Inventory\[.*\.pages\[3\]"/);
});

test("a page shows a hint by its translation key and by nothing else", () => {
  const pages = [
    'delve-bot has the following entity data: [{raw: {extra: [{fallback: "To do", translate: "delve.x.ui.journal.open"}, ' +
      '{fallback: "Press the buttons, then draw the bolt.", translate: "delve.x.obj.the-boat.bolt.hint"}]}}]',
  ];
  assert.equal(pagesShow(pages, "delve.x.obj.the-boat.bolt.hint"), true);
  assert.equal(pagesShow(pages, "delve.x.obj.the-boat.bolt"), false);
  assert.equal(pagesShow([], "delve.x.obj.the-boat.bolt.hint"), false);
});

test("a missing page reds the run and the binding states its zeroes", () => {
  const l = parseJournalLedger(raw);
  const verdicts: JournalVerdict[] = [
    { quest: "quest/the-house", objective: "obj/hear", step: 0, outcome: "shown", detail: "" },
    { quest: "quest/the-boat", objective: "obj/bolt", step: 5, outcome: "missing", detail: "the journal read nothing" },
  ];
  const b = journalBinding(l, new Set(["quest/the-house", "quest/the-boat"]), verdicts);
  assert.deepEqual(
    { judged: b.judged, shown: b.shown, missing: b.missing, started: b.started, gated: b.gated, owing: b.owing },
    { judged: 2, shown: 1, missing: 1, started: 2, gated: 1, owing: 2 },
  );
  const f = journalFailures(verdicts);
  assert.equal(f.length, 1);
  assert.match(f[0]!, /quest\/the-boat started at step 5 and obj\/bolt's hint is on no page/);
  const none = journalBinding(l, new Set(), []);
  assert.match(journalBindingLine(none), /UNBOUND/);
  assert.doesNotMatch(journalBindingLine(b), /UNBOUND/);
});

// spec-0137 §8: the finished pages a world's walk leaves.

import {
  judgeRecordRow,
  parseRecordsLedger,
  recordsBinding,
  recordsBindingLine,
  worldOfPath,
  walkedPathFile,
} from "../src/journal.ts";

const records = parseRecordsLedger({
  party: "#party",
  mainline: "world/0",
  objectives: [
    {
      objective: "obj/horse",
      announced: false,
      record_key: "delve.x.obj.q.horse.record",
      outcomes: [
        { ordinal: 1, key: "delve.x.dlg.keeper.n.opt.0.fx.0.outcome", site: "dialogue#/a", held: "#jr_horse.1" },
        { ordinal: 2, key: "delve.x.dlg.keeper.n.opt.1.fx.0.outcome", site: "dialogue#/b", held: "#jr_horse.2" },
      ],
    },
  ],
  worlds: [
    { world: "world/0", finished: [{ objective: "obj/horse", settles: "done", written: [1] }] },
    { world: "world/1", finished: [{ objective: "obj/horse", settles: "open", written: [] }] },
    { world: "world/2", finished: [{ objective: "obj/horse", settles: "done", written: null }] },
    { world: "world/3", finished: [{ objective: "obj/horse", settles: "failed", written: [2] }] },
  ],
});
const horse = records.objectives[0]!;
const page = (...keys: string[]) => [keys.map((k) => `{translate:"${k}"}`).join(",")];

test("a settled row shows the record and exactly the lines its world wrote", () => {
  const row = records.worlds.get("world/0")![0]!;
  const ok = judgeRecordRow("world/0", page(horse.recordKey!, horse.outcomes[0]!.key!), horse, row);
  assert.equal(ok.outcome, "shown");
  const missing = judgeRecordRow("world/0", page(horse.recordKey!), horse, row);
  assert.equal(missing.outcome, "missing");
  assert.match(missing.detail, /line 1 .* is on no page/);
  const sibling = judgeRecordRow(
    "world/0",
    page(horse.recordKey!, horse.outcomes[0]!.key!, horse.outcomes[1]!.key!),
    horse,
    row,
  );
  assert.equal(sibling.outcome, "missing");
  assert.match(sibling.detail, /line 2 .* never wrote it/);
});

test("a failed task is a finished entry: its record and the lines its world wrote", () => {
  const row = records.worlds.get("world/3")![0]!;
  assert.equal(row.settles, "failed");
  const ok = judgeRecordRow("world/3", page(horse.recordKey!, horse.outcomes[1]!.key!), horse, row);
  assert.equal(ok.outcome, "shown");
  assert.equal(judgeRecordRow("world/3", [], horse, row).outcome, "missing");
});

test("a settles token outside the four is refused", () => {
  assert.throws(
    () =>
      parseRecordsLedger({
        party: "#party",
        mainline: "world/0",
        objectives: [],
        worlds: [{ world: "world/0", finished: [{ objective: "obj/horse", settles: "lost", written: [] }] }],
      }),
    /not done, expired, failed or open/,
  );
});

test("an open hidden task has no page", () => {
  const row = records.worlds.get("world/1")![0]!;
  assert.equal(judgeRecordRow("world/1", [], horse, row).outcome, "shown");
  const leak = judgeRecordRow("world/1", page(horse.recordKey!), horse, row);
  assert.equal(leak.outcome, "missing");
});

test("a row the build cannot order is counted, never judged", () => {
  const row = records.worlds.get("world/2")![0]!;
  const v = judgeRecordRow("world/2", [], horse, row);
  assert.equal(v.outcome, "unjudged");
  const b = recordsBinding([v]);
  assert.deepEqual([b.owed, b.judged, b.unjudged], [1, 0, 1]);
  assert.match(recordsBindingLine("world/2", b), /UNBOUND/);
});

test("the world a path walks is named by its file", () => {
  assert.equal(worldOfPath("/b/validation/world-path-3.json", records), "world/3");
  assert.equal(worldOfPath("/b/critical-path.json", records), "world/0");
});

test("a branch run's records are judged in the world its own path walks, not the mainline", () => {
  // run.ts is handed critical-path.json and walks the driven world's file.
  const walked = walkedPathFile("/b/critical-path.json", "/b/validation/world-path-3.json");
  assert.equal(worldOfPath(walked, records), "world/3");
  assert.equal(worldOfPath(walkedPathFile("/b/critical-path.json", undefined), records), "world/0");
});

// The client cuts a chat line at 4096 characters (prismarine-chat's
// MAX_CHAT_LENGTH); the book has no page bound, so a long book's late pages
// must still be read.
const CHAT_CUT = 4096;

function fakeServer(pages: readonly string[]) {
  const whole = `[${pages.join(", ")}]`;
  return async (commands: readonly string[]) =>
    commands.map((c) => {
      const m = /\.pages\[(\d+)\]"/.exec(c);
      if (m === null) return { lines: [whole.slice(0, CHAT_CUT)], answered: true };
      const page = pages[Number(m[1])];
      return { lines: page === undefined ? [] : [page.slice(0, CHAT_CUT)], answered: true };
    });
}

test("every page of a book longer than one chat line is read, past the cut and past a batch", async () => {
  const pages = Array.from(
    { length: 21 },
    (_, i) => `{raw: {translate: "delve.x.page-${i}.hint", fallback: "${"x".repeat(600)}"}}`,
  );
  const send = fakeServer(pages);
  // The whole book in one line is cut: the late pages are on no page.
  const cut = (await send(["/tellraw @s " + JSON.stringify({ nbt: JOURNAL_PAGES_PATH, entity: "@s" })]))[0]!;
  assert.equal(pagesShow(cut.lines, "delve.x.page-20.hint"), false);
  const read = await readJournalPages(send);
  assert.equal(read.answered, true);
  assert.equal(read.pageCount, 21);
  for (let i = 0; i < 21; i++) assert.equal(pagesShow(read.lines, `delve.x.page-${i}.hint`), true);
});

test("a book whose read is not answered is not read to its end", async () => {
  const read = await readJournalPages(async (cs) => cs.map(() => ({ lines: [], answered: false })));
  assert.equal(read.answered, false);
  assert.equal(read.pageCount, 0);
});

// spec-0140: a quest a mode holds is told nothing while the map is in that
// mode, so its pages are owed when the mode is left, not at its start.

test("the ledger names the mode a quest is held under, strictly", () => {
  const l = parseJournalLedger(raw);
  assert.equal(l.mode, "dw.mode");
  assert.deepEqual(l.quests[0]!.held, []);
  assert.deepEqual(l.quests[1]!.held, ["1", "3..4"]);
  // `held` is required: a ledger that does not say is refused, never read as "no mode".
  const { held: _h, ...noHeld } = raw.quests[0]!;
  assert.throws(() => parseJournalLedger({ ...raw, quests: [noHeld] }), JournalLedgerParseError);
  // Ranges of no mode score are refused.
  assert.throws(() => parseJournalLedger({ ...raw, mode: null, quests: [raw.quests[1]] }), JournalLedgerParseError);
  assert.throws(
    () => parseJournalLedger({ ...raw, quests: [{ ...raw.quests[1], held: ["one"] }] }),
    JournalLedgerParseError,
  );
});

test("a held quest is asked one range at a time and read by its own markers", () => {
  const l = parseJournalLedger(raw);
  const boat = l.quests[1]!;
  assert.deepEqual(heldCommands(l, boat), [
    '/execute if score #party dw.mode matches 1 run tellraw @s {"text":"[dw:journal dw.mode 1]"}',
    '/execute if score #party dw.mode matches 3..4 run tellraw @s {"text":"[dw:journal dw.mode 3..4]"}',
  ]);
  assert.equal(isHeld(l, boat, [[], ["[dw:journal dw.mode 3..4]"]]), true);
  assert.equal(isHeld(l, boat, [[], []]), false);
  // Another range's marker on the wrong reply is not this one's.
  assert.equal(isHeld(l, boat, [["[dw:journal dw.mode 3..4]"], []]), false);
  // A quest no mode holds asks nothing and is never held.
  assert.deepEqual(heldCommands(l, l.quests[0]!), []);
  assert.equal(isHeld(l, l.quests[0]!, [["[dw:journal dw.mode 1]"]]), false);
});

test("a held quest's pages are owed when the mode is left; an unheld one's at its start", () => {
  const l = parseJournalLedger(raw);
  // The shape the gallery walks: the quest starts as the step that enters
  // the mode holding it completes. Held at that boundary; owed at the next,
  // after the mode is left.
  const boat = l.quests[1]!;
  const atEntry = isHeld(l, boat, [["[dw:journal dw.mode 1]"], []]);
  assert.equal(journalStartDue(true, atEntry), "held");
  const afterLeave = isHeld(l, boat, [[], []]);
  assert.equal(journalStartDue(true, afterLeave), "owed");
  assert.equal(journalStartDue(false, false), "not-started");
  // Perturbation: a quest no mode holds, active while the map is in a mode,
  // is owed at its start — its missing page still reds the run.
  const house = l.quests[0]!;
  const due = journalStartDue(true, isHeld(l, house, [["[dw:journal dw.mode 1]"]]));
  assert.equal(due, "owed");
  const f = journalFailures([
    { quest: house.quest, objective: "obj/hear", step: 16, outcome: "missing", detail: "the journal read nothing" },
  ]);
  assert.equal(f.length, 1);
  assert.match(f[0]!, /quest\/the-house started at step 16 and obj\/hear's hint is on no page/);
});

test("the binding counts the quests a mode held at their start", () => {
  const l = parseJournalLedger(raw);
  const b = journalBinding(l, new Set(["quest/the-boat"]), [], new Map([["quest/the-boat", 16]]));
  assert.equal(b.heldAtStart, 1);
  assert.equal(b.stillHeld, 0);
  const never = journalBinding(l, new Set(), [], new Map([["quest/the-boat", 16]]));
  assert.equal(never.stillHeld, 1);
  assert.match(journalBindingLine(b), /1 quest\(s\) started under a mode that held them \(judged at its leaving\), 0 never seen left/);
});
