// A block state as the compiler writes one and vanilla prints one:
// `[namespace:]name[key=value,...]`. The harness's one parse rule for it —
// every reader that compares a block it reads with a block the compiler stated
// goes through here, never a private `split("[")`.
//
// The committed fixture `test/fixtures/block-states.json` is read by this
// module's test and by `tools/tests/test_block_state_parse.py` against
// `tools/ci/check-written-world.py`'s `split`, so the two readers of the format
// answer the same on every row.

/** A parsed block state: the namespaced name and its stated properties. */
export interface BlockState {
  /** `minecraft:white_candle` — a bare name is given the `minecraft:` namespace. */
  readonly name: string;
  /** Every property the text states, as text (`candles` → `"3"`). */
  readonly properties: Readonly<Record<string, string>>;
}

/** Parse `text`. A name without a namespace is vanilla's (`minecraft:`). */
export function parseBlockState(text: string): BlockState {
  const open = text.indexOf("[");
  const rawName = (open < 0 ? text : text.slice(0, open)).trim();
  const name = rawName.includes(":") ? rawName : `minecraft:${rawName}`;
  const properties: Record<string, string> = {};
  if (open >= 0) {
    const inner = text.slice(open + 1, text.endsWith("]") ? -1 : undefined);
    for (const pair of inner.split(",")) {
      const eq = pair.indexOf("=");
      if (eq < 0) continue;
      properties[pair.slice(0, eq).trim()] = pair.slice(eq + 1).trim();
    }
  }
  return { name, properties };
}

/** The canonical text of a state: properties sorted by key, as a save prints them. */
export function blockStateText(state: BlockState): string {
  const keys = Object.keys(state.properties).sort();
  if (keys.length === 0) return state.name;
  return `${state.name}[${keys.map((k) => `${k}=${state.properties[k]}`).join(",")}]`;
}

/**
 * The state a block reading names, from its name and its properties as
 * mineflayer reports them (`getProperties()`: numbers and booleans for int and
 * bool properties), in canonical text.
 */
export function readBlockState(name: string, properties: Readonly<Record<string, unknown>>): string {
  const props: Record<string, string> = {};
  for (const [k, v] of Object.entries(properties)) props[k] = String(v);
  return blockStateText({ name: parseBlockState(name).name, properties: props });
}

/**
 * Whether the block `read` is the block `stated`: the same name, and every
 * property `stated` names holding the same value in `read`. A property the
 * statement leaves out is not judged — a write that omits it takes the block's
 * default, which the statement did not ask about.
 */
export function blockStateSatisfies(stated: string, read: string): boolean {
  const want = parseBlockState(stated);
  const got = parseBlockState(read);
  if (want.name !== got.name) return false;
  return Object.entries(want.properties).every(([k, v]) => got.properties[k] === v);
}
