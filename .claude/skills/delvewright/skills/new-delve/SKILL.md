---
name: new-delve
description: Generate a complete playable Minecraft delve from a creative prompt — staged DSL authoring with validation-loop self-repair, deterministic compile, machine validation, joinable output. Use when the user asks to create/generate a new delve or campaign. Args = the creative prompt (theme one-liner or detailed brief).
version: 1.14.0
requires:
  delvec: ">=1.0.0 <2.0.0"
verified_with: 1.4.0
---

# /new-delve — building a delve, end to end

## Who runs this page, and what is not yours

**You are the agent, and this page is your procedure.** The skill is the
generation front-end; Claude Code is the runtime that executes it, and building
a separate agent runtime is permanently out of scope (ADR-0012). Someone typed
`/new-delve <prompt>`; that person is **the user** for the rest of this page,
and their prompt is a constraint set — what it pins down is honoured verbatim,
what it leaves open is yours to invent.

Every command below is yours to run and every document below is yours to write.
**Three things are not**, because they need a body in the game, a judgement that
is the user's to make, or their permission to touch something outside this
project. At each one you stop, hand over exactly what is needed, and **wait for
an answer**:

| where | what you hand the user | what you wait for |
|---|---|---|
| **Init 4** the client jar | the two ways it can reach the machine, download named as the default | which one — or the directory to copy from |
| **§4** the design gate | the design walkthrough — every scene, near view and far | an explicit yes |
| **§9** the walk | a running server, the connect line, and what to look for item by item | what they saw |

Stopping means: say what you have done, hand over the thing, say what you need
back, and **end your turn there**. Do not proceed on silence, do not substitute
your own judgement for the answer, and **never write anything that asserts a
step whose actor is the user actually happened** — a walk nobody walked and an
approval nobody gave are the two ways this pipeline produces a green run and a
delve no one has ever looked at.

§14 is also a hand-over, but nothing comes back: it ends the run. Anywhere else
the user *may* be offered a choice — which candidate piece, which frame — the
offer is optional and you proceed without it.

## What you are building

You author a delve as a set of **JSON documents**. `delvec` turns them into a
datapack and a world. You never write mcfunction, dialog files or datapack JSON,
and you never hand-edit an `.nbt` — everything the player meets is compiled from
the documents you wrote.

Read `CLAUDE.md` first if you have not — this repository's, and the engine
constitution it names at `$DELVEWRIGHT_ENGINE/CLAUDE.md`. The forbidden
zones apply in full.

The engine constitution names its own other half, `CLAUDE.local.md`, and tells
whoever lacks it to say so and ask "before improvising anything about dispatch,
review, merge or staging". **You will not have it** — it is gitignored on the
operator's machine, so no clone can produce it — **and this page improvises none
of those four.** It stops at §4 and §9 and hands those to the user; §14 hands
over and ends. So record the absence in one line and carry on with the run.
Do not stop to ask for it: the answer is the same every time, and the turn spent
asking comes out of the user's.

## The shape of the run

Everything below happens in this order, and the order is not a suggestion — each
step needs something the step before it produced.

```
Init            build the toolchain, once per machine        ── §Init
                STOP at Init 4 — the client jar is the user's
  ↓
Decide          areas[] or a site plan — one campaign, one    ── §Which placement model
  ↓
 1  workspace   the campaign directory and its documents
 2  placement   world.json areas[]   OR   brief → graph → plan
 3  story       npcs · classes · quest-plan
 4  GATE        STOP — the design walkthrough, the user says yes
 5  content     quests · dialogue
 6  fmt         delvec fmt              ← every campaign, not optional
 7  analyze     delvec analyze
 8  build       delvec build
 9  the walk    STOP — the user walks the blockout, you wait
10  ladder      PackTest · bot · branch runs
11  chronicle   only when the plan declares branch_points
12  visual      the POV sequence, then the renders
13  detail      site-plan campaigns only, and only after the walk
14  hand over   storybook, staging gate, play commands
```

Steps 1–14 are printed below in that order, one after another, with nothing
between them. Everything else on this page is **reference**: what the DSL can
express, how to write the prose, what to do when the prefab library has no piece
you need, what to do when something goes red. Reference sections come after the
steps, and a step names the one it needs.

Two branches change what you do, and both are decided before step 1:

- **areas[] or a site plan.** A campaign has exactly one placement authority.
  See *Which placement model* below — it is the one decision that cannot be
  changed later without redoing step 2.
- **Do you already have approved reference art?** A campaign being re-made from
  an approved design carries its images in `campaigns/<id>/design/`. If that
  directory exists, you need no image provider and Init step 6 is a read rather
  than a setup. See Init step 6.

## Init — build the toolchain before you author anything

**Your working directory is this repository, `delvewright-campaigns`, and you
never leave it.** Every path this page prints is relative to this repository's
root. The compiler and its sibling binaries are built from source out of a
second checkout that sits *beside* this one; that checkout is a toolchain
component, like Chunky or a virtualenv, and no campaign file ever goes in it.

Run all of this before writing a line of a campaign document. The floor is
deliberately low and always available: **clone and build from source.** A
downloadable binary would be an optimisation of that, never the guarantee, so a
tool that cannot be downloaded is built at the step that needs it rather than
skipped.

**If any step here cannot be completed, say so and stop.** Authoring against a
half-built toolchain produces a campaign whose visual half was never reviewed,
and nothing downstream reports that.

### 0. What has to be on the machine already

| | why | check |
|---|---|---|
| `git` | both checkouts | `git --version` |
| `git-lfs` | **this repository's `.nbt` prefabs are LFS objects** (`.gitattributes`). A clone without it materialises text pointers, and every tool that reads a piece fails on a file that looks present | `git lfs version` |
| Rust (stable) | `rustc -vV` names this platform's target triple, which is how step 2 picks its archive; and it is the toolchain floor — `delvec` is built from source when the release carries no archive for this platform | `cargo --version` |
| **Python 3.11+** | the checkers, the reference-image tool, the staging gate. 3.11 and not 3.10: `tomllib` is stdlib from 3.11, and reading `versions.toml` at step 2 needs it, as do three of this repository's five checkers | `python3 --version` |
| **Java 21+** | **the pinned game's own requirement** — 1.21.11 declares `javaVersion.majorVersion: 21` in Mojang's version manifest, and every jar-reading checker runs under it. Chunky is not where this number comes from: its launcher and `--update snapshot` both run under 17 | `java -version` |
| Docker | the machine ladder and the play server | `docker info` |
| A 1.21.11 Minecraft client jar | textures. **How it arrives is the user's choice**, put to them at step 4: downloaded, or copied out of their own installation. Never redistributed by this toolchain | see step 4 |

**Java is a stop — but look before you halt.** A machine whose default `java`
answers below 21 very often *has* a 21 sitting beside it, unselected. Installing
a JDK is the user's action; **choosing among the ones already on their disk is
yours**, and halting for something already present spends the user's session on a
`PATH` line. So when `java -version` answers below 21, enumerate first:

```sh
# `find` and not a glob anywhere: zsh makes an unmatched pattern a hard error,
# and it expands the pattern before the command sees it, so quoting the tool
# does not help. /usr/lib/jvm is absent on macOS and that must not be an error.
for c in $(/usr/libexec/java_home -V 2>&1 | sed -n 's/.* \(\/.*\)$/\1/p') \
         $(find /opt/homebrew/opt /usr/lib/jvm -maxdepth 1 -mindepth 1 \
                 \( -name 'openjdk*' -o -name 'jdk*' -o -name 'temurin*' \
                    -o -name 'zulu*' -o -name 'java*' \) 2>/dev/null) ; do
  [ -x "$c/bin/java" ] || continue
  # `[^"]*` and not `.*`: a greedy match runs to the CLOSING quote and captures
  # the empty string, so every JDK silently reads as version "" and is skipped.
  v="$("$c/bin/java" -version 2>&1 | sed -n '1s/^[^"]*"\([0-9]*\).*/\1/p')"
  [ -n "$v" ] && [ "$v" -ge 21 ] && echo "$v $c"
done | sort -n | head -1
```

**Ask each binary its own version; never read it off the directory name.** On a
Homebrew machine `openjdk@20`, `@22` and `@23` are all symlinks to whatever
`openjdk` currently is, so the name says 20 and the binary answers 26 — and
`openjdk@21`, the one that is genuinely 21, is keg-only and does not appear in
`/usr/libexec/java_home -V` at all. A search that trusts the path both rejects
the right JDK and accepts the wrong one, and neither failure announces itself.

Found one, export it for the session and say which you took:

```sh
export JAVA_HOME=<the path the loop printed>
export PATH="$JAVA_HOME/bin:$PATH"
java -version                            # confirm it, do not assume it
```

**Only if the loop prints nothing, halt** and tell the user a JDK 21 has to be
installed — that, and not the selection, is their action on their own machine.
The failure you avoid by stopping is silent: a jar-reading tool whose Java is too
old exits non-zero with a traceback that never names the version, several hours
into the run, and reads as a broken gate.

### 1. Materialise this repository's prefabs

You are already in the clone. Make sure its LFS objects are real files rather
than pointers — a fresh clone on a machine where `git lfs install` has never
run leaves every `.nbt` as a 130-byte text stub:

```sh
git lfs install
git lfs pull
```

Confirm: `file prefabs/hello-room.nbt` says `gzip compressed data`, not
`ASCII text`.

### 2. The toolchain: `delvec`, downloaded; the engine tree, beside this repository

Two different things arrive in this step and conflating them is how the step
goes wrong.

**`delvec` is one binary** (ADR-0023). Every creator-facing capability is a
subcommand of it — the compiler, the grammar, prefab admission, schematic
conversion, harvest, and both render arms. There is nothing else to install and
no second `PATH` entry to forget. **You download it**; building it is the floor
you fall to, not the route you take.

**The engine CHECKOUT is a separate acquisition**, and it is not the compiler.
Several steps below run a Python tool, a compose file or a reference document
that lives in the engine tree and cannot exist here; they are all written
`"$DELVEWRIGHT_ENGINE/…"`. Nothing is built out of that checkout on the default
path.

**Neither the revision nor the release is yours to choose, and neither is the
default branch.** `versions.toml` here names both — `[engine].authoring_ref` is
the engine this pipeline was last walked against end to end, and
`[engine].release` is the published release built from that same revision by the
engine's own CI. Read both from there; this page restates neither, because a
revision or a version written on a page goes stale the first time the pin moves
and nothing reports it.

```sh
ENGINE_REF="$(python3 -c 'import tomllib; print(tomllib.load(open("versions.toml","rb"))["engine"]["authoring_ref"])')"
ENGINE_RELEASE="$(python3 -c 'import tomllib; print(tomllib.load(open("versions.toml","rb"))["engine"]["release"])')"
[ -d ../delvewright/.git ] \
  || git clone https://github.com/stellarfeline/delvewright.git ../delvewright
git -C ../delvewright fetch origin
git -C ../delvewright checkout --detach "$ENGINE_REF"
export DELVEWRIGHT_ENGINE="$(cd ../delvewright && pwd)"
```

Confirm the checkout landed where it was told:

```sh
[ "$(git -C "$DELVEWRIGHT_ENGINE" rev-parse HEAD)" = "$ENGINE_REF" ] \
  && echo "engine at $ENGINE_REF"
```

**The clone is guarded because `../delvewright` very often already exists** — a
second run on the same machine, or a checkout somebody made by hand — and a bare
`git clone` onto it is a hard failure at the third line of the toolchain step.
The three lines above are the whole answer and they are safe to run any number of
times: the clone happens once, the `fetch` brings the pinned revision into a tree
that may predate it, and the `checkout --detach` puts the tree at the pin from
wherever it was. Do not adopt a pre-existing checkout without those two lines —
the revision, not the directory, is what this step delivers. A
`delvewright.local.toml` a previous run left is not a problem and is not deleted:
it is gitignored there, `checkout` never touches it, and Init step 6's dry-run is
what decides whether it is usable.

If the `checkout` fails with `unable to read tree`, the pin names a revision the
engine no longer carries: say so and stop — do not fall back to the default
branch, which is the moving toolchain this pin exists to replace.

#### 2a. `delvec` from the release shelf — the default

The release carries one archive per target plus one `SHA256SUMS` for the lot.
Take the archive for this platform, **verify it, and only then extract it**.

```sh
DELVEC_BIN="$(cd .. && pwd)/delvec-bin"
mkdir -p "$DELVEC_BIN"
TARGET="$(rustc -vV | sed -n 's/^host: //p')"
ARCHIVE="delvec-$ENGINE_RELEASE-$TARGET.tar.gz"
BASE="https://github.com/stellarfeline/delvewright/releases/download/$ENGINE_RELEASE"

( cd "$DELVEC_BIN" \
  && curl -fsSL -O "$BASE/$ARCHIVE" \
  && curl -fsSL -O "$BASE/SHA256SUMS" \
  && grep " $ARCHIVE\$" SHA256SUMS > "$ARCHIVE.want" \
  && if command -v sha256sum >/dev/null 2>&1; \
     then sha256sum -c "$ARCHIVE.want"; else shasum -a 256 -c "$ARCHIVE.want"; fi \
  && tar -xzf "$ARCHIVE" )

export PATH="$DELVEC_BIN:$PATH"
delvec --version
```

**A checksum mismatch is a refusal, not a retry.** The line must end `: OK`. If
it does not, stop and say so — do not download it again, do not take a different
file, and do not extract the one you have. The published `SHA256SUMS` is the
only thing binding those bytes to that release.

`rustc -vV` is what names the platform, rather than a guess assembled from
`uname`: it prints the exact target triple the archives are named with, and it
is already on the machine because the floor below needs it.

#### 2b. The floor: build it from source

**Only when 2a could not be completed** — the release carries no archive for
this target, or the download failed. Not when the checksum failed; that is a
refusal and this is not a way around it.

```sh
( cd "$DELVEWRIGHT_ENGINE" \
  && cargo --version && rustc --version \
  && cargo build --release -p delvec )

export PATH="$DELVEWRIGHT_ENGINE/target/release:$PATH"
```

One build, one `PATH` entry, one binary.

**Build from inside the clone, and read the two version lines it prints.** The
engine pins its compiler in `rust-toolchain.toml` at its own root, and rustup
finds that file by walking up from the **working directory** — never from a
`--manifest-path`. Build the engine from a directory outside it and rustup never
sees the pin: your default toolchain compiles the engine, at exit 0, with nothing
anywhere saying so. Standing in the clone is what makes the pin apply, which is
why the `cd` is not tidiness.

So the two lines above are the step's evidence, and they must agree with the
engine's own file:

```sh
grep channel "$DELVEWRIGHT_ENGINE/rust-toolchain.toml"
```

`cargo --version` and `rustc --version` inside the clone must both answer the
channel that file names. Compare them against that file rather than against this
page — the page can go stale, the file cannot. **A different number means the
`cd` did not take effect**, and everything built after it was built with the
wrong compiler. Step 0's `cargo --version`, run from *this* repository,
legitimately answers something else: that check asks whether you have Rust at
all, and rustup installs the pinned toolchain on demand the first time you build
in the clone.

#### 2c. One binary, and where its capabilities live

There is **one** binary and this page uses all of it. The subcommand tree is the
whole surface:

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

**`$DELVEWRIGHT_ENGINE` is not a convenience — this page prints it.** Several
steps below invoke a Python checker or a compose file that lives in that
checkout and reads engine sources that cannot exist here. Those are named
`"$DELVEWRIGHT_ENGINE/…"` everywhere they appear, and a bare `tools/…` on this
page always means *this repository's* `tools/`.

**Check whether your shell carries state between commands, before step 3.** Run
`export DW_PROBE=1` and then, as a *separate* command, `echo $DW_PROBE`. An
empty answer means every command you issue gets a fresh shell — the normal case
for an agent — and every `export` above is lost each time.

**When it is empty, put the environment in a file and source it as the first
clause of every later command.** Write it once, outside both repositories, with
the paths this Init produced filled in:

```sh
cat > ../delvewright-env.sh <<'EOF'
export JAVA_HOME=<the JDK path step 0 printed>
export DELVEWRIGHT_ENGINE=<the engine checkout's absolute path>
export PATH="$JAVA_HOME/bin:<the DELVEC_BIN directory from step 2a>:$PATH"
EOF
```

Every command on this page then runs as
`. ../delvewright-env.sh && <the command>`, and that is the form to use
consistently. **Do not reach for the shorter-looking remedy of calling `delvec`
by absolute path**: it carries `delvec` and nothing else, while later steps need
`$DELVEWRIGHT_ENGINE` for every Python checker and compose file, and `JAVA_HOME`
for every jar-reading tool and for Chunky at step 12. A run that takes it reaches
step 10 with `DELVEWRIGHT_ENGINE` empty and reads the failure as a broken
harness. Choosing per command is the other way to reach step 8 and fail on one
invocation alone.

Confirm both:

```sh
delvec --version          # delvec <x.y.z>, dsl <a.b.c>, mc 1.21.11
delvec render fidelity-gate
```

`delvec --version` must print an engine version inside this skill's declared
`requires.delvec` range, and `fidelity-gate` must exit 0. Either failing is a
hard stop. **Write down the `dsl` number it prints** — step 1 needs it.

### 3. `--prefabs prefabs` — the flag that is not optional here

**This is the single command-line fact that decides whether anything below
works.** `delvec`'s `--prefabs` defaults to `campaigns/prefabs`, which is where
the prefab library sits *when the compiler is run from the engine checkout*.
Standing here, the library is at `prefabs/`, and the default resolves to
nothing:

```
internal error: cannot read prefabs dir campaigns/prefabs: No such file or directory (os error 2)
```

That is exit 10 and it says `internal error`, so it reads as a broken compiler
and is not one. **`--prefabs` is a global option: it goes before the
subcommand.** Every `delvec` invocation on this page that reads a piece carries
it, already written in:

```sh
delvec --prefabs prefabs analyze campaigns/<id>
```

`fmt`, `schema` and `metrics` read no piece and do not need it; passing it
anyway is harmless. When in doubt, pass it.

### 4. The 1.21.11 client jar — the user picks how it arrives

Every picture in this pipeline is drawn with Minecraft's own textures. The jar
is in neither repository and this toolchain never redistributes it, so it has to
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

**A — download (the default).** There is **no client-jar pin in either
repository**, so there is no URL for this page to print and you must not invent
one. Resolve it instead, exactly the way
`"$DELVEWRIGHT_ENGINE/tools/check-patrol-types.py"` resolves the *server* jar:
Mojang's version manifest, the version `"$DELVEWRIGHT_ENGINE/versions.toml"` pins
under `[minecraft]`, and the bytes checked against the sha1 that same metadata
publishes. **That file is the ENGINE's**, not this repository's: this
repository's `versions.toml` names engine revisions and nothing about the game.

```sh
mkdir -p ~/.chunky/resources
python3 - <<'PY'
import hashlib, json, pathlib, sys, urllib.request
MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"
VERSION = "1.21.11"      # $DELVEWRIGHT_ENGINE/versions.toml, [minecraft] version
def get(url):
    with urllib.request.urlopen(url, timeout=180) as r:
        return r.read()
entry = next((v for v in json.loads(get(MANIFEST))["versions"]
              if v["id"] == VERSION), None)
if entry is None:
    sys.exit(f"{VERSION} is not in Mojang's manifest")
dl = json.loads(get(entry["url"]))["downloads"]["client"]
raw = get(dl["url"])
got = hashlib.sha1(raw).hexdigest()
if got != dl["sha1"]:
    sys.exit(f"sha1 {got} is not {dl['sha1']} — refusing")
out = pathlib.Path.home() / ".chunky" / "resources" / "minecraft.jar"
out.parent.mkdir(parents=True, exist_ok=True)   # or 31 MB dies on the last line
out.write_bytes(raw)
print(f"{out}  {len(raw)} bytes  sha1 {got}")
PY
```

Nothing there is a constant this page made up. The manifest URL is the one
`"$DELVEWRIGHT_ENGINE/tools/check-patrol-types.py"` and
`"$DELVEWRIGHT_ENGINE/tools/derive-client-langs.py"` both already carry, and the
version is the `"$DELVEWRIGHT_ENGINE/versions.toml"` pin. Point the same walk at
`downloads.server` instead and it reproduces that same file's committed
`server_jar_url` and `server_jar_sha1` exactly — which is how you know the walk
lands on the right game rather than merely on *a* jar. **The client half has no
committed pin to agree with**, so the sha1 checked above is Mojang's own,
verified on the bytes as they arrive: it proves the transfer and the version, and
nothing in either repository would notice if Mojang republished. Say that when
you report, rather than writing a pin of your own onto this page.

**B — copy, from the directory they named.**

```sh
mkdir -p ~/.chunky/resources
cp "<the directory they named>/versions/1.21.11/1.21.11.jar" ~/.chunky/resources/minecraft.jar
```

If they ask where it usually is: `~/Library/Application Support/minecraft` on
macOS, `~/.minecraft` on Linux. Offer those for them to confirm — do not `find`
the disk for them.

Either way the jar ends at `~/.chunky/resources/minecraft.jar`, the last of the
three paths every texture-reading tool tries, in this order: `--textures <jar>`
on the command, `$DELVEWRIGHT_CLIENT_JAR`, then that file. Chunky reads the same
jar when it renders at step 12, which is why one copy serves both.

Confirm the whole ladder answers, rather than discovering it at the review step:

```sh
mkdir -p .out
delvec --prefabs prefabs palette prefabs/hello-room.nbt -o .out/palette.json
```

`mkdir -p .out` is not decoration: `delvec … -o` writes the file and does **not**
create its parent, so a missing directory comes back as `DW0722 … No such file
or directory` at exit 3 — a write error that reads like a missing prefab.
(`delvec render` does create its output tree; the two are not consistent.)

### 5. Chunky — named here, fetched at step 12

Chunky renders every frame that has to *look* like Minecraft: the player-POV
review shots, the storybook art, the whole-map panorama. It is a separate
program — `delvec` writes the scene, Chunky renders it — and **Init does not
install it.** Step 12 does, at the moment the first frame is wanted, and step 14
reuses that install.

It is named here anyway, because Init's job is that nothing later stops on a
missing tool without having said so. A creator who is about to lose their
network needs to learn *now* that a download is still owed, not four hours
later at the review step. So Init confirms the source answers, and installs
nothing:

```sh
curl -sSfIL https://chunkyupdate.lemaik.de/ChunkyLauncher.jar -o /dev/null
```

Exit 0 means step 12 will be able to fetch it. A non-zero is not a stop — it
does not block a single authoring step — but say it out loud here, because it
*is* a stop at step 12.

**Chunky needs no Java 21.** Its launcher and its `--update snapshot` both run
under 17. The 21 in step 0 is the pinned game's own number and belongs to the
jar-reading checkers, never to Chunky.

### 6. Reference images — read the two paths before you spend anything

The design gate at step 4 is confirmed on **pictures of the design**, and there
are two ways to have them. Decide which you are on now, because one of them
needs a paid third-party account and the other needs nothing at all.

**Path A — the campaign already has an approved design.** Look first:

```sh
ls campaigns/<campaign-id>/design/
```

A campaign being re-made carries `design/README.md` (the approved names),
`design/concept/` (one image per scene) and, when the map was
designed as a whole, `design/reference/` (the map views, their prompts, their
style note and their sidecars), with `design.json` beside them at the campaign
root carrying one row per approved image and the sky it was drawn under. **If
that directory exists, the reference exists.** Read it, author from it, judge
against it, and present every later choice beside it. You need no image
provider, and Init is finished at step 7.
Do not re-draw an approved image; the approval is attached to the file that is
there.

**Path B — there is no approved design yet, and you are drawing one.**
`"$DELVEWRIGHT_ENGINE/tools/refimg.py"` draws reference images. It is stdlib
Python and needs nothing built, but it calls a **paid third-party image API**,
and three things must be in place before step 4 — establish them here, not at
the gate:

- a `[refimg]` section in `delvewright.local.toml` **at the engine checkout's
  root**, i.e. `"$DELVEWRIGHT_ENGINE/delvewright.local.toml"`. The tool reads
  that one file and takes no `--config`.
  **Expect it to be absent, and write it — that is this bullet's whole
  instruction.** `delvewright.local.toml*` is gitignored in the engine, so no
  clone carries one and the checkout Init step 2 just made has none. It is owed
  **once per checkout**, which means again the next time step 2 clones the
  engine, however many times you have set one up before; a machine that already
  ran this page still starts a new checkout without it. Where the section comes
  from: the committed `"$DELVEWRIGHT_ENGINE/delvewright.toml"`, which documents
  the shape and stays inert. Copy its commented `[refimg]` block out, strip the
  `# `, and fill in `provider`, `model`, `api_key_env` and the frame keys that
  provider takes. `.local` overrides the committed file section by section, so
  `[refimg]` is the only section this file needs;
