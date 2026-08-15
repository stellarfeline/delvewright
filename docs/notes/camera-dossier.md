# Camera dossier — parameterizing `shot_style`

- **Status**: research note. Input to
  spec-0015's "Shot-style template library" section. No code changes.
- **Question**: spec-0015 asserts that RDR2/GTA V cinematic mode and Unity
  Cinemachine are *algorithmic, not AI* — "a curated template bag, damped
  look-at springs, per-template lens/FOV, min/max shot durations,
  cut-on-occlusion". Verify, refine, and turn into concrete `shot_style`
  presets our 20 Hz spectator dolly can actually deliver.

## 0. Verdict on the hypothesis

| Claim | Verdict |
|---|---|
| Curated template bag, discrete selection | **Confirmed** — and from a primary source. Obbe Vermeij (ex-Rockstar North technical director) records that GTA III's cinematic cam "switch[ed] between random viewpoints near the track", with a wheel cam and a chasing-car view added for vehicles. Discrete templates + randomized pick, from inception. |
| Damped look-at | **Confirmed as industry standard** — Cinemachine's Composer is exactly this (damping, dead zone, soft zone). Not a Rockstar-attributable claim. |
| Per-template lens/FOV | **Refuted for us.** Real in the references; **impossible in vanilla Java** (§1). FOV must be replaced by camera *distance* in our DSL. |
| Min/max shot duration | **Not documented for GTA/RDR2 at all** — no press, wiki, modder, or dev source gives a seconds figure. Take the numbers from film-editing research instead (Galvane et al., §4), which does publish them. |
| Cut-on-occlusion | **Not documented for GTA/RDR2** either (only indirect hints: the camera declines to follow under bridges). It is real in Cinemachine (Deoccluder + ClearShot) and in the academic literature. **We should not copy it as a runtime behaviour** — see §4: our worlds are static, so occlusion is decidable at *compile* time, which is strictly better and needs no vanilla primitive we lack. |

Net: the template-bag architecture is sound and well-sourced. Every *number*
in the spec sentence has to come from Cinemachine defaults and the editing
literature, not from Rockstar — that layer is simply not public.

## 1. The vanilla ceiling (measured against what we emit today)

Java 1.21.11 has no `/camera` (Bedrock does). Our camera is: `gamemode
spectator` + per-tick `tp` of two `item_display` proxies + alternating
`spectate` between them (`crates/compiler/src/emit.rs::cutscene_fns`).

- **Player self-teleport is not interpolated.** The local player's camera is
  driven by client prediction, not netcode interpolation, so a server `tp`
  lands as a discrete jump. This is why spectating a *proxy entity* is the
  right architecture — we already do this.
- **`teleport_duration` is the primitive we are leaving on the table.**
  Display entities (1.19.4+) carry `teleport_duration` (int ticks, clamped
  **0–59**): the client tweens the entity's position across a teleport over N
  ticks. Our camera `item_display`s are summoned **without it**, so the dolly
  is currently a hard 20 Hz staircase — at 60–144 fps each tick's pose is held
  for 3–7 frames. Setting it converts the same command stream into
  client-side smooth motion. This is a vanilla-intended field, not a hack.
  - Better still: **keyframe cadence = `teleport_duration`**. Emitting a
    waypoint every N ticks with `teleport_duration:N` lets the client draw the
    in-betweens — smoother *and* ~N× fewer commands than our current
    every-tick `tp` (spec-0008 budgeted ~40 pkts/s/player). Cost: the client
    interpolates linearly, so N chords a curved path — N must be bounded by
    path curvature, which the compiler knows.
  - **Unverified, and the implementation's first measurement**: (a) whether
    `teleport_duration` interpolates the entity's *rotation* as well as its
    position, and (b) whether our per-tick re-`spectate` bounce (which exists
    to defeat sneak-escape) resets an in-flight interpolation. Mojang bug
    MC-279534 and a Paper resync issue both show this area has real quirks.
    Measure before adopting; do not assume.
- **FOV is not settable.** No command sets it. The only vanilla lever is the
  dynamic-FOV response to Speed/Slowness, and that is doubly dead for us:
  spectator flight speed ignores those effects, and FOV changes propagate to a
  spectator only when the spectated entity **is a player** — ours is an
  `item_display`. Consequence: `fov` stays a *render-tier* parameter (Chunky /
  `delvec snapshot`) and never appears in an in-game `shot_style`. "Lens feel"
  in-game is produced by **camera distance only**.
