// Presenting the item an `interact` step requires — the mainhand half of the
// `interact` verb (`verb_interact_held`).
//
// `interact.requires_item` is MAINHAND-HELD, not merely possessed: the guard the
// compiler emits is `if items entity @s weapon.mainhand <item>`. Presenting the
// item IS the action, so the bot has to actually hold it. Carrying it in the pack
// — all the class loadout ever guaranteed — completes nothing: the trigger is
// swallowed by the guard and the step dies on its own objective timeout with
// nothing in the log to say the hand was empty.
//
// This is actuation and diagnostics only, never a check (CLAUDE.md: the harness
// holds no game logic). The guard stays in the datapack; a bot that cannot put the
// item in its hand still fails the step on the objective marker, and now the log
// above that failure says which of the two it was.

/** The subset of a prismarine-item `Item` this helper matches on. */
export interface HandItem {
  readonly name: string;
}

/**
 * The subset of a mineflayer `Bot` an interaction trigger drives. Generic in the
 * inventory's item type so the real `Bot` — whose `equip` takes the full
 * prismarine-item `Item` — satisfies it under `strictFunctionTypes`, while a test
 * can supply a structural stand-in.
 */
export interface InteractBot<I extends HandItem = HandItem> {
  inventory: { items(): I[] };
  equip(item: I, destination: "hand"): Promise<void>;
  chat(message: string): void;
}

/**
 * Put `requiresItem` in the mainhand, then chat the emitted `/trigger` command —
 * in that order, because the datapack reads the hand on the tick it consumes the
 * trigger. Equipping after the chat would be exactly as useless as not equipping.
 *
 * A step with no `requires_item` (`null`) leaves the hand untouched: the bot must
 * not disarm itself for a step that never asked it to, and every combat step
 * re-equips the loadout on entry anyway, so a key left in hand afterwards is free.
 *
 * Item lookup is exact on the mineflayer name, which is unnamespaced, so the DSL's
 * `minecraft:` prefix is stripped first. `inventory.items()` covers the main
 * inventory and hotbar (not armour or offhand) — the slots a class kit fills and
 * the only ones `equip(…, "hand")` can draw from.
 *
 * Every outcome is reported on stderr, including the two failure modes, because
 * the alternative diagnostic is a bare 30s objective timeout: an item that is not
 * carried at all (a campaign bug — the DSL never gave it to the class) and an
 * equip that the server refused (a run bug). Neither is swallowed and neither is
 * turned into a pass: the step still stands or falls on its objective marker.
 */
export async function presentAndTrigger<I extends HandItem>(
  bot: InteractBot<I>,
  step: { readonly requiresItem: string | null; readonly command: string },
  label: string,
): Promise<void> {
  if (step.requiresItem !== null) {
    const want = step.requiresItem.replace(/^minecraft:/, "");
    const carried = bot.inventory.items().find((i) => i.name === want);
    if (!carried) {
      process.stderr.write(
        `[interact ${label}] requires ${step.requiresItem} IN HAND but it is not in ` +
          `the inventory — the trigger will be refused by the datapack guard\n`,
      );
    } else {
      try {
        await bot.equip(carried, "hand");
        process.stderr.write(`[interact ${label}] holding ${step.requiresItem}\n`);
      } catch (err) {
        process.stderr.write(
          `[interact ${label}] could not put ${step.requiresItem} in hand: ` +
            `${err instanceof Error ? err.message : String(err)}\n`,
        );
      }
    }
  }
  bot.chat(step.command);
}
