# spec-0068: A firework is an effect

- **Status**: Accepted
- **Ground**: written against engine `495fca44` (`origin/main`), read only —
  the stage-5 effect vocabulary (`Verb`, 37 variants, `crates/dsl/src/stages.rs`),
  `emit::emit_play_sound`, the command-tree validator
  (`compiler::commands`), the pinned entity and item registries under
  `crates/delvec/data/` — and against three pages of the Minecraft Wiki read
  for this spec: *Firework Rocket* (entity data, damage, flight height),
  *Data component format/fireworks* and *Data component format/equippable*.
  Every game fact below names which page it comes from and is pinned by the
  test that lands it; nothing was measured on a server. The shape of this
  document is spec-0062's.
- **What it is for**: a scene that ends in fireworks has them — over the
  gate when the guard is drawn up, over the court when the bell is rung —
  written as one effect beside the sound that goes with it, with the burst
  where the campaign says and nobody hurt by it.
- **Research**: the primitive is vanilla's `minecraft:firework_rocket`
  entity carrying a `minecraft:fireworks` item component, both present in the
  pinned 1.21.11 registries (`entities-1.21.11.json`, `items-1.21.11.json`).
  The facts consumed — the entity's `LifeTime` and its randomised default, the
  component's fields and the five explosion shapes, the burst height per
  flight duration, the explosion's damage and radius — are the wiki's and are
  marked **cited**; the rules built on them are **authored**.
- **Numbers**: no spec or ADR beyond this one. **One new DW code**, `DW0899`
  (§5). **`dsl_version` moves**: the effect vocabulary gains a verb.
