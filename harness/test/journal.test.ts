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
  JOURNAL_PAGES_COMMAND,
  type JournalVerdict,
} from "../src/journal.ts";

// The journal names the next act at every quest start. Each reading the bot
// makes off the server is planted here, the refusal shapes with it.

const raw = {
  party: "#party",
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
    quests: [{ ...raw.quests[0], told: [{ objective: "obj/a", done: "dw.o_a", hint_key: null }] }],
  });
  assert.equal(baked.quests[0]!.told[0]!.hintKey, null);
  assert.throws(
    () =>
      parseJournalLedger({
        party: "#party",
        quests: [{ ...raw.quests[0], told: [{ objective: "obj/a", done: "dw.o_a", hint_key: "" }] }],
      }),
    JournalLedgerParseError,
  );
  assert.throws(
    () => parseJournalLedger({ party: "#party", quests: [{ ...raw.quests[0], told: [{ objective: "obj/a" }] }] }),
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
  assert.match(JOURNAL_PAGES_COMMAND, /^\/tellraw @s \{"nbt":"Inventory\[/);
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
