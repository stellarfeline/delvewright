# spec-0071: A creator types judgements only — what the quest stage makes an agent repeat

- **Status**: Proposed (draft)
- **Ground**: written against engine `90e44a8a` (`origin/main`), read only — `ShopOffer`, `Happening`, `SoundAt` and the per-effect `when` guard in `crates/dsl/src/stages.rs`; `happening_subject_checks` in `crates/dsl/src/validate.rs`; `delvec l10n-inventory` and `tools/creator/i18n-translate.py`; `docs/reference/i18n.md` — and against one creator run's work product: a 960-line script that writes `quests.json` and `dialogue.json`, read in full.
- **What it is for**: a creator agent writes the quest stage as the document, not as a program that prints the document.
- **Numbers**: no ADR. **DW code**: `DW0901`, for §2. **`dsl_version`**: **moves, by one minor step.** The format number is the `delvewright-dsl` crate version (ADR-0024), this change edits that crate's source, and a published version cannot carry different bytes — so the number moves whatever the surface does. A minor step rather than a patch because a new refusal is a format change in its own right: a document that compiled before, and whose purchase does not add up, is refused after. The value itself is not written here — it is typed in exactly one place, `crates/dsl/Cargo.toml`, and `tools/lib/version_sites.py` refuses a second. What does **not** move is the surface: §2 adds no field and §3 is a reading rule, so no document stops parsing and no surface label is owed.
- **Non-goals**: a new surface syntax; a host language; a `price` field (§5); any change to what a player experiences.

## 1. The finding

The script has almost no loops. It is one literal and about twenty constructor functions, and every constructor exists to stop typing something twice. That makes the script a measurement of the stage's surface: each constructor marks an input that is a derivation, entered by hand. The constitution's rule is that a derivation is handed by the tool and never typed.

| constructor | what it removes | verdict |
|---|---|---|
| `offer(label, price, give)` | the price, typed once in the label, once as the `at-least` gate on the item, once as the `at-least` gate on the charge, once as the charge's amount, once (minus one) as the `at-most` gate on the apology | §2 |
| `open_gate(g, text)`, `spawn_actor(a, text)`, `despawn_actor`, `close_gate`, `spawn_wave` | `happening.subject` restating the effect's own object | §3 |
| the translation table keyed by English text, and a second script merging it into the sidecar | sidecar keys are positional (`shop.<id>.offer.<i>.label`); an inserted offer renumbers them | §4 |
| `A("x")` → `"anchor/x"`, and the same for `obj/`, `actor/`, `wave/` | the typed id prefix | declined, §5 |
| `sound(s, at)` | `{"at": {"at": "anchor", "anchor": …}}` | declined, §5 |

## 2. A purchase that does not add up is refused where it is written

spec-0032 rules that a price is a gate term and a refusal is authored (`ShopOffer`'s own record: *a `price` field would be a second comparison surface for one meaning*). That stands. Its cost is that one number is typed four or five times with nothing binding the copies: an offer that gates on `at-least 15` and charges `16` compiles.

**Rule (authored), as built.** The bound object is a pair: one effect list, and one state `S` some effect of that list charges — an `add-state` moving `S` by a negative amount. The quantifier is *every effect list whose effects charge a state*, so a shop offer, an environment trigger, a trap payload, a quest bundle and a `sequence` step are one rule reached through the single effect-root enumeration; a shop is the shape's commonest home, never what the rule is about. Each effect is judged under the conjunction of its own `when` and the gate of whatever the list hangs off. For one such pair:

1. An effect charging `−n` fires only at balances its gate leaves open, so the **floor** of that conjunction must be at least `n`; a lower floor is an error naming both literals, the state, and the balance the charge lands on.
2. An effect guarded `S at-most k` and by nothing else on `S` is the arm that answers below the price, so `k` must be `m − 1`, where `m` is the lowest balance at which any charge of the list fires. A lower `k` is a **gap** — balances at which the list neither sells nor answers — and a `k` at or above `m` is an **overlap** — balances at which the player is charged *and* told they cannot afford it. Both name the balances.
3. An effect declaring a floor **and** a ceiling on `S` states an interval and is not an answering arm; rule 2 leaves it alone.

**Strengthened against the draft.** The draft read only *that effect's own guard*. As built, a list is judged together with the gate of what it hangs off — an offer's own gate, a trigger's or trap's arming gate, a nested list's parent `when` — because both must hold for the charge to run. That is what makes the spelling spec-0032 prescribes (the price on the offer's gate, the effects bare) pass, and what refuses an offer gated at 15 whose bare debit takes 16, which reading the effect alone would have missed.

