# The two conditions

Both conditions run the **same** model (Claude Opus 5, in-session), the **same**
harness (MineBench MIT `voxel.exec`: `block` / `box` / `line` / seeded `rng`),
the **same** grid (256³), the **same** palette (`advanced`, 80 block ids), and
the **same recorded seed (121111)**. Only the instruction text differs.

## Condition A — baseline

Verbatim `buildSystemPrompt()` from `minebench/lib/ai/prompts.ts`
(gridSize 256, palette advanced) + `buildUserPrompt(<target>)`. Nothing else.

Note for honesty: the MineBench baseline prompt is **not** a weak prompt. It
already demands recognizability, forbids "flat decoration / visible primitives /
static isolation / uniform detail", and prescribes a primary→secondary→tertiary
build order. Condition A is therefore a *strong* baseline, and A builds were
authored with genuine best effort under it. What the baseline prompt does **not**
contain is any *numeric, checkable* craft rule.

## Condition B — methodology-primed

Condition A's prompt, plus this appended block (rules taken from
`mc-building-toolchain-dossier.md` Part 6, which sources them from craftdex.net,
guide.astroworldmc.com, ArdaCraft and Conquest Reforged):

> ## Mandatory craft constraints
> You must satisfy every rule below and state, in a `<self_check>` block before
> your JSON, how each one is satisfied with concrete numbers from your build.
>
> **Massing.** Define the overall volume before any detail block. Use **3–4
> distinct masses** at varying heights. Every **secondary mass must be 1/3 to
> 1/2** the size of the primary mass. Pass the **Silhouette Test**: the build
> must stay recognizable rendered as a solid black shape.
>
> **Hierarchy.** Exactly one **focal ("hero") element**, at least **25% taller
> or wider** than the supporting structures around it. **Detail density must
> increase toward the focal point** — never distribute detail uniformly.
>
> **Depth.** **Never place walls and pillars on the same vertical plane** —
> pull pillars **1 block proud** of the wall surface. **Any wall run wider than
> 5 blocks requires at least one depth layer** (a recessed band, an engaged
> pilaster, or an inset bay). Use micro-depth wherever possible.
>
> **Rule of Odds.** Build wall segments and bay counts in **odd numbers**
> (5, 7, 9, 11) so centred features (doors, ridges, peaks) get a true centre.
>
> **Palette.** Exactly **3–5 blocks with named roles**: `base`, `secondary`
> (a darker value step for depth/shadow), `texture` (recesses), `detail`
> (trim), `accent` (focal warmth). Distribution **60% primary / 30% secondary
> / 10% accent**, and the **accent must stay under 10% of visible faces**,
> confined to trim, frames and edges — never whole walls.
>
> **Gradients.** Blocks must range **successively** — red never touches yellow,
> yellow never touches blue; each step is a modest value shift. Gradienting is
> not splattering: vary **cluster size, shape and position**; uniform
> distribution reads artificial. Random mixing only in insignificant areas.
