// Shot-calibration bot (spec-0019). A minimal, campaign-agnostic driver that
// reproduces one creator calibration pass against the compose `playtest` server:
//
//   join → look straight down → `/trigger dw.aim set 1` (raycast the floor)
//        → `/trigger dw.faster set 1` (shorten the shot)
//        → `/trigger dw.mark set 1` (re-place the camera at the bot's own cell)
//        → `/trigger dw.done` (stamp the whole proposal) → disconnect.
//
// Every verb mutates `dw:rehearsal` storage only; `dw.done` stamps one
// `[DelveShot]` line per shot into the server log, which `delvec harvest` turns
// into `rehearsal-report.json` (see validation/rehearsal-flow.sh). This bot
// contains NO campaign knowledge — it fires fixed triggers on shot 1 and reports
// the cell it marked so the flow script can assert the harvest matches it.
//
// Env (reuses the executor's connection config): DELVEWRIGHT_MC_HOST/PORT/
// VERSION/AUTH + DELVEWRIGHT_BOT_USERNAME.

import { createBot } from "mineflayer";
import { botConfigFromEnv } from "./executor.ts";

/** Standing eye height the overlay adds before flooring to a cell (vanilla). */
const EYE_HEIGHT = 1.62;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function main(): Promise<number> {
  const config = botConfigFromEnv();
  process.stderr.write(
    `rehearsal-bot connecting to ${config.host}:${config.port} as ${config.username}\n`,
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

  // Let the overlay's per-tick `enable` arm the calibration triggers for us, and
  // let the bot settle onto the floor so its position is the one it reports.
  await delay(3_000);

  // Look straight down so the `dw.aim` raycast lands on the floor under us — the
  // one hit every campaign is guaranteed to have.
  await bot.look(0, Math.PI / 2, true);
  await delay(500);
  process.stderr.write("rehearsal-bot: /trigger dw.aim set 1\n");
  bot.chat("/trigger dw.aim set 1");
  await delay(1_500);

  process.stderr.write("rehearsal-bot: /trigger dw.faster set 1\n");
  bot.chat("/trigger dw.faster set 1");
  await delay(1_500);

  // The cell the overlay will record: floor(feet + eye height) per axis, the
  // same arithmetic `creator/rehearsal/mark_at` does with scoreboard division.
  const p = bot.entity.position;
  const cell = [
    Math.floor(p.x),
    Math.floor(p.y + EYE_HEIGHT),
    Math.floor(p.z),
  ];
  process.stderr.write("rehearsal-bot: /trigger dw.mark set 1\n");
  bot.chat("/trigger dw.mark set 1");
  await delay(1_500);

  process.stderr.write("rehearsal-bot: /trigger dw.done\n");
  bot.chat("/trigger dw.done");
  await delay(2_000);

  // The flow script reads this off stdout to assert the harvest matches what the
  // bot actually marked, rather than merely "something changed".
  process.stdout.write(`MARKED_CELL=${cell[0]},${cell[1]},${cell[2]}\n`);

  bot.quit("calibration captured");
  process.stderr.write("rehearsal-bot: done\n");
  return 0;
}

main()
  .then((code) => process.exit(code))
  .catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`rehearsal-bot FAILED: ${message}\n`);
    process.exit(1);
  });
