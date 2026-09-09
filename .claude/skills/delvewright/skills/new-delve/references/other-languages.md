# Reference: other languages

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
3. Yourself: `delvec --prefabs "$DELVEWRIGHT_PREFABS" l10n-inventory <campaign-dir> --lang <code>` gives the exact
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
   `delvec --prefabs "$DELVEWRIGHT_PREFABS" build --lang <code>` produces the single-language bake for local dev;
   the release path does not use it. `critical-path.json` is language-neutral
   either way, so the ladder is unchanged.

Then re-run step 6, `delvec fmt` — it covers the sidecars too.

**`fx.` keys are POSITION-derived** (`fx.<quest>.oc.<obj>.<index>…`). Inserting
an effect into a list SHIFTS every sibling's key and silently re-attaches old
translations to the wrong lines. When editing effect lists on a localized
campaign, APPEND rather than insert where order allows, and after any structural
edit re-check every shifted key's translation against its new English source —
exact-key coverage (`DW0180`/`DW0181`) cannot see a stale value.
