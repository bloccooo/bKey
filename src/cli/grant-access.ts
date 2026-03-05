import * as p from "@clack/prompts";
import * as A from "@automerge/automerge";
import { readConfig } from "../config";
import { backendFromConfig } from "../storage";
import { unlockWorkspace } from "./unlock";
import { persist } from "../store";
import { wrapDek } from "../crypto";

export async function cmdGrantAccess() {
  const config = await readConfig();
  if (!config) {
    console.error("error: workspace not initialised. Run: bkey init");
    process.exit(1);
  }

  p.intro("bkey grant-access");

  const backend = await backendFromConfig(config.storage);
  const { doc, session } = await unlockWorkspace(backend);

  const pending = Object.values(doc.members ?? {}).filter((m) => !m.wrappedDek);

  if (pending.length === 0) {
    p.outro("No pending access requests.");
    return;
  }

  const selected = await p.multiselect({
    message: `${pending.length} pending request(s). Select members to approve (others will be removed):`,
    options: pending.map((m) => ({
      value: m.id,
      label: `${m.id}`,
      hint: `key: ${m.publicKey.slice(0, 20)}…`,
    })),
    required: false,
  });

  if (p.isCancel(selected)) {
    p.cancel("Cancelled.");
    return;
  }

  const approvedIds = new Set(selected as string[]);
  const toGrant = pending.filter((m) => approvedIds.has(m.id));
  const toRemove = pending.filter((m) => !approvedIds.has(m.id));

  if (toGrant.length === 0 && toRemove.length === 0) {
    p.outro("No changes made.");
    return;
  }

  const updated = A.change(doc, "grant access", (d) => {
    for (const m of toGrant) {
      const entry = d.members[m.id];
      if (!entry) continue;
      entry.wrappedDek = wrapDek(
        session.dek,
        Buffer.from(m.publicKey, "base64"),
      );
    }
    for (const m of toRemove) {
      delete d.members[m.id];
    }
  });

  await persist(updated, backend);

  p.outro(`Done. Granted: ${toGrant.length}, removed: ${toRemove.length}.`);
}
