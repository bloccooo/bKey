import * as A from "@automerge/automerge";
import type { BKeyDocument } from "./types";
import {
  backendFromConfig,
  type StorageBackend,
  type StorageConfig,
} from "./storage";
import isOnline from "is-online";
import envPaths from "env-paths";
import { getPublicKey, unwrapDek } from "./crypto";

const ROOT = "_bkey";
const DOC_EXTENSION = "bkey.enc";
const REMOTE_TIMEOUT_MS = 5000;

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T | null> {
  return Promise.race([
    promise,
    new Promise<null>((resolve) => setTimeout(() => resolve(null), ms)),
  ]);
}

export type StoreConfig = {
  workspaceId: string;
  memberId: string;
  storage: StorageConfig;
};

export class Store {
  private config: StoreConfig;
  private remoteBackend: StorageBackend;
  private localBackend: StorageBackend;

  constructor(config: StoreConfig) {
    this.config = config;
    this.remoteBackend = backendFromConfig(config.storage);
    this.localBackend = backendFromConfig({
      type: "fs",
      root: envPaths("bkey").cache,
    });
  }

  private getPullPath() {
    return `${ROOT}/${this.config.workspaceId}`;
  }

  private getPushPath() {
    return `${ROOT}/${this.config.workspaceId}/${this.config.memberId}.${DOC_EXTENSION}`;
  }

  private mergeFilesIntoDoc(files: Uint8Array<ArrayBufferLike>[]) {
    const docs = files.map((f) => A.load<BKeyDocument>(f));
    return docs.reduce((acc, doc) => A.merge(acc, doc));
  }

  private async pullRemote() {
    const path = this.getPullPath();
    const [, files] = await Promise.all([
      isOnline({ timeout: 1000 }).then((c) => {
        if (!c) throw new Error("offline");
      }),
      withTimeout(this.remoteBackend.pull(path), REMOTE_TIMEOUT_MS),
    ]);
    if (!files || files.length === 0) return null;

    return this.mergeFilesIntoDoc(files);
  }

  private async pullLocal() {
    const path = this.getPullPath();
    const files = await this.localBackend.pull(path);
    if (files.length === 0) return null;

    return this.mergeFilesIntoDoc(files);
  }

  async pull() {
    const localDoc = await this.pullLocal();

    let remoteDoc;

    try {
      remoteDoc = await this.pullRemote();
    } catch (e) {
      console.log(e);
      remoteDoc = null;
    }

    let doc;

    if (remoteDoc && localDoc) {
      doc = A.merge(remoteDoc, localDoc);
    } else if (!remoteDoc && !localDoc) {
      doc = A.init<BKeyDocument>();
      doc = A.change(doc, "init workspace", (d) => {
        d.id = this.config.workspaceId;
        d.name = "my-workspace";
        d.doc_version = 0;
        d.members = {};
        d.projects = {};
        d.secrets = {};
      });
    } else if (!remoteDoc && localDoc) {
      doc = localDoc;
    } else if (!localDoc && remoteDoc) {
      doc = remoteDoc;
    }

    return doc!;
  }

  async persist(doc: A.Doc<BKeyDocument>) {
    const binary = A.save(doc);
    const path = this.getPushPath();

    const pushRemote = Promise.all([
      isOnline({ timeout: 1000 }).then((c) => {
        if (!c) throw new Error("offline");
      }),
      withTimeout(this.remoteBackend.push(path, binary), REMOTE_TIMEOUT_MS),
    ]).catch((e) => {
      console.log(e);
      return null;
    });

    await Promise.all([pushRemote, this.localBackend.push(path, binary)]);

    return doc;
  }
}

export type Session = {
  memberId: string;
  dek: Uint8Array;
};

export async function unlock(
  doc: A.Doc<BKeyDocument>,
  privateKey: Uint8Array,
): Promise<{ session: Session; doc: A.Doc<BKeyDocument> }> {
  const publicKey = getPublicKey(privateKey);
  const pubKeyB64 = Buffer.from(publicKey).toString("base64");

  const member = Object.values(doc.members).find(
    (m) => m.publicKey === pubKeyB64,
  );
  if (!member) throw new Error("Not a member of this workspace.");
  if (!member.wrappedDek)
    throw new Error("Access pending — an existing member needs to sync first.");

  const dek = unwrapDek(member.wrappedDek, privateKey);
  const session: Session = { memberId: member.id, dek };

  return { session, doc };
}