- **No roll.** `tp <rot>` has yaw and pitch; there is no Dutch angle.
- **Our own gaps, independent of vanilla**: `lerp_polyline` parameterizes by
  *segment index*, not arc length — a 3-block segment and a 30-block segment
  get equal time, so speed jumps at every waypoint. And `s = k/ticks` is
  linear, so every move starts and stops instantly. Both are emission-time
  bugs a `shot_style` layer should fix by construction.

**Smoothness budget (proposed, to be calibrated empirically).** A camera at
distance `d` moving at `v` blocks/s perpendicular to the subject sweeps
`2.87·v/d` degrees per tick. Rotation is the binding constraint once
`teleport_duration` handles position. Proposed: **≤ 2 °/tick (40 °/s)
comfortable, > 6 °/tick (120 °/s) a compile error**. Translation without
interpolation: **≤ 3 blocks/s**; with `teleport_duration` set, unbounded
within the curvature limit above.

## 2. Proposed `shot_style` preset set

Every style expands deterministically to `path[] + look_at + ticks`. `dist` is
in blocks from the subject and is the *only* lens control we have. Durations
are anchored on the film baseline in §4.

| Style | Camera behaviour | Look-at | `dist` | When to use | Dur (s) | 20 Hz fidelity |
|---|---|---|---|---|---|---|
| `insert` | Fully static. No translation, no rotation. | Fixed point | 2–4 | A prop, an inscription, a corpse. The beat that always looks right. | 1.5–3 | **Perfect** — nothing moves, judder is structurally impossible. |
| `locked_off` | Static position; only the aim turns as a moving subject passes. Rockstar's roadside "ground view". | Tracks moving subject | 8–20 | An NPC or party walking past camera; arrivals. | 4–10 | **Excellent** for position (zero), rotation-limited: enforce min `dist` so peak yaw ≤ 2 °/tick. |
| `push_in` | Straight dolly toward the subject along the view axis; medium → close. | Fixed subject | 12 → 4 | Dread, dawning realisation, a line of dialogue landing. | 3–6 | **Excellent** — near-zero angular rate; the translation is the shot. |
| `pull_back_reveal` | Reverse of `push_in`: close → wide, revealing context. | Fixed subject | 4 → 16 | "You are not alone"; scale reveals. | 4–8 | **Excellent**, same reason. |
| `establishing_crane` | High and far, descending and closing on the subject. | Fixed subject | 24 → 12, Δy −8 | First sight of an area. spec-0015 grade `establishing`. | 6–12 | **Very good** — long baseline, slow, small angular rate. |
| `orbit_arc` | Constant-radius, constant-height arc, 45–120° around the subject. Cinemachine Orbital Follow. | Fixed subject | 8–16 | Showing a structure or a standing figure in the round. | 5–10 | **Good** — requires the compiler to emit the arc as a dense polyline; cap deg/tick. |
| `side_track` | Parallel dolly abeam a *moving* subject at constant offset — GTA V's signature. Needs the subject's path, which the compiler already owns for `move-npc`. | Tracks moving subject | 6–12 | Walk-and-talk; a procession; escorting. | 4–12 | **Good** — relative bearing barely changes, so angular rate stays low. |
| `two_shot` | Static placement solved so **both** subjects land on opposite thirds (Toric-space construction, §3). | Midpoint, biased | 5–9 | Two NPCs talking; confrontation. | 3–8 | **Excellent** — static. |
| `low_follow` | Low, close, trailing the subject near ground level. RDR2's low chase. | Tracks moving subject | 3–6 | Chases, urgency, a creature stalking. | 3–8 | **Worst case — flag it.** Close + relative motion = the highest angular rate of the set. Only viable with `teleport_duration`; needs a hard subject-speed cap. |

Selection is authored, not random (Rockstar randomizes; we are authoring a
scripted delve, so the LLM picks and the compiler proves). Where a style
admits several placements (`orbit_arc` start angle, `two_shot` side), the
compiler enumerates candidates and picks the best-scoring one — see §4.

## 3. Framing vocabulary worth adopting

From Cinemachine's Composer / Framing Transposer, with their defaults —
useful precisely because they are a decade of shipped tuning. Ranges given as
Cinemachine's; our DSL should narrow to `[0,1]`.

