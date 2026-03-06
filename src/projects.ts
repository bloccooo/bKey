import * as A from "@automerge/automerge";
import { randomUUIDv7 } from "bun";
import type { Project, BKeyDocument } from "./types";

export function addProject(
  doc: A.Doc<BKeyDocument>,
  name: string,
  secretIds: string[] = [],
): A.Doc<BKeyDocument> {
  const id = randomUUIDv7();
  return A.change(doc, `add project ${name}`, (d) => {
    d.projects[id] = { id, name, secret_ids: secretIds };
  });
}

export function removeProject(
  doc: A.Doc<BKeyDocument>,
  id: string,
): A.Doc<BKeyDocument> {
  return A.change(doc, `remove project ${id}`, (d) => {
    delete d.projects[id];
  });
}

export function addSecretToProject(
  doc: A.Doc<BKeyDocument>,
  projectId: string,
  secretId: string,
): A.Doc<BKeyDocument> {
  return A.change(
    doc,
    `add secret ${secretId} to project ${projectId}`,
    (d) => {
      const project = d.projects[projectId];
      if (!project) throw new Error(`Project ${projectId} not found`);
      project.secret_ids.push(secretId);
    },
  );
}

export function updateProject(
  doc: A.Doc<BKeyDocument>,
  id: string,
  name: string,
): A.Doc<BKeyDocument> {
  return A.change(doc, `update project ${id}`, (d) => {
    const p = d.projects[id];
    if (!p) throw new Error(`Project ${id} not found`);
    p.name = name;
  });
}

export function setProjectSecrets(
  doc: A.Doc<BKeyDocument>,
  projectId: string,
  secretIds: string[],
): A.Doc<BKeyDocument> {
  return A.change(doc, `set secrets for project ${projectId}`, (d) => {
    const p = d.projects[projectId];
    if (!p) throw new Error(`Project ${projectId} not found`);
    p.secret_ids.splice(0, p.secret_ids.length, ...secretIds);
  });
}

export function listProjects(doc: A.Doc<BKeyDocument>): Project[] {
  return Object.values(doc.projects);
}
