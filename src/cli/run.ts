import * as p from "@clack/prompts";
import { readConfig, type WorkspaceConfig } from "../config";
import { readBKeyFile } from "../bkey-file";
import { listSecrets } from "../secrets";
import { Store, unlock } from "../store";
import { derivePrivateKey } from "../crypto";

function parseArgs(args: string[]): {
  projectName?: string;
  dryRun: boolean;
  cmd: string[];
} {
  const dashDash = args.indexOf("--");
  const before = dashDash === -1 ? args : args.slice(0, dashDash);
  const cmd = dashDash === -1 ? [] : args.slice(dashDash + 1);

  let projectName: string | undefined;
  let dryRun = false;

  for (let i = 0; i < before.length; i++) {
    if ((before[i] === "--project" || before[i] === "-p") && before[i + 1]) {
      projectName = before[++i];
    } else if (before[i] === "--dry-run") {
      dryRun = true;
    }
  }

  return { projectName, dryRun, cmd };
}

export async function cmdRun(args: string[]) {
  const { projectName: projectArg, dryRun, cmd } = parseArgs(args);

  // Resolve project name: flag → .bkey file → all secrets
  let projectName = projectArg;
  if (!projectName) {
    const bkeyFile = await readBKeyFile();
    projectName = bkeyFile.project;
  }

  const config = await readConfig();
  if (!config || config.workspaces.length === 0) {
    console.error("No workspaces found. Run: bkey setup");
    process.exit(1);
  }

  let workspace: WorkspaceConfig;

  if (config.workspaces.length === 1) {
    workspace = config.workspaces[0] as WorkspaceConfig;
  } else {
    const selected = await p.select({
      message: "Select workspace",
      options: config.workspaces.map((w) => ({ value: w.id, label: w.name })),
    });

    if (p.isCancel(selected)) {
      p.cancel("Cancelled.");
      process.exit(0);
    }

    workspace = config.workspaces.find(
      (w) => w.id === selected,
    ) as WorkspaceConfig;
  }

  const store = new Store({
    memberId: config.memberId,
    workspaceId: workspace.id,
    storage: workspace.storage,
  });

  const doc = await store.pull();
  const privateKey = derivePrivateKey(config.passphrase, doc.id);
  const { session, doc: unlockedDoc } = await unlock(doc, privateKey);

  const allSecrets = listSecrets(unlockedDoc, session.dek);

  // Build env vars from the project's secrets (or all secrets if no project)
  const envVars: Record<string, string> = {};

  if (projectName) {
    const project = Object.values(unlockedDoc.projects).find(
      (p) => p.name === projectName,
    );
    if (!project) {
      console.error(`error: project "${projectName}" not found`);
      process.exit(1);
    }
    for (const id of project.secret_ids) {
      const secret = allSecrets.find((s) => s.id === id);
      if (secret) envVars[secret.name] = secret.value;
    }
  } else {
    for (const secret of allSecrets) {
      envVars[secret.name] = secret.value;
    }
  }

  if (dryRun) {
    const label = projectName ? `project "${projectName}"` : "all secrets";
    console.log(`\nEnv vars that would be injected (${label}):\n`);
    for (const [k, v] of Object.entries(envVars)) {
      console.log(`  ${k}=${v}`);
    }
    console.log();
    return;
  }

  if (cmd.length === 0) {
    console.error(
      "error: no command given. Usage: bkey run [options] -- <command>",
    );
    process.exit(1);
  }

  const [bin, ...rest] = cmd;
  const proc = Bun.spawn([bin!, ...rest], {
    env: { ...process.env, ...envVars },
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  });

  process.exit(await proc.exited);
}