**Loosened against the draft, declared as a loosening.** The draft made *any* charge with no `at-least` on `S` an error. As built, that arm fires only where the list prices the same state somewhere else — another effect's guard, or the gate the list hangs off. A lone `add-state −3` in a list that says nothing else about the datum has no second copy of the number to disagree with, and a campaign that means a datum to go below zero (a debt, a countdown past its floor) is a design this rule has no standing to refuse; a check that resolves against a smaller world than the campaign has refuses content. What it costs is that an ungated drain in an otherwise silent list is not caught. `dw0901_says_nothing_about_a_datum_the_list_does_not_price` in `crates/dsl/tests/v10_economy.rs` pins the narrowing so it cannot widen by accident, and `dw0901_an_ungated_charge_beside_a_priced_arm` pins the arm that remains.

## 3. An effect's happening is about the effect's object unless the creator says otherwise

`Happening.subject` is optional, and the branch contradiction proof (`DW0485`) reasons only over beats that name one — so a careful creator names it every time, and every time it is the id two keys to the left.

**Rule (authored), as built.** A *subject kind* is an `npc/`, `actor/`, `wave/` or `anchor/` id — the namespace `happening.subject` already polices. Where a `happening` hangs on an effect naming **exactly one** object of a subject kind, an absent `subject` resolves to that object, for the proof and for the chronicle alike. An effect naming several resolves nothing, because naming one of them would be the compiler guessing which; so does an effect naming none. A stated `subject` always wins: the caller knows more. One derivation (`QuestEffect::happening_subject`), read by the namespace check and by the chronicle writer, so a beat cannot be about one thing for the proof and another for the account a reviewer is handed.

**The rule is over the object classes an effect names, not over a list of verbs.** The draft listed seven verbs; the enumeration below is what the rule actually answers, computed over the `Verb` variant set the exported schema declares (`every_verb_answers_the_census` in `crates/dsl/src/stages.rs`, which holds its table to that variant set, so a thirty-ninth verb reds until somebody answers for it). Seventeen of thirty-eight resolve.

| resolves | the object it names |
|---|---|
| `open-gate`, `close-gate` | its gate anchor |
| `spawn-npc`, `despawn-npc` | its npc |
| `spawn-actor`, `despawn-actor`, `unleash-actor` | its actor |
| `spawn-wave` | its wave |
| `set-block` | the cell it writes |
| `fill-region`, `clear-region`, `collapse` | its region's anchor |
| `set-checkpoint`, `bonfire` | the seat |
| `play-sound` (with `at: anchor`), `firework` | the mark it sounds or launches from |
| `begin-stealth` (one zone) | that zone's anchor |

| resolves nothing | why |
|---|---|
| `move-npc`, `move-actor` | a body **and** a destination anchor |
| `teleport`, `volley` | two anchors, both load-bearing |
| `cutscene` (a path of more than one cell) | a whole path |
| `begin-stealth` (two zones or more) | one beat, several places |
| `campaign-complete`, `give-item`, `set-flag`, `set-state`, `add-state`, `clear-state`, `drop-stake`, `narrate`, `set-time`, `set-weather`, `damage-players`, `give-effect`, `clear-effect`, `end-stealth`, `sequence` | no object of a subject kind |
| `open-way` | **the named gap** — see below |

Four verbs answer **per instance rather than per verb**, because the object is in an optional field: `play-sound` resolves its mark only when it states `at: anchor`, `damage-players` / `give-effect` / `clear-effect` resolve the anchor of an `in` filter when they carry one, `begin-stealth` resolves a single zone's anchor, and a `cutscene` whose path names one cell resolves it. That is the rule answering, not a gap in it: the same verb is about a place in one document and about nothing in particular in another (`a_verb_whose_object_is_optional_answers_per_instance`).

