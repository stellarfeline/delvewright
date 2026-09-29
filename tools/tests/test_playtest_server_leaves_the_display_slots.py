"""The playtest server never touches a scoreboard display slot.

A display slot belongs to the campaign: spec-0076 lets a campaign put a named
currency on the sidebar, and the build's own `setup` puts it there. A serving
tool that clears or sets a slot after boot overrides the campaign's declaration
on the one server the owner plays on, and nothing downstream can see it: vanilla
has no command that reads which objective a slot shows, so no ladder, PackTest
or probe can catch a slot cleared behind it. The only place to hold the rule is
the tool itself.
"""

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "creator" / "playtest-server.sh"


def test_the_playtest_server_issues_no_setdisplay():
    text = SCRIPT.read_text(encoding="utf-8")
    code = [ln for ln in text.splitlines() if not ln.lstrip().startswith("#")]
    hits = [ln for ln in code if re.search(r"\bsetdisplay\b", ln)]
    assert hits == [], f"playtest-server.sh sets or clears a display slot: {hits}"


def test_the_rule_is_bound_to_the_script_it_names():
    # A rename would make the first test pass on a file that no longer exists.
    assert SCRIPT.is_file(), SCRIPT
    assert "rcon" in SCRIPT.read_text(encoding="utf-8")
