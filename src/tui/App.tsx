import { useState, useEffect } from "react";
import { Box, Text, useInput, useApp } from "ink";
import * as A from "@automerge/automerge";
import type { BKeyDocument, PlaintextSecret } from "../types";
import { addSecret, removeSecret, updateSecret, listSecrets } from "../secrets";
import {
  addProject,
  removeProject,
  updateProject,
  setProjectSecrets,
} from "../projects";
import { Store, type Session } from "../store";
import { wrapDek } from "../crypto";
import { ProjectPane } from "./ProjectPane";
import { SecretPane } from "./SecretPane";
import { MembersPane } from "./MembersPane";
import { Form, type FormField } from "./Form";
import { ProjectSecretsView } from "./ProjectSecretsView";

type Mode =
  | "list"
  | "new-secret"
  | "new-project"
  | "edit-secret"
  | "edit-project"
  | "project-secrets"
  | "invite";

const SECRET_FIELDS: FormField[] = [
  { label: "Name" },
  { label: "Value", secret: true },
  { label: "Description" },
  { label: "Tags (comma-separated)" },
];

const PROJECT_FIELDS: FormField[] = [{ label: "Name" }];

export const App = ({
  initialDoc,
  store,
  session,
  inviteLink,
}: {
  initialDoc: A.Doc<BKeyDocument>;
  store: Store;
  session: Session;
  inviteLink?: string;
}) => {
  const { exit } = useApp();
  const [doc, setDoc] = useState(initialDoc);
  const [mode, setMode] = useState<Mode>("list");
  const [focus, setFocus] = useState<"projects" | "secrets" | "members">(
    "projects",
  );
  const [projIdx, setProjIdx] = useState(0);
  const [secIdx, setSecIdx] = useState(0);
  const [showValues, setShowValues] = useState(false);
  const [syncing, setSyncing] = useState(false);

  // Form state
  const [fieldIdx, setFieldIdx] = useState(0);
  const [fieldInput, setFieldInput] = useState("");
  const [cursor, setCursor] = useState(0);
  const [collectedValues, setCollectedValues] = useState<string[]>([]);
  const [initialValues, setInitialValues] = useState<string[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null);

  // Project-secrets checklist state
  const [psCursor, setPsCursor] = useState(0);
  const [psSelectedIds, setPsSelectedIds] = useState<Set<string>>(new Set());

  const [memberIdx, setMemberIdx] = useState(0);
  const [memberToDelete, setMemberToDelete] = useState<string | null>(null);
  const [memberToGrant, setMemberToGrant] = useState<string | null>(null);

  const projects = Object.values(doc.projects);
  const secrets: PlaintextSecret[] = listSecrets(doc, session.dek);
  const members = Object.values(doc.members ?? {});
  const fields =
    mode === "new-secret" || mode === "edit-secret"
      ? SECRET_FIELDS
      : PROJECT_FIELDS;

  useEffect(() => {
    setSyncing(true);
    store.persist(doc)
      .then((merged) => {
        if (merged !== doc) setDoc(merged);
      })
      .catch(console.error)
      .finally(() => setSyncing(false));
  }, [doc]);

  const openNewForm = (m: "new-secret" | "new-project") => {
    setMode(m);
    setEditingId(null);
    setInitialValues([]);
    setFieldIdx(0);
    setFieldInput("");
    setCursor(0);
    setCollectedValues([]);
  };

  const openEditForm = (
    m: "edit-secret" | "edit-project",
    id: string,
    values: string[],
  ) => {
    setMode(m);
    setEditingId(id);
    setInitialValues(values);
    setFieldIdx(0);
    const first = values[0] ?? "";
    setFieldInput(first);
    setCursor(first.length);
    setCollectedValues([]);
  };

  const openProjectSecrets = (projectId: string) => {
    const project = doc.projects[projectId];
    if (!project) return;
    setPsCursor(0);
    setPsSelectedIds(new Set(project.secret_ids));
    setEditingId(projectId);
    setMode("project-secrets");
  };

  const advanceField = (value: string) => {
    const nextIdx = fieldIdx + 1;
    const nextInitial = initialValues[nextIdx] ?? "";
    setCollectedValues((prev) => [...prev, value]);
    setFieldIdx(nextIdx);
    setFieldInput(nextInitial);
    setCursor(nextInitial.length);
  };

  const submitForm = (allValues: string[]) => {
    if (mode === "new-secret") {
      const [name = "", value = "", description = "", tagsStr = ""] = allValues;
      setDoc((d) =>
        addSecret(d, session.dek, {
          name,
          value,
          description,
          tags: tagsStr
            .split(",")
            .map((t) => t.trim())
            .filter(Boolean),
        }),
      );
    } else if (mode === "new-project") {
      const [name = ""] = allValues;
      setDoc((d) => addProject(d, name));
    } else if (mode === "edit-secret" && editingId) {
      const [name = "", value = "", description = "", tagsStr = ""] = allValues;
      setDoc((d) =>
        updateSecret(d, session.dek, editingId, {
          name,
          value,
          description,
          tags: tagsStr
            .split(",")
            .map((t) => t.trim())
            .filter(Boolean),
        }),
      );
    } else if (mode === "edit-project" && editingId) {
      const [name = ""] = allValues;
      setDoc((d) => updateProject(d, editingId, name));
    }
    setMode("list");
  };

  useInput((char, key) => {
    // --- Member delete confirmation ---
    if (memberToDelete !== null) {
      if (char === "y") {
        setDoc((d) =>
          A.change(d, "remove member", (w) => {
            delete w.members[memberToDelete];
          }),
        );
        setMemberIdx((i) => Math.max(0, i - 1));
        setMemberToDelete(null);
      } else if (char === "n" || key.escape) {
        setMemberToDelete(null);
      }
      return;
    }

    // --- Member grant confirmation ---
    if (memberToGrant !== null) {
      if (char === "y") {
        const member = members.find((m) => m.id === memberToGrant);
        if (member) {
          setDoc((d) =>
            A.change(d, "grant access", (w) => {
              const entry = w.members[member.id];
              if (entry)
                entry.wrappedDek = wrapDek(
                  session.dek,
                  Buffer.from(member.publicKey, "base64"),
                );
            }),
          );
        }
        setMemberToGrant(null);
      } else if (char === "n" || key.escape) {
        setMemberToGrant(null);
      }
      return;
    }

    // --- Invite view ---
    if (mode === "invite") {
      if (key.escape) {
        setMode("list");
        return;
      }
      return;
    }

    // --- Project-secrets checklist ---
    if (mode === "project-secrets") {
      if (key.escape) {
        setMode("list");
        return;
      }

      if (key.upArrow) {
        setPsCursor((c) => Math.max(0, c - 1));
        return;
      }
      if (key.downArrow) {
        setPsCursor((c) => Math.min(secrets.length - 1, c + 1));
        return;
      }

      if (char === " ") {
        const secret = secrets[psCursor];
        if (secret) {
          setPsSelectedIds((prev) => {
            const next = new Set(prev);
            next.has(secret.id) ? next.delete(secret.id) : next.add(secret.id);
            return next;
          });
        }
        return;
      }

      if (key.return && editingId) {
        setDoc((d) => setProjectSecrets(d, editingId, [...psSelectedIds]));
        setMode("list");
        return;
      }
      return;
    }

    // --- Form mode ---
    if (mode !== "list") {
      if (key.escape) {
        setMode("list");
        return;
      }

      if (key.return) {
        const allValues = [...collectedValues, fieldInput];
        if (fieldIdx < fields.length - 1) {
          advanceField(fieldInput);
        } else {
          submitForm(allValues);
        }
        return;
      }

      if (key.leftArrow) {
        setCursor((c) => Math.max(0, c - 1));
        return;
      }
      if (key.rightArrow) {
        setCursor((c) => Math.min(fieldInput.length, c + 1));
        return;
      }
      if (key.home) {
        setCursor(0);
        return;
      }
      if (key.end) {
        setCursor(fieldInput.length);
        return;
      }

      if (key.backspace || key.delete) {
        if (cursor > 0) {
          setFieldInput((v) => v.slice(0, cursor - 1) + v.slice(cursor));
          setCursor((c) => c - 1);
        }
        return;
      }

      if (char && !key.ctrl && !key.meta) {
        setFieldInput((v) => v.slice(0, cursor) + char + v.slice(cursor));
        setCursor((c) => c + 1);
      }
      return;
    }

    // --- List mode ---
    if (char === "q") {
      exit();
      return;
    }
    if (key.tab) {
      setFocus((f) =>
        f === "projects" ? "secrets" : f === "secrets" ? "members" : "projects",
      );
      return;
    }
    if (char === "v") {
      setShowValues((s) => !s);
      return;
    }
    if (char === "n" && focus !== "members") {
      openNewForm(focus === "projects" ? "new-project" : "new-secret");
      return;
    }

    if (key.upArrow) {
      if (focus === "projects") setProjIdx((i) => Math.max(0, i - 1));
      else if (focus === "secrets") setSecIdx((i) => Math.max(0, i - 1));
      else setMemberIdx((i) => Math.max(0, i - 1));
      return;
    }
    if (key.downArrow) {
      if (focus === "projects")
        setProjIdx((i) => Math.min(projects.length - 1, i + 1));
      else if (focus === "secrets")
        setSecIdx((i) => Math.min(secrets.length - 1, i + 1));
      else setMemberIdx((i) => Math.min(members.length - 1, i + 1));
      return;
    }

    if (char === "e") {
      if (focus === "secrets") {
        const sec = secrets[secIdx];
        if (sec)
          openEditForm("edit-secret", sec.id, [
            sec.name,
            sec.value,
            sec.description,
            sec.tags.join(", "),
          ]);
      } else if (focus === "projects") {
        const proj = projects[projIdx];
        if (proj) openEditForm("edit-project", proj.id, [proj.name]);
      }
      return;
    }

    if (char === "s" && focus === "projects") {
      const proj = projects[projIdx];
      if (proj) openProjectSecrets(proj.id);
      return;
    }

    if (char === "d") {
      if (focus === "projects") {
        const proj = projects[projIdx];
        if (proj) {
          setDoc((d) => removeProject(d, proj.id));
          setProjIdx((i) => Math.max(0, i - 1));
        }
      } else if (focus === "secrets") {
        const sec = secrets[secIdx];
        if (sec) {
          setDoc((d) => removeSecret(d, sec.id));
          setSecIdx((i) => Math.max(0, i - 1));
        }
      } else if (focus === "members") {
        const member = members[memberIdx];
        if (member && member.id !== session.memberId) {
          setMemberToDelete(member.id);
        }
      }
    }

    if (char === "g" && focus === "members") {
      const member = members[memberIdx];
      if (member && !member.wrappedDek) {
        setMemberToGrant(member.id);
      }
    }

    if (char === "i" && focus === "members" && inviteLink) {
      setMode("invite");
    }
  });

  if (mode === "invite") {
    return (
      <Box flexDirection="column" padding={1} gap={1}>
        <Text bold color="cyan">
          Invite Link
        </Text>
        <Text dimColor>Share this with the person you want to invite.</Text>
        <Text dimColor>It contains your storage config and credentials.</Text>
        <Box marginTop={1}>
          <Text wrap="wrap">{inviteLink}</Text>
        </Box>
        <Box marginTop={1}>
          <Text dimColor>They should run:</Text>
          <Text> bkey init {inviteLink}</Text>
        </Box>
        <Box marginTop={1}>
          <Text dimColor>[Esc] Close</Text>
        </Box>
      </Box>
    );
  }

  if (mode === "project-secrets") {
    const proj = editingId ? doc.projects[editingId] : null;
    return (
      <ProjectSecretsView
        projectName={proj?.name ?? ""}
        secrets={secrets}
        selectedIds={psSelectedIds}
        cursor={psCursor}
      />
    );
  }

  if (mode !== "list") {
    const title =
      mode === "new-secret"
        ? "New Secret"
        : mode === "new-project"
          ? "New Project"
          : mode === "edit-secret"
            ? "Edit Secret"
            : "Edit Project";
    return (
      <Box flexDirection="column" padding={1}>
        <Form
          title={title}
          fields={fields}
          currentField={fieldIdx}
          completedValues={collectedValues}
          currentInput={fieldInput}
          cursor={cursor}
        />
      </Box>
    );
  }

  const selectedMember = members[memberIdx];
  const footer =
    focus === "projects"
      ? "[n] New  [e] Edit  [s] Secrets  [d] Delete"
      : focus === "secrets"
        ? "[n] New  [e] Edit  [d] Delete  [v] " +
          (showValues ? "Hide" : "Show") +
          " values"
        : (selectedMember?.wrappedDek === "" ? "[g] Grant access  " : "") +
          "[d] Remove member" +
          (inviteLink ? "  [i] Invite" : "");

  return (
    <Box flexDirection="column">
      <Box paddingX={1} gap={2}>
        <Text bold color="cyan">
          bKey
        </Text>
        <Text dimColor>{doc.name}</Text>
        {syncing ? (
          <Text color="yellow">↑ syncing…</Text>
        ) : (
          <Text dimColor>✓ saved</Text>
        )}
      </Box>

      <Box marginTop={1}>
        <MembersPane
          members={members}
          selected={memberIdx}
          focused={focus === "members"}
          currentMemberId={session.memberId}
        />
      </Box>
      <Box flexDirection="row" gap={1}>
        <ProjectPane
          projects={projects}
          selected={projIdx}
          focused={focus === "projects"}
        />
        <SecretPane
          secrets={secrets}
          selected={secIdx}
          focused={focus === "secrets"}
          showValues={showValues}
        />
      </Box>

      <Box marginTop={1} paddingX={1}>
        {memberToDelete !== null ? (
          <Text color="yellow">
            Remove {members.find((m) => m.id === memberToDelete)?.email}? [y]
            Yes [n] No
          </Text>
        ) : memberToGrant !== null ? (
          <Text color="yellow">
            Grant access to {members.find((m) => m.id === memberToGrant)?.email}
            ? [y] Yes [n] No
          </Text>
        ) : (
          <Text dimColor>{footer} [Tab] Switch pane [q] Quit</Text>
        )}
      </Box>
    </Box>
  );
};
