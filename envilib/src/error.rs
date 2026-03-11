use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("not a member of this workspace")]
    NotAMember,

    #[error("access pending — an existing member needs to sync and grant access first")]
    AccessPending,

    #[error("encryption failed")]
    EncryptionFailed,

    #[error("decryption failed")]
    DecryptionFailed,

    #[error("invalid invite link")]
    InvalidInviteLink,

    #[error("invite link expired")]
    InviteLinkExpired,

    #[error("automerge error: {0}")]
    Automerge(#[from] automerge::AutomergeError),

    #[error("autosurgeon error: {0}")]
    Autosurgeon(String),

    #[error("storage error: {0}")]
    Storage(#[from] opendal::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("no config found — run: envi setup")]
    NoConfig,

    #[error("no workspaces configured — run: envi setup")]
    NoWorkspaces,

    #[error("project not found: {0}")]
    ProjectNotFound(String),

    #[error("secret not found: {0}")]
    SecretNotFound(String),

    #[error("{0}")]
    Other(String),
}

impl From<autosurgeon::HydrateError> for Error {
    fn from(e: autosurgeon::HydrateError) -> Self {
        Error::Autosurgeon(e.to_string())
    }
}

impl From<autosurgeon::ReconcileError> for Error {
    fn from(e: autosurgeon::ReconcileError) -> Self {
        Error::Autosurgeon(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
