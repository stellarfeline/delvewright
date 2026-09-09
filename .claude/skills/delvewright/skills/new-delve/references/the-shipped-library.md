# The shipped library — what it is, and how a campaign takes it

## What it is

A published set of `.nbt` prefabs with metadata beside each one, plus
`pools.json` naming the pools an `areas[]` campaign draws from. It lives in the
content repository, `stellarfeline/delvewright-campaigns`, under `prefabs/`, and
it is **an optional input**: a campaign that takes none of it is an ordinary
campaign, and every original-content path reaches a prefab without it —
`delvec grammar` expands a corpus that is Rust source inside the binary, and
`delvec schem` converts an outside schematic.

So nothing clones it on your behalf, and Init does not. This file is what you
read at the moment a campaign asks for it.

## When a campaign asks for it

Two moments, and only two:

- **step 2A**, when the campaign takes `areas[]` — the entries seat pieces from
  a pool, so there has to be a pool;
- **anywhere a document names a shipped piece by id** — a `prefab/<id>` binding,
  a `detail-plan.json` row.

A site-plan campaign that details nothing needs no library at all, and neither
does a campaign whose pieces are its own.

## The revision it must stand at

**The engine names it, and this page does not.** The pinned engine tree carries
the content revision every reproducible build resolves the library from:

```sh
CONTENT_SHA="$("$DELVEWRIGHT_PYTHON" -c 'import tomllib,sys;print(tomllib.load(open(sys.argv[1],"rb"))["content"]["sha"])' "$DELVEWRIGHT_ENGINE/versions.toml")"
CONTENT_REPO="$("$DELVEWRIGHT_PYTHON" -c 'import tomllib,sys;print(tomllib.load(open(sys.argv[1],"rb"))["content"]["repo"])' "$DELVEWRIGHT_ENGINE/versions.toml")"
```

A library at any other revision is a **different set of pieces**, and
determinism (ADR-0006) is over the DSL, the seed and the library together — so
a campaign built against one library and rebuilt against another is not the same
delve, whatever the documents say.

## The postconditions — what must be true before you use one

The means are yours. These four are not, and each one is checkable in a line:

1. **It is at the revision above.** `git -C <clone> rev-parse HEAD` equals
   `$CONTENT_SHA`. A clone standing anywhere else is *named* in `GENERATION.md`,
   with the revision it actually stands at, and step 2 says which one it used.
2. **Its `.nbt` files are real files, not pointers.** They are git-lfs objects,
   so a clone made on a machine where `git lfs install` has never run leaves
   every one of them a 130-byte text stub, and every tool that reads a piece
   fails on a file that looks present. `file <clone>/prefabs/hello-room.nbt`
   must say `gzip compressed data`, never `ASCII text`.
3. **`pools.json` parses and names at least one pool.** An empty library is
   indistinguishable from a clone whose LFS never ran, and the first is a real
   answer while the second is a broken one.
4. **`DELVEWRIGHT_PREFABS` names its `prefabs/` directory**, in
   `~/.delvewright/env.sh`, so every later command reads the same library.

```sh
git -C "$LIB_CLONE" rev-parse HEAD          # == $CONTENT_SHA
file "$LIB_CLONE/prefabs/hello-room.nbt"    # gzip compressed data
"$DELVEWRIGHT_PYTHON" -c 'import json,sys;print(len(json.load(open(sys.argv[1]))["pools"]), "pool(s)")' "$LIB_CLONE/prefabs/pools.json"
```

All four answering is the whole of the postcondition. How the clone got there —
a fresh `git clone`, a clone somebody already had, a worktree — is not this
page's business, and a means that ends at a verified postcondition cannot move a
byte.

`~/.delvewright/campaigns` is the place Init looks for one, so it is the place
to put one unless the working directory already is a content clone.

## What a campaign records about it

**A campaign that used the library says which library.** Write into
`GENERATION.md`, in the same act as the first document that names a piece:

- the content repository and the revision the clone stood at;
- whether that revision is the one the engine's `[content].sha` names, and if
  not, that it is not.

A delve reproduces from its documents, its seed and its library. Two of the
three are in the campaign directory; this line is the third.

## When it is absent

`--prefabs` names a directory that is not there, and the refusal says so. That
is the correct behaviour and it is not a failure of Init: the library is an
optional input, so its absence produces **no answer**, never a plausible wrong
one. Two ways forward, and both are ordinary:

- take the library, by the postconditions above; or
- make the piece the campaign needs — *when the prefab library has no piece you
  need* is the procedure, and it needs nothing from this library at all.
