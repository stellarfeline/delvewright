# Reach volumes and killing volumes — where established practice places each

For whoever designs a rule about a volume that grants something (an arrival
trigger) standing next to a volume that kills (a pit, lava, a fall). What
established practice records about where each is placed, what it does not
record, and what this engine takes from it. Every rule is marked **[cited]**
with its source and rating, or **[authored]** — reasoned here from a cited rule
or from the engine's own measured behaviour. Ratings: STRONG (primary source
read in full), MEDIUM (a secondary account read in full), WEAK (one example,
not a practice).

The research answered two questions and stopped: *is an arrival trigger placed
on the space a body occupies or on the thing it approaches*, and *is there a
stated margin between a killing volume and the footing beside it*.

## 1. An arrival trigger is a volume the body enters

- **A trigger is a volume the player enters, and what it detects is entering,
  staying and leaving.** [cited, MEDIUM] The Level Design Book, *Scripting*:
  *"triggers let us define volumes and areas, and apply behavior to those
  zones"*; *"a trigger is an EVENT"*; triggers *"know when something enters,
  stays, and exits"*. The examples given are *"traps, deaths, lava, etc. or
  opens a door"*.
- **In single-player the trigger is invisible and seamless.** [cited, MEDIUM]
  The same page: *"triggers should be invisible and seamless and magic, and
  sometimes just one-off triggers"*; the multiplayer exception (*"bomb site
  areas in CS, capture points in Overwatch"*) is the one where the volume is
  shown, because there it is a contested place rather than a beat.
- **A progress trigger sits on a threshold the body crosses, not on the object
  of interest.** [cited, MEDIUM] The Level Design Book, *Encounter*: *"Once
  they pass some sort of midfield threshold, trigger enemies and begin the
  fight"*; the one-way entrance (*"a short vertical drop or airlock door
  closing behind them"*) is the same device at a doorway. The trigger is where
  the feet will be.
- **Engines adjudicate a trigger on the actor's collision shape overlapping the
  volume.** [cited, MEDIUM] Unreal Engine documentation, *Trigger Volume
  Actors*: triggers cause an event *"in response to some type of collision
  with another object, such as something hitting or overlapping with the
  Trigger"*; the shape (box, capsule, sphere) is *"the area of influence … used
  by the Trigger to detect if another object has activated it"*.
- **Vanilla Minecraft selects on hitbox intersection with a closed box.**
  [cited, STRONG — the reference read in full] Minecraft Wiki, *Target
  selectors*, volume arguments: the region is *"a cuboid volume from the
  initial position `(x,y,z)` to `(x+dx,y+dy,z+dz)` but with the vector `(1,1,1)`
  added to the x-most, y-most, z-most corner"*, and *"all entities whose
  hitboxes at least partially intersect with that volume are selected"*. This is
  the rule both a `reach` completion and a lethal volume are emitted against,
  and it is why a body reaches a volume from a cell its feet are not in.

## 2. A killing volume sits under the playable area and kills on touch

- **A kill plane is an invisible boundary that kills or respawns on touch, and
  it is placed under the playable area — below platforms, at the bottom of pits
  and cliffs — or on a specific hazard such as a lava pit.** [cited, MEDIUM]
  GDQuest glossary, *Kill plane*: *"an invisible boundary in a game level that
  instantly kills or respawns the player when touched"*; *"typically placed
  under the playable area to catch when the player falls"*; *"below platforms
  in platformers, race tracks in racing games, or at the bottom of pits and
  cliffs"*; *"specific to certain areas like lava pits where the character
  burns up on contact"*.
- **Its purpose is a safety net for a fall that has already happened, not a
  wall the player walks up to.** [cited, MEDIUM] The same entry calls kill
  planes *"safety nets"* against infinite falling and describes the respawn
  (*"the beginning of the level, the last checkpoint, or the edge of the
  platform they fell from"*). Every placement it names is below the footing,
  never level with it.
- **No source read states a margin between a killing volume and the footing
  beside it, or a rule for what a body standing at a hazard's edge may touch.**
  [authored — an absence in the sources, not a finding in them] The pages that
  would carry it for the Source engine (`trigger_hurt` on the Valve Developer
  Community and its talk page, and the TWHL entry) returned HTTP 403 to this
  research and were not read; an unread source is not cited. The Unreal
  *Volumes* reference names a Pain Causing Volume and a Kill Z Volume by
  purpose only.

## 3. What this engine takes from it

- **An arrival is judged where a body's feet can be, and the completion volume
  is a tolerance around that.** [authored, from §1] A `reach` whose anchor is
  a place no body can stand — a solid altar, a lever in a wall, a cell that
  kills — is completed from the footing nearest it, exactly as the practice
  puts a progress trigger on the threshold and not on the thing beyond it. The
  engine names the radius at which the tolerance first reaches that footing
  rather than asking the creator to guess it.
- **A killing volume's keep-out is derived from the selector, not from a
  practice.** [authored, from §1's last rule] Vanilla kills a body whose box
  meets the volume, so the cells a walker may not stand in are the volume
  widened by half the walker's width — one cell horizontally for every body up
  to two blocks wide. The practice in §2 places kill planes under the floor
  where no such question arises; where this engine draws a volume level with a
  floor, the margin is the only reading consistent with the selector, and it is
  authored here because no source states one.
- **A hazard-centred reach is a legitimate shape, expressible at the radius
  that reaches its lip.** [authored, from §1 and the engine's own geometry] For
  a body up to two blocks wide the lip of a one-cell volume is two cells from
  its centre, so a `reach` anchored on the killing cell completes at radius two
  and at no smaller radius; the objective fires when the party stands short of
  the drop, which is what "reach the edge" means. This is the rule
  spec-0062 §4 adopts; the gap it stands on is the absence named in §2.

## 4. Sources

- The Level Design Book — *Scripting*
  (`book.leveldesignbook.com/process/scripting`) and *Encounter*
  (`book.leveldesignbook.com/process/combat/encounter`). CC BY-NC-SA;
  ideas-only under ADR-0013 — nothing is copied beyond the quoted sentences.
- Unreal Engine documentation — *Trigger Volume Actors*
  (`dev.epicgames.com/documentation/unreal-engine/trigger-volume-actors-in-unreal-engine`)
  and *Volume Actors*
  (`dev.epicgames.com/documentation/en-us/unreal-engine/volume-actors-in-unreal-engine`).
- Minecraft Wiki — *Target selectors* (`minecraft.wiki/w/Target_selectors`),
  CC BY-NC-SA; ideas-only, quoted for the rule the pinned server implements.
- GDQuest — glossary, *Kill plane* (`school.gdquest.com/glossary/kill_plane`).
- Not read (HTTP 403): Valve Developer Community `Trigger_hurt` and
  `Talk:Trigger_hurt`; TWHL wiki `trigger_hurt`.
