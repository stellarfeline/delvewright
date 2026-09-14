"""GitHub Releases and tags, read one way — one authority for every caller.

## Who reads it

- the three release workflows (`engine-release.yml`, `dsl-crate-publish.yml`,
  `plugin-release.yml`) and `.github/actions/write-release-tag`: whether the tag
  or Release they are about to write already exists, and the read-back after
  writing it. Nothing on a pull request reads it: merging and releasing are
  unrelated (ADR-0028 §4).

## ABSENT AND UNREACHABLE LOOK THE SAME, WHICH IS WHY THE BIND TEST EXISTS

`GET /repos/{repo}/releases/tags/{tag}` answers 404 for a tag with no published
Release — and a changed path, a blocked host or a revoked token can produce the
same "no". No caller may believe an absence until [`bind_test`] has seen a
Release that certainly exists resolve: `v1.5.0` of `stellarfeline/delvewright`,
a published `delvec` release with its shelf, which ADR-0028 §7 keeps forever.

A draft is not returned by the by-tag endpoint, so "not published" is split
into `draft` and `absent` by listing releases, page by page, until a short page
ends the list — a listing read to a page that came back full is never taken as
the end.

`DW_GITHUB_API` overrides the API base for a test against a local fixture; it is
printed the moment it fires, so an override can never survive silently into a
real answer. `GH_TOKEN` / `GITHUB_TOKEN` authenticate when present.

Stdlib only. CLI exit codes: 0 answered, 1 the bind test failed, 2 usage or an
unreadable answer.
"""

from __future__ import annotations

import base64
import json
import os
import sys
import urllib.error
import urllib.request
from typing import Any

REAL_API = "https://api.github.com"
TIMEOUT_SECONDS = 30
PER_PAGE = 100

BIND_REPO = "stellarfeline/delvewright"
BIND_TAG = "v1.5.0"


class NotFound(Exception):
    pass


class Unreadable(Exception):
    """The API answered with something other than a result or a 404."""


def api_base() -> str:
    base = os.environ.get("DW_GITHUB_API") or REAL_API
    if base != REAL_API:
        print(f"github_releases: DW_GITHUB_API={base} — NOT the real GitHub API", file=sys.stderr)
    return base.rstrip("/")


def fetch(path: str) -> Any:
    req = urllib.request.Request(
        f"{api_base()}/{path}",
        headers={
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "delvewright/github_releases",
        },
    )
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT_SECONDS) as fh:  # noqa: S310
            return json.load(fh)
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            raise NotFound(path) from exc
        raise Unreadable(f"{path} -> HTTP {exc.code}") from exc
    except (urllib.error.URLError, OSError, TimeoutError, json.JSONDecodeError) as exc:
        raise Unreadable(f"{path} -> {exc}") from exc


def published(repo: str, tag: str) -> dict[str, Any] | None:
    """The published Release at `tag`, or None."""
    try:
        rel = fetch(f"repos/{repo}/releases/tags/{tag}")
    except NotFound:
        return None
    if not isinstance(rel, dict):
        raise Unreadable(f"releases/tags/{tag} did not answer an object")
    if rel.get("draft"):
        return None
    return rel


def state(repo: str, tag: str) -> str:
    """`published`, `draft` or `absent`."""
    if published(repo, tag) is not None:
        return "published"
    page = 1
    while True:
        rows = fetch(f"repos/{repo}/releases?per_page={PER_PAGE}&page={page}")
        if not isinstance(rows, list):
            raise Unreadable(f"releases page {page} did not answer a list")
        for row in rows:
            if isinstance(row, dict) and row.get("tag_name") == tag:
                return "draft" if row.get("draft") else "published"
        if len(rows) < PER_PAGE:
            return "absent"
        page += 1


def asset_names(rel: dict[str, Any]) -> list[str]:
    return sorted(a.get("name", "") for a in rel.get("assets", []) if isinstance(a, dict))


def tag_commit(repo: str, tag: str) -> str | None:
    """The commit `tag` resolves to on the remote (annotated tags dereferenced), or None."""
    try:
        ref = fetch(f"repos/{repo}/git/ref/tags/{tag}")
    except NotFound:
        return None
    obj = ref.get("object", {}) if isinstance(ref, dict) else {}
    sha = obj.get("sha")
    while obj.get("type") == "tag":
        tagobj = fetch(f"repos/{repo}/git/tags/{sha}")
        obj = tagobj.get("object", {})
        sha = obj.get("sha")
    return sha if obj.get("type") == "commit" else None


def file_at(repo: str, path: str, ref: str) -> bytes | None:
    """A file's bytes at `ref` on the remote, or None when it does not exist there."""
    try:
        got = fetch(f"repos/{repo}/contents/{path}?ref={ref}")
    except NotFound:
        return None
    if not isinstance(got, dict) or got.get("encoding") != "base64":
        raise Unreadable(f"contents/{path}@{ref} did not answer a base64 file")
    return base64.b64decode(got.get("content", ""))


def download(url: str) -> bytes:
    """A public asset's bytes from its `browser_download_url`.

    Unauthenticated on purpose: the download redirects to a signed storage URL,
    and a bearer token carried across that redirect is a second credential the
    storage host refuses.
    """
    req = urllib.request.Request(url, headers={"User-Agent": "delvewright/github_releases"})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT_SECONDS) as fh:  # noqa: S310
            return fh.read()
    except (urllib.error.URLError, OSError, TimeoutError) as exc:
        raise Unreadable(f"{url} -> {exc}") from exc


def bind_test() -> tuple[bool, str]:
    """Did the lookup resolve a Release that certainly exists? `(ok, message)`."""
    try:
        rel = published(BIND_REPO, BIND_TAG)
    except Unreadable as exc:
        return False, f"the Release lookup could not be read: {exc}"
    if rel is None or not rel.get("assets"):
        return False, (
            f"the Release lookup found no published Release with assets at {BIND_REPO} "
            f"{BIND_TAG}, which certainly exists; 'this Release is absent' would be an "
            f"unbound answer for every other tag too."
        )
    return True, f"{BIND_REPO} {BIND_TAG} resolves to a published Release with {len(rel['assets'])} asset(s)"


USAGE = """usage: github_releases.py <command> [args]

  state <repo> <tag>          published | draft | absent
  assets <repo> <tag>         the published Release's asset names, one per line
  tag-commit <repo> <tag>     the commit the remote tag resolves to, or nothing
  bind-test                   refuse (exit 1) unless a known Release resolves
"""


def main(argv: list[str]) -> int:
    sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
    if not argv:
        print(USAGE, file=sys.stderr)
        return 2
    command, rest = argv[0], argv[1:]
    try:
        if command == "bind-test" and not rest:
            ok, message = bind_test()
            if ok:
                print(f"  ok   {message}")
                return 0
            print(f"github_releases: {message}", file=sys.stderr)
            return 1
        if command in ("state", "assets", "tag-commit") and len(rest) == 2:
            ok, message = bind_test()
            if not ok:
                print(f"github_releases: {message}", file=sys.stderr)
                return 1
            print(f"  bind test: {message}", file=sys.stderr)
            if command == "state":
                print(state(rest[0], rest[1]))
            elif command == "assets":
                rel = published(rest[0], rest[1])
                for name in asset_names(rel) if rel else []:
                    print(name)
            else:
                sha = tag_commit(rest[0], rest[1])
                if sha:
                    print(sha)
            return 0
    except Unreadable as exc:
        print(f"github_releases: UNREADABLE — {exc}", file=sys.stderr)
        return 2
    print(USAGE, file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
