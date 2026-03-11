# vault

> Encrypted · Serverless · Team Secret Manager

A developer-focused secrets CLI built in Rust. Store, share, and inject environment variables into processes at runtime — no server to operate, no plaintext ever written to disk.

```bash
vault run -- npm start
```

---

## How it works

vault stores an encrypted [Automerge](https://automerge.org) document in any object storage backend you configure (S3, GCS, R2, local disk, etc.). The document is the workspace — it contains everything: projects, secrets, and member public keys. Storage credentials live only in your OS keychain and are never shared.

All entity management (secrets, projects) happens through the TUI. The CLI is intentionally minimal — it exists for the `run` command and a few setup utilities.

---

## Installation

```bash
cargo install vault-cli
```

---

## Quick Start

```bash
# 1. Initialize a workspace (opens TUI wizard)
vault init

# 2. Manage secrets and projects in the TUI
vault ui

# 3. Inject secrets at runtime
vault run -- npm start
```

Drop a `.vault` file in your repo root to tie it to a project — then `vault run` needs no flags:

```toml
# .vault  (commit this — contains no secrets or credentials)
project = "myapp"
```

---

## CLI Reference

The CLI is intentionally small. All entity management lives in the TUI (`vault ui`).

```bash
vault init                    # set up a new workspace (TUI wizard)
vault ui                      # open the TUI dashboard
vault run -- <cmd>            # inject secrets into a subprocess
vault join <invite-link>      # accept a workspace invite
vault sync                    # manual push/pull
vault doctor                  # diagnose config, keychain, storage
vault completions <shell>     # generate shell completions
```

### vault run

```bash
vault run -- <command>

# Options
--project <n>    # override project (default: from .vault)
--env     <n>    # override environment (default: from .vault)
--dry-run        # print env vars that would be injected, don't run
```

Examples:

```bash
vault run -- npm start
vault run -- python manage.py runserver
vault run --project backend --env production -- ./server
vault run --dry-run                           # inspect what would be injected
```

---

## TUI

Run `vault ui` to manage everything.

```
┌─ vault ──────────────────────────────────────────────────────┐
│  Projects              Secrets                               │
│  ┌───────────────┐    ┌──────────────────────────────────┐   │
│  │ > myapp       │    │ Name          Tags      Projects  │   │
│  │   backend     │    │ DATABASE_URL  database  2         │   │
│  │   frontend    │    │ API_KEY       payments  1         │   │
│  │   staging     │    │ REDIS_URL     cache     3         │   │
│  └───────────────┘    └──────────────────────────────────┘   │
│                                                              │
│  [n] New  [e] Edit  [d] Delete  [/] Search  [?] Help         │
└──────────────────────────────────────────────────────────────┘
```

| Key | Action |
|---|---|
| `n` | Create new secret or project (context-aware) |
| `e` | Edit selected item inline |
| `d` | Delete with impact warning |
| `/` | Fuzzy search across secrets and projects |
| `Tab` | Switch focus between panes |
| `v` | Toggle secret value visibility |
| `?` | Contextual help |
| `q` | Quit |

---

## Data Model

An Automerge document **is** the workspace. There is no separate workspace entity.

```
vault.enc  (Automerge document, age-encrypted)
├── id                  plaintext uuid  (stable CRDT key)
├── name                encrypted
├── storage_config      plaintext       (needed before decryption to locate the doc)
├── members[]           plaintext       (needed to perform encryption)
├── doc_version         plaintext       (monotonic counter for rollback detection)
├── doc_signature       plaintext       (last editor signature, verified before decryption)
├── projects[]
│   ├── id              plaintext uuid  (stable CRDT key)
│   ├── name            encrypted
│   └── secret_ids[]    encrypted
└── secrets[]
    ├── id              plaintext uuid  (stable CRDT key)
    ├── name            encrypted       e.g. encrypted("DATABASE_URL")
    ├── value           encrypted
    ├── description     encrypted
    └── tags[]          encrypted
```

The only plaintext fields are those the CRDT or crypto layer strictly requires: UUIDs as stable map keys, member public keys (needed to perform encryption), storage config (needed before decryption to know where to fetch the doc), and integrity fields (version counter and signature, which must be readable before decryption to detect tampering). Everything human-readable is encrypted.

Secrets are first-class — a secret can belong to multiple projects. Updating a secret is reflected everywhere immediately. The document is stored at a well-known path in your storage bucket:

```
<bucket>/
  vault.enc              ← live workspace document
  invites/
    <uuid>.enc           ← temporary invite snapshots, deleted after join
```

---

## Key Management

Your entire setup reduces to one thing to remember: **your passphrase**.

```
passphrase + workspace_id
         │
         ▼  argon2id
    32-byte seed
         │
         ▼  ed25519
  age private key  +  age public key
```

Same passphrase on any machine → same keypair for that workspace. No key files to manage or transfer.

After first unlock, the derived key is cached in the OS keychain so subsequent commands are silent.

```
OS Keychain
├── vault/<workspace-id>/age-key      derived age private key
└── vault/<workspace-id>/storage      storage credentials
```

Neither is ever written to `.vault` or committed to any repository.

---

## Invite Flow

Inviting a teammate is a single link with no round trips. The inviter pre-encrypts a doc snapshot with a one-time key that is bundled into the link itself — the invitee needs nothing from the inviter beyond the link.

### Sending an invite (Alice)

In the TUI under Workspace → Invite, or:

```
Alice opens TUI → Workspace → Invite

vault generates a one-time keypair
vault re-encrypts current doc snapshot with:
  - all existing member keys  (existing members unaffected)
  - one-time invite key       (so the link holder can decrypt)
snapshot uploaded to: <bucket>/invites/<uuid>.enc
link generated:       vault-invite://<base64 payload>
```

The link payload contains:

```
{
  workspace_id,
  workspace_name,
  storage_config,       // provider + bucket, no credentials
  invite_private_key,   // one-time decryption key
  snapshot_path,        // invites/<uuid>.enc
  expires_at,           // 24h from generation
  inviter_public_key    // so Bob can verify who invited him
}
```

Alice shares the link however she likes — Slack, email, anywhere.

### Accepting an invite (Bob)

```bash
vault join vault-invite://<payload>

→ link decoded, expiry checked
→ snapshot fetched from storage using config in link
→ doc decrypted with invite_private_key
→ Enter your passphrase: ****
→ Bob's keypair derived from passphrase + workspace_id
→ Enter storage credentials: ****  (saved to keychain)
→ Bob adds his public key to member list
→ re-encrypts all secrets for all members including himself
→ pushes updated doc to storage
→ Done
```

Bob now has full access. Alice sees him as a member next time she syncs.

### Invite expiry and cleanup

- **Expiry** is enforced by the client — links older than 24h are rejected. Additionally, vault deletes the invite snapshot from storage after the join is detected, making the link useless even if someone else has it.
- **One-time use** is best-effort — once vault detects that Bob's key has been added to the doc, it deletes the invite snapshot. There is a small race window between Bob downloading the snapshot and the deletion being detected, but for a dev tools threat model this is an acceptable tradeoff without requiring a server.

### Revoking access

In the TUI under Workspace → Members → Remove:

```
→ member's public key removed from member list
→ all secrets re-encrypted for remaining members only
→ updated doc pushed to storage
→ revoked member's existing local copy becomes stale
   and cannot decrypt future changes
```

---

## Storage Backends

vault uses [opendal](https://opendal.apache.org) for storage. Configure during `vault init` — storage credentials are saved to the OS keychain, never to `.vault`.

**Credential lookup order:**

```
1. OS keychain        vault/<workspace-id>/storage
2. Environment vars   AWS_ACCESS_KEY_ID, GOOGLE_APPLICATION_CREDENTIALS, etc.
3. Provider chain     IAM role, gcloud auth login, Azure CLI, etc.
4. Interactive prompt → saved to keychain
```

### AWS S3

```toml
# .vault
project = "myapp"

# storage config lives in vault.enc, credentials in keychain
```

```bash
vault init
# → Choose provider: S3
# → Bucket: my-vault
# → Region: us-east-1
# → AWS Access Key ID: ****  (saved to keychain)
```

### Cloudflare R2

```bash
vault init
# → Choose provider: R2
# → Account ID: abc123
# → Bucket: my-vault
# → API Token: ****  (saved to keychain)
```

### Google Cloud Storage

```bash
vault init
# → Choose provider: GCS
# → Bucket: my-vault
# → Picks up gcloud auth automatically, or prompts for service account key
```

### Local filesystem

```bash
vault init
# → Choose provider: local
# → Path: /mnt/shared/vault
# → No credentials needed
```

---

## CI/CD

vault works naturally in CI — no daemon, no TUI, just `vault run`. Credentials are passed via environment variables which vault picks up automatically.

```yaml
# GitHub Actions
- name: Install vault
  run: cargo install vault-cli

- name: Deploy
  run: vault run --project myapp --env production -- ./deploy.sh
  env:
    VAULT_PASSPHRASE: ${{ secrets.VAULT_PASSPHRASE }}
    AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
    AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
```

The only secrets that need to live in your CI provider are `VAULT_PASSPHRASE` and storage credentials. All application secrets are managed by vault.

---

## Security

### Guarantees

- Secret values are decrypted **in memory only**, at the moment of `vault run`
- `vault.enc` is always age-encrypted before being written to storage
- **Multi-recipient encryption** — secrets are encrypted with all member public keys simultaneously, so any member can decrypt independently
- **Storage credentials never leave the keychain** — not in `.vault`, not in `vault.enc`, not in invite links
- Invite links contain a one-time key that only decrypts the invite snapshot, not the live doc
- The OS keychain stores both the derived age private key and storage credentials
- **KDF parameters are hardcoded in the client binary** — never read from the doc or storage, so a compromised bucket cannot weaken passphrase derivation
- **Member public keys are self-signed** — each member signs their own public key with their derived age key; vault rejects any unsigned or invalidly signed key, preventing a storage attacker from injecting a fake member key
- **The doc is signed by its last editor** — vault verifies the signature on every sync and rejects docs with invalid or missing signatures
- **Invite snapshots are signed by the inviter** — Bob verifies Alice's signature before loading the snapshot, preventing substitution attacks between upload and download
- **Monotonic version counter** — each client remembers the last doc version it saw and rejects older versions, preventing rollback to a pre-revocation state

### Threat Model

| Threat | Mitigation |
|---|---|
| Stolen storage bucket | `vault.enc` is encrypted at rest, useless without passphrase |
| Compromised bucket injects fake member key | Self-signed public keys — unsigned keys are rejected |
| Compromised bucket substitutes vault.enc | Doc is signed by last editor — invalid signature rejected |
| Compromised bucket substitutes invite snapshot | Invite snapshot signed by inviter — Bob verifies before loading |
| Compromised bucket serves old pre-revocation doc | Monotonic version counter — clients reject older versions |
| Leaked invite link | Expires after 24h; invite snapshot deleted after join detected |
| Committed `.vault` file | Contains only a project name, nothing sensitive |
| Leaked passphrase | Remove member via TUI → re-encrypt all secrets without their key |
| KDF parameters weakened via storage | Parameters are hardcoded in client, never read from storage |
| New device | Re-derive keypair from passphrase, re-enter storage credentials once |

### Known Limitations

**Secret names, project names, and descriptions are encrypted** alongside values. The only plaintext visible to anyone with bucket access is UUIDs (meaningless without the doc), member public keys, storage config, and integrity fields. A compromised bucket reveals nothing about your infrastructure.

**Revocation does not protect historical access.** When a member is revoked, all secrets are re-encrypted without their key. However, if they retained a copy of an older `vault.enc` snapshot, they can still decrypt it. The only way to fully close this gap is to rotate the actual secret values after revoking a member.

**All members have equal write access.** There is no cryptographic read-only role in v1 — all members who can decrypt can also write. Role-based access control is planned for a future release. If you need hard write restrictions today, use bucket-level IAM policies at your storage provider to grant read-only storage access to specific members.

---

## Implementation Roadmap

### Phase 1 — Foundation
> A working `vault run` that injects secrets from a local doc. No sync yet.

- `PROJ-001` Rust workspace with clap, tokio, serde
- `PROJ-002` Core data model: Secret, Project, doc-as-workspace
- `PROJ-003` Automerge integration — serialize/deserialize doc to binary
- `PROJ-004` Local filesystem persistence (`~/.vault/<workspace-id>.enc`)
- `PROJ-005` age encryption — encrypt/decrypt secret values in memory
- `PROJ-006` Passphrase prompt + argon2id KDF → age keypair derivation
- `PROJ-007` `vault run` — inject secrets as env vars into subprocess
- `PROJ-008` `.vault` project file parsing — auto-detect project from cwd

### Phase 2 — Key Management & OS Keychain
> One passphrase, unlocks silently after first use.

- `PROJ-009` keyring crate — cache derived age key in OS keychain
- `PROJ-010` Passphrase strength meter (zxcvbn)
- `PROJ-011` Multi-recipient age encryption for all member keys
- `PROJ-012` Invite link generation — one-time keypair, encrypted snapshot, base64 payload
- `PROJ-013` `vault join` — decode link, fetch snapshot, derive keypair, push updated doc
- `PROJ-014` Invite snapshot cleanup — delete from storage after join detected
- `PROJ-015` Member revocation — re-encrypt without removed member's key

### Phase 3 — TUI
> Full ratatui dashboard. The only way to manage secrets and projects.

- `PROJ-016` ratatui + crossterm app shell with two-pane layout
- `PROJ-017` Project pane — navigate, select
- `PROJ-018` Secret pane — masked values, tag display
- `PROJ-019` Secret creation form — name, value (masked/toggle), description, tags, projects
- `PROJ-020` Project creation form — name, description, attach existing secrets
- `PROJ-021` Inline editing for secrets and projects
- `PROJ-022` Fuzzy search with nucleo
- `PROJ-023` Impact warning dialog for destructive operations
- `PROJ-024` `vault init` wizard — workspace name, storage provider, credentials → keychain
- `PROJ-025` Workspace → Invite flow in TUI
- `PROJ-026` Workspace → Members → Remove in TUI
- `PROJ-027` Contextual help overlay `[?]`

### Phase 4 — Storage Sync (opendal)
> Push and pull the encrypted doc to any object storage provider.

- `PROJ-028` `SyncBackend` trait
- `PROJ-029` opendal backend — push/pull encrypted Automerge binary
- `PROJ-030` Credential resolution: keychain → env vars → provider chain → prompt + save
- `PROJ-031` Optimistic locking via ETags to prevent concurrent push corruption
- `PROJ-032` `vault sync` — manual push/pull
- `PROJ-033` Auto-sync on change — push after every write in TUI
- `PROJ-034` `vault export` — write decrypted secrets as `.env` to stdout

### Phase 5 — Polish
> Production-ready.

- `PROJ-035` Shell completions — bash, zsh, fish
- `PROJ-036` Environment support per project (dev / staging / production)
- `PROJ-037` `vault doctor` — diagnose config, keychain, storage connectivity
- `PROJ-038` `vault run --dry-run`
- `PROJ-039` CI/CD documentation and examples
- `PROJ-040` Homebrew formula + cargo-dist release pipeline

### Future (not in v1)
- P2P sync via iroh — real-time peer sync without a storage backend
- Repository sync mode — encrypted file committed to git with CRDT merge driver
- Hybrid mode — P2P for developers, storage for CI/CD
- Secret audit log and history

---

## Non-Goals (v1)

- P2P or repository sync (planned for v2)
- Role-based access control — read-only members, per-project permissions (planned for v2)
- Web UI
- Fine-grained per-secret ACLs
- Secret rotation or TTL-based expiry
- Integration with AWS Secrets Manager, GCP Secret Manager, etc.
- Windows native build (best-effort)

---

## License

MIT
