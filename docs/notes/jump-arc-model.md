# Jump-arc navigation model — spike findings + proposal (phase 1)

Status: **spike note + model proposal**, written 2026-08-01 as the input the M4
expressiveness spec consumes. Nothing here is implemented: the shipped nav model
still admits walking steps of at most ±1 block of rise and never a horizontal
gap. Souls-mode parkour needs provable jump gaps; this note supplies (a) the
empirical 1.21.11 jump kinematics the model must respect, measured on the pinned
server, and (b) the proposed compiler/nav/harness shape. Water and ladder edge
cases are explicitly **out of scope** (so are momentum tricks: ice, head-hitter
acceleration, 45° strafe — the model must never require what a plain bot cannot
do on cue).

## 1. Method

Scripts: `tools/spike-jump-arc/` (**spike tooling** — throwaway, never shipped,
not wired into CI). `run.sh` starts a disposable vanilla server from the exact
pinned image digest (`versions.toml [images.base]`, MC 1.21.11), serialised via
the shared `/private/tmp/delvewright-validation.lock.d` mkdir mutex and removed
on exit. `measure.mjs` drives a mineflayer bot (the harness's own pinned
dependency) over an RCON-built rig:

- Approach platform (the bot starts ~9.5 blocks behind the edge — ample sprint
  spin-up), an air gap of `g` whole cells, a
  landing platform (`rise` = 0 or +1), a catch floor 9 below for failure
  detection. Everything block-edge to block-edge: `g` is the count of air
  columns between the two platform faces.
- The bot runs east holding `forward` (± `sprint`) and presses `jump` on the
  last supported physics tick at the launch edge. Landing on the landing surface
  = success; dropping to the catch floor = failure. Each config gets 3 attempts;
  a config is *achievable* if any attempt lands (jump timing is tick-quantised,
  which is exactly the jitter the runtime bot will have).
- Ceiling trials re-run the max achievable sprint jump under a stone lid leaving
  `h` air cells of headroom over one phase at a time (launch cells / gap columns
  / landing cells / the full rig).

**Why the bot is the right instrument.** Movement in Minecraft is
client-computed; mineflayer's prismarine-physics is the same vanilla movement
model the validation harness bot uses. So every number below is by construction
a capability of *our actual critical-path bot on the pinned server* — and the
compiler's obligation is exactly "never prove a route the bot (⊆ a plain human
player) cannot walk/jump". Vanilla-wiki kinematics (jump apex ≈ 1.25 blocks,
sprint ≈ 5.6 m/s vs walk ≈ 4.3 m/s) are consistent with the measurements and
serve as the sanity cross-check.

## 2. Raw measurements (1.21.11, pinned image, 3 attempts per config)

Full run of 2026-08-01 (33 configs; `landed` = attempts that ended standing on
the landing platform; `launchX` = feet centre at the jump press, launch-edge
plane at x = 1; `landX` = feet centre at landing; apex = height gained over the
launch surface):

```
walk   gap=1 rise=0 : 3/3  (launchX 0.954, apex +1.252, landX 2.989)
walk   gap=2 rise=0 : 1/3  (marginal — tick-quantised edge timing)
walk   gap=3 rise=0 : 0/3
walk   gap=4 rise=0 : 0/3
sprint gap=1 rise=0 : 3/3  (launchX 0.984, apex +1.252, landX 4.613)
sprint gap=2 rise=0 : 3/3
sprint gap=3 rise=0 : 3/3  (landX 4.613 — needs ≥ 3.701; margin ≈ 0.9)
sprint gap=4 rise=0 : 0/3  (needs ≥ 4.701 — out of reach without tricks)
sprint gap=5 rise=0 : 0/3
sprint gap=6 rise=0 : 0/3
walk   gap=0 rise=1 : 2/3  (step-up jump)
walk   gap=1 rise=1 : 2/3
walk   gap=2 rise=1 : 0/3
sprint gap=0 rise=1 : 3/3
sprint gap=1 rise=1 : 3/3
sprint gap=2 rise=1 : 3/3  (landX 3.758)
sprint gap=3 rise=1 : 0/3
sprint gap=4 rise=1 : 0/3

ceilings — sprint flat gap=3 unless noted; headroom = air cells above the surface
of the covered phase:
  launch  h=2 : 0/3     launch  h=3 : 3/3     launch  h=4 : 3/3
  gap     h=2 : 0/3     gap     h=3 : 3/3 *   gap     h=4 : 3/3
  landing h=2 : 3/3     landing h=3 : 3/3     landing h=4 : 3/3
  full    h=2 : 0/3     full    h=3 : 3/3 *   full    h=4 : 3/3
  (+1 rise, sprint gap=2) full h=2 : 0/3, h=3 : 3/3 *, h=4 : 3/3
  * = apex clamped to +1.200 by the head-bonk, jump still lands (shorter landX).
```

