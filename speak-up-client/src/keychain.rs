use keyring::Entry;

const SERVICE_NAME: &str = "speak-up";

pub fn store_api_key(provider: &str, key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Ok(());
    }
    let entry = Entry::new(SERVICE_NAME, provider)
        .map_err(|e| format!("Failed to create keychain entry: {}", e))?;
    entry
        .set_password(key)
        .map_err(|e| format!("Failed to store API key: {}", e))?;
    tracing::info!("Stored API key for {}", provider);
    Ok(())
}

pub fn get_api_key(provider: &str) -> Option<String> {
    let entry = Entry::new(SERVICE_NAME, provider).ok()?;
    match entry.get_password() {
        Ok(key) => {
            tracing::debug!("Retrieved API key for {}", provider);
            Some(key)
        }
        Err(keyring::Error::NoEntry) => {
            tracing::debug!("No API key found for {}", provider);
            None
        }
        Err(e) => {
            tracing::warn!("Failed to get API key for {}: {}", provider, e);
            None
        }
    }
}

pub fn delete_api_key(provider: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, provider)
        .map_err(|e| format!("Failed to create keychain entry: {}", e))?;
    entry
        .delete_credential()
        .map_err(|e| format!("Failed to delete API key: {}", e))?;
    tracing::info!("Deleted API key for {}", provider);
    Ok(())
}
