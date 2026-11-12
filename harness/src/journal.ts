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
// the journal it holds — the book's own pages, told to it one page at a time as their SNBT by a
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
  /** The translation key of its hint, as the page carries it; `null` in a
   * `--lang` bake, whose pages carry the hint as text with no key. */
  readonly hintKey: string | null;
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
          hintKey: r["hint_key"] === null ? null : str(r["hint_key"], `${tat}/hint_key`),
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

/**
 * The command that tells the bot page `index` of the journal it holds, as its
 * SNBT. One page per command: a chat line the client reads is cut at 4096
 * characters (prismarine-chat's `MAX_CHAT_LENGTH`), and the whole book in one
 * `tellraw` outgrows that while the book has no page bound.
 */
export function journalPageCommand(index: number): string {
  return "/tellraw @s " + JSON.stringify({ nbt: `${JOURNAL_PAGES_PATH}[${index}]`, entity: "@s" });
}

/** How many pages are asked in one bracketed turn. */
export const JOURNAL_PAGE_BATCH = 8;

/** The pages a read found: each page's reply lines, in order. */
export interface JournalRead {
  /** Every line of every page, in page order. */
  readonly lines: readonly string[];
  /** The pages found. */
  readonly pageCount: number;
  /** False when a reply was never observed: the book was not read to its end. */
  readonly answered: boolean;
}

/**
 * Read the book page by page, `JOURNAL_PAGE_BATCH` at a time, until a page
 * reads empty (past the last page the path names nothing). `send` asks each
 * command bracketed and returns each reply in order.
 */
export async function readJournalPages(
  send: (commands: readonly string[]) => Promise<readonly { lines: readonly string[]; answered: boolean }[]>,
): Promise<JournalRead> {
  const lines: string[] = [];
  let pageCount = 0;
  for (let first = 0; ; first += JOURNAL_PAGE_BATCH) {
    const commands = Array.from({ length: JOURNAL_PAGE_BATCH }, (_, i) => journalPageCommand(first + i));
    const replies = await send(commands);
    for (const r of replies) {
      if (!r.answered) return { lines, pageCount, answered: false };
      const text = r.lines.filter((l) => l.trim() !== "");
      if (text.length === 0) return { lines, pageCount, answered: true };
      lines.push(...text);
      pageCount += 1;
    }
  }
}

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
  readonly outcome: "shown" | "missing" | "done-before-read" | "baked";
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
  /** Owed pages a `--lang` bake carries with no key to read them by: unjudged. */
  readonly baked: number;
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
    baked: n("baked"),
    gated: ledger.quests.reduce((a, q) => a + q.gated.length, 0),
  };
}

/** The one line the run prints. */
export function journalBindingLine(b: JournalBinding): string {
  return (
    `journal binding: ${b.shown} of ${b.judged} owed page(s) shown at their quest's start, ` +
    `${b.missing} missing; ${b.started} of ${b.quests} quest(s) seen to start ` +
    `(${b.owing} owe a page), ${b.doneBeforeRead} owed objective(s) already complete when read, ` +
    `${b.baked} unjudged in a --lang bake, ${b.gated} gated objective(s) owed no page` +
    (b.judged === 0 ? " — UNBOUND: no quest start the run saw owed a page" : "")
  );
}

/** The failures a run's verdicts carry. */
export function journalFailures(verdicts: readonly JournalVerdict[]): string[] {
  return verdicts
    .filter((v) => v.outcome === "missing")
    .map((v) => `journal: ${v.quest} started at step ${v.step} and ${v.objective}'s hint is on no page — ${v.detail}`);
}

// ---------------------------------------------------------------------------
// The journal keeps what was done (spec-0137 §8).
//
// The compiler states, per recorded objective, the keys its finished entry
// carries — its `record`, each line the story can write — and, per story
// world, how the objective ends and which lines that world's walk leaves
// written (`validation/journal-records.json`). After the walk's last step the
// bot reads the book it holds: a settled row must show the record and exactly
// the written lines among the objective's lines; an open row must show none of
// them. The expectation is the compiler's world, never the scores the pages
// were selected by.
// ---------------------------------------------------------------------------

