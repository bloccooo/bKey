import { Operator } from "opendal";
import {
  configToBackendOptions,
  type LocalStorageConfig,
  type S3StorageConfig,
  type StorageConfig,
} from "./config";
import { loadCredentials } from "./keychain";

const ROOT = "_bkey";
const DOC_EXTENSION = "bkey.enc";
const DOC_PATH = "bkey.enc";
const CACHE_PATH = ".bkey.cache";

export type BKeyStorageConfig = StorageConfig & {
  memberId: string;
  workspaceId: string;
};

export interface StorageBackend {
  push(data: Uint8Array): Promise<void>;
  pull(): Promise<Uint8Array[]>;
  check(): Promise<boolean>;
}

export function createBackend(
  config: BKeyStorageConfig,
  op: Operator,
): StorageBackend {
  return {
    async push(data) {
      await op.write(
        `/${ROOT}/${config.memberId}.${DOC_EXTENSION}`,
        Buffer.from(data),
      );
    },
    async pull() {
      try {
        const entries = await op.list(`/${ROOT}/`, { recursive: true });
        const bkeyEntries = entries.filter((e) =>
          e.path().endsWith(`.${DOC_EXTENSION}`),
        );

        const files = await Promise.all(
          bkeyEntries.map((e) => op.read(e.path())),
        );

        return files;
      } catch {
        return [];
      }
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

// --- Local filesystem backend (via opendal) ---

export function localBackend(config: LocalStorageConfig): StorageBackend {
  const op = new Operator("fs", { root: config.root || "." });
  return createBackend(config, op);
}

export function s3Backend(
  config: BKeyStorageConfig & {
    accessKeyId: string;
    secretAccessKey: string;
  },
): StorageBackend {
  const op = new Operator("s3", {
    bucket: config.bucket,
    region: config.region,
    ...(config.endpoint ? { endpoint: config.endpoint } : {}),
    access_key_id: config.accessKeyId,
    secret_access_key: config.secretAccessKey,
    root: config.root || "/",
  });
  return createBackend(config, op);
}

// --- Backend from bkey.config.json (credentials loaded from keychain) ---

export function backendFromConfigAndCreds(
  config: BKeyStorageConfig,
  creds: Record<string, string> = {},
): StorageBackend {
  const { type, options } = configToBackendOptions(config, creds);
  const op = new Operator(type, options);
  return createBackend(config, op);
}

export async function backendFromConfig(
  config: BKeyStorageConfig,
  workspaceId: string,
): Promise<StorageBackend> {
  const creds = (await loadCredentials(config.backend, workspaceId)) ?? {};
  return backendFromConfigAndCreds(config, creds);
}

// --- Local cache backend (.bkey.cache in the current directory) ---

export function cacheBackend(): StorageBackend {
  const op = new Operator("fs", { root: "." });
  return createBackend({ backend: "local", root: "." }, op);
}

// --- In-memory backend (useful for testing) ---

export function memoryBackend(): StorageBackend {
  const op = new Operator("memory", {});
  return createBackend(op);
}
