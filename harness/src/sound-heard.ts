// Where a sound packet the bot heard landed, judged against where the bot
// stands: the one rule every sound judgement in the harness reads (the quest
// cues, the containers' arrival notice).

type Vec3 = readonly [number, number, number];

/**
 * How far (blocks, per axis) a sound may land from the bot and still be heard
 * where the bot is. The packet carries its position in eighths of a block, and
 * the body may move a step, or settle onto the floor after a teleport, between
 * the server playing the sound and the client reading it; a sound played at the
 * command source instead of the player misses by the distance between them.
 */
export const SOUND_POS_TOLERANCE = 1.5;

/** A sound id without its `minecraft:` namespace, the form two ids compare in. */
export const bareSound = (s: string): string => s.replace(/^minecraft:/, "");

/** The packet landed at the bot: every axis within [`SOUND_POS_TOLERANCE`]. */
export function heardAt(heard: Vec3, bot: Vec3 | null): boolean {
  return bot !== null && heard.every((v, i) => Math.abs(v - bot[i]!) <= SOUND_POS_TOLERANCE);
}
