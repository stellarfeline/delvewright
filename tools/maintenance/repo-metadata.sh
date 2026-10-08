#!/usr/bin/env bash
#
# The GitHub metadata of both public repositories — description and topics —
# held here so that an edit is a commit, and applied by this script.
#
#   tools/maintenance/repo-metadata.sh --check   # diff live values against this file; exit 1 on drift
#   tools/maintenance/repo-metadata.sh --apply   # set the live values to this file's
#
# What GitHub repository search does with these fields, measured against the
# search API (REST `search/repositories`, "best match" order, both repositories,
# each variant re-measured within a minute of being set — the index refreshes in
# about fifteen seconds):
#
# - A multi-word query is an AND over repository name, description and topics.
#   The README is not searched unless the query itself says `in:readme`
#   (`minecraft adventure map`: 84 repositories; with `in:readme`: 5601). A word
#   missing from the description removes the repository from that query
#   entirely, so coverage is decided by which words the description carries.
# - A hyphenated topic matches only as a whole token: `procedural-generation`
#   did not match `procedural`, `dungeon-generator` does not match `dungeon`.
#   A single-word topic doubles as a search word.
# - Relevance is length-normalised. With the same words present, a 30-word
#   description ranked 32nd for `minecraft adventure map`; 18 to 22 words ranked
#   8th or 9th; 15 words 8th. Name tokens outrank description words: a zero-star
#   repository named `Minecraft-Adventure-Map` stays above any description, and
#   only a rename passes it.
# - `generator` and `generate` do not stem to each other (`ai generate minecraft
#   map` matched only once the description said "generates"); `map` and `maps`
#   do. `OR` is accepted but the pool it opens is ranked by stars.
#
# So: each description stays under about 22 words, every one of them a term a
# searcher — human or agent — types; the pitch lives in the README.

set -euo pipefail

mode="${1:-}"
case "$mode" in --check|--apply) ;; *) sed -n '2,8p' "$0" >&2; exit 2 ;; esac

# One row per repository: name, description, topics (space-separated), tab-separated.
rows() { cat <<'TABLE'
stellarfeline/delvewright	AI Minecraft adventure map generator: story in, it generates a playable RPG dungeon datapack with quests. A Claude Code skill (LLM agent).	minecraft minecraft-datapack datapack minecraft-map adventure-map dungeon dungeon-generator map-generator rpg quest mcfunction minecraft-java-edition procedural-generation dsl llm ai-agents claude claude-code claude-code-skills claude-code-plugins
stellarfeline/delvewright-campaigns	Minecraft adventure maps: story-driven RPG dungeon campaigns for 1–4 players, built with Delvewright and released as server images.	minecraft minecraft-java-edition adventure-map minecraft-map minecraft-maps minecraft-datapack datapack dungeon dungeon-crawler rpg quest adventure-game minecraft-server docker multiplayer campaign game-content delvewright
TABLE
}

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
drift=0
while IFS=$'\t' read -r repo want_desc topics; do
  want_topics="$(tr ' ' '\n' <<<"$topics" | sort | tr '\n' ' ')"
  n_topics="$(wc -w <<<"$topics" | tr -d ' ')"; n_words="$(wc -w <<<"$want_desc" | tr -d ' ')"
  (( n_topics <= 20 )) || { echo "$repo: $n_topics topics, GitHub holds 20" >&2; exit 1; }
  (( n_words <= 22 )) || { echo "$repo: description is $n_words words; the limit this file states is 22" >&2; exit 1; }
  live="$(gh api "repos/$repo" --jq '{d: .description, t: (.topics|sort|join(" "))}')"
  live_desc="$(jq -r .d <<<"$live")"; live_topics="$(jq -r .t <<<"$live") "
  if [ "$mode" = --check ]; then
    if [ "$live_desc" != "$want_desc" ]; then echo "$repo description drifted:"; echo "  live: $live_desc"; echo "  file: $want_desc"; drift=1; fi
    if [ "$live_topics" != "$want_topics" ]; then echo "$repo topics drifted:"; echo "  live: $live_topics"; echo "  file: $want_topics"; drift=1; fi
    [ $drift -eq 0 ] && echo "$repo: description ($n_words words) and $n_topics topics match"
  else
    printf '%s' "$want_desc" > "$tmp/desc"
    gh api -X PATCH "repos/$repo" -F description=@"$tmp/desc" --jq .description
    jq -cn --arg t "$topics" '{names: ($t|split(" "))}' > "$tmp/topics.json"
    echo "$repo: $(gh api -X PUT "repos/$repo/topics" --input "$tmp/topics.json" --jq '.names|length') topics set"
  fi
done < <(rows)
exit $drift
