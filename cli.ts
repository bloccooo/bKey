import { cmdInit } from "./src/cli/init";
import { cmdUi } from "./src/cli/ui";
import { cmdRun } from "./src/cli/run";
import { cmdRequestAccess } from "./src/cli/request-access";

const [cmd, ...rest] = process.argv.slice(2);

switch (cmd) {
  case "init":
    await cmdInit();
    break;
  case "ui":
    await cmdUi();
    break;
  case "run":
    await cmdRun(rest);
    break;
  case "request-access":
    await cmdRequestAccess();
    break;
  default:
    console.log("Usage: bkey <command>");
    console.log("Commands:");
    console.log("  init             Set up storage for this workspace");
    console.log("  ui               Open the TUI dashboard");
    console.log("  run -- <cmd>     Inject secrets into a subprocess");
    console.log("    --project <n>  Override project (default: from .bkey)");
    console.log("    --dry-run      Print vars that would be injected");
    console.log("  request-access   Add your public key to request DEK access");
}
