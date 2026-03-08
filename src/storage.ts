import { Operator } from "opendal";

const DOC_EXTENSION = "bkey.enc";

export type LocalStorageConfig = {
  type: "fs";
  root: string;
};

export type S3StorageConfig = {
  type: "s3";
  bucket: string;
  region: string;
  endpoint?: string; // override for MinIO, Backblaze B2, etc.
  accessKeyId: string;
  secretAccessKey: string;
};

export type R2StorageConfig = {
  type: "r2";
  accountId: string;
  bucket: string;
  accessKeyId: string;
  secretAccessKey: string;
};

export type WebDavStorageConfig = {
  type: "webdav";
  endpoint: string; // e.g. https://dav.example.com/vault
  username: string;
  password: string;
};

export type StorageConfig =
  | LocalStorageConfig
  | S3StorageConfig
  | R2StorageConfig
  | WebDavStorageConfig;

export interface StorageBackend {
  push(path: string, data: Uint8Array): Promise<void>;
  pull(path: string): Promise<Uint8Array[]>;
  check(): Promise<boolean>;
}

function remapStorageConfigToOpendalConfig(config: StorageConfig) {
  return config.type === "s3"
    ? {
        ...config,
        access_key_id: config.accessKeyId,
        secret_access_key: config.secretAccessKey,
      }
    : config.type === "r2"
      ? {
          ...config,
          access_key_id: config.accessKeyId,
          secret_access_key: config.secretAccessKey,
        }
      : config;
}

// --- Backend from bkey.config.json (credentials loaded from keychain) ---

export function backendFromConfig(config: StorageConfig): StorageBackend {
  const { type, ...options } = remapStorageConfigToOpendalConfig(config);
  const op = new Operator(type, { ...options });

  return {
    async push(path, data) {
      await op.write(path, Buffer.from(data));
    },
    async pull(path) {
      const entries = await op.list(path, { recursive: true });
      const bkeyEntries = entries.filter((e) =>
        e.path().endsWith(`.${DOC_EXTENSION}`),
      );

      const files = await Promise.all(
        bkeyEntries.map((e) => op.read(e.path())),
      );

      return files;
    },
    async check() {
      try {
        await op.check();
        return true;
      } catch {
        return false;
      }
    },
  };
}
