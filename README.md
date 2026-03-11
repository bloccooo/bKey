# Envisible [envi]

A team secret manager. Secrets are stored encrypted in a storage backend of your choice (S3, R2, WebDAV, or local) and synced across team members using a [CRDT](https://automerge.org). No central server required.

## Encryption

Each workspace has a single data encryption key (DEK) used to encrypt all secret values with AES-256-GCM. The DEK is never stored in the clear — it is wrapped individually for each member using their public key (X25519 + ECIES), so only someone with the corresponding passphrase can decrypt it. The passphrase itself never leaves the device.

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

## Building from source

Requires Rust (stable).

```sh
cargo build --release
```