/** Where the records ledger sits relative to `critical-path.json`. */
export const RECORDS_LEDGER_SUBPATH = ["validation", "journal-records.json"] as const;

/** One line the story can write for an objective. */
export interface RecordOutcome {
  readonly ordinal: number;
  /** The translation key the line's page carries; `null` in a `--lang` bake. */
  readonly key: string | null;
  readonly site: string;
}

/** One recorded objective. */
export interface RecordObjective {
  readonly objective: string;
  readonly announced: boolean;
  readonly recordKey: string | null;
  readonly outcomes: readonly RecordOutcome[];
}

/** How one recorded objective ends in one world. */
export interface RecordRow {
  readonly objective: string;
  readonly settles: "done" | "expired" | "open";
  /** The ordinals the walk leaves written; `null` where the build cannot say. */
  readonly written: readonly number[] | null;
}

/** The records ledger as the bot reads it. */
export interface RecordsLedger {
  readonly mainline: string | null;
  readonly objectives: readonly RecordObjective[];
  readonly worlds: ReadonlyMap<string, readonly RecordRow[]>;
}

function keyOrNull(v: unknown, at: string): string | null {
  return v === null ? null : str(v, at);
}

/** Parse the records ledger strictly. */
export function parseRecordsLedger(raw: unknown): RecordsLedger {
  const badR = (d: string): never => {
    throw new JournalLedgerParseError(`journal-records.json ${d}`);
  };
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) badR("is not an object");
  const root = raw as Record<string, unknown>;
  const os = root["objectives"];
  const ws = root["worlds"];
  if (!Array.isArray(os)) badR("/objectives is not an array");
  if (!Array.isArray(ws)) badR("/worlds is not an array");
  const objectives = (os as unknown[]).map((o, i): RecordObjective => {
    const r = o as Record<string, unknown>;
    const outs = r["outcomes"];
    if (!Array.isArray(outs)) badR(`/objectives/${i}/outcomes is not an array`);
    return {
      objective: str(r["objective"], `/objectives/${i}/objective`),
      announced: r["announced"] === true,
      recordKey: keyOrNull(r["record_key"] ?? null, `/objectives/${i}/record_key`),
      outcomes: (outs as unknown[]).map((x, j) => {
        const y = x as Record<string, unknown>;
        const ordinal = y["ordinal"];
        if (typeof ordinal !== "number") badR(`/objectives/${i}/outcomes/${j}/ordinal is not a number`);
        return {
          ordinal: ordinal as number,
          key: keyOrNull(y["key"] ?? null, `/objectives/${i}/outcomes/${j}/key`),
          site: str(y["site"], `/objectives/${i}/outcomes/${j}/site`),
        };
      }),
    };
  });
  const worlds = new Map<string, readonly RecordRow[]>();
  (ws as unknown[]).forEach((w, i) => {
    const r = w as Record<string, unknown>;
    const fin = r["finished"];
    if (!Array.isArray(fin)) badR(`/worlds/${i}/finished is not an array`);
    worlds.set(
      str(r["world"], `/worlds/${i}/world`),
      (fin as unknown[]).map((f, j): RecordRow => {
        const x = f as Record<string, unknown>;
        const settles = x["settles"];
        if (settles !== "done" && settles !== "expired" && settles !== "open") {
          badR(`/worlds/${i}/finished/${j}/settles is not done, expired or open`);
        }
        const written = x["written"];
        if (written !== null && !(Array.isArray(written) && written.every((n) => typeof n === "number"))) {
          badR(`/worlds/${i}/finished/${j}/written is neither null nor a list of ordinals`);
        }
        return {
          objective: str(x["objective"], `/worlds/${i}/finished/${j}/objective`),
          settles: settles as RecordRow["settles"],
          written: written as number[] | null,
        };
      }),
    );
  });
  const mainline = root["mainline"];
  return {
    mainline: mainline === null || mainline === undefined ? null : str(mainline, "/mainline"),
    objectives,
    worlds,
  };
}