**The named gap: `open-way`.** Its object is a placed piece (`prefab/<name>`), which the subject namespace does not hold — so the one verb whose whole meaning is opening a way must state its subject or resolve nothing. Widening the namespace to `prefab/` is a DSL surface decision and no ruling is taken here.

This is a reading rule, not a field: no document stops compiling, and §3 adds no surface for a `dsl_version` step to number (the number moves for the crate's own reason — see the header).

## 4. An agent that translates hands over a table, and the tool does the keys

The inventory already carries each key's English; the sidecar already records the source each row was translated from. `tools/creator/i18n-translate.py` fills a sidecar by calling an outside model. A creator agent that is itself the translator has no verb: it must address rows by positional key, which it cannot do stably, so it keyed its table by English text and wrote the merge itself. **Deliver**: `delvec l10n-apply <campaign> --lang <code> --table <file>`, where the table maps canonical English to the translation: every inventory row whose English is in the table is written with its `source`; rows already translated from unchanged English are kept; the run ends by printing `N of M rows translated` and listing every English string still missing and every table entry that matched no row. Two rows with the same English and different intended translations are the named limit: the table form cannot say it, the tool lists such keys, and the sidecar remains directly editable for them.

## 5. Declined, with the reason

- **A `price` field** — settled by spec-0032; §2 removes the hazard without a second surface.
- **Bare ids** — the prefix is what lets a dangling or mistyped reference be refused by kind at the place it is entered.
- **A string shorthand for a position** — it would make `SoundAt` and its siblings untagged unions, the construct ADR-0018 §7 names as the one where a later variant silently changes what an existing document parses as.
- **A scaffold verb for the document envelope — declined, measured.** The envelope (`dsl_version`, `campaign_id`, `stage`, `content`) is typed by hand on each of the twelve stage documents and on every sidecar, which the `/new-delve` page's own first document step states as a template (`references/workspace.md`, *The envelope, and the number in it*). Of the three typed keys, `dsl_version` is **already handed**: exactly one number is accepted, `DW0102` names it, and `delvec fmt` stamps the engine's own (ADR-0024) at the step the page already runs. `stage` is the file's name and `campaign_id` the directory's, and a wrong one is refused at parse rather than carried. `delvec metrics --gym` already writes nine complete documents that build. A scaffold verb would be a third way to create a document beside those two, for keys that are either stamped or refused where they are entered — so it is declined, and the reason is that the derivation this one would hand is the only one already handed.

## Acceptance criteria

1. §2: a fixture offer gating `at-least 15` and charging `16` is refused with the new code, the message naming both literals and the state; a fixture with `at-most 13` / `at-least 15` is refused naming balance `14`; the gallery's shop compiles clean; the diagnostic's binding count over the gallery is printed and non-zero.
2. §3: a fixture with two branches whose contradiction involves a gate opened by an effect with no stated `subject` reds `DW0485`; the same fixture with the derivation disabled in the test does not — the perturbation only this rule can catch. A stated `subject` differing from the effect's object is honoured.
3. §4: on a campaign fixture, `l10n-apply` with a complete table yields a sidecar `delvec validate --lang` accepts with zero missing keys; inserting an offer ahead of an existing one and re-running yields the same translations on the renumbered keys with no table change; an unmatched table entry and a missing row each appear in the printed lists; the output is byte-identical across two runs.
4. `docs/reference/compiler.md` (the new code, the reading rule), `docs/reference/i18n.md` and `docs/reference/tools.md` (`l10n-apply`) are updated in the same change. **The `/new-delve` page's translation step is a recorded debt, not a pass**: the page pins an engine release tag, `tools/ci/check-skill-page.py` refuses a page that drives a verb the pinned CLI does not carry, and the tag this change is built on predates `l10n-apply`. Until it is discharged the verb is documented and unreachable from the page. What discharges it: the change that cuts the next engine tag moves the pin and adds the step in the same commit.
