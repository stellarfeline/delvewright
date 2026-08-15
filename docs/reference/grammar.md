# Box-split grammar back end — live behavior record

What `crates/grammar` (package `delvewright-grammar`) does **today**. spec-0027
is the decision record; this page is the behavior record, and any PR that
changes the crate's surface updates it in the same PR.

It is a library **and** a tool: `delve-grammar` ([`tools.md`](tools.md) §2a) is
its entry point, and the procedure that drives it is
[`prefab-procedure.md`](prefab-procedure.md). Nothing here is reachable from
`delvec` and nothing ships in a delve — generation-time only (ADR-0003). The
engine depends on it nowhere; `crates/compiler` names it as a *dev*-dependency
only, to test the export seam of §7 from both sides.

Two library modules exist for the tool and are public for it:

- [`nav`] — `passable` / `solid` / `standable` / `standable_cells` / `connected`
  / `reachable_with_fall` / `ends` / `components` / `ground_entry` / `sheltered`.
  These were written inside `tests/`, where a
  rule's own gate is the right place for the gate and the wrong place for the
  *predicate* it is written in: a program authored outside this repo has no
  `tests/support` to reach for, so its author had no way to ask whether the piece
  could be walked at all. `tests/support/mod.rs` now delegates here;
  `tests/staging.rs` still carries its own copy, for the reason its own header
  gives, and folding it in is a named follow-up.
