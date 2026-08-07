# ADR-0018: The creator toolchain — cargo as a prerequisite, one authoring crate, and the escape hatch at the grammar IR

- **Status**: Accepted
- **Date**: 2026-08-07
- **Source**: owner decisions in conversation, 2026-08-06 / 2026-08-07
- **Refines**: ADR-0017 (toolchain distribution), ADR-0015 (schema promotion policy)
- **Constrained by**: ADR-0001 (LLM never writes mcfunction), ADR-0006 (determinism),
  ADR-0007 (content lives outside this repo), ADR-0012 (product form)

## Context

`crates/grammar/src/library/bell/` holds ~1131 lines that are the zones of one
specific campaign, inside the engine crate — the exact thing the owner's
2026-08-06 general-engine ruling forbids (`CLAUDE.md`, Methodology). Moving them
required answering where one-off campaign complexity is allowed to live, and two
routes were argued:

- **A** — improve the DSL until the zone programs are expressible in it; content
  holds DSL only.
- **B** — creator-written Rust becomes a legitimate extension mechanism; content
  holds both.

The owner's premise for B is the crux and is **accepted in full**: the DSL cannot
be semantically complete for the "Minecraft campaign" domain, and a genuinely
one-off complex thing *does not want generality* — promoting it into the engine
would itself violate the general-engine rule. Under a two-layer picture (DSL
above, engine Rust below) that premise makes A untenable: one-off complexity has
nowhere to go but the engine, which is precisely how `bell/` got there.

**The two-layer picture was wrong.** The stack is:

```
campaign DSL  →  grammar Program IR  →  VoxelModel  →  .nbt + .json prefab
```

and the middle layer is **already data** — `Program` derives `Serialize,
Deserialize`, and `crates/grammar/src/ir.rs` states the intent as settled
architecture: a program is data, "which is what makes it schema-checkable,
hashable for provenance, diffable in review, and safe to accept from an LLM".

That last clause decides the ADR. The project already tolerates a Turing-complete,
nondeterministic, arbitrary-code generator **above** the determinism boundary —
the LLM — and it is safe only because its output is data saved as the artifact of
record. Creator code has the same shape. What matters is not the language it is
written in but which side of the boundary it sits on.

Two facts bound the cost. `crates/grammar` depends on **serde and serde\_json
only**; `ir.rs` imports `std`, `serde`, `crate::block::BlockState` and
`crate::geom::{Axis, Orientation}`; `compose.rs` imports only `ir`. And the
authoring API does not exist in Rust either: the builders (`abs`, `split`, `call`,
`fill`, `marked`, `cmp`, `all_of`) are private `fn`, and the crate is
`version = "0.0.0"`, `publish = false`.

## Decision

### 1. The escape hatch is the grammar IR, not creator code inside the compiler

A creator may compute a `Program` by any means at **authoring time** — Rust,
Python, an LLM — provided the emitted `Program` is checked in as the artifact of
record. Creator code never runs inside `delvec`, so ADR-0006 is untouched and
`delvec` stays a fixed, pinned binary.

Native creator Rust *inside* the compiler is rejected: Rust has no capability
system, so `SystemTime::now`, `env::var` and `HashMap` iteration order are all
**safe** code and `forbid(unsafe_code)` buys nothing. (A sandboxed WASM module
with an empty import section *would* be deterministic by construction — the IR is
`i64` throughout, so float opcodes can simply be refused — but every library
program is nullary `fn() -> Program`, so such a module could only return a
serializable `Program`. It would be a macro expander over this same data, not an
engine extension. Deferred, not refuted; see revisit triggers.)

Consequence for `bell/`: it moves to the **content repo** as an ordinary crate
depending on the published authoring library. No engine hosting mechanism is
needed, and the ruling that opened this ADR is satisfied without an exception.

**A second hatch already exists and is blessed here**: generating a prefab
directly (`.nbt` + `.json`, checked in, ADR-0004). It is equally safe — the
compiler's checks read *delivered blocks*, not provenance — but it is **inert**:
frozen geometry cannot be seed-varied or composed, and the ADR-0015 promotion
detectors cannot see it. Prefer the IR hatch where either would do.

### 2. cargo is a declared creator prerequisite; the creator-facing toolchain is Rust

Locating a JVM on a creator's machine was considered and **declined for a reason
stronger than preference: it cannot be tested.** A Minecraft player has a
launcher-private JRE (PrismLauncher keeps `java-runtime-beta/delta/gamma` under
its own support dir), not `java` on `PATH`; a discovery ladder over launcher
paths, `JAVA_HOME` and `PATH` is branch logic no CI job here can exercise, which
is the unbound-gate shape this project refuses. Nothing in the toolchain spawns a
JVM today — `delve-render` reads `minecraft.jar` as a zip for textures, in Rust.

