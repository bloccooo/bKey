import * as A from "@automerge/automerge";
import { randomUUIDv7 } from "bun";
import type { Workspace } from "./types";
import type { StorageBackend } from "./storage";
import { getPublicKey, unwrapDek } from "./crypto";

export type Session = {
  memberId: string;
  dek: Uint8Array;
};

const SYNC_TIMEOUT_MS = 5000;

async function tryPull(backend: StorageBackend): Promise<Uint8Array | null> {
  try {
    return await Promise.race([
      backend.pull(),
      new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error("timeout")), SYNC_TIMEOUT_MS)
      ),
    ]);
  } catch {
    return null;
  }
}

/**
 * Local-first load:
 * - Fetches remote and local cache in parallel (remote has a timeout)
 * - Merges both if available (Automerge CRDT — no data loss)
 * - Always writes result to local cache
 * - Best-effort pushes merged result back to remote
 */
export async function loadOrCreate(
  remote: StorageBackend,
  cache: StorageBackend,
): Promise<A.Doc<Workspace>> {
  const [remoteBinary, cacheBinary] = await Promise.all([
    tryPull(remote),
    cache.pull(),
  ]);

  let doc: A.Doc<Workspace>;

  if (remoteBinary && cacheBinary) {
    let localDoc = A.load<Workspace>(cacheBinary);
    const remoteDoc = A.load<Workspace>(remoteBinary);
    localDoc = A.merge(localDoc, remoteDoc);
    doc = localDoc;
    // Push merged result back so remote is up to date with any offline changes
    remote.push(A.save(doc)).catch(() => {});
  } else if (remoteBinary) {
    doc = A.load<Workspace>(remoteBinary);
  } else if (cacheBinary) {
    doc = A.load<Workspace>(cacheBinary);
  } else {
    doc = A.init<Workspace>();
    doc = A.change(doc, "init workspace", (d) => {
      d.id = randomUUIDv7();
      d.name = "my-workspace";
      d.doc_version = 0;
      d.members = {};
      d.projects = {};
      d.secrets = {};
    });
  }

  await cache.push(A.save(doc));
  return doc;
}

/**
 * Read the workspaceId from the local cache without creating or syncing anything.
 * Returns null if no cache exists yet.
 */
export async function peekWorkspaceId(cache: StorageBackend): Promise<string | null> {
  const binary = await cache.pull();
  if (!binary) return null;
  try {
    return A.load<Workspace>(binary).id;
  } catch {
    return null;
  }
}

/**
 * Unlock the workspace using a private key.
 */
export async function unlock(
  doc: A.Doc<Workspace>,
  privateKey: Uint8Array
): Promise<{ session: Session; doc: A.Doc<Workspace> }> {
  const publicKey = getPublicKey(privateKey);
  const pubKeyB64 = Buffer.from(publicKey).toString("base64");

  const member = Object.values(doc.members).find((m) => m.publicKey === pubKeyB64);
  if (!member) throw new Error("Not a member of this workspace. Run: bkey request-access");
  if (!member.wrappedDek) throw new Error("Access pending — an existing member needs to sync first.");

  const dek = unwrapDek(member.wrappedDek, privateKey);
  const session: Session = { memberId: member.id, dek };

  return { session, doc };
}

/**
 * Persist the doc — always writes to local cache, best-effort push to remote.
 */
export async function persist(
  doc: A.Doc<Workspace>,
  remote: StorageBackend,
  cache: StorageBackend,
): Promise<void> {
  const binary = A.save(doc);
  await cache.push(binary);
  remote.push(binary).catch(() => {});
}
