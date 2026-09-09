# Step 11 — the branch chronicle


Skip if it does not. If it does, this step is not optional and not delegable:
skip it and the campaign is not verified, however green the ladder is.

The compiler has compiled your documents **back into natural language**.
`<out>/validation/branch-chronicle-<branch>.md` is one branch's storyline in
compiled play order — every reachable node's `happening` line, first beat to
ending — and `validation/branch-plan.json` lists the branches. You compare like
with like: prose against prose. Nobody reliably compiles JSON in their head.

For **each** branch:

a. Read its chronicle **end to end, in order, in one pass.** Do not skim and do
   not sample: what this catches are contradictions in SEQUENCE ("Antiphos
   survives" at line 12, "Elpenor mourns Antiphos" at line 31).
b. Read it against `DESIGN.md`. Every beat the design promises on this branch
   must appear in the chronicle; every beat in the chronicle must be one the
   design licenses on this branch.
c. Read it against the dialogue **reachable on that branch**. **Every dialogue
   line touching branch-divergent state — who is alive, who is where, what was
   sealed, opened, lost or gained — must be LICENSED by a chronicle line of that
   branch.** An unlicensed line is a finding, not a matter of taste.
d. Write the **citation table into `GENERATION.md`**. Every finding and every
   clearance cites chronicle lines by number:

   | branch | claim reviewed (dialogue/design beat) | chronicle line(s) | verdict |
   |---|---|---|---|
   | `branch/flee` | Elpenor: "We lost him at the mouth." | 14 `departs` | cleared |
   | `branch/flee` | Kalliope: "Antiphos is dead." | — | **FINDING** — no chronicle line licenses a death on this branch |

The pass **fails** if any branch-divergent dialogue line has no citation, if a
branch has no table rows at all, or if any row's verdict is a finding. A finding
is fixed in the documents — move the line behind the right flag, swap the cast's
dialogue root for that branch, or fix the branch the beat is on — and the review
re-run. Never argued away, never left for the human QA hour.