- **the key never enters that file, and there is nothing to paste into it.**
  `api_key_env` is the NAME of an environment variable, read at call time and
  never stored or logged, and an inline `api_key =` is refused outright. So the
  key lives in **the environment your shell sees**, under the name that section
  gives.
  - **The name is yours to choose, so look before you write it.** You are
    writing the section, so it can name whatever variable the machine already
    has — and the block you copied out of `delvewright.toml` names an example,
    not a requirement. Read the environment for provider keys first; this
    prints NAMES and never values:

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
python3 "$DELVEWRIGHT_ENGINE/tools/refimg.py" --prompt "smoke test" --dry-run
```

Absent configuration exits 2 and says exactly what to add. A malformed one is a
hard error.

On path B this is a **hard prerequisite of the whole run**, not of one step. For
a site-plan campaign the map's own reference is the first thing written and
everything below is written against it; for an `areas[]` campaign the same wall
stands at step 4. Reaching either without a provider stops the line where
stopping is most expensive.

### 7. Python, only when an NPC needs a face of its own

Skip unless the design calls for a custom skin. The skin toolchain is a Python
package with dependencies, and `python3 --version` answering is not the same as
the package being importable — a missing skin is a build error, not a silent
skip. Make it a venv here so step 5 does not stop on it:

```sh
python3 -m venv .venv-skin
.venv-skin/bin/pip install -r "$DELVEWRIGHT_ENGINE/tools/skin/requirements.txt"
PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/skin" .venv-skin/bin/python -m delve_skin --help
```

The last line answering is the confirmation. Only the dependencies are
installed; the package itself is reached on `PYTHONPATH`, which leaves no build
artifacts in either repository. Use
`PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/skin" .venv-skin/bin/python -m delve_skin …`
wherever this page says `python -m delve_skin`.

### Init is finished when every one of these answers

Run each of them the way step 2c settled — through the environment file, if your
shell does not carry state. A line here answering wrongly because the environment
was lost is the failure this checklist exists to catch, and it is
indistinguishable from a missing tool unless you carry the file.

```sh
git lfs version                          # the prefab library materialises
file prefabs/hello-room.nbt              # gzip compressed data, not ASCII text
java -version                            # 21 or newer — below that, go back to step 0's enumeration
echo "$DELVEWRIGHT_ENGINE"               # the engine checkout, non-empty
delvec --version                         # the compiler, and the dsl number
delvec grammar list                      # the rule library, through the one binary
delvec render fidelity-gate              # the GPU arms of that same binary
mkdir -p .out && delvec --prefabs prefabs palette prefabs/hello-room.nbt \
    -o .out/palette.json                 # the client jar
curl -sSfIL https://chunkyupdate.lemaik.de/ChunkyLauncher.jar \
    -o /dev/null                         # Chunky's source — fetched at step 12
docker info                              # the ladder and the play server
```

`delvec prefab`, `delvec schem` and `delvec harvest` are subcommands of the
binary the two lines above already exercised, so there is nothing separate to
check for them — `delvec --version` answering is the whole check.

Path B adds
`python3 "$DELVEWRIGHT_ENGINE/tools/refimg.py" --prompt "smoke test" --dry-run`;
a custom skin adds the `delve_skin --help` line from step 7.

Nothing on this page needs `pytest`. This repository's own checks are stdlib
`unittest` and run with nothing installed:
`python3 -m unittest discover -s tools/tests -t tools/tests`.

### Where output goes, and the one place it cannot go

`.out/` and `out/` are gitignored here; put scratch output there. **One tree is
different: the build output the machine ladder boots.** Three ladder entries can
boot a tree anywhere, this repository's `.out/` included: `bot-run.sh` and
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

So every step below that names a build output writes to
`"$DELVEWRIGHT_ENGINE/validation/delve-output"` — the one tree every path can
boot. It is gitignored there and no campaign file goes near it.

## Which placement model

**A campaign has exactly one placement authority, and choosing is the first
thing you do.** Declaring both is refused by name (`DW0839`), so this is not a
decision you can defer or revisit cheaply.

**`areas[]`** — pieces from the prefab library, seated on a fixed stride.
Take it when the campaign is a small number of rooms the library already has.

**A site plan** — the whole map's design of record, from which the engine
*derives* the geometry. Take it whenever the map is the point: when the brief
describes a place with a shape, when the party has to walk somewhere and the
walking is the content, when there is no prefab that is the building the story
is about.

The two branches differ only at step 2 and step 13. Everything else — quests,
NPCs, dialogue, gates, shortcuts, the whole ladder — is identical and does not
know the difference.

**Before you choose, look at a real one.** `delvec metrics --gym <dir>` writes
a complete, buildable, nine-document site-plan campaign in one command, with
nothing to configure:

```sh
delvec metrics --gym .out/gym
delvec --prefabs prefabs build .out/gym -o .out/gym-out
```

Those nine documents are the worked example this page does not print: a real
`world.json`, `npcs.json`, `classes.json`, `quest-plan.json`, `quests.json`,
`dialogue.json`, `geometry-brief.json`, `layout-graph.json` and
`site-plan.json`, all in the shape the schema actually accepts. Read them
whenever a field on this page is not clear. It is also the campaign that
calibrates the metrics table, so its second job is the one described under
*The site plan* below.

### The test, and how to run it

"There is no prefab that is the building the story is about" is the whole
routing question, and it is not answered by reading pool names: a pool called
`vertical-keep` is an entry hall, corridors and terminal rooms, which is the
*inside* of a keep and not a keep. So ask the question in the form that has an
answer:

> **Does the thing the delve is named after have an exterior silhouette the
> player is meant to read** — a shape they stand outside of and see against the
> sky? A pool of interiors can be the inside of a building. It can never be the
> building.

Then look, rather than deciding from memory. One command draws the whole shipped
library as one self-contained page — orbit, plan, cutaway, and a player eye at
every anchor:

```sh
delvec --prefabs prefabs viewer prefabs -o .out/library.html
```

Open it and put one question to the thing the delve is named after. **Is it on
that page, as a shape?** If it is not, and the story asks the player to look at
it from outside, `areas[]` is over: take the site plan, where the geometry is
derived from your own plan and there is no prefab left to be missing. **If
instead it is a place the party walks through**, and the members of some pool
are its rooms, `areas[]` holds — say so in `GENERATION.md` in one line, so the
reading is visible to the next round rather than implied.

Take the site plan on a tie. It costs four more documents and the map's own
reference views; taking `areas[]` wrongly costs step 2 over again, and the page
cannot get you back there from step 5.

---

# The steps

## 1. The workspace, and the documents you are going to write

Create `campaigns/<campaign-id>/`. Everything of the campaign lives
there — the documents, the design record, the generation record, the storybook.
Build output goes beside them and is ignored by git there.

**`campaigns/` is the only place a campaign lives, and a demo level is a
campaign.** A level off the engine's `docs/demo-levels.md` queue — one mechanic
in the spotlight, ten to twenty minutes, minimum cast — is authored through this
page like any other and goes to `campaigns/<id>/`. It has to: a delve is what
somebody plays, and a campaign is the directory under `campaigns/` that carries
`world.json`. **`demos/` is not a second home for one.** What lives there is a
demonstration of a *generation-time* surface — a grammar program, the piece it
exports, its reports, its refusal transcripts — and it carries no campaign
document because no delve is built from it. One place each, and no pointer file
in either.

**The document names, all of them.** `delvec validate` reads a whole campaign
and refuses one that is missing any of them with `DW0874`, which names **every**
document that is absent in a single run, gives the whole required set, and
prints what a stub looks like — so you never have to guess the names. Six are
always required:

```
world.json  npcs.json  classes.json  quest-plan.json  quests.json  dialogue.json
```

Six more are conditional, and every one of them is a real campaign document
with its own schema:

| document | when |
|---|---|
| `geometry-brief.json` · `layout-graph.json` · `site-plan.json` | a site-plan campaign — step 2B; a site-plan campaign has no `areas[]` |
| `design.json` | step 4, the moment the user approves the reference images — one row per approved picture, and the only home the approved sky has |
| `detail-plan.json` | step 13, optional, and only after the blockout has been walked |
| `world-edits.json` | whenever the map editor was used to fix terrain — see *Reference: tools by symptom* |

**Every document has the same envelope**, and it is four keys:

```json
{
  "dsl_version": "0.22.0",
  "campaign_id": "the-weighbridge",
  "stage": "world",
  "content": { }
}
```

`stage` is the document's own name — `world`, `npcs`, `classes`, `quest-plan`,
`quests`, `dialogue`, `world-edits`, `geometry-brief`, `layout-graph`,
`site-plan`, `detail-plan`. `content` is everything else.

