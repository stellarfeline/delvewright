#!/usr/bin/env bash
# The live half of a playtest checkpoint, sourced by
# `tools/creator/playtest-server.sh` after `tools/lib/rcon.sh`. The file half
# (the quest table, the copy and its proof, the records) is
# `tools/lib/playtest_checkpoint.py`.
#
# A checkpoint is the served world, copied while the server is not saving:
# `save-off`, `save-all flush`, copy, `save-on`. Quest completion is read from
# the datapack's own state, `#party dw.q_<quest>`, through the shared rcon rule.
# Kept outside the playtest image's inputs key: nothing here reaches the image.

DW_CKPT_PY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/playtest_checkpoint.py"
# Seconds between two readings of the quest scores while a campaign is served.
DW_CKPT_POLL_S="${DW_CKPT_POLL_S:-5}"
# Seconds a save waits for another save on the same image to finish.
DW_CKPT_LOCK_WAIT_S=120

# dw_ckpt_root — where checkpoints live: on the creator's own disk, beside the
# runtime cache, never in an image and never in a session's directories (a
# checkpoint outlives `down`).
dw_ckpt_root() {
  printf '%s\n' "${DW_PLAYTEST_CHECKPOINTS:-${XDG_CACHE_HOME:-$HOME/.cache}/delvewright/playtest-checkpoints}"
}

# dw_ckpt_image_root <root> <image id> — one directory per kept image, by id.
dw_ckpt_image_root() {
  printf '%s/%s\n' "$1" "${2#sha256:}"
}

# dw_ckpt_quest_reading <container> <score> — `done`, `open` or `unanswered`.
# The WHOLE reply is judged, never a prefix, for the reason
# `dw_placed_reading` gives: rcon clients share one output buffer.
dw_ckpt_quest_reading() {
  local reply
  if ! reply="$(dw_rcon_probe "$1" "execute if score #party $2 matches 1" 2>/dev/null)"; then
    echo unanswered; return 0
  fi
  reply="${reply%"${reply##*[![:space:]]}"}"
  case "$reply" in
    "Test passed") echo done ;;
    "Test failed") echo open ;;
    *) echo unanswered ;;
  esac
}

# dw_ckpt_completed <container> <quests.tsv> — the completed quest scores,
# space-separated, in table order. Returns 1 when any reading went unanswered:
# a partial reading is not a reading.
dw_ckpt_completed() {
  local score name reading out=""
  while IFS=$'\t' read -r score name; do
    [ -n "$score" ] || continue
    reading="$(dw_ckpt_quest_reading "$1" "$score")"
    case "$reading" in
      done) out="$out $score" ;;
      open) ;;
      *) return 1 ;;
    esac
  done <"$2"
  printf '%s\n' "${out# }"
}

# dw_ckpt_names <quests.tsv> <scores…> — the table's names for these scores,
# joined with " + ".
dw_ckpt_names() {
  local table="$1" score name out="" want
  shift
  while IFS=$'\t' read -r score name; do
    for want in "$@"; do
      if [ "$want" = "$score" ]; then out="${out:+$out + }$name"; fi
    done
  done <"$table"
  printf '%s\n' "$out"
}

# dw_ckpt_lock <image root> / dw_ckpt_unlock <image root> — one save at a time
# per image (the watcher and a manual `checkpoint` may meet). A lock whose
# holder is gone is taken over, and said.
dw_ckpt_lock() {
  local lock="$1/.lock" waited=0 holder
  mkdir -p "$1" || return 1
  while ! mkdir "$lock" 2>/dev/null; do
    holder="$(cat "$lock/pid" 2>/dev/null || true)"
    if [ -n "$holder" ] && ! kill -0 "$holder" 2>/dev/null; then
      echo "checkpoint: the save lock's holder (pid $holder) is gone — taking it over" >&2
      rm -rf "$lock"; continue
    fi
    if [ "$waited" -ge "$DW_CKPT_LOCK_WAIT_S" ]; then
      echo "checkpoint: another save on this image (pid ${holder:-?}) held the lock for ${waited}s" >&2
      return 1
    fi
    sleep 1; waited=$((waited + 1))
  done
  echo "$$" >"$lock/pid"
}
dw_ckpt_unlock() { rm -rf "$1/.lock"; }

