// The container class's arrival line reaches the player (spec-0105 §5.3).
//
// When a campaign has scenery containers, `containers.decoration_notice` is told
// to each player by their class apply function, in the function that warps them
// in, after the warp. PackTest cannot see it: the line is a client-bound chat or
// title packet, and its sound a sound packet. This reads both: the bot records
// what it is told and what it hears between its class command and the end of
// that step's settle, and asserts the compiler's resolved English text arrived
// in that window and, when the notice names a sound, that the sound arrived at
// the bot's own position after the warp — where a player hears the arrival.
//
// The compiler states the text and the sound (`validation/holders.json`, its
// `notice`); the harness derives nothing.
//
// Assertions and bookkeeping only — no game logic (CLAUDE.md).

import { readFile } from "node:fs/promises";
import path from "node:path";

/** Where the ledger sits, relative to the critical path's directory. */
export const CONTAINER_LEDGER_SUBPATH = ["validation", "holders.json"] as const;

/** The notice as the compiler resolved it. */
export interface Notice {
  /** The line's plain English text. */
  readonly text: string;
  /** Its channel: `chat`, `title`, `subtitle`, `art` or `actionbar`. */
  readonly style: string;
  /** The sound event it plays, namespaced, or `null` when it plays none. */
  readonly sound: string | null;
}

/** The parts of the census ledger the bot reads. */
export interface ContainerLedger {
  /** Container cells the census counted. */
  readonly cells: number;
  /** Of them, the scenery ones. */
  readonly decoration: number;
  /** The arrival line, or `null` when the campaign declares none. */
  readonly notice: Notice | null;
}

/** Raised when the ledger is malformed — a hard failure, never a skip. */
export class ContainerLedgerParseError extends Error {
  override readonly name = "ContainerLedgerParseError";
}

function bad(detail: string): never {
  throw new ContainerLedgerParseError(`holders.json ${detail}`);
}

const STYLES = new Set(["chat", "title", "subtitle", "art", "actionbar"]);

function count(o: Record<string, unknown>, key: string): number {
  const v = o[key];
  if (typeof v !== "number" || !Number.isInteger(v) || v < 0) {
    bad(`\`${key}\` must be a non-negative integer, got ${JSON.stringify(v)}`);
  }
  return v;
}

/** Parse the ledger strictly: a field the bot reads is present and well-typed. */
export function parseContainerLedger(raw: unknown): ContainerLedger {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) bad("is not an object");
  const o = raw as Record<string, unknown>;
  const cells = count(o, "cells");
  const decoration = count(o, "decoration");
  if (decoration > cells) bad(`counts ${decoration} decoration cell(s) of ${cells}`);
  if (!("notice" in o)) bad("has no `notice` (null when none is declared)");
  const n = o["notice"];
  let notice: Notice | null = null;
  if (n !== null) {
    if (typeof n !== "object" || Array.isArray(n)) bad("`notice` must be null or an object");
    const no = n as Record<string, unknown>;
    const text = no["text"];
    const style = no["style"];
    if (!("sound" in no)) bad("`notice` has no `sound` (null when it plays none)");
    const sound = no["sound"];
    if (sound !== null && (typeof sound !== "string" || sound.length === 0)) {
      bad(`\`notice.sound\` must be null or a non-empty string, got ${JSON.stringify(sound)}`);
    }
    if (typeof text !== "string" || text.length === 0) bad("`notice.text` must be a non-empty string");
    if (typeof style !== "string" || !STYLES.has(style)) {
      bad(`\`notice.style\` must be one of ${[...STYLES].join(", ")}, got ${JSON.stringify(style)}`);
    }
    notice = { text, style, sound: sound as string | null };
  }
  return { cells, decoration, notice };
}

/** Load the ledger beside a critical path; absent means a build that predates it. */
export async function loadContainerLedgerForCriticalPath(
  criticalPathFile: string,
): Promise<ContainerLedger | undefined> {
  const file = path.join(path.dirname(criticalPathFile), ...CONTAINER_LEDGER_SUBPATH);
  let text: string;
  try {
    text = await readFile(file, "utf8");
  } catch {
    return undefined;
  }
  return parseContainerLedger(JSON.parse(text) as unknown);
}

/** One sound packet the client was told about. */
export interface HeardSound {
  readonly name: string;
  readonly pos: readonly [number, number, number];
}