- [`gates`] — `judge(&Expansion, Options) -> Report`, and the distinction the
  module is built around: a **gate** has a verdict and a binding count, a
  **measurement** is a number with no threshold, and the two are never mixed.
  Gates: `blocks-exist` (every painted block state exists in 1.21.11 — see §4b),
  `shape-complete` (every placed state writes its shape-carrying `multipart`
  properties — `DW0735`, §4b), `states-complete` (every placed state writes
  EVERY property the block has — `DW0737`, §4b), `oriented-fills` (an
  orientation-sensitive state is filled only under the identity frame, a passed
  `orientation` guard, or the scope's own axis frame — `DW0736`, §4b),
  `non-empty`, and the opt-in `traversable`, `symmetric` and `reachable-floor`
  (§4c). Two more are emitted **only over a piece that holds what they judge**:
  `stair-shape` (every written stair `shape` is the one vanilla derives at that
  cell — `DW0801`) and `fluid-contained` (every body of fluid is saturated and
  walled — `DW0800`). Measurements: fill, distinct states, standable cells,
  footprint area/perimeter, silhouette complexity, per-block shares, local-frame
  fills, stairs, fluid cells, still (`waterlogged`) cells, run directions leaving
  the piece, and **reachability** (§4d) — how much of the floor a body reaches on
  foot and where the rest of it sits. A zero binding count, and a program declaring no
  anchors, are reported
  as findings rather than folded into a pass.

`library::PROGRAMS` is the registry the tool enumerates, so a rule added to the
library reaches `delve-grammar list` without the tool being edited. The `bell::`
zone programs are deliberately not in it: a zone is one campaign's composition,
not general vocabulary. The `idiom-*` programs **are** in it, and are neither
vocabulary nor content — they are the teaching set of §2c, and they are there
because `list` / `show` is the only way an author reaches the corpus at all.

Each entry carries **the expansion it is judged at** — region, seed, which
optional gates it claims, and whether it is a piece of the vocabulary or a
language example. A program is region-polymorphic, so "which region" is not a
property of the program; it is a property of the entry, and carrying it there is
what lets a sweep be driven from the registry instead of from a list somebody
wrote out. `delve-grammar list` prints it, so an author reaching for a piece gets
its region from the tool rather than from this page.

## 1. Model

A **grammar program** is data: named rules over integer voxel boxes. Expanding
one against a box and a `u64` seed derives a **voxel model** — a dense grid of
full block states.

Every scope in a derivation is a box plus a **frame**. A frame says two things
about each of the rule's local axes: which world axis it names, and which way
along that axis local coordinates increase. The first half is a permutation and
is what lets one rule be reused turned 90°; the second is a reflection and is
what lets one rule be reused as its own mirror image. `reorient` manipulates
both.

A reversed local axis puts the rule's local minimum at the box's world maximum:
split pieces are laid from that end, a local offset is measured from it, and a
derived facing points the other way. Reversing **two** axes is a *rotation*
(`Orientation::is_rotation`), reversing one is a reflection. A route that doubles
back wants the rotation — the half-turn about the vertical, `Reorient::turned` —
because a chiral piece turned round is still the hand it was built, and its
mirror image is not.

```text
Program ─ expand(program, region, {seed, limits, orientation}) ─▶ VoxelModel
        ─ export_prefab(program, region, options, id) ──────────▶ .nbt + .json
```

## 2. Program surface

| Element | Form | Notes |
|---|---|---|
| `version` | document version | **required**; the document's own version, not the crate's |
| `name` | string | provenance label |
| `start` | rule name | expanded into the whole region |
| `params` | name → i64 | size/kind controls; read by `{"expr":"param"}`. A declaration **and** a default: the outermost binding frame |
| `palette` | role → paint | style controls; a paint is a block-state string or a weighted list. Also a frame — `bind` overrides it over a subtree |
| `rules` | name → `[alternative]` | each alternative is `{weight, when, body}` |
| `contract` | `{entry, spaces, no_body, edges}` | the spatial contract (§2d); omitted by a program that makes no spatial claim |

**Rule bodies** (`op`): `fill` (a role or an inline paint), `void` (air), `skip`
(leave as-is), `call`, `split`, `reorient`, `bind`, `mark`, `claim`.

**`version`** is the document's compatibility surface. A version this engine does
not accept is refused outright rather than parsed for the parts that look
familiar, because a document whose newer half was skipped compiles green and
builds the wrong world. A construct a version does not have is refused where it
is written, naming the construct and both versions — which is what lets a
document at `1.0.0` keep compiling to the same bytes forever.

The ledger is every number the format has and the one surface each names
(`crates/grammar/src/version.rs`):

| version | surface | accepted |
|---|---|---|
| `1.0.0` | rules, splits, permuting reorientations, marks | yes |
| `1.1.0` | the frame's direction — `mirror` on a `reorient` request and on an `orientation` guard | yes |
| `1.2.0` | the spatial contract — the program-level `contract` block and the scope-bound `claim` node | yes |
| `1.3.0` | the scope's names as a frame — the `bind` node | yes |
| `1.4.0` | the state's own frame — a `local` paint, on a palette role or inline on a `fill` | yes |

A number names exactly one surface, in every engine build that knows the number;
otherwise two engines both call themselves `1.1.0`, disagree about what a
`1.1.0` document means, and each silently drops the other's half.
`tools/check-version-ledger-uniqueness.py` holds that against `origin/main`, for
this ledger and for `dsl_version`.

A number whose surface is introduced by a change still in flight is **reserved**
in the ledger rather than skipped, because a skipped number is a free number and
a free number is one two changes can take. A document declaring a reserved
version is refused, and the refusal names the surface that owns the number —
building it would mean deserialising that surface into nothing. The reservation
is deleted by the change that lands the surface, in the same edit.

**`split`** cuts one local axis into pieces: `absolute` pieces take a fixed block
count, `relative` pieces share what is left. `rounding` (`truncate` — the
default and upstream's only behaviour — `start`, `end`, `middle`) says where the
indivisible remainder goes; `repeat` tiles the pattern across the axis and clamps
the last piece; `orient` hands every child a new orientation. Children are
matched to pieces in order, and cycled when `repeat` produced more pieces than
children.

**`reorient` / `orient`** name a child axis as `local_*`, `world_*`, `smallest`,
`largest`, or `split_axis` (the axis being cut; splits only). Unnamed axes are
completed to a permutation: keep an axis where possible, otherwise complete the
cycle the request started (asking for "my Z is the old X" swaps X and Z),
otherwise take the lowest free axis.

The same request carries **`mirror`** — the child's local axes that run
*backwards* along the axis they name:

```json
{ "op": "reorient", "orient": { "mirror": { "x": true } },
  "body": { "op": "call", "symbol": "transept_arm" } }
```

A reflected local axis reverses everything measured along it. A `split` lays its
pattern from the axis's low end, which is now the world *high* end, so the same
rule puts its first piece at the opposite face; `corner_min` is the local minimum
corner, so it moves to the other end; `face_center`'s `min`/`max` and `offset`'s
counts follow suit; and a derived `mark` facing turns around. Extents do not
move — a reflected scope is exactly as wide as its mirror image, so every size
expression written for one half of a symmetric shape holds in the other.

`mirror` is **relative to the source axis**, not to the world: it reverses
whichever direction the parent already ran in. Reflecting a reflected frame gives
the original back, so a rule may be reflected at any depth without knowing
whether it already is — which is what lets one rule stand at both sites of a
mirror pair. It sits on the frame request rather than on `split`, because the
frame request is what both `reorient` and a split's `orient` already are, and a
reflection keyed to either alone would leave the other with no surface.

`mirror` is independent of the axis choice — a piece can be turned 90° without
being turned round, and the other way about — and never enters it: `smallest`
and `largest` measure world extents, which a sign cannot move. `Reorient::turned`
reflects `X` and `Z` together, the half-turn about the vertical, which is the
one two-axis combination that is a rotation rather than a reflection and is what
a route doubling back wants.

**`bind`** wraps a body and rebinds names over it — `params`, `palette`, or
both:

```json
{ "op": "bind",
  "params":  { "run":     { "expr": "int", "value": 3 } },
  "palette": { "opening": { "role": "glazing" } },
  "body":    { "op": "call", "symbol": "head" } }
```

A scope is a box, a set of axis names and a set of value names. `split` narrows
the box, `reorient` renames the axes, `bind` rebinds the values, and **all three
are inherited by every child scope, a `call`'s included** — which is what lets an
argument survive a recursion whose rules never mention it. `head` fills
`opening`, `shoulders` calls `head`, and a caller who binds `opening` gets a
glazed arch out of both without editing either.

Four things it is:

- **A frame, not a global.** It lasts exactly as long as its body; the sibling
  beside it reads the enclosing frame, and so does everything after it.
- **Simultaneous.** Every binding in one frame is evaluated in the *enclosing*
  scope before the frame is pushed, so `{"a": param b, "b": param a}` swaps the
  two rather than chaining them.
- **Shadowing.** An inner frame wins over an outer one, name by name; a name it
  does not mention falls through.
- **Closed.** A `bind` may only name a parameter or role the program itself
  declares (`UnknownBinding`), so a misspelt binding is refused where it was
  written instead of quietly expanding the default. A `bind` that binds nothing
  is `EmptyBind`.

A binding writes no cell and draws nothing from the seeded stream, so it can
move no block by itself: rebinding every name of every rule of every library
program to itself gives byte-identical models and identical anchors, at three
seeds (`tests/arguments.rs`).

The three scope constructs compose, and the composition is asserted rather than
assumed (`tests/frames_compose.rs`): a `claim` under a `bind` resolves to the box
the pushed value made, a reflected rule claims the mirror-image box, and a
binding reads the same extents reflected as not.

**A binding is one more way to write a recursion that does not terminate** — an
argument that keeps a guard true for ever — and it needs no new answer: an
unguarded recursion is a `DepthLimit`, deterministic and named (§4). Used the
other way it is a **base case**: a self-call that rebinds `n` to `n + 1` under a
guard on `n` is a recursion that counts, which is an index into the recursion —
for a peel-one-and-recurse rule, the index along the axis. It is still not an
index into position; a `repeat` split's tiles cannot know how far along they
are.

**Guards** (`when`): `always` (default), `otherwise`, `cmp` over integer
expressions of literals / params / scope dimensions with `+ - * / % max min`,
`all` / `any` / `none_of`, and `orientation` (matches an exact frame — how a
directional stair or door picks its facing). The frame it matches is the axis
mapping **and** the reflection, both exactly; a guard that omits `mirror` asks
for an unreflected scope. That strictness is the point: nothing reflects a
`facing=` property, so a stair chosen for one frame is wrong in that frame's
mirror image, and a guard that matched both would place it silently.

**Selection**: every non-`otherwise` alternative whose guard holds is a
candidate; if none hold, the `otherwise` alternatives are; among candidates the
seeded PRNG draws by `weight`. **Two guards that can hold at once are a
probabilistic choice, not a priority order** — guards meant as a decision must be
mutually exclusive. **`otherwise` is the only precedence the language has**: it
is the arm that runs when no other alternative matched, so a decision is written
as mutually exclusive positive guards plus one `otherwise` for the rest. It is
also what terminates a recursion — a self-call whose guard finally fails has
nowhere else to go, and the expansion ends in `NoApplicableRule`.

The IR serialises to JSON (`serde`), which is the authoring form; block states
are their vanilla string, e.g. `"minecraft:oak_stairs[facing=east,half=top]"`.

**A paint is a block-state string or a weighted list**, and the list is a
per-cell draw from the seeded stream:

```json
"palette": {
  "wall": "minecraft:stone_bricks",
  "ruin": [
    { "weight": 9, "block": "minecraft:stone_bricks" },
    { "weight": 3, "block": "minecraft:mossy_stone_bricks" },
    { "weight": 2, "block": "minecraft:cracked_stone_bricks" },
    { "weight": 2, "block": "minecraft:air" }
  ]
}
```

The same two forms are legal inline on a `fill`
(`{"op":"fill","material":{…}}`). `minecraft:air` is a member like any other,
which is what makes a mix a *material that is partly not there*; weights are
positive integers and a zero is refused. **A mix moves no geometry** — the same
cells are visited whatever the weights say — so a restyle can never change what
a gate walked, and a sweep over seeds is a sweep over texture alone.

**A paint names the axes its properties are written in.** `north`, `east`,
`axis=x` and a 16-step `rotation` all name a direction, and a direction is only
meaningful against a frame. Written bare, a state is in the **world** frame:
`north` is the world's `−Z` however the scope was framed. Wrapped in `local`,
it is in the **scope's own** frame and is resolved into the world's at fill
time:

```json
"palette": {
  "grille": { "local": "minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=true]" },
  "rubble": { "local": [ { "weight": 3, "block": "minecraft:oak_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]" } ] }
}
```

Both forms take either shape — one state or a weighted list — and both are legal
inline on a `fill`, because the frame belongs to the state and every consumer of
a state gets it: a palette role, an inline material, and a role a `bind` pushes
down a call. `"grille"` above says *the bars run along my local X*: expanded into
a box whose local X is the world X it writes `east`/`west`, into a box turned
90° it writes `north`/`south`, and into a box whose local X runs *backwards* it
writes the mirror of the first — all from the one binding. That is what lets an
orientation-dependent block be a palette **role**, so a campaign restyles it
without knowing which way the piece was laid; a `--role` override is a restyle
and keeps the frame of the binding it replaces.

**Both halves of the scope's frame are read.** A frame is a signed permutation:
which world axis each local axis names, and whether it runs backwards along it.
A reflection is the sign, so `east` under a reflected local X is the world's
`west` — and a resolver that read the permutation alone would answer "the
identity moves nothing" for every mirrored body and write the state's mirror
image in silence.

The resolution is exact or it refuses (`DW0738`, §4b). A direction (by key or by
value), an `axis` and a `<dir>_<dir>` pair have exact images under every frame.
A 16-step `rotation` and a handedness (`hinge`, a stair's corner `shape`) do not:
they are stated against a fixed vertical *and* a fixed handedness, so they are
determined only under a **pure turn about the vertical** — the identity, or the
horizontal transposition `x↔z`, which is itself a reflection of the horizontal
plane and sends a yaw `r` to `(12 − r) mod 16` and left to right. Reflect any
axis, or move the vertical, and they are refused rather than guessed. A
`top`/`bottom` half needs the vertical kept and running forward. A value that
names no handedness (`straight`, `single`) and a `double` slab are their own
image under every frame and are never refused.

`local` is fenced at document version **1.4.0** (§2e).

### Six things the surface above does not say

1. **`rounding` other than `truncate` is legal on a split with exactly one
   relative piece**, and at weight 1 it is inert: the remainder of dividing by
   one is always zero, so `[abs, rel(1), abs]` covers the axis exactly under
   `truncate` already. `RoundingWithoutRelative` refuses only a split with *no*
   relative piece. Rounding starts to matter at weight ≥ 2 or with several
   shares.
2. **`smallest` / `largest` break a tie toward the lowest world axis** — `X`,
   then `Y`, then `Z` — measured over the axes still unclaimed when the
   extremal spec is resolved. On a cube, `x: largest` names world `X`. Read as
   an *expression* (`{"expr":"dim","dim":"smallest"}`) the same two words are a
   number rather than an axis: the smallest of the three world extents, with no
   tie to break.
3. **A relative piece that resolves to zero blocks is a silent empty child**,
   not an error. A zero *weight* is refused (`BadSize`); a positive weight with
   nothing left to share is a legal zero-volume scope, and `fill` / `void` /
   `skip` all write nothing in it without complaint. What does refuse is
   anything needing a cell of it: an absolute split inside it overflows, and a
   `mark` on it is `MarkOutsideScope`.
4. **A role bound to a WORLD-frame block state does not turn when `largest`
   turns the scope, and does not reflect under `mirror` either.** A `fill`
   writes a world-frame state verbatim; nothing rotates or reflects a `facing=`
   property to follow the frame. So a rule whose frame opens with `z(largest)` —
   every §5b rule does — lays its stairs, doors and voussoirs the same way round
   whatever box it is handed, and `oriented-fills` (§4b) is the gate that reads
   it, both halves of the frame included: the piece is refused, not shipped
   facing the wrong way. Two constructs answer it. Write the state in the
   scope's own frame (`{"local": …}`) and one binding turns *and* reflects with
   the piece — which is how `far_side_bar`'s bar stays a role. Or pin the frame
   with an `orientation` guard and write one alternative per frame, which is
   what a rule needs when the whole BODY differs by frame rather than just the
   state. Because the guard matches the reflection too, the two sides of a
   mirror pair are separable: an alternative guarded on the unreflected frame
   does not fire in the reflected scope, which falls to `otherwise`.

5. **An `absolute` size takes an expression, so anything derivable from the
   scope's own extents needs no argument at all.** `max(1, X / run)` steps a
   taper in two cells a side while its courses are wide and one when they are
   narrow, read off the box each course is handed. This is the cheapest form of
   parameterisation in the language and the first to reach for: it fails only
   where two **same-sized** siblings need different content, which is what
   `bind` is for.
6. **`reorient` is an argument too.** It hands the callee a turned frame, so one
   rule family serves a west rose and both transept roses, or a head that tapers
   across `X` and the same head across `Z`. It cannot pass a paint, a size or a
   role — that is the line between it and `bind`.

Items 1–5 are asserted in `tests/idioms.rs`; item 6 in both `tests/idioms.rs`
and `tests/arguments.rs`.

## 2b. `mark` — anchor declarations

An anchor is **metadata**, not geometry. No composition of `fill` / `split` can
express "this cell is where the boss stands", and reading one back out of the
block pattern afterwards is a guess — which the layering rule forbids. So the
rule that shapes a space declares it while it still has the box in hand.

`mark` wraps a body (like `reorient`) rather than being a statement, because a
rule body is one node: that way a mark can sit on any child of any split and
annotate exactly the piece that child owns. Use `{"op": "skip"}` as the body when
the declaration is all that is wanted.

```json
{ "op": "mark", "mark": { "anchor": "courtyard", "at": "floor_center" },
  "body": { "op": "void" } }
```

| Field | Meaning |
|---|---|
| `anchor` | kebab-case stem. The exported key is `anchor/<stem>`, i.e. the DSL's `anchor/<kebab>` id — a mark cannot name an anchor the DSL could not reference. |
| `at` | which cell (flattened into the mark object, see below) |
| `facing` | `north`/`south`/`east`/`west`. Omitted, it is **derived** as the direction of *decreasing local `Z`* — the way §5b's frame says travel runs. Which world direction that is depends on both halves of the frame: `north`/`south` when local `Z` names world `Z`, `west`/`east` when it names world `X`, the second of each pair when local `Z` is reflected. All four cardinals are reachable. A scope whose local `Z` is *vertical* has no cardinal facing and says so rather than guessing. |
| `index` | `unique` (default) → `anchor/<stem>`; `auto` → `anchor/<stem>-<n>`, `n` counting from 1 per stem in expansion order — how a rule that runs once per tower gives every tower an anchor without knowing how many there are. Matches the hand-built `anchor/alcove-1…` convention. |

`at` is one of:

| `at` | Cell |
|---|---|
| `corner_min` | the scope's **local** minimum corner — the world minimum corner on unreflected axes, the far end on reflected ones |
| `floor_center` | lowest **world** `Y`, centred on world `X`/`Z`. Gravity is a world fact, so this one position ignores the frame entirely — both halves of it |
| `face_center` (+ `axis`, `side`) | the given **local** axis pinned to its local `min`/`max`, the other two centred |
| `offset` (+ `x`, `y`, `z` expressions) | **local** cells from the local minimum corner |

Every `at` but `floor_center` is computed in local coordinates and put through
the frame once, so a mark under a reflection lands on the mirror image of the
cell it names.

Centres round down on an even extent (the lower-middle cell) — it has to be one
of the two, and the same one every time (ADR-0006).

Marks collect into `Expansion::anchors` (a `BTreeMap`, keyed by exported name),
**not** into the `VoxelModel`: a mark writes no blocks, and folding metadata into
the block grid would change what `canonical_bytes` means. The export writes them
into the prefab metadata's `anchors` map in the hand-built `{pos, facing}` shape,
`pos` local to the structure; `PrefabRegistry` reads a grammar prefab's anchors
with the same code path as a hand-built one (`crates/compiler/tests/grammar_prefab.rs`).

Refusals: a non-kebab stem is a `Program::validate` error (before any expansion);
a mark aimed outside its own scope, an underivable facing, and two marks
producing the same name are expansion errors — the collision names both rules.
Two marks on the same **cell** under different names are legal, as in the
hand-built prefabs.

## 2c. The idiom index — how the constructs make shapes

§2 is the list of constructs. What an author is missing is not that list: it is
the handful of ways those constructs compose into a shape, none of which is
visible from a type signature. `prefab-procedure.md` §3 says to start from the
corpus rather than from the schema, which means **the corpus is the
expressiveness** — a technique no program in the library demonstrates does not
exist in practice, whatever the IR supports.

Ten techniques, one minimal program each, all reachable from the tool:

```sh
delve-grammar list                                     # the `idiom-*` block
delve-grammar show   --program idiom-shape > p.json    # the whole program
delve-grammar expand --program idiom-shape --region 15x9x3 --seed 1 -o out/
```

| # | Technique | Program | Region, seed | What it shows |
|---|---|---|---|---|
| 1 | Repetition | `idiom-repetition` | 3 × 5 × 17, 1 | `repeat` tiles a pattern; a self-call carries the remainder, which is the only index there is |
| 2 | Priority | `idiom-priority` | 13 × 6 × 2, 1 | `otherwise` is the only precedence; overlapping guards are a draw |
| 3 | Shape | `idiom-shape` | 15 × 9 × 3, 1 | a taper is a recursion whose step is arithmetic on the remaining dimension — and with the paint inverted it is the opening |
| 4 | Erosion | `idiom-erosion` | 9 × 5 × 3, 1 | `minecraft:air` weighted into a role |
| 5 | Graded erosion | `idiom-erosion-graded` | 9 × 13 × 3, 1 | a gradient is a banded split, one mix per band |
| 6 | Surface detail | `idiom-surface-detail` | 9 × 12 × 9, 1 | the rule that built the surface splits off the layer against it |
| 7 | Symmetry | `idiom-mirror` | 15 × 11 × 2, 1 | `reorient`'s `mirror` gives a rule its own reflection, so a mirror plane is one rule and not two copies |
| 8 | Skip | `idiom-skip` | 7 × 5 × 5, 1 | what `skip` does, and why show-through is not expressible yet |
| 9 | Light | `idiom-light` | 5 × 6 × 13, 1 | a lamp is a role; a one-cell split is a sconce |
| 10 | Arguments | `idiom-arguments` | 15 × 7 × 15, 1 | one rule called with different content — `bind` for the paint, `reorient` for the axis |
| — | A composition demonstration | `idiom-composition-arcade` | 3 × 14 × 20, 1 | eight of the ten at once — a ruined arcade |

Each program exists to teach one technique and nothing else, and each is
expanded at exactly the region and seed above by
`crates/grammar/tests/idioms.rs`, which asserts the claim in its own row. An
entry that stopped being true is a red, not a stale page. They declare no
anchors — the composition declares one — so `expand` prints the no-anchors
finding over them, which is correct: a teaching program is not a prefab a
campaign binds to.

The JSON fragments below are **abridged for reading** — a literal where the
program computes an expression, a `"<…>"` placeholder where an expression is
long. `delve-grammar show --program <id>` prints the program that runs, and that
is the one to copy.

**The index is not the corpus.** The index is a curated set of *techniques* and
grows only when an authoring trial fails for want of one (spec-0033 §4.6, §4.8).
The corpus is every program `delve-grammar list` names, and every IR construct
owes it at least one example — which is why `negated-guard` is in the library
and not in the table above: `none_of` is negation of guards, a language feature
rather than a way of building anything. The demonstration-coverage report is
what holds the corpus to that.

### The order of the splits — decided before any of the ten

Not an eleventh technique and not a row in the table: the ten below are how a
shape is made, and this is the decision taken before any of them, in the first
ten minutes, on almost no information. It decides which of the building's openings
can line up at all.

A split cuts one axis. Its children copy the parent box **unchanged on the two
axes it does not cut**; only the cut axis differs. So two siblings of a split are
guaranteed to have the same extent on the other two axes, and that guarantee is
the only alignment this language gives away: there is no positional index and no
way to say *this opening is the same cells as that one*. Every other alignment is
a constant an author computed, which is a constant an author can get wrong, and
nothing checks it.

**The last axis you split is the only axis on which two things are guaranteed to
meet — so split last on the axis your openings run through.** A hole is then a
*piece of a split whose siblings are the two things that must meet*: a breach and
the deck outside it are two children of one split across the wall's thickness, so
they share their other two extents by construction and there is no sill height to
match. The opening itself is best written as the **absence** of a sibling
(`void`, or a piece with no child painting it), because an absence cannot be
misaligned.

Within one axis, pin a course to a band's **end** rather than to a height:
`[relative 1, absolute 1]` down `Y` makes a slab *the last course of that band*
at any band height — measured at bands 4, 7, 11 and 13 deep, one course each
time, with no rule stating a rise. `[absolute 5, absolute 1]` is a course at a
computed height, and it also refuses any band shorter than six. Write every slab
the first way and the holes between storeys stop needing arithmetic.

The order cannot be changed afterwards without rewriting the decomposition, so
choose it from where the openings are, not from how the building is drawn.

### 1. Repetition

The `-X` lane tiles its piers with `"repeat": true`; the `+X` lane peels one
pier and one bay off the low end and calls itself on the remainder. At the
documented region the two lanes are the same rhythm, cell for cell.

```json
"recursed_row": [
  { "when": { "cond": "cmp", "lhs": {"expr":"dim","dim":"z"}, "op": "ge",
              "rhs": {"expr":"int","value":5} },
    "body": { "op": "split", "axis": "z",
              "sizes": [ {"size":"absolute","blocks":{"expr":"param","name":"pier"}},
                         {"size":"absolute","blocks":{"expr":"param","name":"bay"}},
                         {"size":"relative","weight":{"expr":"int","value":1}} ],
              "children": [ {"op":"fill","material":{"role":"mass"}},
                            {"op":"void"},
                            {"op":"call","symbol":"recursed_row"} ] } },
  { "when": {"cond":"otherwise"}, "body": {"op":"fill","material":{"role":"mass"}} }
]
```

The line between the two forms is the whole entry. A `repeat` split hands every
tile the same pattern, so **no tile can know how far along it is**; a self-call
is handed the box that is left, and that box is the only index the IR exposes.
That is why `stair_flight`'s treads, `store_room`'s tell and every taper in §2c
are recursions and none of them is a `repeat`. Turn the remainder into
arithmetic and the same recursion becomes a shape (idiom 3).

The `otherwise` arm is the base case: the remainder too short for another
pier-and-bay becomes the last pier. Strip it and the expansion is
`NoApplicableRule` at the first scope the guard rejects.

**`repeat` clamps the last tile; it does not rescue a box too short for the first
one.** One pass of the pattern is resolved before any tiling happens, and its
absolute pieces must fit the scope — so a `[absolute 3, absolute 5]` repeat
handed a 7-deep box is `rule "row": split needs 8 blocks but the scope is 7
across`, a refusal, not a single clamped tile. A rule that repeats therefore owes
a guard on the extent and an `otherwise` arm for the short box, exactly as a
recursion does; `boulder_stair` gives its short lane no pockets rather than an
error.

### 2. Priority

Three bays of three widths, one rule deciding what each becomes — arch, slot,
solid pier — with the third arm an `otherwise`.

Selection collects **every** non-`otherwise` alternative whose guard holds and
then draws among them by weight, so writing `X >= 6` and `X >= 3` as the first
two arms is not "prefer the arch": at a 7-wide bay both hold and the seed picks.
The second guard is therefore the complement, spelled out:

```json
{ "cond": "all", "of": [
    { "cond":"cmp", "lhs":{"expr":"dim","dim":"x"}, "op":"ge",
      "rhs":{"expr":"param","name":"slot_min"} },
    { "cond":"cmp", "lhs":{"expr":"dim","dim":"x"}, "op":"lt",
      "rhs":{"expr":"param","name":"arch_min"} } ] }
```

The red is measured rather than argued: with the `lt` half dropped, twelve seeds
build more than one arcade out of the same box.

### 3. Shape

One three-rule recursion — peel a course, inset the remaining box by `step` on
each side, recurse — and it is simultaneously the arch, the gable, the ramp, the
vault, the spire and the batter. Which one you get is a matter of which axis is
split and how big the box is. `church`'s `roofYsplit` / `roofZsplit` /
`rooffill` already contain half of it.

```json
"profile": [
  { "when": { "cond":"all", "of":[ "<X >= 2*step + 1>", "<Y >= 2>" ] },
    "body": { "op":"split", "axis":"y", "rounding":"start",
              "sizes":[ {"size":"absolute","blocks":{"expr":"int","value":1}},
                        {"size":"relative","weight":{"expr":"int","value":1}} ],
              "children":[ {"op":"fill","material":{"role":"mass"}},
                           {"op":"call","symbol":"step_in"} ] } },
  { "when": {"cond":"otherwise"}, "body": {"op":"fill","material":{"role":"mass"}} }
],
"step_in": [ { "body": { "op":"split", "axis":"x", "rounding":"start",
                "sizes":[ "<step>", {"size":"relative","weight":{"expr":"int","value":1}}, "<step>" ],
                "children":[ {"op":"fill","material":{"role":"cut"}},
                             {"op":"call","symbol":"profile"},
                             {"op":"fill","material":{"role":"cut"}} ] } } ]
```

**The step is not fixed at one cell.** An `absolute` size takes an *expression*,
so the inset can be read off the scope it is applied in. Here it is
`max(1, X / run)`, which steps in two cells a side while the courses are wide
and one when they are narrow — a convex batter, not a 45° wedge. Course widths
at the documented region are 15, 11, 9, 7, 5, 3, then a one-wide ridge. What is
*not* expressible is a step that depends on **where** the scope sits: there is
no positional index.

**With the paint inverted it is every opening in the building.** The two roles
are the taper (`mass`) and its complement (`cut`), and the default binding makes
the taper stone standing in air — a gable. Bind them the other way round:

```sh
delve-grammar expand --program idiom-shape --region 15x9x3 --seed 1 \
    --role mass=minecraft:air --role cut=minecraft:stone_bricks \
    --id idiom-shape-arch -o out/
```

and the identical derivation is a solid wall with a stepped pointed opening in
it. **A pitched roof and a pointed arch are the same program with the paint
inverted**; the two expansions are exact complements over all 405 cells, which
the test measures rather than asserts. A straight jamb under the springing is
one more `Y` split below the taper — see the composition.

**Two of these crossing is one more rule, not a Rust generator.** Run a second
prism across the first and the union has a **plus**-shaped cross-section at
every course; a plus is a partition, so the recursion peels the **ring** of its
box rather than insetting the box. Lay one solid course, then cut what is left
into four one-cell slabs and a core — `[1, rel, 1]` down `Z`, and that split's
middle piece `[1, rel, 1]` down `X` — and hand each slab the taper and the core
the crossing rule again.

Which taper each slab gets is the whole of it, and it is why no rule counts
courses. The slabs taken by the first split still span the box's full width, so
they step in one cell and then lay a course. The slabs taken from the middle
piece have already lost a cell at each end to the first split, so **their own
extent is already the width their course needs** and they lay it straight away.
Both are the two rules above, entered at different points — `step_in` for the
first pair, `profile` for the second — with the margins skipped rather than
painted.

Four rules build it: those two, the crossing rule, and one that places the two
bands in the region. The result is a cross-gable with a true valley at each of
the four re-entrant corners, both ridges at one height, over the whole
footprint, at any size. The two bands are given the same width — that is what
one pitch and one ridge height mean together.

### 4. Erosion

A palette role that carries some air is a material that is partly not there, and
that is the whole of decay, rubble, spall and pitting here. One role, one rule,
no geometry. The authoring form is in §2. A role bound to a single block is a
surface of one material, which is the whole explanation for a zone that renders
as monoculture — so this is the cheapest change in the language, and the first
thing to reach for when a piece looks flat.

### 5. Graded erosion

Uniform noise reads as texture; decay has a direction. The language has no
gradient — a mix's weights cannot vary with position — so **the gradient is the
split**: band the surface and give each band its own mix, air share climbing.
More bands is a smoother gradient and nothing else.

The bands are a rounded split, and at the documented region that is
load-bearing: thirteen courses over three shares do not divide, so under the
default `truncate` the pieces are 4, 4, 4 and the thirteenth course is **never
written** — twenty-seven cells of daylight along the top of the wall, with
`blocks-exist` and `non-empty` both perfectly green. `rounding` is owed by every
surface, not only by floors.

### 6. Surface detail

Detail is not a pass over a finished model; there is no such pass. It is one
more piece in the split that made the surface, taken while the rule still has
the box in hand: `[rel 3, abs 1, abs 1, rel 2]` down `Y` is mass, the crust
course that is the top of the mass, the litter course standing on the crust, and
the air above. Scatter members are deliberately not full cubes
(`moss_carpet`, `short_grass`, `brown_mushroom`) —
`tools/block-appearance.py --full-cube-only` is for the structural roles, and a
litter layer is exactly where the rest belong. The same move on a different axis
is a wall's inner face; with a light-emitting member it is idiom 9.

### 7. Symmetry

A frame carries a direction as well as a mapping, so `reorient`'s `mirror` hands
a body its own reflection. **A shape with a mirror plane is therefore one rule
and a reflection of it**, never two copies that nothing keeps in step.

`half` peels one course off the low end of its local `Y` (`[rel 1, abs 1]`,
children `[inset, slot]`) and chamfers by one cell per side, recursing on the
remainder. The window is a full-width waist with `half` below it and the same
`half` above it under `mirror: {y}`:

```json
{ "op": "split", "axis": "y", "sizes": ["<margin>", 1, "<margin>"],
  "children": [
    { "op": "call", "symbol": "half" },
    { "op": "call", "symbol": "slot" },
    { "op": "reorient", "orient": { "mirror": { "y": true } },
      "body": { "op": "call", "symbol": "half" } }
  ] }
```

Together they give a chamfered octagon — a rose window — at glazing widths 3, 5,
7, 9, 9, 9, 7, 5, 3, symmetric about both centre lines of the wall. It re-centres
itself as the wall widens, because the aperture and every course inside it sit in
the middle share of a `[margin, aperture, margin]` split. `tests/idioms.rs`
compares the reflection against the two hand-written copies it replaces, byte for
byte, and the entry's own row asserts the `symmetric` gate over the result.

**This is enough for any shape with a mirror plane.** Two things it does not
reach. A smooth curve: the steps are integers and integer arithmetic has no
square root, so a circle is a polygon here whatever you do. And a block state: a
`fill` writes what it was given verbatim and nothing reflects a `facing=`
property, so a directional role needs the `orientation` guard, which matches the
frame entire and so tells the two sides of a mirror pair apart.

### 8. Skip

`skip` writes nothing; `void` writes air into every cell. **They are
indistinguishable in the finished model**, and that is a property of the IR
rather than of this example: nothing writes a cell twice — a split's children
partition their box, a rule body is a single node, there is no sequencing
operator — so every cell is written by exactly one node or by none, and a model
starts as air. There is no earlier fill for `skip` to leave standing. The test
swaps the two and the bytes do not move.

What `skip` carries today is **intent** — *this box is not mine to write* —
which is what a `mark` whose body writes nothing wants to say, and it costs
nothing where `void` costs one write per cell. Show-through waits on an overlay
primitive, the same missing construct that stops a zone carving a doorway into a
piece's own wall (§5c).

### 9. Light

There is no light construct: a role bound to `minecraft:sea_lantern` is a role,
and a split that gives it one cell every `sconce_period` along a wall course is
a run of sconces. That is the whole technique, and it is the only reason a
program's lighting is the program's own business. The period is the split's own
pattern, so it is a real control — widen it and the same gallery has fewer
sconces.

It matters because a piece that places no light **is** dark, the grammar cannot
warn about it, and the emitted metadata says `"profile": "unmeasured"` and means
it: expansion places blocks, not photons. `delve-admit lighting --write`
(procedure §7) is where the number comes from.

### 10. Arguments

One pointed-arch recursion, four heads: two open onto air and two onto glazing,
two taper across world `X` and two across world `Z`. Three rules.

```json
{ "op": "bind", "palette": { "opening": { "role": "glazing" } },
  "body": { "op": "call", "symbol": "head" } }
```

**Written without arguments the same four heads are eight rules.** The paint is
filled by `shoulders`, the second rule of the recursion — so a glazed head needs
a copy of `shoulders`, which needs a copy of `head` to call it, twice over for
the two axes. Nothing keeps four copies in step and no gate reads the
difference: `tests/arguments.rs` builds the eight-rule program, edits one copy's
taper, and all five always-on gates — `blocks-exist`, `shape-complete`,
`states-complete`, `oriented-fills`, `non-empty` — plus the coverage report and
the determinism
gate are green over a building that now carries two different arches. The
same file proves the collapse is exact — the four-copy program and
`idiom-arguments` are byte-identical at four seeds, anchors included.

The two mechanisms in the entry are not interchangeable and the piece uses both.
`reorient` supplies the **axis** and is the older of the two; `bind` supplies the
**paint**, and a size or a role the same way. Anything the callee can derive from
its own box needs neither (§2, item 5).

The other thing to take from it is where the binding is read: three rules below
the call that pushed it, by a rule that mentions neither glazing nor the caller.
A frame that stopped at the call would leave every rule of a recursion re-passing
every name any caller might bind, and one forgotten thread would silently expand
the default.

### A composition demonstration

`idiom-composition-arcade` is a ruined arcade, and it is here to be **read**
rather than reused: a campaign that wants an arcade writes its own program from
the techniques, against its own fiction. Adding `gothic_arcade` to the
vocabulary would be the catalogue mistake — the next creator wants a headframe,
a gantry, a ziggurat, finds no entry and concludes the back end cannot.

Eight of the ten are in it: the colonnade is a recursion (1) whose `otherwise`
arm places the last pier (2); each bay's head is the taper with the paint
inverted (3), so what narrows is the hole; every masonry role carries some air
(4) and the footing, wall and crest are three mixes up the elevation (5); the
crest's own top course is a litter layer (6); and every pier carries a sconce
cell on both faces (9). Idiom 7 is not in it — nothing here has a mirror plane
the recursion does not already centre for itself — neither is idiom 8, since
the bays are meant to be empty, which is what `void` says — and neither is idiom
10: every bay is the same bay, so its recursion is stated once and called with
nothing.

## 2d. The spatial contract — `claim`, and what a program says about a body

A program can state where a body goes. The statement has two halves, and they
are separate because they answer different questions.

**The rules say where.** `claim` wraps a body the way `mark` does, writes no
blocks, draws nothing from the seeded stream, and gives the scope's box a name:

```json
{ "op": "claim", "region": "nave", "body": { "op": "void" } }
```

**The `contract` block says what.** A name is a space with an envelope, an
out-of-walk region with a kind, or an edge's own volume — stated once, however
many rules claim boxes for it:

```json
"contract": {
  "entry": "near",
  "spaces": { "near": { "envelope": "enclosed" },
              "far":  { "envelope": "enclosed" } },
  "no_body": { "shelf": { "reason": "where a watcher stands" } },
  "edges": [
    { "a": "exterior", "b": "near", "class": "walk" },
    { "a": "near", "b": "far", "class": "barred",
      "bar": { "region": "gate", "block": "bar" } },
    { "a": "far", "b": "exterior", "class": "walk" }
  ]
}
```

**An out-of-walk region carries no kind.** Which exemption a region qualifies
for — walled off, anchored, exterior dressing — is a fact about the blocks, so it
is read off them rather than chosen here. An author who could pick would be
picking which demand has to be met, and a choice between demands is only ever as
strong as the weakest one on offer. What the author supplies is the `reason`,
because no measurement recovers that.

Splitting the two halves is what lets **one** declaration node serve a space, a
stair's transit volume and a bar region. Building a second kind of node per use is how
the third use ends up with no surface at all — and it is also what makes the
`envelope` one statement rather than one per claiming rule.

| Field | Meaning |
|---|---|
| `entry` | the declared space a body enters at |
| `spaces` | name → `{envelope}`; `enclosed`, `open_top` or `open` |
| `no_body` | name → `{reason}`; standable cells deliberately outside the walk, with the author's reason in their own words |
| `edges` | `{a, b, class, …}` in declaration order; an endpoint is a declared space or the reserved name `exterior` |
| `no_body_majority_ack` | the author's acknowledgement that the piece is mostly out-of-walk |

**Edge classes carry exactly the fields they mean**, so a bar on a walk or a rise
on a sightline is not writable in the first place:

| `class` | Fields |
|---|---|
| `walk` | `rise` (default 0), optional `via` |
| `stair` | `rise`, **required** `via` — the treads belong to the edge, not to either end |
| `drop` | `rise`, optional `via`; directed `a` → `b` |
| `barred` | `rise` (default 0), **required** `bar` (`{region, block}`), optional `via` |
| `vision` | **required** `via`; no traversal claim, so no rise |

`via` and `bar.region` name regions some rule claims, exactly as `spaces` and
`no_body` do. `bar.block` is a palette role, and a role bound to a weighted mix
is refused: a bar is one material, and a gate that is mostly a bar is not a state
anything can be in.

**Several claims of one name union.** A room whose cross-section is not a box is
described by the boxes it is actually built from, rather than by a shape
recomputed at the top of the program. The boxes are recorded sorted and
de-duplicated, so the record is the set of cells rather than a trace of the
derivation. A claim on a scope with no cells contributes nothing — the same thing
`fill` and `void` do there — and a region no expansion claimed resolves to no
boxes rather than disappearing, so the zero stays visible.

**A region name is the program's vocabulary**, not the campaign's. It is one or
more kebab-case segments joined by `/`, and `compose::include` prefixes it as it
prefixes a rule, a parameter and a palette role — the opposite of an anchor stem,
which is the campaign's id for a place and is never qualified. Left unqualified,
a piece included twice would union its two rooms into one region and describe a
room that is not there. The destination classifies the regions it takes on in its
own contract, and until it does, `validate` refuses and names the region.

**What is checked, and what is not.** Every name resolves, in both directions: a
claim the contract does not classify is refused, and a contract region no rule
claims is refused. `entry` and every edge endpoint name a declared space or
`exterior`; one name is one thing, so a space cannot also be an out-of-walk
region or an edge's own volume. That is reference integrity and nothing more.
Whether the *blocks* agree with the statement — whether a space is closed, an
edge holds, a cell is reachable — is a question about an expanded model, and
nothing here asks it.

Claims collect into `Expansion::contract`, **not** into the `VoxelModel`, for the
reason marks do: a claim writes no blocks, and folding it into the block grid
would change what `canonical_bytes` means and make "declaring a space changed
nothing about the building" untestable. `tests/contract.rs` asserts exactly that,
over every program in the library, by wrapping every rule of each in a claim.

`spatial-contract` is the corpus example: two rooms, a barred door and a corbel.

### The obligations — what the blocks have to agree with

The declaration is the author's; the blocks are the evidence; a gate is the
disagreement between them. **Nothing is read out of the voxels.** A space's kind,
an edge's class and an envelope's claim all come from the document, and the
checker's only job is to prove the building agrees.

`crates/grammar/src/contract.rs` is that checker, over one pair — a block grid
and a resolved contract. It runs from two doors and is the same code at both:
`delve-grammar expand`, where a red writes no `.nbt`, and `delve-admit audit`,
where a red is `DW0782` and exit 1. It runs whenever a piece declares a
contract; there is no flag.

| gate | what it proves | what it binds to |
| --- | --- | --- |
| `contract-well-formed` | entry carries an exterior traversal edge; no two spaces overlap; each space is one floor (standable span ≤ 2 levels, out-of-walk cells excluded); each out-of-walk region nests wholly in one space or none; each opening lies on the boundary its endpoints share; each transit volume is disjoint from every space and touches both ends; `rise` present, absent and signed per class | spaces + regions + edges |
| `contract-coverage` | every standable cell lies in a declared space, an out-of-walk region, or a traversal edge's transit volume | standable cells |
| `contract-closure` | every boundary cell of an `enclosed` space (and the side faces of `open_top`) is non-passable, except a declared opening, an abutting space, or an abutting out-of-walk region; and an `open`/`open_top` claim is refused over a cell with this piece's own blocks overhead | boundary cells examined |
| `contract-edge-proof` | per class: `walk` connects both ways; `stair` connects through its own treads; `drop` falls forward and does not walk back; `barred` does not connect while its bar stands and does connect with the bar voided; and in every class the declared `rise` equals `min_y(b) − min_y(a)` over the resolved boxes | interior edges |
| `contract-no-body` | every out-of-walk region earns a **computed** kind — `sealed` (the union of sealed regions is itself closed), `posted` (an anchor inside it, and every standable cell within Chebyshev 2 of one), `facade` (not nested in a space, and every standable cell touched by the air outside the piece). A region earning none is red, and so is one holding no standable cell | regions |
| `contract-reachability` | every standable cell of every space, minus nested out-of-walk cells, plus every standable cell of a transit volume, is reached from the entry space by a walk **confined to declared spaces and crossing only through declared edges** — bars standing, drops forward only. A space behind a bar is re-walked with bars opened and the required set is named | target cells |
| `contract-anchors` | every declared anchor lands in a contract element, and the element is written into the metadata as the anchor's `resolves_to` | anchors |
| `contract-exterior-faces` | every `exterior` edge exports a face with cells on it — a claim nothing can mate with is not a face | exterior edges |
| `contract-no-body-majority` | a piece mostly out of walk says so | standable cells |

A binding of zero is red on closure, edge proof and reachability, and is stated
by name as a finding on every gate. One space and no interior edge is the
exception the vacuity rule names: a room with a door has no traversal claim to
prove, and that is a printed finding rather than a red.

The verdict also **enumerates every opt-out instance**: each open envelope, each
sightline, each out-of-walk region with its computed kind and its anchors, each
bar the walk had to open, each exterior face. A count is a thing a script can
satisfy; a list is a thing a reviewer disagrees with.

### An opening is claimed, never discovered

The only thing that excuses a passable boundary cell in an `enclosed` envelope is
a **claimed** region: a declared opening, a neighbouring space, or an abutting
out-of-walk region. An edge on its own excuses nothing.

The reason is the rule every escape hatch here answers to — *an opt-out must be
secured by a property the defect cannot supply.* Between two declared spaces an
undeclared opening costs nothing, because an abutting space is already an excuse
and crossing into one without an edge still fails the confined walk. Toward
`exterior` it would be an unsecured hatch: the demand would be "declare an edge",
and a wall that is simply missing declares one as easily as a door does. So an
opening to the outside is named, its cells are claimed, and the checker then
requires them to touch the space and to be reached by the air outside the piece —
which a wall claimed as a door in the middle of a room cannot supply.

What a piece leaves open at its own outer face is not a closure question at all.
That is the **face contract**, and assembly consumes it (§6).

### The acknowledgement, and what it cannot buy

`no_body_majority_ack` silences the majority gate. As written it demands a
string, which is a property the failure it excuses supplies for free — so it is
narrowed by a fact the author cannot write: it silences a majority made of
`sealed` and `facade` cells, whose demands are facts about the blocks, and never
one made of `posted` cells, which is the kind an author secures by placing
something. It never weakens any region's own proof.

## 2e. The document version, and what an optional field owes

§2 has the ledger — every number the format has and the surface each names.
This section is what an **optional field** owes that ledger.

Writing a construct above the version a document declares is
`ProgramError::FencedConstruct`, naming the construct, the version that
introduced it and the version declared. That is what lets an older document keep
compiling to the same bytes forever, and it is why raising `version` is a
deliberate act rather than a side effect.

**Why an optional field needs this and a new rule body does not.** `Node`,
`Cond`, `Expr`, `Size` and `MarkAt` are tagged, so an engine that predates a new
variant meets an `"op"` it does not know and fails loud. A `#[serde(default)]`
struct field has no such property: it rides through every walk untouched in both
directions, so an engine that predates the field deserialises the document with
the field's default, expands, passes every gate, and writes different geometry.
Two mechanisms answer it, and both are enforced by
`tools/check-grammar-ir-compat.py` in CI:

1. Every IR object type is a **closed schema** (`deny_unknown_fields`), so an
   engine meeting a document from a newer engine refuses it by name. The
   exceptions are the two types with a flattened sum inside them — `mark`, whose
   `at` is flattened, and an `edge`, whose `class` is: serde cannot combine
   `flatten` with the attribute, which compiles and then reads every flattened
   key as unknown, so the engine would refuse its own documents. The ledger below
   is what holds those two, and CI names the incompatibility rather than asking
   for the attribute.
2. Every optional field is in that ledger with the version it arrived at, checked
   in both directions, and anything above `1.0.0` must be refused by name in
   `ir.rs` — a version constant that nothing enforces does not count as a fence.

| Field | Since | Fenced by |
|---|---|---|
| `geom::Mirror.x` | `1.1.0` | `via ir::Reorient.mirror` |
| `geom::Mirror.y` | `1.1.0` | `via ir::Reorient.mirror` |
| `geom::Mirror.z` | `1.1.0` | `via ir::Reorient.mirror` |
| `ir::Alternative.weight` | `1.0.0` | — |
| `ir::Alternative.when` | `1.0.0` | — |
| `ir::Cond.mirror` | `1.1.0` | `MIRROR_SINCE` |
| `ir::Contract.edges` | `1.2.0` | `via ir::Program.contract` |
| `ir::Contract.no_body` | `1.2.0` | `via ir::Program.contract` |
| `ir::Contract.no_body_majority_ack` | `1.2.0` | `via ir::Program.contract` |
| `ir::Contract.spaces` | `1.2.0` | `via ir::Program.contract` |
| `ir::EdgeClass.rise` | `1.2.0` | `via ir::Program.contract` |
| `ir::EdgeClass.via` | `1.2.0` | `via ir::Program.contract` |
| `ir::Mark.facing` | `1.0.0` | — |
| `ir::Mark.index` | `1.0.0` | — |
| `ir::Node.palette` | `1.3.0` | `BIND_SINCE` |
| `ir::Node.params` | `1.3.0` | `BIND_SINCE` |
| `ir::Program.contract` | `1.2.0` | `CONTRACT_SINCE` |
| `ir::Program.palette` | `1.0.0` | — |
| `ir::Program.params` | `1.0.0` | — |
| `ir::Reorient.mirror` | `1.1.0` | `MIRROR_SINCE` |
| `ir::Reorient.x` | `1.0.0` | — |
| `ir::Reorient.y` | `1.0.0` | — |
| `ir::Reorient.z` | `1.0.0` | — |
| `ir::Split.orient` | `1.0.0` | — |
| `ir::Split.repeat` | `1.0.0` | — |
| `ir::Split.rounding` | `1.0.0` | — |

A row above the floor names the constant `version.rs` declares it at, and CI
requires a refusal in `ir.rs` whose **guard reads that field** — a constant is
not a fence, and a refusal that looks at some other field of the same version is
not this field's fence. `via` is for a field reachable only through another
ledgered one: the three booleans of a `mirror` object exist only inside a
`mirror`, and every field of the contract block exists only inside a `contract`,
so the fence on the field that carries them is theirs.

What the fence cannot do, stated rather than left to be discovered: it cannot
reach an engine older than the fence itself, because that engine's refusal would
have to be code it already carries. `1.1.0` is the first version any of this
exists in, so the window it does not cover is the one before `1.0.0` was ever
declared, and no `Program` has been checked in outside this repository.

## 3. Determinism (ADR-0006)

Same program + same region + same seed → byte-identical `VoxelModel`, asserted by
a double-expand test over every library program at five seeds, plus a
seed-sensitivity test over a probabilistic program, over the declared
anchors (names, cells and per-stem numbering alike), and over the resolved
spatial contract — in two processes as well as twice in one, since a process
warmed by the first run can hide an address-order dependency. All randomness is one
splitmix64 stream from the caller's seed; all maps are `BTreeMap`; cells iterate
`x`, then `y`, then `z`; nothing reads the clock, the environment or a path.
`VoxelModel::canonical_bytes` is the comparison/hash form.

Expansion holds no global state — two programs cannot influence each other, which
is regression-tested.

A `bind` frame is resolved from `BTreeMap`s by name and draws nothing, so it can
perturb neither the draw order nor the visit order. Measured rather than argued:
`tests/arguments.rs` expands `idiom-arguments` — whose one recursion is reached
under four different frames — in **two separate processes** and compares the
`.nbt` and the metadata byte for byte, and reaches the same `.nbt` again through
the JSON authoring form of the nine-rule program it replaces.

The same promise is asserted one layer out, on the bytes that actually ship: a
double-**export** test over the three ported programs of §5 at four seeds
compares the `.nbt` and the metadata JSON byte for byte (§6). The §5b staging
rules and the §5c zone programs are **not** in that suite — `tests/export.rs`
carries `temple` / `castle` / `church` and nothing else; what covers the staging
rules is the registry round trip (`crates/compiler/tests/grammar_prefab.rs`),
which exports once and reads back, not twice and compares.

## 4. Failure is loud

The interpreter has no silent degradation. `Program::validate` runs before any
expansion (unknown rule/role/param, empty rule or split, child/piece mismatch on
a non-repeating split, zero weights, a `rounding` other than `truncate` on a
split with no relative piece — nowhere to put the remainder — `split_axis` named
outside a split, an `orientation` guard that is not a
permutation — a guard nothing could ever match — a `mark` whose anchor stem
is not kebab-case, a `bind` that binds nothing, and a `bind` naming a parameter
or role the program does not declare). During expansion: `NoApplicableRule`,
`Split{Overflow|ZeroStride}`, `Orient`, `BadSize`, `Eval`, `PaletteFull` (more
than 65 536 distinct block states in one model), `MarkOutsideScope`,
`MarkFacingNotCardinal`, `AnchorCollision`, and the `DepthLimit` / `ScopeLimit` /
`VolumeLimit` budgets. Errors carry the rule name and print as prose, never as a
`Debug` struct.

**A refusal names the comparison, not just the rule.** A guard exhaustion
(`NoApplicableRule`) reports, for every rejected alternative, each condition
that decided the rejection with **both operands as evaluated at that scope**:
every failed conjunct of an `all` at once (an author handed one constraint at a
time re-runs into the next), a condition that *held* under `none_of`,
required-versus-actual for an `orientation` guard, and a conjunct that cannot
be evaluated (say, a division by zero behind a short-circuit) by name. A
composite operand shows the authored expression, its value, and the value of
each `dim`/`param` inside it —
`required dim:z >= ((param:gate/approach + param:gate/span) + 4); at this scope
left = 12, right = 15  [param:gate/approach = 8, param:gate/span = 3]`. The
refusal also states the scope's **own** local dimensions, orientation and world
box — the dimensions at the failure site, which after `reorient`s and splits
are not the region on the command line — and the derivation path that reached
it, as rule names and split pieces (`mainline › split z→z piece 4/6 › …`).
Wherever a frame is printed, required or actual, it is printed as a frame:
the axis mapping, and `local z reversed` after it when the frame reflects.
Two frames can share a mapping and differ, so a mapping-only rendering shows a
reflected-frame rejection as `required x→x, y→y, z→z; this scope has x→x, y→y,
z→z` — an author told that their guard failed against itself.
`Split`, `Orient`, `BadSize` and `Eval` carry the same scope-and-path record; a
split refusal additionally shows its evaluated size pattern beside the authored
expressions, and a bad size or failed evaluation names the expression it came
from. `PaletteFull`, the budgets and the mark errors do not: the palette and
the budgets are model-global, and a mark error already names its cell and box.
All of it is asserted — operand values included, because a message is a claim —
in `tests/failures.rs`.

The three budgets live on `Limits` and are inputs, never silent clamps:
`max_depth` and `max_scopes` turn an unguarded recursive rule into a diagnostic
instead of a hang; `max_volume` (default 2²⁴ cells) is checked *before* the dense
model is allocated, so an absurd region is an error rather than an OOM kill —
the one failure mode that reports nothing at all. A caller who means to build
something enormous raises the limit explicitly.

Writing outside the model's own region is a caller defect, not an input:
`VoxelModel::set` asserts it in a debug build and drops the write (never wraps
it) in a release one.

These are Rust error values, **not** `DW` diagnostics: the craft-rule diagnostics
of spec-0027 §4 are a later phase and will own a DW range then.

Consequence for authors: a region too small for a program's absolute sizes is an
error, not a building with pieces outside its box. Each library program documents
its minimum region.

## 4b. Blocks have to exist

Every block state the export writes is checked against the pinned 1.21.11
block-state registry (`crates/compiler/data/blocks-1.21.11.json`, 1166 blocks,
via `delvewright_schem::blocks`) — the id, every property name, and every
property value. An unknown state is `ExportError::UnknownBlocks`, a refusal, with
the cell count and a suggested rename.

The check is **at the emitter, not in a test**, for the reason CLAUDE.md records
for commands: the operator running the tool does not run `cargo test`. Its cost
if absent is total and silent — a structure template loads an unknown block as
AIR, so the piece is well-formed, the generator exits 0, the determinism gate
passes, and the feature is simply not there. `minecraft:chain` was renamed
`minecraft:iron_chain` in 1.21.11; when this gate was first run over the library
it found `threshold_motif` painting the old id, i.e. the boss-door bell-rope
curtain — the entire point of that rule — had been 14 cells of air.

`tests/library.rs` asserts it over every program in the library with its binding
count, and `gates::judge` reports the same verdict without exporting.

Three more members of the same spelling rule ride the same sites. Two are gate
and export refusal both (`DW0735`, `DW0736`) and run before the contract
refusal, because a state that omits its connections or lands the wrong way round
changes what the bytes *mean*, and a contract checked against isolated posts
answers about a different building; the third (`DW0737`) is a gate only. A
fourth (`DW0738`) is neither — it refuses during expansion, before there is a
model to judge at all.

- **Shape completeness (`DW0735`).** A placed state must write every property
  named by a `multipart` selector in its block's own blockstate definition
  (`crates/compiler/data/blockstate-shape-props-1.21.11.json`, derived from the
  client jar). A `variants` property the state omits renders the complete
  default model — benign; a `multipart` property it omits drops assembled
  geometry, so a bare `iron_bars`/`oak_fence`/`cobblestone_wall` places as a row
  of isolated posts. When this gate was first run over the corpus it found
  `broken_grate` and `far_side_bar` both painting bare `iron_bars` — every grate
  and every sealed doorway they had ever built. The check is on the state and
  not on the frame, so a `bind` that rebinds a role to an incomplete state is
  caught exactly as an inline one is.
- **Oriented fills (`DW0736`).** A frame permutes *and reflects* geometry and
  never rewrites block-state properties, so a **world-frame** literal
  `facing`/`axis`/connection/`rotation` state inside a turned or mirrored scope
  lands however the scope was framed. **Both halves of the frame count.** A
  reflection is not a permutation — no rotation reproduces one — and it is a
  sign on the axis: under a reflected local `Z` a literal `facing=north` lands
  south, and a door's `hinge` and a stair's corner `shape` flip, which is what
  nothing rotational does. A reflection of an axis the state does not name is
  harmless and is not reported.
  Two mechanisms answer it — the local axis frame (§2), and `Cond::Orientation`,
  one alternative per frame each carrying the matching state — and the expander
  records every fill that uses neither (sensitivity derived from the registry's
  value vocabulary, `BlockRegistry::oriented_mismatch`, which reads the
  permutation and the reflection). A passed guard licenses a fill only while the
  frame it asserted still holds, and the guard names the frame exactly,
  reflection included, so a guard written for one arm of a mirror pair does not
  license the other. `bind` and `claim` move neither half of the frame: a pushed
  paint is read in the frame of the scope it lands in, and a claim hands the
  licence on unchanged.
  First run over the corpus, it found `cliff_path`'s skull yaw literal under the
  recess's own reorientation: the same program at a box longer in world X
  shipped skulls facing along the path instead of out of the niche. The gate's
  detail states three numbers — fills examined, fills carrying properties, and
  how many of those were resolved out of the local frame — so a population that
  moves to the frame is visible rather than a binding that quietly fell.
  A finding names the frame as `x->X,y->Y,z->-Z`, with a leading `-` on a
  reflected axis, so a mirrored author is not shown a frame that reads as
  identity.
- **State completeness (`DW0737`).** The whole class `DW0735` is the hard half
  of. A placed state must write every property its block has, including the
  ones whose default is benign for the model. Vanilla fills an omitted property
  from the block's default state, so a partial state is legal and a running
  server resolves it correctly — and nothing upstream of the server can: the
  review render, the navigation walk, the diff a reviewer reads and the machine
  gates each have to guess, and the guesses disagree with each other and with
  the game. An `oak_stairs[facing=east]` with no `half` and no `shape` is a
  stair whose geometry no document states, and vanilla recomputes `shape` from
  the stair's neighbours on every block update. First run over the corpus, this
  found fourteen authoring sites across nine of the library's programs — the
  church's four roof stairs and both its door pairs among them — and fifteen in
  the drowned-bell zone programs. A gate and not an export refusal: unlike
  `DW0735` the omission costs no geometry in the emitted template, so what it
  judges is what was AUTHORED.
- **Unresolvable local frame (`DW0738`).** A state written in the scope's own
  axis frame whose image the pinned vocabulary does not determine: a yaw or a
  handedness under anything but a pure turn about the vertical — which is to
  say under any reflection, or under a frame that moves the vertical — a
  `top`/`bottom` half under a frame that moves or reverses the vertical, a
  horizontal connection turned onto a block with no `up` key, a rail's
  direction-composed `shape`. Refused at expansion, naming the state, the
  property and the frame. It shares its classifier with `DW0736`, so a state one
  of them calls wrong is never one the other quietly rewrites — the judge and
  the rewriter are one transform, read from two ends, and the refusal is what
  keeps them from ever disagreeing.

`tests/shape_orient.rs` demonstrates all four red→green on real pieces —
`broken_grate`'s bars for the first three, `far_side_bar`'s for the frame, in
both directions. `tests/frames_blockstate.rs` carries the gates across the frame
constructs — an unguarded fill inside a mirrored body, a `claim` under a
reoriented scope, a `bind`-rebound role, and what a refusal's message names
inside a reframed subtree — and `tests/frames_local_paint.rs` is the same sweep
for a LOCAL paint: inside a mirrored body, under a frame that reflects and
permutes at once, under a pushed argument frame, inside a claimed space, and
over all forty-eight frames the grammar can build, where every case either
resolves to a state the pin accepts or refuses with `DW0738`. `tests/library.rs`
and `tests/zones.rs` sweep the gates over every library program and every bell
zone with summed binding counts; `delve-grammar audit` (§4e) runs the same sweep
over a campaign's own zone programs, which is where a zone that has left the
engine's copy behind is caught.

## 4c. Opt-in gates — the claims a piece makes

Three gates run only when the author says the piece makes their claim, because
each is a claim about a *kind* of piece rather than about every piece.

- **`traversable`** — a walk joins every pair of the piece's **declared ways in
  and out**: its `exterior` edges, read as the face contract (§2d). The binding
  count is doors. `allow_falls` adds a one-way fall edge, for a piece entered by
  stepping off a ledge. A piece with fewer than two declared ways out has nothing
  to walk *through* and the gate says so rather than passing. A room with one
  door would fail this correctly and uselessly, which is why it is opt-in.
  A piece that declares **no** contract has no doors to count, so the gate falls
  back to the region's world `Z`-max and `Z`-min faces and its detail says in
  full that the number beside it is standable cells on two faces and not ways in
  — the count that reports 47 approaches where three are doors.
- **`symmetric`** — the piece is its own mirror image across the mid-plane of a
  named world axis. It compares **presence, not block state**: a stair placed
  correctly on both sides of a mirror plane is a different state on each side,
  since nothing reflects a `facing=` property, and comparing states would red
  every symmetric building that contains one. Solid-versus-not is the property a
  mirror plane really asserts. An odd extent leaves the centre plane paired with
  itself and it is not counted; a one-cell axis therefore binds to nothing, which
  is reported as a finding rather than folded into a pass.
- **`reachable-floor`** — every standable cell with something solid overhead can
  be walked to from the grade entrance. The reachability measurement behind it
  runs on every expansion either way (§4d); this flag is only what turns its
  sheltered half into a verdict, for a piece that claims a body can get
  everywhere indoors. It is opt-in for the same reason `traversable` is and more
  so: 13 of the 35 library programs have **no** roofed floor at all — `castle`,
  `church` and `stair-flight` among them — and the gate binds to zero on each,
  which is a finding and not a pass. A piece is entitled to strand floor:
  `rafter_hall`'s rafters are meant to be looked at, and `drop_shaft` is one-way
  by design.

`symmetric` is what reads a defect no other gate can. A shape with a mirror plane
is built by expanding one rule at both sites; if one site is instead a hand-kept
copy, or is missing its reflection, the building has a hole in one flank and
`blocks-exist`, `shape-complete`, `states-complete`, `oriented-fills`,
`non-empty` and `traversable` are all still green over it — a missing half is a hole, and every
state in it is spelled and framed correctly. The
gate compares the halves and names the first cell pair that disagrees.

```sh
delve-grammar expand --program idiom-mirror --region 15x11x2 --seed 1 \
  --symmetric y -o out/
```

## 4d. Reachability — how much of the floor a body can get to

`traversable` proves one thing: a walk joins the approach face to the exit face.
Both faces are at ground level, so a piece passes it with every storey above the
floor stranded. The Notre-Dame zone of `docs/trials/trial-0001-notre-dame.md`
passes it at 31 × 64 × 93 with **2267 of 4982** standable cells reachable and
**zero** reachable above the ground band: five levels of aisle, gallery, belfry
and tower deck that no body can walk to.

So every expansion also carries a **reachability measurement**, printed by
`delve-grammar expand` and written into `<id>.report.json` whether or not any
optional gate was asked for. It walks `nav::components` over the standable cells
from `nav::ground_entry` and reports:

| Number | Meaning |
|---|---|
| `standable` | cells examined — the measurement's binding count. Zero is a finding |
| `entry_cells` | standable cells on a **side face at grade**, where a body walks in. Zero is a finding, never a reachability of zero |
| `reachable` / `reachable_share` | what the walk covers |
| `sheltered` | standable cells with something solid overhead |
| `unreachable_sheltered` | floor under a roof with no route to it — **a room with no way in** |
| `unreachable_open` | unreachable floor open to the sky |
| `pockets`, `largest_pockets` | how many disconnected pockets, and the bounding box of five of them |

**The entrance is derived, not assumed.** Grade is the lowest `Y` at which any
side-face cell is standable, and the entry set is every side-face standable cell
**at grade or one course above it** — `y <= grade + 1`, inclusive of both, one
course because that is the walk's own step height. A belfry
louvre is a standable cell on a side face and is deliberately *not* an entrance:
seeding a walk from every opening in a building is how a reachability measure
reports a stranded gallery as reached.

**A roof is standable and nobody walks it, and the engine cannot tell a roof from
a terrace.** The one distinction it *can* draw is whether anything solid stands
over a cell, so that is the distinction the report draws and the only one it acts
on. `unreachable_sheltered > 0` is raised as a finding by name, with the pockets
to go and look at, ranked most-sheltered-first. `unreachable_open` is a number
and never a finding: almost every building has an unreachable roof, and raising
it every time is the nag that costs the other finding its reader.

`--reachable-floor` (§4c) is what turns the sheltered half into a verdict.

**A one-way descent cannot be stated, and the gate cannot be told about it.**
`nav::reachable_with_fall` is the predicate that would answer "a body gets down
there but not back up", and `drop_shaft`'s own tests are gated on it in both
directions — but nothing outside `cargo test` can ask, so no flag, no report
field and no metadata carries the claim. On a piece whose design *is* a one-way
drop, `--reachable-floor` is therefore not a gate to satisfy but one to leave
off: `drop-shaft` at 9×12×9 seed 1 fails it with 28 of 63 roofed cells
unreached, and a red gate writes **no** `.nbt` (exit 4), so passing the flag does
not ship a piece with a red — it ships nothing. What to do instead: expand
without the flag and read the always-on reachability line, where the stranded
lower level appears as an `unreachable_sheltered` pocket with its bounding box.
That pocket is the design, and the engine cannot tell it from a room with no way
in. The verdict is bounded by the instrument, and this is where it says so.

## 4e. `audit` — the sweep that makes the gates invoked

`expand` judges the one program an operator names. That left the corpus judged
only when somebody remembered to walk it, and a campaign's zone programs — the
artifacts of record — had no caller at all: nine machine gates written over one
zone read 1 of 9 on the unmodified program with five at zero binding, and then
stopped running, because nothing invoked them.

```sh
delve-grammar audit --library                       # the rule library
delve-grammar audit --campaign-root ../content      # every campaign's zones
```

It enumerates a corpus, expands every member at the expansion that corpus
declares, runs the same `gates::judge` `expand` runs, prints a binding count per
gate over the whole sweep, and writes nothing. It reds when any gate fails, when
any gate examined zero objects, and when the corpus it was pointed at was empty.

**The two corpora are counted apart**, because they have different owners and a
zero means a different thing in each:

```
corpus: library N program(s)
corpus: campaign N program(s) over R root(s)[ — FINDING: zero binding, …]
```

The rule library is this repo's own (`library::PROGRAMS`), so `--library` over an
empty one is a defect here and reds. The campaign corpus belongs to the content
repo, where an in-progress campaign lives on its own development branch until the
owner has played it, so a root carrying no zone program is a fact about that
checkout: the run says so as a named finding and stays green. Whether that zero is
the RIGHT zero is a separate question with a separate answer, §4f. Summing the two
totals is what let a full library carry an empty campaign root to a green board
with the word *campaign* nowhere in the output.

A campaign declares its zones in `design/programs/zones.json`, beside the
programs it governs: per zone an id, the program file, the region, the seed and
which optional gates the zone claims (`traversable`, `allow_falls`,
`reachable_floor`, `symmetric`). That file exists because a grammar program is
region-polymorphic — a program alone cannot be expanded, and while the region
lived in a design page nothing could check a zone program at all. The mapping is
a bijection: a programs directory with no manifest is a finding, a program file
no entry names is a finding, and an entry naming no file is a finding. Without
those three, "add a zone program" and "add a zone program nothing will ever
check" are the same action.

A program that is known red is recorded in the pipeline repo's
`.github/zone-audit-exclusions.json` with the exact diagnostic codes it must fail
with and the capability gap that keeps it red. The record INVERTS the assertion
rather than removing it: the program is still expanded and still judged, and it
is a finding if it passes, if it fails with a different code, or if it fails with
one more. An entry belongs there only while the engine is missing a capability
the program needs. Ids are audit labels, so both corpora are recordable:
`library/<program>` and `<campaign>/<zone>`.

Every zone program of every campaign expands and judges green. The rule library
holds one recorded red: `library/causeway` (`DW0800`) floods its ward floor to
ceiling on both flanks of its spine, which is what makes the flanks unwalkable,
and lowering the waterline needs `nav` to know that a body cannot stand on water
— until it does, a lowered waterline would read as walkable floor and the ward's
own claim would go green while being false.

The sweep also totals the **local-frame binding count** — how many fills read
their states in the scope's own axes — beside the gate whose population they
come out of, so a green `oriented-fills` that got greener by writing fewer
world literals says so in numbers rather than by silence.

## 4f. The pinned campaign corpus, enumerated

`.github/content-zone-corpus.json` names the campaigns the pinned content repo
carries and how many zone programs each declares.
`crates/grammar/tests/campaign_zones.rs` checks every number in it against the
content checkout, inside `cargo test`.

It exists because the campaign corpus is not this repo's to produce. An
in-progress campaign lives on its own content-repo development branch and reaches
content `main` only after the owner has played it, and CI checks the content out
at `versions.toml` `[content].sha`. So the pinned tree can legitimately carry no
zone program at all. "The sweep found nothing, so it passes" would then be an
opt-out the defect itself supplies: deleting every zone program of every campaign
produces exactly that state. Enumeration is the different demand — a campaign that
loses its programs reds on a count, and a pin that genuinely carries none passes
with its inventory printed.

| Field | Meaning |
|---|---|
| `content_sha` | must equal `versions.toml` `[content].sha` |
| `on_pin[]` | `campaign`, `zone_programs`, `note` — a campaign the pin carries |
| `off_pin[]` | `campaign`, `zone_programs`, `branch`, `note` — a campaign known to own zone programs somewhere this repo cannot see |

Five assertions, and the last two are what keep the enumeration from becoming a
choice the author makes:

1. `content_sha` equals the pin. This is what binds the record to the event it
   guards: a re-pin cannot land without the inventory being restated at the new
   pin, and a restated inventory is checked against the tree, so writing a number
   the tree disagrees with is a red rather than a shortcut.
2. Every `on_pin` count equals the number of program files in
   `campaigns/<c>/design/programs/` **and** the number of entries in that
   campaign's `zones.json`.
3. Every campaign the checkout carries is named. An unnamed one would sweep as
   zero and say nothing.
4. Every `on_pin` entry is present in the checkout. A campaign emptied of both
   `world.json` and `design/` stops being a campaign directory, and this is what
   notices.
5. Every `off_pin` entry is **absent** from the checkout. `off_pin` is a queue,
   not an exemption: an entry that has landed must move across and have its count
   checked, so a campaign cannot be parked in the queue to avoid the count. Which
   list an entry belongs to is decided by the tree, never by the author.

Every sweep in that file prints the corpus it examined, its binding count, and the
pin it was measured at; a zero is printed as a named zero rather than left to
silence.

This repo can gate only what the pin lets it see. A campaign's zone programs on a
content development branch are gated there, by the content repo's own
`zone-audit.yml`, which runs `delve-grammar audit` against a pinned checkout of
this repo on every push and pull request.

## 5. Rule library — ported buildings

Ported from `yawgmoth/GDMC25` (BSD-3-Clause; see
[`ACKNOWLEDGEMENTS.md`](../ACKNOWLEDGEMENTS.md)) and reachable as
`library::{temple, castle, church}`.

| Program | Controls | Smallest region that expands (measured) |
|---|---|---|
| `temple` | `roof` (pitched/flat/capped/open), `column_height`, `column_size`; role `marble` | X ≥ `6 + 2*column_size`, Y ≥ `1 + column_height + roof height` (5 pitched / 3 flat / 1 capped / 0 open), Z ≥ 7 |
| `castle` | `large_tower`, `small_tower`, `great_hall`, `wall_height`, `wall_width`, `tower_height`; role `stone`; declares `anchor/courtyard` | both horizontal extents ≥ `2*large_tower + 2`, Y ≥ `tower_height + 1` |
| `church` | guards only; roles `wall`, `glass`, four `roof_*` stair facings, two door pairs | height must follow width (the roof steps in 2 per course): Y ≥ 9 and Y ≳ X − 3; 15 × 16 × 30 is comfortable |

Ports are faithful except where a module says otherwise; the three substantive
divergences are recorded at their code: the temple's colonnade repeats to fit the
box instead of being fixed at four columns, the church's one-wide ridge course is
guarded (upstream splits it anyway and writes outside the region), and
constraint `largest` returns the maximum (upstream returned the minimum — a
copy/paste bug).

Two library claims are pinned by arithmetic rather than prose, because prose
rots: the temple's colonnade really does follow the box, and a nine-deep box at
`column_size` 1 really does give upstream's four columns
(`tests/library.rs`).

## 5b. Staging vocabulary — original rules with gates

`library::{cliff_path, watch_bay, rafter_hall, ambush_door, store_room}` are
**original Delvewright content** (licence `original`; nothing ported, no ledger
entry owed). They are the drowned-bell remake's grammar vocabulary — W1 (path
and hazard geometry) and W2 (interior ambush) — and they are a different kind of
rule from §5: a temple is judged by looking at it, these are judged by a
**machine gate about how the space plays**. Every gate below is an assertion in
`crates/grammar/tests/staging.rs` over the expanded model, and each has been
shown to go red when the geometry is wrong.

### The W1 local frame

All five rules share one frame, and it is not arbitrary:

> **Local `Y` is up. Local `Z`-max is the approach end; travel runs toward local
> `Z`-min.** Length is turned onto the box's longer horizontal axis.

A `mark` with no explicit `facing` derives one, and the derived facing is
*always* the negative direction of the world axis the scope calls local `Z`
(§2b) — a rule can only aim an anchor down-axis. Travel is defined that way so
every derived facing points at the thing its anchor is about. The price is that
**indexed anchors number against travel** (a split visits its pieces low to
high, so declaration order is fixed by the axis while the facing points the
other way down it): `anchor/niche-1` is the *last* niche the player meets. With
`mark` as it stands the two cannot both follow travel, and a wrong facing is
wrong *data* where a numbering convention is only documentation. See §7 for the
primitive that would remove the trade-off.

### `cliff_path` — the knockback niche

A one-wide ledge along a drop face, with 1-deep × 2-high recesses cut into the
inner wall every `spacing_min ..= spacing_min + 3` cells (a uniform seeded draw
over whichever of the four spacings the remaining path has room for).

| | |
|---|---|
| Controls | `spacing_min` (6), `niche_height` (2), `watch_back` (3); roles `rock` (the corpse prop is per-frame guarded inline states, `corpse_prop` — its yaw follows the recess's frame, one alternative for each of the four frames a caller can reach: the two horizontal permutations and the half-turn of each. A role in the scope's own axes would say the same in one binding, and this piece is one of the corpus's two remaining demonstrations of the `orientation` guard, §7) |
| Smallest region | 3 × (`niche_height` + 2) × 3, and at least as long as it is wide |
| Anchors | `anchor/niche-<i>` — inside each recess, facing the ledge (derived through a `reorient` that names the across-path axis as local `Z`; this is why the ledge is at local `X`-min). `anchor/niche-watch-<i>` — a ledge cell `watch_back` up-path, facing down-path. |
| Variants | weighted alternatives per slot: teach (2) — one recess with a corpse prop, no occupant; test (3) — one empty recess; twist (1) — two adjacent recesses, each with its own anchor pair |

Gates:

1. **The recess is exactly one deep** — air at the anchor cell and the cell
   above, backing wall immediately behind, floor below, lintel above, ledge in
   front. An occupant's hitbox sits inside it and a swing from the ledge reaches
   it; one deeper and the niche becomes a room the player must enter.
2. **The ledge is the only route** — the standable-cell graph connects the two
   ends, and with the `X`-min lane deleted it does *not*. A recess beside a wide
   path is decoration. The test also asserts every standable cell is either the
   lane or a declared recess, so the lane is one wide by measurement.
3. **Each watch cell sees its niche's mouth** — the same sightline walk as
   `watch_bay` below, to the ledge cell the recess opens onto. Deliberately
   *not* into the recess: a 1-deep recess off a 1-wide ledge is geometrically
   invisible from anywhere down the path, which is precisely what makes it an
   ambush. The legible thing is the contested ground.

### `watch_bay` — observability hardware

A gated passage whose approach end carries a roofed 2×2 bay, walled on three
sides, open only toward the hazard span, with the lane running past it.

| | |
|---|---|
| Controls | `approach` (8), `span` (3), `head` (4), `bay_height` (2), `obstruct` (0 — a test knob) ; role `stone` |
| Smallest region | 6 × (`head` + 2) × (`approach` + `span` + 4) |
| Anchors | `anchor/watch` — the bay cell nearest its open face, facing the span. `anchor/gate` — the span's floor centre, for the campaign's `timed-gate` / `volley`. |

Gates:

1. **Standoff** — `approach` ≥ 6 is enforced *by the rule*: the plan has one
   guarded alternative and no `otherwise`, so a shorter approach is a
   `NoApplicableRule` refusal, not a quietly smaller bay. Measured watch-to-span
   Chebyshev distance is `approach + 1`.
2. **Sightline to every standable span cell**, walked with the same
   Amanatides–Woo traversal and the same eye/centre-mass heights the compiler's
   `DW0388` uses (1.62 / 1.0, both endpoint cells exempt). Deliberately stronger
   than `DW0388`, which asks for sight to *some* cell at 5: the point of
   generating the bay is that the campaign-level proof cannot then fail. That
   proof still runs later, on the assembled world with real hazard declarations
   — what this rule guarantees is that the bay it places **can** satisfy it.
3. **The gate has teeth** — `obstruct = 1` stands one pillar in the bay's own
   column of the approach and the sightline check must go red, while the passage
   stays walkable (so what was caught is blindness, not impassability).

### `rafter_hall` — the rafter perch

A hall whose truss layer is somewhere a body waits. At `h-3` a course of
**corbels** carries `bracket` cells in from each side wall; at `h-2`, over each
corbel's inner end, is a standable cell. Slices repeat every `beam_period` along
the hall and alternate which side is declared a perch.

| | |
|---|---|
| Controls | `beam_period` (4), `bracket` (2), `span_beams` (0 — a test knob); roles `stone`, `timber` |
| Smallest region | the density cap ties width to length, so this is a curve, not a triple: interior `X · Z · period ≥ 24·Z + 24·period`, plus `X ≥ 2·bracket + 3`, `Y ≥ 6`, `Z ≥ period`. 10 × 6 × 12 is the smallest trussed hall at the defaults, pinned from both sides in `tests/staging.rs` |
| Anchors | `anchor/perch-<i>` — a corbel's inner cell. `anchor/hall-door` — the floor cell at the centre of the approach end, which is where the sightline gate stands |
| Variants | `Y < 6` is a **hall with no truss**: same shell, same door anchor, no perches, and not an error. Both shapes are asserted |

**The centre span is open on purpose.** The obvious full-span truss fails gate 1
and cannot be tuned into passing it: an eye on the floor is below the beam plane
and a perch is above it, so every ray crosses the plane over a run of about
`0.42 × distance` cells, and past ~9 cells of hall that run always contains a
nearer beam. A spanning truss hides its own far rafters. Corbels leave the nave
clear, and a ray to any perch crosses the beam plane while still only 16–58% of
the way to that perch's wall — inside the open span at every hall length.

Gates:

1. **Every perch is visible from `anchor/hall-door`**, walked with the same
   Amanatides–Woo traversal as `watch_bay`. Fairness in the souls grammar is
   carried by silhouette, not by sound (`docs/notes/souls-design-language.md`
   §4.3). Teeth: `span_beams = 1` rebuilds the full-span truss and 6 of the
   fixture's 7 perches go blind (0 of 7 at the default), while the nave stays
   walkable end to end — so what was caught is blindness, not a severed hall.
2. **At most one perch per 24 floor cells**, the smallest machine-checkable form
   of the Cathedral's monoculture critique (§4.1). Enforced *by the rule*: the
   cap is arithmetic in the guard, so a hall too narrow for its own rafters is a
   `NoApplicableRule` refusal. Teeth: the fixture at 8 wide would carry the same
   7 rafters over 150 floor cells — a genuine cap breach — and is refused.
3. **The rafters are geometry.** Every perch is standable on timber with
   headroom, and so is the next cell of the same beam. Two red sides in the same
   test: the centre of the nave is *not* standable at rafter height (or the
   truss would be a floor), and no walk from the ground reaches a perch (or it
   would be a mezzanine).

### `ambush_door` — the corner-ambush alcove

A wall across the box with one 1-wide opening, and immediately inside it, one
cell to the `+X` side, a blind pocket.

| | |
|---|---|
| Controls | `head` (3), `door_height` (2), `door_offset` (2), `expose` (0 — a test knob); role `stone` |
| Smallest region | `door_offset + 5 + expose` across, `head + 2` tall, 5 long — and at least as long as it is wide. 7 × 5 × 7 at the defaults |
| Anchors | `anchor/alcove` — the blind cell, facing the door lane (derived through a `reorient` naming the across-wall axis as local `Z`, which is why the alcove is on the `+X` side). `anchor/threshold` — the standable cell in the opening, facing the way the player walks through |

Gates:

1. **The alcove is blind from the approach** — the vocabulary's first
   *negative*-visibility gate, asserted cell by cell over every standable
   approach cell (54 in the fixture, 0 of which see it). Teeth: `expose = 1`
   widens the opening over the alcove's own lane and 29 of the 54 see it.
2. **One swing from the doorway's inside cell** — Chebyshev distance exactly 1,
   swept over `door_offset` so the adjacency is arranged rather than coincident
   with the default.
3. **The doorway is the only route** — the standable-cell graph connects
   approach to inside, and with the doorway's column deleted it does not (the
   same cut `cliff_path` uses on its ledge lane).

A blind alcove is *not* discoverable from the decision point, which §4.2 of the
dossier calls the unfair kind. That is deliberate and it is the **test** rung of
teach/test/twist: the rule declares `anchor/threshold` so a campaign has
somewhere to hang the telegraph that pays for it, and it does not pretend the
pocket is its own tell.

### `store_room` — the container tell

A storeroom whose far wall carries a row of barrels with exactly one `unbanded`
variant among them.

| | |
|---|---|
| Controls | none (the row is as long as the box); roles `stone`, `barrel`, `barrel_unbanded` |
| Smallest region | 5 × 5 × 5 — **both** horizontal extents ≥ 5. `MIN_LINE` (3) is the shortest row in which the odd one always has a neighbour, but the frame makes local `Z` the *larger* horizontal, so a 3-long row can never be reached: the same shape `boulder_stair`'s `MIN_DEPTH` records |
| Anchors | `anchor/store-line` — the barrel at the approach end of the row. `anchor/tell` — the odd barrel's own cell, facing out into the room (hence the row sits at `X`-max) |

**Exactly one, without a counter.** A rule has no memory, so the invariant is in
the derivation's shape: `line_before_tell` either lays a plain barrel and
recurses or spends its draw and hands the rest to `line_after_tell`, a plain
fill that can never produce another; with one cell left neither guarded
alternative applies and the `otherwise` places the tell outright. The two
guarded alternatives overlap on purpose — that overlap *is* the position
distribution (§2), weighted 3:1 toward carrying on.

Gates:

1. **Exactly one tell, and the anchor is on it** — counted off the *blocks* over
   12 seeds, not off the anchors, with the rest of the row asserted to be plain
   barrels so "exactly one" is not an artefact of a one-cell row.
2. **The tell is in the line** — a barrel beside it on at least one side, and the
   row runs the whole lane.
3. **The tell moves with the seed** — 12 seeds put it in ≥ 3 distinct cells (9 in
   practice). A fixed tell is a landmark players learn once.

The default binding keeps both roles one material family and changes only the
variant: a spruce barrel and a spruce log, the same wood with its iron bands
missing. `barrel[open=true]` would be the neater mimic-breath pun and is not
usable — vanilla closes the lid the moment the structure loads, so the tell
would last exactly as long as nobody looked.

All five programs are in the generic library suites too: structural validity,
JSON round trip, palette-swap-moves-no-block over **every** role each binds, and
the double-expand determinism gate over model bytes *and* anchors
(`tests/library.rs`, `tests/determinism.rs`). Their anchors — including generated
`-<i>` names nobody hand-listed, and `store_room`'s seeded tell position —
round-trip through `PrefabRegistry` (`crates/compiler/tests/grammar_prefab.rs`).

### `boulder_stair` — the worn-tread tell (W), and the side pockets (S)

A hazard lane whose centre course takes a `smooth`-variant material down its
length while the side lanes keep the `rough` variant — a Sen's Fortress
telegraph built as paint, not shape. Every `pocket_period` cells its near-side
wall opens into a one-cell dodge.

| | |
|---|---|
| Controls | `head` (4), `pocket_height` (2), `pocket_period` (8); roles `rough`, `smooth` |
| Smallest region | `MIN_X` (5) × (`head` + 2) × `MIN_DEPTH` (= `MIN_X`, since the frame always makes local `Z` the *larger* of the two horizontal extents — a documented depth under the width minimum could never be reached) |
| Anchors | `anchor/stair-run` — the run's floor centre. `anchor/volley-slot` — the vault rib directly over the run's midpoint, for a dart trap; ordinary stone until a campaign binds it (§7: trap anchors are not yet expressible by a rule). `anchor/pocket-<i>` — each dodge, facing the lane |

**S has no grammar of its own in the vocabulary doc.** "Side `alcove` splits
every 8 units (entry S safe pockets)" is the only place the pockets are
described, and it is inside W's own entry — the dispatch line's "W3 = W+S+M+X"
is the only place S reads as a fourth peer letter. What is built is a properly
named, fully gated rule (`pocket_niche`) *inside* `boulder_stair`, not a second
exported program (the IR has no cross-program `call`, so a standalone
`safe_pocket` program could not literally be what this rule's own split uses).
This is filed as an open question for the planner, not decided here.

Gates:

1. **The tread is exactly one material family, at two distress levels** — the
   spec-0027 §4 palette-role budget's own claim, proved against a **test-local
   mirror** of that not-yet-built diagnostic (§7 below; `crates/grammar/src/lib.rs`'s
   own "not built yet" note), scoped to the lane's own floor course. Teeth:
   read the same cells without the family fold and the smooth run's raw share
   genuinely clears the 10% accent ceiling — so the fold is load-bearing, not
   vacuous — and restyling the run to an unrelated material is still correctly
   caught as a genuine accent overrun under the same grouped reading.
2. **The lane is the only continuous route** — the same cut `cliff_path` and
   `ambush_door` use, here on the pocket band: it is solid everywhere except at
   pocket slots, so it cannot substitute for the lane end to end.
3. **A pocket is a one-cell dodge, visible from the lane** — standable, one
   deep, backed and lintelled like `cliff_path`'s niche, but (unlike
   `ambush_door`'s alcove) *not* blind: a dodge nobody can see coming is not an
   escape.
4. **`pocket_period` is a real control**, the same claim `cliff_path` makes for
   `spacing_min`: widening it thins the pockets out.

A box shorter than one `pocket_period` cannot tile even one pocket
(`make_split` checks the un-repeated pattern before it tiles) — the same shape
`rafter_hall` uses for a hall too short for its truss, a plain lane and no
`anchor/pocket-*` is a variant, not an error.

### `threshold_motif` — the boss-door threshold motif (M)

A doorway spanning the box's whole interior width, hung above walking height
with a bell-rope curtain tiled by `split_repeat` — so a motif taught in one
zone and rebuilt wider for another keeps the same strand density without being
retuned per size.

| | |
|---|---|
| Controls | `head` (4), `curtain_height` (2), `strand_period` (1), `single_strand` (0 — a test knob); roles `stone`, `curtain` |
| Smallest region | 3 × (`head` + 2) × 3 — and `head` ≥ `curtain_height` + 2, so there are two full cells of walk clearance under the curtain (one is not enough for `standable`, which also asks for the cell above the player's head) |
| Anchors | `anchor/threshold-narrate` — the doorband's floor centre (`FloorCenter`, so it re-centres at any width), for the beat taught once and cued again elsewhere |

Gates:

1. **Curtain density holds across box sizes** — the entry's whole reason to
   exist: a narrow and a wide doorway carry proportionally more strands, not a
   thinner spread. Teeth: `single_strand` collapses the curtain to one strand
   regardless of width, which is exactly what "the motif degrading" means, and
   the density check catches it at both sizes.
2. **The doorway is walkable beneath the curtain** — the band sits entirely
   above the walk clearance, so the passage connects end to end with or
   without the degrading knob.

### `broken_grate` — the broken grate (X)

A wall's own vent row, walked the same state-machine `store_room` uses for its
barrel line — `line_before_tell` either lays a plain grate and recurses or
spends its draw and hands the rest to `line_after_tell` — so every derivation
breaks exactly one grate cell, applied to a wall band instead of a floor row.

| | |
|---|---|
| Controls | `head` (3), `grate_height` (2); roles `stone`, `grate_broken` (the plain bars are per-orientation guarded inline states, `grate_bars` — their connections follow the row's frame; a role in the scope's own axes would say the same in one binding, and this piece is the corpus's red→green demonstration of the guard, §7) |
| Smallest region | 3 × (`head` + 2) × `MIN_LINE` (3) — the same "three is the shortest row the odd one always has a neighbour in" proof `store_room` makes |
| Anchors | `anchor/grate-secret` — the broken cell, facing out into the room across the row |

Gates:

1. **Exactly one break, and the anchor is on it** — counted off the blocks'
   `(x, z)` over 12 seeds (a break is `grate_height` courses tall, so it is
   several blocks at one row position, not several breaks), with the rest of
   the row asserted to be plain grates.
2. **The break is in the row** — a plain grate beside it on at least one side.
3. **The break moves with the seed** — 12 seeds put it in ≥ 3 distinct places.
4. **The distress variant is not counted as an accent** — the same
   §4-diagnostic-mirror claim `boulder_stair` makes, over the row's own cells:
   an ungrouped reading of the same short row genuinely clears the accent
   ceiling, the family-grouped reading does not, and restyling the break to an
   unrelated material is still correctly caught.

`boulder_stair`, `threshold_motif` and `broken_grate` are in the generic
library suites too, the same promises as the five above: structural validity,
JSON round trip, palette-swap-moves-no-block over every role, double-expand
determinism over bytes and anchors, and their anchors — including
`boulder_stair`'s generated `pocket-<i>` names and `broken_grate`'s seeded
break position — round-trip through `PrefabRegistry`.

### `drop_shaft` — the one-way spill

A floor that steps down `drop` blocks with no ramp, stair or ladder between the
two levels: a landing zone (low `Z`, floor at `Y=0`) directly abuts an entry
zone (high `Z`, floor at `Y=drop`), and the landing's own interior is built
`drop + head` cells tall — reaching the entry's own ceiling height — so the
open column above it has clear air the whole way down. Stepping off the entry
ledge finds nothing underfoot.

| | |
|---|---|
| Controls | `drop` (4), `head` (3), `rescue_ladder` (0 — a test knob); role `rock` |
| Smallest region | 3 × (`drop + head + 1`) × 4, and at least as long as it is wide |
| Anchors | `anchor/spill` — the entry ledge's brink, facing down-path at the drop. `anchor/landing` — the landing floor cell directly below it, facing further down the exit run |

Gates:

1. **`anchor/spill` reaches `anchor/landing`** under `reachable_with_fall`
   (`tests/staging.rs`) — the ±1-step walk `cliff_path` uses, plus a one-way
   **fall** edge: stepping off a standable cell into an adjacent column with
   nothing underfoot, landing on the first solid floor below however far that
   is. Deliberately more permissive than `crate::nav`'s NPC pathing
   (`reachable_walkable`, ±1 only) — a player can walk off a ledge, an escorted
   NPC never has to.
2. **`anchor/landing` does *not* reach `anchor/spill`**, under the *plain* ±1
   step walk (no fall edge helps a climb). Proving the negative under gate 1's
   own stricter model would be circular; the plain walk is the stronger claim.
   Teeth: `rescue_ladder` notches every column of the entry floor but the one
   `anchor/spill` stands on — paired in the test with a `drop` of 2 (short
   enough that one notch plus one diagonal step actually bridges the gap) —
   and the same plain-walk check must find a way back.

### `dumbwaiter` — the walled duct

The same shell as `drop_shaft`'s two zones, with a third — `duct_zone` — cut
into the middle: a narrow, walled shaft (`duct_core`) framed by solid margins,
built exactly like the landing zone's own column (floor low, open the rest of
the way up). The margins are what make it a *duct* rather than a second cliff:
a body can only enter the shaft at the one column where the floor stops, not
by walking off anywhere along the boundary.

| | |
|---|---|
| Controls | `drop` (4), `head` (3), `duct_len` (2), `duct_width` (1), `rescue_ladder` (0 — a test knob); role `rock` |
| Smallest region | (`duct_width + 4`) × (`drop + head + 1`) × (`duct_len + 4`), and at least as long as it is wide |
| Anchors | `anchor/hatch` — the entry ledge's brink, where the floor stops. `anchor/landing` — the landing floor cell nearest the duct |

Gates: the same two as `drop_shaft`, against the same `reachable_with_fall` /
plain-walk pair. The `rescue_ladder` teeth test notches every column but
`anchor/hatch`'s own — wide enough to be certain of overlapping wherever
`duct_core`'s own runtime-split margins actually landed, since a single fixed
column (what `drop_shaft` uses) is not guaranteed to line up with a core whose
position depends on how the margins split at expansion time.

### `far_side_bar` — the sealed shortcut door

The grammar half of a souls shortcut (spec-0016 §2): `ambush_door`'s own
wall-across-the-box shape, but the one opening is filled with bars instead of
left open — not a narrower door, a **barred** one.

| | |
|---|---|
| Controls | `head` (3), `door_height` (2), `unbarred` (0 — a test knob); roles `rock` and `bar`, the second written in the scope's own axis frame (`{"local": …}`) so its connections span the wall's local `X` whichever way the piece is laid, reflections included |
| Smallest region | 3 × (`head + 2`) × 3, and at least as long as it is wide |
| Anchors | `anchor/gate` — the barred opening's own floor cell. A point, not a region: region anchors (`region` + `block`, the shape a `close-gate` / `shortcut` fill actually needs) are not yet expressible by a rule (§7) — the same limitation `watch_bay`'s `anchor/gate` already accepted. `anchor/unlock` — the far room's floor centre, where a campaign's `shortcut.unlock` binds |

Gates:

1. **The near side cannot reach `anchor/unlock` while the bar stands** — near-
   and far-room standable cells are simply not connected at all (the one
   opening is solid), by the same graph-connectivity technique `cliff_path`
   and `ambush_door` use.
2. Teeth: `unbarred = 1` swaps the fill for air, and the same check must find
   the two rooms connected through exactly that doorway — proof both that the
   wall has no other gap, and that the bar (not some second opening) was what
   sealed it.

### `tee_passage` — the junction

An open-ended chain segment whose two side faces are solid except one
1-wide × `door_height` doorway in the local `X`-min face. The lane still runs
end to end, so the piece drops into a zone's piece run like any other; what it
adds is a box a zone can put something in *beside* the route.

| | |
|---|---|
| Controls | `head` (3), `door_height` (2), `sealed` (0 — a test knob that fills the doorway); role `rock` |
| Smallest region | `MIN_WIDTH` (3) × (`head` + 2) × `MIN_LENGTH` (3), and at least as long as it is wide |
| Anchors | `anchor/branch-door` — the doorway's own floor cell, facing **across** travel at the branch (derived through a `reorient` naming the across-lane axis as local `Z`, the same trick `cliff_path` and `ambush_door` use; it is why the doorway is at `X`-min) |

**Vocabulary, not a new primitive.** The IR already expressed "a chain segment
whose one side face carries a doorway": `ambush_door` and `far_side_bar` are
exactly that wall-with-one-opening construction, merely turned 90° from where a
branch needs it. What was missing was a rule that turns it, and the no-hack rule
cuts *against* widening the IR for something the layer below already says. The
two alternatives that were rejected are recorded in §5c.

Gates:

1. **The lane is still a chain segment** — standable end to end, so a tee in a
   piece run is not the thing that severs it.
2. **Exactly one opening, in exactly one side face** — every cell of both
   side-wall planes is read off the model (120 in the fixture) and exactly
   `door_height` are open, all in the `X`-min face at one `Z`. Teeth: `sealed`
   fills the doorway and the count drops to zero while the lane walks unchanged.
   The doorway's own `split` is `split_exact` for this reason, and the reason is
   measured: under truncation the far end of the wall is never written, and the
   gate reports 7 open cells instead of 2.
3. **The doorway is beside the route, not on it** — delete its column and the
   lane still connects end to end. The teeth here are permanent rather than a
   knob, because the defect is not a mis-set parameter but *building the other
   rule*: the same cut is run against an unbarred `far_side_bar` in the same box
   and must sever it. One construction, one cut, opposite answers.

### `causeway` — the flooded ward

A ward whose flood zones are water from the floor almost to the ceiling — not
a shallow pool with a walkable rim — either side of a solid, 1-wide raised
berm (`rise` blocks tall). A guard station sits at the far end, deliberately
**not flush** with the causeway: its own floor sits `tower_rise` blocks higher,
reached by a two-Z-slice post (`guard_support`, with the pillar and
`anchor/elite`; `guard_cantilever`, the same headroom one cell further out with
**no** pillar under it — the same corbel move `rafter_hall` uses to keep a
perch's own sightline clear of its own truss). A flush post
cannot be obstructed without also sealing the causeway itself, since eye height
over a same-height watch cell and target mass over a same-height target cell
both fall inside the exact two-cell band `standable` requires clear; elevating
it is what opens a sightline geometry that can be tested at all.

**One cross-section, and why that is load-bearing.** Every slice of the piece —
the ward and both `Z`-slices of the post — is one five-way `X` split: wall,
flank, a **1-wide spine**, flank, wall. Only the bodies differ (flood/berm in the
ward, plinth/post column at the station), so the berm and the post stand at one
`X` by construction. They used not to: the post marked the centre of its own
full-width column, `(X-2-1)/2` in from the interior edge, while the berm sits
`ceil((X-3)/2)` in — the same cell only at odd `X`. At every **even** width the
guard stood over the flood, one cell off its own causeway, and blinded itself on
20 of the crossing's 22 cells. Both fixtures in the repo were odd, so gate 2 was
green and bound to a geometry the rule never promised. A rule whose gate is "the
post commands the spine" cannot have the spine's position be a coincidence.

**`berm_gate` — a gatehouse instead of a plug.** Off by default the plinth is
solid and the piece is a **terminus** (seam limit 2, §5c). On, the spine's own
column runs *through* the station at berm height, and the course that roofs the
lane is the post's own floor — so the post is untouched: same floor, same
headroom, same `anchor/elite`, and the lane's whole clearance lies below `rise +
1`, the lowest a sightline from the post ever descends. It needs `tower_rise >=
MIN_GATE_RISE` (3: two cells of clearance and the course over them); a shorter
post is refused, not built with a crawlspace under it. The capability sits on the
**cross-section**, not on either `Z`-slice of the post, because both slices need
it and a knob keyed to the first consumer leaves the second with none.

| | |
|---|---|
| Controls | `rise` (3), `head` (3), `tower_rise` (4), `guard_len` (2), `berm_gate` (0), `obstruct` (0 — a test knob); roles `stone`, `water` |
| Smallest region | 5 × (`rise + tower_rise + head`) × (`guard_len + 3`), and at least as long as it is wide; with `berm_gate` on, `tower_rise >= 3` as well |
| Anchors | `anchor/causeway-head` — the causeway's near end. `anchor/elite` — the guard post's floor |

Gates:

1. **The causeway is standable end to end; stepping off it is not** — every
   flood cell fails `standable` outright (its foot cell is water, not air).
2. **The guard station commands the causeway, at every width the rule
   accepts** — `anchor/elite` sees every standable causeway cell (the same
   Amanatides–Woo walk `watch_bay` uses), swept over widths 5..13, odd and even,
   and again with the lane open: 198 sightlines each way. Teeth: `obstruct = 1`
   stands one solid cell level with the guard's own floor — well above the
   causeway's own two-cell clearance band — and the same check must find at
   least one cell it can no longer see, while the causeway stays walkable end to
   end.
3. **`berm_gate` opens a lane and nothing else** — with it on the piece's
   `Z`-max face reaches its `Z`-min face under `connected`; with it off it does
   not; the guard's own floor stays unreachable from the lane under both
   `connected` and the permissive walk-and-fall model, so the post is still not a
   landing; and gate 2 still holds. The `guard_len` cells of lane *under* the
   post are counted and named rather than folded into gate 2: the guard cannot
   see the floor beneath its own feet, which is what "pass under" means, and
   hiding that inside a green would be the vacuity §5c's binding counts exist to
   stop.

### `elite_ground` — the open arena

One uniformly-floored room, no internal wall anywhere. The "engagement circle"
is not a built piece; it is the square of cells within Chebyshev distance
`radius` of `anchor/elite`, which the margin/approach arithmetic places in the
middle of an otherwise ordinary floor — the geometric form of "no fog-gate
motif": there is no threshold for a fog-gate rule to have occupied.

| | |
|---|---|
| Controls | `radius` (4, its own enforced floor), `flank_margin` (4), `approach` (4), `head` (3), `seal_flank` (0 — a test knob: 1 west, 2 east, 3 both); role `stone` |
| Smallest region | both horizontal extents ≥ `2*radius + 1 + 2*flank_margin + 2` (the larger of the rule's own two checks, since flank margins dwarf the approach runs at the defaults — the same "both horizontal extents" shape `castle` states for the identical reason), `head + 2` tall |
| Anchors | `anchor/elite` — the circle's centre, floor height |

Gates:

1. **The circle is open ground, at least 9×9** — `radius` is guarded at `>= 4`
   (no `otherwise`), and every cell within Chebyshev distance `radius` of
   `anchor/elite` is asserted standable: 81 cells at the default.
2. **Two proven flank lanes** — the west band and the east band (`X` strictly
   outside the circle radius) each independently connect the approach end to
   the exit end by `connected`, counted rather than eyeballed. Teeth:
   `seal_flank` walls off one or both bands across the circle's own length —
   exactly the shape a fog-gate motif would take — and the counted route total
   drops from 2 to 1 or 0.

### `stair_flight` — the way up

A walled shaft with a level landing at each end and a rising run of
single-block treads between them. The vocabulary's only ascending piece, and
the only one gated on being walkable in **both** directions — the exact
negation of the gate `drop_shaft` and `dumbwaiter` owe.

| | |
|---|---|
| Controls | `head` (3), `tread` (2 — cells of run per block of rise), `landing_run` (3), `broken_step` (0 — a test knob); role `rock` |
| Smallest region | `MIN_WIDTH` (3) × (`head` + 1 + `MIN_STEPS`) × (2·`landing_run` + `MIN_STEPS`·`tread`) — 3 × 7 × 12 at the defaults — and at least as long as it is wide |
| Rise | `min(Y − head − 1, (Z − 2·landing_run) / tread)` treads; a box that cannot hold `MIN_STEPS` (3) is a refusal, never a doorstep |
| Anchors | `anchor/stair-foot` / `anchor/stair-head` — the two landings' floor centres. `anchor/stair-step-<i>` — every tread, numbered **against** travel as everything here is, so `stair-step-1` is the topmost |

**A climbing run needs no per-iteration index, and the entry that said it did
was wrong about the IR.** `boulder_stair` records that "a repeated slice cannot
climb a block per iteration — there is no index the IR exposes to a `Size`".
That is true of `split_repeat`, which is a *tiling* — every tile handed the same
pattern — and it is not a fact about the IR. The index a stair needs is **the
box that is left**, and a self-call already carries it: `run` fills one course of
its own floor and hands the remainder, one shorter in `Y` and one tread shorter
in `Z`, to itself; the guard reads those dimensions and the recursion stops when
either runs out. This is `store_room`'s own state-machine trick ("a rule has no
memory, so the invariant is in the derivation's shape") aimed at `Y`. **No IR
change was made or needed.**

**A switchback is one rule under a turned frame.** This rule peels its treads
off local `Z`-max (`[rel, abs]`, recursion first); the same rule under
`mirror: {z}` climbs the other way, and under `Reorient::turned` — the half-turn
about the vertical, a rotation rather than a reflection — it climbs back the way
it came with the wall on the same hand, which is what a return flight is. Two
such lanes side by side in `X`, joined at the top of the first, is a dogleg —
which is what a tall tower over a small footprint needs, since a straight flight
climbs at most about `Z / tread`. Not built; recorded because the construction is
available and the shape is a recurring one. `cliff_path` under
`Reorient::turned` is the worked example, asserted cell by cell in
`tests/staging.rs`.

Gates (`tests/staging.rs`), each with its binding count:

1. **The flight walks up, and back down** — the plain ±1-step `connected` walk,
   66 standable cells in the fixture, 2 landings, 8 treads. `connected`'s edge
   relation is symmetric, so both directions are literally one claim; both are
   written down because the calibration is the point. A second test runs the
   *same* predicate over this rule and over `drop_shaft` and requires them to
   disagree — one model of "can a body get there", two rules, opposite verdicts.
2. **The rise is real** — a walk gate alone is vacuous on a flat corridor, so
   the rise is measured off the anchors and pinned (7 blocks over 8 treads), and
   its control is `boulder_stair` — flat by construction — read in the same box
   by the same code, whose lane spans exactly one height.
3. **Every riser is one block and every tread is ground** — 8 treads, 7
   consecutive pairs, read in index order.
4. **It is a shaft** — both long faces solid, all 616 cells of them. Permanent
   teeth rather than a knob: the same reading over `tee_passage`, which
   deliberately opens one side face, must find its 2 open cells.
5. **The run is the only way between the landings** — the same cut
   `cliff_path` and `ambush_door` use, re-walked under the fall model since a
   stairwell is open above its own run.

Teeth: `broken_step` raises one tread by one extra course — the last one, picked
out of the recursion by a guard on the remaining run, so no index is needed
there either. One riser becomes 2 and the next becomes 0; the shaft still looks
like a stair, still has all 8 treads and all its walls, everything below the
break still walks, and the head landing is stranded. That is how a stair fails
in practice, and a gate that only proved the shaft was not broken in two would
be green on it.

### `lift_shaft` — the counterweight shaft

A walled shaft with a landing doorway and a car station per storey, an open
drop under the lowest one, and a floor at the bottom of it. **It builds no
moving part**, because there is none to build: the lift is a `sequence` of
runtime state, region fill/clear and teleport-by-region authored in campaign
JSON (spec-0031), so the car is *filled* at the floor it is called to and the
one it left is *cleared*. This rule is the hole those effects address.

| | |
|---|---|
| Controls | `lane` (3 — the clear cross-section, floor `MIN_LANE`), `storey` (5 — cells between stations), `sill` (6 — open shaft under the lowest station), `door_height` (2), `sealed` (0 — a test knob); role `rock` |
| Smallest region | `lane + 2` × (`sill` + `storey`) × `lane + 2` — 5 × 11 × 5 at the defaults — and at least as deep as it is wide, since this rule's *length* is its depth, the axis the landing face is on |
| Storeys | `(Y − sill) / storey`, and a box whose storeys do not divide it is a **refusal**: a tiling leaves its remainder unwritten and an unwritten cell is air, i.e. a hole in the shaft's own face |
| Anchors | `anchor/lift-station-<i>` — the car's deck cell, which is also the arrival cell. `anchor/lift-call-<i>` — the solid jamb beside storey `i`'s doorway, at the landing's own level. `anchor/lift-pit` — the standable cell at the bottom of the drop. Stations are numbered **bottom up**, the order a `split` visits its pieces |

**The contract came from the shipped lift, not from a guess.**
`crates/compiler/tests/fixtures/lift` reads every cell it needs off *one anchor
per floor*, four ways: `fill-region {anchor, extent [1,0,1]}` builds the deck,
`clear-region` on the same box takes the old one away, `teleport {to: anchor}`
puts the riders on it, and `give-effect {in: {anchor, extent [1,1,1]}}` gathers
them. A runtime region is a box **centred** on its anchor with unsigned
half-extents, so the car is 3×3 — which is why `lane` has a floor of 3 and the
station sits at the lane's centre. A narrower lane would have the campaign's own
`fill-region` writing the deck through the shaft wall, and nothing downstream
could see it.

spec-0031's acceptance record names two blockers, and this rule answers the
half that is geometry: *"no prefab in the library ships a shaft"* — this is one
— and *"a runtime region cannot name a cell at an offset from an anchor, so a
lift's geometry is authored as an anchor per cell"* — `mark` declares point
anchors, and the two cells the spec calls unaddressable (the deck, and the
shaft-bottom volume) are declared here. The remaining half of that finding is
still open and is not a grammar problem: stage 5 cannot see stage 7's region
language, and a car cannot be commanded from inside itself.

**A shaft is a hole, and the hole is the hazard.** The lane is air from the
shaft floor to the top of the last storey, so a landing whose car is elsewhere
opens onto nothing. That makes this an L-family piece and it owes the family's
pair of claims, asserted with the same `standable` predicate and the same two
walks `drop_shaft` and `stair_flight` are gated on.

**A repeat, not a recursion** — the opposite call from `stair_flight`, and for
the reason `boulder_stair` gives: a `split_repeat` is a *tiling*, every tile
handed the same pattern. A stair's treads each have to know how high the last
one was; a shaft's storeys are identical, so no storey needs an index and
`marked_each` supplies one anyway.

Gates (`tests/staging.rs`), each with its binding count:

1. **Every station can hold the car the campaign fills** — 2 stations × 9
   footprint cells = 18, plus 2 call cells. The 3×3 is air; the station is *not*
   standable (a station with a floor under it is an alcove, and the car has
   nothing to arrive as); the call control is solid, on the landing's level, and
   outside the footprint, or the first ride would bury the lever that started
   it.
2. **One opening a storey, and nothing else** — 4 openings over the shell's four
   side planes, in one column. Teeth: `sealed` fills every doorway and the count
   goes to 0 while the two landings stop being standable (11 → 9).
3. **It drops a body and will not carry one back** — 11 standable cells, 2
   landings, 1 pit. The pit is reachable from a landing under walk-and-fall and
   the landings are not reachable from the pit under the plain step. Control:
   the same rule at `sill = 2`, where the drop is one block and the identical
   check finds the way back — the short-drop pairing `drop_shaft` uses for
   `rescue_ladder`.
4. **The stations climb by one storey each**, numbered bottom up, with the drop
   under the lowest equal to `sill − 1`. Refusals: a `storey` that does not
   divide the rise, and a `lane` under `MIN_LANE`.

`drop_shaft`, `dumbwaiter`, `far_side_bar`, `tee_passage`, `causeway`,
`elite_ground`, `stair_flight` and `lift_shaft` carry the same generic-suite and
registry-round-trip promises as the eight above (`tests/library.rs`,
`tests/determinism.rs`, `crates/compiler/tests/grammar_prefab.rs`).

Three anchor names are shared across rules — `anchor/elite` (`causeway`,
`elite_ground`), `anchor/gate` (`watch_bay`, `far_side_bar`) and
`anchor/landing` (`drop_shaft`, `dumbwaiter`). Composing
any of those pairs into one zone means saying which is which at the include site
(`include_renaming`, §5c); saying nothing is still an `AnchorCollision`.
Nothing enumerates the collisions: the list is prose, and the third entry was
missing from it until a sweep counted the stems.

**`counterweight_lift` is not a grammar rule, and no longer needs to be**
(corrected 2026-08-09; the previous entry is preserved below because its
*premise* is still true and only its conclusion was wrong).

What that entry said: a lift needs a moving platform, a moving platform is not
expressible by this crate's IR at all — `fill` / `split` / `mark` place static
blocks and metadata, with no notion of runtime state or motion, and the `.nbt`
export strips command blocks on principle (§6) — so the vocabulary waits on a
prefab pair or a redstone mechanism this crate could target.

Every clause of that is still true. The conclusion is not, because the motion
moved layers. **A lift is a first-class DSL construct** (spec-0031):
runtime state, region fill / clear, and teleport-by-region, composed into
one `sequence` in campaign JSON — deliberately *not* a `lift` verb. Nothing
moves; the car is filled at the destination, its occupants teleported, and the
old car cleared. So this crate never has to express motion, and a
`counterweight_lift` **rule** would be a rule for a thing that no longer exists
as geometry.

What the lift wants from the grammar instead is a **walled shaft with a station
at each floor** — geometry, and static. spec-0031's own acceptance record names
the two things that block it, and neither is an IR-motion problem:

- *"No prefab in the library ships a shaft."* It does now: `lift_shaft` (above)
  is the same shell without the run, plus the deck / arrival / lethal-bottom
  cells the ride names.
- *A runtime region cannot name a cell at an offset from an anchor*, so a lift's
  geometry has to be authored as an anchor per cell. `mark` declares point
  anchors, which is exactly that shape — and `lift_shaft` supplies them.

**Do not build a `counterweight_lift` rule.** `lift_shaft` is the whole of what
the grammar owes a lift, and Z7 is built on it (§5c).

### `hearth_ward` — the rest point's nook

A chain segment with a two-wide pocket walled on three sides beside the lane,
and one declared focus cell inside it. The mechanism a rest point needs
(`bonfire{anchor}`, spec-0016 §1) stated without the fiction: **somewhere off
the road, approachable from one direction only, with one cell declared as its
focus**. A checkpoint binds there; so would a shrine, a vendor, a save crystal.

| | |
|---|---|
| Controls | `head` (3), `nook_len` (3), `nook_height` (2 — must be under `head`), `mouth_sealed` (0) and `back_door` (0 — test knobs); roles `rock`, `hearth_floor` |
| Smallest region | `MIN_WIDTH` (6) × (`head` + 2) × (`nook_len` + 3), and at least as long as it is wide |
| Anchors | `anchor/hearth` — the floor cell at the centre of the nook's inner half, facing out through the mouth (the back wall is at `Z`-max so the derived facing does that) |

**Not a `watch_bay` with a different anchor on it, and the reason is the
claim.** The shape is deliberately the bay's. What is not shareable is what it
proves: `watch_bay` exists to prove a sightline to a hazard span *it builds
itself*, and a rest ward has no span — composing a bay here would drag in a
hazard the zone does not want and bind that rule's only gate to zero cells,
which is a green that measures nothing. That three rules now build a
pocket-off-a-lane is filed as an open question in §7.

Gates:

1. **The lane walks end to end** (78 cells, 6 at each end), so a rest ward in a
   zone's piece run does not sever it. Red: the refusal a box under `MIN_WIDTH`
   gets, against the same box one cell wider, which builds.
2. **The focus is reachable, and reaching it is a detour** — the lane reaches
   `anchor/hearth`, and deleting all 6 nook cells leaves the lane connected end
   to end. A rest you walk *through* is a corridor with a campfire in it. Teeth:
   `mouth_sealed` (76 cells) — the hearth stands, unreachable, and the lane walks
   unchanged.
3. **Exactly one way in** — the nook's standable neighbours are counted and
   there are exactly `NOOK_WIDTH` (2) of them, both in front of the mouth. Teeth:
   `back_door` opens the outer wall behind it and the count goes to 5.

### `bait_stand` — the lure and its watcher

A chain segment carrying a pedestal on the room floor and a standable perch on a
corbel **directly above it**, with the corbel carried in from the side wall so it
cannot hide what it holds. §4 entry **B**, and specifically the dossier's
*variant 1 only*: variant 3 (the displaced trigger) is banned as resented, and
this geometry cannot express it — the rule's whole gate is that the two are in
one frame.

| | |
|---|---|
| Controls | `head` (5 — must clear the perch by a cell), `perch_rise` (4, at least `MIN_RISE`), `bracket` (1), `canopy` (0 — a test knob); roles `stone`, `timber`, `pedestal` |
| Smallest region | (`bracket` + 4, ≥ `MIN_WIDTH` 5) × (`head` + 2) × 4, and at least as long as it is wide |
| Anchors | `anchor/bait` — the pedestal's own top **block**, the call `store_room`'s `tell` already makes for its barrel. `anchor/bait-perch` — the standable cell over it |

**Why the corbel comes in from the side wall.** `rafter_hall` worked this out
for a whole truss: an eye on the floor is below the beam plane and a perch above
it, so a ray from the approach crosses that plane between the two and a beam
lying in the crossing hides the body it carries. Here the beam occupies only the
perch's own `Z` slice, so a ray down the lane crosses the plane at a `Z` the beam
does not occupy. Fairness is bought by the form, not by a box size.

Both anchors take the derived down-travel facing — the same price `rafter_hall`
pays for its perches, and for the same missing primitive (§7).

Gates:

1. **The watcher stands over the lure** — same column, perch above, perch
   standable, pedestal solid, and open air over the pedestal for the lure to sit
   in. Bound over 3 box shapes, because a motif that lines up at one width is a
   coincidence.
2. **Wherever the lure is visible, so is the watcher** — all 42 approach cells
   see `anchor/bait`, and all 42 see `anchor/bait-perch`. Teeth: `canopy` hangs a
   valance in front of the perch — the lure's 42 does not move and the watcher's
   drops to 0, so the red is an *ambush* defect and not a walled-off room.
3. **The room walks end to end** (99 cells, 7 at each end).

### `disarm_stand` — the hazard's control

The **actuation** dual of `watch_bay`'s observation: a hazard run with a walled
stand at its head, and the mechanism set into the stand's *outer* wall — so every
position it can be worked from lies outside the run it governs. §4 entry **D**
("the boulder release can be jammed from the stair head").

| | |
|---|---|
| Controls | `head` (4), `stand_height` (2 — must be under `head`), `release_in_lane` (0) and `stand_sealed` (0 — test knobs); roles `rock`, `mechanism` |
| Smallest region | `MIN_WIDTH` (6) × (`head` + 2) × (`STAND_ZONE` + 2 = 5), and at least as long as it is wide |
| Anchors | `anchor/release` — the mechanism's own block, in the outer wall a cell over the floor. `anchor/run-head` — the floor cell where what is released starts |

A control cell is a **point**; what a campaign hangs on it — an `EnvTrigger` with
`on: use`, a `timed-gate` disarm — is the campaign's business. This rule declares
no trap: trap and trigger anchors are not yet expressible by a rule (§7), and the
same call `boulder_stair`'s `volley-slot` already makes.

Gates:

1. **The lane walks end to end** (107 cells). Red: the refusal an undersized box
   gets.
2. **The release cannot be worked from the run** — all 103 standable cells that
   are not the stand's own pocket are checked for adjacency to `anchor/release`,
   and none is. The binding is the run's size, so the claim cannot go vacuous on
   a shorter box. Teeth: `release_in_lane` sets the mechanism into the divider
   instead and the count rises to 1.
3. **...and it can be worked at all** — the one operating position is standable
   and reachable from the run. Teeth: `stand_sealed` fills the stand's mouth: the
   position stands, unreachable, and the lane walks unchanged.

## 5c. Zone programs — the vocabulary composed

> **Where `REMAKE` is.** Every `REMAKE §n` citation in this file and in
> `crates/grammar/src/library/bell/` names
> `campaigns/the-drowned-bell/REMAKE.md` in the **content** repo
> (`delvewright-campaigns`), which a dev checkout reaches through the
> `campaigns/` symlink at `campaigns/campaigns/the-drowned-bell/REMAKE.md`.
> §3 is the zone list, §4 the lettered staging catalogue (K, R, O, W, …),
> §7 the engine prerequisites.

`library::bell::{barrow_shore, cliff_road, gate_ward, drowned_ward,
chapel_ward, hall_keep, cistern_deep, bell_tower}` are the drowned-bell remake's
**zone programs** (REMAKE §3;
build-sequence step 3). A zone is one grammar program, and these contain no
encounter geometry of their own: they split the zone's box and `call` §5b's
rules. The only blocks a zone writes itself are `cliff_road`'s crag and the air
beside it, the inert `margin` rock filling the side strip `drowned_ward`,
`chapel_ward`, `cistern_deep`, `gate_ward` and `bell_tower` park a branch in, and
the **plinth** — the mass `gate_ward` and `hall_keep` stand on when a zone leaves
one level down, and the mass `bell_tower`'s upper storey stands on when one
climbs (below). All of it is the mass a zone is carved out of, which no piece
handed a sub-box can know about. Only `barrow_shore` writes nothing at all.

### `compose::include` — how one program calls another's rules

A `call` reaches only rules of its own program, so composition is a copy:
`include(destination, source, prefix)` inserts every rule, parameter and palette
role of `source` under `<prefix>/`, rewriting every reference `source` makes —
`call` symbols (self-calls included: the storeroom's tell is placed by a
recursion), `Expr::Param` reads, `fill` roles. `entry(prefix, source)` is the
name the destination calls it by. Refusals: an empty prefix, a prefix containing
`/`, and any name that would be redefined.

**The prefix never touches an anchor**, because an anchor name is the campaign's
contract (`anchor/watch` is what a `timed-gate` binds). So including one piece
*twice*, or two different pieces that happen to share a stem, makes two
declarations of one name — an `AnchorCollision`, refused loudly and asserted as
such, with the remedy named in the message.

**`compose::include_renaming(destination, source, prefix, renames)`** is that
remedy: an explicit, per-anchor rename given at the include site, mapping a stem
the source declares to the stem the composition should carry. `include` is this
call with an empty map, byte for byte (asserted over three programs × four
seeds), so every zone written before it existed is untouched. Only the stems
named move; an indexed mark is renamed by its stem, so `("niche",
"shore-niche")` turns `anchor/niche-1` into `anchor/shore-niche-1`.

Why explicit and per-anchor rather than a blanket prefix: a ward with a causeway
keeper *and* a dormant ward elite has two genuinely different elites, and the
campaign has to be able to name them apart. Making the zone write the rename puts
the contract where a reader of the zone can see it, and a derived prefix would
silently change every anchor name a `timed-gate` already binds.

Refusals, because a rename that quietly does nothing is worse than no rename:
naming a stem the source never declares (the typo guard — without it a misspelled
entry leaves the collision exactly where it was), a target that is not a
kebab-case stem, and a target the destination or the source's own surviving stems
already carry. A collision between two names **nobody renamed** stays an
expansion-time `AnchorCollision`: this checks only the claims the caller made.

The seam's own promise is pinned from both sides: an included program expanded
over the same box gives byte-identical bytes and identically-named anchors to
the program alone, over three programs × four seeds (`tests/compose.rs`). The
one thing that does change is an anchor's `declared_by`, which becomes the
qualified rule name — a composed prefab's anchors say which piece they came
from.

### The frame constrains composition

Every §5b rule opens with `z(Largest)`, so it turns its length onto the longer
horizontal axis of whatever box it gets. A zone piece **shorter than the zone is
wide is therefore turned sideways**, wall across the route. No composition can
override it — a child reorients itself after the parent's `orient` — so every
zone guards it: a short piece run has no applicable alternative and the
expansion is refused. That is why an 11-wide keep gives its threshold room 12
cells of length where `ambush_door` alone is happy with 5. The primitive that
would remove the constraint is a caller-pinned travel axis, the same shape as
the `local_*` facing spec of §7, one layer out. Not built.

### The eight zones

**Every §4 entry now has a rule, every zone is programmed, and every row
composes the entries it names.** The three entries that had no rule — **B**
(bait gallery), **D** (boulder jam) and the hearth — are `bait_stand`,
`disarm_stand` and `hearth_ward` (§5b); `counterweight_lift` is struck and never
will be one, because the lift is a DSL construct now and what a shaft owes the
grammar is geometry, which `lift_shaft` (§5b) supplies. Z7 was the last
unprogrammed zone and is `bell_tower`. Rows are rewritten in the round that
builds them; nothing here claims a zone was built.

| Zone | Program | Composed from | Missing |
|---|---|---|---|
| Z0 Barrow Shore | `barrow_shore` | `elite_ground` | — (**E** is the whole of Z0) |
| Z1 Cliff Road | `cliff_road` | two `cliff_path`s (the far one `turned`) + the zone's gulf and hairpin head | — |
| Z2 Gatehouse | `gate_ward` | `watch_bay`, `ambush_door`, `disarm_stand`, `boulder_stair`, `tee_passage`, `far_side_bar`, `threshold_motif`, `drop_shaft` + the zone's plinth and branch strip | — |
| Z3 Drowned Lower Ward | `drowned_ward` | `causeway`, `tee_passage`, `elite_ground`, `far_side_bar` + the zone's branch strip | — |
| Z4 Chapel Ward (hub) | `chapel_ward` | `dumbwaiter`, `hearth_ward`, `tee_passage`, `far_side_bar` + the zone's branch strip | — |
| Z5 Great Hall + Keep | `hall_keep` | `rafter_hall`, `ambush_door`, `store_room`, `bait_stand`, `threshold_motif`, `dumbwaiter` + the zone's plinth | — |
| Z6 Cistern Deep | `cistern_deep` | `drop_shaft`, `watch_bay`, `broken_grate`, `elite_ground`, `tee_passage`, `far_side_bar` + the zone's branch strip | — |
| Z7 Bell Tower | `bell_tower` | `stair_flight`, `hearth_ward`, `rafter_hall`, `tee_passage`, `threshold_motif`, `elite_ground`, `lift_shaft` + the zone's plinth and branch strip | — |

**Z7's ascent blocker is closed, and the answer was smaller than the question.**
Every vertical piece the vocabulary had was one-way *down* by construction and
by gate, and `boulder_stair` is flat, so the round that wanted a bell tower had
nowhere to start. `stair_flight` (§5b) is the way up: a walled shaft, a landing
at each end, a run of single-block treads, gated on the plain ±1-step walk in
both directions — the literal negation of `drop_shaft`'s gate, asserted with the
same predicate and cross-checked against it in one test.

It needed **no IR change**. The open question was whether a climbing run is
expressible without a per-iteration index, and it is: the index a stair needs is
the box that is left, and a self-call carries it (§5b). The claim that it was
not is corrected at both the sites that made it.

That the composition works is asserted, not assumed:
`zones::a_zone_can_compose_a_route_a_player_walks_up` chains a `tee_passage`
approach with a flight in the throwaway `chained` fixture and walks the composed
model from the zone's own entry face to `anchor/stair-head`, seven blocks up —
133 standable cells, 3 in the entry face, with the level walk to the foot
landing as the control. **Nothing new was needed at the seam**: a flight's foot
landing sits on the same floor course every flat piece uses, so the two mate the
way any two chain pieces do.

**Z7 is built, and the switchback it was expected to need was not needed.** The
round that wrote `bell_tower` had one open shape recorded for it: a straight
flight climbs at most about `Z / tread`, so a tall tower over a small footprint
wants a switchback (§5b). A box-garden tower is not that tower. It is a box like
every other zone's, so the flight gets a long enough run and the zone writes the
**plinth** its four upper pieces stand on, which is the same licensed mass
`cliff_road`'s crag is. The dogleg stays unbuilt and stays recorded — but it is
no longer *blocked*: `reorient`'s `turned()` (§2) makes the return flight the
same rule, and Z1 is the worked example of exactly that shape.

**The seam between a climbing piece and a flat one, and the one thing that can
go silently wrong there.** Every §5b rule lays its own floor at the bottom of
the box it is handed and stands a body one course up; a flight *arrives on* its
head landing, whose floor is the last course the run laid. So the upper storey's
box has to start one course **below** the level a body stands on, and the plinth
is `climb − 1`. Get that by one and the zone is two rooms with a step between
them — which walks perfectly under `connected`'s ±1 edge and passes a route
gate in silence. Hence Z7's gate 1 asserts three things, not one: the walk, the
`RISE` between the two end faces measured off the model, and `anchor/stair-head`
at the exact height of the upper floor.

**A derivation cannot always be spent where it is needed.** A flight's rise is
`(flight_run − 2·landing_run) / tread` — a fact about the box, and the zone
writes that expression out. But a split's size is evaluated *in the scope it is
written in*, and `dim(Z)` inside the upper storey's own box is the upper run and
not the zone's length: the first draft cut a plinth of −4 courses and the
interpreter refused it. So the zone **declares** `climb` and guards the identity
`climb == treads()` at the one scope where the whole length is visible, along
with `shaft/sill == climb` and `Y ≥ climb + flight/head + 1` (which keeps `Z`,
not `Y`, the thing that bounds the climb). Four drifts are each shown refusing.

**A guard that was written, measured, and deleted.** The lift landing is a
`tee_passage` whose side doorway opens on the shaft's own landing doorway, and
the two are turned 90° to each other. That looked like it wanted a parity guard
on `tee_run`: on an even run a `split_exact` gives the odd block to the earliest
share, so surely the two centres land a cell apart. They do not — both are
placed by the same `Rounding::Start` rule counting from the same end of the same
run, and at `tee_run` 20 and 21 both doorways landed on the same cell with the
landing reachable either way. The guard was **deleted**, because a guard whose
red never happens is a green bound to nothing. What stands in its place is the
measurement: Z7's gate 5 walks from the entry face to the shaft's landing sill,
and the drift it does catch is real — moving `lift_shaft`'s lane off centre
(`rel(1)` → `abs(1)` on its first split) reds it with *"the tower cannot reach
its own lift landing"* while every other gate stays green.

**The plinth, as a zone leaves one level down.** Every vertical piece builds its
entry ledge `drop` blocks up and its landing at the floor, so a zone that puts
one anywhere but its own `Z`-max end has to raise everything above the drop to
meet that ledge. Z6 sidestepped this by being *entered* by falling; Z2 and Z5 are
walked into and left down a shaft, so they cannot. The construction is the branch
strip's sibling, licensed by the same clause: split the shaft's slice off the `Z`
end, and give the remainder a `Y` split whose lower piece is inert `margin` rock.
Two details keep it honest:

- the plinth's thickness is **read from the piece** (`par("shaft/drop")`,
  `par("duct/drop")`), never restated as a zone constant, so dialling the fall
  moves the floor with it — which is the only reason the one-way gate's teeth (a
  short drop plus a rescue ladder) still describe a zone that builds;
- the zone guards that a plinth leaves an upper ward at all (`MIN_UPPER` = 5). A
  piece handed too little refuses for itself, loudly; a *remainder of zero* would
  be written silently, which is the failure mode a guard is owed for.

**The tolerance is measured, not assumed.** Build the plinth one block thin and
every gate stays green — correctly, because a one-block mismatch is a step and a
step walks both ways. At **two** it is a drop the plain walk cannot climb and
five gates go red at once (Z2's route-down, the span cut, the sally port, and the
zone-wide walkability). The tempting stronger claim — "the plinth must equal the
drop or the seam is a wall" — is false in one direction, and stating it would
have made a green look stronger than it is.

**The branch, as a zone builds it.** Z4 and Z6 both hang a `far_side_bar` off a
`tee_passage` (seam limit 3, below), and both do it the same way, which is now
the worked pattern rather than a one-off fixture: cut a strip off the side of the
zone's box, fill it with inert `margin` rock except where the branch goes, and
hand the branch's box to the bar shaped **deeper than the junction is long** so
its own `z(Largest)` aims its wall across the mainline. Two guards, both refusals
with no `otherwise`: `strip_depth > <junction>_run`, and every mainline piece's
run measured against the **mainline's** width (`dim(X) - strip_depth`) rather
than the zone's, because that is the box the piece is actually handed.

Z6's junction is full mainline width, and the reason is measured rather than
argued: hand the junction a 5-wide box and fill the other 14 of its slice, and
the zone's flank-route count drops from 2 to 1 — the solid remainder walls off
the arena's own east band, and "a lane each side of the fight" is a gate that
zone owes. (The same class of one-off measurement `branch_chain`'s margin note
records; a permanent second copy of the zone would drift out of step with the
real one and go vacuous.) The price of the full-width junction is the fixture's
size — a 21-deep strip beside a 19-wide mainline, so the box is 40 across.

**A risk this pattern creates, recorded before it can bind wrong.** The strip's
inert `margin` is roughly two fifths of Z6's blocks. Nothing today measures a
zone's block census, but the §4 craft diagnostics (§7) are a palette *budget* —
60/30/10 by family — and run against a whole zone they would be dominated by
mass a player never sees. When those land, a zone's palette claim has to be
scoped to what the player can reach, the way `boulder_stair`'s mirror is already
scoped to the lane's own floor course, or it will be green for the wrong
reason.

**Z1 is a switchback — CLOSED.** The open question was whether `cliff_path`
survives a reflection at all, since its recesses and lane were thought to be
placed by `reorient` rather than by split order.
`tests/staging.rs::the_recess_reorientation_aims_an_anchor_and_writes_nothing`
strips the only `reorient` inside `cliff_path` and the model is byte-identical:
the ledge, recess and backing are three pieces of an `X` split, and that
`reorient` writes no block — it aims `anchor/niche-<i>`, and stripping it leaves
the recess where it was facing `north` instead of `west`.

What a hairpin needs is narrower than a reflection: its second leg is not a
mirror in one axis but a **half-turn** in two, a rotation (§2). Z1's far leg is
the near leg under `Reorient::turned` — same rule, same parameters, no mirrored
copy of the rule body;
`tests/staging.rs::the_cliff_path_turned_round_is_the_same_path_mirrored`
asserts the turned expansion is the plain one mirrored over all 540 cells and
every anchor. No `cliff_turn` landing rule is needed: the hairpin's head is
`turn_run` cells of solid crag to road level, which is mass and absence and
therefore a zone's own business.

`cliff_path`'s corpse prop is per-frame guarded inline states (§5b), and its
guard set covers the two unreflected horizontal frames **and their half-turns**
— four alternatives, `rotation` 8/4/0/12. A guard set stopping at the
unreflected pair refuses a turned leg outright, which is the reachable-frame set
a rule owes its callers once any caller can turn it.

The fairness this was blocking is now gated. §4 K makes survival depend on a
niche being visible "from the previous switchback", so the two legs sit either
side of one gulf and look into each other's recesses;
`tests/zones.rs::every_far_niche_is_visible_from_the_near_leg` binds 4 recesses
over 37 (viewer, niche) pairs at the pinned seed, with `gulf_screen` as its
teeth — a column down the middle of the gulf that leaves both roads walkable and
both drops lethal and takes every crossing sightline to zero.

### The three seam limits — all three closed

Each is asserted rather than asserted-about: every one has a test in
`tests/zones.rs` that watches it happen.

1. **Two pieces that declare one anchor name — CLOSED.** `include` still never
   renames an anchor on its own, so `causeway` + `elite_ground` (`anchor/elite`)
   and `watch_bay` + `far_side_bar` (`anchor/gate`) still collide loudly when a
   zone says nothing. What a zone can now do is say which is which, with a
   per-anchor rename at the include site (above). That was Z3's **T** + **E** and
   Z6's **F**, refused for want of a name.
2. **`causeway` has no exit past its guard post — CLOSED.** Its far end was the
   post's own plinth, solid from the ward floor to `rise + tower_rise`, with the
   post's floor an island the berm could not reach — deliberately, the same "not
   a landing" move that keeps `rafter_hall`'s perches off the nave. So the piece
   was a *terminus*: its `Z`-min face carried no standable cell at berm height,
   its cantilever slice no floor at all, and no walk (fall edges included)
   crossed it, and no frame helped: turning or reflecting a terminus gives a
   terminus.

   `berm_gate` (§5b) is the exit lane, and **terminus is still the default** —
   a guard post that can simply be walked under is a weaker piece and nobody
   should get one by accident, so the old measurement is kept verbatim as the
   proof the default did not move, with the open case appended as the closure.
   The lane landed on the piece's cross-section rather than on `guard_support`,
   which is what let both `Z`-slices of the post have it from one place, and
   sharing that cross-section closed a second defect nobody had reported (the
   even-width blindness, §5b).
3. **A shortcut is a branch, and the seam is a chain — CLOSED.** Every §5b rule
   walls its own two side faces, so pieces joined end to end along one axis and
   nowhere else; a `far_side_bar` in that chain sealed the zone's route instead
   of sitting beside it (spec-0016 §2). `tee_passage` (§5b) is the junction, and
   it is **vocabulary rather than a new primitive**: the IR already expressed "a
   chain segment whose one side face carries a doorway" — `ambush_door` and
   `far_side_bar` are exactly that construction, turned 90°.

   The zone composes the branch out of machinery that already existed: split off
   a side strip, wall its margins, and hand the interior box to `far_side_bar`
   shaped **deeper than wide**, so the bar's own `z(Largest)` aims its travel at
   the chain. That is the same box-shaping discipline "The frame constrains
   composition" already documents, used deliberately instead of fought.
   `drowned_ward`, `chapel_ward` and `cistern_deep` are the three zones built on
   it (above); the
   throwaway `branch_chain` fixture that first demonstrated it is kept as the
   minimal case.

   One consequence for the "no piece was turned" gate: a branch **is** a turned
   piece, on purpose. So the gate no longer allows a `west` facing by name across
   all zones — it takes a per-zone set of the anchors their zone deliberately
   aimed across the route, pins how many there are (17 of 42), and any other
   `west` is still the accidental turn it was written to catch.

   Two alternatives were rejected, recorded so nobody re-derives them:

   - *The zone carves the doorway itself.* Not actually available: split children
     partition their box and a rule body is one node, so there is no construct
     that writes a cell a sibling already wrote. It secretly requires an overlay
     primitive — and the cell it would overwrite is a piece's own asserted wall,
     precisely **not** "mass no piece can know about", which is the clause that
     licenses `cliff_road`'s gulf.
   - *An aperture control on every rule.* All but one of the §5b rules that
     existed when this was decided (twelve of thirteen) have gates that depend on
     solid side walls — route-uniqueness, blindness — so the control could never
     be legally non-default on them. Dead surface.

### The zone programs

| Program | Fixture region | Controls | Anchors |
|---|---|---|---|
| `barrow_shore` | 19 × 6 × 24 | `arena/*` only; role `arena/stone` | `elite` |
| `cliff_road` | 11 × 13 × 40 | `sea` (3), `fall` (8), `turn_run` (4), `ledge_shelf` (0) and `gulf_screen` (0) — both test knobs — plus `near/*` and `far/*`; role `crag` | `near-niche-<i>` / `near-niche-watch-<i>` on the way in, `far-niche-<i>` / `far-niche-watch-<i>` on the way out |
| `gate_ward` | 20 × 10 × 84 | `shaft_run` (12), `motif_run` (10), `tee_run` (10), `stair_run` (16), `stand_run` (10), `door_run` (10) — the gated passage takes the rest — `strip_depth` (11), plus `gate/*`, `door/*`, `stand/*`, `stair/*`, `tee/*`, `sally/*`, `motif/*`, `shaft/*`; role `margin`. The upper ward stands on a plinth `shaft/drop` thick | `watch`, `gate`, `threshold`, `alcove`, `release`, `run-head`, `stair-run`, `volley-slot`, `pocket-<i>`, `branch-door`, `sally-gate`, `unlock`, `threshold-narrate`, `spill`, `landing` |
| `drowned_ward` | 40 × 10 × 60 | `ward_run` (20), `junction_run` (20) — the crossing takes the rest — `strip_depth` (21), plus `ward/*`, `ring/*`, `junction/*`, `shortcut/*`; role `margin`. The zone pins `ward/berm_gate = 1` and `ward/rise = 2`: the post has to be passable at all, and the berm has to meet its neighbours' floor within the one-block step `connected` allows, or the seam is one-way | `causeway-head`, `keeper-elite`, `branch-door`, `gate`, `unlock`, `elite` |
| `chapel_ward` | 16 × 9 × 26 | `strip_depth` (9), `junction_run` (8), `hearth_run` (8) — the chute takes the rest — plus `chute/*`, `hearth/*`, `junction/*`, `shortcut/*`; role `margin` | `hatch`, `landing`, `hearth`, `branch-door`, `gate`, `unlock` |
| `hall_keep` | 11 × 11 × 76 | `duct_run` (12), `motif_run` (12), `gallery_run` (12), `store_run` (12), `door_run` (12) — the hall takes the rest — plus `hall/*`, `door/*`, `stores/*`, `gallery/*`, `motif/*`, `duct/*`; role `margin`. The keep stands on a plinth `duct/drop` thick | `hall-door`, `perch-<i>`, `threshold`, `alcove`, `store-line`, `tell`, `bait`, `bait-perch`, `threshold-narrate`, `hatch`, `landing` |
| `cistern_deep` | 40 × 10 × 100 | `arena_run` (20), `sally_run` (20), `vent_run` (20), `gallery_run` (20) — the shaft takes the rest — `strip_depth` (21), plus `arena/*`, `tee/*`, `sally/*`, `vent/*`, `gallery/*`, `shaft/*`; role `margin` | `spill`, `landing`, `watch`, `gate`, `grate-secret`, `branch-door`, `sally-gate`, `unlock`, `elite` |
| `bell_tower` | 41 × 14 × 125 | `ring_run` (20), `door_run` (20), `tee_run` (21), `loft_run` (20), `hearth_run` (20 — BF5's rope room) — the flight takes the rest — `strip_depth` (22), `climb` (9 — guarded against the flight's own rise, never dialled alone), plus `ring/*`, `door/*`, `tee/*`, `loft/*`, `hearth/*`, `flight/*`, `shaft/*`; roles `plinth`, `margin`. The zone pins `shaft/sill = climb`: the shaft's lowest station has to be the upper storey's own floor, or the landing doorway opens into the plinth | `stair-foot`, `stair-head`, `stair-step-<i>` (9), `hearth`, `hall-door`, `perch-<i>` (5), `branch-door`, `threshold-narrate`, `elite`, `lift-station-1`, `lift-call-1`, `lift-pit` |

**Z3 does not claim a zone-length bypass, and says why.** Z0 and Z6 re-bind
`elite_ground`'s "a lane each side of the fight" across their whole zone. In Z3
that would be false by construction: the causeway is a **one-wide** crossing, so
no band of floor runs the length of the zone at all. The claim is bound where it
is true — across the arena's own run — and the alternative, quietly re-scoping a
zone-length gate until it passed, is exactly the vacuity these binding counts
exist to catch.

**Z0 is one piece, and its gate says so.** With no seam there is nothing for a
composition gate to catch: `barrow_shore`'s flank gate re-binds `elite_ground`'s
own claim to the campaign's box rather than the piece fixture's, and its frame
guard — which for a one-piece zone collapses to `Z > X`, after the zone's own
`z(Largest)` has already normalised the box — can only refuse a *square* one. The
program earns its place because a zone is one program and a campaign binds
`barrow_shore`, not because it proves something new.

Gates (`tests/zones.rs`), each with its **binding count** and the red it has been
watched producing. A gate is about what *composition* did or failed to preserve;
the piece-level claims stay in §5b.

| Gate | Binding | Red demonstrated by |
|---|---|---|
| Every zone is a route end to end | 438 / 40 / 655 / 1100 / 165 / 677 / 2078 / 2172 standable cells | — (the fixture pins the counts; a sealed seam is a red here) |
| Z0 a lane each side of the fight | 2 routes, bands of 111 cells | `arena/seal_flank` = 1 / 2 / 3 → 1 / 1 / 0, shore still walkable |
| Z0 a square box is refused | 1 refusal | — (the refusal is the gate; one cell longer builds) |
| Z1 the ledge is the only route | 36 ledge cells of 40 | deleting the lane severs the zone |
| Z1 the gulf is beside every ledge cell | 36 cells × 3 seeds = 108 columns | `ledge_shelf = 1` → all 36 shallow, road still walkable |
| Z1 every niche opens onto that ledge | 4 niches, 4 watch cells | — (measured against the model, not the params) |
| Z2 the hazard cannot be walked round | 21 span cells, 634 cells re-walked without them **under the fall model** | deleting the span severs the zone |
| Z2 the bay sees the whole span *composed* | 21 span cells | `gate/obstruct = 1` → 6 blind, passage still walkable |
| Z2 the alcove is blind from the whole zone | 135 approach cells (54 in the piece's own fixture) | `door/expose = 1` → 121 see it |
| Z2 a route down, and none back up | 4 entry / 7 exit cells, 655 standable | `shaft/rescue_ladder = 1` (+ `shaft/drop = 2`) → the ward climbs back; two controls |
| Z2 the release is out of reach of the composed run | 603 run cells examined, 0 in reach; 1 operating position, reachable | `stand/release_in_lane = 1` → 1 in-run position |
| Z2 the sally port is sealed, and reached through one doorway | 655 standable, 40 the branch's near room, 1 doorway column cut | `sally/unbarred = 1` (663) opens it; `tee/sealed = 1` (654) makes the branch unreachable, ward still a route down |
| Z5 the doorway is the only route | 205 approach / 471 inside cells, cut re-walked under the fall model too | plugging the door column severs the keep |
| Z5 every perch visible from the hall door | 4 perches | `hall/span_beams = 1` → 3 blind, keep still walkable |
| Z5 the alcove is blind from the whole hall | 205 cells, 4 of them rafters | `door/expose = 1` → 155 see it |
| Z5 exactly one tell | 8 seeds, 6 distinct positions | — (the recursion is the invariant; a broken include reds it) |
| Z5 a route down, and none back up | 9 entry / 9 exit cells, 677 standable | `duct/rescue_ladder = 1` (+ `duct/drop = 2`) → the keep climbs back; two controls |
| Z5 the lure's watcher is legible from the composed gallery | 144 gallery cells, all 144 seeing both | `gallery/canopy = 1` → the lure's 144 holds, the watcher's drops to 0 |
| Z3 a route **both ways**, and only through the gatehouse | 1100 standable, 1 entry cell (the berm is one wide), 19 exit | `ward/berm_gate = 0` restores the plinth → severed under walk *and* walk-and-fall, while the 18-cell crossing still walks |
| Z3 the keeper commands the crossing *composed* | 18 crossing cells, 18 sightlines | `ward/obstruct = 1` → blind cells, ward still walkable |
| Z3 a lane each side of the fight, bound over the **arena's own run** | 2 routes at the default | `ring/seal_flank` = 1 / 2 / 3 → 1 / 1 / 0 |
| Z3 the shortcut is sealed, and reached through one doorway | 1100 standable, 180 the branch's near room, 1 doorway column cut | `shortcut/unbarred = 1` opens it; `junction/sealed = 1` makes the branch unreachable, ward still walks |
| Z4 a route down, and none back up | 5 ledge / 5 exit cells, 165 standable | `chute/rescue_ladder = 1` (+ `chute/drop = 2`) → the hub climbs back; two controls |
| Z4 the shortcut is sealed, and reached through one doorway | 165 standable, 24 the branch's near room, 1 doorway column cut | `shortcut/unbarred = 1` (171 standable) opens it; `junction/sealed = 1` (164) makes the branch unreachable, hub still walks |
| Z4 the hearth is reachable, and off the route | 165 standable, 6 of them the nook, re-walked without them | `hearth/mouth_sealed = 1` (163) → the hearth stands unreachable, hub still crosses |
| Z4 a branch no deeper than its junction is refused | 2 refusals | — (the refusal is the gate; the same box builds at the defaults) |
| Z6 a route down, and none back up | 17 ledge / 19 floor cells, 2078 standable | `shaft/rescue_ladder = 1` (+ `shaft/drop = 2`) → the deep climbs back; two controls |
| Z6 the span cannot be walked round *or fallen past* | 51 span cells, re-walked under the fall model | deleting the span severs the zone |
| Z6 a lane each side of the fight | 2 routes, bands of 767 / 411 cells (the west band is the larger only because the branch's own rooms fall inside it; they span the sally run alone and carry no route) | `arena/seal_flank` = 1 / 2 / 3 → 1 / 1 / 0, cistern still a route down |
| Z6 the sally port is sealed, and reached through one doorway | 2078 standable, 180 the branch's near room, 1 doorway column cut | `sally/unbarred = 1` (2096) opens it; `tee/sealed = 1` (2077) makes the branch unreachable, cistern still a route down |
| Z7 a route, **and the route climbs** | 2172 standable, 17 entry / 19 exit cells, 8 blocks of rise measured off the model, `stair-head` level with the upper floor | `flight/broken_step = 1` strands the head landing and severs the zone while every piece still stands and the entry still reaches the first tread; the flat control is `boulder_stair` in the same box, which reads 0 |
| Z7 every loft perch visible from the loft door | 5 perches | `loft/span_beams = 1` → 4 blind, tower still walkable |
| Z7 no route to the Bellkeeper skips the threshold (**M**) | 17 doorband cells cut, 532 ring-side / 1623 tower-side | cutting the doorband severs the ring; the loft stays reachable, so the cut is the motif and not the zone |
| Z7 a lane each side of the fight, bound over the **ring's own run** | 2 routes at the default | `ring/seal_flank` = 1 / 2 / 3 → 1 / 1 / 0 |
| Z7 the counterweight shaft is entered once and only drops (**L**) | 2172 standable, 1 landing sill in the strip, 1 pit | `tee/sealed = 1` (2171) makes the shaft unreachable while the tower still walks; the pit is reached under walk-and-fall and reaches neither the entry nor its own landing under the plain step |
| Z7 the plinth arithmetic is guarded, not hoped | 4 drifts, each refused | `shaft/sill + 1`, `flight/tread = 1`, `flight/landing_run = 4`, `ring_run = 22` |
| No piece was turned | 83 anchors, travel order + facing; exactly 26 of them across, pinned | `door_run = 7` and `gallery_run = 7` are refused, and so is Z7's `door_run = 19` with `ring_run = 21` (which moves a cell between two runs, so the climb guard cannot be what fired); the same boxes turn `ambush_door` / `watch_bay` / `threshold_motif` alone (`west`) |
| Seam limit 1 closed: a rename lets two pieces declare one stem | 2 pairs, 4 anchors read back by name and declaring rule | dropping the rewrite in `compose::node` → the same `AnchorCollision` |
| Seam limit 1: no rename, still a collision | 2 pairs | — (the collision *is* the gate, named by rule both times) |
| Seam limit 2 closed: `causeway` is a terminus until `berm_gate` opens | 2 `Z`-slices, a 22-cell berm | — (with the gate shut no walk crosses it while the berm still crosses the ward; with it open the faces connect) |
| Seam limit 3: a barred door **on** the route seals the chain | the chain's standable cells, walked twice | `bar/unbarred = 1` reopens exactly that doorway |
| Seam limit 3 closed: the same bar **beside** the route | 43 standable (25 mainline / 18 branch), 9 near-room, 1 doorway column cut | filling the tee's lane → the branch severs the mainline; a 3-wide doorway → the doorway cut no longer isolates the branch |
| ...and the branch's teeth | 42 standable, 9 near-room | `tee/sealed = 1` → the branch is unreachable, mainline still walks |
| A composed route the player walks **up** | 133 standable, 3 in the entry face, 7 blocks of rise | the level walk to the flight's own foot landing is the control; a flat chain reads 0 |

**Which zones fall.** Four of the eight are crossed with the ±1 step
`connected` uses, and **Z7 is one of them on purpose**: its mainline climbs, and a
zone that climbs owes the stronger walk rather than the more permissive one. The
other four have a drop on the route — Z4 and Z6 are *entered* by stepping off a
ledge, Z2 and Z5 are *left* down one — so they are crossed under
`reachable_with_fall` (`connected`'s walk plus a one-way fall), and each of the
four also owes the negative: the extra freedom still does not carry a player back
up (asserted under the plain step, since proving a negative under the generous
model would be circular). The fall model is also the *adversary*: a fall edge only
ever adds routes, so Z2's and Z6's "the span is on the route" cuts and Z5's
doorway cut are re-walked under it — and Z7 uses it in the third way, as the only
model under which its own lift shaft is enterable at all.
`tests/support/mod.rs` carries it, with one
deliberate divergence from `tests/staging.rs`'s piece-scale copy — a landing must
be a member of the cell set under consideration, or a fall would walk straight
through a gate's own cut.

Zone programs are **not** in the export suite. Size is no longer why — every
zone exports, tiling if it must (§6) — so the statement is the narrow one:
**no zone has been put in the export suite, and doing so is a decision nobody
has taken.** Taking it would mean a zone's anchors round-tripping through
`PrefabRegistry` and carrying a spec-0027 §2 provenance row like a rule's,
which is a capability question and not a size one. Meanwhile
their structural validity, JSON round trip, determinism and palette-swap
promises are asserted in `tests/zones.rs`.

## 6. Export — freezing an expansion as a prefab

`export::export_zone(program, region, options, id)` is the export. It takes the
*program*, not a finished model, and expands it itself — which is what makes the
provenance row unforgeable, since the hash and seed in the metadata cannot
describe a different expansion than the one that produced the bytes.

It writes one of two shapes, decided from the region and from nothing an author
says:

- a region within 48 on every axis → `<id>.nbt` (a vanilla structure template)
  and `<id>.json` beside it, the two files a prefab library holds;
- a region past it → a set of `≤48` tiles, `<id>.x<i>y<j>z<k>.nbt`, plus one
  manifest at `<id>.json`.

`export::export_prefab` is the single-template writer the first shape is made
of, and it still refuses an oversize region. Nothing outside the module calls
it: a region an author chose is never the wrong size.

The `.nbt` comes from `delvewright-schem`'s `build_region`, the emitter the
`.schem` asset pipeline already uses: one structure writer, one set of
determinism guarantees (sorted palette, `x`→`y`→`z` cell order, gzip mtime 0).
A structure template is local-coordinate, so the region's **origin** does not
reach the output; its **size** does, and is the declared `structure.size`.

The metadata is the hand-built shape, minus what expansion cannot know. Its
shape is defined once, in `delvewright_schem::prefab` — the crate that also
writes the `.nbt` half — and every tool that produces or edits a prefab reads and
writes it through that one type, so an admission step cannot drop the parts it
does not itself model:

```json
{
  "prefab_id": "prefab/grammar-temple",
  "structure": { "file": "grammar-temple.nbt", "id": "grammar-temple",
                 "size": [13, 14, 21], "data_version": 4671,
                 "generator": "crates/grammar" },
  "anchors": {},
  "connectors": [],
  "lighting": { "profile": "unmeasured" },
  "license": { "source": "original", "spdx": "GPL-3.0-or-later",
               "note": "…", "provenance": "…",
               "generated_by": { "generator": "grammar", "program": "temple",
                                 "program_hash": "sha256:…", "seed": 7 } }
}
```

- **`generated_by`** is the spec-0027 §2 provenance row. `program_hash` is
  `sha256` over the program's canonical serde JSON bytes — content-addressed, so
  a program built in Rust and the same program parsed from JSON hash alike.
- **`anchors`** is exactly what the program's `mark` declarations produced (§2b),
  in the hand-built `{pos, facing}` shape with `pos` local to the structure.
  Nothing infers one from the block pattern afterwards — that is precisely the
  downstream folklore the no-hack rule forbids — so a program that marks nothing
  exports `{}`, as the temple above does, and an anchorless prefab loads and
  indexes normally. The castle, which marks, exports
  `"anchors": { "anchor/courtyard": { "pos": [20, 0, 12], "facing": "north" } }`
  over its 41×14×25 region.
- **`spatial_contract`** is the program's contract (§2d) **as resolved for this
  expansion**: every box is a local cell range of these exact bytes, in the
  `{from, to}` shape a gate anchor already uses. Resolved rather than parametric,
  because a program re-expanded at other parameters means other boxes, and a
  contract carrying the boxes it was authored against would describe one
  expansion and quietly mis-describe every other. The key is absent — not empty
  — for a program that declares no contract: "this piece makes no spatial claim"
  and "this metadata predates contracts" are the same claim here, and neither is
  a contract with nothing in it. A tiled zone carries the same block on its
  manifest, in zone coordinates, because a tile boundary is not part of the
  building.
- **`spatial_contract.faces` is the piece's face contract**: every `exterior`
  edge, as the side of the piece it is on (`east`/`west`/`up`/`down`/`south`/
  `north`), the edge's class, and the opening it leaves there. Derived from the
  edges and the blocks at export and written down, so that **assembly asks the
  metadata rather than reopening the `.nbt`** — which is what `delvec` does with
  it: two placed pieces whose declared faces do not answer each other are
  refused (`DW0780`), naming both areas, both prefabs and both faces. That
  refusal is the one no single-piece gate can reach, because each piece is
  correct and the pair is not.
- **Every anchor carries `resolves_to`**: which element of the contract it lands
  in (`space:<name>`, `no_body:<name>`, `via:<name>`, `bar:<name>`). A campaign
  binds content to an anchor by name, and what says whether that place is play
  space, a door or exterior dressing is the contract. Absent on a piece that
  declares none.
- **`connectors` is empty.** Jigsaw socketing of grammar prefabs waits on the
  tileset conventions; a guessed socket is worse than none. The key is present
  and empty rather than absent, because "this piece has no sockets" and "this
  metadata was written before sockets existed" are different claims, and
  `delve-admit socket` appends to it.
- **`"profile": "unmeasured"`.** A lighting profile is a *measurement*, taken by
  the live 1.21.11 probe. Expansion places blocks, not photons, so it declares
  the true thing and admission to a campaign still runs the probe. `unmeasured`
  is not a synonym for an absent `lighting` block: absence means legacy metadata
  predating the field, this is a positive statement that a measurement is owed.
  A `lit`/`dim`/`dark` declaration still cannot omit `measured_min_light` /
  `measured`, and an `unmeasured` one may not carry them (`delvewright-dsl`
  refuses both at parse).

### The tiled shape

A zone past the cap carries `structure_set` where a single prefab carries
`structure`. Everything else is the same file: same `prefab_id`, same
zone-relative `anchors`, same empty `connectors`, same `lighting`, same
`license` — the provenance row regenerates the whole set at once, because one
expansion produced all of it.

```json
{
  "prefab_id": "prefab/z2-gate-ward",
  "structure_set": {
    "base": "z2-gate-ward", "size": [20, 10, 84], "part_max": 48,
    "grid": [1, 1, 2], "data_version": 4671, "generator": "crates/grammar",
    "parts": [
      { "file": "z2-gate-ward.x0y0z0.nbt", "id": "z2-gate-ward.x0y0z0",
        "grid_index": [0, 0, 0], "offset": [0, 0, 0],  "size": [20, 10, 48] },
      { "file": "z2-gate-ward.x0y0z1.nbt", "id": "z2-gate-ward.x0y0z1",
        "grid_index": [0, 0, 1], "offset": [0, 0, 48], "size": [20, 10, 36] }
    ]
  },
  "anchors": { … }, "connectors": [], "lighting": { … }, "license": { … }
}
```

- The key is a **different name**, never `structure` with an extra field. Every
  existing consumer requires `structure`, so a tool that has not learned about
  tile sets fails to parse this file rather than reading it as a prefab with no
  blocks in it.
- `offset` is **zone-relative**: add it to a tile-local cell to get the zone
  cell. That is the only transform reassembly needs.
- The cuts come from `delvewright_schem::split::plan_split`, the same function
  that tiles an oversize `.schem` import — one tiling, so one reassembly rule
  reads both. They are a pure function of the region and the cap: no RNG, no
  clock, no dependence on the program, the seed or the blocks, so the tiles and
  the manifest are byte-identical across runs (`tests/export.rs`).
- **A tile is packaging and never a unit of judgement.** The gates judge the
  whole expansion, the block-legality check runs over the whole model, and both
  the anchors and every diagnostic position are in zone coordinates. Binding
  counts stay zone-level.
- `TileSet` (`delvewright_schem::split`) is the contract, `Serialize` for the
  writer and `Deserialize` for the readers — one struct, so the halves cannot
  drift. `TileSet::validate` refuses a manifest whose parts do not tile the zone
  exactly, so a truncated one is a refusal and not a building with a hole.

The rest of the loop takes the manifest and treats the zone as one thing:
`delve-render piece <id>.json` reassembles and renders one scene, and
`delve-admit audit <id>.json` audits every tile's bytes for one zone verdict
(with a per-tile listing). Both **refuse** a lone tile of a set and name the
manifest to use instead — a render of a fragment is a review that passes and
means nothing, and a verdict over one tile reads as a verdict over the zone.

Not built: compiler-side placement of a tile group in world assembly, and
jigsaw connector emission. Both are queued.

Refusals, all loud: an `id` that is not a lowercase-kebab path segment, an empty
region, and a model containing a block the structure safety strip would replace
with air — a grammar that asked for a command block meant to, so shipping a
silent hole is refused instead. **Size is not among them.**

The first two are properties of the inputs alone, and are refused before the
expansion runs: `export::is_valid_id` is public so the CLI can ask before it
expands anything, rather than after it has printed a verdict. The third is
knowable only from the expanded model, so it is refused after the gates have
passed — and the verdict is printed only once the prefab is on disk, so no
`pass` line ever sits above a refusal.

`PrefabRegistry` (the engine's reader) loads the result with no diagnostics;
`crates/compiler/tests/grammar_prefab.rs` tests that seam from both sides.

## 7. Not built yet

The §4 craft diagnostics, jigsaw connector emission, and the JSON schema stage in
front of the IR. Later phases of spec-0027.

The §4 palette budget's **measurement** does exist, out of the compiler:
`python3 tools/block-appearance.py --program <p.json>` reads every `palette` role
and every inline `fill` material of a program and reports each mix's
`chroma_mass`, `chromatic_area`, **named** `loudest_member` with its area share,
and `dominant_hue`, stating its binding count. What is missing is the *binding* —
the compiler cannot run it, because the numbers are measured from the EULA-gated
client jar and whether the derived table may be committed is spec-0035 §7's open
question for the owner. Until that is answered the measurement is an authoring
aid an author runs, not a gate, and it must not be described as one. It is also
whole-zone today: scoping it to player-reachable mass is the risk §5 records.

The **contact sheet** is built: `delve-render contact-sheet` lays a directory of
candidate renders out as one page, optionally ordered by a similarity score
against a reference image (`tools/refscore.py`, spec-0028 §3 — the score RANKS
the page and never gates it). What is still missing between the expander and
that page is the automatic part: nothing yet drives "expand N seed-varied
candidates → `batch`-render them → sheet", so the sweep is assembled by hand
today.

`mark` declares point anchors only. Trap anchors (`dispenser`,
`trigger_block`) and the entry names the engine treats specially (`spawn`,
`entry`) are expressible in prefab metadata but not yet by a rule — each needs
its own declaration, not a widened `mark`. A rule can name a *region* (§2d), and
a `barred` edge's bar region is the cells a campaign's `shortcut` / `close-gate`
/ `lift` addresses; what is missing is the export half, since a claimed region
reaches the metadata's `spatial_contract` block and not its `anchors` map.

**Three pieces still spell out per-frame variants they no longer need to.**
An orientation-dependent block is a palette role as of the local axis frame
(§2), and `far_side_bar`'s bar is one. `broken_grate`'s bars, `cliff_path`'s
corpse yaw and `church`'s doors are still written as one guarded alternative
per frame, which is the longer way to say the same thing — and `church`
pays twice over, binding `door_lower`/`alt_door_lower` and
`door_upper`/`alt_door_upper` where one framed role each would do.
**The cost is not fixed: it is one alternative per frame a caller can hand the
rule.** `cliff_path` carries four, because a hairpin leg turns it and the
half-turn of each horizontal permutation is reachable; a local-frame role would
carry one and cover every frame including the ones nothing has asked for yet. Converting
them is mechanical and byte-neutral (the frame resolves to exactly the states
the guards select), and the corpus is what an author copies from, so the
variants that remain teach a workaround for a solved problem. What holds the
first two back is that the `orientation` guard is a real construct with no
other demonstration in the corpus: `coverage` counts `cond:orientation` over
the library alone, and converting every site would take it to zero bindings —
a live surface nothing shows. The general form wanted is a program that
demonstrates the guard for what only the guard can do (a rule body that
differs by orientation, not merely a state), after which the three convert.

**A socket convention — which faces a piece leaves open.** The junction itself is
built (`tee_passage`, §5b), and `far_side_bar` beside a `tee_passage` is the
first worked example of a piece opening a face onto a sibling box. What is still
**convention rather than contract** is the promise those two pieces are keeping
to each other. An `exterior` edge (§2d) states that a piece opens onto the
outside, and its `via` names the cells; what neither states is *which face* — a
mating pair still has nothing to check itself against, so a zone that mates two
pieces is trusting module prose, and a rule that changed which face it opened
would break its callers silently.

Today the check is per-zone and after the fact: a gate walks the composed model
and asserts the branch is reachable exactly through the doorway. That catches the
break, but only in a zone that has such a gate, and only after the geometry is
built. A face-contract on the rule — declared, and checked at compose time
against what the neighbour declares — is the general form, and it is the same
family of problem as jigsaw connector emission above, one layer up.

`tee_passage` makes the gap **smaller in kind and larger in surface**: smaller
because the rule declares `anchor/branch-door` at the opening, so a zone reads
where the face is from the expansion rather than from prose; larger because there
is now a mating pair to get wrong, where before every rule was sealed and the
convention had nothing to bind.

**A pocket off a lane has no owner.** Three §5b rules now build the same shape —
a lane with a pocket walled on three sides beside it: `watch_bay`'s bay,
`hearth_ward`'s nook and `disarm_stand`'s stand. Each proves a *different* claim
about it (a sightline to a span, shelter and detour, actuation from outside a
run), which is why each is its own rule rather than a knob on one; but the
construction itself is written three times, and a fourth consumer would write it
a fourth. The general form is a shared pocket construction the three configure —
not attempted here, for one concrete reason rather than for taste: `watch_bay` is
shipped and composed into two zones, and re-expressing its bay would move the
bytes of zones the byte-identity gate pins. It is named here so the *second*
site is on the record, since generality is decided at the first.

*Checked at Z7, and the count is still three.* `lift_shaft` (§5b) was the
obvious candidate for a fourth and **is not one**. The shape is a *travel lane
with a dead-end recess beside it*; a shaft has neither half. Its lane is
vertical and is a hole rather than a floor — no body walks along it — what sits
beside that lane is solid mass on three faces, and its one opening is a
**through-passage**: the landing sill joins the shaft to the room outside it,
where a pocket is closed on three sides and joins nothing. So the site count is
unchanged by this round, and the threshold at which unification becomes correct
has not moved.

*What Z7 did move is the price.* The reason recorded above for not unifying is
that one of the three (`watch_bay`) is composed into two zones, so re-expressing
it would move pinned bytes. Z7 composes `hearth_ward` for BF5, which puts a
**second** of the three into two zones (Z4 and Z7). Unification now has to hold
four zones' bytes still rather than two. That is a cost that only grows, and it
grows every time a zone round composes one of these three — which is worth
knowing before the fourth site arrives, not after.

**A facing a rule cannot ask for.** A derived facing is the direction of
decreasing local `Z`, and an explicit `facing` is a *world* cardinal, so a rule
that is reused under rotation cannot say "look the way my local `+X` points".
Since a split also always visits its pieces from the low end of the axis it cuts,
"anchors numbered in travel order **and** facing along travel" is not
expressible — §5b pays for the facings with the numbering. A reflection does not
buy it back: `mirror` reverses the visiting order and the derived facing
together, so the two stay locked to each other. The
smallest primitive that would remove the trade-off is a **local-direction facing
spec** on `Mark::facing` (`local_x_min` / `local_x_max` / `local_z_min` /
`local_z_max`, resolved through the scope's orientation at expansion, exactly as
`AxisSpec` already resolves an axis name). It is additive, needs no new node
kind, and would let a rule aim an anchor in any of the four cardinals whichever
way the piece was turned.

**W2 met the same wall, and paid differently.** `rafter_hall`'s perches
alternate between the two side walls; both should look *across* at the nave, and
that is a facing along local `+X` for the left corbel and `-X` for the right.
Only the second is expressible, so both take the derived down-hall facing
instead — an occupant watching the ground the player is walking into, which is a
defensible reading of the move but not the one the geometry asked for. The
workarounds available inside the current IR are all worse than the shortfall:
put perches on one wall only (monoculture, the exact thing the entry's density
cap exists to prevent), or declare a *world* cardinal (which breaks the moment
the frame's `Largest` turns the piece 90°). `store_room` dodged it by choosing
which wall the barrel row sits against, which works for one row and does not
generalise to a rule with two symmetric sides.

That is now two worked examples with the same shape and one rule that only
avoided it by luck of layout. Still not built here — the red line for W2 was
compose-existing-verbs-only — but the case is no longer thin.

## 8. Demonstration coverage — what the corpus proves is reachable

`prefab-procedure.md` §3 sends an author to the corpus, never to the schema. Under
that instruction the corpus **is** the language: a construct no example writes
does not exist in practice, whatever §2 says the IR supports.

```sh
delve-grammar coverage            # the table; exit 4 when anything is at zero
delve-grammar coverage --json coverage.json
```

It counts, over every program `delve-grammar list` names, how many times each
`Node` kind, each `Cond` kind, each thing a frame request asks for
(`frame:rename`, `frame:mirror`) and each palette paint kind is written, and
prints each with its **binding count** and the programs that demonstrate it — the
same shape the expansion gates use, and for the same reason: a number beside the
word `pass` that examined nothing is worse than no number.

**What it measures, and the thing it must never be read as.** It measures
demonstration, not expressiveness. A pass means no part of the IR is left
undemonstrated by the corpus an author is sent to. It is **not** evidence that an
author can build any particular thing, and no document, PR or review may cite it
as such. The command prints that sentence on every run, pass or fail, and carries
it in the JSON, because a number travels further than the page that qualifies it.

**How to read a zero.** A zero is not "the language cannot". It is "an author
following the procedure would never find this", which is the more actionable of
the two and the only one the corpus can answer. Close it by writing the smallest
program that teaches the construct and putting it in the library — not by
removing the construct from the required set. Every construct is required; an
exemption is an entry in `coverage::EXEMPT` carrying its reason, and the report
reds if the corpus later demonstrates an exempt construct, so the allowlist can
only shrink.

**Why the construct list cannot go stale.** The kinds are generated from one
list that produces both the enum and its `ALL` slice, and each kind is assigned
by an exhaustive `match` over `Node`, `Cond` and `Paint` — or, where the IR type
is a struct rather than a sum, by an exhaustive destructure of it, which is how
`Reorient` is classified. A new IR variant, or a new field on a frame request,
therefore **fails to compile** until someone classifies it, and it then begins
life at zero bindings — a surface nothing demonstrates is a finding on the day it
lands. The check is bound to two events rather than to a line in this document:
an IR change cannot compile past it, and a corpus change cannot be pushed past
the `#[test]` in `crates/grammar/src/coverage.rs` that carries the same
assertion inside `cargo test --workspace`. CI runs the command as its own step so
the table reaches the log.
