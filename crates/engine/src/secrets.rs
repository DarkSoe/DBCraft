use crate::{EngineError, Result};

const SERVICE: &str = "ai.storyteller.dbcraft";

pub fn store_password(profile_id: &str, password: &str) -> Result<()> {
    if password.is_empty() {
        let _ = delete_password(profile_id);
        return Ok(());
    }
    let entry = keyring::Entry::new(SERVICE, profile_id)
        .map_err(|e| EngineError::Keyring(e.to_string()))?;
    entry
        .set_password(password)
        .map_err(|e| EngineError::Keyring(e.to_string()))
}

pub fn load_password(profile_id: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE, profile_id)
        .map_err(|e| EngineError::Keyring(e.to_string()))?;
    match entry.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(EngineError::Keyring(e.to_string())),
    }
}

pub fn delete_password(profile_id: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, profile_id)
        .map_err(|e| EngineError::Keyring(e.to_string()))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(EngineError::Keyring(e.to_string())),
    }
}
