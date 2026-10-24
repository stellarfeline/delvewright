// The journal names the next act at every quest start.
//
// A quest that starts tells the party what to do through the journal the
// engine gives each player (spec-0104 §8): one open page per objective the
// party can work at, its title and its hint. A started quest whose first act
// has no page leaves a player holding a book that says nothing, with nothing
// else to go on.
//
// The compiler states, per quest, its activation score and the objectives
// owed a page the moment it starts (`validation/journal-starts.json`); the
// harness derives nothing. At every step boundary the bot reads the activation
// score of each quest it has not judged yet; once a quest is active it reads
// the journal it holds — the book's own pages, told to it as their SNBT by a
// `tellraw` `nbt` component on its own inventory — and every owed objective's
// hint must be on a page, by the hint's
// translation key, within a settle window. An owed objective already complete
// when the quest is first seen active is counted, never judged.
//
// Assertions and bookkeeping only — no game logic (CLAUDE.md).

import { readFile } from "node:fs/promises";
import path from "node:path";

/** Where the ledger sits relative to `critical-path.json`. */
export const JOURNAL_LEDGER_SUBPATH = ["validation", "journal-starts.json"] as const;

/** How long a started quest's pages may take to be written (the guide refresh runs on the tick). */
export const JOURNAL_SETTLE_MS = 3_000;
/** How often the pages are re-read inside the settle window. */
export const JOURNAL_POLL_MS = 250;

/** One objective a quest start owes a page. */
export interface JournalTold {
  readonly objective: string;
  /** The objective's completion score on the party holder. */
  readonly done: string;
  /** The translation key of its hint, as the page carries it. */
  readonly hintKey: string;
}

/** One quest's start. */
export interface JournalStart {
  readonly quest: string;
  /** The quest's activation score on the party holder. */
  readonly active: string;
  readonly told: readonly JournalTold[];
  /** Objectives with no `after` behind a gate nothing serves: owed no page, counted. */
  readonly gated: readonly string[];
}

/** The ledger as the bot reads it. */
export interface JournalLedger {
  readonly party: string;
  readonly quests: readonly JournalStart[];
}

/** Raised when the ledger is malformed — a hard failure, never a skip. */
export class JournalLedgerParseError extends Error {
  override readonly name = "JournalLedgerParseError";
}

function bad(detail: string): never {
  throw new JournalLedgerParseError(`journal-starts.json ${detail}`);
}

function str(v: unknown, at: string): string {
  if (typeof v !== "string" || v.length === 0) bad(`${at} is not a non-empty string`);
  return v;
}

/** Parse the ledger strictly. */
export function parseJournalLedger(raw: unknown): JournalLedger {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) bad("is not an object");
  const root = raw as Record<string, unknown>;
  const party = str(root["party"], "/party");
  const qs = root["quests"];
  if (!Array.isArray(qs)) bad("/quests is not an array");
  const quests = qs.map((q, i): JournalStart => {
    const at = `/quests/${i}`;
    if (typeof q !== "object" || q === null || Array.isArray(q)) bad(`${at} is not an object`);
    const o = q as Record<string, unknown>;
    const told = o["told"];
    if (!Array.isArray(told)) bad(`${at}/told is not an array`);
    const gated = o["gated"];
    if (!Array.isArray(gated) || !gated.every((g) => typeof g === "string")) {
      bad(`${at}/gated is not an array of objective ids`);
    }
    return {
      quest: str(o["quest"], `${at}/quest`),
      active: str(o["active"], `${at}/active`),
      told: told.map((t, j): JournalTold => {
        const tat = `${at}/told/${j}`;
        if (typeof t !== "object" || t === null || Array.isArray(t)) bad(`${tat} is not an object`);
        const r = t as Record<string, unknown>;
        return {
          objective: str(r["objective"], `${tat}/objective`),
          done: str(r["done"], `${tat}/done`),
          hintKey: str(r["hint_key"], `${tat}/hint_key`),
        };
      }),
      gated: gated as string[],
    };
  });
  return { party, quests };
}

