import type { StorageConfig } from "./config";
import { loadCredentials } from "./keychain";

export type InvitePayload = {
  storage: StorageConfig;
  credentials: Record<string, string>;
};

const INVITE_PREFIX = "bkey-invite:";

export async function generateInvite(storage: StorageConfig, workspaceId: string): Promise<string> {
  const credentials = await loadCredentials(storage.backend, workspaceId) ?? {};
  const payload: InvitePayload = { storage, credentials };
  const b64 = Buffer.from(JSON.stringify(payload)).toString("base64url");
  return `${INVITE_PREFIX}${b64}`;
}

export function parseInvite(link: string): InvitePayload {
  if (!link.startsWith(INVITE_PREFIX)) throw new Error("Invalid invite link");
  try {
    const b64 = link.slice(INVITE_PREFIX.length);
    return JSON.parse(Buffer.from(b64, "base64url").toString("utf-8"));
  } catch {
    throw new Error("Invalid or corrupted invite link");
  }
}

export function applyInvite(link: string): InvitePayload {
  return parseInvite(link);
}
