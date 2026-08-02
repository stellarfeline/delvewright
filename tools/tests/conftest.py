"""Hard guarantee for the tools test-suite: no test may reach the network.

The i18n tool talks to a third-party LLM endpoint, so its tests must prove the
request they *would* send without ever sending one — including when a future
refactor accidentally re-binds the injected HTTP poster. Blocking `urlopen` at the
socket boundary makes that failure mode loud instead of silent (and keeps CI
offline-deterministic).
"""

import urllib.request

import pytest


@pytest.fixture(autouse=True)
def no_network(monkeypatch):
    def blocked(*args, **kwargs):
        raise AssertionError(
            "a test attempted a real HTTP request — inject/monkeypatch the poster instead"
        )

    monkeypatch.setattr(urllib.request, "urlopen", blocked)
