use dialoguer::Password;
use envilib::error::{Error, Result};

/// Prompt for an existing passphrase (no validation — just reads it).
pub fn prompt_passphrase() -> Result<String> {
    Password::new()
        .with_prompt("Passphrase")
        .interact()
        .map_err(|e| Error::Other(e.to_string()))
}

/// Prompt for a new passphrase: validates strength and requires confirmation.
/// Loops until a strong-enough passphrase is entered and confirmed.
pub fn prompt_new_passphrase() -> Result<String> {
    loop {
        let passphrase = Password::new()
            .with_prompt("Passphrase")
            .interact()
            .map_err(|e| Error::Other(e.to_string()))?;

        if let Err(msg) = check_strength(&passphrase) {
            eprintln!("Passphrase too weak: {msg}");
            continue;
        }

        let confirmation = Password::new()
            .with_prompt("Confirm passphrase")
            .interact()
            .map_err(|e| Error::Other(e.to_string()))?;

        if passphrase != confirmation {
            eprintln!("Passphrases do not match, try again.");
            continue;
        }

        return Ok(passphrase);
    }
}

/// Returns Ok if the passphrase is strong enough, or Err with a human-readable reason.
fn check_strength(passphrase: &str) -> std::result::Result<(), &'static str> {
    if passphrase.len() < 12 {
        return Err("must be at least 12 characters");
    }

    let has_lower = passphrase.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = passphrase.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = passphrase.chars().any(|c| c.is_ascii_digit());
    let has_special = passphrase.chars().any(|c| !c.is_alphanumeric());

    let score = [has_lower, has_upper, has_digit, has_special]
        .iter()
        .filter(|&&v| v)
        .count();

    if score < 3 {
        return Err("must contain at least 3 of: lowercase, uppercase, digits, special characters");
    }

    Ok(())
}