# dw_ckpt_save <container> <world dir> <image root> <image id> <name> <kind> <quests.tsv>
#
# Prints the new checkpoint's label (`NN name`). The quest scores are read
# BEFORE the flush and again AFTER the copy: they only ever rise, so two equal
# readings mean none moved across the moment the files were written. The copy
# is then held to the world it came from, file by file. `save-on` is owed on
# every path once `save-off` has answered.
dw_ckpt_save() {
  local container="$1" world="$2" iroot="$3" image_id="$4" name="$5" kind="$6" quests="$7"
  local seq tmp before after copied reply digest files bytes why=""
  dw_ckpt_lock "$iroot" || return 1
  seq="$(python3 "$DW_CKPT_PY" next-seq --image-root "$iroot")" || { dw_ckpt_unlock "$iroot"; return 1; }
  tmp="$iroot/.incoming-$seq"
  rm -rf "$tmp"
  mkdir -p "$tmp"
  if ! reply="$(dw_rcon "$container" "save-off")"; then
    rm -rf "$tmp"; dw_ckpt_unlock "$iroot"
    echo "checkpoint: the server refused \`save-off\`: $reply" >&2
    return 1
  fi
  if ! before="$(dw_ckpt_completed "$container" "$quests")"; then
    why="a quest score went unanswered before the flush"
  elif ! reply="$(dw_rcon "$container" "save-all flush")" || [[ $reply != *"Saved the game"* ]]; then
    why="\`save-all flush\` did not answer \"Saved the game\": $reply"
  elif ! copied="$(python3 "$DW_CKPT_PY" copy --src "$world" --dst "$tmp/world")"; then
    why="the copy of $world is not the world it was taken from (above)"
  elif ! after="$(dw_ckpt_completed "$container" "$quests")"; then
    why="a quest score went unanswered after the copy"
  elif [ "$before" != "$after" ]; then
    why="a quest completed while the copy was taken (before: ${before:-none}; after: ${after:-none})"
  fi
  if ! reply="$(dw_rcon "$container" "save-on")"; then
    echo "checkpoint: the server refused \`save-on\` — IT IS NOT SAVING. Run over rcon: save-on ($reply)" >&2
    why="${why:-the server refused save-on}"
  fi
  if [ -n "$why" ]; then
    rm -rf "$tmp"; dw_ckpt_unlock "$iroot"
    echo "checkpoint: NOT saved — $why" >&2
    return 1
  fi
  IFS=$'\t' read -r digest files bytes <<<"$copied"
  python3 "$DW_CKPT_PY" write-meta --dir "$tmp" --seq "$seq" --name "$name" --kind "$kind" \
    --image-id "$image_id" --digest "$digest" --files "$files" --bytes "$bytes" --completed "$after" \
    || { rm -rf "$tmp"; dw_ckpt_unlock "$iroot"; return 1; }
  mv "$tmp" "$iroot/$seq" || { rm -rf "$tmp"; dw_ckpt_unlock "$iroot"; return 1; }
  dw_ckpt_unlock "$iroot"
  echo "checkpoint: saved $seq $name — $files file(s), world digest $digest, $(printf '%s' "$after" | wc -w | tr -d ' ') quest(s) complete" >&2
  printf '%s %s\n' "$seq" "$name"
}

# dw_ckpt_completed_of <checkpoint dir> — the quest scores it recorded complete.
dw_ckpt_completed_of() {
  python3 -c 'import json, sys; sys.stdout.reconfigure(newline="\n"); print(" ".join(json.load(open(sys.argv[1]))["completed"]))' "$1/checkpoint.json"
}

