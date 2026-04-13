use anyhow::{Context, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub struct KeychainStore;

impl KeychainStore {
    pub fn set_json<T: Serialize>(service: &str, account: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        Self::set_password(service, account, &json)
    }

    pub fn get_json<T: DeserializeOwned>(service: &str, account: &str) -> Result<T> {
        let json = Self::get_password(service, account)?;
        serde_json::from_str(&json).context("failed to decode stored keychain payload")
    }

    pub fn set_password(service: &str, account: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(service, account)
            .context("failed to create keychain entry")?;
        entry
            .set_password(value)
            .context("failed to store value in keychain")?;
        Ok(())
    }

    pub fn get_password(service: &str, account: &str) -> Result<String> {
        let entry =
            keyring::Entry::new(service, account).context("failed to create keychain entry")?;
        entry
            .get_password()
            .context("value not found in keychain")
    }

    pub fn delete(service: &str, account: &str) -> Result<()> {
        let entry =
            keyring::Entry::new(service, account).context("failed to create keychain entry")?;
        entry
            .delete_credential()
            .context("failed to delete value from keychain")?;
        Ok(())
    }
}
