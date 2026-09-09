# Init — the commands, and what every failure means

The page carries the step list, each step's postcondition, and the checklist
Init is finished by. This file carries the commands and the meaning of every
way each one fails. Read it while running Init and at no other time.

Every command below runs through the environment file I4 writes, once I4 has
written it: `. ~/.delvewright/env.sh && <the command>`.

## Contents

- [I0 — the mode](#i0--the-mode)
- [I1 — what has to be on the machine already](#i1--what-has-to-be-on-the-machine-already)
- [I2 — the engine tree](#i2--the-engine-tree)
- [I3a — `delvec` from the release shelf](#i3a--delvec-from-the-release-shelf)
- [I3b — `delvec` from source, the floor](#i3b--delvec-from-source-the-floor)
- [I3c — the whole binary](#i3c--the-whole-binary)
- [I4 — the environment file](#i4--the-environment-file)
- [I5 — the client jar](#i5--the-client-jar)
- [I6 — the library, named](#i6--the-library-named)
- [I7 — named, not installed](#i7--named-not-installed)
- [Where output goes, and the one place it cannot go](#where-output-goes-and-the-one-place-it-cannot-go)

## I0 — the mode

The mode is a property of the **working directory** and of nothing else — not
of where this page was loaded from, not of an environment variable, not of what
happens to be on `PATH`.

```sh
DELVEWRIGHT_MODE="$(
  if [ -f crates/delvec/Cargo.toml ] \
     && [ -f .claude/skills/delvewright/skills/new-delve/SKILL.md ]
  then echo dev; else echo creator; fi
)"
[ "$DELVEWRIGHT_MODE" = dev ] \
  && DELVEWRIGHT_ENGINE="$PWD" \
  || DELVEWRIGHT_ENGINE="$HOME/.delvewright/engine"
```

**One assignment, on purpose.** The mode is written here and read everywhere
else; a form with a branch per assignment is a form somebody adds a third branch
to, and a third mode is a page that behaves two ways nobody wrote down.


Both conditions, never one: a directory carrying only the first is a checkout of
the engine that does not carry this page, and a directory carrying only the
second is somebody's copy of the plugin.

**Dev mode owes one more thing.** `campaigns/` in the engine checkout must
resolve to a directory. When it dangles, **stop**: a campaign is never written
into the engine repository, and that link is what keeps it out. Say the link is
broken and what it should point at; do not create one and do not carry on
without it.

## I1 — what has to be on the machine already

| | why | check |
|---|---|---|
| `git` | I2 clones the engine tree, in creator mode | `git --version` |
| **Python 3.11+** | `tomllib` is stdlib from 3.11, and I3a's selector reads the pin with it. The three scripts beside this page are stdlib Python | see below |
| **Java 21+** | **the pinned game's own requirement** — 1.21.11 declares `javaVersion.majorVersion: 21` in Mojang's version manifest, and every jar-reading checker runs under it. Chunky is not where this number comes from: its launcher and `--update snapshot` both run under 17 | `java -version` |
| Docker | the machine ladder and the play server | `docker info` |

**Not here, deliberately.** Rust belongs to I3b: on the default path the archive
arrives built, and demanding a compiler for a download is the front-loading this
Init exists to remove. `git-lfs` belongs to the library,
which is optional and is taken at the step that needs it.

**Python is found by asking, and the answer is recorded once.** Three names may
answer to a Python across the three platforms, and none of them answers
everywhere: on Windows the documented commands are `python` and the `py`
launcher, and while the Python install manager does ship a `python3`, its own
documentation says that one "is not meant to be widely used or recommended" and
describes environments where `python3.exe` is simply not there. So the first
name that answers at 3.11 or above is recorded as `DELVEWRIGHT_PYTHON` for the
whole run, and nothing below invokes a Python by any other name:

```sh
for c in python3 python "py -3" ; do
  v="$($c -c 'import sys;print("%d.%d"%sys.version_info[:2])' 2>/dev/null)" || continue
  case "$v" in 3.1[1-9]|3.[2-9]*|[4-9].*) DELVEWRIGHT_PYTHON="$c"; break ;; esac
done
```

**No Python at 3.11: halt** — installing one is the user's action on their own
machine, and there is no version of this page that runs without `tomllib`.

**Java is a stop — but look before you halt.** A machine whose default `java`
answers below 21 very often *has* a 21 sitting beside it, unselected. Installing
a JDK is the user's action; **choosing among the ones already on their disk is
yours**, and halting for something already present spends the user's session on
a `PATH` line:

```sh
"$DELVEWRIGHT_PYTHON" scripts/find-jdk.py
```

It prints the highest JDK 21+ it found, as `<major> <path>`, and exits 1 having
printed nothing when there is none. It asks each binary its own version and
never reads one off a directory name: on a Homebrew machine `openjdk@20`, `@22`
and `@23` are all symlinks to whatever `openjdk` currently is, so the name says
20 and the binary answers 26 — and `openjdk@21`, the one that is genuinely 21,
is keg-only and does not appear in `/usr/libexec/java_home -V` at all.

Found one, export it for the session and say which you took:

```sh
export JAVA_HOME=<the path it printed>
export PATH="$JAVA_HOME/bin:$PATH"
java -version                            # confirm it, do not assume it
```

**Only if it prints nothing, halt** and tell the user a JDK 21 has to be
installed — that, and not the selection, is their action. The failure you avoid
by stopping is silent: a jar-reading tool whose Java is too old exits non-zero
with a traceback that never names the version, several hours into the run, and
reads as a broken gate.

**Docker absent: halt.** Steps 9 and 10 cannot run at all without it.

## I2 — the engine tree

The engine checkout is **not** the compiler. Several steps run a Python tool, a
compose file or a reference document that lives in that tree and cannot exist
anywhere else; they are all written `"$DELVEWRIGHT_ENGINE/…"`.

**Neither the revision nor the release is yours to choose, and neither is the
default branch.** `versions.toml` beside this page names both. Read them from
there; this page restates neither, because a revision or a version written on a
page goes stale the first time the pin moves and nothing reports it.

```sh
PIN=versions.toml         # beside SKILL.md, in the skill root
ENGINE_REF="$("$DELVEWRIGHT_PYTHON" -c 'import tomllib,sys;print(tomllib.load(open(sys.argv[1],"rb"))["engine"]["ref"])' "$PIN")"
ENGINE_REPO="$("$DELVEWRIGHT_PYTHON" -c 'import tomllib,sys;print(tomllib.load(open(sys.argv[1],"rb"))["engine"]["repo"])' "$PIN")"

mkdir -p ~/.delvewright
[ -d "$DELVEWRIGHT_ENGINE/.git" ] \
  || git clone "https://github.com/$ENGINE_REPO.git" "$DELVEWRIGHT_ENGINE"
git -C "$DELVEWRIGHT_ENGINE" fetch origin
git -C "$DELVEWRIGHT_ENGINE" checkout --detach "$ENGINE_REF"
[ "$(git -C "$DELVEWRIGHT_ENGINE" rev-parse HEAD)" = "$ENGINE_REF" ] \
  && echo "engine at $ENGINE_REF"
```

**The clone is guarded because the directory very often already exists** — a
second run on the same machine, or a checkout somebody made by hand — and a bare
`git clone` onto it is a hard failure at the third line of the toolchain step.
The four lines above are the whole answer and they are safe to run any number of
times: the clone happens once, the `fetch` brings the pinned revision into a
tree that may predate it, and the `checkout --detach` puts the tree at the pin
from wherever it was. A `delvewright.local.toml` a previous run left is not a
problem and is not deleted: it is gitignored there, `checkout` never touches it,
and I7's dry-run is what decides whether it is usable.

**`unable to read tree`** means the pin names a revision the remote no longer
carries: say so and **stop** — never fall back to the default branch, which is
the moving toolchain this pin exists to replace.

**In dev mode nothing is cloned.** Record `git -C "$DELVEWRIGHT_ENGINE"
rev-parse HEAD` and say which revision the run is authoring against.

## I3a — `delvec` from the release shelf

**`delvec` is one binary** (ADR-0023). Every creator-facing capability is a
subcommand of it — the compiler, the grammar, prefab admission, schematic
conversion, harvest, and both render arms. There is nothing else to install and
no second `PATH` entry to forget. **You download it**; building it is the floor
you fall to, not the route you take.

One command, run from the skill root:

```sh
"$DELVEWRIGHT_PYTHON" scripts/fetch-delvec.py --into ~/.delvewright/bin
```

It reads `[engine].repo`, `[engine].release` and `[engine].ref` out of the pin
beside this page — all three, and none of them restated anywhere — maps this
host onto the engine's own `[engine].targets` at `ref`, downloads that archive
and `SHA256SUMS`, reads this
archive's row in either form coreutils writes, verifies the bytes, unpacks, and
asserts `delvec --version` **equals** the release's number. It prints what it
bound: the target, the archive, the digest and the version.

Its exit code is the whole failure table, and none of the four means the same
thing:

| exit | what it means | what to do |
|---|---|---|
| `0` | the pinned engine is unpacked and answering | continue |
| `3` no target for this host | the shelf carries no archive for this platform | **I3b**, the floor. That is the answer ADR-0023 §2 gives for exactly this machine |
| `4` download failed | the transfer never completed | **I3b**, the floor |
| `5` checksum mismatch | the bytes are not the bytes the release published | **a refusal.** Never the floor, never a retry, never extract what you have. The published `SHA256SUMS` is the only thing binding those bytes to that release |
| `6` version is not the pin's | the shelf served a different engine than this page was written against | **stop.** Say which version answered and which the pin names |

## I3b — `delvec` from source, the floor

**Only when I3a exited 3 or 4, and in dev mode always.** Not on a checksum
mismatch: that is a refusal and this is not a way around it.

The floor needs a Rust toolchain, and installing one touches the machine outside
this project. **`cargo --version` absent is a hand-over**: offer `rustup`, say
what it installs and where, and wait.

```sh
( cd "$DELVEWRIGHT_ENGINE" \
  && cargo --version && rustc --version \
  && cargo build --release -p delvec )
export PATH="$DELVEWRIGHT_ENGINE/target/release:$PATH"
```

**Build from inside the clone, and read the two version lines it prints.** The
engine pins its compiler in `rust-toolchain.toml` at its own root, and rustup
finds that file by walking up from the **working directory** — never from a
`--manifest-path`. Build from a directory outside it and rustup never sees the
pin: your default toolchain compiles the engine, at exit 0, with nothing
anywhere saying so. Standing in the clone is what makes the pin apply, which is
why the `cd` is not tidiness.

```sh
grep channel "$DELVEWRIGHT_ENGINE/rust-toolchain.toml"
```

`cargo --version` and `rustc --version` inside the clone must both answer the
channel that file names. Compare them against that file rather than against this
page — the page can go stale, the file cannot. **A different number means the
`cd` did not take effect**, and everything built after it was built with the
wrong compiler: rebuild.

**In dev mode the binary is held to the checkout's own number.** `delvec
--version` must equal `"$DELVEWRIGHT_ENGINE/versions.toml"`'s `[engine].version`;
anything else is a stale binary, and the repair is a rebuild.

## I3c — the whole binary

```sh
delvec --version               # delvec <x.y.z>, dsl <a.b.c>, mc 1.21.11
delvec --prefabs "$DELVEWRIGHT_PREFABS" render fidelity-gate
```

`render fidelity-gate` exiting non-zero means the GPU arms do not answer on this
machine: **stop**, because the visual half of the run cannot be reviewed and
nothing downstream would say so. **Write down the `dsl` number** — step 1 needs
it on every document.

The subcommand tree is the whole surface, and this page uses all of it:

| subcommand | what it is |
|---|---|
| `delvec validate` / `analyze` / `build` / `fmt` / `schema` / `metrics` | the compiler proper |
| `delvec snapshot` / `blocking-chart` / `allocation` / `edit` / `calibrate` | the layout loop |
| `delvec viewer` / `palette` / `scene` / `panorama` / `contact-sheet` / `index` | the CPU render arms |
| `delvec render` | the GPU arms (`piece`, `batch`, `fidelity-gate`) |
| `delvec grammar` | writes a new prefab from a rule program |
| `delvec prefab` | admits a prefab into the library |
| `delvec schem` | converts an outside schematic |
| `delvec harvest` | turns in-game playtest notes into a report |
| `delvec l10n-inventory` | the translation input |

Ask the binary rather than this table when you need the exact shape:
`delvec --help`, and `delvec <subcommand> --help` for a group's own verbs.

## I4 — the environment file

**Check whether your shell carries state between commands first.** Run `export
DW_PROBE=1` and then, as a *separate* command, `echo $DW_PROBE`. An empty answer
means every command you issue gets a fresh shell — the normal case for an agent
— and every `export` above is lost each time.

The environment goes in one file, written once, outside every campaign:

```sh
mkdir -p ~/.delvewright
cat > ~/.delvewright/env.sh <<EOF
export JAVA_HOME="$JAVA_HOME"
export DELVEWRIGHT_MODE="$DELVEWRIGHT_MODE"
export DELVEWRIGHT_ENGINE="$DELVEWRIGHT_ENGINE"
export DELVEWRIGHT_PYTHON="$DELVEWRIGHT_PYTHON"
export DELVEWRIGHT_PREFABS="$DELVEWRIGHT_PREFABS"
export PATH="\$JAVA_HOME/bin:<the bin or target/release directory>:\$PATH"
EOF
```

`DELVEWRIGHT_PREFABS` is written again by I6; write the line now and fill it
there. Every command on this page then runs as `. ~/.delvewright/env.sh && <the
command>`, and that is the form to use consistently.

**Do not reach for the shorter-looking remedy of calling `delvec` by absolute
path**: it carries `delvec` and nothing else, while later steps need
`$DELVEWRIGHT_ENGINE` for every Python tool and compose file, `JAVA_HOME` for
every jar-reading tool and for Chunky at step 12, and `DELVEWRIGHT_PREFABS` for
every invocation that reads a piece. A run that takes it reaches step 10 with
`DELVEWRIGHT_ENGINE` empty and reads the failure as a broken harness.

## I5 — the client jar

Every picture in this pipeline is drawn with Minecraft's own textures. The jar
is in no repository and this toolchain never redistributes it, so it has to
reach the machine one of two ways — and **which way is the user's decision, not
yours.** Reading files outside the project is not something you do because it
happened to be convenient; it is something they asked for.

**This is a hand-over: present both, and wait.** Say it in this form, then end
your turn:

> The renders need Minecraft 1.21.11's own textures — a 31 MB jar. By default I
> **download it from Mojang**, and it lands in `~/.chunky/resources/` and
> nowhere else. If you would rather not download, **tell me your Minecraft
> directory** and I will copy the jar out of it instead. Which?

Take **A, the download,** on a plain yes and on any answer that names no
directory. Take **B** only when they name one — never go looking for it
yourself, and never widen a directory they named into a search.

**A — download (the default).**

```sh
"$DELVEWRIGHT_PYTHON" scripts/fetch-client-jar.py --engine "$DELVEWRIGHT_ENGINE"
```

It resolves the version from `"$DELVEWRIGHT_ENGINE/versions.toml"`'s
`[minecraft]` pin, walks Mojang's version manifest to that version's client
download, checks the sha1 Mojang publishes **on the bytes as they arrive**, and
writes `~/.chunky/resources/minecraft.jar`. **A sha1 that does not match is a
refusal** (exit 5): it does not retry and it writes nothing.

Nothing in it is a constant this page made up. The manifest URL is the one
`"$DELVEWRIGHT_ENGINE/tools/check-patrol-types.py"` and
`"$DELVEWRIGHT_ENGINE/tools/derive-client-langs.py"` both already carry, and the
version is the engine's own pin. **The client half has no committed pin to agree
with**, so the sha1 checked is Mojang's own: it proves the transfer and the
version, and nothing in this project would notice if Mojang republished. Say
that when you report, rather than writing a pin of your own onto this page.

**B — copy, from the directory they named.**

```sh
mkdir -p ~/.chunky/resources
cp "<the directory they named>/versions/1.21.11/1.21.11.jar" \
   ~/.chunky/resources/minecraft.jar
```

If they ask where it usually is: `~/Library/Application Support/minecraft` on
macOS, `~/.minecraft` on Linux. Offer those for them to confirm — do not `find`
the disk for them.

Either way the jar ends at `~/.chunky/resources/minecraft.jar`, the last of the
three paths every texture-reading tool tries, in this order: `--textures <jar>`
on the command, `$DELVEWRIGHT_CLIENT_JAR`, then that file. Chunky reads the same
jar when it renders at step 12, which is why one copy serves both.

**Confirm the whole ladder answers**, over a piece the binary itself can write,
so the confirmation needs no library:

```sh
mkdir -p .out
delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar expand --program idiom-shape \
    --region 15x9x3 --seed 1 -o .out/probe
delvec --prefabs "$DELVEWRIGHT_PREFABS" palette .out/probe/idiom-shape.nbt \
    -o .out/palette.json
```

`mkdir -p .out` is not decoration: `delvec … -o` writes the file and does **not**
create its parent, so a missing directory comes back as `DW0722 … No such file
or directory` at exit 3 — a write error that reads like a missing prefab.
(`delvec render` does create its output tree; the two are not consistent.)

## I6 — the library, named

**Nothing is cloned here.** The shipped prefab library is an optional input:
its absence is a refusal at the step that wanted it, never a wrong answer. This
step decides what `DELVEWRIGHT_PREFABS` names and says out loud what is there.

| the working directory is | `DELVEWRIGHT_PREFABS` becomes |
|---|---|
| a dev engine checkout | the `campaigns/` link's `prefabs/` |
| a clone of the content repository | its own `prefabs/` |
| anything else, and `~/.delvewright/campaigns` exists | that clone's `prefabs/` |
| anything else | `campaigns/prefabs` under the working directory, created empty |

Report which of the four it is, and — when a clone answered — the revision it
stands at against the revision `"$DELVEWRIGHT_ENGINE/versions.toml"`'s
`[content].sha` names. A clone at a different revision is **named, not
corrected**: step 2 says which revision it will use and records it in
`GENERATION.md`.

The whole of what a library must satisfy before it is used, and how one is
taken, is *The shipped library*, named from step 2. Do not take it here.

## I7 — named, not installed

Init's job is that nothing later stops on a missing tool without having said so.
A creator about to lose their network needs to learn **now** that a download is
still owed, not four hours later.

**Chunky.** It renders every frame that has to *look* like Minecraft — the
player-POV review shots, the storybook art, the whole-map panorama. It is a
separate program: `delvec` writes the scene, Chunky renders it. Step 12 installs
it, at the moment the first frame is wanted, and step 14 reuses that install.

```sh
curl -sSfIL "$("$DELVEWRIGHT_PYTHON" -c 'import tomllib,sys;print(tomllib.load(open(sys.argv[1],"rb"))["render"]["chunky_launcher_url"])' "$DELVEWRIGHT_ENGINE/versions.toml")" -o /dev/null
```

Exit 0 means step 12 will be able to fetch it. **A non-zero is not a stop** — it
blocks no authoring step — but say it out loud here, because it *is* a stop at
step 12. Chunky needs no Java 21: its launcher and its `--update snapshot` both
run under 17. The 21 in I1 is the pinned game's own number.

**Reference images, and only on the drawing path.** The design gate at step 4 is
confirmed on pictures of the design, and there are two ways to have them.

*Path A — the campaign already has an approved design.* Look first:

```sh
ls campaigns/<campaign-id>/design/
```

A campaign being re-made carries `design/README.md` (the approved names),
`design/concept/` (one image per scene) and, when the map was designed as a
whole, `design/reference/` (the map views, their prompts, their style note and
their sidecars), with `design.json` beside them at the campaign root carrying
one row per approved image and the sky it was drawn under. **If that directory
exists, the reference exists.** Read it, author from it, judge against it, and
present every later choice beside it. You need no image provider, and Init is
finished. Do not re-draw an approved image; the approval is attached to the file
that is there.

*Path B — there is no approved design yet, and you are drawing one.*
`"$DELVEWRIGHT_ENGINE/tools/refimg.py"` draws reference images. It is stdlib
Python and needs nothing built, but it calls a **paid third-party image API**,
and three things must be in place before step 4 — establish them here, not at
the gate:

- a `[refimg]` section in `"$DELVEWRIGHT_ENGINE/delvewright.local.toml"`. The
  tool reads that one file and takes no `--config`.
  **Expect it to be absent, and write it — that is this bullet's whole
  instruction.** `delvewright.local.toml*` is gitignored in the engine, so no
  clone carries one and the checkout I2 just made has none. It is owed **once
  per checkout**, which means again the next time I2 clones the engine, however
  many times you have set one up before. Where the section comes from: the
  committed `"$DELVEWRIGHT_ENGINE/delvewright.toml"`, which documents the shape
  and stays inert. Copy its commented `[refimg]` block out, strip the `# `, and
  fill in `provider`, `model`, `api_key_env` and the frame keys that provider
  takes. `.local` overrides the committed file section by section, so `[refimg]`
  is the only section this file needs;
- **the key never enters that file, and there is nothing to paste into it.**
  `api_key_env` is the NAME of an environment variable, read at call time and
  never stored or logged, and an inline `api_key =` is refused outright. So the
  key lives in **the environment your shell sees**, under the name that section
  gives.
  - **The name is yours to choose, so look before you write it.** You are
    writing the section, so it can name whatever variable the machine already
    has — and the block you copied out of `delvewright.toml` names an example,
    not a requirement. Read the environment for provider keys first; this prints
    NAMES and never values:

    ```sh
    env | grep -Eo '^[A-Z0-9_]*(API_KEY|APIKEY|TOKEN)[A-Z0-9_]*' | sort
    ```

    If one of them belongs to an image provider, write **that** name into
    `api_key_env` and set `provider`/`model` to match it. Copying the example
    name literally and then finding it unset is how a run spends a turn of the
    user's asking for a key they already have under another name.
  - **Only when the list holds no provider key at all, ask the user for one**
    rather than guessing at a provider;
- a confirmation that costs no call:

```sh
"$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_ENGINE/tools/refimg.py" \
    --prompt "smoke test" --dry-run
```

Absent configuration exits 2 and says exactly what to add. A malformed one is a
hard error.

On path B this is a **hard prerequisite of the whole run**, not of one step. For
a site-plan campaign the map's own reference is the first thing written and
everything below is written against it; for an `areas[]` campaign the same wall
stands at step 4. Reaching either without a provider stops the line where
stopping is most expensive.

**The skin toolchain is not mentioned in Init at all.** A face is established
when a design first calls for one, at step 5.

## Where output goes, and the one place it cannot go

`.out/` under the working directory is scratch; put everything disposable there.
**One tree is different: the build output the machine ladder boots.**

Three ladder entries can boot a tree anywhere: `bot-run.sh` and
`packtest-run.sh` take `--output <tree>`, and `branch-runs.sh` takes the same
tree from `DELVE_OUTPUT` — give that one an absolute path, because
`branch-runs.sh` resolves a relative value against the engine root while compose
resolves it against `validation/`, so one relative value names two trees.

**Two paths still need the tree one level inside the engine's `validation/`,**
and for two different reasons. A bare `docker compose … --profile play` sets no
`DELVE_DOCKERFILE`, so `../Dockerfile.delve` is resolved against the build
context and only a tree beside `validation/` finds it. `--profile playtest` is
narrower still: that service's build block hardcodes `context: ./delve-output`,
so it cannot be pointed outside `validation/` at all. Anywhere else they fail
with `failed to read dockerfile`, which reads as a broken harness and is not one.

So every step that names a build output writes to
`"$DELVEWRIGHT_ENGINE/validation/delve-output"` — the one tree every path can
boot, in both modes, because the engine tree is a checkout in both. It is
gitignored there and no campaign file goes near it.
