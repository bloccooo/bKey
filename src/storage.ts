import { Operator } from "opendal";
import { configToBackendOptions, type StorageConfig } from "./config";
import { loadCredentials } from "./keychain";

const DOC_PATH = "bkey.enc";
const CACHE_PATH = ".bkey.cache";

export interface StorageBackend {
  push(data: Uint8Array): Promise<void>;
  pull(): Promise<Uint8Array | null>;
}

// --- Local filesystem backend (via opendal) ---

export function localBackend(root: string): StorageBackend {
  const op = new Operator("fs", { root });
  return {
    async push(data) {
      await op.write(DOC_PATH, Buffer.from(data));
    },
    async pull() {
      try {
        return await op.read(DOC_PATH);
      } catch {
        return null; // file doesn't exist yet
      }
    },
  };
}

// --- S3-compatible backend (S3, R2, GCS via S3 compat) ---

export type S3Config = {
  bucket: string;
  region: string;
  endpoint?: string;     // override for R2 / minio
  accessKeyId: string;
  secretAccessKey: string;
};

export function s3Backend(config: S3Config): StorageBackend {
  const op = new Operator("s3", {
    bucket: config.bucket,
    region: config.region,
    ...(config.endpoint ? { endpoint: config.endpoint } : {}),
    access_key_id: config.accessKeyId,
    secret_access_key: config.secretAccessKey,
  });
  return {
    async push(data) {
      await op.write(DOC_PATH, Buffer.from(data));
    },
    async pull() {
      try {
        return await op.read(DOC_PATH);
      } catch {
        return null;
      }
    },
  };
}

// --- Backend from bkey.config.json (credentials loaded from keychain) ---

export function backendFromConfigAndCreds(
  config: StorageConfig,
  creds: Record<string, string> = {},
): StorageBackend {
  const { type, options } = configToBackendOptions(config, creds);
  const op = new Operator(type, options);
  return {
    async push(data) {
      await op.write(DOC_PATH, Buffer.from(data));
    },
    async pull() {
      try {
        return await op.read(DOC_PATH);
      } catch {
        return null;
      }
    },
  };
}

export async function backendFromConfig(
  config: StorageConfig,
  workspaceId: string,
): Promise<StorageBackend> {
  const creds = await loadCredentials(config.backend, workspaceId) ?? {};
  return backendFromConfigAndCreds(config, creds);
}

// --- Local cache backend (.bkey.cache in the current directory) ---

export function cacheBackend(): StorageBackend {
  const op = new Operator("fs", { root: "." });
  return {
    async push(data) {
      await op.write(CACHE_PATH, Buffer.from(data));
    },
    async pull() {
      try {
        return await op.read(CACHE_PATH);
      } catch {
        return null;
      }
    },
  };
}

// --- In-memory backend (useful for testing) ---

export function memoryBackend(): StorageBackend {
  const op = new Operator("memory", {});
  return {
    async push(data) {
      await op.write(DOC_PATH, Buffer.from(data));
    },
    async pull() {
      try {
        return await op.read(DOC_PATH);
      } catch {
        return null;
      }
    },
  };
}
