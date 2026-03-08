import * as p from "@clack/prompts";
import { readConfig, writeConfig } from "../config";
import { randomUUIDv7 } from "bun";
import { applyInvite } from "../invite";
import { createHash } from "crypto";
import * as A from "@automerge/automerge";
import { Store } from "../store";
import { type StorageConfig } from "../storage";
import {
  derivePrivateKey,
  generateDek,
  getPublicKey,
  wrapDek,
} from "../crypto";

function memberIdFromMemberName(memberName: string): string {
  const h = createHash("sha256").update(memberName).digest("hex");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20, 32)}`;
}

async function collectStorageConfig(): Promise<StorageConfig> {
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
    process.exit(0);
  }

  if (backend === "local") {
    const root = await p.text({
      message: "Storage path",
      placeholder: "./bkey-storage",
      defaultValue: "./bkey-storage",
    });
    if (p.isCancel(root)) {
      p.cancel("Cancelled.");
      process.exit(0);
    }
    return { type: "fs", root };
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
      type: "s3",
      bucket: group.bucket,
      region: group.region,
      ...(group.endpoint ? { endpoint: group.endpoint } : {}),
      accessKeyId: group.accessKeyId,
      secretAccessKey: group.secretAccessKey,
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
      type: "r2",
      accountId: group.accountId,
      bucket: group.bucket,
      accessKeyId: group.accessKeyId,
      secretAccessKey: group.secretAccessKey,
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
    type: "webdav",
    endpoint: group.endpoint,
    username: group.username,
    password: group.password,
  };
}

export async function cmdSetup(inviteLinkArg?: string) {
  p.intro("bkey setup");

  let config = await readConfig();

  if (!config) {
    const group = await p.group(
      {
        memberName: () => p.text({ message: "Member name" }),
        passphrase: () => p.password({ message: "Pass Phrase" }),
      },
      {
        onCancel: () => {
          p.cancel("Cancelled.");
          process.exit(0);
        },
      },
    );

    config = {
      version: "v1",
      memberName: group.memberName,
      passphrase: group.passphrase,
      memberId: memberIdFromMemberName(group.memberName),
      workspaces: [],
    };

    await writeConfig(config);
  }

  const initializationAction = await p.select({
    message: "Initialize workspace",
    options: [
      { value: "create", label: "Create new workspace" },
      {
        value: "import",
        label: "Import existing workspace",
      },
    ],
  });

  if (p.isCancel(initializationAction)) {
    p.cancel("Cancelled.");
    return null;
  }

  if (initializationAction === "import" || inviteLinkArg) {
    const inviteLink =
      inviteLinkArg ||
      (await p.text({
        message: "Invite link",
      }));

    if (p.isCancel(inviteLink)) {
      p.cancel("Cancelled.");
      return null;
    }

    let payload;
    try {
      payload = applyInvite(inviteLink);
    } catch (err) {
      p.cancel(err instanceof Error ? err.message : "Invalid invite link");
      return;
    }

    const store = new Store({
      memberId: config.memberId,
      workspaceId: payload.workspace.id,
      storage: payload.storage,
    });

    let doc = await store.pull();

    const privateKey = derivePrivateKey(config.passphrase, doc.id);
    const publicKey = getPublicKey(privateKey);

    config.workspaces.push({
      id: doc.id,
      name: doc.name,
      storage: payload.storage,
    });

    await writeConfig(config);

    doc = A.change(doc, "add member", (d) => {
      d.members[config.memberId] = {
        id: config.memberId,
        email: config.memberName,
        publicKey: Buffer.from(publicKey).toString("base64"),
        wrappedDek: "", // Pending validation
      };
    });

    await store.persist(doc);
  } else {
    const name = await p.text({
      message: "Workspace name",
    });

    if (p.isCancel(name)) {
      p.cancel("Cancelled.");
      return null;
    }

    const storageConfig = await collectStorageConfig();
    const workspaceId = randomUUIDv7();

    const store = new Store({
      memberId: config.memberId,
      workspaceId,
      storage: storageConfig,
    });

    config.workspaces.push({
      id: workspaceId,
      name: name,
      storage: storageConfig,
    });

    await writeConfig(config);

    // Initialize document
    let doc = await store.pull();

    const privateKey = derivePrivateKey(config.passphrase, doc.id);
    const publicKey = getPublicKey(privateKey);
    const dek = generateDek();
    const wrappedDek = wrapDek(dek, publicKey);

    doc = A.change(doc, "add member", (d) => {
      d.members[config.memberId] = {
        id: config.memberId,
        email: config.memberName,
        publicKey: Buffer.from(publicKey).toString("base64"),
        wrappedDek,
      };
    });

    await store.persist(doc);
  }
}
