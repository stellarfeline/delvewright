# Device logic — a lock, a key, a door and a button do what they read as

For the agent designing a level's devices and writing their captions, and for the engine
rounds that check them. The question answered: when a player presses, unlocks or opens a
thing, what must be true of the thing for the press to mean what it looks like it means.
The consumer is spec-0124; the rules below are the ones its checks and its guide text rest on.

Every rule is marked **[cited]** with its source or **[authored]** — reasoned here from a
cited rule, with no source of its own. Sources whose licence does not admit copying are read
for their rules only (ADR-0013); each is listed under *Sources* with what was taken.

## 1. Lock and key

- **[cited]** (Dormans 2010, §2–3) The lock-and-key pair is the unit of action-adventure
  mission structure: a mission grammar generates strings such as *key + room + lock + room +
  treasure*, and the space generator keeps a reference to the mission symbol each element was
  generated for, so that keys are not placed *at random locations instead of behind tests or
  locks as specified by the mission*. A key and its lock are coupled in the mission; the space
  is built to keep the coupling.
- **[cited]** (Boris the Brave, *Lock and Key Dungeons*) *A lock is something that stops
  further progress by the player, and a key something that enables that progress*; a *hard
  requirement* must be resolved as intended, a *soft requirement* only encourages a path. The
  same page names the failure mode of a lock whose key is an event flag: *the cause and effect
  have no logical connection whatsoever*.
- **[authored]** The lock sits on the thing it bars. The player's model of the pair is *this
  door is barred; this key opens this door*: the object the key is used on is the object that
  stops progress, and using the key there is what releases it. A key used on one object that
  releases another has no lock the player can see; its effect is an event flag in Boris the
  Brave's sense, however physical the key.
- **[authored]** A key's effect is legible where the key is used. The release is visible from
  the cell the player stands in to use it — the door swings, the bars lift, the lid rises —
  or the beat shows it (§3). What releases out of sight, unshown, is to the player a key that
  did nothing.

## 2. Affordance, signifier, feedback

- **[cited]** (Norman, *The Design of Everyday Things*, 2013 edition, as the Wikipedia
  article *Affordance* records it) *Affordances determine what actions are possible.
  Signifiers communicate where the action should take place.* Norman's running example is the
  door: a flat plate signifies push, a bar or handle signifies pull.
- **[cited]** (Gaver 1991, *Technology affordances*, as the same article records it) A
  **false affordance** is *an apparent affordance* in which the actor *perceives possibilities
  for action that are nonexistent* — the placebo button.
