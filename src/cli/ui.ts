import * as p from "@clack/prompts";
import { render } from "ink";
import React from "react";
import { Box, Text } from "ink";
import { App } from "../tui/App";
import { readConfig, type WorkspaceConfig } from "../config";
import { Store, unlock } from "../store";
import { generateInvite } from "../invite";
import { derivePrivateKey } from "../crypto";

export async function cmdUi() {
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

  // Pull and unlock before ink takes over the terminal
  const doc = await store.pull();
  const workspaceId = doc.id;

  const privateKey = derivePrivateKey(config.passphrase, workspaceId);

  const { rerender, unmount } = render(
    React.createElement(
      Box,
      { paddingX: 1 },
      React.createElement(Text, { dimColor: true }, "Loading secrets…"),
    ),
  );

  try {
    const { session, doc: updatedDoc } = await unlock(doc, privateKey);
    const inviteLink = await generateInvite(workspace.storage, {
      id: doc.id,
      name: doc.name,
    });

    rerender(
      React.createElement(App, {
        initialDoc: updatedDoc,
        store,
        session,
        inviteLink,
      }),
    );
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    rerender(
      React.createElement(
        Box,
        { paddingX: 1, gap: 1 },
        React.createElement(Text, { color: "red" }, "✗"),
        React.createElement(Text, null, message),
      ),
    );
    unmount();
    process.exit(1);
  }
}