/** What the bot was told in the window: chat lines, title texts and sounds. */
export interface ToldWindow {
  readonly chat: readonly string[];
  readonly titles: readonly string[];
  /** Chat lines that fell out of the ring before the bot read them. */
  readonly lost: number;
  /** Every sound packet the bot received in the window. */
  readonly sounds: readonly HeardSound[];
  /** The bot's own position at the end of the window — after the warp. */
  readonly arrival: readonly [number, number, number];
}

/** The arrival line's verdict. */
export interface NoticeVerdict {
  readonly declared: boolean;
  readonly ok: boolean;
  readonly detail: string;
  /** The notice's sound as it arrived, or `null` when none was owed or none arrived. */
  readonly sound: HeardSound | null;
}

/**
 * How far (blocks, per axis) the notice's sound may land from the bot's
 * position after the warp. The packet carries its position in eighths of a
 * block and the body may settle onto the floor after the teleport; a sound at
 * the command source instead of the player misses by the distance between them.
 */
export const NOTICE_SOUND_TOLERANCE = 1.5;

const bare = (s: string): string => s.replace(/^minecraft:/, "");

/**
 * **Judge the window against the ledger.** A declared notice must arrive: a
 * chat-family line (`chat`, `actionbar`) among the chat lines, an on-screen one
 * among the titles, carrying the compiler's text. With none declared there is
 * no text to look for, and the verdict says what the window held.
 */
export function judgeNotice(ledger: ContainerLedger, told: ToldWindow): NoticeVerdict {
  const n = ledger.notice;
  const lines = told.chat.length + told.titles.length;
  if (n === null) {
    return {
      declared: false,
      ok: true,
      detail:
        `no notice declared (${ledger.decoration} of ${ledger.cells} container cell(s) are ` +
        `scenery); ${lines} line(s) told in the window`,
      sound: null,
    };
  }
  const sound = judgeNoticeSound(n, told);
  const onScreen = n.style === "title" || n.style === "subtitle" || n.style === "art";
  const pool = onScreen ? told.titles : told.chat;
  const want = n.style === "art" ? n.text.toUpperCase() : n.text;
  const found = pool.some((l) => l.includes(n.text) || l.includes(want));
  if (found) {
    return {
      declared: true,
      ok: sound.ok,
      detail: `the ${n.style} notice arrived in the class step's window; ${sound.detail}`,
      sound: sound.heard,
    };
  }
  const unobserved =
    told.lost > 0 ? `; ${told.lost} chat line(s) fell out of the ring unread, so the window was not fully observed` : "";
  return {
    declared: true,
    ok: false,
    detail:
      `the ${n.style} notice \`${n.text}\` did NOT arrive in the class step's window: ` +
      `${pool.length} ${onScreen ? "title" : "chat"} line(s) told, none carrying it${unobserved}; ` +
      sound.detail,
    sound: sound.heard,
  };
}

/** The notice's sound: owed, it arrived naming the sound at the bot after the warp. */
function judgeNoticeSound(
  n: Notice,
  told: ToldWindow,
): { ok: boolean; detail: string; heard: HeardSound | null } {
  const at = JSON.stringify(told.arrival.map((v) => Math.round(v * 8) / 8));
  if (n.sound === null) {
    return { ok: true, detail: `no sound owed (${told.sounds.length} sound packet(s) heard)`, heard: null };
  }
  const want = n.sound;
  const named = told.sounds.filter((h) => bare(h.name) === bare(want));
  const here = named.find((h) =>
    h.pos.every((v, i) => Math.abs(v - told.arrival[i]!) <= NOTICE_SOUND_TOLERANCE),
  );
  if (here) {
    return {
      ok: true,
      detail: `its sound ${want} arrived at ${JSON.stringify(here.pos)}, the bot at ${at} after the warp`,
      heard: here,
    };
  }
  const elsewhere = named[0];
  return {
    ok: false,
    detail: elsewhere
      ? `its sound ${want} arrived at ${JSON.stringify(elsewhere.pos)}, NOT at the bot (${at} ` +
        `after the warp, tolerance ${NOTICE_SOUND_TOLERANCE})`
      : `its sound ${want} did NOT arrive: ${told.sounds.length} sound packet(s) heard in the ` +
        `window, none naming it (the bot at ${at} after the warp)`,
    heard: elsewhere ?? null,
  };
}