- **[cited]** (Norman, as the Wikipedia article *The Design of Everyday Things* records the
  book's principles) **Feedback**: the user receives *full and continuous feedback about the
  results of the actions*. **Mapping**: the relationships *between the controls and their
  effects* can be determined.
- **[cited]** (Liz England, *The Door Problem*) The questions a door must answer, in the
  designer's own list: *Can the player open every door in the game? Or are some doors for
  decoration? How does the player know the difference?* — *What tells a player a door is
  locked and will open, as opposed to a door that they will never open?* — *Are there doors
  that can open but the player can never enter them?*
- **[cited]** (*The Level Design Book*, *Doors*; CC BY-NC-SA, read for its distinctions only)
  Door kinds the page separates: doors that can never be opened or closed; *fake doors that
  lead nowhere and serve only as set dressing*; critical-path doors that need a specific key;
  doors locked or unlocked by a global quest event. *The simplest, most stable bug-free type
  of video game door is no door.*
- **[cited]** (Salen & Zimmerman, *Rules of Play*, as Taylor's review quotes it, p. 37)
  Meaningful play is *the process by which a player takes action within the designed system
  of a game and the system responds to the action*. The book's two conditions on that
  response — that the relation between action and outcome be *discernable* to the player and
  *integrated* into the larger game — are recalled, not re-read, and are not relied on below.
- **[authored]** In Minecraft a door leaf is the game's strongest signifier of passage: it
  is the one block whose whole vanilla behaviour is to open onto what is behind it. A door
  that never opens, or opens onto a wall, is Gaver's false affordance in the exact sense.
  So a door **the story addresses** — one a quest step is anchored on, one a key is used on,
  one an objective says the player opens — is a way: it stands in the opening between two
  places and, once its step has passed, it opens by hand onto the far place.
- **[ruling]** A door must do a door's job: shut, it stops a body passing; open, it lets one
  pass. A door that does not close a way either is not placed or gets wall joined to both
  sides of it. This holds for every door block in the built world, of every door kind, whether
  or not a story addresses it. There is no scenery door: the one block whose whole behaviour
  is to open onto what is behind it is not placed where nothing is behind it, and L3's
  told-once allowance for scenery that looks usable does not reach doors.
- **[authored]** The same holds for every block the game opens by hand (door, trapdoor,
  fence gate): its press is an answer in itself (spec-0117, *release*), so a shut one promises
  a later opening, and the opening must lead somewhere.

## 3. Cause and effect at a distance

- **[cited]** (Bear's Smile Games, *Prologue level design: Vol. 2*; a devlog, read for its
  rule) *It is important that the player sees the result of his actions as soon as he uses the
  lever*; the gate and the lever are placed *side by side* and connected with wire so the
  relation is in view; *in the later stages of the game, it is possible to break the basic
  principle by deceiving the player's expectation* — a deliberate exception on a learned
  rule, not a default.
- **[cited]** (`game-writing.md` L1) *A closed door with a lever beside it needs no
  explanation.* The level carries the relation; text only restates it.
- **[cited]** (spec-0118 §9, `content.md` step 5, `game-writing.md` L4) A hidden mechanism
  may stand far from its effect; the beat that opens the hidden way carries a close-up shot of
  the opened mouth, and no text before the shot names the secret. The record already allows
  distance when the effect is shown.
- **[authored]** The two legitimate shapes of a remote effect are therefore: the effect is
  in the presser's sight from where the device is used, or the beat shows it with a shot. An
  effect that is neither is Boris the Brave's event flag — a press the player cannot connect
  to anything — and is refused where it is declared, not found in a playtest.

## 4. Captions

- **[cited]** (`game-writing.md` F4, itself on the Microsoft Writing Style Guide through F1)
  A button label answers *what will happen if I press this*, in the player's own words, within
  the widget's budget.
- **[cited]** (Norman, mapping, §2) The relation between a control and its effect must be
  determinable from the control.
- **[authored]** A caption names the acts the button performs and none it does not. A
  *Save and quit* on a button that saves and quits nothing breaks the mapping on the one
  control a player reads most carefully — the one that ends a session. Where the engine knows
  the acts an option performs and a lexicon knows the verbs, the mismatch is machine-checkable
  (spec-0124 §6.4); every other caption is held to this rule by the writer.

## 5. The rules, for the spec

- **R1 [authored, on §1]** A lock's object is the key's object: the thing the key is used on
  is the thing that stops progress, and using the key there releases it.
- **R2 [authored, on §1 and §3]** A key's effect is in the presser's sight from where it is
  used, or the beat shows it.
- **R3 [authored, on §2]** A door the story addresses is a way: it stands in the opening
  between two places and opens by hand onto the far place once its step has passed.
- **R4 [ruling, §2]** Every door in the built world closes an opening: shut, its leaf stops a
  body passing between the two cells it separates, and the cells beside the leaf at both of
  its courses stop a body; open, it lets one pass. A door that closes nothing is not placed, or
  wall is joined to both sides of it.
- **R5 [authored, on §4]** A caption names the acts the button performs and none it does not.
- **R6 [cited, §3]** A remote effect that is neither in sight nor shown is refused; a
  deliberate deception of a learned rule is a declared exception, never a default.

## Sources

- Joris Dormans, *Adventures in Level Design: Generating Missions and Spaces for Action
  Adventure Games*, PCGames 2010 (ACM, copyright the author/ACM) — read for its rules,
  paraphrased, one short phrase quoted; nothing copied.
- Boris the Brave, *Lock and Key Dungeons* (2021, blog, no licence stated) — ideas only, two
  short phrases quoted.
- Wikipedia, *Affordance* and *The Design of Everyday Things* (CC BY-SA 4.0) — for Norman's
  2013 definitions, Gaver's 1991 category and the book's principles; the books themselves are
  not quoted.
- Liz England, *The Door Problem* (2014, blog, copyright the author) — the designer's own
  question list, quoted in short for its rules; ideas only.
- *The Level Design Book*, *Doors* (CC BY-NC-SA) — ideas only (ADR-0013).
- Bear's Smile Games, *Prologue level design: Vol. 2* (devlog, no licence stated) — ideas
  only, two short phrases quoted.
- Nick Taylor's review of Salen & Zimmerman, *Rules of Play* (MIT Press) — for the one
  quoted definition; the book is not re-read here.
- The record: `docs/reference/game-writing.md` (F1, F4, L1, L3, L4), spec-0117 (*release*),
  spec-0118 §9, `.claude/skills/delvewright/skills/new-delve/references/content.md` step 5;
  `docs/reference/doors-by-hand.md` and spec-0123 §3.5 (`origin/feature/a-place-reads-as-what-it-is`)
  for the leaf's shape and the census of a door's framing.