- **Non-goals**: a `particle` verb (§7 names it as the next member of the
  class and stops); a rocket fired *at* something (a crossbow's shape); a
  firework a player holds or uses; a display of many rockets as one verb —
  that is a `sequence` of fireworks, which the vocabulary already writes;
  flight durations beyond the three the game crafts (§3.3); a runtime check
  of who was standing where when it burst.

## 1. The defect

**Finding, from reading the tree.** `delvec schema --stage all` carries the
word `firework` nowhere; writing `{"type": "firework"}` into an effect list is
`DW0100`, *unknown variant*, the message enumerating the 37 verbs the stage
has. The nearest member of the class the verb would join — a one-shot effect
fired at a declared point — is `play-sound` with `at: {anchor}`: three of
the released castle tour's 155 nodes, and none of them can make a light. The
primitive is there to reach: `minecraft:firework_rocket` is in the pinned
entity registry, `minecraft:firework_rocket` and `minecraft:firework_star` in
the pinned item registry, and `summon` is in the pinned command tree.

## 2. The row's general form, kept and made exact

**Authored**, against `CLAUDE.md`'s *This is a general engine* paragraph.

The row says: a verb that fires a firework at a declared point, taking the
same anchor-and-offset the other one-shot point effects take, with its
emitted command checked against the pinned command tree like every other.
That is right, with two things made exact:

1. **The point is a mark.** The class *a point in the world named from an
   anchor* is spec-0066's `Mark` — anchor plus integer offset, the shape the
   three camera position types already carry. `play-sound`'s `at: {anchor}`
   does **not** carry an offset today, so "the same anchor-and-offset the
   other one-shot point effects take" describes a class of one; spec-0066
   §4.4 gives the sound its offset, and this verb takes the same type. If this
   spec lands first, the verb's `at` is written as `{anchor, offset}` in the
   camera types' spelling and spec-0066 unifies the type under it; either
   order ends with one type.
2. **The command tree checks the command, not the payload.** The validator
   walks `summon <entity> <pos> <nbt>` and accepts the NBT token whole
   (`compiler::commands`, *validation depth*); it can say nothing about the
   component inside it. What holds the payload is a test that lands with the
   emitter, pinning the component's shape from the wiki page, and the demo
   level, which is where the burst is looked at.

And one thing the row does not say, which is the half a player would meet:
**a firework's burst hurts.** The explosion deals up to 7 HP with one star,
2 HP more per additional star, to mobs and players within five blocks and not
behind a solid block [cited — *Firework Rocket*]. A verb that can put a burst
beside a body is a verb that owes a proof about where the burst is (§5).

## 3. The surface

**Authored.**

```json
{ "type": "firework",
  "at": { "anchor": "anchor/gate", "offset": [0, 0, 2] },
  "flight": 1,
  "explosions": [
    { "shape": "large_ball", "colors": ["#ffd700", "#ffffff"],
      "fade_colors": ["#8b0000"], "trail": true, "twinkle": true }
  ] }
```

### 3.1 Fields

- `at` — the mark the rocket is launched from: the cell's centre, at the
  mark's own plane. Required.
- `explosions` — one to seven bursts, each `{shape, colors, fade_colors?,
  trail?, twinkle?}`; `shape` is one of the game's five, `small_ball`,
  `large_ball`, `star`, `creeper`, `burst` [cited — *fireworks* component];
  `colors` is one or more `#rrggbb` strings, the spelling `PotionContents.
  color` already uses, emitted as the packed integers the component reads;
  `fade_colors` the same, default none; `trail` and `twinkle` default false.
  At least one explosion, because a rocket with none is a flare that glides
  along whatever it meets and shows nothing [cited]; at most seven, which is
  the game's own crafting cap and the largest count the page states a damage
  for — 19 HP, under a full body's 20 — so no rocket this verb writes can kill
  an unhurt player by itself (§5). The component would take 256; a display is
  written as many rockets in a `sequence`, not as one rocket that could.
- `flight` — 1, 2 or 3, default 1: the game's crafted flight durations, and
  the three the wiki states a height for (§3.3).

### 3.2 Emission

One command per effect, through the ordinary effect path:

```
summon minecraft:firework_rocket <x>.5 <y> <z>.5 {LifeTime:<L>,FireworksItem:{id:"minecraft:firework_rocket",count:1,components:{"minecraft:fireworks":{flight_duration:<flight>b,explosions:[{shape:"large_ball",colors:[I;16766720,16777215],fade_colors:[I;9109504],has_trail:1b,has_twinkle:1b}]}}}}
```

Two rules about it:

1. **`LifeTime` is written, never left to the game.** Unset, the game
   randomises it at launch — `(flight + 1) × 10 + random(0..5) + random(0..6)`
   ticks [cited — *Firework Rocket*, entity data] — so two runs of one
   datapack would burst at two heights and the proof in §5 would be about a
   number nobody chose. The emitter writes `L = (flight + 1) × 10`, the
   floor of that range, so the burst is the lowest the game would ever put
   it, which is the conservative side of the height proof.
2. **The item field's key is pinned by a test.** The wiki names the entity's
   item field `FireworksItem`; the test that lands the emitter asserts the
   emitted key against the pinned page, and the demo level's PackTest asserts
   a summoned rocket carries the component, so a renamed key cannot ship
   silent.

### 3.3 Where the burst is

The rocket climbs from the mark and bursts at `LifeTime`. The wiki states the
burst height as a range per flight duration — 8 to 20 blocks for 1, 18 to 34
for 2, 32 to 52 for 3 [cited] — the range being the randomised `LifeTime`;
with `LifeTime` fixed at its floor the burst height is the range's floor:
**8, 18 or 32 blocks above the mark**. That figure is authored from the page
and is the number the demo level measures on the pinned server before the
proof in §5 is trusted with it; the acceptance criteria say so.

## 4. What is checked

**Authored.**

1. **The mark resolves** — the dangling-reference codes (`DW0360` at every
   effect root) own a mark whose anchor is nothing, as they do for every
   anchor-bearing effect.
2. **The command** is walked against the pinned tree by the emitter like
   every line it writes.
3. **The burst is in open air, and nobody posted stands in its reach** (§5).

## 5. The refusal — a burst where the campaign put a body, or a roof

**Authored.** `DW0899`, build tier (exit 3), `compiler::firework` —
`compiler::lethal`'s neighbour, asked over the assembled world once the mark is a cell. Two
shapes of one rule — *a firework bursts where the campaign meant it to, and
hurts nobody the campaign posted*:

- **A roof in the way.** The column of cells above the mark, from the mark's
  cell to the burst height of §3.3, holds a solid cell. A rocket under a roof
  does not burst where the page says; it bursts against the roof, at a height
  this spec cannot state and possibly within five blocks of the floor the
  party stands on. The message names the mark, the first solid cell in the
  column and the height the flight needed; the remedy is a lower flight, a
  mark with sky over it, or a taller room — never removing the check.
- **A posted body in reach.** Any place the campaign requires a body to be —
  the entry spawn, every `set-checkpoint` and `bonfire` seat, every NPC and
  actor mark, every cast placement, every wave seat: `DW0511`'s own
  enumeration — lies within five blocks of the burst point. Judged on cell
  distance to the burst cell, with no line-of-sight credit: a wall the page
  says blocks the damage is not modelled, so the rule is conservative in the
  safe direction. The message names the burst cell and the posts in reach;
  the remedy is the mark, the flight, or the post.

**What it deliberately does not catch, stated rather than implied.** Players
are not posted; a player standing on a wall walk level with a burst eight
blocks over the court, within five blocks of it, takes up to 7 HP from one
star and 19 from seven — never a full body's 20 — and it is a hazard a
player can see coming. The
demo level is where that is looked at, and the round summary says so.

Every build prints `firework binding: F firework(s) declared, B burst
column(s) checked to H cell(s), P post(s) within reach examined, R refused`
— zeroes included.

## 6. What the gallery, the record and the skill owe

**Authored.**

- **The gallery element** (spec-0039). The hall is a 31 × 8 × 31 stone room
  and has no eight-block column of air over any floor cell, so the element
  lives where the sky is: the `overlays/valley-site` point, whose walled
  court stands under open sky, fires a firework of five explosions — one per
  shape, so every `FireworkShape` variant is bound — with `fade_colors`,
  `trail` and `twinkle` written on at least one, at flight 1, on the beat the
  overlay already ends on. Bound by perturbation: changing a colour moves the
  `summon` line. Units: `QuestEffect::firework` and each of its properties,
  the explosion object's properties, the five shape variants.
- **The probe.** One committed probe for `DW0899`: the same firework
  declared in the primary's hall at `anchor/exit`, refused at `build` (shape
  1, the roof). Shape 2 is a unit test over `compiler::firework::judge`, with
  a posted body four cells from the burst cell and six cells from it, under
  open sky. It is asked of the rule directly rather than of a campaign because
  of what the arithmetic says: a burst stands at least eight blocks over its
  mark and the reach is five, so the only body a burst can catch is one posted
  three or more courses **above** the launch plane — a wall walk, a gallery, a
  tower — and no fixture in this repository has an anchor like that under open
  sky. What carries a campaign's posts into the rule is asserted separately:
  adding a respawn seat to a built campaign moves `lethal::posted_places` and
  the firework gate's own count together, because there is one enumeration.
