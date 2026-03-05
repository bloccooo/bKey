import { readConfig } from "../config";
import { backendFromConfig, localBackend } from "../storage";
import { readBKeyFile } from "../bkey-file";
import { listSecrets } from "../secrets";
import { unlockWorkspace } from "./unlock";

function parseArgs(args: string[]): {
  projectName?: string;
  dryRun: boolean;
  cmd: string[];
} {
  const dashDash = args.indexOf("--");
  const before = dashDash === -1 ? args : args.slice(0, dashDash);
  const cmd    = dashDash === -1 ? []   : args.slice(dashDash + 1);

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

  // Load doc from storage
  const config = await readConfig();
  const backend = config ? await backendFromConfig(config.storage) : localBackend(".");
  const { doc, session } = await unlockWorkspace(backend);

  const allSecrets = listSecrets(doc, session.dek);

  // Build env vars from the project's secrets (or all secrets if no project)
  const envVars: Record<string, string> = {};

  if (projectName) {
    const project = Object.values(doc.projects).find((p) => p.name === projectName);
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
    console.error("error: no command given. Usage: bkey run [options] -- <command>");
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
