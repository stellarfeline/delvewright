# Step 14 — handing it over


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
`$DELVEWRIGHT_ENGINE/validation/render-shots.sh`'s scene set, plus `delvec --prefabs "$DELVEWRIGHT_PREFABS" panorama <build-dir> -o
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

**The host line, for a server strangers join.** A storybook whose reader is going
to leave the delve running for people they do not know owes them one more line,
because the delve does not clean itself by default:

```
Add `-e DELVE_RESET_WHEN_EMPTY=90` to the run command and the world is thrown away
and built again from the image once nobody has been online for 90 seconds — so the
next arrival starts a delve nobody has touched. Leave it out and the world is kept,
which is what a group playing together over several evenings wants.
```

Say the consequence in the same breath, because it is the whole meaning of the
flag and it is not guessable: **with it set, every start resets** — restarting the
container under a party ends that party's run, since the removal sits on the boot
path and the boot is the only way in. The floor is 60 seconds and a smaller value
refuses to start.

**Then report to the user** — this hand-over ends the run: the campaign
summary, the playtime estimate, the validation results, what the walk found and
what was done about it, anything still open, and the two commands they will
actually use.

```sh

# play — one command: build, gate, serve, and print the connect line
"$DELVEWRIGHT_ENGINE/tools/playtest-server.sh" up campaigns/<id> \
    --prefabs "$DELVEWRIGHT_PREFABS" --delvec "$(command -v delvec)" --out "$PWD/.out/delve"

# playtest, with in-game notes
EULA=TRUE CREATOR_NAME=<mc name> docker compose -f "$DELVEWRIGHT_ENGINE/validation/compose.yaml" \
    -f "$DELVEWRIGHT_ENGINE/validation/owner-play.yaml" --profile playtest up
```

`owner-play.yaml` is what publishes `localhost:25565`; the base compose file
publishes nothing. Both paths run the staging gate first — see step 9.

---
