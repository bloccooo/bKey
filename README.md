# bKey

An encrypted, serverless team secret manager. Secrets are stored as an [Automerge](https://automerge.org) CRDT document in a backend of your choice (local, S3, R2, WebDAV). Credentials are kept in your system keychain — never in config files.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/bloccooo/bKey/main/install.sh | bash
```

Supports macOS (Apple Silicon & Intel) and Linux (x64). The binary is installed to `/usr/local/bin/bkey`.

> **Linux note:** bKey uses your system keychain via [libsecret](https://wiki.gnome.org/Projects/Libsecret). Install it first if needed:
> ```sh
> sudo apt install libsecret-1-0   # Debian/Ubuntu
> sudo dnf install libsecret       # Fedora
> ```

## Usage

### `bkey init`

Interactive setup wizard. Configures a storage backend and saves credentials to your system keychain.

```sh
bkey init
```

### `bkey ui`

Opens the terminal UI to manage secrets and projects.

```sh
bkey ui
```

**Key bindings:**

| Key | Action |
|-----|--------|
| `n` | New secret / project |
| `e` | Edit selected |
| `d` | Delete selected |
| `s` | Manage project secrets (project pane) |
| `v` | Toggle secret value visibility |
| `Tab` | Switch between Projects / Secrets pane |
| `q` | Quit |

### `bkey run`

Inject secrets as environment variables into a command.

```sh
# Using a .bkey file in the current directory
bkey run -- env

# Specify project explicitly
bkey run --project myapp -- node server.js

# Preview what would be injected
bkey run --project myapp --dry-run
```

A `.bkey` file in your project root can specify the default project:

```
project = "myapp"
```

## Building from source

Requires [Bun](https://bun.sh).

```sh
bun install
bun run build   # produces ./bkey
```
