import { render } from "ink";
import React from "react";
import { Box, Text } from "ink";
import { App } from "../tui/App";
import { readConfig } from "../config";
import { backendFromConfig, localBackend } from "../storage";
import { unlockWorkspace } from "./unlock";
import { generateInvite } from "../invite";

export async function cmdUi() {
  const config = await readConfig();
  const backend = config ? await backendFromConfig(config.storage) : localBackend(".");

  const { rerender, unmount } = render(
    React.createElement(Box, { paddingX: 1 },
      React.createElement(Text, { dimColor: true }, "Loading secrets…")
    )
  );

  try {
    const { doc, session } = await unlockWorkspace(backend);
    const inviteLink = config ? await generateInvite(config.storage) : undefined;
    rerender(React.createElement(App, { initialDoc: doc, backend, session, inviteLink }));
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    rerender(
      React.createElement(Box, { paddingX: 1, gap: 1 },
        React.createElement(Text, { color: "red" }, "✗"),
        React.createElement(Text, null, message),
      )
    );
    unmount();
    process.exit(1);
  }
}
