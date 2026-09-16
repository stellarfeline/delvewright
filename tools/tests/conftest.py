"""Two hard guarantees for the tools test-suite: no network, and a committer.

The i18n tool talks to a third-party LLM endpoint, so its tests must prove the
request they *would* send without ever sending one — including when a future
refactor accidentally re-binds the injected HTTP poster. Blocking `urlopen` at the
socket boundary makes that failure mode loud instead of silent (and keeps CI
offline-deterministic).
"""

import os
import urllib.request

import pytest


@pytest.fixture(autouse=True)
def no_network(monkeypatch):
    def blocked(*args, **kwargs):
        raise AssertionError(
            "a test attempted a real HTTP request — inject/monkeypatch the poster instead"
        )

    monkeypatch.setattr(urllib.request, "urlopen", blocked)


# --- the committer every scratch repository gets, without asking ------------
#
# Seven files in this suite run `git commit` in a repository they build, and
# before this they did it two ways: four set an identity in the environment of
# each call, three ran `git config user.email` per repository — and a file that
# began committing without knowing it had to do either passed on a developer's
# machine and failed on a runner with `empty ident name`. That is the same
# rule written per caller, with the failure falling on whoever writes the next
# caller.
#
# So the identity is here, once, autouse: every test in the suite runs with a
# committer, and a new test that builds a repository and commits gets it for
# free rather than discovering the rule from a red runner. The two config
# variables are pointed at nothing in the same breath and for the opposite
# reason — what the machine happens to hold must not decide what a scratch
# repository does, in either direction.
GIT_IDENTITY = {
    "GIT_AUTHOR_NAME": "delvewright-tests",
    "GIT_AUTHOR_EMAIL": "tests@example.invalid",
    "GIT_COMMITTER_NAME": "delvewright-tests",
    "GIT_COMMITTER_EMAIL": "tests@example.invalid",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_SYSTEM": os.devnull,
}


@pytest.fixture(autouse=True)
def git_identity(monkeypatch):
    for key, value in GIT_IDENTITY.items():
        monkeypatch.setenv(key, value)
