import * as p from "@clack/prompts";
import { randomUUIDv7 } from "bun";
import * as A from "@automerge/automerge";
import { writeConfig, readConfig, type StorageConfig } from "../config";
import { saveCredentials, saveIdentity } from "../keychain";
import {
  backendFromConfig,
  backendFromConfigAndCreds,
  cacheBackend,
} from "../storage";
import type { StorageBackend } from "../storage";
import { loadOrCreate, persist, peekWorkspaceId } from "../store";
import { applyInvite } from "../invite";
import type { Workspace } from "../types";
import {
  derivePrivateKey,
  getPublicKey,
  generateDek,
  wrapDek,
} from "../crypto";

export async function cmdInit(inviteLink?: string) {
  const existing = await readConfig();

  p.intro("bkey init");

  // Step 1: Storage config + credentials (credentials saved to keychain later,
  // once doc.id is known so we can key the vault by workspaceId)
  let storage: StorageConfig;
  let inlineCredentials: Record<string, string> | null = null;

  if (inviteLink) {
    let payload;
    try {
      payload = applyInvite(inviteLink);
    } catch (err) {
      p.cancel(err instanceof Error ? err.message : "Invalid invite link");
      return;
    }
    storage = payload.storage;
    inlineCredentials = payload.credentials;
    p.log.info(`Storage: ${storage.backend} (from invite link)`);
  } else if (existing) {
    const overwrite = await p.confirm({
      message: "bkey.config.json already exists. Reconfigure storage?",
      initialValue: false,
    });
    if (p.isCancel(overwrite)) {
      p.cancel("Cancelled.");
      return;
    }
    if (overwrite) {
      const result = await collectStorageConfig();
      if (!result) return;
      storage = result.storage;
      inlineCredentials = result.credentials;
    } else {
      p.log.info(`Using existing ${existing.storage.backend} storage config.`);
      storage = existing.storage;
    }
  } else {
    const result = await collectStorageConfig();
    if (!result) return;
    storage = result.storage;
    inlineCredentials = result.credentials;
  }

  // Step 2: Build backend — use inline credentials if available, otherwise load
  // from the vault keyed by the workspaceId peeked from the local cache.
  let backend;
  if (inlineCredentials !== null) {
    backend = backendFromConfigAndCreds(storage, inlineCredentials);
  } else {
    const workspaceId = await peekWorkspaceId(cacheBackend());
    backend = workspaceId
      ? await backendFromConfig(storage, workspaceId)
      : backendFromConfigAndCreds(storage, {});
  }

  // Step 3: Load doc — now we know the workspaceId
  const doc = await loadOrCreate(backend, cacheBackend());

  // Save inline credentials to the workspace vault now that we have doc.id
  if (inlineCredentials && Object.keys(inlineCredentials).length > 0) {
    await saveCredentials(storage.backend, inlineCredentials, doc.id);
  }

  const members = Object.values(doc.members ?? {});

  if (members.length > 0) {
    // Workspace is already initialised → this user is joining, request access
    await requestAccessFlow(doc, backend);
    if (inviteLink || !existing) {
      await writeConfig({ storage });
    }
  } else {
    // Fresh workspace → full init (create DEK, add first member)
    await fullInitFlow(storage, backend, doc);
  }
}

type StorageConfigResult = {
  storage: StorageConfig;
  credentials: Record<string, string>;
};

async function collectStorageConfig(): Promise<StorageConfigResult | null> {
  const backend = await p.select({
    message: "Choose a storage backend",
    options: [
      { value: "local", label: "Local filesystem", hint: "good for testing" },
      {
        value: "s3",
        label: "S3-compatible",
        hint: "AWS S3, MinIO, Backblaze B2, etc.",
      },
      { value: "r2", label: "Cloudflare R2" },
      { value: "webdav", label: "WebDAV", hint: "Nextcloud, ownCloud, etc." },
    ],
  });
  if (p.isCancel(backend)) {
    p.cancel("Cancelled.");
    return null;
  }

  if (backend === "local") {
    const root = await p.text({
      message: "Storage path",
      placeholder: "./bkey-storage",
      defaultValue: "./bkey-storage",
    });
    if (p.isCancel(root)) {
      p.cancel("Cancelled.");
      return null;
    }
    return { storage: { backend: "local", root }, credentials: {} };
  }

  if (backend === "s3") {
    const group = await p.group(
      {
        bucket: () => p.text({ message: "Bucket name" }),
        region: () =>
          p.text({
            message: "Region",
            placeholder: "us-east-1",
            defaultValue: "us-east-1",
          }),
        endpoint: () =>
          p.text({
            message: "Endpoint URL (leave blank for AWS)",
            placeholder: "https://s3.example.com",
            defaultValue: "",
          }),
        accessKeyId: () => p.text({ message: "Access Key ID" }),
        secretAccessKey: () => p.password({ message: "Secret Access Key" }),
      },
      {
        onCancel: () => {
          p.cancel("Cancelled.");
          process.exit(0);
        },
      },
    );
    return {
      storage: {
        backend: "s3",
        bucket: group.bucket,
        region: group.region,
        ...(group.endpoint ? { endpoint: group.endpoint } : {}),
      },
      credentials: {
        accessKeyId: group.accessKeyId,
        secretAccessKey: group.secretAccessKey,
      },
    };
  }

  if (backend === "r2") {
    const group = await p.group(
      {
        accountId: () => p.text({ message: "Cloudflare Account ID" }),
        bucket: () => p.text({ message: "Bucket name" }),
        accessKeyId: () => p.text({ message: "R2 Access Key ID" }),
        secretAccessKey: () => p.password({ message: "R2 Secret Access Key" }),
      },
      {
        onCancel: () => {
          p.cancel("Cancelled.");
          process.exit(0);
        },
      },
    );
    return {
      storage: {
        backend: "r2",
        accountId: group.accountId,
        bucket: group.bucket,
      },
      credentials: {
        accessKeyId: group.accessKeyId,
        secretAccessKey: group.secretAccessKey,
      },
    };
  }

  // webdav
  const group = await p.group(
    {
      endpoint: () =>
        p.text({
          message: "WebDAV endpoint URL",
          placeholder: "https://dav.example.com/vault",
        }),
      username: () =>
        p.text({ message: "Username (leave blank if none)", defaultValue: "" }),
      password: () => p.password({ message: "Password (leave blank if none)" }),
    },
    {
      onCancel: () => {
        p.cancel("Cancelled.");
        process.exit(0);
      },
    },
  );
  return {
    storage: { backend: "webdav", endpoint: group.endpoint },
    credentials: {
      ...(group.username ? { username: group.username } : {}),
      ...(group.password ? { password: group.password } : {}),
    },
  };
}

