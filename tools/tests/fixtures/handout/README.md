# The three-place fixture, and its recorded allocations

`campaign/` is the smallest site plan the per-place handout has anything to say
about: a landing a body arrives in, a hall whose west wall the landing owns and
whose south arch is barred until the keeper yields, and a yard open to the sky.
Three places, two seams — one whose plane the other side owns, one gated with
the bar on this side — one owed spawn, one owed gate region, one owed unlock
footing. It validates and builds with every place a stand-in.

`allocations.json` is `delvec allocation campaign --all`, recorded by the engine
at `delvec 1.14.0`, as the stub `delvec` of
`tools/tests/test_handout.py` answers with. It is a copy of an instrument's
output and never an authority: `crates/delvec/tests/handout_script.rs` holds it
equal to the built binary's own answer, byte for byte, so a change to what the
engine hands a place reds there and the file is re-recorded with the same
command.

`blocks.json` is `delvec grammar blocks`, recorded the same way and held equal
to the built binary's answer by the same test file: re-record it with
`delvec grammar blocks > tools/tests/fixtures/handout/blocks.json`.

The per-place procedure of `references/detail.md` has been executed green on
this fixture once, end to end: three fresh agents, one per place in
critical-path order, each from its own handout; `delvec detail --all` and
`delvec build` exit 0 with all three places detailed and none massed. The
numbers are in the body of the commit that added this paragraph.