**What number goes in `dsl_version`: the one `delvec --version` printed after
`dsl`.** A new campaign writes the engine's current number on every document.
The per-feature minimums this page states elsewhere ("needs `dsl_version`
0.10.0 on the quests stage") are the *floor* a surface became available at —
they exist so an old campaign keeps compiling unchanged, and a new campaign is
already above all of them. Write the current number and none of those sentences
applies to you.

**Get the shape from the engine, never from memory.** Before writing a
document:

```sh
delvec schema --stage world          # or npcs, quests, site-plan, walk-record, …
delvec schema --stage all            # every document at once
```

The schema is the authority on a document's form. Where anything else disagrees
with it, the schema is what parses. If you want a filled-in example rather than
a schema, `delvec metrics --gym .out/gym` writes nine complete documents that
build (see *Which placement model*).

Two more files belong to the campaign and are prose, not documents the compiler
reads:

- **`DESIGN.md`** — the authoritative design record: layout, dramaturgy beats,
  the branch/ending table. Every later round is judged against it.
- **`GENERATION.md`** — the campaign's own decisions, in your words as their
  author: what the brief pinned down and what you invented, the `dsl_version`,
  this campaign's **posture note** (see *Reference: writing craft* §B), and later
  the findings ledger and the chronicle citation table. **Not the prompt
  verbatim and not a date.** Every artifact in this repository is
  English-first and carries no personal information, no verbatim personal
  speech, and no record of who decided something or when — the engine
  constitution's Conventions name these generation logs explicitly. So write the
  constraint ("the brief pinned the tide as a story beat, never a clock"), never
  the sentence somebody typed; write the decision, never its date or its author.
  The record is for whoever authors the next round of this campaign, and a date
  tells them nothing a `git log` does not.

**Commit the campaign onto its own `campaign/<campaign-id>` branch**, as soon
as the documents are on disk — not onto `main`, and not held back until
everything is green. A campaign is in progress until somebody has walked it, and
everything of it lands on that branch: the documents, the design record, any
prefab it needs, the generation record, the storybook. Sort a file by which
artifact it belongs to: if abandoning the campaign would delete it, it is the
campaign. Conventional message; do not push unless asked. The documents are the
artifact of record: the delve must rebuild byte-identically from them with no
model in the loop.

**A campaign correctly stopped at the design gate does not build yet, and that
is not a defect.** Nothing in CI compiles it for you: the campaign is built by
you, with the engine `versions.toml` `[engine].authoring_ref` names, and it
builds before the pull request that merges it to `main` — that is the run that
decides whether it ships.

**While you are authoring incrementally, stub the later documents** and mark the
stubs clearly, so `delvec validate` can run at all. A stub is the envelope plus
a `content` carrying only what its schema requires — `DW0874` prints the recipe.
Two things to expect while stubbing:

- **`quest-plan.json` is the one document the recipe cannot be followed for
  literally.** Its schema requires `finale` as well as `quests`, and `finale`
  has to name a member of `quests`, so an empty `quests` is `DW0131`. Its
  smallest stub is one planned quest with `finale` naming it.
- **Stubs owe things, and the diagnostics name them**: every stage-2 NPC needs
  a stage-6 dialogue tree (`DW0152`), a declared language needs a covering
  sidecar (`DW0180`), and a planned quest with no stage-5 expansion is
  `DW0150` — which is the ordinary state between step 3 and step 5 and is
  discussed there.

**`world.json` is written here, before placement**, because seven of its eight
required fields are not about placement at all: `title`, `theme`, `premise`,
`seed`, `target_minutes`, `time` and `weather`. Run `delvec schema --stage world`
and write them now. The eighth is `areas`, and which of the two ways it is filled
is step 2's whole question: `areas[]` on path 2A, and **empty** on a site-plan
campaign, where the plan is the placement authority and declaring both is
`DW0839`.

**`time` and `weather` are the delve's hour, and the engine will not choose it
for you.** `time` takes `day`, `noon`, `dusk`, `night`, `midnight` or `dawn`
(`sunrise` is accepted as a synonym of `dawn`); `weather` takes `clear`, `rain`
or `thunder`. Neither has a default: "this delve is played at noon" is a design
decision, not a mechanism, so a `world.json` that omits either is `DW0100` like
any other missing required field. `DW0874`'s stub recipe is "its envelope, and a
`content` carrying only the fields its schema requires", so a stubbed
`world.json` carries both from the moment it is stubbed. Both are
dimension-global and frozen by environment sealing, so the state declared here is
the state for the whole delve.

**The order on this page runs backwards from the decision, and that is the
trap.** Step 4 confirms the design on concept art, and art has an hour in it — a
night sea, a storm, first light on a headland. So write the hour the brief and
the design are set in **here**, and expect step 4 to hold the pictures to it: at
approval each image's row in `design.json` states the sky it was drawn under, and
from step 5 onward `DW0890` refuses a campaign whose reachable skies and whose
approved rows are not the same two sets. Two measurements also key off the
declared hour: the dark-cell proof measures under the **darkest reachable**
`(time, weather)` sky, so a space lit only by the sky is judged at the night floor
once a night hour is declared (`DW0210`, and the lighting contract at 2A); and
`DW0496`, the daylight-burning refusal, stands only while the hour is a pinned
clear daytime one — so declaring `night` is a design decision that also switches
that gate off, which is why *Reference: authoring pitfalls* forbids reaching for
the hour to save a mob.

**Some of `world.json`'s optional fields commit something you are not writing
yet.** They all sit in this document at this step, and the ones below are
refused, unbound or silently contradicted many steps later, so decide each one
here with the thing it obliges in front of you. The rest are described where
they are used: `languages` in *Reference: other languages*, `difficulty` in
*Reference: authoring pitfalls*, and `outro` — the delve's closing line, the
last player-visible sentence of the run, absent = the finale quest's `goal`.

- **`min_players`.** Absent = 1. Declaring `n ≥ 2` says the delve *requires* n
  bodies, and that is a claim about the QUEST GRAPH: the analyzer demands an
  objective with `n` `after` arms in `n` places — parallel work the party
  divides — and a delve that is one serial chain a single player walks is
  `DW0358` at **step 7**, after `quests.json` is written. So decide the number
  with the quest plan at step 3, not with the world at step 1: a brief that says
  "for two players" is a brief that has asked for a mechanism, and the design
  gains one or the number comes down.
- **`horizon`.** Absent = `void`, and that is the right answer unless the
  surround is part of the design. `valley` rings the map in generated mountain;
  both of those keep the area datum where every piece was authored for it.
  **`ocean` is different and it is paired with a piece set**: it swaps in a
  superflat sea at y=62 and DROPS the area datum to y=60 so a piece meets the
  water at its own declared `waterline_y`. Only pieces carrying that field are
  authored for it, and the invariant that proves the meeting (`DW0344`) examines
  only those — a piece without it is not checked and is not lifted. So the
  question is not "does the library have sea pieces", it is **how much of the
  pool I am about to place is authored for the sea**. Ask it, per pool, before
  you take `ocean`:

  ```sh
  python3 - <<'EOF'
  import json
  pools = json.load(open("prefabs/pools.json"))["pools"]
  for pid, p in sorted(pools.items()):
      names = [m["prefab"].split("/", 1)[1] for m in p["members"]]
      have = [n for n in names
              if json.load(open(f"prefabs/{n}.json")).get("waterline_y") is not None]
      print(f"{pid}: {len(have)} of {len(names)} member(s) declare waterline_y  {sorted(have)}")
  EOF
  ```

  **Read what it prints, and do not carry a number off this page** — the
  library is a separate artifact on its own cadence, and a count written here
  describes whichever version of it somebody last looked at. Zero for your pool
  means `ocean` gives you a dropped datum, an invariant examining zero pieces
  and no lever to lift anything clear: that campaign takes `void` and puts the
  sea in the fiction. Short of every member means the unlisted ones stand in the
  water with nothing checking them, which is exactly the silence `DW0344`
  reports about itself.
- **`boundary`, which `ocean` obliges.** Absent = no boundary. It declares the
  playable region: the compiler derives one from the placed geometry plus a
  `margin` of blocks on every side (default 16, range `0..=64`), and a per-second
  clock returns anyone who leaves it to their last checkpoint, with an optional
  `message` on the actionbar. **`horizon: ocean` with no `boundary` is
  `DW0320`** — an infinite swimmable sea with no return rule — so those two are
  written together or neither is written.

## 2. Placement — where everything is

### 2A. `areas[]`

`world.json`'s `content.areas` seats pieces from the prefab library. Prefer
`prefab_pool` for real layouts; read `prefabs/pools.json` and the
per-prefab metadata for the pools, anchors and lighting profiles available.
Respect the lighting contract — darkness only as declared design, with a
mitigation the quest graph provides.

**Two rules about multiple areas, and both are about pieces rather than about
your story.** Areas sit 256 blocks apart across void with no walkable link, so
crossing between them is not a walk — the compiler emits a one-way teleport on
the objective that crosses, which is what makes "the boulder seals the cave" a
fact about the geometry rather than an assertion. That crossing is emitted only
under two conditions:

1. **The campaign's first beat plays in the area the party starts in.** A
   crossing rides on an objective completing, and at the spawn nothing has
   completed yet — so there is no beat to hang the first crossing on. A delve
   whose opening move would be a teleport out of its own spawn area has put its
   spawn in the wrong area; move the spawn, or put a beat in the spawn area
   first.

2. **Every area a beat crosses into declares an entry point.** That is an
   anchor carrying `"role": "entry"` in the piece's metadata, or — for pieces
   admitted before the role existed — an anchor literally named `spawn` or
   `entry`. **Very few pieces have one**, and a pool usually holds exactly one
   that does, so a multi-area campaign is a constraint on which piece each area
   may bind rather than a free narrative move. How few is a fact about the
   library and not about this page, so read it rather than taking a number from
   here — the library is a separate artifact on its own cadence:

```sh
python3 - <<'EOF'
import json, glob, os
n = 0
for f in sorted(glob.glob("prefabs/*.json")):
    if os.path.basename(f) == "pools.json": continue
    n += 1
    a = json.load(open(f)).get("anchors") or {}
    if any(v.get("role") == "entry" for v in a.values()) or {"spawn","entry"} & set(a):
        print(os.path.basename(f)[:-5])
print(f"-- out of {n} piece(s) in the library")
EOF
```

A crossing that was never emitted is not a quiet difference — it is a delve the
party cannot finish. See *Reference: when something goes red* for what it looks
like.

**A pool area guarantees its `entry`-role member's anchors, plus whatever this
campaign REQUIRES.** An area declaring `pieces: {min: 3, max: 4}` over a
thirteen-member pool seats a subset: the `entry` member at the area origin on
every draw, one carrier for each anchor the campaign requires the solver to
guarantee, and `connector` fillers drawn from the seed. A `room` or `terminal`
member nothing requires is never seated, and an anchor it declares is not in the
built world.

Ask the engine, before the story is shaped around something that is not there —
it reads the library and needs no campaign, so you can ask it at this step:

```sh
delvec --prefabs prefabs prefab anchors            # every pool
delvec --prefabs prefabs prefab anchors --pool pool/cave-shore
```

It prints, per pool, the member count, the `entry` member, the anchors that
member declares — the unconditional guarantee — and every other name with the
carrier it would have to arrive on and the role that decides when the layout
seats it.

**Read what it prints; do not carry a number off this page.**

**Three ways to design against that, and the second is the one this page used to
omit.** Design against the entry member's anchors. Or *require* the anchor you
want, at a site the solver must honour — an objective, an NPC stand, a wave
spawn, a lane waypoint, or an anchor-bearing effect in that area — which forces
its carrier to be seated, and everything else that carrier declares with it. Or
bind a single `prefab` instead of a `prefab_pool`, in which case every anchor it
declares is yours.

**When you get it wrong, `validate` says so, not the build.** `DW0889`
(advisory) names every anchor your documents carry that the area does not
guarantee, with the piece it lives on and why the layout may not seat it. It is
a warning because the filler draw may well seat the piece — but if it did not,
`DW0360` fails the build at step 8. `DW0498` is a different line about the same
area: it reports a pool that seated one anchor-bearing piece *twice*, which
needs the settled draw and so still arrives at step 8.

**One more piece fact worth knowing before you place anything.** An anchor name
is unique per *area*, so binding the same prefab to two areas makes every anchor
it declares ambiguous (`DW0857`); the fix available to you is a **different
piece for one of the two areas**, not renaming an anchor in the shared library.
A `DW0498` repeat is a hard `DW0305` the moment an objective, NPC stand, gate or
wave spawn hangs on one of the names it made ambiguous.

**When the library has no piece the design needs, decide which of two things you
are looking at.** Neither answer is "make a prefab now".

- **The missing piece is the building the story is about** — the keep, the mill,
  the tower the delve is named after. Then `areas[]` was never the right
  placement model: go back to *Which placement model* and take the site plan,
  where the geometry is derived from your own plan and there is no prefab left to
  be missing. This is the one decision that cannot be changed later without
  redoing step 2, so change it here and not at step 5.
- **The missing piece is a room `areas[]` still wants** — a second area's entry
  point, a room whose chest a `collect` has to adopt, a shape the pools do not
  hold. Then you owe a piece, and you build it **after the design gate at step 4
  and before step 5**, entering *Reference: when the prefab library has no piece
  you need* at that point. That is the section's second entrance; step 13.3 is
  the other.

**After the gate, for the gate's own reason.** Step 4 confirms the design on
concept art drawn *before any prefab exists*; a piece built first and rendered
for the gate is a build being approved as a design, which is the inversion step
4 names. And before step 5, because step 5 binds anchors and the anchors are the
piece's own — there is no `cast` block to write against a piece that has not
declared its marks.

**Until then, do not carry a name for a piece that does not exist.** An
`areas[]` entry naming a piece the library does not have is `DW0856`, and the
refusal is not the expensive part: an area whose piece is absent contributes no
anchor set at all, so every per-area anchor proof over it is SKIPPED rather than
failed, and steps 3 and 4 then read quieter than they are. Write the area entry
when the piece is admitted; at the gate, say which pieces the design still owes
and what each one is for.

### 2B. The site plan

Three documents, in this order, each the input the next one needs. **The order
is the only order that compiles** — there is no blockout document and nothing to
author early, so no later document can reach green first.

**Before any of them: the whole map gets a reference of its own.** A composition
written without one is free invention with no criterion. On path A of Init step
6 it is already in `design/reference/` — read it. On path B, draw it now; the
form and the commands are in *Reference: drawing the map's reference*.

1. **`geometry-brief.json`** — the whole's written design reduced to *numbers*:
   `facts[]` of `{id, value, unit?, note}`. A fact is a number with a name.
   Write the numbers the design actually commits to — how far across the site
   is, how tall the thing the campaign is named after stands, how far the
   approach runs. Reference imagery is style authority and never dimensional
   authority: an identity binds to a number, never to a picture.

2. **`layout-graph.json`** — the space as a graph, **before any coordinate
   exists**. `nodes[]` are places (`{id, intent, size_class | way_class, note?}`);
   `edges[]` are connections (`walk | stair | drop | barred | vision`, with
   `gating`, `one_way`, `shortcut`, `opens_from`). Plus `entry`, `goal`, an
   authored `critical_path[]`, and `beats[]` binding every place-bound quest beat
   to the node it happens in.
   - **A place is classified exactly once, and there are two vocabularies.**
     `size_class` is a rung of the size ladder and bounds the footprint on BOTH
     horizontal axes — that is what a room, a hall or an arena is. `way_class` is
     for a place bounded in one axis and free in the other: a road, a causeway, a
     corridor, a duct. Write a way when the shape is a ROUTE — a cut ledge one
     body wide climbing a whole cliff face is 4 by 90, and no rung admits that,
     because a class spanning 4..90 on an axis has stopped classifying. Declaring
     both, or neither, is `DW0875`.
     - A way class bounds the **cross-section** only. There is no length
       standard and there never will be one: the run is your plan's business, and
       all the engine asks is that the box's longer extent EXCEED the class's
       widest cross-section (`DW0832`). A square box can never be a way, which is
       the point — it is a room.
   - `size_class`, `way_class` and every seam `opening` name an entry in the
     **metrics table**. `delvec metrics` prints it — 341 lines of JSON on stdout,
     and a summary plus its binding counts on stderr, so read them separately:
     `delvec metrics > table.json`. **Write the BARE name** — `"size_class":
     "hall"`, `"way_class": "road"`, `"opening": "arch"` — and a name the table
     does not define is `DW0812`, which lists the defined set for that kind.
     The table's own JSON keys carry a namespace the document never writes:
     the entry is at `building["size-class.hall"]`, and the compiler puts the
     `size-class.` / `way-class.` / `opening.` in front of what you wrote to
     look it up, so `"size_class": "size-class.hall"` is `DW0812` and not a
     synonym. Strip the prefix when you read a key out of `table.json`.
   - The graph is checked as a graph, cheaply, before geometry exists to make
     it expensive: every place reachable under gating (`DW0816`), the authored
     critical path actually a quest-legal path (`DW0817`), no one-way edge that
     strands a body (`DW0819`), no "shortcut" that closes no loop (`DW0820`).
   - `intent` is a free label no check keys on — write what the place is *for*.

3. **`site-plan.json`** — the geometric embedding of that graph. `region` (the
   whole map's one box, in world coordinates), `datums` (named ground planes),
   **one `boxes[]` entry per node**, **one `seams[]` entry per traversal edge**,
   `volumes[]` for mass the whole owns (the mountain a cave is inside), a
   `sightlines[]` entry per `vision` edge, optional `views[]` for the walk to
   judge the silhouette from, and `identities[]` binding the plan back to the
   brief's facts.
   - **Extent flows down.** The region comes from the brief and the boxes
     partition it. A box is never grounds to grow the region (`DW0826`): shrink
     or move the box, or change the brief's fact and re-derive, visibly.
   - **A box says what it is; a seam says where two boxes meet; the engine
     derives the grid.** A box is `{node, extent, floor, ceiling}` — `extent`
     is the interior a body stands in, on the kit grid. Write `min` on **one**
     box (the entry) to say where the whole stands in the region; write it on
     no other box unless you mean to assert its corner, because a pin that
     disagrees with the seams is `DW0883`.
   - **A seam is `{edge, face, opening | contact, at?, meets?, stair_in?}`.**
     `face` is the side of the edge's `a` box the crossing is on. The engine
     puts `b` one cell beyond that face — the wall — and the crossing in the
     **middle** of both faces. When the door is not in the middle, say where:
     `at` is cells from `a`'s low corner along the face, `meets` cells from
     `b`'s (an integer on a wall face; `[dx, dz]` through a floor or ceiling).
     A seam that closes a loop places nothing: both boxes already stand, and
     the engine checks that its cells are the same seen from either side
     (`DW0828`).
   - **Nothing about a sill, a rise or a wall thickness is written anywhere.**
     The sill is the higher of the two floors; the rise is their difference;
     the wall is the one cell the packing leaves between neighbours.
   - **A seam is one of two kinds, and both or neither is `DW0876`.** Write
     `opening` for a PORTAL — a doorway at a standard the table names, whose
     every cell the built world must have open (`DW0829`, `DW0836`). Write
     `contact` for a FRONT — two places that simply meet, along a span of the
     face they share:
     `{"edge": …, "face": …, "at": <cells>, "contact": {"extent": [u, v]}}` —
     `at` is the seam offset above (one integer on a wall face, `[dx, dz]`
     through a floor or ceiling) and `extent` is the span `[u, v]` on the
     face's own two in-plane axes, anchored there. Omit `extent` to run the
     span from `at` to the far edge of the face on both axes.
     - A contact means **continuous ground**: no wall along the span, no frame,
       no sill, and crossing legitimate anywhere along it the step rule admits.
       Do not reach for a wide `opening` to spell a front — there is no standard
       the width of your courtyard and there is not going to be one, because a
       front's width is a fact of your two boxes and a table that enumerated it
       would gain an entry per campaign.
     - A contact must be **wider than the broadest standard opening**. Anything
       narrower could have been a portal, and is refused as one (`DW0876`).
     - A contact carries `walk` or `drop` only. A rim falling to a lower court is
       a real broad hand-off; a stair, a barred door and a sightline are not
       things a front can be.
     - The engine MEASURES which columns of the span a body crosses, over the
       built bytes, and refuses a front nothing can cross (`DW0877`). Saying the
       face is fine is not a declaration it accepts.
   - A stair's rise is not authored — it is the difference between the two
     floors the plan already chose — and its `stair_in` names which box pays for
     the run (`DW0830`). Treads rise off a walk plane, so `stair_in` is always
     the LOWER place.
     - **The run is spent on ONE axis, and the seam's face picks which.** For
       a seam on a wall face the host affords its extent along that face's
       normal — an east or west face spends x, a north or south face spends z
       — so a 4 × 40 host with the stair on its east face affords **4**, and
       `DW0830` says so in those words ("affords 4 on x"). Only a seam
       through a floor or ceiling gets the host's longer horizontal axis.
       Give the host its length on the axis the face points along, or host
       the stair in the other place.
   - `delvec validate` prints every box's derived corner and which seam placed
     it. Read your `volumes[]`, `sightlines[]` and `views[]` against that
     output — they are still world coordinates.

**A site-plan campaign has one area, `area/site`.** Quests, NPCs and waves name
it; `world.json`'s `areas[]` is empty, and declaring both authorities is
`DW0839`.

**The anchors are synthesized, and these are the only names there are** — there
are no prefabs to read anchor names out of:

| Anchor | Where |
| --- | --- |
| `spawn` | the entry node |
| `anchor/node-<place>` | the floor centre of each place — where NPCs, waves and `reach-anchor` objectives go |
| `anchor/seam-<edge>` | the gate region over a `barred` seam: what `open-gate` or a `shortcut` names |
| `anchor/unlock-<edge>` | the far-side affordance of a one-sided `barred` seam, where a `shortcut`'s `unlock` stands. Present only when `opens_from` is `a` or `b` |

`<place>` and `<edge>` are the part of the id after the `/` — `node/near-hall`
becomes `anchor/node-near-hall`. Every barred way must be opened by something
naming its own seam (`DW0818`), and a sealed door a player can push on owes an
answer (`DW0429`) — a `use` trigger anchored on the gate.

**The numbers the whole thing is built to are provisional** until the metrics
gym has been walked, and every build says so (`DW0813`). That is the gym's
second job: `delvec metrics --gym <dir>` builds a site-plan campaign out of the
table itself — one place per rung of the size-class ladder at each of its
bounds, one way per class at each width the kit grid lets a plan draw it at,
every standard opening, both stair pitches, a designed fall at the drop policy's
cap. It reports what the table defines that it could not instantiate
(`DW0840`) — read that line, not just the green.

## 3. The story documents — `npcs`, `classes`, `quest-plan`

Write them in that order; each conditions the next. Get each one's shape from
`delvec schema --stage <name>` first, and run `delvec --prefabs prefabs validate <campaign-dir>`
after each, fixing by diagnostic code. **Three failed repairs on the same code
means stop and look at the design**, not at the syntax.

**That loop does not end on a green run, and it is not supposed to** — read
*Where step 3 ends*, below, before you start it. Once the quest plan is on disk
the campaign carries refusals that are correct and that only step 5 can clear,
and looping against them is the one way to spend an afternoon here.

- **NPCs**: personas per the schema — `archetype`, `speech_style` and
  `motivation` are required, and step 5 honours them in every line.
- **Classes**: pre-provided gear, no grind. If the campaign places a bonfire,
  **every class kit must declare a flask** (`DW0476`) — see *Reference:
  authoring pitfalls*.
- **Quest plan**: acts, dependencies, which area each quest belongs to,
  mandatory-only quests, paced to `target_minutes`.
- **Declare every story fork here.** If a choice forks who lives, where the
  party ends up, or which ending plays, it is a `branch_points` entry:
  `{id, opens_at, forks_on:[flags], branches:[{id, flags, leads_to}]}`.
  `leads_to` is one field — a `quest/…` the branches converge at, or an
  `ending/…` this branch runs to; the id prefix says which. Name each ending on
  the `campaign-complete` that fires it (`"ending": "ending/<slug>"`). A flag
  that gates casts, staging or quest structure and is not set on every
  playthrough must belong to a declared point (`DW0480`). Every declared branch
  must reach an ending (`DW0482`) and must be exclusive: no sibling's flag may
  be producible on it (`DW0484`).

### Where step 3 ends

**The campaign does not validate when this step is done, and no amount of
looping will make it.** The quest plan names quests, stage 5 is step 5, and the
compiler is right to refuse a campaign that plans a quest nothing expands. So
the finish line for this step is not a green run — it is a run whose every
remaining error the rule below accounts for.

Run `delvec --prefabs prefabs validate <campaign-dir>` and sort the output by
**one rule, which is the whole of this step**:

> A refusal whose message names something only `quests.json` or `dialogue.json`
> can supply is the step-3 state, and is not a repair you owe. Every other code
> is a repair you owe now.

Those two documents are step 5, so nothing you write here can produce what they
hold. **Read the message, not the code.** Each of these says in its own words
what is missing and where it comes from — a flag no `set-flag` produces, an
ending no `campaign-complete` declares, a body no `spawn-npc` summons. Every one
of them is a stage-5 effect or a stage-6 dialogue option, every one of them
clears at step 5, and looping on any of them here is the way to lose an
afternoon.

The codes below are the **instances a campaign hits today**, not the rule. A
code that is not in this list is judged by the rule above — by what its message
says is missing — never by its absence here.

**`DW0890` is not one of them and cannot be**, which is worth knowing before you
meet it: the design record does not exist until step 4 approves one, so at this
step `validate` prints `0 reference(s) recorded over 0 image file(s)` and says
nothing further. It becomes live at step 5, on the first `validate` after the
gate, and it is a repair you owe there like any other.

| code | what it is saying | what to do |
|---|---|---|
| `DW0150` | the plan is written and stage 5 is not. **One** diagnostic naming every planned quest — it says so itself: *an authoring state, not a fault*. | nothing. It clears at step 5, for every quest at once. |
| `DW0818` | site-plan campaigns only: every layout-graph beat and quest-gated way names a quest stage 5 has not written. The message says so and points at `DW0150`. | nothing. Same state, same clearing. |
| `DW0152` | one per NPC with no dialogue tree. | **stubbable, unlike the rest.** A tree of one node with an empty `options` is accepted and raises nothing new, so clear it here if you want a shorter list to read at step 4; leaving it is equally fine. Step 5 replaces the stub either way. |
| `DW0172` | an objective gate, or a `branch_points` `forks_on`, names a flag no `set-flag` produces — and every `set-flag` lives in a stage-5 quest effect or a stage-6 dialogue option. | nothing, once you have checked the flag is one step 5 is going to set. A flag no beat in the plan is ever going to produce is a plan defect and you fix it here. |
| `DW0112` | a branch's `leads_to` names an `ending/…` no `campaign-complete` declares, and `campaign-complete` is a stage-5 effect. | nothing. **But `DW0112` is the generic unresolved-reference code** — on any other path (an npc's `area`, a `datum`, a trigger's target) it is a genuinely broken reference and you fix it now. The path in the message is what tells the two apart. |
| `DW0482` | a declared branch reaches no ending, because nothing sets its flags yet. The same absence `DW0172` names, one diagnostic per branch. | nothing. It clears with the `set-flag`s at step 5. |
| `DW0197` | a `deferred: true` NPC that no `spawn-npc` summons, and `spawn-npc` is a stage-5 effect. | nothing, if the character is meant to walk in at a beat step 5 will write. If it was never meant to be deferred, drop `deferred` now — that is a stage-2 repair, not a stage-5 one. |
| `DW0816` | site-plan campaigns only: a place the graph's closure never reaches from `entry` under gating and one-way direction. The message names the place, the nearest place a body can stand, and how many of the graph's places are reachable at all. | **read the caveat before you decide.** Where the graph gates on a flag and stage 5 declares no quests, the message says so and points at `DW0150` — the way may be one a stage-5 `set-flag` opens, and the check is the same as `DW0172`'s: is a beat in the plan going to produce that flag? Where the message carries no such caveat, the place is unreached for a reason the mission has nothing to do with, and it is a graph repair you owe now. |
| `DW0817` | site-plan campaigns only: the authored `critical_path[]` does not hold — four faults under one code. It does not run `entry` → `goal`; a step names two places no connection joins, or joins them the other way; a step crosses a connection not open yet at that point in the walk; or it never visits a place where a beat of the mandatory quest spine happens. | **three of the four are yours now.** Only the *not-open-yet* fault carries the unwritten-mission caveat, and only when that connection waits on a flag — the other three are judgements about the graph by itself, which an absent mission has nothing to say about, so they are repairs to the graph or the path here. Read which fault the message names. |

Measured on a twenty-four-place site-plan campaign at the end of this step: 42
errors over seven of those codes — `DW0818` ×28, `DW0152` ×6, `DW0482` ×2,
`DW0172` ×2, `DW0112` ×2, `DW0197` ×1, `DW0150` ×1 — plus the two ordinary
warnings (`DW0813`, `DW0822`) that stand on every site-plan campaign. A list
that long is the expected shape here, not evidence that something went wrong.

**Do not try to clear `DW0150` by writing empty stage-5 quests.** It is the
obvious move and it is strictly worse: a quest carrying only what the schema
requires is refused again by `DW0481`, because every quest must say what it does
to the story, and by `DW0460` once for every NPC live in it. Measured on a
five-quest plan, writing the five minimal expansions took the campaign from 5
errors to 15. `DW0150` says this in its own message; believe it.

**And the "three failed repairs" rule does not fire on these**, because there is
nothing to repair. It fires on a code you have genuinely tried three times to
fix.

If someone else gave you the brief, show them a 3–6 line summary of each
document — the summary, not the JSON — and wait, unless they asked for an
uninterrupted run.

## 4. The design gate — STOP, the user says yes

**Stop here, end your turn, and do not begin step 5 until the user has said
yes.** Steps 1–3 settle
*what the delve is*; step 5 is where the expensive authoring happens, and every
problem this gate would have caught gets paid for twice once it is written.

You arrive here holding a campaign that does not validate, carrying only
refusals whose messages name something `quests.json` or `dialogue.json` will
supply — the rule *Where step 3 ends* states, whether or not the code is one it
lists. That is the correct state to be at this gate in, and it is not something
to mention, apologise for, or try to fix before asking. What is being confirmed
is the design.

Deliver a **walkthrough of the whole design**: the complete story, every scene's
design, and each scene carrying **both a near view and a far view**. Near view is
the scene as a player stands in it; far view is the same scene in its
surroundings, so staging and sightlines read. Not a document with pictures in it
— a visual walkthrough, in the medium the review happens in. A design the
reviewer cannot see is a design they cannot approve.

**A confirmation is an explicit yes, not the absence of an objection**, and it
is the user's — never yours, and never a reviewer you invented. If you were told
to run the whole thing uninterrupted, this gate still happens: an uninterrupted
run removes the per-step pauses, not the two gates whose whole purpose is the
user's judgement.

**Which pictures these are.** At this gate they are **reference images**:
concept art drawn from the scene description *before any prefab exists*, so what
is confirmed is the design, not a build. A **render** is a candidate prefab
imaged by `delvec render`, and belongs to curation later. Two stages, two
producers; building prefabs first and rendering them inverts the gate.

**A derived blockout has no reference image to judge.** On a site-plan campaign
the pictures at this gate are still design art — the map's own reference views
from step 2B, and the scene concept art anchored on view 1 — and what they show
is the design. **None of them shows the blockout**, and none is drawn to: the
massing is not authored, it is derived from the plan and the metrics table at
step 8, so a picture of it could only be made by inventing what the derivation is
going to do, and it would carry a picture's authority while doing it. So this
gate confirms the design and stops there. **The blockout is judged at step 9, in
the walk** — a site-plan campaign's first real gate, where scale, pacing, route
legibility and the silhouette from the declared `views[]` are settled by somebody
standing in it. Never send anyone to compare the built map against a reference
image of the built map; there is not one.

- On path A of Init step 6, the images are already in
  `campaigns/<id>/design/concept/` and already approved, with `design/README.md`
  naming the approved set. The gate is: present the design
  beside them and confirm the design still is what they show.
- On path B, `$DELVEWRIGHT_ENGINE/tools/refimg.py` draws them; prompt iteration is the work, and a
  subject needing more than one view is drawn as a **sequence of single
  full-frame views**, never one canvas cut into panels — the form is in
  *Reference: drawing the map's reference*.
  - **Anchor every scene on the map's own view 1, under the map series'
    style note.** The scenes are places in one map, so they are one series
    with it: pass view 1's interaction id — read out of its sidecar's
    `.id` — to `--chain-from` on every scene, with the same `--style-note`
    string the map views were drawn under. Anchor each on view 1 itself,
    never on the scene before it, for the reason the map series does it:
    chaining picture to picture compounds the drift instead of bounding
    it. Twenty scenes then come back in one hand, and a reviewer reads the
    set as one place rather than twenty unrelated pictures.

**Before you hand it over, read `world.json` against the pictures, and put each
image's sky under the image.** The images show an hour and a sky, and `time` and
`weather` are where that is declared — written at **step 1**, before these
images existed. **The walkthrough shows every image with its two tokens under
it**, in the words the document uses (`night` + `clear`, `dawn` + `rain`), so
the yes you are asking for is a yes to the picture and the hour together rather
than to the picture alone. Where a picture and the document disagree, make them
agree **now** — a one-field edit here, or a night's worth of art redrawn later.
The same reading applies to anything else in the images the documents have to
carry: a piece the library does not hold (step 2A says what you owe for one),
and a room whose light the pictures show as a mood rather than as a lighting
declaration.

**The moment images are confirmed they become campaign files.** Copy them **and
their `.json` sidecars** to `campaigns/<id>/design/concept/`, one per scene,
named for the scene, and write `campaigns/<id>/design/README.md` carrying the
approved names, what each one shows, and the sentence every later round is held
to: *author from the image, judge against it, present every choice beside it.*
**No date and no approver** — this file is a repository artifact under the same
rule `GENERATION.md` is; what it records is that this set is the approved one,
not when or by whom.
Commit them with the campaign. Everything under `design/` is tracked with
git-lfs (`.gitattributes`), so an approved image commits as a small pointer and
its bytes travel out of band — the `git lfs install` from Init step 1 is all this
needs, and it is why carrying a campaign's whole design set in the repository
does not make the next person's clone of it a several-hundred-megabyte download. `$DELVEWRIGHT_ENGINE/tools/refimg.py` writes to a gitignored working
directory, which is right for a draft and wrong for an approved one — **an
approval that lives only in a published page is bound to nothing.** The sidecar
travels with the image because it is what makes the image re-issuable with one
word changed: prompt, style note, resolved frame, anchor id. An image whose
prompt is gone can only be replaced, never edited.

**Write `campaigns/<id>/design.json` in the same act, one row per image you just
copied.** It is the machine half of this approval and the only home the approved
sky has: a row is `{name, shows, time, weather}`, where `name` is the file's path
stem under `design/` with no extension (`concept/shore-far`), `shows` is the one
sentence the README carries for it, and `time` and `weather` are the sky you read
off *that* picture — a set whose finale is a sunrise has two skies, and each row
says its own. Run `delvec schema --stage design` for the exact shape. Those two
tokens are the only judgement on this surface; everything else the engine
derives, and nothing anywhere opens the image.

From step 5 on, `DW0890` holds the record and the directory to each other in both
directions — a row naming no file, an approved file with no row, a stem two files
answer to — and holds the skies the rows state to the skies the world can reach.
A campaign that ships no `design.json` has approved no design: validation
measures that as a zero and says so without refusing, and `tools/staging-gate.py`
is where the zero is a red, so a build nobody can stage is what an unwritten
record costs.

**Every later step that asks anyone to choose reads `design/` first**, and
presents the choice beside that scene's image, under the approved name, saying
which element of the image the thing on offer corresponds to. This binds hardest
on contact-sheet curation, which is the step most likely to run in a later
session that never saw this gate.

Two tools help when a still image cannot answer the question:

- `delvec --prefabs prefabs viewer <nbt|dir|manifest.json> -o <page.html>` — one self-contained
  page the reviewer drives: orbit, plan, a player point of view at eye height at
  every anchor, and a cutaway that takes the roof off. Every block is drawn from
  the pinned version's own models and textures, so a wall is a wall.
- `delvec --prefabs prefabs contact-sheet <renders> -o <png>` — when candidate prefabs exist and
  someone is choosing between them, all of them on one page. `$DELVEWRIGHT_ENGINE/tools/refscore.py`
  can order the page by similarity to this gate's reference image; the score
  only **orders** the page, it never removes a candidate.

**A structural device enters a campaign only behind a green machine gate.** If a
shortcut loop, a one-way drop, an ambush reversal or a multi-path interlock has
no machine gate proving its class, it does not go in yet. Never author it now
and prove it later. When a design wants a device whose gate does not exist, that
is a capability gap: report it, and either the gate lands first or the design
does without it.

## 5. The content documents — `quests` and `dialogue`

This is the long step. **What the DSL can express is in *Reference: what a quest
can do*** — read it before writing, not after a refusal. **How the prose has to
be written is in *Reference: writing craft*** — run its section A over every
line before calling this step done.

The order inside `quests.json` matters: **write the `cast` block first, before
the objectives.** Every quest declares, for every NPC live in it,
`{at, doing, dialogue}` — position first, story second.

Then `dialogue.json`. Two rules that are not style preferences:

- **A dialogue option is a button caption, not a sentence.** Vanilla draws each
  option on a fixed 150-GUI-px button and *scrolls* a label that does not fit.
  Author to **≤20 Latin / ≤12 Han characters**; the compiler refuses over-long
  ones (`DW0331`). What does not fit belongs in the node's body text, which
  wraps, in the option's `tooltip`, or in the NPC's reply — never in the button.
- **Re-derive every node's option list from that node's situation.** Never carry
  an option list forward from an earlier node.

Loop `delvec --prefabs prefabs validate <campaign-dir>` until clean after each.
**This is the step where clean is reachable**, and it is where the codes step 3
ended holding go away: writing `quests.json` clears `DW0150` for every quest at
once and `DW0818` with it, and the effects it carries clear `DW0172`, `DW0112`
on a branch's `leads_to`, `DW0482` and `DW0197`; `dialogue.json` clears
`DW0152`. If any of them
survives a written stage 5 it has stopped being the expected state and is a real
finding — a `DW0150` here means an id in the plan and an id in `quests.json` do
not match, and the message says how many quests stage 5 declares, which is how
you tell that apart from the step-3 state.

**One code arrives rather than goes away, and this is the first run that can
raise it.** `DW0890` refuses when the approved design and the built world do not
agree about the sky: the rows step 4 wrote into `design.json` state `night` and
`world.json` declares `noon`, or an effect reaches a `dawn` no row states, or a
row names a picture that is not under `design/` — or a picture there has no row.
Every run prints its binding line first (`design record: N reference(s) recorded
over M image file(s) …`), refusing or not, so read that line and check the two
counts are the two you approved. The remedy is always a document edit on
whichever side is wrong, and *never* moving the hour to satisfy a mob — that is
`DW0496`'s rule, and *Reference: authoring pitfalls* says why.

If the campaign declares other languages, the localization stage is a **final
document stage after `dialogue`** — see *Reference: other languages*. It does
not change anything below.

## 6. `delvec fmt` — every campaign, every time

**Mandatory, for every campaign, whether or not it has a second language**, and
again after every later fix including every playtest-round repair.

```sh
delvec fmt campaigns/<id>
```

It rewrites every document and l10n sidecar in canonical form: object keys
sorted, two-space indent, non-ASCII raw, one trailing newline. It exists because
a three-key insertion into a non-canonical file produces a hundred-line diff
that nobody can review. **Array order is semantic and it never touches it**
(`quests[]`, `objectives[]`, `effects[]` are ordered), and it proves that on
every file it writes — so running it is never a risk to the campaign. It states
its binding count: `examined N file(s); reformatted M, 0 unparseable`.

Exit 1 means something is wrong with the JSON itself, not with its layout:
`DW0770` unparseable (it prints `line:col`), `DW0771` a duplicate object key —
which means one of the two values is already being silently discarded, so fix
the document rather than the formatter. Never hand-sort a file, and never "fix"
a `DW0773` by editing: re-run `fmt`. CI runs `delvec fmt --check`, so a campaign
that skips this reds there instead.

## 7. `delvec analyze`

```sh
delvec --prefabs prefabs analyze campaigns/<id>
```

Quest-graph reachability, deadlock, dark-room mitigation. Fix findings in the
documents — never by weakening the campaign. A dead quest is a design bug.

## 8. `delvec build`

```sh
delvec --prefabs prefabs build campaigns/<id> \
    -o "$DELVEWRIGHT_ENGINE/validation/delve-output"
```

Must exit 0. `$DELVEWRIGHT_ENGINE/validation/delve-output` is the
**zero-configuration** tree: every later step on this page boots it with no
variables set, which is why the command above names it.

**It is not the only place the tree may live.** `packtest-run.sh`,
`bot-run.sh`, `branch-runs.sh` and the play server all boot a tree anywhere,
including one inside this repository.

**If you put the tree in your own repository, TWO variables travel together.**
`dockerfile` is resolved relative to the build CONTEXT, so moving the context
without moving the dockerfile gives `failed to read dockerfile`, which reads as
a broken harness and is not one — it is the tree being somewhere the dockerfile
is not. The entry scripts export both for you; a hand-written
`docker compose … --profile play` does not:

```sh
export DELVE_OUTPUT="$PWD/.out/delve"                       # absolute
export DELVE_DOCKERFILE="$DELVEWRIGHT_ENGINE/validation/Dockerfile.delve"
delvec --prefabs prefabs build campaigns/<id> -o "$DELVE_OUTPUT"
```

Compose cannot compute an absolute default, so this is a real pair and not
something a better default removes. Set neither and everything above is
unchanged.

The build writes more than a datapack. Four things to read, and **the last two
stand on site-plan campaigns only** — an `areas[]` build emits neither, and
their absence there is the placement model, not a run that went wrong:

- `critical-path.json`, at the **root** of the output — the playthrough the
  proof found, step by step. **Every campaign.** A step that crosses areas
  carries a `transport` key; step 2A is about what puts it there.
- `render-plan.json` — the deterministic shot list, each shot with the `expect`
  line step 12 checks it against. **Every campaign.**
- **Site-plan campaigns**: the three hashes and the engine revision it prints
  at the end. Step 13 copies them.
- **Site-plan campaigns**: `DW0822`, the pacing line — it stands beside
  `DW0813` on every site-plan build and on no other. It measures the critical
  path over the built
  blockout — so many blocks of route, and about so many minutes at the
  metrics table's blocks-per-minute rate — and it carries **no threshold and
  refuses nothing**, so nothing in the engine compares it to your
  `target_minutes`. **You do that.** A map that measures twelve minutes
  against a `target_minutes` of 150 is not a warning anyone will raise; it is
  a map whose walking is a twelfth of its billing, and the gap is either
  content you have not written yet or a design that is smaller than it says.
  **On an `areas[]` campaign there is no pacing measurement at all**, so
  `target_minutes` there is your own estimate and nothing reads it — do not go
  looking for this line, and do not read its absence as a build that skipped
  something.

**There is no blockout document and nothing to author early.** A site-plan
campaign's geometry is derived from the plan and the metrics table by this
command, which then runs the battery over the bytes it laid: every seam built
where it was allocated (`DW0836`), every place reached from the entry
(`DW0837`), and no crossing between places anywhere a seam was not allocated
(`DW0838`).

**`--perturb <knob>` asks the derivation for a named defect and shows you the
observer catching it** — `slide-openings`, `sink`, `short-walls`, `brick-up`,
`low-ceiling`, `wall-contacts`, one per run, each printing which code it expects
(`sink`, `brick-up` and `low-ceiling` also take `--perturb-place`, and it is
refused for the others). It writes nothing — `--out` is refused beside it and the
exit is always non-zero — so a perturbed tree does not exist to be shipped,
walked or admitted. Reach for it when a battery has been green on this campaign
from the first build and you want to know it is looking at the bytes rather than
replaying the arithmetic that laid them.

## 9. The walk — STOP, this one is the user's

**You have no body in the game.** You bring the world up and the user walks it;
a blockout somebody has stood in tells them things no picture and no green check
will: scale, whether the route reads, whether the silhouette is the thing that
was designed.

It happens **now**, before the ladder and before any review — every step after
this costs more to redo than to defer.

Start the server. **One command does all of it** — it builds the campaign,
runs the staging gate against that exact tree, starts the container, and
verifies over rcon that the datapack actually loaded before it says READY:

```sh
"$DELVEWRIGHT_ENGINE/tools/playtest-server.sh" up campaigns/<id> \
    --prefabs prefabs --delvec "$(command -v delvec)" --out "$PWD/.out/delve"
```

**`--delvec` is optional now; this prints it so the command is exact.** Without
it the script uses the `delvec` already on `PATH` when that binary IS this
engine — its `--version` equal to the engine checkout's `versions.toml`
`[engine].version`, which is what Init step 2a installed — and otherwise builds
from source. Either way it prints, in one line, which binary it chose and why,
before it builds anything. Read that line: if it says it is building from
source, the `delvec` on `PATH` is a different engine from the checkout at
`$ENGINE_REF`, and that disagreement is worth stopping for.

It writes its own build tree wherever `--out` says, so nothing about this path
touches the engine's `validation/` directory, and it daemonizes — it prints the
connect line and gives you your shell back. `up` also TAKES the host-25565
mutex and holds it until `down`, so no automation can bind the port under the
user's feet.

The **second path** is the compose pair, and it is the one to take when step
8's tree is already sitting at `"$DELVEWRIGHT_ENGINE/validation/delve-output"`
and the ladder is coming next anyway — it serves that tree instead of building
a fresh one:

```sh
python3 "$DELVEWRIGHT_ENGINE/tools/staging-gate.py" --campaign campaigns/<id> \
    --build "$DELVEWRIGHT_ENGINE/validation/delve-output" \
    --report .out/round-1-gate.md
EULA=TRUE docker compose -f "$DELVEWRIGHT_ENGINE/validation/compose.yaml" \
    -f "$DELVEWRIGHT_ENGINE/validation/owner-play.yaml" --profile play up
```

That form runs in the FOREGROUND and holds the terminal until you stop it.
Either way the gate runs first — `owner-play.yaml` refuses to start without an
admission token minted for that exact build tree, and `playtest-server.sh`
calls the gate itself rather than trusting anyone to remember.

Then hand the user, in one message:

- **how to get in** — Minecraft Java 1.21.11 → Multiplayer → Direct Connect →
  `localhost:25565`. That wording is for the message you send them and nowhere
  else: the storybook's own connect line is step 14's, and it names no game
  version;
- **what to look for, item by item.** Not "have a look". Name the scale
  question, the route, each silhouette you are unsure of, and — per item — every
  finding still open from an earlier round that they must **not** test (see
  *Playtest rounds*, rule 2). Anything the staging gate reported red goes in this
  list by class;
- **how to tell you they are done.**

**Then end your turn and wait.** Do not run the ladder, do not start step 12,
and do not write `walk-record.json`. When they report back, their words are the
finding — record them, and take the server down:
`$DELVEWRIGHT_ENGINE/tools/playtest-server.sh down --name <name>` — which also frees
the 25565 mutex. `--name` defaults to `dw-playtest`; pass the one you brought up.

The staging gate is not optional here and not skippable by going around it:
`owner-play.yaml` is the only file that publishes 25565, and it refuses to start
the server without a token the gate minted for *that exact build tree*.

**A red gate is not a defect count.** It is the list of defect classes a
playtester is not protected from, drawn from every finding ever reported on any
campaign — so a campaign that contains none of the objects a row is about shows
as `UNBOUND`, and that is a fact about the ledger, not about your delve. Read
the list, put it in what you hand the user item by item, and never backfill a
weak check to turn a row green. To go in anyway on a build you know is red:

```sh
python3 "$DELVEWRIGHT_ENGINE/tools/staging-gate.py" --campaign <dir> --build <out> \
    --stage-anyway "<why this session needs a red build>" --acknowledge-red <N>
```

It prints every class being overridden, records the reason, and the server
announces it at boot — so anything hit from those classes in that session is the
override, not a new finding.

For a site-plan campaign **this walk is the campaign's first real gate**: scale,
pacing, route legibility, and the silhouette from the declared `views[]`. Say so
when you hand it over — the user is not being asked to admire it, they are the
gate. A finding edits the graph or the plan and regenerates — there is no hand edit to
lose, because there was never a hand edit to make.

`docker compose … down -v` is the compose path's teardown.

## 10. The machine ladder

Docker required. Every entry script takes a **`--project <id>`** and it is
required everywhere: the validation stack pins no container name and publishes
no host port, so the compose project is the only name the stack has, and two
ladders with distinct ids run side by side with no lock and no queueing. An
entry script invoked without one fails loudly rather than landing in a shared
default. Use `dw-<campaign>-r<round>`.

Each script fresh-volumes its own project before and after every run, so a
persisted world cannot keep completed objectives completed and fail a "fresh"
playthrough for reasons that have nothing to do with the delve.

```sh
EULA=TRUE "$DELVEWRIGHT_ENGINE/validation/packtest-run.sh" --project dw-<campaign>-r1
EULA=TRUE "$DELVEWRIGHT_ENGINE/validation/bot-run.sh" --project dw-<campaign>-r1
```

Both must exit 0. Then read `$DELVEWRIGHT_ENGINE/validation/run-out/<id>/run-report.json` — it is
project-scoped, so two ladders can never overwrite each other's.

- The bot ladder has two labelled stages once the delve has mandatory combat:
  `critical-path` and `die-retry`. The die-retry stage adds two scripted deaths
  per encounter, so a combat-heavy delve needs a larger timeout than the
  20-minute default: `DELVEWRIGHT_RUN_TIMEOUT_MS=2400000` on the command.
- **Read the `floor_gate` block every time.** It is the compiler's coverage
  ledger. `not_covered` names each fight the delve bills `elite`/`boss` that the
  gate cannot measure, with the reason — an empty findings list over an
  uncovered elite is silence, not a pass. **`covered`, `not_covered` and
  `actors[]` all empty is the worst case, not the best**: it means no body in the
  campaign declares a tier, the gate examined nothing, and it would have been
  green no matter what you shipped. Report that as **unbound**, never as a pass.
- An **empty `assist_windows`** is not evidence of anything on its own — read
  the `encounters` block beside it, which states each encounter's assist policy
  and the phase the run reached. Expect several windows per encounter.
- Reading one death trial: `respawn_pos` is where the bot actually came back and
  `at_checkpoint` is derived from it; `returned` is the walk back from exactly
  there. `re_engaged` and `outcome` are observed ONLY when `returned` — a trial
  that never got back reads `outcome: unproven`, which means the loop was never
  in a position to be judged, not that the fight vanished. `kit_kept: false` is a
  broken world seal, not a difficulty knob. A red `die-retry` is a content bug of
  the most serious kind: the delve is completable but dying is not safe. Never
  set `DELVEWRIGHT_DIE_RETRY=0` to get green — the report records a skipped stage
  as skipped, not as passed.

**Branch runs, required whenever the build emitted `validation/branch-plan.json`.**
One critical-path run proves one storyline; a campaign that forks must have every
branch walked, each in its own fresh world.

```sh
EULA=TRUE "$DELVEWRIGHT_ENGINE/validation/branch-runs.sh" --project dw-<campaign>-r1
```

It writes `$DELVEWRIGHT_ENGINE/validation/run-out/<id>/branch-runs.json`: per branch, ran or
skipped-with-reason, and the result. `DELVEWRIGHT_BRANCHES=<ids>` narrows it for
local iteration, and **a narrowed run is not a validated campaign** — the report
says which branches it skipped.

**On any red, triage before touching anything.**

- *Content bug* — your documents declare something wrong, unreachable or unlit:
  fix the campaign in the documents.
- *Toolchain bug* — the compiler or harness misbehaves on a campaign the
  diagnostics accept: **stop content work and report it**, with evidence. Never
  hand-edit compiler output, never restructure the campaign to dodge the bug,
  never weaken a check or reroll a seed to get green. A workaround that turns a
  toolchain bug green ships the bug to every future campaign. Escalating is
  success.

*Reference: when something goes red* maps the symptoms you are most likely to
meet to their actual causes.

## 11. The branch chronicle — only when the plan declares `branch_points`

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

## 12. Visual review

Yours to do, not a checklist to hand off: judging a frame is the whole task.

**Read this step's whole cost before you run its first command.** Rendering is
the largest single bill on this page — hours, not minutes, and it scales with
your campaign's scene count rather than with its size on disk. *What the set
costs*, below, gives the rate and where to read the count, and it decides which
frames you render rather than how fast you render them.

**Judge the player's eye first and the set second.** The question a playtest asks
is *what does a player walking in experience*, and only a first-person frame on
the actual assembled route answers it. The build emits those: a `pov` camera at
eye height on every corner-thinned critical-path waypoint, looking along the walk
and, at each leg's end, toward the objective it arrives at, each with its own
machine `expect` line. Every POV eye sits on a proven-standable waypoint.

**So read the POV sequence in route order before you open a single orbit
render, and treat it as the primary evidence.** A scene that photographs well
from outside and reads as a corridor of grey stone from the doorway is a
finding, not a pass.

**Read it with `campaigns/<id>/design/concept/` open beside it, and say per
scene whether the built area is the place the approved image shows.** This is
the page's own discipline everywhere else — *author from the image, judge
against it, present every choice beside it*, the sentence step 4 wrote into
`design/README.md` — and step 12 is where the two are finally in the same hand.
Nothing upstream compares them: the gate confirms a design, the build assembles
whatever the jigsaw seated from the pool, and until this step they have never
been introduced. So for each POV frame, name the approved image it answers to
and write one of two things — *this is that place*, or a finding saying which
element of the image is not there. A frame with no approved image to answer to
is itself the finding: the design gate approved something the build does not
contain.

**Read the sky on the first frame, because it answers in one glance and it
answers for the whole set.** `design.json` records the sky each approved image
was drawn under, as the `time` and `weather` tokens `world.json` uses; read the
frame against the row for the image it answers to. `DW0890` has already held
those two sides equal at every `validate` since step 5, so a disagreement here
is not the noon default — it is a row that states an hour the picture does not
show, which is the one half of this no machine can check. It is a one-field edit
and a rebuild, and it is cheap only if you catch it on the first frame instead
of after reading the set: an hour that is wrong is wrong in every frame, and
re-rendering the set costs what *What the set costs* says it costs.

Two shapes to expect, because they are what the machine cannot say:

- **The area is a different place.** A shore drawn as a black rock headland
  built as a flat sand floor in a box of cobble is not a lighting note or a
  detail gap; it is the pool having no piece that is the thing the design is
  about, arriving five steps after step 2A could have acted on it. Record it as
  a finding against the placement model, not against the render.
- **The area is that place with something wrong in it** — a floating slab, a
  seam that does not meet, furniture the design does not have. Those are
  document-level fixes, below.

**A site-plan campaign has no approved image of its blockout** and this
paragraph does not apply to it — step 4 says why, and its own judgement happened
at the walk.

**Save the world first — `delvec build` does not write one.** A delve's geometry
is stamped by the datapack over the first ticks of a server boot, so a build tree
carries no world save, and a Chunky scene names the world it loads. This boots
the tree once, waits over rcon until the datapack reports it has finished
placing, stops it, and writes `<build-dir>/world/`:

```sh
EULA=TRUE "$DELVEWRIGHT_ENGINE/validation/world-save.sh" \
    "$DELVEWRIGHT_ENGINE/validation/delve-output" --project dw-<id>
```

Docker, as for every other boot. `--project` is required and has no default —
it is the compose project this boot owns, so two of them run side by side
instead of tearing each other's volumes down; `--timeout` is the wait on the
datapack and defaults to 600 seconds. Nothing else in either repository produces
a world save, and the world it writes is server-written and not byte-reproducible
— which is why nothing hashes it and why re-running it is free.

`render-shots.sh` refuses without it, by name, rather than emitting scenes over
a world that is not there. That refusal is the whole reason this step is
separate: Chunky renders a missing world as an empty sky at exit 0, with the
reason buried in a Java stack trace, so the alternative is hundreds of plausible,
identical pictures of nothing and every command in the recipe green.

```sh
"$DELVEWRIGHT_ENGINE/validation/render-shots.sh" "$DELVEWRIGHT_ENGINE/validation/delve-output"
```

That writes **Chunky scenes, not images** — one per shot, plus the shot index.
Turning them into pictures needs Chunky, and **this is the step that installs
it**: Init named it and deliberately did not fetch it. Two commands, once per
machine, and they need the network:

```sh
curl -LO https://chunkyupdate.lemaik.de/ChunkyLauncher.jar
java -jar ChunkyLauncher.jar --update snapshot
```

The launcher self-installs a core into `~/.chunky/lib`, and what
`--update snapshot` installs is **today's** snapshot, never the pinned one:
`--update` takes a release channel, and the update site's `lib/` path serves the
current core whatever name it is asked for, so no command installs the pin. A
snapshot core is required either way — the stable line does not read 1.21.x
worlds. `render-shots.sh` has already named the pinned core and said which cores
this machine holds; if it said `MISMATCH` or `NONE installed`, every frame below
comes off a renderer this project has not verified its scene format against, and
you say so in the review rather than presenting them as pinned. Confirm the
launcher runs: `java -jar ChunkyLauncher.jar --version` prints a launcher
version, and `--update snapshot` ends in either an install or "No updates
found".

`curl -LO` drops the jar in the current directory, which is this repository's
root, and `java -jar ChunkyLauncher.jar` only resolves from there. `*.jar` is
not ignored here, so put it somewhere outside the tree or under `.out/` and
**write down the absolute path** — step 14 invokes it again, quite possibly in a
later session, and this page prints the bare form for readability.

Chunky reads the client jar placed at Init step 4; it needs no Java 21 of its
own. Then, one process per scene, in parallel:

```sh
java -jar ChunkyLauncher.jar -scene-dir "$DELVEWRIGHT_ENGINE/validation/delve-output/shots/scenes" \
    -render <scene-name> -f -threads <n>
java -jar ChunkyLauncher.jar -scene-dir "$DELVEWRIGHT_ENGINE/validation/delve-output/shots/scenes" \
    -snapshot <scene-name> <out>.png
```

`<scene-name>` is the file stem without `.json`. **`-target` is the sample
budget** — how many samples per pixel the path tracer accumulates before it
stops — and it is the one knob that trades render time against noise; it is not
a lighting, framing or quality setting, and nothing about the picture's content
changes with it. The command above passes none, so each scene renders to the
budget `delvec scene` already wrote into it — `sppTarget: 500`, the review tier.
Add `-target <n>` only to go somewhere else on the ladder: ~64 for a draft you
only need to judge framing on, ~300 for final art (`delvec panorama --spp`'s own
default), 500 for a review frame. A POV frame you are reading as primary
evidence is rendered at the scene's own number, and 64 is for deciding whether
the camera is pointed at the right thing. Chunky's progress counter
reads `(N of <image height>)` and counts scanlines rather than samples, so watch
`spp` against the target and not that number. The core is CPU-only, so the way
to go faster is one process per scene in parallel with `-threads <n>` each, never
a smaller budget on the frame you are about to judge. This step does not skip —
a visual channel that fails soft is a review that passed without looking.

**What the set costs, before you start it rather than four hours into it.** The
path tracer is CPU-bound, so the bill is a rate times a count and neither half is
hidden from you. **The rate, measured**: one POV scene at the review tier the
scene carries, `-threads 10` on a ten-core machine, took **299 s** — about five
minutes of the whole machine for one frame — and a thirteen-frame POV sequence on
that machine ran **82 minutes** end to end, so roughly 380 s a frame once a set
is going. **The count is what `render-shots.sh` already printed**: its last line
is `shot set ready: <N> Chunky scene(s) (incl. the whole-map panorama)`, and that
`<N>` is the number to multiply — read it, do not assume a size, and do not carry
a number off this page, because it is a property of your campaign. Even a
two-scene delve's full set runs to hours on that machine.

**Parallelism buys wall time, never total work.** One process per scene fills the
cores you have and stops there; the core-hours are the same whatever the process
layout, so the advice above is about where not to spend the budget rather than
about making the bill smaller. So decide the reading order up front and say what
you did: the POV sequence in route order is the primary evidence and is rendered
at the scene's own budget; the interior, seam, NPC and spawn-orbit shots come
after it. **A set you did not render is reported as not rendered**, per scene,
never left in a report where it reads as a frame that passed.

Every camera in `render-plan.json` is proven to stand in open air (`DW0724`);
read `camera_eye_proof` for how many were examined and how many had to be pulled
in off a stand-off that was inside geometry, and treat a shot carrying
`camera.requested_pos` as a hint about the build — the room is tighter than the
shot wanted, which is worth a look while you are there.

**Then the pieces**, for an `areas[]` campaign. Render **the pieces your
campaign actually uses**, one at a time:

```sh
delvec render piece prefabs/<piece>.nbt -o <workspace>/renders/<piece>
delvec render fidelity-gate      # must exit 0 before trusting any render
```

`delvec render batch <dir>` renders every prefab in a directory — 36 pieces and
435 shots for the shipped library — which is a library-curation tool, not a
campaign-review one. A site-plan campaign has no prefabs at this step at all.

Open the exterior/top/interior/anchor PNGs and check each against its `expect`
line: marker visible? room not dark? NPC facing the camera with its name as text
rather than JSON? seam clean? **Findings are document-level** — fix the campaign
(lighting profile, anchor, NPC facing, name string) and rebuild. Never hand-edit
output. Declared-dark interiors render faithfully dark, and no render will tell you
whether one is playable: that judgement belongs to the user's walk at step 9,
under the night-vision mitigation. Put it on the list you hand them there.
Never brighten a scene to make a review pass.

`delvec --prefabs prefabs viewer <nbt|dir|manifest.json> -o <page.html>` is the CPU half of the
same channel and needs no GPU. Read its fidelity list before handing the page to
anyone: it names every blockstate the page cannot draw as the game draws it — a
block the pinned version does not have (`DW0790`), and the one that reads as
fine and is not, a palette entry that leaves shape-carrying properties unwritten
(`DW0791`), where the shape comes from the version's default state rather than
from the file. That is a defect in the prefab, not in the page.

## 13. Detail — site-plan campaigns only, and only after the walk

Optional. A blockout is walkable and legible and made of concrete; detailing
replaces one place's massing with a real building. **Detail one place at a
time** — every unbound box is still massed, so the map builds, walks and renders
at every point between none detailed and all of them.

`detail-plan.json` has two fields and **there is no coordinate in it and no way
to write one** — no region, no extent, no datum, no seam, no offset. A `place`
and a `piece` is all a row can say:

```json
{
  "palette": { "role/wall": "minecraft:stone_bricks" },
  "details": [
    { "place": "node/near-hall",
      "piece": "prefab/near-hall",
      "anchors": { "anchor/node-near-hall": "hearth" } }
  ]
}
```

Where the piece goes is computed from the site plan's own box: the play space
plus the one floor course under it. The piece must be **exactly** that shape —
undersize is refused the same way oversize is (`DW0843`), because the box is the
footprint and a smaller building means a smaller box, which is a site-plan edit
and another walk.

1. **Record the walk the user did at step 9.** The `verdict` is theirs, not
   yours; if nobody has walked this build, you cannot write this file and detail
   does not start. Write `walk-record.json` beside the
   documents — `delvec schema --stage walk-record` is its shape. It is a
   campaign artifact rather than a stage document, so it carries no
   `dsl_version`, no `campaign_id` and no `stage`. Fill it with the three hashes
   every site-plan build prints (`site_plan_sha256`, `layout_graph_sha256`,
   `blockout_sha256`), the engine revision printed beside them, `verdict:
   "passed"`, and whatever the walk noted. **Copy all four out of the build
   output rather than computing them.** Nothing about detail compiles without it
   (`DW0841`), including asking for an allocation. **The first two hashes are
   the record's freshness key**: the whole a walk judges is derived from the
   plan AND the graph, so editing either one — even an edit that moves no block,
   such as which side a barred way opens from — re-opens this gate and asks for
   another walk.
2. **`delvec --prefabs prefabs allocation <campaign-dir> <place>`** — the frame's extents, the
   datum, every seam with the face class it must be answered by, and the owed
   anchor names. Build the piece against that and nothing else. It is an input
   to nothing; ask again whenever you want it.
3. **Build the piece** — a grammar program's export or a piece admitted through
   `delvec prefab`; the engine consumes the object, never the tool that made it.
   It must carry a spatial contract (`DW0843`), answer every seam, and open no
   way the plan did not allocate (`DW0844`, both directions). See *Reference:
   when the prefab library has no piece you need*.
4. **Bind it**, re-binding each owed anchor name to one of the piece's own
   anchors (`DW0845`). That is what keeps the quest layer working: those names
   were bound to places before any detail existed, and detailing must never
   force a quest edit.

Only when `details[]` binds every node does a declared vista stop being an
advisory and become a refusal (`DW0821`) — by then there is nothing left to
carve.

## 14. Hand it over

**The storybook.** Write `campaigns/<id>/README.md` — the
reader-facing introduction. Background and setting ONLY: premise, lore, public
NPC introductions (never a persona's `secret`), classes, playtime, the build and
play commands. No puzzle solutions, no quest structure, no endings. Images are
relative links into `media/`, small JPEGs, exterior or starting-scene shots
only, picked from the visual-review set — never interiors or late-game
locations. A localized `README.<code>.md` per declared language.

Storybook art is Chunky — the install from step 12 — in two passes. Draft with
`delvec snapshot` — fast, disposable, for judging *layout*: is the right thing in
frame, from the right side, at the right distance. Then produce the shipped
image with Chunky from
`$DELVEWRIGHT_ENGINE/validation/render-shots.sh`'s scene set, plus `delvec panorama <build-dir> -o
<dir>` for the whole-map hero shot every release owes (`--bearing` picks the
corner). Never hand-edit a scene JSON: if the frame you want is not emittable,
that is a `delvec render` gap to report, not a file to patch.

**Every edition opens with the engine-version marker**, on its own line directly
under the title. This is the one piece of internal machinery a storybook carries
— it is what a server host needs before running the delve — so it stays in this
exact form and nothing else internal joins it:

```
> **Requires delve engine <max per-stage dsl_version> or newer** — last verified with delvec <version> on Minecraft Java <mc version>.
```

The first number is the MAX `dsl_version` over the campaign's documents; the
second is `delvec --version`'s, from the build that just went green; the third
is the game version that same line prints, which is the one number a host needs
before they can join at all. The check parses the line anchored and whole, so a
marker missing the Minecraft clause is reported as MALFORMED rather than as
missing, and the expected line is printed back verbatim. The line is
byte-identical in every localized edition — it is a version stamp, not prose. A
translated gloss may follow on the next line but may not restate the numbers.

**Write no other version number anywhere in the storybook.** The marker is the
only one a check can keep true; every other is hand-typed and goes stale in
silence. So: no campaign-version stamp, and the host command names `:latest` —
that IS the storybook's claim — with one sentence sending a reader who wants an
exact version to the release page, where the tag is machine-written.

**That includes the connect line, which is where it bites.** Step 9's wording
names the game version because it is going into a chat message; written into the
storybook it is a second version literal and the check refuses it by name. Point
at the marker instead — it already carries the number, and it is the one copy
anything keeps true:

```
Start the server, then in the Minecraft Java client at the version the marker
above names: Multiplayer → Direct Connect → `localhost:25565`.
```

`localhost:25565` and `-p 25565:25565` are safe to write: the check knows a port
from a version. Then prove it:

```sh
python3 "$DELVEWRIGHT_ENGINE/tools/check-storybook-version.py" --campaigns campaigns
```

Green before you report. A stale marker waves a host on an old engine straight
into a delve their engine cannot run.

**Then report to the user** — this hand-over ends the run: the campaign
summary, the playtime estimate, the validation results, what the walk found and
what was done about it, anything still open, and the two commands they will
actually use.

```sh

# play — one command: build, gate, serve, and print the connect line
"$DELVEWRIGHT_ENGINE/tools/playtest-server.sh" up campaigns/<id> \
    --prefabs prefabs --delvec "$(command -v delvec)" --out "$PWD/.out/delve"

# playtest, with in-game notes
EULA=TRUE CREATOR_NAME=<mc name> docker compose -f "$DELVEWRIGHT_ENGINE/validation/compose.yaml" \
    -f "$DELVEWRIGHT_ENGINE/validation/owner-play.yaml" --profile playtest up
```

`owner-play.yaml` is what publishes `localhost:25565`; the base compose file
publishes nothing. Both paths run the staging gate first — see step 9.

---

# Reference

Everything below is looked up, not read in order. A step above names the section
it needs.

## Reference: turning a prompt into a campaign

A prompt is a **constraint set over the documents**: honour everything it pins
down — theme, specific levels, plot beats, NPCs, homages — and invent the rest.
Ask two or three clarifying questions only if the prompt is too thin to pick a
theme and a target length; otherwise proceed.

**A thin prompt is creative licence.** When a prompt pins down little — a
one-line theme, no detailed brief — treat it as a **showcase** brief and
deliberately exercise the breadth of what the engine supports, so that a
stranger playing the result discovers what it can do. Do not work from a written
feature list, which rots as the schema moves: **query the live schema**
(`delvec schema --stage all`) for the available verbs and effects, then aim to
include, wherever the story can carry them coherently: multi-area transport as a
narrative beat, flag-gated dialogue consequences, real props and set dressing,
at least one tuned combat or stealth encounter, narration beats, and varied NPC
presentation. **Coherence and pacing always win over feature count** — never
bolt on a mechanic the story cannot motivate.

**A detailed brief is the opposite**: honour exactly what it pins down and
showcase nothing extra.

**Every round changes only what was asked for.** A mechanics fix must not
incidentally rewrite story, staging or dialogue; an approved change that moves
the design updates `DESIGN.md` in the same commit; and every round ends with a
conformance review — diff the campaign's current behaviour against `DESIGN.md`
beat by beat and report any deviation nobody asked for instead of shipping it.
Drift found in review is restored to the design or escalated, never silently
kept.

## Reference: what a quest can do

The verbs, effects and fields available in `quests.json` and `dialogue.json`.
Get the exact shapes from `delvec schema --stage quests` and `--stage dialogue`;
this section is what they are *for* and the traps in each.

### Objectives

- **Author `title` and `hint` for every non-`talk-to` objective.** `title` is a
  short player-facing name ("Unbar the Deep Gate"); `hint` is one line of
  location or direction guidance ("Past the entrance hall, take the left passage
  to the barred door"). The compiler surfaces them in-game when the objective
  activates, in chat and with a sound; without them the player gets no guidance
  and cannot find interact/collect/reach targets. For `talk-to` they are
  **required whenever the target NPC is not already visible from where the
  previous objective completed** — a different room, down a corridor, across an
  area. Read the "may omit" allowance narrowly: an off-screen NPC 60 blocks away
  through an unfamiliar cave leaves the player with nothing.
- **Hint wording**: landmark-relative directions from places the player already
  knows — the entrance hall, the gate, a named NPC. Never room-shape jargon
  ("corner room", "L-shaped hall") and never solver-internal terms (anchor,
  piece or socket ids).
- **`interact.requires_item` is HELD, not carried**: the player must have the
  item in their **main hand** when they click — presenting it is the action.
  Author `missing_item_hint` whenever the empty-handed click deserves diegetic
  feedback (a sleeping giant mumbles in its sleep; a locked door rattles and
  holds). It is narrated in chat to that player, only while the objective is
  open, and without it the click is met with total silence, which reads as a
  broken affordance.

### Items, containers and loot

- **Furnish the containers.** A prefab's chests and barrels are empty until a
  `loot[]` entry fills them (`{id, anchor, items:[{item, count?, name?,
  enchantments?}]}`), and an empty chest reads as a bug to the player. Give
  every reachable container consumables or props; a named `name` is how a prop
  becomes a story object. The container must already exist in the piece at that
  anchor — **the compiler fills furniture, it never places it** (`DW0431`).
  Elites and set-piece actors take `equipment` in the same shape wave mobs use,
  enchantments included.
- **A `collect` has three shapes, and which are available to you depends on the
  library.** Give the item an `item_name` ("Cheese", "Tide Ledger") in all
  three: it is what the player reads on the stack, it translates like every
  other player-visible string, and an unnamed generic item says nothing about
  what the quest asked for.
  1. **`container: <anchor>`** — adopt a chest or barrel **the piece already
     placed**. The compiler fills that container and places nothing of its own;
     a floating chest conjured beside the barrel the player has been walking
     past is the defect this avoids. Set `fill_count` so the container reads
     plausibly full — it counts padding SLOTS after the objective's own stack,
     and `1 + fill_count` must fit the container's 27. The container must really
     be there in the piece (`DW0438`) and must not also be filled by a `loot`
     entry or another `collect` (`DW0435`).

     **Measured over the shipped library: exactly 1 of 36 prefabs can satisfy
     this** — `island-mountain`, through its four `anchor/cheese-barrel*`
     anchors. Five pieces contain a chest or barrel anywhere
     (`hero-galleon-oak`, `hero-standing-monolith`, `island-beach-camp`,
     `island-galley`, `island-mountain`), and only that one stands an anchor on
     one. **Two pieces declare an anchor literally named `anchor/chest` whose
     cell is air** (`cave-room-small`, `keep-room-small-a`) — the obvious thing
     to reach for, and it is `DW0438` every time. In `pool/stone-keep` there is
     no piece that can carry a `container` at all. Check rather than assume; the
     library moves.
  2. **`dropped_by: <wave>`** — the item comes off a body instead of out of a
     box. The compiler places no container and PROVES the chain: that the wave
     really yields the item (`DW0492`) and that its `kill` objective runs first
     (`DW0493`). `dropped_by` names a wave, never an actor — an actor's death is
     observable by no objective. This is the right shape whenever the story can
     carry it, and it needs nothing from the piece library.
  3. **Neither** — the compiler places its own chest at the objective's anchor.
     Legitimate when the room genuinely has no furniture, and the thing to avoid
     is a conjured chest standing *beside* a container the room already has.
  4. If the beat really needs a piece with a container in it and none exists,
     that is a piece to make: *Reference: when the prefab library has no piece
     you need*.
- **An elite or boss leaves ONE thing behind, and you say which.** Give the
  fight's `drops[]` a declared subset — a `{"slot": "main_hand"}` for the axe
  the player watched swing, or a `{"item": …, "name": …}` for a quest token —
  never the whole kit. Only an `elite`/`boss` encounter may declare drops
  (`DW0491`); a slot must be one the same body's `equipment` really fills
  (`DW0490`).

### State, flags and gates

- **A number the world remembers is `state`, not a flag.** A flag is boolean,
  party-wide and one-way — nothing clears one — so it says "this happened" and
  nothing else. When a beat needs a *quantity* that goes down as well as up (a
  toll still owed, a floor a lift is at, whether a ride is in progress), declare
  it in the `state` list: `{"id": "state/<kebab>", "scope": "party" | "player",
  "initial": <n>, "note": "<what the number means>"}`. `scope` is required and
  never guessed — `party` is one shared value, `player` gives each member their
  own. Write it with `set-state` / `add-state` (signed: a negative `amount`
  counts down) / `clear-state` (back to `initial`).
- **Read it in the gate, never in a verb.** `requires_state: [{"state": …,
  "op": "equals"|"not-equals"|"at-least"|"at-most", "value": <n>}]` is accepted
  everywhere `requires_flags` is — an objective, any gatable effect, a trigger,
  a trap, a dialogue option, a cast placement — so "the door opens at zero" and
  "this line is withheld below two" are the same construct.
- **Every datum must be both written somewhere and read somewhere**: a gate
  reading a datum nothing writes is `DW0501`, and a datum no gate reads is
  `DW0502`. Both mean the mechanism is decoration.
- A `player`-scoped datum can only be touched where a player is acting — a
  dialogue option, a cast placement, an `on_death` beat, an effect on a quest
  beat a player completes, or a trigger declaring `audience: "presser"`, which
  runs as the player who clicked. These have **no** acting player and reject one
  (`DW0503`): an objective/trigger/trap *gate*, a party-audience trigger's
  `effects`, a trap's `payload`, a shortcut's `on_unlock`, a `sequence` step and
  a `move-npc`/`move-actor` `on_arrive`. Use `party` scope there.

### The story layer

- **Every story node declares a `happening`.** One line saying what the node
  does to the story: `{verb, text, subject?}`, where `verb` is one of `dies` /
  `survives` / `departs` / `arrives` / `learns` / `believes` / `gains` / `loses`
  / `opens` / `seals`. Required on every quest, every objective, every staging /
  wave / gate / `campaign-complete` effect, and every dialogue option that sets a
  flag — a missing one is `DW0481`. It is the event-flow twin of the cast
  ledger's `doing`: you cannot fill it without deciding what the beat *is*. Keep
  `subject` accurate (`npc/…`, `actor/…`, `wave/…`, `anchor/…`, or an `item/…`
  label) — the compiler reads only the verb and the subject, and uses them to
  catch a dead character who later acts, or a sealed gate later walked through,
  per branch (`DW0485`).

  `happening` belongs to the objects named above, and only those. It is not a
  field on an arbitrary effect: putting one on a quest-level `set-flag` is
  `DW0100`, and the refusal enumerates what that object does take.
- **The `cast` block, first in every quest.** Every quest declares, for every
  NPC live in it, `{at, doing, dialogue}`. `at` is an anchor, or `"offstage"` /
  `"dead"`, which must match a real `despawn-npc` — declaring a position does
  not move anybody (`DW0461`). `doing` is free prose and is the point: you
  cannot fill it without deciding the character's business in this beat, and the
  dialogue stage writes their lines against it. `dialogue` is a dialogue root
  id, `{"barks": [...]}`, `"unchanged"`, or `"none"`.
  - **A sleeping, working or background NPC gets a `barks` pool**, not
    `"none"`. Right-click then yields one inconsequential in-character line
    instead of dead silence. Use `"none"` only when the silence is the
    statement.
  - **Write `"unchanged"` when you are deliberately carrying dialogue forward**
    — never re-spell the same root id, and never omit `dialogue` hoping it
    defaults (it does not: `DW0463`). `"unchanged"` at an NPC's first
    appearance is `DW0466`.
  - **Treat the `DW0467` staleness warning as a design smell, not a nuisance.**
    It means an NPC's right-click never learns that the story moved. Give it a
    scene that changes, or make it a bark-pool background character — do not
    silence it by shuffling spellings.
  - Omitting a live NPC is `DW0460`: an unaccounted NPC is how a crew member
    ends up standing forgotten in an alcove while the player escapes.
- **Post-fork casts are per branch, every quest.** After a fork opens, an NPC
  whose situation differs by branch declares a **list** of placements, each
  gated by the flags of the branch it belongs to — in *every* later quest, not
  just the first. Leaving one ungated as a fallback is `DW0483`: later
  declarations win, so the fallback keeps governing the branch that already has
  its own, and the fork moves the ledger without moving the bodies.

### Dialogue

- **`button = caption, tooltip = the full line.`** When the caption cannot carry
  what the character actually says — the wine beat, where "Pour it out." stands
  for a whole sentence — author the option's optional `tooltip`: vanilla shows it
  in a hover box beside the button. It **wraps** (no `DW0331`, no width budget),
  so it takes a full sentence. Use it for the *said line*, not for hints or
  mechanics; the button still has to be readable on its own, since a player on a
  controller or reading fast never hovers. It translates under its own key.
- **Premise and exposition options must retire once their moment passes**, via
  the cast ledger's dialogue swap (declare a later root) or a flag gate. A "who
  are we" / "what is that thing" option must be **impossible** at the finale.
  The defect this prevents: after a climactic escape, a crew NPC still offering
  "Tell me what he is." and "Is there another way out?" — questions the
  character has already lived through the answers to.
- Flavour NPCs get real trees too.

### Things that change the world

- **A region can be filled or cleared while the delve runs, and no gate need be
  involved.** `fill-region {region{anchor,extent}, block}` writes a block over a
  declared box; `clear-region {region{anchor,extent}}` empties it.
  `open-gate`/`close-gate` are the same operation with the box and the block
  read off a prefab gate anchor — reach for those when a prefab already declares
  the threshold, and for these when the box is yours: a bridge that
  materialises, a floor that sinks, a wall that opens, a platform summoned under
  the party. The completability proof honours them from the point in the quest
  graph where the effect fires: a fill the only route must cross afterwards
  fails the build (`DW0311`), and a clear is credited as passable, so a route may
  legitimately depend on one. **Two things it will not model, so do not build on
  them**: a clear that opens a box into water (the water flows back in and the
  proof does not know), and a clear over rubble another mechanism dropped there
  (a `collapse` debris field, a shut timed gate) — those stay solid.
- **A piece can ship BROKEN and have the campaign repair it.** A zone whose
  spatial contract declares a `way` on one of its edges is severed there as
  built — a stair whose treads are missing, a bridge that is not down, a doorway
  packed with rubble — and `open-way {piece, way}` is what opens it:
  `{ "type": "open-way", "piece": "prefab/z7-bell-tower", "way": "broken-flight" }`.
  This is how a beat *changes the building*: the collapsed stair the party
  rebuilds, the drawbridge the lever lowers, the shortcut opened once from the
  far side and open forever. **You write the reference and nothing else** — the
  cells, the block and the direction all come from the piece's own metadata, so
  there is no geometry to get wrong and no second place for it to drift. Two
  rules follow, both build errors rather than surprises in play: the piece must
  be placed exactly once for the reference to name it (`DW0547`), and anything
  the party is required to reach beyond the break — an objective anchor, an
  NPC's stand, a wave's spawn — needs a **forced** `open-way` on an objective the
  quest graph puts *before* it (`DW0548`). A way nothing opens is fine and stays
  shut: a door that never opens is content. Every staged way's fate is listed in
  `validation/ways.json` after a build.
- **A prefab's gate is SHUT until the campaign opens it.** A gate anchor's
  region holds whatever the prefab authors there — `hello-room`'s doorway is
  iron bars, the island's cave mouth is air — and the compiler measures which. If
  anything the party must reach lies past a barred gate, some objective they are
  **forced** to complete has to `open-gate` it first, or the build fails naming
  the anchor (`DW0317`). "Forced" excludes every optional bundle: a trap payload,
  `on_death`, a shop offer, a shortcut's far-side unlock. To spell "the party
  walks up to this and the door opens", use an environment `trigger` — that one
  counts.
- **A teleport selects a REGION, never a block.** `teleport {from {anchor,
  extent}, to}` moves **everything** inside the box to the destination anchor —
  players and entities alike, which is what makes a cargo platform the same
  mechanism as a passenger one. Nothing is exempt, so do not draw the volume
  over an affordance the engine anchors to a block (an interact objective, a
  click trigger, a bonfire, a shortcut lever, a disarm, a sealed gate): the
  hitbox would ride and the hardware would stay, and the build refuses it
  (`DW0542`). Two things to design AROUND rather than against, both measured on
  the pinned server: **a teleport is not a rescue** — accumulated fall distance
  carries across it unchanged and is charged in full at the destination, so a
  platform arriving under a falling player past ~20 blocks is the surface they
  die on; and **nav does not know about it** — a route that exists only through
  a teleport still fails the completability proof, so keep a walked route to
  anything the critical path needs.

### Danger, death and money

- **A trap is a trigger plus a payload, and the payload is ordinary effects.**
  `traps[] {id, at, trigger, payload, lethality?, disarm?, reset?, + the
  ordinary gate}`. `trigger` is `pressure-plate` / `tripwire` /
  `trapped-chest` — the one job redstone keeps, because it is the part the
  player sees, suspects and learns to read. The consequence is `payload`: an
  ordered effect list in the **same vocabulary a quest uses**, plus two trap
  verbs — `volley {from_anchor, kill_zone, projectile?, salvos?, interval?}`,
  which blankets every standable cell of its zone rather than sniping (escaping
  is a decision to leave, never a lucky strafe; the line of fire to every cell
  is proven at build time, `DW0442`), and `collapse {region_anchor,
  falling_block?, then_floor?}`, which deletes a region and buries whoever is
  under it, with the settled world re-run through the completability proof
  (`DW0445`). A trap that declares no consequence at all is `DW0440`.
- **`at` is an ORDINARY POINT ANCHOR and the piece needs no hardware.** The
  compiler owns the detection tick for a command payload: a plate or a tripwire
  is a position test on that anchor's cell, and a trapped chest is the same
  interaction-entity `use` a disarm lever rides. So any anchor carrying a `pos`
  that some area's prefab provides will hold a trap — and on a site-plan
  campaign that is `anchor/node-<place>`, which is exactly what a derived map
  has. `volley`'s `from_anchor` is another one, and all it owes is a clear cell.
  **The name `anchor/trap` is a convention from the redstone era and the check
  does not read it**: what is required is a point anchor that resolves, not one
  called that. Read any text telling you to "bind the trap to an `anchor/trap`
  marker" — the refusal's remedy line and the schema's own field description
  both say it — as saying *that anchor does not resolve*, and answer it with an
  anchor that does. Measured over the shipped library: **0 of 36 prefabs
  declares an `anchor/trap`**, or any anchor carrying `dispenser` or
  `trigger_block` metadata, out of 103 anchors in all — so the piece that
  reading asks for does not exist here, and a command payload never wanted one.
- **Two things do want hardware in the piece, and neither of them is the
  payload.** The superseded `effect: {dispense: {…}}` fills a dispenser socket
  the prefab pre-wired, so it needs that metadata; and a trap carrying a gate
  (`requires_flags` / `forbids_flags` / `requires_state`) is gated *physically* —
  the compiler takes the trigger block out of the world while the gate is shut
  and puts it back verbatim — so it needs the anchor's declared `trigger_block`,
  and a `trapped-chest` can never be gated because removal would destroy its
  inventory (`DW0363`). Against this library neither is authorable at all. Write
  an ungated `payload` trap, and gate the story beat that arms it instead.
- **A place that kills is DECLARED, never faked with the art.** A cliff whose
  fall must be fatal, a lava pit, an acid pool, an out-of-bounds plane: all one
  declaration, `lethal_volumes[] {id, region{anchor,extent}, message,
  damage_type?}`. Never obtain the behaviour by changing the world instead —
  making the horizon `void` so the fall kills serves exactly one fiction and is
  never the move here. `message` is REQUIRED and is what the player reads as
  they die (blank is `DW0512`); `damage_type` words vanilla's own broadcast
  (`fall`, `on_fire`, …). The volume is geometry the completability proof
  honours: if the party's only route to an objective crosses it the build fails
  naming the volume (`DW0510`), and nothing the campaign POSTS — the entry spawn,
  a checkpoint, a bonfire, an NPC's anchor, a `cast` placement, an actor — may
  sit inside one (`DW0511`). Put the volume where a player can SEE what will
  happen before they commit to it; a killing box nobody can read is 初见杀 with
  no lesson in it.
- **What happens when a player dies is content, not engine behaviour.** The
  quests document takes a campaign-wide `on_death`: a bundle of ordinary effects
  that runs at the moment a player dies, for that player. One per campaign — it
  is not a field on a checkpoint, because dying is true everywhere in the delve;
  put `requires_flags`/`forbids_flags` on the effects inside it if the beat
  should only land in some phase of the story. Do NOT write a death beat the
  mainline depends on: nothing inside it is credited as a flag producer,
  deliberately, so a door it alone opens is a door only a corpse can open.
- **A death that costs something leaves a stake, and the engine decides where.**
  `stakes[] {id, state, forfeit?, max_live?, on_full?, collect_by?,
  collected_message, marker_item?}`, dropped by a `drop-stake` effect in
  `on_death`. The datum must be `player`-scoped (`DW0520`) — a stake is one
  player's wager, never the party's. You do **not** choose where it lands: the
  compiler computes the point, on the walkable way back from the respawn point
  in force, nearest to where they died, so a death in a lethal volume leaves its
  stake at the near lip rather than inside the hazard, and a death on a lift car
  leaves it on solid ground. If your geometry can strand one — a one-way drop
  with no shortcut back — the build fails naming the place (`DW0525`), and the
  fix is a route back or a `lethal_volume`, never deleting the stake. Souls
  behaviour is `max_live: 1, on_full: "replace"`; no death cost at all is
  `max_live: 0`; a memorial at every death site is a larger `max_live` with
  `on_full: "keep"`.
- **A currency is a NAMED datum, and a price is a GATE.** There is no
  `currencies` section and no `price` field, on purpose. Give a `state[]` datum
  a `name` and it becomes a purse the player reads: the engine states
  `<name>: <value>` on that player's action bar on every write, translated like
  any other line. A shop is `shops[] {id, anchor, title, marker_item?,
  offers[{label, tooltip?, effects[], + the ordinary gate}]}`, and its prices are
  `requires_state` comparisons — exactly the ones a door or a dialogue line
  would use. **Write the refusal yourself**: put the purchase behind `at-least
  <price>` and an apology `narrate` behind `at-most <price − 1>`, both as gated
  effects of the same offer, so a player who cannot afford something is told
  rather than left pressing a dead button. An offer with no effects at all is
  `DW0523`. **Order matters and the compiler will tell you (`DW0527`)**: put the
  refusal and any confirmation BEFORE the debit. Sibling effects are consecutive
  commands, so a gate written after the debit reads the balance the debit just
  produced — buy your last coin and you are charged and apologised to in the same
  breath.

### Bodies

- **`base_entity` accepts any entity id, and NPCs are inert by construction.**
  Every NPC is summoned `NoAI,Invulnerable,Silent,NoGravity,PersistenceRequired`
  plus a separate interaction hitbox, and there is no registry validation on the
  field — so any mob id becomes a talking statue that cannot move or hurt
  anyone. This is how a villager-sized cast can include a giant: e.g.
  `minecraft:warden` as a Cyclops you must slip past. *Caveat:* `Silent:1b` also
  suppresses that entity's ambient sounds (the Warden's heartbeat), and the
  emitted `VillagerData` tag is inert on a non-villager.
- **A body that moves unlike its species DECLARES it, and the build holds it to
  the claim.** `traversal { locomotion: ground|climber|flier }` on an NPC or an
  actor. By default the compiler reads locomotion off the entity id — spiders
  climb, ghasts fly, everything else walks and is checked — so a walked leg that
  goes OVER a wall line instead of round to its opening is `DW0453`. If that is
  your fiction (a sheep that climbs), declare it and the advisory is answered. It
  is **not** an off switch: a declaration that changes no verdict is refused
  (`DW0454`), so you may only claim a climber where the route really climbs;
  `aquatic` is refused outright (`DW0455`) because nothing in the model could
  hold a body to it; and no declaration touches the error tier — a declared
  climber still cannot walk through a closed fence gate (`DW0452`). Declare it on
  the body, never on the beat.
- **A status effect is a verb — and it ends by expiring, never by being
  cleared.** `give-effect {effect, seconds, amplifier?, hide_particles?, in?}`
  grants any pinned-1.21.11 status effect; `in {anchor, extent}` narrows it to
  the players inside a box, so "blind whoever is riding" does not blind the
  delve. `seconds` is REQUIRED and there is no infinite form, on purpose: an
  effect whose only removal is a later step is one the player keeps forever
  whenever that step does not run — a logout, a crash, a death mid-chain. So **do
  not write "grant, then clear at the end"**; write a duration that covers the
  beat plus slack and let it expire. Pairing a live grant with a `clear-effect`
  of the same effect in the same bundle is `DW0540`. `clear-effect {effect?,
  in?}` exists for effects the campaign did NOT grant (a potion the player
  drank, a `wither` a mob applied); omit `effect` to clear everything.

### Sealed things, and pacing

- **Anything you seal, you must say what it says.** A `shortcut`'s barred door
  and a `close-gate`'s wall are both things the party walks up to and pushes on,
  and one with nothing to say is `DW0429` — the build refuses. The compiler will
  not decide the tone of your door or your wall for you and then not tell you it
  did. One rule for both: they are two objects of the same class.
  - A `close-gate` discharges it either way — `"sealed_hint": "<what the wall
    says>"` on the effect, or a trigger. `sealed_hint` is only the *wording*.
  - A `shortcut` has no wording field, deliberately: its line is a trigger.
- **Write the reply with the general verb:**

  ```json
  {"id": "trigger/…", "at": "<the gate anchor>", "on": {"on": "use"},
   "once": false, "audience": "presser", "effects": [{"type": "narrate",
   "style": "actionbar", "text": "The door cannot be opened from this side."}]}
  ```

  Anchoring it on the gate is what makes it *ride* the sealed body's own
  hitboxes instead of summoning a co-located second one (`DW0422`), and on a
  shortcut door the body stands on the sealed side only — so the line fires where
  it is true and nowhere else, and retires when the door opens. Once you write
  one, the compiler supplies nothing: one press, one answer. ANY `use` trigger on
  the gate discharges `DW0429`, whatever it does — but a `strike` does not,
  because pressing a thing is a right-click.
  - `audience: "presser"` addresses the one player who clicked, and works on
    `on: use` only — vanilla can attribute right-clicks and nothing else
    (`DW0427`). Leave it out and the beat addresses the whole party, which is
    right for a lever that opens a gate and wrong for a reply.
  - `style: "actionbar"` is the reply strip above the hotbar: it does not
    interrupt, does not stack, and is not width-checked. Use it for replies; use
    `title`/`subtitle` for beats.
  - Trigger ids starting with `dw-` are reserved for the compiler (`DW0428`).
  - Two `close-gate`s on one anchor must still agree on the wording (`DW0423`).
- **A beat that can FAIL the player must not arm before they could have read
  it.** Any fail-able beat — follow-an-NPC, escort, timed escape, stealth onset —
  arms only after a grace window long enough to read the on-screen prompt that
  explains it: the player must never be failable before they could have
  understood what is being asked. Where the DSL has an explicit knob, set it
  consciously rather than inheriting the default (`begin-stealth`'s
  `grace_ticks`); where the pacing is authored, put the first enforcing step late
  enough in the `sequence`'s `at_ticks`. Budget the window from the prompt's
  length, not from a habit — a two-line chat prompt is several seconds of reading
  before the first step is taken. The defect this prevents: the flock the player
  is told to follow leaves while they are still reading the instruction, and the
  beat then fails them for it.
- Pace to `target_minutes`; no grind; mandatory-only quests.

## Reference: writing craft

Everything a player reads is prose: dialogue, objective titles and hints,
narration beats, item and area names, the storybook. This is the craft checklist
for all of it, and section A is run over every line before step 5 is called done.

These are **pattern warnings, not technique bans.** They govern *automatic*
writing — the phrasing that arrives before you have decided anything, the hand
reaching before the mind does — not the device itself. A simile is not
forbidden; the simile you did not choose is. Banning a technique outright
produces stilted avoidance, which is its own tell.

### A. Automatic-phrasing tells

1. **Observation + verdict.** A line, then the text grading it — "*…, more
   statement than question*", "*…, and it was not a request*". The verdict
   instructs the player how to hear what they just read. Cut the verdict; if the
   line cannot stand without it, the line is wrong.
2. **Standalone simile fragments.** "*Like a blow to the chest.*" — a comparison
   set alone as though it were the feeling. Test every simile: does it make the
   player see the **thing** more sharply, or make them notice the author? The
   second kind goes.
3. **Stock intensity moves.** The air growing thick or heavy; time slowing;
   silence stretching; words left hanging in the air; a breath the character did
   not know they were holding. These are the default gestures at "this moment
   matters", and every generated delve reaches for them unprompted.
4. **Repetition as intensity.** Saying it again, louder — "*more than tired:
   hollow*"; three-beat lists where two beats carry all the meaning.
5. **Correction pairs.** Naming a false label in order to knock it down — "*not a
   warning, a promise*"; "*it stopped being a door and became a mouth*". Once per
   campaign is a rhetorical choice; three times is a signature.
6. **Purposeless gesture.** A nod, a tightening jaw, a hand moving to a hilt,
   costing nothing to delete. A gesture signifies by contrast with what that
   character usually does. If it can be cut with no loss, cut it.
7. **Explaining your own subtext.** An NPC says the hard thing, then the next
   line paraphrases what it meant. Trust the player; they have already read it.

Applies hardest where the text is shortest: `hint`, `title`, bark pools and
`missing_item_hint` have no room to recover from a wasted clause.

### B. Convergence is the real tell — vary the posture per campaign

StoryScope (arXiv:2604.03136) separates human from AI fiction at **93.2%
macro-F1 from narrative structure alone, with every stylistic signal withheld**,
and span-level style editing of the prose moves that number by 1.6 points. So
the AI tell is not a phrase you can scrub. It is **convergence**: five different
models occupy one tight region of narrative space while human stories are
dispersed around it (mean rarity 0.49 vs 0.71). Section A is hygiene; this
section is the actual defence.

Measured gaps worth authoring against (AI vs human in that corpus):

| axis | the machine default |
|---|---|
| thematic explicitness | the narrator states the story's point — 77% vs 52% |
| emotion rendering | somatic: tight chest, cold sweat — 81% vs 38%. **Humans name the feeling outright 29% of the time; AI 8%.** |
| plot shape | no subplots 79% vs 57%; protagonist-driven resolution 69% vs 46% |
| resolution | closes on internal understanding or acceptance, 47% vs 27% |
| time order | strictly chronological; humans jump, flash back, withhold |
| morality | morally ambivalent protagonist 38% vs 59% |
| address | humans break the fourth wall (67% vs 39%) and address the audience (28% vs 7%) |

**Claude specifically** is the most distinctive of the models measured, and its
fingerprint is restraint: *the flattest event escalation of any source*, the most
uniform narrative voice, epilogues over avalanche endings, and reverence toward
genre convention rather than subversion (62% vs 39–56%). Read that as a standing
instruction: **the default delve escalates too evenly and ends too quietly.**
Give a campaign a beat that is disproportionate to what came before, and let at
least one thing end badly or unresolved.

Operationally, per campaign:

- Pick **at least three** axes above and push them off the default *for this
  campaign* — a delve told out of order; a cast whose antagonist is right; an
  ending that refuses to explain itself; an NPC who names their fear in plain
  words instead of clenching a fist.
- Record the choice as a one-line **posture note** in `GENERATION.md`: which
  three axes, and how. It is a design commitment, not a report.
- Vary them **between** campaigns. A fixed counter-recipe applied every time just
  builds a second cluster — dispersion is the human signal, not any particular
  pole.
- Corollary, and it inverts the usual advice: **"show, don't tell" is a machine
  default here.** Somatic rendering is what the pole looks like. Sometimes let a
  character say they are afraid.

### C. HARD RULE — dialogue options are labels, not sentences

**A dialogue option is a button caption.** Vanilla draws each option as a
fixed-width button; a label wider than the button *scrolls* rather than wrapping
or shrinking, and a shelf of scrolling captions is a miserable thing to read and
pick from. This is not a style preference — it is the widget.

The geometry, so the budget is arithmetic and not taste. The compiler emits each
node as a `minecraft:multi_action` dialog with `columns: 1` and **no `width`
override**, so every option button is vanilla's default **150 GUI px**, leaving
roughly **146 px** for the label after the widget's inset. Dialog buttons draw at
pose scale ×1, so one font pixel is one GUI pixel — unlike `narrate` titles,
which `DW0330` budgets at ×4/×2.

**Width is measured in font pixels, not characters**, because `i` and `W` differ
by 3× and a Han glyph (advance 9) is 1.5× a Latin one (typical advance 6), so any
character count is unfair to whichever script it was not tuned for. The character
counts below are the authoring rule of thumb derived from those advances — the
pixel budget is the real rule:

| | scroll threshold | **author to** |
|---|---|---|
| English | ~24 characters (146 px ÷ ~6 px average advance) | **≤ 20 characters** |
| Chinese (`zh-*`) | ~16 characters (146 px ÷ 9 px Han advance) | **≤ 12 characters** |

Author to the target, not the threshold: the English is the source a translation
grows from, and a label at the English limit has nowhere to go in `zh-cn`.

```
BAD   "I don't know — are you sure there isn't another way out of the cave?"
GOOD  "Another way out?"

BAD   "我不太确定，你是说这座洞窟还有别的出口吗？"     (20 chars ≈ 180 px — scrolls)
GOOD  "还有别的出口吗？"                              (8 chars ≈ 72 px)
```

The content that does not fit belongs in the node's body text, which wraps
normally, or in the NPC's reply — not in the button. This applies to every
`.opt.<n>.label`, in the English source **and** in every l10n sidecar.

`DW0331` enforces this at compile time on the same font-pixel measurement
`DW0330` uses. Author to the target and it never fires.

### D. HARD RULE — a name spelled the same way IS the same name

Every name you write over a body is translated, and **whether two bodies share
one translation is decided by whether you spelled them identically** — not by
whether you meant the same character. Apply this while you are naming, because it
is unrecoverable later: by the time a translator sees the list, your intent is
gone and only the spelling is left.

**Bodies that are one character: spell the name byte-identically.** A character
usually occupies more than one declaration — an NPC that stands and talks, plus
one actor puppet per cutscene pose it is staged in. Written identically, all of
them are one name: the translator is asked once and every body renders the same
way, in every language.

```
GOOD  npc/polyphemus            "Polyphemus"
      actor/polyphemus-walker   "Polyphemus"      ← same character, same spelling
      actor/polyphemus-roused   "Polyphemus"
      actor/polyphemus-blinded  "Polyphemus"

BAD   npc/polyphemus            "Polyphemus"
      actor/polyphemus-roused   "Polyphemus "     ← a trailing space is a second
      actor/polyphemus-blinded  "polyphemus"        character, and the giant is
                                                    renamed mid-cutscene
```

Differ by a space, a case, or a `the` and the player meets two characters — one
of whom may be called something else entirely in Chinese. Copy the NPC's name; do
not retype it.

**Bodies that are genuinely different: spell them differently.** The rule runs
both ways. Two unrelated NPCs you both called `Guard` are one name and will be
translated once, so if they must read as two people, write two names.

**Wave mobs are the exception, and it is the one to plan around.** A wave mob's
name is *not* pooled with anything: three waves whose mobs you both named
`Drowned of Poseidon` are three separate names, asked of the translator three
times, and free to come back as three different Chinese strings — the same squad
under three names, in one delve. So:

- If several waves really are **one creature**, still write the identical string
  — it is the honest source, and the localization stage carries a glossary that
  holds proper nouns steady across batches. Then **say so in the campaign's
  posture note**, so the localization stage knows those rows must agree.
- If they are **not** one creature, give them names that differ. Do not reuse a
  name for flavour across waves that the fiction treats as distinct — you get the
  cost of a shared name with none of the benefit.

Fewer distinct names is the cheaper delve in every language. A name you reuse
deliberately is free; a name you reuse accidentally is a defect the English build
can never show you.

### E. Plain-prose baseline (Strunk 1918, public domain)

Two rules carry most of the load for text rendered into a chat line:

- Rule 12, "Use definite, specific, concrete language" — the objective hint that
  names a landmark beats the one that names a mood.
- Rule 13, "Omit needless words": *"Vigorous writing is concise. A sentence
  should contain no unnecessary words, a paragraph no unnecessary sentences…"*
  His substitutions are still live — `owing to the fact that` → since, `in spite
  of the fact that` → though, `he is a man who` → he, `in a hasty manner` →
  hastily. And: *"In especial the expression `the fact that` should be revised
  out of every sentence in which it occurs."*

Concision is not the same as flatness. Cut the padding, keep the beat.

## Reference: drawing the map's reference

Needed at step 2B on path B of Init step 6. On path A the views are already in
`design/reference/` and this section is how to *extend* the series, not how to
start one.

**The form is several views of the one subject, each its own image, generated in
sequence.** Front, side, straight-down plan, a named angle — whatever the shape
needs. **Not several views divided into one canvas**: a fixed canvas cut into
four spends three quarters of its resolution on gutters and neighbours, and the
detail a reference exists to preserve is the first thing to go. It also makes
the unit of judgement wrong — one unusable panel forces the whole sheet to be
re-rolled, where one unusable view is re-rolled alone for the cost of one image.

1. Write the **style note** once: what this place is, in what hand, plus the
   sentence that each image is ONE single full-frame view and never a sheet, a
   grid, a panel or an inset. It is held constant across the series
   (`--style-note`), and it is recorded in every sidecar, so a later round can
   extend the series instead of starting one.
2. **View 1 is generated from the prompt alone**, and confirmed for style before
   anything else is drawn. Frame it for what it shows.
3. **Every later view is generated from the prompt plus VIEW 1** — pass view 1's
   interaction id to `--chain-from`, read out of view 1's sidecar (`.id`).
   `.id` names the whole INTERACTION, and a chained call joins the one it was
   chained from — so every sidecar in the series carries the same `.id`, and
   reading it out of view 1's is how you name the series rather than a view.
   **Anchor every one of them on the FIRST image, never on the one before it**:
   chaining view to view compounds the drift instead of bounding it. Frame each
   for what it shows — a straight-down site plan is `--aspect-ratio 1:1`, an
   elevation is not — which is per call and never a config edit.

```bash

# the style note, written once and held constant for the whole series
STYLE="Halgrave, in the same hand throughout: <palette, light, brushwork>. Each
image is ONE single full-frame view of that place, filling the frame edge to
edge: never a sheet, never a grid, never a panel, never an inset or a caption."

# view 1 — from the prompt alone, framed as an elevation
python3 "$DELVEWRIGHT_ENGINE/tools/refimg.py" --prompt-file v1-front.txt --style-note "$STYLE" \
    --aspect-ratio 16:9 --out .refimg/map-view1-front

# read view 1's interaction id out of its sidecar — this is the series anchor
V1=$(python3 -c 'import json;print(json.load(open(".refimg/map-view1-front.json"))["id"])')

# every later view: the same anchor, its own prompt, its own frame
python3 "$DELVEWRIGHT_ENGINE/tools/refimg.py" --prompt-file v2-west.txt  --style-note "$STYLE" \
    --chain-from "$V1" --aspect-ratio 16:9 --out .refimg/map-view2-west
python3 "$DELVEWRIGHT_ENGINE/tools/refimg.py" --prompt-file v3-plan.txt  --style-note "$STYLE" \
    --chain-from "$V1" --aspect-ratio 1:1  --out .refimg/map-view3-plan
```

`$V1` is the same string in every later call, and that is where the "anchored on
view 1" claim is checked — the `chain_from` field in the sidecars, not a
sentence in a report. A frame the configured provider cannot honour is refused
rather than dropped, so a wrong flag stops the call instead of returning a
correctly-styled picture of the wrong shape.

**The trade, stated because it is why this step has a check in it.**
Co-generating views in one canvas is what guaranteed they agreed about the
*geometry* of the subject; generating them in sequence guarantees only *style*.
So **the geometric facts live in the written brief, and a drift is checked
against text rather than eyeballed** — which is exactly what `geometry-brief.json`
is, and why reference imagery is style authority and never dimensional
authority. Read each view against the brief's facts, not against your memory of
the last picture.

The check this exists to make possible: a zone program once exported as a flat
chain of rooms with no climb, no belfry and no bell, under the name of the tower
its campaign was named after, and it survived until somebody held it against the
zone's own image. A silhouette drawn from three sides is legible enough that the
same collapse cannot pass.

**When the views are confirmed they become campaign files** — copy them and their
sidecars to `campaigns/<id>/design/reference/`, named for the view, and commit
them with the campaign. An approval that lives only in a gitignored working
directory is bound to nothing, and the sidecar is what makes a view re-issuable
with one word changed: it carries the prompt, the style note, the resolved frame
and the anchor id.

## Reference: when the prefab library has no piece you need

Follow `$DELVEWRIGHT_ENGINE/docs/reference/prefab-procedure.md` — it is the procedure, and these are
its mandatory steps, in order. Do not improvise around them. All four binaries
this needs were built at Init step 2.

1. **Write the scene description first** — one or two sentences: what a body does
   in the space, the material feeling, what the campaign will attach. Written
   after the render, it is a description of the render.

2. **Choose the palette by measurement, never from memory** — and it is three
   steps, not one. A block's name is not its appearance (`packed_mud` is orange,
   142/107/80).

   **Screen** the shelf by constraints rather than by a guessed hex:

   ```sh
   python3 "$DELVEWRIGHT_ENGINE/tools/block-appearance.py" --screen --where full_cube --where 'L>=0.75' \
       --where 'C_mean<0.02' --where 'texture_range<=0.30'
   ```

   That takes 1146 blocks to a handful (`L` = Oklab lightness, `C_mean` = how
   coloured, `texture_range` = how loud the pattern; `form=`, `family=`,
   `not tinted`, `not gravity` are facets too). Then **measure the mix**:
   `--mix 'a=3,b=3,c=4'` or `--program p.json` reports `chroma_mass`,
   `chromatic_area`, the **named** `loudest_member` with its area share, and
   `dominant_hue` — never a mean as the verdict, because a mean cannot see that
   60% of a wall is one loud family when the craft rule gives it 10%. A member
   carries its block state, properties and all (`--mix 'deepslate[axis=y]=3,stone=1'`).
   **Read the binding line before the colours**: it reads `examined of declared`,
   declared being the palette's own role count plus each inline fill, so
   `18 of 18` is a measurement of the palette and `8 of 18` is not — a declared
   paint the tool could not read is named with its reason and exits 2.

   Then **LOOK**: `--sheet` writes `.sheets/palette/swatches.png`, every survivor
   tiled and every mix rendered as its seeded weighted tiling — **read that PNG
   before binding anything.** A shortlist is not a choice, and the screen will
   hand you blocks that are right on every measured axis and wrong for the job (a
   light source, a gravity block, wool). Record the measured hex beside each role.

   The tool needs the pinned block registry from `$DELVEWRIGHT_ENGINE/crates/dsl/data/` **and** a
   1.21.11 client jar, and refuses by name when either is absent. That does not
   make the step optional: take role names from the corpus instead
   (`delvec grammar list`, then `delvec grammar show --program <nearest>`), which is
   a palette somebody already measured, and record where each name came from.
   Never invent one — a block that does not exist is refused at export, and one
   that exists but looks nothing like its name is caught only by eye at 5 below.

3. **Author a grammar program.** Read the **idiom index** first
   (`$DELVEWRIGHT_ENGINE/docs/reference/grammar.md` §2c): ten techniques with a runnable program each
   — repetition, `otherwise`, taper/arch/gable (one recursion), air-in-a-mix
   erosion, graded erosion, surface detail, symmetry without reflection, `skip`,
   light, and arguments (`bind` — one rule called with different content). It is
   the part of the language no type signature shows, and a scene that looks
   impossible is usually one of the ten. **Never copy a rule to change its paint,
   its size or its axis**: a caller passes a paint or a size with `bind`, an axis
   with `reorient`, and anything derivable from the box with an expression over
   `dim` — a copied rule family is one nothing keeps in step and no gate reads.

   ```sh
   delvec grammar show --program idiom-shape
   delvec grammar list
   delvec grammar show --program <nearest> > p.json
   delvec grammar check --file p.json          # after every edit
   ```

   You write JSON — never Rust, and never blocks by hand. Four traps the
   procedure names: two guards that can both hold are a **probability, not a
   priority** (the "none of the above" arm is `otherwise`, and it is also what
   stops a recursion); **`rounding` is owed by every surface, not only floors** —
   the default truncates and an unwritten cell is air, which no gate reads; a
   palette role may be a **weighted list with `minecraft:air` in it**, which is
   the whole of decay and the cure for a piece that renders as one flat material;
   and a `facing=` block state **does not turn when the frame turns and does not
   flip when it reflects** — `oriented-fills` (`DW0736`) refuses the piece rather
   than shipping it facing the wrong way. Say which axes the state is written in:
   wrap it as `{"local": "minecraft:iron_bars[east=true,…]"}` and its directions
   mean the scope's own, so one palette role gives the right state at every frame,
   reflections included. Where the whole rule BODY differs by frame, use an
   `orientation` guard instead — one alternative per frame, naming the reflection
   as well as the axes.

   **Decide the split order before the first rule.** A split's children copy the
   parent box on the two axes it does not cut, so siblings of a split are the only
   two things guaranteed to line up, and there is no way to say "this opening is
   the same cells as that one". Hence: **the last axis you split is the only axis
   on which two things are guaranteed to meet — split last on the axis your
   openings run through**, and write a hole as a piece of that split whose
   siblings are the two things that must meet (best as the *absence* of a sibling,
   which cannot be misaligned). Within one axis, pin a course to a band's end and
   not to a height: `[relative 1, absolute 1]` is *the last course of this band* at
   any band height, where `[absolute 5, absolute 1]` is a computed height that also
   refuses a short band. Every constant you do not eliminate this way fails
   silently.

   One more refusal to expect: **`repeat` clamps the last tile but does not rescue
   a box too short for the first one** — one pass of the pattern is resolved before
   any tiling, so a repeat whose absolutes sum to 8 across a 7-deep box is a hard
   refusal. Guard the extent and give the short box an `otherwise` arm.

   **Then say where a body goes — the spatial contract.** The rules say what
   blocks stand where; they never say which voids are rooms, where the doors are,
   or what a neighbour may mate with. That is a `claim` node per body of space
   plus one `contract` block classifying the names, written in the same document.
   **The authoring surface is `$DELVEWRIGHT_ENGINE/docs/reference/grammar.md` §2d** — that section is
   the only place that says how, and `delvec grammar show --program
   spatial-contract` prints a runnable one. Write one whenever the piece has more
   than one way in and out. Step 4's `traversable` judges a piece that has none —
   it derives the sides the piece opens on from the blocks — but only a contract
   turns that count into declared ways in, and only a contract can state a way
   out the blocks cannot show: a piece entered from **above** binds zero there
   and reds. Budget it as part of authoring: the moment a `contract` block is
   present, **nine obligations run with no flag** at both
   doors that read the piece — every name must resolve, every standable cell must
   lie in something declared, an `enclosed` space must be closed except at a
   claimed opening, every declared edge must hold on the bytes, and every anchor
   must land in a declared element. An `exterior` edge is **one claim per space,
   not one per door**: with no `via` it exports one face for every outer face its
   space reaches, so writing one edge per end on an L-shaped passage exports each
   end twice.

   **Anchors are part of this step too.** `mark` (`grammar.md` §2b) is the only
   way a campaign can name a place inside the piece, and it is also what gives
   step 5 its interior cameras — a piece with no anchor gets no eye shots at all.
   `at: floor_center` takes the lowest **world** Y of the scope it sits on, so a
   mark wrapping a column that includes its own floor slab lands *in* the floor
   and reds `contract-anchors`; mark the void, or use `at: offset` with the
   walkable Y. A `mark` may also carry `role` (`grammar.md` §2b, one term today:
   `entry`) — that is how a grammar-authored piece declares the 2A entry point
   without needing to write a name it structurally cannot write; one anchor per
   area may carry it (`DW0804`), and every other anchor still binds by name.
   For a hand-built or ingested piece, where no `mark` ever ran, the same role
   is given after the fact with `delvec prefab anchor --role <term>` / `--no-role`
   (step 6) — run `--help` on the authoring pin's own binary before trusting this
   route name, since a later engine may fold it under a different command.

4. **Expand and let the machine judge**:

   ```sh
   delvec grammar expand --file p.json --region XxYxZ --seed N \
       --traversable --reachable-floor -o out/
   ```

   Pass `--traversable` for any passage, stair or route; pass `--reachable-floor`
   for any piece with an inside a body is meant to walk around. A red gate writes
   no `.nbt` (exit 4). **Read the `findings` in the report** — a gate that bound to
   zero objects, or a program that declared no anchors, is a finding, not a pass.

   `--traversable` asks the piece which faces it opens on, so a route is judged
   the same whichever way it runs and a passage that turns a corner is judged the
   same as a straight one. Where the piece declares a spatial contract those faces
   are its `exterior` edges and the binding count is doors; where it declares
   none they are the sides of the region its standable floor reaches, and the
   count is open sides — and a derived side is not a door, which the detail line
   says beside the number. Fewer than two is a refusal that names both repairs:
   open or declare the second way out, or stop claiming the piece is a route. A
   red here writes no `.nbt`, so it is not a warning to ship past. A piece
   entered from **above** lands there with a binding of **zero**, because a
   standable cell never lies on the region's top plane; that one is repaired by
   declaring the face at step 3, not by dropping the flag.

   Three of the always-on gates are about how a block state is SPELLED —
   `shape-complete` (`DW0735`), `states-complete` (`DW0737`) and `oriented-fills`
   (`DW0736`). Write every property of every block state you paint, including the
   ones whose default looks obvious: a state that omits one means whatever a
   running server decides, and the render you are about to check the piece against
   cannot know which.

   **`oriented-fills` has three answers, not two.** `undecided` (`DW0742`) means
   the piece is not wrong at this region and was not checked at it either: a scope
   reoriented by `largest`/`smallest` stands in the identity frame only while this
   box's axes happen to rank the way the request already names, and at another
   region the same state would be refused. It writes the `.nbt` and it is not a
   red — but it is the one verdict that will change under you the day the zone's
   region does. The named fills are in the gate's detail; wrap each as
   `{"local": …}` and the answer becomes `pass` at every region.

   **Read the `reachability` line too**, which prints whether you asked or not:
   `traversable` joins ground-level ways in and says nothing about the storeys
   above, so a building can pass every gate with half its floor stranded.
   Unreachable floor **under a roof** is a room with no way in, and the report
   gives you the box to go and look at. Unreachable floor open to the sky is a
   roof, and is nobody's defect.

   If the piece is one of a campaign's **zones**, its program belongs to the
   campaign: put it in `campaigns/<campaign>/design/programs/` and name it in
   `zones.json` there with the region, seed and gate claims it is built at
   (`traversable`, `allow_falls`, `reachable_floor`, `symmetric`).
   `delvec grammar audit --campaign-root .` judges every zone a
   campaign declares, and CI in both repositories runs it — a program that
   directory carries and the manifest does not name is a red.

   **One design the gate cannot be told about: a one-way descent.** A level a body
   drops into and does not climb back out of is unreachable on foot on purpose,
   and nothing in the CLI, the report or the metadata can state that claim. So do
   **not** pass `--reachable-floor` on such a piece — it fails and a red gate
   writes no `.nbt`, so the flag ships nothing rather than shipping a known red.
   Expand without it, read the always-on reachability line, and record in the
   campaign's `GENERATION.md` that the `unreachable_sheltered` pocket it names is
   the drop and not a room with no way in. That verdict is bounded by the
   instrument, and this is the step at which to say so.

5. **Look at it**:

   ```sh
   delvec render piece out/<id>.nbt -o shots/
   ```

   and compare against the scene description from 1 above. The gates prove it is
   buildable and walkable; they
   say nothing about whether it is the scene you asked for. If the expand wrote a
   tile set instead of one `.nbt`, pass the manifest — `delvec render piece
   out/<id>.json` — which renders the assembled zone as one scene, eye shots
   included. Never review a single tile; the command refuses one anyway.

   **Open the `eye-<anchor>.png` frames FIRST.** They are the only cameras inside
   the piece — a body's eye at 1.62, at each declared anchor, looking the way that
   anchor faces. A piece that declares no anchor gets **none of them**, and the run
   says so in its binding count; that is step 3's `mark` still owed, not a render
   fault. The exterior orbit shots (`ext-*`, `door-*`, `anchor-*`) are fitted from
   outside, and on a roofed piece they are all the same picture of the same rock —
   but **`top` is a cutaway plan**, the roof taken off, and on a piece whose
   identity is a route rather than a face — a passage, a junction, a stair — it is
   the only planned camera that sees the route. Do **not** re-aim it by hand: a
   `--view name=…,face=up` is `"cutaway": false` at the same pitch and
   photographs the roof. Where a loud palette flattens the plan, add a `--view`
   square at each open face (`--view name=east-mouth,face=east`) and read the two
   together. Read `<id>-shots.json` beside the images for which cell each body is
   standing in: a camera whose anchor cell held a gate or a barrel steps back
   along the facing and says so (`DW0727`), and an anchor with no body cell gets no
   eye shot at all — the run states that count. A flat grey frame is outside the
   piece, and a shot that is *only* that is reported as an empty frame: the camera
   is aimed at nothing.

   **When the piece is a building whose identity is one elevation** — a west front,
   a gatehouse, an approach face — add the camera for it: `--view
   name=west-front,face=north` (repeatable) appends a level, square-on shot of that
   face of the model, and no planned camera is square-on at a face. `of=` aims it
   at a declared anchor instead of the whole model; `zoom=` tightens or backs off.
   Do not build a forecourt and stand an anchor on it: a 70° eye camera reaches
   only ≈0.7 × its distance above eye height, so it looks through the doorway
   instead of at the façade, and the forecourt shrinks the building in every
   exterior frame. Keys: `$DELVEWRIGHT_ENGINE/docs/reference/tools.md` §4.

6. **Admit it**: the `delvec prefab` chain, which for a generated piece is
   `audit` → `socket` → `lighting --write` → `audit` again.

   ```sh
   delvec prefab audit    out/<id>.nbt        # a TILE SET passes out/<id>.json
   delvec prefab socket   out/<id>.nbt --pos X,Y,Z --facing <dir> --opening 3,3 \
                        --name <ns>:<name> --target <ns>:<name> --pool pool/<name>
   delvec prefab lighting out/<id>.nbt --write
   delvec prefab audit    out/<id>.nbt
   ```

   **Hand `audit` the `.nbt`, not the `.json`** — the metadata beside a single
   template is not a manifest and is refused (`DW0732`, exit 2). Only a tile set
   has a manifest, and then `audit` and `lighting` both take it and answer about
   one zone; handing any command a single tile is `DW0739`, and so is handing it
   a tile copied away from its manifest.

   Two subcommands are **not** on this route. `delvec prefab anchor` writes a place
   into an anchor whose producer could not — a hand-built or ingested piece; a
   grammar program already declared its anchors with `mark` at step 3.
   `delvec prefab catalog validate` reads a **catalog card**, the per-asset
   verification record of the ingestion route (`catalog/<asset-id>.json`), which
   is a different document from prefab metadata — run on a prefab's `.json` it
   correctly reports that file is not a catalog card.

   **`socket --pos` is the jigsaw cell: bottom-centre of the opening, in the wall
   plane.** The opening is built from it — width centred on it, height climbing
   from it — so read it off the piece: with a spatial contract, the metadata
   written beside it carries each exterior face's opening as a `from`/`to` cell
   pair (`spatial_contract.faces[]`), where step 4's report and `audit` name only
   the face's direction and cell count; with none, a `mark` whose scope is the
   mouth itself records that cell. `--facing` points **out** of the piece;
   `--name`/`--target` are what mate one piece to another; `--pool` is the
   `prefab_pool` the far side comes from. The carved socket leaves a
   `minecraft:jigsaw` block in the doorway carrying `final_state: minecraft:air`,
   so the world replaces it with air at placement — a re-run `audit` reporting one
   fewer cell on that face, and `minecraft:jigsaw` in the palette, is the marker
   being counted and not a blocked door.

   `socket` is also the **only** step that edits the blocks, so a piece that
   carries one no longer matches what its `license.generated_by` row regenerates;
   that row reproduces the `.nbt` as step 4 expanded it.

   A grammar prefab has **no connectors and no lighting** until this step, so it
   cannot enter a `prefab_pool` and will be dark, until you do it. `lighting`
   measures the darkest standable floor cell a body can walk to from a
   ground-level entrance, and reports the count it bound to.

   **Which sky the piece is measured under is the piece's own claim**, read off
   its `spatial_contract`. A piece that declares no contract, or one declaring
   any space `open` or `open_top`, is modelled standing in open air: sky light
   enters through its openings from the side, which is the only way a roof over
   open ground is ever lit, and without it a colonnade, a portico or a pier under
   a deck reads as pitch black. A piece whose contract declares **every** space
   `enclosed` claims no floor of its own stands under the sky, so it is measured
   with no sky at all and its figure is its own block light. That is the honest
   measurement for a piece a `detail-plan` row binds — its frame is the play
   space plus one floor course and the roof over it belongs to the whole — and it
   is why a `lit` verdict borrowed from open sky is worth nothing there: the same
   emitterless piece reds `DW0210` at the first build. The report says which sky
   was applied and why, in `assumed_sky.admits_sky` and `assumed_sky.why`, and the
   written `method` sentence says it again. Read them; a figure taken under the
   wrong sky is a number, not a verdict. Every piece a `details[]` row can bind
   declares a contract, because one that declares none is refused outright
   (`DW0843`).

   Two refusals to expect and not work around: `DW0752` means the
   probe bound to **zero** cells — usually a piece whose only way in is a socket
   that has not been carved yet, so run `socket` first; `DW0753` means there is no
   metadata to write into, and the fix is to create it, never to let the tool
   invent a `spdx: UNKNOWN` one.

**What the grammar cannot express — escalate, do not work around**: block
entities of any kind (chest loot, sign text, spawners — bind those in the
campaign against an anchor the piece declares), **smooth** curves, diagonals, a
profile step that varies independently of the box, a vault bending on two axes at
once, and terrain. **Neither a stepped arch nor a symmetric shape is on this
list** — the first is idiom 3 (one recursion whose step is arithmetic on the
remaining dimension, and the same program inverted is the opening), the second is
idiom 7 (a rule body written mirrored, since `reorient` permutes and never
mirrors). Check §2c before escalating. **Size is not on this list** either: a
region of any extent expands, and one past the 48-per-axis structure-template cap
is written as a tile set plus a manifest at `<id>.json`. Never shrink a scene to
fit a file format. **Tiling changes nothing downstream of the export**: the
campaign binds `prefab/<id>` in the same line it binds any piece, its anchors are
the zone coordinates the program marked, and world assembly places the whole
zone. Nothing an author writes says a piece is tiled, and a document that mentions
a tile is wrong.

A piece that comes from **outside** — a community schematic — instead enters via
`delvec schem convert` and then the same admission chain with `resolve-jigsaw`
before `socket`. Never place an un-audited piece: `audit` is the licence and
code-injection gate, and the `DW0733` check that the blocks in it exist at all.
Flags in `$DELVEWRIGHT_ENGINE/docs/reference/tools.md` §2a and §3.

## Reference: other languages

Needed only when the brief asks for one — or when the brief arrives in a
non-English language **and asks for localized in-game text** (中文文本 etc.). It is
a **final document stage after `dialogue`**, once the English campaign is
complete. Everything in the campaign documents stays English, always; other
languages are delivered as sidecars.

1. Declare the codes in `world.json`: `"languages": ["zh-cn", …]` (BCP-47-style;
   `en` is implicit and canonical and is **never** listed). Each code must be one
   the compiler can map to a Minecraft lang-file name (`zh-cn` → `zh_cn`); an
   unmapped code is `DW0184` at validate time, never a language quietly missing
   from the shipped pack.
2. **Who translates.** If `"$DELVEWRIGHT_ENGINE/delvewright.toml"` or its `.local` sibling has an
   `[i18n]` section AND the environment variable it names (`api_key_env`) is set:

   ```sh
   python3 "$DELVEWRIGHT_ENGINE/tools/i18n-translate.py" "$PWD/campaigns/<id>" \
       --lang <code> --reflect
   ```

   `--reflect` is the three-step translate → critique → revise pass and is where
   translationese actually dies — always pass it. It writes and validates the
   sidecar for you; then go to 4 below. Otherwise translate yourself, 3 and 4
   below.
   Generation-time only either way — a shipped delve never calls a model.
3. Yourself: `delvec --prefabs prefabs l10n-inventory <campaign-dir> --lang <code>` gives the exact
   key inventory as JSON (key, English, speaking NPC, existing translation).
   **Translate FROM the finished English** — never author a language natively —
   honour each NPC's `persona.speech_style`, keep a Minecraft-appropriate
   register, cover the inventory **exactly**. Run the **same three-step pass the
   tool runs**: draft; then re-read the draft against the English and write down
   what is wrong on accuracy / fluency (including the target language's
   translationese habits — for zh: 名词化, 弱动词, 的的不休, over-marked 被,
   front-loaded modifiers) / style-register / terminology; then revise, leaving
   lines that were already right byte-identical. Write `l10n/<code>.json`:
   `{ dsl_version, campaign_id, kind: "l10n", lang: "<code>", content: { <key>: … } }`.
4. Re-`validate` until zero `DW0180`/`DW0181`. **The default build ships every
   declared language and the client picks its own**: `delvec build` emits each
   authored string as `{"translate": key, "fallback": English}` and writes
   `assets/delvewright/lang/<mc_code>.json` per language into the delve's
   resource pack. A player whose locale you do not ship — or who declines the
   resource-pack prompt — reads the English fallback. Nothing extra to run.
   `delvec build --lang <code>` produces the single-language bake for local dev;
   the release path does not use it. `critical-path.json` is language-neutral
   either way, so the ladder is unchanged.

Then re-run step 6, `delvec fmt` — it covers the sidecars too.

**`fx.` keys are POSITION-derived** (`fx.<quest>.oc.<obj>.<index>…`). Inserting
an effect into a list SHIFTS every sibling's key and silently re-attaches old
translations to the wrong lines. When editing effect lists on a localized
campaign, APPEND rather than insert where order allows, and after any structural
edit re-check every shifted key's translation against its new English source —
exact-key coverage (`DW0180`/`DW0181`) cannot see a stale value.

## Reference: tools by symptom

Two classes, one rule each:

- **Workflow tools are steps, not options.** Where a step above says "always" or
  "mandatory", skipping it is skipping validation.
- **Human-in-the-loop tools are offered, never required.** When the flow reaches
  the marked point, say in one line that the tool exists and what it would catch
  — then keep going. Never block or wait on a use/don't-use answer.

The full inventory — every binary, script and flag that exists today — is
`$DELVEWRIGHT_ENGINE/docs/reference/tools.md`. Check it before assuming a capability is missing.

- **Judging any visual outcome** (cutscene framing, set dressing, terrain):
  `delvec snapshot` (`--camera x,y,z,yaw,pitch`, `--at <anchor> --dist`,
  `--shot <render-plan id>`, `--labels`) and `delvec blocking-chart` (per-floor
  cutaways). *Always* for cutscenes: render start, mid and end of every dolly
  segment and **look at the frames** before calling the step done — `DW0308`
  proves the path is air, not that the shot is pointed at the subject, and an
  inside-out cinematic can be fully DW-green.
- **Terrain or visual fixes beyond swapping prefabs**: `delvec edit` — the map
  editor loop (edit script batch → replay → snapshot). The script is a campaign
  document, `world-edits.json`: batches of declared edits, replayed
  deterministically, with the post-batch invariants enforced. Never hand-patch
  `.nbt` or invent block edits outside it.
- **Handing a build to a playtester**: the playtest note flow — `/trigger dw.note`
  in-game, then `delvec harvest` → `playtest-report.json`. Human-optional.
- **Delivering or revising a cutscene**: shot calibration — in-game
  `/trigger dw.mark set <s>` (stand where the camera should be), `dw.aim set <s>`
  (look at the subject), `dw.faster`/`dw.slower set <s>`, then `/trigger dw.done`
  once; `delvec harvest` writes `rehearsal-report.json` and `delvec calibrate
  <report> --layout <out>/creator-datapack/layout.json` turns it into an
  anchor+offset patch you apply and rebuild. Human-optional. (Beat replay —
  `dw.beat` / `dw.shot` / `dw.free` — does not exist; do not promise it.)
- **A prefab library needing human taste, not machine checks**: `delvec prefab
  gallery` (browse world) → a reviewer walks it and leaves notes → `delvec prefab
  curate` / `curate-merge` fold them into the catalog cards. Human-optional.
- **Several candidate prefabs for one slot, and a human has to pick**: `delvec
  contact-sheet <renders> -o <png>` — all the candidates on one page, each
  labelled with its rank and id, with `$DELVEWRIGHT_ENGINE/tools/refscore.py` optionally ordering the
  page by similarity to the design gate's reference image. Human-optional. Say
  plainly that the score only orders the page: every candidate is on it, and the
  low scorer is present, last — the human is the selector, the number is not.
- **A picture cannot say what a prefab is like to be inside**: `delvec --prefabs prefabs viewer
  <nbt|dir|manifest.json> -o <page.html>` — one self-contained HTML page with a
  camera the reviewer drives: exterior, plan, a player point of view at eye
  height (1.62) standing at every declared anchor and doorway, plus a cutaway
  slider that takes the roof off. A zone that ships as several tiles and a
  manifest shows as one building. Pass a directory to put a whole library on one
  page. Human-optional; read its fidelity list first (step 12).
- **A picture of the whole map** (storybook hero image, release asset): `delvec
  panorama <build-dir> -o <dir> [--bearing se|sw|ne|nw] [--spp N]` — a 45° oblique
  scene framing the entire layout, computed from the plan. Never hand-edit a scene
  JSON to get one.
- **An NPC needs a look no vanilla mob gives you**: the skin toolchain,
  `PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/skin" .venv-skin/bin/python -m delve_skin all <cast.json>
  --skins-dir … --preview-dir … --catalog-dir …`, in the venv from Init step 7. **Look at the previews**, and
  always set `model` (`wide`/`slim`) — an omitted model renders slim and distorts
  a wide skin. The compiler bakes the PNG into the delve's resource pack from
  `campaigns/<id>/skins/`.
- **Cleaning up a ladder or a play session by hand**: `$DELVEWRIGHT_ENGINE/validation/fresh-volumes.sh
  --project <id>`. `--project` is required everywhere and there is no daemon-wide
  mode. It reclaims what the project owns — containers, volumes and networks —
  and proves it. The `--profile play` stack from step 9 pins a fixed container
  name, so tear that one down with `docker compose … down -v` or
  `$DELVEWRIGHT_ENGINE/tools/playtest-server.sh down` rather than with `fresh-volumes.sh`.

## Reference: when something goes red

The symptoms most likely to stop you, and what they actually mean.

**`DW0874`: "this campaign directory is missing N of the 6 stage documents"
(exit 1).** Not a fault — `delvec validate` is whole-campaign and needs all six
required documents on disk. The refusal names every one that is absent, the
whole set of six, and what a stub is; stub the ones you have not written yet.
Step 1 has the names and the one document whose stub is not empty.

**`internal error: cannot read campaign dir: …` (exit 10) instead.** That is a
different finding and it is not an authoring state: either the path is not a
campaign directory at all, or a document is there and cannot be opened. Check
the path first.

**Step 3 ends red and nothing you do clears it.** Expected — read *Where step 3
ends*, which gives the rule: a refusal whose message names something only
`quests.json` or `dialogue.json` can supply is the state of a campaign whose
plan is written and whose quests are not, and it clears at step 5. It is a list
of codes, not two, and the list is longer than it looks — `DW0172`, `DW0482`
and `DW0197` are the same absence seen from the branch, the fork and the cast.
Writing empty stage-5 quests to get past them makes it worse, and `DW0150` says
so.

**`DW0311`: "the player cannot walk from [x] to [y] … no collision-free path",
and the two points are hundreds of blocks apart.** They are in different areas,
and the crossing between them was never emitted. The message's own suggestions —
a wedged doorway seam, a void gap, a fence ring, a `close-gate` to reopen — are
about the same-area case and will not apply. Read step 2A: **the destination
area's piece must declare an entry point**, and only 5 of 36 shipped prefabs do.
Bind that area to one of them, or make a piece that has one.

**The build is green, PackTest is green, and the bot fails its FIRST step with
"No path to the goal!"** The party's first objective is in an area they did not spawn in,
and no crossing carries them there — the delve is not completable and nothing
before the bot said so. Read step 2A rule 1: put a beat in the spawn area first,
or move the spawn. Confirm by reading `critical-path.json` at the build output's
root: the first `reach`/`talk-to` step should be at coordinates inside the spawn
area, and a step that crosses areas carries a `transport` key.

**`DW0438`: a `collect` container "holds `minecraft:air`, not a container".**
The anchor you pointed at is not standing on a chest or barrel — an anchor named
`anchor/chest` is not evidence there is one, and two shipped pieces declare
exactly that over air. Read *Reference: what a quest can do* → *Items,
containers and loot*: use `dropped_by`, or drop the `container` field and let the
compiler place one, or make a piece that carries the container. Do not point the
field at a different anchor and hope.

**`DW0857`: an anchor "is provided by 2 of this campaign's areas".** You bound
one prefab to two areas. The remedy available to you is **a different piece for
one of them** — not renaming an anchor, which belongs to the shared library and
would change it for every campaign.

**A bot step times out with "objective … did not complete".** Read the rest of
that line before touching the campaign. The bot reports the server's own answer
to the `/trigger` it sent: *the server ANSWERED …* means the trigger reached the
delve and a datapack guard consumed it — a re-used world whose scoreboard already
carries the objective does exactly this, so run `fresh-volumes.sh --project <id>`
and re-run before believing the content is at fault. *The server never
answered …* means the command never got there and the failure is the harness's,
not the delve's.

**`failed to read dockerfile: open Dockerfile.delve`.** The build tree is
outside the engine's `validation/`, on one of the paths that resolves the
Dockerfile against the tree: `branch-runs.sh`, or a bare `docker compose` on the
`play` or `playtest` profile. `packtest-run.sh` and `bot-run.sh` pass an
absolute path and never raise it. Build into
`$DELVEWRIGHT_ENGINE/validation/delve-output`, or copy the tree there — see step 8.

**`refimg: no delvewright.local.toml` (exit 2).** Init step 6, path B — and it
is the state **every** fresh engine checkout is in, not a file that went
missing. The name is gitignored in the engine, so nothing ever shipped one and
nobody deleted it: writing it is the step, and it is owed again for every new
checkout. The file belongs at
`"$DELVEWRIGHT_ENGINE/delvewright.local.toml"`, not here — the tool resolves it
against its own checkout and takes no `--config`. It names
the file, the section, and where the template is. If the campaign already has an
approved `design/` directory you are on path A and do not need this at all.

**The staging gate refuses with a long UNBOUND list.** Not a defect count — see
step 9. Read it item by item into the round summary; override deliberately if
the session needs a red build.

**`delvec metrics` "prints 341 lines of JSON, not a table".** It prints the
table as JSON on stdout and its human summary and binding counts on stderr.
Redirect them separately: `delvec metrics > table.json`.

**A red that came from the toolchain rather than the campaign.** Stop content
work and report it, with evidence. Never hand-edit compiler output, never
restructure the campaign to dodge it, never weaken a check or reroll a seed to
get green. Escalating is success.

## Reference: authoring pitfalls

- **Difficulty is declarable** (`world.difficulty`: `easy`/`normal`/`hard`).
  Absent, the compiler derives `easy` for a wave campaign — which HALVES the
  damage players take (`min(dmg/2+1, dmg)`), the setting behind "the enemies are
  too weak". A souls-style brief almost certainly wants `normal` or `hard`; when
  you change it, retune the combat arithmetic (mob `attributes`, class gear, wave
  sizes) rather than only flipping the keyword. `peaceful` is rejected
  (`DW0468`) — it deletes every hostile. Scripted `actors` take the same
  `attributes` block wave mobs do, so an elite can be tuned on both its staged
  puppet and its unleashed twin.
- **The machine proves the LOOP, not the win.** Three things are checked about
  every mandatory encounter, and it is worth authoring toward them rather than
  discovering them as red builds:
  1. *Winnability arithmetic* (`DW0470`–`DW0473`): a required hostile must be
     damageable (Resistance amplifier 4 is total immunity — use at most 3, or put
     the durability in `attributes.max_health`), must have a standable cell beside
     it to be fought from, must fall inside the time-to-kill budget, and no
     `damage-players` in a quest bundle may land ≥ 20 (a full-health player) —
     that is a scripted death, not difficulty. A hit the party can dodge (a trap
     payload, a stealth `on_caught`, a `damage-players` with a `within` zone) is
     deliberately outside the check.
  2. **Declare `attributes.max_health` on every mandatory wave stack.** Vanilla
     publishes no per-entity default attributes, so an undeclared stack gets no
     numeric bound at all and the build warns `DW0475`.
  3. *The die-retry ladder stage*: the bot rests at every bonfire on the path (a
     fire only ARMS on arrival — the respawn point moves when the party RESTS),
     then deliberately dies twice at every encounter and proves respawn at the
     governing checkpoint → the route back → the encounter is still finishable →
     no completed objective was lost. Author with that in mind: every encounter
     needs a checkpoint or bonfire that governs it, and a wave the party must be
     able to re-fight wants `respawns_on_rest`. Leaving it off is legitimate — a
     won fight stays won, and the stage records that as `cleared-before-retry` and
     passes it. What it reds is `stranded`: nothing left to fight AND the
     objective unfinished, so the party can neither complete the encounter nor
     re-fight it. Turning `respawns_on_rest` ON buys a stricter check: the wave
     must come back WHOLE — declared count, all-new mobs, full health — because a
     retry must never let the party grind a fight down one swing per death.
  4. *The inverted floor gate*: mark a set-piece fight `tier: "elite"` or
     `"boss"` — on the **wave** or on the **actor**, same three keywords. The
     ladder then gives it one UNASSISTED bot attempt; if the bot — a poor fencer
     by design — wins cold, the run reports the fight as too easy for its billing.
     Leave ordinary pressure waves unmarked: they carry no such expectation.
     Marking is how you opt into the scrutiny, so mark honestly. **Mark the actor
     when the elite IS an actor** — the kneeling armoured thing that stands up
     when struck is a `spawn-actor` + `unleash-actor` beat, not a wave, and an
     unmarked one is a boss no proof ever looks at.
  5. *A tier the gate cannot measure is said out loud, not swallowed*: the gate
     warns on a first-try win and is silent otherwise, so an encounter nobody
     fought would look exactly like one that was fought and lost. The compiler
     therefore warns `DW0477` — and records `floor-gate: not covered (reason)` in
     `validation/combat-plan.json` — for a tiered actor no `unleash-actor` beat
     ever wakes (an `Invulnerable` puppet is scenery; a `vulnerable` one is `NoAI`
     and never swings back), and for a tiered wave no critical-path `kill`
     objective names. If you meant it as a fight, add the unleash or the `kill`
     objective; if you meant it as set dressing, drop the tier.
  Ordinary fights run the ladder under a bounded, logged combat assist, so bot
  fencing skill never caps how hard the delve is allowed to be.
- **Bonfires owe the party a flask.** Right-clicking a `bonfire` opens exactly two
  options — *rest and save* (full restore: health, hunger, negative effects
  cleared, flask refilled, checkpoint moved, `respawns_on_rest` waves re-seated,
  `on_rest[]` fired) and *save only* (the checkpoint, nothing else). The
  replenished item is a class-kit entry marked `"flask": true`, and **every class
  kit in a campaign that places a bonfire must declare one** — a bonfire campaign
  with a flaskless kit is the build error `DW0476`. Author it as a real recovery
  consumable with the per-rest budget you tuned against as its `count`: resting
  sets the stack back to exactly that number, up or down, so the flask is a budget
  and never a stockpile.

  **A potion must say what is in it.** A `minecraft:potion` (or splash/lingering
  potion, or tipped arrow) with no `contents` is vanilla's *Uncraftable Potion* —
  it heals nothing however you name it — so declaring one is the build error
  `DW0487`. Either name a vanilla brew or list the effects:

  ```json
  { "item": "minecraft:potion", "count": 5, "name": "Ashen Flask", "flask": true,
    "contents": { "potion": "minecraft:strong_healing" } }

  { "item": "minecraft:potion", "count": 5, "name": "Ashen Flask", "flask": true,
    "contents": {
      "effects": [
        { "effect": "minecraft:instant_health", "amplifier": 1 },
        { "effect": "minecraft:regeneration", "duration": 200, "amplifier": 0 }
      ],
      "color": "#ff9c30"
    } }
  ```

  `potion` is a 1.21.11 potion id, where strength and duration are part of the id
  (`minecraft:strong_healing`, `minecraft:long_night_vision`) rather than separate
  fields. `duration` is in **ticks** (20 = one second) and is required for every
  lasting effect — and forbidden on the instantaneous ones
  (`instant_health`/`instant_damage`), which land once on drinking. `amplifier` is
  0 = level I. Anything vanilla cannot pour is `DW0486`. The bonfire's three
  dialog strings default to canonical English; author `prompt`/`rest_label`/
  `save_label` only when the fiction wants its own words, and keep the two labels
  button captions (`DW0331`).
- **Place a bonfire OUT of every hostile's reach.** A rest point is where the
  party respawns and where every `respawns_on_rest` wave is put back on its feet,
  so a fire inside a hostile's `follow_range` delivers the party straight into
  combat on arrival — the build error `DW0478`. The clearance is measured against
  where the force actually IS: a wave's seated spawn cells, and for a `lane` wave
  the whole marched polyline (a lane wave walks its corridor while you are
  elsewhere, so a fire beside the far end of a lane is a fire in the lane).
  Fighting actors — anything `unleash-actor`ed, or staged `vulnerable` — count
  too, at their staging anchor. Put fires in side rooms, past the threshold, or
  beyond the end of the lane; never buy the clearance by shrinking `follow_range`,
  which retunes the fight to hide the placement. A re-seated wave always comes
  back **stationed** — a lane wave at its lane start, a plain wave at its anchor —
  so the safe zone stays true across every rest and every death.
- **Wave tuning**: `follow_range` below ~16 means distant wave mobs never engage;
  a kill objective whose mobs idle is unfinishable-in-practice even though
  machine-valid. Undead waves burn in daylight — the ONLY sanctioned fix is a
  helmet on the mob: `equipment.head`, any head item, on every burning stack the
  party is asked to fight. **Never `set-time`**; the delve's hour is a pacing
  decision, and moving it to save a mob spends a beat. The compiler enforces this
  (`DW0496`): a species in vanilla's `#minecraft:burn_in_daylight` staged for a
  `kill`-adjudicated fight whose walkable ground reaches open sky, under a pinned
  clear daytime hour, with an empty head slot, is a build error naming the sunlit
  cell. Roofing the arena clears it too. One species the helmet does not save — a
  phantom burns through it — so an open-air phantom fight has to be roofed or
  restaged. Never route wave mobs like actors: waves are native AI; if a beat
  needs lane-then-fight movement, that is the routed-then-feral primitive, which
  does not exist — not a `follow_range` trick.
- **Open-air by default**: stage scenes in the open unless a beat NEEDS enclosure
  (a cave passage, an interior puzzle, a reveal). The horizon — surround terrain,
  sky, backdrop — is part of the composition; a campaign of enclosed boxes wastes
  it. When an enclosed beat is necessary, prefer routing the player back into the
  open between beats over chaining interiors.

## Playtest rounds

Generation is round 1. Everything after it is an iteration round against the
playtester's findings, and the playtest hour is the scarcest resource in the
pipeline. Full derivation: `$DELVEWRIGHT_ENGINE/docs/reference/playtest-methodology.md`. Mandatory
here:

1. **Keep a findings ledger in `GENERATION.md`** — one row per finding: number,
   its wording as reported, the round it was reported, status. Status is
   `fixed@rN`, `open`, `engine` (blocked on a capability gap), or `ruled` (closed
   with no code change). This table is the campaign's memory; a finding that lives
   only in chat is a finding that will be reported to you twice.
2. **Triage every finding the day it arrives**, as *content* or *capability gap*.
   A capability gap — the DSL has no way to express what was asked for — is never
   patched downstream and is therefore a **staging blocker**: either the engine
   work lands before the next playtest, or the round summary says, per item, that
   it is still open and not to test it. A finding that survives more than one
   round is a capability gap; staging a build while those rows are open is what
   makes a playtester meet the same defect twice.
3. **Close each finding twice: the instance, and the general form.** After fixing
   the instance, ask what rule it is an instance *of*, and file that rule as a
   diagnostic (the DW code is minted for you — never mint one yourself). When the
   diagnostic exists, **re-run it against the current build**: that sweep is the
   deliverable, not the code, and it routinely finds a second live instance the
   moment it lands. Where no diagnostic is possible, write that down; it becomes a
   risk item at the next staging review.
4. **Append every finding to the engine repository's
   `$DELVEWRIGHT_ENGINE/docs/playtest-findings.json`**,
   the same day, with its general form and the check that carries it — this is the
   cross-campaign ledger, and `GENERATION.md`'s table is the per-campaign view of
   it. A finding recorded only in the campaign is a finding the NEXT campaign
   learns nothing from.
5. **Audit the FULL ledger from round 1 before staging any build** — never from
   the last round, and never by reading. You do not have to remember to: the
   staging paths REQUIRE it (step 9). Run it yourself first so the red list is in
   the round summary before anyone is invited.
6. **Pre-flight, in this order, before the invitation**: full ladder green
   (PackTest → bot critical path + die-retry → every branch run) → staging gate →
   localized builds and a double-build that is byte-identical → server boots and
   self-checks → then invite. Not "the build compiled, come look".
7. **Update `DESIGN.md` in the same round and run its conformance review.** A
   design record left unupdated across rounds accumulates changes nobody asked
   for, and the audit that catches up finds them all at once.
8. **Close the round in `GENERATION.md` with its machine record**, not just prose:
   how many validation-loop iterations it took to reach green, and every DW code
   the round hit **with its count** (`DW0205 x3, DW0483 x3, DW0450 x1`). Write it
   even when the count is zero — a round that hit nothing is the datum that says
   the gates had nothing to say. It is the only source from which rounds-to-green
   can be read afterwards; a round summarised in prose alone is a round whose cost
   cannot be recovered.

## Hard rules

- Persist the campaign documents before validation, not after — a crash must
  never lose the campaign.
- **Apply a ruling at the scope it was given.** If a wider rule seems right,
  propose it in one line and wait — generalizing a ruling is a design decision,
  not an inference to make silently. A one-beat pacing ruling read as a
  campaign-wide ceiling is a campaign nobody approved.
- **Unrequested change is a rejection cause on its own**, independent of whether
  the change is good. Author what the round asked for; anything else you believe
  the campaign needs is a proposal in the round summary.
- Every player-visible string in the campaign documents stays **English** —
  always. Other languages are sidecars. A brief written in Chinese still yields
  English documents; add a `zh-cn` sidecar only when localized in-game text is
  asked for.
- **Commit only canonically formatted JSON.** The last thing before any `git add`
  of a document or sidecar is `delvec fmt <campaign-dir>`; CI runs
  `delvec fmt --check`. A diff that rewrites a file nobody edited is the defect
  this closes.
- Homages: original text only. Cultural reference, never asset ingestion.
- If a mechanic the brief wants has no DSL verb, do NOT fake it with adjacent
  verbs silently — say what is missing and offer the closest authorable
  alternative. A change to the language itself is not made from inside a campaign.
- Never weaken a check, a test or a threshold to get green, and never reroll a
  seed. A red check is information. Fix the cause or escalate; escalating is
  success.