async function requestAccessFlow(
  doc: A.Doc<Workspace>,
  backend: StorageBackend,
) {
  p.log.step("Workspace found. Requesting access…");

  const email = await p.text({ message: "Your email address" });
  if (p.isCancel(email) || !email) {
    p.cancel("Cancelled.");
    return;
  }

  const passphrase = await p.password({ message: "Create a passphrase" });
  if (p.isCancel(passphrase) || !passphrase) {
    p.cancel("Cancelled.");
    return;
  }

  const confirm = await p.password({ message: "Confirm passphrase" });
  if (p.isCancel(confirm)) {
    p.cancel("Cancelled.");
    return;
  }
  if (passphrase !== confirm) {
    p.cancel("Passphrases do not match.");
    return;
  }

  const privateKey = derivePrivateKey(passphrase, doc.id);
  const publicKey = getPublicKey(privateKey);
  const pubKeyB64 = Buffer.from(publicKey).toString("base64");

  const existingMember = Object.values(doc.members ?? {}).find(
    (m) => m.publicKey === pubKeyB64,
  );
  if (existingMember) {
    p.outro(
      existingMember.wrappedDek
        ? "You already have access to this workspace."
        : "Access request already pending. Wait for an existing member to run 'bkey grant-access'.",
    );
    return;
  }

  const memberId = randomUUIDv7();
  const updated = A.change(doc, "request access", (d) => {
    if (!d.members) d.members = {};
    d.members[memberId] = {
      id: memberId,
      email,
      publicKey: pubKeyB64,
      wrappedDek: "",
    };
  });

  await persist(updated, backend, cacheBackend());
  await saveIdentity(doc.id, {
    memberId,
    privateKey: Buffer.from(privateKey).toString("base64"),
  });

  p.outro(
    "Access requested. An existing member needs to run 'bkey grant-access' to approve you.",
  );
}

async function fullInitFlow(
  storage: StorageConfig,
  backend: StorageBackend,
  doc: A.Doc<Workspace>,
) {
  p.log.step("Setting up encryption keys…");

  const email = await p.text({ message: "Your email address" });
  if (p.isCancel(email) || !email) {
    p.cancel("Cancelled.");
    return;
  }

  const passphrase = await p.password({ message: "Create a passphrase" });
  if (p.isCancel(passphrase) || !passphrase) {
    p.cancel("Cancelled.");
    return;
  }

  const confirm = await p.password({ message: "Confirm passphrase" });
  if (p.isCancel(confirm)) {
    p.cancel("Cancelled.");
    return;
  }
  if (passphrase !== confirm) {
    p.cancel("Passphrases do not match.");
    return;
  }

  const memberId = randomUUIDv7();
  const privateKey = derivePrivateKey(passphrase, doc.id);
  const publicKey = getPublicKey(privateKey);
  const dek = generateDek();
  const wrappedDek = wrapDek(dek, publicKey);

  const updated = A.change(doc, "init members", (d) => {
    if (!d.members) d.members = {};
    d.members[memberId] = {
      id: memberId,
      email,
      publicKey: Buffer.from(publicKey).toString("base64"),
      wrappedDek,
    };
  });

  await persist(updated, backend, cacheBackend());
  await saveIdentity(updated.id, {
    memberId,
    privateKey: Buffer.from(privateKey).toString("base64"),
  });
  await writeConfig({ storage });

  p.outro("Workspace initialised. Run bkey ui to manage secrets.");
}