# dw_ckpt_verify_restored <container> <checkpoint dir> <quests.tsv> <budget_s> <poll_s>
#
# The served world is the checkpoint's, not the image's: every quest score
# reads back as the checkpoint recorded it. A world the image had copied in
# instead (a fresh delve) reads every quest open and is refused here. On
# failure sets DW_VERIFY_FAILURE and returns 1.
dw_ckpt_verify_restored() {
  local container="$1" ck="$2" quests="$3" budget="$4" poll="$5" want got total
  DW_VERIFY_FAILURE=""
  dw_playtest_wait_placed "$container" "$budget" "$poll" || return 1
  want="$(dw_ckpt_completed_of "$ck")" || { DW_VERIFY_FAILURE="cannot read $ck/checkpoint.json"; return 1; }
  # Table order on both sides: the record is sorted, the reading in table order.
  want="$(printf '%s\n' $want | sort | tr '\n' ' ')"
  if ! got="$(dw_ckpt_completed "$container" "$quests")"; then
    DW_VERIFY_FAILURE="a quest score went unanswered on the restored server"; return 1
  fi
  got="$(printf '%s\n' $got | sort | tr '\n' ' ')"
  total="$(awk -F'\t' 'NF {n++} END {print n+0}' "$quests")"
  if [ "$want" != "$got" ]; then
    DW_VERIFY_FAILURE="the restored server reads complete: ${got:-none}; the checkpoint recorded: ${want:-none}"
    return 1
  fi
  echo "restored: all $total quest score(s) read back as the checkpoint recorded them ($(printf '%s' "$want" | wc -w | tr -d ' ') complete)"
}

# dw_ckpt_watch <container> <world dir> <image root> <image id> <quests.tsv> <alive fn>
#
# The automatic checkpoints: read every quest score each DW_CKPT_POLL_S; the
# first full reading is the baseline, and each later one that finds a quest
# newly complete saves a checkpoint named by the table. Runs until <alive fn>
# fails (the server or the session is gone).
dw_ckpt_watch() {
  local container="$1" world="$2" iroot="$3" image_id="$4" quests="$5" alive="$6"
  local completed="" baseline=0 now score flips misses=0 name total
  total="$(awk -F'\t' 'NF {n++} END {print n+0}' "$quests")"
  echo "watch: $total quest score(s), read every ${DW_CKPT_POLL_S}s on $container"
  while :; do
    if ! "$alive"; then echo "watch: the session ended — stopping"; return 0; fi
    if now="$(dw_ckpt_completed "$container" "$quests")"; then
      if [ "$baseline" = 0 ]; then
        completed="$now"; baseline=1
        echo "watch: baseline: $(printf '%s' "$now" | wc -w | tr -d ' ') of $total complete"
      else
        flips=()
        for score in $now; do
          case " $completed " in *" $score "*) ;; *) flips+=("$score") ;; esac
        done
        if [ "${#flips[@]}" -gt 0 ]; then
          name="$(dw_ckpt_names "$quests" "${flips[@]}")"
          if dw_ckpt_save "$container" "$world" "$iroot" "$image_id" "$name" auto "$quests" >/dev/null; then
            completed="$now"; misses=0
          else
            misses=$((misses + 1))
            if [ "$misses" -ge 3 ]; then
              echo "watch: MISSED the checkpoint for '$name' — three saves in a row failed (above)"
              completed="$now"; misses=0
            fi
          fi
        fi
      fi
    fi
    # Backgrounded and waited on, so a TERM ends the wait at once rather than
    # after the sleep (the caller's trap kills DW_CKPT_SLEEP_PID).
    sleep "$DW_CKPT_POLL_S" &
    DW_CKPT_SLEEP_PID=$!
    wait "$DW_CKPT_SLEEP_PID" 2>/dev/null || true
    DW_CKPT_SLEEP_PID=""
  done
}