Headline numbers:

| quantity | measured |
|---|---|
| jump apex (rise over launch surface) | **+1.252** blocks (vanilla 1.2522 ✓) |
| walk-jump max flat gap, reliable (3/3) | **1** air block (2 is 1-in-3 marginal — never admit) |
| sprint-jump max flat gap, reliable | **3** air blocks (the classic "4-block jump"); 4 impossible |
| sprint-jump max gap at rise +1 | **2** air blocks; walk at rise +1 is only 2/3 even at gap 0–1 |
| headroom over launch cells | **3** air cells (2 kills the jump) |
| headroom over gap columns (above launch surface) | **3** air cells (2 kills it; at exactly 3 the arc bonk-clamps to +1.20 yet still lands) |
| headroom over landing cells (above landing surface) | **2** air cells suffice (the arc is already descending) |

Sanity cross-check: sprint ground speed 5.612 m/s vs walk 4.317 m/s and the
1.2522 jump apex from the community-documented movement formulas predict exactly
this ordering and the 3-vs-1 air-gap ceiling; the measured `landX` margins
(≈ 0.9 blocks spare at sprint gap 3, ≈ 0.29 at walk gap 1) match the flight-time
arithmetic. The mid-air head-bonk result (a lid 3 above the launch surface clamps
the apex to +1.20 but the jump still clears 3 gaps) is the empirical surprise a
model built from wiki numbers alone would have gotten conservatively wrong in the
safe direction — headroom 3 is admissible everywhere except over landing, where 2
suffices.

## 3. Proposed model (for the M4 spec to consume)

### 3.1 A new nav edge type: `jump`

Today `World::neighbors_fp` offers only cardinal steps with `dy ∈ {-1, 0, +1}`
and no horizontal gap. The proposal adds a second edge family, generated only
where the campaign opts in (below):

A **jump edge** `L → D` is admissible when all of:

1. **Cells.** `L` (launch) and `D` (landing) are standable cells; the
   displacement is along ONE cardinal axis (no diagonal jumps in v1) with air
   gap `g` whole columns between the two platform faces and `rise = y_D − y_L ∈
   {0, +1}` (down-jumps are a separate, easier family — see open questions).
2. **Envelope** (only 3-of-3-reliable configs from §2 are ever admissible;
   the required-content margin on top of that is §3.4):
   - walk-jump, `rise = 0`: `g ≤ 1` (gap 2 measured 1-in-3 — never admitted);
   - sprint-jump, `rise = 0`: `g ≤ 3`;
   - sprint-jump, `rise = +1`: `g ≤ 2`; walk-jump at `rise = +1` is not
     admitted at any gap (measured 2-of-3 even at gap 1). (`g = 0, rise = +1`
     is the ordinary ±1 walking step the base model already has — no jump edge.)
3. **Runway.** For a sprint edge: the `R` cells behind `L` along the jump axis
   are standable and form a straight walkable run (sprint spin-up; §2 used 10,
   the M4 spec should measure-down or conservatively require ~4).
4. **Clearance volume** (all cells must be unoccupied in the assembled model,
   from §2's ceiling matrix):
   - launch column (and the runway, for sprint): **3** cells above `L`'s surface;
   - every gap column: **3** cells above the *launch* surface (at exactly 3 the
     arc bonk-clamps to +1.20 and still lands — 3 is the proven floor, 2 is not);
   - landing column: **2** cells above `D`'s surface (ordinary standing headroom
     — the arc is descending by then).
5. **Dry.** No gap column is water-flooded and neither endpoint is (water is out
   of scope; a flooded gap is a swim, not a jump).

