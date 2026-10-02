// A served pack is answered the way a vanilla client answers it, and a run can
// tell whether the pack it was sent is the pack the build made (spec-0084 §11).
import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import {
  PACK_RESULT,
  installResourcePack,
  judgeResourcePack,
  sha1Hex,
  type PackBot,
  type PackPush,
} from "../src/resource-pack.ts";

class FakeClient extends EventEmitter {
  readonly written: { name: string; params: Record<string, unknown> }[] = [];
  write(name: string, params: object): void {
    this.written.push({ name, params: params as Record<string, unknown> });
  }
}

function fakeBot(): { bot: PackBot; client: FakeClient } {
  const client = new FakeClient();
  return { bot: { _client: client as unknown as PackBot["_client"] }, client };
}

const PACK = new TextEncoder().encode("a resource pack");
const SHA1 = sha1Hex(PACK);

async function settle(): Promise<void> {
  await new Promise((r) => setImmediate(r));
  await new Promise((r) => setImmediate(r));
}

test("a push is accepted, downloaded and reported loaded, as the vanilla client does", async () => {
  const { bot, client } = fakeBot();
  const state = installResourcePack(bot, async () => PACK);
  client.emit("add_resource_pack", { uuid: "u-1", url: "http://pack:8000/resourcepack.zip", hash: SHA1 });
  await settle();
  const results = client.written.map((w) => w.params["result"]);
  assert.deepEqual(results, [PACK_RESULT.ACCEPTED, PACK_RESULT.DOWNLOADED, PACK_RESULT.SUCCESSFULLY_LOADED]);
  assert.ok(client.written.every((w) => w.name === "resource_pack_receive" && w.params["uuid"] === "u-1"));
  assert.equal(state.pushes()[0]?.downloadedSha1, SHA1);
});

test("a pack that does not download, or downloads other bytes, is answered FAILED_DOWNLOAD", async () => {
  const { bot, client } = fakeBot();
  installResourcePack(bot, async () => {
    throw new Error("connection refused");
  });
  client.emit("add_resource_pack", { uuid: "u-2", url: "http://nowhere/x.zip", hash: SHA1 });
  await settle();
  assert.equal(client.written.at(-1)?.params["result"], PACK_RESULT.FAILED_DOWNLOAD);

  const other = fakeBot();
  installResourcePack(other.bot, async () => new TextEncoder().encode("other bytes"));
  other.client.emit("add_resource_pack", { uuid: "u-3", url: "http://pack/x.zip", hash: SHA1 });
  await settle();
  assert.equal(other.client.written.at(-1)?.params["result"], PACK_RESULT.FAILED_DOWNLOAD);
});

function push(over: Partial<PackPush> = {}): PackPush {
  return {
    url: "http://pack:8000/resourcepack.zip",
    hash: SHA1,
    uuid: "u",
    downloadedSha1: SHA1,
    error: undefined,
    result: PACK_RESULT.SUCCESSFULLY_LOADED,
    ...over,
  };
}

test("the verdict holds the pushes to the build's manifest, in both directions", () => {
  assert.deepEqual(judgeResourcePack(SHA1, [push()]).failures, []);
  assert.deepEqual(judgeResourcePack(undefined, []).failures, []);
  // A pack the build ships and nobody serves is the defect this exists to catch.
  assert.equal(judgeResourcePack(SHA1, []).failures.length, 1);
  assert.equal(judgeResourcePack(undefined, [push()]).failures.length, 1);
  assert.equal(judgeResourcePack("0".repeat(40), [push()]).failures.length, 2);
  assert.equal(judgeResourcePack(SHA1, [push({ downloadedSha1: undefined, error: "x" })]).failures.length, 1);
});
