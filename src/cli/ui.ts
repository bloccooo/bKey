import { render } from "ink";
import React from "react";
import { Box, Text } from "ink";
import { App } from "../tui/App";
import { readConfig } from "../config";
import { backendFromConfig, backendFromConfigAndCreds, cacheBackend, localBackend } from "../storage";
import { unlockWorkspace } from "./unlock";
import { generateInvite } from "../invite";
import { peekWorkspaceId } from "../store";

export async function cmdUi() {
  const config = await readConfig();

  // Peek at the local cache to get workspaceId so credentials can be loaded
  // from the right vault entry before connecting to the remote backend.
  let backend;
  if (config) {
    const workspaceId = await peekWorkspaceId(cacheBackend());
    backend = workspaceId
      ? await backendFromConfig(config.storage, workspaceId)
      : backendFromConfigAndCreds(config.storage, {});
  } else {
    backend = localBackend(".");
  }

  const { rerender, unmount } = render(
    React.createElement(Box, { paddingX: 1 },
      React.createElement(Text, { dimColor: true }, "Loading secrets…")
    )
  );

  try {
    const { doc, session } = await unlockWorkspace(backend);
    const inviteLink = config ? await generateInvite(config.storage, doc.id) : undefined;
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
