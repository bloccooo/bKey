import * as p from "@clack/prompts";
import { randomUUIDv7 } from "bun";
import { readConfig } from "../config";
import { backendFromConfig, localBackend } from "../storage";
import { loadOrCreate, persist } from "../store";
import { saveIdentity } from "../keychain";
import { derivePrivateKey, getPublicKey } from "../crypto";
import * as A from "@automerge/automerge";

export async function cmdRequestAccess() {
  const config = await readConfig();
  if (!config?.workspaceId) {
    console.error("error: no workspace found. Run bkey init first.");
    process.exit(1);
  }

  p.intro("bkey request-access");

  const passphrase = await p.password({ message: "Enter your passphrase" });
  if (p.isCancel(passphrase) || !passphrase) { p.cancel("Cancelled."); return; }

  const backend = config ? await backendFromConfig(config.storage) : localBackend(".");
  const doc = await loadOrCreate(backend);

  const privateKey = derivePrivateKey(passphrase, config.workspaceId);
  const publicKey = getPublicKey(privateKey);
  const pubKeyB64 = Buffer.from(publicKey).toString("base64");

  // Check if already a member
  const existing = Object.values(doc.members ?? {}).find((m) => m.publicKey === pubKeyB64);
  if (existing) {
    p.outro(existing.wrappedDek
      ? "You already have access to this workspace."
      : "Access request already pending. Wait for an existing member to sync."
    );
    return;
  }

  const memberId = randomUUIDv7();
  const updated = A.change(doc, "request access", (d) => {
    if (!d.members) d.members = {};
    d.members[memberId] = {
      id: memberId,
      publicKey: pubKeyB64,
      wrappedDek: "",  // pending — existing members grant on next sync
    };
  });

  await persist(updated, backend);
  await saveIdentity(config.workspaceId, {
    memberId,
    privateKey: Buffer.from(privateKey).toString("base64"),
  });

  p.outro("Access requested. An existing member needs to open bkey (ui or run) to grant you access.");
}