| Knob | Cinemachine default | Our reading |
|---|---|---|
| `screen_x` / `screen_y` | 0.5 / 0.5 (range −0.5…1.5) | Where the subject sits in frame. Rule-of-thirds presets: 0.333 / 0.667. |
| `dead_zone` (w, h) | 0 (range 0…2) | Subject may drift this far before the camera re-aims at all. |
| `soft_zone` (w, h) | 0.8 (range 0…2) | Beyond the dead zone, correction ramps in over this band. |
| `bias` (x, y) | 0 (range −0.5…0.5) | Offsets the zones — leading room ahead of a walking subject. |
| `damping` (h, v) | **0.5 s** (Composer); 1.0 s (Framing Transposer position damping) | Seconds for the aim to catch up. |
| `camera_distance` | 10 | A sane default medium-shot `dist`. |
| default blend | EaseInOut, 2.0 s (Brain) | Our *within-shot* easing curve. |
| ClearShot default blend | **Cut, 0 s** | Vindicates our hard cut between shots — a cut is the convention, not a limitation. |
| `min_duration` / `activate_after` (ClearShot) | 0 s | We should set real per-style minima instead (§2). |

**Damping must be baked, and can be.** Cinemachine damping is a per-frame
spring — nondeterministic-by-construction state we are forbidden from
emitting. But we do not need runtime state: the subject is either a static
anchor point or an NPC on a **compiler-planned** `move-npc` path. So the
compiler can run the dead-zone / soft-zone / damping filter offline across the
shot's tick samples and emit the resulting yaw/pitch per keyframe as
constants. Same visual result, pure function of the DSL, byte-identical under
ADR-0006. This is the correct layering: springs are an authoring-time
*algorithm*, never a runtime *mechanism*.

Same trick fixes §1's two emission bugs: arc-length reparameterize the
polyline, then apply the style's easing curve to the arc-length parameter.

## 4. Cut and composition rules as compile-time assertions

The editing literature publishes the numbers Rockstar does not. All of these
are decidable statically against the assembled voxel model, which makes them
DW diagnostics rather than runtime behaviours — and they slot directly into
spec-0015's pillar 4 ("shot grammar = the assertion").

- **Shot duration.** Galvane et al. model shot lengths as log-normal with
  Average Shot Length `ASL = exp(µ + σ²/2)`; their *Back to the Future*
  reference scene measures **ASL ≈ 6.6 s**, with generated fast-cut and
  slow-cut variants at **2 s** and **10 s**, and a **30 s** maximum-shot
  horizon. Our per-shot clamp is 1–400 ticks (0.05–20 s), which comfortably
  contains this; the §2 min/max columns are anchored on it.
- **30° rule.** Two consecutive shots on the same subject must differ by a
  **minimum view-angle change of 30°** (or a clear shot-size change), else the
  cut reads as a jump cut. Trivially checkable between adjacent
  `CameraShot`s. Proposed new DW code.
- **180° rule / line of action.** For consecutive shots sharing two subjects,
  the sign of their on-screen x-position difference must not flip. Also
  statically checkable. Proposed new DW code.
- **Visibility.** `DW0308` today proves only that the *camera path* clears
  solid blocks; nothing proves the **subject is visible from the camera**. The
  references solve this at runtime (Cinemachine's Deoccluder; Oskam et al.'s
  visibility-aware roadmap with Monte-Carlo-precomputed inter-sphere
  visibility). We have no vanilla raycast primitive and, per the no-hack
  doctrine, should not invent one — but we do not need it: **our geometry is
  static and our subjects move on compiler-planned paths, so occlusion is
  decidable at emission.** Ray-march camera→subject over the voxel model at
  each sampled tick. That is strictly stronger than cut-on-occlusion: the
  reference systems *recover* from a bad shot, we *cannot ship* one.
- **Compile-time ClearShot.** Where a style has candidate placements, score
  each by (unoccluded fraction of the shot) → (distance vs. the style's
  optimal `dist`) → (subject inside the soft zone), ties broken by
  enumeration order. Deterministic, no RNG. Cinemachine's ClearShot resolved
  one layer earlier.
- **Toric space** gives the closed form for `two_shot`: parameterizing the
  camera as `(α, θ, φ)` about two subjects reduces the placement search from
  7-DOF to 4-DOF and yields *exact* on-screen positions for both subjects
  rather than a search. Worth implementing when `two_shot` lands.

