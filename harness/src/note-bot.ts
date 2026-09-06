// Playtest note-flow bot (spec-0006 M2). A minimal, campaign-agnostic driver that
// reproduces one creator note capture against the compose `playtest` server:
//
//   join → `/trigger dw.note` (the mark) → chat a fixture note → disconnect.
//
// The overlay stamps a `[DelveNote] …` line into the server log on the trigger and
// the chat line is logged as normal player chat; `delvec harvest` pairs them after
// the session (see validation/playtest-note-flow.sh). This bot contains NO campaign
// knowledge — it only fires the fixed `dw.note` trigger and speaks a fixture string.
//
// Env (reuses the executor's connection config): DELVEWRIGHT_MC_HOST/PORT/VERSION/
// AUTH + DELVEWRIGHT_BOT_USERNAME. The note text is DELVEWRIGHT_NOTE_TEXT (default a
// Chinese string, to exercise multilingual capture end-to-end).

import { createBot } from "mineflayer";
import { botConfigFromEnv } from "./executor.ts";

/** Default fixture note — Chinese, matching the spec-0006 example. */
const DEFAULT_NOTE = "这个房间太暗了";

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function main(): Promise<number> {
  const config = botConfigFromEnv();
  const noteText = process.env["DELVEWRIGHT_NOTE_TEXT"] ?? DEFAULT_NOTE;

  process.stderr.write(
    `note-bot connecting to ${config.host}:${config.port} as ${config.username}\n`,
  );

  const bot = createBot({
    host: config.host,
    port: config.port,
    username: config.username,
    version: config.version,
    auth: config.auth,
  });

  await new Promise<void>((resolve, reject) => {
    bot.once("spawn", () => resolve());
    bot.once("error", reject);
    bot.once("kicked", (reason) => reject(new Error(`kicked before spawn: ${reason}`)));
    bot.once("end", (reason) => reject(new Error(`disconnected before spawn: ${reason}`)));
  });

  // Let a few server ticks pass so the overlay's per-tick `enable` has armed
  // `dw.note` for this player before we fire it.
  await delay(2_000);
  process.stderr.write("note-bot: firing /trigger dw.note\n");
  bot.chat("/trigger dw.note");

  // The stamp is emitted on the next tick; give it room, then type the note.
  await delay(2_000);
  process.stderr.write(`note-bot: chatting note ${JSON.stringify(noteText)}\n`);
  bot.chat(noteText);

  // Let the chat line flush to the server log before we disconnect.
  await delay(2_000);
  bot.quit("note captured");
  process.stderr.write("note-bot: done\n");
  return 0;
}

main()
  .then((code) => process.exit(code))
  .catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`note-bot FAILED: ${message}\n`);
    process.exit(1);
  });
