import * as A from "@automerge/automerge";
import { randomUUIDv7 } from "bun";
import type { Workspace } from "./types";
import type { StorageBackend } from "./storage";
import { getPublicKey, unwrapDek, wrapDek } from "./crypto";

export type Session = {
  memberId: string;
  dek: Uint8Array;
};

export async function loadOrCreate(
  backend: StorageBackend
): Promise<A.Doc<Workspace>> {
  const binary = await backend.pull();

  if (binary) {
    return A.load<Workspace>(binary);
  }

  let doc = A.init<Workspace>();
  doc = A.change(doc, "init workspace", (d) => {
    d.id = randomUUIDv7();
    d.name = "my-workspace";
    d.doc_version = 0;
    d.members = {};
    d.projects = {};
    d.secrets = {};
  });
  return doc;
}

/**
 * Unlock the workspace using a private key.
 * - Finds the member entry matching the public key
 * - Decrypts the DEK
 * - Grants access to any pending members (wrappedDek === "")
 * Returns the (possibly updated) doc and the session.
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

  // Grant access to any pending members
  const pending = Object.values(doc.members).filter((m) => !m.wrappedDek);
  if (pending.length > 0) {
    doc = A.change(doc, "grant pending access", (d) => {
      for (const m of pending) {
        const entry = d.members[m.id];
        if (!entry) continue;
        const pubKey = Buffer.from(m.publicKey, "base64");
        entry.wrappedDek = wrapDek(dek, pubKey);
      }
    });
  }

  return { session, doc };
}

export async function persist(
  doc: A.Doc<Workspace>,
  backend: StorageBackend
): Promise<void> {
  const binary = A.save(doc);
  await backend.push(binary);
}
