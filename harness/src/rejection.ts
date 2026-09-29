// "The server refused that command", for the harness.
//
// A command whose response nobody reads cannot fail. The repo's one definition of
// a refusal lives in `tools/lib/rcon.mjs`, for the Node tools that drive a live
// server over rcon; the harness drives one over the chat channel as an opped
// player, and the replies are the same strings from the same server.
//
// The harness cannot import that file: it ships as a container
// (`harness/Dockerfile`) that carries `harness/` and nothing else, so a
// cross-directory import would resolve on a developer's machine and be missing in
// every run that matters. So the pattern is restated here ONCE, for every harness
// call site, and `harness/test/rejection.test.ts` reads
// `tools/lib/rcon.mjs` and asserts the two are character-for-character the same
// source. A shape enters the rule there, on evidence, and this file follows it or
// the test reds.

/**
 * Reply shapes that mean the server did not do what was asked.
 *
 * The source string is compared against `tools/lib/rcon.mjs`'s by
 * `harness/test/rejection.test.ts`. Editing one without the other is a red test,
 * which is the whole reason the copy is allowed to exist.
 */
export const REJECTION = new RegExp(
  "(<--\\[HERE\\]" +
    "|^Unknown or incomplete command|^Incorrect argument|^Expected |^Invalid |^Unknown " +
    "|^That position is not loaded|^Cannot place blocks outside of the world" +
    "|^No blocks were filled|^Could not set the block|^No entity was found" +
    "|^No targets matched|^Malformed |^Failed to " +
    "|^Unable to modify player data)",
);

/** True when `reply` is the server saying it refused or could not parse a command. */
export function isRejection(reply: string): boolean {
  return REJECTION.test(String(reply).trim());
}
