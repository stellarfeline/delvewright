# Souls design language — a working dossier

Written 2026-08-02 for the planner authoring the M4 souls campaign, after the
owner's verdict that the spec-0016 draft's souls understanding was shallow.
This is a **working reference**, not an essay: what the grammar actually is,
what the real games measure, and which of our primitives can stage each piece.

Scope: FromSoftware level/encounter grammar (Demon's Souls → Elden Ring) read
by a designer who must rebuild it in vanilla Minecraft adventure mode with no
stamina, no i-frames, no builds, and no estus.

**Sourcing rule applied throughout** (CLAUDE.md attribution ledger). Every
non-obvious claim carries a source. License triage — the surprise is that the
wikis we would reflexively trust are **not** the permissive ones:

| Source class | Quotability |
|---|---|
| Fandom wikis, Wikipedia | CC BY-SA — safe to quote with attribution |
| arXiv / DIGRA papers | open access — safe to cite and quote academically |
| **Fextralife wikis** | **NOT CC BY-SA.** Their [Terms of Use](https://fextralife.com/terms-of-use/) grant only personal, non-commercial, non-transferable use → treat as All Rights Reserved, **ideas-only** |
| Journalism (Kotaku, TheGamer, PC Gamer, GameDeveloper, …) | ARR — short attributed quotes only |
| Video essays (GMTK, Matthewmatosis, Joseph Anderson) | ARR — paraphrase + attribute, never transcribe |
| Forums (ResetEra, Steam, GameFAQs) | ARR per-poster — ideas-only, never quote |

Ledger entries for the analyses leaned on are in
[`docs/ACKNOWLEDGEMENTS.md`](../ACKNOWLEDGEMENTS.md).

**Research gap, stated up front:** there is no accessible developer-authored
GDC talk on FromSoft's shortcut grammar. Miyazaki does not speak at GDC; the
one Dark Souls GDC Vault talk that surfaced is a business-strategy piece
([Fischer, GDC 2017](https://gdcvault.com/play/1024461/Strategic-Design-Or-Why-Dark)),
not level design. The analytical canon here is interviews + video essays +
wikis. Treat every "FromSoft intended X" claim below as interpretation unless
it is tied to a named developer quote.

---

## 1. The loop-back shortcut grammar

### 1.1 What the shape actually is

The owner's ruling — two routes between rest points, the short one sealed,
opened permanently from the far side — is the correct primitive. The series'
own vocabulary for it:

- **The one-way door.** A door barred from the far side; you arrive at its back
  and lift the bar. DS1 Depths→Blighttown; DS3 Road of Sacrifices (a door
  openable only from outside the Abyss Watchers arena); DS3 Irithyll (a locked
  door beside the first bonfire, reached by breaking an illusory railing and
  circling behind); Elden Ring's Roundtable Hold Two Fingers door; the
  Shadow Keep one-way door before the Gaius lift.
  Sources: [Fextralife Blighttown](https://darksouls.wiki.fextralife.com/Blighttown) (ideas-only),
  [Steam DS3 discussion](https://steamcommunity.com/app/374320/discussions/0/361787186438198418/),
  [Steam Irithyll](https://steamcommunity.com/app/374320/discussions/0/4362373511172075139/),
  [Sportskeeda Roundtable](https://www.sportskeeda.com/esports/how-open-door-two-fingers-roundtable-hold-elden-ring).
- **The elevator kick.** Undead Parish→Firelink and New Londo→Firelink (DS1)
  are the canonical hub-openers; Sen's Fortress cage lift and Stormveil's
  Rampart Tower lift are level-internal loop-tighteners.
  ([Fextralife Undead Parish](https://darksouls.wiki.fextralife.com/Undead+Parish),
  [New Londo](https://darksouls.wiki.fextralife.com/New+Londo+Ruins),
  [GameFAQs Sen's lift](https://gamefaqs.gamespot.com/ps3/606312-dark-souls/answers/325737-how-does-the-elevator-in-sens-fortress-work),
  [Gamepur Stormveil](https://www.gamepur.com/guides/elden-ring-how-to-activate-the-rampart-tower-elevator-in-stormveil-castle) — all ideas-only.)
- **The ladder kick-down.** The Undead Parish ladder drop is the moment every
  retrospective names as the series' "aha"; Bloodborne's Central Yharnam ladder
  + adjacent lever; DS3 Irithyll Dungeon's ladder-then-elevator loop
  (James Roha, [*World Design lessons from FromSoftware*](https://medium.com/@Jamesroha/world-design-lessons-from-fromsoftware-78cadc8982df), ARR blog, ideas-only).
- **The far-side lever/gate.** Bloodborne's Round Plaza gate and the Central
  Yharnam→Great Bridge gate; Stormveil's interior lever that opens the front
  gate onto the outer approach.

### 1.2 The distinction our spec was missing: hub-openers vs. local loops

Not all shortcuts are the same beat. **Hub-openers** (Undead Parish and New
Londo lifts → Firelink) collapse the *world map*; that is the "the world folds
into itself" reveal, and it is a once-or-twice-per-game event. **Local loops**
(Sen's cage lift, Stormveil's rampart) merely shorten the current level and
happen constantly. Roha frames the hub-opener as doing two jobs at once —
mechanical relief *and* the cognitive reveal that the world is one coherent
object.

Demon's Souls is the counter-case worth knowing: it has a **strict hub model**
(the Nexus is the only hub; no cross-Archstone shortcuts exist), so all its
shortcuts are local loops
([Fextralife Nexus](https://demonssouls.wiki.fextralife.com/The+Nexus), ideas-only).
A delve can be authored either way. Ours should pick deliberately.

### 1.3 The trend the series itself followed

Sekiro largely **abandoned** the loop-back: community critique holds that most
of its shortcuts are pointless because idol-to-idol warping is simply better,
and the grapple lets players vault the gates the shortcut would have opened
([ResetEra](https://www.resetera.com/threads/the-idol-placement-in-sekiro-is-abysmal-no-spoilers.110668/), ideas-only).
Matthewmatosis's much-cited contrast is that Demon's Souls levels loop back on
themselves while DS2's are "long tunnels with dead ends"
(video essay, ARR, paraphrased via
[NeoGAF](https://www.neogaf.com/threads/matthewmatosis-commentary-of-dark-souls.1122794/)).
Joseph Anderson makes the same charge against DS2's linearity
([summary](https://critpoints.net/2016/06/08/joseph-anderson-on-dark-souls/)).

Design consequence for us: **the loop-back is a property of walkable
connectivity, and it dies the moment fast travel is cheaper than the loop.**
Our delves have no fast travel, so the loop pays by construction — an advantage
over Sekiro's own version of the pattern.

### 1.4 Runback length — the honest numbers

The owner kept a ≤ 60 s bonfire→failure-point lint. Here is what the real games
measure. The only piece of games journalism found that states actual times is
TheGamer's runback ranking (ARR, figures paraphrased):

| Boss / game | Reported runback |
|---|---|
| Lud & Zallen, Frigid Outskirts (DS2) | ~5 min on a *good* run |
| Old Hero (Demon's Souls) | ~3 min optimal, ~4 with caution |
| Martyr Logarius (Bloodborne) | ~2 min |
| Tower Knight (Demon's Souls) | ~2 min past 25+ enemies |
| Bed of Chaos (DS1) | ~1.5 min |
| Placidusax (Elden Ring) | ~1.5 min |
| Sir Alonne / Blue Smelter (DS2) | ~1 min once memorised |

Source: [TheGamer, *8 Longest And Most Annoying Boss Runbacks*](https://www.thegamer.com/longest-annoying-soulsborne-boss-runbacks/).
Note that GameRant's equivalent list gives **no numbers at all**
([*DS2: 10 Hardest Boss Runs*](https://gamerant.com/dark-souls-2-hardest-boss-runs/))
— runback length is a thing players feel intensely and almost nobody measures.

Against that, the modern floor: Elden Ring's **Stakes of Marika** exist
specifically to "make it easier and faster to reach the point where you have
died… removing some of the frustration that came with death in previous Souls
games" ([Fextralife](https://eldenring.wiki.fextralife.com/Stakes+of+Marika), ideas-only;
analysis at [Punished Backlog](https://punishedbacklog.com/game-design-discussion-elden-rings-stake-of-marika/)).
Bloodborne's boss lamps can mostly be moved to the arena door, with a short
named exception list (Rom, Amy, Micolash, Celestial Emissary, Orphan).

**Correction to a belief our spec drafts leaned on:** DS3 does *not*
universally place a bonfire before every fog gate. Dragonslayer Armour and the
Deacons only get their bonfire *after* the fight, and a popular mod exists
purely to retrofit the convention
([Nexus, *Bonfires Before Bosses*](https://www.nexusmods.com/darksoulsremastered/mods/1065)) —
i.e. players experienced its absence as a real gap.

Miyazaki's own framing of retry friction is about *routing*, not distance:
players should be able to "come back to something later when they're at an
impasse… and not have to bang their head against a wall over and over"
([PlayStation Blog, 2022-01-28](https://blog.playstation.com/2022/01/28/an-interview-with-fromsoftwares-hidetaka-miyazki/), developer primary).

**Verdict on our ≤ 60 s lint.** It is not a souls-typical number — it is
roughly the *modern FromSoft target*, and it is stricter than the entire
notorious tier (1.5–5 min). That is fine; the real problem is the opposite one.
Measured vanilla 1.21.11 ground speed is **4.317 m/s walking, 5.612 m/s
sprinting** ([`jump-arc-model.md`](jump-arc-model.md)), so 60 s of walking is
**~259 blocks**, sprinting ~337. A box-garden delve is rarely 259 blocks
end-to-end. **The lint as specified will essentially never fire.** If the
planner wants retry cost to be a real design dial, the threshold has to be
box-garden-scaled (30 s ≈ 130 blocks walking is already generous), and it
should be measured *per failure point*, not per campaign.

### 1.5 The emotional arc

The sequence the grammar produces is despair → mastery → relief, and the
shortcut is where relief is banked. The Level Design Book's Undead Burg study
is the most concrete published account of the beat-level manipulation: it reads
the one-way drops as enforcing forward flow, treats enemy placement as
"breadcrumbs" for wayfinding, and analyses the fog-gate→empty-room→dragon
sequence as deliberate false relief before a shock
([leveldesignbook.com](https://book.leveldesignbook.com/studies/sp/undead-burg), ARR, ideas-only).

Academic framing exists if the paper wants it: Andriano, *Enjoying the
Uncertainty* (Games and Culture, 2025,
[SAGE](https://journals.sagepub.com/doi/abs/10.1177/15554120241226837)) reads
Dark Souls as performing incompleteness through level design. Paywalled —
paraphrase only, and not independently verified beyond its abstract.

**Do not attribute "the level is the boss" or "the shortcut is the reward" to
anyone.** Both are community shorthand; no primary source exists for either
phrasing.

---

## 2. 初见杀 — the first-encounter kill, taxonomy

The owner's ruling (spec-0016 §3) is right: the un-telegraphed first kill is
core vocabulary. What the research sharpens is *which axis* separates a beloved
one from a resented one — and it is **not** telegraphing.

### 2.1 The A/B pair that defines the line

**Beloved — the Hellkite Wyvern on the Undead Burg bridge.** It lands and
breathes fire down the bridge, and it kills you. But the encounter is
saturated with outs: it is not meant to be fought at all when first met; you
can sprint the bridge under a shield at low equip burden and duck right at the
midpoint — which activates **the first shortcut in the game**, a ladder down to
your previous bonfire; there is a route underneath the bridge that bypasses it
entirely; and you can come back later and kill it
([Fandom Hellkite Drake](https://darksouls.fandom.com/wiki/Hellkite_Drake), CC BY-SA;
[Fextralife](https://darksouls.wiki.fextralife.com/The+Bridge+Wyvern), ideas-only).
Note what that means structurally: **the thing that kills you and the shortcut
you unlock are the same piece of level.**

**Resented — the Capra Demon.** The player consensus, consistent across
threads, is a stack of four faults: a boss *with adds* in a cramped room; a
tree that blocks the camera; dogs that stunlock while the demon does the
killing; and — the opening beat — the demon lunges the instant the fog-gate
animation ends, where every prior boss gave the player a moment to collect
themselves ([ResetEra](https://www.resetera.com/threads/is-capra-demon-a-bad-boss-fight.1586605/),
[ResetEra](https://www.resetera.com/threads/the-capra-demon-is-some-bullshit.674218/) — ideas-only, never quoted).

Neither has a telegraph. The difference is **whether the second attempt has
agency**. Hellkite offers run-past, snipe, under-route, return-later. Capra
offers "the same three enemies, the same 8×8 room, but better".

### 2.2 The five axes that decide it

1. **Consistency** — same trap, same place, every time. Determinism is what
   converts a death into a lesson. (Our compiler gives this for free.)
2. **Second-read counterplay** — luring, positioning, thrown items, ranged
   opening, routing around. At least one must exist.
3. **Escapability of the arena** — a sealed room multiplies every other fault;
   open terrain divides them. This is the whole Capra/Hellkite delta.
4. **Death cost** — a long runback converts a fair lesson into resentment (§1.4).
5. **Information available *before* commitment** — not a telegraph in the
   moment, a **sightline beforehand**. You can stand outside Sen's Fortress and
   watch a blade cycle. You cannot see inside the Capra room.

Axis 5 is the one our spec conflated with telegraphing. They are different: the
owner is right that in-the-moment telegraphs are optional; observability from
the decision point is not.

### 2.3 The catalogue

- **Mimics** — the only ambush in the set with a genuine tell, and it is triple
  redundant: the chain bends back and forth toward the front instead of curving
  in one arc to the back; the lid lifts for a breath roughly every 10 seconds;
  the chest is paler than a real one. The wiki's own guidance is that the chain
  is the reliable read because it needs no waiting
  ([Fextralife](https://darksouls.wiki.fextralife.com/mimic), ideas-only;
  [Fandom](https://darksouls.fandom.com/wiki/Mimic), CC BY-SA).
  The design lesson: a fair "gotcha" hides in plain sight and rewards the
  player who *stops to look*.
- **Sen's Fortress** — blades, boulders, pitfalls, arrow traps, all deliberately
  screaming "trap" (§5.1). The counter-example to the idea that souls hides its
  dangers.
- **Displaced-trigger ambush** — the Irithyll chest that summons basilisks
  *after* the pickup (§4.2). The resented kind, and for a precise reason: no
  observation from the decision point could have revealed it.

### 2.4 The scripted first death

A distinct beat, worth its own thought. Elden Ring's Grafted Scion is placed as
a tutorial boss players are meant to lose to. If you *do* beat it you get its
armaments — and then walk a little further, hit a cliff, and a cutscene forces
you off it anyway; the game proceeds identically. Demon's Souls did the same
with the Vanguard and the Dragon God
([CBR](https://www.cbr.com/elden-ring-grafted-scion-scripted-death/),
[ScreenRant](https://screenrant.com/elden-ring-grafted-scion-boss-tutorial-beat-reward/),
[Digital Trends](https://www.digitaltrends.com/gaming/elden-ring-tutorial-boss-grafted-scion/) — ARR).

Note that the games **reward the anomaly** rather than preventing it. The
content is: *death is introduced as a mechanic before it is introduced as a
punishment, the first one is free, and beating the unbeatable is acknowledged.*

---

## 3. The optional elite

### 3.1 The pattern

A deliberately over-tuned enemy in open, walkable ground near the start, which
you may fight or walk past.

- **Tree Sentinel (Elden Ring)** — the canonical modern instance. The stated
  role, consistently across guides: it teaches that *you are not obligated to
  fight everything the moment you see it*; you are meant to go around, find
  easier fights, and come back stronger
  ([GameRant](https://gamerant.com/elden-ring-how-to-defeat-the-tree-sentinel/),
  [GamesRadar](https://www.gamesradar.com/elden-ring-tree-sentinel-boss-how-to-beat/), ARR).
  **No Miyazaki quote about the Tree Sentinel exists** — searched, not found.
  The "statement of intent" reading is journalistic, and should be labelled as
  such in any spec that cites it.
- **DS1 Black Knights** — Undead Burg, Undead Parish, Darkroot Basin and
  others, guarding optional treasure, and **permanently dead once killed**
  (only the five in the Kiln of the First Flame respawn). DS3 reversed this and
  respawns them everywhere (§6.2).
- **DS2 Heide Knights** — the clearest *staging* of the pattern: they sit
  hunched and **neutral until provoked**, taking a beat to stand before
  attacking. In the original release they never respawn; Scholar of the First
  Sin added six respawning ones at Heide's Tower of Flame, four of which stay
  non-hostile until the Dragonrider dies
  ([Fandom](https://darksouls.fandom.com/wiki/Heide_Knight), CC BY-SA).

### 3.2 What it teaches

1. The world is **not level-gated** — strength is never a prerequisite for
   physical access.
2. **Fleeing is a legitimate verb**, not a failure state.
3. Power becomes **legible as earned**: the elite is a fixed benchmark you
   re-measure yourself against on the way back through.
4. Optional means **signposted-hard, not blocked.**

### 3.3 How the bypass is staged — the four signals

Our spec's obligation ("prove a bypass route exists") is necessary but not
sufficient. A bypass must be **legible**, not merely extant. The four signals
the real games use:

1. **No fog gate.** Every optional elite above stands in open world space. The
   fog gate is FromSoft's single clearest "this is mandatory" marker — which is
   exactly why the Capra Demon reads as a trap and the Tree Sentinel does not.
2. **Visible open ground around it**, wide enough that walking past is
   obviously a thing the level expects.
3. **Dormancy or leash as an at-a-glance tell.** A hunched, neutral Heide
   Knight *is* the sentence "I will not chase you unless you pick this fight."
   This is the cheapest and most legible of the four.
4. **Conspicuous over-presentation as a warning label** — gold armour, oversize
   silhouette, a mount. The aesthetic is the telegraph before any combat data
   exists.

Signal 3 is the one we can stage most cheaply and are not currently staging at
all (§10 gap G4).

---

## 4. Ambush placement grammar

### 4.1 Corner discipline — the catalogue

- **DS3 Cathedral of the Deep** is the densest ambush level in the series and
  the best single study: a Thrall leaps from a doorway on the left; more Thralls
  wait on the ceiling to drop; several perch on curved rafters; another climbs a
  wall to attack beside a corpse item. It is also the level most criticised for
  ambush *monoculture* — a Steam poster's charge is over-reliance on ambush
  relative to every other tool
  ([Fextralife](https://darksouls3.wiki.fextralife.com/Cathedral_of_the_Deep) ideas-only;
  [Steam](https://steamcommunity.com/app/374320/discussions/0/361787186421229346) ideas-only).
- **Elden Ring catacombs** keep the grammar into the open-world era: a Heavy
  Skeletal Swordsman around a turn in Caelid Catacombs; two Fanged Imps in a
  Stormfoot dead-end room with one tucked at the right corner; a Cliffbottom imp
  lying down beside the entrance (Fextralife Caelid / Stormfoot / Cliffbottom
  Catacombs pages, ideas-only).
- **Bloodborne Yahar'gul** places Chime Maidens — buff/revive support enemies —
  in corners and dead ends specifically, so the ambush is a *priority-target*
  puzzle rather than a damage spike
  ([GamerGuides](https://www.gamerguides.com/bloodborne/guide/walkthrough/unseen-tombstone-2/yahargul-unseen-village), ARR).
- **DS1 Depths** hides giant rats inside the first two breakable boxes past the
  stairs — the container ambush. The same level lets the player *invert* it:
  another rat can be reached by dropping through a hole for a plunging attack
  ([Fextralife](https://darksouls.wiki.fextralife.com/Giant+Rat), ideas-only).

### 4.2 Bait-item ambushes

The cleanest instance found: on the DS3 Cathedral rooftop, a Thrall hangs on
the slanted roof waiting for the player to grab the item below — and the level
repeats the trick at a second rooftop pickup.

Three variants, in ascending harshness:

1. **Co-located** — ambusher visible near the bait (Cathedral rooftop). Fair:
   the tell is in frame.
2. **The bait *is* the ambusher** — mimic chests (13 in DS3), placed exactly
   where a too-good reward is plausible, e.g. the Irithyll Dungeon corridor with
   seven patrolling Jailers ([GosuNoob](https://www.gosunoob.com/dark-souls-3/mimic-chest-locations/), ARR).
3. **Displaced trigger** — a *real* chest at the end of the Irithyll rat tunnel,
   after which five or six basilisks converge to curse you. The ambush is
   separated in space and time from the pickup, so no observation before
   committing could have revealed it (same source).

Variant 3 is the one that reads unfair, and the reason generalises: **the
ambush must be discoverable from the position where the player decides.**

### 4.3 Sound cues — the finding that should change our spec

Our drafts implicitly assumed FromSoft warns you audibly. **It largely does
not.** The fairness apparatus is overwhelmingly *visual*: silhouettes against
light, exposed hanging poses, a paler chest colour, a straight versus coiled
chain, worn-smooth stair treads. No source found supports the idea that
ordinary enemies emit idle breathing or footsteps audible through walls as a
pre-emptive ambush warning — there is no stealth-game sound cone here.

The complete set of documented audio tells is four items long:

| Cue | Game | Character |
|---|---|---|
| Mimic breath — lid lifts about every 10 s | DS1/DS3 | close-range, requires stopping to watch ([Fextralife](https://darksouls.wiki.fextralife.com/mimic), ideas-only) |
| Winter Lantern song + orange glow | Bloodborne | *post*-detection state, not a warning ([Fextralife](https://bloodborne.wiki.fextralife.com/Winter+Lantern), ideas-only) |
| Sen's boulder direction-change noise | DS1 | genuine pre-emptive cue |
| The chariot's loud stop on the balcony below | Elden Ring | gates a timed action |

Two of the four are *mechanism* cues, not enemy cues. Design consequence for
us: **an ambush's fairness budget must be spent on sightline and silhouette,
not on a sound.** That is fortunate, because vanilla's audio surface is
`play-sound` at a point — coarse, and easy to over-trust.

### 4.4 Density and pacing — teach / test / twist

The Level Design Book's Undead Burg study is the only source found that lays out
ambush pacing as a structure rather than a list. Over one stretch it reads:

1. **Teach** — one enemy in a suspicious doorway; semi-predictable.
2. **Test** — two hidden enemies at a ledge; not predictable.
3. **Twist** — four enemies visible hanging below; *very* predictable, and
   foilable by an observant player.

The study's own gloss on the third beat is the principle: the designer could
easily have hidden the hangers on the far side, and deliberately left them
exposed — ambushes are fair *if you are smart enough*
([leveldesignbook.com](https://book.leveldesignbook.com/studies/sp/undead-burg), ARR, ideas-only).
The same study notes enemies double as **breadcrumbs** — placement is
wayfinding as well as threat.

Escalating checkpoint spacing is a reviewer-level observation rather than a
measured one: bonfire-to-boss distance grows as the game proceeds, pacing the
learning curve ([Wikipedia](https://en.wikipedia.org/wiki/Bonfire_(Dark_Souls)), CC BY-SA).

---

## 5. Timed and periodic hazards

### 5.1 Sen's Fortress, the canonical trap road

Miyazaki's own concept word for the area was a **"trap road"** — the approach
to Anor Londo, built so arrival feels like "I made it". The art designer,
Masanori Waragai, is explicit about telegraphing:

> "We almost tried hard to make them obvious and create things that screamed
> 'trap'."
> — [PCGamesN, *Sen's Fortress: the trap house*](https://www.pcgamesn.com/dark-souls-remastered/sens-fortress-trap-house)

Concretely: worn, rounded staircase treads mark the boulder paths; pendulum
blades swing over railless catwalks and are passed by reading gaps, never by
sprinting or rolling; boulders change direction between visits and announce the
change with a noise; arrow traps fire from pressure plates. The signature
absence — **Sen's has no mid-level bonfire**, and reviewers read that omission
as deliberate tension escalation
([Wikipedia](https://en.wikipedia.org/wiki/Bonfire_(Dark_Souls)), CC BY-SA).

### 5.2 The hazards are not merely dodgeable — they are *solvable*

This is the second finding our spec did not have. Across the best-documented
timed hazards, the player can permanently remove the threat:

- **Fringefolk Hero's Grave chariot** (Elden Ring) runs a fast, repeating
  up-down cycle with side alcoves as safe pockets; it is destroyed by shooting
  three pot-traps, timed to the audible moment the chariot stops on the balcony
  below.
- **Auriza Hero's Grave** goes further — hitting a fire-breathing trap spawns an
  extra chariot so the chariots collide and destroy each other.
- **Smouldering Lake's triple ballista** (DS3) fires on anyone crossing the
  lake, and is switched off by following an underground path to a ladder
  beneath it ([Fandom](https://darksouls.fandom.com/wiki/Smouldering_Lake), CC BY-SA).

So the real grammar is a **three-rung ladder**: readable → avoidable →
*disable-able*. Our `traps[].disarm{via,sets_flag}` already sits on the third
rung; nothing in our timed-gate design does.

### 5.3 Sanity-checking the ≥ 20% duty-cycle floor

**No source reports a cycle duration, frame count, or safe-window percentage
for any FromSoft periodic hazard.** Not for Sen's pendulums, not for the
chariot, not for the ballista. Speedrun frame data on these did not surface.
The number is genuinely undocumented in accessible literature.

What is consistently attested is the *qualitative* rule set, and all three parts
matter more than the ratio:

1. **Observable from safety before committing** — you can stand and watch a full
   cycle. (This is the strongest, most universal one.)
2. **Deterministic and repeatable**, never randomised.
3. Often **disable-able** (§5.2).

**Verdict.** Keep ≥ 20% — it is a defensible floor and it is cheap to prove —
but the dossier's honest position is that it is *our* invention, not a measured
property of the genre, and it is also **not the load-bearing constraint**. A
gate with a 50% duty cycle that the player cannot see before stepping into it is
worse than a 20% gate they watched for ten seconds. If only one proof can be
afforded, prove **observability** — a standable cell outside the gate's span
with line of sight to it — not the ratio.

### 5.4 Two premises to strike

- **Elden Ring Divine Tower lifts** are activate-and-ride, not timing puzzles.
- **DS3 Profaned Capital's lift** is likewise straightforward — defeat the
  giant, ride up, open the shortcut. No lever-timing puzzle exists there.

Neither is a precedent for timed-lift design. (Sen's twin cage lift — right cage
rises, wrong cage drops you to the bottom — is a real choice-under-observation
mechanism, and both cages are visible at once, which is §5.3 rule 1 again.)

---

## 6. Bonfire and checkpoint philosophy

### 6.1 What a bonfire is *for*

Mechanically it saves progress, restores health/magic/Estus, allows levelling
and warping. But Miyazaki's stated intent is social and tonal — bonfires were
meant to be flexible spaces where players could

> "gather together and communicate – not verbally communicate, but emotionally
> communicate"

— envisioned as "centers of relaxation" with a heartwarming tone inside a dark
fantasy world. Critics received them as a **"physical manifestation of relief"**
([Wikipedia, *Bonfire (Dark Souls)*](https://en.wikipedia.org/wiki/Bonfire_(Dark_Souls)), CC BY-SA, quotable).

For a 1–4 player co-op delve this is directly actionable and we have been
under-using it: the bonfire is a *staging* beat, not just a respawn anchor. It
is where the party regroups and where dialogue lands.

### 6.2 What resets on rest — precise, per game

| | Ordinary enemies | Mini/sub-bosses | Bosses | Named elites | Items | Shortcuts/levers |
|---|---|---|---|---|---|---|
| **DS1** | respawn | — | no | **Black Knights do NOT respawn**, except the five in the Kiln of the First Flame | no | persist |
| **DS3** | respawn | — | no | **Black Knights DO respawn** — a genuine cross-game rule change | no | persist |
| **Elden Ring** | respawn on grace rest **and** on the late-night→morning transition (~20–25 min cycle) for enemies not near you | — | no | field bosses (Tree Sentinel, Night's Cavalry) do not respawn | no | persist |
| **Sekiro** | respawn on idol rest | **sub-bosses DO respawn** | no | — | no | persist |

Sources: [Fextralife Bonfire](https://darksouls.wiki.fextralife.com/Bonfire),
[Black Knight](https://darksouls.wiki.fextralife.com/The+Black+Knight),
[Sculptor's Idol](https://sekiroshadowsdietwice.wiki.fextralife.com/Sculptor's+Idol) — all ideas-only.

Two corrections to assumptions our spec carried:

- **"Mini-bosses stay dead" is not a series rule.** It holds for DS/ER field
  bosses; **Sekiro respawns its sub-bosses** on every idol rest.
- **"Elites stay dead" is DS1-specific.** DS3 respawns Black Knights. If our
  campaign wants a Black-Knight-style permanent elite, that is a deliberate
  DS1 citation, not a genre default.

Sekiro also attaches a *cost* to resting-by-dying that has no analogue
elsewhere: dying and respawning can inflict **Dragonrot** on an NPC, which
lowers Unseen Aid (the chance not to lose XP/currency on death) from its 30%
default; using the mid-combat Resurrection does not
([Fextralife Dragonrot](https://sekiroshadowsdietwice.wiki.fextralife.com/Dragonrot), ideas-only).

**Shortcut persistence is an inference, not a quoted rationale.** No source
states "the world resets, your progress persists" as a design principle; it is
synthesised from the mechanics being uniform across four games. Nothing
contradicts it. Our `shortcut{gate,unlock}` permanence rule is correct — just
don't cite a developer for it.

### 6.3 Sightline teases — an unfilled gap

The brief asked for named examples of a bonfire visible but not yet reachable.
**None was found.** The closest attested structure is the *inverse*: Sen's
Fortress withholding a bonfire entirely as an escalation device (§5.1). Either
the sightline tease is rarer than folklore suggests, or it is undocumented. Do
not build a delve beat on the assumption that it is canonical practice — though
nothing argues against inventing it.

### 6.4 The Bloodborne lamp claim is false

Our research brief carried the belief that patch 1.03 added lamp-to-lamp
travel. **The v1.03 notes contain no lamp or fast-travel change at all** —
1.03 covered load times, elevator fixes, boss-immobilisation fixes and
multiplayer exploits. Bloodborne **never** shipped official lamp-to-lamp
warping; the only route is the third-party PC mod Lamp2Lamp.

The criticism is real (every trip routes through the Hunter's Dream, two
loading screens instead of one), and the *rationale* — forcing the hub
round-trip pushes players to spend Blood Echoes before risking them, and forces
them to learn Yharnam's layout and open its shortcuts instead of skipping the
city — is secondary-source inference with no developer quote behind it
([ScreenRant](https://screenrant.com/bloodborne-no-fast-travel-real-reason-forgiving-easier/), ARR).

That rationale, however inferred, is the strongest argument in the dossier for
**why our delves should have no fast travel**: it is what keeps §1's loop-back
worth building.


## 7. Risk-reward vocabulary — the trades our verbs must stage

### 7.1 Guarded treasure, and escalation as the fair form

The Level Design Book's teach/test/twist reading (§4.4) is the reusable shape:
the reward-guarding ambush is fair when it is the **third** instance of a
pattern the level already taught twice. A delve that opens with its cleverest
ambush has spent the lesson before teaching it.

### 7.2 Poison swamps as toll roads

Miyazaki, on Elden Ring (developer primary, short quote):

> "when making the game I rediscovered my love for making poison swamps. I know
> how people feel about them, but you know, suddenly I realize I'm in the middle
> of making one and I just can't help myself."
> — [Game Informer, 2022-01-28](https://gameinformer.com/2022/01/28/hidetaka-miyazaki-rediscovered-his-love-of-creating-poison-swamps-in-elden-ring)
> (also covered by [PC Gamer](https://www.pcgamer.com/elden-ring-has-multiple-poison-swamps-because-i-cant-help-myself-miyazaki-says/))

He later judged that he "went a little too far" with Elden Ring's swamps and
varied the hazard vocabulary for Shadow of the Erdtree
([PC Gamer](https://www.pcgamer.com/games/action/miyazaki-went-a-little-too-far-with-elden-rings-poisons-swamps-but-says-he-learned-a-lesson-which-unfortunately-is-that-he-needed-to-come-up-with-new-and-different-ways-to-kill-everyone/),
[GamesRadar](https://www.gamesradar.com/elden-ring-director-cant-stop-making-poison-swamps-says-shadow-of-the-erdtrees-one-was-a-point-of-introspection-and-reflection-for-me/) — ARR).

**Terminology flag:** "toll road" is *our* framing. It is not attested critical
vocabulary; no source uses it. The attested mechanics for Blighttown are:
movement is slowed, and the swamp is scattered with rewards gated behind
poison-resistance gear (Rusted Iron Ring, Purple Moss Clumps, Poisonbite Ring,
Thief/Pyromancer sets) ([Fextralife](https://darksouls.wiki.fextralife.com/Blighttown), ideas-only).

The reusable anatomy is four compounding levers:

1. **Movement debuff** (wading).
2. **Attrition, not burst** — the cost is *duration of exposure*.
3. **A consumable tax** — the fare (resist items, gear).
4. **An available dry path** — the load-bearing one.

**A swamp with no dry path is not a toll road, it is a tax.** Same line as
§3.3's bypass legibility, and the same line Miyazaki himself concedes he
crossed.

### 7.3 Illusory walls — and why we should probably not ship them

Sourced lineage ([Vice, Klepek](https://www.vice.com/en/article/be-wary-of-liar-the-weird-history-behind-elden-rings-illusory-walls/), ARR):
the mechanic predates Souls (King's Field IV used a colour-shift item to reveal
them). **Demon's Souls had three, and they jiggled** — a visual tell inviting
investigation. Dark Souls removed the jiggle and expanded to seventeen. DS2
escalated to about twenty-eight and broke its own convention by adding
press-to-interact and barrel-explosion variants. Bloodborne pared back to five;
DS3 returned to twenty-six. DS1's single Sen's Fortress wall that requires
attacking rather than rolling is attributed by the researcher known as
"Illusory Wall" to devs copy-pasting the wrong object defence value — i.e. the
series' most-argued-over inconsistency is probably a bug.

The compensation for removing the tell was **crowdsourced**: the player message
system — which promptly became a troll vector (false "secret ahead" messages at
dead ends).

**A single-sitting 1–4 player delve has no message crowd.** An uncued secret in
a delve is a secret nobody finds. If we stage secrets, they need an in-world
cue — the Demon's-Souls answer, not the Dark-Souls-1 one.

### 7.4 The currency loop — an honest gap

Souls / blood echoes / runes drop where you died and are recoverable exactly
once. This is the series' fundamental risk/reward engine, and **no developer
statement justifying it specifically was found** — the searchable Miyazaki
commentary is general death-as-learning framing. Recorded as an unfilled
citation rather than papered over.

Our no-grind rule (CLAUDE.md) forbids the farmable half of this loop anyway;
what survives translation is the *recoverable-once* shape, which needs no
currency — a dropped quest item, a one-shot retrieval, works identically.

---

## 8. What does not translate — and the honest substitutes

Vanilla adventure mode has no stamina, no dodge, no i-frames, no poise, no
parry, no lock-on, no builds. These are not "hard to implement" — several have
**no vanilla primitive at all**, so under the no-hack doctrine they are
excluded, not approximated.

| Souls system | What it actually does | Vanilla status | Our honest substitute |
|---|---|---|---|
| **Stamina** | Gates attack/roll/block/sprint; the pacing governor that punishes the fifth swing | **No primitive.** Hunger gates sprint on a far slower, non-combat timescale | None. Do **not** emulate with a scoreboard bar — that is the hack. Substitute the *function* (commitment cost) with geometry |
| **I-frames / roll** | "The dodge is the defence" — the read that defines souls combat | **No primitive.** No dodge input exists | None. Defence becomes **positioning and routing** |
| **Poise / hyperarmor / stagger** | Invisible meter deciding whether a hit interrupts you | **No primitive.** MC hit-stun is flat | None |
| **Backstab / parry / riposte** | Positional and timing criticals | **No primitive.** Attack is one facing-agnostic swing | None |
| **Lock-on camera** | Strafe-relative-to-target; structures all spacing | **No primitive** | None |
| **Builds / equip load** | Ties stat choice to *combat timing* | Armor is flat mitigation | Class kits shift **verbs**, not timing: reach, ranged, mobility |
| **Estus economics** | Limited heals, refilled only at a bonfire; drinking is a punishable commitment | **Reproducible** — checkpoint-bound charge resets and item-use timing are within datapack reach | **Green zone — build it.** The one combat-economy piece that ports cleanly |
| **Farmable healing** | Bloodborne's blood vials | Reproducible | **Rejected.** Collides head-on with our no-grind rule, and is the series' own acknowledged regression |

### 8.1 The five verbs we actually have

Strip the combat layer and souls leaves: **positioning, routing, item use,
timing** — plus the meta-verb, **information**. Every mechanic in §§1–7 that
survives translation resolves into those five.

### 8.2 Is that still souls? Both sides

**For.** The genre demonstrably survives losing stamina. Hollow Knight is
routinely classed as a soulslike despite having no stamina bar at all; the
absence is treated as a *benefit* for its platforming (the Path of Pain), with
the focus moved onto precision and movement. What it keeps is precisely our
list: a corpse run to retrieve lost currency, and checkpoints the player must
go out of their way to find and activate; its skeleton is a connected map
gated by traversal ability rather than stats
([GameRant, *Best Soulslike Games Without A Stamina Bar*](https://gamerant.com/best-soulslike-games-no-stamina-bar/),
[Gfinity](https://www.gfinityesports.com/article/hollow-knight-is-a-soulslike-and-heres-why-we-should-stop-pretending-it-isnt) — ARR).

The Minecraft prior art points the same way. Every vanilla-legal souls attempt
found confines itself to exactly the layer we can reach: **SoulsCraft**
([Modrinth](https://modrinth.com/datapack/soulscraft)) and *Dark Souls System in
Vanilla Minecraft* ([PlanetMinecraft](https://www.planetminecraft.com/project/dark-souls-system-in-vanilla-minecraft/))
both implement bonfires that set spawn, restore health and Estus charges, and
carry a souls-levelling layer — and nothing resembling stamina or rolling.
Reproducing souls *combat* in Minecraft requires a mod
([Bonfires Mod](https://www.curseforge.com/minecraft/mc-mods/bonfires)). The
checkpoint-and-level layer is the part that ports; the market has already
proven it twice.

**Against.** A real part of the audience gatekeeps the label on stamina-based
combat, and genre-boundary arguments turn on whether a stamina bar is
*integrated* or merely present (same GameRant piece). Some hold that Hollow
Knight is a Metroidvania that borrows souls furniture rather than a soulslike
([Android Police](https://www.androidpolice.com/is-hollow-knight-silksong-a-soulslike/), ARR).

**The resolution the evidence supports:** you may drop a combat subsystem, but
you must **replace the tension it carried** — you cannot merely delete it.
Hollow Knight replaced stamina tension with checkpoint distance and map
commitment.

**Our replacement, as a design contract:** the tension stamina used to carry is
carried by **committed geometry** — one-way drops, sealed shortcuts, timed
gates, lethal parkour, bonfire distance — and by **a finite, bonfire-refilled
heal**. A delve with neither is not a hard souls delve; it is a walk.

---

## 9. The review checklist — "does this delve speak souls?"

Reviewable against a finished campaign DSL. A delve should clear most of these;
failing more than a third means it wears the costume without the grammar.

**Structure**
1. Is there at least one **two-route loop** between rest points — a long,
   dangerous route and a short route sealed until opened from the far side?
2. Does at least one shortcut open back toward a **hub**, not just shorten the
   current level? (§1.2 — the world-folds-in reveal is a distinct beat.)
3. Is the shortcut's unlock **permanent**, and does it visibly pay (the short
   route is genuinely shorter)?
4. Is there **no fast travel** competing with the loop? (§6.4)
5. Is there a **point of no return by geometry** somewhere — a one-way drop or
   sealed gate the player chooses to cross?

**Encounter**
6. Does every ambush have **at least one second-read counterplay** — lure,
   position, ranged opening, or route-around? (§2.2 axis 2)
7. Is every hazard **observable from a safe cell before the player commits**?
   (§2.2 axis 5, §5.3 — the strongest single rule in the dossier.)
8. Is every hazard **deterministic and repeatable**?
9. Is at least one hazard **disable-able**, not merely dodgeable? (§5.2)
10. Is there **no boss-with-adds in a sealed small room** opening with an
    instant lunge? (§2.1 — the anti-pattern.)
11. Does ambush density follow **teach → test → twist** rather than a flat
    sprinkle? (§4.4)
12. Is fairness carried by **sightline and silhouette**, not by a sound cue?
    (§4.3)

**Optional content**
13. Is there an **optional elite** near the start that is over-tuned and
    walk-past-able?
14. Is its bypass **legible** — no fog gate, open ground, and a dormancy or
    leash tell? (§3.3)
15. Does the elite **stay dead** once killed, so the return trip reads as
    earned? (§6.2 — a DS1 citation, not a genre default.)

**Economy**
16. Are heals **finite and bonfire-refilled**, never farmable? (§8)
17. Is there a **guarded reward** whose risk the player can price before
    committing?
18. Is there a **toll-road hazard with a dry path** — attrition the player can
    pay in exposure time, or route around? (§7.2)

**Retry**
19. Is the retry cost **short enough to be an investment** — and measured per
    failure point, not per campaign? (§1.4)
20. On rest, does the **world reset while progress persists** — enemies and
    traps re-arm, shortcuts and items stay?
21. Is every death **legible as the player's own over-extension**?

**Secrets**
22. Does every secret have an **in-world cue**? (§7.3 — no message crowd exists
    in a delve.)

---

## 10. Mapping to our primitives — and the gap list

`✅` stageable today · `🟡` stageable but under-specified · `❌` no primitive
(gap; **no hacks proposed**).

| Grammar element | Our primitive | State |
|---|---|---|
| Rest point, world-reset-on-rest | `bonfire{anchor}` + `on_rest[]` (spec-0016 §1) over `set-checkpoint`/`on_respawn` (spec-0012); trap `reset: rearm`; `respawns_on_rest` waves | ✅ |
| Progress persists across rest | items/flags never reset by construction; `shortcut` gate permanence is a compile error to violate | ✅ |
| Two-route loop-back, sealed short route | `shortcut{gate, unlock}` + frontier proof (unlock reachable only via the long route) + payoff proof | ✅ |
| Point of no return | `close-gate` + the DAG-causal seal in nav (`DW0311`/`DW0315`) | ✅ |
| One-way drop | fall geometry + `DW0315` no-stranding | ✅ |
| Ambush at a corner/doorway | `ambush{at,actors[],trigger,telegraph?}` over deferred NPCs/actors + `approach`/`strike` | ✅ |
| Deterministic re-encounter | ADR-0006 determinism | ✅ |
| Timed hazard with a readable window | `timed-gate{open_ticks,closed_ticks,phase?}`, ≥ 20% duty cycle | 🟡 — ratio is proven, **observability is not** (G1) |
| Disable-able hazard | `traps[].disarm{via,sets_flag}` | ✅ for traps, ❌ for `timed-gate` (G2) |
| Lethal parkour | jump edges, envelope = measured-max − 1, `parkour: true` (spec-0016 §5) | ✅ |
| Routed-then-feral lane mobs | `wave.lane{waypoints[],aggro_radius}` over vanilla raider patrol | ✅ |
| Optional elite with a bypass | optional-elite proof (a route avoiding its aggro radius) | 🟡 — proves existence, not **legibility** (G4) |
| Elite that stays dead | absence of `respawns_on_rest` on that wave | ✅ |
| Guarded visible reward | `collect` + `prop` chest at an ambush anchor | ✅ |
| Toll-road attrition hazard | area `mitigation` + `damage-players{amount,in}` over a zone; dry path = ordinary geometry | ✅ |
| Finite bonfire-refilled heal | — | ❌ **G3** |
| Boss fog gate as a mandatory marker | `close-gate` seals behind; nothing marks *ahead* | 🟡 (G5) |
| Secret with an in-world cue | `set-block` / `play-sound` / lighting | ✅ (authoring discipline, not a primitive gap) |
| Retry-cost budget | ≤ 60 s lint (spec-0016 §7) | 🟡 — **effectively inert at box-garden scale** (G6) |
| Sightline tease (visible unreachable bonfire) | ordinary geometry | ✅ — and unattested in the real games (§6.3) |
| Stamina, i-frames, poise, parry, lock-on | — | ❌ **excluded by no-hack doctrine, not a gap to fill** |

### The gap list (for the planner — no hacks proposed)

- **G1 — hazard observability is unproven.** Our timed-gate proof checks the
  duty-cycle ratio. The dossier's strongest and most universal finding (§5.3,
  §2.2 axis 5) is that the real games guarantee something else: you can stand
  somewhere safe and watch a full cycle before committing. **Proposed
  obligation:** a standable cell outside the gate's span with line of sight to
  it, from which the approach is reachable. This is a nav + visibility query we
  already have most of the machinery for (the cutscene clip check does line
  segments through the occupancy grid). If only one of the two proofs survives,
  it should be this one, not the 20%.
- **G2 — `timed-gate` has no `disarm`.** `traps[]` reaches the third rung of the
  readable → avoidable → disable-able ladder; timed gates stop at rung two.
  Vanilla has the primitives (a lever interact setting a flag that suppresses
  the clock), so this is a DSL surface gap, not a ceiling.
- **G3 — no finite, bonfire-refilled heal.** The single most portable piece of
  souls' combat economy, and we have no verb for it. `give-item` grants; nothing
  *caps and refills at rest*. This is the highest-value addition the dossier
  found. Reachable in vanilla (item counts + `on_rest[]`), so again a surface
  gap, not a ceiling.
- **G4 — optional-elite bypass is proven but not legible.** §3.3 shows the real
  games spend three further signals on it. The cheapest to add is **dormancy**:
  an elite that is passive until provoked (the Heide Knight tell). We have no
  way to declare a hostile-but-dormant wave — vanilla `NoAI` puppets exist
  (`actors`), and `unleash-actor` already converts a puppet to a real-AI twin,
  so the pieces are present; what is missing is a *player-provoked* unleash
  trigger. Flagging, not designing.
- **G5 — no "this is mandatory" marker.** The fog gate is FromSoft's clearest
  piece of information design and we have no dual. Everything we have marks
  what is *sealed*, not what is *committed-to-ahead*. Worth a spec conversation;
  a `narrate`-plus-prop convention is authoring discipline, not a primitive.
- **G6 — the retry-cost lint is inert.** At 4.317 m/s walking, 60 s is ~259
  blocks; a box-garden delve is rarely that wide, so the lint will essentially
  never fire (§1.4). It is not wrong, it is unbinding. Either scale it to the
  delve's own dimensions or drop it and rely on bonfire-density review.

---

## 11. Provenance

Research conducted 2026-08-02 by web survey (developer interviews, trade-press
design analysis, level-design case studies, wikis, community deconstruction).
Licence triage per the table at the head of this note; ledger entries in
[`docs/ACKNOWLEDGEMENTS.md`](../ACKNOWLEDGEMENTS.md). No third-party text is
reproduced beyond short attributed quotes from permissively-licensed or
fair-use-quotable sources; Fextralife and forum sources are cited as
*ideas-only* and never quoted.

Claims deliberately **not** made, because the research did not support them:
that FromSoft gives long-range audio ambush warnings; that DS3 always places a
bonfire before a fog gate; that Bloodborne patched in lamp-to-lamp travel; that
Elden Ring Divine Tower or DS3 Profaned Capital lifts are timing puzzles; that
mini-bosses universally stay dead; that any measured duty-cycle figure exists
for a FromSoft periodic hazard; that "toll road" is attested critical
vocabulary; that a named "visible but unreachable bonfire" example exists.
