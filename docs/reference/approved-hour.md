# The approved hour — how an approved look records its sky

For whoever designs the artifact that carries the time of day and weather a
human approved a look in, so that the built world can be held to it. What
established practice does, what it does not do, and what this engine takes from
it. Every rule is marked **[cited]** with its source and rating, or
**[authored]** — reasoned here from a cited rule or from the engine's own
measured behaviour. Ratings: STRONG (primary source read in full), MEDIUM (a
secondary account read in full), WEAK (one example, not a practice).

## 1. Animation: the key is a painting, and the painting is the record

- **A colour script is a sequence of small paintings that fixes the colour and
  lighting of a story beat by beat, made before production and used as the
  reference everything downstream is judged against.** [cited, MEDIUM]
  StudioBinder, "What is a Color Script": *"a visual roadmap of a film's
  story, told through the strategic use of color … a sequence of miniatures or
  digital paintings that mirror the film's key scenes"*, which *"serve as a
  comprehensive visual reference that helps the entire production team
  understand and execute the director's vision"*. Pixar is credited with the
  practice from *A Bug's Life* (1998).
- **Colour keys are the per-scene form of the same thing, and lighting reads
  them.** [cited, MEDIUM] Envato Tuts+, "How to Make Colour Keys for
  Animation": *"Colour Keys are used in animation to help establish the look
  and feel of the background as well as the overall colour of the scene.
  Essentially they are background paintings that lighting, layout and other
  departments in the studio use as reference during production."* Animation
  Career Review's colour-key-artist profile says the same of who consumes them:
  lighting, materials and production teams.
- **A key is drawn for one lighting condition, and a scene wanting two gets
  two keys.** [cited, MEDIUM] The Envato tutorial's worked example paints a
  daytime key and a night key of one background as two separate paintings. The
  hour is a property of the key, never a caption over a set of keys.
- **In every account found, the built scene is held to the key by a person
  reading both.** [authored — an absence in the sources, not a finding in
  them] No source describes a machine comparing a lit shot to its key; the key
  is "reference", and the lighting department's review is the check.

## 2. Games: the hour is a field of the level's setting, written before geometry

- **Time of day and weather are decided at the setting step of level
  planning, before any geometry, as named properties of the level's theme.**
  [cited, MEDIUM] World of Level Design, "How to Plan Level Designs and Game
  Environments in 11 Steps", step 2 (*Setting, Location and Theme*): *"Theme is
  more abstract such as a particular design style, time of day, time in
  history, weather, atmosphere, mood, feeling or an event."* Step 10 (*Visual
  Development*) lists lighting among the elements the visual pass is held to.
- **A level's "ground rules" include day/night and weather.** [cited, MEDIUM]
  Encyclopedia MDPI, "Level Design": environmental conditions such as day/night
  and weather are listed among the ground rules a level designer sets.
- **Level design documents carry them as literal fields.** [cited, WEAK — one
  student portfolio, an example rather than a practice] A published level
  design document records *"Time of day: Night"* and *"Climate / Weather:
  Cloudy"* as header fields of an area.
- **Concept art for game environments is annotated for the teams that build
  from it.** [cited, MEDIUM] N-iX, "Understanding Video Game Concept Art":
  finished environment concept art *"is usually annotated or with callouts
  highlighting elements like textures, lighting angles, or gameplay-critical
  features"*, serving as the team's visual bible.

## 3. What this engine takes from it

**[authored]**, from §1 and §2 together, and from the engine's own record of
what it can and cannot read (`tools/refimg.py`: no part of the toolchain
places, reads or compiles a reference image).

1. **The record of an approved hour is a written token beside the approved
   painting, not the painting.** Practice keeps the hour *in* the key (§1) and
   *as a field* of the setting (§2); this engine cannot read a picture, so the
   field is the only half a machine can hold anything to. The token is written
   at the moment of approving, by the creator, reading the picture — the same
   act §1's lighting department performs, moved to the one place the picture
   and the token are in one hand.
2. **One token per painting, never one caption per set.** A design whose
   finale is a sunrise has two keys (§1, third rule), and a set-level caption
   could state only one of them.
3. **The machine holds the world to the token; the human holds the token to
   the picture.** What a machine can prove is that every hour the built world
   reaches was written beside some approved picture and that every written
   hour is one the world reaches. Whether the token is true of the picture
   stays a human reading, and it is made where it costs least — at approval,
   not at the first rendered frame.
4. **The hour has no default.** §2's practice decides it at the setting step
   as a fact of the level; a mechanism that silently supplies one when the
   author said nothing is a design decision (this delve is played at noon)
   encoded in a primitive, which is the defect `CLAUDE.md` names.

## 4. What was not found

- No source in which a shot is audited against its colour key by anything but
  a reviewer's eye. The rule that a machine can hold only the token is
  therefore an engineering constraint of this engine, not a cited practice.
- Real-time engines' time-of-day systems (a sky profile, a sun position, a
  weather preset as level data) were not researched as an authority for *how
  the field is stored*; §2's level-document practice was enough to settle
  *that* it is stored as a field of the level.

Sources: StudioBinder, "What is a Color Script and What Are They Used For?"
(studiobinder.com/blog/what-is-a-color-script-definition, read in full);
Envato Tuts+, "How to Make Colour Keys for Animation Scenes and Projects"
(photography.tutsplus.com, read in full); Animation Career Review, "Color Key
Artist — Career Profile" (animationcareerreview.com, search excerpt);
World of Level Design, "How to Plan Level Designs and Game Environments in 11
Steps" (worldofleveldesign.com, read in full); Encyclopedia MDPI, "Level
Design" (encyclopedia.pub/entry/28441, search excerpt); N-iX, "Understanding
Video Game Concept Art" (gamestudio.n-ix.com, search excerpt); a published
student level design document (keltondalm.wordpress.com, search excerpt). All are
ideas-only sources (ADR-0013); nothing is copied from them beyond the quoted
sentences.
