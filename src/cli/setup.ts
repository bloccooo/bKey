import * as p from "@clack/prompts";
import * as A from "@automerge/automerge";
import { readConfig, writeConfig, type StorageConfig } from "../config";
import { randomUUIDv7 } from "bun";
import { applyInvite } from "../invite";
import { createHash } from "crypto";
import { backendFromConfigAndCreds } from "../storage";
import { persist, pullRemoteDocument } from "../store";
import type { BKeyDocument } from "../types";

function memberIdFromMemberName(memberName: string): string {
  const h = createHash("sha256").update(memberName).digest("hex");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20, 32)}`;
}

async function collectStorageConfig(): Promise<{
  storage: StorageConfig;
  credentials: Record<string, string>;
} | null> {
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

export async function cmdSetup(inviteLink?: string) {
  p.intro("bkey setup");

  let config = await readConfig();

  if (!config) {
    const memberName = await p.text({
      message: "Member name",
    });

    if (p.isCancel(memberName)) {
      p.cancel("Cancelled.");
      return null;
    }

    config = {
      version: "v1",
      memberName,
      memberId: memberIdFromMemberName(memberName),
      workspaces: [],
    };

    await writeConfig(config);
  }

  // Initialize new workspace if none
  if (config.workspaces.length === 0) {
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

    if (initializationAction === "import") {
      const inviteLink = await p.text({
        message: "Invite link",
      });

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

      payload.storage.memberId = config.memberId;

      if (payload.storage.backend === "s3") {
        const credentials = await p.group(
          {
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

        const backend = backendFromConfigAndCreds(payload.storage, credentials);
        const isValid = await backend.check();

        if (!isValid) {
          p.cancel("Invalid storage.");
          return null;
        }

        let document = await pullRemoteDocument(backend);

        if (!document) {
          const name = await p.text({
            message: "Workspace name",
          });

          if (p.isCancel(name)) {
            p.cancel("Cancelled.");
            return null;
          }

          document = A.init<BKeyDocument>();

          if (!document) {
            p.cancel("Unable to initialize document.");
            return null;
          }

          document = A.change(document, "init workspace", (d) => {
            d.id = randomUUIDv7();
            d.name = name;
            d.doc_version = 0;
            d.members = {};
            d.projects = {};
            d.secrets = {};
          });
        }

        config.workspaces.push({
          id: document.id,
          name: document.name,
          storage: payload.storage,
        });

        // await writeConfig(config);
        await persist(document, backend);

        // console.log(credentials);
      }

      console.log(payload);
    } else {
      const name = await p.text({
        message: "Workspace name",
      });

      if (p.isCancel(name)) {
        p.cancel("Cancelled.");
        return null;
      }

      const backend = await p.select({
        message: "Choose a storage backend",
        options: [
          {
            value: "local",
            label: "Local filesystem",
            hint: "good for testing",
          },
          {
            value: "s3",
            label: "S3-compatible",
            hint: "AWS S3, MinIO, Backblaze B2, etc.",
          },
          { value: "r2", label: "Cloudflare R2" },
          {
            value: "webdav",
            label: "WebDAV",
            hint: "Nextcloud, ownCloud, etc.",
          },
        ],
      });
      if (p.isCancel(backend)) {
        p.cancel("Cancelled.");
        return null;
      }
    }
  }
}
