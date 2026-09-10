# Where Chunky keeps its cores — resolved the way CHUNKY resolves it.
#
# ## The defect this exists to close
#
# Chunky is a Java program, and its settings directory comes from the JVM
# property `user.home` (`SettingsDirectory.getHomeDirectory` in the pinned core:
# `new File(System.getProperty("user.home"), ".chunky")`). A shell's `$HOME` is a
# different thing, and on macOS the two disagree whenever `$HOME` has been moved:
# `user.home` is derived from the OS account record, not from the environment.
# Measured, with `$HOME` pointed at a scratch directory:
#
#     $HOME          = <scratch>/home
#     java user.home = /Users/<account>
#
# A check that read `$HOME` therefore looked in a directory the renderer never
# opens. On the first full end-to-end drill it printed `NONE installed` while the
# pinned core sat in the real directory the whole time — and, worse, the
# `MISMATCH` verdict it also names was UNREACHABLE on any machine where the two
# paths differ, because the comparison never saw the cores that were there.
#
# ## Chunky's own order, and how much of it can be seen from out here
#
# `SettingsDirectory.getSettingsDirectory` takes the first that answers:
#
#   1. the `-Dchunky.home` system property, if set and non-empty;
#   2. the WORKING directory, if it is a writable directory holding a readable
#      `chunky.json` (a portable install);
#   3. the PROGRAM directory (the jar's own), on the same test;
#   4. `user.home/.chunky`.
#
# Only (1) and (4) can be resolved from here: (2) and (3) depend on where the
# creator stands and where they put the launcher jar, neither of which this
# script is in a position to know. So `DELVEWRIGHT_CHUNKY_HOME` states (1)
# outright — it is the variable a machine that keeps Chunky elsewhere sets — and
# otherwise (4) is resolved from the JVM.
#
# ## And it never guesses in silence
#
# When there is no `java` on `PATH`, `user.home` cannot be read, and the answer
# falls back to `$HOME` — which is exactly the value that was wrong. So the
# fallback is CARRIED, in `DW_CHUNKY_HOME_SOURCE`, and every caller prints it:
# an absence reported from a directory nobody confirmed the renderer reads is a
# statement about this script, not about the machine.
#
# Usage:
#   . tools/lib/chunky-home.sh
#   dw_resolve_chunky_home        # sets DW_CHUNKY_HOME + DW_CHUNKY_HOME_SOURCE
#
# One home for this rule: anything else that needs the Chunky directory sources
# this rather than spelling `$HOME/.chunky` again.

# The JVM's `user.home`, or empty when there is no readable `java`.
dw_java_user_home() {
  command -v java >/dev/null 2>&1 || return 0
  # `-XshowSettings:properties` prints the property table to STDERR and exits.
  java -XshowSettings:properties -version 2>&1 \
    | awk -F' = ' '/^ *user\.home = /{print $2; exit}'
}

# Sets DW_CHUNKY_HOME and DW_CHUNKY_HOME_SOURCE. Never fails: an unresolvable
# home is a stated fallback, not an error, because the caller's own verdict is
# what has to carry the uncertainty.
dw_resolve_chunky_home() {
  if [ -n "${DELVEWRIGHT_CHUNKY_HOME:-}" ]; then
    DW_CHUNKY_HOME="$DELVEWRIGHT_CHUNKY_HOME"
    DW_CHUNKY_HOME_SOURCE="DELVEWRIGHT_CHUNKY_HOME"
    return 0
  fi
  local user_home
  user_home="$(dw_java_user_home)"
  if [ -n "$user_home" ]; then
    DW_CHUNKY_HOME="$user_home/.chunky"
    DW_CHUNKY_HOME_SOURCE="java user.home — the property Chunky itself reads"
    return 0
  fi
  DW_CHUNKY_HOME="${HOME:-}/.chunky"
  DW_CHUNKY_HOME_SOURCE="\$HOME — UNVERIFIED: no \`java\` on PATH, so \`user.home\` could not be read, and Chunky resolves this directory from \`user.home\` and not from \`\$HOME\`"
  return 0
}
