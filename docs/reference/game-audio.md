# Game audio — how a delve's music is chosen

For the agent deciding what each scene of a campaign plays: a track, or silence, per place and per story stage. The engine's surface for it is spec-0109 (`audio/background_music` and `audio/music_volume` on every atmosphere and on the ground, `set-atmosphere` to move it, `stop-sound` to cut, `hears` on a cutscene); this page is the craft the surface serves. Every rule is marked **cited** (with its source) or **authored** (this project's rule, with the reason it exists). An authored rule is a decision, not a finding.

## 1. Music is a state of the place

- **A1 — Music answers the state of the world, not the player's input.** Music that changes with what the game world is (where the camera stands, what has happened there) is the *adaptive* class; music that answers a button is the *interactive* class. Area music is adaptive: the world changed, so the music did (**cited**: Collins, *Game Sound*, ch. 1 — the adaptive/interactive distinction).
- **A2 — Design music as a set of states joined by transitions.** The working structure of an adaptive score is a set of music states — one per area or situation — with composed transitions between them and short stingers over them for events; a state is entered when the game says so, and the transition decides how the old music leaves (**cited**: Sweet, *Writing Interactive Music for Video Games*, ch. 8 *Horizontal Resequencing* and ch. 10 *Writing Transitions and Stingers*; the Wwise interactive-music model of switch containers, states, transitions and stingers). In a delve a state is an atmosphere's music declaration, a state change is a `set-atmosphere` repaint, and the transition is the declaration's `replace_current_music` and the destination's `music_volume` (spec-0109 §3.1).
- **A3 — A scene that must own its music replaces what plays; ambient music waits.** The pinned game's own constants say it: the boss and the credits replace the current track with no delay; overworld and underwater game music never interrupts and waits ten to twenty minutes between pieces (**cited**: `Musics.<clinit>`, spec-0109 §2.1). Write a boss, an ending or a revelation as `replace_current_music: true` with `min_delay` and `max_delay` 0 or small; write a region's ambient music with no replace and long delays.

## 2. Silence is a state

- **S1 — Declare silence; never leave it to the game.** A biome that says nothing inherits the dimension's calm overworld music (**cited**: `dimension_type/overworld.json`, spec-0109 §1). Every atmosphere and the ground declare `audio/background_music` — `{}` is the word for silence (**cited**: the pinned `pale_garden` biome, which declares `{}` with `music_volume: 0.0`).
- **S2 — Silence is a weight, spent where the place must carry itself.** The *Dark Souls* line scores bosses, hubs and endings and leaves exploration to its own ambience, so the places without music feel hostile and alone and the music, when it comes, lands (**cited**: Canela, *The Sound of Dark Souls*; Freitas, *From epic fail to epic music*, JoSSIT 3). A hidden room with music is a reward for finding it (**cited**: Canela). A delve's exploration spaces may be silent by design; the ambience (`audio/ambient_sounds`, spec-0080) is what the player hears there.
- **S3 — A fade is a threshold; a cut is a shock.** Walking into a silent place fades the music out over about fifteen seconds when the place declares `music_volume: 0.0`; a sudden hush is `stop-sound music` at the beat, or at a trigger on the threshold (**authored**, on the pinned fade curve of spec-0109 §2.1 and on A2's transition kinds: the game offers exactly these two shapes, and which one a scene needs is the creator's call).

## 3. Tracks

- **T1 — A named event may be a set.** The client draws one file at random each time an event starts: `music.overworld.forest` is one of twenty-one pieces, `music.game` one of twenty-seven; every `music_disc.*` event and `music.credits`, `music.dragon`, `music.end` are one piece each (**cited**: the pinned client's `sounds.json`, spec-0109 §2.6). Name a single-file event for a specific piece; name a set for a mood.
- **T2 — The toast names the track.** A vanilla track announces itself and its artist in the player's now-playing toast, under the player's own option (**cited**: spec-0109 §2.1). A creator who names a vanilla piece names what the toast will show.
- **T3 — The player's options shorten, never lengthen.** The music-frequency option caps a declared gap at twenty, ten or zero minutes and the music slider scales the gain; a declaration states the longest gap and the loudest level the player will get (**cited**: `MusicManager$MusicFrequency`, spec-0109 §2.1).
- **T4 — Vanilla's events only.** A track of the campaign's own is a resource-pack sound under ADR-0013's licence families; until that spec lands, the vocabulary is the pinned registry (**cited**: spec-0109 non-goals).

## Sources

- Audiokinetic. *Wwise* interactive music: switch containers, states, transitions and stingers — the Wwise Help pages *Understanding stingers* and the transition matrix, and Audiokinetic's *Creating Stingers* tutorial.
- Canela, D. *The Sound of Dark Souls*. Game Developer (ideas only, as in `docs/ACKNOWLEDGEMENTS.md`).
- Collins, K. *Game Sound: An Introduction to the History, Theory, and Practice of Video Game Music and Sound Design*. MIT Press, 2008.
- Freitas, J. *From epic fail to epic music: music, silence and failure on "Dark Souls 3"*. Journal of Sound, Silence, Image and Technology 3, 55–74, 2020.
- Sweet, M. *Writing Interactive Music for Video Games: A Composer's Guide*. Addison-Wesley, 2014 — chapters 8 and 10, read from the published table of contents.
- The pinned 1.21.11 game, as spec-0109 §2 reads it.