## 5. Excluded — no vanilla primitive (per the no-hack doctrine)

FOV / focal-length changes and zooms (§1) · depth of field, rack focus ·
camera roll / Dutch angle · handheld Perlin shake (emittable as per-tick
jitter, but at 20 Hz it reads as stutter, not as handheld — exclude) ·
runtime occlusion recovery and camera wall-sliding (replaced by §4's
compile-time proof) · any per-frame spring or damping evaluated at runtime
(replaced by §3's baked constants).

## 6. Sources

Ledger entries added to `../ACKNOWLEDGEMENTS.md` in the same PR. Everything
here is **ideas-only**: no source was ported, and none is licensed for it.

**Rockstar / RAGE** — no GDC talk, engine paper, or `cameras.meta` schema for
the cinematic camera exists publicly; shot durations and cut triggers are
undocumented (independently re-verified 2026-08-01: searches for the cut
interval return only "it frequently changes"). Load-bearing sources: Obbe
Vermeij's dev-history account of the cinematic cam's origin, relayed by
TheGamer (fetched and quote-verified; primary-source dev — GTA III's
train ride was boring, so he made the camera "switch between random viewpoints
near the track", then added a wheel cam and a chasing-car view for vehicles);
GTAForums player threads for community shot names (folklore tier,
suggestive only — it confirms San Andreas introduced selectable angles
"helicopter, pedestrian, etc." but names no timings); and the invisible
truck-cab camera rig in *Father/Son*, reported by PCGamesN and torn down on
rage.re. That teardown is the single most useful Rockstar-technique datum we
found, and it argues **for** our `side_track` preset: Rockstar mounted the
cinematic camera on two phantom vehicles (invisible, invincible, no collision)
running the same vehicle *recording* as the chase, precisely because camera
animations cannot retime themselves to stay with a subject that speeds up and
slows down, whereas a vehicle recording can. Their fix for "keep pace with a
moving subject" was to put the camera on the subject's own motion track — which
is exactly what `side_track` does with a compiler-known `move-npc` path.
All unlicensed web content.

**Unity Cinemachine** — `docs.unity3d.com` package manual (CM 2.9 / 3.1) plus
`Unity-Technologies/com.unity.cinemachine` source for defaults the manual does
not tabulate. Licensed under the **Unity Companion License**, restricted to
Unity-dependent use: reference only, never ported. Source of every number in
§3.

**Academic** — Christie, Olivier & Normand, *Camera Control in Computer
Graphics*, CGF 27(8), 2008 (the STAR survey: taxonomy, shot classification by
body-part cutoff, line-of-action, rule of thirds; author-hosted PDF, ©
Eurographics/Wiley). Galvane, Ronfard, Lino & Christie, *Continuity Editing
for 3D Animation*, AAAI 2015 (the 30° rule, the 180° rule as x-order sign
reversal, log-normal ASL model and the 6.6 s / 2 s / 10 s / 30 s figures;
freely hosted by AAAI, © AAAI). Lino & Christie, *Intuitive and Efficient
Camera Control with the Toric Space*, SIGGRAPH 2015, and *Efficient
Composition for Virtual Camera Control*, SCA 2012 (§4's `two_shot`
construction; Inria/HAL open archive, © ACM). Oskam, Sumner, Thuerey & Gross,
*Visibility Transition Planning for Dynamic Camera Control*, SCA 2009
(visibility-aware roadmap; © ACM). Bares, Thainimit & McDermott, *A Model for
Constraint-Based Camera Planning*, AAAI Spring Symposium 2000. None is
CC-BY; none ships usable reference code; all ideas-only.

**Minecraft** — `minecraft.wiki` *Entity format/display entity* and
*Commands/spectate* and *Spectator* pages are the authority for
`teleport_duration` (0–59 ticks), `interpolation_duration`, `/spectate`
semantics and spectator HUD/FOV behaviour. Mojang MC-279534 and PaperMC issue
11694 document interpolation quirks worth defensive testing. Prior-art
datapacks were surveyed and **none is adoptable**: Cutscene Engine (Modrinth)
is All-Rights-Reserved; the PlanetMinecraft camera packs state no license;
`shibomb/whole-minecraft-cameraman` is MIT but is a **Paper plugin**, which
ADR-0003 forbids on the player-facing server — cited only as independent
confirmation that "spectate a proxy with a smooth teleport duration" is the
established technique.
