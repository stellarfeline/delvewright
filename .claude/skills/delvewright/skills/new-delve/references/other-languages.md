# Reference: other languages

Needed when the user asked for one — which step 1 asks them, so this page is
reached with an answer in hand rather than a guess. A brief that arrives in
another language is a reason to ask, never an answer by itself. It is
a **final document stage after `dialogue`**, once the English campaign is
complete. Everything in the campaign documents stays English, always; other
languages are delivered as sidecars.

1. Declare the codes in `world.json`: `"languages": ["zh-cn", …]` (BCP-47-style;
   `en` is implicit and canonical and is **never** listed). Each code must be one
   the compiler can map to a Minecraft lang-file name (`zh-cn` → `zh_cn`); an
   unmapped code is `DW0184` at validate time, never a language quietly missing
   from the shipped pack.
2. **Who writes it.** You write the English, per *Reference: writing craft*
   §F, and the campaign documents already carry each line's intent (the beat's
   `happening`, the cast's `doing`, the persona). The other language is
   **transcreated from that intent by the provider**: rewritten as a native
   writer would write it, with the facts, names and numbers of the English.
   **Set up DeepSeek for Chinese** — in-agent Chinese measured worse in a blind
   native-reader trial (`$DELVEWRIGHT_ENGINE/docs/reference/game-writing-evidence.md` §7),
   so a DeepSeek key is the recommended setup, not an optional upgrade.
   Setup: if `"$DELVEWRIGHT_ENGINE/delvewright.local.toml"` has no `[i18n]`
   section, copy the commented `[i18n]` block out of
   `"$DELVEWRIGHT_ENGINE/delvewright.toml"` into it and strip the `# `
   (`base_url = "https://api.deepseek.com/v1"`, `model = "deepseek-v4-pro"`).
   The key lives in the environment under the name `api_key_env` gives, never in
   a file. Look before asking —
   `env | grep -Eo '^[A-Z0-9_]*(API_KEY|APIKEY|TOKEN)[A-Z0-9_]*'` prints names
   only: a set `DELVEWRIGHT_I18N_API_KEY`, or a name carrying `DEEPSEEK` (write
   that name into `api_key_env`), is the key. With neither, ask the user once
   for a DeepSeek key and say why. Then:

   ```sh
   "$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_ENGINE/tools/creator/i18n-translate.py" "$PWD/campaigns/<id>" \
       --lang <code> --delvec "$(command -v delvec)"
   ```

   It writes the sidecar in canonical form (it runs `delvec fmt` on it),
   fact-checks every line (placeholders, numbers, one rendering per declared
   name) and validates. **Review the refused rows it names**: each is printed
   with its English, the line refused and what failed, and is missing from the
   sidecar; write those lines yourself through `delvec l10n-apply` (3 below)
   and re-validate. Then go to 4. Generation-time only — a shipped delve never
   calls a model. If the run dies `HTTP 400` before any batch, read the
   provider's sentence in the error: a retired `model` id is the usual one, and
   `curl -H "Authorization: Bearer $KEY" <base_url>/models` lists what it offers.
   Only when the user declines a key, write it yourself, 3 and 4 below, and tell
   them the Chinese is the fallback quality.
3. Yourself: `delvec --prefabs "$DELVEWRIGHT_PREFABS" l10n-inventory <campaign-dir> --lang <code>` gives the exact
   key inventory as JSON (key, English, kind, speaking NPC, situation, existing
   translation, stale). Hand your lines back with `delvec --prefabs "$DELVEWRIGHT_PREFABS" l10n-apply <campaign-dir> --lang <code> --table <file>`:
   a table of canonical English → your line, from which the tool writes
   `l10n/<code>.json` — it addresses the keys, so an inserted effect that shifts
   an `fx.` key cannot re-attach a line to the wrong English. It prints `N of M rows
   translated` and every English still missing; exit 1 until none is.
   Write each line from its `kind`, speaker and `situation` with the English as
   the fact source — `$DELVEWRIGHT_ENGINE/docs/reference/game-writing.md` §4 for
   Chinese — honour each NPC's `persona.speech_style`, and cover the inventory
   **exactly**. Then re-read every line against its English for facts, names and
   numbers, and against the translationese habits (for zh: 名词化, 弱动词,
   的的不休, over-marked 被, front-loaded modifiers), leaving lines that were
   already right byte-identical.
4. Re-`validate` until zero `DW0180`/`DW0181`. **The default build ships every
   declared language and the client picks its own**: `delvec build` emits each
   authored string as `{"translate": key, "fallback": English}` and writes
   `assets/delvewright/lang/<mc_code>.json` per language into the delve's
   resource pack. A player whose locale you do not ship — or who declines the
   resource-pack prompt — reads the English fallback. Nothing extra to run.
   `delvec --prefabs "$DELVEWRIGHT_PREFABS" build --lang <code>` produces the single-language bake for local dev;
   the release path does not use it. `critical-path.json` is language-neutral
   either way, so the ladder is unchanged.

Then re-run step 6, `delvec fmt` — it covers the sidecars too. A sidecar
`i18n-translate.py` wrote is already canonical; one you wrote by hand in 3 is not
until this runs.

**`fx.` keys are POSITION-derived** (`fx.<quest>.oc.<obj>.<index>…`). Inserting
an effect into a list SHIFTS every sibling's key and silently re-attaches old
translations to the wrong lines. When editing effect lists on a localized
campaign, APPEND rather than insert where order allows, and after any structural
edit re-check every shifted key's translation against its new English source —
exact-key coverage (`DW0180`/`DW0181`) cannot see a stale value.
