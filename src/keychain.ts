import keytar from "keytar";

const SERVICE = "bkey";
const VAULT_ACCOUNT = "vault";
// All secrets for a workspace are stored in a single keychain entry so macOS
// only prompts once per workspace. Account name: "${VAULT_ACCOUNT}-{workspaceId}".

type Vault = Record<string, string>;

const vaultCaches = new Map<string, Vault>();

async function loadVault(workspaceId: string): Promise<Vault> {
  const cached = vaultCaches.get(workspaceId);
  if (cached !== undefined) return cached;
  const raw = await keytar.getPassword(SERVICE, `${VAULT_ACCOUNT}-${workspaceId}`);
  let vault: Vault = {};
  if (raw) {
    try { vault = JSON.parse(raw); } catch {}
  }
  vaultCaches.set(workspaceId, vault);
  return vault;
}

async function saveVault(workspaceId: string, vault: Vault): Promise<void> {
  vaultCaches.set(workspaceId, vault);
  await keytar.setPassword(SERVICE, `${VAULT_ACCOUNT}-${workspaceId}`, JSON.stringify(vault));
}

// --- Storage backend credentials ---

export async function saveCredentials(
  backend: string,
  creds: Record<string, string>,
  workspaceId: string,
): Promise<void> {
  const vault = await loadVault(workspaceId);
  vault[backend] = JSON.stringify(creds);
  await saveVault(workspaceId, vault);
}

export async function loadCredentials(
  backend: string,
  workspaceId: string,
): Promise<Record<string, string> | null> {
  const vault = await loadVault(workspaceId);
  const raw = vault[backend];
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export async function deleteCredentials(backend: string, workspaceId: string): Promise<void> {
  const vault = await loadVault(workspaceId);
  delete vault[backend];
  await saveVault(workspaceId, vault);
}

// --- Member identity ---
// The derived X25519 private key and member UUID are cached in the keychain
// after the first passphrase unlock, so subsequent commands are silent.

export type Identity = {
  memberId: string;
  privateKey: string; // base64
};

export async function saveIdentity(workspaceId: string, identity: Identity): Promise<void> {
  const vault = await loadVault(workspaceId);
  vault["identity"] = JSON.stringify(identity);
  await saveVault(workspaceId, vault);
}

export async function loadIdentity(workspaceId: string): Promise<Identity | null> {
  const vault = await loadVault(workspaceId);
  const raw = vault["identity"];
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}
