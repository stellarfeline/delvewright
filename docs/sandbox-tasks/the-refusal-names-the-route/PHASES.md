# Phases

A task of any size is cut into **gated rounds**. You do one phase, you stop, you
report, and you wait to be released before starting the next one. You do not run
to the end.

This is not a formality and it is not about supervision. It exists because the
expensive failure mode on a task like this is **committing early to a wrong
model of the problem and then building three phases on top of it**. A phase gate
is the only place that model is cheap to correct: at the gate it costs one
message, and two phases later it costs the whole run.

---

## The rule

- **Stop at the end of every phase**, even when the next one looks obvious and
  you are confident. Especially then: "the next step is obvious" is what it feels
  like from inside a wrong model.
- **Report, then wait.** Do not begin the next phase while writing the report and
  do not begin it after writing the report. A phase ends when you have been
  released, not when you have finished the work.
- **A phase may end early.** If the phase's own findings say the plan is wrong,
  that IS the phase's deliverable. Report it and stop. A round that returns "the
  premise is false, and here is why" is worth more than a round that returns the
  code it was asked for.
- **Never widen a phase.** If you find a second defect, write it in the report
  under "found and not changed". Fixing it is not initiative, it is an
  unrequested change, and it makes the round harder to judge — the reviewer can
  no longer tell which change produced which effect.

## What a phase gate must state

Four things, in this order. Nothing else is required and length is not a virtue.

1. **What is done.** What you actually changed, in files and in one sentence of
   intent per file. Not a narration of what you tried.
2. **What the acceptance tests say.** The literal command and its literal
   output — which tests are green, which are red, and *which assertion* a red
   fails on. Not "tests pass". A red that you expected to still be red is
   reported the same way as a green, with the reason it is still red.
3. **What you believe should happen next, and why.** Your model of the problem,
   stated plainly enough to be contradicted. This is the part the gate exists
   for. If your understanding of the problem changed during the phase, say so and
   say what changed it.
4. **What you found and did not change.** Defects, surprises, things that look
   wrong, anything in the task documents that was misleading or absent. An empty
   section here after a substantial phase usually means nothing was looked at.

## What a phase gate must NOT contain

- A request for the answer. "Should I do A or B" is a legitimate question only
  after you have said which one you think is right and why.
- A summary of the diff. The diff is readable.
- Work from the next phase, done in advance "to save time". It costs time,
  because it has to be undone when the gate corrects the model.

---

## Worked example — this task, cut into phases

This particular task is small enough that a competent agent might finish it in
one sitting. It is cut into three anyway, because the *shape* is what transfers:
**understand, then change, then prove**, with a gate between each.

### Phase 1 — establish the problem, change nothing

Read the acceptance test and the source it exercises. Determine what the current
behaviour actually is by running the test and reading the output, not by reading
the code and predicting.

**Deliverable:** a statement of what the refusal currently says, what the
acceptance test demands instead, and what you believe the difference is *about*.
No code changes. `git diff` is empty at this gate.

**Why it gates:** an agent that has misread which of the two messages is the
problem will build the wrong thing correctly. That misreading is visible in one
paragraph and invisible in a diff.

### Phase 2 — the smallest change that turns the acceptance test green

Change the source. Nothing else.

**Deliverable:** the diff, the acceptance test's output before and after, and the
full crate suite's output. If the change required something you were told not to
do, this is where you say so instead of doing it.

**Why it gates:** this is where the invariant gets weakened by accident. The
reviewer's step 7 — "does any building that was refused before now pass?" —
cannot be run against a phase 3 diff that has been tidied.

### Phase 3 — prove it does not do more than it claims

Show the change is *conditional* in the way the acceptance test's second half
demands: the new behaviour appears where it should and stays absent where it
should not. Demonstrate it, do not assert it.

**Deliverable:** the demonstration, and the "found and did not change" list for
the whole run.

**Why it gates:** a change that fires unconditionally passes the first half of
the acceptance test and is wrong. If phases 2 and 3 were one round, this is the
half that gets skipped when the first half goes green.

---

## Template

Copy this per gate.

```
## Phase <n> gate

### 1. Done
- <file> — <one sentence of intent>

### 2. What the acceptance tests say
$ <command>
<output>

Red: <test> — fails on <assertion>. Expected/unexpected, because <reason>.

### 3. Next, and why
<your model of the problem, stated so it can be contradicted>

### 4. Found, not changed
- <finding>
- <anything in the task documents that was misleading, missing or wrong>
```
