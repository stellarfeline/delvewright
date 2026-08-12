# Idea ledger

The owner's ideas arrive mid-task — in play, in review, in passing — and most
of this project's features started as one. This ledger is where an idea lands
THE MOMENT it is voiced, before any other work continues. It exists because
the two previous vessels both failed the "never lost" requirement: a spec is
too heavy to open per idea (and opening one guarantees nothing about
scheduling), and the planner's memory does not survive sessions.

**The binding that makes loss impossible:** `tools/planner-state.sh` prints
every row still in `captured` or `elaborating`, with its age, on the state
page — twice a day in a long session and after every compaction. A row can
only leave that list by graduating (`spec'd` / `queued`) or by an explicit
owner `declined`. Silence is not an exit.

**Capture protocol (planner):** one row, one line, appended immediately —
interrupting whatever task is in flight, because the append costs seconds and
the loss costs a feature. English, impersonal (this repo is public; verbatim
phrasing, when it matters, goes to `docs/notes/private/ideas-raw.md`). When
the owner asks for elaboration, the idea graduates to a spec and the row
records `spec'd → spec-NNNN`; when it is scheduled, `queued → task/PR`.

| id | date | idea | status | next |
|---|---|---|---|---|
| IDEA-0001 | 2026-08-12 | Idea-capture mechanism itself: lighter than a spec, mechanically un-losable, elaboration on demand | queued | this file + state-page section |