/** Load the records ledger beside a critical path; absent is a build that records nothing. */
export async function loadRecordsLedgerForCriticalPath(
  criticalPathFile: string,
): Promise<RecordsLedger | undefined> {
  const file = path.join(path.dirname(criticalPathFile), ...RECORDS_LEDGER_SUBPATH);
  let text: string;
  try {
    text = await readFile(file, "utf8");
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw e;
  }
  return parseRecordsLedger(JSON.parse(text) as unknown);
}

/** The world a path file walks: `world-path-<k>.json` is `world/<k>`; the critical path is the mainline. */
export function worldOfPath(file: string, ledger: RecordsLedger): string | null {
  const base = path.basename(file);
  const m = /^world-path-(\d+)\.json$/.exec(base);
  if (m) return `world/${m[1]}`;
  return ledger.mainline;
}

/** The verdict on one row. */
export interface RecordVerdict {
  readonly world: string;
  readonly objective: string;
  readonly outcome: "shown" | "missing" | "unjudged";
  readonly detail: string;
}

/** Judge one row against the pages the bot read. */
export function judgeRecordRow(
  world: string,
  pages: readonly string[],
  obj: RecordObjective,
  row: RecordRow,
): RecordVerdict {
  const v = (outcome: RecordVerdict["outcome"], detail: string): RecordVerdict => ({
    world,
    objective: obj.objective,
    outcome,
    detail,
  });
  if (row.written === null) return v("unjudged", "the walk does not order this objective's lines");
  if (obj.recordKey === null && obj.outcomes.every((o) => o.key === null)) {
    return v("unjudged", "no key: a --lang bake");
  }
  const shows = (k: string | null) => k !== null && pagesShow(pages, k);
  const wrong: string[] = [];
  if (row.settles === "open") {
    if (shows(obj.recordKey)) wrong.push(`the record ${obj.recordKey} is shown while the task is open`);
    for (const o of obj.outcomes) {
      if (shows(o.key)) wrong.push(`line ${o.ordinal} (${o.key}) is shown while the task is open`);
    }
  } else {
    if (obj.recordKey !== null && !shows(obj.recordKey)) wrong.push(`the record ${obj.recordKey} is on no page`);
    for (const o of obj.outcomes) {
      const owed = row.written.includes(o.ordinal);
      if (o.key === null) continue;
      if (owed && !shows(o.key)) wrong.push(`line ${o.ordinal} (${o.key}, ${o.site}) is on no page`);
      if (!owed && shows(o.key)) wrong.push(`line ${o.ordinal} (${o.key}, ${o.site}) is shown and this world never wrote it`);
    }
  }
  return wrong.length === 0
    ? v("shown", `${row.settles}, lines [${row.written.join(", ")}]`)
    : v("missing", wrong.join("; "));
}

/** The binding of the records gate over one run. */
export interface RecordsBinding {
  readonly owed: number;
  readonly judged: number;
  readonly shown: number;
  readonly missing: number;
  readonly unjudged: number;
}

export function recordsBinding(verdicts: readonly RecordVerdict[]): RecordsBinding {
  const n = (o: RecordVerdict["outcome"]) => verdicts.filter((v) => v.outcome === o).length;
  return {
    owed: verdicts.length,
    judged: n("shown") + n("missing"),
    shown: n("shown"),
    missing: n("missing"),
    unjudged: n("unjudged"),
  };
}

export function recordsBindingLine(world: string | null, b: RecordsBinding): string {
  return (
    `journal records binding: ${b.judged} of ${b.owed} row(s) judged in ${world ?? "no world"}, ` +
    `${b.shown} shown as the world left them, ${b.missing} wrong, ${b.unjudged} unjudged` +
    (b.owed > 0 && b.judged === 0 ? " — UNBOUND: no row this walk owes could be judged" : "")
  );
}

export function recordsFailures(verdicts: readonly RecordVerdict[]): string[] {
  return verdicts
    .filter((v) => v.outcome === "missing")
    .map((v) => `journal records: ${v.objective} in ${v.world} — ${v.detail}`);
}
