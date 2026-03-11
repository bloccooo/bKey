# envi

A team secret manager. Secrets are stored as a [CRDT](https://automerge.org) document in a storage backend of your choice (S3, R2, WebDAV, or local). No central server required.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/bloccooo/bKey/main/install.sh | bash
```

Supports macOS (Apple Silicon & Intel) and Linux (x64). Installs to `/usr/local/bin/envi`.

## Commands

### `envi setup`

Create a new workspace or join an existing one via an invite link.

```sh
envi setup                     # create workspace
envi setup envi-invite:<token> # join workspace
```

### `envi ui`

Open the terminal UI to manage secrets, projects, and members.

```sh
envi ui
```

**Key bindings:**

| Key | Action |
|-----|--------|
| `n` | New item |
| `e` | Edit selected |
| `d` | Delete selected |
| `s` | Manage project secrets (project pane) |
| `g` | Grant access to member (members pane) |
| `i` | Generate invite link (members pane) |
| `v` | Toggle value visibility |
| `Tab` | Switch pane |
| `q` | Quit |

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
