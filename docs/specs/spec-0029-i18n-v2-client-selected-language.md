# spec-0029 — i18n v2: the client picks the language

Status: **Implemented and shipped** (built and released in
`nobodys-cave-island v1.1.0`).

> The status above is verified by measurement, on the island built from
> `delvec 1.1.0` with no `--lang`:
>
> - the resource pack carries `assets/delvewright/lang/en_us.json` **and**
>   `assets/delvewright/lang/zh_cn.json`, **447 keys each**;
> - emission is components, not literals, nested arguments included —
>   `tellraw @a {"color":"green","fallback":"Objective complete: %s",`
>   `"translate":"delvewright.ui.objective.complete","with":[{"color":"white",`
>   `"fallback":"Step Into the Cave","translate":"obj.cave-of-plenty.checkpoint.title"}]}`
>
> A spec's `Status` is an approval record and nothing binds it to whether the
> thing exists, so it drifts in the one direction that matters — a shipped
> feature still reading as unbuilt, which is then reported to the owner as
> outstanding work. See the open finding on binding it.

## Context

spec-0024 §2 records option (a): v1 releases ship
**baked in one language** (`build --lang`), and v2 — *"translate keys +
per-language files in this same resource pack, the client auto-selects its own
locale, any unsupported locale falls back to English"* — was unblocked but not
built. `nobodys-cave-island v1.0.0` therefore shipped Chinese-only: an English
player joining it reads Chinese, with no fallback.

The work is smaller than it looks, because the hard half already exists.
`dsl::l10n::each_string` already visits **every** player-visible string with a
stable key, over all five effect roots, and `DW0331`/`DW0330` already width-check
*source and translation*. What is missing is only the delivery shape.

## Decision

**A player-visible string is emitted as a translatable component, and the
languages ride in the resource pack the release already ships.**

1. **Emission.** Wherever the compiler emits a text component carrying an
   authored string, it emits
   `{"translate": "<l10n key>", "fallback": "<English source>"}`
   instead of the literal. The key is the existing l10n key — no new key scheme,
   no new inventory, and `each_string` stays the single authority over what is
   translatable (the property that keeps *measured* and *translated* from
   drifting).
2. **Delivery.** `build` emits `assets/delvewright/lang/<mc_code>.json` into the
   resource pack for English **and** every declared language: a flat
   `{key: string}` map, English from the source documents, others from
   `l10n/<code>.json`. `resourcepack::build_pack` already takes arbitrary `extra`
   assets; this is a caller change, not a pack-format change.
3. **Fallback is vanilla's, and it is per-component.** `fallback` is resolved by
   the client whenever the key is absent — a locale we do not ship, a key a
   translator missed, **and a player who declined the resource-pack prompt**.
   That last one is why the fallback lives on the component rather than relying
   on the pack's own `en_us.json`: a declined pack has no lang files at all, and
   the delve must still be playable in English.
4. **`--lang` survives as a single-language bake**, unchanged, for local dev and
   for anyone who wants a one-language artifact. The release path stops using it.
5. **Language-code mapping is explicit.** Our codes are BCP-47-ish (`zh-cn`);
   Minecraft's pack files are `zh_cn.json`. One documented mapping table, and an
   unmappable declared language is a compile error, never a silently dropped
   language.

## What this changes for a campaign

Nothing in the DSL. No stage document changes, no `dsl_version` bump: this is an
emission change, and `world.json.content.languages` already declares the set.

Every campaign's emitted bytes change (literals become components). Released
delves reproduce through their pinned engine, per the versioning discipline —
`v1.0.0` stays byte-reproducible from its tag and its engine pin.

## Non-goals

- Translating the storybook, release notes, or any repo artifact. Those are
  authored per language already.
- Runtime translation of anything. No LLM ships in a delve.
- Changing who translates or the sidecar format (`docs/reference/i18n.md`).
- Server-side locale detection. The client already knows its own locale; asking
  the server to guess would be the workaround, not the primitive.

## Risks this spec accepts, and what it owes them

- **A string that is not in a text component cannot carry a translate key.** The
  implementation must enumerate every emission site `each_string` covers and
  prove each one lands in a component. Any site that cannot is a **named
  exclusion in `compiler.md`**, not a silent literal — and a diagnostic, so a
  future string added there cannot ship untranslated.
- **The bot's `displayNameOf` degrades.** `harness/src/executor.ts` reads an
  entity's custom name to *prefer* among same-type candidates; a component whose
  text is a translate key will not match an authored name. Identity never rests
  on it (the file says so), so this is a preference heuristic getting weaker, not
  a broken assertion — but it must be measured, not assumed: the bot run reports
  how many candidate-preference decisions it made and how many had a usable name.
  A drop to zero usable names is a finding.
- **PackTest assertions on rendered text** must be audited the same way.
- **The width gates get more binding, not less.** `DW0330`/`DW0331` already check
  source and translation. Under v2 *any* declared language may be what a player
  sees, so every declared language's string must pass — which is what the gates
  already do, and now it is load-bearing rather than belt-and-braces.

## Acceptance criteria

1. `delvec build <campaign>` (no `--lang`) emits, for a campaign declaring
   `["zh-cn"]`, exactly `assets/delvewright/lang/en_us.json` and
   `assets/delvewright/lang/zh_cn.json` in the resource pack, and the two key
   sets are **equal** — a key in one and not the other fails the build.
2. Every value in `en_us.json` equals the English source string for that key, as
   produced by `each_string` — asserted by comparing against a fresh inventory,
   not against a fixture.
3. No emitted datapack file contains an authored player-visible literal where a
   translate key belongs: a test walks the emitted commands/dialogs for each
   inventoried key's English text and finds none, except at sites listed in the
   named-exclusion table.
4. Every emitted translatable component carries a non-empty `fallback` equal to
   the English source. Binding count printed; zero components examined is a
   failure, not a pass.
5. A declared language with no mapping to a Minecraft code is a compile error
   naming the language and the mapping table.
6. Determinism holds: the double-build byte-identity gate passes, lang files
   included (they are `BTreeMap`-ordered).
7. The bot's critical path passes on a build carrying components, and the run
   report states the candidate-preference binding described above.
8. `nobodys-cave-island` builds with both languages, and a run with the client
   locale unset reads English.

## Consequences

- The resource pack stops being optional dressing and becomes the language
  carrier. A player who declines it gets English — correct, and now documented
  in the storybook's own host-facing terms.
- A release stops choosing a language. `release.yml`'s `lang` step, which reads
  `languages[0]` as "the primary authored language" — a set read as an ordered
  preference, and wrong for any campaign declaring two — is deleted rather than
  fixed.
- Cite: spec-0024 §2 (the v1/v2 ruling), `docs/reference/i18n.md` (sidecars),
  `docs/reference/compiler.md` (key scheme, width gates), ADR-0006
  (determinism), ADR-0010 (packaging).
