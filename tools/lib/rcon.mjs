// The repo's ONE definition of "the server refused that command", for every
// Node tool that drives a live Minecraft server (task #70).
//
// ## Why this file exists
//
// A command whose response nobody reads cannot fail. `gamerule fallDamage false`
// sat in the jump-arc rig for a whole spike, one line above a comment asserting
// the bot took no fall damage; 1.21.11 answered "Incorrect argument for command"
// and changed nothing, and because the rig discarded every reply, the run was
// green and the assertion was false. The same eight legacy gamerule identifiers
// were live at two more sites. None of them was a hard bug to see — there was
// simply nowhere that a rejection could be seen.
//
// The rule was already written, correctly, INSIDE one spike
// (`tools/spike-death-teleport/measure.mjs`'s `ok()`), which is exactly the shape
// CLAUDE.md names: a general mechanism privately re-implemented inside one verb,
// so the next caller has nothing to reuse and writes the unchecked version. It
// now lives here, keyed to the object class — "a command issued to a live
// server" — and the spike imports it.
//
// ## What counts as a rejection
//
// Two families, both measured on the pinned 1.21.11 server:
//
//   * PARSE failures — the command never ran. Brigadier answers with the offending
//     prefix and a `<--[HERE]` cursor ("Incorrect argument for commandgamerule
//     fallDamage<--[HERE]"). The cursor is the reliable marker; the leading text
//     varies by failure kind.
//   * REFUSALS — the command parsed and did nothing. These are the ones that build
//     a rig out of blocks that were never placed: `fill` into an unloaded chunk
//     answers "That position is not loaded", a selector that matched nobody
//     answers "No entity was found".
//
// A reply that is neither is the command's normal answer and is returned as-is.
// `probe()` is the deliberate opt-out, for the one legitimate case: a measurement
// that is ASKING whether the server rejects something.

import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileP = promisify(execFile);

/**
 * Reply shapes that mean the server did not do what was asked.
 *
 * The list is the UNION of every private copy this rule had before it moved
 * here, and assembling it found that no two copies agreed: the death-teleport
 * spike knew the `<--[HERE]` cursor and not `No targets matched`; the
 * area-effect-arrow spike knew `No targets matched`, `Malformed ` and a broad
 * `Failed to ` and not the cursor. Each private copy was silent on exactly the
 * refusals its own run never happened to provoke, which is the failure mode a
 * shared rule exists to end — so a shape enters this list on evidence and never
 * leaves it.
 */
export const REJECTION = new RegExp(
  "(<--\\[HERE\\]" +
    "|^Unknown or incomplete command|^Incorrect argument|^Expected |^Invalid |^Unknown " +
    "|^That position is not loaded|^Cannot place blocks outside of the world" +
    "|^No blocks were filled|^Could not set the block|^No entity was found" +
    "|^No targets matched|^Malformed |^Failed to )",
);

/** True when `reply` is the server saying it refused or could not parse `cmd`. */
export function isRejection(reply) {
  return REJECTION.test(String(reply).trim());
}

/** Throw if the server refused `cmd`; otherwise hand back its reply. */
export function assertAccepted(cmd, reply) {
  if (isRejection(reply)) {
    throw new Error(`server rejected \`${cmd}\`: ${String(reply).trim()}`);
  }
  return reply;
}

/**
 * A checked rcon channel over `docker exec <container> rcon-cli`.
 *
 * `run(cmd)` is the default and THROWS on a rejection — a setup command that
 * silently did nothing is the failure this module exists to make impossible.
 * `probe(cmd)` returns the raw reply without judging it, for a measurement whose
 * subject is the rejection itself.
 */
export function rconChannel(container) {
  const send = async (cmd) => {
    const { stdout } = await execFileP("docker", ["exec", container, "rcon-cli", cmd]);
    return stdout.trim();
  };
  return {
    async run(cmd) {
      return assertAccepted(cmd, await send(cmd));
    },
    async probe(cmd) {
      return send(cmd);
    },
  };
}
