import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CAMPAIGN_TOKEN,
  markerLine,
  parseCensusMob,
  parseCensusSummary,
  parseCompletionMarker,
} from "../src/markers.ts";

// The wire format is the whole contract between the compiler and the bot's success
// criteria, so these tests pin the exact bytes and — more importantly — everything
// that must NOT parse. The failure this closes was a substring match: any chat line
// containing the old token satisfied it.

test("parses a per-objective marker", () => {
  assert.deepEqual(parseCompletionMarker("[dw:complete nobodys-cave obj/reach-camp]"), {
    campaignId: "nobodys-cave",
    token: "obj/reach-camp",
  });
});

test("parses the campaign-completion marker", () => {
  assert.deepEqual(parseCompletionMarker("[dw:complete hello-world campaign]"), {
    campaignId: "hello-world",
    token: CAMPAIGN_TOKEN,
  });
});

test("markerLine renders exactly what parseCompletionMarker accepts", () => {
  for (const token of ["campaign", "obj/greet", "obj/light-the-brazier"]) {
    const line = markerLine("nobodys-cave-island", token);
    assert.deepEqual(parseCompletionMarker(line), {
      campaignId: "nobodys-cave-island",
      token,
    });
  }
});

test("rejects a marker embedded in a longer chat line (no substring matching)", () => {
  // The whole class of forgery the old unanchored matcher allowed: any line that
  // merely CONTAINED the token counted as completion.
  for (const line of [
    "<player> [dw:complete hello-world campaign]",
    "The keeper says: [dw:complete hello-world obj/greet] — or so the rumour goes",
    "prefix [dw:complete hello-world campaign]",
    "[dw:complete hello-world campaign] suffix",
    "[dw:complete hello-world campaign] [dw:complete hello-world obj/greet]",
  ]) {
    assert.equal(parseCompletionMarker(line), undefined, line);
  }
});

test("rejects authored lookalikes and near-miss spellings", () => {
  for (const line of [
    "[Delvewright] complete dw.campaign 1", // the retired unanchored token
    "[dw:complete hello-world]", // no token
    "[dw:complete campaign]", // no campaign id
    "[dw:complete hello world campaign]", // campaign id is one kebab token
    "[dw:complete hello-world obj/Greet]", // ids are lowercase kebab
    "[dw:complete hello-world obj/]", // empty local part
    "[dw:complete hello-world greet]", // objective ids carry their `obj/` prefix
    "[dw:complete hello-world quest/greet]", // wrong id namespace
    "[dw:complete  hello-world campaign]", // double space
    "[dw:complete hello-world campaign ]", // trailing space inside
    " [dw:complete hello-world campaign]", // leading whitespace
    "[dw:complete hello-world campaign] ", // trailing whitespace
    "[dw:completed hello-world campaign]",
    "dw:complete hello-world campaign",
    "",
  ]) {
    assert.equal(parseCompletionMarker(line), undefined, JSON.stringify(line));
  }
});

test("a campaign id is matched exactly — another campaign's marker is a different marker", () => {
  const other = parseCompletionMarker("[dw:complete other-delve obj/greet]");
  assert.deepEqual(other, { campaignId: "other-delve", token: "obj/greet" });
  // The executor compares campaignId against the running path's; this test pins that
  // the parse keeps them distinguishable rather than normalizing them together.
  assert.notEqual(other?.campaignId, "hello-world");
});

// --- the wave census channel (task #123) -------------------------------------

test("a census summary parses into the server's own counts", () => {
  assert.deepEqual(parseCensusSummary("[dw:census the-drowned-bell wave/gate-assault 7 2 1 1]"), {
    campaignId: "the-drowned-bell",
    wave: "wave/gate-assault",
    seq: 7,
    present: 2,
    branded: 1,
    damaged: 1,
  });
});

test("a census mob line carries position and health as real units", () => {
  // The wire is fixed-point ×100 so nothing has to parse a float; a negative
  // coordinate must survive it, which is most of the bell's map.
  assert.deepEqual(
    parseCensusMob("[dw:censusmob the-drowned-bell wave/gate-assault 7 1250 7100 -8450 375 600]"),
    {
      campaignId: "the-drowned-bell",
      wave: "wave/gate-assault",
      seq: 7,
      pos: [12.5, 71, -84.5],
      health: 3.75,
      maxHealth: 6,
    },
  );
});

test("the census channel is as anchored as the completion channel", () => {
  for (const line of [
    "[dw:census the-drowned-bell wave/gate-assault 7 2 1]", // one field short
    "[dw:census the-drowned-bell obj/hold-the-gate 7 2 1 1]", // not a wave id
    "[dw:census the-drowned-bell wave/gate-assault 7 2 1 1] ", // trailing space
    " [dw:census the-drowned-bell wave/gate-assault 7 2 1 1]", // leading space
    "look out: [dw:census the-drowned-bell wave/gate-assault 7 2 1 1]", // substring
    "[dw:census the-drowned-bell wave/gate-assault 7 2 1 x]", // non-numeric
    "",
  ]) {
    assert.equal(parseCensusSummary(line), undefined, JSON.stringify(line));
  }
  assert.equal(parseCensusMob("[dw:censusmob c wave/x 1 0 0 0 1]"), undefined, "one field short");
  // The two lines never parse as each other.
  assert.equal(parseCensusSummary("[dw:censusmob c wave/x 1 0 0 0 1 1]"), undefined);
  assert.equal(parseCensusMob("[dw:census c wave/x 1 2 0 0]"), undefined);
});
