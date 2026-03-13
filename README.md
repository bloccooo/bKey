# Envisible [envi]

[![CI](https://github.com/bloccooo/envisible/actions/workflows/ci.yml/badge.svg)](https://github.com/bloccooo/envisible/actions/workflows/ci.yml)

A serverless secret manager for teams. Secrets are stored encrypted in a storage backend of your choice (S3, R2, WebDAV, or local) and synced across team members using a [CRDT](https://automerge.org) — no central server, no shared master password, no trust in the storage provider.

Also designed for the agentic era: credentials are scoped per terminal session, injected only into explicitly declared processes, and never accessible to tools running in other terminals — protecting against prompt injection attacks and curious agents.

## Encryption

Secrets are encrypted with AES-256-GCM using a shared workspace key (DEK). Each member holds their own copy of the DEK, wrapped with a personal X25519 key pair derived from their passphrase, workspace ID, and a random member ID via Argon2id. The passphrase never leaves the device.

Each member maintains their own Automerge document, signed with an Ed25519 key before upload. Peers reject unsigned or tampered files before merging. Member keys are authenticated with an HMAC-SHA256 keyed by the DEK, so any DEK holder can detect if a member's keys were replaced at the storage layer.

## Install

```sh
curl -fsSL https://blocco.studio/envi/install.sh | bash
```

Supports macOS (Apple Silicon & Intel) and Linux (x64). Installs to `/usr/local/bin/envi`.

## Commands

### `envi setup`

Create a new workspace or join an existing one via an invite link.

```sh
envi setup                     # create workspace
envi setup envi-invite:<token> # join workspace
```

### `envi`

Open the terminal UI to manage secrets, projects, and members.

```sh
envi
```

**Key bindings:**

| Key   | Action                                |
| ----- | ------------------------------------- |
| `n`   | New item                              |
| `e`   | Edit selected                         |
| `d`   | Delete selected                       |
| `s`   | Manage project secrets (project pane) |
| `g`   | Grant access to member (members pane) |
| `i`   | Generate invite link (members pane)   |
| `v`   | Toggle value visibility               |
| `Tab` | Switch pane                           |
| `q`   | Quit                                  |

### `envi exec`

Inject secrets as environment variables into a command.

```sh
envi exec -- node server.js
envi exec --project myapp -- node server.js
envi exec --project myapp --dry-run
```

A `.envi` file in the project root can specify the default project:

```
project = "myapp"
```

### `envi sync`

Pull the latest state from the storage backend and push local changes.

```sh
envi sync
```

### `envi logout`

Stop the key agent and clear cached credentials from RAM. The next command will prompt for the passphrase again.

Credentials are scoped per terminal session (TTY), so logging out in one terminal only affects that terminal. Other open terminals retain their cached credentials. TTY scoping also limits the blast radius of prompt injection attacks — a process running in a different terminal (e.g. an AI agent) cannot reuse credentials unlocked in your interactive session.

```sh
envi logout
```

### `envi clear`

Remove all local data: stop the agent, delete the local cache, and remove the config from the OS keychain. Use this to fully reset the installation.

```sh
envi clear
```

## Security roadmap

The current implementation uses sound cryptographic primitives (AES-256-GCM, X25519/ECIES, Argon2id) but currently has known limitations.

**Known bugs:**

- **Member name impersonation** — member names are self-declared and not verified. Nothing prevents two members from registering with the same display name, or a malicious joiner from choosing a name that mimics a legitimate member. Fixed by requiring the inviting member to countersign new members (see below).

**Planned hardening, roughly in priority order:**

- ~~**Remove passphrase persistence**~~ — done. The passphrase is never written to disk; the derived key is held only in RAM by a short-lived background agent and cleared on `envi logout`.
- ~~**Password reuse across members**~~ — fixed. Private keys are now derived from `(passphrase, workspace_id, member_id)` where `member_id` is a random UUID generated at setup. Key material is bound to a specific member identity, so knowing another member's passphrase is not sufficient to derive their key.
- ~~**Authenticated CRDT documents**~~ — done. Each member's Automerge document is signed with an Ed25519 key before being pushed to storage. All files must be signed — unsigned files are rejected. Pending members are excluded from the canonical bytes so a new member registering their public key does not invalidate the existing signature. Member public and signing keys are protected by a per-member HMAC-SHA256 keyed by the shared DEK, so key substitution is detectable by any DEK holder.
- **Genesis trust anchor** — the first time a member pulls a workspace, they have no prior state to verify signing keys against (TOFU). A future version will embed a signing key fingerprint in the invite link so the first pull can be verified against the invite.
- **Signed invite links** — invite links will be signed by the issuing member's private key. Peers will verify the signature on join, ensuring the invite was issued by a legitimate workspace member and preventing forged or tampered links.
- **Member identity verification** — new members self-register by writing their own public key into the shared document. A future version will require the inviting member to countersign the joining member's public key, preventing a malicious actor from substituting their own key during the join flow.
- **Single-use, expiring invite links** — invite links currently have no expiry and can be reused indefinitely. They will include a short-lived nonce so that replayed or leaked links cannot be used to register new members.
- **Scoped secret injection** — `envi exec` will require secrets to be explicitly declared (e.g. in the `.envi` file) rather than injecting the full workspace vault, limiting the blast radius of prompt-injection attacks against AI agents.

## Building from source

Requires Rust (stable).

```sh
cargo build --release
```
