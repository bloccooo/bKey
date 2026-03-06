// Collections are maps keyed by UUID so Automerge has stable CRDT keys.
// Secret fields (name, value, description, tags) are AES-256-GCM encrypted
// using the workspace DEK. The DEK itself is X25519/ECIES-wrapped per member.

export type Secret = {
  id: string; // plaintext UUID (CRDT key)
  name: string; // encrypted (base64 AES-GCM)
  value: string; // encrypted (base64 AES-GCM)
  description: string; // encrypted (base64 AES-GCM)
  tags: string; // encrypted JSON array (base64 AES-GCM)
};

export type Project = {
  id: string; // plaintext UUID (CRDT key)
  name: string; // plaintext (not sensitive)
  secret_ids: string[]; // plaintext UUIDs
};

export type Member = {
  id: string; // plaintext UUID (CRDT key)
  email: string; // plaintext identifier
  publicKey: string; // plaintext base64 X25519 public key
  wrappedDek: string; // ECIES-wrapped DEK; empty string = pending access
};

export type BKeyDocument = {
  id: string; // plaintext UUID
  name: string; // plaintext
  doc_version: number; // plaintext monotonic counter
  members: Record<string, Member>; // keyed by member UUID
  projects: Record<string, Project>; // keyed by project UUID
  secrets: Record<string, Secret>; // keyed by secret UUID
};

// In-memory plaintext view of a secret (after decryption)
export type PlaintextSecret = {
  id: string;
  name: string;
  value: string;
  description: string;
  tags: string[];
};
