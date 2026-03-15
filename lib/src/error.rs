use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("not a member of this vault")]
    NotAMember,

    #[error("access pending — an existing member needs to sync and grant access first")]
    AccessPending,

    #[error("encryption failed")]
    EncryptionFailed,

    #[error("decryption failed")]
    DecryptionFailed,

    #[error("invalid document signature")]
    InvalidSignature,

    #[error("key MAC verification failed for member {0}")]
    InvalidKeyMac(String),

    #[error("invalid invite token")]
    InvalidInviteLink,

    #[error("invite token expired")]
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

    #[error("no vaults configured — run: envi setup")]
    NoVaults,

#[error("secret not found: {0}")]
    SecretNotFound(String),

    #[error("keychain error: {0}")]
    Keychain(String),

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