Rust also removes a defect rather than adding one: a Java authoring API would be a
**hand-written mirror** of the IR types, and a mirror drifts — the
capability-duplication defect (`CLAUDE.md`, Methodology) at a language boundary.
Creators writing Rust use *the same types*.

The migration is small because the creator surface is already Rust: eleven
`delvec` subcommands (`validate`, `analyze`, `build`, `schema`, `l10n-inventory`,
`snapshot`, `blocking-chart`, `edit apply`, `edit preview`, `calibrate`). The
classification is by audience, not a blanket rule:

| class | language | why |
|---|---|---|
| `delvec` and creator-facing tools | Rust | one binary, one install, cross-platform by build |
| CI-only checks (15) | Python, stdlib-only | the runner guarantees `python3`; cross-platform is irrelevant; a JVM/Rust build step would slow every job for nothing |
| `tools/extract-*.py` (maintenance) | Python | harvest registries from the game; never in a creator's hands |
| `tools/skin/` (`delve_skin`) | **Python, kept** | see §3 |

### 3. Python is a declared prerequisite too, for skins — `delve_skin` is not ported

**Owner ruling, 2026-08-07**: do not reinvent the wheel. `delve_skin` vendors
skinpy-extended and a skinview3d-lineage headless renderer; a Rust port buys
nothing a creator can perceive. Skipping skin generation when Python is absent was
considered and **declined**: a missing skin is a build error today (`DW0309`, and
`read_skins` says so deliberately — "not a silent skip", so `edit` proves exactly
what `build` proves), and a degraded path would be another branch nobody
exercises. The audience makes the requirement cheap: a creator who installed
Claude Code is a developer.

So the creator prerequisites are **cargo** and **Python 3**, both declared, both
installed once, neither discovered.

### 4. The authoring surface moves into `delvewright-dsl` — crates.io stays at two packages

The IR types and builders (`ir`, `block`, `geom`, `compose`) move into
`delvewright-dsl`, published, made `pub`, and given the builder API that exists in
neither language today. `crates/grammar` keeps `publish = false` and retains only
the expander (`expand`, `export`, `model`, `split`, `orient`, `rng`), depending on
`delvewright-dsl`. A creator writes `delvewright-dsl = "0.2"`.

Publishing `delvewright-grammar` as a third package is cheaper today — flip
`publish = false`, no refactor — and was **declined** for a reason ADR-0016 makes
concrete: it adds a **fourth version line** to keep in lockstep, and
`validation/check-versions.sh` grows another arm that can silently fall a notch
out of step. Folding into `delvewright-dsl` adds none. The crate's own published
description already covers it — *"Staged JSON DSL types and schemas … the format
the delvec compiler reads"* — and a grammar `Program` is exactly that.

Exposing the API from `delvec` itself was **declined** for two reasons. `delvec`
is a binary crate, so a lib target would make a creator building a data structure
pull the whole compiler and CLI tree. Worse, it welds the authoring API to
`delvec`'s version line, which ADR-0017 §2 separated on purpose — a fix to the
format bumps dsl and leaves `delvec` and its tag valid, and that separation is
what makes a half-failed release retryable.

### 5. `delvec` gains the IR loader `docs/reference/grammar.md` already claims

That file states the IR "serialises to JSON (`serde`), which is the authoring
form". **No loader exists** — the only `from_str::<Program>` calls in the tree are
two tests. By this project's own taxonomy that documented claim is **unbound**: it
describes an authoring surface nobody can author against, and it is the reason
`bell/` had to be Rust (composition needs a `&Program`, obtainable only from a
Rust constructor).

### 6. The data is normative; a checked-in generator is provenance, not source

§1 lets a creator keep the generator that produced a `Program` — so the content
repo can hold both a generator and its output, and something must say which one
is the truth. **The `Program` (and the prefab) is normative. A checked-in
generator has no special standing: it is an ordinary authoring script.**

The alternative — requiring checked-in generators to be themselves reproducible,
so a CI re-run-and-diff could gate them — is rejected, because it re-imports onto
creator code exactly the determinism requirement §1 declined to impose. A hatch
whose occupants must be deterministic is not the hatch that was argued for.

**This ADR's own anti-mirror argument (§2) applies here and is named rather than
smoothed**: a generator and its artifact *are* a mirror pair, and mirrors drift.
The difference from the rejected Java API is not that this pair cannot drift — it
is that **one side is declared non-normative**, which is the only thing that stops
a mirror from being a defect. Two halves both claiming authority is the defect;
one authoritative half and one convenience copy is a tool.

So, precisely, and the asymmetry matters:

- **prefab ↔ `Program` IS bound**: prefab metadata already records
  `license.generated_by { generator, program, program_hash, seed }`.