- **The record.** `docs/reference/compiler.md`: the effect's surface row, the
  emission row beside `play-sound`'s, the new code's row and the binding
  line; the `LifeTime` rule and the `FireworksItem` key are written there
  with the page they were pinned from — in the pull request that lands the
  code.
- **The skill.** A creator reads it from `references/quest-capabilities.md`
  under *Things that change the world* — a firework is one effect at a mark;
  it needs eight blocks of sky per flight step; it is refused under a roof
  and beside a posted body; a display is a `sequence` of them — and the
  height it flies is stated there as the page's number.
- **The demo level.** A row in `docs/demo-levels.md`: a
  court under open sky where a rung bell answers with one rocket, then a
  sequence of five; the level measures the burst height at each flight on the
  pinned server and the record takes the measured number over §3.3's cited
  one if they differ.

## 7. Scope, and what is named out of it

**Authored.**

- **`particle`** is the next one-shot point effect and takes the same mark;
  it is not written here because no finding asked for it, and the pattern is
  this spec: a verb, a mark, an emitted vanilla command, a proof about what
  the primitive reaches.
- A rocket aimed at a target, a rocket a player carries, a rocket that
  boosts an elytra — player and crossbow shapes with no puppet in them.
- Sound: a rocket makes its own launch and blast sounds; a campaign that
  wants a peal beside it writes a `play-sound` at the same mark.

