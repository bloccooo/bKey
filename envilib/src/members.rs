use crate::{
    crypto::{decrypt_field, encrypt_field, generate_dek, wrap_dek},
    error::{Error, Result},
    types::EnviDocument,
};
use automerge::AutoCommit;
use autosurgeon::{hydrate, reconcile};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

/// Remove a member and rotate the DEK so they can no longer decrypt secrets.
/// Returns the new DEK — callers must update their session accordingly.
pub fn remove_member(
    doc: &mut AutoCommit,
    current_dek: &[u8; 32],
    member_id: &str,
) -> Result<[u8; 32]> {
    let mut state: EnviDocument = hydrate(doc)?;

    if !state.members.contains_key(member_id) {
        return Err(Error::Other(format!("member not found: {member_id}")));
    }

    state.members.remove(member_id);
    let new_dek = rotate_dek_in_state(&mut state, current_dek)?;
    reconcile(doc, &state)?;
    Ok(new_dek)
}

/// Manually rotate the DEK without removing any member.
/// Returns the new DEK — callers must update their session accordingly.
pub fn rotate_dek(doc: &mut AutoCommit, current_dek: &[u8; 32]) -> Result<[u8; 32]> {
    let mut state: EnviDocument = hydrate(doc)?;
    let new_dek = rotate_dek_in_state(&mut state, current_dek)?;
    reconcile(doc, &state)?;
    Ok(new_dek)
}

/// Core rotation logic: decrypt all secrets, generate a new DEK, re-encrypt
/// secrets and re-wrap the DEK for all active members in `state`.
/// Returns the new DEK.
fn rotate_dek_in_state(state: &mut EnviDocument, current_dek: &[u8; 32]) -> Result<[u8; 32]> {
    // Decrypt all secrets with the current DEK.
    // Store as (id, name, value, description, tags_json).
    let plaintexts: Vec<(String, String, String, String, String)> = state
        .secrets
        .values()
        .map(|s| {
            Ok((
                s.id.clone(),
                decrypt_field(&s.name, current_dek)?,
                decrypt_field(&s.value, current_dek)?,
                decrypt_field(&s.description, current_dek)?,
                decrypt_field(&s.tags, current_dek)?,
            ))
        })
        .collect::<Result<_>>()?;

    let new_dek = generate_dek();

    // Re-encrypt all secrets with the new DEK.
    for (id, name, value, description, tags_json) in &plaintexts {
        let secret = state.secrets.get_mut(id).expect("secret must exist");
        secret.name = encrypt_field(name, &new_dek)?;
        secret.value = encrypt_field(value, &new_dek)?;
        secret.description = encrypt_field(description, &new_dek)?;
        secret.tags = encrypt_field(tags_json, &new_dek)?;
    }

    // Re-wrap the new DEK for every remaining active member.
    // Pending members (empty wrapped_dek) are left pending — they need
    // to be re-granted by an active member using the new DEK.
    for member in state.members.values_mut() {
        if member.wrapped_dek.is_empty() {
            continue;
        }
        let pub_key_bytes = B64
            .decode(&member.public_key)
            .map_err(|_| Error::DecryptionFailed)?;
        let pub_key: [u8; 32] = pub_key_bytes
            .try_into()
            .map_err(|_| Error::DecryptionFailed)?;
        member.wrapped_dek = wrap_dek(&new_dek, &pub_key)?;
    }

    Ok(new_dek)
}
