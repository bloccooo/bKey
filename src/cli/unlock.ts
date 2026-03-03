import * as p from "@clack/prompts";
import type { BKeyConfig } from "../config";
import type { StorageBackend } from "../storage";
import { loadIdentity, saveIdentity } from "../keychain";
import { loadOrCreate, unlock, persist, type Session } from "../store";
import { derivePrivateKey } from "../crypto";
import type * as A from "@automerge/automerge";
import type { Workspace } from "../types";

/**
 * Load the workspace doc, derive the session (DEK + memberId), and
 * persist any access grants for pending members.
 */
export async function unlockWorkspace(
  config: BKeyConfig | null,
  backend: StorageBackend
): Promise<{ doc: A.Doc<Workspace>; session: Session }> {
  const workspaceId = config?.workspaceId;
  if (!workspaceId) {
    console.error("error: workspace not initialised. Run: bkey init");
    process.exit(1);
  }

  const doc = await loadOrCreate(backend);

  // Try cached identity first
  let identity = await loadIdentity(workspaceId);

  if (!identity) {
    // Prompt for passphrase, derive + cache
    const passphrase = await p.password({ message: "Enter passphrase" });
    if (p.isCancel(passphrase) || !passphrase) {
      p.cancel("Cancelled.");
      process.exit(0);
    }
    const privateKey = derivePrivateKey(passphrase, workspaceId);
    // Find member id by public key
    const { getPublicKey } = await import("../crypto");
    const pubKeyB64 = Buffer.from(getPublicKey(privateKey)).toString("base64");
    const member = Object.values(doc.members ?? {}).find((m) => m.publicKey === pubKeyB64);
    if (!member) {
      console.error("error: not a member of this workspace. Run: bkey request-access");
      process.exit(1);
    }
    identity = { memberId: member.id, privateKey: Buffer.from(privateKey).toString("base64") };
    await saveIdentity(workspaceId, identity);
  }

  const privateKey = Buffer.from(identity.privateKey, "base64");
  const { session, doc: updatedDoc } = await unlock(doc, privateKey);

  // Persist if access was granted to pending members
  if (updatedDoc !== doc) {
    await persist(updatedDoc, backend);
  }

  return { doc: updatedDoc, session };
}
