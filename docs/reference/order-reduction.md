# Order reduction in the every-order search

The reader is an engineer changing `DW0485`'s every-order search (`crates/delvec/src/compiler/branch.rs`, `check_every_order`). It states which established method the search reduces orders by, what each part of it is taken from, and what the engine adds.

## The question

`DW0485` asks whether any legal play order of a branch shows a hard event contradiction. The legal orders of a branch with several strands a player can interleave grow as the product of the strands' lengths. The search merges two orders that reach one play state, which leaves the product of the strands' progress. A reduction that is exact must leave out only orders whose every clash some walked order also shows.

## The method: stubborn sets

Cited. A stubborn set at a state is a set of transitions such that exploring only its enabled members from that state preserves every reachable deadlock (Valmari, *Stubborn sets for reduced state space generation*, Advances in Petri Nets 1990, LNCS 483). The enabled members of such a set form a persistent set (Godefroid, *Partial-Order Methods for the Verification of Concurrent Systems*, LNCS 1032, Springer 1996, ch. 4). Godefroid's construction builds one from a seed: an enabled transition in the set adds every transition it may conflict with, and a disabled transition in the set adds a *necessary enabling set*, transitions one of which must occur before it can be enabled.

Cited. A safety property survives the reduction when every transition the reduced set leaves out is invisible to the property, and a cycle proviso keeps transitions from being postponed forever (Peled, *All from one, one for all: on model checking using representatives*, CAV 1993; Clarke, Grumberg and Peled, *Model Checking*, MIT Press 1999, ch. 10). Persistent sets combine with state caching for safety properties (Godefroid, Holzmann and Pirottin, *State-space caching revisited*, Formal Methods in System Design 7, 1995).

These sources were cited from the record, not re-read for this note.

## How the engine applies it

Authored.

- **Transitions** are the steps of the branch's path. **The state** is the walk's play state plus, per tracked subject, the last carrying line about it, which is what makes a clash a property of one transition at one state.
- **No proviso is needed.** Every step settles an objective, so the state graph is acyclic and no step can be postponed forever.
- **Visibility is folded into conflict.** Two steps whose lines act on one tracked subject conflict. Between steps that do not conflict, each writes the same lines in either order and reads the same tracker entries, so a clash is shown at the same transition in both orders. A step that can end the delve conflicts with every other step, because nothing is walked after it.
- **Conflict is static and over-approximated** from each step's footprint (`Footprints` in `branch.rs`): the flags, data, objective statuses, quest progress and quest activity it can write, and what it reads where a write can change what it does or shut it. Flags are only set, quests only start and objectives only settle, so a `requires` gate a legal step already passes cannot be shut. Two additions to one datum commute. A step that cannot be its quest's last objective never fires the quest's completion bundle.
- **A necessary enabling set** is read from the first conjunct of the replay's own legality test that fails (`Walk::blocked`). A quest that is not running needs an objective of the quest that starts it. A missing prerequisite or flag needs its writers. A held forbidden flag can never be cleared. A deed due in front of the step adds the writers of what the deed reads. A failed objective's step is legal anywhere, so the writers of the step's own status are always in the set.
- **The seed** is every legal step in turn; the search walks the smallest set found.
- **The quiet-step reduction stays.** Where a quiet step is legal it is walked alone; that is a singleton persistent set with its own argument.

## How it is checked

- `the_search_finds_what_walking_every_order_finds` compares the clashes the reduced, merged search finds with walking every legal order one at a time, over every `DW0485` fixture.
- `the_footprints_agree_with_the_replay` checks the footprints against the replay at every reachable state of every fixture. Two legal steps called free of conflict must commute, and a disabled step must stay disabled after any legal step outside its necessary enabling set.
- The binding line states at how many play states the stubborn set left a legal step out.
