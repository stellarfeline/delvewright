// Sequencing sugar over the repository's ONE rcon channel (`tools/lib/rcon.mjs`),
// plus the `data get` readers these probes share. No rejection rule lives here:
// `run` is `rconChannel`'s, which throws on a refusal and on a cut reply, and
// `probe` is its deliberate opt-out for a command whose non-answer is expected
// (a `kill` that matched nobody, a `fill` a re-run finds already done).
import { rconChannel } from "../lib/rcon.mjs";

export function rig(container) {
  const ch = rconChannel(container);
  return {
    run: (cmd) => ch.run(cmd),
    probe: (cmd) => ch.probe(cmd),
    /** Several reads, in order. Each is unjudged: a `scoreboard players get` for
     *  a player with no score yet legitimately answers "none is set". */
    async batch(cmds) {
      const out = [];
      for (const c of cmds) out.push(await ch.probe(c));
      return out;
    },
    /** Several mutations, in order, every one of them judged. */
    async runAll(cmds) {
      const out = [];
      for (const c of cmds) out.push(await ch.run(c));
      return out;
    },
  };
}

const DATA = / has the following entity data: /;
/** The `data get` payload, or null when the tag is absent. */
export function nbt(line) {
  const i = String(line).search(DATA);
  return i < 0 ? null : String(line).slice(i).replace(DATA, "");
}
export const num = (line) => {
  const v = nbt(line);
  return v === null ? null : Number.parseFloat(v);
};
/** A `scoreboard players get` value, or null when the player holds no score. */
export const score = (line) => {
  const m = String(line).match(/ has (-?\d+) \[/);
  return m ? Number(m[1]) : null;
};
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
/** True when a `execute … run time query gametime` condition held. */
export function held(line) {
  return String(line).startsWith("The time is");
}
