import * as p from "@clack/prompts";
import { type StorageBackend, cacheBackend } from "../storage";
import { loadIdentity, saveIdentity } from "../keychain";
import { loadOrCreate, unlock, type Session } from "../store";
import { derivePrivateKey, getPublicKey } from "../crypto";
import type * as A from "@automerge/automerge";
import type { BKeyDocument } from "../types";

/**
 * Load the workspace doc and derive the session (DEK + memberId).
 */
export async function unlockWorkspace(
  backend: StorageBackend,
): Promise<{ doc: A.Doc<BKeyDocument>; session: Session }> {
  const doc = await loadOrCreate(backend, cacheBackend());
  const workspaceId = doc.id;

  // Try cached identity first
  let identity = await loadIdentity(workspaceId);

  if (!identity) {
    const passphrase = await p.password({ message: "Enter passphrase" });
    if (p.isCancel(passphrase) || !passphrase) {
      p.cancel("Cancelled.");
      process.exit(0);
    }
    const privateKey = derivePrivateKey(passphrase, workspaceId);
    const pubKeyB64 = Buffer.from(getPublicKey(privateKey)).toString("base64");
    const member = Object.values(doc.members ?? {}).find(
      (m) => m.publicKey === pubKeyB64,
    );
    if (!member) {
      throw new Error("Not a member of this workspace. Run: bkey init");
    }
    identity = {
      memberId: member.id,
      privateKey: Buffer.from(privateKey).toString("base64"),
    };
    await saveIdentity(workspaceId, identity);
  }

  const privateKey = Buffer.from(identity.privateKey, "base64");
  const { session, doc: updatedDoc } = await unlock(doc, privateKey);

  return { doc: updatedDoc, session };
}
