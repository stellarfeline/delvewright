// MineflayerExecutor: a talk-to step.

import type { TalkToStep } from "../critical-path.ts";
import { delay } from "./connection.ts";
import { EFFECT_SETTLE_MS } from "./objective.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** The walk-goal radius of each interaction step — and therefore the set of
 * standing cells the crosshair sweep is entitled to try. One constant per step so
 * the goal the bot walks to and the stances it is judged over can never drift. */
const TALK_RANGE = 3;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  async talkTo(this: MineflayerExecutor, step: TalkToStep): Promise<void> {
    // Walk to the NPC first (realism; some dialog effects are reach-gated), then
    // chat the dialog-option `/trigger` command the button would have run. The
    // trigger is sent HOWEVER the walk ended — arriving is the means, the trigger is
    // the step — and `chatTrigger` records the server's answer so a step that then
    // times out can name which side swallowed it.
    await this.walkTo(step.pos, TALK_RANGE, `npc ${step.npc}`, step.sneak, {
      objective: step.objective,
      transport: step.transport,
    });
    // The trigger stands in for a dialog button this bot cannot click — so before
    // it is sent, prove the button was REACHABLE: cast the crosshair ray a player
    // casts and require this NPC's own hitbox to be what it meets first. Without
    // this, a second body on the NPC's cell is invisible to the machine and fatal
    // to the player (owner island QA, terminal finding).
    this.requireCrosshair(step.pos, `talk-to ${step.npc}`, TALK_RANGE);
    this.chatTrigger(step.command);
    // A dialogue that OPENED proves nothing: the option must actually complete the
    // objective this step stands for. Wait for that objective's own marker.
    await this.requireObjective(step.objective, `talk-to ${step.npc}`);
    await delay(EFFECT_SETTLE_MS);
  },
};
