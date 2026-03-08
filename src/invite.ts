import type { StorageConfig } from "./storage";

export type WorkspacePayload = {
  id: string;
  name: string;
};

export type InvitePayload = {
  workspace: WorkspacePayload;
  storage: StorageConfig;
};

const INVITE_PREFIX = "bkey-invite:";

export async function generateInvite(
  storage: StorageConfig,
  workspace: WorkspacePayload,
): Promise<string> {
  const payload: InvitePayload = { storage, workspace };
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