## 8. Acceptance criteria

Machine-checkable; each names its instrument and the result it has on the tree
that lands it. `delvec` is this tree's `target/debug/delvec`, built from the
crate manifests this tree carries; `gallery-prefabs` is the tree
`prefabs/gallery-generator` writes.

1. **The surface.** `delvec schema --stage all` exports `QuestEffect::firework`
   with `at` (a `Mark`), `flight` (`minimum: 1`, `maximum: 3`, optional) and
   `explosions` (`minItems: 1`, `maxItems: 7` of `{shape, colors, fade_colors,
   trail, twinkle}`), and a `FireworkShape` of exactly five string constants, at
   the `dsl_version` the crate manifest states; the effect union's `oneOf` names
   38 verbs. *Met —
   `crates/dsl/tests/v29_firework.rs::the_schema_exports_the_verb_and_its_bounds`,
   `::the_schema_exports_the_explosion_and_its_colour_pattern`,
   `::the_effect_union_names_thirty_eight_verbs`; the export is 534011 bytes
   carrying `firework` 20 times.*
2. **The emission.** A test declares one firework and reads the emitted line off
   the build: it carries `LifeTime:20` for flight 1 (`30`, `40` for 2, 3),
   `FireworksItem`, `flight_duration:<f>b`, the five shape names as the
   component spells them, and colours as `[I;…]` packed integers equal to the
   `#rrggbb` values. `emit::build` walks every line it writes against
   `CommandTree::v1_21_11`, so reaching the assertion is the tree's verdict; two
   builds are byte-identical (ADR-0006). *Met —
   `crates/delvec/tests/v29_firework.rs::the_emitted_rocket_carries_the_component_the_page_names`,
   `::lifetime_is_written_per_flight`,
   `::every_shape_and_every_optional_field_reaches_the_component`,
   `::two_builds_are_byte_identical`.*
3. **The colour rule.** `#ffd700` emits `16766720` and a malformed colour string
   is `DW0100`, naming the schema's own pattern. The rule is one function,
   `dsl::color`, read by the validator, the firework emitter and the potion
   bottle's `custom_color`. *Met —
   `crates/dsl/tests/v29_firework.rs::a_colour_packs_to_what_vanilla_stores`,
   `::a_malformed_colour_is_dw0100`.*
4. **The roof.** A firework under a solid cell is `DW0899` naming that cell; the
   same mark under an open column of eight is green; flight 2 over an open eight
   is refused naming eighteen. *Met —
   `crates/delvec/tests/v29_firework.rs::the_roof_rule_reads_the_column_the_flight_needs`
   over `firework::judge`, and end to end through a real build by
   `::a_rocket_under_the_halls_ceiling_is_dw0899` (the hall's ceiling stands
   four cells over `anchor/exit`, which is the solid cell the refusal names) and
   its perturbation `::the_same_rocket_under_open_sky_is_green`.*
