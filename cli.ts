import { cmdUi } from "./src/cli/ui";
import { cmdRun } from "./src/cli/run";
import { cmdSetup } from "./src/cli/setup";

const [cmd, ...rest] = process.argv.slice(2);

switch (cmd) {
  case "setup":
    await cmdSetup(rest[0]);
    break;
  case "ui":
  case undefined:
    await cmdUi();
    break;
  case "run":
    await cmdRun(rest);
    break;
  default:
    console.log("Usage: bkey [command]");
    console.log("Commands:");
    console.log("  (none)           Open the TUI dashboard");
    console.log(
      "  init [invite]    Set up a new workspace, or join an existing one",
    );
    console.log(
      "  setup            Set up a new workspace, or join an existing one",
    );
    console.log("  run -- <cmd>     Inject secrets into a subprocess");
    console.log("    --project <n>  Override project (default: from .bkey)");
    console.log("    --dry-run      Print vars that would be injected");
}
