import envPaths from "env-paths";
import path from "node:path";

type ConfigVersion = "v1";
const CONFIG_PATH = path.join(envPaths("bkey").config, "config.json");

export type SharedStorageConfig = {
  root?: string;
  // memberId: string;
};

export type LocalStorageConfig = SharedStorageConfig & {
  backend: "local";
};

export type S3StorageConfig = SharedStorageConfig & {
  backend: "s3";
  bucket: string;
  region: string;
  endpoint?: string; // override for MinIO, Backblaze B2, etc.
  // credentials stored in keychain under account "s3"
};

export type R2StorageConfig = SharedStorageConfig & {
  backend: "r2";
  accountId: string;
  bucket: string;
  // credentials stored in keychain under account "r2"
};

export type WebDavStorageConfig = SharedStorageConfig & {
  backend: "webdav";
  endpoint: string; // e.g. https://dav.example.com/vault
  // credentials stored in keychain under account "webdav"
};

export type StorageConfig =
  | LocalStorageConfig
  | S3StorageConfig
  | R2StorageConfig
  | WebDavStorageConfig;

export type WorkspaceConfig = {
  id: string;
  name: string;
  storage: StorageConfig;
};

export type BKeyConfig = {
  version: ConfigVersion;
  memberName: string;
  memberId: string;
  workspaces: WorkspaceConfig[];
};

export async function readConfig(): Promise<BKeyConfig | null> {
  const file = Bun.file(CONFIG_PATH);
  if (!(await file.exists())) return null;
  return file.json();
}

export async function writeConfig(config: BKeyConfig): Promise<void> {
  await Bun.write(CONFIG_PATH, JSON.stringify(config, null, 2));
}

export function configToBackendOptions(
  config: StorageConfig,
  creds: Record<string, string> = {},
): { type: string; options: Record<string, string> } {
  switch (config.backend) {
    case "local":
      return { type: "fs", options: { root: config.root || "/" } };
    case "s3": {
      const options: Record<string, string> = {
        bucket: config.bucket,
        region: config.region,
      };
      if (config.endpoint) options["endpoint"] = config.endpoint;
      if (creds.accessKeyId) options["access_key_id"] = creds.accessKeyId;
      if (creds.secretAccessKey)
        options["secret_access_key"] = creds.secretAccessKey;
      return { type: "s3", options };
    }
    case "r2": {
      const options: Record<string, string> = {
        bucket: config.bucket,
        region: "auto",
        endpoint: `https://${config.accountId}.r2.cloudflarestorage.com`,
      };
      if (creds.accessKeyId) options["access_key_id"] = creds.accessKeyId;
      if (creds.secretAccessKey)
        options["secret_access_key"] = creds.secretAccessKey;
      return { type: "s3", options };
    }
    case "webdav": {
      const options: Record<string, string> = { endpoint: config.endpoint };
      if (creds.username) options["username"] = creds.username;
      if (creds.password) options["password"] = creds.password;
      return { type: "webdav", options };
    }
  }
}
