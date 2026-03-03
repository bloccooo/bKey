import keytar from "keytar";

const SERVICE = "bkey";

// --- Storage backend credentials ---
// Stored as JSON per backend type.
// e.g. service="bkey", account="s3" → { accessKeyId, secretAccessKey }

export async function saveCredentials(
  backend: string,
  creds: Record<string, string>
): Promise<void> {
  await keytar.setPassword(SERVICE, backend, JSON.stringify(creds));
}

export async function loadCredentials(
  backend: string
): Promise<Record<string, string> | null> {
  const raw = await keytar.getPassword(SERVICE, backend);
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export async function deleteCredentials(backend: string): Promise<void> {
  await keytar.deletePassword(SERVICE, backend);
}

// --- Member identity ---
// The derived X25519 private key and member UUID are cached in the keychain
// after the first passphrase unlock, so subsequent commands are silent.

export type Identity = {
  memberId: string;
  privateKey: string; // base64
};

export async function saveIdentity(workspaceId: string, identity: Identity): Promise<void> {
  await keytar.setPassword(SERVICE, `identity-${workspaceId}`, JSON.stringify(identity));
}

export async function loadIdentity(workspaceId: string): Promise<Identity | null> {
  const raw = await keytar.getPassword(SERVICE, `identity-${workspaceId}`);
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}
