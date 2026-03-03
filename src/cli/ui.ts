import { render } from "ink";
import React from "react";
import { Box, Text } from "ink";
import { App } from "../tui/App";
import { readConfig } from "../config";
import { backendFromConfig, localBackend } from "../storage";
import { unlockWorkspace } from "./unlock";

export async function cmdUi() {
  const config = await readConfig();
  const backend = config ? await backendFromConfig(config.storage) : localBackend(".");

  const { rerender } = render(
    React.createElement(Box, { paddingX: 1 },
      React.createElement(Text, { dimColor: true }, "Loading secrets…")
    )
  );

  const { doc, session } = await unlockWorkspace(config, backend);
  rerender(React.createElement(App, { initialDoc: doc, backend, session }));
}
