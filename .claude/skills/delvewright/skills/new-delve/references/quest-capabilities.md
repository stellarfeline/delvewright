## Contents

- [Reference: what a quest can do](#reference-what-a-quest-can-do)
- [Objectives](#objectives)
- [Items, containers and loot](#items-containers-and-loot)
- [State, flags and gates](#state-flags-and-gates)
- [The story layer](#the-story-layer)
- [Dialogue](#dialogue)
- [Things that change the world](#things-that-change-the-world)
- [Danger, death and money](#danger-death-and-money)
- [Bodies](#bodies)
- [Sealed things, and pacing](#sealed-things-and-pacing)

# Reference: what a quest can do

The verbs, effects and fields available in `quests.json` and `dialogue.json`.
Get the exact shapes from `delvec schema --stage quests` and `--stage dialogue`;
this section is what they are *for* and the traps in each.

## Objectives

- **Author `title` and `hint` for every non-`talk-to` objective.** `title` is a
  short player-facing name ("Unbar the Deep Gate"); `hint` is one line of
  location or direction guidance ("Past the entrance hall, take the left passage
  to the barred door"). The compiler surfaces them in-game when the objective
  activates, in chat and with a sound; without them the player gets no guidance
  and cannot find interact/collect/reach targets. For `talk-to` they are
  **required whenever the target NPC is not already visible from where the
  previous objective completed** — a different room, down a corridor, across an
  area. Read the "may omit" allowance narrowly: an off-screen NPC 60 blocks away
  through an unfamiliar cave leaves the player with nothing.
- **Hint wording**: landmark-relative directions from places the player already
  knows — the entrance hall, the gate, a named NPC. Never room-shape jargon
  ("corner room", "L-shaped hall") and never solver-internal terms (anchor,
  piece or socket ids).
- **A plate is stepped on, never clicked.** An `interact` completes on a
  right-click, so a `prop` that is a pressure plate or a tripwire is refused
  (`DW0957`). When the step is the act, write a trigger on the plate's anchor —
  `{"on": {"on": "step"}}`, whose anchor cell must hold a plate or a tripwire
  (`DW0917`) — and add `"audience": "presser"` when the beat answers the one
  player who stepped on it.
- **`interact.requires_item` is HELD, not carried**: the player must have the
  item in their **main hand** when they click — presenting it is the action.
  Author `missing_item_hint` whenever the empty-handed click deserves diegetic
  feedback (a sleeping giant mumbles in its sleep; a locked door rattles and
  holds). It is narrated in chat to that player, only while the objective is
  open, and without it the click is met with total silence, which reads as a
  broken affordance.

## Items, containers and loot

- **Furnish the containers.** A prefab's chests and barrels are empty until a
  `loot[]` entry fills them (`{id, anchor, items:[{item, count?, name?,
  enchantments?}]}`), and an empty chest reads as a bug to the player. Give
  every reachable container consumables or props; a named `name` is how a prop
  becomes a story object. The container must already exist in the piece at that
  anchor — **the compiler fills furniture, it never places it** (`DW0431`).
  Elites and set-piece actors take `equipment` in the same shape wave mobs use,
  enchantments included.
- **A handed-over stack is described the same way.** `give-item {item, count,
  name?, carrier?, enchantments?}` — so a shop offer can sell an iron sword with
  `"enchantments": {"minecraft:sharpness": 2}`, and a quest can hand over a
  `minecraft:enchanted_book` with `"enchantments": {"minecraft:mending": 1}`.
  Write a book's enchantments in the same `enchantments` field: the compiler
  stores them on the book (`stored_enchantments`), which is what an anvil
  applies, and the same holds for a book in a `loot` chest. Ids and levels are
  checked as everywhere else (`DW0433`, `DW0434`); a level above the survival
  maximum is allowed.
- **`equipment` has the game's eight slots, and a body shows only some of
  them.** `head`, `chest`, `legs`, `feet`, `main_hand`, `off_hand`, `body`
  (horse armour, wolf armour, a llama's carpet, a nautilus's armour, a happy
  ghast's harness) and `saddle`; each takes a bare item id or `{item,
  enchantments}`. The server keeps whatever you write on any living body, but
  the player sees a piece only where that body's model draws it, so a piece is
  refused where the game would not show it on that body (`DW0898`): a slot the
  body does not draw (a chestplate on a horse, a sword in a creeper's hand, a
  helmet on a villager, whose head takes a pumpkin or a skull but not armour),
  an item in a slot other than its own (a helmet in `legs`; the hands take
  anything), or an item the body may not wear (a saddle on a zombie, horse
  armour on a skeleton horse). The refusal lists what the body does draw. A
  held weapon wanted only for its damage is written as `attributes`, which apply
  whether or not anything is drawn. `drops[].slot` takes the same eight names.
- **A `collect` has three shapes, and which are available to you depends on the
  library.** Give the item an `item_name` ("Cheese", "Tide Ledger") in all
  three: it is what the player reads on the stack, it translates like every
  other player-visible string, and an unnamed generic item says nothing about
  what the quest asked for.
  1. **`container: <anchor>`** — adopt a chest or barrel **the piece already
     placed**. The compiler fills that container and places nothing of its own;
     a floating chest conjured beside the barrel the player has been walking
     past is the defect this avoids. Set `fill_count` so the container reads
     plausibly full — it counts padding SLOTS after the objective's own stack,
     and `1 + fill_count` must fit the container's 27. The container must really
     be there in the piece (`DW0438`) and must not also be filled by a `loot`
     entry or another `collect` (`DW0435`).

     **Few shipped pieces stand an anchor on a container**, and an anchor's
     name is no evidence of one: **two pieces declare an anchor literally named
     `anchor/chest` whose cell is air** (`cave-room-small`, `keep-room-small-a`)
     — the obvious thing to reach for, and it is `DW0438` every time. In
     `pool/stone-keep` there is no piece that can carry a `container` at all.
     Check rather than assume; the library moves.
  2. **`dropped_by: <wave>`** — the item comes off a body instead of out of a
     box. The compiler places no container and PROVES the chain: that the wave
     really yields the item (`DW0492`) and that its `kill` objective runs first
     (`DW0493`). `dropped_by` names a wave, never an actor — an actor's death is
     observable by no objective. This is the right shape whenever the story can
     carry it, and it needs nothing from the piece library.
  3. **Neither** — the compiler places its own chest at the objective's anchor.
     Legitimate when the room genuinely has no furniture, and the thing to avoid
     is a conjured chest standing *beside* a container the room already has.
  4. If the beat really needs a piece with a container in it and none exists,
     that is a piece to make: *Reference: when the prefab library has no piece
     you need*.
- **An elite or boss leaves ONE thing behind, and you say which.** Give the
  fight's `drops[]` a declared subset — a `{"slot": "main_hand"}` for the axe
  the player watched swing, or a `{"item": …, "name": …}` for a quest token —
  never the whole kit. Only an `elite`/`boss` encounter may declare drops
  (`DW0491`); a slot must be one the same body's `equipment` really fills
  (`DW0490`).

## State, flags and gates

- **A number the world remembers is `state`, not a flag.** A flag is boolean,
  party-wide and one-way — nothing clears one — so it says "this happened" and
  nothing else. When a beat needs a *quantity* that goes down as well as up (a
  toll still owed, a floor a lift is at, whether a ride is in progress), declare
  it in the `state` list: `{"id": "state/<kebab>", "scope": "party" | "player",
  "initial": <n>, "note": "<what the number means>"}`. `scope` is required and
  never guessed — `party` is one shared value, `player` gives each member their
  own. Write it with `set-state` / `add-state` (signed: a negative `amount`
  counts down) / `clear-state` (back to `initial`).
- **Read it in the gate, never in a verb.** `requires_state: [{"state": …,
  "op": "equals"|"not-equals"|"at-least"|"at-most", "value": <n>}]` is accepted
  everywhere `requires_flags` is — an objective, any gatable effect, a trigger,
  a trap, a dialogue option, a cast placement — so "the door opens at zero" and
  "this line is withheld below two" are the same construct.
- **Every datum must be both written somewhere and read somewhere**: a gate
  reading a datum nothing writes is `DW0501`, and a datum no gate reads is
  `DW0502`. Both mean the mechanism is decoration.
- A `player`-scoped datum can only be touched where a player is acting — a
  dialogue option, a cast placement, an `on_death` beat, an effect on a quest
  beat a player completes, or a trigger declaring `audience: "presser"`, which
  runs as the player who clicked (`use`) or stepped on (`step`). These have **no** acting player and reject one
  (`DW0503`): an objective/trigger/trap *gate*, a party-audience trigger's
  `effects`, a trap's `payload`, a shortcut's `on_unlock`, a `move-npc`/
  `move-actor` `on_arrive`, a `bonfire`'s `on_rest`, and a `sequence` step of a
  timeline started in one of those. A timeline started where a player acted
  keeps that player through every step. Use `party` scope there.

## The story layer

- **Every story node declares a `happening`.** One line saying what the node
  does to the story: `{verb, text, subject?}`, where `verb` is one of `dies` /
  `survives` / `departs` / `arrives` / `learns` / `believes` / `gains` / `loses`
  / `opens` / `seals`. Required on every quest, every objective, every staging /
  wave / gate / `campaign-complete` effect, and every dialogue option that sets a
  flag — a missing one is `DW0481`. It is the event-flow twin of the cast
  ledger's `doing`: you cannot fill it without deciding what the beat *is*. Keep
  `subject` accurate (`npc/…`, `actor/…`, `wave/…`, `anchor/…`, or an `item/…`
  label) — the compiler reads only the verb and the subject, and uses them to
  catch a dead character who later acts, or a sealed gate later walked through,
  per branch (`DW0485`).

  `happening` belongs to the objects named above, and only those. It is not a
  field on an arbitrary effect: putting one on a quest-level `set-flag` is
  `DW0100`, and the refusal enumerates what that object does take.
- **The `cast` block, first in every quest.** Every quest declares, for every
  NPC live in it, `{at, doing, dialogue}`. `at` is an anchor, a mark
  `{"anchor": …, "offset": [x, y, z]}` — the spelling for a body that stands at
  an offset, which must spell the same offset the body stands at — or
  `"offstage"` / `"dead"`, which must match a real `despawn-npc`. Declaring a
  position does not move anybody (`DW0461`). `doing` is free prose and is the point: you
  cannot fill it without deciding the character's business in this beat, and the
  dialogue stage writes their lines against it. `dialogue` is a dialogue root
  id, `{"barks": [...]}`, `"unchanged"`, or `"none"`.
  - **A sleeping, working or background NPC gets a `barks` pool**, not
    `"none"`. Right-click then yields one inconsequential in-character line
    instead of dead silence. Use `"none"` only when the silence is the
    statement.
  - **Write `"unchanged"` when you are deliberately carrying dialogue forward**
    — never re-spell the same root id, and never omit `dialogue` hoping it
    defaults (it does not: `DW0463`). `"unchanged"` at an NPC's first
    appearance is `DW0466`.
  - **Treat the `DW0467` staleness warning as a design smell, not a nuisance.**
    It means an NPC's right-click never learns that the story moved. Give it a
    scene that changes, or make it a bark-pool background character — do not
    silence it by shuffling spellings.
  - Omitting a live NPC is `DW0460`: an unaccounted NPC is how a crew member
    ends up standing forgotten in an alcove while the player escapes.
- **Post-fork casts are per branch, every quest.** After a fork opens, an NPC
  whose situation differs by branch declares a **list** of placements, each
  gated by the flags of the branch it belongs to — in *every* later quest, not
  just the first. Leaving one ungated as a fallback is `DW0483`: later
  declarations win, so the fallback keeps governing the branch that already has
  its own, and the fork moves the ledger without moving the bodies.

## Dialogue

- **`button = caption, tooltip = the full line.`** When the caption cannot carry
  what the character actually says — the wine beat, where "Pour it out." stands
  for a whole sentence — author the option's optional `tooltip`: vanilla shows it
  in a hover box beside the button. It **wraps** (no `DW0331`, no width budget),
  so it takes a full sentence. Use it for the *said line*, not for hints or
  mechanics; the button still has to be readable on its own, since a player on a
  controller or reading fast never hovers. It translates under its own key.
- **Every dialog button takes the same optional `tooltip`**: a dialogue option's
  `tooltip`, a shop offer's `tooltip`, and a bonfire's `rest_tooltip` /
  `save_tooltip` (beside `rest_label` / `save_label`). A class button's tooltip is
  its required `blurb`. Each is optional, wraps, and translates under its own key;
  state one only when the button's caption cannot say what pressing it does.
- **Premise and exposition options must retire once their moment passes**, via
  the cast ledger's dialogue swap (declare a later root) or a flag gate. A "who
  are we" / "what is that thing" option must be **impossible** at the finale.
  The defect this prevents: after a climactic escape, a crew NPC still offering
  "Tell me what he is." and "Is there another way out?" — questions the
  character has already lived through the answers to.
- Flavour NPCs get real trees too.

## Things that change the world

- **A region can be filled or cleared while the delve runs, and no gate need be
  involved.** `fill-region {region{anchor,extent}, block}` writes a block over a
  declared box; `clear-region {region{anchor,extent}}` empties it.
  `open-gate`/`close-gate` are the same operation with the box and the block
  read off a prefab gate anchor — reach for those when a prefab already declares
  the threshold, and for these when the box is yours: a bridge that
  materialises, a floor that sinks, a wall that opens, a platform summoned under
  the party. The completability proof honours them from the point in the quest
  graph where the effect fires: a fill the only route must cross afterwards
  fails the build (`DW0311`), and a clear is credited as passable, so a route may
  legitimately depend on one. **Two things it will not model, so do not build on
  them**: a clear that opens a box into water (the water flows back in and the
  proof does not know), and a clear over rubble another mechanism dropped there
  (a `collapse` debris field, a shut timed gate) — those stay solid.
- **A piece can ship BROKEN and have the campaign repair it.** A zone whose
  spatial contract declares a `way` on one of its edges is severed there as
  built — a stair whose treads are missing, a bridge that is not down, a doorway
  packed with rubble — and `open-way {piece, way}` is what opens it:
  `{ "type": "open-way", "piece": "prefab/z7-bell-tower", "way": "broken-flight" }`.
  This is how a beat *changes the building*: the collapsed stair the party
  rebuilds, the drawbridge the lever lowers, the shortcut opened once from the
  far side and open forever. **You write the reference and nothing else** — the
  cells, the block and the direction all come from the piece's own metadata, so
  there is no geometry to get wrong and no second place for it to drift. Two
  rules follow, both build errors rather than surprises in play: the piece must
  be placed exactly once for the reference to name it (`DW0547`), and anything
  the party is required to reach beyond the break — an objective anchor, an
  NPC's stand, a wave's spawn — needs a **forced** `open-way` on an objective the
  quest graph puts *before* it (`DW0548`). A way nothing opens is fine and stays
  shut: a door that never opens is content. Every staged way's fate is listed in
  `validation/ways.json` after a build.
- **A prefab's gate is SHUT until the campaign opens it.** A gate anchor's
  region holds whatever the prefab authors there — `hello-room`'s doorway is
  iron bars, the island's cave mouth is air — and the compiler measures which. If
  anything the party must reach lies past a barred gate, some objective they are
  **forced** to complete has to `open-gate` it first, or the build fails naming
  the anchor (`DW0317`). "Forced" excludes every optional bundle: a trap payload,
  `on_death`, a shop offer, a shortcut's far-side unlock. To spell "the party
  walks up to this and the door opens", use an environment `trigger` — that one
  counts.
- **A firework is one effect at a mark.** `firework {at {anchor, offset?},
  flight?, explosions}` fires one rocket where the campaign says — over the gate
  when the guard is drawn up, over the court when the bell is rung — beside the
  `play-sound` that goes with it. `explosions` is one to seven bursts of
  `{shape, colors, fade_colors?, trail?, twinkle?}`; `shape` is one of the
  game's five (`small_ball`, `large_ball`, `star`, `creeper`, `burst`) and every
  colour is a `#rrggbb` literal, the spelling a potion's `color` uses.
  `flight` is 1, 2 or 3 — the three the game crafts — and decides how high the
  rocket goes: the burst stands **8, 18 or 32 blocks over the mark**, a stated
  number rather than a roll, because the compiler writes the rocket's lifetime
  itself. So the mark needs that much sky: a rocket fired indoors bursts against
  the ceiling and the build refuses it, naming the cell that stopped it. It is
  also refused when the burst lands within five blocks of a place the campaign
  posts a body — an NPC's stand, an actor's post, a checkpoint seat, a wave's
  seat — because the burst hurts what it reaches. **Players are not posted, and
  are not proved safe**: a party standing on a wall walk level with a burst over
  the court takes up to 19 HP from a seven-star rocket, never a full body's
  twenty, and that is a hazard they can see coming — judge it in playtest. A
  display of many rockets is a `sequence` of `firework` effects, not one
  overloaded rocket.
- **A perception beat is a `sequence` of `give-effect`, `particle` and
  `play-sound`** — the screen darkens and swims, a face fills it, something
  sounds from behind. `particle {particle, at, count?, spread?, speed?}` spawns a
  vanilla particle at a mark (`{anchor, offset?}`) or at `"players"` — each
  addressed player; the full-screen face is `"particle":
  "minecraft:elder_guardian", "at": "players"`. Only particle types a bare id
  spawns are admitted (not `dust`, `block`, `item`, …). A sound behind the
  listener is `"at": {"at": "players", "offset": [0, 0, -3]}` — the offset is in
  the listener's own frame, `+z` the way they face. Three rules for the beat:
  **it is never the only signal** —
  each of these effects sits behind a player setting the engine cannot read
  (Distortion Effects, Darkness Pulsing, the sound sliders), so a change a
  player must act on is also told by a `narrate`, a changed block or a gate;
  write **`hide_particles: true`** on every grant, so a blinding reads as the
  world rather than a potion; and know what the engine fixed for you — every
  particle is written in force mode, every sound in the `master` category, and
  every grant ends by its duration. **A blinding near a killing volume or a
  deadly drop is refused**: a blinded player cannot see the hazard beside them.
  Shorten `seconds`, draw `in` farther from the hazard, or use
  `minecraft:nausea`, which leaves the floor visible. A grant of night vision,
  blindness or darkness that ends while a cutscene beside it in the same
  timeline or bundle is still rolling is refused too — give it the seconds the
  error names. Two things to plan around: a second grant of an effect the
  player already has shows nothing unless it is stronger or longer, so a beat
  re-armed inside its own length re-applies nothing visible; and a timeline
  started again before it ends re-times every step for both players.
- **A teleport selects a REGION, never a block.** `teleport {from {anchor,
  extent}, to {anchor, offset?}}` moves **everything** inside the box to the
  destination mark —
  players and entities alike, which is what makes a cargo platform the same
  mechanism as a passenger one. Nothing is exempt, so do not draw the volume
  over an affordance the engine anchors to a block (an interact objective, a
  click trigger, a bonfire, a shortcut lever, a disarm, a sealed gate): the
  hitbox would ride and the hardware would stay, and the build refuses it
  (`DW0542`). Two things to design AROUND rather than against, both measured on
  the pinned server: **a teleport is not a rescue** — accumulated fall distance
  carries across it unchanged and is charged in full at the destination, so a
  platform arriving under a falling player past ~20 blocks is the surface they
  die on; and **the proof leans only on a link** (next item) — every other
  teleport is a gather, so a route that exists only through one fails the
  completability proof.
- **A crossing inside one area is a link — mandatory shape (spec-0083).** When
  the only way onward between two places of ONE area is a carry (a boat at each
  shore, a lift, a portal), author it as:
  1. an **objective** that puts the party in the carry's volume (*board the
     boat* — a `reach` on the hull) whose completion sets a flag;
  2. a **`triggers[]` entry declared `"once": false`** (`use`, `strike` or
     `approach`), gated on that flag, hosting the `teleport` — so a straggler
     left on the jetty presses it again and follows. A teleport in an
     objective's or quest's bundle, a `once` trigger, a trap payload, an offer,
     a death bundle or a shortcut is a **gather**: the first body through it
     travels, the rest are left behind, and the proof refuses the leg (`DW0311`,
     naming that teleport and this remedy);
  3. the trigger's body (a lever, a tiller) **just outside the volume and
     within a strike of a cell inside it** — the party stands inside to press
     it. The build refuses a link with no such cell, a `to` inside the volume,
     a `to` with no floor at the teleport's tick, or a `to` in another area
     (that is a crossing — put the next beat there instead);
  4. **cutscene first, teleport after**: a crossing played as a cutscene is one
     `sequence` — the `cutscene` at tick 0, the `teleport` at a tick past the
     cutscene's end (the refusal names the first tick that holds; a teleport
     under the open bracket is undone when the camera returns);
  5. a `to` at least **8 blocks** (horizontally) from the volume, or the bot
     cannot see the carry happen (the harness refuses the path);
  6. with a layout graph, a **`carry` edge** between the two places, gated on
     the same flag, each direction it allows realised by a link — the build
     refuses either without the other; the volume's anchor a station of the
     near place and `to`'s anchor a station of the far one.
  The build then takes the link only where a walk fails, splices the press
  into `critical-path.json` (`stand`, `transport`) and counts it on the
  `DW0311 binding:` line (`N carried by a link`) — read that line to confirm.

## Danger, death and money

- **A trap is a trigger plus a payload, and the payload is ordinary effects.**
  `traps[] {id, at, trigger, payload, lethality?, disarm?, reset?, + the
  ordinary gate}`. `trigger` is `pressure-plate` / `tripwire` /
  `trapped-chest` — the one job redstone keeps, because it is the part the
  player sees, suspects and learns to read. The consequence is `payload`: an
  ordered effect list in the **same vocabulary a quest uses**, plus two trap
  verbs — `volley {from_anchor, kill_zone, projectile?, salvos?, interval?}`,
  which blankets every standable cell of its zone rather than sniping (escaping
  is a decision to leave, never a lucky strafe; the line of fire to every cell
  is proven at build time, `DW0442`), and `collapse {region_anchor,
  falling_block?, then_floor?}`, which deletes a region and buries whoever is
  under it, with the settled world re-run through the completability proof
  (`DW0445`). A trap that declares no consequence at all is `DW0440`.
- **`at` is an ORDINARY POINT ANCHOR, and its cell holds the trigger block.**
  The compiler owns the detection tick for a command payload: a plate or a
  tripwire is a position test on that anchor's cell, and a trapped chest is the
  same interaction-entity `use` a disarm lever rides. The block the `trigger`
  names — a `*_pressure_plate`, a `minecraft:tripwire`, a
  `minecraft:trapped_chest` — is the piece's, standing in that cell: the party
  has to see what springs, and the build refuses a trap whose cell holds
  anything else, a plain chest included. Place it with the piece (a detail
  program, an edit batch, or a piece of your own); never a runtime `set-block`.
  So any anchor carrying a `pos` that some area's prefab provides, with the
  trigger block in its cell, will hold a trap — and on a site-plan
  campaign that is `anchor/node-<place>`, which is exactly what a derived map
  has. `volley`'s `from_anchor` is another one, and all it owes is a clear cell.
  **The check does not read the name `anchor/trap`**: what is required is a point anchor that resolves, not one
  called that. Read any text telling you to "bind the trap to an `anchor/trap`
  marker" — the refusal's remedy line and the schema's own field description
  both say it — as saying *that anchor does not resolve*, and answer it with an
  anchor that does. **No shipped piece declares an `anchor/trap`**, or any
  anchor carrying `dispenser` or `trigger_block` metadata — so the piece that
  reading asks for does not exist here, and a command payload never wanted one:
  it wants the trigger block in an ordinary anchor's cell.
- **Two things do want hardware in the piece, and neither of them is the
  payload.** The dispenser `effect: {dispense: {…}}` fills a dispenser socket
  the prefab pre-wired, so it needs that metadata; and a trap carrying a gate
  (`requires_flags` / `forbids_flags` / `requires_state`) is gated *physically* —
  the compiler takes the trigger block out of the world while the gate is shut
  and puts it back verbatim — so it needs the anchor's declared `trigger_block`,
  and a `trapped-chest` can never be gated because removal would destroy its
  inventory (`DW0363`). Against this library neither is authorable at all. Write
  an ungated `payload` trap, and gate the story beat that arms it instead.
- **A place that kills is DECLARED, never faked with the art.** A cliff whose
  fall must be fatal, a lava pit, an acid pool, an out-of-bounds plane: all one
  declaration, `lethal_volumes[] {id, region{anchor,extent}, message,
  damage_type?}`. Never obtain the behaviour by changing the world instead —
  making the horizon `void` so the fall kills serves exactly one fiction and is
  never the move here. `message` is REQUIRED and is what the player reads as
  they die (blank is `DW0512`); `damage_type` words vanilla's own broadcast
  (`fall`, `on_fire`, …). The volume is geometry the completability proof
  honours: if the party's only route to an objective crosses it the build fails
  naming the volume (`DW0510`), and nothing the campaign POSTS — the entry spawn,
  a checkpoint, a bonfire, an NPC's anchor, a `cast` placement, an actor — may
  sit inside one (`DW0511`). Put the volume where a player can SEE what will
  happen before they commit to it; a killing box nobody can read is 初见杀 with
  no lesson in it.
- **A volume kills your own waves too, so seat no wave where it can walk into
  one.** A member that can get into a lethal volume by walking, stepping or
  jumping up one block (a floor lantern is a step), climbing onto a wall or curb
  from something higher, dropping off an edge or sinking in water, within its
  stack's follow range of its seat, drowns itself before the party arrives;
  mobs make no gap jumps. A fence gate, door or trapdoor the party can open
  counts as open, because a player who opens one may leave it open and die.
  Repair by placement: seat the wave elsewhere, put the volume behind a rise of
  two blocks, or replace the gate with a jump across a dry cut, which the party
  makes and a mob does not. Never ring the hazard with blocks nobody can see.
- **A fight the party must win stays where the party can strike it.** A body a
  `kill` objective waits on is knocked by every blow; on a shelf with an open
  edge it goes over, and if it survives the fall somewhere the party cannot
  follow — below the playable region's edge, down a drop with no way back —
  the objective waits forever. Close the edge with a parapet a body is knocked
  into and not over (a wall course, 1.5 high), give the party a way down, or
  seat the fight away from the drop. Never fence the shelf with blocks nobody
  can see, and never shrink `follow_range` to hide the placement.
- **What happens when a player dies is content, not engine behaviour.** The
  quests document takes a campaign-wide `on_death`: a bundle of ordinary effects
  that runs at the moment a player dies, for that player. One per campaign — it
  is not a field on a checkpoint, because dying is true everywhere in the delve;
  put `requires_flags`/`forbids_flags` on the effects inside it if the beat
  should only land in some phase of the story. Do NOT write a death beat the
  mainline depends on: nothing inside it is credited as a flag producer,
  deliberately, so a door it alone opens is a door only a corpse can open.
- **A death that costs something leaves a stake, and the engine decides where.**
  `stakes[] {id, state, forfeit?, max_live?, on_full?, collect_by?,
  collected_message, marker_item?}`, dropped by a `drop-stake` effect in
  `on_death`. The datum must be `player`-scoped (`DW0520`) — a stake is one
  player's wager, never the party's. You do **not** choose where it lands: the
  compiler computes the point, on the walkable way back from the respawn point
  in force, nearest to where they died, so a death in a lethal volume leaves its
  stake at the near lip rather than inside the hazard, and a death on a lift car
  leaves it on solid ground. If your geometry can strand one — a one-way drop
  with no shortcut back — the build fails naming the place (`DW0525`), and the
  fix is a route back or a `lethal_volume`, never deleting the stake. Souls
  behaviour is `max_live: 1, on_full: "replace"`; no death cost at all is
  `max_live: 0`; a memorial at every death site is a larger `max_live` with
  `on_full: "keep"`.
- **A currency is a NAMED datum, and a price is a GATE.** There is no
  `currencies` section and no `price` field, on purpose. Give a `state[]` datum
  a `name` and it becomes a purse the player reads: the engine states
  `<name>: <value>` on that player's action bar on every write, translated like
  any other line. A shop is `shops[] {id, anchor, title, marker_item?,
  offers[{label, tooltip?, effects[], + the ordinary gate}]}`, and its prices are
  `requires_state` comparisons — exactly the ones a door or a dialogue line
  would use. **Write the refusal yourself**: put the purchase behind `at-least
  <price>` and an apology `narrate` behind `at-most <price − 1>`, both as gated
  effects of the same offer, so a player who cannot afford something is told
  rather than left pressing a dead button. An offer with no effects at all is
  `DW0523`. **Order matters and the compiler will tell you (`DW0527`)**: put the
  refusal and any confirmation BEFORE the debit. Sibling effects are consecutive
  commands, so a gate written after the debit reads the balance the debit just
  produced — buy your last coin and you are charged and apologised to in the same
  breath.
- **A purse can STAND on screen.** Give the named datum `"display": "sidebar"`
  and its balance stands on the right of every player's screen at every moment,
  headed by its `name`, one line per player showing that player's own balance —
  the party reads who can afford what. Without it the purse is spoken on the
  action bar only when it changes, and once the line fades nobody can check it
  without spending or earning. One datum per campaign may stand (the sidebar
  holds one objective), it must carry a `name`, and it must be `player`-scoped —
  a `party` purse cannot stand, because the sidebar hides its holder. Each of
  those is refused at `delvec validate` as `DW0919`, naming the datum and the fix.
- **A fight pays per body through `on_kill`.** `on_kill {fires?, effects[]}` is
  **optional** on any `waves[]` or `actors[]` entry. Its effects run **once per
  body a player is credited with killing**, as that player: a `player`-scoped
  datum written there pays the killer, and a `party`-scoped datum pays the party
  once. A body that dies with no player credited — a fall, a hazard, another mob
  — pays nothing, and the fight still clears. Where the fight **comes back**
  after the party has met it — a rest re-seats it, or the beat that seats it can
  fire again — you **must** say `fires: "first-kill"` (the fight pays as many
  kills as it has bodies, once, over the whole delve) or `fires: "every-kill"`
  (every kill pays, so the fight can be farmed); where it does not come back,
  leave `fires` off. Who is paid is the datum's scope, never a field on the
  bundle:

  | you want | write | with four players |
  |---|---|---|
  | the one who lands the blow keeps it | `add-state` on a `player` datum | the killer's purse moves; the others' do not |
  | a shared purse | `add-state` on a `party` datum | one write to the party per kill |
  | every player paid per kill, each into their own purse | — | not a shape this bundle has; pay a `party` datum and price from it |

  The engine refuses a bundle no player can be credited with — on a wave no
  beat spawns, or on an actor never unleashed and not `vulnerable` — and it
  refuses `every-kill` on a fight that never comes back and a missing `fires`
  on one that does, naming the fight each time. The bundle of the last body fires before the `kill` objective's
  `on_objective_complete`, so a beat for the whole fight falling stays on the
  objective.

## Bodies

- **A body stands at a mark: an anchor plus an optional `offset`.** `"anchor":
  "anchor/muster", "offset": [0, 0, -2]` on an NPC or an actor stands it two
  blocks north of the anchor's cell. A rank of six men-at-arms is one anchor and
  six offsets — never six anchors, and never six bodies on one anchor
  (`DW0896`). A walk's destination takes the same object: `move-npc` /
  `move-actor` write `"to": {"anchor": "anchor/rank", "offset": [0, 0, -6]}`, so
  a rank is formed by walking as well as spawned in formation. Every offset stays
  inside the piece its anchor belongs to, and the build refuses one that leaves
  it, naming the cell it reaches and the box it left — an offset says where
  beside a place, never which place; to stand a body in another room, name that
  room's anchor.
- **`base_entity` accepts any entity id, and NPCs are inert by construction.**
  Every NPC is summoned `NoAI,Invulnerable,Silent,NoGravity,PersistenceRequired`
  plus a separate interaction hitbox, and there is no registry validation on the
  field — so any mob id becomes a talking statue that cannot move or hurt
  anyone. This is how a villager-sized cast can include a giant: e.g.
  `minecraft:warden` as a Cyclops you must slip past. *Caveat:* `Silent:1b` also
  suppresses that entity's ambient sounds (the Warden's heartbeat), and the
  emitted `VillagerData` tag is inert on a non-villager.
- **A body that moves unlike its species DECLARES it, and the build holds it to
  the claim.** `traversal { locomotion: ground|climber|flier }` on an NPC or an
  actor. By default the compiler reads locomotion off the entity id — spiders
  climb, ghasts fly, everything else walks and is checked — so a walked leg that
  goes OVER a wall line instead of round to its opening is `DW0453`. If that is
  your fiction (a sheep that climbs), declare it and the advisory is answered. It
  is **not** an off switch: a declaration that changes no verdict is refused
  (`DW0454`), so you may only claim a climber where the route really climbs;
  `aquatic` is refused outright (`DW0455`) because nothing in the model could
  hold a body to it; and no declaration touches the error tier — a declared
  climber still cannot walk through a closed fence gate (`DW0452`). Declare it on
  the body, never on the beat.
- **What a body can wear depends on what it is.** Dress it from this table;
  anything else is refused.

  | Body | Wears |
  |---|---|
  | zombie, husk, drowned, zombie villager, skeleton, stray, bogged, parched, wither skeleton, piglin, piglin brute, zombified piglin, player-shaped mannequin (any skinned body), armor stand | armour in `head`/`chest`/`legs`/`feet`, a pumpkin, skull or block in `head`, an elytra in `chest`, anything in both hands |
  | giant | armour in the four armour slots, anything in both hands |
  | evoker, illusioner, pillager, vindicator, copper golem | a pumpkin, skull or block in `head`; anything in both hands |
  | villager, wandering trader | a pumpkin, skull or block in `head`; anything in `main_hand` |
  | fox, dolphin, panda, witch | anything in `main_hand` |
  | allay, vex | anything in both hands |
  | horse, zombie horse, skeleton horse, nautilus, zombie nautilus | `body` and `saddle` |
  | camel, camel husk, donkey, mule, pig, strider | `saddle` |
  | wolf, llama, trader llama, happy ghast | `body` |
  | every other living body — creeper, spider, warden, enderman, iron golem, ghast among them | nothing |

  **Four bodies show their hand only while they fight or sit**: an evoker while
  casting, an illusioner while casting or aggressive, a vindicator while
  aggressive, a panda while sitting. A NoAI actor of those species shows its
  hand after it is unleashed, not before. **A horse is dressed through `body`
  and `saddle`**, and the item says which horse it fits: iron horse armour on a
  horse or a zombie horse, never a skeleton horse. **A mount is posted, not
  walked**: the router measures a horse, camel, donkey, mule, llama or strider as
  a person-sized body, so stage a mounted set piece on its mark and do not route
  it through doorways.
- **A body is posted where it is declared, and never walked onto furniture.**
  An NPC or actor anchored on a table stands on the table. A `move-npc` or
  `move-actor` routes round anything the piece declares as furniture, and a
  destination on it ends on the floor beside it. A walk whose only way is over
  the table is refused naming it (`DW0510`): move the mark or open a way round.
- **A status effect is a verb — and it ends by expiring, never by being
  cleared.** `give-effect {effect, seconds, amplifier?, hide_particles?}`
  grants any pinned-1.21.11 status effect; the effect's `in {anchor, extent}`
  narrows it to the players inside a box, so "blind whoever is riding" does not
  blind the delve. `seconds` is REQUIRED and there is no infinite form, on purpose: an
  effect whose only removal is a later step is one the player keeps forever
  whenever that step does not run — a logout, a crash, a death mid-chain. So **do
  not write "grant, then clear at the end"**; write a duration that covers the
  beat plus slack and let it expire. Pairing a live grant with a `clear-effect`
  of the same effect in the same bundle is `DW0540`. `clear-effect {effect?}`
  exists for effects the campaign did NOT grant (a potion the player
  drank, a `wither` a mob applied); omit `effect` to clear everything.
- **A fight can show its health: `health_bar`.** A health bar is
  **optional** on any wave or actor: `"health_bar": { "range": 16 }`, with
  `title`, `color` and `style` optional beside it. Declaring one draws a named
  bar over the fight's total health for every player within `range` blocks of a
  live body of the fight, from the moment they enter that range until the last
  body falls. The compiler **warns** — an advisory, never a failure — only when a
  fight billed `tier: "boss"` declares none, and says nothing about any other
  tier — whether an `elite` or `ordinary` fight shows a bar is yours to decide.
  - *The title is the fight's own name* unless you state `title`: an actor's
    `name`, or the `name` of a wave's one mob entry, translated under that
    name's own key. A wave of two entries or more, or a body with no name, has
    no single name to draw, so it must state `title`, or the build is refused.
  - *`range` is the size of the place the fight happens in*, in blocks (4–64),
    not the body's `follow_range`: the bar should appear as a player crosses
    into that place, before the body has noticed anyone. It disappears for a
    player who leaves the range, dies or watches a cutscene.
  - *Spell the shape with waves.* A named body with lesser bodies around it is
    two waves spawned by the same beat, the bar on the named body's wave only;
    a fight of several bodies (`count` above 1, or several entries) is one bar
    over their summed health, dropping as they fall; two fights are two waves
    with a bar each, drawn at once.
  - *On an actor the bar reads the body that can be hurt*: the unleashed body,
    or the puppet itself when the actor is `vulnerable`. A bar on an actor that
    is neither is refused — it would sit full forever.
  - `color` and `style` are the game's own boss-bar words (`red`, `purple`,
    `notched_6`, `notched_20`, …); anything else is refused, and the refusal
    lists the words the game accepts. Leave them out to keep the game's defaults.
  - A rest that re-seats the fight re-seats the bar with it: it reads full
    again when the party walks back in.
- **A thing that can be hit and hits back is an assembly, not a body.**
  `assemblies[]` declares an object built of display entities that stands at a
  mark — a limb rising from a pit, a statue that swings, a pendulum of chains —
  plays clips, can be struck, and strikes a player who comes near. It has no
  health and never dies, so it is never a fight: what striking it *does* is
  yours, written with the ordinary verbs.
  - *Its shape and motion come from a rig*, `"rig": "rig/<name>"`, a file a
    generator writes into the prefab library (`rigs/<name>/rig.json`). You never
    write keyframes. Read what a rig offers before you use it: the engine's
    rig describe verb prints its parts, each clip with its length in ticks, and
    the cells each clip's last frame stands in relative to the mark, turned the
    way your assembly faces. `initial` is the clip it plays from
    `spawn-assembly`; `play-clip {assembly, clip}` switches it; a `sequence`
    times the beat after a clip from the tick lengths `rig describe` prints;
    `despawn-assembly` removes it unseen. Naming a clip the rig lacks is refused
    with the rig's clips listed.
  - *It is struck in melee only.* Its `hitbox {width, height, offset?}` is an
    invisible box a left-click registers on; **an arrow passes straight through
    it**, so a ranged class cannot hit it and a party wants someone who fights
    up close. The box must cover the parts the player sees, at most 6 wide and
    22 tall, and when your critical path needs it struck, somewhere the party
    can stand must be within arm's reach (3 blocks) of it.
  - *A hit count is a `state` you declare*, not a field on the thing: a
    `strike-assembly` trigger with `"once": false` that does `add-state` one per
    blow, and the effects that happen at the count gated on `requires_state`
    (play `retract`, set a flag, open the way). Give the trigger a
    `forbids_flags` of the flag the last blow sets, so it stands down. When the
    path depends on the count, the bot strikes it as many times as the gate
    needs.
  - *Where a blow lands is checked; when and how hard is yours.* `strikes
    {while_in, pattern: [{windup, hold, strike, on_land}]}` winds up, holds,
    strikes, and runs `on_land` the moment the strike clip's last frame has
    finished drawing — usually a `damage-players` with an `in` box, and a sound.
    Three things are refused, all so the danger stays visible: **a blow that
    reaches a player who never stepped into the arming region** (keep every
    landing box inside `while_in`); **a blow on a cell the limb does not come
    down on** — within a block of the floor, where the player stands; a limb
    hanging over their head has not landed on them; and **a limb that comes
    down beyond the blow's area** — the area is what the animation shows, so a
    long limb's blow is a long box along where it lies, not one cell, with one
    cell of give round the box. Declare the box along the cells where the
    strike clip's last frame lies on the floor (rig describe prints them).
  - *The wind-up's length is two numbers, both yours*: the wind-up clip's
    frames times its cadence (rig describe prints the ticks), then `hold`.
    `ticks_per_frame` on a step plays its wind-up and strike at a pace you
    choose (1–20 ticks a keyframe) instead of the clip's own. How long the
    wind-up is, how long it holds and how much it hurts — down to no warning and
    a blow that kills — are your judgement; nothing refuses them. For
    reference, a single expected stimulus takes a person about a quarter of a
    second to answer (roughly five ticks of clear movement), and a wind-up that
    barely moves at first gives no warning at all however long it is.
  - *It can turn to strike.* `"aim": {"facings": 8}` in `strikes` turns the
    thing, at every wind-up, to whichever of 8 (or your number of) evenly spaced
    facings points nearest the closest player in `while_in`, and the blow's box
    turns with it. Write the box for the assembly's own `facing`; every facing a
    player in `while_in` can make it take is checked the same way, so keep
    `while_in` to the side the thing is meant to strike, and keep the
    `damage-players` at the top of `on_land` (one inside another effect cannot
    be turned and is refused).

## Sealed things, and pacing

- **Anything you seal, you must say what it says.** A `shortcut`'s barred door
  and a `close-gate`'s wall are both things the party walks up to and pushes on,
  and one with nothing to say is `DW0429` — the build refuses. The compiler will
  not decide the tone of your door or your wall for you and then not tell you it
  did. One rule for both: they are two objects of the same class.
  - A `close-gate` discharges it either way — `"sealed_hint": "<what the wall
    says>"` on the effect, or a trigger. `sealed_hint` is only the *wording*.
  - A `shortcut` has no wording field, deliberately: its line is a trigger.
- **Write the reply with the general verb:**

  ```json
  {"id": "trigger/…", "at": "<the gate anchor>", "on": {"on": "use"},
   "once": false, "audience": "presser", "effects": [{"type": "narrate",
   "style": "actionbar", "text": "The door cannot be opened from this side."}]}
  ```

  Anchoring it on the gate is what makes it *ride* the sealed body's own
  hitboxes instead of summoning a co-located second one (`DW0422`), and on a
  shortcut door the body stands on the sealed side only — so the line fires where
  it is true and nowhere else, and retires when the door opens. Once you write
  one, the compiler supplies nothing: one press, one answer. ANY `use` trigger on
  the gate discharges `DW0429`, whatever it does — but a `strike` does not,
  because pressing a thing is a right-click.
  - `audience: "presser"` addresses the one player who clicked, and works on
    `on: use` and `on: step` only — vanilla can attribute a right-click and a
    step and nothing else (`DW0427`). Leave it out and the beat addresses the whole party, which is
    right for a lever that opens a gate and wrong for a reply.
  - `style: "actionbar"` is the reply strip above the hotbar: it does not
    interrupt, does not stack, and is not width-checked. Use it for replies; use
    `title`/`subtitle` for beats.
  - Trigger ids starting with `dw-` are reserved for the compiler (`DW0428`).
  - Two `close-gate`s on one anchor must still agree on the wording (`DW0423`).
- **A beat that can FAIL the player must not arm before they could have read
  it.** Any fail-able beat — follow-an-NPC, escort, timed escape, stealth onset —
  arms only after a grace window long enough to read the on-screen prompt that
  explains it: the player must never be failable before they could have
  understood what is being asked. Where the DSL has an explicit knob, set it
  consciously rather than inheriting the default (`begin-stealth`'s
  `grace_ticks`); where the pacing is authored, put the first enforcing step late
  enough in the `sequence`'s `at_ticks`. Budget the window from the prompt's
  length, not from a habit — a two-line chat prompt is several seconds of reading
  before the first step is taken. The defect this prevents: the flock the player
  is told to follow leaves while they are still reading the instruction, and the
  beat then fails them for it.
- Pace to `target_minutes`; no grind; mandatory-only quests.
