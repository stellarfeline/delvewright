# spec-0071: A creator types judgements only — what the quest stage makes an agent repeat

- **Status**: Proposed (draft)
- **Ground**: written against engine `90e44a8a` (`origin/main`), read only — `ShopOffer`, `Happening`, `SoundAt` and the per-effect `when` guard in `crates/dsl/src/stages.rs`; `happening_subject_checks` in `crates/dsl/src/validate.rs`; `delvec l10n-inventory` and `tools/creator/i18n-translate.py`; `docs/reference/i18n.md` — and against one creator run's work product: a 960-line script that writes `quests.json` and `dialogue.json`, read in full.
- **What it is for**: a creator agent writes the quest stage as the document, not as a program that prints the document.
- **Numbers**: no ADR. **DW codes**: one, for §2, allocated at dispatch. **`dsl_version`**: moves only if §3 is taken in its field form (§3 says which).
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

**Rule (authored).** Within one offer, for each state `S`: if an effect adds a negative amount `−n` to `S`, that effect's own guard must require `S at-least m` with `m ≥ n`; otherwise the offer can drive `S` below what the creator gated on, and it is an error naming both literals. If an effect in the same offer is guarded `S at-most k` while another is guarded `S at-least m`, then `k = m − 1` or the diagnostic names the gap — a balance at which the offer neither sells nor refuses — or the overlap. The rule reads the gate, so it binds to every effect root that carries the same shape, not to shops: the quantifier is *every effect list whose effects charge a state*, and the binding count is stated from the objects.

## 3. An effect's happening is about the effect's object unless the creator says otherwise

`Happening.subject` is optional, and the branch contradiction proof (`DW0485`) reasons only over beats that name one — so a careful creator names it every time, and every time it is the id two keys to the left. **Rule (authored)**: where a `happening` hangs on an effect that has exactly one object of a subject kind (`open-gate`/`close-gate` → its anchor; `spawn-actor`/`despawn-actor` → its actor; `spawn-wave` → its wave; `spawn-npc`/`despawn-npc` → its npc), an absent `subject` resolves to that object for the proof and the chronicle. A stated `subject` always wins: the caller knows more. An effect with no single object resolves nothing, as today. This is a reading rule, not a field: no document stops compiling and `dsl_version` does not move. The acceptance is a fixture whose contradiction is found *only* through a derived subject.

## 4. An agent that translates hands over a table, and the tool does the keys

The inventory already carries each key's English; the sidecar already records the source each row was translated from. `tools/creator/i18n-translate.py` fills a sidecar by calling an outside model. A creator agent that is itself the translator has no verb: it must address rows by positional key, which it cannot do stably, so it keyed its table by English text and wrote the merge itself. **Deliver**: `delvec l10n-apply <campaign> --lang <code> --table <file>`, where the table maps canonical English to the translation: every inventory row whose English is in the table is written with its `source`; rows already translated from unchanged English are kept; the run ends by printing `N of M rows translated` and listing every English string still missing and every table entry that matched no row. Two rows with the same English and different intended translations are the named limit: the table form cannot say it, the tool lists such keys, and the sidecar remains directly editable for them.

## 5. Declined, with the reason

- **A `price` field** — settled by spec-0032; §2 removes the hazard without a second surface.
- **Bare ids** — the prefix is what lets a dangling or mistyped reference be refused by kind at the place it is entered.
- **A string shorthand for a position** — it would make `SoundAt` and its siblings untagged unions, the construct ADR-0018 §7 names as the one where a later variant silently changes what an existing document parses as.
- **Not examined**: the document envelope (`dsl_version`, `campaign_id`, `stage`) is typed by hand in every stage file. Whether a scaffold verb should hand it is left to the round that implements this spec to measure against the `/new-delve` page's own first step.

## Acceptance criteria

1. §2: a fixture offer gating `at-least 15` and charging `16` is refused with the new code, the message naming both literals and the state; a fixture with `at-most 13` / `at-least 15` is refused naming balance `14`; the gallery's shop compiles clean; the diagnostic's binding count over the gallery is printed and non-zero.
2. §3: a fixture with two branches whose contradiction involves a gate opened by an effect with no stated `subject` reds `DW0485`; the same fixture with the derivation disabled in the test does not — the perturbation only this rule can catch. A stated `subject` differing from the effect's object is honoured.
3. §4: on a campaign fixture, `l10n-apply` with a complete table yields a sidecar `delvec validate --lang` accepts with zero missing keys; inserting an offer ahead of an existing one and re-running yields the same translations on the renumbered keys with no table change; an unmatched table entry and a missing row each appear in the printed lists; the output is byte-identical across two runs.
4. `docs/reference/compiler.md` (the new code, the reading rule), `docs/reference/i18n.md` and `docs/reference/tools.md` (`l10n-apply`), and the `/new-delve` page's translation step are updated in the same change.
