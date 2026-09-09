# Playtest rounds

Generation is round 1. Everything after it is an iteration round against the
playtester's findings, and the playtest hour is the scarcest resource in the
pipeline. Full derivation: `$DELVEWRIGHT_ENGINE/docs/reference/playtest-methodology.md`. Mandatory
here:

1. **Keep a findings ledger in `GENERATION.md`** — one row per finding: number,
   its wording as reported, the round it was reported, status. Status is
   `fixed@rN`, `open`, `engine` (blocked on a capability gap), or `ruled` (closed
   with no code change). This table is the campaign's memory; a finding that lives
   only in chat is a finding that will be reported to you twice.
2. **Triage every finding the day it arrives**, as *content* or *capability gap*.
   A capability gap — the DSL has no way to express what was asked for — is never
   patched downstream and is therefore a **staging blocker**: either the engine
   work lands before the next playtest, or the round summary says, per item, that
   it is still open and not to test it. A finding that survives more than one
   round is a capability gap; staging a build while those rows are open is what
   makes a playtester meet the same defect twice.
3. **Close each finding twice: the instance, and the general form.** After fixing
   the instance, ask what rule it is an instance *of*, and file that rule as a
   diagnostic (the DW code is minted for you — never mint one yourself). When the
   diagnostic exists, **re-run it against the current build**: that sweep is the
   deliverable, not the code, and it routinely finds a second live instance the
   moment it lands. Where no diagnostic is possible, write that down; it becomes a
   risk item at the next staging review.
4. **Append every finding to the engine repository's
   `$DELVEWRIGHT_ENGINE/docs/playtest-findings.json`**,
   the same day, with its general form and the check that carries it — this is the
   cross-campaign ledger, and `GENERATION.md`'s table is the per-campaign view of
   it. A finding recorded only in the campaign is a finding the NEXT campaign
   learns nothing from.
5. **Audit the FULL ledger from round 1 before staging any build** — never from
   the last round, and never by reading. You do not have to remember to: the
   staging paths REQUIRE it (step 9). Run it yourself first so the red list is in
   the round summary before anyone is invited.
6. **Pre-flight, in this order, before the invitation**: full ladder green
   (PackTest → bot critical path + die-retry → every branch run) → staging gate →
   localized builds and a double-build that is byte-identical → server boots and
   self-checks → then invite. Not "the build compiled, come look".
7. **Update `DESIGN.md` in the same round and run its conformance review.** A
   design record left unupdated across rounds accumulates changes nobody asked
   for, and the audit that catches up finds them all at once.
8. **Close the round in `GENERATION.md` with its machine record**, not just prose:
   how many validation-loop iterations it took to reach green, and every DW code
   the round hit **with its count** (`DW0205 x3, DW0483 x3, DW0450 x1`). Write it
   even when the count is zero — a round that hit nothing is the datum that says
   the gates had nothing to say. It is the only source from which rounds-to-green
   can be read afterwards; a round summarised in prose alone is a round whose cost
   cannot be recovered.
