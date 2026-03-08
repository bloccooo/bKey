import envPaths from "env-paths";
import path from "node:path";
import type { StorageConfig } from "./storage";

type ConfigVersion = "v1";
const CONFIG_PATH = path.join(envPaths("bkey").config, "config.json");

export type WorkspaceConfig = {
  id: string;
  name: string;
  storage: StorageConfig;
};

export type BKeyConfig = {
  version: ConfigVersion;
  memberName: string;
  memberId: string;
  passphrase: string;
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