/** Load the ledger beside a critical path; absent is a build with no journal. */
export async function loadJournalLedgerForCriticalPath(
  criticalPathFile: string,
): Promise<JournalLedger | undefined> {
  const file = path.join(path.dirname(criticalPathFile), ...JOURNAL_LEDGER_SUBPATH);
  let text: string;
  try {
    text = await readFile(file, "utf8");
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw e;
  }
  return parseJournalLedger(JSON.parse(text) as unknown);
}

/**
 * The command that says `[dw:journal <objective> <score>]` to the bot alone
 * exactly when the holder's score is 1. A delve seals
 * `send_command_feedback false`, so no command's own reply reaches the bot:
 * every reading is a `tellraw` the bot is sent.
 */
export function scoreIsOneCommand(holder: string, objective: string): string {
  return (
    `/execute if score ${holder} ${objective} matches 1 run tellraw @s ` +
    JSON.stringify({ text: scoreMarker(objective) })
  );
}

/** The line {@link scoreIsOneCommand} tells. */
export function scoreMarker(objective: string): string {
  return `[dw:journal ${objective} 1]`;
}

/** Whether a bracketed reply carries the score's marker. */
export function scoreIsOne(lines: readonly string[], objective: string): boolean {
  const m = scoreMarker(objective);
  return lines.some((l) => l.trim() === m);
}

/** The NBT path of the pages of the journal a player holds. */
export const JOURNAL_PAGES_PATH =
  'Inventory[{components:{"minecraft:custom_data":{dw_journal:1b}}}]' +
  '.components."minecraft:written_book_content".pages';

/** The command that tells the bot the pages of the journal it holds, as their SNBT. */
export const JOURNAL_PAGES_COMMAND =
  "/tellraw @s " + JSON.stringify({ nbt: JOURNAL_PAGES_PATH, entity: "@s" });

/** Whether the pages reply carries a page with the hint's translation key. */
export function pagesShow(lines: readonly string[], hintKey: string): boolean {
  return lines.some((l) => l.includes(`"${hintKey}"`));
}

/** The verdict on one owed page. */
export interface JournalVerdict {
  readonly quest: string;
  readonly objective: string;
  /** The step boundary the quest was first seen active at. */
  readonly step: number;
  readonly outcome: "shown" | "missing" | "done-before-read";
  readonly detail: string;
}

/** The binding of the journal gate over one run. */
export interface JournalBinding {
  /** Quests the ledger declares, and how many owe a page at their start. */
  readonly quests: number;
  readonly owing: number;
  /** Quests the run saw start. */
  readonly started: number;
  /** Pages judged, shown, missing; owed objectives already done when read. */
  readonly judged: number;
  readonly shown: number;
  readonly missing: number;
  readonly doneBeforeRead: number;
  /** Objectives owed no page by the ledger (gated, nothing serves them). */
  readonly gated: number;
}

/** Count what the run judged. */
export function journalBinding(
  ledger: JournalLedger,
  started: ReadonlySet<string>,
  verdicts: readonly JournalVerdict[],
): JournalBinding {
  const n = (o: JournalVerdict["outcome"]) => verdicts.filter((v) => v.outcome === o).length;
  return {
    quests: ledger.quests.length,
    owing: ledger.quests.filter((q) => q.told.length > 0).length,
    started: started.size,
    judged: n("shown") + n("missing"),
    shown: n("shown"),
    missing: n("missing"),
    doneBeforeRead: n("done-before-read"),
    gated: ledger.quests.reduce((a, q) => a + q.gated.length, 0),
  };
}

/** The one line the run prints. */
export function journalBindingLine(b: JournalBinding): string {
  return (
    `journal binding: ${b.shown} of ${b.judged} owed page(s) shown at their quest's start, ` +
    `${b.missing} missing; ${b.started} of ${b.quests} quest(s) seen to start ` +
    `(${b.owing} owe a page), ${b.doneBeforeRead} owed objective(s) already complete when read, ` +
    `${b.gated} gated objective(s) owed no page` +
    (b.judged === 0 ? " — UNBOUND: no quest start the run saw owed a page" : "")
  );
}

/** The failures a run's verdicts carry. */
export function journalFailures(verdicts: readonly JournalVerdict[]): string[] {
  return verdicts
    .filter((v) => v.outcome === "missing")
    .map((v) => `journal: ${v.quest} started at step ${v.step} and ${v.objective}'s hint is on no page — ${v.detail}`);
}
