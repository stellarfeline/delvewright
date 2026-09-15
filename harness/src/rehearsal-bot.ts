// Shot-calibration bot (spec-0019). A minimal, campaign-agnostic driver that
// reproduces one creator calibration pass against the compose `playtest` server:
//
//   join → look straight down → `/trigger dw.aim set 1` (raycast the floor)
//        → `/trigger dw.faster set 1` (shorten the shot)
//        → `/trigger dw.mark set 1` (re-place the camera at the bot's own cell)
//        → `/trigger dw.done` (stamp the whole proposal)
//        → the hand camera (spec-0069): `/trigger dw.cam set 1` standing,
//          `set 2` sneaking, `/trigger dw.free`, rise into the air and turn,
//          `set 3` there, `/trigger dw.free` back → disconnect.
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
/** Sneaking eye height (vanilla; minecraft.wiki "Player"). */
const SNEAK_EYE_HEIGHT = 1.27;

/** The rotation a client sends for mineflayer's radians (its own conversion,
 * then the float the packet carries). */
function notchian(yaw: number, pitch: number): [number, number] {
  return [
    Math.fround(((Math.PI - yaw) * 180) / Math.PI),
    Math.fround((-pitch * 180) / Math.PI),
  ];
}

/** Mineflayer radians for a Minecraft rotation in degrees. */
function radians(yawDeg: number, pitchDeg: number): [number, number] {
  return [Math.PI - (yawDeg * Math.PI) / 180, (-pitchDeg * Math.PI) / 180];
}

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

  // --- the hand camera (spec-0069) -----------------------------------------
  // Each capture reports the pose the bot holds as it fires — its own position
  // plus the eye height of the pose it is in, and the rotation its client sent —
  // so the flow holds the harvested camera report to the bot, not to the server.
  const capture = async (slot: number, eye: number, yawDeg: number, pitchDeg: number) => {
    const [yaw, pitch] = radians(yawDeg, pitchDeg);
    await bot.look(yaw, pitch, true);
    await delay(1_000);
    const q = bot.entity.position;
    const [ny, np] = notchian(bot.entity.yaw, bot.entity.pitch);
    process.stdout.write(
      `CAMERA_POSE slot=${slot} eye=${q.x},${q.y + eye},${q.z} yaw=${ny} pitch=${np}\n`,
    );
    process.stderr.write(`rehearsal-bot: /trigger dw.cam set ${slot}\n`);
    bot.chat(`/trigger dw.cam set ${slot}`);
    await delay(1_500);
  };

  await capture(1, EYE_HEIGHT, 30, 15);

  bot.setControlState("sneak", true);
  await delay(1_000);
  await capture(2, SNEAK_EYE_HEIGHT, -45.5, -20);
  bot.setControlState("sneak", false);
  await delay(1_000);

  const before = bot.entity.position.clone();
  const [beforeYaw, beforePitch] = notchian(bot.entity.yaw, bot.entity.pitch);
  process.stderr.write("rehearsal-bot: /trigger dw.free\n");
  bot.chat("/trigger dw.free");
  await delay(2_000);
  process.stdout.write(`FREE_MODE=${bot.game.gameMode}\n`);
  // A spectator passes through blocks and does not fall; the bot's own physics
  // knows neither, so it is switched off while the body is out of itself.
  bot.physicsEnabled = false;
  bot.entity.position.y += 4;
  bot.entity.position.x += 1.25;
  await delay(1_000);
  await capture(3, EYE_HEIGHT, 200, 35);
  process.stderr.write("rehearsal-bot: /trigger dw.free (back)\n");
  bot.chat("/trigger dw.free");
  await delay(2_000);
  bot.physicsEnabled = true;
  const after = bot.entity.position;
  const [afterYaw, afterPitch] = notchian(bot.entity.yaw, bot.entity.pitch);
  process.stdout.write(
    `FREE_RETURN mode=${bot.game.gameMode} before=${before.x},${before.y},${before.z},${beforeYaw},${beforePitch} ` +
      `after=${after.x},${after.y},${after.z},${afterYaw},${afterPitch}\n`,
  );

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