A* treats a jump edge with cost `g + 2` (a jump is never preferred over an
equal-length walk; deterministic tie-break unchanged). Edge enumeration must be
deterministic: launch cells in `BTreeSet` order, axes in the fixed `HORIZ`
order (ADR-0006).

### 3.2 Opt-in scoping

Jump edges are generated only inside areas whose stage-1/2 declaration sets a
`parkour` flag (exact DSL surface is the M4 spec's call). Every existing
campaign therefore keeps a pure-walk proof and byte-identical output — the same
compatibility discipline as use-gates.

### 3.3 Diagnostics + export (proposed codes; final numbers assigned when
implemented — next free in the range at time of writing is DW0348, and other
workers allocate concurrently, so re-check at implementation time)

- **`DW0348` — required gap is unjumpable** (build, exit 3). A critical-path leg
  (or checkpoint/stealth/disarm reachability) is unroutable by walking, and
  adding jump edges *beyond the admitted envelope* would connect it — i.e. the
  content *requires* a jump the model cannot prove (too long, no runway, blocked
  clearance, water in the gap, or the area never opted in). The message names
  the gap cells, the measured ceiling it exceeds, and the fix directions (shrink
  the gap / clear the arc / add runway / opt the area in) — never "weaken the
  envelope".
- **`DW0349` — exported jump edge invalidated in the final world** (build, exit
  3). The DW0314 analogue: after relight/fixtures, a proven jump edge's
  clearance volume or endpoints are re-asserted against the FINAL model; any
  violation is a compiler/assembly defect to escalate, never an edge to nudge.
- **Export annotation** (not a diagnostic): a leg whose proven route uses jump
  edges lists them in `validation/critical-path-waypoints.json` as
  `jumps: [{launch, landing, sprint}]`, with launch and landing force-kept as
  explicit waypoints — the same shape `use_gates` uses. The
  harness replays annotated jumps as scripted maneuvers, everything else as
  ordinary pathfinding.

### 3.4 Safety margin policy (proposal)

The compiler must never over-prove. §3.1 already drops every config that was not
3-of-3 in the spike (walk gap 2, all walk-jumps at rise +1). On top of that,
proposal: **critical-path-required** jumps admit the measured maximum **minus
one** on `g` (required: sprint flat ≤ 2, sprint rise+1 ≤ 1, walk flat not
required-able), while optional (souls-bonus, off the forced path) jumps may use
the full reliable envelope (sprint 3 / 2, walk 1) — the harness only has to
prove required content on cue, retry-bounded. The M4 spec owns the final call;
whichever margin it picks, the envelope constants live in one place with this
note as source.

### 3.5 What the harness needs (feasibility)

The bot must sprint-jump **on cue**. Feasibility is demonstrated by this spike
itself: `measure.mjs` lands its jumps purely with
`setControlState("forward"/"sprint"/"jump")` on the last supported tick — the
same mineflayer primitives the harness executor already uses for sneak legs.
The executor addition is a bounded scripted maneuver per exported jump edge:

1. pathfind normally to the waypoint at the start of the runway;
2. align to the jump axis (the runway is straight by construction), face the
   landing cell;
3. run the spike's control loop (forward + sprint per the annotation, jump at
   the launch-edge plane), detect landing/failure from own physics exactly as
   `attempt()` does;
4. on failure: stall-recover back to the runway start (the existing
   stall-recovery machinery) and retry within the leg's existing retry budget.

mineflayer-pathfinder's built-in `allowParkour` is deliberately **not** used for
proof replay: its parkour solver is heuristic and version-sensitive, while the
compiler's proof names exact launch/landing cells — a deterministic macro
replays exactly what was proven (harness stays assertions + navigation, no game
logic). No new dependency is needed.

## 4. Out of scope / open questions for the M4 spec

- Water/ladder landings and launches (excluded above).
- Down-jumps (`rise < 0`): trivially easier, but fall damage starts past −3;
  souls content wants controlled drops — separate envelope, same edge shape.
- Diagonal jumps: real players use them; v1 stays cardinal for a provable,
  replayable envelope.
- Margin final call (§3.4) and the DSL opt-in surface (§3.2).
- Whether the unjumpable-gap diagnostic should also fire analysis-tier (exit 2)
  for *optional* areas
  whose only connection is unjumpable (a softer capacity warning, like DW0312).
