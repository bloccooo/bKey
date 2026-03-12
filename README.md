# Envisible [envi]

A team secret manager built for the agentic era. Secrets are stored encrypted in a storage backend of your choice (S3, R2, WebDAV, or local) and synced across team members using a [CRDT](https://automerge.org). No central server required.

Designed to limit secret exposure when working alongside AI agents: credentials are scoped per terminal session, injected only into explicitly declared processes, and never accessible to tools running in other terminals — reducing the blast radius of prompt injection attacks.

## Encryption

Secret values are encrypted with AES-256-GCM using a shared workspace key. That key is wrapped individually for each member via X25519 + ECIES, derived from their passphrase using argon2id. The passphrase never leaves the device.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/bloccooo/envisible/main/install.sh | bash
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

### `envi run`

Inject secrets as environment variables into a command.

```sh
envi run -- node server.js
envi run --project myapp -- node server.js
envi run --project myapp --dry-run
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

- **Password reuse across members** — because private keys are derived purely from `(passphrase, workspace_id)`, a member who knows another member's passphrase can derive their private key and decrypt secrets as them. There is no binding between a member's identity and their key material beyond the passphrase itself. Fixed by member identity verification (see below).
- **Member name impersonation** — member names are self-declared and not verified. Nothing prevents two members from registering with the same name, or a malicious joiner from choosing a name that mimics a legitimate member. Fixed by requiring the inviting member to countersign new members (see below).

**Planned hardening, roughly in priority order:**

- ~~**Remove passphrase persistence**~~ — done. The passphrase is never written to disk; the derived key is held only in RAM by a short-lived background agent and cleared on `envi logout`.
- **Signed invite links** — invite links will be signed by the issuing member's private key. Peers will verify the signature on join, ensuring the invite was issued by a legitimate workspace member and preventing forged or tampered links.
- **Member identity verification** — new members self-register by writing their own public key into the shared document. A future version will require the inviting member to countersign the joining member's public key, preventing a malicious actor from substituting their own key during the join flow.
- **Single-use, expiring invite links** — invite links currently have no expiry and can be reused indefinitely. They will include a short-lived nonce so that replayed or leaked links cannot be used to register new members.
- **Authenticated CRDT documents** — each member's automerge document will be signed with their private key. Peers will reject documents with invalid signatures, preventing a storage-level attacker from injecting rogue members or wiping secrets.
- **Scoped secret injection** — `envi run` will require secrets to be explicitly declared (e.g. in the `.envi` file) rather than injecting the full workspace vault, limiting the blast radius of prompt-injection attacks against AI agents.

## Building from source

Requires Rust (stable).

```sh
cargo build --release
```
