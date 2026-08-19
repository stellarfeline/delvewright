# The sandbox-task packet format

A **sandbox task packet** hands an agent that has never seen this project a piece
of real work, in an environment with no access to the people who wrote it, and
gets back something judgeable. This page is the skeleton; the directory beside it
is the first instance, and doubles as the worked example.

One packet per directory under `docs/sandbox-tasks/<slug>/`.

```
docs/sandbox-tasks/<slug>/
  TASK.md        # the problem, and the done-condition. For the agent.
  PHASES.md      # how the work is cut into gated rounds. For the agent.
  READ-ONLY.md   # what may not be touched, and the forbidden moves. For the agent.
  read-only.txt  # the same list as regexes. For the reviewer's first command.
  CHECKLIST.md   # what the reviewer runs on the returned patch. NOT for the agent.
```

Plus one thing that does not live in the directory: **the acceptance test, in the
repository's normal test location**, committed on the packet's branch and red.

---

## The load-bearing decisions

Four, and they are what make the format work rather than the file names.

### 1. The acceptance test is the specification, and it ships red

`TASK.md` states the problem and the done-condition. It does **not** state the
route — the route is what the agent is being measured on finding. The precise
done-condition therefore lives in a test the agent can run, not in prose it can
argue with. The test is red when the packet is handed over, and turning it green
without touching it is the whole task.

This has a cost, and it is the format's sharpest edge — see "Where this format is
weakest" below.

### 2. The read-only list is checked by a command, before anything else

An agent that edits a test gets a green build **by construction**, so a reviewer
who runs the tests first is reading evidence the patch manufactured. The
checklist therefore opens by diffing the read-only set, and that step ends the
review on a hit — no weighing whether the edit was reasonable.

The list is a file of regexes so the check is a command rather than an intention.
An obligation that lives in a sentence is not a gate; it is a sentence.

Two refinements that took a round to find and both transfer verbatim:

- **`--diff-filter=MDR`, not the whole diff.** *Modified*, *deleted*, *renamed*
  are what "an existing test moved" looks like. An **added** test file is
  legitimate, and refusing it by path pattern trains the agent to write no tests
  at all. Added files get their own checklist step, read by eye.
- **Inline `#[cfg(test)]` blocks inside `src/` cannot be caught by a path
  pattern**, so they are a separate grep step rather than a line in the list.
  Any read-only list that is *only* paths has this hole.

### 3. The forbidden moves are six lines, in the agent's interest

Not a policy document. Six imperative lines the agent can hold in its head, each
of which describes a move that looks locally reasonable and is globally fatal.
The last one is always a permission: *if you cannot do it inside those rules,
stop and report why — that is a pass.* Without it, the rules read as a trap and
the agent optimises for looking compliant instead of reporting.

### 4. Phases turn a one-shot into a conversation

The expensive failure is committing early to a wrong model of the problem and
building on it. A phase gate is the only place that is cheap to correct. The
shape that transfers is **understand → change → prove**, with a gate between
each, and the gate's four required statements:

1. what is done; 2. what the acceptance tests literally say, per assertion;
3. what the agent believes should happen next, **stated so it can be
contradicted**; 4. what it found and did not change.

Item 3 is the one the gate exists for. Items 1 and 2 are readable from the diff
and the CI output; item 3 is the only thing in the report that is not recoverable
by other means.

---

## Writing `TASK.md`

Three sections, in this order, and the order is the point. Options-first or
mechanism-first is the failure: nobody can choose between doors into a building
they cannot see.

**1. What was observed to be wrong.** Concretely, in the output, and how it was
noticed. Paste the literal bad output. Then, explicitly: **what goes wrong if we
do nothing.**

**2. What the thing under discussion IS.** One sentence on what the subsystem is
and why it exists at all, then only the two or three facts the agent needs.
Everything else about the project is noise and is left out.

**3. What must be true when you are done.** A numbered list of conditions, each
one checkable by a command. Then, in one paragraph, the refusal to say more:
*the route is what you are being measured on finding.*

Then environment, commands, and what to report.

### The test that decides whether `TASK.md` is finished

**Could a stranger answer "what goes wrong if we do nothing?" from this document
alone?** If not, it is not a task description — it is a status line with a
question mark on the end.

A second test, cheaper and nearly as good: **is there a sentence whose subject is
an internal identifier?** A ticket number, a diagnostic code, a file path. Those
are references, never subjects. A stranger cannot resolve them and gains nothing.

---

## Writing the acceptance test

The discipline, in order. Steps 3 and 4 are the ones that get skipped and they
are the ones that matter.

1. **Write it against the current bytes**, not against the report that motivated
   the task. Confirm the defect is actually present at the revision the packet is
   cut from. *"This is already correct, and here is how it got that way"* is a
   sharper result than any packet, and it is only reachable if this step is real.
2. **Assert on identifiers, not on prose.** Assert what the author must be told
   in terms of the things they type — a class name, a field name, an error code —
   never a sentence. Wording then stays free, and the test stays about behaviour.
3. **Give it a negative half.** The positive half alone is satisfied by an
   unconditional change. The negative half — the neighbouring case where the new
   behaviour must *not* appear — is what forces the agent to compute a condition
   instead of appending a string. **This is the single highest-value paragraph in
   the test**, and it is the one that would not have been written without asking
   "what is the laziest patch that turns this green?"
4. **Watch it fail, then watch it pass under a throwaway fix you then discard.**
   A test nobody has seen go green may be unsatisfiable, and shipping an
   unsatisfiable task wastes the entire exercise. The throwaway never reaches the
   branch.
5. **Then break the throwaway on purpose**, in the specific way a lazy agent
   would — make the change unconditional — and confirm the test goes red again.
   Step 4 proves the task is possible; **step 5 proves the task is not trivially
   passable**, and it is a different measurement with an unrelated failure mode.
6. **Write, in a comment on the test, what would make it vacuous** — enumerated,
   in the order the modes bite. Someone will later be tempted to delete the
   negative half because it "duplicates" the positive one. The comment is what
   stops them.

---

## Where this format is weakest

Stated so the next instance checks it rather than discovers it.

**The acceptance test leaks the answer, and how much it leaks scales inversely
with task size.** The test is read-only and the agent is told to read it, so
whatever the test asserts is handed over. On a *message-shaped* task the leak is
close to total: a test asserting that a string contains `stair` and `via` has
named the construct the agent was supposed to discover, and the remaining work is
mechanical. On a *behavioural* task the test asserts an outcome — this input
produces that verdict — and the route from here to there is genuinely not in it.

**So the format is a poor fit for small message-shaped tasks and a good fit for
behavioural ones**, and the fit is a property of the subject, not of the writing.
When the task is small, either accept that the exercise measures execution rather
than diagnosis, or move the discovery into `PHASES.md` — make phase 1's
deliverable the agent's own statement of the problem, gated *before* it is
allowed to read the acceptance test at all.

**A second weakness, milder:** `CHECKLIST.md` step 7 — *did this make anything
newly legal?* — is the only step no command can run. Steps 1–6 all stay green
when a refusal quietly gets narrower in a way no fixture exercises. It is
therefore the step most likely to be skipped, and the one most worth keeping.
