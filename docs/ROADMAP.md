# Roadmap

Where the production line stands, and the blocks of work that come next, in order. Each block ends on an exit criterion a machine can check. A block is done when its criterion holds.

## Where the line stands

- **The engine** is `delvec` with the DSL crate `delvewright-dsl`, pinned to one Minecraft Java version (`versions.toml` holds every pin), published as [GitHub releases](https://github.com/stellarfeline/delvewright/releases) and to crates.io.
- **The front end** is the `/new-delve` Claude Code plugin in `.claude/skills/delvewright/`. Its Init fetches the pinned, checksum-verified `delvec` release archive for the creator's platform, with a source build as the floor ([`init.md`](../.claude/skills/delvewright/skills/new-delve/references/init.md)).
- **The proof** is the validation ladder: static quest-graph and command checks at compile time, PackTest on every pull request, and a mineflayer bot walking the critical path on release candidates ([`playtest-methodology.md`](reference/playtest-methodology.md)). The engine's own surface is exercised by the [gallery](../gallery/README.md), and each mechanic is demonstrated on a [demo level](demo-levels.md).
- **Released campaigns** live in [delvewright-campaigns](https://github.com/stellarfeline/delvewright-campaigns/releases): Nobody's Isle, Doune Castle: A Guided Tour, and Vesperhold.

## Block 1 — what shipping Vesperhold found (now)

Shipping Vesperhold surfaced defects in the engine's mechanisms, its proofs and its release plumbing. This block closes the ones still open.

**Creator-facing surface**
- A creator warning for a villager-bodied NPC under open sky on a thunder beat, which can turn into a witch ([issue](https://github.com/stellarfeline/delvewright/issues/832)).
- Every dialogue button carries a tooltip (spec-0078).
- A fallen player waits before respawning (spec-0077).

**Content repository**
- Vesperhold's program file moves to Git LFS ([issue](https://github.com/stellarfeline/delvewright-campaigns/issues/158)).
- Vesperhold 1.1, built on the engine that carries this block.

**Exit:** every issue and pull request linked above is closed by a merged change (`gh issue view <n> --json state` reads `CLOSED`); spec-0077 and spec-0078 are each merged with their implementation or declined; and a `release/vesperhold/v1.1.0` release exists in the content repository, published by its release workflow.

## Block 2 — quests beyond a line

Both ideas are recorded for the [idea ledger](ideas.md) in an open [pull request](https://github.com/stellarfeline/delvewright/issues/831).

- **A non-linear quest system.** A campaign's objectives form one line today, each checkpoint a gate the party visits in order. A web of parallel and branching quests is researched against established RPG practice, and the research lands under `docs/reference/` before a spec is written.
- **Quest points that are placed blocks.** An objective is marked by a real block the player uses (a lever, a button, a campfire) instead of a glowing item display, which shows through walls.

**Exit:** each idea has an approved spec in `docs/specs/` and its implementation merged, every new DSL unit is bound in the gallery (`tools/ci/check-gallery-coverage.py` green), and each has a built row in [`demo-levels.md`](demo-levels.md).

## Block 3 — the drill

Once every known engine-side obstacle is closed, the owner re-makes a campaign from zero through `/new-delve` alone, starting from an empty directory. Each step must be walkable as written; an obstacle found becomes a fix in the engine or the skill, and the drill restarts.

**Exit:** a campaign authored only through `/new-delve`, with no hand edit to compiler output and no engine change made during the drill, passes the release validation ladder and is published by the content repository's release workflow.

## Gating rule: density

A denser campaign tier (a castle with a heavy shortcut-loop topology and dense rooms) waits until three consecutive playtests have produced zero mechanical findings. The count of consecutive clean playtests is not yet recorded in the repository.

## Recorded directions, not scheduled

- **A modpack production line.** The same pipeline one scale up: a curated modpack and an adventure-designed open world, with story pockets set in natural terrain and the journey between them composed rather than searched.
- **A survival hub.** A vanilla survival world connected to delve instances with `/transfer`; nothing in the engine may assume a single-server topology in a way that blocks it.
- **Community campaigns.** The content repository accepts campaign-source pull requests (DSL only, rebuilt by trusted CI), and community prefabs enter through the audited admission pipeline (spec-0007).
- **Bedrock players.** GeyserMC, which lets Bedrock clients join a Java server, is the candidate for cross-platform reach without a stack switch (ADR-0019).
- **Other agent runtimes.** The DSL, compiler and validation contract are runtime-agnostic, so another hosted agent runtime can be adopted as a front end without touching `crates/` (ADR-0012). Building an agent runtime is out of scope.
