// The delve's resource pack, as a client receives it (spec-0084 §11).
//
// A delve's pack is SERVED: the server pushes `add_resource_pack` (URL, SHA-1,
// UUID) during configuration and does not let the player into the world until
// the client answers with a final status. mineflayer 4.37.1 surfaces the push
// and answers nothing on its own, so a bot that ignored it would wait in
// configuration forever. This module answers the way the vanilla client does
// — ACCEPTED, then the download, then DOWNLOADED and SUCCESSFULLY_LOADED, or
// FAILED_DOWNLOAD — and records every push, so a run can say whether the pack
// it was sent is the pack the build made.
//
// {@link judgeResourcePack} is that verdict: the build's manifest records a pack
// exactly when a push arrived, every push carries the manifest's SHA-1, and the
// bytes downloaded from the pushed URL hash to it.

import { createHash } from "node:crypto";

/** The 1.20.3+ `resource_pack_receive` results the vanilla client sends. */
export const PACK_RESULT = {
  SUCCESSFULLY_LOADED: 0,
  DECLINED: 1,
  FAILED_DOWNLOAD: 2,
  ACCEPTED: 3,
  DOWNLOADED: 4,
} as const;

/** One push, and what this client made of it. */
export interface PackPush {
  readonly url: string;
  readonly hash: string;
  readonly uuid: unknown;
  /** The SHA-1 of the bytes downloaded from `url`, or `undefined` if the download failed. */
  downloadedSha1: string | undefined;
  /** Why the download failed, when it did. */
  error: string | undefined;
  /** The final status sent. */
  result: number | undefined;
}

/** The slice of a mineflayer bot this module reads and writes. */
export interface PackBot {
  readonly _client: {
    on(event: string, listener: (packet: Record<string, unknown>) => void): unknown;
    write(name: string, params: object): void;
  };
}

/** Fetch a URL's bytes. Injected so a test needs no network. */
export type Fetcher = (url: string) => Promise<Uint8Array>;

const defaultFetcher: Fetcher = async (url) => {
  const res = await fetch(url, { signal: AbortSignal.timeout(15_000) });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return new Uint8Array(await res.arrayBuffer());
};

export interface ResourcePackState {
  /** Every push received, in order. */
  pushes(): readonly PackPush[];
}

export function sha1Hex(bytes: Uint8Array): string {
  return createHash("sha1").update(bytes).digest("hex");
}

/**
 * Answer every `add_resource_pack` push the way the vanilla client does.
 * Install it in the turn the bot is created: the push arrives in configuration,
 * before `login`.
 */
export function installResourcePack(bot: PackBot, fetcher: Fetcher = defaultFetcher): ResourcePackState {
  const pushes: PackPush[] = [];
  bot._client.on("add_resource_pack", (packet) => {
    const push: PackPush = {
      url: String(packet["url"] ?? ""),
      hash: String(packet["hash"] ?? "").toLowerCase(),
      uuid: packet["uuid"],
      downloadedSha1: undefined,
      error: undefined,
      result: undefined,
    };
    pushes.push(push);
    const answer = (result: number): void => {
      bot._client.write("resource_pack_receive", { uuid: push.uuid, result });
    };
    answer(PACK_RESULT.ACCEPTED);
    fetcher(push.url)
      .then((bytes) => {
        push.downloadedSha1 = sha1Hex(bytes);
        if (push.hash !== "" && push.downloadedSha1 !== push.hash) {
          push.error = `downloaded sha1 ${push.downloadedSha1} is not the pushed ${push.hash}`;
          push.result = PACK_RESULT.FAILED_DOWNLOAD;
          answer(PACK_RESULT.FAILED_DOWNLOAD);
          return;
        }
        answer(PACK_RESULT.DOWNLOADED);
        push.result = PACK_RESULT.SUCCESSFULLY_LOADED;
        answer(PACK_RESULT.SUCCESSFULLY_LOADED);
      })
      .catch((err: unknown) => {
        push.error = err instanceof Error ? err.message : String(err);
        push.result = PACK_RESULT.FAILED_DOWNLOAD;
        answer(PACK_RESULT.FAILED_DOWNLOAD);
      });
  });
  return { pushes: () => pushes };
}

/** The run-report section, and the verdict. */
export interface ResourcePackVerdict {
  readonly manifestSha1: string | undefined;
  readonly pushes: readonly PackPush[];
  readonly failures: readonly string[];
}

/**
 * Hold what the client received to what the build made. `manifestSha1` is the
 * manifest's `resource_pack_sha1`, `undefined` for a build with no pack.
 */
export function judgeResourcePack(
  manifestSha1: string | undefined,
  pushes: readonly PackPush[],
): ResourcePackVerdict {
  const failures: string[] = [];
  if (manifestSha1 === undefined) {
    if (pushes.length > 0) {
      failures.push(`the build ships no resource pack, and the server pushed ${pushes.length}`);
    }
  } else if (pushes.length === 0) {
    failures.push(
      `the build ships a resource pack (sha1 ${manifestSha1}) and the server pushed none — ` +
        `it is not being served`,
    );
  }
  for (const p of pushes) {
    if (manifestSha1 !== undefined && p.hash !== manifestSha1) {
      failures.push(`the server pushed ${p.url} with sha1 ${p.hash}, and the build's pack is ${manifestSha1}`);
    }
    if (p.downloadedSha1 === undefined) {
      failures.push(`the pushed pack at ${p.url} did not download: ${p.error ?? "no answer"}`);
    } else if (manifestSha1 !== undefined && p.downloadedSha1 !== manifestSha1) {
      failures.push(`${p.url} serves sha1 ${p.downloadedSha1}, and the build's pack is ${manifestSha1}`);
    }
  }
  return { manifestSha1, pushes, failures };
}
