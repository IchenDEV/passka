use crate::types::CredentialData;
use anyhow::{Context, Result};

const SERVICE_NAME: &str = "passka";

pub struct KeychainStore;

impl KeychainStore {
    pub fn set(name: &str, data: &CredentialData) -> Result<()> {
        let json = serde_json::to_string(data)?;
        let entry = keyring::Entry::new(SERVICE_NAME, name)
            .context("failed to create keychain entry")?;
        entry
            .set_password(&json)
            .context("failed to store credential in keychain")?;
        Ok(())
    }

    pub fn get(name: &str) -> Result<CredentialData> {
        let entry =
            keyring::Entry::new(SERVICE_NAME, name).context("failed to create keychain entry")?;
        let json = entry
            .get_password()
            .context("credential not found in keychain")?;
        let data: CredentialData = serde_json::from_str(&json)?;
        Ok(data)
    }

    pub fn get_field(name: &str, field: &str) -> Result<String> {
        let data = Self::get(name)?;
        data.fields
            .get(field)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("field '{field}' not found in credential '{name}'"))
    }

    pub fn update_field(name: &str, field: &str, value: &str) -> Result<()> {
        let mut data = Self::get(name)?;
        data.fields.insert(field.to_string(), value.to_string());
        Self::set(name, &data)
    }

    pub fn delete(name: &str) -> Result<()> {
        let entry =
            keyring::Entry::new(SERVICE_NAME, name).context("failed to create keychain entry")?;
        entry
            .delete_credential()
            .context("failed to delete credential from keychain")?;
        Ok(())
    }
}
