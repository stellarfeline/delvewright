#!/usr/bin/env bash
# The one rule for `package-verify`, the `CARGO_TARGET_DIR` two scripts point
# `cargo package` at: `tools/check-publishable.sh` (the whole shelf, every
# push) and `tools/crates-io-publish.sh --only` (the DSL crate's own two
# automatic sites, which package just that one crate themselves — see the
# "WHICH CRATES ONE RUN DECIDES ABOUT" section of that script).
#
# WHY IT LIVES AT `$ROOT/package-verify`, NOT `$ROOT/target/package-verify`
#
# `Swatinem/rust-cache` walks `./target` (the default `workspaces: .`) on every
# restore and save for the calling job. Run 34066557967, job "does the dsl
# crate's number move" (`crates-io-publish.sh --plan --only delvewright-dsl`,
# the ONLY step in that job besides checkout and the cache action itself, and
# one with no cleanup of its own at all at the time) carried four failure
# annotations from that action's OWN post step: `ENOENT: no such file or
# directory, opendir '.../target/package-verify/package/delvewright-dsl-0.19.0
# /tests/trybuild'` and `…/tests/target` — `cargo package` re-extracting into a
# destination the cache had just restored a stale copy of leaves exactly the
# shape a directory-walking cache action trips on. A tree nested inside the one
# directory an external action owns end to end is the wrong place for scratch
# state THIS repo's own scripts own end to end; moving it beside `target/`
# rather than under it means the action never walks it, no matter what `cargo
# package` does to it mid-job — the question stops being "what exactly does the
# cache action's walk do" and becomes "is it looking here at all", which is
# checkable by reading `Swatinem/rust-cache`'s own default (`workspaces: .`
# caches `target/`, nothing outside it).
#
# WHAT A PACKAGING RUN MAY LEAVE BEHIND ON SUCCESS
#
# Only `package/*.crate` and `package/*.crate.sha256` — the shape
# `tools/crates-io-publish.sh`'s `local_crate_path` / `local_cksum` read as
# proof `check-publishable.sh` verified these exact bytes. Everything else —
# the extracted `<name>-<version>/` source trees (each carrying its own nested
# `tests/` fixtures, the ENOENT class above), the temporary local registry
# `cargo package` builds a multi-crate run's dependents against
# (`package/tmp-registry/`), and the verify build's own `debug/`/`release/`
# output alongside `package/` — is scratch a packaging run owns end to end, not
# something worth keeping past the run that made it.
#
# Usage: dw_prune_package_verify <verify-target-dir>   # e.g. $ROOT/package-verify
dw_prune_package_verify() {
  local verify_target="$1"
  if [ -d "$verify_target/package" ]; then
    find "$verify_target/package" -mindepth 1 -maxdepth 1 \
      ! -name '*.crate' ! -name '*.crate.sha256' -exec rm -rf {} +
  fi
  find "$verify_target" -mindepth 1 -maxdepth 1 ! -name package -exec rm -rf {} +
}