- **generator ↔ `Program` CANNOT be bound**, and this ADR accepts that
  undetectable drift rather than pretending otherwise. A creator who wants the
  link may run a re-generate-and-diff; it **reports, it never gates**.

ADR-0012 set this precedent already: the generated DSL documents are the artifact
of record, not the LLM run that produced them. The LLM case simply had no
checked-in generator to drift, which is why the question only surfaces now.

### 7. The `Program` JSON is a versioned compatibility surface, from the first one

§4 publishes the type and §5 makes its JSON the authoring form, so a checked-in
`Program` becomes a long-lived on-disk format. **The crate's semver covers the
Rust API; nothing today covers the document.** Those are two compatibility
surfaces and only one has an answer.

Today the IR has **no version field and no `deny_unknown_fields`**, so an older
`delvec` meeting a newer `Program` **silently ignores what it does not
understand** and emits a world that is quietly wrong. This repo has already paid
for that exact failure once, in prefab metadata, and fixed it with
`deny_unknown_fields` + `DW0346`. The IR has neither.

Two constructs make it sharper: `Expr`, `Cond`, `Size` and `Mark::at` are
internally tagged, and two types are `#[serde(untagged)]` — where adding a variant
can silently change which variant an existing document parses as.

Therefore, **before the first `Program` is checked in anywhere**:

1. `Program` carries a **required** `version` — the document's own, not the
   crate's.
2. The loader **refuses** a version it does not know. Best-effort parsing of an
   unknown future document is the silent-wrongness failure again.
3. `deny_unknown_fields` across the IR types, so old-engine-meets-new-program is a
   named error rather than a skip.
4. New constructs are **fenced by version**, as stage surfaces already are, and
   `CLAUDE.md`'s version-adoption discipline extends to checked-in `Program`s in
   active campaigns.

The cost argument is the one this project already accepted for capability
ownership: a required version field **before** the first document exists costs
nothing, and **after** it is a migration of every checked-in artifact. Generality
and compatibility are both decided at the first site.

### 8. What this ADR does NOT decide

Everything above is scoped to **geometry and composition**, which is all `bell/`
is. The **campaign-semantics** half — a novel mechanic, a new trigger kind, a new
interaction — has no data layer beneath the DSL: beneath it is mcfunction, and
ADR-0001 forbids reaching it. **That half has no hatch, and this ADR does not give
it one.** Conflating the two halves is what made the vocabulary/composition line
and the general/specific line appear to disagree.

## Consequences

- ADR-0017's shelf premise changes and is **narrowed, not contradicted**: five
  prebuilt targets existed so a creator need not have cargo; cargo is now
  required, so the shelf becomes the fast path and the answer for a machine
  without a C linker. `cargo install` builds from source and therefore needs one
  (Xcode Command Line Tools on macOS, MSVC Build Tools on Windows) — the most
  common `cargo install` failure on a fresh Windows box, and the reason the shelf
  is not optional. (`cargo binstall` — prebuilt from Releases, falling back to
  building — is worth evaluating against the ADR-0017 asset naming; not adopted
  here.)
- `versions.toml` and the skill's prerequisites gain **cargo** and **Python 3**,
  declared, and `docs/reference/tools.md` says which class each tool is in.
- `delvewright-dsl` takes a minor bump for the IR surface; the version line stays
  its own, per ADR-0017 §2.
- ADR-0015's second-campaign gate becomes mechanically detectable, because the
  hatch is data: a hatch program that writes no blocks of its own and whose every
  `call` resolves into an included piece **is** composition by definition, and one
  included by another campaign **has** become vocabulary. Both are exact. Neither
  is computable over opaque code — which is the property that stops a hatch from
  anaesthetising the expressiveness signal this project steers by.
- The Route A test the owner proposed — the zone programs must be expressible
  *losslessly and more concisely* — becomes a runnable experiment once §4/§5 land:
  port the five bell zones to data and compare bytes and length. **It is allowed
  to fail**; if the data form is longer or less reviewable, that is the answer.

## Revisit triggers

- **The campaign-semantics half** (§8): the first campaign blocked on a mechanic
  that is neither geometry nor a composition of existing DSL primitives — a
  genuine ADR-0015 gate-(a) event on the semantics side. Decide it against that
  instance, not in the abstract; the detectors above are what will surface it.
- A creator needs a scope predicate or size computation outside `Expr`
  (`Add/Sub/Mul/Div/Rem/Max/Min`), meaning a rule must *run* during derivation.
  That is the real boundary of the data answer, and it would reopen the sandboxed
  (WASM) form in §1.
- Creators come to need *computed* programs rather than written ones, at which
  point §1's deferred WASM analysis applies.