5. **The reach.** A body posted four cells from a firework's burst cell under
   open sky is `DW0899` naming the post; six cells away is green; the
   enumeration of posts is `DW0511`'s own, asserted by adding a post class to
   one and reading it from the other. *Met —
   `crates/delvec/tests/v29_firework.rs::a_posted_body_in_reach_of_the_burst_is_dw0899`,
   `::a_wall_between_the_burst_and_the_post_is_not_credited`,
   `::a_post_on_the_launch_plane_is_never_in_reach`, and
   `::the_posts_are_dw0511s_own_enumeration`, which adds a respawn seat to a
   built campaign and reads the same number from `lethal::posted_places` and
   from the firework gate. §6 records why the first three are asked of the rule
   rather than of a campaign.*
6. **The binding line.** Every build that assembles a world prints §5's line;
   the valley-site overlay reports `F = 1`, `H = 8`. *Met — the overlay's build
   prints `firework binding: 1 firework(s) declared, 1 burst column(s) checked
   to 8 cell(s), 3 post(s) within reach examined, 0 refused`; the primary, which
   declares none, prints the same line with zeroes.
   `crates/delvec/tests/v29_firework.rs::the_binding_line_states_what_was_examined`
   pins both texts.*
7. **The pinned facts.** One test names the three wiki pages and asserts the
   constants read from them — five shapes, the `LifeTime` formula's fixed term
   `(flight + 1) × 10`, the heights 8/18/32, the five-block radius, the
   `FireworksItem` key — so a re-pin is one diff in `crates/dsl/src/firework.rs`.
   *Met — `crates/dsl/tests/v29_firework.rs::the_pinned_facts_are_what_the_pages_say`.*
8. **The live half.** The demo level's PackTest asserts a rocket entity with the
   component exists on the tick after the effect fires; the level's generation
   record states the measured burst height per flight; if it differs from §3.3
   the constant of criterion 7 is changed and this section is cited as
   superseded on that number. *Not yet due — the demo level is queued
   (criterion 11), and until it is built the heights 8/18/32 are cited, never
   measured. Recorded here so the gap is read rather than assumed.*
9. **The gallery.** §6's element builds green on the valley-site overlay;
   perturbing a colour moves the `summon` line; the probe is refused at build
   with `DW0899`; `tools/check-gallery-coverage.py` reports 0 units in neither
   state. *Met — the coverage gate reports 899 units enumerated, 895 bound, 4
   refusal-proven, 0 in neither state, and 36 probes refused with the code they
   name, `a-rocket-under-a-roof` among them. The perturbation is the emitted
   line: the overlay's colours reach the `summon` as `colors:[I;16766720,…]`,
   so changing one changes the byte.*
10. **The record and the skill.** The rows and page of §6, in the pull request
    that lands the code. *Met — `docs/reference/compiler.md` carries the surface
    row, the emission row beside `play-sound`'s, the `DW0899` section and the
    binding paragraph; `references/quest-capabilities.md` carries the creator's
    paragraph under *Things that change the world*, stating the behaviour
    without the code, because the skill page pins an engine that does not yet
    declare `DW0899` and `tools/check-skill-page.py` holds it to that pin.*
11. A demo-level row is queued when the code lands. *Met — **The Bell and the
    Sky** in `docs/demo-levels.md`.*

## 9. Decisions for the owner

- A firework is **one effect, `firework`, at a mark**, with the game's five
  shapes, colours as `#rrggbb`, and flight 1–3 — the alternative is a display
  verb with its own timing, refused because `sequence` already is one.
- The burst height is **fixed by the emitter** at the lowest the game would
  choose — the alternative is leaving `LifeTime` to the game, which makes the
  height random and the safety proof meaningless.
- A firework **under a roof, or within five blocks of a posted body, is
  refused** (one new DW code) — the alternative is an advisory, which would
  ship a burst that hurts the guard it was fired over.
- Players at the burst's level are **not proved safe**, and the record says
  so — the alternative is a rule about where players might stand, which no
  static model can state.
- The verb takes the **mark type spec-0066 introduces**; if this lands first
  it carries `{anchor, offset}` in the camera types' spelling — either way one
  type.
- **`dsl_version` moves**; no ADR.
