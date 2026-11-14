# Stack identity: what a `custom_data` mark does to a stack

This page is for whoever writes a rule that tells one stack of an item apart from another. It records what the pinned Minecraft 1.21.11 server does with a stack that carries a `minecraft:custom_data` component, which is the mark a holder's `collect` stack carries (spec-0111 §4.2, `compiler::holders::placement_mark`). Each line is **measured** (by the instrument named below), **cited** (a line of the tree) or **authored** (what the engine takes from it). Vanilla behaviour is established once and then treated as constant (CLAUDE.md). This page is where it was established.

## 1. The question

A `collect` that adopts a holder fills the holder with `<item>[custom_name=…,custom_data={dw_collect:"<objective id>"}] <count>`. Its per-tick held check counts `if items entity @s container.* <item>[custom_data~{dw_collect:"<objective id>"}]`, and its completion advancement carries the same SNBT as `predicates.minecraft:custom_data` (cited: `emit::collect_stack`, `emit::collect_predicate`, `emit::collect_advancement_item`). The rule rests on two things:

1. **Stacking.** A marked stack has to merge with stacks that carry the same mark, as players expect, and stay apart from every other stack of the item.
2. **Matching.** The predicate has to match the stamped stack and nothing else, even when a `custom_name` is on the stack too.

## 2. The instrument

`tools/maintenance/probe-stack-identity.py` at `4725cf9df38f`. It boots the pinned server jar and refuses any jar whose sha256 is not the `versions.toml` pin. It places a chest with three stacks set by hand: slot 0 marked `obj/a`, slot 3 named and marked `obj/a`, slot 4 marked `obj/a` with a second `custom_data` key. It then pushes four stacks through a hopper above the chest, one at a time, and reads which chest slot each push landed in. A hopper inserts by the game's own merge rule, the same one a player's inventory uses when it stacks a pick-up. Last, it reads nine `execute if items block … container.<n> <predicate>` tests. A push the hopper never delivers ends the run as having measured nothing. The expected table is `PUSHES` / `COUNTS` / `EXPECT` in the tool, and the tool exits 1 when the game disagrees with it.

## 3. What the pinned game does

Measured: two runs, one before the instrument was committed and one at `4725cf9df38f`, against server jar sha256 `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726` on OpenJDK 21.0.13. Both runs read 4 pushes and 9 predicate reads with 0 disagreeing.

| Pushed stack | Landed in |
|---|---|
| `paper[custom_data={dw_collect:"obj/a"}]` onto slot 0's marked `obj/a` | slot 0 (merged) |
| plain `paper` | slot 1 (own stack) |
| `paper[custom_data={dw_collect:"obj/b"}]` | slot 2 (own stack) |
| `paper[custom_data={dw_collect:"obj/a"}]` again | slot 0 (merged, count 3) |

| Slot holds | `paper[custom_data~{dw_collect:"obj/a"}]` | `paper` |
|---|---|---|
| marked `obj/a` | matches | matches |
| plain | **does not match** | matches |
| marked `obj/b` | **does not match** (matches `~{dw_collect:"obj/b"}`) | matches |
| named, marked `obj/a` | matches | — |
| marked `obj/a` plus another key | matches | — |

- **Measured.** A marked stack merges with a stack that carries the same mark. It never merges with an unmarked stack or with a stack that carries a different mark. A player who takes one holder's stack and then another holder's stack of the same item holds two stacks.
- **Measured.** `custom_data~<mark>` is a partial match. It matches a stack whose `custom_data` contains the mark, whatever other components or keys the stack carries. It matches no stack that lacks the mark. A bare item id matches every stack of the item, marked or not, so `clear`, a `requires_item` gate and any other plain-id read still see a marked stack.
- **Cited, second method.** The advancement form of the predicate (`predicates.minecraft:custom_data`) is exercised on the running server by the generated PackTests `collect_container` (the unmarked stack completes nothing, the marked one completes) and `collect_own_stack` (one holder's stack completes its own collect and leaves the other open). They run in the PackTest tier on the gallery.
- **Authored.** The mark is the objective's id, so two holders of one item count apart. A collect that adopts no holder carries no mark and is completed by any stack of its item. When such a collect can be open beside another collect of that item, the build stops with `DW1110`.
