# A lane mob that fights — what the pinned server does

Reader: the engine work on a wave's `lane` (spec-0016 §6), deciding what a raider walking a lane perceives before and after it is released to its own AI. Every statement is **[cited]** — read in the pinned server jar (sha256 `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726`, `versions.toml`) through the Mojang mappings for 1.21.11 — or **[measured]** on that jar run as a vanilla server (superflat, `difficulty=easy`, stone pads, forceloaded, a `NoAI` villager as the target pillagers take without line of sight).

## 1. `follow_range` is the radius a target is kept at

- **[cited]** `TargetGoal.canContinueToUse` ends the goal — and the target — when the mob's squared distance to it is greater than `getFollowDistance()` squared, which is the mob's `minecraft:follow_range` attribute. `NearestAttackableTargetGoal` acquires within the same radius, and `HurtByTargetGoal` sets the attacker as the target with no range of its own, so it is dropped on the next check when the attacker stands beyond `follow_range`.
- **[cited]** `Pillager.registerGoals` builds `RangedCrossbowAttackGoal(this, 1.0, 8.0F)`: the pillager stops and charges within 8 blocks of its target and walks toward it from further away. `Pillager.createAttributes` sets `follow_range` 32, `Vindicator.createAttributes` 12.
- **[measured]** A crossbow pillager summoned with `follow_range` 8 left a villager 12 blocks away untouched for 20 seconds (health 20.0); one whose `follow_range` was reset to its default shot it (13.0). A pillager with `follow_range` 8 hurt by the villager from 14 blocks (`damage … by`) did not answer in 15 seconds (20.0); one released after the same hit answered (17.0).

## 2. The primitives the release uses

- **[cited]** `attribute <target> <attribute> base reset` (`AttributeCommand.resetAttributeBase` → `AttributeMap.resetBaseValue`) sets the base to the value of the entity type's own `AttributeSupplier`, not the attribute's global default. **[measured]** On a pillager summoned with `follow_range` 8 it answered `reset to default 32.0`.
- **[measured]** `execute on attacker` selects the entity that last hurt the executing mob (the game keeps it 100 ticks); after `damage <pillager> 1 minecraft:mob_attack by <villager>`, `execute as <pillager> if function <f>` with `f` = `execute on target run return 1` / `execute on attacker if entity @s[type=minecraft:villager] run return 1` / `return 0` passed, and failed on the same pillager before the hit.

## 3. The reach the proofs read, and the check that the game gives it

- **[cited]** Every species' `follow_range` base is read from the pinned jar into `crates/delvec/data/entity-engagement-1.21.11.json` (`tools/maintenance/extract-entity-engagement.py`, two methods that agree on all 89 types that carry it). A released lane mob reaches the wider of that value and the lane's `aggro_radius`; `nav::follow::mob_reach` is that reading, and every proof over a fight's range takes it.
- The generated PackTest `engage_<wave>` stands each hostile kind on a pad above the world — a lane kind past its radius midway to its reach, any other at half its reach — has the dummy strike it once, runs the lane clock, and waits until every body holds the dummy as its target at once. A lane pillager on an 8-block lane is stood 20 blocks off: held at the radius, as before the release reset, it drops the target, as §1's measurement shows.
