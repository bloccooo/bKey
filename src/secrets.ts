import * as A from "@automerge/automerge";
import { randomUUIDv7 } from "bun";
import type { Secret, PlaintextSecret, BKeyDocument } from "./types";
import { encryptField, decryptField } from "./crypto";

function encryptSecret(
  fields: Omit<PlaintextSecret, "id">,
  dek: Uint8Array,
): Omit<Secret, "id"> {
  return {
    name: encryptField(fields.name, dek),
    value: encryptField(fields.value, dek),
    description: encryptField(fields.description, dek),
    tags: encryptField(JSON.stringify(fields.tags), dek),
  };
}

function decryptSecret(s: Secret, dek: Uint8Array): PlaintextSecret {
  return {
    id: s.id,
    name: decryptField(s.name, dek),
    value: decryptField(s.value, dek),
    description: decryptField(s.description, dek),
    tags: JSON.parse(decryptField(s.tags, dek)),
  };
}

export function addSecret(
  doc: A.Doc<BKeyDocument>,
  dek: Uint8Array,
  fields: Omit<PlaintextSecret, "id">,
): A.Doc<BKeyDocument> {
  const id = randomUUIDv7();
  const encrypted = encryptSecret(fields, dek);
  return A.change(doc, `add secret`, (d) => {
    d.secrets[id] = { id, ...encrypted };
  });
}

export function removeSecret(
  doc: A.Doc<BKeyDocument>,
  id: string,
): A.Doc<BKeyDocument> {
  return A.change(doc, `remove secret ${id}`, (d) => {
    delete d.secrets[id];
    for (const project of Object.values(d.projects)) {
      const idx = project.secret_ids.indexOf(id);
      if (idx !== -1) project.secret_ids.splice(idx, 1);
    }
  });
}

export function updateSecret(
  doc: A.Doc<BKeyDocument>,
  dek: Uint8Array,
  id: string,
  fields: Omit<PlaintextSecret, "id">,
): A.Doc<BKeyDocument> {
  const encrypted = encryptSecret(fields, dek);
  return A.change(doc, `update secret ${id}`, (d) => {
    const s = d.secrets[id];
    if (!s) throw new Error(`Secret ${id} not found`);
    s.name = encrypted.name;
    s.value = encrypted.value;
    s.description = encrypted.description;
    s.tags = encrypted.tags;
  });
}

export function listSecrets(
  doc: A.Doc<BKeyDocument>,
  dek: Uint8Array,
): PlaintextSecret[] {
  return Object.values(doc.secrets).map((s) => decryptSecret(s, dek));
}
