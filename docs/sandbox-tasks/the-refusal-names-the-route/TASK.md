# Task: a refusal that names the route

You are working in a Rust repository you have not seen before. You do not need to
understand the product to do this task. Everything you need is below.

Read `READ-ONLY.md` before you edit anything, and `PHASES.md` before you decide
how far to go in one sitting.

---

## 1. What was observed to be wrong

An author was building a structure with a staircase in it. The tool this
repository provides refused their description of it with:

```
space "flight" has standable floor at y 1..4, which is 4 levels — a space is ONE
floor (at most two consecutive levels, for a dais). Two levels are two places and
a transition, and a transition is an edge that owes a `rise`
```

The refusal is **correct**. The author's description really did break the rule it
names, and the rule is a good rule.

The author read it, concluded the tool could not express a staircase at all, and
reported three separate structures as impossible to describe. Work was scheduled
to add a capability to the tool.

**The capability already existed.** There was a construct that describes a
staircase exactly, sitting in the tool the whole time. The author never found it,
because nothing in the refusal pointed at it — the refusal says which rule was
broken and stops there. The tool does have a sentence that explains where a
staircase's steps belong, and it is a good sentence; it is only ever printed to
someone who has *already* chosen the right construct. From where this author was
standing it was unreachable.

**What goes wrong if we do nothing:** authors keep hitting a refusal that is
right and useless, keep concluding the tool cannot do something it can, and keep
spending real time building capabilities that are already there. This has
happened; it cost one wasted round of design work and three structures reported
as impossible that were not.

---

## 2. What the thing under discussion IS

A **spatial contract** is a description an author writes of a building they
built — which parts of it are places a body can stand, and how a body gets from
one to another — which the tool then checks against the actual blocks, so a
building that does not match what its author claims is refused rather than
shipped.

Two facts about it are all you need:

- **A "space" is one floor.** That is deliberate and it is not up for discussion:
  it is the rule that makes the height of a transition between two places a
  measurable number. Do not weaken it, do not add an exception to it, do not add
  a flag that turns it off.
- The refusal above is produced by that rule, in
  `crates/grammar/src/contract.rs`.

---

## 3. What must be true when you are done

1. **The acceptance test passes.**

   ```
   cargo test -p delvewright-grammar --test contract_check \
     a_climb_refused_as_one_space_is_told_that_treads_belong_to_a_stairs_via
   ```

   It is red right now, on an unmodified checkout. That is expected: it describes
   the behaviour you are being asked to produce, and it is the definition of
   "done". Read it. It is the specification.

2. **Nothing else went red.**

   ```
   cargo test -p delvewright-grammar
   cargo fmt --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

   All three were green before you started. All three are green when you finish.

3. **You changed no file on the read-only list**, and you changed no test. See
   `READ-ONLY.md`. This is checked on the returned patch before anything else,
   and a change there ends the task regardless of what the build says.

4. **The rule itself is unchanged.** A space is still one floor. Every building
   that was refused before is still refused. You are changing what the author is
   *told*, not what is *allowed* — if your change makes something newly legal,
   you have solved a different problem.

The route from here to there is what you are being measured on. It is not
written down anywhere, and asking for it is not the task. The evidence you need
is in the repository: the acceptance test says what the message must contain, and
the source file that produces the refusal is named above. Everything else you
should work out.

---

## 4. Environment

Rust and Python are installed. Network access works, so `cargo` resolves and
downloads dependencies normally — the first build takes a few minutes and that is
expected, not a failure. Java is **not** installed; nothing in this task needs it,
and any test that would need it is outside the crate you are working in.

The whole task lives inside `crates/grammar/`. You should not need to touch
another crate, and if you find yourself doing so, stop and say why in your report
before continuing.

### Commands

```sh
# build just the crate under test (first run downloads dependencies)
cargo build -p delvewright-grammar

# the acceptance test, on its own
cargo test -p delvewright-grammar --test contract_check \
  a_climb_refused_as_one_space_is_told_that_treads_belong_to_a_stairs_via

# everything that must stay green
cargo test -p delvewright-grammar
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

If `cargo test -p delvewright-grammar` fails with a "No such file or directory"
panic mentioning `campaigns`, that is a missing fixture directory in your
environment and **not** something you caused. Report it and fall back to
`cargo test -p delvewright-grammar --test contract_check`, which does not need it.

---

## 5. What to report

Not a summary of your diff — the diff is readable. Report:

- what you concluded the problem actually was, in your own words;
- what you changed and why that is the right place for it;
- the exact command output showing the acceptance test red before and green
  after;
- anything you found that is wrong and that you did **not** change;
- anything in this document that was misleading, missing, or wrong.
